use crate::overview_tui_helpers::truncate;
use crate::panes::PaneSnapshot;
use crate::panes_support::{pane_row, task_state_label};
use crate::panes_tui::{PaneSwitcherState, spawn_hydrator};
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
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table, Wrap};
use std::io::{self, IsTerminal};
use std::sync::mpsc;
use std::time::Duration;

const POLL_INTERVAL: Duration = Duration::from_millis(80);

const TEXT: Color = Color::Rgb(236, 239, 244);
const MUTED: Color = Color::Rgb(134, 144, 160);
const ACCENT: Color = Color::Rgb(88, 214, 255);
const GOOD: Color = Color::Rgb(96, 255, 160);
const WARN: Color = Color::Rgb(255, 204, 102);
pub fn run(store: &Store, source_pane_id: Option<&str>) -> Result<()> {
    if !io::stdout().is_terminal() {
        return Err(anyhow!(
            "panes switch --tui requires an interactive terminal"
        ));
    }

    let mut session = TerminalSession::new()?;
    let mut state = PaneSwitcherState::load(store, source_pane_id)?;
    let (tx, rx) = mpsc::channel();
    spawn_hydrator(state.rows.clone(), tx);
    let mut selected = state.initial_selected(source_pane_id);
    state.selected = selected;

    session
        .terminal
        .draw(|frame| draw(frame, &state, state.loaded_count, state.rows.len()))?;

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
            session
                .terminal
                .draw(|frame| draw(frame, &state, state.loaded_count, state.rows.len()))?;
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
                *selected = selected.saturating_sub(1);
            }
            state.selected = *selected;
            KeyAction::None
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if !state.rows.is_empty() {
                *selected = (*selected + 1).min(state.rows.len() - 1);
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

fn draw(frame: &mut Frame<'_>, state: &PaneSwitcherState, loaded: usize, total: usize) {
    let outer = Layout::vertical([
        Constraint::Length(5),
        Constraint::Min(0),
        Constraint::Length(2),
    ])
    .split(frame.area());

    draw_header(frame, outer[0], loaded, total, state);

    let body = if outer[1].width >= 124 {
        Layout::horizontal([Constraint::Percentage(68), Constraint::Percentage(32)]).split(outer[1])
    } else {
        Layout::vertical([Constraint::Percentage(60), Constraint::Percentage(40)]).split(outer[1])
    };

    draw_table(frame, body[0], state);
    draw_detail(frame, body[1], state);
    draw_footer(frame, outer[2]);
}

fn draw_header(
    frame: &mut Frame<'_>,
    area: Rect,
    loaded: usize,
    total: usize,
    state: &PaneSwitcherState,
) {
    let status = if total == 0 {
        "empty".to_string()
    } else if loaded >= total {
        "ready".to_string()
    } else {
        format!("loading {loaded}/{total}")
    };
    let selected = state
        .rows
        .get(state.selected)
        .map(|entry| truncate(&entry.snapshot.session_name, 28))
        .unwrap_or_else(|| "none".to_string());

    let title = Paragraph::new(vec![
        Line::from(vec![
            Span::styled(
                "SWARMUX",
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled(
                "PANES",
                Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("state ", Style::default().fg(MUTED)),
            Span::styled(status, Style::default().fg(WARN)),
            Span::raw("  "),
            Span::styled("selected ", Style::default().fg(MUTED)),
            Span::styled(selected, Style::default().fg(GOOD)),
        ]),
        Line::from(vec![
            Span::styled("keys ", Style::default().fg(MUTED)),
            Span::styled(
                "j/k",
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" move ", Style::default().fg(MUTED)),
            Span::styled(
                "enter",
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" switch ", Style::default().fg(MUTED)),
            Span::styled(
                "q",
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" quit", Style::default().fg(MUTED)),
        ]),
    ])
    .block(
        Block::default()
            .borders(Borders::BOTTOM)
            .border_style(Style::default().fg(MUTED)),
    )
    .wrap(Wrap { trim: true });

    frame.render_widget(title, area);
}

fn draw_table(frame: &mut Frame<'_>, area: Rect, state: &PaneSwitcherState) {
    let rows = state
        .rows
        .iter()
        .enumerate()
        .map(|(index, entry)| entry.row_cells(index == state.selected))
        .collect::<Vec<_>>();
    let table = Table::new(
        rows,
        [
            Constraint::Length(1),
            Constraint::Length(18),
            Constraint::Length(18),
            Constraint::Length(24),
            Constraint::Min(24),
        ],
    )
    .header(
        Row::new(vec![
            Cell::from(" "),
            Cell::from("Session"),
            Cell::from("Window"),
            Cell::from("Command"),
            Cell::from("Path"),
        ])
        .style(Style::default().fg(MUTED).add_modifier(Modifier::BOLD)),
    )
    .block(
        Block::default()
            .title("Panes")
            .title_style(Style::default().fg(ACCENT).add_modifier(Modifier::BOLD))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(MUTED)),
    )
    .column_spacing(1);

    frame.render_widget(table, area);
}

fn draw_detail(frame: &mut Frame<'_>, area: Rect, state: &PaneSwitcherState) {
    let Some(snapshot) = state.rows.get(state.selected).map(|entry| &entry.snapshot) else {
        let empty = Paragraph::new("No panes")
            .block(
                Block::default()
                    .title("Selected")
                    .title_style(Style::default().fg(ACCENT).add_modifier(Modifier::BOLD))
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(MUTED)),
            )
            .wrap(Wrap { trim: true });
        frame.render_widget(empty, area);
        return;
    };

    let loaded = state
        .rows
        .get(state.selected)
        .is_some_and(|entry| entry.metadata_loaded);
    let git_label = snapshot
        .git
        .as_ref()
        .map(|git| git.label.as_str())
        .unwrap_or(if loaded { "n/a" } else { "loading" });
    let repo = snapshot.repo.as_deref().unwrap_or("loading");
    let branch = snapshot.branch.as_deref().unwrap_or("loading");
    let task_title = snapshot
        .task
        .as_ref()
        .map(|task| truncate(&task.title, 40))
        .unwrap_or_else(|| truncate(&snapshot.pane_current_command, 40));

    let detail = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("selected ", Style::default().fg(MUTED)),
            Span::styled(
                task_title,
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("row ", Style::default().fg(MUTED)),
            Span::styled(pane_row(snapshot), Style::default().fg(TEXT)),
        ]),
        Line::from(vec![
            Span::styled("session ", Style::default().fg(MUTED)),
            Span::styled(
                truncate(&snapshot.session_name, 28),
                Style::default().fg(GOOD),
            ),
        ]),
        Line::from(vec![
            Span::styled("window ", Style::default().fg(MUTED)),
            Span::styled(
                truncate(&snapshot.window_name, 28),
                Style::default().fg(WARN),
            ),
            Span::raw("  "),
            Span::styled("pane ", Style::default().fg(MUTED)),
            Span::styled(
                truncate(&snapshot.pane_title, 28),
                Style::default().fg(TEXT),
            ),
        ]),
        Line::from(vec![
            Span::styled("repo ", Style::default().fg(MUTED)),
            Span::styled(truncate(repo, 28), Style::default().fg(GOOD)),
        ]),
        Line::from(vec![
            Span::styled("branch ", Style::default().fg(MUTED)),
            Span::styled(truncate(branch, 28), Style::default().fg(ACCENT)),
            Span::raw("  "),
            Span::styled("git ", Style::default().fg(MUTED)),
            Span::styled(
                truncate(git_label, 30),
                Style::default().fg(if loaded { GOOD } else { WARN }),
            ),
        ]),
        Line::from(vec![
            Span::styled("path ", Style::default().fg(MUTED)),
            Span::styled(
                truncate(&snapshot.pane_current_path, 50),
                Style::default().fg(TEXT),
            ),
        ]),
        Line::from(vec![
            Span::styled("command ", Style::default().fg(MUTED)),
            Span::styled(
                truncate(&snapshot.pane_current_command, 50),
                Style::default().fg(TEXT),
            ),
        ]),
        Line::from(vec![
            Span::styled("state ", Style::default().fg(MUTED)),
            Span::styled(
                if loaded { "ready" } else { "loading" },
                Style::default()
                    .fg(if loaded { GOOD } else { WARN })
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled("task ", Style::default().fg(MUTED)),
            Span::styled(
                snapshot
                    .task
                    .as_ref()
                    .map(|task| task_state_label(&task.state))
                    .unwrap_or("unmanaged"),
                Style::default().fg(if snapshot.task.is_some() {
                    ACCENT
                } else {
                    MUTED
                }),
            ),
        ]),
    ])
    .block(
        Block::default()
            .title("Selected")
            .title_style(Style::default().fg(ACCENT).add_modifier(Modifier::BOLD))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(MUTED)),
    )
    .wrap(Wrap { trim: true });

    frame.render_widget(detail, area);
}

fn draw_footer(frame: &mut Frame<'_>, area: Rect) {
    let footer = Paragraph::new(vec![Line::from(vec![
        Span::styled(
            "enter",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" activates the selected pane", Style::default().fg(MUTED)),
        Span::raw("  "),
        Span::styled(
            "Esc",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" or ", Style::default().fg(MUTED)),
        Span::styled(
            "q",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" quits", Style::default().fg(MUTED)),
    ])])
    .block(
        Block::default()
            .borders(Borders::TOP)
            .border_style(Style::default().fg(MUTED)),
    )
    .wrap(Wrap { trim: true });

    frame.render_widget(footer, area);
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
