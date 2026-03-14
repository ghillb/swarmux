mod beads;
mod cli;
mod config;
mod id;
mod model;
mod runtime;
mod schema;
mod store;
mod validation;

use anyhow::{Context, Result, anyhow};
use clap::Parser;
use cli::{
    Cli, Commands, DispatchArgs, DispatchMode, FailArgs, IdArgs, ListArgs, LogsArgs, NotifyArgs,
    OutputFormat, OverviewScope, PruneArgs, SendArgs, ShowArgs, StateArgs, StopArgs, SubmitArgs,
    WatchArgs,
};
use config::{AppConfig, TaskRuntime};
use model::{DryRunSubmitResponse, SubmitPayload, TaskMode, TaskOrigin, TaskRecord, TaskState};
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::path::Path;
use std::thread;
use std::time::Duration;
use store::{Store, require_task_id};

#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
struct NotifyState {
    delivered: BTreeMap<String, String>,
}

#[derive(Clone)]
struct Notification {
    task: TaskRecord,
    output_excerpt: Option<String>,
    token_count: Option<String>,
}

struct NotifyOutcome {
    reconciled: usize,
    notifications: Vec<Notification>,
}

pub fn run() -> Result<()> {
    let cli = Cli::parse();
    let config = AppConfig::from_env()?;
    let store = Store::new(config);

    match cli.command {
        Commands::Schema => emit(&cli.output, &schema::schema_json()),
        Commands::Doctor => run_doctor(&store, cli.output),
        Commands::Init => {
            match store.paths().backend {
                config::BackendKind::Files => store.ensure_layout()?,
                config::BackendKind::Beads => beads::init(store.paths())?,
            }
            emit(
                &cli.output,
                &json!({"ok": true, "home": store.paths().home.display().to_string()}),
            )
        }
        Commands::Paths => emit(&cli.output, &store.paths().paths_info()),
        Commands::Submit(args) => run_submit(&store, cli.output, args),
        Commands::Start(args) => run_start(&store, cli.output, args),
        Commands::Delegate(args) => run_delegate(&store, cli.output, args),
        Commands::Dispatch(args) => run_dispatch(&store, cli.output, args),
        Commands::List(args) => run_list(&store, cli.output, args),
        Commands::Show(args) => run_show(&store, cli.output, args),
        Commands::Logs(args) => run_logs(&store, cli.output, args),
        Commands::Notify(args) => run_notify(&store, cli.output, args),
        Commands::Watch(args) => run_watch(&store, cli.output, args),
        Commands::Send(args) => run_send(&store, cli.output, args),
        Commands::Attach(args) => run_attach(&store, args),
        Commands::Stop(args) => run_stop(&store, cli.output, args),
        Commands::Reconcile => run_reconcile(&store, cli.output),
        Commands::Prune(args) => run_prune(&store, cli.output, args),
        Commands::Overview(args) => run_overview(&store, cli.output, args),
        Commands::Done(args) => {
            run_state_update(&store, cli.output, args, TaskState::Succeeded, None)
        }
        Commands::Fail(args) => run_fail(&store, cli.output, args),
    }
}

fn run_submit(store: &Store, output: OutputFormat, args: SubmitArgs) -> Result<()> {
    let payload = read_submit_payload(&args)?;
    validation::validate_submit_payload(&payload)?;

    if args.dry_run {
        return emit(
            &output,
            &DryRunSubmitResponse {
                ok: true,
                dry_run: true,
                task: payload,
            },
        );
    }

    let task = match store.paths().backend {
        config::BackendKind::Files => store.submit(payload)?,
        config::BackendKind::Beads => beads::submit(store.paths(), payload)?,
    };
    emit(&output, &task)
}

fn run_start(store: &Store, output: OutputFormat, args: IdArgs) -> Result<()> {
    require_task_id(&args.id)?;
    let task = get_task(store, &args.id)?;
    let previous = task.state.clone();
    let task = runtime::start_task(&task)?;
    match store.paths().backend {
        config::BackendKind::Files => store.overwrite(&task, previous, task.reason.clone())?,
        config::BackendKind::Beads => beads::set_state(
            store.paths(),
            &task.id,
            task.state.clone(),
            task.reason.clone(),
            task.last_error.clone(),
        )
        .map(|_| ())?,
    }
    emit(&output, &task)
}

