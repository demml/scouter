use potato_head::LLMTestServer;
use pyo3::prelude::*;
use scouter_client::MockConfig;
use scouter_mocks::ScouterTestServer;

#[pymodule]
pub fn mock(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<ScouterTestServer>()?;
    m.add_class::<LLMTestServer>()?;
    m.add_class::<MockConfig>()?;

    Ok(())
}
