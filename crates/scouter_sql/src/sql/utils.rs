use crate::sql::error::SqlError;
use chrono::{DateTime, Utc};
use sqlx::postgres::PgRow;

use scouter_types::{
    CustomMetricRecord, GenAIEvalRecord, GenAIEvalTaskResultRecord, GenAIEvalWorkflowRecord,
    IntoServerRecord, PsiRecord, RecordType, ServerRecords, SpcRecord,
};
/// Generic function to deserialize PgRows into ServerRecords
pub fn pg_rows_to_server_records<T>(
    rows: &[PgRow],
    _record_type: &RecordType,
) -> Result<ServerRecords, SqlError>
where
    T: for<'r> sqlx::FromRow<'r, PgRow> + IntoServerRecord + Send + Unpin,
{
    let mut records = Vec::with_capacity(rows.len());

    for row in rows {
        let record: T = sqlx::FromRow::from_row(row)?;
        records.push(record.into_server_record());
    }

    Ok(ServerRecords::new(records))
}

/// Parses Postgres rows into ServerRecords based on RecordType
pub fn parse_pg_rows(
    rows: &[sqlx::postgres::PgRow],
    record_type: &RecordType,
) -> Result<ServerRecords, SqlError> {
    match record_type {
        RecordType::Spc => pg_rows_to_server_records::<SpcRecord>(rows, record_type),
        RecordType::Psi => {
            crate::sql::utils::pg_rows_to_server_records::<PsiRecord>(rows, record_type)
        }
        RecordType::Custom => {
            crate::sql::utils::pg_rows_to_server_records::<CustomMetricRecord>(rows, record_type)
        }
        RecordType::GenAIEval => {
            crate::sql::utils::pg_rows_to_server_records::<GenAIEvalRecord>(rows, record_type)
        }
        RecordType::GenAITask => crate::sql::utils::pg_rows_to_server_records::<
            GenAIEvalTaskResultRecord,
        >(rows, record_type),
        RecordType::GenAIWorkflow => crate::sql::utils::pg_rows_to_server_records::<
            GenAIEvalWorkflowRecord,
        >(rows, record_type),
        _ => Err(SqlError::InvalidRecordTypeError(record_type.to_string())),
    }
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