fn run_delegate(store: &Store, output: OutputFormat, args: SubmitArgs) -> Result<()> {
    let payload = read_submit_payload(&args)?;
    run_delegate_payload(store, output, payload, args.dry_run, "delegate")
}

fn run_dispatch(store: &Store, output: OutputFormat, args: DispatchArgs) -> Result<()> {
    let dry_run = args.dry_run;
    let payload = submit_payload_from_dispatch(store.paths(), args)?;
    run_delegate_payload(store, output, payload, dry_run, "dispatch")
}

fn read_submit_payload(args: &SubmitArgs) -> Result<SubmitPayload> {
    let raw = match (&args.json, &args.json_file) {
        (Some(raw), None) => raw.clone(),
        (None, Some(path)) => std::fs::read_to_string(path)
            .with_context(|| format!("failed to read payload file: {}", path.display()))?,
        (None, None) => return Err(anyhow!("submit requires --json or --json-file")),
        (Some(_), Some(_)) => {
            return Err(anyhow!("submit accepts only one of --json or --json-file"));
        }
    };

    serde_json::from_str::<SubmitPayload>(&raw).context("failed to parse submit payload JSON")
}

fn emit<T: serde::Serialize>(output: &OutputFormat, value: &T) -> Result<()> {
    match output {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string(value)?);
        }
        OutputFormat::Text => {
            println!("{}", serde_json::to_string_pretty(value)?);
        }
    }

    Ok(())
}

fn run_list(store: &Store, output: OutputFormat, args: ListArgs) -> Result<()> {
    let tasks = list_tasks(store)?;
    let payload = Value::Array(
        tasks
            .into_iter()
            .map(serde_json::to_value)
            .collect::<serde_json::Result<Vec<_>>>()?,
    );
    let payload = json!({ "tasks": project_payload(payload, args.fields.as_deref())? });
    emit(&output, &payload)
}

fn run_show(store: &Store, output: OutputFormat, args: ShowArgs) -> Result<()> {
    require_task_id(&args.id)?;
    let task = get_task(store, &args.id)?;
    let payload = serde_json::to_value(task)?;
    let payload = project_payload(payload, args.fields.as_deref())?;
    emit(&output, &payload)
}

fn run_logs(store: &Store, output: OutputFormat, args: LogsArgs) -> Result<()> {
    require_task_id(&args.id)?;
    let task = get_task(store, &args.id)?;
    let text = runtime::read_logs(&task, args.raw, args.lines)?;
    emit(
        &output,
        &json!({
            "id": task.id,
            "state": task.state,
            "text": text,
        }),
    )
}

fn run_notify(store: &Store, output: OutputFormat, args: NotifyArgs) -> Result<()> {
    let outcome = collect_notifications(store, args.tmux, args.show_tokens)?;
    match output {
        OutputFormat::Json => emit(&output, &notify_value(&outcome)),
        OutputFormat::Text => emit_watch_tick(&output, &outcome, args.show_tokens),
    }
}

fn run_watch(store: &Store, output: OutputFormat, args: WatchArgs) -> Result<()> {
    let mut iterations = 0u64;

    loop {
        let outcome = collect_notifications(store, args.tmux, args.show_tokens)?;
        emit_watch_tick(&output, &outcome, args.show_tokens)?;

        iterations += 1;
        if args.max_iterations.is_some_and(|max| iterations >= max) {
            return Ok(());
        }

        thread::sleep(Duration::from_millis(args.interval_ms));
    }
}

fn run_send(store: &Store, output: OutputFormat, args: SendArgs) -> Result<()> {
    require_task_id(&args.id)?;
    let task = get_task(store, &args.id)?;
    runtime::send_input(&task, &args.input)?;
    emit(&output, &json!({"ok": true, "id": task.id}))
}

fn run_attach(store: &Store, args: IdArgs) -> Result<()> {
    require_task_id(&args.id)?;
    let task = get_task(store, &args.id)?;
    runtime::attach_task(&task)
}

