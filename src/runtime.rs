use crate::model::{TaskRecord, TaskState};
use anyhow::{Context, Result, anyhow};
use std::fs::{self, OpenOptions};
use std::io::{Read, Seek, SeekFrom};
use std::process::Command;

const EXIT_MARKER: &str = "__SWARMUX_EXIT_CODE__=";

pub struct ReconcileOutcome {
    pub updated: usize,
}

pub struct PruneOutcome {
    pub worktree_removed: usize,
    pub session_killed: usize,
}

pub fn start_task(task: &TaskRecord) -> Result<TaskRecord> {
    if let (Some(branch), Some(worktree)) = (&task.branch, &task.worktree) {
        create_worktree(&task.repo_root, branch, worktree)?;
    }

    let session = task
        .session
        .as_deref()
        .ok_or_else(|| anyhow!("task is missing session"))?;
    let workdir = task
        .worktree
        .as_deref()
        .ok_or_else(|| anyhow!("task is missing worktree"))?;

    spawn_tmux_session(session, workdir, &task.log_file, &task.command)?;

    let mut next = task.clone();
    next.state = TaskState::Running;
    next.reason = "process_spawned".to_string();
    next.updated_at = chrono::Utc::now();
    Ok(next)
}

pub fn send_input(task: &TaskRecord, input: &str) -> Result<()> {
    let session = task
        .session
        .as_deref()
        .ok_or_else(|| anyhow!("task is missing session"))?;
    run_tmux(["send-keys", "-t", session, input, "C-m"]).map(|_| ())
}

pub fn display_message(message: &str) -> Result<()> {
    run_tmux_dynamic(&["display-message", message]).map(|_| ())
}

pub fn interrupt_task(task: &TaskRecord) -> Result<()> {
    let session = task
        .session
        .as_deref()
        .ok_or_else(|| anyhow!("task is missing session"))?;
    run_tmux(["send-keys", "-t", session, "C-c"]).map(|_| ())
}

pub fn kill_task(task: &TaskRecord) -> Result<()> {
    let session = task
        .session
        .as_deref()
        .ok_or_else(|| anyhow!("task is missing session"))?;
    run_tmux(["kill-session", "-t", session]).map(|_| ())
}

pub fn attach_task(task: &TaskRecord) -> Result<()> {
    let session = task
        .session
        .as_deref()
        .ok_or_else(|| anyhow!("task is missing session"))?;
    run_tmux(["attach-session", "-t", session]).map(|_| ())
}

pub fn read_logs(task: &TaskRecord, raw: bool, lines: usize) -> Result<String> {
    if let Some(session) = &task.session
        && let Ok(text) = capture_pane(session, lines)
    {
        return Ok(if raw { text } else { sanitize_logs(&text) });
    }

    let text = tail_file(&task.log_file, lines)?;
    Ok(if raw { text } else { sanitize_logs(&text) })
}

pub fn reconcile(
    tasks: &mut [TaskRecord],
    lock_path: &std::path::Path,
) -> Result<ReconcileOutcome> {
    let _lock = acquire_lock(lock_path)?;
    let mut updated = 0usize;

    for task in tasks.iter_mut() {
        if task.state != TaskState::Running && task.state != TaskState::Dispatching {
            continue;
        }

        let Some(session) = &task.session else {
            continue;
        };

        if has_tmux_session(session)? {
            continue;
        }

        let exit_code = read_exit_code(&task.log_file)?;
        task.updated_at = chrono::Utc::now();
        match exit_code {
            Some(0) => {
                task.state = TaskState::Succeeded;
                task.reason = "process_exit_0".to_string();
            }
            Some(130) => {
                task.state = TaskState::Canceled;
                task.reason = "process_exit_interrupt".to_string();
            }
            Some(code) => {
                task.state = TaskState::Failed;
                task.reason = "process_exit_nonzero".to_string();
                task.last_error = Some(format!("process exited with code {code}"));
            }
            None => {
                task.state = TaskState::Failed;
                task.reason = "missing_session_no_exit_marker".to_string();
                task.last_error = Some("missing exit marker".to_string());
            }
        }
        task.finished_at = Some(task.updated_at);
        updated += 1;
    }

    Ok(ReconcileOutcome { updated })
}

pub fn prune(task: &TaskRecord, apply: bool) -> Result<PruneOutcome> {
    let mut outcome = PruneOutcome {
        worktree_removed: 0,
        session_killed: 0,
    };

    if !matches!(task.mode, crate::model::TaskMode::Auto) || !task.state.is_terminal() {
        return Ok(outcome);
    }

    if !apply {
        return Ok(outcome);
    }

    if let Some(session) = &task.session
        && has_tmux_session(session)?
    {
        kill_task(task)?;
        outcome.session_killed += 1;
    }

    if let (Some(branch), Some(worktree)) = (&task.branch, &task.worktree) {
        remove_worktree(&task.repo_root, worktree)?;
        if local_branch_exists(&task.repo_root, branch)? {
            delete_branch(&task.repo_root, branch)?;
        }
        outcome.worktree_removed += 1;
    }

    Ok(outcome)
}

