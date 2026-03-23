use arrow::error::ArrowError;
use arrow::ipc::reader::StreamReader;
use arrow::ipc::writer::StreamWriter;
use arrow_array::RecordBatch;
use std::io::Cursor;

/// Deserialize Arrow IPC stream bytes into a Vec of RecordBatches.
///
/// Returns an empty Vec for empty input (e.g., zero-row query results).
/// Returns an error only if the bytes are malformed.
pub fn ipc_bytes_to_batches(data: &[u8]) -> Result<Vec<RecordBatch>, ArrowError> {
    if data.is_empty() {
        return Ok(Vec::new());
    }
    let cursor = Cursor::new(data);
    let reader = StreamReader::try_new(cursor, None)?;
    reader.collect()
}

/// Serialize a slice of RecordBatches into Arrow IPC stream bytes.
///
/// Used for query responses. Returns an empty vec if there are no batches.
pub fn batches_to_ipc_bytes(batches: &[RecordBatch]) -> Result<Vec<u8>, ArrowError> {
    if batches.is_empty() {
        return Ok(Vec::new());
    }
    let schema = batches[0].schema();
    let mut buf = Vec::new();
    let mut writer = StreamWriter::try_new(&mut buf, &schema)?;
    for batch in batches {
        writer.write(batch)?;
    }
    writer.finish()?;
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use arrow::datatypes::{DataType, Field, Schema};
    use arrow_array::{Float64Array, Int64Array, StringArray};
    use std::sync::Arc;

    fn test_schema() -> Schema {
        Schema::new(vec![
            Field::new("id", DataType::Int64, false),
            Field::new("name", DataType::Utf8, false),
            Field::new("score", DataType::Float64, true),
        ])
    }

    fn test_batch() -> RecordBatch {
        let schema = Arc::new(test_schema());
        RecordBatch::try_new(
            schema,
            vec![
                Arc::new(Int64Array::from(vec![1, 2, 3])),
                Arc::new(StringArray::from(vec!["alice", "bob", "charlie"])),
                Arc::new(Float64Array::from(vec![Some(0.9), None, Some(0.7)])),
            ],
        )
        .unwrap()
    }

    #[test]
    fn test_round_trip() {
        let batch = test_batch();
        let bytes = batches_to_ipc_bytes(std::slice::from_ref(&batch)).unwrap();
        let decoded = ipc_bytes_to_batches(&bytes).unwrap();

        assert_eq!(1, decoded.len());
        assert_eq!(batch.num_rows(), decoded[0].num_rows());
        assert_eq!(batch.num_columns(), decoded[0].num_columns());
        assert_eq!(batch.schema(), decoded[0].schema());

        // Verify data equality
        let orig_ids: &Int64Array = batch.column(0).as_any().downcast_ref().unwrap();
        let decoded_ids: &Int64Array = decoded[0].column(0).as_any().downcast_ref().unwrap();
        assert_eq!(orig_ids.values(), decoded_ids.values());
    }

    #[test]
    fn test_multiple_batches_round_trip() {
        let batch = test_batch();
        let bytes = batches_to_ipc_bytes(&[batch.clone(), batch.clone()]).unwrap();
        let decoded = ipc_bytes_to_batches(&bytes).unwrap();
        assert_eq!(2, decoded.len());
        assert_eq!(batch.num_rows(), decoded[0].num_rows());
        assert_eq!(batch.num_rows(), decoded[1].num_rows());
    }

    #[test]
    fn test_empty_batches_round_trip() {
        let bytes = batches_to_ipc_bytes(&[]).unwrap();
        assert!(bytes.is_empty());
        // Empty bytes should round-trip to empty batches, not error
        let decoded = ipc_bytes_to_batches(&bytes).unwrap();
        assert!(decoded.is_empty());
    }

    #[test]
    fn test_malformed_bytes() {
        let result = ipc_bytes_to_batches(b"not valid ipc data");
        assert!(result.is_err());
    }
}
