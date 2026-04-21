use crate::error::TraceEngineError;
use crate::parquet::tracing::catalog::TraceCatalogProvider;
use crate::parquet::tracing::traits::arrow_schema_to_delta;
use crate::parquet::utils::register_cloud_logstore_factories;
use crate::storage::ObjectStore;
use arrow::array::{
    Date32Builder, FixedSizeBinaryBuilder, Int8Builder, TimestampMicrosecondArray,
    TimestampMicrosecondBuilder,
};
use arrow::compute;
use arrow::datatypes::{DataType, Field, Schema, TimeUnit};
use arrow_array::Array;
use arrow_array::RecordBatch;
use chrono::{DateTime, Datelike, Utc};
use datafusion::functions_aggregate::expr_fn::{max, min};
use datafusion::logical_expr::{cast as df_cast, col, lit};
use datafusion::prelude::SessionContext;
use datafusion::scalar::ScalarValue;
use deltalake::{DeltaTable, DeltaTableBuilder, TableProperty};
use std::sync::Arc;
use tokio::sync::oneshot;
use tokio::sync::{mpsc, RwLock as AsyncRwLock};
use tracing::{error, info, instrument};
use url::Url;

const UNIX_EPOCH_DAYS: i32 = 719_163;
const DISPATCH_TABLE_NAME: &str = "trace_dispatch";

const TRACE_ID_COL: &str = "trace_id";
const ENTITY_UID_COL: &str = "entity_uid";
const START_TIME_COL: &str = "start_time";
const EVENT_TYPE_COL: &str = "event_type";
const HAS_CANDIDATE_EVENT_COL: &str = "has_candidate_event";
const HAS_ACK_EVENT_COL: &str = "has_ack_event";
const CREATED_AT_COL: &str = "created_at";
const PARTITION_DATE_COL: &str = "partition_date";

const EVENT_TYPE_CANDIDATE: i8 = 0;
const EVENT_TYPE_ACK: i8 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DispatchEventType {
    Candidate,
    Ack,
}

