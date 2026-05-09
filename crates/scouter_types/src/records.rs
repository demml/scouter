use crate::agent::{
    AgentAssertion, ComparisonOperator, EvaluationTaskType, ExecutionPlan, TraceAssertion,
};
use crate::agent::{DocumentMedia, EvalMedia, ImageMedia};
use crate::error::RecordError;
use crate::trace::TraceServerRecord;
use crate::{DriftType, SpanId, Status, TraceId, depythonize_object_to_value};
use crate::{EntityType, TagRecord};
use chrono::DateTime;
use chrono::Utc;
use owo_colors::OwoColorize;
use potato_head::PyHelperFuncs;
use potato_head::create_uuid7;
use pyo3::exceptions::PyTypeError;
use pyo3::prelude::*;
use pythonize::pythonize;
use scouter_macro::impl_mask_entity_id;
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[cfg(feature = "server")]
use sqlx::{FromRow, Row, postgres::PgRow, types::Json};
use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt::Display;
use std::str::FromStr;
use tabled::Tabled;
use tabled::{
    Table,
    settings::{Alignment, Color, Format, Style, object::Rows},
};

#[pyclass(from_py_object, eq)]
#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
pub enum RecordType {
    #[default]
    Spc,
    Psi,
    Observability,
    Custom,
    AgentEval,
    AgentTask,
    AgentWorkflow,
    Trace,
}

impl RecordType {
    pub fn to_drift_type(&self) -> &str {
        match self {
            RecordType::Spc => DriftType::Spc.to_string(),
            RecordType::Psi => DriftType::Psi.to_string(),
            RecordType::Custom => DriftType::Custom.to_string(),
            RecordType::AgentEval => DriftType::Agent.to_string(),
            RecordType::AgentTask => DriftType::Agent.to_string(),
            RecordType::AgentWorkflow => DriftType::Agent.to_string(),
            _ => "unknown",
        }
    }
}

impl Display for RecordType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for RecordType {
    type Err = RecordError;

    fn from_str(record_type: &str) -> Result<Self, Self::Err> {
        match record_type.to_lowercase().as_str() {
            "spc" => Ok(RecordType::Spc),
            "psi" => Ok(RecordType::Psi),
            "observability" => Ok(RecordType::Observability),
            "custom" => Ok(RecordType::Custom),
            "agent_event" => Ok(RecordType::AgentEval),
            "agent_task" => Ok(RecordType::AgentTask),
            "agent_workflow" => Ok(RecordType::AgentWorkflow),
            "trace" => Ok(RecordType::Trace),
            _ => Err(RecordError::InvalidDriftTypeError),
        }
    }
}

impl RecordType {
    pub fn as_str(&self) -> &str {
        match self {
            RecordType::Spc => "spc",
            RecordType::Psi => "psi",
            RecordType::Observability => "observability",
            RecordType::Custom => "custom",
            RecordType::AgentEval => "agent_event",
            RecordType::AgentTask => "agent_task",
            RecordType::AgentWorkflow => "agent_workflow",
            RecordType::Trace => "trace",
        }
    }
}

#[pyclass(from_py_object)]
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

    pub entity_id: i32,
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
            entity_id: 0,
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

#[pyclass(from_py_object)]
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
    pub entity_id: i32,
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
            entity_id: 0,
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

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub enum EvalRecordSource {
    #[default]
    User,
    Queue,
    TraceDispatch,
}

impl EvalRecordSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            EvalRecordSource::User => "user",
            EvalRecordSource::Queue => "queue",
            EvalRecordSource::TraceDispatch => "trace_dispatch",
        }
    }
}

impl FromStr for EvalRecordSource {
    type Err = RecordError;

    fn from_str(source: &str) -> Result<Self, Self::Err> {
        match source.to_lowercase().as_str() {
            "user" => Ok(EvalRecordSource::User),
            "queue" => Ok(EvalRecordSource::Queue),
            "trace_dispatch" => Ok(EvalRecordSource::TraceDispatch),
            _ => Err(RecordError::InvalidDriftTypeError),
        }
    }
}

#[pyclass(from_py_object)]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EvalRecord {
    #[pyo3(get, set)]
    pub record_id: String,
    #[pyo3(get, set)]
    pub session_id: String,
    #[pyo3(get)]
    pub created_at: chrono::DateTime<Utc>,
    #[pyo3(get)]
    pub uid: String,
    pub context: Value,
    pub id: i64,
    pub updated_at: Option<DateTime<Utc>>,
    pub processing_started_at: Option<DateTime<Utc>>,
    pub processing_ended_at: Option<DateTime<Utc>>,
    pub processing_duration: Option<i32>,
    pub entity_id: i32,
    pub entity_uid: String,
    pub status: Status,
    #[pyo3(get)]
    pub entity_type: EntityType,
    pub retry_count: i32,
    pub trace_id: Option<TraceId>,
    pub span_id: Option<SpanId>,
    #[serde(default)]
    pub record_source: EvalRecordSource,
    #[pyo3(get)]
    #[serde(default)]
    pub tags: Vec<String>,
    #[pyo3(get)]
    #[serde(default)]
    pub media: Vec<EvalMedia>,
}

