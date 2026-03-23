use crate::config::PaneSwitcherHighlight;
use crate::model::TaskRecord;
use crate::panes::{PaneGitSummary, PaneSnapshot};
use crate::panes_support::{
    RawPane, build_label, git_context, git_info, list_tasks, list_tmux_panes, pane_sort_key,
};
use crate::panes_tui_detail::{row_git_line, row_repo_line};
use crate::store::Store;
use anyhow::Result;
use ratatui::style::{Color, Modifier, Style};
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

#[derive(Clone, Debug)]
pub(crate) struct PaneEntry {
    pub(crate) snapshot: PaneSnapshot,
    pub(crate) metadata_loaded: bool,
}

#[derive(Debug)]
pub(crate) struct PaneSwitcherState {
    pub(crate) rows: Vec<PaneEntry>,
    pub(crate) selected: usize,
    pub(crate) loaded_count: usize,
}

#[derive(Debug)]
pub(crate) enum HydrationUpdate {
    PaneGit {
        pane_id: String,
        git: Option<PaneGitSummary>,
    },
}

impl PaneSwitcherState {
    pub(crate) fn load(store: &Store, source_pane_id: Option<&str>) -> Result<Self> {
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
        let raw_panes = list_tmux_panes(Some(filter.as_str()))?;
        let mut rows = raw_panes
            .into_iter()
            .map(|raw| build_entry(&raw, &tasks_by_session, source_pane_id))
            .collect::<Vec<_>>();

        rows.sort_by_key(|entry| pane_sort_key(&entry.snapshot));

        Ok(Self {
            rows,
            selected: 0,
            loaded_count: 0,
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

    pub(crate) fn apply_update(&mut self, update: HydrationUpdate) -> bool {
        match update {
            HydrationUpdate::PaneGit { pane_id, git } => {
                let Some(entry) = self
                    .rows
                    .iter_mut()
                    .find(|entry| entry.snapshot.pane_id == pane_id)
                else {
                    return false;
                };

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
                self.loaded_count += 1;
                true
            }
        }
    }
}

impl PaneEntry {
    pub(crate) fn row_cells(
        &self,
        selected: bool,
        mode: PaneSwitcherHighlight,
        show_arrow: bool,
    ) -> Row<'static> {
        let marker = if selected && show_arrow {
            "▶"
        } else if self.snapshot.current {
            "●"
        } else {
            " "
        };
        let style = if selected {
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
        };

        Row::new(vec![
            Cell::from(marker),
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
        .style(style)
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
            rows: vec![
                entry("%1", "alpha", false, None),
                entry("%2", "beta", true, Some(task("b", "beta", Some("main")))),
            ],
            selected: 0,
            loaded_count: 0,
        };

        assert_eq!(state.initial_selected(Some("%2")), 1);
        assert_eq!(state.initial_selected(None), 1);
    }

    #[test]
    fn hydration_updates_label_and_loaded_count() {
        let mut state = PaneSwitcherState {
            rows: vec![entry(
                "%1",
                "alpha",
                true,
                Some(task("a", "alpha", Some("main"))),
            )],
            selected: 0,
            loaded_count: 0,
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
            rows: Vec::new(),
            selected: 3,
            loaded_count: 0,
        };

        assert_eq!(state.clamp_selected(3), 0);
    }
}
