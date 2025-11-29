use crate::sql::error::SqlError;
use crate::sql::schema::llm_drift_record_from_row;
use chrono::{DateTime, Utc};
use scouter_types::{
    CustomMetricInternalRecord, InternalServerRecord, InternalServerRecords,
    LLMMetricInternalRecord, PsiInternalRecord, RecordType, SpcInternalRecord,
};

use sqlx::{postgres::PgRow, Row};
/// Helper for converting a row to an `SpcInternalRecord`.
fn spc_record_from_row(row: &PgRow) -> Result<SpcInternalRecord, SqlError> {
    Ok(SpcInternalRecord {
        created_at: row.try_get("created_at")?,
        entity_id: row.try_get("entity_id")?,
        feature: row.try_get("feature")?,
        value: row.try_get("value")?,
    })
}

/// Helper for converting a row to a `PsiInternalRecord`.
fn psi_record_from_row(row: &PgRow) -> Result<PsiInternalRecord, SqlError> {
    let bin_id: i32 = row.try_get("bin_id")?;
    let bin_count: i32 = row.try_get("bin_count")?;

    Ok(PsiInternalRecord {
        created_at: row.try_get("created_at")?,
        entity_id: row.try_get("entity_id")?,
        feature: row.try_get("feature")?,
        bin_id: bin_id as usize,
        bin_count: bin_count as usize,
    })
}

/// Helper for converting a row to a `CustomMetricInternalRecord`.
fn custom_record_from_row(row: &PgRow) -> Result<CustomMetricInternalRecord, SqlError> {
    Ok(CustomMetricInternalRecord {
        created_at: row.try_get("created_at")?,
        entity_id: row.try_get("entity_id")?,
        metric: row.try_get("metric")?,
        value: row.try_get("value")?,
    })
}

fn llm_drift_metric_from_row(row: &PgRow) -> Result<LLMMetricInternalRecord, SqlError> {
    Ok(LLMMetricInternalRecord {
        record_uid: row.try_get("record_uid")?,
        created_at: row.try_get("created_at")?,
        entity_id: row.try_get("entity_id")?,
        metric: row.try_get("metric")?,
        value: row.try_get("value")?,
    })
}

/// Converts a slice of `PgRow` to a `ServerRecords` based on the provided `RecordType`.
///
/// # Arguments
/// * `rows` - A slice of `PgRow` to be converted.
/// * `record_type` - The type of record to convert to.
///
/// # Returns
/// * `Result<InternalServerRecords, SqlError>` - A result containing the converted `InternalServerRecords` or an error.
///
/// # Errors
/// * Returns an error if the conversion fails or if the record type is not supported.
pub fn pg_rows_to_server_records(
    rows: &[PgRow],
    record_type: &RecordType,
) -> Result<InternalServerRecords, SqlError> {
    // Get correct conversion function base on record type
    // Returns an error if the record type is not supported
    let convert_fn = match record_type {
        RecordType::Spc => |row| Ok(InternalServerRecord::Spc(spc_record_from_row(row)?)),
        RecordType::Psi => |row| Ok(InternalServerRecord::Psi(psi_record_from_row(row)?)),
        RecordType::Custom => |row| Ok(InternalServerRecord::Custom(custom_record_from_row(row)?)),
        RecordType::LLMDrift => |row| {
            Ok(InternalServerRecord::LLMDrift(llm_drift_record_from_row(
                row,
            )?))
        },
        RecordType::LLMMetric => |row| {
            Ok(InternalServerRecord::LLMMetric(llm_drift_metric_from_row(
                row,
            )?))
        },
        _ => return Err(SqlError::InvalidRecordTypeError(record_type.to_string())),
    };

    // Pre-allocate vector with exact capacity needed
    let records: Result<Vec<InternalServerRecord>, SqlError> =
        rows.iter().map(convert_fn).collect();

    // Convert the result into ServerRecords
    records.map(InternalServerRecords::new)
}

#[derive(Debug)]
pub struct QueryTimestamps {
    /// Begin and end datetimes for querying archived data
    pub archived_range: Option<(DateTime<Utc>, DateTime<Utc>)>,

    pub archived_minutes: Option<i32>,

    /// Minutes from retention date to end_datetime for querying current data
    pub current_minutes: Option<i32>,
}

/// Splits a date range into archived and current table queries based on retention period
///
/// # Arguments
/// * `begin_datetime` - Start of the query range
/// * `end_datetime` - End of the query range
/// * `retention_period` - Number of days to keep data in current table
///
/// # Returns
/// * `QueryTimestamps` containing:
///   - archived_range: Some((begin, end)) if query needs archived data
///   - current_minutes: Some(minutes) if query needs current data
///
/// # Examples
/// ```
/// let begin = Utc::now() - Duration::days(60);  // 60 days ago
/// let end = Utc::now() - Duration::days(1);     // yesterday
/// let retention = 30;                           // keep 30 days in current table
///
/// let result = split_custom_interval(begin, end, &retention)?;
/// // Will return:
/// // - archived_range: Some((60 days ago, 30 days ago))
/// // - current_minutes: Some(41760) // minutes for last 29 days
/// ```
pub fn split_custom_interval(
    begin_datetime: DateTime<Utc>,
    end_datetime: DateTime<Utc>,
    retention_period: &i32,
) -> Result<QueryTimestamps, SqlError> {
    if begin_datetime >= end_datetime {
        return Err(SqlError::InvalidDateRangeError);
    }

    let retention_date = Utc::now() - chrono::Duration::days(*retention_period as i64);
    let mut timestamps = QueryTimestamps {
        archived_range: None,
        current_minutes: None,
        archived_minutes: None,
    };

    // Handle data in archived range (before retention date)
    if begin_datetime < retention_date {
        let archive_end = if end_datetime <= retention_date {
            end_datetime
        } else {
            retention_date
        };
        timestamps.archived_range = Some((begin_datetime, archive_end));
    }

    // Handle data in current range (after retention date)
    if end_datetime > retention_date {
        let current_begin = if begin_datetime < retention_date {
            retention_date
        } else {
            begin_datetime
        };
        let minutes = end_datetime
            .signed_duration_since(current_begin)
            .num_minutes() as i32;
        timestamps.current_minutes = Some(minutes);
    }

    // calculate archived minutes
    if let Some((begin, end)) = timestamps.archived_range {
        timestamps.archived_minutes = Some(end.signed_duration_since(begin).num_minutes() as i32);
    }

    Ok(timestamps)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn test_split_custom_interval() {
        let now = Utc::now();
        let retention_period = &30; // 30 days retention

        // Case 1: Query entirely within archived range
        let result = split_custom_interval(
            now - Duration::days(60),
            now - Duration::days(40),
            retention_period,
        )
        .unwrap();
        assert!(result.archived_range.is_some());
        assert!(result.current_minutes.is_none());

        // Case 2: Query entirely within current range
        let result = split_custom_interval(
            now - Duration::days(20),
            now - Duration::days(1),
            retention_period,
        )
        .unwrap();
        assert!(result.archived_range.is_none());
        assert!(result.current_minutes.is_some());

        // Case 3: Query spanning both ranges
        let result = split_custom_interval(
            now - Duration::days(60),
            now - Duration::days(1),
            retention_period,
        )
        .unwrap();
        assert!(result.archived_range.is_some());
        assert!(result.current_minutes.is_some());

        // Case 4: Invalid date range
        let result = split_custom_interval(
            now - Duration::days(1),
            now - Duration::days(2),
            retention_period,
        );
        assert!(result.is_err());
    }
}