#[pymethods]
impl EvalRecord {
    #[new]
    #[pyo3(signature = (
        context = None,
        id = None,
        session_id = None,
        trace_id = None,
        media = None,
        profile_uid = None,
        trace_context = None,
    ))]

    /// Creates a new EvalRecord instance.
    /// The context is either a python dictionary or a pydantic basemodel.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        py: Python<'_>,
        context: Option<Bound<'_, PyAny>>,
        id: Option<String>,
        session_id: Option<String>,
        trace_id: Option<String>,
        media: Option<Vec<Bound<'_, PyAny>>>,
        profile_uid: Option<String>,
        trace_context: Option<Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        // check if context is a PyDict or PyObject(Pydantic model)
        let context_val = match context {
            Some(ctx) => depythonize_object_to_value(py, &ctx)?,
            None => Value::Object(serde_json::Map::new()),
        };

        let media_vec = extract_media_vec(media)?;
        validate_media_record_cap(&media_vec)?;

        let (trace_id_resolved, span_id_resolved) = match trace_context {
            Some(ctx) => extract_span_context(py, &ctx)?,
            None => (
                trace_id
                    .as_deref()
                    .and_then(|tid| TraceId::from_hex(tid).ok()),
                None,
            ),
        };

        Ok(EvalRecord {
            uid: create_uuid7(),
            created_at: Utc::now(),
            context: context_val,
            record_id: id.unwrap_or_default(),
            session_id: session_id.unwrap_or_else(create_uuid7),
            trace_id: trace_id_resolved,
            span_id: span_id_resolved,
            entity_uid: profile_uid.unwrap_or_default(),
            media: media_vec,
            ..Default::default()
        })
    }
    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        PyHelperFuncs::__str__(self)
    }

    pub fn get_record_type(&self) -> RecordType {
        RecordType::AgentEval
    }

    pub fn model_dump_json(&self) -> String {
        // serialize the struct to a string
        PyHelperFuncs::__json__(self)
    }

    #[getter]
    pub fn get_trace_id(&self) -> Option<String> {
        self.trace_id.as_ref().map(|tid| tid.to_hex())
    }

    #[getter]
    pub fn get_span_id(&self) -> Option<String> {
        self.span_id.as_ref().map(|sid| sid.to_hex())
    }

    #[getter]
    pub fn context<'py>(&self, py: Python<'py>) -> Result<Bound<'py, PyAny>, RecordError> {
        Ok(pythonize(py, &self.context)?)
    }
    #[pyo3(signature = (key, value))]
    pub fn update_context_field(
        &mut self,
        py: Python<'_>,
        key: String,
        value: &Bound<'_, PyAny>,
    ) -> Result<(), RecordError> {
        let py_value = depythonize_object_to_value(py, value)?;

        match &mut self.context {
            Value::Object(map) => {
                map.insert(key, py_value);
            }
            _ => {
                let mut new_map = serde_json::Map::new();
                new_map.insert(key, py_value);
                self.context = Value::Object(new_map);
            }
        }

        Ok(())
    }

    /// Add a tag in "key=value" format.
    #[pyo3(signature = (key, value))]
    pub fn add_tag(&mut self, key: String, value: String) {
        self.tags.push(format!("{}={}", key, value));
    }
}

