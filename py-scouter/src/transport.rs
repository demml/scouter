use pyo3::prelude::*;
use scouter_client::*;

#[pymodule]
pub fn transport(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<KafkaConfig>()?;
    m.add_class::<RabbitMQConfig>()?;
    m.add_class::<RedisConfig>()?;
    m.add_class::<HTTPConfig>()?;
    Ok(())
}
