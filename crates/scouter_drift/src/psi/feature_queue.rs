use scouter_types::{
    psi::BinType, Features, PsiServerRecord, RecordType, ServerRecord, ServerRecords,
};
use tracing::{debug, error, instrument};

use crate::psi::monitor::PsiMonitor;
use core::result::Result::Ok;
use scouter_error::FeatureQueueError;
use scouter_types::psi::{Bin, PsiDriftProfile};
use std::collections::HashMap;

pub struct PsiFeatureQueue {
    pub drift_profile: PsiDriftProfile,
    pub queue: HashMap<String, HashMap<usize, usize>>,
    pub monitor: PsiMonitor,
    pub feature_names: Vec<String>,
}

impl PsiFeatureQueue {
    #[instrument(skip(value, bins), name = "Numeric Scalar", level = "debug")]
    fn find_numeric_bin_given_scaler(
        value: f64,
        bins: &[Bin],
    ) -> Result<&usize, FeatureQueueError> {
        let bin = bins
            .iter()
            .find(|bin| value > bin.lower_limit.unwrap() && value <= bin.upper_limit.unwrap())
            .map(|bin| &bin.id);

        match bin {
            Some(bin) => Ok(bin),
            None => {
                error!("Failed to find bin for value: {}", value);
                Err(FeatureQueueError::GetBinError)
            }
        }
    }

    #[instrument(skip(queue, value, bins), name = "Numeric Queue", level = "debug")]
    fn process_numeric_queue(
        queue: &mut HashMap<usize, usize>,
        value: f64,
        bins: &[Bin],
    ) -> Result<(), FeatureQueueError> {
        let bin_id = Self::find_numeric_bin_given_scaler(value, bins)?;
        let count = queue
            .get_mut(bin_id)
            .ok_or(FeatureQueueError::GetBinError)
            .map_err(|e| {
                error!("Error processing numeric queue: {:?}", e);
                e
            })?;
        *count += 1;

        Ok(())
    }

    #[instrument(skip(feature, queue, value), name = "Binary Queue", level = "debug")]
    fn process_binary_queue(
        feature: &str,
        queue: &mut HashMap<usize, usize>,
        value: f64,
    ) -> Result<(), FeatureQueueError> {
        if value == 0.0 {
            let bin_id = 0;
            let count = queue
                .get_mut(&bin_id)
                .ok_or(FeatureQueueError::GetBinError)
                .map_err(|e| {
                    error!("Error processing binary queue: {:?}", e);
                    e
                })?;
            *count += 1;
        } else if value == 1.0 {
            let bin_id = 1;
            let count = queue
                .get_mut(&bin_id)
                .ok_or(FeatureQueueError::GetBinError)
                .map_err(|e| {
                    error!("Error processing binary queue: {:?}", e);
                    e
                })?;
            *count += 1;
        } else {
            error!("Failed to convert binary value");
            return Err(FeatureQueueError::InvalidValueError(
                feature.to_string(),
                "failed to convert binary value".to_string(),
            ));
        }
        Ok(())
    }

    #[instrument(skip(queue, value), name = "Process Categorical", level = "debug")]
    fn process_categorical_queue(
        queue: &mut HashMap<usize, usize>,
        value: &usize,
    ) -> Result<(), FeatureQueueError> {
        let count = queue
            .get_mut(value)
            .ok_or(FeatureQueueError::GetBinError)
            .map_err(|e| {
                error!("Error processing categorical queue: {:?}", e);
                e
            })?;
        *count += 1;
        Ok(())
    }

    pub fn new(drift_profile: PsiDriftProfile) -> Self {
        let features_to_monitor = drift_profile
            .config
            .alert_config
            .features_to_monitor
            .clone();

        let queue: HashMap<String, HashMap<usize, usize>> = drift_profile
            .features
            .iter()
            .filter(|(feature_name, _)| features_to_monitor.contains(feature_name))
            .map(|(feature_name, feature_drift_profile)| {
                let inner_map: HashMap<usize, usize> = feature_drift_profile
                    .bins
                    .iter()
                    .map(|bin| (bin.id, 0))
                    .collect();
                (feature_name.clone(), inner_map)
            })
            .collect();

        let feature_names = queue.keys().cloned().collect();

        PsiFeatureQueue {
            drift_profile,
            queue,
            monitor: PsiMonitor::new(),
            feature_names,
        }
    }

