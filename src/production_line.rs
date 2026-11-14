use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::master::{MasterPlanError, PlanProposal};
use crate::safety::contains_disallowed_control;

pub const PRODUCTION_LINE_SCHEMA_VERSION: u32 = 1;
pub const TASK_LIST_FILE_NAME: &str = "任务清单.md";
pub const MAX_KEY_RESULTS: usize = 16;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct GoalContract {
    pub goal_id: String,
    pub master_conversation_id: String,
    pub objective: String,
    #[serde(default)]
    pub non_goals: Vec<String>,
    #[serde(default)]
    pub constraints: Vec<String>,
    #[serde(default)]
    pub key_results: Vec<KeyResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct KeyResult {
    pub id: String,
    pub statement: String,
    pub baseline: String,
    pub target: String,
    pub measurement_method: String,
    pub work_item_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProductionPlan {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    pub goal: GoalContract,
    pub plan: PlanProposal,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TaskListSnapshot {
    pub path: PathBuf,
    pub plan_sha256: String,
    pub markdown_sha256: String,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ProductionLineError {
    #[error("unsupported production-line schema version {0}")]
    UnsupportedVersion(u32),
    #[error("goal id or master conversation id is empty or unsafe")]
    InvalidGoalIdentity,
    #[error("goal objective is empty, unsafe or too large")]
    InvalidGoalObjective,
    #[error("goal has more than {MAX_KEY_RESULTS} key results")]
    TooManyKeyResults,
    #[error("goal {0} entries are empty, unsafe or too numerous")]
    InvalidGoalList(&'static str),
    #[error("key result id '{0}' is empty, duplicated or unsafe")]
    InvalidKeyResultId(String),
    #[error("key result '{id}' field '{field}' is empty, unsafe or too large")]
    InvalidKeyResultField { id: String, field: &'static str },
    #[error("key result '{0}' has no work item")]
    EmptyKeyResult(String),
    #[error("multi-task goal must contain at least one measurable key result")]
    MissingKeyResults,
    #[error("production-line plan must remain unapproved; approval is a separate user action")]
    PlanMustRemainUnapproved,
    #[error("goal id does not match plan goal id")]
    GoalIdentityMismatch,
    #[error("work item '{0}' is not assigned to a key result")]
    UnassignedWorkItem(String),
    #[error("work item '{0}' is assigned to more than one key result")]
    DuplicateKeyResultAssignment(String),
    #[error("key result references unknown work item '{0}'")]
    UnknownKeyResultWorkItem(String),
    #[error("invalid master plan: {0}")]
    InvalidPlan(#[from] MasterPlanError),
    #[error("unable to serialize production-line plan: {0}")]
    Serialization(String),
    #[error("unable to write task list: {0}")]
    TaskListIo(String),
    #[error("immutable task list already exists with different content: {0}")]
    TaskListConflict(String),
}

fn default_schema_version() -> u32 {
    PRODUCTION_LINE_SCHEMA_VERSION
}

impl ProductionPlan {
    pub fn validate(&self) -> Result<(), ProductionLineError> {
        if self.schema_version != PRODUCTION_LINE_SCHEMA_VERSION {
            return Err(ProductionLineError::UnsupportedVersion(self.schema_version));
        }
        if !safe_identity(&self.goal.goal_id) || !safe_identity(&self.goal.master_conversation_id) {
            return Err(ProductionLineError::InvalidGoalIdentity);
        }
        validate_text(&self.goal.objective, 8_192)
            .map_err(|_| ProductionLineError::InvalidGoalObjective)?;
        validate_text_list("non-goals", &self.goal.non_goals, 32, 2_000)?;
        validate_text_list("constraints", &self.goal.constraints, 32, 2_000)?;
        self.plan.validate()?;
        if self.plan.approved {
            return Err(ProductionLineError::PlanMustRemainUnapproved);
        }
        if self.goal.goal_id != self.plan.goal_id {
            return Err(ProductionLineError::GoalIdentityMismatch);
        }
        if self.goal.key_results.len() > MAX_KEY_RESULTS {
            return Err(ProductionLineError::TooManyKeyResults);
        }
        if self.plan.work_items.len() > 1 && self.goal.key_results.is_empty() {
            return Err(ProductionLineError::MissingKeyResults);
        }
        if self.goal.key_results.is_empty() {
            return Ok(());
        }

        let plan_tasks = self
            .plan
            .work_items
            .iter()
            .map(|item| item.id.as_str())
            .collect::<BTreeSet<_>>();
        let mut kr_ids = BTreeSet::new();
        let mut assignments = BTreeMap::<&str, usize>::new();
        for key_result in &self.goal.key_results {
            if !safe_identity(&key_result.id) || !kr_ids.insert(key_result.id.as_str()) {
                return Err(ProductionLineError::InvalidKeyResultId(
                    key_result.id.clone(),
                ));
            }
            for (field, value) in [
                ("statement", key_result.statement.as_str()),
                ("baseline", key_result.baseline.as_str()),
                ("target", key_result.target.as_str()),
                ("measurement_method", key_result.measurement_method.as_str()),
            ] {
                validate_text(value, 2_000).map_err(|_| {
                    ProductionLineError::InvalidKeyResultField {
                        id: key_result.id.clone(),
                        field,
                    }
                })?;
            }
            if key_result.work_item_ids.is_empty() {
                return Err(ProductionLineError::EmptyKeyResult(key_result.id.clone()));
            }
            for task_id in &key_result.work_item_ids {
                if !plan_tasks.contains(task_id.as_str()) {
                    return Err(ProductionLineError::UnknownKeyResultWorkItem(
                        task_id.clone(),
                    ));
                }
                *assignments.entry(task_id.as_str()).or_default() += 1;
            }
        }

        for task_id in plan_tasks {
            match assignments.get(task_id).copied().unwrap_or_default() {
                0 => {
                    return Err(ProductionLineError::UnassignedWorkItem(task_id.to_string()));
                }
                1 => {}
                _ => {
                    return Err(ProductionLineError::DuplicateKeyResultAssignment(
                        task_id.to_string(),
                    ));
                }
            }
        }
        Ok(())
    }

    pub fn plan_sha256(&self) -> Result<String, ProductionLineError> {
        self.validate()?;
        let bytes = serde_json::to_vec(self)
            .map_err(|error| ProductionLineError::Serialization(error.to_string()))?;
        Ok(hex::encode(Sha256::digest(bytes)))
    }

    pub fn render_task_list(&self) -> Result<String, ProductionLineError> {
        self.render_task_list_with_status("prepared")
    }

    pub fn render_plan_preview(&self) -> Result<String, ProductionLineError> {
        self.render_task_list_with_status("proposed")
    }

    fn render_task_list_with_status(&self, status: &str) -> Result<String, ProductionLineError> {
        let plan_sha256 = self.plan_sha256()?;
        let order = self.plan.topological_order()?;
        let mut output = String::new();

        push_line(&mut output, "---");
        push_line(&mut output, "schema: skynet.1agent.task-list/v1");
        push_line(
            &mut output,
            &format!(
                "goal_id: {}",
                serde_json::to_string(&self.goal.goal_id)
                    .expect("serializing a string cannot fail")
            ),
        );
        push_line(
            &mut output,
            &format!(
                "master_conversation_id: {}",
                serde_json::to_string(&self.goal.master_conversation_id)
                    .expect("serializing a string cannot fail")
            ),
        );
        push_line(
            &mut output,
            &format!(
                "plan_id: {}",
                serde_json::to_string(&self.plan.plan_id)
                    .expect("serializing a string cannot fail")
            ),
        );
        push_line(&mut output, &format!("plan_version: {}", self.plan.version));
        push_line(&mut output, &format!("plan_sha256: {plan_sha256}"));
        push_line(&mut output, &format!("status: {status}"));
        push_line(&mut output, "---\n");
        push_line(&mut output, "# 任务清单\n");
        push_line(
            &mut output,
            "> 本文件由结构化计划确定性生成。批准后不可编辑；运行时状态由 MasterState 和督战看板维护。\n",
        );
        push_line(&mut output, "## 目标\n");
        push_line(&mut output, &format!("{}\n", self.goal.objective));

        if !self.goal.non_goals.is_empty() {
            push_line(&mut output, "## 非目标\n");
            for value in &self.goal.non_goals {
                push_line(&mut output, &format!("- {value}"));
            }
            output.push('\n');
        }
        if !self.goal.constraints.is_empty() {
            push_line(&mut output, "## 约束\n");
            for value in &self.goal.constraints {
                push_line(&mut output, &format!("- {value}"));
            }
            output.push('\n');
        }

        if !self.goal.key_results.is_empty() {
            push_line(&mut output, "## Key Results\n");
            for key_result in &self.goal.key_results {
                push_line(
                    &mut output,
                    &format!("### {} — {}\n", key_result.id, key_result.statement),
                );
                push_line(&mut output, &format!("- 基线：{}", key_result.baseline));
                push_line(&mut output, &format!("- 目标：{}", key_result.target));
                push_line(
                    &mut output,
                    &format!("- 测量方式：{}", key_result.measurement_method),
                );
                push_line(
                    &mut output,
                    &format!("- Work Items：{}\n", key_result.work_item_ids.join(", ")),
                );
            }
        }

        push_line(&mut output, "## Work Items\n");
        for task_id in order {
            let item = self
                .plan
                .work_items
                .iter()
                .find(|item| item.id == task_id)
                .expect("topological order references a work item");
            let key_result_ids = self
                .goal
                .key_results
                .iter()
                .filter(|key_result| key_result.work_item_ids.iter().any(|id| id == &item.id))
                .map(|key_result| key_result.id.as_str())
                .collect::<Vec<_>>();
            push_line(&mut output, &format!("### {} — {}\n", item.id, item.title));
            push_line(&mut output, &format!("{}\n", item.objective));
            if !key_result_ids.is_empty() {
                push_line(
                    &mut output,
                    &format!("- Key Result：{}", key_result_ids.join(", ")),
                );
            }
            push_line(
                &mut output,
                &format!("- 依赖：{}", join_or_none(&item.dependencies)),
            );
            push_line(
                &mut output,
                &format!("- 能力：{}", join_or_none(&item.required_capabilities)),
            );
            push_line(
                &mut output,
                &format!("- 工具：{}", join_or_none(&item.required_tools)),
            );
            push_line(
                &mut output,
                &format!(
                    "- 写入权限：{}",
                    if item.permissions.allow_write {
                        "允许"
                    } else {
                        "禁止"
                    }
                ),
            );
            push_line(
                &mut output,
                &format!(
                    "- 网络权限：{}",
                    if item.permissions.allow_network {
                        "允许"
                    } else {
                        "禁止"
                    }
                ),
            );
            push_line(
                &mut output,
                &format!("- 验收：{}", item.acceptance_criteria.join("；")),
            );
            push_line(
                &mut output,
                &format!("- 预期产物：{}", item.expected_artifacts.join(", ")),
            );
            push_line(
                &mut output,
                &format!("- 工作区：`{}`", item.workspace.display()),
            );
            push_line(
                &mut output,
                &format!(
                    "- 预算：{} steps / {} ms / {} tool calls\n",
                    item.budget.max_steps, item.budget.timeout_ms, item.budget.max_tool_calls
                ),
            );
        }
        Ok(output)
    }

    pub fn write_task_list(
        &self,
        state_root: impl AsRef<Path>,
    ) -> Result<TaskListSnapshot, ProductionLineError> {
        self.validate()?;
        let directory = state_root
            .as_ref()
            .join(&self.goal.goal_id)
            .join(format!("v{}", self.plan.version));
        std::fs::create_dir_all(&directory)
            .map_err(|error| ProductionLineError::TaskListIo(error.to_string()))?;
        let path = directory.join(TASK_LIST_FILE_NAME);
        let markdown = self.render_task_list()?;
        let bytes = markdown.as_bytes();

        if path.exists() {
            let existing = std::fs::read(&path)
                .map_err(|error| ProductionLineError::TaskListIo(error.to_string()))?;
            if existing != bytes {
                return Err(ProductionLineError::TaskListConflict(
                    path.display().to_string(),
                ));
            }
        } else {
            let mut temporary = tempfile::NamedTempFile::new_in(&directory)
                .map_err(|error| ProductionLineError::TaskListIo(error.to_string()))?;
            temporary
                .write_all(bytes)
                .map_err(|error| ProductionLineError::TaskListIo(error.to_string()))?;
            temporary
                .as_file()
                .sync_all()
                .map_err(|error| ProductionLineError::TaskListIo(error.to_string()))?;
            temporary
                .persist(&path)
                .map_err(|error| ProductionLineError::TaskListIo(error.error.to_string()))?;
        }

        Ok(TaskListSnapshot {
            path,
            plan_sha256: self.plan_sha256()?,
            markdown_sha256: hex::encode(Sha256::digest(bytes)),
        })
    }
}

fn validate_text_list(
    field: &'static str,
    values: &[String],
    max_items: usize,
    max_chars: usize,
) -> Result<(), ProductionLineError> {
    if values.len() > max_items {
        return Err(ProductionLineError::InvalidGoalList(field));
    }
    for value in values {
        validate_text(value, max_chars).map_err(|_| ProductionLineError::InvalidGoalList(field))?;
    }
    Ok(())
}

fn validate_text(value: &str, max_chars: usize) -> Result<(), ()> {
    if value.trim().is_empty()
        || value.chars().count() > max_chars
        || contains_disallowed_control(value)
    {
        return Err(());
    }
    Ok(())
}

fn safe_identity(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
}

fn join_or_none(values: &[String]) -> String {
    if values.is_empty() {
        "无".into()
    } else {
        values.join(", ")
    }
}

fn push_line(output: &mut String, line: &str) {
    writeln!(output, "{line}").expect("writing to String cannot fail");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{PermissionSet, RiskLevel};
    use crate::master::{MASTER_PLAN_SCHEMA_VERSION, WorkBudget, WorkItem, WorkItemStatus};

    fn item(id: &str, dependencies: &[&str]) -> WorkItem {
        WorkItem {
            id: id.into(),
            title: format!("任务 {id}"),
            objective: format!("完成 {id}"),
            dependencies: dependencies.iter().map(|value| (*value).into()).collect(),
            required_capabilities: vec!["rust".into()],
            required_tools: vec!["shell".into()],
            permissions: PermissionSet::default(),
            acceptance_criteria: vec![format!("{id} 验收通过")],
            expected_artifacts: vec![format!("{id}.log")],
            workspace: PathBuf::from("/tmp"),
            budget: WorkBudget::default(),
            deadline_unix_ms: None,
            risk: RiskLevel::Low,
            status: WorkItemStatus::Pending,
        }
    }

    fn key_result(id: &str, work_item_ids: &[&str]) -> KeyResult {
        KeyResult {
            id: id.into(),
            statement: format!("{id} 达成"),
            baseline: "0".into(),
            target: "1".into(),
            measurement_method: "automated test".into(),
            work_item_ids: work_item_ids.iter().map(|value| (*value).into()).collect(),
        }
    }

    fn production_plan(work_items: Vec<WorkItem>, key_results: Vec<KeyResult>) -> ProductionPlan {
        ProductionPlan {
            schema_version: PRODUCTION_LINE_SCHEMA_VERSION,
            goal: GoalContract {
                goal_id: "goal-1".into(),
                master_conversation_id: "master-1".into(),
                objective: "交付可验证能力".into(),
                non_goals: vec!["不做无关扩张".into()],
                constraints: vec!["失败关闭".into()],
                key_results,
            },
            plan: PlanProposal {
                schema_version: MASTER_PLAN_SCHEMA_VERSION,
                plan_id: "plan-1".into(),
                goal_id: "goal-1".into(),
                version: 1,
                summary: "交付计划".into(),
                work_items,
                max_parallel: 2,
                approved: false,
            },
        }
    }

    #[test]
    fn two_key_results_four_tasks_validate_and_render_deterministically() {
        let plan = production_plan(
            vec![
                item("a", &[]),
                item("b", &["a"]),
                item("c", &[]),
                item("d", &["c"]),
            ],
            vec![
                key_result("kr-1", &["a", "b"]),
                key_result("kr-2", &["c", "d"]),
            ],
        );
        plan.validate().unwrap();
        let first = plan.render_task_list().unwrap();
        let second = plan.render_task_list().unwrap();
        assert_eq!(first, second);
        let decoded: ProductionPlan =
            serde_json::from_slice(&serde_json::to_vec(&plan).unwrap()).unwrap();
        assert_eq!(decoded.render_task_list().unwrap(), first);
        assert!(first.contains("# 任务清单"));
        assert!(first.contains("### kr-1"));
        assert!(first.contains("### a — 任务 a"));
        assert!(first.contains("plan_sha256:"));
        assert!(
            plan.render_plan_preview()
                .unwrap()
                .contains("status: proposed")
        );
    }

    #[test]
    fn multi_task_goal_requires_key_results_and_complete_assignment() {
        let without_kr = production_plan(vec![item("a", &[]), item("b", &[])], Vec::new());
        assert_eq!(
            without_kr.validate(),
            Err(ProductionLineError::MissingKeyResults)
        );

        let partial = production_plan(
            vec![item("a", &[]), item("b", &[])],
            vec![key_result("kr-1", &["a"])],
        );
        assert_eq!(
            partial.validate(),
            Err(ProductionLineError::UnassignedWorkItem("b".into()))
        );
    }

    #[test]
    fn unknown_or_duplicate_key_result_tasks_fail_closed() {
        let unknown = production_plan(vec![item("a", &[])], vec![key_result("kr-1", &["missing"])]);
        assert_eq!(
            unknown.validate(),
            Err(ProductionLineError::UnknownKeyResultWorkItem(
                "missing".into()
            ))
        );

        let duplicate = production_plan(
            vec![item("a", &[])],
            vec![key_result("kr-1", &["a"]), key_result("kr-2", &["a"])],
        );
        assert_eq!(
            duplicate.validate(),
            Err(ProductionLineError::DuplicateKeyResultAssignment(
                "a".into()
            ))
        );
    }

    #[test]
    fn unmeasurable_or_unsafe_goal_contract_fails_closed() {
        let mut plan = production_plan(
            vec![item("a", &[]), item("b", &[])],
            vec![key_result("kr-1", &["a", "b"])],
        );
        plan.goal.key_results[0].measurement_method.clear();
        assert_eq!(
            plan.validate(),
            Err(ProductionLineError::InvalidKeyResultField {
                id: "kr-1".into(),
                field: "measurement_method",
            })
        );

        let mut unsafe_goal =
            production_plan(vec![item("a", &[])], vec![key_result("kr-1", &["a"])]);
        unsafe_goal.goal.goal_id = "../escape".into();
        assert_eq!(
            unsafe_goal.validate(),
            Err(ProductionLineError::InvalidGoalIdentity)
        );
    }

    #[test]
    fn single_direct_task_can_omit_key_results() {
        let plan = production_plan(vec![item("a", &[])], Vec::new());
        plan.validate().unwrap();
    }

    #[test]
    fn production_plan_rejects_model_self_approval() {
        let mut plan = production_plan(vec![item("a", &[])], Vec::new());
        plan.plan.approved = true;
        assert_eq!(
            plan.validate(),
            Err(ProductionLineError::PlanMustRemainUnapproved)
        );
    }

    #[test]
    fn task_list_is_atomic_and_immutable_for_a_revision() {
        let plan = production_plan(vec![item("a", &[])], vec![key_result("kr-1", &["a"])]);
        let root = tempfile::tempdir().unwrap();
        let snapshot = plan.write_task_list(root.path()).unwrap();
        assert_eq!(
            snapshot.path.file_name().unwrap().to_string_lossy(),
            TASK_LIST_FILE_NAME
        );
        assert_eq!(
            snapshot.path,
            root.path()
                .join("goal-1")
                .join("v1")
                .join(TASK_LIST_FILE_NAME)
        );
        assert_eq!(snapshot.plan_sha256, plan.plan_sha256().unwrap());
        assert_eq!(
            snapshot.markdown_sha256,
            hex::encode(Sha256::digest(std::fs::read(&snapshot.path).unwrap()))
        );
        assert_eq!(plan.write_task_list(root.path()).unwrap(), snapshot);

        let changed = production_plan(
            vec![item("renamed", &[])],
            vec![key_result("kr-1", &["renamed"])],
        );
        assert!(matches!(
            changed.write_task_list(root.path()),
            Err(ProductionLineError::TaskListConflict(_))
        ));
    }

    #[test]
    fn invalid_plan_creates_no_task_list() {
        let plan = production_plan(vec![item("a", &[]), item("b", &[])], Vec::new());
        let root = tempfile::tempdir().unwrap();
        assert_eq!(
            plan.write_task_list(root.path()),
            Err(ProductionLineError::MissingKeyResults)
        );
        assert!(!root.path().join("goal-1").exists());
    }
}
