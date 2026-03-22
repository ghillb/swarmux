use crate::model::TaskRecord;
use crate::overview_tui::{AppState, Tab};
use crate::overview_tui_data::DashboardData;
use crate::overview_tui_helpers::{
    metric_line, relative_time, render_counts, selected_task, status_spans, task_detail_lines,
    truncate, window_start,
};
use crate::panes_support::{runtime_label, task_state_label};
use ratatui::prelude::*;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table, Tabs, Wrap};

const TEXT: Color = Color::Rgb(232, 235, 241);
const MUTED: Color = Color::Rgb(134, 144, 160);
const ACCENT: Color = Color::Rgb(88, 214, 255);
const GOOD: Color = Color::Rgb(96, 255, 160);
const WARN: Color = Color::Rgb(255, 204, 102);
const BAD: Color = Color::Rgb(255, 108, 108);
const VIOLET: Color = Color::Rgb(191, 150, 255);
const TEAL: Color = Color::Rgb(95, 241, 223);
const WIDE_BREAKPOINT: u16 = 124;

pub(crate) fn draw(frame: &mut Frame<'_>, app: &AppState, data: &DashboardData) {
    let outer = Layout::vertical([
        Constraint::Length(5),
        Constraint::Length(3),
        Constraint::Min(0),
        Constraint::Length(2),
    ])
    .split(frame.area());

    draw_header(frame, outer[0], app, data);
    draw_tabs(frame, outer[1], app);

    match app.tab {
        Tab::Overview => draw_overview(frame, outer[2], app, data),
        Tab::Operational => draw_operational(frame, outer[2], app, data),
        Tab::ClientAll => draw_client_all(frame, outer[2], app, data),
    }

    draw_footer(frame, outer[3], data);
}

