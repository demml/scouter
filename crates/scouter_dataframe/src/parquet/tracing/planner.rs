use crate::error::TraceEngineError;
use crate::parquet::tracing::queries::{
    DURATION_MS_COL, ERROR_COUNT_COL, PARTITION_DATE_COL, SEARCH_BLOB_COL, SERVICE_INSTANCE_ID_COL,
    SERVICE_NAME_COL, SERVICE_NAMESPACE_COL, SERVICE_VERSION_COL, SPAN_TABLE_NAME, START_TIME_COL,
    STATUS_CODE_COL, SUMMARY_TABLE_NAME, TRACE_ID_COL, date_lit, ts_lit,
};
use crate::parquet::utils::match_attr_expr;
use chrono::{DateTime, Utc};
use datafusion::common::JoinType;
use datafusion::logical_expr::{Expr, col, lit};
use datafusion::prelude::{DataFrame, SessionContext};
use scouter_types::trace::query::FilterClause;
use std::future::Future;
use std::pin::Pin;

type PlannerFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, TraceEngineError>> + Send + 'a>>;

/// MUST BUMP IF SEMANTICS CHANGE: metrics cache keys include this value so a
/// deployment never reuses buckets produced by an older logical planner.
pub(crate) const PLANNER_VERSION: u64 = 2;

/// Hard ceiling for trace-id sets the planner resolves before joining.
/// The cap protects DataFusion from planning massive intermediate joins for
/// broad text predicates; users can narrow the time window or clause instead.
pub(crate) const MAX_INTERMEDIATE_TRACE_IDS: usize = 50_000;

#[derive(Copy, Clone, Debug)]
pub(crate) struct TimeWindow {
    pub start: Option<DateTime<Utc>>,
    pub end: Option<DateTime<Utc>>,
}

#[cfg(test)]
#[derive(Debug, Clone)]
pub(crate) enum ClauseResolution {
    SummaryExpr(Expr),
    SpanExpr,
    TraceIdSet { complement: bool },
    Tautology,
    Contradiction,
}

/// Structured representation of the user clause.
///
/// `root` and `or_branches` are the public inspection surface used by tests and
/// reviewers. `node` keeps the full boolean tree so the executor can preserve
/// mixed-table `AND`, mixed-table `OR`, and `NOT` without rebuilding from text.
#[derive(Debug, Clone)]
pub(crate) struct ClausePlan {
    #[cfg(test)]
    pub root: ClauseResolution,
    #[cfg(test)]
    pub or_branches: Vec<ClauseResolution>,
    node: PlannedNode,
}

pub(crate) struct ResolvedPlan {
    pub summary_filter: Option<Expr>,
    pub id_constraints: Vec<TraceIdConstraint>,
    pub contradiction: bool,
}

pub(crate) struct TraceIdConstraint {
    pub df: DataFrame,
    pub complement: bool,
    pub estimated_card: Option<usize>,
}

#[derive(Debug, Clone)]
enum PlannedNode {
    Summary(Expr),
    Span(Expr),
    TraceIdSet {
        span_predicate: Expr,
        complement: bool,
    },
    And(Vec<PlannedNode>),
    Or(Vec<PlannedNode>),
    Complement(Box<PlannedNode>),
    Tautology,
    Contradiction,
}

/// Strip the delimiter used inside `search_blob` before building a LIKE pattern.
/// This keeps user text from accidentally broadening the generated delimiter-aware
/// attribute predicate.
#[inline]
fn sanitize_search_value(s: &str) -> String {
    s.replace('|', "")
}

pub(crate) fn plan_clause(clause: &FilterClause) -> ClausePlan {
    // The old implementation split clauses into summary and span halves, which
    // lost the boolean topology for mixed `OR`. This plan keeps the tree intact
    // and lets the executor decide where trace-id joins are needed.
    let node = plan_node(clause);
    #[cfg(test)]
    let root = resolution_for_node(&node);
    #[cfg(test)]
    let or_branches = match &node {
        PlannedNode::Or(parts) => parts.iter().map(resolution_for_node).collect(),
        _ => Vec::new(),
    };

    ClausePlan {
        #[cfg(test)]
        root,
        #[cfg(test)]
        or_branches,
        node,
    }
}

