use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendKind {
    Files,
    Beads,
}

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub home: PathBuf,
    pub backend: BackendKind,
}

#[derive(Debug, Clone, Serialize)]
pub struct PathsInfo {
    pub home: String,
    pub backend: String,
    pub tasks_dir: String,
    pub logs_dir: String,
    pub locks_dir: String,
    pub events_file: String,
    pub notify_file: String,
}

impl AppConfig {
    pub fn from_env() -> Self {
        let home = std::env::var_os("SWARMUX_HOME")
            .map(PathBuf::from)
            .or_else(default_home)
            .unwrap_or_else(|| PathBuf::from(".swarmux"));

        let backend = match std::env::var("SWARMUX_BACKEND")
            .unwrap_or_else(|_| "files".to_string())
            .as_str()
        {
            "beads" => BackendKind::Beads,
            _ => BackendKind::Files,
        };

        Self { home, backend }
    }

    pub fn tasks_dir(&self) -> PathBuf {
        self.home.join("tasks")
    }

    pub fn logs_dir(&self) -> PathBuf {
        self.home.join("logs")
    }

    pub fn locks_dir(&self) -> PathBuf {
        self.home.join("locks")
    }

    pub fn events_file(&self) -> PathBuf {
        self.home.join("events.jsonl")
    }

    pub fn notify_file(&self) -> PathBuf {
        self.home.join("notify.json")
    }

    pub fn paths_info(&self) -> PathsInfo {
        PathsInfo {
            home: self.home.display().to_string(),
            backend: match self.backend {
                BackendKind::Files => "files".to_string(),
                BackendKind::Beads => "beads".to_string(),
            },
            tasks_dir: self.tasks_dir().display().to_string(),
            logs_dir: self.logs_dir().display().to_string(),
            locks_dir: self.locks_dir().display().to_string(),
            events_file: self.events_file().display().to_string(),
            notify_file: self.notify_file().display().to_string(),
        }
    }
}

fn default_home() -> Option<PathBuf> {
    if let Some(xdg) = std::env::var_os("XDG_STATE_HOME") {
        return Some(PathBuf::from(xdg).join("swarmux"));
    }

    dirs::home_dir().map(|home| home.join(".local").join("state").join("swarmux"))
}