fn draw_header(frame: &mut Frame<'_>, area: Rect, app: &AppState, data: &DashboardData) {
    let title = Paragraph::new(vec![
        Line::from(vec![
            Span::styled(
                "SWARMUX",
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled(
                "OVERVIEW",
                Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(status_spans(app, data)),
    ])
    .block(
        Block::default()
            .borders(Borders::BOTTOM)
            .border_style(Style::default().fg(MUTED)),
    )
    .wrap(Wrap { trim: true });

    frame.render_widget(title, area);
}

fn draw_tabs(frame: &mut Frame<'_>, area: Rect, app: &AppState) {
    let tabs = Tabs::new(vec![
        Line::from("Overview"),
        Line::from("Operational"),
        Line::from("Client All"),
    ])
    .select(match app.tab {
        Tab::Overview => 0,
        Tab::Operational => 1,
        Tab::ClientAll => 2,
    })
    .divider(Span::styled("│", Style::default().fg(MUTED)))
    .highlight_style(
        Style::default()
            .fg(ACCENT)
            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
    )
    .style(Style::default().fg(MUTED))
    .block(
        Block::default()
            .borders(Borders::BOTTOM)
            .border_style(Style::default().fg(MUTED)),
    );

    frame.render_widget(tabs, area);
}

fn draw_overview(frame: &mut Frame<'_>, area: Rect, app: &AppState, data: &DashboardData) {
    let body = Layout::vertical([Constraint::Length(7), Constraint::Min(0)]).split(area);
    let cards = Layout::horizontal([
        Constraint::Percentage(33),
        Constraint::Percentage(34),
        Constraint::Percentage(33),
    ])
    .split(body[0]);

    draw_card(
        frame,
        cards[0],
        "Live",
        vec![
            metric_line("visible", data.visible_summary.total, GOOD),
            metric_line("active", data.visible_summary.active(), ACCENT),
            metric_line("terminal", data.visible_summary.terminal(), WARN),
        ],
    );
    draw_card(
        frame,
        cards[1],
        "Mix",
        vec![
            metric_line("queued", data.visible_summary.queued, MUTED),
            metric_line("running", data.visible_summary.running, GOOD),
            metric_line("waiting", data.visible_summary.waiting_input, WARN),
            metric_line("manual", data.visible_summary.manual, VIOLET),
            metric_line("auto", data.visible_summary.auto, TEAL),
        ],
    );
    draw_card(
        frame,
        cards[2],
        "Scope",
        vec![
            metric_line("filtered", data.visible_summary.total, GOOD),
            metric_line("all", data.all_summary.total, ACCENT),
            metric_line("sessions", data.visible_summary.with_session, WARN),
        ],
    );

    let selected = selected_task(Tab::Overview, app, data);
    let content = if area.width >= WIDE_BREAKPOINT {
        Layout::horizontal([Constraint::Percentage(64), Constraint::Percentage(36)]).split(body[1])
    } else {
        Layout::vertical([Constraint::Percentage(64), Constraint::Percentage(36)]).split(body[1])
    };

    draw_task_table(
        frame,
        content[0],
        "Recent Tasks",
        &data.visible_tasks,
        app.overview_selected,
        false,
    );
    draw_detail_panel(frame, content[1], "Selected Task", selected);
}

fn draw_operational(frame: &mut Frame<'_>, area: Rect, app: &AppState, data: &DashboardData) {
    let body = Layout::vertical([Constraint::Length(7), Constraint::Min(0)]).split(area);
    let top = Layout::horizontal([
        Constraint::Percentage(33),
        Constraint::Percentage(33),
        Constraint::Percentage(34),
    ])
    .split(body[0]);

    draw_card(
        frame,
        top[0],
        "Flow",
        vec![
            metric_line("queued", data.all_summary.queued, MUTED),
            metric_line("dispatching", data.all_summary.dispatching, WARN),
            metric_line("running", data.all_summary.running, GOOD),
            metric_line("waiting", data.all_summary.waiting_input, BAD),
        ],
    );
    draw_card(
        frame,
        top[1],
        "Health",
        vec![
            metric_line("succeeded", data.all_summary.succeeded, GOOD),
            metric_line("failed", data.all_summary.failed, BAD),
            metric_line("canceled", data.all_summary.canceled, MUTED),
            metric_line("terminal", data.all_summary.terminal(), ACCENT),
        ],
    );
    draw_card(
        frame,
        top[2],
        "Mix",
        vec![
            metric_line("auto", data.all_summary.auto, TEAL),
            metric_line("manual", data.all_summary.manual, VIOLET),
            metric_line("headless", data.all_summary.headless, ACCENT),
            metric_line("mirrored", data.all_summary.mirrored, GOOD),
            metric_line("tui", data.all_summary.tui, WARN),
        ],
    );

    let selected = selected_task(Tab::Operational, app, data);
    let bottom = if area.width >= WIDE_BREAKPOINT {
        Layout::horizontal([Constraint::Percentage(58), Constraint::Percentage(42)]).split(body[1])
    } else {
        Layout::vertical([Constraint::Percentage(58), Constraint::Percentage(42)]).split(body[1])
    };

    let stats = if area.width >= WIDE_BREAKPOINT {
        Layout::vertical([Constraint::Percentage(50), Constraint::Percentage(50)]).split(bottom[0])
    } else {
        Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(bottom[0])
    };

    draw_card(
        frame,
        stats[0],
        "Repositories",
        render_counts(&data.repo_counts, GOOD),
    );
    draw_card(
        frame,
        stats[1],
        "Runtime",
        render_counts(&data.runtime_counts, ACCENT),
    );
    draw_detail_panel(frame, bottom[1], "Latest Task", selected);
}

fn draw_client_all(frame: &mut Frame<'_>, area: Rect, app: &AppState, data: &DashboardData) {
    let selected = selected_task(Tab::ClientAll, app, data);
    let body = if area.width >= WIDE_BREAKPOINT {
        Layout::horizontal([Constraint::Percentage(68), Constraint::Percentage(32)]).split(area)
    } else {
        Layout::vertical([Constraint::Percentage(66), Constraint::Percentage(34)]).split(area)
    };

    draw_task_table(
        frame,
        body[0],
        "Client All",
        &data.all_tasks,
        app.client_selected,
        true,
    );
    draw_detail_panel(frame, body[1], "Selected Task", selected);
}

fn draw_footer(frame: &mut Frame<'_>, area: Rect, data: &DashboardData) {
    let footer = Paragraph::new(vec![Line::from(vec![
        Span::styled(
            "j/k",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" move ", Style::default().fg(MUTED)),
        Span::styled(
            "h/l",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" tabs ", Style::default().fg(MUTED)),
        Span::styled(
            "tab",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" switch ", Style::default().fg(MUTED)),
        Span::styled("r", Style::default().fg(GOOD).add_modifier(Modifier::BOLD)),
        Span::styled(" refresh ", Style::default().fg(MUTED)),
        Span::styled("q", Style::default().fg(BAD).add_modifier(Modifier::BOLD)),
        Span::styled(" quit ", Style::default().fg(MUTED)),
        Span::styled(
            format!(
                "{} visible / {} total",
                data.visible_summary.total, data.all_summary.total
            ),
            Style::default().fg(WARN).add_modifier(Modifier::BOLD),
        ),
    ])])
    .block(
        Block::default()
            .borders(Borders::TOP)
            .border_style(Style::default().fg(MUTED)),
    );

    frame.render_widget(footer, area);
}

fn draw_card(frame: &mut Frame<'_>, area: Rect, title: &str, lines: Vec<Line<'static>>) {
    let card = Paragraph::new(lines)
        .block(
            Block::default()
                .title(title)
                .title_alignment(Alignment::Left)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(ACCENT)),
        )
        .wrap(Wrap { trim: true });
    frame.render_widget(card, area);
}

fn draw_detail_panel(frame: &mut Frame<'_>, area: Rect, title: &str, task: Option<&TaskRecord>) {
    let lines = task.map(task_detail_lines).unwrap_or_else(|| {
        vec![Line::from(Span::styled(
            "no task selected",
            Style::default().fg(MUTED),
        ))]
    });
    let panel = Paragraph::new(lines)
        .block(
            Block::default()
                .title(title)
                .title_alignment(Alignment::Left)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(ACCENT)),
        )
        .wrap(Wrap { trim: true });
    frame.render_widget(panel, area);
}

fn draw_task_table(
    frame: &mut Frame<'_>,
    area: Rect,
    title: &str,
    tasks: &[TaskRecord],
    selected: usize,
    show_session: bool,
) {
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(ACCENT));

    if tasks.is_empty() {
        frame.render_widget(
            Paragraph::new("no tasks")
                .style(Style::default().fg(MUTED))
                .block(block),
            area,
        );
        return;
    }

    let visible_rows = block.inner(area).height.saturating_sub(2).max(1) as usize;
    let start = window_start(selected, visible_rows, tasks.len());
    let end = (start + visible_rows).min(tasks.len());
    let rows = tasks[start..end]
        .iter()
        .enumerate()
        .map(|(offset, task)| task_row(task, start + offset == selected, show_session))
        .collect::<Vec<_>>();

    if show_session {
        let table = Table::new(
            rows,
            [
                Constraint::Length(2),
                Constraint::Length(8),
                Constraint::Length(12),
                Constraint::Length(12),
                Constraint::Length(10),
                Constraint::Length(14),
                Constraint::Min(16),
            ],
        )
        .column_spacing(1)
        .style(Style::default().fg(TEXT))
        .header(
            Row::new(vec![
                Cell::from(" "),
                Cell::from("updated"),
                Cell::from("state"),
                Cell::from("runtime"),
                Cell::from("repo"),
                Cell::from("session"),
                Cell::from("title"),
            ])
            .style(Style::default().fg(MUTED).add_modifier(Modifier::BOLD)),
        )
        .block(block);
        frame.render_widget(table, area);
    } else {
        let table = Table::new(
            rows,
            [
                Constraint::Length(2),
                Constraint::Length(8),
                Constraint::Length(12),
                Constraint::Length(12),
                Constraint::Length(10),
                Constraint::Min(18),
            ],
        )
        .column_spacing(1)
        .style(Style::default().fg(TEXT))
        .header(
            Row::new(vec![
                Cell::from(" "),
                Cell::from("updated"),
                Cell::from("state"),
                Cell::from("runtime"),
                Cell::from("repo"),
                Cell::from("title"),
            ])
            .style(Style::default().fg(MUTED).add_modifier(Modifier::BOLD)),
        )
        .block(block);
        frame.render_widget(table, area);
    }
}

fn task_row(task: &TaskRecord, selected: bool, show_session: bool) -> Row<'static> {
    let mut cells = vec![
        Cell::from(if selected { "▶" } else { "·" }),
        Cell::from(relative_time(task.updated_at)),
        Cell::from(task_state_label(&task.state)),
        Cell::from(runtime_label(task)),
        Cell::from(truncate(&task.repo, 10)),
    ];

    if show_session {
        cells.push(Cell::from(truncate(
            task.session.as_deref().unwrap_or("-"),
            12,
        )));
    }

    cells.push(Cell::from(truncate(
        &task.title,
        if show_session { 36 } else { 40 },
    )));

    Row::new(cells).style(if selected {
        Style::default()
            .fg(ACCENT)
            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
    } else {
        Style::default().fg(TEXT)
    })
}
