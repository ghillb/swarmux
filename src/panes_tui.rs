use crate::config::PaneSwitcherHighlight;
use crate::model::TaskRecord;
use crate::panes::{PaneGitSummary, PaneSnapshot};
use crate::panes_support::{
    RawPane, build_label, current_tmux_session_name, git_context, git_info, list_tasks,
    list_tmux_panes, pane_sort_key,
};
use crate::panes_tui_detail::{git_summary_spans, row_git_line, row_repo_line};
use crate::store::Store;
use anyhow::Result;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Cell, Row};
use std::sync::mpsc::Sender;
use std::thread;

const TEXT: Color = Color::Rgb(236, 239, 244);
const GOOD: Color = Color::Rgb(96, 255, 160);
const ACCENT: Color = Color::Rgb(88, 214, 255);
const SURFACE: Color = Color::Rgb(29, 32, 40);

pub fn run(store: &Store, source_pane_id: Option<&str>) -> Result<()> {
    crate::panes_tui_render::run(store, source_pane_id)
}

pub fn run_sidebar(store: &Store, source_pane_id: Option<&str>) -> Result<()> {
    crate::panes_tui_render::run_sidebar(store, source_pane_id)
}

#[derive(Clone, Debug)]
pub(crate) struct PaneEntry {
    pub(crate) snapshot: PaneSnapshot,
    pub(crate) metadata_loaded: bool,
}

#[derive(Debug)]
pub(crate) struct PaneSwitcherState {
    pub(crate) all_rows: Vec<PaneEntry>,
    pub(crate) rows: Vec<PaneEntry>,
    pub(crate) selected: usize,
    pub(crate) loaded_count: usize,
    pub(crate) current_session_only: bool,
    pub(crate) current_session_name: Option<String>,
}

#[derive(Debug)]
pub(crate) enum HydrationUpdate {
    PaneGit {
        pane_id: String,
        git: Option<PaneGitSummary>,
    },
}

impl PaneSwitcherState {
    pub(crate) fn load(
        store: &Store,
        current_pane_id: Option<&str>,
        excluded_pane_id: Option<&str>,
        current_session_only: bool,
    ) -> Result<Self> {
        let tasks = list_tasks(store)?;
        let tasks_by_session = tasks
            .iter()
            .filter_map(|task| {
                task.session
                    .as_ref()
                    .map(|session| (session.clone(), task.clone()))
            })
            .collect::<std::collections::BTreeMap<_, _>>();

        let filter = store.paths().settings.tmux.ignore_filter();
        let current_pane_id = current_pane_id
            .map(str::to_string)
            .or_else(|| std::env::var("TMUX_PANE").ok());
        let current_session_name = current_tmux_session_name()?;
        let raw_panes = list_tmux_panes(Some(filter.as_str()))?;
        let mut all_rows = raw_panes
            .into_iter()
            .filter(|raw| excluded_pane_id != Some(raw.pane_id.as_str()))
            .map(|raw| build_entry(&raw, &tasks_by_session, current_pane_id.as_deref()))
            .collect::<Vec<_>>();

        all_rows.sort_by_key(|entry| pane_sort_key(&entry.snapshot));
        let rows = filter_rows(
            &all_rows,
            current_session_only,
            current_session_name.as_deref(),
        );

        Ok(Self {
            all_rows,
            rows,
            selected: 0,
            loaded_count: 0,
            current_session_only,
            current_session_name,
        })
    }

    pub(crate) fn initial_selected(&self, source_pane_id: Option<&str>) -> usize {
        source_pane_id
            .and_then(|pane_id| {
                self.rows
                    .iter()
                    .position(|entry| entry.snapshot.pane_id == pane_id)
            })
            .or_else(|| self.rows.iter().position(|entry| entry.snapshot.current))
            .unwrap_or(0)
    }

    pub(crate) fn clamp_selected(&self, selected: usize) -> usize {
        if self.rows.is_empty() {
            0
        } else {
            selected.min(self.rows.len() - 1)
        }
    }

    pub(crate) fn toggle_current_session_only(&mut self) {
        self.current_session_only = !self.current_session_only;
        let selected_pane_id = self
            .rows
            .get(self.selected)
            .map(|entry| entry.snapshot.pane_id.clone());
        self.rebuild_rows(selected_pane_id.as_deref());
    }

