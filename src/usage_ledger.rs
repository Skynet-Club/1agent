use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::ToolStatus;
use crate::journal::{JournalEvent, JournalEventKind, read_events};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct UsageTotals {
    pub calls: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cost_micros: u64,
    pub zero_cost_calls: u64,
}

impl UsageTotals {
    pub fn add(&mut self, input_tokens: u64, output_tokens: u64, cost_micros: u64) {
        self.calls = self.calls.saturating_add(1);
        self.input_tokens = self.input_tokens.saturating_add(input_tokens);
        self.output_tokens = self.output_tokens.saturating_add(output_tokens);
        self.cost_micros = self.cost_micros.saturating_add(cost_micros);
        if cost_micros == 0 {
            self.zero_cost_calls = self.zero_cost_calls.saturating_add(1);
        }
    }

    pub fn total_tokens(&self) -> u64 {
        self.input_tokens.saturating_add(self.output_tokens)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UsageGroup {
    pub provider_id: String,
    pub model: String,
    pub totals: UsageTotals,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunUsage {
    pub run_id: Uuid,
    pub totals: UsageTotals,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskUsageGroup {
    pub objective: String,
    pub runs: u64,
    pub totals: UsageTotals,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolUsageGroup {
    pub tool: String,
    pub calls: u64,
    pub failures: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct UsageLedger {
    pub journals: usize,
    pub totals: UsageTotals,
    pub providers: Vec<UsageGroup>,
    pub tasks: Vec<TaskUsageGroup>,
    pub tools: Vec<ToolUsageGroup>,
    pub runs: Vec<RunUsage>,
}

pub fn build_usage_ledger(
    journal_paths: impl IntoIterator<Item = PathBuf>,
) -> Result<UsageLedger, String> {
    let mut journals = 0;
    let mut events = Vec::new();
    for path in journal_paths {
        journals += 1;
        events.extend(read_events(&path).map_err(|error| format!("{}: {error}", path.display()))?);
    }
    let mut ledger = summarize_usage(&events);
    ledger.journals = journals;
    Ok(ledger)
}

pub fn summarize_usage(events: &[JournalEvent]) -> UsageLedger {
    let mut providers = BTreeMap::<(String, String), UsageTotals>::new();
    let mut tasks = BTreeMap::<String, (UsageTotals, BTreeSet<Uuid>)>::new();
    let mut tool_usage = BTreeMap::<String, (u64, u64)>::new();
    let mut runs = BTreeMap::<Uuid, UsageTotals>::new();
    let mut totals = UsageTotals::default();
    let mut objectives = BTreeMap::<Uuid, String>::new();
    for event in events {
        match &event.event {
            JournalEventKind::RunStarted { request, .. }
            | JournalEventKind::ResumeStarted { request, .. } => {
                objectives
                    .entry(event.run_id)
                    .or_insert_with(|| request.objective.clone());
            }
            JournalEventKind::ModelResponded { response } => {
                let usage = &response.usage;
                totals.add(usage.input_tokens, usage.output_tokens, usage.cost_micros);
                providers
                    .entry((response.provider_id.clone(), response.model.clone()))
                    .or_default()
                    .add(usage.input_tokens, usage.output_tokens, usage.cost_micros);
                let objective = objectives
                    .get(&event.run_id)
                    .cloned()
                    .unwrap_or_else(|| "(untitled run)".into());
                let (task_totals, task_runs) = tasks.entry(objective).or_default();
                task_totals.add(usage.input_tokens, usage.output_tokens, usage.cost_micros);
                task_runs.insert(event.run_id);
                runs.entry(event.run_id).or_default().add(
                    usage.input_tokens,
                    usage.output_tokens,
                    usage.cost_micros,
                );
            }
            JournalEventKind::ToolObserved { observation } => {
                let (calls, failures) = tool_usage.entry(observation.tool.clone()).or_default();
                *calls += 1;
                if observation.status != ToolStatus::Ok {
                    *failures += 1;
                }
            }
            _ => {}
        }
    }
    let mut tasks = tasks
        .into_iter()
        .map(|(objective, (totals, runs))| TaskUsageGroup {
            objective,
            runs: runs.len() as u64,
            totals,
        })
        .collect::<Vec<_>>();
    tasks.sort_by(|left, right| {
        right
            .totals
            .cost_micros
            .cmp(&left.totals.cost_micros)
            .then_with(|| left.objective.cmp(&right.objective))
    });
    let mut tools = tool_usage
        .into_iter()
        .map(|(tool, (calls, failures))| ToolUsageGroup {
            tool,
            calls,
            failures,
        })
        .collect::<Vec<_>>();
    tools.sort_by(|left, right| {
        right
            .calls
            .cmp(&left.calls)
            .then_with(|| left.tool.cmp(&right.tool))
    });
    UsageLedger {
        journals: 0,
        totals,
        providers: providers
            .into_iter()
            .map(|((provider_id, model), totals)| UsageGroup {
                provider_id,
                model,
                totals,
            })
            .collect(),
        tasks,
        tools,
        runs: runs
            .into_iter()
            .map(|(run_id, totals)| RunUsage { run_id, totals })
            .collect(),
    }
}

fn task_label(objective: &str) -> String {
    let first_line = objective.lines().next().unwrap_or("").trim();
    let mut label: String = first_line.chars().take(80).collect();
    if first_line.chars().count() > 80 {
        label.push('…');
    }
    if label.is_empty() {
        "(empty task)".into()
    } else {
        label
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LatencyGroup {
    pub provider_id: String,
    pub model: String,
    pub samples: u64,
    pub p50_ms: u64,
    pub p95_ms: u64,
}

/// Pair each model response with the preceding request in the same run and
/// report p50/p95 wall-clock latency per provider/model.
pub fn summarize_latency(events: &[JournalEvent]) -> Vec<LatencyGroup> {
    let mut pending = BTreeMap::<Uuid, VecDeque<u128>>::new();
    let mut groups = BTreeMap::<(String, String), Vec<u64>>::new();
    for event in events {
        match &event.event {
            JournalEventKind::ModelRequested { .. } => {
                pending
                    .entry(event.run_id)
                    .or_default()
                    .push_back(event.timestamp_unix_ms);
            }
            JournalEventKind::ModelResponded { response } => {
                let Some(started) = pending.get_mut(&event.run_id).and_then(VecDeque::pop_front)
                else {
                    continue;
                };
                let latency = event
                    .timestamp_unix_ms
                    .saturating_sub(started)
                    .min(u128::from(u64::MAX)) as u64;
                groups
                    .entry((response.provider_id.clone(), response.model.clone()))
                    .or_default()
                    .push(latency);
            }
            _ => {}
        }
    }
    groups
        .into_iter()
        .map(|((provider_id, model), samples)| LatencyGroup {
            provider_id,
            model,
            samples: samples.len() as u64,
            p50_ms: crate::metrics::percentile(&samples, 50).unwrap_or(0),
            p95_ms: crate::metrics::percentile(&samples, 95).unwrap_or(0),
        })
        .collect()
}

pub fn render_usage_ledger(ledger: &UsageLedger) -> String {
    let mut output = format!(
        "Token 与成本汇总\n\
         运行记录      {} 次\n\
         模型调用      {} 次\n\
         输入 tokens   {}\n\
         输出 tokens   {}\n\
         总 tokens     {}\n\
         估算成本      {}（provider cost_micros）\n\
         零成本调用    {} 次",
        ledger.journals,
        ledger.totals.calls,
        ledger.totals.input_tokens,
        ledger.totals.output_tokens,
        ledger.totals.total_tokens(),
        ledger.totals.cost_micros,
        ledger.totals.zero_cost_calls
    );
    if !ledger.providers.is_empty() {
        output.push_str("\n\n按模型服务分组");
        for group in &ledger.providers {
            output.push_str(&format!(
                "\n- {}/{}\n\
                 调用次数      {}\n\
                 输入 tokens   {}\n\
                 输出 tokens   {}\n\
                 估算成本      {}",
                group.provider_id,
                group.model,
                group.totals.calls,
                group.totals.input_tokens,
                group.totals.output_tokens,
                group.totals.cost_micros
            ));
        }
    }
    if !ledger.tasks.is_empty() {
        output.push_str("\n\n按任务分组（成本降序，最多 10 条）");
        for task in ledger.tasks.iter().take(10) {
            output.push_str(&format!(
                "\n- {}\n\
                 运行次数      {}\n\
                 模型调用      {} 次\n\
                 tokens        {}\n\
                 估算成本      {}",
                task_label(&task.objective),
                task.runs,
                task.totals.calls,
                task.totals.total_tokens(),
                task.totals.cost_micros
            ));
        }
    }
    if !ledger.tools.is_empty() {
        output.push_str("\n\n按工具分组（调用次数降序）");
        for tool in ledger.tools.iter().take(10) {
            output.push_str(&format!(
                "\n- {}\n\
                 调用次数      {}\n\
                 失败次数      {}",
                tool.tool, tool.calls, tool.failures
            ));
        }
    }
    if !ledger.runs.is_empty() {
        output.push_str("\n\n按运行分组");
        for run in ledger.runs.iter().rev().take(20).rev() {
            output.push_str(&format!(
                "\n- {}\n\
                 模型调用      {} 次\n\
                 tokens        {}\n\
                 估算成本      {}",
                run.run_id,
                run.totals.calls,
                run.totals.total_tokens(),
                run.totals.cost_micros
            ));
        }
    }
    if ledger.totals.calls == 0 {
        output.push_str("\n\n当前会话尚未记录模型 token 用量。");
    } else {
        output.push_str(
            "\n\n说明：成本按已配置的模型价格元数据估算，不代表最终账单；1A 之外的 Agent 不计入本汇总。",
        );
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        ModelResponse, ModelTurn, PROTOCOL_VERSION, TaskRequest, ToolObservation, Usage,
    };

    fn response(provider: &str, input: u64, output: u64, cost: u64) -> ModelResponse {
        ModelResponse {
            provider_id: provider.into(),
            model: "model".into(),
            turn: ModelTurn::ToolCalls { calls: Vec::new() },
            usage: Usage {
                input_tokens: input,
                output_tokens: output,
                cost_micros: cost,
                cache_hit_tokens: 0,
                cache_miss_tokens: 0,
            },
        }
    }

    #[test]
    fn usage_ledger_aggregates_every_model_response() {
        let run_id = Uuid::new_v4();
        let events = vec![
            JournalEvent {
                schema_version: PROTOCOL_VERSION,
                run_id,
                seq: 0,
                timestamp_unix_ms: 1,
                event: JournalEventKind::RunStarted {
                    request: TaskRequest::from_objective("compare options", PathBuf::from(".")),
                    mode: crate::domain::CognitiveMode::DirectAnswer,
                },
            },
            JournalEvent {
                schema_version: PROTOCOL_VERSION,
                run_id,
                seq: 1,
                timestamp_unix_ms: 2,
                event: JournalEventKind::ModelResponded {
                    response: response("p1", 1_000, 200, 30),
                },
            },
            JournalEvent {
                schema_version: PROTOCOL_VERSION,
                run_id,
                seq: 2,
                timestamp_unix_ms: 3,
                event: JournalEventKind::ModelResponded {
                    response: response("p1", 500, 100, 12),
                },
            },
            JournalEvent {
                schema_version: PROTOCOL_VERSION,
                run_id,
                seq: 3,
                timestamp_unix_ms: 4,
                event: JournalEventKind::ToolObserved {
                    observation: ToolObservation {
                        call_id: "call-1".into(),
                        tool: "read_file".into(),
                        status: ToolStatus::Ok,
                        summary: "ok".into(),
                        content: String::new(),
                        error_code: None,
                        metadata: BTreeMap::new(),
                    },
                },
            },
            JournalEvent {
                schema_version: PROTOCOL_VERSION,
                run_id,
                seq: 4,
                timestamp_unix_ms: 5,
                event: JournalEventKind::ToolObserved {
                    observation: ToolObservation {
                        call_id: "call-2".into(),
                        tool: "read_file".into(),
                        status: ToolStatus::Failed,
                        summary: "failed".into(),
                        content: String::new(),
                        error_code: Some("tool_failed".into()),
                        metadata: BTreeMap::new(),
                    },
                },
            },
        ];
        let ledger = summarize_usage(&events);
        assert_eq!(ledger.totals.calls, 2);
        assert_eq!(ledger.totals.input_tokens, 1_500);
        assert_eq!(ledger.totals.output_tokens, 300);
        assert_eq!(ledger.totals.cost_micros, 42);
        assert_eq!(ledger.providers[0].totals.calls, 2);
        assert_eq!(ledger.tasks[0].objective, "compare options");
        assert_eq!(ledger.tasks[0].runs, 1);
        assert_eq!(ledger.tasks[0].totals.cost_micros, 42);
        assert_eq!(ledger.runs[0].totals.total_tokens(), 1_800);
        assert_eq!(ledger.tools[0].tool, "read_file");
        assert_eq!(ledger.tools[0].calls, 2);
        assert_eq!(ledger.tools[0].failures, 1);
        let rendered = render_usage_ledger(&ledger);
        assert!(rendered.contains("模型调用      2 次"));
        assert!(rendered.contains("总 tokens     1800"));
        assert!(rendered.contains("按任务分组"));
        assert!(rendered.contains("compare options"));
        assert!(rendered.contains("按工具分组"));
        assert!(rendered.contains("read_file"));
    }
}
