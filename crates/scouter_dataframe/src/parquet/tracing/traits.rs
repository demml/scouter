use arrow::datatypes::*;
use std::sync::Arc;

pub trait TraceSchemaExt {
    /// Define the Arrow schema for trace spans
    fn create_schema() -> Schema {
        Schema::new(vec![
            // ========== Core Identifiers ==========
            Field::new("trace_id", DataType::FixedSizeBinary(16), false),
            Field::new("span_id", DataType::FixedSizeBinary(8), false),
            Field::new("parent_span_id", DataType::FixedSizeBinary(8), true),
            Field::new("root_span_id", DataType::FixedSizeBinary(8), false),
            // ========== Metadata ==========
            // Dictionary encoding for high-repetition string fields
            Field::new(
                "service_name",
                DataType::Dictionary(Box::new(DataType::Int32), Box::new(DataType::Utf8)),
                false,
            ),
            Field::new("span_name", DataType::Utf8, false),
            Field::new(
                "span_kind",
                DataType::Dictionary(Box::new(DataType::Int8), Box::new(DataType::Utf8)),
                true,
            ),
            // ========== Temporal Data ==========
            Field::new(
                "start_time",
                DataType::Timestamp(TimeUnit::Nanosecond, Some("UTC".into())),
                false,
            ),
            Field::new(
                "end_time",
                DataType::Timestamp(TimeUnit::Nanosecond, Some("UTC".into())),
                true,
            ),
            Field::new("duration_ms", DataType::Int64, true),
            // ========== Status ==========
            Field::new("status_code", DataType::Int32, false),
            Field::new("status_message", DataType::Utf8, true),
            // ========== Hierarchy/Navigation ==========
            Field::new("depth", DataType::Int32, false),
            Field::new("span_order", DataType::Int32, false),
            Field::new(
                "path",
                DataType::List(Arc::new(Field::new("item", DataType::Utf8, false))),
                false,
            ),
            Field::new(
                "attributes",
                DataType::Map(
                    Arc::new(Field::new(
                        "entries",
                        DataType::Struct(
                            vec![
                                Field::new("key", DataType::Utf8, false),
                                Field::new("value", DataType::Utf8View, true),
                            ]
                            .into(),
                        ),
                        false,
                    )),
                    false,
                ),
                false,
            ),
            // ========== Events (Nested) ==========
            Field::new(
                "events",
                DataType::List(Arc::new(Field::new(
                    "item",
                    DataType::Struct(
                        vec![
                            Field::new("name", DataType::Utf8, false),
                            Field::new(
                                "timestamp",
                                DataType::Timestamp(TimeUnit::Nanosecond, Some("UTC".into())),
                                false,
                            ),
                            Field::new(
                                "attributes",
                                DataType::Map(
                                    Arc::new(Field::new(
                                        "entries",
                                        DataType::Struct(
                                            vec![
                                                Field::new("key", DataType::Utf8, false),
                                                Field::new("value", DataType::Utf8View, true),
                                            ]
                                            .into(),
                                        ),
                                        false,
                                    )),
                                    false,
                                ),
                                false,
                            ),
                            Field::new("dropped_attributes_count", DataType::UInt32, false),
                        ]
                        .into(),
                    ),
                    false,
                ))),
                false,
            ),
            // ========== Links (Nested) ==========
            Field::new(
                "links",
                DataType::List(Arc::new(Field::new(
                    "item",
                    DataType::Struct(
                        vec![
                            Field::new("trace_id", DataType::FixedSizeBinary(16), false),
                            Field::new("span_id", DataType::FixedSizeBinary(8), false),
                            Field::new("trace_state", DataType::Utf8, true),
                            Field::new(
                                "attributes",
                                DataType::Map(
                                    Arc::new(Field::new(
                                        "entries",
                                        DataType::Struct(
                                            vec![
                                                Field::new("key", DataType::Utf8, false),
                                                Field::new("value", DataType::Utf8View, true),
                                            ]
                                            .into(),
                                        ),
                                        false,
                                    )),
                                    false,
                                ),
                                false,
                            ),
                            Field::new("dropped_attributes_count", DataType::UInt32, false),
                        ]
                        .into(),
                    ),
                    false,
                ))),
                false,
            ),
            // ========== Payload (Large JSON) ==========
            // Use Utf8View for potentially very large input/output values
            Field::new("input", DataType::Utf8View, true),
            Field::new("output", DataType::Utf8View, true),
            // ========== Full-Text Search Optimization ==========
            // Pre-computed concatenated search string to avoid JSON parsing
            Field::new("search_blob", DataType::Utf8View, false),
        ])
    }
}

