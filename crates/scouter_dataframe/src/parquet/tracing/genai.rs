use crate::error::TraceEngineError;
use crate::parquet::control::{get_pod_id, ControlTableEngine};
use crate::parquet::tracing::catalog::TraceCatalogProvider;
use crate::parquet::tracing::queries::{date_lit, ts_lit};
use crate::parquet::tracing::traits::arrow_schema_to_delta;
use crate::parquet::utils::register_cloud_logstore_factories;
use crate::storage::ObjectStore;
use arrow::array::*;
use arrow::compute;
use arrow::datatypes::*;
use arrow_array::Array;
use arrow_array::RecordBatch;
use chrono::{DateTime, Datelike, Utc};
use datafusion::functions_aggregate::expr_fn::{avg, count, sum};
use datafusion::logical_expr::{col, lit};
use datafusion::prelude::*;
use deltalake::datafusion::parquet::basic::{Compression, Encoding, ZstdLevel};
use deltalake::datafusion::parquet::file::properties::WriterProperties;
use deltalake::datafusion::parquet::schema::types::ColumnPath;
use deltalake::operations::optimize::OptimizeType;
use deltalake::{DeltaTable, DeltaTableBuilder, TableProperty};
use mini_moka::sync::Cache;
use scouter_types::{
    GenAiAgentActivity, GenAiModelUsage, GenAiOperationBreakdown, GenAiSpanFilters,
    GenAiSpanRecord, GenAiTokenBucket, GenAiToolActivity, SpanId, TraceId,
};
use ahash::AHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::oneshot;
use tokio::sync::{mpsc, RwLock as AsyncRwLock};
use tokio::time::interval;
use tracing::{debug, error, info, instrument};
use url::Url;

const GEN_AI_TABLE_NAME: &str = "gen_ai_spans";
const TASK_GENAI_OPTIMIZE: &str = "genai_optimize";
const TASK_GENAI_RETENTION: &str = "genai_retention";
const UNIX_EPOCH_DAYS: i32 = 719_163;

// ── Column name constants ────────────────────────────────────────────────────
const TRACE_ID_COL: &str = "trace_id";
const SPAN_ID_COL: &str = "span_id";
const SERVICE_NAME_COL: &str = "service_name";
const START_TIME_COL: &str = "start_time";
const END_TIME_COL: &str = "end_time";
const DURATION_MS_COL: &str = "duration_ms";
const STATUS_CODE_COL: &str = "status_code";
const OPERATION_NAME_COL: &str = "operation_name";
const PROVIDER_NAME_COL: &str = "provider_name";
const REQUEST_MODEL_COL: &str = "request_model";
const RESPONSE_MODEL_COL: &str = "response_model";
const RESPONSE_ID_COL: &str = "response_id";
const INPUT_TOKENS_COL: &str = "input_tokens";
const OUTPUT_TOKENS_COL: &str = "output_tokens";
const CACHE_CREATION_INPUT_TOKENS_COL: &str = "cache_creation_input_tokens";
const CACHE_READ_INPUT_TOKENS_COL: &str = "cache_read_input_tokens";
const FINISH_REASONS_COL: &str = "finish_reasons";
const OUTPUT_TYPE_COL: &str = "output_type";
const CONVERSATION_ID_COL: &str = "conversation_id";
const AGENT_NAME_COL: &str = "agent_name";
const AGENT_ID_COL: &str = "agent_id";
const TOOL_NAME_COL: &str = "tool_name";
const TOOL_TYPE_COL: &str = "tool_type";
const TOOL_CALL_ID_COL: &str = "tool_call_id";
const REQUEST_TEMPERATURE_COL: &str = "request_temperature";
const REQUEST_MAX_TOKENS_COL: &str = "request_max_tokens";
const REQUEST_TOP_P_COL: &str = "request_top_p";
const ERROR_TYPE_COL: &str = "error_type";
const OPENAI_API_TYPE_COL: &str = "openai_api_type";
const OPENAI_SERVICE_TIER_COL: &str = "openai_service_tier";
const LABEL_COL: &str = "label";
const PARTITION_DATE_COL: &str = "partition_date";

// ── Schema ───────────────────────────────────────────────────────────────────

fn create_genai_schema() -> Schema {
    Schema::new(vec![
        Field::new(TRACE_ID_COL, DataType::FixedSizeBinary(16), false),
        Field::new(SPAN_ID_COL, DataType::FixedSizeBinary(8), false),
        Field::new(
            SERVICE_NAME_COL,
            DataType::Dictionary(Box::new(DataType::Int32), Box::new(DataType::Utf8)),
            false,
        ),
        Field::new(
            START_TIME_COL,
            DataType::Timestamp(TimeUnit::Microsecond, Some("UTC".into())),
            false,
        ),
        Field::new(
            END_TIME_COL,
            DataType::Timestamp(TimeUnit::Microsecond, Some("UTC".into())),
            true,
        ),
        Field::new(DURATION_MS_COL, DataType::Int64, false),
        Field::new(STATUS_CODE_COL, DataType::Int32, false),
        Field::new(
            OPERATION_NAME_COL,
            DataType::Dictionary(Box::new(DataType::Int8), Box::new(DataType::Utf8)),
            true,
        ),
        Field::new(
            PROVIDER_NAME_COL,
            DataType::Dictionary(Box::new(DataType::Int8), Box::new(DataType::Utf8)),
            true,
        ),
        Field::new(
            REQUEST_MODEL_COL,
            DataType::Dictionary(Box::new(DataType::Int16), Box::new(DataType::Utf8)),
            true,
        ),
        Field::new(
            RESPONSE_MODEL_COL,
            DataType::Dictionary(Box::new(DataType::Int16), Box::new(DataType::Utf8)),
            true,
        ),
        Field::new(RESPONSE_ID_COL, DataType::Utf8View, true),
        Field::new(INPUT_TOKENS_COL, DataType::Int64, true),
        Field::new(OUTPUT_TOKENS_COL, DataType::Int64, true),
        Field::new(CACHE_CREATION_INPUT_TOKENS_COL, DataType::Int64, true),
        Field::new(CACHE_READ_INPUT_TOKENS_COL, DataType::Int64, true),
        Field::new(
            FINISH_REASONS_COL,
            DataType::List(Arc::new(Field::new("item", DataType::Utf8, true))),
            true,
        ),
        Field::new(
            OUTPUT_TYPE_COL,
            DataType::Dictionary(Box::new(DataType::Int8), Box::new(DataType::Utf8)),
            true,
        ),
        Field::new(CONVERSATION_ID_COL, DataType::Utf8, true),
        Field::new(
            AGENT_NAME_COL,
            DataType::Dictionary(Box::new(DataType::Int16), Box::new(DataType::Utf8)),
            true,
        ),
        Field::new(AGENT_ID_COL, DataType::Utf8View, true),
        Field::new(
            TOOL_NAME_COL,
            DataType::Dictionary(Box::new(DataType::Int16), Box::new(DataType::Utf8)),
            true,
        ),
        Field::new(
            TOOL_TYPE_COL,
            DataType::Dictionary(Box::new(DataType::Int8), Box::new(DataType::Utf8)),
            true,
        ),
        Field::new(TOOL_CALL_ID_COL, DataType::Utf8View, true),
        Field::new(REQUEST_TEMPERATURE_COL, DataType::Float64, true),
        Field::new(REQUEST_MAX_TOKENS_COL, DataType::Int64, true),
        Field::new(REQUEST_TOP_P_COL, DataType::Float64, true),
        Field::new(
            ERROR_TYPE_COL,
            DataType::Dictionary(Box::new(DataType::Int8), Box::new(DataType::Utf8)),
            true,
        ),
        Field::new(
            OPENAI_API_TYPE_COL,
            DataType::Dictionary(Box::new(DataType::Int8), Box::new(DataType::Utf8)),
            true,
        ),
        Field::new(OPENAI_SERVICE_TIER_COL, DataType::Utf8, true),
        Field::new(LABEL_COL, DataType::Utf8, true),
        Field::new(PARTITION_DATE_COL, DataType::Date32, false),
    ])
}

// ── BatchBuilder ─────────────────────────────────────────────────────────────

struct GenAiBatchBuilder {
    schema: Arc<Schema>,
    trace_id: FixedSizeBinaryBuilder,
    span_id: FixedSizeBinaryBuilder,
    service_name: StringDictionaryBuilder<Int32Type>,
    start_time: TimestampMicrosecondBuilder,
    end_time: TimestampMicrosecondBuilder,
    duration_ms: Int64Builder,
    status_code: Int32Builder,
    operation_name: StringDictionaryBuilder<Int8Type>,
    provider_name: StringDictionaryBuilder<Int8Type>,
    request_model: StringDictionaryBuilder<Int16Type>,
    response_model: StringDictionaryBuilder<Int16Type>,
    response_id: StringViewBuilder,
    input_tokens: Int64Builder,
    output_tokens: Int64Builder,
    cache_creation_input_tokens: Int64Builder,
    cache_read_input_tokens: Int64Builder,
    finish_reasons: ListBuilder<StringBuilder>,
    output_type: StringDictionaryBuilder<Int8Type>,
    conversation_id: StringBuilder,
    agent_name: StringDictionaryBuilder<Int16Type>,
    agent_id: StringViewBuilder,
    tool_name: StringDictionaryBuilder<Int16Type>,
    tool_type: StringDictionaryBuilder<Int8Type>,
    tool_call_id: StringViewBuilder,
    request_temperature: Float64Builder,
    request_max_tokens: Int64Builder,
    request_top_p: Float64Builder,
    error_type: StringDictionaryBuilder<Int8Type>,
    openai_api_type: StringDictionaryBuilder<Int8Type>,
    openai_service_tier: StringBuilder,
    label: StringBuilder,
    partition_date: Date32Builder,
}

impl GenAiBatchBuilder {
    fn new(schema: Arc<Schema>, capacity: usize) -> Self {
        Self {
            schema,
            trace_id: FixedSizeBinaryBuilder::with_capacity(capacity, 16),
            span_id: FixedSizeBinaryBuilder::with_capacity(capacity, 8),
            service_name: StringDictionaryBuilder::new(),
            start_time: TimestampMicrosecondBuilder::with_capacity(capacity).with_timezone("UTC"),
            end_time: TimestampMicrosecondBuilder::with_capacity(capacity).with_timezone("UTC"),
            duration_ms: Int64Builder::with_capacity(capacity),
            status_code: Int32Builder::with_capacity(capacity),
            operation_name: StringDictionaryBuilder::new(),
            provider_name: StringDictionaryBuilder::new(),
            request_model: StringDictionaryBuilder::new(),
            response_model: StringDictionaryBuilder::new(),
            response_id: StringViewBuilder::new(),
            input_tokens: Int64Builder::with_capacity(capacity),
            output_tokens: Int64Builder::with_capacity(capacity),
            cache_creation_input_tokens: Int64Builder::with_capacity(capacity),
            cache_read_input_tokens: Int64Builder::with_capacity(capacity),
            finish_reasons: ListBuilder::new(StringBuilder::new()),
            output_type: StringDictionaryBuilder::new(),
            conversation_id: StringBuilder::with_capacity(capacity, capacity * 32),
            agent_name: StringDictionaryBuilder::new(),
            agent_id: StringViewBuilder::new(),
            tool_name: StringDictionaryBuilder::new(),
            tool_type: StringDictionaryBuilder::new(),
            tool_call_id: StringViewBuilder::new(),
            request_temperature: Float64Builder::with_capacity(capacity),
            request_max_tokens: Int64Builder::with_capacity(capacity),
            request_top_p: Float64Builder::with_capacity(capacity),
            error_type: StringDictionaryBuilder::new(),
            openai_api_type: StringDictionaryBuilder::new(),
            openai_service_tier: StringBuilder::with_capacity(capacity, capacity * 16),
            label: StringBuilder::with_capacity(capacity, capacity * 16),
            partition_date: Date32Builder::with_capacity(capacity),
        }
    }

