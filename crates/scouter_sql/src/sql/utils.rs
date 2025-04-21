use scouter_error::SqlError;
use scouter_types::{
    CustomMetricServerRecord, PsiServerRecord, RecordType, ServerRecord, ServerRecords,
    SpcServerRecord,
};
use sqlx::{postgres::PgRow, Row};

/// Helper for converting a row to an `SpcServerRecord`.
fn spc_record_from_row(row: &PgRow) -> Result<SpcServerRecord, SqlError> {
    Ok(SpcServerRecord {
        created_at: row
            .try_get("created_at")
            .map_err(|e| SqlError::GeneralError(format!("Failed to extract created_at: {}", e)))?,
        name: row
            .try_get("name")
            .map_err(|e| SqlError::GeneralError(format!("Failed to extract name: {}", e)))?,
        space: row
            .try_get("space")
            .map_err(|e| SqlError::GeneralError(format!("Failed to extract space: {}", e)))?,
        version: row
            .try_get("version")
            .map_err(|e| SqlError::GeneralError(format!("Failed to extract version: {}", e)))?,
        feature: row
            .try_get("feature")
            .map_err(|e| SqlError::GeneralError(format!("Failed to extract feature: {}", e)))?,
        value: row
            .try_get("value")
            .map_err(|e| SqlError::GeneralError(format!("Failed to extract value: {}", e)))?,
    })
}

/// Helper for converting a row to a `PsiServerRecord`.
fn psi_record_from_row(row: &PgRow) -> Result<PsiServerRecord, SqlError> {
    let bin_id: i32 = row
        .try_get("bin_id")
        .map_err(|e| SqlError::GeneralError(format!("Failed to extract bin_id: {}", e)))?;
    let bin_count: i32 = row
        .try_get("bin_count")
        .map_err(|e| SqlError::GeneralError(format!("Failed to extract bin_count: {}", e)))?;

    Ok(PsiServerRecord {
        created_at: row
            .try_get("created_at")
            .map_err(|e| SqlError::GeneralError(format!("Failed to extract created_at: {}", e)))?,
        name: row
            .try_get("name")
            .map_err(|e| SqlError::GeneralError(format!("Failed to extract name: {}", e)))?,
        space: row
            .try_get("space")
            .map_err(|e| SqlError::GeneralError(format!("Failed to extract space: {}", e)))?,
        version: row
            .try_get("version")
            .map_err(|e| SqlError::GeneralError(format!("Failed to extract version: {}", e)))?,
        feature: row
            .try_get("feature")
            .map_err(|e| SqlError::GeneralError(format!("Failed to extract feature: {}", e)))?,
        bin_id: bin_id as usize,
        bin_count: bin_count as usize,
    })
}

/// Helper for converting a row to a `ustomMetricServerRecord`.
fn custom_record_from_row(row: &PgRow) -> Result<CustomMetricServerRecord, SqlError> {
    Ok(CustomMetricServerRecord {
        created_at: row
            .try_get("created_at")
            .map_err(|e| SqlError::GeneralError(format!("Failed to extract created_at: {}", e)))?,
        name: row
            .try_get("name")
            .map_err(|e| SqlError::GeneralError(format!("Failed to extract name: {}", e)))?,
        space: row
            .try_get("space")
            .map_err(|e| SqlError::GeneralError(format!("Failed to extract space: {}", e)))?,
        version: row
            .try_get("version")
            .map_err(|e| SqlError::GeneralError(format!("Failed to extract version: {}", e)))?,
        metric: row
            .try_get("metric")
            .map_err(|e| SqlError::GeneralError(format!("Failed to extract metric: {}", e)))?,
        value: row
            .try_get("value")
            .map_err(|e| SqlError::GeneralError(format!("Failed to extract value: {}", e)))?,
    })
}

/// Converts a slice of `PgRow` to a `ServerRecords` based on the provided `RecordType`.
///
/// # Arguments
/// * `rows` - A slice of `PgRow` to be converted.
/// * `record_type` - The type of record to convert to.
///
/// # Returns
/// * `Result<ServerRecords, SqlError>` - A result containing the converted `ServerRecords` or an error.
///
/// # Errors
/// * Returns an error if the conversion fails or if the record type is not supported.
pub fn pg_rows_to_server_records(
    rows: &[PgRow],
    record_type: &RecordType,
) -> Result<ServerRecords, SqlError> {
    // Get correct conversion function base on record type
    // Returns an error if the record type is not supported
    let convert_fn = match record_type {
        RecordType::Spc => |row| Ok(ServerRecord::Spc(spc_record_from_row(row)?)),
        RecordType::Psi => |row| Ok(ServerRecord::Psi(psi_record_from_row(row)?)),
        RecordType::Custom => |row| Ok(ServerRecord::Custom(custom_record_from_row(row)?)),
        _ => {
            return Err(SqlError::GeneralError(format!(
                "Unsupported record type: {:?}",
                record_type
            )))
        }
    };

    // Pre-allocate vector with exact capacity needed
    let records: Result<Vec<ServerRecord>, SqlError> = rows.iter().map(convert_fn).collect();

    // Convert the result into ServerRecords
    records.map(ServerRecords::new)
}
