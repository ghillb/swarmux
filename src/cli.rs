use crate::config::TaskRuntime;
use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    Json,
    Text,
}

#[derive(Debug, Parser)]
#[command(name = "swarmux")]
#[command(about = "tmux-backed local swarm orchestration built for agents first")]
pub struct Cli {
    #[arg(long, global = true, value_enum, default_value_t = OutputFormat::Json)]
    pub output: OutputFormat,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    Schema,
    Doctor,
    Init,
    Paths,
    Panes(PanesArgs),
    Submit(SubmitArgs),
    Start(IdArgs),
    Delegate(SubmitArgs),
    Dispatch(DispatchArgs),
    List(ListArgs),
    Show(ShowArgs),
    Logs(LogsArgs),
    Notify(NotifyArgs),
    Wait(WaitArgs),
    Watch(WatchArgs),
    Send(SendArgs),
    SetRef(SetRefArgs),
    Attach(IdArgs),
    Stop(StopArgs),
    Reconcile,
    Prune(PruneArgs),
    Overview(OverviewArgs),
    Done(StateArgs),
    Fail(FailArgs),
}

#[derive(Debug, clap::Args)]
pub struct SubmitArgs {
    #[arg(long)]
    pub dry_run: bool,

    #[arg(long)]
    pub human: bool,

    #[arg(long, conflicts_with = "json_file")]
    pub json: Option<String>,

    #[arg(long, conflicts_with = "json")]
    pub json_file: Option<PathBuf>,
}

#[derive(Debug, clap::Args)]
pub struct DispatchArgs {
    #[arg(long)]
    pub dry_run: bool,

    #[arg(long)]
    pub human: bool,

    #[arg(long)]
    pub title: Option<String>,

    #[arg(long)]
    pub repo_ref: Option<String>,

    #[arg(long)]
    pub repo_root: Option<String>,

    #[arg(long, value_enum, default_value = "auto")]
    pub mode: DispatchMode,

    #[arg(long)]
    pub connected: bool,

    #[arg(long, conflicts_with = "runtime")]
    pub mirrored: bool,

    #[arg(long, value_enum)]
    pub runtime: Option<TaskRuntime>,

    #[arg(long)]
    pub prompt: Option<String>,

    #[arg(long)]
    pub pane_id: Option<String>,

    #[arg(long)]
    pub agent: Option<String>,

    #[arg(long)]
    pub worktree: Option<String>,

    #[arg(long)]
    pub session: Option<String>,

    #[arg(long)]
    pub priority: Option<u8>,

    #[arg(long)]
    pub external_ref: Option<String>,

    #[arg(last = true)]
    pub command: Vec<String>,
}

#[derive(Debug, Clone, ValueEnum)]
pub enum DispatchMode {
    Auto,
    Manual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum OverviewScope {
    Terminal,
    NonTerminal,
    All,
}

#[derive(Debug, clap::Args)]
pub struct ListArgs {
    #[arg(long)]
    pub fields: Option<String>,
}

#[derive(Debug, clap::Args)]
pub struct ShowArgs {
    pub id: String,

    #[arg(long)]
    pub fields: Option<String>,
}

#[derive(Debug, clap::Args)]
pub struct IdArgs {
    pub id: String,
}

#[derive(Debug, clap::Args)]
pub struct LogsArgs {
    pub id: String,

    #[arg(long, default_value_t = 200)]
    pub lines: usize,

    #[arg(long)]
    pub raw: bool,
}

#[derive(Debug, clap::Args)]
pub struct NotifyArgs {
    #[arg(long)]
    pub tmux: bool,

    #[arg(long)]
    pub show_tokens: bool,
}

#[derive(Debug, clap::Args)]
pub struct WaitArgs {
    pub ids: Vec<String>,

    #[arg(long)]
    pub states: Option<String>,

    #[arg(long, default_value_t = 2_000)]
    pub interval_ms: u64,

    #[arg(long)]
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, clap::Args)]
pub struct WatchArgs {
    pub ids: Vec<String>,

    #[arg(long)]
    pub states: Option<String>,

    #[arg(long, default_value_t = 2_000)]
    pub interval_ms: u64,

    #[arg(long)]
    pub timeout_ms: Option<u64>,

    #[arg(long, default_value_t = 30)]
    pub lines: usize,

    #[arg(long)]
    pub raw: bool,
}

#[derive(Debug, clap::Args)]
pub struct SendArgs {
    pub id: String,

    #[arg(long)]
    pub input: String,
}

#[derive(Debug, clap::Args)]
pub struct SetRefArgs {
    pub id: String,
    pub url: String,
}

#[derive(Debug, clap::Args)]
pub struct StopArgs {
    pub id: String,

    #[arg(long)]
    pub kill: bool,

    #[arg(long)]
    pub reason: Option<String>,
}

#[derive(Debug, clap::Args)]
pub struct PruneArgs {
    #[arg(long)]
    pub apply: bool,
}

#[derive(Debug, clap::Args)]
pub struct OverviewArgs {
    #[arg(long)]
    pub title: bool,

    #[arg(long)]
    pub once: bool,

    #[arg(long, conflicts_with_all = ["once", "title"])]
    pub tui: bool,

    #[arg(long, value_enum, default_value = "non-terminal")]
    pub scope: OverviewScope,
}

#[derive(Debug, clap::Args)]
pub struct PanesArgs {
    #[command(subcommand)]
    pub command: Option<PanesCommand>,
}

#[derive(Debug, Subcommand)]
pub enum PanesCommand {
    SyncTmuxMeta,
    Switch(SwitchArgs),
}

#[derive(Debug, clap::Args)]
pub struct SwitchArgs {
    #[arg(long)]
    pub tui: bool,

    #[arg(long = "tui-sidebar", conflicts_with = "tui")]
    pub tui_sidebar: bool,

    #[arg(long = "launch-sidebar", conflicts_with = "tui")]
    pub launch_sidebar: bool,

    #[arg(long)]
    pub pane_id: Option<String>,
}

#[derive(Debug, clap::Args)]
pub struct StateArgs {
    pub id: String,

    #[arg(long, default_value = "manual_done")]
    pub reason: String,
}

#[derive(Debug, clap::Args)]
pub struct FailArgs {
    pub id: String,

    #[arg(long, default_value = "manual_fail")]
    pub reason: String,

    #[arg(long)]
    pub error: String,
}
