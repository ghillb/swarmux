use crate::config::TaskRuntime;
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

pub struct PaneContext {
    pub pane_id: String,
    pub session_name: String,
    pub window_id: String,
    pub window_name: String,
    pub pane_current_path: String,
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

    spawn_tmux_session(
        session,
        workdir,
        &task.log_file,
        &task.command,
        task.runtime,
    )?;

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
    run_tmux(["send-keys", "-t", session, input, "C-m"])?;
    append_log_line(&task.log_file, &format!("> {input}"))?;
    Ok(())
}

pub fn display_message(message: &str) -> Result<()> {
    run_tmux_dynamic(&["display-message", message]).map(|_| ())
}

pub fn current_pane_context(target: Option<&str>) -> Result<PaneContext> {
    let pane_id = match target {
        Some(value) => value.to_string(),
        None => std::env::var("TMUX_PANE")
            .context("connected dispatch requires TMUX_PANE or --pane-id")?,
    };

    Ok(PaneContext {
        session_name: tmux_format(&pane_id, "#{session_name}")?,
        window_id: tmux_format(&pane_id, "#{window_id}")?,
        window_name: tmux_format(&pane_id, "#{window_name}")?,
        pane_current_path: tmux_format(&pane_id, "#{pane_current_path}")?,
        pane_id,
    })
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
    let text = tail_file(&task.log_file, lines)?;
    Ok(if raw { text } else { sanitize_logs(&text) })
}

pub fn output_excerpt(task: &TaskRecord, max_chars: usize) -> Result<Option<String>> {
    let text = tail_file(&task.log_file, 50)?;
    Ok(output_excerpt_from_text(&text, max_chars))
}

pub fn token_count(task: &TaskRecord) -> Result<Option<String>> {
    let text = tail_file(&task.log_file, 50)?;
    Ok(token_count_from_text(&text))
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
    runtime: TaskRuntime,
) -> Result<()> {
    if let Some(parent) = std::path::Path::new(log_file).parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(log_file, "")?;
    let script_path = launch_script_path(log_file, runtime, "run");

    match runtime {
        TaskRuntime::Headless => {
            write_launch_script(&script_path, headless_launch_script())?;
            spawn_headless_tmux_session(session, workdir, &script_path, log_file, command)
        }
        TaskRuntime::Mirrored => {
            write_launch_script(&script_path, mirrored_launch_script())?;
            let pipe_script_path = launch_script_path(log_file, runtime, "pipe");
            write_launch_script(&pipe_script_path, pipe_launch_script())?;
            spawn_mirrored_tmux_session(
                session,
                workdir,
                &script_path,
                &pipe_script_path,
                log_file,
                command,
            )
        }
    }
}

fn spawn_headless_tmux_session(
    session: &str,
    workdir: &str,
    script_path: &str,
    log_file: &str,
    command: &[String],
) -> Result<()> {
    let mut args = vec![
        "new-session".to_string(),
        "-d".to_string(),
        "-s".to_string(),
        session.to_string(),
        "-c".to_string(),
        workdir.to_string(),
        "/bin/sh".to_string(),
        script_path.to_string(),
        log_file.to_string(),
    ];
    args.extend(command.iter().cloned());

    let args_ref = args.iter().map(String::as_str).collect::<Vec<_>>();
    run_tmux_dynamic(&args_ref).map(|_| ())
}

