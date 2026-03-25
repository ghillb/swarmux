use crate::beads;
use crate::config::BackendKind;
use crate::config::TaskRuntime;
use crate::model::{TaskRecord, TaskState};
use crate::panes::{PaneGitSummary, PaneSnapshot};
use crate::store::Store;
use anyhow::{Context, Result, anyhow};
use std::path::Path;
use std::process::Command;

const AGENT_WIDTH: usize = 12;
const RUNTIME_WIDTH: usize = 10;
const REPO_WIDTH: usize = 18;
const BRANCH_WIDTH: usize = 12;
const GIT_WIDTH: usize = 14;

const PANE_LIST_FORMAT: &str = "#{session_name}\t#{window_id}\t#{window_index}\t#{window_name}\t#{pane_id}\t#{pane_index}\t#{pane_active}\t#{pane_current_path}\t#{pane_current_command}\t#{pane_title}";

#[derive(Debug, Clone)]
pub(crate) struct RawPane {
    pub session_name: String,
    pub window_id: String,
    pub window_index: i64,
    pub window_name: String,
    pub pane_id: String,
    pub pane_index: i64,
    pub pane_active: bool,
    pub pane_current_path: String,
    pub pane_current_command: String,
    pub pane_title: String,
}

#[derive(Debug, Clone)]
pub(crate) struct GitInfo {
    pub repo_root: String,
    pub repo: String,
    pub branch: Option<String>,
    pub summary: PaneGitSummary,
}

#[derive(Debug, Clone)]
pub(crate) struct GitContext {
    pub repo_root: String,
    pub repo: String,
    pub branch: Option<String>,
}

pub(crate) fn list_tasks(store: &Store) -> Result<Vec<TaskRecord>> {
    match store.paths().backend {
        BackendKind::Files => store.list(),
        BackendKind::Beads => beads::list(store.paths()),
    }
}

