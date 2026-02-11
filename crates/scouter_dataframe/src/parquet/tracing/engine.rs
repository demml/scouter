use crate::error::TraceEngineError;
use crate::parquet::tracing::traits::arrow_schema_to_delta;
use crate::parquet::tracing::traits::attribute_field;
use crate::parquet::tracing::traits::TraceSchemaExt;
use crate::storage::ObjectStore;
use arrow::array::*;
use arrow::datatypes::*;
use arrow_array::RecordBatch;
use datafusion::prelude::SessionContext;
use deltalake::operations::optimize::OptimizeType;
use deltalake::DeltaTable;
use scouter_settings::ObjectStorageSettings;
use scouter_types::sql::TraceSpan;
use scouter_types::SpanId;
use scouter_types::TraceId;
use scouter_types::{Attribute, SpanEvent, SpanLink};
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::oneshot;
use tokio::sync::{mpsc, RwLock as AsyncRwLock};
use tokio::time::{interval, Duration};
use tracing::{debug, error, info, instrument};
use url::Url;
const TRACE_SPAN_TABLE_NAME: &str = "trace_spans";

pub enum TableCommand {
    Write {
        spans: Vec<TraceSpan>,
        respond_to: oneshot::Sender<Result<(), TraceEngineError>>,
    },
    Optimize {
        respond_to: oneshot::Sender<Result<(), TraceEngineError>>,
    },
    Shutdown,
}

async fn build_url(object_store: &ObjectStore) -> Result<Url, TraceEngineError> {
    let base_url = object_store.get_base_url()?; // Use existing method
    Ok(base_url)
}

#[instrument(skip_all)]
async fn create_table(table_url: Url, schema: SchemaRef) -> Result<DeltaTable, TraceEngineError> {
    info!("Creating new Delta table at URL: {}", table_url);

    let table = DeltaTable::try_from_url(table_url).await?;

    let delta_fields = arrow_schema_to_delta(&schema);

    table
        .create()
        .with_table_name(TRACE_SPAN_TABLE_NAME)
        .with_columns(delta_fields)
        .await
        .map_err(Into::into)
}

#[instrument(skip_all)]
async fn build_or_create_table(
    object_store: &ObjectStore,
    schema: SchemaRef,
) -> Result<DeltaTable, TraceEngineError> {
    let table_url = build_url(object_store).await?;

    info!("Attempting to load table at URL: {}", table_url);

    // For local filesystem, ensure directory exists
    if table_url.scheme() == "file" {
        if let Ok(path) = table_url.to_file_path() {
            if !path.exists() {
                info!("Creating directory for local table: {:?}", path);
                std::fs::create_dir_all(&path)?;
            }
        }
    }

    match DeltaTable::try_from_url(table_url.clone()).await {
        Ok(table) => {
            info!("Loaded existing Delta table");
            Ok(table)
        }
        Err(deltalake::DeltaTableError::NotATable(_)) => {
            info!("Table does not exist, creating new table");
            create_table(table_url, schema).await
        }
        Err(e) => Err(e.into()),
    }
}
/// Core trace span dataframe for high-throughput observability workloads
///
/// Design decisions:
/// - Dictionary encoding for service_name, span_kind (high cardinality, high repetition)
/// - FixedSizeBinary for IDs (compact representation vs hex strings)
/// - Nested structures for events/links to maintain relational integrity
/// - Search blob for full-text queries without parsing JSON
/// - Attribute shredding foundation for future optimization
pub struct TraceSpanDBEngine {
    schema: Arc<Schema>,
    pub object_store: ObjectStore,
    table: Arc<AsyncRwLock<DeltaTable>>,
    pub ctx: Arc<SessionContext>,
}

impl TraceSchemaExt for TraceSpanDBEngine {}

impl TraceSpanDBEngine {
    pub async fn new(storage_settings: &ObjectStorageSettings) -> Result<Self, TraceEngineError> {
        let object_store = ObjectStore::new(storage_settings)?;
        let schema = Arc::new(Self::create_schema());
        let delta_table = build_or_create_table(&object_store, schema.clone()).await?;
        let ctx = object_store.get_session()?;
        ctx.register_table(TRACE_SPAN_TABLE_NAME, Arc::new(delta_table.clone()))?;

        Ok(TraceSpanDBEngine {
            schema,
            object_store,
            table: Arc::new(AsyncRwLock::new(delta_table)),
            ctx: Arc::new(ctx),
        })
    }

