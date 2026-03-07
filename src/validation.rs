use crate::model::{SubmitPayload, TaskMode};
use anyhow::{Result, anyhow};
use std::path::Path;

const RESERVED_CHARS: [char; 3] = ['%', '?', '#'];

pub fn validate_submit_payload(payload: &SubmitPayload) -> Result<()> {
    reject_control_chars(&payload.title, "title")?;
    reject_control_chars(&payload.repo_ref, "repo_ref")?;
    reject_control_chars(&payload.repo_root, "repo_root")?;
    reject_reserved_chars(&payload.repo_ref, "repo_ref")?;
    reject_path_traversal(&payload.repo_root, "repo_root")?;

    if payload.command.is_empty() {
        return Err(anyhow!("command must not be empty"));
    }

    for part in &payload.command {
        reject_control_chars(part, "command")?;
    }

    match payload.mode {
        TaskMode::Auto => Ok(()),
        TaskMode::Manual => {
            let worktree = payload
                .worktree
                .as_deref()
                .ok_or_else(|| anyhow!("manual mode requires worktree"))?;
            let session = payload
                .session
                .as_deref()
                .ok_or_else(|| anyhow!("manual mode requires session"))?;

            reject_control_chars(worktree, "worktree")?;
            reject_control_chars(session, "session")?;
            reject_reserved_chars(session, "session")?;
            reject_path_traversal(worktree, "worktree")?;
            Ok(())
        }
    }
}

fn reject_control_chars(value: &str, field: &str) -> Result<()> {
    if value.chars().any(|ch| ch.is_control()) {
        return Err(anyhow!("{field} must not contain control characters"));
    }

    Ok(())
}

fn reject_reserved_chars(value: &str, field: &str) -> Result<()> {
    if value.chars().any(|ch| RESERVED_CHARS.contains(&ch)) {
        return Err(anyhow!("{field} must not contain %, ?, or #"));
    }

    Ok(())
}

fn reject_path_traversal(value: &str, field: &str) -> Result<()> {
    let path = Path::new(value);

    if path
        .components()
        .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err(anyhow!("{field} must not contain path traversal"));
    }

    Ok(())
}