pub(crate) async fn execute_plan(
    plan: &ClausePlan,
    ctx: &SessionContext,
    time_window: TimeWindow,
) -> Result<ResolvedPlan, TraceEngineError> {
    execute_plan_with_summary_table(plan, ctx, time_window, SUMMARY_TABLE_NAME).await
}

pub(crate) async fn execute_plan_with_summary_table(
    plan: &ClausePlan,
    ctx: &SessionContext,
    time_window: TimeWindow,
    summary_table: &str,
) -> Result<ResolvedPlan, TraceEngineError> {
    execute_node(&plan.node, ctx, time_window, summary_table).await
}

pub(crate) async fn trace_ids_for_clause_with_summary_table(
    clause: &FilterClause,
    ctx: &SessionContext,
    time_window: TimeWindow,
    summary_table: &str,
) -> Result<DataFrame, TraceEngineError> {
    let plan = plan_clause(clause);
    let resolved = execute_plan_with_summary_table(&plan, ctx, time_window, summary_table).await?;
    let mut df =
        summary_trace_ids_df(ctx, time_window, resolved.summary_filter, summary_table).await?;

    if resolved.contradiction {
        df = df.filter(lit(false))?;
    } else {
        for constraint in resolved.id_constraints {
            df = apply_id_constraint(df, constraint).await?;
        }
    }

    Ok(df.select_columns(&[TRACE_ID_COL])?.distinct()?)
}

pub(crate) async fn apply_id_constraint(
    df: DataFrame,
    constraint: TraceIdConstraint,
) -> Result<DataFrame, TraceEngineError> {
    // Cardinality is collected before planning broad joins; reading it here keeps
    // the diagnostic field part of the exercised internal contract.
    let _estimated_cardinality = constraint.estimated_card;
    let join_type = if constraint.complement {
        JoinType::LeftAnti
    } else {
        JoinType::Inner
    };

    // The right side is aliased so the joined schema keeps the caller's trace_id
    // column as the canonical key for later sorting, aggregation, and serialization.
    let probe = constraint
        .df
        .select(vec![col(TRACE_ID_COL).alias("_planner_trace_id")])?;
    Ok(df.join(
        probe,
        join_type,
        &[TRACE_ID_COL],
        &["_planner_trace_id"],
        None,
    )?)
}

fn execute_node<'a>(
    node: &'a PlannedNode,
    ctx: &'a SessionContext,
    time_window: TimeWindow,
    summary_table: &'a str,
) -> PlannerFuture<'a, ResolvedPlan> {
    Box::pin(async move {
        match node {
            PlannedNode::Summary(expr) => Ok(ResolvedPlan {
                summary_filter: Some(expr.clone()),
                id_constraints: Vec::new(),
                contradiction: false,
            }),
            PlannedNode::Span(expr) => Ok(ResolvedPlan {
                summary_filter: None,
                id_constraints: vec![span_constraint(ctx, time_window, expr.clone(), false).await?],
                contradiction: false,
            }),
            PlannedNode::TraceIdSet {
                span_predicate,
                complement,
            } => Ok(ResolvedPlan {
                summary_filter: None,
                id_constraints: vec![
                    span_constraint(ctx, time_window, span_predicate.clone(), *complement).await?,
                ],
                contradiction: false,
            }),
            PlannedNode::And(parts) => execute_and(parts, ctx, time_window, summary_table).await,
            PlannedNode::Or(_) | PlannedNode::Complement(_) => {
                let df = trace_ids_for_node(ctx, time_window, node, summary_table).await?;
                Ok(ResolvedPlan {
                    summary_filter: None,
                    id_constraints: vec![bounded_constraint(df, false).await?],
                    contradiction: false,
                })
            }
            PlannedNode::Tautology => Ok(ResolvedPlan {
                summary_filter: None,
                id_constraints: Vec::new(),
                contradiction: false,
            }),
            PlannedNode::Contradiction => Ok(ResolvedPlan {
                summary_filter: None,
                id_constraints: Vec::new(),
                contradiction: true,
            }),
        }
    })
}