    #[instrument(skip(self, features), name = "Insert", level = "debug")]
    pub fn insert(&mut self, features: Features) -> Result<(), FeatureQueueError> {
        let feat_map = &self.drift_profile.config.feature_map;
        for feature in features.iter() {
            if let Some(feature_drift_profile) = self.drift_profile.features.get(feature.name()) {
                let name = feature.name().to_string();

                // if feature not in features_to_monitor, skip
                if !self.feature_names.contains(&name) {
                    continue;
                }

                let bins = &feature_drift_profile.bins;

                let queue = self
                    .queue
                    .get_mut(&name)
                    .ok_or(FeatureQueueError::GetFeatureError)?;

                match feature_drift_profile.bin_type {
                    BinType::Numeric | BinType::Binary => {
                        let value = feature.to_float(feat_map).map_err(|e| {
                            error!("Error converting feature to float: {:?}", e);
                            FeatureQueueError::InvalidValueError(
                                feature.name().to_string(),
                                e.to_string(),
                            )
                        })?;

                        match feature_drift_profile.bin_type {
                            BinType::Numeric => Self::process_numeric_queue(queue, value, bins)?,
                            BinType::Binary => {
                                Self::process_binary_queue(feature.name(), queue, value)?
                            }
                            _ => unreachable!(),
                        }
                    }
                    BinType::Category => {
                        let value = self
                            .drift_profile
                            .config
                            .feature_map
                            .features
                            .get(&name)
                            .ok_or(FeatureQueueError::GetFeatureError)?
                            .get(&feature.to_string())
                            .ok_or(FeatureQueueError::GetFeatureError)?;
                        Self::process_categorical_queue(queue, value)?;
                    }
                }
            }
        }
        Ok(())
    }

