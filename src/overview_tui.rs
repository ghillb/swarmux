use crate::cli::OverviewScope;
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
    Overview,
    Operational,
    ClientAll,
}

#[derive(Debug)]
pub(super) struct AppState {
    pub(super) tab: Tab,
    pub(super) overview_selected: usize,
    pub(super) operational_selected: usize,
    pub(super) client_selected: usize,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            tab: Tab::Overview,
            overview_selected: 0,
            operational_selected: 0,
            client_selected: 0,
        }
    }
}

enum KeyAction {
    None,
    Quit,
    Refresh,
}

pub fn run(store: &Store, scope: OverviewScope) -> Result<()> {
    if !io::stdout().is_terminal() {
        return Err(anyhow!("overview --tui requires an interactive terminal"));
    }

    let mut session = TerminalSession::new()?;
    let mut app = AppState::default();
    let mut data = data::DashboardData::load(store, scope)?;
    app.clamp_to(&data);
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
                        data = data::DashboardData::load(store, scope)?;
                        app.clamp_to(&data);
                        last_refresh = Instant::now();
                    }
                    KeyAction::None => {}
                },
                Event::Resize(_, _) => {}
                _ => {}
            }
        }

        if last_refresh.elapsed() >= REFRESH_INTERVAL {
            data = data::DashboardData::load(store, scope)?;
            app.clamp_to(&data);
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
    fn clamp_to(&mut self, data: &data::DashboardData) {
        self.overview_selected = clamp_index(self.overview_selected, data.visible_tasks.len());
        self.operational_selected = clamp_index(self.operational_selected, data.all_tasks.len());
        self.client_selected = clamp_index(self.client_selected, data.all_tasks.len());
    }

    fn selected_mut(&mut self) -> &mut usize {
        match self.tab {
            Tab::Overview => &mut self.overview_selected,
            Tab::Operational => &mut self.operational_selected,
            Tab::ClientAll => &mut self.client_selected,
        }
    }
}

fn handle_key(app: &mut AppState, key: KeyEvent, data: &data::DashboardData) -> KeyAction {
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => KeyAction::Quit,
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => KeyAction::Quit,
        KeyCode::Left | KeyCode::Char('h') => {
            app.tab = match app.tab {
                Tab::Overview => Tab::ClientAll,
                Tab::Operational => Tab::Overview,
                Tab::ClientAll => Tab::Operational,
            };
            KeyAction::None
        }
        KeyCode::Right | KeyCode::Char('l') | KeyCode::Tab => {
            app.tab = match app.tab {
                Tab::Overview => Tab::Operational,
                Tab::Operational => Tab::ClientAll,
                Tab::ClientAll => Tab::Overview,
            };
            KeyAction::None
        }
        KeyCode::Up | KeyCode::Char('k') => {
            let len = tab_len(app.tab, data);
            move_selection(app.selected_mut(), len, -1);
            KeyAction::None
        }
        KeyCode::Down | KeyCode::Char('j') => {
            let len = tab_len(app.tab, data);
            move_selection(app.selected_mut(), len, 1);
            KeyAction::None
        }
        KeyCode::PageUp => {
            let len = tab_len(app.tab, data);
            move_selection(app.selected_mut(), len, -8);
            KeyAction::None
        }
        KeyCode::PageDown => {
            let len = tab_len(app.tab, data);
            move_selection(app.selected_mut(), len, 8);
            KeyAction::None
        }
        KeyCode::Home => {
            *app.selected_mut() = 0;
            KeyAction::None
        }
        KeyCode::End => {
            let len = tab_len(app.tab, data);
            *app.selected_mut() = len.saturating_sub(1);
            KeyAction::None
        }
        KeyCode::Char('r') => KeyAction::Refresh,
        _ => KeyAction::None,
    }
}

fn tab_len(tab: Tab, data: &data::DashboardData) -> usize {
    match tab {
        Tab::Overview => data.visible_tasks.len(),
        Tab::Operational | Tab::ClientAll => data.all_tasks.len(),
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
    fn clamp_to_limits_each_tab_selection() {
        let data = data::DashboardData {
            generated_at: Utc::now(),
            scope: OverviewScope::All,
            visible_tasks: vec![task("a")],
            all_tasks: vec![task("a"), task("b")],
            visible_summary: data::TaskSummary::from(&[task("a")]),
            all_summary: data::TaskSummary::from(&[task("a"), task("b")]),
            repo_counts: vec![],
            runtime_counts: vec![],
        };
        let mut app = AppState {
            tab: Tab::Overview,
            overview_selected: 9,
            operational_selected: 8,
            client_selected: 7,
        };

        app.clamp_to(&data);

        assert_eq!(app.overview_selected, 0);
        assert_eq!(app.operational_selected, 1);
        assert_eq!(app.client_selected, 1);
    }
}
