use crate::spc::monitor::SpcMonitor;
use core::result::Result::Ok;
use ndarray::prelude::*;
use ndarray::Array2;
use pyo3::prelude::*;
use scouter_error::FeatureQueueError;
use scouter_types::spc::SpcDriftProfile;
use scouter_types::{Features, ServerRecords};
use std::collections::HashMap;
use tracing::instrument;
use tracing::{debug, error};

#[pyclass]
pub struct SpcFeatureQueue {
    pub drift_profile: SpcDriftProfile,
    pub queue: HashMap<String, Vec<f64>>,
    pub monitor: SpcMonitor,
    pub feature_names: Vec<String>,
}

#[pymethods]
impl SpcFeatureQueue {
    #[new]
    #[instrument(skip(drift_profile))]
    pub fn new(drift_profile: SpcDriftProfile) -> Self {
        let queue: HashMap<String, Vec<f64>> = drift_profile
            .config
            .alert_config
            .features_to_monitor
            .iter()
            .map(|feature| (feature.clone(), Vec::new()))
            .collect();

        let feature_names = queue.keys().cloned().collect();

        SpcFeatureQueue {
            drift_profile,
            queue,
            monitor: SpcMonitor::new(),
            feature_names,
        }
    }

    #[instrument(skip(self, features), name = "Insert")]
    pub fn insert(&mut self, features: Features) -> Result<(), FeatureQueueError> {
        let feat_map = &self.drift_profile.config.feature_map;

        debug!("Inserting features into queue");
        features.iter().for_each(|feature| {
            let name = feature.name().to_string();

            if self.feature_names.contains(&name) {
                if let Some(queue) = self.queue.get_mut(&name) {
                    if let Ok(value) = feature.to_float(feat_map) {
                        queue.push(value);
                    }
                }
            }
        });

        Ok(())
    }

    // Create drift records from queue items
    //
    // returns: DriftServerRecords
    #[instrument(skip(self), name = "Create Server Records")]
    pub fn create_drift_records(&self) -> Result<ServerRecords, FeatureQueueError> {
        // filter out empty queues
        let (arrays, feature_names): (Vec<_>, Vec<_>) = self
            .queue
            .iter()
            .filter(|(_, values)| !values.is_empty())
            .map(|(feature, values)| {
                (
                    Array2::from_shape_vec((values.len(), 1), values.clone()).unwrap(),
                    feature.clone(),
                )
            })
            .unzip();

        let n = arrays[0].dim().0;
        if arrays.iter().any(|array| array.dim().0 != n) {
            error!("Shape mismatch");
            return Err(FeatureQueueError::DriftRecordError(
                "Shape mismatch".to_string(),
            ));
        }

        let concatenated = ndarray::concatenate(
            Axis(1),
            &arrays.iter().map(|a| a.view()).collect::<Vec<_>>(),
        )
        .map_err(|e| {
            error!("Failed to concatenate arrays: {:?}", e);
            FeatureQueueError::DriftRecordError(format!("Failed to concatenate arrays: {:?}", e))
        })?;

        let records = self
            .monitor
            .sample_data(&feature_names, &concatenated.view(), &self.drift_profile)
            .map_err(|e| {
                error!("Failed to create drift record: {:?}", e);
                FeatureQueueError::DriftRecordError(format!(
                    "Failed to create drift record: {:?}",
                    e
                ))
            })?;

        Ok(records)
    }

    // Clear all queues
    pub fn clear_queue(&mut self) {
        self.queue.iter_mut().for_each(|(_, queue)| {
            queue.clear();
        });
    }
}

#[cfg(test)]
mod tests {

    use scouter_types::spc::{SpcAlertConfig, SpcDriftConfig};
    use scouter_types::Feature;

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
            Some(vec![
                "feature_1".to_string(),
                "feature_2".to_string(),
                "feature_3".to_string(),
            ]),
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

        for _ in 0..9 {
            let one = Feature::int("feature_1".to_string(), 1);
            let two = Feature::int("feature_2".to_string(), 2);
            let three = Feature::int("feature_3".to_string(), 3);

            let features = Features {
                features: vec![one, two, three],
            };

            feature_queue.insert(features).unwrap();
        }

        assert_eq!(feature_queue.queue.get("feature_1").unwrap().len(), 9);
        assert_eq!(feature_queue.queue.get("feature_2").unwrap().len(), 9);
        assert_eq!(feature_queue.queue.get("feature_3").unwrap().len(), 9);

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
    }
}