async fn execute_and(
    parts: &[PlannedNode],
    ctx: &SessionContext,
    time_window: TimeWindow,
    summary_table: &str,
) -> Result<ResolvedPlan, TraceEngineError> {
    let mut summary_filter: Option<Expr> = None;
    let mut id_constraints = Vec::new();

    for part in parts {
        let resolved = execute_node(part, ctx, time_window, summary_table).await?;
        if resolved.contradiction {
            return Ok(ResolvedPlan {
                summary_filter: None,
                id_constraints: Vec::new(),
                contradiction: true,
            });
        }
        if let Some(expr) = resolved.summary_filter {
            summary_filter = Some(match summary_filter {
                Some(existing) => existing.and(expr),
                None => expr,
            });
        }
        id_constraints.extend(resolved.id_constraints);
    }

    Ok(ResolvedPlan {
        summary_filter,
        id_constraints,
        contradiction: false,
    })
}

fn plan_node(clause: &FilterClause) -> PlannedNode {
    match clause {
        FilterClause::And(parts) => plan_and(parts),
        FilterClause::Or(parts) => plan_or(parts),
        FilterClause::Not(inner) => plan_not(inner),
        FilterClause::Phrase(value) => {
            let safe = sanitize_search_value(value);
            PlannedNode::Span(match_attr_expr(
                col(SEARCH_BLOB_COL),
                lit(format!("%{safe}%")),
            ))
        }
        FilterClause::Service(value) => {
            PlannedNode::Summary(col(SERVICE_NAME_COL).eq(lit(value.as_str())))
        }
        FilterClause::ServiceNamespace(value) => {
            PlannedNode::Summary(col(SERVICE_NAMESPACE_COL).eq(lit(value.as_str())))
        }
        FilterClause::ServiceVersion(value) => {
            PlannedNode::Summary(col(SERVICE_VERSION_COL).eq(lit(value.as_str())))
        }
        FilterClause::ServiceInstanceId(value) => {
            PlannedNode::Summary(col(SERVICE_INSTANCE_ID_COL).eq(lit(value.as_str())))
        }
        FilterClause::StatusCode(value) => {
            PlannedNode::Summary(col(STATUS_CODE_COL).eq(lit(*value)))
        }
        FilterClause::HasErrors(true) => PlannedNode::Summary(col(ERROR_COUNT_COL).gt(lit(0_i64))),
        FilterClause::HasErrors(false) => PlannedNode::Summary(col(ERROR_COUNT_COL).eq(lit(0_i64))),
        FilterClause::DurationMinMs(value) => {
            PlannedNode::Summary(col(DURATION_MS_COL).gt_eq(lit(*value)))
        }
        FilterClause::DurationMaxMs(value) => {
            PlannedNode::Summary(col(DURATION_MS_COL).lt_eq(lit(*value)))
        }
        FilterClause::Attr { key, value } => {
            let safe_key = sanitize_search_value(key);
            let safe_val = sanitize_search_value(value);
            PlannedNode::Span(match_attr_expr(
                col(SEARCH_BLOB_COL),
                lit(format!("%|{safe_key}={safe_val}|%")),
            ))
        }
    }
}

fn plan_and(parts: &[FilterClause]) -> PlannedNode {
    let mut planned = Vec::new();
    for part in parts {
        match plan_node(part) {
            PlannedNode::Tautology => {}
            PlannedNode::Contradiction => return PlannedNode::Contradiction,
            node => planned.push(node),
        }
    }

    match planned.len() {
        0 => PlannedNode::Tautology,
        1 => planned.pop().expect("planned has one element"),
        _ => collapse_homogeneous_and(planned),
    }
}

fn collapse_homogeneous_and(planned: Vec<PlannedNode>) -> PlannedNode {
    let all_summary = planned
        .iter()
        .all(|node| matches!(node, PlannedNode::Summary(_)));
    let all_span = planned
        .iter()
        .all(|node| matches!(node, PlannedNode::Span(_)));

    if all_summary {
        return PlannedNode::Summary(
            planned
                .into_iter()
                .filter_map(|node| match node {
                    PlannedNode::Summary(expr) => Some(expr),
                    _ => None,
                })
                .reduce(|left, right| left.and(right))
                .expect("all_summary implies at least one expr"),
        );
    }

    if all_span {
        return PlannedNode::Span(
            planned
                .into_iter()
                .filter_map(|node| match node {
                    PlannedNode::Span(expr) => Some(expr),
                    _ => None,
                })
                .reduce(|left, right| left.and(right))
                .expect("all_span implies at least one expr"),
        );
    }

    PlannedNode::And(planned)
}