impl EvalRecord {
    /// will update the entire context of the record
    /// If new context is a map, update context fields individually
    /// if not, replace entire context
    /// # Arguments
    /// * `new_context` - New context to set
    pub fn update_context(
        &mut self,
        py: Python<'_>,
        new_context: &Bound<'_, PyAny>,
    ) -> Result<(), RecordError> {
        let py_value = depythonize_object_to_value(py, new_context)?;

        match &mut self.context {
            Value::Object(map) => {
                if let Value::Object(new_map) = py_value {
                    for (key, value) in new_map {
                        map.insert(key, value);
                    }
                } else {
                    self.context = py_value;
                }
            }
            _ => {
                self.context = py_value;
            }
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new_rs(
        context: Value,
        created_at: DateTime<Utc>,
        uid: String,
        entity_uid: String,
        record_id: Option<String>,
        session_id: Option<String>,
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
            entity_id: 0, // This is a placeholder, to be set when inserting into DB
            record_id: record_id.unwrap_or_default(),
            entity_type: EntityType::Agent,
            session_id: session_id.unwrap_or_else(create_uuid7),
            retry_count: 0,
            trace_id: None,
            span_id: None,
            record_source: EvalRecordSource::User,
            tags: Vec::new(),
            media: Vec::new(),
        }
    }

    // helper for masking sensitive data from the record when
    // return to the user. Currently, only removes entity_id
    pub fn mask_sensitive_data(&mut self) {
        self.entity_id = -1;
    }

    pub fn set_failed_with_error(&mut self, error_kind: &str) {
        self.status = Status::Failed;
        if let Value::Object(ref mut map) = self.context {
            map.insert("error".to_string(), Value::String(error_kind.to_string()));
        } else {
            let mut map = serde_json::Map::new();
            map.insert("error".to_string(), Value::String(error_kind.to_string()));
            self.context = Value::Object(map);
        }
    }
}

impl Default for EvalRecord {
    fn default() -> Self {
        Self {
            created_at: Utc::now(),
            uid: create_uuid7(),
            context: Value::Null,
            record_id: String::new(),
            id: 0,
            updated_at: None,
            processing_started_at: None,
            processing_ended_at: None,
            processing_duration: None,
            entity_id: 0,
            entity_uid: String::new(),
            status: Status::Pending,
            entity_type: EntityType::Agent,
            session_id: create_uuid7(),
            retry_count: 0,
            trace_id: None,
            span_id: None,
            record_source: EvalRecordSource::User,
            tags: Vec::new(),
            media: Vec::new(),
        }
    }
}

const MEDIA_RECORD_CAP_BYTES: usize = 10 * 1024 * 1024;

fn extract_media_vec(media: Option<Vec<Bound<'_, PyAny>>>) -> PyResult<Vec<EvalMedia>> {
    let Some(media) = media else {
        return Ok(Vec::new());
    };

    media
        .into_iter()
        .map(|item| {
            if let Ok(media) = item.extract::<EvalMedia>() {
                return Ok(media);
            }
            if let Ok(wrapper) = item.extract::<PyRef<ImageMedia>>() {
                return Ok(wrapper.0.clone());
            }
            if let Ok(wrapper) = item.extract::<PyRef<DocumentMedia>>() {
                return Ok(wrapper.0.clone());
            }
            Err(PyTypeError::new_err(
                "media items must be EvalMedia, ImageMedia, or DocumentMedia",
            ))
        })
        .collect()
}

fn validate_media_record_cap(media: &[EvalMedia]) -> Result<(), RecordError> {
    let total: usize = media.iter().map(EvalMedia::decoded_byte_len).sum();
    if total > MEDIA_RECORD_CAP_BYTES {
        return Err(RecordError::ValidationError(
            "media payload exceeds 10 MiB; provide a URL instead of inline bytes for larger payloads"
                .into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod eval_record_tests {
    use super::*;

    #[test]
    fn eval_record_set_failed_with_error_populates_context_error() {
        let mut record = EvalRecord::default();
        record.set_failed_with_error("EvalRequiresTrace");

        assert_eq!(record.status, Status::Failed);
        assert_eq!(record.context["error"], "EvalRequiresTrace");
    }
}

fn extract_span_context(
    _py: Python<'_>,
    ctx: &Bound<'_, PyAny>,
) -> PyResult<(Option<TraceId>, Option<SpanId>)> {
    let is_valid = ctx
        .getattr("is_valid")
        .ok()
        .and_then(|value| value.extract::<bool>().ok())
        .unwrap_or(false);
    if !is_valid {
        return Ok((None, None));
    }

    let Some(trace_id) = ctx.getattr("trace_id").ok().and_then(|value| {
        value
            .extract::<u128>()
            .ok()
            .map(|id| TraceId::from_bytes(id.to_be_bytes()))
            .or_else(|| {
                value
                    .extract::<String>()
                    .ok()
                    .and_then(|hex| TraceId::from_hex(hex.trim_start_matches("0x")).ok())
            })
    }) else {
        return Ok((None, None));
    };

    let Some(span_id) = ctx.getattr("span_id").ok().and_then(|value| {
        value
            .extract::<u64>()
            .ok()
            .map(|id| SpanId::from_bytes(id.to_be_bytes()))
            .or_else(|| {
                value
                    .extract::<String>()
                    .ok()
                    .and_then(|hex| SpanId::from_hex(hex.trim_start_matches("0x")).ok())
            })
    }) else {
        return Ok((None, None));
    };

    Ok((Some(trace_id), Some(span_id)))
}

#[cfg(feature = "server")]
impl FromRow<'_, PgRow> for EvalRecord {
    fn from_row(row: &PgRow) -> Result<Self, sqlx::Error> {
        let context: Value = serde_json::from_value(row.try_get("context")?).unwrap_or(Value::Null);

        // load status from string
        let status_string = row.try_get::<String, &str>("status")?;
        let status = Status::from_str(&status_string).unwrap_or(Status::Pending);
        let trace_id = row
            .try_get::<Option<Vec<u8>>, &str>("trace_id")
            .ok()
            .flatten()
            .and_then(|bytes| TraceId::from_slice(&bytes).ok())
            .or_else(|| {
                row.try_get::<Option<String>, &str>("trace_id")
                    .ok()
                    .flatten()
                    .and_then(|hex| TraceId::from_hex(&hex).ok())
            });
        let span_id = row
            .try_get::<Option<Vec<u8>>, &str>("span_id")
            .ok()
            .flatten()
            .and_then(|bytes| SpanId::from_slice(&bytes).ok())
            .or_else(|| {
                row.try_get::<Option<String>, &str>("span_id")
                    .ok()
                    .flatten()
                    .and_then(|hex| SpanId::from_hex(&hex).ok())
            });

        Ok(EvalRecord {
            record_id: row.try_get("record_id")?,
            session_id: row.try_get("session_id")?,
            created_at: row.try_get("created_at")?,
            context,
            uid: row.try_get("uid")?,
            id: row.try_get("id")?,
            updated_at: row.try_get("updated_at")?,
            processing_started_at: row.try_get("processing_started_at")?,
            processing_ended_at: row.try_get("processing_ended_at")?,
            processing_duration: row.try_get("processing_duration")?,
            entity_id: row.try_get("entity_id")?,
            entity_uid: String::new(), // mask entity_uid when loading from DB
            status,
            entity_type: EntityType::Agent,
            retry_count: row.try_get("retry_count")?,
            trace_id,
            span_id,
            record_source: row
                .try_get::<String, &str>("record_source")
                .ok()
                .and_then(|value| EvalRecordSource::from_str(&value).ok())
                .unwrap_or(EvalRecordSource::User),
            tags: row.try_get::<Vec<String>, &str>("tags").unwrap_or_default(),
            media: row
                .try_get::<Json<Vec<EvalMedia>>, _>("media")
                .map(|j| j.0)
                .unwrap_or_default(),
        })
    }
}

#[pyclass(from_py_object)]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BoxedEvalRecord {
    pub record: Box<EvalRecord>,
}

impl BoxedEvalRecord {
    pub fn new(record: EvalRecord) -> Self {
        Self {
            record: Box::new(record),
        }
    }
}

#[derive(Tabled)]
pub struct WorkflowResultTableEntry {
    #[tabled(rename = "Scenario ID")]
    pub scenario_id: String,
    #[tabled(rename = "Created At")]
    pub created_at: String,
    #[tabled(rename = "Record UID")]
    pub record_uid: String,
    #[tabled(rename = "Total Tasks")]
    pub total_tasks: String,
    #[tabled(rename = "Passed")]
    pub passed_tasks: String,
    #[tabled(rename = "Failed")]
    pub failed_tasks: String,
    #[tabled(rename = "Pass Rate")]
    pub pass_rate: String,
    #[tabled(rename = "Duration (ms)")]
    pub duration_ms: String,
}

#[pyclass(from_py_object)]
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AgentEvalWorkflowResult {
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
    pub duration_ms: i64,

    pub entity_uid: String,

    pub execution_plan: ExecutionPlan,

    pub id: i64,
}

impl AgentEvalWorkflowResult {
    pub fn mask_sensitive_data(&mut self) {
        self.entity_id = -1;
    }
}

#[cfg(feature = "server")]
impl FromRow<'_, PgRow> for AgentEvalWorkflowResult {
    fn from_row(row: &PgRow) -> Result<Self, sqlx::Error> {
        let execution_plan: ExecutionPlan = serde_json::from_value(row.try_get("execution_plan")?)
            .unwrap_or(ExecutionPlan {
                stages: vec![],
                nodes: HashMap::new(),
            });

        Ok(AgentEvalWorkflowResult {
            created_at: row.try_get("created_at")?,
            record_uid: row.try_get("record_uid")?,
            entity_id: row.try_get("entity_id")?,
            total_tasks: row.try_get("total_tasks")?,
            passed_tasks: row.try_get("passed_tasks")?,
            failed_tasks: row.try_get("failed_tasks")?,
            pass_rate: row.try_get("pass_rate")?,
            duration_ms: row.try_get("duration_ms")?,
            entity_uid: String::new(), // mask entity_uid when loading from DB
            id: row.try_get("id")?,
            execution_plan,
        })
    }
}

#[pymethods]
impl AgentEvalWorkflowResult {
    pub fn __str__(&self) -> String {
        PyHelperFuncs::__str__(self)
    }

    pub fn model_dump_json(&self) -> String {
        PyHelperFuncs::__json__(self)
    }

    pub fn as_table(&self) {
        let workflow_table = self.to_table_entry(None);
        let mut table = Table::new(vec![workflow_table]);
        table.with(Style::sharp());

        table.modify(
            Rows::new(0..1),
            (
                Format::content(|s: &str| s.truecolor(245, 77, 85).bold().to_string()),
                Alignment::center(),
                Color::BOLD,
            ),
        );

        println!("\n{}", "Workflow Summary".truecolor(245, 77, 85).bold());
        println!("{}", table)
    }
}

impl AgentEvalWorkflowResult {
    pub fn to_table_entry(&self, scenario_id: Option<&str>) -> WorkflowResultTableEntry {
        let pass_rate_display = format!("{:.1}%", self.pass_rate * 100.0);
        WorkflowResultTableEntry {
            scenario_id: scenario_id.unwrap_or("").to_string(),
            created_at: self.created_at.format("%Y-%m-%d %H:%M:%S").to_string(),
            record_uid: {
                let uid = &self.record_uid;
                let suffix = uid[uid.len().saturating_sub(8)..].to_string();
                suffix.truecolor(249, 179, 93).to_string()
            },
            total_tasks: self.total_tasks.to_string(),
            passed_tasks: if self.passed_tasks > 0 {
                self.passed_tasks.to_string().green().to_string()
            } else {
                self.passed_tasks.to_string()
            },
            failed_tasks: if self.failed_tasks > 0 {
                self.failed_tasks.to_string().red().to_string()
            } else {
                self.failed_tasks.to_string()
            },
            pass_rate: if self.pass_rate >= 0.9 {
                pass_rate_display.green().to_string()
            } else if self.pass_rate >= 0.6 {
                pass_rate_display.yellow().to_string()
            } else {
                pass_rate_display.red().to_string()
            },
            duration_ms: self.duration_ms.to_string(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new(
        record_uid: String,
        total_tasks: i32,
        passed_tasks: i32,
        failed_tasks: i32,
        duration_ms: i64,
        entity_id: i32,
        entity_uid: String,
        execution_plan: ExecutionPlan,
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
            entity_uid,
            execution_plan,
            id: 0,
        }
    }
}

#[derive(Tabled)]
pub struct TaskResultTableEntry {
    #[tabled(rename = "Record ID")]
    pub record_id: String,
    #[tabled(rename = "Task ID")]
    pub task_id: String,
    #[tabled(rename = "Type")]
    pub task_type: String,
    #[tabled(rename = "✓")]
    pub passed: String,
    #[tabled(rename = "Assertion")]
    pub assertion: String,
    #[tabled(rename = "Operator")]
    pub operator: String,
    #[tabled(rename = "Expected")]
    pub expected: String,
    #[tabled(rename = "Actual")]
    pub actual: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[allow(clippy::large_enum_variant)]
pub enum Assertion {
    FieldPath(Option<String>),
    TraceAssertion(TraceAssertion),
    AgentAssertion(AgentAssertion),
}

// Detailed result for an individual evaluation task within a workflow
#[pyclass(from_py_object)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalTaskResult {
    #[pyo3(get)]
    pub created_at: DateTime<Utc>,

    #[pyo3(get)]
    pub start_time: DateTime<Utc>,

    #[pyo3(get)]
    pub end_time: DateTime<Utc>,

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

    pub assertion: Assertion,

    #[pyo3(get)]
    pub operator: ComparisonOperator,

    pub expected: Value,

    pub actual: Value,

    #[pyo3(get)]
    pub message: String,

    pub entity_uid: String,

    pub condition: bool,

    pub stage: i32,
}

#[pymethods]
impl EvalTaskResult {
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

    #[getter]
    pub fn assertion(&self) -> String {
        match &self.assertion {
            Assertion::FieldPath(path) => path.clone().unwrap_or_default(),
            Assertion::TraceAssertion(trace_assertion) => trace_assertion.to_string(),
            Assertion::AgentAssertion(agent_assertion) => agent_assertion.to_string(),
        }
    }
}

impl EvalTaskResult {
    pub(crate) fn to_table_entry(&self, record_id: &str) -> TaskResultTableEntry {
        let expected_str =
            serde_json::to_string(&self.expected).unwrap_or_else(|_| "null".to_string());
        let actual_str = serde_json::to_string(&self.actual).unwrap_or_else(|_| "null".to_string());

        let truncate = |s: String, max_len: usize| {
            if s.len() > max_len {
                format!("{}...", &s[..max_len.saturating_sub(3)])
            } else {
                s
            }
        };

        TaskResultTableEntry {
            record_id: truncate(record_id.to_string(), 16)
                .truecolor(249, 179, 93)
                .to_string(),
            task_id: truncate(self.task_id.clone(), 20),
            task_type: self.task_type.to_string(),
            passed: if self.passed {
                "✓".green().to_string()
            } else {
                "✗".red().to_string()
            },
            assertion: truncate(self.assertion(), 22),
            operator: self.operator.to_string(),
            expected: truncate(expected_str, 20),
            actual: truncate(actual_str, 30),
        }
    }
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
        record_uid: String,
        task_id: String,
        task_type: EvaluationTaskType,
        passed: bool,
        value: f64,
        assertion: Assertion,
        operator: ComparisonOperator,
        expected: Value,
        actual: Value,
        message: String,
        entity_id: i32,
        entity_uid: String,
        condition: bool,
        stage: i32,
    ) -> Self {
        Self {
            start_time,
            end_time,
            record_uid,
            created_at: Utc::now(),
            task_id,
            task_type,
            passed,
            value,
            assertion,
            operator,
            expected,
            actual,
            message,
            entity_id,
            entity_uid,
            condition,
            stage,
        }
    }
}

impl Default for EvalTaskResult {
    fn default() -> Self {
        Self {
            created_at: Utc::now(),
            start_time: Utc::now(),
            end_time: Utc::now(),
            record_uid: String::new(),
            task_id: String::new(),
            task_type: EvaluationTaskType::Assertion,
            passed: false,
            value: 0.0,
            assertion: Assertion::FieldPath(None),
            operator: ComparisonOperator::Equals,
            expected: Value::Null,
            actual: Value::Null,
            message: String::new(),
            entity_id: 0,
            entity_uid: String::new(),
            condition: false,
            stage: 0,
        }
    }
}

#[cfg(feature = "server")]
impl FromRow<'_, PgRow> for EvalTaskResult {
    fn from_row(row: &PgRow) -> Result<Self, sqlx::Error> {
        let expected: Value =
            serde_json::from_value(row.try_get("expected")?).unwrap_or(Value::Null);
        let actual: Value = serde_json::from_value(row.try_get("actual")?).unwrap_or(Value::Null);
        let task_type: EvaluationTaskType =
            EvaluationTaskType::from_str(&row.try_get::<String, &str>("task_type")?)
                .unwrap_or(EvaluationTaskType::Assertion);
        let comparison_operator: ComparisonOperator =
            ComparisonOperator::from_str(&row.try_get::<String, &str>("operator")?)
                .unwrap_or(ComparisonOperator::Equals);
        let assertion: Assertion =
            serde_json::from_value(row.try_get("assertion")?).unwrap_or(Assertion::FieldPath(None));

        Ok(EvalTaskResult {
            record_uid: row.try_get("record_uid")?,
            created_at: row.try_get("created_at")?,
            start_time: row.try_get("start_time")?,
            end_time: row.try_get("end_time")?,
            task_id: row.try_get("task_id")?,
            task_type,
            passed: row.try_get("passed")?,
            value: row.try_get("value")?,
            assertion,
            operator: comparison_operator,
            expected,
            actual,
            message: row.try_get("message")?,
            entity_id: row.try_get("entity_id")?,

            // empty here, not needed for server operations
            // entity_uid is only used when creating new records
            entity_uid: String::new(),
            condition: row.try_get("condition")?,
            stage: row.try_get("stage")?,
        })
    }
}

#[pyclass(from_py_object)]
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

    pub entity_id: i32,
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
            entity_id: 0,
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

#[pyclass(from_py_object)]
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

#[pyclass(from_py_object)]
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

#[pyclass(from_py_object)]
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

    pub entity_id: i32,
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

#[pyclass(from_py_object)]
#[derive(Debug, Serialize, Deserialize, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum ServerRecord {
    Spc(SpcRecord),
    Psi(PsiRecord),
    Custom(CustomMetricRecord),
    Observability(ObservabilityMetrics),
    AgentEval(BoxedEvalRecord),
    AgentTaskRecord(EvalTaskResult),
    AgentWorkflowRecord(AgentEvalWorkflowResult),
}

#[pymethods]
impl ServerRecord {
    #[getter]
    pub fn record<'py>(&self, py: Python<'py>) -> Result<Bound<'py, PyAny>, RecordError> {
        match self {
            ServerRecord::Spc(record) => Ok(PyHelperFuncs::to_bound_py_object(py, record)?),
            ServerRecord::Psi(record) => Ok(PyHelperFuncs::to_bound_py_object(py, record)?),
            ServerRecord::Custom(record) => Ok(PyHelperFuncs::to_bound_py_object(py, record)?),
            ServerRecord::Observability(record) => {
                Ok(PyHelperFuncs::to_bound_py_object(py, record)?)
            }
            ServerRecord::AgentEval(record) => {
                // unbox the record
                let record = record.record.as_ref();
                Ok(PyHelperFuncs::to_bound_py_object(py, record)?)
            }
            ServerRecord::AgentTaskRecord(record) => {
                Ok(PyHelperFuncs::to_bound_py_object(py, record)?)
            }
            ServerRecord::AgentWorkflowRecord(record) => {
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
            ServerRecord::AgentEval(record) => record.record.__str__(),
            ServerRecord::AgentTaskRecord(record) => record.__str__(),
            ServerRecord::AgentWorkflowRecord(record) => record.__str__(),
        }
    }

