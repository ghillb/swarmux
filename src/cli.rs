use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Debug, Clone, ValueEnum)]
pub enum OutputFormat {
    Json,
    Text,
}

#[derive(Debug, Parser)]
#[command(name = "swarmux")]
#[command(about = "tmux-backed local swarm orchestration built for agents first")]
pub struct Cli {
    #[arg(long, global = true, value_enum, default_value_t = OutputFormat::Text)]
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
    Submit(SubmitArgs),
    Start(IdArgs),
    Delegate(SubmitArgs),
    Dispatch(DispatchArgs),
    List(ListArgs),
    Show(ShowArgs),
    Logs(LogsArgs),
    Notify(NotifyArgs),
    Watch(WatchArgs),
    Send(SendArgs),
    Attach(IdArgs),
    Stop(StopArgs),
    Reconcile,
    Prune(PruneArgs),
    Popup(PopupArgs),
    Done(StateArgs),
    Fail(FailArgs),
}

#[derive(Debug, clap::Args)]
pub struct SubmitArgs {
    #[arg(long)]
    pub dry_run: bool,

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
    pub title: Option<String>,

    #[arg(long)]
    pub repo_ref: String,

    #[arg(long)]
    pub repo_root: String,

    #[arg(long, value_enum, default_value = "auto")]
    pub mode: DispatchMode,

    #[arg(long)]
    pub worktree: Option<String>,

    #[arg(long)]
    pub session: Option<String>,

    #[arg(long)]
    pub priority: Option<u8>,

    #[arg(long)]
    pub external_ref: Option<String>,

    #[arg(last = true, required = true)]
    pub command: Vec<String>,
}

#[derive(Debug, Clone, ValueEnum)]
pub enum DispatchMode {
    Auto,
    Manual,
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
}

#[derive(Debug, clap::Args)]
pub struct WatchArgs {
    #[arg(long)]
    pub tmux: bool,

    #[arg(long, default_value_t = 2_000)]
    pub interval_ms: u64,

    #[arg(long)]
    pub max_iterations: Option<u64>,
}

#[derive(Debug, clap::Args)]
pub struct SendArgs {
    pub id: String,

    #[arg(long)]
    pub input: String,
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
pub struct PopupArgs {
    #[arg(long)]
    pub title: bool,

    #[arg(long)]
    pub once: bool,
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