pub(crate) fn list_tmux_panes(filter: Option<&str>) -> Result<Vec<RawPane>> {
    let mut command = Command::new("tmux");
    command.args(["list-panes", "-a", "-F", PANE_LIST_FORMAT]);
    if let Some(filter) = filter {
        command.args(["-f", filter]);
    }

    let output = command.output().context("failed to run tmux")?;
    if !output.status.success() {
        return Err(anyhow!(
            "tmux failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    let raw = String::from_utf8_lossy(&output.stdout).into_owned();
    let mut panes = Vec::new();

    for line in raw.lines().filter(|line| !line.trim().is_empty()) {
        let mut fields = line.split('\t');
        panes.push(RawPane {
            session_name: field(&mut fields, "session_name")?.to_string(),
            window_id: field(&mut fields, "window_id")?.to_string(),
            window_index: field(&mut fields, "window_index")?.parse::<i64>()?,
            window_name: field(&mut fields, "window_name")?.to_string(),
            pane_id: field(&mut fields, "pane_id")?.to_string(),
            pane_index: field(&mut fields, "pane_index")?.parse::<i64>()?,
            pane_active: parse_bool(field(&mut fields, "pane_active")?)?,
            pane_current_path: field(&mut fields, "pane_current_path")?.to_string(),
            pane_current_command: field(&mut fields, "pane_current_command")?.to_string(),
            pane_title: field(&mut fields, "pane_title")?.to_string(),
        });
    }

    Ok(panes)
}

pub(crate) fn current_tmux_session_name() -> Result<Option<String>> {
    let output = run_tmux(["display-message", "-p", "#{session_name}"])?;
    let session_name = output.trim().to_string();

    if session_name.is_empty() {
        Ok(None)
    } else {
        Ok(Some(session_name))
    }
}

pub(crate) fn git_info(path: &str) -> Result<GitInfo> {
    let context = git_context(path)?;
    let status = run_git_optional([
        "-C",
        path,
        "status",
        "--porcelain=v1",
        "--untracked-files=all",
    ])?
    .unwrap_or_default();
    let tracked_diff = run_git_optional(["-C", path, "diff", "--numstat"])?.unwrap_or_default();
    let staged_diff =
        run_git_optional(["-C", path, "diff", "--cached", "--numstat"])?.unwrap_or_default();
    let summary = parse_git_summary(&status, &tracked_diff, &staged_diff);

    Ok(GitInfo {
        repo_root: context.repo_root,
        repo: context.repo,
        branch: context.branch,
        summary,
    })
}

pub(crate) fn git_context(path: &str) -> Result<GitContext> {
    let repo_root = run_git_optional(["-C", path, "rev-parse", "--show-toplevel"])?
        .ok_or_else(|| anyhow!("not a git repo"))?;
    let branch = run_git_optional(["-C", path, "branch", "--show-current"])?
        .map(|branch| branch.trim().to_string())
        .filter(|branch| !branch.is_empty());
    let repo = repo_name(&repo_root);

    Ok(GitContext {
        repo_root,
        repo,
        branch,
    })
}

pub(crate) fn build_label(
    raw: &RawPane,
    task: Option<&TaskRecord>,
    repo: Option<&str>,
    branch: Option<&str>,
    git: Option<&PaneGitSummary>,
) -> String {
    let mut segments = Vec::new();

    if let Some(task) = task {
        segments.push(shorten(&task.title, 42));
        let agent = task
            .command
            .first()
            .map(|value| value.as_str())
            .unwrap_or("task");
        segments.push(format!(
            "{agent}/{}{}",
            runtime_label(task),
            if task.state == TaskState::WaitingInput {
                "/waiting_input"
            } else {
                ""
            }
        ));
    } else if !raw.pane_current_command.trim().is_empty() {
        segments.push(shorten(&raw.pane_current_command, 32));
    }

    if let Some(repo) = repo {
        let mut repo_segment = repo.to_string();
        if let Some(branch) = branch {
            repo_segment.push('@');
            repo_segment.push_str(branch);
        }
        segments.push(repo_segment);
    }

    if let Some(git) = git {
        segments.push(git.label.clone());
    }

    segments.push(short_path(&raw.pane_current_path));
    segments.join(" | ")
}

pub(crate) fn pane_sort_key(pane: &PaneSnapshot) -> (u8, u8, u8, String, i64, i64, String) {
    let current = if pane.current { 0 } else { 1 };
    let state_rank = pane
        .task
        .as_ref()
        .map(|task| match task.state {
            TaskState::WaitingInput => 0,
            TaskState::Running | TaskState::Dispatching => 1,
            TaskState::Queued => 2,
            TaskState::Succeeded | TaskState::Failed | TaskState::Canceled => 3,
        })
        .unwrap_or(4);
    let managed = if pane.managed_by_swarmux { 0 } else { 1 };
    (
        current,
        state_rank,
        managed,
        pane.session_name.clone(),
        pane.window_index,
        pane.pane_index,
        pane.pane_id.clone(),
    )
}

pub(crate) fn pane_counts(panes: &[PaneSnapshot]) -> crate::panes::PaneCounts {
    crate::panes::PaneCounts {
        panes: panes.len(),
        sessions: panes
            .iter()
            .map(|pane| pane.session_name.clone())
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        managed_panes: panes.iter().filter(|pane| pane.managed_by_swarmux).count(),
        dirty_panes: panes
            .iter()
            .filter(|pane| pane.git.as_ref().is_some_and(|git| git.dirty))
            .count(),
        waiting_input: panes
            .iter()
            .filter(|pane| {
                pane.task
                    .as_ref()
                    .is_some_and(|task| task.state == TaskState::WaitingInput)
            })
            .count(),
        current: panes.iter().filter(|pane| pane.current).count(),
    }
}

pub(crate) fn runtime_label(task: &TaskRecord) -> &'static str {
    match task.runtime {
        TaskRuntime::Headless => "headless",
        TaskRuntime::Mirrored => "mirrored",
        TaskRuntime::Tui => "tui",
    }
}

pub(crate) fn task_state_label(state: &TaskState) -> &'static str {
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

pub(crate) fn set_pane_option(pane_id: &str, name: &str, value: &str) -> Result<()> {
    run_tmux(["set-option", "-pt", pane_id, name, value]).map(|_| ())
}

pub(crate) fn bool_flag(value: bool) -> &'static str {
    if value { "1" } else { "0" }
}

pub(crate) fn pane_row(pane: &PaneSnapshot) -> String {
    let agent = pane
        .task
        .as_ref()
        .and_then(|task| task.command.first())
        .map(|value| value.as_str())
        .unwrap_or(&pane.pane_current_command);
    let branch = pane.branch.as_deref().unwrap_or("");
    let git = pane
        .git
        .as_ref()
        .map(|git| git.label.as_str())
        .unwrap_or("");
    let repo = pane.repo.as_deref().unwrap_or("");
    let runtime = pane
        .task
        .as_ref()
        .map(|task| runtime_label(task))
        .unwrap_or("");

    format!(
        "{} │ {} │ {} │ {} │ {}",
        fixed_cell(agent, AGENT_WIDTH),
        fixed_cell(repo, REPO_WIDTH),
        fixed_cell(branch, BRANCH_WIDTH),
        fixed_cell(git, GIT_WIDTH),
        fixed_cell(runtime, RUNTIME_WIDTH),
    )
}

fn parse_git_summary(status: &str, tracked_diff: &str, staged_diff: &str) -> PaneGitSummary {
    let mut changed_files = 0usize;
    let mut deleted_files = 0usize;
    let mut untracked_files = 0usize;

    for line in status.lines() {
        if line.starts_with("??") {
            untracked_files += 1;
            continue;
        }

        let status_bytes = line.chars().take(2).collect::<Vec<_>>();
        if status_bytes.contains(&'D') {
            deleted_files += 1;
            continue;
        }
        if status_bytes
            .iter()
            .any(|ch| matches!(ch, 'A' | 'M' | 'R' | 'C' | 'T' | 'U'))
        {
            changed_files += 1;
        }
    }

    let (tracked_insertions, tracked_deletions) = parse_numstat(tracked_diff);
    let (staged_insertions, staged_deletions) = parse_numstat(staged_diff);
    let insertions = tracked_insertions + staged_insertions;
    let deletions = tracked_deletions + staged_deletions;
    let dirty = changed_files > 0
        || deleted_files > 0
        || untracked_files > 0
        || insertions > 0
        || deletions > 0;

    PaneGitSummary {
        dirty,
        changed_files,
        deleted_files,
        untracked_files,
        insertions,
        deletions,
        label: git_summary_label(
            dirty,
            changed_files,
            deleted_files,
            untracked_files,
            insertions,
            deletions,
        ),
    }
}

fn parse_numstat(text: &str) -> (usize, usize) {
    let mut insertions = 0usize;
    let mut deletions = 0usize;

    for line in text.lines() {
        let mut parts = line.split_whitespace();
        let Some(inserted) = parts.next() else {
            continue;
        };
        let Some(removed) = parts.next() else {
            continue;
        };

        if inserted != "-" {
            insertions += inserted.parse::<usize>().unwrap_or(0);
        }
        if removed != "-" {
            deletions += removed.parse::<usize>().unwrap_or(0);
        }
    }

    (insertions, deletions)
}

fn git_summary_label(
    dirty: bool,
    changed_files: usize,
    deleted_files: usize,
    untracked_files: usize,
    insertions: usize,
    deletions: usize,
) -> String {
    if !dirty {
        return "clean".to_string();
    }

    let mut parts = Vec::new();
    if changed_files > 0 {
        parts.push(format!("chg{changed_files}"));
    }
    if deleted_files > 0 {
        parts.push(format!("del{deleted_files}"));
    }
    if untracked_files > 0 {
        parts.push(format!("new{untracked_files}"));
    }
    if insertions > 0 || deletions > 0 {
        parts.push(format!("+{insertions}/-{deletions}"));
    }

    if parts.is_empty() {
        "dirty".to_string()
    } else {
        parts.join(" ")
    }
}

fn repo_name(repo_root: &str) -> String {
    Path::new(repo_root)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("repo")
        .to_string()
}

fn short_path(path: &str) -> String {
    if let Some(home) = dirs::home_dir() {
        let home = home.display().to_string();
        if let Some(rest) = path.strip_prefix(&home) {
            if rest.is_empty() {
                return "~".to_string();
            }
            return format!("~{rest}");
        }
    }

    path.to_string()
}

fn shorten(text: &str, max_chars: usize) -> String {
    let chars = text.chars().collect::<Vec<_>>();
    if chars.len() <= max_chars {
        return text.to_string();
    }

    if max_chars <= 3 {
        return chars.into_iter().take(max_chars).collect();
    }

    let mut out = chars
        .into_iter()
        .take(max_chars.saturating_sub(3))
        .collect::<String>();
    out.push_str("...");
    out
}

fn field<'a, I>(fields: &mut I, name: &str) -> Result<&'a str>
where
    I: Iterator<Item = &'a str>,
{
    fields
        .next()
        .ok_or_else(|| anyhow!("missing tmux field: {name}"))
}

fn parse_bool(raw: &str) -> Result<bool> {
    match raw {
        "1" | "true" => Ok(true),
        "0" | "false" => Ok(false),
        value => Err(anyhow!("invalid tmux bool: {value}")),
    }
}

fn fixed_cell(text: &str, width: usize) -> String {
    let text = truncate_to_width(text, width);
    let padding = width.saturating_sub(text.chars().count());
    format!("{text}{}", " ".repeat(padding))
}

fn truncate_to_width(text: &str, width: usize) -> String {
    let chars = text.chars().collect::<Vec<_>>();
    if chars.len() <= width {
        return text.to_string();
    }

    if width <= 1 {
        return chars.into_iter().take(width).collect();
    }

    let mut out = chars
        .into_iter()
        .take(width.saturating_sub(1))
        .collect::<String>();
    out.push('…');
    out
}

fn run_git_optional<const N: usize>(args: [&str; N]) -> Result<Option<String>> {
    let output = Command::new("git")
        .args(args)
        .output()
        .context("failed to run git")?;
    if !output.status.success() {
        return Ok(None);
    }

    Ok(Some(
        String::from_utf8_lossy(&output.stdout).trim().to_string(),
    ))
}

fn run_tmux<const N: usize>(args: [&str; N]) -> Result<String> {
    let output = Command::new("tmux")
        .args(args)
        .output()
        .context("failed to run tmux")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(anyhow!("tmux failed: {stderr}"));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}
