use crate::config::AppConfig;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskMode {
    Auto,
    Manual,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmitPayload {
    pub title: String,
    pub repo_ref: String,
    pub repo_root: String,
    pub mode: TaskMode,
    #[serde(default)]
    pub worktree: Option<String>,
    #[serde(default)]
    pub session: Option<String>,
    pub command: Vec<String>,
    #[serde(default)]
    pub priority: Option<u8>,
    #[serde(default)]
    pub external_ref: Option<String>,
    #[serde(default)]
    pub origin: Option<TaskOrigin>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DryRunSubmitResponse {
    pub ok: bool,
    pub dry_run: bool,
    pub task: SubmitPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskState {
    Queued,
    Dispatching,
    Running,
    WaitingInput,
    Succeeded,
    Failed,
    Canceled,
}

impl TaskState {
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Succeeded | Self::Failed | Self::Canceled)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRecord {
    pub id: String,
    pub title: String,
    pub repo: String,
    pub repo_root: String,
    pub mode: TaskMode,
    pub branch: Option<String>,
    pub worktree: Option<String>,
    pub session: Option<String>,
    pub command: Vec<String>,
    pub priority: u8,
    pub external_ref: Option<String>,
    pub origin: Option<TaskOrigin>,
    pub state: TaskState,
    pub reason: String,
    pub last_error: Option<String>,
    pub log_file: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
}

impl TaskRecord {
    pub fn from_submit_with_id(payload: SubmitPayload, config: &AppConfig, id: String) -> Self {
        let now = Utc::now();
        let log_file = config.logs_dir().join(format!("{id}.log"));
        let repo = repo_slug(&payload.repo_ref);
        let (branch, worktree, session) = match payload.mode {
            TaskMode::Auto => {
                let repo_key = payload
                    .repo_root
                    .rsplit('/')
                    .find(|part| !part.is_empty())
                    .unwrap_or("repo");
                let branch = format!("swx/{id}");
                let worktree = config
                    .home
                    .join("worktrees")
                    .join(repo_key)
                    .join(&id)
                    .display()
                    .to_string();
                let session = format!("swx-{repo}-{id}");
                (Some(branch), Some(worktree), Some(session))
            }
            TaskMode::Manual => (None, payload.worktree, payload.session),
        };

        Self {
            id,
            title: payload.title,
            repo,
            repo_root: payload.repo_root,
            mode: payload.mode,
            branch,
            worktree,
            session,
            command: payload.command,
            priority: payload.priority.unwrap_or(2),
            external_ref: payload.external_ref,
            origin: payload.origin,
            state: TaskState::Queued,
            reason: "submitted".to_string(),
            last_error: None,
            log_file: log_file.display().to_string(),
            created_at: now,
            updated_at: now,
            finished_at: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskOrigin {
    pub pane_id: String,
    pub session_name: String,
    pub window_id: String,
    pub window_name: String,
    pub pane_current_path: String,
}

fn repo_slug(repo: &str) -> String {
    let leaf = repo
        .trim()
        .rsplit(['/', '\\'])
        .find(|part| !part.is_empty())
        .unwrap_or("");
    let leaf = leaf.strip_suffix(".git").unwrap_or(leaf);

    let mut slug = String::new();
    let mut previous_dash = false;

    for ch in leaf.to_ascii_lowercase().chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch);
            previous_dash = false;
            continue;
        }

        if !previous_dash {
            slug.push('-');
            previous_dash = true;
        }
    }

    let trimmed = slug.trim_matches('-').to_string();
    if trimmed.is_empty() {
        "repo".to_string()
    } else {
        trimmed
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventRecord {
    pub task_id: String,
    pub from_state: Option<TaskState>,
    pub to_state: TaskState,
    pub reason: String,
    pub timestamp: DateTime<Utc>,
}

impl EventRecord {
    pub fn submitted(task: &TaskRecord) -> Self {
        Self {
            task_id: task.id.clone(),
            from_state: None,
            to_state: TaskState::Queued,
            reason: "submitted".to_string(),
            timestamp: Utc::now(),
        }
    }

    pub fn transition(
        task: &TaskRecord,
        from_state: TaskState,
        to_state: TaskState,
        reason: String,
    ) -> Self {
        Self {
            task_id: task.id.clone(),
            from_state: Some(from_state),
            to_state,
            reason,
            timestamp: Utc::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::repo_slug;

    #[test]
    fn repo_slug_normalizes_special_characters() {
        assert_eq!(repo_slug("Owner/Repo.Name"), "repo-name");
        assert_eq!(repo_slug("core"), "core");
    }

    #[test]
    fn repo_slug_uses_leaf_component() {
        assert_eq!(repo_slug("owner/repo"), "repo");
        assert_eq!(repo_slug("git@github.com:owner/repo.git"), "repo");
    }

    #[test]
    fn repo_slug_falls_back_for_empty_input() {
        assert_eq!(repo_slug("   "), "repo");
        assert_eq!(repo_slug("---"), "repo");
    }
}
