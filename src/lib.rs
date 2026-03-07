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
    Cli, Commands, FailArgs, IdArgs, ListArgs, LogsArgs, OutputFormat, PruneArgs, SendArgs,
    ShowArgs, StateArgs, StopArgs, SubmitArgs,
};
use config::AppConfig;
use model::{DryRunSubmitResponse, SubmitPayload, TaskState};
use serde_json::{Map, Value, json};
use store::{Store, require_task_id};

pub fn run() -> Result<()> {
    let cli = Cli::parse();
    let config = AppConfig::from_env();
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
        Commands::List(args) => run_list(&store, cli.output, args),
        Commands::Show(args) => run_show(&store, cli.output, args),
        Commands::Logs(args) => run_logs(&store, cli.output, args),
        Commands::Send(args) => run_send(&store, cli.output, args),
        Commands::Attach(args) => run_attach(&store, args),
        Commands::Stop(args) => run_stop(&store, cli.output, args),
        Commands::Reconcile => run_reconcile(&store, cli.output),
        Commands::Prune(args) => run_prune(&store, cli.output, args),
        Commands::Popup(args) => run_popup(&store, cli.output, args),
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
    validation::validate_submit_payload(&payload)?;

    if args.dry_run {
        return emit(
            &output,
            &json!({
                "ok": true,
                "dry_run": true,
                "command": "delegate",
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
    emit(&output, &json!({"updated": outcome.updated}))
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

fn run_popup(store: &Store, output: OutputFormat, args: cli::PopupArgs) -> Result<()> {
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

    match output {
        OutputFormat::Json => emit(&output, &json!({ "counts": counts, "tasks": tasks })),
        OutputFormat::Text => {
            println!("Swarmux popup");
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
            Ok(())
        }
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