fn run_stop(store: &Store, output: OutputFormat, args: StopArgs) -> Result<()> {
    require_task_id(&args.id)?;
    let task = get_task(store, &args.id)?;
    if args.kill {
        runtime::kill_task(&task)?;
        let updated = set_state_backend(
            store,
            &task.id,
            TaskState::Canceled,
            args.reason
                .unwrap_or_else(|| "manual_stop_kill".to_string()),
            None,
        )?;
        return emit(&output, &updated);
    }

    runtime::interrupt_task(&task)?;
    let updated = set_state_backend(
        store,
        &task.id,
        TaskState::WaitingInput,
        args.reason
            .unwrap_or_else(|| "manual_stop_interrupt".to_string()),
        None,
    )?;
    emit(&output, &updated)
}

fn run_reconcile(store: &Store, output: OutputFormat) -> Result<()> {
    let updated = reconcile_store(store)?;
    emit(&output, &json!({"updated": updated}))
}

fn run_prune(store: &Store, output: OutputFormat, args: PruneArgs) -> Result<()> {
    let tasks = list_tasks(store)?;
    let mut removed = 0usize;
    let mut killed = 0usize;
    for task in tasks {
        let outcome = runtime::prune(&task, args.apply)?;
        removed += outcome.worktree_removed;
        killed += outcome.session_killed;
    }
    emit(
        &output,
        &json!({
            "apply": args.apply,
            "worktree_removed": removed,
            "session_killed": killed,
        }),
    )
}

fn run_state_update(
    store: &Store,
    output: OutputFormat,
    args: StateArgs,
    state: TaskState,
    last_error: Option<String>,
) -> Result<()> {
    require_task_id(&args.id)?;
    let task = set_state_backend(store, &args.id, state, args.reason, last_error)?;
    emit(&output, &task)
}

fn run_fail(store: &Store, output: OutputFormat, args: FailArgs) -> Result<()> {
    require_task_id(&args.id)?;
    let task = set_state_backend(
        store,
        &args.id,
        TaskState::Failed,
        args.reason,
        Some(args.error),
    )?;
    emit(&output, &task)
}

fn run_doctor(store: &Store, output: OutputFormat) -> Result<()> {
    let mut checks = vec![
        ("git", command_available("git")),
        ("tmux", command_available("tmux")),
    ];
    match store.paths().backend {
        config::BackendKind::Files => checks.push(("backend=files", true)),
        config::BackendKind::Beads => checks.push(("backend=beads", beads::doctor().is_ok())),
    }

    match output {
        OutputFormat::Json => emit(
            &output,
            &json!({
                "checks": checks
                    .iter()
                    .map(|(name, ok)| json!({"name": name, "ok": ok}))
                    .collect::<Vec<_>>()
            }),
        ),
        OutputFormat::Text => {
            for (name, ok) in checks {
                println!("[{}] {}", if ok { "ok" } else { "missing" }, name);
            }
            Ok(())
        }
    }
}

fn run_overview(store: &Store, output: OutputFormat, args: cli::OverviewArgs) -> Result<()> {
    if args.title {
        println!(
            "Swarmux - {} - {}",
            store.paths().paths_info().backend,
            chrono::Utc::now()
        );
        return Ok(());
    }

    let tasks = match store.paths().backend {
        config::BackendKind::Files => store.list()?,
        config::BackendKind::Beads => beads::list(store.paths())?,
    };

    let counts = json!({
        "total": tasks.len(),
        "queued": tasks.iter().filter(|task| task.state == TaskState::Queued).count(),
        "running": tasks.iter().filter(|task| task.state == TaskState::Running).count(),
        "waiting_input": tasks.iter().filter(|task| task.state == TaskState::WaitingInput).count(),
        "succeeded": tasks.iter().filter(|task| task.state == TaskState::Succeeded).count(),
        "failed": tasks.iter().filter(|task| task.state == TaskState::Failed).count(),
        "canceled": tasks.iter().filter(|task| task.state == TaskState::Canceled).count(),
    });
    let filtered_tasks = tasks
        .into_iter()
        .filter(|task| overview_scope_matches(&task.state, &args.scope))
        .collect::<Vec<_>>();

    match output {
        OutputFormat::Json => emit(
            &output,
            &json!({
                "counts": counts,
                "scope": overview_scope_label(&args.scope),
                "tasks": filtered_tasks,
            }),
        ),
        OutputFormat::Text => {
            println!();
            println!(
                "tasks: total={} queued={} running={} waiting_input={} succeeded={} failed={} canceled={}",
                counts["total"],
                counts["queued"],
                counts["running"],
                counts["waiting_input"],
                counts["succeeded"],
                counts["failed"],
                counts["canceled"],
            );
            for task in filtered_tasks {
                println!("{} {} {}", task.id, state_label(&task.state), task.title);
            }
            Ok(())
        }
    }
}