    /// Build a RecordBatch from a vector of TraceSpan records
    pub fn build_batch(&self, spans: Vec<TraceSpan>) -> Result<RecordBatch, TraceEngineError> {
        // we need to time the batch building process to identify bottlenecks
        let start_time = std::time::Instant::now();
        let mut builder = TraceSpanBatchBuilder::new(self.schema.clone());

        for span in spans {
            builder.append(&span)?;
        }

        let record_batch = builder
            .finish()
            .inspect_err(|e| error!("Failed to build RecordBatch: {}", e))?;

        let duration = start_time.elapsed();
        debug!(
            "Built RecordBatch with {} rows in {:?}",
            record_batch.num_rows(),
            duration
        );
        Ok(record_batch)
    }

    /// Helper to write spans directly to the Delta table
    /// Write will consume current table state and return updated table
    async fn write_spans(&self, spans: Vec<TraceSpan>) -> Result<(), TraceEngineError> {
        info!("Engine received write request for {} spans", spans.len());

        let batch = self
            .build_batch(spans)
            .inspect_err(|e| error!("failed to build batch: {:?}", e))?;
        info!("Built batch with {} rows", batch.num_rows());

        let mut table_guard = self.table.write().await;
        info!("Acquired table write lock");

        // Try to update table state, but ignore if table is freshly created
        if let Err(e) = table_guard.update_incremental(None).await {
            // If table is new, it won't have log files yet - this is expected
            info!("Table update skipped (table may be newly created): {}", e);
        }

        let current_table = table_guard.clone();

        let updated_table = current_table
            .write(vec![batch])
            .with_save_mode(deltalake::protocol::SaveMode::Append)
            .await?;

        info!("Successfully wrote batch to Delta Lake");

        // Re-register with SessionContext so queries see the new data
        {
            self.ctx.deregister_table(TRACE_SPAN_TABLE_NAME)?;
            self.ctx
                .register_table(TRACE_SPAN_TABLE_NAME, Arc::new(updated_table.clone()))?;
        }

        *table_guard = updated_table;

        Ok(())
    }

    async fn optimize_table(&self) -> Result<(), TraceEngineError> {
        let mut table_guard = self.table.write().await;

        let current_table = table_guard.clone();

        let (updated_table, _metrics) = current_table
            .optimize()
            .with_target_size(128 * 1024 * 1024)
            .with_type(OptimizeType::ZOrder(vec![
                "start_time".to_string(),
                "service_name".to_string(),
            ]))
            .await?;

        // Re-register with SessionContext
        self.ctx.deregister_table(TRACE_SPAN_TABLE_NAME)?;
        self.ctx
            .register_table(TRACE_SPAN_TABLE_NAME, Arc::new(updated_table.clone()))?;

        *table_guard = updated_table;

        Ok(())
    }

    #[instrument(skip_all, name = "buffering_actor")]
    pub fn start_actor(
        self,
        compaction_interval_hours: u64,
    ) -> (mpsc::Sender<TableCommand>, tokio::task::JoinHandle<()>) {
        let (tx, mut rx) = mpsc::channel::<TableCommand>(100);

        let handle = tokio::spawn(async move {
            let mut compaction_ticker =
                interval(Duration::from_secs(compaction_interval_hours * 3600));
            compaction_ticker.tick().await;

            loop {
                tokio::select! {
                    Some(cmd) = rx.recv() => {
                        match cmd {
                            TableCommand::Write { spans, respond_to } => {
                                match self.write_spans(spans).await {
                                    Ok(_) => {
                                        let _ = respond_to.send(Ok(()));
                                    }
                                    Err(e) => {
                                        tracing::error!("Write failed: {}", e);
                                        let _ = respond_to.send(Err(e));
                                    }
                                }
                            }
                            TableCommand::Optimize { respond_to } => {
                                match self.optimize_table().await {
                                    Ok(_) => {
                                        tracing::info!("Compaction completed");
                                        let _ = respond_to.send(Ok(()));
                                    }
                                    Err(e) => {
                                        tracing::error!("Compaction failed: {}", e);
                                        let _ = respond_to.send(Err(e));
                                    }
                                }
                            }
                            TableCommand::Shutdown => {
                                tracing::info!("Shutting down table engine");
                                break;
                            }
                        }
                    }
                    _ = compaction_ticker.tick() => {
                        if let Err(e) = self.optimize_table().await {
                            tracing::error!("Scheduled compaction failed: {}", e);
                        } else {
                            tracing::info!("Scheduled compaction completed");
                        }
                    }
                }
            }
        });

        (tx, handle)
    }
}