    pub fn get_record_type(&self) -> RecordType {
        match self {
            ServerRecord::Spc(_) => RecordType::Spc,
            ServerRecord::Psi(_) => RecordType::Psi,
            ServerRecord::Custom(_) => RecordType::Custom,
            ServerRecord::Observability(_) => RecordType::Observability,
            ServerRecord::AgentEval(_) => RecordType::AgentEval,
            ServerRecord::AgentTaskRecord(_) => RecordType::AgentTask,
            ServerRecord::AgentWorkflowRecord(_) => RecordType::AgentWorkflow,
        }
    }
}

#[pyclass(from_py_object)]
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
                ServerRecord::AgentEval(inner) => Ok(&inner.record.entity_uid), // this is the profile uid
                ServerRecord::AgentTaskRecord(inner) => Ok(&inner.entity_uid),
                ServerRecord::AgentWorkflowRecord(inner) => Ok(&inner.entity_uid),
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

impl IntoServerRecord for EvalRecord {
    fn into_server_record(self) -> ServerRecord {
        ServerRecord::AgentEval(BoxedEvalRecord::new(self))
    }
}

impl IntoServerRecord for AgentEvalWorkflowResult {
    fn into_server_record(self) -> ServerRecord {
        ServerRecord::AgentWorkflowRecord(self)
    }
}
impl IntoServerRecord for EvalTaskResult {
    fn into_server_record(self) -> ServerRecord {
        ServerRecord::AgentTaskRecord(self)
    }
}

