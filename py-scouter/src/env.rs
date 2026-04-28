use pyo3::prelude::*;

#[pyclass]
pub struct ScouterEnv;

#[pymethods]
impl ScouterEnv {
    #[staticmethod]
    pub fn set_offline() {
        std::env::set_var("SCOUTER_OFFLINE", "1");
    }

    #[staticmethod]
    pub fn is_offline() -> bool {
        std::env::var("SCOUTER_OFFLINE").as_deref() == Ok("1")
    }
}

pub fn add_env_module(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<ScouterEnv>()?;
    Ok(())
}