    fn append(&mut self, rec: &GenAiSpanRecord) -> Result<(), TraceEngineError> {
        self.trace_id.append_value(rec.trace_id.as_bytes())?;
        self.span_id.append_value(rec.span_id.as_bytes())?;
        self.service_name.append_value(&rec.service_name);
        self.start_time
            .append_value(rec.start_time.timestamp_micros());
        match rec.end_time {
            Some(end) => self.end_time.append_value(end.timestamp_micros()),
            None => self.end_time.append_null(),
        }
        self.duration_ms.append_value(rec.duration_ms);
        self.status_code.append_value(rec.status_code);

        match &rec.operation_name {
            Some(v) => self.operation_name.append_value(v),
            None => self.operation_name.append_null(),
        }
        match &rec.provider_name {
            Some(v) => self.provider_name.append_value(v),
            None => self.provider_name.append_null(),
        }
        match &rec.request_model {
            Some(v) => self.request_model.append_value(v),
            None => self.request_model.append_null(),
        }
        match &rec.response_model {
            Some(v) => self.response_model.append_value(v),
            None => self.response_model.append_null(),
        }
        match &rec.response_id {
            Some(v) => self.response_id.append_value(v),
            None => self.response_id.append_null(),
        }
        self.input_tokens.append_option(rec.input_tokens);
        self.output_tokens.append_option(rec.output_tokens);
        self.cache_creation_input_tokens
            .append_option(rec.cache_creation_input_tokens);
        self.cache_read_input_tokens
            .append_option(rec.cache_read_input_tokens);

        if rec.finish_reasons.is_empty() {
            self.finish_reasons.append(false);
        } else {
            for reason in &rec.finish_reasons {
                self.finish_reasons.values().append_value(reason);
            }
            self.finish_reasons.append(true);
        }

        match &rec.output_type {
            Some(v) => self.output_type.append_value(v),
            None => self.output_type.append_null(),
        }
        match &rec.conversation_id {
            Some(v) => self.conversation_id.append_value(v),
            None => self.conversation_id.append_null(),
        }
        match &rec.agent_name {
            Some(v) => self.agent_name.append_value(v),
            None => self.agent_name.append_null(),
        }
        match &rec.agent_id {
            Some(v) => self.agent_id.append_value(v),
            None => self.agent_id.append_null(),
        }
        match &rec.tool_name {
            Some(v) => self.tool_name.append_value(v),
            None => self.tool_name.append_null(),
        }
        match &rec.tool_type {
            Some(v) => self.tool_type.append_value(v),
            None => self.tool_type.append_null(),
        }
        match &rec.tool_call_id {
            Some(v) => self.tool_call_id.append_value(v),
            None => self.tool_call_id.append_null(),
        }
        self.request_temperature
            .append_option(rec.request_temperature);
        self.request_max_tokens
            .append_option(rec.request_max_tokens);
        self.request_top_p.append_option(rec.request_top_p);

        match &rec.error_type {
            Some(v) => self.error_type.append_value(v),
            None => self.error_type.append_null(),
        }
        match &rec.openai_api_type {
            Some(v) => self.openai_api_type.append_value(v),
            None => self.openai_api_type.append_null(),
        }
        match &rec.openai_service_tier {
            Some(v) => self.openai_service_tier.append_value(v),
            None => self.openai_service_tier.append_null(),
        }
        match &rec.label {
            Some(v) => self.label.append_value(v),
            None => self.label.append_null(),
        }

        let days = rec.start_time.date_naive().num_days_from_ce() - UNIX_EPOCH_DAYS;
        self.partition_date.append_value(days);
        Ok(())
    }

    fn finish(mut self) -> Result<RecordBatch, TraceEngineError> {
        let columns: Vec<Arc<dyn Array>> = vec![
            Arc::new(self.trace_id.finish()),
            Arc::new(self.span_id.finish()),
            Arc::new(self.service_name.finish()),
            Arc::new(self.start_time.finish()),
            Arc::new(self.end_time.finish()),
            Arc::new(self.duration_ms.finish()),
            Arc::new(self.status_code.finish()),
            Arc::new(self.operation_name.finish()),
            Arc::new(self.provider_name.finish()),
            Arc::new(self.request_model.finish()),
            Arc::new(self.response_model.finish()),
            Arc::new(self.response_id.finish()),
            Arc::new(self.input_tokens.finish()),
            Arc::new(self.output_tokens.finish()),
            Arc::new(self.cache_creation_input_tokens.finish()),
            Arc::new(self.cache_read_input_tokens.finish()),
            Arc::new(self.finish_reasons.finish()),
            Arc::new(self.output_type.finish()),
            Arc::new(self.conversation_id.finish()),
            Arc::new(self.agent_name.finish()),
            Arc::new(self.agent_id.finish()),
            Arc::new(self.tool_name.finish()),
            Arc::new(self.tool_type.finish()),
            Arc::new(self.tool_call_id.finish()),
            Arc::new(self.request_temperature.finish()),
            Arc::new(self.request_max_tokens.finish()),
            Arc::new(self.request_top_p.finish()),
            Arc::new(self.error_type.finish()),
            Arc::new(self.openai_api_type.finish()),
            Arc::new(self.openai_service_tier.finish()),
            Arc::new(self.label.finish()),
            Arc::new(self.partition_date.finish()),
        ];
        RecordBatch::try_new(self.schema, columns).map_err(Into::into)
    }
}

// ── TableCommand ─────────────────────────────────────────────────────────────

pub enum GenAiTableCommand {
    Write {
        records: Vec<GenAiSpanRecord>,
        respond_to: oneshot::Sender<Result<(), TraceEngineError>>,
    },
    Optimize {
        respond_to: oneshot::Sender<Result<(), TraceEngineError>>,
    },
    Vacuum {
        retention_hours: u64,
        respond_to: oneshot::Sender<Result<(), TraceEngineError>>,
    },
    Expire {
        cutoff_date: chrono::NaiveDate,
        respond_to: oneshot::Sender<Result<(), TraceEngineError>>,
    },
    Shutdown,
}

// ── Delta table helpers ──────────────────────────────────────────────────────

async fn build_genai_url(object_store: &ObjectStore) -> Result<Url, TraceEngineError> {
    let mut base = object_store.get_base_url()?;
    let mut path = base.path().to_string();
    if !path.ends_with('/') {
        path.push('/');
    }
    path.push_str(GEN_AI_TABLE_NAME);
    base.set_path(&path);
    Ok(base)
}

async fn create_genai_table(
    object_store: &ObjectStore,
    table_url: Url,
    schema: SchemaRef,
) -> Result<DeltaTable, TraceEngineError> {
    info!(
        "Creating gen_ai_spans table [{}://.../{} ]",
        table_url.scheme(),
        table_url
            .path_segments()
            .and_then(|mut s| s.next_back())
            .unwrap_or(GEN_AI_TABLE_NAME)
    );
    let store = object_store.as_dyn_object_store();
    let table = DeltaTableBuilder::from_url(table_url.clone())?
        .with_storage_backend(store, table_url)
        .build()?;
    let delta_fields = arrow_schema_to_delta(&schema);
    table
        .create()
        .with_table_name(GEN_AI_TABLE_NAME)
        .with_columns(delta_fields)
        .with_partition_columns(vec![PARTITION_DATE_COL.to_string()])
        .with_configuration_property(
            TableProperty::DataSkippingStatsColumns,
            Some("start_time,end_time,service_name,duration_ms,status_code,operation_name,provider_name,partition_date"),
        )
        .await
        .map_err(Into::into)
}

async fn build_or_create_genai_table(
    object_store: &ObjectStore,
    schema: SchemaRef,
) -> Result<DeltaTable, TraceEngineError> {
    register_cloud_logstore_factories();
    let table_url = build_genai_url(object_store).await?;
    info!(
        "Loading gen_ai_spans table [{}://.../{} ]",
        table_url.scheme(),
        table_url
            .path_segments()
            .and_then(|mut s| s.next_back())
            .unwrap_or(GEN_AI_TABLE_NAME)
    );

    let is_delta_table = if table_url.scheme() == "file" {
        if let Ok(path) = table_url.to_file_path() {
            if !path.exists() {
                info!("Creating directory for gen_ai_spans table: {:?}", path);
                std::fs::create_dir_all(&path)?;
            }
            path.join("_delta_log").exists()
        } else {
            false
        }
    } else {
        let store = object_store.as_dyn_object_store();
        match DeltaTableBuilder::from_url(table_url.clone()) {
            Ok(builder) => builder
                .with_storage_backend(store, table_url.clone())
                .load()
                .await
                .is_ok(),
            Err(_) => false,
        }
    };

    if is_delta_table {
        info!(
            "Loaded existing gen_ai_spans table [{}://.../{} ]",
            table_url.scheme(),
            table_url
                .path_segments()
                .and_then(|mut s| s.next_back())
                .unwrap_or(GEN_AI_TABLE_NAME)
        );
        let store = object_store.as_dyn_object_store();
        DeltaTableBuilder::from_url(table_url.clone())?
            .with_storage_backend(store, table_url)
            .load()
            .await
            .map_err(Into::into)
    } else {
        info!("gen_ai_spans table does not exist, creating new table");
        create_genai_table(object_store, table_url, schema).await
    }
}

// ── DBEngine ─────────────────────────────────────────────────────────────────

pub struct GenAiSpanDBEngine {
    schema: Arc<Schema>,
    table: Arc<AsyncRwLock<DeltaTable>>,
    ctx: Arc<SessionContext>,
    catalog: Arc<TraceCatalogProvider>,
    control: ControlTableEngine,
}

impl GenAiSpanDBEngine {
    pub async fn new(
        object_store: &ObjectStore,
        ctx: Arc<SessionContext>,
        catalog: Arc<TraceCatalogProvider>,
    ) -> Result<Self, TraceEngineError> {
        let schema = Arc::new(create_genai_schema());
        let delta_table = build_or_create_genai_table(object_store, schema.clone()).await?;
        if let Ok(provider) = delta_table.table_provider().await {
            catalog.swap(GEN_AI_TABLE_NAME, provider);
        } else {
            info!("Empty gen_ai_spans table at init — deferring catalog registration until first write");
        }

        let control = ControlTableEngine::new(object_store, get_pod_id()).await?;

        Ok(GenAiSpanDBEngine {
            schema,
            table: Arc::new(AsyncRwLock::new(delta_table)),
            ctx,
            catalog,
            control,
        })
    }

    fn build_writer_props() -> WriterProperties {
        WriterProperties::builder()
            .set_max_row_group_size(32_768)
            // Bloom filter on trace_id
            .set_column_bloom_filter_enabled(ColumnPath::new(vec![TRACE_ID_COL.to_string()]), true)
            .set_column_bloom_filter_fpp(ColumnPath::new(vec![TRACE_ID_COL.to_string()]), 0.01)
            .set_column_bloom_filter_ndv(ColumnPath::new(vec![TRACE_ID_COL.to_string()]), 32_768)
            // Bloom filter on service_name
            .set_column_bloom_filter_enabled(
                ColumnPath::new(vec![SERVICE_NAME_COL.to_string()]),
                true,
            )
            .set_column_bloom_filter_fpp(
                ColumnPath::new(vec![SERVICE_NAME_COL.to_string()]),
                0.01,
            )
            .set_column_bloom_filter_ndv(ColumnPath::new(vec![SERVICE_NAME_COL.to_string()]), 256)
            // Bloom filter on conversation_id
            .set_column_bloom_filter_enabled(
                ColumnPath::new(vec![CONVERSATION_ID_COL.to_string()]),
                true,
            )
            .set_column_bloom_filter_fpp(
                ColumnPath::new(vec![CONVERSATION_ID_COL.to_string()]),
                0.01,
            )
            .set_column_bloom_filter_ndv(
                ColumnPath::new(vec![CONVERSATION_ID_COL.to_string()]),
                8_192,
            )
            // Delta encoding on near-sorted integer columns
            .set_column_encoding(
                ColumnPath::new(vec![START_TIME_COL.to_string()]),
                Encoding::DELTA_BINARY_PACKED,
            )
            .set_column_encoding(
                ColumnPath::new(vec![DURATION_MS_COL.to_string()]),
                Encoding::DELTA_BINARY_PACKED,
            )
            .set_column_encoding(
                ColumnPath::new(vec![INPUT_TOKENS_COL.to_string()]),
                Encoding::DELTA_BINARY_PACKED,
            )
            .set_column_encoding(
                ColumnPath::new(vec![OUTPUT_TOKENS_COL.to_string()]),
                Encoding::DELTA_BINARY_PACKED,
            )
            // ZSTD level 3 compression
            .set_compression(Compression::ZSTD(ZstdLevel::try_new(3).unwrap()))
            .build()
    }

    fn build_batch(
        &self,
        records: Vec<GenAiSpanRecord>,
    ) -> Result<RecordBatch, TraceEngineError> {
        let mut builder = GenAiBatchBuilder::new(self.schema.clone(), records.len());
        for rec in &records {
            builder.append(rec)?;
        }
        builder.finish()
    }

    async fn write_records(
        &self,
        records: Vec<GenAiSpanRecord>,
    ) -> Result<(), TraceEngineError> {
        let count = records.len();
        info!("Writing {} gen_ai spans", count);
        let batch = self.build_batch(records)?;

        let mut table_guard = self.table.write().await;

        let current_table = table_guard.clone();
        let updated_table = current_table
            .write(vec![batch])
            .with_save_mode(deltalake::protocol::SaveMode::Append)
            .with_writer_properties(Self::build_writer_props())
            .with_partition_columns(vec![PARTITION_DATE_COL.to_string()])
            .await?;

        let new_provider = updated_table.table_provider().await?;
        self.catalog.swap(GEN_AI_TABLE_NAME, new_provider);
        updated_table.update_datafusion_session(&self.ctx.state())?;

        *table_guard = updated_table;
        info!("gen_ai_spans table updated with {} records", count);
        Ok(())
    }

    async fn optimize_table(&self) -> Result<(), TraceEngineError> {
        let mut table_guard = self.table.write().await;
        let (updated_table, _metrics) = table_guard
            .clone()
            .optimize()
            .with_target_size(std::num::NonZero::new(128 * 1024 * 1024).unwrap())
            .with_type(OptimizeType::ZOrder(vec![
                START_TIME_COL.to_string(),
                SERVICE_NAME_COL.to_string(),
            ]))
            .with_writer_properties(Self::build_writer_props())
            .await?;

        self.catalog
            .swap(GEN_AI_TABLE_NAME, updated_table.table_provider().await?);
        updated_table.update_datafusion_session(&self.ctx.state())?;
        *table_guard = updated_table;
        Ok(())
    }