fn create_worktree(repo_root: &str, branch: &str, worktree: &str) -> Result<()> {
    run_git([
        "-C", repo_root, "worktree", "add", "-B", branch, worktree, "HEAD",
    ])
    .map(|_| ())
}

fn remove_worktree(repo_root: &str, worktree: &str) -> Result<()> {
    run_git(["-C", repo_root, "worktree", "remove", "-f", worktree]).map(|_| ())
}

fn local_branch_exists(repo_root: &str, branch: &str) -> Result<bool> {
    let output = Command::new("git")
        .args(["-C", repo_root, "show-ref", "--verify", "--quiet"])
        .arg(format!("refs/heads/{branch}"))
        .output()
        .context("failed to run git show-ref")?;
    Ok(output.status.success())
}

fn delete_branch(repo_root: &str, branch: &str) -> Result<()> {
    run_git(["-C", repo_root, "branch", "-D", branch]).map(|_| ())
}

fn spawn_tmux_session(
    session: &str,
    workdir: &str,
    log_file: &str,
    command: &[String],
) -> Result<()> {
    if let Some(parent) = std::path::Path::new(log_file).parent() {
        fs::create_dir_all(parent)?;
    }
    let script = r#"log_file="$1"; shift; : > "$log_file"; "$@" >> "$log_file" 2>&1; code=$?; printf '__SWARMUX_EXIT_CODE__=%s\n' "$code" >> "$log_file"; exit "$code""#;
    let mut args = vec![
        "new-session".to_string(),
        "-d".to_string(),
        "-s".to_string(),
        session.to_string(),
        "-c".to_string(),
        workdir.to_string(),
        "/bin/sh".to_string(),
        "-lc".to_string(),
        script.to_string(),
        "--".to_string(),
        log_file.to_string(),
    ];
    args.extend(command.iter().cloned());

    let args_ref = args.iter().map(String::as_str).collect::<Vec<_>>();
    run_tmux_dynamic(&args_ref).map(|_| ())
}

fn capture_pane(session: &str, lines: usize) -> Result<String> {
    run_tmux([
        "capture-pane",
        "-p",
        "-S",
        &format!("-{}", lines.max(1)),
        "-t",
        session,
    ])
}

fn has_tmux_session(session: &str) -> Result<bool> {
    let output = Command::new("tmux")
        .args(["has-session", "-t", session])
        .output()
        .context("failed to run tmux has-session")?;

    if output.status.success() {
        return Ok(true);
    }
    if output.status.code() == Some(1) {
        return Ok(false);
    }

    Err(anyhow!("tmux has-session failed"))
}

fn run_tmux<const N: usize>(args: [&str; N]) -> Result<String> {
    run_command("tmux", &args)
}

fn run_tmux_dynamic(args: &[&str]) -> Result<String> {
    run_command("tmux", args)
}

fn run_git<const N: usize>(args: [&str; N]) -> Result<String> {
    run_command("git", &args)
}

fn run_command(program: &str, args: &[&str]) -> Result<String> {
    let output = Command::new(program)
        .args(args)
        .output()
        .with_context(|| format!("failed to run {program}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(anyhow!("{program} failed: {stderr}"));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn sanitize_logs(text: &str) -> String {
    text.lines()
        .filter(|line| !line.contains(EXIT_MARKER))
        .map(|line| {
            line.chars()
                .filter(|ch| !ch.is_control() || *ch == '\n' || *ch == '\t')
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn tail_file(path: &str, lines: usize) -> Result<String> {
    let mut file =
        fs::File::open(path).with_context(|| format!("failed to read log file: {path}"))?;
    let len = file.metadata()?.len() as i64;
    let offset = (len - 65_536).max(0) as u64;
    file.seek(SeekFrom::Start(offset))?;
    let mut text = String::new();
    file.read_to_string(&mut text)?;
    let kept = text
        .lines()
        .rev()
        .take(lines.max(1))
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>();
    Ok(kept.join("\n"))
}

fn read_exit_code(path: &str) -> Result<Option<i32>> {
    let text =
        fs::read_to_string(path).with_context(|| format!("failed to read log file: {path}"))?;
    for line in text.lines().rev() {
        if let Some(value) = line.strip_prefix(EXIT_MARKER) {
            return Ok(value.parse::<i32>().ok());
        }
    }

    Ok(None)
}

fn acquire_lock(path: &std::path::Path) -> Result<LockGuard> {
    let file = OpenOptions::new().write(true).create_new(true).open(path);
    match file {
        Ok(_) => Ok(LockGuard {
            path: path.to_path_buf(),
        }),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            Err(anyhow!("reconcile lock already held"))
        }
        Err(error) => Err(error.into()),
    }
}

struct LockGuard {
    path: std::path::PathBuf,
}

impl Drop for LockGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}
