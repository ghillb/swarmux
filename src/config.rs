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
    pub tmux: TmuxConfig,
    #[serde(default)]
    pub ui: UiConfig,
    #[serde(default)]
    pub connected: ConnectedConfig,
    #[serde(default)]
    pub agents: BTreeMap<String, AgentConfig>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct TmuxConfig {
    #[serde(default)]
    pub session_ignore: Vec<String>,
}

impl TmuxConfig {
    pub fn ignore_filter(&self) -> String {
        tmux_session_ignore_filter(&self.session_ignore)
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct UiConfig {
    #[serde(default)]
    pub pane_switcher_highlight: PaneSwitcherHighlight,
    #[serde(default)]
    pub pane_switcher_show_arrow: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaneSwitcherHighlight {
    Solid,
    #[default]
    Underline,
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

fn tmux_session_ignore_filter(patterns: &[String]) -> String {
    let mut clauses = patterns
        .iter()
        .map(String::as_str)
        .map(str::trim)
        .filter(|pattern| !pattern.is_empty())
        .map(|pattern| format!("#{{!=:#{{m:{pattern},#{{session_name}}}},1}}"));

    match clauses.next() {
        Some(first) => clauses.fold(first, |expr, clause| format!("#{{&&:{expr},{clause}}}")),
        None => "1".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn tmux_config_defaults_have_no_session_ignore_list() {
        assert!(TmuxConfig::default().session_ignore.is_empty());
    }

    #[test]
    fn tmux_config_builds_a_true_filter_when_the_list_is_empty() {
        let config = TmuxConfig {
            session_ignore: Vec::new(),
        };

        assert_eq!(config.ignore_filter(), "1");
    }

    #[test]
    fn file_config_loads_a_custom_tmux_session_ignore_list() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(
            &path,
            r#"
[tmux]
session_ignore = ["alpha-*", "beta-*"]
"#,
        )
        .unwrap();

        let config = load_file_config(&path).unwrap();

        assert_eq!(
            config.tmux.session_ignore,
            vec!["alpha-*".to_string(), "beta-*".to_string()]
        );
    }

    #[test]
    fn file_config_defaults_tmux_session_ignore_when_missing() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "[connected]\nagent = \"codex\"\n").unwrap();

        let config = load_file_config(&path).unwrap();

        assert!(config.tmux.session_ignore.is_empty());
        assert!(matches!(
            config.ui.pane_switcher_highlight,
            PaneSwitcherHighlight::Underline
        ));
        assert!(!config.ui.pane_switcher_show_arrow);
    }

    #[test]
    fn file_config_loads_pane_switcher_highlight_mode() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(
            &path,
            "[ui]\npane_switcher_highlight = \"solid\"\npane_switcher_show_arrow = true\n",
        )
        .unwrap();

        let config = load_file_config(&path).unwrap();

        assert!(matches!(
            config.ui.pane_switcher_highlight,
            PaneSwitcherHighlight::Solid
        ));
        assert!(config.ui.pane_switcher_show_arrow);
    }
}
