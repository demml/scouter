use crate::error::TraceEngineError;
use crate::parquet::tracing::span_view::TraceSpanBatch;
use arrow::array::RecordBatch;
use chrono::{DateTime, Utc};
use datafusion::logical_expr::{col, lit, SortExpr};
use datafusion::prelude::*;
use std::sync::Arc;
use tracing::{error, info, instrument};

// common columns
pub const START_TIME_COL: &str = "start_time";
pub const END_TIME_COL: &str = "end_time";
pub const SERVICE_NAME_COL: &str = "service_name";
pub const TRACE_ID_COL: &str = "trace_id";
pub const SPAN_ID_COL: &str = "span_id";
pub const PARENT_SPAN_ID_COL: &str = "parent_span_id";
pub const ROOT_SPAN_ID_COL: &str = "root_span_id";
pub const SPAN_NAME_COL: &str = "span_name";
pub const SPAN_KIND_COL: &str = "span_kind";
pub const DURATION_MS_COL: &str = "duration_ms";
pub const STATUS_CODE_COL: &str = "status_code";
pub const STATUS_MESSAGE_COL: &str = "status_message";
pub const DEPTH_COL: &str = "depth";
pub const SPAN_ORDER_COL: &str = "span_order";
pub const PATH_COL: &str = "path";
pub const ATTRIBUTES_COL: &str = "attributes";
pub const EVENTS_COL: &str = "events";
pub const LINKS_COL: &str = "links";
pub const INPUT_COL: &str = "input";
pub const OUTPUT_COL: &str = "output";
pub const ALL_COLUMNS: [&str; 20] = [
    TRACE_ID_COL,
    SPAN_ID_COL,
    PARENT_SPAN_ID_COL,
    ROOT_SPAN_ID_COL,
    SERVICE_NAME_COL,
    SPAN_NAME_COL,
    SPAN_KIND_COL,
    START_TIME_COL,
    END_TIME_COL,
    DURATION_MS_COL,
    STATUS_CODE_COL,
    STATUS_MESSAGE_COL,
    DEPTH_COL,
    SPAN_ORDER_COL,
    PATH_COL,
    ATTRIBUTES_COL,
    EVENTS_COL,
    LINKS_COL,
    INPUT_COL,
    OUTPUT_COL,
];
pub const SPAN_TABLE_NAME: &str = "trace_spans";

struct TraceQueryBuilder {
    df: DataFrame,
}

impl TraceQueryBuilder {
    async fn set_table(
        ctx: Arc<SessionContext>,
        table_name: &str,
    ) -> Result<Self, TraceEngineError> {
        let df = ctx
            .table(table_name)
            .await
            .inspect_err(|e| error!("Failed to load table {}: {}", table_name, e))?;
        Ok(Self { df })
    }

    fn select_columns(mut self, columns: &[&str]) -> Result<Self, TraceEngineError> {
        self.df = self.df.select_columns(columns)?;
        Ok(self)
    }

    fn add_filter(mut self, expr: Expr) -> Result<Self, TraceEngineError> {
        self.df = self.df.filter(expr)?;
        Ok(self)
    }

    fn add_sort(mut self, sort: Vec<SortExpr>) -> Result<Self, TraceEngineError> {
        self.df = self.df.sort(sort)?;
        Ok(self)
    }

    fn with_limit(mut self, n: Option<usize>) -> Result<Self, TraceEngineError> {
        self.df = self.df.limit(0, n)?;
        Ok(self)
    }

    async fn execute(self) -> Result<Vec<RecordBatch>, TraceEngineError> {
        let batches = self
            .df
            .collect()
            .await
            .inspect_err(|e| error!("Failed to collect query results: {}", e))?;
        Ok(batches)
    }
}
/// High-performance query patterns for Delta Lake trace storage
///
/// Design principles:
/// 1. Time-based filters FIRST (enables partition pruning)
/// 2. Binary ID comparisons (no hex decoding overhead)
/// 3. Dictionary-encoded column filters (service_name, span_kind)
/// 4. Minimize data scanned via projection pushdown
/// 5. Leverage pre-computed fields (depth, span_order, path, root_span_id)
pub struct TraceQueries {
    ctx: Arc<SessionContext>,
}

impl TraceQueries {
    pub fn new(ctx: Arc<SessionContext>) -> Self {
        Self { ctx }
    }

