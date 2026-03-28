use crate::cli::OverviewScope;
use crate::model::TaskState;
use crate::runtime;
use crate::store::Store;
use crate::{overview_tui_data as data, overview_tui_render as render};
use anyhow::{Result, anyhow};
use crossterm::cursor::{Hide, Show};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::backend::CrosstermBackend;
use ratatui::prelude::*;
use std::io::{self, IsTerminal};
use std::time::{Duration, Instant};

const REFRESH_INTERVAL: Duration = Duration::from_secs(2);
const POLL_INTERVAL: Duration = Duration::from_millis(200);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Tab {
    Tasks,
    Stats,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum TasksFilter {
    Active,
    Terminal,
    All,
}

impl TasksFilter {
    pub(super) fn from_scope(scope: OverviewScope) -> Self {
        match scope {
            OverviewScope::Terminal => Self::Terminal,
            OverviewScope::NonTerminal => Self::Active,
            OverviewScope::All => Self::All,
        }
    }

    pub(super) fn next(self) -> Self {
        match self {
            Self::Active => Self::Terminal,
            Self::Terminal => Self::All,
            Self::All => Self::Active,
        }
    }

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Terminal => "terminal",
            Self::All => "all",
        }
    }

    pub(super) fn matches(self, state: &TaskState) -> bool {
        match self {
            Self::Active => !state.is_terminal(),
            Self::Terminal => state.is_terminal(),
            Self::All => true,
        }
    }
}

#[derive(Debug)]
pub(super) struct AppState {
    pub(super) tab: Tab,
    pub(super) tasks_filter: TasksFilter,
    pub(super) tasks_selected: usize,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            tab: Tab::Tasks,
            tasks_filter: TasksFilter::Active,
            tasks_selected: 0,
        }
    }
}

enum KeyAction {
    None,
    Quit,
    Refresh,
    FocusSession(String),
    Stop { id: String, kill: bool },
}

pub fn run(store: &Store, scope: OverviewScope) -> Result<()> {
    if !io::stdout().is_terminal() {
        return Err(anyhow!("overview --tui requires an interactive terminal"));
    }

    let mut session = TerminalSession::new()?;
    let mut app = AppState {
        tasks_filter: TasksFilter::from_scope(scope),
        ..AppState::default()
    };
    let mut data = data::DashboardData::load(store)?;
    app.clamp_to(data.filtered_tasks(app.tasks_filter).len());
    let mut last_refresh = Instant::now();

    loop {
        session
            .terminal
            .draw(|frame| render::draw(frame, &app, &data))?;

        let elapsed = last_refresh.elapsed();
        let poll = if elapsed >= REFRESH_INTERVAL {
            Duration::from_millis(0)
        } else {
            POLL_INTERVAL.min(REFRESH_INTERVAL - elapsed)
        };

        if event::poll(poll)? {
            match event::read()? {
                Event::Key(key) => match handle_key(&mut app, key, &data) {
                    KeyAction::Quit => break,
                    KeyAction::Refresh => {
                        data = data::DashboardData::load(store)?;
                        app.clamp_to(data.filtered_tasks(app.tasks_filter).len());
                        last_refresh = Instant::now();
                    }
                    KeyAction::FocusSession(session) => {
                        runtime::focus_task_session(&session)?;
                        break;
                    }
                    KeyAction::Stop { id, kill } => {
                        crate::stop_task(store, &id, kill, None)?;
                        data = data::DashboardData::load(store)?;
                        app.clamp_to(data.filtered_tasks(app.tasks_filter).len());
                        last_refresh = Instant::now();
                    }
                    KeyAction::None => {}
                },
                Event::Resize(_, _) => {}
                _ => {}
            }
        }

        if last_refresh.elapsed() >= REFRESH_INTERVAL {
            data = data::DashboardData::load(store)?;
            app.clamp_to(data.filtered_tasks(app.tasks_filter).len());
            last_refresh = Instant::now();
        }
    }

    Ok(())
}

struct TerminalSession {
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
}

impl TerminalSession {
    fn new() -> Result<Self> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, Hide)?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;
        Ok(Self { terminal })
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(self.terminal.backend_mut(), LeaveAlternateScreen, Show);
        let _ = self.terminal.show_cursor();
    }
}

