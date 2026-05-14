use arrow::datatypes::{DataType, Schema};
use deltalake::datafusion::parquet::basic::{Compression, Encoding, ZstdLevel};
use deltalake::datafusion::parquet::file::properties::{
    EnabledStatistics, WriterProperties, WriterPropertiesBuilder,
};
use deltalake::datafusion::parquet::schema::types::ColumnPath;
use scouter_types::dataset::schema::{SCOUTER_BATCH_ID, SCOUTER_CREATED_AT};

pub const DEFAULT_ROW_GROUP_ROWS: usize = 32_768;
pub const DEFAULT_ZSTD_LEVEL: i32 = 3;

fn path(name: &str) -> ColumnPath {
    ColumnPath::new(vec![name.to_string()])
}

fn zstd() -> Compression {
    Compression::ZSTD(ZstdLevel::try_new(DEFAULT_ZSTD_LEVEL).unwrap())
}

fn base_builder() -> WriterPropertiesBuilder {
    WriterProperties::builder()
        .set_max_row_group_row_count(Some(DEFAULT_ROW_GROUP_ROWS))
        .set_compression(zstd())
}

fn with_bloom(
    builder: WriterPropertiesBuilder,
    column: &str,
    fpp: f64,
    ndv: u64,
) -> WriterPropertiesBuilder {
    builder
        .set_column_bloom_filter_enabled(path(column), true)
        .set_column_bloom_filter_fpp(path(column), fpp)
        .set_column_bloom_filter_ndv(path(column), ndv)
}

fn with_page_stats(builder: WriterPropertiesBuilder, column: &str) -> WriterPropertiesBuilder {
    builder.set_column_statistics_enabled(path(column), EnabledStatistics::Page)
}

fn with_delta_encoding(builder: WriterPropertiesBuilder, column: &str) -> WriterPropertiesBuilder {
    builder.set_column_encoding(path(column), Encoding::DELTA_BINARY_PACKED)
}

pub fn trace_span_writer_props() -> WriterProperties {
    let builder = base_builder();
    let builder = with_bloom(builder, "trace_id", 0.01, DEFAULT_ROW_GROUP_ROWS as u64);
    let builder = with_bloom(builder, "service_name", 0.01, 256);
    let builder = with_bloom(builder, "service_namespace", 0.01, 256);
    let builder = with_bloom(builder, "service_version", 0.01, 256);
    let builder = with_bloom(builder, "span_name", 0.01, DEFAULT_ROW_GROUP_ROWS as u64);
    let builder = with_page_stats(builder, "start_time");
    let builder = with_page_stats(builder, "status_code");
    let builder = with_delta_encoding(builder, "start_time");
    let builder = with_delta_encoding(builder, "duration_ms");

    builder
        .set_column_dictionary_enabled(path("span_name"), true)
        .build()
}

pub fn trace_summary_writer_props() -> WriterProperties {
    let builder = base_builder();
    let builder = with_bloom(builder, "service_name", 0.01, 256);
    let builder = with_bloom(builder, "service_namespace", 0.01, 256);
    let builder = with_bloom(builder, "service_version", 0.01, 256);

    builder.build()
}

pub fn genai_span_writer_props() -> WriterProperties {
    let builder = base_builder();
    let builder = with_bloom(builder, "trace_id", 0.01, DEFAULT_ROW_GROUP_ROWS as u64);
    let builder = with_bloom(builder, "service_name", 0.01, 256);
    let builder = with_bloom(builder, "service_namespace", 0.01, 256);
    let builder = with_bloom(builder, "service_version", 0.01, 256);
    let builder = with_bloom(builder, "conversation_id", 0.01, 8_192);
    let builder = with_bloom(builder, "entity_id", 0.01, 128);
    let builder = with_delta_encoding(builder, "start_time");
    let builder = with_delta_encoding(builder, "duration_ms");
    let builder = with_delta_encoding(builder, "input_tokens");
    let builder = with_delta_encoding(builder, "output_tokens");

    builder.build()
}

pub fn trace_dispatch_writer_props() -> WriterProperties {
    let builder = base_builder();
    let builder = with_bloom(builder, "trace_id", 0.01, DEFAULT_ROW_GROUP_ROWS as u64);
    let builder = with_bloom(builder, "entity_uid", 0.01, DEFAULT_ROW_GROUP_ROWS as u64);
    let builder = with_page_stats(builder, "start_time");
    let builder = with_delta_encoding(builder, "start_time");

    builder.build()
}

pub fn eval_scenario_writer_props() -> WriterProperties {
    let builder = base_builder();
    let builder = with_bloom(builder, "collection_id", 0.01, 10_000);

    builder.build()
}

pub fn control_writer_props() -> WriterProperties {
    WriterProperties::builder()
        .set_max_row_group_row_count(Some(1_024))
        .set_compression(zstd())
        .build()
}

pub fn bifrost_writer_props(schema: &Schema) -> WriterProperties {
    let mut builder = base_builder();
    builder = with_delta_encoding(builder, SCOUTER_CREATED_AT);
    builder = with_bloom(builder, SCOUTER_BATCH_ID, 0.01, 10_000);
    builder = with_page_stats(builder, SCOUTER_CREATED_AT);

    for field in schema.fields() {
        let name = field.name();
        if (name.ends_with("_id") || name.ends_with("_key"))
            && matches!(
                field.data_type(),
                DataType::Utf8 | DataType::Utf8View | DataType::LargeUtf8
            )
            && name != SCOUTER_BATCH_ID
        {
            builder = with_bloom(builder, name, 0.01, 10_000);
        }
    }

    builder.build()
}

pub fn column_path_for_test(name: &str) -> ColumnPath {
    path(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use deltalake::datafusion::parquet::basic::Compression;

    #[test]
    fn trace_span_writer_props_enable_lookup_pruning() {
        let props = trace_span_writer_props();
        let trace_id = column_path_for_test("trace_id");
        let start_time = column_path_for_test("start_time");
        let duration = column_path_for_test("duration_ms");

        assert_eq!(
            props.max_row_group_row_count(),
            Some(DEFAULT_ROW_GROUP_ROWS)
        );
        assert!(props.bloom_filter_properties(&trace_id).is_some());
        assert_eq!(
            props.statistics_enabled(&start_time),
            EnabledStatistics::Page
        );
        assert_eq!(
            props.encoding(&duration),
            Some(Encoding::DELTA_BINARY_PACKED)
        );
        assert!(matches!(props.compression(&trace_id), Compression::ZSTD(_)));
    }

    #[test]
    fn table_profiles_share_row_group_policy() {
        let profiles = [
            trace_summary_writer_props(),
            genai_span_writer_props(),
            trace_dispatch_writer_props(),
            eval_scenario_writer_props(),
        ];

        for props in profiles {
            assert_eq!(
                props.max_row_group_row_count(),
                Some(DEFAULT_ROW_GROUP_ROWS)
            );
        }
    }
}