    pub(crate) fn apply_update(&mut self, update: HydrationUpdate) -> bool {
        match update {
            HydrationUpdate::PaneGit { pane_id, git } => {
                let mut updated = false;

                if let Some(entry) = self
                    .all_rows
                    .iter_mut()
                    .find(|entry| entry.snapshot.pane_id == pane_id)
                {
                    updated |= apply_git_update(entry, git.clone());
                }

                if let Some(entry) = self
                    .rows
                    .iter_mut()
                    .find(|entry| entry.snapshot.pane_id == pane_id)
                {
                    updated |= apply_git_update(entry, git);
                }

                if updated {
                    self.refresh_loaded_count();
                }

                updated
            }
        }
    }

    fn rebuild_rows(&mut self, selected_pane_id: Option<&str>) {
        self.rows = filter_rows(
            &self.all_rows,
            self.current_session_only,
            self.current_session_name.as_deref(),
        );
        self.selected = self.initial_selected(selected_pane_id);
        self.refresh_loaded_count();
    }

    fn refresh_loaded_count(&mut self) {
        self.loaded_count = self
            .rows
            .iter()
            .filter(|entry| entry.metadata_loaded)
            .count();
    }
}

impl PaneEntry {
    pub(crate) fn row_style(&self, selected: bool, mode: PaneSwitcherHighlight) -> Style {
        if selected {
            match mode {
                PaneSwitcherHighlight::Solid => Style::default()
                    .bg(ACCENT)
                    .fg(SURFACE)
                    .add_modifier(Modifier::BOLD),
                PaneSwitcherHighlight::Underline => Style::default()
                    .fg(ACCENT)
                    .add_modifier(Modifier::BOLD)
                    .add_modifier(Modifier::UNDERLINED),
            }
        } else if self.snapshot.current {
            Style::default().fg(GOOD).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(TEXT)
        }
    }

    fn marker(&self, selected: bool, show_arrow: bool) -> &'static str {
        if selected && show_arrow {
            "▶"
        } else if self.snapshot.current {
            "●"
        } else {
            " "
        }
    }

    pub(crate) fn row_cells(
        &self,
        selected: bool,
        mode: PaneSwitcherHighlight,
        show_arrow: bool,
    ) -> Row<'static> {
        Row::new(vec![
            Cell::from(self.marker(selected, show_arrow)),
            Cell::from(crate::overview_tui_helpers::truncate(
                &self.snapshot.session_name,
                18,
            )),
            Cell::from(crate::overview_tui_helpers::truncate(
                &self.snapshot.window_name,
                18,
            )),
            Cell::from(crate::overview_tui_helpers::truncate(
                &self.snapshot.pane_title,
                24,
            )),
            Cell::from(row_repo_line(&self.snapshot)),
            Cell::from(row_git_line(&self.snapshot, self.metadata_loaded)),
        ])
        .style(self.row_style(selected, mode))
    }

    pub(crate) fn sidebar_text(
        &self,
        width: usize,
        selected: bool,
        mode: PaneSwitcherHighlight,
        show_arrow: bool,
        show_session: bool,
    ) -> Text<'static> {
        let width = width.max(1);
        let title = crate::overview_tui_helpers::truncate(
            &self.snapshot.pane_title,
            width.saturating_sub(2).max(1),
        );
        let repo = self.snapshot.repo.as_deref().unwrap_or("n/a");
        let branch = self.snapshot.branch.as_deref().unwrap_or("");
        let git = self
            .snapshot
            .git
            .as_ref()
            .map(|git| git.label.as_str())
            .unwrap_or(if self.metadata_loaded {
                "n/a"
            } else {
                "loading"
            });

        let repo_text = if branch.is_empty() {
            repo.to_string()
        } else {
            format!("{repo}@{branch}")
        };
        let mut left_text = repo_text.clone();

        if show_session {
            left_text.push_str("  ");
            left_text.push_str(&self.snapshot.session_name);
        }

        let right_text = git.to_string();
        let left_text = crate::overview_tui_helpers::truncate(
            &left_text,
            width.saturating_sub(2 + right_text.chars().count()).max(1),
        );
        let spacer_width =
            width.saturating_sub(2 + left_text.chars().count() + right_text.chars().count());

        let title_style = if selected {
            match mode {
                PaneSwitcherHighlight::Solid => Style::default()
                    .bg(ACCENT)
                    .fg(SURFACE)
                    .add_modifier(Modifier::BOLD),
                PaneSwitcherHighlight::Underline => {
                    Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
                }
            }
        } else if self.snapshot.current {
            Style::default().fg(GOOD).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(TEXT)
        };

        let mut detail_spans = vec![Span::raw("  "), Span::styled(left_text, Style::default())];
        if spacer_width > 0 {
            detail_spans.push(Span::raw(" ".repeat(spacer_width)));
        }
        detail_spans.extend(git_summary_spans(git, self.metadata_loaded));

        let detail = Line::from(detail_spans);
        let detail = match mode {
            PaneSwitcherHighlight::Solid if selected => detail.style(
                Style::default()
                    .bg(ACCENT)
                    .fg(SURFACE)
                    .add_modifier(Modifier::BOLD),
            ),
            PaneSwitcherHighlight::Underline if selected => {
                detail.style(Style::default().add_modifier(Modifier::UNDERLINED))
            }
            _ => detail,
        };

        Text::from(vec![
            Line::from(vec![Span::styled(
                format!("{} {}", self.marker(selected, show_arrow), title),
                title_style,
            )]),
            detail,
        ])
    }
}

