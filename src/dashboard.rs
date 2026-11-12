//! Static HTML operations dashboard derived from journals, schedules and
//! alerts. It is a report, not a live server: every value is escaped and the
//! page is self-contained.

use crate::alerts::AlertRecord;
use crate::metrics::JournalMetricsReport;
use crate::scheduler::JobHealth;
use crate::usage_ledger::{LatencyGroup, UsageLedger};

pub struct DashboardInputs<'a> {
    pub usage: &'a UsageLedger,
    pub latency: &'a [LatencyGroup],
    pub health: &'a [JobHealth],
    pub alerts: &'a [AlertRecord],
    /// Local gate metrics; `None` when the journal directory is absent.
    pub gate: Option<&'a JournalMetricsReport>,
    pub generated_ms: u128,
}

pub fn render(inputs: &DashboardInputs<'_>) -> String {
    let mut html = String::with_capacity(16 * 1024);
    html.push_str(
        "<!doctype html>\n<html lang=\"zh-CN\">\n<head>\n<meta charset=\"utf-8\">\n\
         <meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; style-src 'unsafe-inline'\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <title>1Agent 运行看板</title>\n<style>\n",
    );
    html.push_str(
        ":root{color-scheme:light dark}body{font:14px/1.5 system-ui,sans-serif;margin:2rem;max-width:1100px}\
         h1{font-size:1.4rem;margin:0 0 .3rem}h2{font-size:1rem;margin:2rem 0 .5rem;border-bottom:1px solid #8884;padding-bottom:.3rem}\
         .muted{opacity:.65}.grid{display:grid;grid-template-columns:repeat(auto-fit,minmax(150px,1fr));gap:.7rem}\
         .card{border:1px solid #8884;border-radius:.5rem;padding:.6rem .8rem}.card b{display:block;font-size:1.25rem}\
         table{border-collapse:collapse;width:100%}th,td{text-align:left;padding:.35rem .5rem;border-bottom:1px solid #8883;vertical-align:top}\
         code{font-family:ui-monospace,monospace}.pill{display:inline-block;padding:0 .45rem;border-radius:999px;border:1px solid #8886}\
         .ok{color:#16794b}.overdue{color:#a05a00}.failing{color:#b42318}\
         </style>\n</head>\n<body>\n",
    );
    html.push_str("<h1>1Agent 运行看板</h1>\n");
    html.push_str(&format!(
        "<p class=\"muted\">生成时间 {} ms · journal {} 个 · 模型调用 {} 次</p>\n",
        inputs.generated_ms, inputs.usage.journals, inputs.usage.totals.calls
    ));

    html.push_str("<div class=\"grid\">\n");
    card(
        &mut html,
        "总 tokens",
        &inputs.usage.totals.total_tokens().to_string(),
    );
    card(
        &mut html,
        "估算成本（micros）",
        &inputs.usage.totals.cost_micros.to_string(),
    );
    card(
        &mut html,
        "工具调用",
        &inputs
            .usage
            .tools
            .iter()
            .map(|tool| tool.calls)
            .sum::<u64>()
            .to_string(),
    );
    card(&mut html, "计划任务", &inputs.health.len().to_string());
    card(&mut html, "告警", &inputs.alerts.len().to_string());
    html.push_str("</div>\n");

    if let Some(gate) = inputs.gate {
        html.push_str("<h2>门禁指标</h2>\n<div class=\"grid\">\n");
        card(&mut html, "runs", &gate.runs.to_string());
        card(&mut html, "completed", &gate.completed.to_string());
        card(&mut html, "crash-free", &bps_label(gate.crash_free_bps));
        card(&mut html, "recovery", &bps_label(gate.recovery_bps));
        card(&mut html, "replay", &bps_label(gate.replay_bps));
        card(&mut html, "tool success", &bps_label(gate.tool_success_bps));
        card(
            &mut html,
            "audit complete",
            &bps_label(gate.audit_complete_bps),
        );
        card(&mut html, "denied", &gate.denied_observations.to_string());
        if let Some(ttft) = gate.ttft_latency {
            card(&mut html, "TTFT p95 ms", &ttft.p95_ms.to_string());
        }
        html.push_str("</div>\n");
    }

    html.push_str("<h2>模型服务</h2>\n");
    if inputs.usage.providers.is_empty() {
        empty(&mut html);
    } else {
        html.push_str(
            "<table><thead><tr><th>provider</th><th>model</th><th>调用</th><th>输入 tokens</th><th>输出 tokens</th><th>成本 micros</th></tr></thead><tbody>\n",
        );
        for group in &inputs.usage.providers {
            html.push_str(&format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>\n",
                escape(&group.provider_id),
                escape(&group.model),
                group.totals.calls,
                group.totals.input_tokens,
                group.totals.output_tokens,
                group.totals.cost_micros
            ));
        }
        html.push_str("</tbody></table>\n");
    }

    html.push_str("<h2>延迟</h2>\n");
    if inputs.latency.is_empty() {
        empty(&mut html);
    } else {
        html.push_str(
            "<table><thead><tr><th>provider</th><th>model</th><th>samples</th><th>p50 ms</th><th>p95 ms</th></tr></thead><tbody>\n",
        );
        for group in inputs.latency {
            html.push_str(&format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>\n",
                escape(&group.provider_id),
                escape(&group.model),
                group.samples,
                group.p50_ms,
                group.p95_ms
            ));
        }
        html.push_str("</tbody></table>\n");
    }

    html.push_str("<h2>按任务</h2>\n");
    if inputs.usage.tasks.is_empty() {
        empty(&mut html);
    } else {
        html.push_str(
            "<table><thead><tr><th>任务</th><th>运行</th><th>调用</th><th>tokens</th><th>成本 micros</th></tr></thead><tbody>\n",
        );
        for task in &inputs.usage.tasks {
            html.push_str(&format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>\n",
                escape(&task.objective),
                task.runs,
                task.totals.calls,
                task.totals.total_tokens(),
                task.totals.cost_micros
            ));
        }
        html.push_str("</tbody></table>\n");
    }

    html.push_str("<h2>按工具</h2>\n");
    if inputs.usage.tools.is_empty() {
        empty(&mut html);
    } else {
        html.push_str(
            "<table><thead><tr><th>工具</th><th>调用</th><th>失败</th></tr></thead><tbody>\n",
        );
        for tool in &inputs.usage.tools {
            html.push_str(&format!(
                "<tr><td><code>{}</code></td><td>{}</td><td>{}</td></tr>\n",
                escape(&tool.tool),
                tool.calls,
                tool.failures
            ));
        }
        html.push_str("</tbody></table>\n");
    }

    html.push_str("<h2>计划健康</h2>\n");
    if inputs.health.is_empty() {
        empty(&mut html);
    } else {
        html.push_str(
            "<table><thead><tr><th>状态</th><th>任务</th><th>last</th><th>next run ms</th><th>overdue ms</th></tr></thead><tbody>\n",
        );
        for job in inputs.health {
            html.push_str(&format!(
                "<tr><td><span class=\"pill {}\">{}</span></td><td>{} <span class=\"muted\">{}</span></td><td>{}</td><td>{}</td><td>{}</td></tr>\n",
                escape(&job.state),
                escape(&job.state),
                escape(&job.name),
                escape(&job.id),
                escape(job.last_status.as_deref().unwrap_or("-")),
                job.next_run_ms,
                job.overdue_ms
            ));
        }
        html.push_str("</tbody></table>\n");
    }

    html.push_str("<h2>最近告警</h2>\n");
    if inputs.alerts.is_empty() {
        empty(&mut html);
    } else {
        html.push_str(
            "<table><thead><tr><th>时间 ms</th><th>类型</th><th>provider/model</th><th>消息</th></tr></thead><tbody>\n",
        );
        for alert in inputs.alerts.iter().rev().take(20) {
            let source = match (&alert.provider_id, &alert.model) {
                (Some(provider), Some(model)) => format!("{provider}/{model}"),
                (Some(provider), None) => provider.clone(),
                _ => "-".into(),
            };
            html.push_str(&format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>\n",
                alert.at_ms,
                escape(&alert.kind),
                escape(&source),
                escape(&alert.message)
            ));
        }
        html.push_str("</tbody></table>\n");
    }

    html.push_str("</body>\n</html>\n");
    html
}

