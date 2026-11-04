use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::master::WorkItemStatus;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SchedulerLimits {
    pub global_active: usize,
    pub per_conversation: usize,
}

impl Default for SchedulerLimits {
    fn default() -> Self {
        Self {
            global_active: 6,
            per_conversation: 3,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ScheduledTask {
    pub task_id: String,
    pub dependencies: Vec<String>,
    pub workspace: PathBuf,
    pub allow_write: bool,
    pub status: WorkItemStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ConversationQueue {
    pub conversation_id: String,
    pub tasks: Vec<ScheduledTask>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SchedulerStart {
    pub conversation_id: String,
    pub task_id: String,
    pub stable_id: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SchedulerWaitReason {
    Dependency,
    GlobalCapacity,
    ConversationCapacity,
    WriteConflict,
    NotReady,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SchedulerWait {
    pub conversation_id: String,
    pub task_id: String,
    pub stable_id: String,
    pub reason: SchedulerWaitReason,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SchedulerDecision {
    pub starts: Vec<SchedulerStart>,
    pub waiting: Vec<SchedulerWait>,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum SchedulerError {
    #[error("scheduler limits must be greater than zero")]
    InvalidLimits,
    #[error("conversation id '{0}' is empty or duplicated")]
    InvalidConversation(String),
    #[error("task id '{0}' is empty or duplicated within its conversation")]
    InvalidTask(String),
    #[error("task '{task}' depends on unknown stable id '{dependency}'")]
    UnknownDependency { task: String, dependency: String },
}

pub fn schedule_ready(
    conversations: &[ConversationQueue],
    limits: SchedulerLimits,
) -> Result<SchedulerDecision, SchedulerError> {
    if limits.global_active == 0 || limits.per_conversation == 0 {
        return Err(SchedulerError::InvalidLimits);
    }
    let mut conversation_ids = BTreeSet::new();
    let mut stable_status = BTreeMap::<String, WorkItemStatus>::new();
    let mut global_active = 0usize;
    let mut conversation_active = BTreeMap::<String, usize>::new();
    let mut active_write_roots = BTreeSet::<PathBuf>::new();

    for conversation in conversations {
        if !safe_identity(&conversation.conversation_id)
            || !conversation_ids.insert(conversation.conversation_id.as_str())
        {
            return Err(SchedulerError::InvalidConversation(
                conversation.conversation_id.clone(),
            ));
        }
        let mut task_ids = BTreeSet::new();
        for task in &conversation.tasks {
            if !safe_identity(&task.task_id) || !task_ids.insert(task.task_id.as_str()) {
                return Err(SchedulerError::InvalidTask(task.task_id.clone()));
            }
            stable_status.insert(
                stable_id(&conversation.conversation_id, &task.task_id),
                task.status,
            );
        }
        for task in &conversation.tasks {
            let task_stable = stable_id(&conversation.conversation_id, &task.task_id);
            for dependency in &task.dependencies {
                let dependency_stable = stable_id(&conversation.conversation_id, dependency);
                if !stable_status.contains_key(&dependency_stable) {
                    return Err(SchedulerError::UnknownDependency {
                        task: task_stable,
                        dependency: dependency_stable,
                    });
                }
            }
            if is_active(task.status) {
                global_active += 1;
                *conversation_active
                    .entry(conversation.conversation_id.clone())
                    .or_default() += 1;
                if task.allow_write {
                    active_write_roots.insert(task.workspace.clone());
                }
            }
        }
    }

    let mut decision = SchedulerDecision::default();
    let mut started = BTreeSet::new();
    let mut selected_write_roots = BTreeSet::<PathBuf>::new();
    loop {
        let mut progressed = false;
        for conversation in conversations {
            if global_active >= limits.global_active {
                break;
            }
            if conversation_active
                .get(&conversation.conversation_id)
                .copied()
                .unwrap_or_default()
                >= limits.per_conversation
            {
                continue;
            }
            let candidate = conversation.tasks.iter().find(|task| {
                let stable = stable_id(&conversation.conversation_id, &task.task_id);
                !started.contains(&stable)
                    && matches!(
                        task.status,
                        WorkItemStatus::Pending | WorkItemStatus::Ready | WorkItemStatus::Retrying
                    )
                    && task.dependencies.iter().all(|dependency| {
                        stable_status.get(&stable_id(&conversation.conversation_id, dependency))
                            == Some(&WorkItemStatus::Verified)
                    })
                    && (!task.allow_write
                        || (!active_write_roots
                            .iter()
                            .any(|root| paths_overlap(root, &task.workspace))
                            && !selected_write_roots
                                .iter()
                                .any(|root| paths_overlap(root, &task.workspace))))
            });
            let Some(task) = candidate else {
                continue;
            };
            let stable = stable_id(&conversation.conversation_id, &task.task_id);
            started.insert(stable.clone());
            global_active += 1;
            *conversation_active
                .entry(conversation.conversation_id.clone())
                .or_default() += 1;
            if task.allow_write {
                selected_write_roots.insert(task.workspace.clone());
            }
            decision.starts.push(SchedulerStart {
                conversation_id: conversation.conversation_id.clone(),
                task_id: task.task_id.clone(),
                stable_id: stable,
            });
            progressed = true;
        }
        if !progressed || global_active >= limits.global_active {
            break;
        }
    }

    for conversation in conversations {
        for task in &conversation.tasks {
            let stable = stable_id(&conversation.conversation_id, &task.task_id);
            if is_active(task.status)
                || matches!(
                    task.status,
                    WorkItemStatus::Verified | WorkItemStatus::Rejected | WorkItemStatus::Cancelled
                )
                || started.contains(&stable)
            {
                continue;
            }
            let dependency_wait = task.dependencies.iter().any(|dependency| {
                stable_status.get(&stable_id(&conversation.conversation_id, dependency))
                    != Some(&WorkItemStatus::Verified)
            });
            let reason = if dependency_wait {
                SchedulerWaitReason::Dependency
            } else if !matches!(
                task.status,
                WorkItemStatus::Pending | WorkItemStatus::Ready | WorkItemStatus::Retrying
            ) {
                SchedulerWaitReason::NotReady
            } else if task.allow_write
                && (active_write_roots
                    .iter()
                    .any(|root| paths_overlap(root, &task.workspace))
                    || selected_write_roots
                        .iter()
                        .any(|root| paths_overlap(root, &task.workspace)))
            {
                SchedulerWaitReason::WriteConflict
            } else if conversation_active
                .get(&conversation.conversation_id)
                .copied()
                .unwrap_or_default()
                >= limits.per_conversation
            {
                SchedulerWaitReason::ConversationCapacity
            } else {
                SchedulerWaitReason::GlobalCapacity
            };
            decision.waiting.push(SchedulerWait {
                conversation_id: conversation.conversation_id.clone(),
                task_id: task.task_id.clone(),
                stable_id: stable,
                reason,
            });
        }
    }
    Ok(decision)
}

fn is_active(status: WorkItemStatus) -> bool {
    matches!(
        status,
        WorkItemStatus::Assigned
            | WorkItemStatus::Starting
            | WorkItemStatus::Running
            | WorkItemStatus::AwaitingInput
            | WorkItemStatus::Verifying
    )
}

fn stable_id(conversation_id: &str, task_id: &str) -> String {
    format!("{conversation_id}:{task_id}")
}

fn safe_identity(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
}

fn paths_overlap(left: &std::path::Path, right: &std::path::Path) -> bool {
    left == right || left.starts_with(right) || right.starts_with(left)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task(
        id: &str,
        dependencies: &[&str],
        workspace: &str,
        allow_write: bool,
        status: WorkItemStatus,
    ) -> ScheduledTask {
        ScheduledTask {
            task_id: id.into(),
            dependencies: dependencies.iter().map(|value| (*value).into()).collect(),
            workspace: PathBuf::from(workspace),
            allow_write,
            status,
        }
    }

    #[test]
    fn two_goals_share_a_global_cap_deterministically() {
        let conversations = vec![
            ConversationQueue {
                conversation_id: "master-1".into(),
                tasks: vec![
                    task("t1", &[], "/tmp/ws1", false, WorkItemStatus::Pending),
                    task("t2", &[], "/tmp/ws1", false, WorkItemStatus::Pending),
                    task("t3", &["t1"], "/tmp/ws1", false, WorkItemStatus::Pending),
                    task("t4", &["t2"], "/tmp/ws1", false, WorkItemStatus::Pending),
                ],
            },
            ConversationQueue {
                conversation_id: "master-2".into(),
                tasks: vec![
                    task("t1", &[], "/tmp/ws2", false, WorkItemStatus::Pending),
                    task("t2", &[], "/tmp/ws2", false, WorkItemStatus::Pending),
                    task("t3", &[], "/tmp/ws2", false, WorkItemStatus::Pending),
                    task("t4", &[], "/tmp/ws2", false, WorkItemStatus::Pending),
                    task("t5", &[], "/tmp/ws2", false, WorkItemStatus::Pending),
                ],
            },
        ];
        let decision = schedule_ready(
            &conversations,
            SchedulerLimits {
                global_active: 3,
                per_conversation: 2,
            },
        )
        .unwrap();
        assert_eq!(
            decision
                .starts
                .iter()
                .map(|item| item.stable_id.as_str())
                .collect::<Vec<_>>(),
            ["master-1:t1", "master-2:t1", "master-1:t2"]
        );
        assert_eq!(decision.waiting.len(), 6);
        assert!(decision.waiting.iter().any(|item| {
            item.stable_id == "master-2:t2" && item.reason == SchedulerWaitReason::GlobalCapacity
        }));
    }

    #[test]
    fn only_verified_dependencies_unlock_tasks() {
        let conversations = vec![ConversationQueue {
            conversation_id: "master-1".into(),
            tasks: vec![
                task("t1", &[], "/tmp/ws", false, WorkItemStatus::Running),
                task("t2", &["t1"], "/tmp/ws", false, WorkItemStatus::Ready),
            ],
        }];
        let decision = schedule_ready(&conversations, SchedulerLimits::default()).unwrap();
        assert!(decision.starts.is_empty());
        assert_eq!(decision.waiting[0].reason, SchedulerWaitReason::Dependency);

        let conversations = vec![ConversationQueue {
            conversation_id: "master-1".into(),
            tasks: vec![
                task("t1", &[], "/tmp/ws", false, WorkItemStatus::Verified),
                task("t2", &["t1"], "/tmp/ws", false, WorkItemStatus::Ready),
            ],
        }];
        let decision = schedule_ready(&conversations, SchedulerLimits::default()).unwrap();
        assert_eq!(decision.starts[0].stable_id, "master-1:t2");
    }

    #[test]
    fn overlapping_write_roots_are_serialized() {
        let conversations = vec![ConversationQueue {
            conversation_id: "master-1".into(),
            tasks: vec![
                task("a", &[], "/tmp/ws", true, WorkItemStatus::Running),
                task("b", &[], "/tmp/ws/sub", true, WorkItemStatus::Ready),
                task("c", &[], "/tmp/other", true, WorkItemStatus::Ready),
            ],
        }];
        let decision = schedule_ready(&conversations, SchedulerLimits::default()).unwrap();
        assert_eq!(
            decision
                .starts
                .iter()
                .map(|item| item.task_id.as_str())
                .collect::<Vec<_>>(),
            ["c"]
        );
        assert!(decision.waiting.iter().any(|item| {
            item.task_id == "b" && item.reason == SchedulerWaitReason::WriteConflict
        }));
    }
}
