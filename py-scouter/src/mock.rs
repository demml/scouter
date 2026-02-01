use potato_head::mock::LLMTestServer;
use pyo3::prelude::*;
use scouter_client::MockConfig;
use scouter_mocks::{
    create_multi_service_trace, create_nested_trace, create_sequence_pattern_trace,
    create_simple_trace, create_trace_with_attributes, create_trace_with_errors, ScouterTestServer,
};

pub fn add_mock_module(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<ScouterTestServer>()?;
    m.add_class::<LLMTestServer>()?;
    m.add_class::<MockConfig>()?;
    m.add_function(wrap_pyfunction!(create_simple_trace, m)?)?;
    m.add_function(wrap_pyfunction!(create_nested_trace, m)?)?;
    m.add_function(wrap_pyfunction!(create_trace_with_attributes, m)?)?;
    m.add_function(wrap_pyfunction!(create_multi_service_trace, m)?)?;
    m.add_function(wrap_pyfunction!(create_sequence_pattern_trace, m)?)?;
    m.add_function(wrap_pyfunction!(create_trace_with_errors, m)?)?;

    Ok(())
}
