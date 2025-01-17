use pyo3::prelude::*;
use scouter_client::*;

#[pymodule]
pub fn client(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<ScouterClient>()?;
    m.add_class::<SpcFeatureResult>()?;
    m.add_class::<DriftRequest>()?;
    m.add_class::<TimeInterval>()?;
    Ok(())
}
