use chrono::{DateTime, Datelike, TimeZone, Timelike, Utc};
use scouter_types::{
    AgentBucketRow, AgentDashboardResponse, AgentDashboardSummary, AgentMetricBucket,
    GenAiAgentActivity, GenAiAgentActivityResponse, GenAiErrorBreakdownResponse, GenAiErrorCount,
    GenAiModelUsage, GenAiModelUsageResponse, GenAiOperationBreakdown,
    GenAiOperationBreakdownResponse, GenAiSpanRecord, GenAiTokenBucket, GenAiTokenMetricsResponse,
    GenAiToolActivity, GenAiTraceMetricsResponse, ModelCostBreakdown, ModelPricing,
    ToolDashboardResponse, ToolTimeBucket,
};
use std::cmp::Reverse;
use std::collections::{HashMap, HashSet};

const VALID_BUCKET_INTERVALS: &[&str] =
    &["second", "minute", "hour", "day", "week", "month", "year"];

type OperationGroupKey = (String, Option<String>);
type OperationGroupTotals = (i64, i64, i64, i64, i64);
type AgentActivityKey = (Option<String>, Option<String>, Option<String>);
type AgentActivityTotals = (i64, i64, i64, Option<DateTime<Utc>>);
type ToolGroupKey = (Option<String>, Option<String>);
type ToolTimeGroupKey = (DateTime<Utc>, Option<String>, Option<String>);

pub fn validate_bucket_interval(bucket_interval: &str) -> Result<(), String> {
    if VALID_BUCKET_INTERVALS.contains(&bucket_interval) {
        Ok(())
    } else {
        Err(format!(
            "Invalid bucket_interval '{}'. Must be one of: {}",
            bucket_interval,
            VALID_BUCKET_INTERVALS.join(", ")
        ))
    }
}

fn truncate_to_bucket(
    timestamp: DateTime<Utc>,
    bucket_interval: &str,
) -> Result<DateTime<Utc>, String> {
    let date = timestamp.date_naive();
    let bucket = match bucket_interval {
        "second" => date.and_hms_opt(timestamp.hour(), timestamp.minute(), timestamp.second()),
        "minute" => date.and_hms_opt(timestamp.hour(), timestamp.minute(), 0),
        "hour" => date.and_hms_opt(timestamp.hour(), 0, 0),
        "day" => date.and_hms_opt(0, 0, 0),
        "week" => {
            let week_start =
                date - chrono::Duration::days(timestamp.weekday().num_days_from_monday() as i64);
            week_start.and_hms_opt(0, 0, 0)
        }
        "month" => chrono::NaiveDate::from_ymd_opt(timestamp.year(), timestamp.month(), 1)
            .and_then(|d| d.and_hms_opt(0, 0, 0)),
        "year" => chrono::NaiveDate::from_ymd_opt(timestamp.year(), 1, 1)
            .and_then(|d| d.and_hms_opt(0, 0, 0)),
        _ => return Err(format!("Invalid bucket_interval '{bucket_interval}'")),
    }
    .ok_or_else(|| format!("Failed to truncate timestamp for bucket '{bucket_interval}'"))?;

    Ok(Utc.from_utc_datetime(&bucket))
}

fn percentile(values: &[i64], quantile: f64) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    let idx = ((sorted.len() - 1) as f64 * quantile).round() as usize;
    Some(sorted[idx] as f64)
}

fn model_for_span(span: &GenAiSpanRecord) -> String {
    span.response_model
        .clone()
        .or_else(|| span.request_model.clone())
        .unwrap_or_else(|| "unknown".to_string())
}

fn is_agent_span(span: &GenAiSpanRecord) -> bool {
    matches!(
        span.operation_name.as_deref(),
        Some("invoke_agent" | "create_agent")
    ) || span.agent_name.is_some()
}

fn is_tool_span(span: &GenAiSpanRecord) -> bool {
    span.tool_name.is_some() || matches!(span.operation_name.as_deref(), Some("execute_tool"))
}