fn overview_scope_matches(state: &TaskState, scope: &OverviewScope) -> bool {
    match scope {
        OverviewScope::Terminal => state.is_terminal(),
        OverviewScope::NonTerminal => !state.is_terminal(),
        OverviewScope::All => true,
    }
}

fn overview_scope_label(scope: &OverviewScope) -> &'static str {
    match scope {
        OverviewScope::Terminal => "terminal",
        OverviewScope::NonTerminal => "non_terminal",
        OverviewScope::All => "all",
    }
}

fn set_state_backend(
    store: &Store,
    id: &str,
    state: TaskState,
    reason: String,
    last_error: Option<String>,
) -> Result<model::TaskRecord> {
    match store.paths().backend {
        config::BackendKind::Files => store.set_state(id, state, reason, last_error),
        config::BackendKind::Beads => {
            beads::set_state(store.paths(), id, state, reason, last_error)
        }
    }
}

fn get_task(store: &Store, id: &str) -> Result<model::TaskRecord> {
    match store.paths().backend {
        config::BackendKind::Files => store.get(id),
        config::BackendKind::Beads => beads::show(store.paths(), id),
    }
}

fn list_tasks(store: &Store) -> Result<Vec<model::TaskRecord>> {
    match store.paths().backend {
        config::BackendKind::Files => store.list(),
        config::BackendKind::Beads => beads::list(store.paths()),
    }
}

fn run_delegate_payload(
    store: &Store,
    output: OutputFormat,
    payload: SubmitPayload,
    dry_run: bool,
    command_name: &str,
) -> Result<()> {
    validation::validate_submit_payload(&payload)?;

    if dry_run {
        return emit(
            &output,
            &json!({
                "ok": true,
                "dry_run": true,
                "command": command_name,
                "task": payload,
            }),
        );
    }

    let submitted = match store.paths().backend {
        config::BackendKind::Files => store.submit(payload)?,
        config::BackendKind::Beads => beads::submit(store.paths(), payload)?,
    };
    let previous = submitted.state.clone();
    let started = runtime::start_task(&submitted)?;
    match store.paths().backend {
        config::BackendKind::Files => {
            store.overwrite(&started, previous, started.reason.clone())?
        }
        config::BackendKind::Beads => {
            beads::set_state(
                store.paths(),
                &started.id,
                started.state.clone(),
                started.reason.clone(),
                started.last_error.clone(),
            )?;
        }
    }
    emit(
        &output,
        &json!({
            "submitted": submitted,
            "started": started,
        }),
    )
}

fn submit_payload_from_dispatch(config: &AppConfig, args: DispatchArgs) -> Result<SubmitPayload> {
    if args.connected {
        return connected_submit_payload_from_dispatch(config, args);
    }

    if args.prompt.is_some() {
        return Err(anyhow!("--prompt requires --connected"));
    }
    if args.pane_id.is_some() {
        return Err(anyhow!("--pane-id requires --connected"));
    }
    if args.agent.is_some() {
        return Err(anyhow!("--agent requires --connected"));
    }
    if args.command.is_empty() {
        return Err(anyhow!("dispatch requires a command after --"));
    }

    let runtime = dispatch_runtime(None, &args);
    let title = args
        .title
        .unwrap_or_else(|| default_dispatch_title(&args.command));
    Ok(SubmitPayload {
        title,
        repo_ref: args
            .repo_ref
            .ok_or_else(|| anyhow!("dispatch requires --repo-ref"))?,
        repo_root: args
            .repo_root
            .ok_or_else(|| anyhow!("dispatch requires --repo-root"))?,
        mode: match args.mode {
            DispatchMode::Auto => TaskMode::Auto,
            DispatchMode::Manual => TaskMode::Manual,
        },
        runtime,
        worktree: args.worktree,
        session: args.session,
        command: args.command,
        priority: args.priority,
        external_ref: args.external_ref,
        origin: None,
    })
}

