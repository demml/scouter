use crate::sql::error::SqlError;
use crate::sql::schema::genai_event_record_from_row;
use chrono::{DateTime, Utc};
use scouter_types::{
    CustomMetricInternalRecord, GenAIMetricInternalRecord, InternalServerRecord,
    InternalServerRecords, PsiInternalRecord, RecordType, SpcInternalRecord,
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

fn genai_drift_metric_from_row(row: &PgRow) -> Result<GenAIMetricInternalRecord, SqlError> {
    Ok(GenAIMetricInternalRecord {
        uid: row.try_get("uid")?,
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
        RecordType::GenAIEvent => |row| {
            Ok(InternalServerRecord::GenAIDrift(
                genai_event_record_from_row(row)?,
            ))
        },
        RecordType::GenAIMetric => |row| {
            Ok(InternalServerRecord::GenAIMetric(
                genai_drift_metric_from_row(row)?,
            ))
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

    /// Total minutes in the archived range
    pub archived_minutes: Option<i32>,

    /// Begin and end datetimes for querying active/current data
    pub active_range: Option<(DateTime<Utc>, DateTime<Utc>)>,
}

/// Splits a date range into archived and current table queries based on retention period
///
/// # Arguments
/// * `start_datetime` - Start of the query range
/// * `end_datetime` - End of the query range
/// * `retention_period` - Number of days to keep data in current table
///
/// # Returns
/// * `QueryTimestamps` containing:
///   - archived_range: Some((begin, end)) if query needs archived data
///   - archived_minutes: Some(minutes) total minutes in archived range
///   - active_range: Some((begin, end)) if query needs current/active data
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
/// // - archived_minutes: Some(43200) // minutes for 30 days
/// // - active_range: Some((30 days ago, yesterday))
/// ```
pub fn split_custom_interval(
    start_datetime: DateTime<Utc>,
    end_datetime: DateTime<Utc>,
    retention_period: &i32,
) -> Result<QueryTimestamps, SqlError> {
    if start_datetime >= end_datetime {
        return Err(SqlError::InvalidDateRangeError);
    }

    let retention_date = Utc::now() - chrono::Duration::days(*retention_period as i64);
    let mut timestamps = QueryTimestamps {
        archived_range: None,
        archived_minutes: None,
        active_range: None,
    };

    // Handle data in archived range (before retention date)
    if start_datetime < retention_date {
        let archive_end = if end_datetime <= retention_date {
            end_datetime
        } else {
            retention_date
        };
        timestamps.archived_range = Some((start_datetime, archive_end));
        timestamps.archived_minutes = Some(
            archive_end
                .signed_duration_since(start_datetime)
                .num_minutes() as i32,
        );
    }

    // Handle data in active range (after retention date)
    if end_datetime > retention_date {
        let active_begin = if start_datetime < retention_date {
            retention_date
        } else {
            start_datetime
        };
        timestamps.active_range = Some((active_begin, end_datetime));
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
        assert!(result.active_range.is_none());

        // Case 2: Query entirely within current range
        let result = split_custom_interval(
            now - Duration::days(20),
            now - Duration::days(1),
            retention_period,
        )
        .unwrap();
        assert!(result.archived_range.is_none());
        assert!(result.active_range.is_some());

        // Case 3: Query spanning both ranges
        let result = split_custom_interval(
            now - Duration::days(60),
            now - Duration::days(1),
            retention_period,
        )
        .unwrap();
        assert!(result.archived_range.is_some());
        assert!(result.active_range.is_some());

        // Case 4: Invalid date range
        let result = split_custom_interval(
            now - Duration::days(1),
            now - Duration::days(2),
            retention_period,
        );
        assert!(result.is_err());
    }
}
