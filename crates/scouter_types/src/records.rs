use crate::error::RecordError;
use crate::genai::{ComparisonOperator, EvaluationTaskType};
use crate::trace::TraceServerRecord;
use crate::DriftType;

use crate::Status;
use crate::TagRecord;
use chrono::DateTime;
use chrono::Utc;
use potato_head::PyHelperFuncs;
use pyo3::prelude::*;
use pythonize::pythonize;
use scouter_macro::impl_mask_entity_id;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt::Display;
use std::str::FromStr;

#[cfg(feature = "server")]
use sqlx::{postgres::PgRow, FromRow, Row};

#[pyclass(eq)]
#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
pub enum RecordType {
    #[default]
    Spc,
    Psi,
    Observability,
    Custom,
    GenAIEvent,
    GenAITask,
    GenAIWorkflow,
    Trace,
}

impl RecordType {
    pub fn to_drift_type(&self) -> &str {
        match self {
            RecordType::Spc => DriftType::Spc.to_string(),
            RecordType::Psi => DriftType::Psi.to_string(),
            RecordType::Custom => DriftType::Custom.to_string(),
            RecordType::GenAIEvent => DriftType::GenAI.to_string(),
            RecordType::GenAITask => DriftType::GenAI.to_string(),
            RecordType::GenAIWorkflow => DriftType::GenAI.to_string(),
            _ => "unknown",
        }
    }
}

impl Display for RecordType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RecordType::Spc => write!(f, "spc"),
            RecordType::Psi => write!(f, "psi"),
            RecordType::Observability => write!(f, "observability"),
            RecordType::Custom => write!(f, "custom"),
            RecordType::GenAIEvent => write!(f, "genai_event"),
            RecordType::GenAITask => write!(f, "genai_task"),
            RecordType::GenAIWorkflow => write!(f, "genai_workflow"),
            RecordType::Trace => write!(f, "trace"),
        }
    }
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct SpcRecord {
    #[pyo3(get)]
    pub created_at: chrono::DateTime<Utc>,

    #[pyo3(get)]
    #[cfg_attr(feature = "server", sqlx(skip))]
    pub uid: String,

    #[pyo3(get)]
    pub feature: String,

    #[pyo3(get)]
    pub value: f64,

    pub entity_id: Option<i32>,
}

#[pymethods]
impl SpcRecord {
    #[new]
    pub fn new(uid: String, feature: String, value: f64) -> Self {
        Self {
            created_at: Utc::now(),
            uid,
            feature,
            value,
            entity_id: None,
        }
    }

    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        PyHelperFuncs::__str__(self)
    }

    pub fn get_record_type(&self) -> RecordType {
        RecordType::Spc
    }

    pub fn model_dump_json(&self) -> String {
        // serialize the struct to a string
        PyHelperFuncs::__json__(self)
    }

    pub fn to_dict(&self) -> HashMap<String, String> {
        let mut record = HashMap::new();
        record.insert("created_at".to_string(), self.created_at.to_string());
        record.insert("uid".to_string(), self.uid.clone());
        record.insert("feature".to_string(), self.feature.clone());
        record.insert("value".to_string(), self.value.to_string());
        record
    }
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct PsiRecord {
    #[pyo3(get)]
    pub created_at: chrono::DateTime<Utc>,
    #[pyo3(get)]
    #[cfg_attr(feature = "server", sqlx(skip))]
    pub uid: String,
    #[pyo3(get)]
    pub feature: String,
    #[pyo3(get)]
    pub bin_id: i32,
    #[pyo3(get)]
    pub bin_count: i32,
    pub entity_id: Option<i32>,
}

#[pymethods]
impl PsiRecord {
    #[new]
    pub fn new(uid: String, feature: String, bin_id: i32, bin_count: i32) -> Self {
        Self {
            created_at: Utc::now(),
            uid,
            feature,
            bin_id,
            bin_count,
            entity_id: None,
        }
    }

    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        PyHelperFuncs::__str__(self)
    }

    pub fn get_record_type(&self) -> RecordType {
        RecordType::Psi
    }

    pub fn model_dump_json(&self) -> String {
        // serialize the struct to a string
        PyHelperFuncs::__json__(self)
    }
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct GenAIEventRecord {
    #[pyo3(get)]
    pub created_at: chrono::DateTime<Utc>,

    #[pyo3(get)]
    pub uid: String,

    pub context: Value,

    #[cfg_attr(feature = "server", sqlx(try_from = "String"))]
    pub status: Status,

    pub id: i64,

    pub updated_at: Option<DateTime<Utc>>,

    pub processing_started_at: Option<DateTime<Utc>>,

    pub processing_ended_at: Option<DateTime<Utc>>,

    pub processing_duration: Option<i32>,

    #[cfg_attr(feature = "server", sqlx(skip))]
    pub entity_uid: String,

    pub entity_id: Option<i32>,
}

