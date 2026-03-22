use pyo3::prelude::*;
use scouter_client::{DatasetClient, DatasetProducer, TableConfig, WriteConfig};

pub fn add_dataset_module(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<DatasetClient>()?;
    m.add_class::<DatasetProducer>()?;
    m.add_class::<TableConfig>()?;
    m.add_class::<WriteConfig>()?;
    Ok(())
}
