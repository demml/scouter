use pyo3::prelude::*;
pub use scouter_client::{EvaluationConfig, GenAIEvalResults};

pub fn add_evaluate_module(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<GenAIEvalResults>()?;
    m.add_class::<EvaluationConfig>()?;
    //m.add_function(wrap_pyfunction!(evaluate_genai, m)?)?;
    Ok(())
}