    #[instrument(skip(self), name = "Create records", level = "debug")]
    pub fn create_drift_records(&self) -> Result<ServerRecords, FeatureQueueError> {
        // filter out any feature thats not in features_to_monitor
        // Keep feature if any value in the bin map is greater than 0

        let filtered_queue = self
            .queue
            .iter()
            .filter(|(_, bin_map)| bin_map.iter().any(|(_, count)| *count > 0))
            .collect::<HashMap<_, _>>();

        debug!("Filtered queue count: {:?}", filtered_queue.len());

        let records = filtered_queue
            .iter()
            .flat_map(|(feature_name, bin_map)| {
                bin_map.iter().map(move |(bin_id, count)| {
                    ServerRecord::Psi(PsiServerRecord::new(
                        self.drift_profile.config.space.clone(),
                        self.drift_profile.config.name.clone(),
                        self.drift_profile.config.version.clone(),
                        feature_name.to_string(),
                        *bin_id,
                        *count,
                    ))
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

    pub fn clear_queue(&mut self) {
        self.queue.values_mut().for_each(|bin_map| {
            bin_map.values_mut().for_each(|count| *count = 0);
        });
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::utils::CategoricalFeatureHelpers;
    use ndarray::{Array, Axis};
    use ndarray_rand::rand_distr::Uniform;
    use ndarray_rand::RandomExt;
    use rand::distributions::Bernoulli;
    use scouter_types::psi::PsiAlertConfig;
    use scouter_types::psi::PsiDriftConfig;
    use scouter_types::{Feature, DEFAULT_VERSION};

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

        let alert_config = PsiAlertConfig {
            features_to_monitor: features.clone(),
            ..Default::default()
        };
        let config = PsiDriftConfig::new("name", "repo", DEFAULT_VERSION, alert_config, None);

        let profile = monitor
            .create_2d_drift_profile(&features, &array.view(), &config.unwrap())
            .unwrap();
        assert_eq!(profile.features.len(), 3);

        let mut feature_queue = PsiFeatureQueue::new(profile);

        assert_eq!(feature_queue.queue.len(), 3);

        for _ in 0..9 {
            let one = Feature::float("feature_1".to_string(), min);
            let two = Feature::float("feature_2".to_string(), min);
            let three = Feature::float("feature_3".to_string(), max);

            let features = Features {
                features: vec![one, two, three],
            };

            feature_queue.insert(features).unwrap();
        }

        assert_eq!(
            *feature_queue
                .queue
                .get("feature_1")
                .unwrap()
                .get(&1)
                .unwrap(),
            9
        );
        assert_eq!(
            *feature_queue
                .queue
                .get("feature_2")
                .unwrap()
                .get(&1)
                .unwrap(),
            9
        );
        assert_eq!(
            *feature_queue
                .queue
                .get("feature_3")
                .unwrap()
                .get(&10)
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

        let mut profile = monitor
            .create_2d_drift_profile(&features, &array.view(), &PsiDriftConfig::default())
            .unwrap();
        profile.config.alert_config.features_to_monitor = features.clone();

        assert_eq!(profile.features.len(), 2);

        let mut feature_queue = PsiFeatureQueue::new(profile);

        assert_eq!(feature_queue.queue.len(), 2);

        for _ in 0..9 {
            let one = Feature::float("feature_1".to_string(), 0.0);
            let two = Feature::float("feature_2".to_string(), 1.0);

            let features = Features {
                features: vec![one, two],
            };

            feature_queue.insert(features).unwrap();
        }

        assert_eq!(
            *feature_queue
                .queue
                .get("feature_1")
                .unwrap()
                .get(&0)
                .unwrap(),
            9
        );
        assert_eq!(
            *feature_queue
                .queue
                .get("feature_2")
                .unwrap()
                .get(&1)
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

        let mut config = PsiDriftConfig {
            feature_map: feature_map.clone(),
            ..Default::default()
        };

        config.alert_config.features_to_monitor =
            vec!["feature_1".to_string(), "feature_2".to_string()];

        let array = psi_monitor
            .convert_strings_to_ndarray_f64(&string_features, &string_vec, &feature_map)
            .unwrap();

        assert_eq!(array.shape(), &[5, 2]);

        let profile = psi_monitor
            .create_2d_drift_profile(&string_features, &array.view(), &config)
            .unwrap();
        assert_eq!(profile.features.len(), 2);

        let mut feature_queue = PsiFeatureQueue::new(profile);

        assert_eq!(feature_queue.queue.len(), 2);

        for _ in 0..9 {
            let one = Feature::string("feature_1".to_string(), "c".to_string());
            let two = Feature::string("feature_2".to_string(), "a".to_string());

            let features = Features {
                features: vec![one, two],
            };

            feature_queue.insert(features).unwrap();
        }

        assert_eq!(
            *feature_queue
                .queue
                .get("feature_1")
                .unwrap()
                .get(&2)
                .unwrap(),
            9
        );
        assert_eq!(
            *feature_queue
                .queue
                .get("feature_2")
                .unwrap()
                .get(&0)
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

        let mut config = PsiDriftConfig {
            feature_map,
            ..Default::default()
        };

        config.alert_config.features_to_monitor =
            vec!["feature_1".to_string(), "feature_2".to_string()];

        let profile = psi_monitor
            .create_2d_drift_profile(&string_features, &array.view(), &config)
            .unwrap();
        assert_eq!(profile.features.len(), 2);

        let mut feature_queue = PsiFeatureQueue::new(profile);

        assert_eq!(feature_queue.queue.len(), 2);

        let is_empty = feature_queue.is_empty();
        assert_eq!(is_empty as u8, 1);

        for _ in 0..9 {
            let one = Feature::string("feature_1".to_string(), "c".to_string());
            let two = Feature::string("feature_2".to_string(), "a".to_string());

            let features = Features {
                features: vec![one, two],
            };

            feature_queue.insert(features).unwrap();
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

        let mut profile = monitor
            .create_2d_drift_profile(&features, &array.view(), &PsiDriftConfig::default())
            .unwrap();

        profile.config.alert_config.features_to_monitor = features.clone();

        assert_eq!(profile.features.len(), 3);

        let mut feature_queue = PsiFeatureQueue::new(profile);

        assert_eq!(feature_queue.queue.len(), 3);

        for _ in 0..9 {
            let one = Feature::float("feature_1".to_string(), 1.0);
            let two = Feature::float("feature_2".to_string(), 10.0);
            let three = Feature::float("feature_3".to_string(), 10000.0);

            let features = Features {
                features: vec![one, two, three],
            };

            feature_queue.insert(features).unwrap();
        }

        let drift_records = feature_queue.create_drift_records().unwrap();

        // We have 3 features, the 3 features are numeric in nature and thus should have 10 bins assigned per due to our current decile approach.
        // Each record contains information for a given feature bin pair and this we should see a vec of len 30
        assert_eq!(drift_records.records.len(), 30);
    }
}