fn default_dispatch_title(command: &[String]) -> String {
    let joined = command.join(" ");
    let trimmed = joined.trim();
    if trimmed.is_empty() {
        return "dispatch task".to_string();
    }

    let max_chars = 80usize;
    let truncated = trimmed.chars().take(max_chars).collect::<String>();
    if trimmed.chars().count() > max_chars {
        format!("{truncated}...")
    } else {
        truncated
    }
}

fn connected_submit_payload_from_dispatch(
    config: &AppConfig,
    args: DispatchArgs,
) -> Result<SubmitPayload> {
    if matches!(args.mode, DispatchMode::Manual) {
        return Err(anyhow!("--connected supports only --mode auto"));
    }
    if args.worktree.is_some() || args.session.is_some() {
        return Err(anyhow!(
            "--connected does not accept --worktree or --session"
        ));
    }
    if args.repo_ref.is_some() || args.repo_root.is_some() {
        return Err(anyhow!(
            "--connected does not accept --repo-ref or --repo-root"
        ));
    }
    if args.agent.is_some() && !args.command.is_empty() {
        return Err(anyhow!(
            "--connected does not accept both --agent and an explicit command prefix"
        ));
    }

    let mut command = resolve_connected_command(config, &args)?;
    let prompt = args
        .prompt
        .clone()
        .ok_or_else(|| anyhow!("--connected requires --prompt"))?;

    let pane = runtime::current_pane_context(args.pane_id.as_deref())?;
    let repo_root = infer_repo_root(&pane.pane_current_path)?;
    let repo_ref = infer_repo_ref(&repo_root);
    command.push(prompt.clone());
    let runtime = dispatch_runtime(Some(config.settings.connected.runtime), &args);

    Ok(SubmitPayload {
        title: args.title.unwrap_or(prompt),
        repo_ref,
        repo_root,
        mode: TaskMode::Auto,
        runtime,
        worktree: None,
        session: None,
        command,
        priority: args.priority,
        external_ref: args.external_ref,
        origin: Some(TaskOrigin {
            pane_id: pane.pane_id,
            session_name: pane.session_name,
            window_id: pane.window_id,
            window_name: pane.window_name,
            pane_current_path: pane.pane_current_path,
        }),
    })
}

fn resolve_connected_command(config: &AppConfig, args: &DispatchArgs) -> Result<Vec<String>> {
    if !args.command.is_empty() {
        return Ok(args.command.clone());
    }

    if let Some(agent) = &args.agent {
        return command_for_agent(config, agent);
    }

    if let Some(agent) = &config.settings.connected.agent {
        return command_for_agent(config, agent);
    }

    if !config.settings.connected.command.is_empty() {
        return Ok(config.settings.connected.command.clone());
    }

    Err(anyhow!(
        "--connected requires a command prefix after --, [connected].command, or a configured agent"
    ))
}

fn command_for_agent(config: &AppConfig, agent: &str) -> Result<Vec<String>> {
    let entry = config
        .settings
        .agents
        .get(agent)
        .ok_or_else(|| anyhow!("unknown agent in config.toml: {agent}"))?;

    if entry.command.is_empty() {
        return Err(anyhow!("agent command is empty in config.toml: {agent}"));
    }

    Ok(entry.command.clone())
}

fn dispatch_runtime(default_runtime: Option<TaskRuntime>, args: &DispatchArgs) -> TaskRuntime {
    if let Some(runtime) = args.runtime {
        return runtime;
    }
    if args.mirrored {
        return TaskRuntime::Mirrored;
    }

    default_runtime.unwrap_or(TaskRuntime::Headless)
}

