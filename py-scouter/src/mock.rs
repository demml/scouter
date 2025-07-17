use pyo3::prelude::*;
use scouter_client::MockConfig;
use scouter_mocks::{OpenAITestServer, ScouterTestServer};

#[pymodule]
pub fn mock(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<ScouterTestServer>()?;
    m.add_class::<OpenAITestServer>()?;
    m.add_class::<MockConfig>()?;

    Ok(())
}