/// Helper trait to convert ServerRecord to their respective internal record types
pub trait ToDriftRecords {
    fn to_spc_drift_records(self) -> Result<Vec<SpcRecord>, RecordError>;
    fn to_observability_drift_records(self) -> Result<Vec<ObservabilityMetrics>, RecordError>;
    fn to_psi_drift_records(self) -> Result<Vec<PsiRecord>, RecordError>;
    fn to_custom_metric_drift_records(self) -> Result<Vec<CustomMetricRecord>, RecordError>;
    fn to_agent_eval_records(self) -> Result<Vec<BoxedEvalRecord>, RecordError>;
    fn to_agent_workflow_records(self) -> Result<Vec<AgentEvalWorkflowResult>, RecordError>;
    fn to_agent_task_records(self) -> Result<Vec<EvalTaskResult>, RecordError>;
}

impl ToDriftRecords for ServerRecords {
    fn to_spc_drift_records(self) -> Result<Vec<SpcRecord>, RecordError> {
        extract_owned_records(self.records, |record| match record {
            ServerRecord::Spc(inner) => Some(inner),
            _ => None,
        })
    }

    fn to_observability_drift_records(self) -> Result<Vec<ObservabilityMetrics>, RecordError> {
        extract_owned_records(self.records, |record| match record {
            ServerRecord::Observability(inner) => Some(inner),
            _ => None,
        })
    }