#[pymethods]
impl GenAIEventRecord {
    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        PyHelperFuncs::__str__(self)
    }

    pub fn get_record_type(&self) -> RecordType {
        RecordType::GenAIEvent
    }

    pub fn model_dump_json(&self) -> String {
        // serialize the struct to a string
        PyHelperFuncs::__json__(self)
    }
}

impl GenAIEventRecord {
    #[allow(clippy::too_many_arguments)]
    pub fn new_rs(
        context: Value,
        created_at: DateTime<Utc>,
        uid: String,
        entity_uid: String,
    ) -> Self {
        Self {
            created_at,
            context,
            status: Status::Pending,
            id: 0, // This is a placeholder, as the ID will be set by the database
            uid,
            updated_at: None,
            processing_started_at: None,
            processing_ended_at: None,
            processing_duration: None,
            entity_uid,
            entity_id: None,
        }
    }

    // helper for masking sensitive data from the record when
    // return to the user. Currently, only removes entity_id
    pub fn mask_sensitive_data(&mut self) {
        self.entity_id = None;
    }
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BoxedGenAIEventRecord {
    pub record: Box<GenAIEventRecord>,
}

impl BoxedGenAIEventRecord {
    pub fn new(record: GenAIEventRecord) -> Self {
        Self {
            record: Box::new(record),
        }
    }
}

#[pyclass]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct GenAIEvalWorkflowRecord {
    #[pyo3(get)]
    pub created_at: DateTime<Utc>,

    #[pyo3(get)]
    pub record_uid: String,

    pub entity_id: i32,

    #[pyo3(get)]
    pub total_tasks: i32,

    #[pyo3(get)]
    pub passed_tasks: i32,

    #[pyo3(get)]
    pub failed_tasks: i32,

    #[pyo3(get)]
    pub pass_rate: f64,

    #[pyo3(get)]
    pub duration_ms: i32,
}

#[pymethods]
impl GenAIEvalWorkflowRecord {
    pub fn __str__(&self) -> String {
        PyHelperFuncs::__str__(self)
    }

    pub fn model_dump_json(&self) -> String {
        PyHelperFuncs::__json__(self)
    }
}

impl GenAIEvalWorkflowRecord {
    pub fn new(
        record_uid: String,
        total_tasks: i32,
        passed_tasks: i32,
        failed_tasks: i32,
        duration_ms: i32,
        entity_id: i32,
    ) -> Self {
        let pass_rate = if total_tasks > 0 {
            passed_tasks as f64 / total_tasks as f64
        } else {
            0.0
        };

        Self {
            record_uid,
            created_at: Utc::now(),
            total_tasks,
            passed_tasks,
            failed_tasks,
            pass_rate,
            duration_ms,
            entity_id,
        }
    }
}

// Detailed result for an individual evaluation task within a workflow
#[pyclass]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenAIEvalTaskResultRecord {
    #[pyo3(get)]
    pub created_at: DateTime<Utc>,

    #[pyo3(get)]
    pub record_uid: String,

    // this is not exposed to python
    pub entity_id: i32,

    #[pyo3(get)]
    pub task_id: String,

    #[pyo3(get)]
    pub task_type: EvaluationTaskType,

    #[pyo3(get)]
    pub passed: bool,

    #[pyo3(get)]
    pub value: f64,

    #[pyo3(get)]
    pub field_path: Option<String>,

    #[pyo3(get)]
    pub operator: ComparisonOperator,

    pub expected: Value,

    pub actual: Value,

    #[pyo3(get)]
    pub message: String,
}

#[pymethods]
impl GenAIEvalTaskResultRecord {
    #[getter]
    pub fn get_expected<'py>(&self, py: Python<'py>) -> Result<Bound<'py, PyAny>, RecordError> {
        let py_value = pythonize(py, &self.expected)?;
        Ok(py_value)
    }

    #[getter]
    pub fn get_actual<'py>(&self, py: Python<'py>) -> Result<Bound<'py, PyAny>, RecordError> {
        let py_value = pythonize(py, &self.actual)?;
        Ok(py_value)
    }

    pub fn __str__(&self) -> String {
        PyHelperFuncs::__str__(self)
    }

    pub fn model_dump_json(&self) -> String {
        PyHelperFuncs::__json__(self)
    }
}

