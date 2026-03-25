use crate::overview_tui_helpers::truncate;
use crate::panes::PaneSnapshot;
use crate::panes_support::task_state_label;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

const TEXT: Color = Color::Rgb(236, 239, 244);
const GOOD: Color = Color::Rgb(96, 255, 160);
const ACCENT: Color = Color::Rgb(88, 214, 255);
const WARN: Color = Color::Rgb(255, 204, 102);
const RED: Color = Color::Rgb(255, 107, 107);
const MUTED: Color = Color::Rgb(134, 144, 160);

pub(crate) fn header_summary_line(snapshot: Option<&PaneSnapshot>, status: &str) -> Line<'static> {
    let status_text = status.to_string();
    let status_ready = status == "ready";
    let (path, task_state, task_active) = if let Some(snapshot) = snapshot {
        let path = truncate(&snapshot.pane_current_path, 40);
        let task_state = snapshot
            .task
            .as_ref()
            .map(|task| task_state_label(&task.state))
            .unwrap_or("unmanaged");
        (path, task_state, snapshot.task.is_some())
    } else {
        ("n/a".to_string(), "unmanaged", false)
    };

    let spans = vec![
        Span::styled("state ", Style::default().fg(MUTED)),
        Span::styled(
            status_text,
            Style::default().fg(if status_ready { GOOD } else { WARN }),
        ),
        Span::raw("  "),
        Span::styled("path ", Style::default().fg(MUTED)),
        Span::styled(path, Style::default().fg(GOOD)),
        Span::raw("   "),
        Span::styled("task ", Style::default().fg(MUTED)),
        Span::styled(
            task_state,
            Style::default().fg(if task_active { ACCENT } else { MUTED }),
        ),
    ];

    Line::from(spans)
}

pub(crate) fn footer_line() -> Line<'static> {
    Line::from(vec![
        Span::styled(
            "j/k",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" moves  ", Style::default().fg(MUTED)),
        Span::styled(
            "enter",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" activates the selected pane  ", Style::default().fg(MUTED)),
        Span::styled(
            "s",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" toggles session filter  ", Style::default().fg(MUTED)),
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
    ])
}

pub(crate) fn row_repo_line(snapshot: &PaneSnapshot) -> Line<'static> {
    let repo = snapshot.repo.as_deref().unwrap_or("n/a");
    let branch = snapshot.branch.as_deref().unwrap_or("");
    let repo = if branch.is_empty() {
        repo.to_string()
    } else {
        format!("{repo}@{branch}")
    };

    Line::from(vec![Span::raw(truncate(&repo, 18))])
}

pub(crate) fn row_git_line(snapshot: &PaneSnapshot, loaded: bool) -> Line<'static> {
    let label = snapshot
        .git
        .as_ref()
        .map(|git| git.label.as_str())
        .unwrap_or(if loaded { "n/a" } else { "loading" });
    Line::from(git_summary_spans(label, loaded))
}