    fn to_psi_drift_records(self) -> Result<Vec<PsiRecord>, RecordError> {
        extract_owned_records(self.records, |record| match record {
            ServerRecord::Psi(inner) => Some(inner),
            _ => None,
        })
    }

    fn to_custom_metric_drift_records(self) -> Result<Vec<CustomMetricRecord>, RecordError> {
        extract_owned_records(self.records, |record| match record {
            ServerRecord::Custom(inner) => Some(inner),
            _ => None,
        })
    }

    fn to_agent_eval_records(self) -> Result<Vec<BoxedEvalRecord>, RecordError> {
        extract_owned_records(self.records, |record| match record {
            ServerRecord::AgentEval(inner) => Some(inner),
            _ => None,
        })
    }

    fn to_agent_workflow_records(self) -> Result<Vec<AgentEvalWorkflowResult>, RecordError> {
        extract_owned_records(self.records, |record| match record {
            ServerRecord::AgentWorkflowRecord(inner) => Some(inner),
            _ => None,
        })
    }

    fn to_agent_task_records(self) -> Result<Vec<EvalTaskResult>, RecordError> {
        extract_owned_records(self.records, |record| match record {
            ServerRecord::AgentTaskRecord(inner) => Some(inner),
            _ => None,
        })
    }
}