impl GenAIEvalTaskResultRecord {
    pub fn new(
        record_uid: String,
        task_id: String,
        task_type: EvaluationTaskType,
        passed: bool,
        value: f64,
        field_path: Option<String>,
        operator: ComparisonOperator,
        expected: Value,
        actual: Value,
        message: String,
        entity_id: i32,
    ) -> Self {
        Self {
            record_uid,
            created_at: Utc::now(),
            task_id,
            task_type,
            passed,
            value,
            field_path,
            operator,
            expected,
            actual,
            message,
            entity_id,
        }
    }
}

#[cfg(feature = "server")]
impl FromRow<'_, PgRow> for GenAIEvalTaskResultRecord {
    fn from_row(row: &PgRow) -> Result<Self, sqlx::Error> {
        let expected: Value =
            serde_json::from_value(row.try_get("expected")?).unwrap_or(Value::Null);
        let actual: Value = serde_json::from_value(row.try_get("actual")?).unwrap_or(Value::Null);
        let task_type: EvaluationTaskType =
            EvaluationTaskType::from_str(&row.try_get::<String, &str>("task_type")?)
                .unwrap_or(EvaluationTaskType::Assertion);
        let comparison_operator: ComparisonOperator =
            ComparisonOperator::from_str(&row.try_get::<String, &str>("operator")?)
                .unwrap_or(ComparisonOperator::Equal);

        Ok(GenAIEvalTaskResultRecord {
            record_uid: row.try_get("record_uid")?,
            created_at: row.try_get("created_at")?,
            task_id: row.try_get("task_id")?,
            task_type: task_type,
            passed: row.try_get("passed")?,
            value: row.try_get("value")?,
            field_path: row.try_get("field_path")?,
            operator: comparison_operator,
            expected,
            actual,
            message: row.try_get("message")?,
            entity_id: row.try_get("entity_id")?,
        })
    }
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct CustomMetricRecord {
    #[pyo3(get)]
    pub created_at: chrono::DateTime<Utc>,
    #[pyo3(get)]
    // skip this row when decoding pgrow
    #[cfg_attr(feature = "server", sqlx(skip))]
    pub uid: String,
    #[pyo3(get)]
    pub metric: String,
    #[pyo3(get)]
    pub value: f64,

    pub entity_id: Option<i32>,
}

#[pymethods]
impl CustomMetricRecord {
    #[new]
    pub fn new(uid: String, metric: String, value: f64) -> Self {
        Self {
            created_at: chrono::Utc::now(),
            uid,
            metric: metric.to_lowercase(),
            value,
            entity_id: None,
        }
    }

    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        PyHelperFuncs::__str__(self)
    }

    pub fn model_dump_json(&self) -> String {
        // serialize the struct to a string
        PyHelperFuncs::__json__(self)
    }

    pub fn get_record_type(&self) -> RecordType {
        RecordType::Custom
    }

    pub fn to_dict(&self) -> HashMap<String, String> {
        let mut record = HashMap::new();
        record.insert("created_at".to_string(), self.created_at.to_string());
        record.insert("uid".to_string(), self.uid.clone());
        record.insert("metric".to_string(), self.metric.clone());
        record.insert("value".to_string(), self.value.to_string());
        record
    }
}

#[pyclass]
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct LatencyMetrics {
    #[pyo3(get)]
    pub p5: f64,

    #[pyo3(get)]
    pub p25: f64,

    #[pyo3(get)]
    pub p50: f64,

    #[pyo3(get)]
    pub p95: f64,

    #[pyo3(get)]
    pub p99: f64,
}

#[pyclass]
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct RouteMetrics {
    #[pyo3(get)]
    pub route_name: String,

    #[pyo3(get)]
    pub metrics: LatencyMetrics,

    #[pyo3(get)]
    pub request_count: i64,

    #[pyo3(get)]
    pub error_count: i64,

    #[pyo3(get)]
    pub error_latency: f64,

    #[pyo3(get)]
    pub status_codes: HashMap<usize, i64>,
}