/// Efficient builder for converting TraceSpan records into Arrow RecordBatch
///
/// Design notes:
/// - Pre-allocates builders to minimize reallocations
/// - Uses type-safe builders to catch schema mismatches at compile time
/// - Handles null values properly for optional fields
pub struct TraceSpanBatchBuilder {
    schema: SchemaRef,

    // ID builders
    trace_id: FixedSizeBinaryBuilder,
    span_id: FixedSizeBinaryBuilder,
    parent_span_id: FixedSizeBinaryBuilder,
    root_span_id: FixedSizeBinaryBuilder,

    // Metadata builders
    service_name: StringDictionaryBuilder<Int32Type>,
    span_name: StringBuilder,
    span_kind: StringDictionaryBuilder<Int8Type>,

    // Time builders
    start_time: TimestampMicrosecondBuilder,
    end_time: TimestampMicrosecondBuilder,
    duration_ms: Int64Builder,

    // Status builders
    status_code: Int32Builder,
    status_message: StringBuilder,

    // Hierarchy builders
    depth: Int32Builder,
    span_order: Int32Builder,
    path: ListBuilder<StringBuilder>,

    // Attribute builders
    attributes: MapBuilder<StringBuilder, StringViewBuilder>,

    // Nested structure builders
    events: ListBuilder<StructBuilder>,
    links: ListBuilder<StructBuilder>,

    // Payload builders
    input: StringViewBuilder,
    output: StringViewBuilder,

    // Search optimizer
    search_blob: StringViewBuilder,
}

impl TraceSpanBatchBuilder {
    pub fn new(schema: SchemaRef) -> Self {
        // Initialize all builders
        let trace_id = FixedSizeBinaryBuilder::new(16);
        let span_id = FixedSizeBinaryBuilder::new(8);
        let parent_span_id = FixedSizeBinaryBuilder::new(8);
        let root_span_id = FixedSizeBinaryBuilder::new(8);

        let service_name = StringDictionaryBuilder::<Int32Type>::new();
        let span_name = StringBuilder::new();
        let span_kind = StringDictionaryBuilder::<Int8Type>::new();

        let start_time = TimestampMicrosecondBuilder::new().with_timezone("UTC");
        let end_time = TimestampMicrosecondBuilder::new().with_timezone("UTC");
        let duration_ms = Int64Builder::new();

        let status_code = Int32Builder::new();
        let status_message = StringBuilder::new();

        let depth = Int32Builder::new();
        let span_order = Int32Builder::new();
        let path = ListBuilder::new(StringBuilder::new());

        let map_field_name = MapFieldNames {
            entry: "key_value".to_string(),
            key: "key".to_string(),
            value: "value".to_string(),
        };
        let attributes = MapBuilder::new(
            Some(map_field_name.clone()),
            StringBuilder::new(),
            StringViewBuilder::new(),
        );

        // Events list builder - must match SpanEvent struct
        let event_fields = vec![
            Field::new("name", DataType::Utf8, false),
            Field::new(
                "timestamp",
                DataType::Timestamp(TimeUnit::Microsecond, Some("UTC".into())),
                false,
            ),
            attribute_field(),
            Field::new("dropped_attributes_count", DataType::UInt32, false),
        ];

        let event_struct_builders = vec![
            Box::new(StringBuilder::new()) as Box<dyn ArrayBuilder>,
            Box::new(TimestampMicrosecondBuilder::new().with_timezone("UTC"))
                as Box<dyn ArrayBuilder>,
            Box::new(MapBuilder::new(
                Some(map_field_name.clone()),
                StringBuilder::new(),
                StringViewBuilder::new(),
            )) as Box<dyn ArrayBuilder>,
            Box::new(UInt32Builder::new()) as Box<dyn ArrayBuilder>,
        ];

        let event_struct_builder = StructBuilder::new(event_fields, event_struct_builders);
        let events = ListBuilder::new(event_struct_builder);

        // Links list builder - must match SpanLink struct
        let link_fields = vec![
            Field::new("trace_id", DataType::FixedSizeBinary(16), false),
            Field::new("span_id", DataType::FixedSizeBinary(8), false),
            Field::new("trace_state", DataType::Utf8, false),
            attribute_field(),
            Field::new("dropped_attributes_count", DataType::UInt32, false),
        ];

        let link_struct_builders = vec![
            Box::new(FixedSizeBinaryBuilder::new(16)) as Box<dyn ArrayBuilder>,
            Box::new(FixedSizeBinaryBuilder::new(8)) as Box<dyn ArrayBuilder>,
            Box::new(StringBuilder::new()) as Box<dyn ArrayBuilder>,
            Box::new(MapBuilder::new(
                Some(map_field_name.clone()),
                StringBuilder::new(),
                StringViewBuilder::new(),
            )) as Box<dyn ArrayBuilder>,
            Box::new(UInt32Builder::new()) as Box<dyn ArrayBuilder>,
        ];

        let link_struct_builder = StructBuilder::new(link_fields, link_struct_builders);
        let links = ListBuilder::new(link_struct_builder);

        let input = StringViewBuilder::new();
        let output = StringViewBuilder::new();
        let search_blob = StringViewBuilder::new();

        Self {
            schema,
            trace_id,
            span_id,
            parent_span_id,
            root_span_id,
            service_name,
            span_name,
            span_kind,
            start_time,
            end_time,
            duration_ms,
            status_code,
            status_message,
            depth,
            span_order,
            path,
            attributes,
            events,
            links,
            input,
            output,
            search_blob,
        }
    }

