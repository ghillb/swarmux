use crate::config::PaneSwitcherHighlight;
use crate::panes::PaneSnapshot;
use crate::panes_tui::PaneSwitcherState;
use crate::panes_tui::spawn_hydrator;
use crate::panes_tui_detail::{footer_line, header_summary_line};
use crate::store::Store;
use anyhow::{Context, Result, anyhow};
use crossterm::cursor::{Hide, Show};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::backend::CrosstermBackend;
use ratatui::prelude::*;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState};
use std::io::{self, IsTerminal};
use std::sync::mpsc;
use std::time::Duration;

const POLL_INTERVAL: Duration = Duration::from_millis(80);

const MUTED: Color = Color::Rgb(134, 144, 160);
const ACCENT: Color = Color::Rgb(88, 214, 255);
pub fn run(store: &Store, source_pane_id: Option<&str>) -> Result<()> {
    if !io::stdout().is_terminal() {
        return Err(anyhow!(
            "panes switch --tui requires an interactive terminal"
        ));
    }

    let mut session = TerminalSession::new()?;
    let mut state = PaneSwitcherState::load(store, source_pane_id)?;
    let highlight = store.paths().settings.ui.pane_switcher_highlight;
    let show_arrow = store.paths().settings.ui.pane_switcher_show_arrow;
    let (tx, rx) = mpsc::channel();
    spawn_hydrator(state.rows.clone(), tx);
    let mut selected = state.initial_selected(source_pane_id);
    state.selected = selected;

    session.terminal.draw(|frame| {
        draw(
            frame,
            &state,
            state.loaded_count,
            state.rows.len(),
            highlight,
            show_arrow,
        )
    })?;

    loop {
        let mut redraw = false;

        while let Ok(update) = rx.try_recv() {
            if state.apply_update(update) {
                selected = state.clamp_selected(selected);
                state.selected = selected;
                redraw = true;
            }
        }

        if event::poll(POLL_INTERVAL)? {
            match event::read()? {
                Event::Key(key) => match handle_key(key, &mut state, &mut selected) {
                    KeyAction::Quit => break,
                    KeyAction::Activate(target) => {
                        activate_pane(&target)?;
                        break;
                    }
                    KeyAction::None => redraw = true,
                },
                Event::Resize(_, _) => redraw = true,
                _ => {}
            }
        }

        if redraw {
            session.terminal.draw(|frame| {
                draw(
                    frame,
                    &state,
                    state.loaded_count,
                    state.rows.len(),
                    highlight,
                    show_arrow,
                )
            })?;
        }
    }

    Ok(())
}

#[derive(Debug)]
enum KeyAction {
    None,
    Quit,
    Activate(Box<PaneSnapshot>),
}

fn handle_key(key: KeyEvent, state: &mut PaneSwitcherState, selected: &mut usize) -> KeyAction {
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => KeyAction::Quit,
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => KeyAction::Quit,
        KeyCode::Up | KeyCode::Char('k') => {
            if !state.rows.is_empty() {
                *selected = if *selected == 0 {
                    state.rows.len() - 1
                } else {
                    *selected - 1
                };
            }
            state.selected = *selected;
            KeyAction::None
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if !state.rows.is_empty() {
                *selected = if *selected + 1 >= state.rows.len() {
                    0
                } else {
                    *selected + 1
                };
            }
            state.selected = *selected;
            KeyAction::None
        }
        KeyCode::Home => {
            *selected = 0;
            state.selected = *selected;
            KeyAction::None
        }
        KeyCode::End => {
            *selected = state.rows.len().saturating_sub(1);
            state.selected = *selected;
            KeyAction::None
        }
        KeyCode::Enter => state
            .rows
            .get(state.selected)
            .map(|entry| KeyAction::Activate(Box::new(entry.snapshot.clone())))
            .unwrap_or(KeyAction::None),
        _ => KeyAction::None,
    }
}

fn activate_pane(snapshot: &PaneSnapshot) -> Result<()> {
    run_tmux(["switch-client", "-t", &snapshot.session_name])?;
    run_tmux(["select-window", "-t", &snapshot.window_id])?;
    run_tmux(["select-pane", "-t", &snapshot.pane_id])?;

    Ok(())
}

fn run_tmux<const N: usize>(args: [&str; N]) -> Result<()> {
    let output = std::process::Command::new("tmux")
        .args(args)
        .output()
        .context("failed to run tmux")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(anyhow!("tmux failed: {stderr}"));
    }

    Ok(())
}