fn build_token_metrics(
    spans: &[GenAiSpanRecord],
    bucket_interval: &str,
) -> Result<GenAiTokenMetricsResponse, String> {
    let mut buckets: HashMap<DateTime<Utc>, GenAiTokenBucket> = HashMap::new();
    for span in spans {
        let bucket_start = truncate_to_bucket(span.start_time, bucket_interval)?;
        let bucket = buckets
            .entry(bucket_start)
            .or_insert_with(|| GenAiTokenBucket {
                bucket_start,
                ..Default::default()
            });
        bucket.total_input_tokens += span.input_tokens.unwrap_or_default();
        bucket.total_output_tokens += span.output_tokens.unwrap_or_default();
        bucket.total_cache_creation_tokens += span.cache_creation_input_tokens.unwrap_or_default();
        bucket.total_cache_read_tokens += span.cache_read_input_tokens.unwrap_or_default();
        bucket.span_count += 1;
        if span.error_type.is_some() {
            bucket.error_rate += 1.0;
        }
    }

    let mut buckets: Vec<_> = buckets
        .into_values()
        .map(|mut bucket| {
            if bucket.span_count > 0 {
                bucket.error_rate /= bucket.span_count as f64;
            }
            bucket
        })
        .collect();
    buckets.sort_by_key(|bucket| bucket.bucket_start);
    Ok(GenAiTokenMetricsResponse { buckets })
}

fn build_operation_breakdown(spans: &[GenAiSpanRecord]) -> GenAiOperationBreakdownResponse {
    let mut groups: HashMap<OperationGroupKey, OperationGroupTotals> = HashMap::new();
    for span in spans {
        let key = (
            span.operation_name.clone().unwrap_or_default(),
            span.provider_name.clone(),
        );
        let entry = groups.entry(key).or_default();
        entry.0 += 1;
        entry.1 += span.duration_ms;
        entry.2 += span.input_tokens.unwrap_or_default();
        entry.3 += span.output_tokens.unwrap_or_default();
        if span.error_type.is_some() {
            entry.4 += 1;
        }
    }

    let mut operations: Vec<_> = groups
        .into_iter()
        .map(
            |((operation_name, provider_name), (count, duration, input, output, errors))| {
                GenAiOperationBreakdown {
                    operation_name,
                    provider_name,
                    span_count: count,
                    avg_duration_ms: if count > 0 {
                        duration as f64 / count as f64
                    } else {
                        0.0
                    },
                    total_input_tokens: input,
                    total_output_tokens: output,
                    error_rate: if count > 0 {
                        errors as f64 / count as f64
                    } else {
                        0.0
                    },
                }
            },
        )
        .collect();
    operations.sort_by_key(|operation| Reverse(operation.span_count));
    GenAiOperationBreakdownResponse { operations }
}

fn build_model_usage(spans: &[GenAiSpanRecord]) -> GenAiModelUsageResponse {
    let mut groups: HashMap<(String, Option<String>), Vec<&GenAiSpanRecord>> = HashMap::new();
    for span in spans {
        groups
            .entry((model_for_span(span), span.provider_name.clone()))
            .or_default()
            .push(span);
    }

    let mut models: Vec<_> = groups
        .into_iter()
        .map(|((model, provider_name), rows)| {
            let durations: Vec<i64> = rows.iter().map(|span| span.duration_ms).collect();
            let span_count = rows.len() as i64;
            let errors = rows.iter().filter(|span| span.error_type.is_some()).count() as i64;
            GenAiModelUsage {
                model,
                provider_name,
                span_count,
                total_input_tokens: rows
                    .iter()
                    .map(|span| span.input_tokens.unwrap_or_default())
                    .sum(),
                total_output_tokens: rows
                    .iter()
                    .map(|span| span.output_tokens.unwrap_or_default())
                    .sum(),
                p50_duration_ms: percentile(&durations, 0.5),
                p95_duration_ms: percentile(&durations, 0.95),
                error_rate: if span_count > 0 {
                    errors as f64 / span_count as f64
                } else {
                    0.0
                },
            }
        })
        .collect();
    models.sort_by_key(|model| Reverse(model.span_count));
    GenAiModelUsageResponse { models }
}

fn build_agent_activity(spans: &[GenAiSpanRecord]) -> GenAiAgentActivityResponse {
    let mut groups: HashMap<AgentActivityKey, AgentActivityTotals> = HashMap::new();

    for span in spans.iter().filter(|span| is_agent_span(span)) {
        let key = (
            span.agent_name.clone(),
            span.agent_id.clone(),
            span.conversation_id.clone(),
        );
        let entry = groups.entry(key).or_default();
        entry.0 += 1;
        entry.1 += span.input_tokens.unwrap_or_default();
        entry.2 += span.output_tokens.unwrap_or_default();
        entry.3 = Some(
            entry
                .3
                .map_or(span.start_time, |last| last.max(span.start_time)),
        );
    }

    let agents = groups
        .into_iter()
        .map(
            |((agent_name, agent_id, conversation_id), (span_count, input, output, last_seen))| {
                GenAiAgentActivity {
                    agent_name,
                    agent_id,
                    conversation_id,
                    span_count,
                    total_input_tokens: input,
                    total_output_tokens: output,
                    last_seen,
                }
            },
        )
        .collect();

    GenAiAgentActivityResponse { agents }
}