    async fn vacuum_table(&self, retention_hours: u64) -> Result<(), TraceEngineError> {
        let mut table_guard = self.table.write().await;
        let (updated_table, _metrics) = table_guard
            .clone()
            .vacuum()
            .with_retention_period(chrono::Duration::hours(retention_hours as i64))
            .with_enforce_retention_duration(false)
            .await?;

        self.catalog
            .swap(GEN_AI_TABLE_NAME, updated_table.table_provider().await?);
        updated_table.update_datafusion_session(&self.ctx.state())?;
        *table_guard = updated_table;
        Ok(())
    }

    async fn expire_table(&self, cutoff_date: chrono::NaiveDate) -> Result<(), TraceEngineError> {
        use chrono::TimeZone;
        let mut table_guard = self.table.write().await;
        let cutoff_dt =
            chrono::Utc.from_utc_datetime(&cutoff_date.and_hms_opt(0, 0, 0).unwrap());
        let predicate = col(PARTITION_DATE_COL).lt(date_lit(&cutoff_dt));
        let (updated_table, metrics) = table_guard
            .clone()
            .delete()
            .with_predicate(predicate)
            .await?;

        info!(
            "Expired {} gen_ai rows older than {}",
            metrics.num_deleted_rows, cutoff_date
        );

        self.catalog
            .swap(GEN_AI_TABLE_NAME, updated_table.table_provider().await?);
        updated_table.update_datafusion_session(&self.ctx.state())?;
        *table_guard = updated_table;
        Ok(())
    }

    async fn refresh_table(&self) -> Result<(), TraceEngineError> {
        let mut table_guard = self.table.write().await;
        let current_version = table_guard.version();

        let mut refreshed = table_guard.clone();
        match refreshed.update_incremental(None).await {
            Ok(_) => {
                if refreshed.version() > current_version {
                    info!(
                        "gen_ai_spans table refreshed: v{:?} → v{:?}",
                        current_version,
                        refreshed.version()
                    );
                    let new_provider = refreshed.table_provider().await?;
                    self.catalog.swap(GEN_AI_TABLE_NAME, new_provider);
                    refreshed.update_datafusion_session(&self.ctx.state())?;
                    *table_guard = refreshed;
                }
            }
            Err(e) => {
                debug!("gen_ai_spans table refresh skipped: {}", e);
            }
        }
        Ok(())
    }

    async fn try_run_optimize(&self, interval_hours: u64) {
        match self.control.try_claim_task(TASK_GENAI_OPTIMIZE).await {
            Ok(true) => match self.optimize_table().await {
                Ok(()) => {
                    if let Err(e) = self.vacuum_table(0).await {
                        error!("Post-optimize vacuum failed (gen_ai): {}", e);
                    }
                    let _ = self
                        .control
                        .release_task(
                            TASK_GENAI_OPTIMIZE,
                            chrono::Duration::hours(interval_hours as i64),
                        )
                        .await;
                }
                Err(e) => {
                    error!("gen_ai optimize failed: {}", e);
                    let _ = self
                        .control
                        .release_task_on_failure(TASK_GENAI_OPTIMIZE)
                        .await;
                }
            },
            Ok(false) => {}
            Err(e) => error!("gen_ai optimize claim check failed: {}", e),
        }
    }

    async fn try_run_retention(&self, retention_days: u32) {
        match self.control.try_claim_task(TASK_GENAI_RETENTION).await {
            Ok(true) => {
                let cutoff =
                    (Utc::now() - chrono::Duration::days(retention_days as i64)).date_naive();
                match self.expire_table(cutoff).await {
                    Ok(()) => {
                        if let Err(e) = self.vacuum_table(0).await {
                            error!("Failed to vacuum after retention: {}", e);
                        }
                        let _ = self
                            .control
                            .release_task(TASK_GENAI_RETENTION, chrono::Duration::hours(24))
                            .await;
                    }
                    Err(e) => {
                        error!("gen_ai retention failed: {}", e);
                        let _ = self
                            .control
                            .release_task_on_failure(TASK_GENAI_RETENTION)
                            .await;
                    }
                }
            }
            Ok(false) => {}
            Err(e) => error!("gen_ai retention claim check failed: {}", e),
        }
    }

    #[instrument(skip_all, name = "genai_engine_actor")]
    pub fn start_actor(
        self,
        compaction_interval_hours: u64,
        retention_days: Option<u32>,
        refresh_interval_secs: u64,
    ) -> (
        mpsc::Sender<GenAiTableCommand>,
        tokio::task::JoinHandle<()>,
    ) {
        let (tx, mut rx) = mpsc::channel::<GenAiTableCommand>(10_000);

        let handle = tokio::spawn(async move {
            info!(refresh_interval_secs, "GenAiSpanDBEngine actor started");

            let mut scheduler_ticker = interval(Duration::from_secs(5 * 60));
            scheduler_ticker.tick().await;

            let mut refresh_ticker = interval(Duration::from_secs(refresh_interval_secs.max(1)));
            refresh_ticker.tick().await;

            loop {
                tokio::select! {
                    Some(cmd) = rx.recv() => {
                        match cmd {
                            GenAiTableCommand::Write { records, respond_to } => {
                                let result = self.write_records(records).await;
                                if let Err(ref e) = result {
                                    error!("gen_ai write failed: {}", e);
                                }
                                let _ = respond_to.send(result);
                            }
                            GenAiTableCommand::Optimize { respond_to } => {
                                let _ = respond_to.send(self.optimize_table().await);
                                if let Err(e) = self.vacuum_table(0).await {
                                    error!("Post-optimize vacuum failed (gen_ai): {}", e);
                                }
                            }
                            GenAiTableCommand::Vacuum { retention_hours, respond_to } => {
                                let _ = respond_to.send(self.vacuum_table(retention_hours).await);
                            }
                            GenAiTableCommand::Expire { cutoff_date, respond_to } => {
                                let _ = respond_to.send(self.expire_table(cutoff_date).await);
                            }
                            GenAiTableCommand::Shutdown => {
                                info!("GenAiSpanDBEngine actor shutting down");
                                break;
                            }
                        }
                    }
                    _ = scheduler_ticker.tick() => {
                        self.try_run_optimize(compaction_interval_hours).await;
                        if let Some(days) = retention_days {
                            self.try_run_retention(days).await;
                        }
                    }
                    _ = refresh_ticker.tick() => {
                        if let Err(e) = self.refresh_table().await {
                            error!("gen_ai table refresh failed: {}", e);
                        }
                    }
                }
            }
        });

        (tx, handle)
    }
}

// ── Service ──────────────────────────────────────────────────────────────────

pub struct GenAiSpanService {
    pub engine_tx: mpsc::Sender<GenAiTableCommand>,
    engine_handle: tokio::task::JoinHandle<()>,
    pub query_service: GenAiQueries,
}

impl GenAiSpanService {
    pub async fn new(
        object_store: &ObjectStore,
        compaction_interval_hours: u64,
        ctx: Arc<SessionContext>,
        catalog: Arc<TraceCatalogProvider>,
        refresh_interval_secs: u64,
        retention_days: Option<u32>,
    ) -> Result<Self, TraceEngineError> {
        let engine =
            GenAiSpanDBEngine::new(object_store, ctx.clone(), catalog).await?;
        let (engine_tx, engine_handle) =
            engine.start_actor(compaction_interval_hours, retention_days, refresh_interval_secs);

        Ok(GenAiSpanService {
            engine_tx,
            engine_handle,
            query_service: GenAiQueries::new(ctx),
        })
    }

    pub async fn write_records(
        &self,
        records: Vec<GenAiSpanRecord>,
    ) -> Result<(), TraceEngineError> {
        let (tx, rx) = oneshot::channel();
        self.engine_tx
            .send(GenAiTableCommand::Write {
                records,
                respond_to: tx,
            })
            .await
            .map_err(|_| TraceEngineError::ChannelClosed)?;
        rx.await.map_err(|_| TraceEngineError::ChannelClosed)?
    }

    pub async fn optimize(&self) -> Result<(), TraceEngineError> {
        let (tx, rx) = oneshot::channel();
        self.engine_tx
            .send(GenAiTableCommand::Optimize { respond_to: tx })
            .await
            .map_err(|_| TraceEngineError::ChannelClosed)?;
        rx.await.map_err(|_| TraceEngineError::ChannelClosed)?
    }

    pub async fn signal_shutdown(&self) {
        info!("GenAiSpanService signaling shutdown");
        let _ = self.engine_tx.send(GenAiTableCommand::Shutdown).await;
    }

    pub async fn shutdown(self) -> Result<(), TraceEngineError> {
        info!("GenAiSpanService shutting down");
        self.engine_tx
            .send(GenAiTableCommand::Shutdown)
            .await
            .map_err(|_| TraceEngineError::ChannelClosed)?;
        if let Err(e) = self.engine_handle.await {
            error!("GenAi engine handle error: {}", e);
        }
        info!("GenAiSpanService shutdown complete");
        Ok(())
    }
}

// ── Queries ──────────────────────────────────────────────────────────────────

fn cache_key<H: Hash>(params: &H) -> u64 {
    let mut hasher = AHasher::default();
    params.hash(&mut hasher);
    hasher.finish()
}

#[derive(Clone)]
pub struct GenAiQueries {
    ctx: Arc<SessionContext>,
    metrics_cache: Cache<u64, Arc<Vec<GenAiTokenBucket>>>,
    model_cache: Cache<u64, Arc<Vec<GenAiModelUsage>>>,
}

impl GenAiQueries {
    pub fn new(ctx: Arc<SessionContext>) -> Self {
        Self {
            ctx,
            metrics_cache: Cache::builder()
                .max_capacity(64)
                .time_to_live(Duration::from_secs(30))
                .build(),
            model_cache: Cache::builder()
                .max_capacity(64)
                .time_to_live(Duration::from_secs(30))
                .build(),
        }
    }

    fn apply_time_filters(
        df: DataFrame,
        start: &DateTime<Utc>,
        end: &DateTime<Utc>,
    ) -> Result<DataFrame, TraceEngineError> {
        let df = df.filter(col(PARTITION_DATE_COL).gt_eq(date_lit(start)))?;
        let df = df.filter(col(PARTITION_DATE_COL).lt_eq(date_lit(end)))?;
        let df = df.filter(col(START_TIME_COL).gt_eq(ts_lit(start)))?;
        let df = df.filter(col(START_TIME_COL).lt(ts_lit(end)))?;
        Ok(df)
    }

    fn apply_optional_filter(
        df: DataFrame,
        column: &str,
        value: Option<&str>,
    ) -> Result<DataFrame, TraceEngineError> {
        match value {
            Some(v) => Ok(df.filter(col(column).eq(lit(v)))?),
            None => Ok(df),
        }
    }