impl DispatchEventType {
    fn as_i8(self) -> i8 {
        match self {
            Self::Candidate => EVENT_TYPE_CANDIDATE,
            Self::Ack => EVENT_TYPE_ACK,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TraceDispatchRecord {
    pub trace_id: [u8; 16],
    pub entity_uid: [u8; 16],
    pub start_time: DateTime<Utc>,
    pub event_type: DispatchEventType,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct DispatchCandidate {
    pub trace_id: [u8; 16],
    pub entity_uid: [u8; 16],
    pub start_time: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct DispatchCursor {
    pub start_time: DateTime<Utc>,
    pub trace_id: [u8; 16],
    pub entity_uid: [u8; 16],
}

#[derive(Debug, Clone)]
pub struct DispatchPage {
    pub items: Vec<DispatchCandidate>,
    pub next_cursor: Option<DispatchCursor>,
    pub has_next: bool,
}

fn create_dispatch_schema() -> Schema {
    Schema::new(vec![
        Field::new(TRACE_ID_COL, DataType::FixedSizeBinary(16), false),
        Field::new(ENTITY_UID_COL, DataType::FixedSizeBinary(16), false),
        Field::new(
            START_TIME_COL,
            DataType::Timestamp(TimeUnit::Microsecond, Some("UTC".into())),
            false,
        ),
        Field::new(EVENT_TYPE_COL, DataType::Int8, false),
        Field::new(
            CREATED_AT_COL,
            DataType::Timestamp(TimeUnit::Microsecond, Some("UTC".into())),
            false,
        ),
        Field::new(PARTITION_DATE_COL, DataType::Date32, false),
    ])
}

struct TraceDispatchBatchBuilder {
    schema: Arc<Schema>,
    trace_id: FixedSizeBinaryBuilder,
    entity_uid: FixedSizeBinaryBuilder,
    start_time: TimestampMicrosecondBuilder,
    event_type: Int8Builder,
    created_at: TimestampMicrosecondBuilder,
    partition_date: Date32Builder,
}

impl TraceDispatchBatchBuilder {
    fn new(schema: Arc<Schema>, capacity: usize) -> Self {
        Self {
            schema,
            trace_id: FixedSizeBinaryBuilder::with_capacity(capacity, 16),
            entity_uid: FixedSizeBinaryBuilder::with_capacity(capacity, 16),
            start_time: TimestampMicrosecondBuilder::with_capacity(capacity).with_timezone("UTC"),
            event_type: Int8Builder::with_capacity(capacity),
            created_at: TimestampMicrosecondBuilder::with_capacity(capacity).with_timezone("UTC"),
            partition_date: Date32Builder::with_capacity(capacity),
        }
    }

    fn append(&mut self, rec: &TraceDispatchRecord) -> Result<(), TraceEngineError> {
        self.trace_id.append_value(rec.trace_id)?;
        self.entity_uid.append_value(rec.entity_uid)?;
        self.start_time
            .append_value(rec.start_time.timestamp_micros());
        self.event_type.append_value(rec.event_type.as_i8());
        self.created_at
            .append_value(rec.created_at.timestamp_micros());
        let days = rec.start_time.date_naive().num_days_from_ce() - UNIX_EPOCH_DAYS;
        self.partition_date.append_value(days);
        Ok(())
    }

    fn finish(mut self) -> Result<RecordBatch, TraceEngineError> {
        let columns: Vec<Arc<dyn arrow_array::Array>> = vec![
            Arc::new(self.trace_id.finish()),
            Arc::new(self.entity_uid.finish()),
            Arc::new(self.start_time.finish()),
            Arc::new(self.event_type.finish()),
            Arc::new(self.created_at.finish()),
            Arc::new(self.partition_date.finish()),
        ];
        Ok(RecordBatch::try_new(self.schema, columns)?)
    }
}

pub enum DispatchTableCommand {
    Write {
        records: Vec<TraceDispatchRecord>,
        respond_to: oneshot::Sender<Result<(), TraceEngineError>>,
    },
    Shutdown,
}

async fn build_dispatch_url(object_store: &ObjectStore) -> Result<Url, TraceEngineError> {
    let mut base = object_store.get_base_url()?;
    let mut path = base.path().to_string();
    if !path.ends_with('/') {
        path.push('/');
    }
    path.push_str(DISPATCH_TABLE_NAME);
    base.set_path(&path);
    Ok(base)
}

async fn create_dispatch_table(
    object_store: &ObjectStore,
    table_url: Url,
    schema: Arc<Schema>,
) -> Result<DeltaTable, TraceEngineError> {
    let table = DeltaTableBuilder::from_url(table_url.clone())?
        .with_storage_backend(object_store.as_dyn_object_store(), table_url)
        .build()?;

    table
        .create()
        .with_table_name(DISPATCH_TABLE_NAME)
        .with_columns(arrow_schema_to_delta(&schema))
        .with_partition_columns(vec![PARTITION_DATE_COL.to_string()])
        .with_configuration_property(
            TableProperty::DataSkippingStatsColumns,
            Some("start_time,event_type,partition_date"),
        )
        .await
        .map_err(Into::into)
}

async fn build_or_create_dispatch_table(
    object_store: &ObjectStore,
    schema: Arc<Schema>,
) -> Result<DeltaTable, TraceEngineError> {
    register_cloud_logstore_factories();
    let table_url = build_dispatch_url(object_store).await?;

    let is_delta_table = if table_url.scheme() == "file" {
        if let Ok(path) = table_url.to_file_path() {
            if !path.exists() {
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
        DeltaTableBuilder::from_url(table_url.clone())?
            .with_storage_backend(object_store.as_dyn_object_store(), table_url)
            .load()
            .await
            .map_err(Into::into)
    } else {
        create_dispatch_table(object_store, table_url, schema).await
    }
}

pub struct TraceDispatchDBEngine {
    schema: Arc<Schema>,
    table: Arc<AsyncRwLock<DeltaTable>>,
    ctx: Arc<SessionContext>,
    catalog: Arc<TraceCatalogProvider>,
}

impl TraceDispatchDBEngine {
    pub async fn new(
        object_store: &ObjectStore,
        ctx: Arc<SessionContext>,
        catalog: Arc<TraceCatalogProvider>,
    ) -> Result<Self, TraceEngineError> {
        let schema = Arc::new(create_dispatch_schema());
        let delta_table = build_or_create_dispatch_table(object_store, schema.clone()).await?;
        if let Ok(provider) = delta_table.table_provider().await {
            catalog.swap(DISPATCH_TABLE_NAME, provider);
        } else {
            info!("Empty trace dispatch table at init; deferring catalog registration");
        }

        Ok(Self {
            schema,
            table: Arc::new(AsyncRwLock::new(delta_table)),
            ctx,
            catalog,
        })
    }

    fn build_batch(
        &self,
        records: Vec<TraceDispatchRecord>,
    ) -> Result<RecordBatch, TraceEngineError> {
        let mut builder = TraceDispatchBatchBuilder::new(self.schema.clone(), records.len());
        for rec in &records {
            builder.append(rec)?;
        }
        builder.finish()
    }

    async fn write_records(
        &self,
        records: Vec<TraceDispatchRecord>,
    ) -> Result<(), TraceEngineError> {
        if records.is_empty() {
            return Ok(());
        }

        let batch = self.build_batch(records)?;

        let mut table_guard = self.table.write().await;
        let current_table = table_guard.clone();
        let updated_table = current_table
            .write(vec![batch])
            .with_save_mode(deltalake::protocol::SaveMode::Append)
            .with_partition_columns(vec![PARTITION_DATE_COL.to_string()])
            .await?;

        let provider = updated_table.table_provider().await?;
        self.catalog.swap(DISPATCH_TABLE_NAME, provider);
        updated_table.update_datafusion_session(&self.ctx.state())?;
        *table_guard = updated_table;

        Ok(())
    }

    #[instrument(skip_all, name = "dispatch_engine_actor")]
    pub fn start_actor(
        self,
    ) -> (
        mpsc::Sender<DispatchTableCommand>,
        tokio::task::JoinHandle<()>,
    ) {
        let (tx, mut rx) = mpsc::channel::<DispatchTableCommand>(100);
        let handle = tokio::spawn(async move {
            info!("TraceDispatchDBEngine actor started");
            while let Some(cmd) = rx.recv().await {
                match cmd {
                    DispatchTableCommand::Write {
                        records,
                        respond_to,
                    } => {
                        let result = self.write_records(records).await;
                        if let Err(ref e) = result {
                            error!("Dispatch write failed: {}", e);
                        }
                        let _ = respond_to.send(result);
                    }
                    DispatchTableCommand::Shutdown => {
                        info!("TraceDispatchDBEngine actor shutting down");
                        break;
                    }
                }
            }
        });

        (tx, handle)
    }
}

pub struct TraceDispatchService {
    engine_tx: mpsc::Sender<DispatchTableCommand>,
    engine_handle: tokio::task::JoinHandle<()>,
    pub query_service: TraceDispatchQueries,
}

impl TraceDispatchService {
    pub async fn new(
        object_store: &ObjectStore,
        ctx: Arc<SessionContext>,
        catalog: Arc<TraceCatalogProvider>,
    ) -> Result<Self, TraceEngineError> {
        let engine = TraceDispatchDBEngine::new(object_store, ctx.clone(), catalog).await?;
        let (engine_tx, engine_handle) = engine.start_actor();
        Ok(Self {
            engine_tx,
            engine_handle,
            query_service: TraceDispatchQueries::new(ctx),
        })
    }

    pub async fn write_records(
        &self,
        records: Vec<TraceDispatchRecord>,
    ) -> Result<(), TraceEngineError> {
        let (tx, rx) = oneshot::channel();
        self.engine_tx
            .send(DispatchTableCommand::Write {
                records,
                respond_to: tx,
            })
            .await
            .map_err(|_| TraceEngineError::ChannelClosed)?;
        rx.await.map_err(|_| TraceEngineError::ChannelClosed)?
    }

    pub async fn signal_shutdown(&self) {
        let _ = self.engine_tx.send(DispatchTableCommand::Shutdown).await;
    }

    pub async fn shutdown(self) -> Result<(), TraceEngineError> {
        self.engine_tx
            .send(DispatchTableCommand::Shutdown)
            .await
            .map_err(|_| TraceEngineError::ChannelClosed)?;
        if let Err(e) = self.engine_handle.await {
            error!("Dispatch engine handle error: {}", e);
        }
        Ok(())
    }
}

pub struct TraceDispatchQueries {
    ctx: Arc<SessionContext>,
}

impl TraceDispatchQueries {
    pub fn new(ctx: Arc<SessionContext>) -> Self {
        Self { ctx }
    }

    pub async fn get_pending_dispatch(
        &self,
        lookback_start: DateTime<Utc>,
        limit: usize,
        cursor: Option<&DispatchCursor>,
    ) -> Result<DispatchPage, TraceEngineError> {
        let page_size = limit.max(1);
        let mut df = self.ctx.table(DISPATCH_TABLE_NAME).await?;

        df = df.filter(
            col(START_TIME_COL).gt_eq(lit(ScalarValue::TimestampMicrosecond(
                Some(lookback_start.timestamp_micros()),
                Some("UTC".into()),
            ))),
        )?;
        // Pending dispatch keys are trace/entity pairs with at least one candidate event
        // and no ack event.
        df = df.aggregate(
            vec![col(TRACE_ID_COL), col(ENTITY_UID_COL)],
            vec![
                min(col(START_TIME_COL)).alias(START_TIME_COL),
                max(df_cast(
                    col(EVENT_TYPE_COL).eq(lit(EVENT_TYPE_CANDIDATE)),
                    DataType::Int8,
                ))
                .alias(HAS_CANDIDATE_EVENT_COL),
                max(df_cast(
                    col(EVENT_TYPE_COL).eq(lit(EVENT_TYPE_ACK)),
                    DataType::Int8,
                ))
                .alias(HAS_ACK_EVENT_COL),
            ],
        )?;
        df = df.filter(
            col(HAS_CANDIDATE_EVENT_COL)
                .eq(lit(1_i8))
                .and(col(HAS_ACK_EVENT_COL).eq(lit(0_i8))),
        )?;

        if let Some(cursor) = cursor {
            let cursor_ts = lit(ScalarValue::TimestampMicrosecond(
                Some(cursor.start_time.timestamp_micros()),
                Some("UTC".into()),
            ));
            let cursor_trace = lit(ScalarValue::Binary(Some(cursor.trace_id.to_vec())));
            let cursor_entity = lit(ScalarValue::Binary(Some(cursor.entity_uid.to_vec())));

            let expr = col(START_TIME_COL)
                .lt(cursor_ts.clone())
                .or(col(START_TIME_COL).eq(cursor_ts.clone()).and(
                    col(TRACE_ID_COL)
                        .lt(cursor_trace.clone())
                        .or(col(TRACE_ID_COL)
                            .eq(cursor_trace)
                            .and(col(ENTITY_UID_COL).lt(cursor_entity))),
                ));
            df = df.filter(expr)?;
        }

        df = df.sort(vec![
            col(START_TIME_COL).sort(false, false),
            col(TRACE_ID_COL).sort(false, false),
            col(ENTITY_UID_COL).sort(false, false),
        ])?;
        df = df.limit(0, Some(page_size + 1))?;

        let batches = df.collect().await?;
        let mut items = batches_to_dispatch_candidates(batches)?;
        let has_next = items.len() > page_size;
        if has_next {
            items.pop();
        }
        let next_cursor = if has_next {
            items.last().map(|item| DispatchCursor {
                start_time: item.start_time,
                trace_id: item.trace_id,
                entity_uid: item.entity_uid,
            })
        } else {
            None
        };

        Ok(DispatchPage {
            items,
            next_cursor,
            has_next,
        })
    }

    pub async fn trace_belongs_to_entity(
        &self,
        trace_id: &[u8],
        entity_uid: &[u8],
    ) -> Result<bool, TraceEngineError> {
        let mut df = self.ctx.table(DISPATCH_TABLE_NAME).await?;
        df = df.filter(col(TRACE_ID_COL).eq(lit(ScalarValue::Binary(Some(trace_id.to_vec())))))?;
        df =
            df.filter(col(ENTITY_UID_COL).eq(lit(ScalarValue::Binary(Some(entity_uid.to_vec())))))?;
        df = df.limit(0, Some(1))?;
        let batches = df.collect().await?;
        Ok(batches.iter().any(|b| b.num_rows() > 0))
    }
}

fn batches_to_dispatch_candidates(
    batches: Vec<RecordBatch>,
) -> Result<Vec<DispatchCandidate>, TraceEngineError> {
    let mut items = Vec::new();
    for batch in &batches {
        let trace_col = batch.column_by_name(TRACE_ID_COL).ok_or_else(|| {
            TraceEngineError::UnsupportedOperation("missing trace_id".to_string())
        })?;
        let trace_binary = compute::cast(trace_col, &DataType::Binary)?;
        let trace_arr = trace_binary
            .as_any()
            .downcast_ref::<arrow::array::BinaryArray>()
            .ok_or(TraceEngineError::DowncastError("trace_id binary cast"))?;

        let entity_col = batch.column_by_name(ENTITY_UID_COL).ok_or_else(|| {
            TraceEngineError::UnsupportedOperation("missing entity_uid".to_string())
        })?;
        let entity_binary = compute::cast(entity_col, &DataType::Binary)?;
        let entity_arr = entity_binary
            .as_any()
            .downcast_ref::<arrow::array::BinaryArray>()
            .ok_or(TraceEngineError::DowncastError("entity_uid binary cast"))?;

        let start_col = batch.column_by_name(START_TIME_COL).ok_or_else(|| {
            TraceEngineError::UnsupportedOperation("missing start_time".to_string())
        })?;
        let start_cast = compute::cast(
            start_col,
            &DataType::Timestamp(TimeUnit::Microsecond, Some("UTC".into())),
        )?;
        let start_arr = start_cast
            .as_any()
            .downcast_ref::<TimestampMicrosecondArray>()
            .ok_or(TraceEngineError::DowncastError("start_time timestamp cast"))?;

        for i in 0..batch.num_rows() {
            let trace = trace_arr.value(i);
            let entity = entity_arr.value(i);
            if trace.len() != 16 || entity.len() != 16 {
                continue;
            }

            let mut trace_id = [0u8; 16];
            trace_id.copy_from_slice(trace);

            let mut entity_uid = [0u8; 16];
            entity_uid.copy_from_slice(entity);

            let micros = start_arr.value(i);
            let start_time = DateTime::<Utc>::from_timestamp_micros(micros)
                .ok_or(TraceEngineError::InvalidTimestamp("dispatch start_time"))?;

            items.push(DispatchCandidate {
                trace_id,
                entity_uid,
                start_time,
            });
        }
    }

    Ok(items)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::ObjectStore;
    use datafusion::catalog::CatalogProvider;
    use scouter_settings::ObjectStorageSettings;
    use tempfile::TempDir;

    fn make_local_settings(dir: &TempDir) -> ObjectStorageSettings {
        ObjectStorageSettings {
            storage_uri: dir.path().to_string_lossy().to_string(),
            ..ObjectStorageSettings::default()
        }
    }

    fn make_test_ctx(object_store: &ObjectStore) -> Arc<SessionContext> {
        Arc::new(
            object_store
                .get_session_with_catalog(
                    crate::parquet::tracing::engine::TRACE_CATALOG_NAME,
                    "default",
                )
                .unwrap(),
        )
    }

    fn make_test_catalog(ctx: &Arc<SessionContext>) -> Arc<TraceCatalogProvider> {
        let catalog = Arc::new(TraceCatalogProvider::new());
        ctx.register_catalog(
            crate::parquet::tracing::engine::TRACE_CATALOG_NAME,
            Arc::clone(&catalog) as Arc<dyn CatalogProvider>,
        );
        catalog
    }

    #[tokio::test]
    async fn pending_dispatch_returns_candidates_without_ack() -> Result<(), TraceEngineError> {
        let dir = tempfile::tempdir().unwrap();
        let settings = make_local_settings(&dir);
        let object_store = ObjectStore::new(&settings).unwrap();
        let ctx = make_test_ctx(&object_store);
        let catalog = make_test_catalog(&ctx);
        let service = TraceDispatchService::new(&object_store, ctx, catalog).await?;

        let now = Utc::now();
        let candidate_only_trace = [1u8; 16];
        let candidate_only_entity = [11u8; 16];
        let acked_trace = [2u8; 16];
        let acked_entity = [22u8; 16];

        // Candidate-only key (duplicated to validate key-level deduplication).
        service
            .write_records(vec![
                TraceDispatchRecord {
                    trace_id: candidate_only_trace,
                    entity_uid: candidate_only_entity,
                    start_time: now - chrono::Duration::seconds(20),
                    event_type: DispatchEventType::Candidate,
                    created_at: now,
                },
                TraceDispatchRecord {
                    trace_id: candidate_only_trace,
                    entity_uid: candidate_only_entity,
                    start_time: now - chrono::Duration::seconds(10),
                    event_type: DispatchEventType::Candidate,
                    created_at: now,
                },
            ])
            .await?;

        // Candidate + ack key should be excluded from pending.
        service
            .write_records(vec![
                TraceDispatchRecord {
                    trace_id: acked_trace,
                    entity_uid: acked_entity,
                    start_time: now - chrono::Duration::seconds(15),
                    event_type: DispatchEventType::Candidate,
                    created_at: now,
                },
                TraceDispatchRecord {
                    trace_id: acked_trace,
                    entity_uid: acked_entity,
                    start_time: now - chrono::Duration::seconds(5),
                    event_type: DispatchEventType::Ack,
                    created_at: now,
                },
            ])
            .await?;

        let page = service
            .query_service
            .get_pending_dispatch(now - chrono::Duration::minutes(5), 50, None)
            .await?;

        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].trace_id, candidate_only_trace);
        assert_eq!(page.items[0].entity_uid, candidate_only_entity);
        assert!(!page.has_next);

        service.shutdown().await?;
        Ok(())
    }

    #[tokio::test]
    async fn pending_dispatch_paginates_unacked_candidates() -> Result<(), TraceEngineError> {
        let dir = tempfile::tempdir().unwrap();
        let settings = make_local_settings(&dir);
        let object_store = ObjectStore::new(&settings).unwrap();
        let ctx = make_test_ctx(&object_store);
        let catalog = make_test_catalog(&ctx);
        let service = TraceDispatchService::new(&object_store, ctx, catalog).await?;

        let now = Utc::now();
        let older_start = now - chrono::Duration::seconds(30);
        let newer_start = now - chrono::Duration::seconds(10);

        service
            .write_records(vec![
                TraceDispatchRecord {
                    trace_id: [3u8; 16],
                    entity_uid: [33u8; 16],
                    start_time: older_start,
                    event_type: DispatchEventType::Candidate,
                    created_at: now,
                },
                TraceDispatchRecord {
                    trace_id: [4u8; 16],
                    entity_uid: [44u8; 16],
                    start_time: newer_start,
                    event_type: DispatchEventType::Candidate,
                    created_at: now,
                },
            ])
            .await?;

        let first = service
            .query_service
            .get_pending_dispatch(now - chrono::Duration::minutes(5), 1, None)
            .await?;
        assert_eq!(first.items.len(), 1);
        assert!(first.has_next);
        assert_eq!(first.items[0].trace_id, [4u8; 16]);

        let second = service
            .query_service
            .get_pending_dispatch(
                now - chrono::Duration::minutes(5),
                1,
                first.next_cursor.as_ref(),
            )
            .await?;
        assert_eq!(second.items.len(), 1);
        assert!(!second.has_next);
        assert_eq!(second.items[0].trace_id, [3u8; 16]);

        service.shutdown().await?;
        Ok(())
    }
}