fn draw(
    frame: &mut Frame<'_>,
    state: &PaneSwitcherState,
    loaded: usize,
    total: usize,
    highlight: PaneSwitcherHighlight,
    show_arrow: bool,
) {
    let outer = Layout::vertical([
        Constraint::Length(2),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .split(frame.area());

    draw_header(frame, outer[0], loaded, total, state);

    draw_table(frame, outer[1], state, highlight, show_arrow);
    draw_footer(frame, outer[2]);
}

fn draw_header(
    frame: &mut Frame<'_>,
    area: Rect,
    loaded: usize,
    total: usize,
    state: &PaneSwitcherState,
) {
    let header = Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).split(area);

    let snapshot = state.rows.get(state.selected).map(|entry| &entry.snapshot);
    let status = if total == 0 {
        "empty"
    } else if loaded >= total {
        "ready"
    } else {
        "loading"
    };

    frame.render_widget(Paragraph::new(header_title_line(area.width)), header[0]);
    frame.render_widget(
        Paragraph::new(header_summary_line(snapshot, status)).block(
            Block::default()
                .borders(Borders::LEFT | Borders::RIGHT)
                .border_style(Style::default().fg(MUTED)),
        ),
        header[1],
    );
}

fn draw_table(
    frame: &mut Frame<'_>,
    area: Rect,
    state: &PaneSwitcherState,
    highlight: PaneSwitcherHighlight,
    show_arrow: bool,
) {
    let rows = state
        .rows
        .iter()
        .enumerate()
        .map(|(index, entry)| entry.row_cells(index == state.selected, highlight, show_arrow))
        .collect::<Vec<_>>();
    let table = Table::new(
        rows,
        [
            Constraint::Length(1),
            Constraint::Length(18),
            Constraint::Length(18),
            Constraint::Length(24),
            Constraint::Length(18),
            Constraint::Min(18),
        ],
    )
    .header(
        Row::new(vec![
            Cell::from(" "),
            Cell::from("Session"),
            Cell::from("Window"),
            Cell::from("Title"),
            Cell::from("Repo"),
            Cell::from("Git"),
        ])
        .style(Style::default().fg(MUTED).add_modifier(Modifier::BOLD)),
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(MUTED)),
    )
    .column_spacing(1);

    let mut table_state = TableState::new().with_selected(Some(state.selected));
    frame.render_stateful_widget(table, area, &mut table_state);
}

fn header_title_line(width: u16) -> Line<'static> {
    let title = "SWARMUX PANES";
    let title_len = title.chars().count();
    let min_len = title_len + 4;

    if (width as usize) < min_len {
        return Line::from(vec![Span::styled(
            title,
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        )]);
    }

    let fill = "─".repeat(width as usize - min_len);

    Line::from(vec![
        Span::styled("┌ ", Style::default().fg(MUTED)),
        Span::styled(
            title,
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" ", Style::default().fg(MUTED)),
        Span::styled(fill, Style::default().fg(MUTED)),
        Span::styled("┐", Style::default().fg(MUTED)),
    ])
}

fn draw_footer(frame: &mut Frame<'_>, area: Rect) {
    frame.render_widget(Paragraph::new(footer_line()), area);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::PaneSwitcherHighlight;
    use crate::panes::PaneSnapshot;
    use crate::panes_tui::PaneEntry;
    use crate::panes_tui::PaneSwitcherState;

    fn entry(name: &str) -> PaneEntry {
        PaneEntry {
            snapshot: PaneSnapshot {
                current: false,
                managed_by_swarmux: false,
                session_name: name.to_string(),
                window_id: "@1".to_string(),
                window_index: 0,
                window_name: "window".to_string(),
                pane_id: format!("%{name}"),
                pane_index: 0,
                pane_active: true,
                pane_current_path: "/tmp".to_string(),
                pane_current_command: "bash".to_string(),
                pane_title: "pane".to_string(),
                task: None,
                repo_root: None,
                repo: None,
                branch: None,
                git: None,
                label: String::new(),
            },
            metadata_loaded: false,
        }
    }

    #[test]
    fn handle_key_wraps_down_from_last_row_to_first() {
        let mut state = PaneSwitcherState {
            rows: vec![entry("a"), entry("b"), entry("c")],
            selected: 2,
            loaded_count: 0,
        };
        let mut selected = 2;

        let action = handle_key(
            KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE),
            &mut state,
            &mut selected,
        );

        assert!(matches!(action, KeyAction::None));
        assert_eq!(selected, 0);
        assert_eq!(state.selected, 0);
    }

    #[test]
    fn handle_key_wraps_up_from_first_row_to_last() {
        let mut state = PaneSwitcherState {
            rows: vec![entry("a"), entry("b"), entry("c")],
            selected: 0,
            loaded_count: 0,
        };
        let mut selected = 0;

        let action = handle_key(
            KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE),
            &mut state,
            &mut selected,
        );

        assert!(matches!(action, KeyAction::None));
        assert_eq!(selected, 2);
        assert_eq!(state.selected, 2);
    }

    #[test]
    fn solid_and_underline_are_distinct_styles() {
        let entry = PaneEntry {
            snapshot: PaneSnapshot {
                current: false,
                managed_by_swarmux: false,
                session_name: "session".to_string(),
                window_id: "@1".to_string(),
                window_index: 0,
                window_name: "window".to_string(),
                pane_id: "%1".to_string(),
                pane_index: 0,
                pane_active: true,
                pane_current_path: "/tmp".to_string(),
                pane_current_command: "bash".to_string(),
                pane_title: "pane".to_string(),
                task: None,
                repo_root: None,
                repo: None,
                branch: None,
                git: None,
                label: String::new(),
            },
            metadata_loaded: false,
        };

        let solid = ratatui::style::Styled::style(&entry.row_cells(
            true,
            PaneSwitcherHighlight::Solid,
            false,
        ));
        let underline = ratatui::style::Styled::style(&entry.row_cells(
            true,
            PaneSwitcherHighlight::Underline,
            false,
        ));

        assert_eq!(solid.bg, Some(ACCENT));
        assert_eq!(underline.bg, None);
        assert!(underline.add_modifier.contains(Modifier::UNDERLINED));
    }
}
