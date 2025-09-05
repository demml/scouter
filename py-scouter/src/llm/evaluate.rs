use pyo3::prelude::*;
pub use scouter_client::{evaluate_llm, EvalResult, LLMEvalMetric, LLMEvalRecord, LLMEvalResults};

#[pymodule]
pub fn evaluate(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<EvalResult>()?;
    m.add_class::<LLMEvalMetric>()?;
    m.add_class::<LLMEvalRecord>()?;
    m.add_class::<LLMEvalResults>()?;
    m.add_function(wrap_pyfunction!(evaluate_llm, m)?)?;
    Ok(())
}