    /// Append a single TraceSpan to the batch
    pub fn append(&mut self, span: &TraceSpan) -> Result<(), TraceEngineError> {
        // IDs - convert hex strings to binary
        Self::append_id_as_bytes(&span.trace_id, &mut self.trace_id, 16).inspect_err(|e| {
            error!("Failed to append trace_id for span {}: {}", span.span_id, e);
        })?;
        Self::append_id_as_bytes(&span.span_id, &mut self.span_id, 8).inspect_err(|e| {
            error!("Failed to append span_id for span {}: {}", span.span_id, e);
        })?;
        Self::append_id_as_bytes(&span.root_span_id, &mut self.root_span_id, 8).inspect_err(
            |e| {
                error!(
                    "Failed to append root_span_id for span {}: {}",
                    span.span_id, e
                );
            },
        )?;

        match &span.parent_span_id {
            Some(pid) => Self::append_id_as_bytes(pid, &mut self.parent_span_id, 8)?,
            None => self.parent_span_id.append_null(),
        }

        // Metadata
        self.service_name.append_value(&span.service_name);
        self.span_name.append_value(&span.span_name);

        match &span.span_kind {
            Some(kind) => self.span_kind.append_value(kind),
            None => self.span_kind.append_null(),
        }

        // Timestamps
        self.start_time
            .append_value(span.start_time.timestamp_micros());

        self.end_time.append_value(span.end_time.timestamp_micros());

        self.duration_ms.append_value(span.duration_ms);

        // Status
        self.status_code.append_value(span.status_code);
        match &span.status_message {
            Some(msg) => self.status_message.append_value(msg),
            None => self.status_message.append_null(),
        }

        // Hierarchy
        self.depth.append_value(span.depth);
        self.span_order.append_value(span.span_order);

        // Path (list of strings)
        for path_segment in &span.path {
            self.path.values().append_value(path_segment);
        }
        self.path.append(true);

        // Attributes (map)
        self.append_attributes(&span.attributes).inspect_err(|e| {
            error!(
                "Failed to append attributes for span {}: {}",
                span.span_id, e
            );
        })?;

        // Events (nested list of structs)
        self.append_events(&span.events).inspect_err(|e| {
            error!("Failed to append events for span {}: {}", span.span_id, e);
        })?;

        // Links (nested list of structs)
        self.append_links(&span.links).inspect_err(|e| {
            error!("Failed to append links for span {}: {}", span.span_id, e);
        })?;

        // Payloads (potentially large JSON)
        match &span.input {
            Some(v) => self.input.append_value(v.to_string()),
            None => self.input.append_null(),
        }

        match &span.output {
            Some(v) => self.output.append_value(v.to_string()),
            None => self.output.append_null(),
        }

        // Search blob - concatenate searchable fields
        let search_text = self.build_search_blob(span);
        self.search_blob.append_value(search_text);

        Ok(())
    }