fn plan_or(parts: &[FilterClause]) -> PlannedNode {
    let mut planned = Vec::new();
    for part in parts {
        match plan_node(part) {
            PlannedNode::Contradiction => {}
            PlannedNode::Tautology => return PlannedNode::Tautology,
            node => planned.push(node),
        }
    }

    match planned.len() {
        0 => PlannedNode::Contradiction,
        1 => planned.pop().expect("planned has one element"),
        _ => collapse_homogeneous_or(planned),
    }
}

fn collapse_homogeneous_or(planned: Vec<PlannedNode>) -> PlannedNode {
    let all_summary = planned
        .iter()
        .all(|node| matches!(node, PlannedNode::Summary(_)));
    let all_span = planned
        .iter()
        .all(|node| matches!(node, PlannedNode::Span(_)));

    if all_summary {
        return PlannedNode::Summary(
            planned
                .into_iter()
                .filter_map(|node| match node {
                    PlannedNode::Summary(expr) => Some(expr),
                    _ => None,
                })
                .reduce(|left, right| left.or(right))
                .expect("all_summary implies at least one expr"),
        );
    }

    if all_span {
        return PlannedNode::Span(
            planned
                .into_iter()
                .filter_map(|node| match node {
                    PlannedNode::Span(expr) => Some(expr),
                    _ => None,
                })
                .reduce(|left, right| left.or(right))
                .expect("all_span implies at least one expr"),
        );
    }

    // Mixed OR cannot be pushed as two filters; it must become a union of
    // per-branch trace-id sets or `OR` is accidentally converted into `AND`.
    PlannedNode::Or(planned)
}

fn plan_not(inner: &FilterClause) -> PlannedNode {
    match plan_node(inner) {
        PlannedNode::Summary(expr) => PlannedNode::Summary(datafusion::logical_expr::not(expr)),
        PlannedNode::Span(expr) => PlannedNode::TraceIdSet {
            span_predicate: expr,
            complement: true,
        },
        PlannedNode::TraceIdSet {
            span_predicate,
            complement,
        } => PlannedNode::TraceIdSet {
            span_predicate,
            complement: !complement,
        },
        PlannedNode::Tautology => PlannedNode::Contradiction,
        PlannedNode::Contradiction => PlannedNode::Tautology,
        node => PlannedNode::Complement(Box::new(node)),
    }
}

#[cfg(test)]
fn resolution_for_node(node: &PlannedNode) -> ClauseResolution {
    match node {
        PlannedNode::Summary(expr) => ClauseResolution::SummaryExpr(expr.clone()),
        PlannedNode::Span(_) => ClauseResolution::SpanExpr,
        PlannedNode::TraceIdSet { complement, .. } => ClauseResolution::TraceIdSet {
            complement: *complement,
        },
        PlannedNode::Tautology => ClauseResolution::Tautology,
        PlannedNode::Contradiction => ClauseResolution::Contradiction,
        PlannedNode::And(_) | PlannedNode::Or(_) | PlannedNode::Complement(_) => {
            ClauseResolution::Tautology
        }
    }
}

