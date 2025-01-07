use ndarray::ArrayView2;

pub struct ArrayData<'a> {
    pub string_features: Vec<String>,
    pub string_array: Vec<Vec<String>>,
    pub numeric_features: Vec<String>,
    pub numeric_array: Option<ArrayView2<f32>>,
}