    /// Get all spans for a trace_id (direct equivalent of Postgres scouter.get_trace_spans)
    ///
    /// This is MUCH simpler than the Postgres version because:
    /// - No recursive CTE needed (depth/path/root_span_id pre-computed)
    /// - Already sorted by span_order
    /// - Direct binary comparison on trace_id
    ///
    /// # Arguments
    /// * `trace_id_bytes` - 16-byte trace ID (no hex encoding needed)
    /// * `service_name` - Optional service name filter
    /// * `start_time_hint` - Optional time range hint for partition pruning
    ///
    /// # Performance
    /// - Without time hint: Scans all Delta Lake files (still fast due to min/max stats)
    /// - With time hint: 100-1000x faster via partition pruning
    #[instrument(skip_all)]
    pub async fn get_trace_spans(
        &self,
        trace_id_bytes: Option<&[u8]>,
        service_name: Option<&str>,
        start_time: Option<&DateTime<Utc>>,
        end_time: Option<&DateTime<Utc>>,
        limit: Option<usize>,
    ) -> Result<Vec<TraceSpanBatch>, TraceEngineError> {
        let mut builder = TraceQueryBuilder::set_table(self.ctx.clone(), SPAN_TABLE_NAME).await?;
        builder = builder.select_columns(&ALL_COLUMNS)?;

        // Add time filters first to enable partition pruning
        let mut time_filter: Option<Expr> = None;
        if let Some(start) = start_time {
            let filter = col(START_TIME_COL).gt_eq(lit(start.to_rfc3339()));
            time_filter = Some(match time_filter {
                Some(existing) => existing.and(filter),
                None => filter,
            });
        }

        if let Some(end) = end_time {
            let filter = col(START_TIME_COL).lt(lit(end.to_rfc3339()));
            time_filter = Some(match time_filter {
                Some(existing) => existing.and(filter),
                None => filter,
            });
        }

        if let Some(filter) = time_filter {
            builder = builder.add_filter(filter)?;
        }

        if let Some(trace_id_bytes) = trace_id_bytes {
            builder = builder.add_filter(col(TRACE_ID_COL).eq(lit(&trace_id_bytes[..])))?;
        }

        if let Some(service) = service_name {
            builder = builder.add_filter(col(SERVICE_NAME_COL).eq(lit(service)))?;
        }

        let sort = col(SPAN_ORDER_COL).sort(true, true);
        builder = builder.add_sort(vec![sort])?;
        builder = builder.with_limit(limit)?;

        let batches = builder.execute().await?;

        info!(
            "Queried {} spans across {} batches",
            batches.iter().map(|b| b.num_rows()).sum::<usize>(),
            batches.len()
        );

        batches_to_span_views(batches)
    }

    /// Get trace tree structure (same as get_trace_spans but returns raw RecordBatches)
    ///
    /// Use this when you need Arrow-native processing without conversion overhead
    pub async fn get_trace_tree_batches(
        &self,
        trace_id_bytes: &[u8],
        start_time_hint: Option<(&str, &str)>,
    ) -> Result<Vec<RecordBatch>, TraceEngineError> {
        // Start with the base table
        let mut df = self.ctx.table("trace_spans").await?;

        // Apply time filter first (enables partition pruning)
        if let Some((min, max)) = start_time_hint {
            df = df.filter(
                col("start_time")
                    .gt_eq(lit(min))
                    .and(col("start_time").lt(lit(max))),
            )?;
        }

        df = df.filter(col("trace_id").eq(lit(&trace_id_bytes[..])))?;

        // Select only needed columns for tree structure
        df = df.select_columns(&[
            "span_id",
            "parent_span_id",
            "root_span_id",
            "service_name",
            "span_name",
            "span_kind",
            "start_time",
            "end_time",
            "duration_ms",
            "status_code",
            "status_message",
            "depth",
            "span_order",
            "path",
            "attributes",
        ])?;

        // Order by span_order
        df = df.sort(vec![col("span_order").sort(true, true)])?;

        let batches = df.collect().await?;
        Ok(batches)
    }
}

/// Convert RecordBatches to zero-copy TraceSpanBatch views
///
/// This creates Arc-backed views with NO allocations!
/// Allocations only happen during serialization (hex encoding, JSON stringify)
fn batches_to_span_views(
    batches: Vec<RecordBatch>,
) -> Result<Vec<TraceSpanBatch>, TraceEngineError> {
    batches
        .iter()
        .map(|batch| {
            TraceSpanBatch::from_record_batch(batch)
                .map_err(|e| TraceEngineError::BatchConversion(format!("Arrow error: {}", e)))
        })
        .collect()
}