fn trace_ids_for_node<'a>(
    ctx: &'a SessionContext,
    time_window: TimeWindow,
    node: &'a PlannedNode,
    summary_table: &'a str,
) -> PlannerFuture<'a, DataFrame> {
    Box::pin(async move {
        match node {
            PlannedNode::Summary(expr) => {
                summary_trace_ids_df(ctx, time_window, Some(expr.clone()), summary_table).await
            }
            PlannedNode::Span(expr) => {
                span_trace_ids_df(ctx, time_window, Some(expr.clone())).await
            }
            PlannedNode::TraceIdSet {
                span_predicate,
                complement,
            } => {
                if *complement {
                    complement_trace_ids_for_span_predicate(
                        ctx,
                        time_window,
                        span_predicate.clone(),
                        summary_table,
                    )
                    .await
                } else {
                    span_trace_ids_df(ctx, time_window, Some(span_predicate.clone())).await
                }
            }
            PlannedNode::And(parts) => {
                trace_ids_for_and(ctx, time_window, parts, summary_table).await
            }
            PlannedNode::Or(parts) => {
                trace_ids_for_or(ctx, time_window, parts, summary_table).await
            }
            PlannedNode::Complement(inner) => {
                let universe = summary_trace_ids_df(ctx, time_window, None, summary_table).await?;
                let matched = trace_ids_for_node(ctx, time_window, inner, summary_table).await?;
                anti_join_trace_ids(universe, matched)
            }
            PlannedNode::Tautology => {
                summary_trace_ids_df(ctx, time_window, None, summary_table).await
            }
            PlannedNode::Contradiction => {
                summary_trace_ids_df(ctx, time_window, None, summary_table)
                    .await?
                    .filter(lit(false))
                    .map_err(TraceEngineError::DatafusionError)
            }
        }
    })
}

async fn trace_ids_for_and(
    ctx: &SessionContext,
    time_window: TimeWindow,
    parts: &[PlannedNode],
    summary_table: &str,
) -> Result<DataFrame, TraceEngineError> {
    let mut df = summary_trace_ids_df(ctx, time_window, None, summary_table).await?;

    for part in parts {
        match part {
            PlannedNode::Tautology => {}
            PlannedNode::Contradiction => {
                df = df.filter(lit(false))?;
            }
            PlannedNode::Summary(expr) => {
                df = df.filter(expr.clone())?;
            }
            node => {
                let right = trace_ids_for_node(ctx, time_window, node, summary_table).await?;
                df = join_trace_ids(df, right, JoinType::Inner)?;
            }
        }
    }

    Ok(df.select_columns(&[TRACE_ID_COL])?.distinct()?)
}

async fn trace_ids_for_or(
    ctx: &SessionContext,
    time_window: TimeWindow,
    parts: &[PlannedNode],
    summary_table: &str,
) -> Result<DataFrame, TraceEngineError> {
    let mut iter = parts.iter();
    let Some(first) = iter.next() else {
        return summary_trace_ids_df(ctx, time_window, None, summary_table)
            .await?
            .filter(lit(false))
            .map_err(TraceEngineError::DatafusionError);
    };

    let mut df = trace_ids_for_node(ctx, time_window, first, summary_table).await?;
    for part in iter {
        df = df.union(trace_ids_for_node(ctx, time_window, part, summary_table).await?)?;
    }

    Ok(df.distinct()?)
}

async fn summary_trace_ids_df(
    ctx: &SessionContext,
    time_window: TimeWindow,
    filter: Option<Expr>,
    summary_table: &str,
) -> Result<DataFrame, TraceEngineError> {
    let mut df = if summary_table == SUMMARY_TABLE_NAME {
        super::summary::deduped_summary_df(ctx, time_window).await?
    } else {
        apply_summary_time_window(ctx.table(summary_table).await?, time_window)?
    };
    if let Some(expr) = filter {
        df = df.filter(expr)?;
    }
    Ok(df.select_columns(&[TRACE_ID_COL])?.distinct()?)
}

async fn span_trace_ids_df(
    ctx: &SessionContext,
    time_window: TimeWindow,
    filter: Option<Expr>,
) -> Result<DataFrame, TraceEngineError> {
    let mut df = ctx.table(SPAN_TABLE_NAME).await?;
    df = apply_time_window(df, time_window)?;
    if let Some(expr) = filter {
        df = df.filter(expr)?;
    }
    Ok(df.select_columns(&[TRACE_ID_COL])?.distinct()?)
}

async fn complement_trace_ids_for_span_predicate(
    ctx: &SessionContext,
    time_window: TimeWindow,
    span_predicate: Expr,
    summary_table: &str,
) -> Result<DataFrame, TraceEngineError> {
    let universe = summary_trace_ids_df(ctx, time_window, None, summary_table).await?;
    let matched = span_trace_ids_df(ctx, time_window, Some(span_predicate)).await?;
    anti_join_trace_ids(universe, matched)
}

