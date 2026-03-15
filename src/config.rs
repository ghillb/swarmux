use anyhow::{Context, Result};
use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendKind {
    Files,
    Beads,
}

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub home: PathBuf,
    pub config_home: PathBuf,
    pub backend: BackendKind,
    pub settings: FileConfig,
}

#[derive(Debug, Clone, Serialize)]
pub struct PathsInfo {
    pub home: String,
    pub backend: String,
    pub tasks_dir: String,
    pub logs_dir: String,
    pub locks_dir: String,
    pub config_dir: String,
    pub events_file: String,
    pub notify_file: String,
    pub config_file: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct FileConfig {
    pub home: Option<PathBuf>,
    pub backend: Option<String>,
    #[serde(default)]
    pub connected: ConnectedConfig,
    #[serde(default)]
    pub agents: BTreeMap<String, AgentConfig>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ConnectedConfig {
    #[serde(default)]
    pub command: Vec<String>,
    pub agent: Option<String>,
    #[serde(default)]
    pub runtime: TaskRuntime,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct AgentConfig {
    #[serde(default)]
    pub command: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum, Default)]
#[serde(rename_all = "snake_case")]
pub enum TaskRuntime {
    #[default]
    Headless,
    Mirrored,
    Tui,
}

impl AppConfig {
    pub fn from_env() -> Result<Self> {
        let config_home = std::env::var_os("SWARMUX_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(default_config_home)
            .unwrap_or_else(|| PathBuf::from(".config"));
        let config_dir = config_home.join("swarmux");
        let settings = load_file_config(&config_dir.join("config.toml"))?;
        let home = std::env::var_os("SWARMUX_HOME")
            .map(PathBuf::from)
            .or_else(|| settings.home.clone())
            .or_else(default_state_home)
            .unwrap_or_else(|| PathBuf::from(".swarmux"));
        let backend = match std::env::var("SWARMUX_BACKEND") {
            Ok(value) => parse_backend_kind(&value)?,
            Err(_) => settings
                .backend
                .as_deref()
                .map(parse_backend_kind)
                .transpose()?
                .unwrap_or(BackendKind::Files),
        };

        Ok(Self {
            home,
            config_home,
            backend,
            settings,
        })
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

    pub fn config_dir(&self) -> PathBuf {
        self.config_home.join("swarmux")
    }

    pub fn events_file(&self) -> PathBuf {
        self.home.join("events.jsonl")
    }

    pub fn notify_file(&self) -> PathBuf {
        self.home.join("notify.json")
    }

    pub fn config_file(&self) -> PathBuf {
        self.config_dir().join("config.toml")
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
            config_dir: self.config_dir().display().to_string(),
            events_file: self.events_file().display().to_string(),
            notify_file: self.notify_file().display().to_string(),
            config_file: self.config_file().display().to_string(),
        }
    }
}

fn load_file_config(path: &std::path::Path) -> Result<FileConfig> {
    if !path.exists() {
        return Ok(FileConfig::default());
    }

    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read config file: {}", path.display()))?;
    toml::from_str(&raw).with_context(|| format!("failed to parse config file: {}", path.display()))
}

fn default_state_home() -> Option<PathBuf> {
    if let Some(xdg) = std::env::var_os("XDG_STATE_HOME") {
        return Some(PathBuf::from(xdg).join("swarmux"));
    }

    dirs::home_dir().map(|home| home.join(".local").join("state").join("swarmux"))
}

fn default_config_home() -> Option<PathBuf> {
    dirs::config_dir()
}

fn parse_backend_kind(raw: &str) -> Result<BackendKind> {
    match raw {
        "files" => Ok(BackendKind::Files),
        "beads" => Ok(BackendKind::Beads),
        _ => Err(anyhow::anyhow!("invalid backend: {raw}")),
    }
}
