use pyo3::prelude::*;
use scouter_client::DatasetClient;

pub fn add_dataset_module(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<DatasetClient>()?;
    Ok(())
}
