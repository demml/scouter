use pyo3::prelude::*;

pub type ConvertedData<'py> = (
    Vec<String>,
    Option<Bound<'py, PyAny>>,
    Option<String>,
    Vec<String>,
    Option<Vec<Vec<String>>>,
);

pub struct DataTypes {
    pub integer_features: Vec<String>,
    pub float_features: Vec<String>,
    pub string_features: Vec<String>,
    pub numeric_features: Vec<String>,
}

impl DataTypes {
    pub fn new(
        integer_features: Vec<String>,
        float_features: Vec<String>,
        string_features: Vec<String>,
    ) -> Self {
        let numeric_features = integer_features
            .iter()
            .chain(float_features.iter())
            .cloned()
            .collect();
        Self {
            integer_features,
            float_features,
            string_features,
            numeric_features,
        }
    }

    pub fn has_mixed_types(&self) -> bool {
        !self.integer_features.is_empty() && !self.float_features.is_empty()
    }
}

pub enum NumericType {
    Integer,
    Float,
}