impl AppState {
    fn clamp_to(&mut self, tasks_len: usize) {
        self.tasks_selected = clamp_index(self.tasks_selected, tasks_len);
    }

    fn selected_mut(&mut self) -> Option<&mut usize> {
        match self.tab {
            Tab::Tasks => Some(&mut self.tasks_selected),
            Tab::Stats => None,
        }
    }
}

fn handle_key(app: &mut AppState, key: KeyEvent, data: &data::DashboardData) -> KeyAction {
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => KeyAction::Quit,
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => KeyAction::Quit,
        KeyCode::Left | KeyCode::Char('h') => {
            app.tab = match app.tab {
                Tab::Tasks => Tab::Stats,
                Tab::Stats => Tab::Tasks,
            };
            KeyAction::None
        }
        KeyCode::Right | KeyCode::Char('l') | KeyCode::Tab => {
            app.tab = match app.tab {
                Tab::Tasks => Tab::Stats,
                Tab::Stats => Tab::Tasks,
            };
            KeyAction::None
        }
        KeyCode::Char('f') if matches!(app.tab, Tab::Tasks) => {
            app.tasks_filter = app.tasks_filter.next();
            app.clamp_to(data.filtered_tasks(app.tasks_filter).len());
            KeyAction::None
        }
        KeyCode::Enter if matches!(app.tab, Tab::Tasks) => {
            let origin = data
                .filtered_tasks(app.tasks_filter)
                .get(app.tasks_selected)
                .and_then(|task| (!task.state.is_terminal()).then(|| task.session.clone()))
                .flatten();

            match origin {
                Some(session) => KeyAction::FocusSession(session),
                None => KeyAction::None,
            }
        }
        KeyCode::Char('x') if matches!(app.tab, Tab::Tasks) => {
            match data
                .filtered_tasks(app.tasks_filter)
                .get(app.tasks_selected)
                .filter(|task| !task.state.is_terminal())
            {
                Some(task) => KeyAction::Stop {
                    id: task.id.clone(),
                    kill: false,
                },
                None => KeyAction::None,
            }
        }
        KeyCode::Char('X') if matches!(app.tab, Tab::Tasks) => {
            match data
                .filtered_tasks(app.tasks_filter)
                .get(app.tasks_selected)
                .filter(|task| !task.state.is_terminal())
            {
                Some(task) => KeyAction::Stop {
                    id: task.id.clone(),
                    kill: true,
                },
                None => KeyAction::None,
            }
        }
        KeyCode::Up | KeyCode::Char('k') => {
            let len = data.filtered_tasks(app.tasks_filter).len();
            if let Some(selected) = app.selected_mut() {
                move_selection(selected, len, -1);
            }
            KeyAction::None
        }
        KeyCode::Down | KeyCode::Char('j') => {
            let len = data.filtered_tasks(app.tasks_filter).len();
            if let Some(selected) = app.selected_mut() {
                move_selection(selected, len, 1);
            }
            KeyAction::None
        }
        KeyCode::PageUp => {
            let len = data.filtered_tasks(app.tasks_filter).len();
            if let Some(selected) = app.selected_mut() {
                move_selection(selected, len, -8);
            }
            KeyAction::None
        }
        KeyCode::PageDown => {
            let len = data.filtered_tasks(app.tasks_filter).len();
            if let Some(selected) = app.selected_mut() {
                move_selection(selected, len, 8);
            }
            KeyAction::None
        }
        KeyCode::Home => {
            if let Some(selected) = app.selected_mut() {
                *selected = 0;
            }
            KeyAction::None
        }
        KeyCode::End => {
            let len = data.filtered_tasks(app.tasks_filter).len();
            if let Some(selected) = app.selected_mut() {
                *selected = len.saturating_sub(1);
            }
            KeyAction::None
        }
        KeyCode::Char('r') => KeyAction::Refresh,
        _ => KeyAction::None,
    }
}

fn move_selection(selected: &mut usize, len: usize, delta: isize) {
    if len == 0 {
        *selected = 0;
        return;
    }

    let next = ((*selected as isize) + delta).clamp(0, (len - 1) as isize) as usize;
    *selected = next;
}