fn apply_time_window(
    mut df: DataFrame,
    time_window: TimeWindow,
) -> Result<DataFrame, TraceEngineError> {
    // Partition pruning must precede row-level timestamp pruning so Delta/DataFusion
    // can discard whole date directories before opening Parquet metadata.
    if let Some(start) = time_window.start {
        df = df.filter(col(PARTITION_DATE_COL).gt_eq(date_lit(&start)))?;
        df = df.filter(col(START_TIME_COL).gt_eq(ts_lit(&start)))?;
    }
    if let Some(end) = time_window.end {
        df = df.filter(col(PARTITION_DATE_COL).lt_eq(date_lit(&end)))?;
        df = df.filter(col(START_TIME_COL).lt(ts_lit(&end)))?;
    }
    Ok(df)
}

fn apply_summary_time_window(
    mut df: DataFrame,
    time_window: TimeWindow,
) -> Result<DataFrame, TraceEngineError> {
    if let Some(start) = time_window.start {
        df = df.filter(col(START_TIME_COL).gt_eq(ts_lit(&start)))?;
    }
    if let Some(end) = time_window.end {
        df = df.filter(col(START_TIME_COL).lt(ts_lit(&end)))?;
    }
    Ok(df)
}

fn join_trace_ids(
    left: DataFrame,
    right: DataFrame,
    join_type: JoinType,
) -> Result<DataFrame, TraceEngineError> {
    let probe = right.select(vec![col(TRACE_ID_COL).alias("_planner_trace_id")])?;
    Ok(left.join(
        probe,
        join_type,
        &[TRACE_ID_COL],
        &["_planner_trace_id"],
        None,
    )?)
}

fn anti_join_trace_ids(left: DataFrame, right: DataFrame) -> Result<DataFrame, TraceEngineError> {
    join_trace_ids(left, right, JoinType::LeftAnti)
}

async fn span_constraint(
    ctx: &SessionContext,
    time_window: TimeWindow,
    span_predicate: Expr,
    complement: bool,
) -> Result<TraceIdConstraint, TraceEngineError> {
    let df = if complement {
        complement_trace_ids_for_span_predicate(
            ctx,
            time_window,
            span_predicate,
            SUMMARY_TABLE_NAME,
        )
        .await?
    } else {
        span_trace_ids_df(ctx, time_window, Some(span_predicate)).await?
    };
    bounded_constraint(df, false).await
}