pub(crate) fn git_summary_spans(label: &str, loaded: bool) -> Vec<Span<'static>> {
    let label = label.trim();
    if label.is_empty() {
        return vec![Span::styled("n/a", Style::default().fg(MUTED))];
    }

    if label == "loading" {
        return vec![Span::styled("loading", Style::default().fg(WARN))];
    }

    if label == "n/a" {
        return vec![Span::styled("n/a", Style::default().fg(MUTED))];
    }

    if label == "clean" {
        return vec![Span::styled(
            "clean",
            Style::default().fg(if loaded { GOOD } else { MUTED }),
        )];
    }

    let mut spans = Vec::new();
    for token in label.split_whitespace() {
        if !spans.is_empty() {
            spans.push(Span::raw(" "));
        }

        let style = if token.starts_with("chg") {
            Style::default().fg(WARN).add_modifier(Modifier::BOLD)
        } else if token.starts_with("del") {
            Style::default().fg(RED).add_modifier(Modifier::BOLD)
        } else if token.starts_with("new") {
            Style::default().fg(GOOD).add_modifier(Modifier::BOLD)
        } else if let Some(rest) = token.strip_prefix('+') {
            if let Some((insertions, deletions)) = rest.split_once("/-") {
                spans.push(Span::styled(
                    "+",
                    Style::default().fg(GOOD).add_modifier(Modifier::BOLD),
                ));
                spans.push(Span::styled(
                    insertions.to_string(),
                    Style::default().fg(GOOD).add_modifier(Modifier::BOLD),
                ));
                spans.push(Span::raw("/"));
                spans.push(Span::styled(
                    "-",
                    Style::default().fg(RED).add_modifier(Modifier::BOLD),
                ));
                spans.push(Span::styled(
                    deletions.to_string(),
                    Style::default().fg(RED).add_modifier(Modifier::BOLD),
                ));
                continue;
            } else {
                Style::default().fg(GOOD).add_modifier(Modifier::BOLD)
            }
        } else if let Some(rest) = token.strip_prefix('-') {
            if rest.chars().all(|ch| ch.is_ascii_digit()) {
                spans.push(Span::styled(
                    "-",
                    Style::default().fg(RED).add_modifier(Modifier::BOLD),
                ));
                spans.push(Span::styled(
                    rest.to_string(),
                    Style::default().fg(RED).add_modifier(Modifier::BOLD),
                ));
                continue;
            } else {
                Style::default().fg(RED).add_modifier(Modifier::BOLD)
            }
        } else if token.starts_with('+') || token.starts_with('-') {
            Style::default().fg(MUTED)
        } else {
            Style::default().fg(TEXT)
        };

        spans.push(Span::styled(token.to_string(), style));
    }

    spans
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AppConfig, BackendKind, FileConfig, TaskRuntime};
    use crate::model::{TaskMode, TaskState};
    use crate::panes::{PaneGitSummary, PaneSnapshot};
    use std::path::PathBuf;

    fn task(id: &str, title: &str, branch: Option<&str>) -> crate::model::TaskRecord {
        let config = AppConfig {
            home: PathBuf::from("/tmp/swarmux-test-home"),
            config_home: PathBuf::from("/tmp/swarmux-test-config"),
            backend: BackendKind::Files,
            settings: FileConfig::default(),
        };
        let payload = crate::model::SubmitPayload {
            title: title.to_string(),
            repo_ref: "core".to_string(),
            repo_root: "/tmp/core".to_string(),
            mode: TaskMode::Manual,
            runtime: TaskRuntime::Tui,
            worktree: None,
            session: Some(format!("session-{id}")),
            command: vec!["echo".to_string(), title.to_string()],
            priority: None,
            external_ref: None,
            origin: None,
        };
        let mut task =
            crate::model::TaskRecord::from_submit_with_id(payload, &config, id.to_string());
        task.state = TaskState::Running;
        task.branch = branch.map(str::to_string);
        task
    }

    #[test]
    fn git_summary_spans_colorize_the_summary_tokens() {
        let spans = git_summary_spans("chg2 del1 new4 +12/-3", true);

        assert_eq!(
            spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect::<Vec<_>>(),
            vec![
                "chg2", " ", "del1", " ", "new4", " ", "+", "12", "/", "-", "3"
            ]
        );
        assert_eq!(spans[0].style.fg, Some(WARN));
        assert_eq!(spans[2].style.fg, Some(RED));
        assert_eq!(spans[4].style.fg, Some(GOOD));
        assert_eq!(spans[6].style.fg, Some(GOOD));
        assert_eq!(spans[7].style.fg, Some(GOOD));
        assert_eq!(spans[9].style.fg, Some(RED));
        assert_eq!(spans[10].style.fg, Some(RED));
    }

    #[test]
    fn header_summary_line_contains_expected_fields() {
        let snapshot = PaneSnapshot {
            current: true,
            managed_by_swarmux: true,
            session_name: "session-a".to_string(),
            window_id: "@1".to_string(),
            window_index: 1,
            window_name: "shell".to_string(),
            pane_id: "%1".to_string(),
            pane_index: 1,
            pane_active: true,
            pane_current_path: "/tmp/core".to_string(),
            pane_current_command: "bash".to_string(),
            pane_title: "shell".to_string(),
            task: Some(task("a", "Implement selection", Some("main"))),
            repo_root: Some("/tmp/core".to_string()),
            repo: Some("core".to_string()),
            branch: Some("master".to_string()),
            git: Some(PaneGitSummary {
                dirty: true,
                changed_files: 2,
                deleted_files: 1,
                untracked_files: 4,
                insertions: 12,
                deletions: 3,
                label: "chg2 del1 new4 +12/-3".to_string(),
            }),
            label: "initial".to_string(),
        };

        let text = header_summary_line(Some(&snapshot), "ready")
            .spans
            .into_iter()
            .map(|span| span.content.into_owned())
            .collect::<Vec<_>>()
            .join("");

        assert!(text.contains("state ready"));
        assert!(text.contains("path /tmp/core"));
        assert!(text.contains("task running"));
    }

    #[test]
    fn header_summary_line_handles_empty_state() {
        let text = header_summary_line(None, "empty")
            .spans
            .into_iter()
            .map(|span| span.content.into_owned())
            .collect::<Vec<_>>()
            .join("");

        assert!(text.contains("state empty"));
        assert!(text.contains("path n/a"));
        assert!(text.contains("task unmanaged"));
    }

    #[test]
    fn row_repo_line_renders_repo_and_branch() {
        let snapshot = PaneSnapshot {
            current: true,
            managed_by_swarmux: false,
            session_name: "session-a".to_string(),
            window_id: "@1".to_string(),
            window_index: 1,
            window_name: "shell".to_string(),
            pane_id: "%1".to_string(),
            pane_index: 1,
            pane_active: true,
            pane_current_path: "/tmp/core".to_string(),
            pane_current_command: "bash".to_string(),
            pane_title: "shell".to_string(),
            task: None,
            repo_root: None,
            repo: Some(".files".to_string()),
            branch: Some("master".to_string()),
            git: None,
            label: "initial".to_string(),
        };

        let text = row_repo_line(&snapshot)
            .spans
            .into_iter()
            .map(|span| span.content.into_owned())
            .collect::<Vec<_>>()
            .join("");

        assert_eq!(text, ".files@master");
    }

    #[test]
    fn row_git_line_renders_git_summary_tokens() {
        let snapshot = PaneSnapshot {
            current: true,
            managed_by_swarmux: false,
            session_name: "session-a".to_string(),
            window_id: "@1".to_string(),
            window_index: 1,
            window_name: "shell".to_string(),
            pane_id: "%1".to_string(),
            pane_index: 1,
            pane_active: true,
            pane_current_path: "/tmp/core".to_string(),
            pane_current_command: "bash".to_string(),
            pane_title: "shell".to_string(),
            task: None,
            repo_root: None,
            repo: Some(".files".to_string()),
            branch: Some("master".to_string()),
            git: Some(PaneGitSummary {
                dirty: true,
                changed_files: 1,
                deleted_files: 1,
                untracked_files: 0,
                insertions: 2,
                deletions: 1,
                label: "chg1 +2/-1".to_string(),
            }),
            label: "initial".to_string(),
        };

        let text = row_git_line(&snapshot, true)
            .spans
            .into_iter()
            .map(|span| span.content.into_owned())
            .collect::<Vec<_>>()
            .join("");

        assert!(text.contains("chg1"));
        assert!(text.contains("+2/-1"));
    }

    #[test]
    fn footer_line_contains_merged_help() {
        let text = footer_line()
            .spans
            .into_iter()
            .map(|span| span.content.into_owned())
            .collect::<Vec<_>>()
            .join("");

        assert!(text.contains("j/k"));
        assert!(text.contains("moves"));
        assert!(text.contains("enter activates the selected pane"));
        assert!(text.contains("s toggles session filter"));
        assert!(text.contains("Esc or q quits"));
    }
}
