use pyo3::prelude::*;
use scouter_client::MockConfig;
use scouter_mocks::ScouterTestServer;

#[pymodule]
pub fn mock(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<ScouterTestServer>()?;
    m.add_class::<MockConfig>()?;
    // Add classes based on feature flags
    #[cfg(feature = "server")]
    {
        m.add_class::<scouter_mocks::PotatoPyAgent>()?;
        m.add_class::<scouter_mocks::PotatoWorkflow>()?;
        m.add_class::<scouter_mocks::PotatoTask>()?;
        m.add_class::<scouter_mocks::PotatoPrompt>()?;
        m.add_class::<scouter_mocks::PotatoScore>()?;
        m.add_class::<scouter_mocks::PotatoOpenAITestServer>()?;
    }

    #[cfg(not(feature = "server"))]
    {
        m.add_class::<scouter_mocks::MockAgent>()?;
        m.add_class::<scouter_mocks::MockWorkflow>()?;
        m.add_class::<scouter_mocks::MockTask>()?;
        m.add_class::<scouter_mocks::MockPrompt>()?;
        m.add_class::<scouter_mocks::MockScore>()?;
        m.add_class::<scouter_mocks::MockOpenAITestServer>()?;
    }

    Ok(())
}