    /// Convert hex string ID to binary and append
    fn append_id_as_bytes(
        hex_str: &str,
        builder: &mut FixedSizeBinaryBuilder,
        expected_size: usize,
    ) -> Result<(), TraceEngineError> {
        match expected_size {
            16 => {
                let bytes = TraceId::hex_to_bytes(hex_str)?;
                builder.append_value(&bytes)?;
            }
            8 => {
                let bytes = SpanId::hex_to_bytes(hex_str)?;
                builder.append_value(&bytes)?;
            }
            _ => {
                return Err(TraceEngineError::InvalidHexId(
                    hex_str.to_string(),
                    "Unsupported ID size".to_string(),
                ))
            }
        }
        Ok(())
    }

    /// Append attributes as a map (keys must be sorted)
    fn append_attributes(&mut self, attributes: &[Attribute]) -> Result<(), TraceEngineError> {
        for attr in attributes {
            self.attributes.keys().append_value(&attr.key);

            // Convert serde_json::Value to string for storage
            let value_str = match &attr.value {
                Value::String(s) => s.clone(),
                Value::Null => String::new(),
                other => other.to_string(),
            };

            self.attributes.values().append_value(value_str);
        }
        self.attributes.append(true)?;
        Ok(())
    }

    /// Append events as a list of structs
    fn append_events(&mut self, events: &[SpanEvent]) -> Result<(), TraceEngineError> {
        let event_struct = self.events.values();
        for event in events {
            // Event name
            let name_builder = event_struct
                .field_builder::<StringBuilder>(0)
                .ok_or_else(|| TraceEngineError::DowncastError("event name builder"))?;
            name_builder.append_value(&event.name);

            // Event timestamp
            let time_builder = event_struct
                .field_builder::<TimestampMicrosecondBuilder>(1)
                .ok_or_else(|| TraceEngineError::DowncastError("event timestamp builder"))?;
            time_builder.append_value(event.timestamp.timestamp_micros());

            // Event attributes (nested map) - must be sorted
            let attr_builder = event_struct
                .field_builder::<MapBuilder<StringBuilder, StringViewBuilder>>(2)
                .ok_or_else(|| TraceEngineError::DowncastError("event attributes builder"))?;

            for attr in &event.attributes {
                attr_builder.keys().append_value(&attr.key);
                let value_str = match &attr.value {
                    Value::String(s) => s.clone(),
                    Value::Null => String::new(),
                    other => other.to_string(),
                };
                attr_builder.values().append_value(value_str);
            }
            attr_builder.append(true)?;

            // Dropped attributes count
            let dropped_builder =
                event_struct
                    .field_builder::<UInt32Builder>(3)
                    .ok_or_else(|| {
                        TraceEngineError::DowncastError("dropped attributes count builder")
                    })?;
            dropped_builder.append_value(event.dropped_attributes_count);

            event_struct.append(true);
        }

        self.events.append(true);
        Ok(())
    }

