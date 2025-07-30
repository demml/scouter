use crate::sql::error::SqlError;
use chrono::{DateTime, Utc};
use potato_head::create_uuid7;
use scouter_types::psi::DistributionData;
use scouter_types::BoxedLLMDriftServerRecord;
use scouter_types::LLMDriftServerRecord;
use scouter_types::{
    alert::Alert, get_utc_datetime, psi::FeatureBinProportionResult, BinnedMetric,
    BinnedMetricStats, RecordType,
};
use scouter_types::{EntityType, LLMRecord};
use semver::{BuildMetadata, Prerelease, Version};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{postgres::PgRow, Error, FromRow, Row};
use std::collections::BTreeMap;
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DriftRecord {
    pub created_at: DateTime<Utc>,
    pub name: String,
    pub space: String,
    pub version: String,
    pub feature: String,
    pub value: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpcFeatureResult {
    pub feature: String,
    pub created_at: Vec<DateTime<Utc>>,
    pub values: Vec<f64>,
}

impl<'r> FromRow<'r, PgRow> for SpcFeatureResult {
    fn from_row(row: &'r PgRow) -> Result<Self, Error> {
        Ok(SpcFeatureResult {
            feature: row.try_get("feature")?,
            created_at: row.try_get("created_at")?,
            values: row.try_get("values")?,
        })
    }
}

#[derive(Debug)]
pub struct FeatureDistributionWrapper(pub String, pub DistributionData);

impl<'r> FromRow<'r, PgRow> for FeatureDistributionWrapper {
    fn from_row(row: &'r PgRow) -> Result<Self, Error> {
        let feature: String = row.try_get("feature")?;
        let sample_size: i64 = row.try_get("sample_size")?;
        let bins_json: serde_json::Value = row.try_get("bins")?;
        let bins: BTreeMap<usize, f64> =
            serde_json::from_value(bins_json).map_err(|e| Error::Decode(e.into()))?;

        Ok(FeatureDistributionWrapper(
            feature,
            DistributionData {
                sample_size: sample_size as u64,
                bins,
            },
        ))
    }
}

pub struct BinnedMetricWrapper(pub BinnedMetric);

impl<'r> FromRow<'r, PgRow> for BinnedMetricWrapper {
    fn from_row(row: &'r PgRow) -> Result<Self, Error> {
        let stats_json: Vec<serde_json::Value> = row.try_get("stats")?;

        let stats: Vec<BinnedMetricStats> = stats_json
            .into_iter()
            .map(|value| serde_json::from_value(value).unwrap_or_default())
            .collect();

        Ok(BinnedMetricWrapper(BinnedMetric {
            metric: row.try_get("metric")?,
            created_at: row.try_get("created_at")?,
            stats,
        }))
    }
}

pub struct AlertWrapper(pub Alert);

impl<'r> FromRow<'r, PgRow> for AlertWrapper {
    fn from_row(row: &'r PgRow) -> Result<Self, Error> {
        let alert_value: serde_json::Value = row.try_get("alert")?;
        let alert: BTreeMap<String, String> =
            serde_json::from_value(alert_value).unwrap_or_default();

        Ok(AlertWrapper(Alert {
            created_at: row.try_get("created_at")?,
            name: row.try_get("name")?,
            space: row.try_get("space")?,
            version: row.try_get("version")?,
            alert,
            entity_name: row.try_get("entity_name")?,
            id: row.try_get("id")?,
            drift_type: row.try_get("drift_type")?,
            active: row.try_get("active")?,
        }))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRequest {
    pub name: String,
    pub space: String,
    pub version: String,
    pub profile: String,
    pub drift_type: String,
    pub previous_run: DateTime<Utc>,
    pub schedule: String,
    pub uid: String,
}

impl<'r> FromRow<'r, PgRow> for TaskRequest {
    fn from_row(row: &'r PgRow) -> Result<Self, Error> {
        let profile: serde_json::Value = row.try_get("profile")?;

        Ok(TaskRequest {
            name: row.try_get("name")?,
            space: row.try_get("space")?,
            version: row.try_get("version")?,
            profile: profile.to_string(),
            drift_type: row.try_get("drift_type")?,
            previous_run: row.try_get("previous_run")?,
            schedule: row.try_get("schedule")?,
            uid: row.try_get("uid")?,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservabilityResult {
    pub route_name: String,
    pub created_at: Vec<DateTime<Utc>>,
    pub p5: Vec<f64>,
    pub p25: Vec<f64>,
    pub p50: Vec<f64>,
    pub p95: Vec<f64>,
    pub p99: Vec<f64>,
    pub total_request_count: Vec<i64>,
    pub total_error_count: Vec<i64>,
    pub error_latency: Vec<f64>,
    pub status_counts: Vec<HashMap<String, i64>>,
}

impl<'r> FromRow<'r, PgRow> for ObservabilityResult {
    fn from_row(row: &'r PgRow) -> Result<Self, Error> {
        // decode status counts to vec of jsonb
        let status_counts: Vec<serde_json::Value> = row.try_get("status_counts")?;

        // convert vec of jsonb to vec of hashmaps
        let status_counts: Vec<HashMap<String, i64>> = status_counts
            .into_iter()
            .map(|value| serde_json::from_value(value).unwrap_or_default())
            .collect();

        Ok(ObservabilityResult {
            route_name: row.try_get("route_name")?,
            created_at: row.try_get("created_at")?,
            p5: row.try_get("p5")?,
            p25: row.try_get("p25")?,
            p50: row.try_get("p50")?,
            p95: row.try_get("p95")?,
            p99: row.try_get("p99")?,
            total_request_count: row.try_get("total_request_count")?,
            total_error_count: row.try_get("total_error_count")?,
            error_latency: row.try_get("error_latency")?,
            status_counts,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BinProportion {
    pub bin_id: usize,
    pub proportion: f64,
}

#[derive(Debug)]
pub struct FeatureBinProportionResultWrapper(pub FeatureBinProportionResult);

impl<'r> FromRow<'r, PgRow> for FeatureBinProportionResultWrapper {
    fn from_row(row: &'r PgRow) -> Result<Self, Error> {
        // Extract the bin_proportions as a Vec of tuples
        let bin_proportions_json: Vec<serde_json::Value> = row.try_get("bin_proportions")?;

        // Convert the Vec of tuples into a Vec of BinProportion structs
        let bin_proportions: Vec<BTreeMap<usize, f64>> = bin_proportions_json
            .into_iter()
            .map(|json| serde_json::from_value(json).unwrap_or_default())
            .collect();

        let overall_proportions_json: serde_json::Value = row.try_get("overall_proportions")?;
        let overall_proportions: BTreeMap<usize, f64> =
            serde_json::from_value(overall_proportions_json).unwrap_or_default();

        Ok(FeatureBinProportionResultWrapper(
            FeatureBinProportionResult {
                feature: row.try_get("feature")?,
                created_at: row.try_get("created_at")?,
                bin_proportions,
                overall_proportions,
            },
        ))
    }
}
#[derive(Debug, Clone, FromRow)]
pub struct Entity {
    pub space: String,
    pub name: String,
    pub version: String,
    pub begin_timestamp: DateTime<Utc>,
    pub end_timestamp: DateTime<Utc>,
}

impl Entity {
    pub fn get_write_path(&self, record_type: &RecordType) -> String {
        format!(
            "{}/{}/{}/{}",
            self.space, self.name, self.version, record_type
        )
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct User {
    pub id: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub active: bool,
    pub username: String,
    pub password_hash: String,
    pub hashed_recovery_codes: Vec<String>,
    pub permissions: Vec<String>,
    pub group_permissions: Vec<String>,
    pub role: String,
    pub favorite_spaces: Vec<String>,
    pub refresh_token: Option<String>,
    pub email: String,
    pub updated_at: DateTime<Utc>,
}

impl User {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        username: String,
        password_hash: String,
        email: String,
        hashed_recovery_codes: Vec<String>,
        permissions: Option<Vec<String>>,
        group_permissions: Option<Vec<String>>,
        role: Option<String>,
        favorite_spaces: Option<Vec<String>>,
    ) -> Self {
        let created_at = get_utc_datetime();

        User {
            id: None,
            created_at,
            active: true,
            username,
            password_hash,
            hashed_recovery_codes,
            permissions: permissions.unwrap_or(vec!["read:all".to_string()]),
            group_permissions: group_permissions.unwrap_or(vec!["user".to_string()]),
            favorite_spaces: favorite_spaces.unwrap_or_default(),
            role: role.unwrap_or("user".to_string()),
            refresh_token: None,
            email,
            updated_at: created_at,
        }
    }
}

impl FromRow<'_, PgRow> for User {
    fn from_row(row: &PgRow) -> Result<Self, sqlx::Error> {
        let id = row.try_get("id")?;
        let created_at = row.try_get("created_at")?;
        let updated_at = row.try_get("updated_at")?;
        let active = row.try_get("active")?;
        let username = row.try_get("username")?;
        let password_hash = row.try_get("password_hash")?;
        let email = row.try_get("email")?;
        let role = row.try_get("role")?;
        let refresh_token = row.try_get("refresh_token")?;

        let group_permissions: Vec<String> =
            serde_json::from_value(row.try_get("group_permissions")?).unwrap_or_default();

        let permissions: Vec<String> =
            serde_json::from_value(row.try_get("permissions")?).unwrap_or_default();

        let hashed_recovery_codes: Vec<String> =
            serde_json::from_value(row.try_get("hashed_recovery_codes")?).unwrap_or_default();

        let favorite_spaces: Vec<String> =
            serde_json::from_value(row.try_get("favorite_spaces")?).unwrap_or_default();

        Ok(User {
            id,
            created_at,
            updated_at,
            active,
            username,
            password_hash,
            email,
            role,
            refresh_token,
            hashed_recovery_codes,
            permissions,
            group_permissions,
            favorite_spaces,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct UpdateAlertResult {
    pub id: i32,
    pub active: bool,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct LLMDriftServerSQLRecord {
    pub uid: String,

    pub created_at: chrono::DateTime<Utc>,

    pub space: String,

    pub name: String,

    pub version: String,

    pub prompt: Option<Value>,

    pub context: Value,

    pub score: Value,

    pub status: String,

    pub id: i64,

    pub updated_at: Option<DateTime<Utc>>,

    pub processing_started_at: Option<DateTime<Utc>>,

    pub processing_ended_at: Option<DateTime<Utc>>,

    pub processing_duration: Option<i64>, // Interval in seconds for the drift calculation
}

impl LLMDriftServerSQLRecord {
    /// Method use when server receives a record from the client
    pub fn from_server_record(record: &LLMDriftServerRecord) -> Self {
        LLMDriftServerSQLRecord {
            created_at: record.created_at,
            space: record.space.clone(),
            name: record.name.clone(),
            version: record.version.clone(),
            prompt: record.prompt.clone(),
            context: record.context.clone(),
            score: record.score.clone(),
            status: record.status.to_string(),
            id: 0,               // This is a placeholder, as the ID will be set by the database
            uid: create_uuid7(), // This is also a placeholder, as the UID will be set by the database
            updated_at: None,
            processing_started_at: None,
            processing_ended_at: None,
            processing_duration: None, // This will be set when the record is processed
        }
    }
}

impl From<LLMDriftServerSQLRecord> for LLMDriftServerRecord {
    fn from(sql_record: LLMDriftServerSQLRecord) -> Self {
        Self {
            id: sql_record.id,
            created_at: sql_record.created_at,
            space: sql_record.space,
            name: sql_record.name,
            version: sql_record.version,
            context: sql_record.context,
            score: sql_record.score,
            prompt: sql_record.prompt,
            status: sql_record.status.parse().unwrap_or_default(), // Handle parsing appropriately
            processing_started_at: sql_record.processing_started_at,
            processing_ended_at: sql_record.processing_ended_at,
            processing_duration: sql_record.processing_duration,
            updated_at: sql_record.updated_at,
            uid: sql_record.uid,
        }
    }
}

/// Converts a `PgRow` to a `BoxedLLMDriftServerRecord`
/// Conversion is done by first converting the row to an `LLMDriftServerSQLRecord`
/// and then converting that to an `LLMDriftServerRecord`.
pub fn llm_drift_record_from_row(row: &PgRow) -> Result<BoxedLLMDriftServerRecord, SqlError> {
    let sql_record = LLMDriftServerSQLRecord::from_row(row)?;
    let record = LLMDriftServerRecord::from(sql_record);

    Ok(BoxedLLMDriftServerRecord {
        record: Box::new(record),
    })
}

pub fn llm_drift_metric_from_row(row: &PgRow) -> Result<BoxedLLMDriftServerRecord, SqlError> {
    let sql_record = LLMDriftServerSQLRecord::from_row(row)?;
    let record = LLMDriftServerRecord::from(sql_record);

    Ok(BoxedLLMDriftServerRecord {
        record: Box::new(record),
    })
}

pub struct LLMRecordWrapper(pub LLMRecord);

impl<'r> FromRow<'r, PgRow> for LLMRecordWrapper {
    fn from_row(row: &'r PgRow) -> Result<Self, sqlx::Error> {
        let llm_record = LLMRecord {
            uid: row.try_get("uid")?,
            created_at: row.try_get("created_at")?,
            space: row.try_get("space")?,
            name: row.try_get("name")?,
            version: row.try_get("version")?,
            context: row.try_get("context")?,
            prompt: row.try_get("prompt")?,
            score: row.try_get("score")?,
            entity_type: EntityType::LLM,
        };
        Ok(Self(llm_record))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct VersionResult {
    pub created_at: DateTime<Utc>,
    pub name: String,
    pub space: String,
    pub major: i32,
    pub minor: i32,
    pub patch: i32,
    pub pre_tag: Option<String>,
    pub build_tag: Option<String>,
}

impl VersionResult {
    pub fn to_version(&self) -> Result<Version, SqlError> {
        let mut version = Version::new(self.major as u64, self.minor as u64, self.patch as u64);

        if self.pre_tag.is_some() {
            version.pre = Prerelease::new(self.pre_tag.as_ref().unwrap())?;
        }

        if self.build_tag.is_some() {
            version.build = BuildMetadata::new(self.build_tag.as_ref().unwrap())?;
        }

        Ok(version)
    }
}
