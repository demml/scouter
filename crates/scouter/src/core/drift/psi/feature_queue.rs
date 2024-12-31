use crate::core::drift::base::{Feature, RecordType, ServerRecord, ServerRecords};

use crate::core::drift::psi::monitor::PsiMonitor;
use crate::core::drift::psi::types::{Bin, PsiDriftProfile, PsiServerRecord};
use crate::core::error::FeatureQueueError;
use core::result::Result::Ok;
use pyo3::prelude::*;
use std::collections::HashMap;

#[pyclass]
pub struct PsiFeatureQueue {
    pub drift_profile: PsiDriftProfile,
    pub queue: HashMap<String, HashMap<String, usize>>,
    pub monitor: PsiMonitor,
}

impl PsiFeatureQueue {
    fn feature_is_numeric(bins: &[Bin]) -> bool {
        bins.iter()
            .any(|q| q.lower_limit.is_some() && q.upper_limit.is_some())
    }

    fn feature_is_binary(bins: &[Bin]) -> bool {
        let no_thresholds = bins
            .iter()
            .any(|bin| bin.lower_limit.is_none() && bin.upper_limit.is_none());
        let binary_bin_ids = bins.iter().all(|bin| bin.id == "0" || bin.id == "1");
        no_thresholds && bins.len() == 2 && binary_bin_ids
    }

    fn feature_is_categorical(bins: &[Bin]) -> bool {
        let no_thresholds = bins
            .iter()
            .any(|bin| bin.lower_limit.is_none() && bin.upper_limit.is_none());
        let all_non_numeric_ids = bins.iter().all(|bin| bin.id.parse::<f64>().is_err());
        no_thresholds && all_non_numeric_ids
    }

    fn find_numeric_bin_given_scaler(value: f64, bins: &[Bin]) -> &String {
        bins.iter()
            .find(|bin| value > bin.lower_limit.unwrap() && value <= bin.upper_limit.unwrap())
            .map(|bin| &bin.id)
            .expect("-inf and +inf occupy the first and last threshold so a bin should always be returned.")
    }

    fn process_numeric_queue(
        queue: &mut HashMap<String, usize>,
        value: f64,
        bins: &[Bin],
    ) -> Result<(), FeatureQueueError> {
        let bin_id = Self::find_numeric_bin_given_scaler(value, bins);
        let count = queue
            .get_mut(bin_id)
            .ok_or(FeatureQueueError::GetBinError)?;
        *count += 1;
        Ok(())
    }

    fn process_binary_queue(
        feature: &str,
        queue: &mut HashMap<String, usize>,
        value: f64,
    ) -> Result<(), FeatureQueueError> {
        if value == 0.0 {
            let bin_id = "0".to_string();
            let count = queue
                .get_mut(&bin_id)
                .ok_or(FeatureQueueError::GetBinError)?;
            *count += 1;
        } else if value == 1.0 {
            let bin_id = "1".to_string();
            let count = queue
                .get_mut(&bin_id)
                .ok_or(FeatureQueueError::GetBinError)?;
            *count += 1;
        } else {
            return Err(FeatureQueueError::InvalidValueError(
                feature.to_string(),
                value,
            ));
        }
        Ok(())
    }

    fn process_categorical_queue(
        queue: &mut HashMap<String, usize>,
        value: &str,
    ) -> Result<(), FeatureQueueError> {
        let count = queue.get_mut(value).ok_or(FeatureQueueError::GetBinError)?;
        *count += 1;
        Ok(())
    }
}

#[pymethods]
impl PsiFeatureQueue {
    #[new]
    pub fn new(drift_profile: PsiDriftProfile) -> Self {
        let queue: HashMap<String, HashMap<String, usize>> = drift_profile
            .features
            .iter()
            .map(|(feature_name, feature_drift_profile)| {
                let inner_map: HashMap<String, usize> = feature_drift_profile
                    .bins
                    .iter()
                    .map(|bin| (bin.id.clone(), 0))
                    .collect();
                (feature_name.clone(), inner_map)
            })
            .collect();

        PsiFeatureQueue {
            drift_profile,
            queue,
            monitor: PsiMonitor::new(),
        }
    }

    pub fn insert(&mut self, features: Vec<Feature>) -> PyResult<()> {
        for feature in features {
            if let Some(feature_drift_profile) = self.drift_profile.features.get(feature.name()) {
                let bins = &feature_drift_profile.bins;
                if let Some(queue) = self.queue.get_mut(feature.name()) {
                    if Self::feature_is_numeric(bins) {
                        let value = feature.to_float(None, &None)?;

                        // check if some, if not return error
                        if let Some(value) = value {
                            Self::process_numeric_queue(queue, value, bins)?;
                        }
                    } else if Self::feature_is_binary(bins) {
                        let value = feature.to_float(None, &None)?;

                        if let Some(value) = value {
                            Self::process_binary_queue(feature.name(), queue, value)?
                        };
                    } else if Self::feature_is_categorical(bins) {
                        let value = feature.to_string();

                        Self::process_categorical_queue(queue, &value)?;
                    }
                }
            }
        }
        Ok(())
    }