fn card(html: &mut String, label: &str, value: &str) {
    html.push_str(&format!(
        "<div class=\"card\"><span class=\"muted\">{}</span><b>{}</b></div>\n",
        escape(label),
        escape(value)
    ));
}

fn bps_label(value: Option<u16>) -> String {
    value
        .map(|value| format!("{:.2}%", f64::from(value) / 100.0))
        .unwrap_or_else(|| "-".into())
}

fn empty(html: &mut String) {
    html.push_str("<p class=\"muted\">暂无数据</p>\n");
}

fn escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            character => out.push(character),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scheduler::JobHealth;
    use crate::usage_ledger::{
        TaskUsageGroup, ToolUsageGroup, UsageGroup, UsageLedger, UsageTotals,
    };

    fn totals(calls: u64, cost: u64) -> UsageTotals {
        UsageTotals {
            calls,
            input_tokens: 10,
            output_tokens: 5,
            cost_micros: cost,
            zero_cost_calls: 0,
        }
    }

    #[test]
    fn renders_sections_and_escapes_dynamic_text() {
        let usage = UsageLedger {
            journals: 2,
            totals: totals(3, 42),
            providers: vec![UsageGroup {
                provider_id: "p<1>".into(),
                model: "m&2".into(),
                totals: totals(3, 42),
            }],
            tasks: vec![TaskUsageGroup {
                objective: "<script>alert(1)</script>".into(),
                runs: 1,
                totals: totals(3, 42),
            }],
            tools: vec![ToolUsageGroup {
                tool: "read_file".into(),
                calls: 2,
                failures: 1,
            }],
            runs: vec![],
        };
        let health = vec![JobHealth {
            id: "schedule-1".into(),
            name: "daily".into(),
            state: "failing".into(),
            last_status: Some("failed".into()),
            next_run_ms: 10,
            overdue_ms: 0,
        }];
        let alerts = vec![AlertRecord {
            alert_id: "a1".into(),
            at_ms: 5,
            kind: "test".into(),
            provider_id: None,
            model: None,
            observed: 0,
            threshold: 0,
            message: "<b>slow</b>".into(),
        }];
        let html = render(&DashboardInputs {
            usage: &usage,
            latency: &[],
            health: &health,
            alerts: &alerts,
            gate: None,
            generated_ms: 99,
        });
        assert!(html.contains("1Agent 运行看板"));
        assert!(html.contains("按任务"));
        assert!(html.contains("按工具"));
        assert!(html.contains("计划健康"));
        assert!(html.contains("failing"));
        assert!(html.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
        assert!(!html.contains("<script>"));
        assert!(html.contains("p&lt;1&gt;"));
        assert!(html.contains("m&amp;2"));
        assert!(html.contains("&lt;b&gt;slow&lt;/b&gt;"));
        assert!(html.contains("Content-Security-Policy"));
    }

    #[test]
    fn empty_inputs_still_render() {
        let usage = UsageLedger::default();
        let html = render(&DashboardInputs {
            usage: &usage,
            latency: &[],
            health: &[],
            alerts: &[],
            gate: None,
            generated_ms: 1,
        });
        assert!(html.contains("暂无数据"));
        assert!(html.trim_end().ends_with("</html>"));
    }

    #[test]
    fn gate_metrics_render_when_present() {
        let usage = UsageLedger::default();
        let gate = JournalMetricsReport {
            journals: 2,
            runs: 2,
            completed: 1,
            terminal_stops: 1,
            interrupted: 1,
            crash_free_bps: Some(5_000),
            resumes: 1,
            resumed_completed: 1,
            recovery_bps: Some(10_000),
            replay_total: 1,
            replay_ok: 1,
            replay_bps: Some(10_000),
            compacted_runs: 0,
            compacted_replay_ok: 0,
            compacted_replay_bps: None,
            tool_calls: 2,
            tool_ok: 1,
            tool_success_bps: Some(5_000),
            denied_observations: 1,
            audit_total: 1,
            audit_complete: 1,
            audit_complete_bps: Some(10_000),
            context_compactions: 0,
            input_tokens: 10,
            output_tokens: 5,
            cost_micros: 0,
            run_latency: None,
            ttft_latency: Some(crate::metrics::LatencyDigest {
                count: 1,
                min_ms: 12,
                p50_ms: 12,
                p95_ms: 12,
                max_ms: 12,
            }),
            route_latency: None,
            context_latency: None,
            model_latency: None,
            tool_latency: None,
        };
        let html = render(&DashboardInputs {
            usage: &usage,
            latency: &[],
            health: &[],
            alerts: &[],
            gate: Some(&gate),
            generated_ms: 1,
        });
        assert!(html.contains("门禁指标"));
        assert!(html.contains("50.00%"));
        assert!(html.contains("TTFT p95 ms"));
    }
}