fn spawn_mirrored_tmux_session(
    session: &str,
    workdir: &str,
    script_path: &str,
    pipe_script_path: &str,
    log_file: &str,
    command: &[String],
) -> Result<()> {
    run_tmux(["new-session", "-d", "-s", session, "-c", workdir])?;

    let pipe_command = format!(
        "/bin/sh {} {}",
        shell_quote(pipe_script_path),
        shell_quote(log_file)
    );
    run_tmux_dynamic(&["pipe-pane", "-o", "-t", session, &pipe_command])?;

    let mut args = vec![
        "respawn-pane".to_string(),
        "-k".to_string(),
        "-t".to_string(),
        session.to_string(),
        "/bin/sh".to_string(),
        script_path.to_string(),
        log_file.to_string(),
    ];
    args.extend(command.iter().cloned());

    let args_ref = args.iter().map(String::as_str).collect::<Vec<_>>();
    run_tmux_dynamic(&args_ref).map(|_| ())
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

fn tmux_format(target: &str, format: &str) -> Result<String> {
    Ok(run_tmux(["display-message", "-p", "-t", target, format])?
        .trim()
        .to_string())
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
            strip_ansi_sequences(line)
                .chars()
                .filter(|ch| !ch.is_control() || *ch == '\n' || *ch == '\t')
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn output_excerpt_from_text(text: &str, max_chars: usize) -> Option<String> {
    let visible_lines = visible_log_lines(text);
    let mut fallback = None;

    for (index, visible) in visible_lines.iter().enumerate().rev() {
        if fallback.is_none() {
            fallback = Some(visible.clone());
        }

        if is_numeric_summary_line(visible)
            && index > 0
            && visible_lines[index - 1].eq_ignore_ascii_case("tokens used")
        {
            continue;
        }

        if visible.eq_ignore_ascii_case("tokens used") {
            continue;
        }

        return Some(truncate_excerpt(visible, max_chars));
    }

    fallback
        .as_deref()
        .map(|line| truncate_excerpt(line, max_chars))
}

fn token_count_from_text(text: &str) -> Option<String> {
    let visible_lines = visible_log_lines(text);

    visible_lines
        .windows(2)
        .filter(|window| {
            window[0].eq_ignore_ascii_case("tokens used") && is_numeric_summary_line(&window[1])
        })
        .map(|window| window[1].to_string())
        .next_back()
}

fn truncate_excerpt(text: &str, max_chars: usize) -> String {
    let chars = text.chars().collect::<Vec<_>>();
    let start = chars.len().saturating_sub(max_chars);
    let excerpt = chars[start..].iter().collect::<String>();
    format!("...{excerpt}")
}

fn is_numeric_summary_line(line: &str) -> bool {
    line.chars()
        .all(|ch| ch.is_ascii_digit() || ch == ',' || ch == '.')
}

fn visible_log_lines(text: &str) -> Vec<String> {
    sanitize_logs(text)
        .lines()
        .map(strip_log_timestamp)
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn strip_ansi_sequences(line: &str) -> String {
    let mut cleaned = String::new();
    let mut chars = line.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch != '\u{1b}' {
            cleaned.push(ch);
            continue;
        }

        if chars.peek() == Some(&'[') {
            chars.next();
            for next in chars.by_ref() {
                if ('@'..='~').contains(&next) {
                    break;
                }
            }
        }
    }

    cleaned
}

fn append_log_line(path: &str, line: &str) -> Result<()> {
    let mut file = OpenOptions::new()
        .append(true)
        .create(true)
        .open(path)
        .with_context(|| format!("failed to open log file: {path}"))?;
    use std::io::Write;
    writeln!(file, "{} {}", log_timestamp(), line)
        .with_context(|| format!("failed to append log file: {path}"))
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
        if let Some((_, value)) = line.split_once(EXIT_MARKER) {
            return Ok(value.parse::<i32>().ok());
        }
    }

    Ok(None)
}

fn log_timestamp() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

fn strip_log_timestamp(line: &str) -> &str {
    let bytes = line.as_bytes();
    if bytes.len() > 20
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes[10] == b'T'
        && bytes[13] == b':'
        && bytes[16] == b':'
        && bytes[19] == b'Z'
        && bytes[20] == b' '
    {
        &line[21..]
    } else {
        line
    }
}

fn shell_quote(value: &str) -> String {
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

fn launch_script_path(log_file: &str, runtime: TaskRuntime, kind: &str) -> String {
    let suffix = match runtime {
        TaskRuntime::Headless => "headless",
        TaskRuntime::Mirrored => "mirrored",
    };
    format!("{log_file}.{suffix}.{kind}.sh")
}

fn write_launch_script(path: &str, script: &str) -> Result<()> {
    fs::write(path, script).with_context(|| format!("failed to write launch script: {path}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(path)
            .with_context(|| format!("failed to stat launch script: {path}"))?
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(path, perms)
            .with_context(|| format!("failed to chmod launch script: {path}"))?;
    }
    Ok(())
}

fn headless_launch_script() -> &'static str {
    r#"#!/bin/sh
set -u
log_file="$1"
shift
: > "$log_file"
fifo="$(mktemp -u "${TMPDIR:-/tmp}/swarmux.XXXXXX.fifo")"
rm -f "$fifo"
mkfifo "$fifo"
{
  while IFS= read -r line || [ -n "$line" ]; do
    printf '%s %s\n' "$(date -u +"%Y-%m-%dT%H:%M:%SZ")" "$line" >> "$log_file"
  done < "$fifo"
} &
reader=$!
"$@" > "$fifo" 2>&1
code=$?
wait "$reader"
rm -f "$fifo"
printf '%s __SWARMUX_EXIT_CODE__=%s\n' "$(date -u +"%Y-%m-%dT%H:%M:%SZ")" "$code" >> "$log_file"
exit "$code"
"#
}

fn mirrored_launch_script() -> &'static str {
    r#"#!/bin/sh
set -u
log_file="$1"
shift
"$@"
code=$?
printf '%s __SWARMUX_EXIT_CODE__=%s\n' "$(date -u +"%Y-%m-%dT%H:%M:%SZ")" "$code" >> "$log_file"
exit "$code"
"#
}

fn pipe_launch_script() -> &'static str {
    r#"#!/bin/sh
set -u
log_file="$1"
while IFS= read -r line || [ -n "$line" ]; do
  printf '%s %s\n' "$(date -u +"%Y-%m-%dT%H:%M:%SZ")" "$line" >> "$log_file"
done
"#
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

#[cfg(test)]
mod tests {
    use super::{output_excerpt_from_text, strip_ansi_sequences, token_count_from_text};

    #[test]
    fn strip_ansi_sequences_removes_color_codes() {
        assert_eq!(strip_ansi_sequences("\u{1b}[35mhello\u{1b}[0m"), "hello");
    }

    #[test]
    fn output_excerpt_skips_tokens_used_footer() {
        let text = "\
2026-03-14T08:21:32Z \u{1b}[35mHi\u{1b}[0m\n\
2026-03-14T08:21:33Z tokens used\n\
2026-03-14T08:21:33Z 7,892\n";

        assert_eq!(
            output_excerpt_from_text(text, 25),
            Some("...Hi".to_string())
        );
    }

    #[test]
    fn token_count_extracts_last_tokens_used_footer() {
        let text = "\
2026-03-14T08:21:32Z Hi\n\
2026-03-14T08:21:33Z tokens used\n\
2026-03-14T08:21:33Z 7,892\n";

        assert_eq!(token_count_from_text(text), Some("7,892".to_string()));
    }
}