async fn bounded_constraint(
    df: DataFrame,
    complement: bool,
) -> Result<TraceIdConstraint, TraceEngineError> {
    let actual = df.clone().count().await?;
    if actual > MAX_INTERMEDIATE_TRACE_IDS {
        return Err(TraceEngineError::IntermediateSetTooLarge {
            actual,
            limit: MAX_INTERMEDIATE_TRACE_IDS,
        });
    }

    Ok(TraceIdConstraint {
        df,
        complement,
        estimated_card: Some(actual),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use arrow::datatypes::{DataType, Field, Schema};
    use arrow_array::{ArrayRef, BinaryArray, RecordBatch, StringArray};
    use datafusion::datasource::MemTable;
    use std::sync::Arc;

    fn is_summary(resolution: &ClauseResolution) -> bool {
        matches!(resolution, ClauseResolution::SummaryExpr(_))
    }

    fn is_span(resolution: &ClauseResolution) -> bool {
        matches!(resolution, ClauseResolution::SpanExpr)
    }

    #[test]
    fn pure_summary_clause_plans_to_summary_expr() {
        let plan = plan_clause(&FilterClause::And(vec![
            FilterClause::Service("api".into()),
            FilterClause::HasErrors(true),
        ]));

        assert!(is_summary(&plan.root));
        assert!(plan.or_branches.is_empty());
    }

    #[test]
    fn pure_span_clause_plans_to_span_expr() {
        let plan = plan_clause(&FilterClause::Phrase("timeout".into()));

        assert!(is_span(&plan.root));
        assert!(plan.or_branches.is_empty());
    }

    #[test]
    fn mixed_and_keeps_full_tree_for_executor() {
        let plan = plan_clause(&FilterClause::And(vec![
            FilterClause::Service("api".into()),
            FilterClause::Phrase("timeout".into()),
        ]));

        assert!(matches!(plan.node, PlannedNode::And(_)));
        assert!(plan.or_branches.is_empty());
    }

    #[test]
    fn mixed_or_exposes_flat_branches() {
        let plan = plan_clause(&FilterClause::Or(vec![
            FilterClause::Service("api".into()),
            FilterClause::Phrase("timeout".into()),
        ]));

        assert_eq!(plan.or_branches.len(), 2);
        assert!(is_summary(&plan.or_branches[0]));
        assert!(is_span(&plan.or_branches[1]));
    }

    #[test]
    fn not_summary_stays_summary_expr() {
        let plan = plan_clause(&FilterClause::Not(Box::new(FilterClause::Service(
            "api".into(),
        ))));

        assert!(is_summary(&plan.root));
    }

    #[test]
    fn not_span_becomes_complement_trace_id_set() {
        let plan = plan_clause(&FilterClause::Not(Box::new(FilterClause::Phrase(
            "timeout".into(),
        ))));

        assert!(matches!(
            plan.root,
            ClauseResolution::TraceIdSet {
                complement: true,
                ..
            }
        ));
    }

    #[test]
    fn double_not_span_restores_positive_trace_id_set() {
        let plan = plan_clause(&FilterClause::Not(Box::new(FilterClause::Not(Box::new(
            FilterClause::Phrase("timeout".into()),
        )))));

        assert!(matches!(
            plan.root,
            ClauseResolution::TraceIdSet {
                complement: false,
                ..
            }
        ));
    }

    #[test]
    fn empty_and_is_tautology() {
        let plan = plan_clause(&FilterClause::And(vec![]));
        assert!(matches!(plan.root, ClauseResolution::Tautology));
    }

    #[test]
    fn empty_or_is_contradiction() {
        let plan = plan_clause(&FilterClause::Or(vec![]));
        assert!(matches!(plan.root, ClauseResolution::Contradiction));
    }

    #[test]
    fn sanitize_strips_pipe() {
        assert_eq!(sanitize_search_value("val|ue"), "value");
        assert_eq!(sanitize_search_value("clean"), "clean");
        assert_eq!(sanitize_search_value("|"), "");
    }

    #[tokio::test]
    async fn span_resolution_overflow_returns_explicit_error() {
        let ctx = SessionContext::new();
        let schema = Arc::new(Schema::new(vec![
            Field::new(TRACE_ID_COL, DataType::Binary, false),
            Field::new(SEARCH_BLOB_COL, DataType::Utf8, false),
        ]));
        let ids: Vec<Vec<u8>> = (0..=MAX_INTERMEDIATE_TRACE_IDS)
            .map(|i| (i as u128).to_be_bytes().to_vec())
            .collect();
        let traces = BinaryArray::from_iter_values(ids.iter().map(|id| id.as_slice()));
        let blobs = StringArray::from_iter_values(std::iter::repeat_n(
            "|message=hit|",
            MAX_INTERMEDIATE_TRACE_IDS + 1,
        ));
        let batch = RecordBatch::try_new(
            schema.clone(),
            vec![Arc::new(traces) as ArrayRef, Arc::new(blobs) as ArrayRef],
        )
        .unwrap();
        let table = MemTable::try_new(schema, vec![vec![batch]]).unwrap();
        ctx.register_table(SPAN_TABLE_NAME, Arc::new(table))
            .unwrap();

        // Broad span predicates can otherwise create huge join inputs; the
        // planner must fail loudly instead of silently truncating matches.
        let plan = plan_clause(&FilterClause::Phrase("hit".to_string()));
        let result = execute_plan(
            &plan,
            &ctx,
            TimeWindow {
                start: None,
                end: None,
            },
        )
        .await;
        assert!(result.is_err(), "broad span predicate should exceed cap");
        let err = result.err().unwrap();

        assert!(matches!(
            err,
            TraceEngineError::IntermediateSetTooLarge {
                actual,
                limit: MAX_INTERMEDIATE_TRACE_IDS
            } if actual == MAX_INTERMEDIATE_TRACE_IDS + 1
        ));
    }
}
