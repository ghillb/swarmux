use crate::cli::OverviewScope;
use crate::config::TaskRuntime;
use crate::model::{TaskMode, TaskRecord, TaskState};
use crate::overview_scope_matches;
use crate::panes_support::runtime_label;
use crate::store::Store;
use anyhow::Result;
use chrono::{DateTime, Utc};
use std::cmp::Reverse;

#[derive(Clone, Debug, Default)]
pub(crate) struct TaskSummary {
    pub(crate) total: usize,
    pub(crate) queued: usize,
    pub(crate) dispatching: usize,
    pub(crate) running: usize,
    pub(crate) waiting_input: usize,
    pub(crate) succeeded: usize,
    pub(crate) failed: usize,
    pub(crate) canceled: usize,
    pub(crate) manual: usize,
    pub(crate) auto: usize,
    pub(crate) headless: usize,
    pub(crate) mirrored: usize,
    pub(crate) tui: usize,
    pub(crate) with_session: usize,
}

impl TaskSummary {
    pub(crate) fn from(tasks: &[TaskRecord]) -> Self {
        let mut summary = Self {
            total: tasks.len(),
            ..Self::default()
        };

        for task in tasks {
            match task.state {
                TaskState::Queued => summary.queued += 1,
                TaskState::Dispatching => summary.dispatching += 1,
                TaskState::Running => summary.running += 1,
                TaskState::WaitingInput => summary.waiting_input += 1,
                TaskState::Succeeded => summary.succeeded += 1,
                TaskState::Failed => summary.failed += 1,
                TaskState::Canceled => summary.canceled += 1,
            }

            summary.with_session += usize::from(task.session.is_some());
            match task.mode {
                TaskMode::Manual => summary.manual += 1,
                TaskMode::Auto => summary.auto += 1,
            }

            match task.runtime {
                TaskRuntime::Headless => summary.headless += 1,
                TaskRuntime::Mirrored => summary.mirrored += 1,
                TaskRuntime::Tui => summary.tui += 1,
            }
        }

        summary
    }

    pub(crate) fn active(&self) -> usize {
        self.queued + self.dispatching + self.running + self.waiting_input
    }

    pub(crate) fn terminal(&self) -> usize {
        self.succeeded + self.failed + self.canceled
    }
}

#[derive(Clone, Debug)]
pub(crate) struct DashboardData {
    pub(crate) generated_at: DateTime<Utc>,
    pub(crate) scope: OverviewScope,
    pub(crate) visible_tasks: Vec<TaskRecord>,
    pub(crate) all_tasks: Vec<TaskRecord>,
    pub(crate) visible_summary: TaskSummary,
    pub(crate) all_summary: TaskSummary,
    pub(crate) repo_counts: Vec<(String, usize)>,
    pub(crate) runtime_counts: Vec<(String, usize)>,
}

impl DashboardData {
    pub(crate) fn load(store: &Store, scope: OverviewScope) -> Result<Self> {
        let mut all_tasks = store.list()?;
        all_tasks.sort_by_key(|task| (task.updated_at, task.created_at));
        all_tasks.reverse();

        let visible_tasks = all_tasks
            .iter()
            .filter(|task| overview_scope_matches(&task.state, &scope))
            .cloned()
            .collect::<Vec<_>>();

        let visible_summary = TaskSummary::from(&visible_tasks);
        let all_summary = TaskSummary::from(&all_tasks);
        let repo_counts = count_by(&all_tasks, |task| task.repo.clone(), 5);
        let runtime_counts = count_by(&all_tasks, |task| runtime_label(task).to_string(), 3);

        Ok(Self {
            generated_at: Utc::now(),
            scope,
            visible_tasks,
            all_tasks,
            visible_summary,
            all_summary,
            repo_counts,
            runtime_counts,
        })
    }
}

fn count_by<F>(tasks: &[TaskRecord], mut key: F, limit: usize) -> Vec<(String, usize)>
where
    F: FnMut(&TaskRecord) -> String,
{
    let mut counts = std::collections::BTreeMap::<String, usize>::new();
    for task in tasks {
        *counts.entry(key(task)).or_insert(0) += 1;
    }

    let mut items = counts.into_iter().collect::<Vec<_>>();
    items.sort_by_key(|(label, count)| (Reverse(*count), label.clone()));
    items.truncate(limit);
    items
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AppConfig, BackendKind, FileConfig};
    use crate::model::SubmitPayload;
    use std::path::PathBuf;

    fn task(id: &str, title: &str, state: TaskState, runtime: TaskRuntime) -> TaskRecord {
        let config = AppConfig {
            home: PathBuf::from("/tmp/swarmux-test-home"),
            config_home: PathBuf::from("/tmp/swarmux-test-config"),
            backend: BackendKind::Files,
            settings: FileConfig::default(),
        };
        let payload = SubmitPayload {
            title: title.to_string(),
            repo_ref: "core".to_string(),
            repo_root: "/tmp/core".to_string(),
            mode: TaskMode::Manual,
            runtime,
            worktree: None,
            session: Some(format!("session-{id}")),
            command: vec!["echo".to_string(), title.to_string()],
            priority: None,
            external_ref: None,
            origin: None,
        };
        let mut task = TaskRecord::from_submit_with_id(payload, &config, id.to_string());
        task.state = state;
        task
    }

    #[test]
    fn dashboard_counts_visible_and_all_tasks_separately() {
        let all = vec![
            task("a", "alpha", TaskState::Running, TaskRuntime::Tui),
            task("b", "beta", TaskState::Succeeded, TaskRuntime::Mirrored),
            task("c", "gamma", TaskState::WaitingInput, TaskRuntime::Headless),
        ];
        let dashboard = DashboardData {
            generated_at: Utc::now(),
            scope: OverviewScope::NonTerminal,
            visible_tasks: all
                .iter()
                .filter(|task| overview_scope_matches(&task.state, &OverviewScope::NonTerminal))
                .cloned()
                .collect(),
            all_tasks: all.clone(),
            visible_summary: TaskSummary::from(
                &all.iter()
                    .filter(|task| overview_scope_matches(&task.state, &OverviewScope::NonTerminal))
                    .cloned()
                    .collect::<Vec<_>>(),
            ),
            all_summary: TaskSummary::from(&all),
            repo_counts: count_by(&all, |task| task.repo.clone(), 5),
            runtime_counts: count_by(&all, |task| runtime_label(task).to_string(), 3),
        };

        assert_eq!(dashboard.visible_summary.total, 2);
        assert_eq!(dashboard.all_summary.total, 3);
        assert_eq!(dashboard.all_summary.succeeded, 1);
        assert_eq!(dashboard.all_summary.running, 1);
        assert_eq!(dashboard.runtime_counts.len(), 3);
        assert_eq!(dashboard.repo_counts.len(), 1);
    }
}