#[pyclass]
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct ObservabilityMetrics {
    #[pyo3(get)]
    pub uid: String,

    #[pyo3(get)]
    pub request_count: i64,

    #[pyo3(get)]
    pub error_count: i64,

    #[pyo3(get)]
    pub route_metrics: Vec<RouteMetrics>,

    pub entity_id: Option<i32>,
}

#[pymethods]
impl ObservabilityMetrics {
    pub fn model_dump_json(&self) -> String {
        // serialize records to a string
        PyHelperFuncs::__json__(self)
    }

    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        PyHelperFuncs::__str__(self)
    }

    pub fn get_record_type(&self) -> RecordType {
        RecordType::Observability
    }
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum ServerRecord {
    Spc(SpcRecord),
    Psi(PsiRecord),
    Custom(CustomMetricRecord),
    Observability(ObservabilityMetrics),
    GenAIEvent(BoxedGenAIEventRecord),
    GenAITaskRecord(GenAIEvalTaskResultRecord),
    GenAIWorkflowRecord(GenAIEvalWorkflowRecord),
}

#[pymethods]
impl ServerRecord {
    #[new]
    pub fn new(record: &Bound<'_, PyAny>) -> Result<Self, RecordError> {
        let record_type = record
            .call_method0("get_record_type")?
            .extract::<RecordType>()?;

        match record_type {
            RecordType::Spc => {
                let spc_record = record.extract::<SpcRecord>()?;
                Ok(ServerRecord::Spc(spc_record))
            }
            RecordType::Psi => {
                let psi_record = record.extract::<PsiRecord>()?;
                Ok(ServerRecord::Psi(psi_record))
            }
            RecordType::Custom => {
                let custom_record = record.extract::<CustomMetricRecord>()?;
                Ok(ServerRecord::Custom(custom_record))
            }
            RecordType::Observability => {
                let observability_record = record.extract::<ObservabilityMetrics>()?;
                Ok(ServerRecord::Observability(observability_record))
            }
            RecordType::GenAIEvent => {
                let genai_event_record = record.extract::<GenAIEventRecord>()?;
                Ok(ServerRecord::GenAIEvent(BoxedGenAIEventRecord::new(
                    genai_event_record,
                )))
            }

            _ => Err(RecordError::InvalidDriftTypeError),
        }
    }

    #[getter]
    pub fn record<'py>(&self, py: Python<'py>) -> Result<Bound<'py, PyAny>, RecordError> {
        match self {
            ServerRecord::Spc(record) => Ok(PyHelperFuncs::to_bound_py_object(py, record)?),
            ServerRecord::Psi(record) => Ok(PyHelperFuncs::to_bound_py_object(py, record)?),
            ServerRecord::Custom(record) => Ok(PyHelperFuncs::to_bound_py_object(py, record)?),
            ServerRecord::Observability(record) => {
                Ok(PyHelperFuncs::to_bound_py_object(py, record)?)
            }
            ServerRecord::GenAIEvent(record) => Ok(PyHelperFuncs::to_bound_py_object(py, record)?),
            ServerRecord::GenAITaskRecord(record) => {
                Ok(PyHelperFuncs::to_bound_py_object(py, record)?)
            }
            ServerRecord::GenAIWorkflowRecord(record) => {
                Ok(PyHelperFuncs::to_bound_py_object(py, record)?)
            }
        }
    }

    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        match self {
            ServerRecord::Spc(record) => record.__str__(),
            ServerRecord::Psi(record) => record.__str__(),
            ServerRecord::Custom(record) => record.__str__(),
            ServerRecord::Observability(record) => record.__str__(),
            ServerRecord::GenAIEvent(record) => record.record.__str__(),
            ServerRecord::GenAITaskRecord(record) => record.__str__(),
            ServerRecord::GenAIWorkflowRecord(record) => record.__str__(),
        }
    }

    pub fn get_record_type(&self) -> RecordType {
        match self {
            ServerRecord::Spc(_) => RecordType::Spc,
            ServerRecord::Psi(_) => RecordType::Psi,
            ServerRecord::Custom(_) => RecordType::Custom,
            ServerRecord::Observability(_) => RecordType::Observability,
            ServerRecord::GenAIEvent(_) => RecordType::GenAIEvent,
            ServerRecord::GenAITaskRecord(_) => RecordType::GenAITask,
            ServerRecord::GenAIWorkflowRecord(_) => RecordType::GenAIWorkflow,
        }
    }
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ServerRecords {
    #[pyo3(get)]
    pub records: Vec<ServerRecord>,
}