// Replace extract_records with this consuming version
fn extract_owned_records<T>(
    records: Vec<ServerRecord>,
    extractor: impl Fn(ServerRecord) -> Option<T>,
) -> Result<Vec<T>, RecordError> {
    let mut extracted = Vec::with_capacity(records.len());

    for record in records {
        if let Some(value) = extractor(record) {
            extracted.push(value);
        } else {
            return Err(RecordError::InvalidDriftTypeError);
        }
    }

    Ok(extracted)
}

#[derive(Debug)]
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
    EvalRecord,
    SpcRecord,
    PsiRecord,
    CustomMetricRecord,
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_eval_record_tags_defaults_empty() {
        let record = EvalRecord::default();
        assert!(record.tags.is_empty());
    }

    #[test]
    fn test_eval_record_tags_can_be_set() {
        let record = EvalRecord {
            tags: vec!["scenario_id=scenario_42".to_string()],
            ..Default::default()
        };
        assert_eq!(record.tags, vec!["scenario_id=scenario_42"]);
    }

    #[test]
    fn test_eval_record_tags_serde_roundtrip() {
        let record = EvalRecord {
            tags: vec!["scenario_id=scenario_1".to_string(), "env=test".to_string()],
            ..Default::default()
        };
        let json = serde_json::to_string(&record).unwrap();
        let deserialized: EvalRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.tags, record.tags);
    }

    #[test]
    fn test_eval_record_tags_missing_in_json_defaults_empty() {
        // Verify #[serde(default)] allows deserializing from older JSON without "tags" field
        let record = EvalRecord::default();
        let mut json_val: serde_json::Value = serde_json::to_value(&record).unwrap();
        json_val.as_object_mut().unwrap().remove("tags");
        let deserialized: EvalRecord = serde_json::from_value(json_val).unwrap();
        assert!(deserialized.tags.is_empty());
    }

    #[test]
    fn eval_record_accepts_eval_media_directly() {
        use crate::agent::{EvalMediaKind, EvalMediaSource};

        let media = EvalMedia {
            id: "chart".to_string(),
            kind: EvalMediaKind::Image,
            source: EvalMediaSource::Url {
                url: "https://example.com/chart.png".to_string(),
                mime_type: Some("image/png".to_string()),
            },
        };
        let record = EvalRecord {
            media: vec![media.clone()],
            ..Default::default()
        };

        assert_eq!(record.media, vec![media]);
    }

    #[test]
    fn eval_record_accepts_image_media_wrapper() {
        use crate::agent::{EvalMediaKind, EvalMediaSource};

        let wrapper = ImageMedia(EvalMedia {
            id: "chart".to_string(),
            kind: EvalMediaKind::Image,
            source: EvalMediaSource::Url {
                url: "https://example.com/chart.png".to_string(),
                mime_type: None,
            },
        });
        let record = EvalRecord {
            media: vec![wrapper.clone().into_eval_media()],
            ..Default::default()
        };

        assert_eq!(record.media, vec![wrapper.0]);
    }

    #[test]
    fn eval_record_accepts_document_media_wrapper() {
        use crate::agent::{EvalMediaKind, EvalMediaSource};

        let wrapper = DocumentMedia(EvalMedia {
            id: "report".to_string(),
            kind: EvalMediaKind::Document,
            source: EvalMediaSource::Base64 {
                mime_type: "application/pdf".to_string(),
                data: "JVBERg==".to_string(),
            },
        });
        let record = EvalRecord {
            media: vec![wrapper.clone().into_eval_media()],
            ..Default::default()
        };

        assert_eq!(record.media, vec![wrapper.0]);
    }

    #[test]
    fn eval_record_media_missing_in_json_defaults_empty() {
        use crate::agent::{EvalMediaKind, EvalMediaSource};

        let record = EvalRecord {
            media: vec![EvalMedia {
                id: "chart".to_string(),
                kind: EvalMediaKind::Image,
                source: EvalMediaSource::Url {
                    url: "https://example.com/chart.png".to_string(),
                    mime_type: None,
                },
            }],
            ..Default::default()
        };
        let mut json_val: serde_json::Value = serde_json::to_value(&record).unwrap();
        json_val.as_object_mut().unwrap().remove("media");
        let deserialized: EvalRecord = serde_json::from_value(json_val).unwrap();

        assert!(deserialized.media.is_empty());
    }

    #[test]
    fn eval_record_rejects_oversized_media() {
        use crate::agent::{EvalMediaKind, EvalMediaSource};

        let media = EvalMedia {
            id: "chart".to_string(),
            kind: EvalMediaKind::Image,
            source: EvalMediaSource::Base64 {
                mime_type: "image/png".to_string(),
                data: "a".repeat((MEDIA_RECORD_CAP_BYTES + 1) * 4 / 3 + 4),
            },
        };

        let err = validate_media_record_cap(&[media]).unwrap_err();

        assert!(matches!(err, RecordError::ValidationError(_)));
    }

    #[test]
    fn eval_record_url_media_does_not_count_toward_cap() {
        use crate::agent::{EvalMediaKind, EvalMediaSource};

        let inline = EvalMedia {
            id: "chart".to_string(),
            kind: EvalMediaKind::Image,
            source: EvalMediaSource::Base64 {
                mime_type: "image/png".to_string(),
                data: "a".repeat(9 * 1024 * 1024 * 4 / 3),
            },
        };
        let url = EvalMedia {
            id: "report".to_string(),
            kind: EvalMediaKind::Document,
            source: EvalMediaSource::Url {
                url: "https://example.com/report.pdf".to_string(),
                mime_type: Some("application/pdf".to_string()),
            },
        };

        validate_media_record_cap(&[url, inline]).unwrap();
    }
}
