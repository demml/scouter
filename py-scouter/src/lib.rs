pub mod agent;
pub mod alert;
pub mod bifrost;
pub mod client;
pub mod drift;
pub mod evaluate;
pub mod logging;
pub mod mock;
pub mod observe;
pub mod profile;
pub mod queue;
pub mod tracing;
pub mod transport;
pub mod types;

use pyo3::prelude::*;

#[pymodule]
fn _scouter(m: &Bound<'_, PyModule>) -> PyResult<()> {
    bifrost::add_dataset_module(m)?;
    queue::add_queue_module(m)?;
    logging::add_logging_module(m)?;
    client::add_client_module(m)?;
    drift::add_drift_module(m)?;
    alert::add_alert_module(m)?;
    types::add_types_module(m)?;
    profile::add_profile_module(m)?;
    observe::add_observe_module(m)?;
    mock::add_mock_module(m)?;
    agent::add_agent_module(m)?;
    evaluate::add_evaluate_module(m)?;
    tracing::add_tracing_module(m)?;
    transport::add_transport_module(m)?;

    Ok(())
}