    pub fn create_drift_records(&self) -> Result<ServerRecords, FeatureQueueError> {
        let records = self
            .queue
            .iter()
            .flat_map(|(feature_name, bin_map)| {
                bin_map
                    .iter()
                    .map(move |(bin_id, count)| ServerRecord::Psi {
                        record: PsiServerRecord::new(
                            self.drift_profile.config.repository.clone(),
                            self.drift_profile.config.name.clone(),
                            self.drift_profile.config.version.clone(),
                            feature_name.clone(),
                            bin_id.clone(),
                            *count,
                        ),
                    })
            })
            .collect::<Vec<ServerRecord>>();

        Ok(ServerRecords::new(records, RecordType::Psi))
    }

    pub fn is_empty(&self) -> bool {
        !self
            .queue
            .values()
            .any(|bin_map| bin_map.values().any(|count| *count > 0))
    }

    fn clear_queue(&mut self) {
        self.queue.values_mut().for_each(|bin_map| {
            bin_map.values_mut().for_each(|count| *count = 0);
        });
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::core::drift::psi::types::PsiDriftConfig;
    use crate::core::utils::CategoricalFeatureHelpers;
    use ndarray::{Array, Axis};
    use ndarray_rand::rand_distr::Uniform;
    use ndarray_rand::RandomExt;
    use rand::distributions::Bernoulli;

    #[test]
    fn test_feature_queue_insert_numeric() {
        let min = 1.0;
        let max = 87.0;
        let mut array = Array::random((1030, 3), Uniform::new(min, max));

        // Ensure that each column has at least one `1.0` and one `87.0`
        for col in 0..3 {
            array[[0, col]] = min;
            array[[1, col]] = max;
        }

        let features = vec![
            "feature_1".to_string(),
            "feature_2".to_string(),
            "feature_3".to_string(),
        ];

        let monitor = PsiMonitor::new();
        let config = PsiDriftConfig::new(
            Some("name".to_string()),
            Some("repo".to_string()),
            None,
            None,
            None,
            None,
            None,
        );

        let profile = monitor
            .create_2d_drift_profile(&features, &array.view(), &config.unwrap())
            .unwrap();
        assert_eq!(profile.features.len(), 3);

        let mut feature_queue = PsiFeatureQueue::new(profile);

        assert_eq!(feature_queue.queue.len(), 3);

        for _ in 0..9 {
            let one = Feature::int("feature_1".to_string(), 1);
            let two = Feature::int("feature_2".to_string(), 2);
            let three = Feature::int("feature_3".to_string(), 3);

            feature_queue.insert(vec![one, two, three]).unwrap();
        }

        assert_eq!(
            *feature_queue
                .queue
                .get("feature_1")
                .unwrap()
                .get("decile_1")
                .unwrap(),
            9
        );
        assert_eq!(
            *feature_queue
                .queue
                .get("feature_2")
                .unwrap()
                .get("decile_1")
                .unwrap(),
            9
        );
        assert_eq!(
            *feature_queue
                .queue
                .get("feature_3")
                .unwrap()
                .get("decile_10")
                .unwrap(),
            9
        );
    }

    #[test]
    fn test_feature_queue_insert_binary() {
        let binary_column =
            Array::random((100, 1), Bernoulli::new(0.5).unwrap())
                .mapv(|x| if x { 1.0 } else { 0.0 });
        let uniform_column = Array::random((100, 1), Uniform::new(0.0, 20.0));
        let array = ndarray::concatenate![Axis(1), binary_column, uniform_column];

        let features = vec!["feature_1".to_string(), "feature_2".to_string()];

        let monitor = PsiMonitor::new();
        let config = PsiDriftConfig::new(
            Some("name".to_string()),
            Some("repo".to_string()),
            None,
            None,
            None,
            None,
            None,
        );

        let profile = monitor
            .create_2d_drift_profile(&features, &array.view(), &config.unwrap())
            .unwrap();
        assert_eq!(profile.features.len(), 2);

        let mut feature_queue = PsiFeatureQueue::new(profile);

        assert_eq!(feature_queue.queue.len(), 2);

        for _ in 0..9 {
            let one = Feature::float("feature_1".to_string(), 0.0);
            let two = Feature::float("feature_2".to_string(), 1.0);

            feature_queue.insert(vec![one, two]).unwrap();
        }

        assert_eq!(
            *feature_queue
                .queue
                .get("feature_1")
                .unwrap()
                .get("0")
                .unwrap(),
            9
        );
        assert_eq!(
            *feature_queue
                .queue
                .get("feature_2")
                .unwrap()
                .get("decile_1")
                .unwrap(),
            9
        );
    }

    #[test]
    fn test_feature_queue_insert_categorical() {
        let psi_monitor = PsiMonitor::default();
        let string_vec = vec![
            vec![
                "a".to_string(),
                "b".to_string(),
                "c".to_string(),
                "d".to_string(),
                "e".to_string(),
            ],
            vec![
                "a".to_string(),
                "a".to_string(),
                "a".to_string(),
                "b".to_string(),
                "b".to_string(),
            ],
        ];

        let string_features = vec!["feature_1".to_string(), "feature_2".to_string()];

        let feature_map = psi_monitor
            .create_feature_map(&string_features, &string_vec)
            .unwrap();

        assert_eq!(feature_map.features.len(), 2);

        let array = psi_monitor
            .convert_strings_to_ndarray_f64(&string_features, &string_vec, &feature_map)
            .unwrap();

        assert_eq!(array.shape(), &[5, 2]);

        let config = PsiDriftConfig::new(
            Some("name".to_string()),
            Some("repo".to_string()),
            None,
            Some(feature_map),
            None,
            None,
            None,
        );

        let profile = psi_monitor
            .create_2d_drift_profile(&string_features, &array.view(), &config.unwrap())
            .unwrap();
        assert_eq!(profile.features.len(), 2);

        let mut feature_queue = PsiFeatureQueue::new(profile);

        assert_eq!(feature_queue.queue.len(), 2);

        for _ in 0..9 {
            let one = Feature::string("feature_1".to_string(), "c".to_string());
            let two = Feature::string("feature_2".to_string(), "a".to_string());

            feature_queue.insert(vec![one, two]).unwrap();
        }

        assert_eq!(
            *feature_queue
                .queue
                .get("feature_1")
                .unwrap()
                .get("c")
                .unwrap(),
            9
        );
        assert_eq!(
            *feature_queue
                .queue
                .get("feature_2")
                .unwrap()
                .get("a")
                .unwrap(),
            9
        );
    }

    #[test]
    fn test_feature_queue_is_empty() {
        let psi_monitor = PsiMonitor::default();
        let string_vec = vec![
            vec![
                "a".to_string(),
                "b".to_string(),
                "c".to_string(),
                "d".to_string(),
                "e".to_string(),
            ],
            vec![
                "a".to_string(),
                "a".to_string(),
                "a".to_string(),
                "b".to_string(),
                "b".to_string(),
            ],
        ];

        let string_features = vec!["feature_1".to_string(), "feature_2".to_string()];

        let feature_map = psi_monitor
            .create_feature_map(&string_features, &string_vec)
            .unwrap();

        assert_eq!(feature_map.features.len(), 2);

        let array = psi_monitor
            .convert_strings_to_ndarray_f64(&string_features, &string_vec, &feature_map)
            .unwrap();

        assert_eq!(array.shape(), &[5, 2]);

        let config = PsiDriftConfig::new(
            Some("name".to_string()),
            Some("repo".to_string()),
            None,
            Some(feature_map),
            None,
            None,
            None,
        );

        let profile = psi_monitor
            .create_2d_drift_profile(&string_features, &array.view(), &config.unwrap())
            .unwrap();
        assert_eq!(profile.features.len(), 2);

        let mut feature_queue = PsiFeatureQueue::new(profile);

        assert_eq!(feature_queue.queue.len(), 2);

        let is_empty = feature_queue.is_empty();
        assert_eq!(is_empty as u8, 1);

        for _ in 0..9 {
            let one = Feature::string("feature_1".to_string(), "c".to_string());
            let two = Feature::string("feature_2".to_string(), "a".to_string());

            feature_queue.insert(vec![one, two]).unwrap();
        }

        let is_empty = feature_queue.queue.is_empty();
        assert_eq!(is_empty as u8, 0);
    }

    #[test]
    fn test_feature_queue_create_drift_records() {
        let array = Array::random((1030, 3), Uniform::new(1.0, 100.0));
        let features = vec![
            "feature_1".to_string(),
            "feature_2".to_string(),
            "feature_3".to_string(),
        ];

        let monitor = PsiMonitor::new();
        let config = PsiDriftConfig::new(
            Some("name".to_string()),
            Some("repo".to_string()),
            None,
            None,
            None,
            None,
            None,
        );

        let profile = monitor
            .create_2d_drift_profile(&features, &array.view(), &config.unwrap())
            .unwrap();
        assert_eq!(profile.features.len(), 3);

        let mut feature_queue = PsiFeatureQueue::new(profile);

        assert_eq!(feature_queue.queue.len(), 3);

        Python::with_gil(|_| {
            for _ in 0..9 {
                let one = Feature::float("feature_1".to_string(), 1.0);
                let two = Feature::float("feature_2".to_string(), 10.0);
                let three = Feature::float("feature_3".to_string(), 10000.0);

                feature_queue.insert(vec![one, two, three]).unwrap();
            }
        });

        let drift_records = feature_queue.create_drift_records().unwrap();

        // We have 3 features, the 3 features are numeric in nature and thus should have 10 bins assigned per due to our current decile approach.
        // Each record contains information for a given feature bin pair and this we should see a vec of len 30
        assert_eq!(drift_records.records.len(), 30);
    }
}