    pub async fn get_token_metrics(
        &self,
        service_name: Option<&str>,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        bucket_interval: &str,
        operation_name: Option<&str>,
        provider_name: Option<&str>,
    ) -> Result<Vec<GenAiTokenBucket>, TraceEngineError> {
        let key = cache_key(&(
            "token_metrics",
            service_name,
            start.timestamp_micros(),
            end.timestamp_micros(),
            bucket_interval,
            operation_name,
            provider_name,
        ));
        if let Some(cached) = self.metrics_cache.get(&key) {
            return Ok((*cached).clone());
        }

        const VALID_INTERVALS: &[&str] =
            &["second", "minute", "hour", "day", "week", "month", "year"];
        if !VALID_INTERVALS.contains(&bucket_interval) {
            return Err(TraceEngineError::UnsupportedOperation(format!(
                "Invalid bucket_interval '{}'. Must be one of: {}",
                bucket_interval,
                VALID_INTERVALS.join(", ")
            )));
        }

        let df = self.ctx.table(GEN_AI_TABLE_NAME).await?;
        let df = Self::apply_time_filters(df, &start, &end)?;
        let df = Self::apply_optional_filter(df, SERVICE_NAME_COL, service_name)?;
        let df = Self::apply_optional_filter(df, OPERATION_NAME_COL, operation_name)?;
        let df = Self::apply_optional_filter(df, PROVIDER_NAME_COL, provider_name)?;

        let df = df.with_column(
            "bucket_start",
            datafusion::functions::expr_fn::date_trunc(
                lit(bucket_interval),
                col(START_TIME_COL),
            ),
        )?;

        let df = df.aggregate(
            vec![col("bucket_start")],
            vec![
                sum(col(INPUT_TOKENS_COL)).alias("total_input_tokens"),
                sum(col(OUTPUT_TOKENS_COL)).alias("total_output_tokens"),
                sum(col(CACHE_CREATION_INPUT_TOKENS_COL)).alias("total_cache_creation_tokens"),
                sum(col(CACHE_READ_INPUT_TOKENS_COL)).alias("total_cache_read_tokens"),
                count(lit(1)).alias("span_count"),
                avg(datafusion::logical_expr::cast(
                    col(ERROR_TYPE_COL).is_not_null(),
                    DataType::Float64,
                ))
                .alias("error_rate"),
            ],
        )?;

        let df = df.sort(vec![col("bucket_start").sort(true, true)])?;
        let batches = df.collect().await?;

        let mut results = Vec::new();
        for batch in &batches {
            let raw_bucket = batch.column_by_name("bucket_start").ok_or_else(|| {
                TraceEngineError::UnsupportedOperation("missing bucket_start column".into())
            })?;
            let bucket_casted = compute::cast(
                raw_bucket,
                &DataType::Timestamp(TimeUnit::Microsecond, Some("UTC".into())),
            )?;
            let bucket_starts = bucket_casted
                .as_any()
                .downcast_ref::<TimestampMicrosecondArray>()
                .ok_or_else(|| {
                    TraceEngineError::UnsupportedOperation(
                        "bucket_start cast to TimestampMicrosecondArray failed".into(),
                    )
                })?;
            let input_tokens = batch
                .column_by_name("total_input_tokens")
                .and_then(|c| c.as_any().downcast_ref::<Int64Array>())
                .ok_or_else(|| {
                    TraceEngineError::UnsupportedOperation(
                        "missing total_input_tokens column".into(),
                    )
                })?;
            let output_tokens = batch
                .column_by_name("total_output_tokens")
                .and_then(|c| c.as_any().downcast_ref::<Int64Array>())
                .ok_or_else(|| {
                    TraceEngineError::UnsupportedOperation(
                        "missing total_output_tokens column".into(),
                    )
                })?;
            let cache_creation = batch
                .column_by_name("total_cache_creation_tokens")
                .and_then(|c| c.as_any().downcast_ref::<Int64Array>())
                .ok_or_else(|| {
                    TraceEngineError::UnsupportedOperation(
                        "missing total_cache_creation_tokens column".into(),
                    )
                })?;
            let cache_read = batch
                .column_by_name("total_cache_read_tokens")
                .and_then(|c| c.as_any().downcast_ref::<Int64Array>())
                .ok_or_else(|| {
                    TraceEngineError::UnsupportedOperation(
                        "missing total_cache_read_tokens column".into(),
                    )
                })?;
            let span_counts = batch
                .column_by_name("span_count")
                .and_then(|c| c.as_any().downcast_ref::<Int64Array>())
                .ok_or_else(|| {
                    TraceEngineError::UnsupportedOperation("missing span_count column".into())
                })?;
            let error_rates = batch
                .column_by_name("error_rate")
                .and_then(|c| c.as_any().downcast_ref::<Float64Array>())
                .ok_or_else(|| {
                    TraceEngineError::UnsupportedOperation("missing error_rate column".into())
                })?;

            for i in 0..batch.num_rows() {
                let bucket_start = DateTime::from_timestamp_micros(bucket_starts.value(i))
                    .ok_or(TraceEngineError::InvalidTimestamp(
                        "out-of-range bucket_start timestamp",
                    ))?;
                results.push(GenAiTokenBucket {
                    bucket_start,
                    total_input_tokens: if input_tokens.is_null(i) {
                        0
                    } else {
                        input_tokens.value(i)
                    },
                    total_output_tokens: if output_tokens.is_null(i) {
                        0
                    } else {
                        output_tokens.value(i)
                    },
                    total_cache_creation_tokens: if cache_creation.is_null(i) {
                        0
                    } else {
                        cache_creation.value(i)
                    },
                    total_cache_read_tokens: if cache_read.is_null(i) {
                        0
                    } else {
                        cache_read.value(i)
                    },
                    span_count: span_counts.value(i),
                    error_rate: if error_rates.is_null(i) {
                        0.0
                    } else {
                        error_rates.value(i)
                    },
                });
            }
        }

        self.metrics_cache.insert(key, Arc::new(results.clone()));
        Ok(results)
    }

    pub async fn get_operation_breakdown(
        &self,
        service_name: Option<&str>,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        provider_name: Option<&str>,
    ) -> Result<Vec<GenAiOperationBreakdown>, TraceEngineError> {
        let df = self.ctx.table(GEN_AI_TABLE_NAME).await?;
        let df = Self::apply_time_filters(df, &start, &end)?;
        let df = Self::apply_optional_filter(df, SERVICE_NAME_COL, service_name)?;
        let df = Self::apply_optional_filter(df, PROVIDER_NAME_COL, provider_name)?;

        let df = df.aggregate(
            vec![col(OPERATION_NAME_COL), col(PROVIDER_NAME_COL)],
            vec![
                count(lit(1)).alias("span_count"),
                avg(col(DURATION_MS_COL)).alias("avg_duration_ms"),
                sum(col(INPUT_TOKENS_COL)).alias("total_input_tokens"),
                sum(col(OUTPUT_TOKENS_COL)).alias("total_output_tokens"),
                avg(datafusion::logical_expr::cast(
                    col(ERROR_TYPE_COL).is_not_null(),
                    DataType::Float64,
                ))
                .alias("error_rate"),
            ],
        )?;

        let batches = df.collect().await?;
        let mut results = Vec::new();
        for batch in &batches {
            let op_arr = compute::cast(
                batch.column_by_name(OPERATION_NAME_COL).ok_or_else(|| {
                    TraceEngineError::UnsupportedOperation(
                        "missing operation_name column".into(),
                    )
                })?,
                &DataType::Utf8,
            )?;
            let op_names = op_arr
                .as_any()
                .downcast_ref::<StringArray>()
                .ok_or_else(|| {
                    TraceEngineError::UnsupportedOperation(
                        "operation_name cast to StringArray failed".into(),
                    )
                })?;

            let prov_arr = compute::cast(
                batch.column_by_name(PROVIDER_NAME_COL).ok_or_else(|| {
                    TraceEngineError::UnsupportedOperation(
                        "missing provider_name column".into(),
                    )
                })?,
                &DataType::Utf8,
            )?;
            let prov_names = prov_arr
                .as_any()
                .downcast_ref::<StringArray>()
                .ok_or_else(|| {
                    TraceEngineError::UnsupportedOperation(
                        "provider_name cast to StringArray failed".into(),
                    )
                })?;

            let span_counts = batch
                .column_by_name("span_count")
                .and_then(|c| c.as_any().downcast_ref::<Int64Array>())
                .ok_or_else(|| {
                    TraceEngineError::UnsupportedOperation("missing span_count column".into())
                })?;
            let avg_durations = batch
                .column_by_name("avg_duration_ms")
                .and_then(|c| c.as_any().downcast_ref::<Float64Array>())
                .ok_or_else(|| {
                    TraceEngineError::UnsupportedOperation("missing avg_duration_ms column".into())
                })?;
            let input_tokens = batch
                .column_by_name("total_input_tokens")
                .and_then(|c| c.as_any().downcast_ref::<Int64Array>())
                .ok_or_else(|| {
                    TraceEngineError::UnsupportedOperation(
                        "missing total_input_tokens column".into(),
                    )
                })?;
            let output_tokens = batch
                .column_by_name("total_output_tokens")
                .and_then(|c| c.as_any().downcast_ref::<Int64Array>())
                .ok_or_else(|| {
                    TraceEngineError::UnsupportedOperation(
                        "missing total_output_tokens column".into(),
                    )
                })?;
            let error_rates = batch
                .column_by_name("error_rate")
                .and_then(|c| c.as_any().downcast_ref::<Float64Array>())
                .ok_or_else(|| {
                    TraceEngineError::UnsupportedOperation("missing error_rate column".into())
                })?;

            for i in 0..batch.num_rows() {
                results.push(GenAiOperationBreakdown {
                    operation_name: if op_names.is_null(i) {
                        String::new()
                    } else {
                        op_names.value(i).to_string()
                    },
                    provider_name: if prov_names.is_null(i) {
                        None
                    } else {
                        Some(prov_names.value(i).to_string())
                    },
                    span_count: span_counts.value(i),
                    avg_duration_ms: if avg_durations.is_null(i) {
                        0.0
                    } else {
                        avg_durations.value(i)
                    },
                    total_input_tokens: if input_tokens.is_null(i) {
                        0
                    } else {
                        input_tokens.value(i)
                    },
                    total_output_tokens: if output_tokens.is_null(i) {
                        0
                    } else {
                        output_tokens.value(i)
                    },
                    error_rate: if error_rates.is_null(i) {
                        0.0
                    } else {
                        error_rates.value(i)
                    },
                });
            }
        }
        Ok(results)
    }

    pub async fn get_model_usage(
        &self,
        service_name: Option<&str>,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        provider_name: Option<&str>,
    ) -> Result<Vec<GenAiModelUsage>, TraceEngineError> {
        let key = cache_key(&(
            "model_usage",
            service_name,
            start.timestamp_micros(),
            end.timestamp_micros(),
            provider_name,
        ));
        if let Some(cached) = self.model_cache.get(&key) {
            return Ok((*cached).clone());
        }

        let df = self.ctx.table(GEN_AI_TABLE_NAME).await?;
        let df = Self::apply_time_filters(df, &start, &end)?;
        let df = Self::apply_optional_filter(df, SERVICE_NAME_COL, service_name)?;
        let df = Self::apply_optional_filter(df, PROVIDER_NAME_COL, provider_name)?;

        let df = df.with_column(
            "model",
            datafusion::functions::core::expr_fn::coalesce(vec![
                col(RESPONSE_MODEL_COL),
                col(REQUEST_MODEL_COL),
            ]),
        )?;

        let df = df.aggregate(
            vec![col("model"), col(PROVIDER_NAME_COL)],
            vec![
                count(lit(1)).alias("span_count"),
                sum(col(INPUT_TOKENS_COL)).alias("total_input_tokens"),
                sum(col(OUTPUT_TOKENS_COL)).alias("total_output_tokens"),
                datafusion::functions_aggregate::expr_fn::approx_percentile_cont(
                    datafusion::logical_expr::expr::Sort {
                        expr: col(DURATION_MS_COL),
                        asc: true,
                        nulls_first: false,
                    },
                    lit(0.5_f64),
                    None,
                )
                .alias("p50_duration_ms"),
                datafusion::functions_aggregate::expr_fn::approx_percentile_cont(
                    datafusion::logical_expr::expr::Sort {
                        expr: col(DURATION_MS_COL),
                        asc: true,
                        nulls_first: false,
                    },
                    lit(0.95_f64),
                    None,
                )
                .alias("p95_duration_ms"),
                avg(datafusion::logical_expr::cast(
                    col(ERROR_TYPE_COL).is_not_null(),
                    DataType::Float64,
                ))
                .alias("error_rate"),
            ],
        )?;

        let batches = df.collect().await?;
        let mut results = Vec::new();
        for batch in &batches {
            let model_arr = compute::cast(
                batch
                    .column_by_name("model")
                    .ok_or_else(|| {
                        TraceEngineError::UnsupportedOperation("missing model column".into())
                    })?,
                &DataType::Utf8,
            )?;
            let models = model_arr
                .as_any()
                .downcast_ref::<StringArray>()
                .ok_or_else(|| {
                    TraceEngineError::UnsupportedOperation(
                        "model cast to StringArray failed".into(),
                    )
                })?;

            let prov_arr = compute::cast(
                batch.column_by_name(PROVIDER_NAME_COL).ok_or_else(|| {
                    TraceEngineError::UnsupportedOperation(
                        "missing provider_name column".into(),
                    )
                })?,
                &DataType::Utf8,
            )?;
            let prov_names = prov_arr
                .as_any()
                .downcast_ref::<StringArray>()
                .ok_or_else(|| {
                    TraceEngineError::UnsupportedOperation(
                        "provider_name cast to StringArray failed".into(),
                    )
                })?;

            let span_counts = batch
                .column_by_name("span_count")
                .and_then(|c| c.as_any().downcast_ref::<Int64Array>())
                .ok_or_else(|| {
                    TraceEngineError::UnsupportedOperation("missing span_count column".into())
                })?;
            let input_tokens = batch
                .column_by_name("total_input_tokens")
                .and_then(|c| c.as_any().downcast_ref::<Int64Array>())
                .ok_or_else(|| {
                    TraceEngineError::UnsupportedOperation(
                        "missing total_input_tokens column".into(),
                    )
                })?;
            let output_tokens = batch
                .column_by_name("total_output_tokens")
                .and_then(|c| c.as_any().downcast_ref::<Int64Array>())
                .ok_or_else(|| {
                    TraceEngineError::UnsupportedOperation(
                        "missing total_output_tokens column".into(),
                    )
                })?;
            let p50 = batch
                .column_by_name("p50_duration_ms")
                .and_then(|c| c.as_any().downcast_ref::<Float64Array>());
            let p95 = batch
                .column_by_name("p95_duration_ms")
                .and_then(|c| c.as_any().downcast_ref::<Float64Array>());
            let error_rates = batch
                .column_by_name("error_rate")
                .and_then(|c| c.as_any().downcast_ref::<Float64Array>())
                .ok_or_else(|| {
                    TraceEngineError::UnsupportedOperation("missing error_rate column".into())
                })?;

            for i in 0..batch.num_rows() {
                results.push(GenAiModelUsage {
                    model: if models.is_null(i) {
                        "unknown".to_string()
                    } else {
                        models.value(i).to_string()
                    },
                    provider_name: if prov_names.is_null(i) {
                        None
                    } else {
                        Some(prov_names.value(i).to_string())
                    },
                    span_count: span_counts.value(i),
                    total_input_tokens: if input_tokens.is_null(i) {
                        0
                    } else {
                        input_tokens.value(i)
                    },
                    total_output_tokens: if output_tokens.is_null(i) {
                        0
                    } else {
                        output_tokens.value(i)
                    },
                    p50_duration_ms: p50.and_then(|a| {
                        if a.is_null(i) {
                            None
                        } else {
                            Some(a.value(i))
                        }
                    }),
                    p95_duration_ms: p95.and_then(|a| {
                        if a.is_null(i) {
                            None
                        } else {
                            Some(a.value(i))
                        }
                    }),
                    error_rate: if error_rates.is_null(i) {
                        0.0
                    } else {
                        error_rates.value(i)
                    },
                });
            }
        }

        self.model_cache.insert(key, Arc::new(results.clone()));
        Ok(results)
    }

