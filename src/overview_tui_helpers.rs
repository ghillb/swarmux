use crate::model::{TaskMode, TaskRecord, TaskState};
use crate::overview_tui::TasksFilter;
use crate::overview_tui_data::DashboardData;
use crate::panes_support::{runtime_label, task_state_label};
use chrono::{DateTime, Utc};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

const TEXT: Color = Color::Rgb(232, 235, 241);
const MUTED: Color = Color::Rgb(134, 144, 160);
const ACCENT: Color = Color::Rgb(88, 214, 255);
const GOOD: Color = Color::Rgb(96, 255, 160);
const WARN: Color = Color::Rgb(255, 204, 102);
const BAD: Color = Color::Rgb(255, 108, 108);
const VIOLET: Color = Color::Rgb(191, 150, 255);
const TEAL: Color = Color::Rgb(95, 241, 223);
const METRIC_LABEL_WIDTH: usize = 13;

pub(crate) fn status_spans(
    filter: Option<TasksFilter>,
    selected_index: usize,
    selected_total: usize,
    selected: Option<&TaskRecord>,
    data: &DashboardData,
) -> Vec<Span<'static>> {
    let focus = selected
        .map(|task| truncate(&task.title, 30))
        .unwrap_or_else(|| "none".to_string());
    let mut spans = Vec::new();

    if let Some(filter) = filter {
        let ordinal = if selected_total == 0 {
            0
        } else {
            selected_index.min(selected_total - 1).saturating_add(1)
        };
        spans.extend([
            Span::styled("filter ", Style::default().fg(MUTED)),
            Span::styled(filter.label(), Style::default().fg(VIOLET)),
            Span::raw("  "),
            Span::styled("selected ", Style::default().fg(MUTED)),
            Span::styled(
                format!("{ordinal}/{selected_total} {focus}"),
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
            ),
        ]);
    }

    if !spans.is_empty() {
        spans.push(Span::raw("  "));
    }

    spans.extend([
        Span::styled("total ", Style::default().fg(MUTED)),
        Span::styled(
            data.all_summary.total.to_string(),
            Style::default().fg(WARN),
        ),
        Span::raw("  "),
        Span::styled("active ", Style::default().fg(MUTED)),
        Span::styled(
            data.all_summary.active().to_string(),
            Style::default().fg(GOOD),
        ),
        Span::raw("  "),
        Span::styled("updated ", Style::default().fg(MUTED)),
        Span::styled(relative_time(data.generated_at), Style::default().fg(TEAL)),
    ]);

    spans
}

pub(crate) fn selected_task<'a>(
    tasks: &[&'a TaskRecord],
    selected: usize,
) -> Option<&'a TaskRecord> {
    tasks.get(selected).copied()
}

pub(crate) fn task_detail_lines(task: &TaskRecord) -> Vec<Line<'static>> {
    let mut lines = vec![
        detail_line("id", truncate(&task.id, 26), ACCENT),
        detail_line(
            "state",
            task_state_label(&task.state),
            state_color(&task.state),
        ),
        detail_line("runtime", runtime_label(task), runtime_color(task)),
        detail_line("mode", mode_label(&task.mode), VIOLET),
        detail_line("repo", truncate(&task.repo, 28), GOOD),
        detail_line("root", truncate(&task.repo_root, 40), TEXT),
        detail_line("reason", truncate(&task.reason, 36), WARN),
        detail_line("created", format_timestamp(task.created_at), MUTED),
        detail_line("updated", format_timestamp(task.updated_at), MUTED),
        detail_line("command", truncate(&task.command.join(" "), 48), TEXT),
    ];

    if let Some(session) = &task.session {
        lines.push(detail_line("session", truncate(session, 32), TEAL));
    }
    if let Some(branch) = &task.branch {
        lines.push(detail_line("branch", truncate(branch, 32), ACCENT));
    }
    if let Some(worktree) = &task.worktree {
        lines.push(detail_line("worktree", truncate(worktree, 40), TEXT));
    }
    if let Some(origin) = &task.origin {
        lines.push(detail_line(
            "origin",
            truncate(
                &format!("{} / {}", origin.session_name, origin.pane_current_path),
                40,
            ),
            MUTED,
        ));
    }
    if let Some(finished) = task.finished_at {
        lines.push(detail_line("finished", format_timestamp(finished), MUTED));
    }
    if let Some(error) = &task.last_error {
        lines.push(detail_line("error", truncate(error, 48), BAD));
    }

    lines
}

pub(crate) fn metric_line(label: &str, value: usize, color: Color) -> Line<'static> {
    let label = truncate(label, METRIC_LABEL_WIDTH);
    Line::from(vec![
        Span::styled(
            format!("{label:<width$}", width = METRIC_LABEL_WIDTH),
            Style::default().fg(MUTED),
        ),
        Span::styled(
            format!("{value:>5}"),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ),
    ])
}

pub(crate) fn render_counts(counts: &[(String, usize)], color: Color) -> Vec<Line<'static>> {
    if counts.is_empty() {
        return vec![Line::from(Span::styled("none", Style::default().fg(MUTED)))];
    }

    counts
        .iter()
        .map(|(label, count)| metric_line(label, *count, color))
        .collect()
}

pub(crate) fn relative_time(moment: DateTime<Utc>) -> String {
    let seconds = (Utc::now() - moment).num_seconds().max(0);
    if seconds < 60 {
        return format!("{seconds}s");
    }

    let minutes = seconds / 60;
    if minutes < 60 {
        return format!("{minutes}m");
    }

    let hours = minutes / 60;
    if hours < 24 {
        return format!("{hours}h");
    }

    format!("{}d", hours / 24)
}

pub(crate) fn truncate(value: &str, max: usize) -> String {
    let count = value.chars().count();
    if count <= max {
        return value.to_string();
    }

    let mut out = value
        .chars()
        .take(max.saturating_sub(3))
        .collect::<String>();
    out.push_str("...");
    out
}

pub(crate) fn window_start(selected: usize, visible_rows: usize, len: usize) -> usize {
    if len <= visible_rows || visible_rows == 0 {
        return 0;
    }

    let half = visible_rows / 2;
    selected.saturating_sub(half).min(len - visible_rows)
}

fn detail_line(label: &str, value: impl Into<String>, color: Color) -> Line<'static> {
    let value = value.into();
    Line::from(vec![
        Span::styled(format!("{label:<10}"), Style::default().fg(MUTED)),
        Span::styled(
            value,
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ),
    ])
}

fn state_color(state: &TaskState) -> Color {
    match state {
        TaskState::Queued => MUTED,
        TaskState::Dispatching => WARN,
        TaskState::Running => GOOD,
        TaskState::WaitingInput => WARN,
        TaskState::Succeeded => GOOD,
        TaskState::Failed => BAD,
        TaskState::Canceled => MUTED,
    }
}

fn runtime_color(task: &TaskRecord) -> Color {
    match runtime_label(task) {
        "headless" => ACCENT,
        "mirrored" => TEAL,
        "tui" => GOOD,
        _ => MUTED,
    }
}

fn mode_label(mode: &TaskMode) -> &'static str {
    match mode {
        TaskMode::Auto => "auto",
        TaskMode::Manual => "manual",
    }
}

fn format_timestamp(moment: DateTime<Utc>) -> String {
    moment.format("%Y-%m-%d %H:%M:%S UTC").to_string()
}