fn build_agent_dashboard(
    spans: &[GenAiSpanRecord],
    bucket_interval: &str,
    model_pricing: &HashMap<String, ModelPricing>,
) -> Result<AgentDashboardResponse, String> {
    let agent_spans: Vec<_> = spans.iter().filter(|span| is_agent_span(span)).collect();
    let mut groups: HashMap<(DateTime<Utc>, String), Vec<&GenAiSpanRecord>> = HashMap::new();
    let mut unique_agents = HashSet::new();
    let mut unique_conversations = HashSet::new();

    for span in agent_spans {
        if let Some(agent_name) = &span.agent_name {
            unique_agents.insert(agent_name.clone());
        }
        if let Some(conversation_id) = &span.conversation_id {
            unique_conversations.insert(conversation_id.clone());
        }
        groups
            .entry((
                truncate_to_bucket(span.start_time, bucket_interval)?,
                model_for_span(span),
            ))
            .or_default()
            .push(span);
    }

    let rows: Vec<_> = groups
        .into_iter()
        .map(|((bucket_start, model), rows)| {
            let span_count = rows.len() as i64;
            let error_count = rows.iter().filter(|span| span.error_type.is_some()).count() as i64;
            let durations: Vec<i64> = rows.iter().map(|span| span.duration_ms).collect();
            AgentBucketRow {
                bucket_start,
                model: Some(model),
                span_count,
                error_count,
                error_rate: if span_count > 0 {
                    error_count as f64 / span_count as f64
                } else {
                    0.0
                },
                avg_duration_ms: if span_count > 0 {
                    durations.iter().sum::<i64>() as f64 / span_count as f64
                } else {
                    0.0
                },
                p50_duration_ms: percentile(&durations, 0.5),
                p95_duration_ms: percentile(&durations, 0.95),
                p99_duration_ms: percentile(&durations, 0.99),
                input_tokens: rows
                    .iter()
                    .map(|span| span.input_tokens.unwrap_or_default())
                    .sum(),
                output_tokens: rows
                    .iter()
                    .map(|span| span.output_tokens.unwrap_or_default())
                    .sum(),
                cache_creation_tokens: rows
                    .iter()
                    .map(|span| span.cache_creation_input_tokens.unwrap_or_default())
                    .sum(),
                cache_read_tokens: rows
                    .iter()
                    .map(|span| span.cache_read_input_tokens.unwrap_or_default())
                    .sum(),
            }
        })
        .collect();

    let mut response = fold_agent_buckets(&rows, model_pricing);
    response.summary.unique_agent_count = unique_agents.len() as i64;
    response.summary.unique_conversation_count = unique_conversations.len() as i64;
    Ok(response)
}