pub(crate) fn spawn_hydrator(rows: Vec<PaneEntry>, tx: Sender<HydrationUpdate>) {
    thread::spawn(move || {
        for entry in rows {
            let git = git_info(&entry.snapshot.pane_current_path)
                .ok()
                .map(|info| info.summary);
            if tx
                .send(HydrationUpdate::PaneGit {
                    pane_id: entry.snapshot.pane_id,
                    git,
                })
                .is_err()
            {
                break;
            }
        }
    });
}

fn filter_rows(
    rows: &[PaneEntry],
    current_session_only: bool,
    current_session_name: Option<&str>,
) -> Vec<PaneEntry> {
    rows.iter()
        .filter(|entry| {
            !current_session_only
                || current_session_name
                    .is_none_or(|session_name| entry.snapshot.session_name == session_name)
        })
        .cloned()
        .collect()
}

fn apply_git_update(entry: &mut PaneEntry, git: Option<PaneGitSummary>) -> bool {
    if entry.metadata_loaded {
        return true;
    }

    entry.snapshot.git = git;
    let raw = snapshot_to_raw(&entry.snapshot);
    entry.snapshot.label = build_label(
        &raw,
        entry.snapshot.task.as_ref(),
        entry.snapshot.repo.as_deref(),
        entry.snapshot.branch.as_deref(),
        entry.snapshot.git.as_ref(),
    );
    entry.metadata_loaded = true;
    true
}

fn build_entry(
    raw: &RawPane,
    tasks_by_session: &std::collections::BTreeMap<String, TaskRecord>,
    current_pane_id: Option<&str>,
) -> PaneEntry {
    let task = tasks_by_session.get(&raw.session_name).cloned();
    let git_context = git_context(&raw.pane_current_path).ok();
    let repo_root = task
        .as_ref()
        .map(|task| task.repo_root.clone())
        .or_else(|| git_context.as_ref().map(|git| git.repo_root.clone()));
    let repo = task
        .as_ref()
        .map(|task| task.repo.clone())
        .or_else(|| git_context.as_ref().map(|git| git.repo.clone()));
    let branch = task
        .as_ref()
        .and_then(|task| task.branch.clone())
        .or_else(|| git_context.as_ref().and_then(|git| git.branch.clone()));

    let snapshot = PaneSnapshot {
        current: current_pane_id.is_some_and(|pane_id| pane_id == raw.pane_id),
        managed_by_swarmux: task.is_some(),
        session_name: raw.session_name.clone(),
        window_id: raw.window_id.clone(),
        window_index: raw.window_index,
        window_name: raw.window_name.clone(),
        pane_id: raw.pane_id.clone(),
        pane_index: raw.pane_index,
        pane_active: raw.pane_active,
        pane_current_path: raw.pane_current_path.clone(),
        pane_current_command: raw.pane_current_command.clone(),
        pane_title: raw.pane_title.clone(),
        task,
        repo_root,
        repo,
        branch,
        git: None,
        label: String::new(),
    };

    let mut entry = PaneEntry {
        snapshot,
        metadata_loaded: false,
    };
    let raw_snapshot = snapshot_to_raw(&entry.snapshot);
    entry.snapshot.label = build_label(
        &raw_snapshot,
        entry.snapshot.task.as_ref(),
        entry.snapshot.repo.as_deref(),
        entry.snapshot.branch.as_deref(),
        None,
    );
    entry
}

