use crate::error::FeatureQueueError;
use crate::queue::traits::FeatureQueue;
use core::result::Result::Ok;
use scouter_drift::psi::monitor::PsiMonitor;
use scouter_types::{
    psi::{Bin, BinType, PsiDriftProfile},
    Feature, PsiServerRecord, QueueExt, ServerRecord, ServerRecords,
};
use std::collections::HashMap;
use tracing::{debug, error, info, instrument};

pub struct PsiFeatureQueue {
    pub drift_profile: PsiDriftProfile,
    pub empty_queue: HashMap<String, HashMap<usize, usize>>,
    pub monitor: PsiMonitor,
    pub feature_names: Vec<String>,
}

impl PsiFeatureQueue {
    #[instrument(skip_all)]
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

    #[instrument(skip_all)]
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

    #[instrument(skip_all)]
    fn process_categorical_queue(
        queue: &mut HashMap<usize, usize>,
        value: &usize,
    ) -> Result<(), FeatureQueueError> {
        let count = queue
            .get_mut(value)
            .ok_or(FeatureQueueError::GetBinError)
            .inspect_err(|e| {
                error!("Error processing categorical queue: {:?}", e);
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

        let empty_queue: HashMap<String, HashMap<usize, usize>> = drift_profile
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

        let feature_names = empty_queue.keys().cloned().collect();

        PsiFeatureQueue {
            drift_profile,
            empty_queue,
            monitor: PsiMonitor::new(),
            feature_names,
        }
    }

    #[instrument(skip_all, name = "insert_psi")]
    pub fn insert(
        &self,
        features: &[Feature],
        queue: &mut HashMap<String, HashMap<usize, usize>>,
    ) -> Result<(), FeatureQueueError> {
        let apple = f64::INFINITY as usize;
        println!("{apple}");
        let feat_map = &self.drift_profile.config.feature_map;
        for feature in features.iter() {
            if let Some(feature_drift_profile) = self.drift_profile.features.get(feature.name()) {
                let name = feature.name().to_string();

                // if feature not in features_to_monitor, skip
                if !self.feature_names.contains(&name) {
                    error!(
                        "Feature {} not in features to monitor, skipping",
                        feature.name()
                    );
                    continue;
                }

                let bins = &feature_drift_profile.bins;
                let queue = queue
                    .get_mut(&name)
                    .ok_or(FeatureQueueError::GetFeatureError)?;

                match feature_drift_profile.bin_type {
                    BinType::Numeric => {
                        let value = feature.to_float(feat_map).map_err(|e| {
                            error!("Error converting feature to float: {:?}", e);
                            FeatureQueueError::InvalidValueError(
                                feature.name().to_string(),
                                e.to_string(),
                            )
                        })?;

                        if !value.is_finite() {
                            info!(
                                "Non finite value detected for {}, value will not be inserted into queue",
                                feature.name()
                            );
                            continue;
                        }

                        Self::process_numeric_queue(queue, value, bins)?
                    }
                    BinType::Category => {
                        let value = feature.to_usize(feat_map).map_err(|e| {
                            error!("Error converting feature to usize: {:?}", e);
                            FeatureQueueError::InvalidValueError(
                                feature.name().to_string(),
                                e.to_string(),
                            )
                        })?;

                        Self::process_categorical_queue(queue, &value)?
                    }
                }
            }
        }
        Ok(())
    }

    #[instrument(skip_all)]
    pub fn create_drift_records(
        &self,
        queue: HashMap<String, HashMap<usize, usize>>,
    ) -> Result<ServerRecords, FeatureQueueError> {
        // filter out any feature thats not in features_to_monitor
        // Keep feature if any value in the bin map is greater than 0

        let filtered_queue = queue
            .iter()
            .filter(|(_, bin_map)| bin_map.iter().any(|(_, count)| *count > 0))
            .collect::<HashMap<_, _>>();

        debug!("Filtered queue count: {:?}", filtered_queue.len());
        println!("made it here");

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

        Ok(ServerRecords::new(records))
    }
}

impl FeatureQueue for PsiFeatureQueue {
    fn create_drift_records_from_batch<T: QueueExt>(
        &self,
        batch: Vec<T>,
    ) -> Result<ServerRecords, FeatureQueueError> {
        // clones the empty map (so we don't need to recreate it on each call)
        let mut queue = self.empty_queue.clone();

        for elem in batch {
            self.insert(elem.features(), &mut queue)?;
        }
        self.create_drift_records(queue)
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use ndarray::{Array, Axis};
    use ndarray_rand::rand::distributions::Bernoulli;
    use ndarray_rand::rand_distr::Uniform;
    use ndarray_rand::RandomExt;
    use scouter_drift::utils::CategoricalFeatureHelpers;
    use scouter_types::psi::PsiAlertConfig;
    use scouter_types::psi::PsiDriftConfig;
    use scouter_types::EntityType;
    use scouter_types::{Features, DEFAULT_VERSION};

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
        let config = PsiDriftConfig::new(
            "name",
            "repo",
            DEFAULT_VERSION,
            alert_config,
            None,
            None,
            None,
        );

        let profile = monitor
            .create_2d_drift_profile(&features, &array.view(), &config.unwrap())
            .unwrap();
        assert_eq!(profile.features.len(), 3);

        let feature_queue = PsiFeatureQueue::new(profile);

        assert_eq!(feature_queue.empty_queue.len(), 3);

        let mut batch_features = Vec::new();
        for _ in 0..9 {
            let one = Feature::float("feature_1".to_string(), min);
            let two = Feature::float("feature_2".to_string(), min);
            let three = Feature::float("feature_3".to_string(), max);

            let features = Features {
                features: vec![one, two, three],
                entity_type: EntityType::Feature,
            };

            batch_features.push(features);
        }

        let mut queue = feature_queue.empty_queue.clone();
        for feature in batch_features {
            feature_queue.insert(&feature.features, &mut queue).unwrap();
        }

        assert_eq!(*queue.get("feature_1").unwrap().get(&1).unwrap(), 9);
        assert_eq!(*queue.get("feature_2").unwrap().get(&1).unwrap(), 9);
        assert_eq!(*queue.get("feature_3").unwrap().get(&10).unwrap(), 9);
    }

    #[test]
    fn test_feature_queue_insert_numeric_categorical() {
        let numeric_cat_column =
            Array::random((100, 1), Bernoulli::new(0.5).unwrap())
                .mapv(|x| if x { 1.0 } else { 0.0 });
        let uniform_column = Array::random((100, 1), Uniform::new(0.0, 20.0));
        let array = ndarray::concatenate![Axis(1), numeric_cat_column, uniform_column];

        let features = vec!["feature_1".to_string(), "feature_2".to_string()];

        let monitor = PsiMonitor::new();

        let drift_config = PsiDriftConfig {
            categorical_features: Some(features.clone()),
            ..Default::default()
        };

        let mut profile = monitor
            .create_2d_drift_profile(&features, &array.view(), &drift_config)
            .unwrap();
        profile.config.alert_config.features_to_monitor = features.clone();

        assert_eq!(profile.features.len(), 2);

        let feature_queue = PsiFeatureQueue::new(profile);

        assert_eq!(feature_queue.empty_queue.len(), 2);

        let mut batch_features = Vec::new();
        for _ in 0..9 {
            let one = Feature::float("feature_1".to_string(), 0.0);
            let two = Feature::float("feature_2".to_string(), 1.0);

            let features = Features {
                features: vec![one, two],
                entity_type: EntityType::Feature,
            };

            batch_features.push(features);
        }

        let mut queue = feature_queue.empty_queue.clone();
        for feature in batch_features {
            feature_queue.insert(&feature.features, &mut queue).unwrap();
        }

        assert_eq!(*queue.get("feature_1").unwrap().get(&0).unwrap(), 9);
        assert_eq!(*queue.get("feature_2").unwrap().get(&1).unwrap(), 9);
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
            categorical_features: Some(string_features.clone()),
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

        let feature_queue = PsiFeatureQueue::new(profile);

        assert_eq!(feature_queue.empty_queue.len(), 2);

        let mut batch_features = Vec::new();
        for _ in 0..9 {
            let one = Feature::string("feature_1".to_string(), "c".to_string());
            let two = Feature::string("feature_2".to_string(), "a".to_string());

            let features = Features {
                features: vec![one, two],
                entity_type: EntityType::Feature,
            };
            batch_features.push(features);
        }

        let mut queue = feature_queue.empty_queue.clone();
        for feature in batch_features {
            feature_queue.insert(&feature.features, &mut queue).unwrap();
        }

        assert_eq!(*queue.get("feature_1").unwrap().get(&2).unwrap(), 9);
        assert_eq!(*queue.get("feature_2").unwrap().get(&0).unwrap(), 9);
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

        let feature_queue = PsiFeatureQueue::new(profile);

        assert_eq!(feature_queue.empty_queue.len(), 2);

        let mut batch_features = Vec::new();
        for _ in 0..9 {
            let one = Feature::string("feature_1".to_string(), "c".to_string());
            let two = Feature::string("feature_2".to_string(), "a".to_string());

            let features = Features {
                features: vec![one, two],
                entity_type: EntityType::Feature,
            };

            batch_features.push(features);
        }

        let mut queue = feature_queue.empty_queue.clone();
        for feature in batch_features {
            feature_queue.insert(&feature.features, &mut queue).unwrap();
        }

        let is_empty = !queue
            .values()
            .any(|bin_map| bin_map.values().any(|count| *count > 0));

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

        let feature_queue = PsiFeatureQueue::new(profile);

        assert_eq!(feature_queue.empty_queue.len(), 3);

        let mut batch_features = Vec::new();
        for _ in 0..9 {
            let one = Feature::float("feature_1".to_string(), 1.0);
            let two = Feature::float("feature_2".to_string(), 10.0);
            let three = Feature::float("feature_3".to_string(), 10000.0);

            let features = Features {
                features: vec![one, two, three],
                entity_type: EntityType::Feature,
            };

            batch_features.push(features);
        }

        let drift_records = feature_queue
            .create_drift_records_from_batch(batch_features)
            .unwrap();

        // We have 3 features, the 3 features are numeric in nature and thus should have 10 bins assigned per due to our current decile approach.
        // Each record contains information for a given feature bin pair and this we should see a vec of len 30
        assert_eq!(drift_records.records.len(), 30);
    }
}
