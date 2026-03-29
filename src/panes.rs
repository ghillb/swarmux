use crate::cli::{JumpArgs, OutputFormat, PanesArgs, PanesCommand};
use crate::emit;
use crate::model::TaskRecord;
use crate::panes_support::{
    bool_flag, build_label, git_info, list_tasks, list_tmux_panes, pane_counts, pane_row,
    pane_sort_key, runtime_label, set_pane_option, task_state_label, tmux_command,
};
use crate::runtime;
use crate::store::Store;
use anyhow::{Context, Result, anyhow};
use serde::Serialize;
use std::collections::BTreeMap;
use std::env;

#[derive(Debug, Clone, Serialize)]
pub struct PaneGitSummary {
    pub dirty: bool,
    pub changed_files: usize,
    pub deleted_files: usize,
    pub untracked_files: usize,
    pub insertions: usize,
    pub deletions: usize,
    pub label: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PaneSnapshot {
    pub current: bool,
    pub managed_by_swarmux: bool,
    pub session_name: String,
    pub window_id: String,
    pub window_index: i64,
    pub window_name: String,
    pub pane_id: String,
    pub pane_index: i64,
    pub pane_active: bool,
    #[serde(skip_serializing)]
    pub window_bell_flag: bool,
    pub pane_current_path: String,
    pub pane_current_command: String,
    pub pane_title: String,
    pub task: Option<TaskRecord>,
    pub repo_root: Option<String>,
    pub repo: Option<String>,
    pub branch: Option<String>,
    pub git: Option<PaneGitSummary>,
    pub label: String,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct PaneCounts {
    pub(crate) panes: usize,
    pub(crate) sessions: usize,
    pub(crate) managed_panes: usize,
    pub(crate) dirty_panes: usize,
    pub(crate) waiting_input: usize,
    pub(crate) current: usize,
}

#[derive(Debug, Clone, Serialize)]
struct PaneListResponse {
    current_pane: Option<String>,
    counts: PaneCounts,
    panes: Vec<PaneSnapshot>,
}

#[derive(Debug, Clone, Serialize)]
struct PaneSyncResponse {
    ok: bool,
    updated: usize,
    counts: PaneCounts,
}

#[derive(Debug, Clone, Serialize)]
struct PaneSwitchResponse {
    ok: bool,
    updated: usize,
}

#[derive(Debug, Clone, Serialize)]
struct PaneJumpResponse {
    ok: bool,
    index: usize,
    pane_id: String,
    session_name: String,
    window_id: String,
    task_id: Option<String>,
}

const SWARMUX_TREE_FORMAT: &str =
    "#{?pane_format,#{@swx_row},#{?window_format,#{window_index}:#{window_name},#{session_name}}}";
const SWARMUX_TREE_TEMPLATE: &str = r##"sh -lc 'target="$1"; pane_id="$(tmux display-message -p -t "$target" "#{pane_id}" 2>/dev/null || true)"; if [ -z "$pane_id" ]; then exit 0; fi; session_name="$(tmux display-message -p -t "$target" "#{session_name}")"; window_id="$(tmux display-message -p -t "$target" "#{window_id}")"; tmux switch-client -t "$session_name"; tmux select-window -t "$window_id"; tmux select-pane -t "$pane_id"' sh '%%'"##;
const SWARMUX_TREE_TITLE: &str = "Swarmux Panes";

pub fn run(store: &Store, output: OutputFormat, args: PanesArgs) -> Result<()> {
    match args.command {
        Some(PanesCommand::SyncTmuxMeta) => {
            let outcome = sync_tmux_meta(store)?;
            emit(&output, &outcome)
        }
        Some(PanesCommand::Switch(args)) => {
            if args.launch_sidebar {
                return launch_sidebar(args.pane_id.as_deref());
            }

            if args.tui {
                return crate::panes_tui::run(store, args.pane_id.as_deref());
            }

            if args.tui_sidebar {
                return crate::panes_tui::run_sidebar(store, args.pane_id.as_deref());
            }

            switch(store, output)
        }
        Some(PanesCommand::Jump(args)) => jump(store, output, args),
        None => {
            let response = list_panes(store)?;
            emit(&output, &response)
        }
    }
}

fn list_panes(store: &Store) -> Result<PaneListResponse> {
    let panes = build_panes(store)?;
    let counts = pane_counts(&panes);
    let current_pane = panes
        .iter()
        .find(|pane| pane.current)
        .map(|pane| pane.pane_id.clone());

    Ok(PaneListResponse {
        current_pane,
        counts,
        panes,
    })
}

fn sync_tmux_meta(store: &Store) -> Result<PaneSyncResponse> {
    let panes = build_panes(store)?;
    let counts = pane_counts(&panes);
    let mut updated = 0usize;

    for pane in panes {
        let row = pane_row(&pane);
        set_pane_option(&pane.pane_id, "@swx_current", bool_flag(pane.current))?;
        set_pane_option(
            &pane.pane_id,
            "@swx_managed",
            bool_flag(pane.managed_by_swarmux),
        )?;
        set_pane_option(&pane.pane_id, "@swx_label", &pane.label)?;
        set_pane_option(&pane.pane_id, "@swx_row", &row)?;
        set_pane_option(
            &pane.pane_id,
            "@swx_title",
            pane.task
                .as_ref()
                .map(|task| task.title.as_str())
                .unwrap_or(&pane.pane_current_command),
        )?;
        set_pane_option(
            &pane.pane_id,
            "@swx_agent",
            pane.task
                .as_ref()
                .and_then(|task| task.command.first())
                .map(String::as_str)
                .unwrap_or(&pane.pane_current_command),
        )?;
        set_pane_option(
            &pane.pane_id,
            "@swx_runtime",
            pane.task.as_ref().map(runtime_label).unwrap_or(""),
        )?;
        set_pane_option(
            &pane.pane_id,
            "@swx_state",
            pane.task
                .as_ref()
                .map(|task| task_state_label(&task.state))
                .unwrap_or(""),
        )?;
        set_pane_option(
            &pane.pane_id,
            "@swx_repo",
            pane.repo.as_deref().unwrap_or(""),
        )?;
        set_pane_option(
            &pane.pane_id,
            "@swx_branch",
            pane.branch.as_deref().unwrap_or(""),
        )?;
        set_pane_option(
            &pane.pane_id,
            "@swx_git",
            pane.git
                .as_ref()
                .map(|git| git.label.as_str())
                .unwrap_or(""),
        )?;
        set_pane_option(
            &pane.pane_id,
            "@swx_path",
            pane.repo_root.as_deref().unwrap_or(&pane.pane_current_path),
        )?;
        set_pane_option(
            &pane.pane_id,
            "@swx_task_id",
            pane.task
                .as_ref()
                .map(|task| task.id.as_str())
                .unwrap_or(""),
        )?;
        updated += 1;
    }

    Ok(PaneSyncResponse {
        ok: true,
        updated,
        counts,
    })
}

fn switch(store: &Store, output: OutputFormat) -> Result<()> {
    let sync = sync_tmux_meta(store)?;
    launch_swarmux_tree_popup(&store.paths().settings.tmux.ignore_filter())?;

    if matches!(output, OutputFormat::Json) {
        emit(
            &output,
            &PaneSwitchResponse {
                ok: true,
                updated: sync.updated,
            },
        )?;
    }

    Ok(())
}

fn jump(store: &Store, output: OutputFormat, args: JumpArgs) -> Result<()> {
    ensure_tmux_client("panes jump")?;
    if !(1..=9).contains(&args.index) {
        return Err(anyhow!("--index must be between 1 and 9"));
    }
    let panes = build_panes(store)?;
    let target = panes
        .into_iter()
        .filter(|pane| pane.managed_by_swarmux)
        .nth(args.index.saturating_sub(1))
        .ok_or_else(|| anyhow!("no managed pane at index {}", args.index))?;

    focus_pane(&target)?;
    emit(
        &output,
        &PaneJumpResponse {
            ok: true,
            index: args.index,
            pane_id: target.pane_id,
            session_name: target.session_name,
            window_id: target.window_id,
            task_id: target.task.as_ref().map(|task| task.id.clone()),
        },
    )
}

fn launch_sidebar(source_pane_id: Option<&str>) -> Result<()> {
    let context = runtime::current_pane_context(source_pane_id)?;
    let binary = std::env::current_exe().context("failed to resolve swarmux binary")?;
    let mut command = tmux_command();
    command.args([
        "split-window",
        "-P",
        "-F",
        "#{pane_id}",
        "-h",
        "-l",
        "42",
        "-c",
        context.pane_current_path.as_str(),
        "-e",
        "SWARMUX_TUI_SIDEBAR_AUTOCLOSE=1",
    ]);
    command.arg(&binary);
    command.args(sidebar_child_args(context.pane_id.as_str()));

    let output = command.output().context("failed to run tmux")?;

    if !output.status.success() {
        return Err(anyhow!("tmux failed to open the sidebar pane"));
    }

    let pane_id = String::from_utf8(output.stdout)
        .context("tmux returned a non-utf8 sidebar pane id")?
        .trim()
        .to_string();

    if pane_id.is_empty() {
        return Err(anyhow!("tmux did not return a sidebar pane id"));
    }

    let status = tmux_command()
        .args([
            "select-pane",
            "-t",
            pane_id.as_str(),
            "-T",
            "swarmux-sidebar",
        ])
        .status()
        .context("failed to run tmux")?;

    if !status.success() {
        return Err(anyhow!("tmux failed to label the sidebar pane"));
    }

    Ok(())
}

fn ensure_tmux_client(command_name: &str) -> Result<()> {
    if std::env::var_os("TMUX").is_none() {
        return Err(anyhow!("{command_name} requires running inside tmux"));
    }
    Ok(())
}

fn focus_pane(pane: &PaneSnapshot) -> Result<()> {
    run_tmux_status(
        ["switch-client", "-t", pane.session_name.as_str()],
        "tmux failed to switch client",
    )?;
    run_tmux_status(
        ["select-window", "-t", pane.window_id.as_str()],
        "tmux failed to select window",
    )?;
    run_tmux_status(
        ["select-pane", "-t", pane.pane_id.as_str()],
        "tmux failed to select pane",
    )
}

fn run_tmux_status<const N: usize>(args: [&str; N], error_message: &str) -> Result<()> {
    let status = tmux_command()
        .args(args)
        .status()
        .context("failed to run tmux")?;

    if !status.success() {
        return Err(anyhow!("{}", error_message));
    }

    Ok(())
}

fn sidebar_child_args(source_pane_id: &str) -> Vec<String> {
    vec![
        "panes".to_string(),
        "switch".to_string(),
        "--tui-sidebar".to_string(),
        "--pane-id".to_string(),
        source_pane_id.to_string(),
    ]
}

fn launch_swarmux_tree_popup(filter: &str) -> Result<()> {
    let popup_w = env::var("SWARM_POPUP_W").unwrap_or_else(|_| "96%".to_string());
    let popup_h = env::var("SWARM_POPUP_H").unwrap_or_else(|_| "85%".to_string());
    let choose_tree_command = format!(
        "tmux choose-tree -f {} -F {} {}",
        shell_quote(filter),
        shell_quote(SWARMUX_TREE_FORMAT),
        shell_quote(SWARMUX_TREE_TEMPLATE),
    );

    let status = tmux_command()
        .args([
            "display-popup",
            "-T",
            SWARMUX_TREE_TITLE,
            "-w",
            popup_w.as_str(),
            "-h",
            popup_h.as_str(),
            "-E",
            choose_tree_command.as_str(),
        ])
        .status()
        .context("failed to run tmux")?;

    if !status.success() {
        return Err(anyhow!("tmux failed to open the swarmux popup"));
    }

    Ok(())
}

fn shell_quote(value: &str) -> String {
    if value.is_empty() {
        return "''".to_string();
    }

    let mut quoted = String::from("'");
    for ch in value.chars() {
        if ch == '\'' {
            quoted.push_str("'\"'\"'");
        } else {
            quoted.push(ch);
        }
    }
    quoted.push('\'');
    quoted
}

fn build_panes(store: &Store) -> Result<Vec<PaneSnapshot>> {
    let tasks = list_tasks(store)?;
    let tasks_by_session = tasks
        .iter()
        .filter_map(|task| {
            task.session
                .as_ref()
                .map(|session| (session.clone(), task.clone()))
        })
        .collect::<BTreeMap<_, _>>();

    let current_pane = std::env::var("TMUX_PANE").ok();
    let raw_panes = list_tmux_panes(None)?;
    let mut panes = raw_panes
        .into_iter()
        .map(|raw| {
            let task = tasks_by_session.get(&raw.session_name).cloned();
            let git_info = git_info(&raw.pane_current_path).ok();
            let current = current_pane.as_deref() == Some(raw.pane_id.as_str());
            let repo_root = git_info.as_ref().map(|info| info.repo_root.clone());
            let repo = git_info
                .as_ref()
                .map(|info| info.repo.clone())
                .or_else(|| task.as_ref().map(|task| task.repo.clone()));
            let branch = git_info
                .as_ref()
                .and_then(|info| info.branch.clone())
                .filter(|branch| !branch.is_empty());
            let git = git_info.as_ref().map(|info| info.summary.clone());
            let label = build_label(
                &raw,
                task.as_ref(),
                repo.as_deref(),
                branch.as_deref(),
                git.as_ref(),
            );

            PaneSnapshot {
                current,
                managed_by_swarmux: task.is_some(),
                session_name: raw.session_name,
                window_id: raw.window_id,
                window_index: raw.window_index,
                window_name: raw.window_name,
                pane_id: raw.pane_id,
                pane_index: raw.pane_index,
                pane_active: raw.pane_active,
                window_bell_flag: raw.window_bell_flag,
                pane_current_path: raw.pane_current_path,
                pane_current_command: raw.pane_current_command,
                pane_title: raw.pane_title,
                task,
                repo_root,
                repo,
                branch,
                git,
                label,
            }
        })
        .collect::<Vec<_>>();

    panes.sort_by_key(pane_sort_key);
    Ok(panes)
}

#[cfg(test)]
mod tests {
    use super::sidebar_child_args;

    #[test]
    fn sidebar_launcher_uses_plain_tui_invocation() {
        assert_eq!(
            sidebar_child_args("%79"),
            vec!["panes", "switch", "--tui-sidebar", "--pane-id", "%79",]
        );
    }
}