fn build_tool_dashboard(
    spans: &[GenAiSpanRecord],
    bucket_interval: &str,
) -> Result<ToolDashboardResponse, String> {
    let tool_spans: Vec<_> = spans.iter().filter(|span| is_tool_span(span)).collect();
    let mut aggregate_groups: HashMap<ToolGroupKey, Vec<&GenAiSpanRecord>> = HashMap::new();
    let mut time_groups: HashMap<ToolTimeGroupKey, Vec<&GenAiSpanRecord>> = HashMap::new();

    for span in tool_spans {
        let key = (span.tool_name.clone(), span.tool_type.clone());
        aggregate_groups.entry(key.clone()).or_default().push(span);
        time_groups
            .entry((
                truncate_to_bucket(span.start_time, bucket_interval)?,
                key.0,
                key.1,
            ))
            .or_default()
            .push(span);
    }

    let aggregates = aggregate_groups
        .into_iter()
        .map(|((tool_name, tool_type), rows)| {
            let call_count = rows.len() as i64;
            let error_count = rows.iter().filter(|span| span.error_type.is_some()).count() as i64;
            GenAiToolActivity {
                tool_name,
                tool_type,
                call_count,
                avg_duration_ms: if call_count > 0 {
                    rows.iter().map(|span| span.duration_ms).sum::<i64>() as f64 / call_count as f64
                } else {
                    0.0
                },
                error_rate: if call_count > 0 {
                    error_count as f64 / call_count as f64
                } else {
                    0.0
                },
            }
        })
        .collect();

    let mut time_series: Vec<_> = time_groups
        .into_iter()
        .map(|((bucket_start, tool_name, tool_type), rows)| {
            let call_count = rows.len() as i64;
            let error_count = rows.iter().filter(|span| span.error_type.is_some()).count() as i64;
            ToolTimeBucket {
                bucket_start,
                tool_name,
                tool_type,
                call_count,
                avg_duration_ms: if call_count > 0 {
                    rows.iter().map(|span| span.duration_ms).sum::<i64>() as f64 / call_count as f64
                } else {
                    0.0
                },
                error_rate: if call_count > 0 {
                    error_count as f64 / call_count as f64
                } else {
                    0.0
                },
            }
        })
        .collect();
    time_series.sort_by_key(|bucket| bucket.bucket_start);

    Ok(ToolDashboardResponse {
        aggregates,
        time_series,
    })
}

fn build_error_breakdown(spans: &[GenAiSpanRecord]) -> GenAiErrorBreakdownResponse {
    let mut groups: HashMap<String, i64> = HashMap::new();
    for error_type in spans.iter().filter_map(|span| span.error_type.as_ref()) {
        *groups.entry(error_type.clone()).or_default() += 1;
    }
    let mut errors: Vec<_> = groups
        .into_iter()
        .map(|(error_type, count)| GenAiErrorCount { error_type, count })
        .collect();
    errors.sort_by_key(|error| Reverse(error.count));
    GenAiErrorBreakdownResponse { errors }
}

#[allow(clippy::too_many_arguments)]
pub fn build_trace_metrics_response(
    trace_id: String,
    aggregate_spans: &[GenAiSpanRecord],
    spans: Vec<GenAiSpanRecord>,
    bucket_interval: &str,
    model_pricing: &HashMap<String, ModelPricing>,
    span_limit: usize,
    spans_truncated: bool,
    sensitive_content_redacted: bool,
) -> Result<GenAiTraceMetricsResponse, String> {
    validate_bucket_interval(bucket_interval)?;
    let token_metrics = build_token_metrics(aggregate_spans, bucket_interval)?;
    let agent_dashboard = build_agent_dashboard(aggregate_spans, bucket_interval, model_pricing)?;
    let tool_dashboard = build_tool_dashboard(aggregate_spans, bucket_interval)?;
    let operation_breakdown = build_operation_breakdown(aggregate_spans);
    let model_usage = build_model_usage(aggregate_spans);
    let agent_activity = build_agent_activity(aggregate_spans);
    let error_breakdown = build_error_breakdown(aggregate_spans);

    Ok(GenAiTraceMetricsResponse {
        trace_id,
        has_genai_spans: !aggregate_spans.is_empty(),
        spans,
        span_limit,
        spans_truncated,
        sensitive_content_redacted,
        token_metrics,
        operation_breakdown,
        model_usage,
        agent_activity,
        agent_dashboard,
        tool_dashboard,
        error_breakdown,
    })
}

fn compute_cost(
    input: i64,
    output: i64,
    cache_creation: i64,
    cache_read: i64,
    pricing: &ModelPricing,
) -> f64 {
    (input as f64 / 1_000_000.0) * pricing.input_per_million
        + (output as f64 / 1_000_000.0) * pricing.output_per_million
        + (cache_creation as f64 / 1_000_000.0) * pricing.cache_creation_per_million
        + (cache_read as f64 / 1_000_000.0) * pricing.cache_read_per_million
}