    /// Append links as a list of structs
    fn append_links(&mut self, links: &[SpanLink]) -> Result<(), TraceEngineError> {
        let link_struct = self.links.values();

        for link in links {
            // Link trace_id
            let trace_builder = link_struct
                .field_builder::<FixedSizeBinaryBuilder>(0)
                .ok_or_else(|| TraceEngineError::DowncastError("link trace_id builder"))?;

            let trace_bytes = TraceId::hex_to_bytes(&link.trace_id).map_err(|e| {
                TraceEngineError::InvalidHexId(link.trace_id.clone(), e.to_string())
            })?;
            trace_builder.append_value(&trace_bytes)?;

            // Link span_id
            let span_builder = link_struct
                .field_builder::<FixedSizeBinaryBuilder>(1)
                .ok_or_else(|| TraceEngineError::DowncastError("link span_id builder"))?;

            let span_bytes = SpanId::hex_to_bytes(&link.span_id)
                .map_err(|e| TraceEngineError::InvalidHexId(link.span_id.clone(), e.to_string()))?;
            span_builder.append_value(&span_bytes)?;

            // Link trace_state - SpanLink.trace_state is String (non-nullable), can be empty
            let state_builder = link_struct
                .field_builder::<StringBuilder>(2)
                .ok_or_else(|| TraceEngineError::DowncastError("link trace_state builder"))?;
            state_builder.append_value(&link.trace_state);

            // Link attributes - must be sorted
            let attr_builder = link_struct
                .field_builder::<MapBuilder<StringBuilder, StringViewBuilder>>(3)
                .ok_or_else(|| TraceEngineError::DowncastError("link attributes builder"))?;

            for attr in &link.attributes {
                attr_builder.keys().append_value(&attr.key);
                let value_str = match &attr.value {
                    Value::String(s) => s.clone(),
                    Value::Null => String::new(),
                    other => other.to_string(),
                };
                attr_builder.values().append_value(value_str);
            }
            attr_builder.append(true)?;

            // Dropped attributes count
            let dropped_builder =
                link_struct
                    .field_builder::<UInt32Builder>(4)
                    .ok_or_else(|| {
                        TraceEngineError::DowncastError("link dropped attributes count builder")
                    })?;
            dropped_builder.append_value(link.dropped_attributes_count);

            link_struct.append(true);
        }

        self.links.append(true);
        Ok(())
    }

    /// Build a concatenated search string for full-text queries
    ///
    /// This avoids parsing JSON during queries by pre-computing searchable text
    fn build_search_blob(&self, span: &TraceSpan) -> String {
        let mut search = String::with_capacity(512);

        // Service and span name
        search.push_str(&span.service_name);
        search.push(' ');
        search.push_str(&span.span_name);
        search.push(' ');

        // Status message
        if let Some(msg) = &span.status_message {
            search.push_str(msg);
            search.push(' ');
        }

        // Attributes (key:value pairs)
        for attr in &span.attributes {
            search.push_str(&attr.key);
            search.push(':');

            let value_str = match &attr.value {
                Value::String(s) => s.as_str(),
                Value::Number(n) => {
                    search.push_str(&n.to_string());
                    continue;
                }
                Value::Bool(b) => {
                    search.push_str(&b.to_string());
                    continue;
                }
                Value::Null => continue,
                _ => {
                    search.push_str(&attr.value.to_string());
                    continue;
                }
            };

            search.push_str(value_str);
            search.push(' ');
        }

        // Event names (for searchability)
        for event in &span.events {
            search.push_str(&event.name);
            search.push(' ');
        }

        search
    }

    /// Finalize and build the RecordBatch
    pub fn finish(mut self) -> Result<RecordBatch, TraceEngineError> {
        let batch = RecordBatch::try_new(
            self.schema.clone(),
            vec![
                Arc::new(self.trace_id.finish()),
                Arc::new(self.span_id.finish()),
                Arc::new(self.parent_span_id.finish()),
                Arc::new(self.root_span_id.finish()),
                Arc::new(self.service_name.finish()),
                Arc::new(self.span_name.finish()),
                Arc::new(self.span_kind.finish()),
                Arc::new(self.start_time.finish()),
                Arc::new(self.end_time.finish()),
                Arc::new(self.duration_ms.finish()),
                Arc::new(self.status_code.finish()),
                Arc::new(self.status_message.finish()),
                Arc::new(self.depth.finish()),
                Arc::new(self.span_order.finish()),
                Arc::new(self.path.finish()),
                Arc::new(self.attributes.finish()),
                Arc::new(self.events.finish()),
                Arc::new(self.links.finish()),
                Arc::new(self.input.finish()),
                Arc::new(self.output.finish()),
                Arc::new(self.search_blob.finish()),
            ],
        )?;

        Ok(batch)
    }
}