fn clamp_index(index: usize, len: usize) -> usize {
    if len == 0 { 0 } else { index.min(len - 1) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::TaskRuntime;
    use crate::model::{TaskMode, TaskRecord, TaskState};
    use chrono::Utc;

    fn task(title: &str) -> TaskRecord {
        TaskRecord {
            id: format!("id-{title}"),
            title: title.to_string(),
            repo: "core".to_string(),
            repo_root: "/tmp/core".to_string(),
            mode: TaskMode::Manual,
            runtime: TaskRuntime::Tui,
            branch: None,
            worktree: None,
            session: Some("session-1".to_string()),
            command: vec!["echo".to_string(), title.to_string()],
            priority: 1,
            external_ref: None,
            origin: None,
            state: TaskState::Running,
            reason: "running".to_string(),
            last_error: None,
            log_file: "/tmp/core/log".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            finished_at: None,
        }
    }

    #[test]
    fn move_selection_clamps_to_bounds() {
        let mut selected = 1;
        move_selection(&mut selected, 3, 1);
        assert_eq!(selected, 2);
        move_selection(&mut selected, 3, 1);
        assert_eq!(selected, 2);
        move_selection(&mut selected, 3, -10);
        assert_eq!(selected, 0);
    }

    #[test]
    fn tasks_filter_cycles_and_maps_scope() {
        assert_eq!(
            TasksFilter::from_scope(OverviewScope::NonTerminal),
            TasksFilter::Active
        );
        assert_eq!(
            TasksFilter::from_scope(OverviewScope::Terminal),
            TasksFilter::Terminal
        );
        assert_eq!(
            TasksFilter::from_scope(OverviewScope::All),
            TasksFilter::All
        );
        assert_eq!(TasksFilter::Active.next(), TasksFilter::Terminal);
        assert_eq!(TasksFilter::Terminal.next(), TasksFilter::All);
        assert_eq!(TasksFilter::All.next(), TasksFilter::Active);
        assert!(!TasksFilter::Active.matches(&TaskState::Succeeded));
        assert!(TasksFilter::Terminal.matches(&TaskState::Succeeded));
        assert!(TasksFilter::All.matches(&TaskState::Succeeded));
    }

    #[test]
    fn clamp_to_limits_tasks_selection() {
        let data = data::DashboardData {
            generated_at: Utc::now(),
            all_tasks: vec![task("a"), task("b")],
            all_summary: data::TaskSummary::from(&[task("a"), task("b")]),
            repo_counts: vec![],
            runtime_counts: vec![],
        };
        let mut app = AppState {
            tab: Tab::Tasks,
            tasks_filter: TasksFilter::Active,
            tasks_selected: 9,
        };

        app.clamp_to(data.filtered_tasks(app.tasks_filter).len());

        assert_eq!(app.tasks_selected, 1);
    }

    #[test]
    fn enter_focuses_active_task_session() {
        let mut active_task = task("a");
        active_task.session = Some("session-a".to_string());

        let data = data::DashboardData {
            generated_at: Utc::now(),
            all_tasks: vec![active_task],
            all_summary: data::TaskSummary::from(&[task("a")]),
            repo_counts: vec![],
            runtime_counts: vec![],
        };
        let mut app = AppState {
            tab: Tab::Tasks,
            tasks_filter: TasksFilter::Active,
            tasks_selected: 0,
        };

        let action = handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            &data,
        );

        assert!(matches!(action, KeyAction::FocusSession(_)));
    }

    #[test]
    fn x_keys_stop_active_task_only() {
        let data = data::DashboardData {
            generated_at: Utc::now(),
            all_tasks: vec![task("a")],
            all_summary: data::TaskSummary::from(&[task("a")]),
            repo_counts: vec![],
            runtime_counts: vec![],
        };
        let mut app = AppState {
            tab: Tab::Tasks,
            tasks_filter: TasksFilter::Active,
            tasks_selected: 0,
        };

        let interrupt = handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE),
            &data,
        );
        assert!(matches!(interrupt, KeyAction::Stop { kill: false, .. }));

        let kill = handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('X'), KeyModifiers::NONE),
            &data,
        );
        assert!(matches!(kill, KeyAction::Stop { kill: true, .. }));
    }
}