    pub async fn get_agent_activity(
        &self,
        service_name: Option<&str>,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        agent_name: Option<&str>,
    ) -> Result<Vec<GenAiAgentActivity>, TraceEngineError> {
        let df = self.ctx.table(GEN_AI_TABLE_NAME).await?;
        let df = Self::apply_time_filters(df, &start, &end)?;
        let df = Self::apply_optional_filter(df, SERVICE_NAME_COL, service_name)?;

        // Filter: operation is agent-related OR agent_name is present
        let df = df.filter(
            col(OPERATION_NAME_COL)
                .in_list(
                    vec![lit("invoke_agent"), lit("create_agent")],
                    false,
                )
                .or(col(AGENT_NAME_COL).is_not_null()),
        )?;

        let df = Self::apply_optional_filter(df, AGENT_NAME_COL, agent_name)?;

        let df = df.aggregate(
            vec![
                col(AGENT_NAME_COL),
                col(AGENT_ID_COL),
                col(CONVERSATION_ID_COL),
            ],
            vec![
                count(lit(1)).alias("span_count"),
                sum(col(INPUT_TOKENS_COL)).alias("total_input_tokens"),
                sum(col(OUTPUT_TOKENS_COL)).alias("total_output_tokens"),
                datafusion::functions_aggregate::expr_fn::max(col(START_TIME_COL))
                    .alias("last_seen"),
            ],
        )?;

        let batches = df.collect().await?;
        let mut results = Vec::new();
        for batch in &batches {
            let agent_names_arr = compute::cast(
                batch.column_by_name(AGENT_NAME_COL).ok_or_else(|| {
                    TraceEngineError::UnsupportedOperation("missing agent_name column".into())
                })?,
                &DataType::Utf8,
            )?;
            let agent_names = agent_names_arr
                .as_any()
                .downcast_ref::<StringArray>()
                .ok_or_else(|| {
                    TraceEngineError::UnsupportedOperation(
                        "agent_name cast to StringArray failed".into(),
                    )
                })?;

            let agent_ids_arr = compute::cast(
                batch.column_by_name(AGENT_ID_COL).ok_or_else(|| {
                    TraceEngineError::UnsupportedOperation("missing agent_id column".into())
                })?,
                &DataType::Utf8,
            )?;
            let agent_ids = agent_ids_arr
                .as_any()
                .downcast_ref::<StringArray>()
                .ok_or_else(|| {
                    TraceEngineError::UnsupportedOperation(
                        "agent_id cast to StringArray failed".into(),
                    )
                })?;

            let conv_ids_arr = compute::cast(
                batch
                    .column_by_name(CONVERSATION_ID_COL)
                    .ok_or_else(|| {
                        TraceEngineError::UnsupportedOperation(
                            "missing conversation_id column".into(),
                        )
                    })?,
                &DataType::Utf8,
            )?;
            let conv_ids = conv_ids_arr
                .as_any()
                .downcast_ref::<StringArray>()
                .ok_or_else(|| {
                    TraceEngineError::UnsupportedOperation(
                        "conversation_id cast to StringArray failed".into(),
                    )
                })?;

            let span_counts = batch
                .column_by_name("span_count")
                .and_then(|c| c.as_any().downcast_ref::<Int64Array>())
                .ok_or_else(|| {
                    TraceEngineError::UnsupportedOperation("missing span_count column".into())
                })?;
            let input_tokens = batch
                .column_by_name("total_input_tokens")
                .and_then(|c| c.as_any().downcast_ref::<Int64Array>())
                .ok_or_else(|| {
                    TraceEngineError::UnsupportedOperation(
                        "missing total_input_tokens column".into(),
                    )
                })?;
            let output_tokens = batch
                .column_by_name("total_output_tokens")
                .and_then(|c| c.as_any().downcast_ref::<Int64Array>())
                .ok_or_else(|| {
                    TraceEngineError::UnsupportedOperation(
                        "missing total_output_tokens column".into(),
                    )
                })?;
            let last_seens = batch
                .column_by_name("last_seen")
                .and_then(|c| c.as_any().downcast_ref::<TimestampMicrosecondArray>());

            for i in 0..batch.num_rows() {
                results.push(GenAiAgentActivity {
                    agent_name: if agent_names.is_null(i) {
                        None
                    } else {
                        Some(agent_names.value(i).to_string())
                    },
                    agent_id: if agent_ids.is_null(i) {
                        None
                    } else {
                        Some(agent_ids.value(i).to_string())
                    },
                    conversation_id: if conv_ids.is_null(i) {
                        None
                    } else {
                        Some(conv_ids.value(i).to_string())
                    },
                    span_count: span_counts.value(i),
                    total_input_tokens: if input_tokens.is_null(i) {
                        0
                    } else {
                        input_tokens.value(i)
                    },
                    total_output_tokens: if output_tokens.is_null(i) {
                        0
                    } else {
                        output_tokens.value(i)
                    },
                    last_seen: last_seens.and_then(|a| {
                        if a.is_null(i) {
                            None
                        } else {
                            DateTime::from_timestamp_micros(a.value(i))
                        }
                    }),
                });
            }
        }
        Ok(results)
    }