fn infer_repo_root(path: &str) -> Result<String> {
    let output = std::process::Command::new("git")
        .args(["-C", path, "rev-parse", "--show-toplevel"])
        .output()
        .context("failed to run git rev-parse")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(anyhow!("connected dispatch requires a git repo: {stderr}"));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn infer_repo_ref(repo_root: &str) -> String {
    std::path::Path::new(repo_root)
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("repo")
        .to_string()
}

fn reconcile_store(store: &Store) -> Result<usize> {
    let mut tasks = list_tasks(store)?;
    let outcome = runtime::reconcile(
        &mut tasks,
        &store.paths().locks_dir().join("reconcile.lock"),
    )?;
    for task in &tasks {
        let current = get_task(store, &task.id)?;
        if current.updated_at != task.updated_at {
            match store.paths().backend {
                config::BackendKind::Files => {
                    store.overwrite(task, current.state, task.reason.clone())?
                }
                config::BackendKind::Beads => beads::set_state(
                    store.paths(),
                    &task.id,
                    task.state.clone(),
                    task.reason.clone(),
                    task.last_error.clone(),
                )
                .map(|_| ())?,
            };
        }
    }
    Ok(outcome.updated)
}

fn load_notify_state(path: &Path) -> Result<NotifyState> {
    if !path.exists() {
        return Ok(NotifyState::default());
    }

    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read notify state: {}", path.display()))?;
    serde_json::from_str(&raw)
        .with_context(|| format!("failed to parse notify state: {}", path.display()))
}

fn save_notify_state(path: &Path, state: &NotifyState) -> Result<()> {
    let raw = serde_json::to_vec_pretty(state)?;
    std::fs::write(path, raw)
        .with_context(|| format!("failed to write notify state: {}", path.display()))
}

fn notification_signature(task: &TaskRecord) -> String {
    task.finished_at.unwrap_or(task.updated_at).to_rfc3339()
}

fn notification_message(task: &TaskRecord) -> String {
    format!(
        "swarmux {} {} {}",
        task.id,
        state_label(&task.state),
        task.title
    )
}

fn notification_value(notification: &Notification) -> Value {
    let task = &notification.task;
    json!({
        "id": task.id,
        "title": task.title,
        "state": state_label(&task.state),
        "reason": task.reason,
        "finished_at": task.finished_at,
        "message": notification_message(task),
        "output_excerpt": notification.output_excerpt,
        "token_count": notification.token_count,
    })
}

fn notify_value(outcome: &NotifyOutcome) -> Value {
    json!({
        "reconciled": {
            "updated": outcome.reconciled,
        },
        "count": outcome.notifications.len(),
        "notifications": outcome
            .notifications
            .iter()
            .map(notification_value)
            .collect::<Vec<_>>(),
    })
}

fn collect_notifications(store: &Store, tmux: bool, show_tokens: bool) -> Result<NotifyOutcome> {
    let reconciled = reconcile_store(store)?;
    let tasks = list_tasks(store)?;
    let notify_path = store.paths().notify_file();
    let mut state = load_notify_state(&notify_path)?;

    let notifications = tasks
        .iter()
        .filter(|task| task.state.is_terminal())
        .filter(|task| state.delivered.get(&task.id) != Some(&notification_signature(task)))
        .map(|task| {
            Ok(Notification {
                task: task.clone(),
                output_excerpt: runtime::output_excerpt(task, 25)?,
                token_count: if show_tokens {
                    runtime::token_count(task)?
                } else {
                    None
                },
            })
        })
        .collect::<Result<Vec<_>>>()?;

    if tmux {
        if std::env::var_os("TMUX").is_none() {
            return Err(anyhow!("notify/watch --tmux requires running inside tmux"));
        }

        for notification in &notifications {
            runtime::display_message(&notification_message(&notification.task))?;
        }
    }

    for notification in &notifications {
        state.delivered.insert(
            notification.task.id.clone(),
            notification_signature(&notification.task),
        );
    }
    save_notify_state(&notify_path, &state)?;

    Ok(NotifyOutcome {
        reconciled,
        notifications,
    })
}

fn emit_watch_tick(
    output: &OutputFormat,
    outcome: &NotifyOutcome,
    show_tokens: bool,
) -> Result<()> {
    if outcome.reconciled == 0 && outcome.notifications.is_empty() {
        return Ok(());
    }

    match output {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string(&notify_value(outcome))?);
        }
        OutputFormat::Text => {
            if outcome.reconciled > 0 {
                println!("reconciled updated={}", outcome.reconciled);
            }
            if outcome.notifications.is_empty() {
                return Ok(());
            }

            println!("{}", watch_header(show_tokens));
            for notification in &outcome.notifications {
                println!("{}", watch_row(notification, show_tokens));
            }
        }
    }

    Ok(())
}

