use pyo3::prelude::*;
pub use scouter_client::{
    evaluate_llm, Embedding, LLMEvalMetric, LLMEvalRecord, LLMEvalResults, LLMEvalTaskResult,
    MetricResult,
};

#[pymodule]
pub fn evaluate(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Embedding>()?;
    m.add_class::<LLMEvalTaskResult>()?;
    m.add_class::<MetricResult>()?;
    m.add_class::<LLMEvalMetric>()?;
    m.add_class::<LLMEvalRecord>()?;
    m.add_class::<LLMEvalResults>()?;
    m.add_function(wrap_pyfunction!(evaluate_llm, m)?)?;
    Ok(())
}