    pub async fn get_tool_activity(
        &self,
        service_name: Option<&str>,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<GenAiToolActivity>, TraceEngineError> {
        let df = self.ctx.table(GEN_AI_TABLE_NAME).await?;
        let df = Self::apply_time_filters(df, &start, &end)?;
        let df = Self::apply_optional_filter(df, SERVICE_NAME_COL, service_name)?;

        let df = df.filter(
            col(TOOL_NAME_COL)
                .is_not_null()
                .or(col(OPERATION_NAME_COL).eq(lit("execute_tool"))),
        )?;

        let df = df.aggregate(
            vec![col(TOOL_NAME_COL), col(TOOL_TYPE_COL)],
            vec![
                count(lit(1)).alias("call_count"),
                avg(col(DURATION_MS_COL)).alias("avg_duration_ms"),
                avg(datafusion::logical_expr::cast(
                    col(ERROR_TYPE_COL).is_not_null(),
                    DataType::Float64,
                ))
                .alias("error_rate"),
            ],
        )?;

        let batches = df.collect().await?;
        let mut results = Vec::new();
        for batch in &batches {
            let tool_names_arr = compute::cast(
                batch.column_by_name(TOOL_NAME_COL).ok_or_else(|| {
                    TraceEngineError::UnsupportedOperation("missing tool_name column".into())
                })?,
                &DataType::Utf8,
            )?;
            let tool_names = tool_names_arr
                .as_any()
                .downcast_ref::<StringArray>()
                .ok_or_else(|| {
                    TraceEngineError::UnsupportedOperation(
                        "tool_name cast to StringArray failed".into(),
                    )
                })?;

            let tool_types_arr = compute::cast(
                batch.column_by_name(TOOL_TYPE_COL).ok_or_else(|| {
                    TraceEngineError::UnsupportedOperation("missing tool_type column".into())
                })?,
                &DataType::Utf8,
            )?;
            let tool_types = tool_types_arr
                .as_any()
                .downcast_ref::<StringArray>()
                .ok_or_else(|| {
                    TraceEngineError::UnsupportedOperation(
                        "tool_type cast to StringArray failed".into(),
                    )
                })?;

            let call_counts = batch
                .column_by_name("call_count")
                .and_then(|c| c.as_any().downcast_ref::<Int64Array>())
                .ok_or_else(|| {
                    TraceEngineError::UnsupportedOperation("missing call_count column".into())
                })?;
            let avg_durations = batch
                .column_by_name("avg_duration_ms")
                .and_then(|c| c.as_any().downcast_ref::<Float64Array>())
                .ok_or_else(|| {
                    TraceEngineError::UnsupportedOperation("missing avg_duration_ms column".into())
                })?;
            let error_rates = batch
                .column_by_name("error_rate")
                .and_then(|c| c.as_any().downcast_ref::<Float64Array>())
                .ok_or_else(|| {
                    TraceEngineError::UnsupportedOperation("missing error_rate column".into())
                })?;

            for i in 0..batch.num_rows() {
                results.push(GenAiToolActivity {
                    tool_name: if tool_names.is_null(i) {
                        None
                    } else {
                        Some(tool_names.value(i).to_string())
                    },
                    tool_type: if tool_types.is_null(i) {
                        None
                    } else {
                        Some(tool_types.value(i).to_string())
                    },
                    call_count: call_counts.value(i),
                    avg_duration_ms: if avg_durations.is_null(i) {
                        0.0
                    } else {
                        avg_durations.value(i)
                    },
                    error_rate: if error_rates.is_null(i) {
                        0.0
                    } else {
                        error_rates.value(i)
                    },
                });
            }
        }
        Ok(results)
    }

    pub async fn get_error_breakdown(
        &self,
        service_name: Option<&str>,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        operation_name: Option<&str>,
    ) -> Result<Vec<(String, i64)>, TraceEngineError> {
        let df = self.ctx.table(GEN_AI_TABLE_NAME).await?;
        let df = Self::apply_time_filters(df, &start, &end)?;
        let df = Self::apply_optional_filter(df, SERVICE_NAME_COL, service_name)?;
        let df = Self::apply_optional_filter(df, OPERATION_NAME_COL, operation_name)?;
        let df = df.filter(col(ERROR_TYPE_COL).is_not_null())?;

        let df = df.aggregate(
            vec![col(ERROR_TYPE_COL)],
            vec![count(lit(1)).alias("count")],
        )?;
        let df = df.sort(vec![col("count").sort(false, false)])?;

        let batches = df.collect().await?;
        let mut results = Vec::new();
        for batch in &batches {
            let error_types_arr = compute::cast(
                batch.column_by_name(ERROR_TYPE_COL).ok_or_else(|| {
                    TraceEngineError::UnsupportedOperation("missing error_type column".into())
                })?,
                &DataType::Utf8,
            )?;
            let error_types = error_types_arr
                .as_any()
                .downcast_ref::<StringArray>()
                .ok_or_else(|| {
                    TraceEngineError::UnsupportedOperation(
                        "error_type cast to StringArray failed".into(),
                    )
                })?;
            let counts = batch
                .column_by_name("count")
                .and_then(|c| c.as_any().downcast_ref::<Int64Array>())
                .ok_or_else(|| {
                    TraceEngineError::UnsupportedOperation("missing count column".into())
                })?;

            for i in 0..batch.num_rows() {
                let error_type = if error_types.is_null(i) {
                    "unknown".to_string()
                } else {
                    error_types.value(i).to_string()
                };
                results.push((error_type, counts.value(i)));
            }
        }
        Ok(results)
    }

    pub async fn get_genai_spans(
        &self,
        filters: &GenAiSpanFilters,
    ) -> Result<Vec<GenAiSpanRecord>, TraceEngineError> {
        let df = self.ctx.table(GEN_AI_TABLE_NAME).await?;

        let df = if let (Some(start), Some(end)) = (&filters.start_time, &filters.end_time) {
            Self::apply_time_filters(df, start, end)?
        } else if let Some(start) = &filters.start_time {
            let df = df.filter(col(PARTITION_DATE_COL).gt_eq(date_lit(start)))?;
            df.filter(col(START_TIME_COL).gt_eq(ts_lit(start)))?
        } else if let Some(end) = &filters.end_time {
            let df = df.filter(col(PARTITION_DATE_COL).lt_eq(date_lit(end)))?;
            df.filter(col(START_TIME_COL).lt(ts_lit(end)))?
        } else {
            df
        };

        let df = Self::apply_optional_filter(df, SERVICE_NAME_COL, filters.service_name.as_deref())?;
        let df = Self::apply_optional_filter(
            df,
            OPERATION_NAME_COL,
            filters.operation_name.as_deref(),
        )?;
        let df =
            Self::apply_optional_filter(df, PROVIDER_NAME_COL, filters.provider_name.as_deref())?;
        let df = Self::apply_optional_filter(
            df,
            CONVERSATION_ID_COL,
            filters.conversation_id.as_deref(),
        )?;
        let df =
            Self::apply_optional_filter(df, AGENT_NAME_COL, filters.agent_name.as_deref())?;
        let df =
            Self::apply_optional_filter(df, TOOL_NAME_COL, filters.tool_name.as_deref())?;
        let df =
            Self::apply_optional_filter(df, ERROR_TYPE_COL, filters.error_type.as_deref())?;

        // model filter: match either request_model or response_model
        let df = if let Some(model) = &filters.model {
            df.filter(
                col(REQUEST_MODEL_COL)
                    .eq(lit(model.as_str()))
                    .or(col(RESPONSE_MODEL_COL).eq(lit(model.as_str()))),
            )?
        } else {
            df
        };

        let df = df.sort(vec![col(START_TIME_COL).sort(false, false)])?;
        let df = df.limit(0, Some(filters.limit.unwrap_or(100).min(10_000)))?;

        let batches = df.collect().await?;
        batches_to_genai_records(batches)
    }

    pub async fn get_conversation_spans(
        &self,
        conversation_id: &str,
        start: Option<DateTime<Utc>>,
        end: Option<DateTime<Utc>>,
    ) -> Result<Vec<GenAiSpanRecord>, TraceEngineError> {
        let df = self.ctx.table(GEN_AI_TABLE_NAME).await?;

        let df = if let (Some(s), Some(e)) = (&start, &end) {
            Self::apply_time_filters(df, s, e)?
        } else if let Some(s) = &start {
            let df = df.filter(col(PARTITION_DATE_COL).gt_eq(date_lit(s)))?;
            df.filter(col(START_TIME_COL).gt_eq(ts_lit(s)))?
        } else if let Some(e) = &end {
            let df = df.filter(col(PARTITION_DATE_COL).lt_eq(date_lit(e)))?;
            df.filter(col(START_TIME_COL).lt(ts_lit(e)))?
        } else {
            df
        };

        let df = df.filter(col(CONVERSATION_ID_COL).eq(lit(conversation_id)))?;
        let df = df.sort(vec![col(START_TIME_COL).sort(true, true)])?;
        let df = df.limit(0, Some(1_000))?;

        let batches = df.collect().await?;
        batches_to_genai_records(batches)
    }
}

// ── Arrow → GenAiSpanRecord conversion ───────────────────────────────────────

fn nullable_string(arr: &StringArray, i: usize) -> Option<String> {
    if arr.is_null(i) {
        None
    } else {
        Some(arr.value(i).to_string())
    }
}

fn nullable_i64(arr: &Int64Array, i: usize) -> Option<i64> {
    if arr.is_null(i) {
        None
    } else {
        Some(arr.value(i))
    }
}

fn nullable_f64(arr: &Float64Array, i: usize) -> Option<f64> {
    if arr.is_null(i) {
        None
    } else {
        Some(arr.value(i))
    }
}

fn cast_to_string_array(
    batch: &RecordBatch,
    col_name: &str,
) -> Result<StringArray, TraceEngineError> {
    let col_ref = batch.column_by_name(col_name).ok_or_else(|| {
        TraceEngineError::UnsupportedOperation(format!("missing {} column", col_name))
    })?;
    let casted = compute::cast(col_ref, &DataType::Utf8)?;
    casted
        .as_any()
        .downcast_ref::<StringArray>()
        .cloned()
        .ok_or_else(|| {
            TraceEngineError::UnsupportedOperation(format!(
                "{} cast to StringArray failed",
                col_name
            ))
        })
}

fn batches_to_genai_records(
    batches: Vec<RecordBatch>,
) -> Result<Vec<GenAiSpanRecord>, TraceEngineError> {
    let mut records = Vec::new();

    for batch in &batches {
        // IDs
        let trace_id_col = batch.column_by_name(TRACE_ID_COL).ok_or_else(|| {
            TraceEngineError::UnsupportedOperation("missing trace_id column".into())
        })?;
        let trace_id_binary = compute::cast(trace_id_col, &DataType::Binary)?;
        let trace_ids = trace_id_binary
            .as_any()
            .downcast_ref::<BinaryArray>()
            .ok_or_else(|| {
                TraceEngineError::UnsupportedOperation("trace_id cast to BinaryArray failed".into())
            })?;

        let span_id_col = batch.column_by_name(SPAN_ID_COL).ok_or_else(|| {
            TraceEngineError::UnsupportedOperation("missing span_id column".into())
        })?;
        let span_id_binary = compute::cast(span_id_col, &DataType::Binary)?;
        let span_ids = span_id_binary
            .as_any()
            .downcast_ref::<BinaryArray>()
            .ok_or_else(|| {
                TraceEngineError::UnsupportedOperation("span_id cast to BinaryArray failed".into())
            })?;

        let service_names = cast_to_string_array(batch, SERVICE_NAME_COL)?;
        let operation_names = cast_to_string_array(batch, OPERATION_NAME_COL)?;
        let provider_names = cast_to_string_array(batch, PROVIDER_NAME_COL)?;
        let request_models = cast_to_string_array(batch, REQUEST_MODEL_COL)?;
        let response_models = cast_to_string_array(batch, RESPONSE_MODEL_COL)?;
        let response_ids = cast_to_string_array(batch, RESPONSE_ID_COL)?;
        let output_types = cast_to_string_array(batch, OUTPUT_TYPE_COL)?;
        let conversation_ids = cast_to_string_array(batch, CONVERSATION_ID_COL)?;
        let agent_names = cast_to_string_array(batch, AGENT_NAME_COL)?;
        let agent_ids = cast_to_string_array(batch, AGENT_ID_COL)?;
        let tool_names = cast_to_string_array(batch, TOOL_NAME_COL)?;
        let tool_types = cast_to_string_array(batch, TOOL_TYPE_COL)?;
        let tool_call_ids = cast_to_string_array(batch, TOOL_CALL_ID_COL)?;
        let error_types = cast_to_string_array(batch, ERROR_TYPE_COL)?;
        let openai_api_types = cast_to_string_array(batch, OPENAI_API_TYPE_COL)?;
        let openai_service_tiers = cast_to_string_array(batch, OPENAI_SERVICE_TIER_COL)?;
        let labels = cast_to_string_array(batch, LABEL_COL)?;

        let start_times = batch
            .column_by_name(START_TIME_COL)
            .and_then(|c| c.as_any().downcast_ref::<TimestampMicrosecondArray>())
            .ok_or_else(|| {
                TraceEngineError::UnsupportedOperation("missing start_time column".into())
            })?;
        let end_times = batch
            .column_by_name(END_TIME_COL)
            .and_then(|c| c.as_any().downcast_ref::<TimestampMicrosecondArray>())
            .ok_or_else(|| {
                TraceEngineError::UnsupportedOperation("missing end_time column".into())
            })?;
        let durations = batch
            .column_by_name(DURATION_MS_COL)
            .and_then(|c| c.as_any().downcast_ref::<Int64Array>())
            .ok_or_else(|| {
                TraceEngineError::UnsupportedOperation("missing duration_ms column".into())
            })?;
        let status_codes = batch
            .column_by_name(STATUS_CODE_COL)
            .and_then(|c| c.as_any().downcast_ref::<Int32Array>())
            .ok_or_else(|| {
                TraceEngineError::UnsupportedOperation("missing status_code column".into())
            })?;
        let input_tokens = batch
            .column_by_name(INPUT_TOKENS_COL)
            .and_then(|c| c.as_any().downcast_ref::<Int64Array>())
            .ok_or_else(|| {
                TraceEngineError::UnsupportedOperation("missing input_tokens column".into())
            })?;
        let output_tokens = batch
            .column_by_name(OUTPUT_TOKENS_COL)
            .and_then(|c| c.as_any().downcast_ref::<Int64Array>())
            .ok_or_else(|| {
                TraceEngineError::UnsupportedOperation("missing output_tokens column".into())
            })?;
        let cache_creation = batch
            .column_by_name(CACHE_CREATION_INPUT_TOKENS_COL)
            .and_then(|c| c.as_any().downcast_ref::<Int64Array>())
            .ok_or_else(|| {
                TraceEngineError::UnsupportedOperation(
                    "missing cache_creation_input_tokens column".into(),
                )
            })?;
        let cache_read = batch
            .column_by_name(CACHE_READ_INPUT_TOKENS_COL)
            .and_then(|c| c.as_any().downcast_ref::<Int64Array>())
            .ok_or_else(|| {
                TraceEngineError::UnsupportedOperation(
                    "missing cache_read_input_tokens column".into(),
                )
            })?;
        let request_temps = batch
            .column_by_name(REQUEST_TEMPERATURE_COL)
            .and_then(|c| c.as_any().downcast_ref::<Float64Array>())
            .ok_or_else(|| {
                TraceEngineError::UnsupportedOperation(
                    "missing request_temperature column".into(),
                )
            })?;
        let request_max_tokens_arr = batch
            .column_by_name(REQUEST_MAX_TOKENS_COL)
            .and_then(|c| c.as_any().downcast_ref::<Int64Array>())
            .ok_or_else(|| {
                TraceEngineError::UnsupportedOperation(
                    "missing request_max_tokens column".into(),
                )
            })?;
        let request_top_ps = batch
            .column_by_name(REQUEST_TOP_P_COL)
            .and_then(|c| c.as_any().downcast_ref::<Float64Array>())
            .ok_or_else(|| {
                TraceEngineError::UnsupportedOperation("missing request_top_p column".into())
            })?;

        let finish_reasons_list = batch
            .column_by_name(FINISH_REASONS_COL)
            .and_then(|c| c.as_any().downcast_ref::<ListArray>());

        for i in 0..batch.num_rows() {
            let trace_id_bytes = trace_ids.value(i);
            let trace_id = if trace_id_bytes.len() == 16 {
                let mut arr = [0u8; 16];
                arr.copy_from_slice(trace_id_bytes);
                TraceId::from_bytes(arr)
            } else {
                TraceId::default()
            };

            let span_id_bytes = span_ids.value(i);
            let span_id = if span_id_bytes.len() == 8 {
                let mut arr = [0u8; 8];
                arr.copy_from_slice(span_id_bytes);
                SpanId::from_bytes(arr)
            } else {
                SpanId::default()
            };

            let start_time = DateTime::from_timestamp_micros(start_times.value(i))
                .ok_or(TraceEngineError::InvalidTimestamp(
                    "out-of-range start_time timestamp",
                ))?;

            let end_time = if end_times.is_null(i) {
                None
            } else {
                Some(
                    DateTime::from_timestamp_micros(end_times.value(i)).ok_or(
                        TraceEngineError::InvalidTimestamp("out-of-range end_time timestamp"),
                    )?,
                )
            };

            let finish_reasons = if let Some(list) = finish_reasons_list {
                if list.is_null(i) {
                    vec![]
                } else {
                    let inner = list.value(i);
                    let str_arr = compute::cast(&inner, &DataType::Utf8)
                        .ok()
                        .and_then(|a| a.as_any().downcast_ref::<StringArray>().cloned());
                    match str_arr {
                        Some(arr) => (0..arr.len())
                            .filter(|j| !arr.is_null(*j))
                            .map(|j| arr.value(j).to_string())
                            .collect(),
                        None => vec![],
                    }
                }
            } else {
                vec![]
            };

            records.push(GenAiSpanRecord {
                trace_id,
                span_id,
                service_name: service_names.value(i).to_string(),
                start_time,
                end_time,
                duration_ms: durations.value(i),
                status_code: status_codes.value(i),
                operation_name: nullable_string(&operation_names, i),
                provider_name: nullable_string(&provider_names, i),
                request_model: nullable_string(&request_models, i),
                response_model: nullable_string(&response_models, i),
                response_id: nullable_string(&response_ids, i),
                input_tokens: nullable_i64(input_tokens, i),
                output_tokens: nullable_i64(output_tokens, i),
                cache_creation_input_tokens: nullable_i64(cache_creation, i),
                cache_read_input_tokens: nullable_i64(cache_read, i),
                finish_reasons,
                output_type: nullable_string(&output_types, i),
                conversation_id: nullable_string(&conversation_ids, i),
                agent_name: nullable_string(&agent_names, i),
                agent_id: nullable_string(&agent_ids, i),
                tool_name: nullable_string(&tool_names, i),
                tool_type: nullable_string(&tool_types, i),
                tool_call_id: nullable_string(&tool_call_ids, i),
                request_temperature: nullable_f64(request_temps, i),
                request_max_tokens: nullable_i64(request_max_tokens_arr, i),
                request_top_p: nullable_f64(request_top_ps, i),
                error_type: nullable_string(&error_types, i),
                openai_api_type: nullable_string(&openai_api_types, i),
                openai_service_tier: nullable_string(&openai_service_tiers, i),
                label: nullable_string(&labels, i),
            });
        }
    }

    Ok(records)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::ObjectStore;
    use scouter_settings::ObjectStorageSettings;
    use scouter_types::{extract_gen_ai_span, Attribute, SpanId, TraceId, TraceSpanRecord};

    struct TestEnv {
        object_store: ObjectStore,
        ctx: Arc<SessionContext>,
        catalog: Arc<TraceCatalogProvider>,
        _tmp: tempfile::TempDir,
    }

    fn make_test_env() -> TestEnv {
        let _ = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::INFO)
            .try_init();

        let tmp = tempfile::TempDir::new().unwrap();
        let settings = ObjectStorageSettings {
            storage_uri: tmp.path().to_str().unwrap().to_string(),
            ..ObjectStorageSettings::default()
        };
        let object_store = ObjectStore::new(&settings).unwrap();
        let ctx = Arc::new(
            object_store
                .get_session_with_catalog(
                    crate::parquet::tracing::engine::TRACE_CATALOG_NAME,
                    "default",
                )
                .unwrap(),
        );
        let catalog = {
            use datafusion::catalog::CatalogProvider;
            let catalog = Arc::new(TraceCatalogProvider::new());
            ctx.register_catalog(
                crate::parquet::tracing::engine::TRACE_CATALOG_NAME,
                Arc::clone(&catalog) as Arc<dyn CatalogProvider>,
            );
            catalog
        };
        TestEnv {
            object_store,
            ctx,
            catalog,
            _tmp: tmp,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn make_genai_record(
        trace_id: &TraceId,
        span_id: SpanId,
        service_name: &str,
        operation: &str,
        provider: &str,
        model: &str,
        input_tokens: i64,
        output_tokens: i64,
    ) -> GenAiSpanRecord {
        let now = Utc::now();
        GenAiSpanRecord {
            trace_id: *trace_id,
            span_id,
            service_name: service_name.to_string(),
            start_time: now,
            end_time: Some(now + chrono::Duration::milliseconds(100)),
            duration_ms: 100,
            status_code: 0,
            operation_name: Some(operation.to_string()),
            provider_name: Some(provider.to_string()),
            request_model: Some(model.to_string()),
            response_model: Some(model.to_string()),
            input_tokens: Some(input_tokens),
            output_tokens: Some(output_tokens),
            ..Default::default()
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn make_genai_span(
        trace_id: &TraceId,
        span_id: SpanId,
        service_name: &str,
        operation: &str,
        provider: &str,
        model: &str,
        input_tokens: i64,
        output_tokens: i64,
    ) -> TraceSpanRecord {
        let now = Utc::now();
        TraceSpanRecord {
            created_at: now,
            trace_id: *trace_id,
            span_id,
            parent_span_id: None,
            flags: 1,
            trace_state: String::new(),
            scope_name: "test.scope".to_string(),
            scope_version: None,
            span_name: "op".to_string(),
            span_kind: "INTERNAL".to_string(),
            start_time: now,
            end_time: now + chrono::Duration::milliseconds(100),
            duration_ms: 100,
            status_code: 0,
            status_message: "OK".to_string(),
            attributes: vec![
                Attribute {
                    key: "gen_ai.operation.name".to_string(),
                    value: serde_json::Value::String(operation.to_string()),
                },
                Attribute {
                    key: "gen_ai.provider.name".to_string(),
                    value: serde_json::Value::String(provider.to_string()),
                },
                Attribute {
                    key: "gen_ai.request.model".to_string(),
                    value: serde_json::Value::String(model.to_string()),
                },
                Attribute {
                    key: "gen_ai.usage.input_tokens".to_string(),
                    value: serde_json::Value::Number(input_tokens.into()),
                },
                Attribute {
                    key: "gen_ai.usage.output_tokens".to_string(),
                    value: serde_json::Value::Number(output_tokens.into()),
                },
            ],
            events: vec![],
            links: vec![],
            label: None,
            input: serde_json::Value::Null,
            output: serde_json::Value::Null,
            service_name: service_name.to_string(),
            resource_attributes: vec![],
        }
    }

    #[tokio::test]
    async fn test_genai_service_initialization() -> Result<(), TraceEngineError> {
        let env = make_test_env();
        let service =
            GenAiSpanService::new(&env.object_store, 24, env.ctx, env.catalog, 10, None).await?;
        service.shutdown().await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_genai_extraction_round_trip() -> Result<(), TraceEngineError> {
        let env = make_test_env();
        let service =
            GenAiSpanService::new(&env.object_store, 24, env.ctx, env.catalog, 10, None).await?;

        let trace_id = TraceId::from_bytes([1u8; 16]);
        let span = make_genai_span(
            &trace_id,
            SpanId::from_bytes([1u8; 8]),
            "test-svc",
            "chat",
            "anthropic",
            "claude-3",
            100,
            200,
        );
        let genai_record = extract_gen_ai_span(&span).expect("should extract gen_ai span");
        service.write_records(vec![genai_record]).await?;
        tokio::time::sleep(tokio::time::Duration::from_secs(4)).await;

        let start = Utc::now() - chrono::Duration::hours(1);
        let end = Utc::now() + chrono::Duration::hours(1);
        let metrics = service
            .query_service
            .get_token_metrics(None, start, end, "hour", None, None)
            .await?;

        assert!(!metrics.is_empty(), "Expected at least one bucket");
        let total_input: i64 = metrics.iter().map(|b| b.total_input_tokens).sum();
        let total_output: i64 = metrics.iter().map(|b| b.total_output_tokens).sum();
        assert_eq!(total_input, 100, "Expected 100 input tokens");
        assert_eq!(total_output, 200, "Expected 200 output tokens");

        service.shutdown().await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_non_genai_spans_not_extracted() {
        let span = TraceSpanRecord {
            trace_id: TraceId::from_bytes([2u8; 16]),
            span_id: SpanId::from_bytes([2u8; 8]),
            service_name: "test-svc".to_string(),
            start_time: Utc::now(),
            end_time: Utc::now() + chrono::Duration::milliseconds(50),
            duration_ms: 50,
            status_code: 0,
            attributes: vec![
                Attribute {
                    key: "http.method".to_string(),
                    value: serde_json::Value::String("GET".to_string()),
                },
            ],
            ..Default::default()
        };
        assert!(
            extract_gen_ai_span(&span).is_none(),
            "Non-genai spans should not be extracted"
        );

        let span_no_attrs = TraceSpanRecord {
            trace_id: TraceId::from_bytes([3u8; 16]),
            span_id: SpanId::from_bytes([3u8; 8]),
            service_name: "test-svc".to_string(),
            start_time: Utc::now(),
            end_time: Utc::now() + chrono::Duration::milliseconds(50),
            duration_ms: 50,
            status_code: 0,
            ..Default::default()
        };
        assert!(
            extract_gen_ai_span(&span_no_attrs).is_none(),
            "Spans without attributes should not be extracted"
        );

        let span_tokens_only = TraceSpanRecord {
            trace_id: TraceId::from_bytes([4u8; 16]),
            span_id: SpanId::from_bytes([4u8; 8]),
            service_name: "test-svc".to_string(),
            start_time: Utc::now(),
            end_time: Utc::now() + chrono::Duration::milliseconds(50),
            duration_ms: 50,
            status_code: 0,
            attributes: vec![Attribute {
                key: "gen_ai.usage.input_tokens".to_string(),
                value: serde_json::Value::Number(100.into()),
            }],
            ..Default::default()
        };
        assert!(
            extract_gen_ai_span(&span_tokens_only).is_none(),
            "Spans with tokens but no operation_name should not be extracted"
        );
    }

    #[tokio::test]
    async fn test_genai_token_metrics_bucketed() -> Result<(), TraceEngineError> {
        let env = make_test_env();
        let service =
            GenAiSpanService::new(&env.object_store, 24, env.ctx, env.catalog, 10, None).await?;

        let records: Vec<GenAiSpanRecord> = (0..5)
            .map(|i| {
                make_genai_record(
                    &TraceId::from_bytes([10 + i as u8; 16]),
                    SpanId::from_bytes([10 + i as u8; 8]),
                    "test-svc",
                    "chat",
                    "anthropic",
                    "claude-3",
                    100,
                    200,
                )
            })
            .collect();
        service.write_records(records).await?;
        tokio::time::sleep(tokio::time::Duration::from_secs(4)).await;

        let start = Utc::now() - chrono::Duration::hours(1);
        let end = Utc::now() + chrono::Duration::hours(1);
        let metrics = service
            .query_service
            .get_token_metrics(None, start, end, "hour", None, None)
            .await?;

        let total_spans: i64 = metrics.iter().map(|b| b.span_count).sum();
        let total_input: i64 = metrics.iter().map(|b| b.total_input_tokens).sum();
        let total_output: i64 = metrics.iter().map(|b| b.total_output_tokens).sum();
        assert_eq!(total_spans, 5, "Expected 5 total spans");
        assert_eq!(total_input, 500, "Expected 500 total input tokens");
        assert_eq!(total_output, 1000, "Expected 1000 total output tokens");

        service.shutdown().await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_genai_model_usage() -> Result<(), TraceEngineError> {
        let env = make_test_env();
        let service =
            GenAiSpanService::new(&env.object_store, 24, env.ctx, env.catalog, 10, None).await?;

        let mut records = Vec::new();
        for i in 0..3 {
            records.push(make_genai_record(
                &TraceId::from_bytes([20 + i as u8; 16]),
                SpanId::from_bytes([20 + i as u8; 8]),
                "test-svc",
                "chat",
                "anthropic",
                "claude-3",
                100,
                200,
            ));
        }
        for i in 0..2 {
            records.push(make_genai_record(
                &TraceId::from_bytes([30 + i as u8; 16]),
                SpanId::from_bytes([30 + i as u8; 8]),
                "test-svc",
                "chat",
                "openai",
                "gpt-4",
                150,
                250,
            ));
        }
        service.write_records(records).await?;
        tokio::time::sleep(tokio::time::Duration::from_secs(4)).await;

        let start = Utc::now() - chrono::Duration::hours(1);
        let end = Utc::now() + chrono::Duration::hours(1);
        let usage = service
            .query_service
            .get_model_usage(None, start, end, None)
            .await?;

        assert_eq!(usage.len(), 2, "Expected 2 model usage rows");

        service.shutdown().await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_genai_operation_breakdown() -> Result<(), TraceEngineError> {
        let env = make_test_env();
        let service =
            GenAiSpanService::new(&env.object_store, 24, env.ctx, env.catalog, 10, None).await?;

        let records = vec![
            make_genai_record(
                &TraceId::from_bytes([40u8; 16]),
                SpanId::from_bytes([40u8; 8]),
                "test-svc",
                "chat",
                "anthropic",
                "claude-3",
                100,
                200,
            ),
            make_genai_record(
                &TraceId::from_bytes([41u8; 16]),
                SpanId::from_bytes([41u8; 8]),
                "test-svc",
                "execute_tool",
                "anthropic",
                "claude-3",
                50,
                100,
            ),
        ];
        service.write_records(records).await?;
        tokio::time::sleep(tokio::time::Duration::from_secs(4)).await;

        let start = Utc::now() - chrono::Duration::hours(1);
        let end = Utc::now() + chrono::Duration::hours(1);
        let breakdown = service
            .query_service
            .get_operation_breakdown(None, start, end, None)
            .await?;

        assert_eq!(breakdown.len(), 2, "Expected 2 operation breakdown rows");
        let ops: Vec<&str> = breakdown.iter().map(|b| b.operation_name.as_str()).collect();
        assert!(ops.contains(&"chat"), "Expected chat operation");
        assert!(
            ops.contains(&"execute_tool"),
            "Expected execute_tool operation"
        );

        service.shutdown().await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_genai_service_filter() -> Result<(), TraceEngineError> {
        let env = make_test_env();
        let service =
            GenAiSpanService::new(&env.object_store, 24, env.ctx, env.catalog, 10, None).await?;

        let mut records = Vec::new();
        for i in 0..3 {
            records.push(make_genai_record(
                &TraceId::from_bytes([50 + i as u8; 16]),
                SpanId::from_bytes([50 + i as u8; 8]),
                "svc-alpha",
                "chat",
                "anthropic",
                "claude-3",
                100,
                200,
            ));
        }
        for i in 0..2 {
            records.push(make_genai_record(
                &TraceId::from_bytes([60 + i as u8; 16]),
                SpanId::from_bytes([60 + i as u8; 8]),
                "svc-beta",
                "chat",
                "openai",
                "gpt-4",
                150,
                250,
            ));
        }
        service.write_records(records).await?;
        tokio::time::sleep(tokio::time::Duration::from_secs(4)).await;

        let start = Utc::now() - chrono::Duration::hours(1);
        let end = Utc::now() + chrono::Duration::hours(1);

        let alpha_metrics = service
            .query_service
            .get_token_metrics(Some("svc-alpha"), start, end, "hour", None, None)
            .await?;
        let alpha_spans: i64 = alpha_metrics.iter().map(|b| b.span_count).sum();
        assert_eq!(alpha_spans, 3, "Expected 3 spans for svc-alpha");

        let beta_metrics = service
            .query_service
            .get_token_metrics(Some("svc-beta"), start, end, "hour", None, None)
            .await?;
        let beta_spans: i64 = beta_metrics.iter().map(|b| b.span_count).sum();
        assert_eq!(beta_spans, 2, "Expected 2 spans for svc-beta");

        service.shutdown().await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_get_operation_breakdown() -> Result<(), TraceEngineError> {
        let env = make_test_env();
        let service =
            GenAiSpanService::new(&env.object_store, 24, env.ctx, env.catalog, 10, None).await?;

        let records = vec![
            make_genai_record(
                &TraceId::from_bytes([70u8; 16]),
                SpanId::from_bytes([70u8; 8]),
                "test_service",
                "chat",
                "anthropic",
                "claude-3",
                100,
                200,
            ),
            make_genai_record(
                &TraceId::from_bytes([71u8; 16]),
                SpanId::from_bytes([71u8; 8]),
                "test_service",
                "chat",
                "anthropic",
                "claude-3",
                100,
                200,
            ),
            make_genai_record(
                &TraceId::from_bytes([72u8; 16]),
                SpanId::from_bytes([72u8; 8]),
                "test_service",
                "execute_tool",
                "anthropic",
                "claude-3",
                50,
                100,
            ),
            make_genai_record(
                &TraceId::from_bytes([73u8; 16]),
                SpanId::from_bytes([73u8; 8]),
                "test_service",
                "execute_tool",
                "anthropic",
                "claude-3",
                50,
                100,
            ),
            make_genai_record(
                &TraceId::from_bytes([74u8; 16]),
                SpanId::from_bytes([74u8; 8]),
                "test_service",
                "execute_tool",
                "anthropic",
                "claude-3",
                50,
                100,
            ),
        ];
        service.write_records(records).await?;
        tokio::time::sleep(tokio::time::Duration::from_secs(4)).await;

        let start = Utc::now() - chrono::Duration::hours(1);
        let end = Utc::now() + chrono::Duration::hours(1);
        let breakdown = service
            .query_service
            .get_operation_breakdown(Some("test_service"), start, end, None)
            .await?;

        assert_eq!(breakdown.len(), 2, "Expected 2 operation rows");
        let chat_row = breakdown.iter().find(|b| b.operation_name == "chat");
        let tool_row = breakdown.iter().find(|b| b.operation_name == "execute_tool");
        assert!(chat_row.is_some(), "Expected chat row");
        assert!(tool_row.is_some(), "Expected execute_tool row");
        assert_eq!(chat_row.unwrap().span_count, 2, "Expected 2 chat spans");
        assert_eq!(tool_row.unwrap().span_count, 3, "Expected 3 execute_tool spans");

        service.shutdown().await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_get_model_usage() -> Result<(), TraceEngineError> {
        let env = make_test_env();
        let service =
            GenAiSpanService::new(&env.object_store, 24, env.ctx, env.catalog, 10, None).await?;

        let mut records = Vec::new();
        for i in 0..2u8 {
            records.push(make_genai_record(
                &TraceId::from_bytes([80 + i; 16]),
                SpanId::from_bytes([80 + i; 8]),
                "test_service",
                "chat",
                "anthropic",
                "claude-3-5-sonnet",
                100,
                200,
            ));
        }
        records.push(make_genai_record(
            &TraceId::from_bytes([82u8; 16]),
            SpanId::from_bytes([82u8; 8]),
            "test_service",
            "chat",
            "openai",
            "gpt-4o",
            150,
            250,
        ));
        service.write_records(records).await?;
        tokio::time::sleep(tokio::time::Duration::from_secs(4)).await;

        let start = Utc::now() - chrono::Duration::hours(1);
        let end = Utc::now() + chrono::Duration::hours(1);
        let usage = service
            .query_service
            .get_model_usage(None, start, end, None)
            .await?;

        assert_eq!(usage.len(), 2, "Expected 2 model rows");
        let sonnet_row = usage.iter().find(|u| u.model == "claude-3-5-sonnet");
        let gpt_row = usage.iter().find(|u| u.model == "gpt-4o");
        assert!(sonnet_row.is_some(), "Expected claude-3-5-sonnet row");
        assert!(gpt_row.is_some(), "Expected gpt-4o row");
        assert_eq!(
            sonnet_row.unwrap().total_input_tokens,
            200,
            "Expected 200 input tokens for sonnet"
        );
        assert_eq!(
            gpt_row.unwrap().total_input_tokens,
            150,
            "Expected 150 input tokens for gpt-4o"
        );

        service.shutdown().await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_get_agent_activity() -> Result<(), TraceEngineError> {
        let env = make_test_env();
        let service =
            GenAiSpanService::new(&env.object_store, 24, env.ctx, env.catalog, 10, None).await?;

        let now = Utc::now();
        let record = GenAiSpanRecord {
            trace_id: TraceId::from_bytes([90u8; 16]),
            span_id: SpanId::from_bytes([90u8; 8]),
            service_name: "test_service".to_string(),
            start_time: now,
            end_time: Some(now + chrono::Duration::milliseconds(100)),
            duration_ms: 100,
            status_code: 0,
            operation_name: Some("invoke_agent".to_string()),
            provider_name: Some("anthropic".to_string()),
            request_model: Some("claude-3".to_string()),
            response_model: Some("claude-3".to_string()),
            input_tokens: Some(100),
            output_tokens: Some(200),
            agent_name: Some("test-agent".to_string()),
            conversation_id: Some("conv-123".to_string()),
            ..Default::default()
        };
        service.write_records(vec![record]).await?;
        tokio::time::sleep(tokio::time::Duration::from_secs(4)).await;

        let start = Utc::now() - chrono::Duration::hours(1);
        let end = Utc::now() + chrono::Duration::hours(1);
        let activity = service
            .query_service
            .get_agent_activity(None, start, end, None)
            .await?;

        assert_eq!(activity.len(), 1, "Expected 1 agent activity row");
        assert_eq!(
            activity[0].agent_name.as_deref(),
            Some("test-agent"),
            "Expected agent_name == test-agent"
        );

        service.shutdown().await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_get_tool_activity() -> Result<(), TraceEngineError> {
        let env = make_test_env();
        let service =
            GenAiSpanService::new(&env.object_store, 24, env.ctx, env.catalog, 10, None).await?;

        let now = Utc::now();
        let record = GenAiSpanRecord {
            trace_id: TraceId::from_bytes([91u8; 16]),
            span_id: SpanId::from_bytes([91u8; 8]),
            service_name: "test_service".to_string(),
            start_time: now,
            end_time: Some(now + chrono::Duration::milliseconds(50)),
            duration_ms: 50,
            status_code: 0,
            operation_name: Some("execute_tool".to_string()),
            provider_name: Some("anthropic".to_string()),
            request_model: Some("claude-3".to_string()),
            response_model: Some("claude-3".to_string()),
            input_tokens: Some(10),
            output_tokens: Some(20),
            tool_name: Some("web_search".to_string()),
            tool_type: Some("function".to_string()),
            ..Default::default()
        };
        service.write_records(vec![record]).await?;
        tokio::time::sleep(tokio::time::Duration::from_secs(4)).await;

        let start = Utc::now() - chrono::Duration::hours(1);
        let end = Utc::now() + chrono::Duration::hours(1);
        let activity = service
            .query_service
            .get_tool_activity(None, start, end)
            .await?;

        assert_eq!(activity.len(), 1, "Expected 1 tool activity row");
        assert_eq!(
            activity[0].tool_name.as_deref(),
            Some("web_search"),
            "Expected tool_name == web_search"
        );

        service.shutdown().await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_get_error_breakdown() -> Result<(), TraceEngineError> {
        let env = make_test_env();
        let service =
            GenAiSpanService::new(&env.object_store, 24, env.ctx, env.catalog, 10, None).await?;

        let now = Utc::now();
        let mut records = Vec::new();
        for i in 0..2u8 {
            records.push(GenAiSpanRecord {
                trace_id: TraceId::from_bytes([92 + i; 16]),
                span_id: SpanId::from_bytes([92 + i; 8]),
                service_name: "test_service".to_string(),
                start_time: now,
                end_time: Some(now + chrono::Duration::milliseconds(100)),
                duration_ms: 100,
                status_code: 1,
                operation_name: Some("chat".to_string()),
                error_type: Some("timeout".to_string()),
                ..Default::default()
            });
        }
        records.push(GenAiSpanRecord {
            trace_id: TraceId::from_bytes([94u8; 16]),
            span_id: SpanId::from_bytes([94u8; 8]),
            service_name: "test_service".to_string(),
            start_time: now,
            end_time: Some(now + chrono::Duration::milliseconds(100)),
            duration_ms: 100,
            status_code: 1,
            operation_name: Some("chat".to_string()),
            error_type: Some("rate_limit".to_string()),
            ..Default::default()
        });
        for i in 0..2u8 {
            records.push(make_genai_record(
                &TraceId::from_bytes([95 + i; 16]),
                SpanId::from_bytes([95 + i; 8]),
                "test_service",
                "chat",
                "anthropic",
                "claude-3",
                100,
                200,
            ));
        }
        service.write_records(records).await?;
        tokio::time::sleep(tokio::time::Duration::from_secs(4)).await;

        let start = Utc::now() - chrono::Duration::hours(1);
        let end = Utc::now() + chrono::Duration::hours(1);
        let breakdown = service
            .query_service
            .get_error_breakdown(None, start, end, None)
            .await?;

        assert_eq!(breakdown.len(), 2, "Expected 2 error type rows");
        let timeout_row = breakdown.iter().find(|(et, _)| et == "timeout");
        let rate_limit_row = breakdown.iter().find(|(et, _)| et == "rate_limit");
        assert!(timeout_row.is_some(), "Expected timeout error type");
        assert!(rate_limit_row.is_some(), "Expected rate_limit error type");
        assert_eq!(timeout_row.unwrap().1, 2, "Expected 2 timeout errors");
        assert_eq!(rate_limit_row.unwrap().1, 1, "Expected 1 rate_limit error");

        service.shutdown().await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_get_genai_spans_filtered() -> Result<(), TraceEngineError> {
        let env = make_test_env();
        let service =
            GenAiSpanService::new(&env.object_store, 24, env.ctx, env.catalog, 10, None).await?;

        let mut records = Vec::new();
        for i in 0..3u8 {
            records.push(make_genai_record(
                &TraceId::from_bytes([100 + i; 16]),
                SpanId::from_bytes([100 + i; 8]),
                "service_alpha",
                "chat",
                "anthropic",
                "claude-3",
                100,
                200,
            ));
        }
        for i in 0..2u8 {
            records.push(make_genai_record(
                &TraceId::from_bytes([103 + i; 16]),
                SpanId::from_bytes([103 + i; 8]),
                "service_beta",
                "chat",
                "openai",
                "gpt-4",
                150,
                250,
            ));
        }
        service.write_records(records).await?;
        tokio::time::sleep(tokio::time::Duration::from_secs(4)).await;

        let start = Utc::now() - chrono::Duration::hours(1);
        let end = Utc::now() + chrono::Duration::hours(1);
        let spans = service
            .query_service
            .get_genai_spans(&GenAiSpanFilters {
                service_name: Some("service_alpha".to_string()),
                start_time: Some(start),
                end_time: Some(end),
                ..Default::default()
            })
            .await?;

        assert_eq!(spans.len(), 3, "Expected 3 spans for service_alpha");
        for span in &spans {
            assert_eq!(
                span.service_name, "service_alpha",
                "All spans should belong to service_alpha"
            );
        }

        service.shutdown().await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_get_conversation_spans() -> Result<(), TraceEngineError> {
        let env = make_test_env();
        let service =
            GenAiSpanService::new(&env.object_store, 24, env.ctx, env.catalog, 10, None).await?;

        let now = Utc::now();
        let mut records = Vec::new();
        for i in 0..3u8 {
            records.push(GenAiSpanRecord {
                trace_id: TraceId::from_bytes([110 + i; 16]),
                span_id: SpanId::from_bytes([110 + i; 8]),
                service_name: "test_service".to_string(),
                start_time: now + chrono::Duration::milliseconds(i as i64 * 10),
                end_time: Some(now + chrono::Duration::milliseconds(i as i64 * 10 + 100)),
                duration_ms: 100,
                status_code: 0,
                operation_name: Some("chat".to_string()),
                provider_name: Some("anthropic".to_string()),
                request_model: Some("claude-3".to_string()),
                response_model: Some("claude-3".to_string()),
                input_tokens: Some(100),
                output_tokens: Some(200),
                conversation_id: Some("conv-abc".to_string()),
                ..Default::default()
            });
        }
        for i in 0..2u8 {
            records.push(make_genai_record(
                &TraceId::from_bytes([113 + i; 16]),
                SpanId::from_bytes([113 + i; 8]),
                "test_service",
                "chat",
                "anthropic",
                "claude-3",
                100,
                200,
            ));
        }
        service.write_records(records).await?;
        tokio::time::sleep(tokio::time::Duration::from_secs(4)).await;

        let start = Utc::now() - chrono::Duration::hours(1);
        let end = Utc::now() + chrono::Duration::hours(1);
        let spans = service
            .query_service
            .get_conversation_spans("conv-abc", Some(start), Some(end))
            .await?;

        assert_eq!(spans.len(), 3, "Expected 3 spans for conv-abc");
        for span in &spans {
            assert_eq!(
                span.conversation_id.as_deref(),
                Some("conv-abc"),
                "All spans should have conversation_id == conv-abc"
            );
        }

        service.shutdown().await?;
        Ok(())
    }
}
