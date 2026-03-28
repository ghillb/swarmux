use crate::model::TaskRecord;
use crate::overview_tui::{AppState, Tab, TasksFilter};
use crate::overview_tui_data::DashboardData;
use crate::overview_tui_helpers::{
    metric_line, relative_time, render_counts, selected_task, status_spans, task_detail_lines,
    truncate, window_start,
};
use crate::panes_support::{runtime_label, task_state_label};
use ratatui::prelude::*;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table, Wrap};

const TEXT: Color = Color::Rgb(232, 235, 241);
const MUTED: Color = Color::Rgb(134, 144, 160);
const ACCENT: Color = Color::Rgb(88, 214, 255);
const GOOD: Color = Color::Rgb(96, 255, 160);
const WARN: Color = Color::Rgb(255, 204, 102);
const BAD: Color = Color::Rgb(255, 108, 108);
const TEAL: Color = Color::Rgb(95, 241, 223);
const WIDE_BREAKPOINT: u16 = 124;

pub(crate) fn draw(frame: &mut Frame<'_>, app: &AppState, data: &DashboardData) {
    let filtered_tasks = data.filtered_tasks(app.tasks_filter);
    let outer = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(0),
        Constraint::Length(2),
    ])
    .split(frame.area());

    draw_header(frame, outer[0], app, data, &filtered_tasks);

    match app.tab {
        Tab::Tasks => draw_tasks(frame, outer[1], app, &filtered_tasks),
        Tab::Stats => draw_stats(frame, outer[1], data),
    }

    draw_footer(frame, outer[2], data, filtered_tasks.len());
}

fn draw_header(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &AppState,
    data: &DashboardData,
    tasks: &[&TaskRecord],
) {
    let selected = if matches!(app.tab, Tab::Tasks) {
        selected_task(tasks, app.tasks_selected)
    } else {
        None
    };
    frame.render_widget(
        Block::default()
            .borders(Borders::BOTTOM)
            .border_style(Style::default().fg(MUTED)),
        area,
    );

    let content = Rect {
        x: area.x,
        y: area.y,
        width: area.width,
        height: area.height.saturating_sub(1),
    };
    let rows = Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).split(content);
    let top = Layout::horizontal([Constraint::Min(0), Constraint::Length(13)]).split(rows[0]);

    let title = Paragraph::new(Line::from(vec![
        Span::styled(
            "SWARMUX",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(
            "TASKS",
            Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
        ),
    ]));
    frame.render_widget(title, top[0]);

    let tabs = Paragraph::new(Line::from(vec![
        Span::styled(
            "Tasks",
            if matches!(app.tab, Tab::Tasks) {
                Style::default()
                    .fg(ACCENT)
                    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
            } else {
                Style::default().fg(MUTED)
            },
        ),
        Span::styled(" │ ", Style::default().fg(MUTED)),
        Span::styled(
            "Stats",
            if matches!(app.tab, Tab::Stats) {
                Style::default()
                    .fg(ACCENT)
                    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
            } else {
                Style::default().fg(MUTED)
            },
        ),
    ]))
    .alignment(Alignment::Right);
    frame.render_widget(tabs, top[1]);

    let status = Paragraph::new(Line::from(status_spans(
        if matches!(app.tab, Tab::Tasks) {
            Some(app.tasks_filter)
        } else {
            None
        },
        app.tasks_selected,
        tasks.len(),
        selected,
        data,
    )))
    .style(Style::default().fg(MUTED));
    frame.render_widget(status, rows[1]);
}

fn draw_tasks(frame: &mut Frame<'_>, area: Rect, app: &AppState, tasks: &[&TaskRecord]) {
    let selected = selected_task(tasks, app.tasks_selected);
    let content = if area.width >= WIDE_BREAKPOINT {
        Layout::horizontal([Constraint::Percentage(68), Constraint::Percentage(32)]).split(area)
    } else {
        Layout::vertical([Constraint::Percentage(68), Constraint::Percentage(32)]).split(area)
    };

    draw_task_table(
        frame,
        content[0],
        "Tasks",
        tasks,
        app.tasks_selected,
        true,
        empty_tasks_message(app.tasks_filter),
    );
    draw_detail_panel(frame, content[1], "Selected Task", selected);
}

fn draw_stats(frame: &mut Frame<'_>, area: Rect, data: &DashboardData) {
    let cards = if area.width >= WIDE_BREAKPOINT {
        Layout::horizontal([
            Constraint::Percentage(33),
            Constraint::Percentage(33),
            Constraint::Percentage(34),
        ])
        .split(area)
    } else {
        Layout::vertical([
            Constraint::Percentage(33),
            Constraint::Percentage(33),
            Constraint::Percentage(34),
        ])
        .split(area)
    };

    draw_card(
        frame,
        cards[0],
        "State",
        vec![
            metric_line("total", data.all_summary.total, GOOD),
            metric_line("active", data.all_summary.active(), ACCENT),
            metric_line("terminal", data.all_summary.terminal(), WARN),
            metric_line("queued", data.all_summary.queued, MUTED),
            metric_line("dispatching", data.all_summary.dispatching, WARN),
            metric_line("running", data.all_summary.running, GOOD),
            metric_line("waiting", data.all_summary.waiting_input, WARN),
            metric_line("succeeded", data.all_summary.succeeded, GOOD),
            metric_line("failed", data.all_summary.failed, BAD),
            metric_line("canceled", data.all_summary.canceled, MUTED),
        ],
    );
    draw_card(
        frame,
        cards[1],
        "Runtime",
        render_counts(&data.runtime_counts, TEAL),
    );
    draw_card(
        frame,
        cards[2],
        "Repositories",
        render_counts(&data.repo_counts, GOOD),
    );
}

fn draw_footer(frame: &mut Frame<'_>, area: Rect, data: &DashboardData, filtered: usize) {
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
        Span::styled("f", Style::default().fg(GOOD).add_modifier(Modifier::BOLD)),
        Span::styled(" filter ", Style::default().fg(MUTED)),
        Span::styled(
            "enter",
            Style::default().fg(GOOD).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" switch ", Style::default().fg(MUTED)),
        Span::styled("x", Style::default().fg(WARN).add_modifier(Modifier::BOLD)),
        Span::styled(" stop ", Style::default().fg(MUTED)),
        Span::styled("X", Style::default().fg(BAD).add_modifier(Modifier::BOLD)),
        Span::styled(" kill ", Style::default().fg(MUTED)),
        Span::styled("r", Style::default().fg(GOOD).add_modifier(Modifier::BOLD)),
        Span::styled(" refresh ", Style::default().fg(MUTED)),
        Span::styled("q", Style::default().fg(BAD).add_modifier(Modifier::BOLD)),
        Span::styled(" quit ", Style::default().fg(MUTED)),
        Span::styled(
            format!("{} tasks / {} total", filtered, data.all_summary.total),
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
    tasks: &[&TaskRecord],
    selected: usize,
    show_session: bool,
    empty_message: &'static str,
) {
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(ACCENT));

    if tasks.is_empty() {
        frame.render_widget(
            Paragraph::new(vec![
                Line::from(Span::styled(empty_message, Style::default().fg(MUTED))),
                Line::from(Span::styled(
                    "press f to cycle active / terminal / all",
                    Style::default().fg(MUTED),
                )),
            ])
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

fn empty_tasks_message(filter: TasksFilter) -> &'static str {
    match filter {
        TasksFilter::Active => "no active tasks",
        TasksFilter::Terminal => "no terminal tasks",
        TasksFilter::All => "no tasks",
    }
}