#[pymethods]
impl ServerRecords {
    #[new]
    pub fn new(records: Vec<ServerRecord>) -> Self {
        Self { records }
    }
    pub fn model_dump_json(&self) -> String {
        // serialize records to a string
        PyHelperFuncs::__json__(self)
    }

    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        PyHelperFuncs::__str__(self)
    }
}

impl ServerRecords {
    pub fn record_type(&self) -> Result<RecordType, RecordError> {
        if let Some(first) = self.records.first() {
            Ok(first.get_record_type())
        } else {
            Err(RecordError::EmptyServerRecordsError)
        }
    }
    // Helper function to load records from bytes. Used by scouter-server consumers
    //
    // # Arguments
    //
    // * `bytes` - A slice of bytes
    pub fn load_from_bytes(bytes: &[u8]) -> Result<Self, RecordError> {
        let records: ServerRecords = serde_json::from_slice(bytes)?;
        Ok(records)
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn unique_dates(&self) -> Result<Vec<DateTime<Utc>>, RecordError> {
        let mut dates = HashSet::new();
        let record_type = self.record_type().unwrap_or(RecordType::Spc);
        for record in &self.records {
            match record {
                ServerRecord::Spc(inner) => {
                    if record_type == RecordType::Spc {
                        dates.insert(inner.created_at);
                    }
                }
                ServerRecord::Psi(inner) => {
                    if record_type == RecordType::Psi {
                        dates.insert(inner.created_at);
                    }
                }
                ServerRecord::Custom(inner) => {
                    if record_type == RecordType::Custom {
                        dates.insert(inner.created_at);
                    }
                }
                _ => {
                    return Err(RecordError::InvalidDriftTypeError);
                }
            }
        }
        let dates: Vec<DateTime<Utc>> = dates.into_iter().collect();

        Ok(dates)
    }

    /// gets the uid from the first record type found in the records
    /// This is a helper for consumers that need to get an entity_id associated with the given uid
    pub fn uid(&self) -> Result<&String, RecordError> {
        if let Some(first) = self.records.first() {
            match first {
                ServerRecord::Spc(inner) => Ok(&inner.uid),
                ServerRecord::Psi(inner) => Ok(&inner.uid),
                ServerRecord::Custom(inner) => Ok(&inner.uid),
                ServerRecord::Observability(inner) => Ok(&inner.uid),
                ServerRecord::GenAIEvent(inner) => Ok(&inner.record.entity_uid),
                _ => Err(RecordError::InvalidDriftTypeError),
            }
        } else {
            Err(RecordError::EmptyServerRecordsError)
        }
    }
}

/// Trait to convert a deserialized record into a ServerRecord variant
pub trait IntoServerRecord {
    fn into_server_record(self) -> ServerRecord;
}

impl IntoServerRecord for SpcRecord {
    fn into_server_record(self) -> ServerRecord {
        ServerRecord::Spc(self)
    }
}

impl IntoServerRecord for PsiRecord {
    fn into_server_record(self) -> ServerRecord {
        ServerRecord::Psi(self)
    }
}

impl IntoServerRecord for CustomMetricRecord {
    fn into_server_record(self) -> ServerRecord {
        ServerRecord::Custom(self)
    }
}

impl IntoServerRecord for GenAIEventRecord {
    fn into_server_record(self) -> ServerRecord {
        ServerRecord::GenAIEvent(BoxedGenAIEventRecord::new(self))
    }
}

impl IntoServerRecord for GenAIEvalWorkflowRecord {
    fn into_server_record(self) -> ServerRecord {
        ServerRecord::GenAIWorkflowRecord(self)
    }
}
impl IntoServerRecord for GenAIEvalTaskResultRecord {
    fn into_server_record(self) -> ServerRecord {
        ServerRecord::GenAITaskRecord(self)
    }
}

/// Helper trait to convert ServerRecord to their respective internal record types
pub trait ToDriftRecords {
    fn to_spc_drift_records(&self) -> Result<Vec<&SpcRecord>, RecordError>;
    fn to_observability_drift_records(&self) -> Result<Vec<&ObservabilityMetrics>, RecordError>;
    fn to_psi_drift_records(&self) -> Result<Vec<&PsiRecord>, RecordError>;
    fn to_custom_metric_drift_records(&self) -> Result<Vec<&CustomMetricRecord>, RecordError>;
    fn to_genai_event_records(&self) -> Result<Vec<&BoxedGenAIEventRecord>, RecordError>;
    fn to_genai_workflow_records(&self) -> Result<Vec<&GenAIEvalWorkflowRecord>, RecordError>;
    fn to_genai_task_records(&self) -> Result<Vec<&GenAIEvalTaskResultRecord>, RecordError>;
}

impl ToDriftRecords for ServerRecords {
    fn to_spc_drift_records(&self) -> Result<Vec<&SpcRecord>, RecordError> {
        extract_records(self, |record| match record {
            ServerRecord::Spc(inner) => Some(inner),
            _ => None,
        })
    }

