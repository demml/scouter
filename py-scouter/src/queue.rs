use pyo3::prelude::*;
use scouter_client::*;

#[pymodule]
pub fn queue(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<ScouterQueue>()?;
    m.add_class::<ScouterProducer>()?;
    m.add_class::<KafkaConfig>()?;
    m.add_class::<RabbitMQConfig>()?;
    m.add_class::<HTTPConfig>()?;
    Ok(())
}