fn watch_header(show_tokens: bool) -> String {
    if show_tokens {
        format!(
            "{:<20}  {:<4}  {:<12}  {:>8}  {:<28}  {}",
            "time", "id", "state", "tokens", "title", "excerpt"
        )
    } else {
        format!(
            "{:<20}  {:<4}  {:<12}  {:<28}  {}",
            "time", "id", "state", "title", "excerpt"
        )
    }
}

fn watch_row(notification: &Notification, show_tokens: bool) -> String {
    let time = notification
        .task
        .finished_at
        .unwrap_or(notification.task.updated_at)
        .format("%Y-%m-%dT%H:%M:%SZ")
        .to_string();
    let state = state_label(&notification.task.state);
    let title = truncate_cell(&notification.task.title, 28);
    let excerpt = notification.output_excerpt.as_deref().unwrap_or("");

    if show_tokens {
        let tokens = notification.token_count.as_deref().unwrap_or("");
        format!(
            "{:<20}  {:<4}  {:<12}  {:>8}  {:<28}  {}",
            time, notification.task.id, state, tokens, title, excerpt
        )
    } else {
        format!(
            "{:<20}  {:<4}  {:<12}  {:<28}  {}",
            time, notification.task.id, state, title, excerpt
        )
    }
}

fn truncate_cell(value: &str, width: usize) -> String {
    let chars = value.chars().collect::<Vec<_>>();
    if chars.len() <= width {
        return value.to_string();
    }
    if width <= 3 {
        return chars.into_iter().take(width).collect();
    }

    let mut truncated = chars
        .into_iter()
        .take(width.saturating_sub(3))
        .collect::<String>();
    truncated.push_str("...");
    truncated
}

fn state_label(state: &TaskState) -> &'static str {
    match state {
        TaskState::Queued => "queued",
        TaskState::Dispatching => "dispatching",
        TaskState::Running => "running",
        TaskState::WaitingInput => "waiting_input",
        TaskState::Succeeded => "succeeded",
        TaskState::Failed => "failed",
        TaskState::Canceled => "canceled",
    }
}

fn command_available(name: &str) -> bool {
    std::process::Command::new(name)
        .arg("--help")
        .output()
        .is_ok()
}

fn project_payload(value: Value, fields: Option<&str>) -> Result<Value> {
    let Some(fields) = fields else {
        return Ok(value);
    };

    let selected = fields
        .split(',')
        .map(str::trim)
        .filter(|field| !field.is_empty())
        .collect::<Vec<_>>();

    if selected.is_empty() {
        return Err(anyhow!("--fields must not be empty"));
    }

    match value {
        Value::Object(map) => Ok(Value::Object(project_object(map, &selected))),
        Value::Array(items) => Ok(Value::Array(
            items
                .into_iter()
                .map(|item| match item {
                    Value::Object(map) => Value::Object(project_object(map, &selected)),
                    other => other,
                })
                .collect(),
        )),
        other => Ok(other),
    }
}

fn project_object(map: Map<String, Value>, selected: &[&str]) -> Map<String, Value> {
    let mut projected = Map::new();
    for field in selected {
        if let Some(value) = map.get(*field) {
            projected.insert((*field).to_string(), value.clone());
        }
    }
    projected
}