//#[async_trait]
//pub trait TraceWriterExt: DeltaTableExt {
//    /// Background task: compact small files every hour
//    ///
//    /// This handles the small files created by frequent flushes
//    pub async fn start_auto_compaction(
//        self: Arc<Self>,
//        interval_minutes: u64,
//    ) -> Result<(), DataFrameError> {
//        let mut ticker = interval(Duration::from_secs(interval_minutes * 60));
//
//        loop {
//            ticker.tick().await;
//
//            if let Err(e) = self.optimize_table().await {
//                tracing::warn!("Auto-compaction failed: {}", e);
//            }
//        }
//    }
//
//    /// Compact small files + apply Z-ORDER
//    pub async fn optimize_table(&self) -> Result<(), DataFrameError> {
//        let table = self.table();
//        table.update_state().await?;
//        table
//            .optimize()
//            .with_target_size(128 * 1024 * 1024)
//            .with_type(OptimizeType::ZOrder(vec![
//                "start_time".to_string(),
//                "service_name".to_string(),
//                "trace_id".to_string(),
//            ])).await?;
//
//        Ok(())
//    }
//
//    /// High-throughput streaming writer with real-time query support
//    ///
//    /// Strategy:
//    /// - Flush every 5 seconds (configurable) for query latency
//    /// - OR flush at 10K spans (smaller batches for responsiveness)
//    /// - Delta Lake handles small file compaction via auto-optimize
//    pub async fn start_streaming_writer(
//        self: Arc<Self>,
//        rx: mpsc::Receiver<Vec<TraceSpan>>,
//        flush_interval_secs: u64,
//        max_batch_size: usize,
//    ) -> Result<(), DataFrameError> {
//        let table_url = Url::parse(&format!("{}/{}", self.storage_root(), self.table_name()))?;
//
//        let mut table = DeltaTableBuilder::from_url(table_url)?.build()?;
//
//        table
//            .optimize()
//            .with_target_size(128 * 1024 * 1024)
//            .with_type(OptimizeType::ZOrder(vec![
//                "start_time".to_string(),
//                "service_name".to_string(),
//                "trace_id".to_string(),
//            ]))
//            .await?;
//
//        self.streaming_write_loop(
//            rx,
//            &mut table,
//            &mut writer,
//            flush_interval_secs,
//            max_batch_size,
//        )
//        .await
//    }
//
//    async fn streaming_write_loop(
//        &self,
//        mut rx: mpsc::Receiver<Vec<TraceSpan>>,
//        table: &mut deltalake::DeltaTable,
//        writer: &mut DeltaWriter,
//        flush_interval_secs: u64,
//        max_batch_size: usize,
//    ) -> Result<(), DataFrameError> {
//        let mut buffer = Vec::with_capacity(max_batch_size);
//        let mut flush_timer = interval(Duration::from_secs(flush_interval_secs));
//        let mut last_flush = Instant::now();
//
//        loop {
//            tokio::select! {
//                // Receive incoming spans
//                Some(spans) = rx.recv() => {
//                    buffer.extend(spans);
//
//                    // Flush if buffer reaches threshold
//                    if buffer.len() >= max_batch_size {
//                        self.flush_buffer(&mut buffer, writer, table).await?;
//                        last_flush = Instant::now();
//                    }
//                }
//
//                // Time-based flush for low-volume periods
//                _ = flush_timer.tick() => {
//                    if !buffer.is_empty() && last_flush.elapsed().as_secs() >= flush_interval_secs {
//                        self.flush_buffer(&mut buffer, writer, table).await?;
//                        last_flush = Instant::now();
//                    }
//                }
//
//                // Channel closed - final flush
//                else => {
//                    if !buffer.is_empty() {
//                        self.flush_buffer(&mut buffer, writer, table).await?;
//                    }
//                    break;
//                }
//            }
//        }
//
//        Ok(())
//    }
//
//    async fn flush_buffer(
//        &self,
//        buffer: &mut Vec<TraceSpan>,
//        table: &mut DeltaTable,
//    ) -> Result<(), DataFrameError> {
//        if buffer.is_empty() {
//            return Ok(());
//        }
//
//        let batch = self.build_batch(std::mem::take(buffer))?;
//
//        let properties = WriterProperties::builder()
//                    .set_compression(Compression::SNAPPY)
//                    .build();
//
//        let builder  = table.clone().write(vec![batch]).with_writer_properties(
//                WriterProperties::builder()
//                    .set_compression(Compression::SNAPPY)
//                    .build(),
//            )
//            .await?;
//
//        WriteBuilder
//
//
//
//        deltalake::operations::DeltaOps::from(table.clone())
//            .write(vec![batch])
//            .with_writer_properties(
//                WriterProperties::builder()
//                    .set_compression(Compression::SNAPPY)
//                    .build(),
//            )
//            .await?;
//
//        // Reload table metadata for next write
//        *table = DeltaTableBuilder::from_url(Url::parse(&format!(
//            "{}/{}",
//            self.storage_root(),
//            self.table_name()
//        ))?)?
//        .build()?;
//
//        Ok(())
//    }
//}
//
//}
