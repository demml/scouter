use crate::error::RecordError;
use crate::genai::{ComparisonOperator, EvaluationTaskType, ExecutionPlan};
use crate::trace::TraceServerRecord;
use crate::{depythonize_object_to_value, DriftType, Status};
use crate::{EntityType, TagRecord};
use chrono::DateTime;
use chrono::Utc;
use owo_colors::OwoColorize;
use potato_head::create_uuid7;
use potato_head::PyHelperFuncs;
use pyo3::prelude::*;
use pythonize::pythonize;
use scouter_macro::impl_mask_entity_id;
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[cfg(feature = "server")]
use sqlx::{postgres::PgRow, FromRow, Row};
use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt::Display;
use std::str::FromStr;
use tabled::Tabled;
use tabled::{
    settings::{object::Rows, Alignment, Color, Format, Style},
    Table,
};

#[pyclass(eq)]
#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
pub enum RecordType {
    #[default]
    Spc,
    Psi,
    Observability,
    Custom,
    GenAIEval,
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
            RecordType::GenAIEval => DriftType::GenAI.to_string(),
            RecordType::GenAITask => DriftType::GenAI.to_string(),
            RecordType::GenAIWorkflow => DriftType::GenAI.to_string(),
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
            "genai_event" => Ok(RecordType::GenAIEval),
            "genai_task" => Ok(RecordType::GenAITask),
            "genai_workflow" => Ok(RecordType::GenAIWorkflow),
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
            RecordType::GenAIEval => "genai_event",
            RecordType::GenAITask => "genai_task",
            RecordType::GenAIWorkflow => "genai_workflow",
            RecordType::Trace => "trace",
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

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GenAIEvalRecord {
    #[pyo3(get, set)]
    pub record_id: String,
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
}

#[pymethods]
impl GenAIEvalRecord {
    #[new]
    #[pyo3(signature = (context=None, id = None))]

    /// Creates a new GenAIEvalRecord instance.
    /// The context is either a python dictionary or a pydantic basemodel.
    pub fn new(
        py: Python<'_>,
        context: Option<Bound<'_, PyAny>>,
        id: Option<String>,
    ) -> Result<Self, RecordError> {
        // check if context is a PyDict or PyObject(Pydantic model)
        let context_val = match context {
            Some(ctx) => depythonize_object_to_value(py, &ctx)?,
            None => Value::Object(serde_json::Map::new()),
        };

        Ok(GenAIEvalRecord {
            uid: create_uuid7(),
            created_at: Utc::now(),
            context: context_val,
            record_id: id.unwrap_or_default(),

            ..Default::default()
        })
    }
    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        PyHelperFuncs::__str__(self)
    }

    pub fn get_record_type(&self) -> RecordType {
        RecordType::GenAIEval
    }

    pub fn model_dump_json(&self) -> String {
        // serialize the struct to a string
        PyHelperFuncs::__json__(self)
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
}

impl GenAIEvalRecord {
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
            entity_type: EntityType::GenAI,
        }
    }

    // helper for masking sensitive data from the record when
    // return to the user. Currently, only removes entity_id
    pub fn mask_sensitive_data(&mut self) {
        self.entity_id = -1;
    }
}

impl Default for GenAIEvalRecord {
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
            entity_type: EntityType::GenAI,
        }
    }
}

#[cfg(feature = "server")]
impl FromRow<'_, PgRow> for GenAIEvalRecord {
    fn from_row(row: &PgRow) -> Result<Self, sqlx::Error> {
        let context: Value = serde_json::from_value(row.try_get("context")?).unwrap_or(Value::Null);

        // load status from string
        let status_string = row.try_get::<String, &str>("status")?;
        let status = Status::from_str(&status_string).unwrap_or(Status::Pending);

        Ok(GenAIEvalRecord {
            record_id: row.try_get("record_id")?,
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
            entity_type: EntityType::GenAI,
        })
    }
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BoxedGenAIEvalRecord {
    pub record: Box<GenAIEvalRecord>,
}