fn snapshot_to_raw(snapshot: &PaneSnapshot) -> RawPane {
    RawPane {
        session_name: snapshot.session_name.clone(),
        window_id: snapshot.window_id.clone(),
        window_index: snapshot.window_index,
        window_name: snapshot.window_name.clone(),
        pane_id: snapshot.pane_id.clone(),
        pane_index: snapshot.pane_index,
        pane_active: snapshot.pane_active,
        pane_current_path: snapshot.pane_current_path.clone(),
        pane_current_command: snapshot.pane_current_command.clone(),
        pane_title: snapshot.pane_title.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AppConfig, BackendKind, FileConfig, TaskRuntime};
    use crate::model::{TaskMode, TaskState};
    use std::path::PathBuf;

    fn task(id: &str, title: &str, branch: Option<&str>) -> TaskRecord {
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
        let mut task = TaskRecord::from_submit_with_id(payload, &config, id.to_string());
        task.state = TaskState::Running;
        task.branch = branch.map(str::to_string);
        task
    }

    fn entry(
        pane_id: &str,
        session_name: &str,
        current: bool,
        task: Option<TaskRecord>,
    ) -> PaneEntry {
        let snapshot = PaneSnapshot {
            current,
            managed_by_swarmux: task.is_some(),
            session_name: session_name.to_string(),
            window_id: "@1".to_string(),
            window_index: 1,
            window_name: "shell".to_string(),
            pane_id: pane_id.to_string(),
            pane_index: 1,
            pane_active: true,
            pane_current_path: "/tmp/core".to_string(),
            pane_current_command: "bash".to_string(),
            pane_title: "shell".to_string(),
            task,
            repo_root: Some("/tmp/core".to_string()),
            repo: Some("core".to_string()),
            branch: Some("main".to_string()),
            git: None,
            label: "initial".to_string(),
        };

        PaneEntry {
            snapshot,
            metadata_loaded: false,
        }
    }

    #[test]
    fn initial_selection_prefers_explicit_source_then_current_row() {
        let state = PaneSwitcherState {
            all_rows: vec![
                entry("%1", "alpha", false, None),
                entry("%2", "beta", true, Some(task("b", "beta", Some("main")))),
            ],
            rows: vec![
                entry("%1", "alpha", false, None),
                entry("%2", "beta", true, Some(task("b", "beta", Some("main")))),
            ],
            selected: 0,
            loaded_count: 0,
            current_session_only: false,
            current_session_name: None,
        };

        assert_eq!(state.initial_selected(Some("%2")), 1);
        assert_eq!(state.initial_selected(None), 1);
    }

    #[test]
    fn hydration_updates_label_and_loaded_count() {
        let mut state = PaneSwitcherState {
            all_rows: vec![entry(
                "%1",
                "alpha",
                true,
                Some(task("a", "alpha", Some("main"))),
            )],
            rows: vec![entry(
                "%1",
                "alpha",
                true,
                Some(task("a", "alpha", Some("main"))),
            )],
            selected: 0,
            loaded_count: 0,
            current_session_only: false,
            current_session_name: None,
        };
        let updated = state.apply_update(HydrationUpdate::PaneGit {
            pane_id: "%1".to_string(),
            git: Some(PaneGitSummary {
                dirty: true,
                changed_files: 1,
                deleted_files: 0,
                untracked_files: 2,
                insertions: 3,
                deletions: 4,
                label: "chg1 new2 +3/-4".to_string(),
            }),
        });

        assert!(updated);
        assert_eq!(state.loaded_count, 1);
        assert!(state.rows[0].metadata_loaded);
        assert!(state.rows[0].snapshot.label.contains("chg1 new2 +3/-4"));
    }

    #[test]
    fn selected_target_clamps_empty_state() {
        let state = PaneSwitcherState {
            all_rows: Vec::new(),
            rows: Vec::new(),
            selected: 3,
            loaded_count: 0,
            current_session_only: false,
            current_session_name: None,
        };

        assert_eq!(state.clamp_selected(3), 0);
    }

    #[test]
    fn filter_rows_keeps_only_current_session_when_enabled() {
        let current = entry("%1", "current", true, None);
        let other = entry("%2", "other", false, None);

        let rows = filter_rows(&[current.clone(), other], true, Some("current"));

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].snapshot.session_name, "current");
    }
}