pub fn fold_agent_buckets(
    rows: &[AgentBucketRow],
    model_pricing: &HashMap<String, ModelPricing>,
) -> AgentDashboardResponse {
    let mut bucket_map: HashMap<i64, AgentMetricBucket> = HashMap::new();
    let mut model_tokens: HashMap<String, (i64, i64, i64, i64)> = HashMap::new();
    let has_pricing = !model_pricing.is_empty();

    for row in rows {
        let ts = row.bucket_start.timestamp_micros();
        let bucket = bucket_map.entry(ts).or_insert_with(|| AgentMetricBucket {
            bucket_start: row.bucket_start,
            ..Default::default()
        });

        let prev_count = bucket.span_count;
        let new_count = prev_count + row.span_count;
        if new_count > 0 {
            bucket.avg_duration_ms = (bucket.avg_duration_ms * prev_count as f64
                + row.avg_duration_ms * row.span_count as f64)
                / new_count as f64;
        }
        bucket.span_count = new_count;
        bucket.error_count += row.error_count;
        if bucket.span_count > 0 {
            bucket.error_rate = bucket.error_count as f64 / bucket.span_count as f64;
        }
        bucket.total_input_tokens += row.input_tokens;
        bucket.total_output_tokens += row.output_tokens;
        bucket.total_cache_creation_tokens += row.cache_creation_tokens;
        bucket.total_cache_read_tokens += row.cache_read_tokens;

        if bucket.p50_duration_ms.is_none() {
            bucket.p50_duration_ms = row.p50_duration_ms;
        }
        if bucket.p95_duration_ms.is_none() {
            bucket.p95_duration_ms = row.p95_duration_ms;
        }
        if bucket.p99_duration_ms.is_none() {
            bucket.p99_duration_ms = row.p99_duration_ms;
        }

        if has_pricing {
            let model_key = row.model.as_deref().unwrap_or("unknown");
            if let Some(pricing) = model_pricing.get(model_key) {
                let cost = compute_cost(
                    row.input_tokens,
                    row.output_tokens,
                    row.cache_creation_tokens,
                    row.cache_read_tokens,
                    pricing,
                );
                *bucket.total_cost.get_or_insert(0.0) += cost;
            }
        }

        let model_key = row.model.clone().unwrap_or_else(|| "unknown".to_string());
        let entry = model_tokens.entry(model_key).or_default();
        entry.0 += row.input_tokens;
        entry.1 += row.output_tokens;
        entry.2 += row.cache_creation_tokens;
        entry.3 += row.cache_read_tokens;
    }

    let mut buckets: Vec<AgentMetricBucket> = bucket_map.into_values().collect();
    buckets.sort_by_key(|b| b.bucket_start);

    let total_requests: i64 = rows.iter().map(|r| r.span_count).sum();
    let total_errors: i64 = rows.iter().map(|r| r.error_count).sum();
    let overall_error_rate = if total_requests > 0 {
        total_errors as f64 / total_requests as f64
    } else {
        0.0
    };
    let avg_duration_ms = if total_requests > 0 {
        rows.iter()
            .map(|r| r.avg_duration_ms * r.span_count as f64)
            .sum::<f64>()
            / total_requests as f64
    } else {
        0.0
    };
    let total_input: i64 = rows.iter().map(|r| r.input_tokens).sum();
    let total_output: i64 = rows.iter().map(|r| r.output_tokens).sum();
    let total_cache_creation: i64 = rows.iter().map(|r| r.cache_creation_tokens).sum();
    let total_cache_read: i64 = rows.iter().map(|r| r.cache_read_tokens).sum();
    let p50 = rows.iter().find_map(|r| r.p50_duration_ms);
    let p95 = rows.iter().find_map(|r| r.p95_duration_ms);
    let p99 = rows.iter().find_map(|r| r.p99_duration_ms);

    let cost_by_model: Vec<ModelCostBreakdown> = model_tokens
        .into_iter()
        .map(|(model, (inp, out, cc, cr))| {
            let total_cost = if has_pricing {
                model_pricing
                    .get(&model)
                    .map(|p| compute_cost(inp, out, cc, cr, p))
            } else {
                None
            };
            ModelCostBreakdown {
                model,
                total_input_tokens: inp,
                total_output_tokens: out,
                total_cache_creation_tokens: cc,
                total_cache_read_tokens: cr,
                total_cost,
            }
        })
        .collect();

    let summary = AgentDashboardSummary {
        total_requests,
        avg_duration_ms,
        p50_duration_ms: p50,
        p95_duration_ms: p95,
        p99_duration_ms: p99,
        overall_error_rate,
        total_input_tokens: total_input,
        total_output_tokens: total_output,
        total_cache_creation_tokens: total_cache_creation,
        total_cache_read_tokens: total_cache_read,
        unique_agent_count: 0,
        unique_conversation_count: 0,
        cost_by_model,
    };

    AgentDashboardResponse { summary, buckets }
}