impl BoxedGenAIEvalRecord {
    pub fn new(record: GenAIEvalRecord) -> Self {
        Self {
            record: Box::new(record),
        }
    }
}

#[derive(Tabled)]
pub struct WorkflowResultTableEntry {
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

#[pyclass]
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GenAIEvalWorkflowResult {
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

impl GenAIEvalWorkflowResult {
    pub fn mask_sensitive_data(&mut self) {
        self.entity_id = -1;
    }
}

#[cfg(feature = "server")]
impl FromRow<'_, PgRow> for GenAIEvalWorkflowResult {
    fn from_row(row: &PgRow) -> Result<Self, sqlx::Error> {
        let execution_plan: ExecutionPlan = serde_json::from_value(row.try_get("execution_plan")?)
            .unwrap_or(ExecutionPlan {
                stages: vec![],
                nodes: HashMap::new(),
            });

        Ok(GenAIEvalWorkflowResult {
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
impl GenAIEvalWorkflowResult {
    pub fn __str__(&self) -> String {
        PyHelperFuncs::__str__(self)
    }

    pub fn model_dump_json(&self) -> String {
        PyHelperFuncs::__json__(self)
    }

    pub fn as_table(&self) {
        let workflow_table = self.to_table_entry();
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

impl GenAIEvalWorkflowResult {
    pub fn to_table_entry(&self) -> WorkflowResultTableEntry {
        let pass_rate_display = format!("{:.1}%", self.pass_rate * 100.0);
        WorkflowResultTableEntry {
            created_at: self.created_at.format("%Y-%m-%d %H:%M:%S").to_string(),
            record_uid: self.record_uid.truecolor(249, 179, 93).to_string(),
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
    #[tabled(rename = "Created At")]
    pub created_at: String,
    #[tabled(rename = "Record UID")]
    pub record_uid: String,
    #[tabled(rename = "Task ID")]
    pub task_id: String,
    #[tabled(rename = "Task Type")]
    pub task_type: String,
    #[tabled(rename = "Condition")]
    pub condition: bool,
    #[tabled(rename = "Stage")]
    pub stage: i32,
    #[tabled(rename = "Passed")]
    pub passed: String,
    #[tabled(rename = "Field Path")]
    pub field_path: String,
    #[tabled(rename = "Operator")]
    pub operator: String,
    #[tabled(rename = "Expected")]
    pub expected: String,
    #[tabled(rename = "Actual")]
    pub actual: String,
}

// Detailed result for an individual evaluation task within a workflow
#[pyclass]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenAIEvalTaskResult {
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

    #[pyo3(get)]
    pub field_path: Option<String>,

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
impl GenAIEvalTaskResult {
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

    pub fn as_table(&self) {
        let task_table = self.to_table_entry();
        let mut table = Table::new(vec![task_table]);
        table.with(Style::sharp());

        table.modify(
            Rows::new(0..1),
            (
                Format::content(|s: &str| s.truecolor(245, 77, 85).bold().to_string()),
                Alignment::center(),
                Color::BOLD,
            ),
        );

        println!("\n{}", "Task Details".truecolor(245, 77, 85).bold());
        println!("{}", table);
    }
}

impl GenAIEvalTaskResult {
    pub(crate) fn to_table_entry(&self) -> TaskResultTableEntry {
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
            created_at: self.created_at.format("%Y-%m-%d %H:%M:%S").to_string(),

            record_uid: truncate(self.record_uid.clone(), 16)
                .truecolor(249, 179, 93)
                .to_string(), // UUIDs are ~36 chars
            task_id: truncate(self.task_id.clone(), 16),
            task_type: self.task_type.to_string(),
            condition: self.condition,
            stage: self.stage,
            passed: if self.passed {
                "✓".green().to_string()
            } else {
                "✗".red().to_string()
            },

            field_path: truncate(self.field_path.clone().unwrap_or_default(), 12),
            operator: self.operator.to_string(),
            expected: truncate(expected_str, 20),
            actual: truncate(actual_str, 20),
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
        field_path: Option<String>,
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
            field_path,
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

#[cfg(feature = "server")]
impl FromRow<'_, PgRow> for GenAIEvalTaskResult {
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

        Ok(GenAIEvalTaskResult {
            record_uid: row.try_get("record_uid")?,
            created_at: row.try_get("created_at")?,
            start_time: row.try_get("start_time")?,
            end_time: row.try_get("end_time")?,
            task_id: row.try_get("task_id")?,
            task_type,
            passed: row.try_get("passed")?,
            value: row.try_get("value")?,
            field_path: row.try_get("field_path")?,
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

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum ServerRecord {
    Spc(SpcRecord),
    Psi(PsiRecord),
    Custom(CustomMetricRecord),
    Observability(ObservabilityMetrics),
    GenAIEval(BoxedGenAIEvalRecord),
    GenAITaskRecord(GenAIEvalTaskResult),
    GenAIWorkflowRecord(GenAIEvalWorkflowResult),
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
            ServerRecord::GenAIEval(record) => {
                // unbox the record
                let record = record.record.as_ref();
                Ok(PyHelperFuncs::to_bound_py_object(py, record)?)
            }
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
            ServerRecord::GenAIEval(record) => record.record.__str__(),
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
            ServerRecord::GenAIEval(_) => RecordType::GenAIEval,
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
                ServerRecord::GenAIEval(inner) => Ok(&inner.record.entity_uid), // this is the profile uid
                ServerRecord::GenAITaskRecord(inner) => Ok(&inner.entity_uid),
                ServerRecord::GenAIWorkflowRecord(inner) => Ok(&inner.entity_uid),
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

impl IntoServerRecord for GenAIEvalRecord {
    fn into_server_record(self) -> ServerRecord {
        ServerRecord::GenAIEval(BoxedGenAIEvalRecord::new(self))
    }
}

impl IntoServerRecord for GenAIEvalWorkflowResult {
    fn into_server_record(self) -> ServerRecord {
        ServerRecord::GenAIWorkflowRecord(self)
    }
}
impl IntoServerRecord for GenAIEvalTaskResult {
    fn into_server_record(self) -> ServerRecord {
        ServerRecord::GenAITaskRecord(self)
    }
}

/// Helper trait to convert ServerRecord to their respective internal record types
pub trait ToDriftRecords {
    fn to_spc_drift_records(self) -> Result<Vec<SpcRecord>, RecordError>;
    fn to_observability_drift_records(self) -> Result<Vec<ObservabilityMetrics>, RecordError>;
    fn to_psi_drift_records(self) -> Result<Vec<PsiRecord>, RecordError>;
    fn to_custom_metric_drift_records(self) -> Result<Vec<CustomMetricRecord>, RecordError>;
    fn to_genai_eval_records(self) -> Result<Vec<BoxedGenAIEvalRecord>, RecordError>;
    fn to_genai_workflow_records(self) -> Result<Vec<GenAIEvalWorkflowResult>, RecordError>;
    fn to_genai_task_records(self) -> Result<Vec<GenAIEvalTaskResult>, RecordError>;
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

    fn to_genai_eval_records(self) -> Result<Vec<BoxedGenAIEvalRecord>, RecordError> {
        extract_owned_records(self.records, |record| match record {
            ServerRecord::GenAIEval(inner) => Some(inner),
            _ => None,
        })
    }

    fn to_genai_workflow_records(self) -> Result<Vec<GenAIEvalWorkflowResult>, RecordError> {
        extract_owned_records(self.records, |record| match record {
            ServerRecord::GenAIWorkflowRecord(inner) => Some(inner),
            _ => None,
        })
    }

    fn to_genai_task_records(self) -> Result<Vec<GenAIEvalTaskResult>, RecordError> {
        extract_owned_records(self.records, |record| match record {
            ServerRecord::GenAITaskRecord(inner) => Some(inner),
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
    GenAIEvalRecord,
    SpcRecord,
    PsiRecord,
    CustomMetricRecord,
);
