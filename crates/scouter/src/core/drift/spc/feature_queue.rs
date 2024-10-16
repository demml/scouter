use crate::core::drift::base::ServerRecords;
use crate::core::drift::spc::monitor::SpcMonitor;
use crate::core::drift::spc::types::SpcDriftProfile;
use crate::core::error::FeatureQueueError;
use core::result::Result::Ok;
use ndarray::prelude::*;
use ndarray::Array2;
use pyo3::prelude::*;
use pyo3::types::PyAny;
use std::collections::HashMap;

#[pyclass]
pub struct SpcFeatureQueue {
    pub drift_profile: SpcDriftProfile,
    pub queue: HashMap<String, Vec<f64>>,
    pub mapped_features: Vec<String>,
    pub feature_names: Vec<String>,
    pub monitor: SpcMonitor,
}

#[pymethods]
impl SpcFeatureQueue {
    #[new]
    pub fn new(drift_profile: SpcDriftProfile) -> Self {
        let queue: HashMap<String, Vec<f64>> = drift_profile
            .features
            .keys()
            .map(|feature| (feature.clone(), Vec::new()))
            .collect();

        let mapped_features = if drift_profile.config.feature_map.is_some() {
            drift_profile
                .config
                .feature_map
                .as_ref()
                .unwrap()
                .features
                .keys()
                .cloned()
                .collect()
        } else {
            Vec::new()
        };

        let feature_names = queue.keys().cloned().collect();

        SpcFeatureQueue {
            drift_profile,
            queue,
            mapped_features,
            feature_names,
            monitor: SpcMonitor::new(),
        }
    }

    // create a python function that will take a python dictionary of string keys and either int, float or string values
    // and append the values to the corresponding feature queue
    pub fn insert(
        &mut self,
        py: Python,
        feature_values: HashMap<String, Py<PyAny>>,
    ) -> Result<(), FeatureQueueError> {
        for (feature, value) in feature_values {
            if let Some(queue) = self.queue.get_mut(&feature) {
                // map floats
                if let Ok(val) = value.bind(py).extract::<f64>() {
                    queue.push(val);

                // map ints
                } else if let Ok(val) = value.bind(py).extract::<i64>() {
                    queue.push(val as f64);

                // map strings to feature map
                } else if let Ok(val) = value.bind(py).extract::<String>() {
                    // map to feature map
                    if self.mapped_features.contains(&feature) {
                        let feature_map = self
                            .drift_profile
                            .config
                            .feature_map
                            .as_ref()
                            .ok_or(FeatureQueueError::MissingFeatureMapError)?
                            .features
                            .get(&feature)
                            .ok_or(FeatureQueueError::GetFeatureError)?;

                        let transformed_val = feature_map
                            .get(&val)
                            .unwrap_or(feature_map.get("missing").unwrap());

                        queue.push(*transformed_val as f64);
                    }
                }
            }
        }
        Ok(())
    }

    // Create drift records from queue items
    //
    // returns: DriftServerRecords
    fn create_drift_records(&self) -> Result<ServerRecords, FeatureQueueError> {
        // concatenate all the feature queues into a single ndarray
        let mut arrays: Vec<Array2<f64>> = Vec::new();
        let mut feature_names: Vec<String> = Vec::new();

        self.queue.iter().for_each(|(feature, values)| {
            arrays.push(Array2::from_shape_vec((values.len(), 1), values.clone()).unwrap());
            feature_names.push(feature.clone());
        });

        let n = arrays[0].dim().0;
        for array in &arrays {
            if array.dim().0 != n {
                return Err(FeatureQueueError::DriftRecordError(
                    "Shape mismatch".to_string(),
                ));
            }
        }

        let concatenated = ndarray::concatenate(
            Axis(1),
            &arrays.iter().map(|a| a.view()).collect::<Vec<_>>(),
        )
        .map_err(|e| {
            FeatureQueueError::DriftRecordError(format!("Failed to concatenate arrays: {:?}", e))
        })?;

        let records = self
            .monitor
            .sample_data(&feature_names, &concatenated.view(), &self.drift_profile)
            .map_err(|e| {
                FeatureQueueError::DriftRecordError(format!(
                    "Failed to create drift record: {:?}",
                    e
                ))
            })?;
        Ok(records)
    }

    // Clear all queues
    fn clear_queue(&mut self) {
        self.queue.iter_mut().for_each(|(_, queue)| {
            queue.clear();
        });
    }
}

#[cfg(test)]
mod tests {

    use crate::core::drift::spc::types::{SpcAlertConfig, SpcDriftConfig};

    use super::*;
    use ndarray::Array;
    use ndarray_rand::rand_distr::Uniform;
    use ndarray_rand::RandomExt;

    #[test]
    fn test_feature_queue_new() {
        let array = Array::random((1030, 3), Uniform::new(0., 10.));

        let features = vec![
            "feature_1".to_string(),
            "feature_2".to_string(),
            "feature_3".to_string(),
        ];

        let monitor = SpcMonitor::new();
        let alert_config = SpcAlertConfig::default();
        let config = SpcDriftConfig::new(
            Some("name".to_string()),
            Some("repo".to_string()),
            None,
            None,
            None,
            None,
            None,
            Some(alert_config),
            None,
        );

        let profile = monitor
            .create_2d_drift_profile(&features, &array.view(), &config.unwrap())
            .unwrap();
        assert_eq!(profile.features.len(), 3);

        let mut feature_queue = SpcFeatureQueue::new(profile);

        assert_eq!(feature_queue.queue.len(), 3);

        pyo3::prepare_freethreaded_python();

        // test insert
        let mut feature_values: HashMap<String, Py<PyAny>> = HashMap::new();

        Python::with_gil(|py| {
            for _ in 0..9 {
                feature_values.insert("feature_1".to_string(), 1.into_py(py));
                feature_values.insert("feature_2".to_string(), 2.into_py(py));
                feature_values.insert("feature_3".to_string(), 3.into_py(py));

                feature_queue.insert(py, feature_values.clone()).unwrap();
            }

            feature_queue.insert(py, feature_values).unwrap();

            assert_eq!(feature_queue.queue.get("feature_1").unwrap().len(), 10);
            assert_eq!(feature_queue.queue.get("feature_2").unwrap().len(), 10);
            assert_eq!(feature_queue.queue.get("feature_3").unwrap().len(), 10);

            let records = feature_queue.create_drift_records().unwrap();

            assert_eq!(records.records.len(), 3);

            feature_queue.clear_queue();

            assert_eq!(feature_queue.queue.get("feature_1").unwrap().len(), 0);

            // serialize records
            let json_records = records.model_dump_json();
            assert!(!json_records.is_empty());

            // deserialize records
            let records: ServerRecords = serde_json::from_str(&json_records).unwrap();
            assert_eq!(records.records.len(), 3);

            // convert to bytes and back
            let bytes = json_records.as_bytes();

            let records = ServerRecords::load_from_bytes(bytes).unwrap();
            assert_eq!(records.records.len(), 3);
        });
    }
}
