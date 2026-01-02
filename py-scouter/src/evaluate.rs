use pyo3::prelude::*;
pub use scouter_client::{
    evaluate_genai, EvaluationConfig, GenAIEvalMetric, GenAIEvalRecord, GenAIEvalResults,
    GenAIEvalTaskResult,
};

pub fn add_evaluate_module(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<GenAIEvalTaskResult>()?;
    m.add_class::<GenAIEvalMetric>()?;
    m.add_class::<GenAIEvalRecord>()?;
    m.add_class::<GenAIEvalResults>()?;
    m.add_class::<EvaluationConfig>()?;
    m.add_function(wrap_pyfunction!(evaluate_genai, m)?)?;
    Ok(())
}