    fn to_observability_drift_records(&self) -> Result<Vec<&ObservabilityMetrics>, RecordError> {
        extract_records(self, |record| match record {
            ServerRecord::Observability(inner) => Some(inner),
            _ => None,
        })
    }

    fn to_psi_drift_records(&self) -> Result<Vec<&PsiRecord>, RecordError> {
        extract_records(self, |record| match record {
            ServerRecord::Psi(inner) => Some(inner),
            _ => None,
        })
    }

    fn to_custom_metric_drift_records(&self) -> Result<Vec<&CustomMetricRecord>, RecordError> {
        extract_records(self, |record| match record {
            ServerRecord::Custom(inner) => Some(inner),
            _ => None,
        })
    }

    fn to_genai_event_records(&self) -> Result<Vec<&BoxedGenAIEventRecord>, RecordError> {
        extract_records(self, |record| match record {
            ServerRecord::GenAIEvent(inner) => Some(inner),
            _ => None,
        })
    }

    fn to_genai_workflow_records(&self) -> Result<Vec<&GenAIEvalWorkflowRecord>, RecordError> {
        extract_records(self, |record| match record {
            ServerRecord::GenAIWorkflowRecord(inner) => Some(inner),
            _ => None,
        })
    }

    fn to_genai_task_records(&self) -> Result<Vec<&GenAIEvalTaskResultRecord>, RecordError> {
        extract_records(self, |record| match record {
            ServerRecord::GenAITaskRecord(inner) => Some(inner),
            _ => None,
        })
    }
}

fn extract_records<T>(
    server_records: &ServerRecords,
    extractor: impl Fn(&ServerRecord) -> Option<&T>,
) -> Result<Vec<&T>, RecordError> {
    let mut records = Vec::new();

    for record in &server_records.records {
        if let Some(extracted) = extractor(record) {
            records.push(extracted);
        } else {
            return Err(RecordError::InvalidDriftTypeError);
        }
    }

    Ok(records)
}

pub enum MessageType {
    Server,
    Trace,
    Tag,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessageRecord {
    ServerRecords(ServerRecords),
    TraceServerRecord(TraceServerRecord),
    TagServerRecord(TagRecord),
}

impl MessageRecord {
    pub fn record_type(&self) -> MessageType {
        match self {
            MessageRecord::ServerRecords(_) => MessageType::Server,
            MessageRecord::TraceServerRecord(_) => MessageType::Trace,
            MessageRecord::TagServerRecord(_) => MessageType::Tag,
        }
    }

    pub fn len(&self) -> usize {
        match self {
            MessageRecord::ServerRecords(records) => records.len(),
            MessageRecord::TraceServerRecord(_) => 1,
            _ => 1,
        }
    }

    pub fn is_empty(&self) -> bool {
        match self {
            MessageRecord::ServerRecords(records) => records.is_empty(),
            _ => false,
        }
    }

    pub fn model_dump_json(&self) -> String {
        // serialize records to a string
        match self {
            MessageRecord::ServerRecords(records) => records.model_dump_json(),
            MessageRecord::TraceServerRecord(record) => PyHelperFuncs::__json__(record),
            MessageRecord::TagServerRecord(record) => PyHelperFuncs::__json__(record),
        }
    }
}

/// implement iterator for MEssageRecord to iterate over ServerRecords.records
impl MessageRecord {
    pub fn iter_server_records(&self) -> Option<impl Iterator<Item = &ServerRecord>> {
        match self {
            MessageRecord::ServerRecords(records) => Some(records.records.iter()),
            _ => None,
        }
    }
}

impl_mask_entity_id!(
    crate::ScouterRecordExt =>
    GenAIEventRecord,
    SpcRecord,
    PsiRecord,
    CustomMetricRecord,
);
