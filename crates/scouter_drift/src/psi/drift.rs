#[cfg(feature = "sql")]
pub mod psi_drifter {

    use crate::error::DriftError;
    use crate::psi::monitor::PsiMonitor;
    use crate::psi::types::{FeatureBinMapping, FeatureBinProportionPairs};
    use chrono::{DateTime, Utc};
    use rayon::iter::{IntoParallelIterator, ParallelIterator};
    use scouter_dispatch::AlertDispatcher;
    use scouter_settings::ObjectStorageSettings;
    use scouter_sql::sql::traits::PsiSqlLogic;
    use scouter_sql::{sql::cache::entity_cache, PostgresClient};
    use scouter_types::psi::{
        BinnedPsiFeatureMetrics, BinnedPsiMetric, FeatureDistributions, PsiDriftProfile,
        PsiFeatureAlert, PsiFeatureAlerts, PsiFeatureDriftProfile,
    };
    use scouter_types::{contracts::DriftRequest, ProfileBaseArgs};

    use sqlx::{Pool, Postgres};
    use std::collections::{BTreeMap, HashMap};
    use tracing::info;
    use tracing::{debug, error, instrument};

    pub struct PsiDrifter {
        profile: PsiDriftProfile,
    }

    impl PsiDrifter {
        pub fn new(profile: PsiDriftProfile) -> Self {
            Self { profile }
        }

        fn get_monitored_profiles(&self) -> Vec<PsiFeatureDriftProfile> {
            self.profile
                .config
                .alert_config
                .features_to_monitor
                .iter()
                .map(|key| self.profile.features[key].clone())
                .collect()
        }

        async fn resolve_target_feature_distributions(
            &self,
            limit_datetime: &DateTime<Utc>,
            db_pool: &Pool<Postgres>,
        ) -> Result<Option<FeatureDistributions>, DriftError> {
            let entity_id = entity_cache()
                .get_entity_id_from_uid(db_pool, &self.profile.config.uid)
                .await?;
            let feature_distributions = PostgresClient::get_feature_distributions(
                db_pool,
                limit_datetime,
                &self.profile.config.alert_config.features_to_monitor,
                &entity_id,
            )
            .await
            .inspect_err(|e| {
                error!(
                    "Error: Unable to fetch feature bin proportions from DB for {}/{}/{}: {}",
                    self.profile.space(),
                    self.profile.name(),
                    self.profile.version(),
                    e
                );
            })?;

            if feature_distributions.is_empty() {
                info!(
                    "No enough target samples collected for {}/{}/{}. Skipping alert processing.",
                    self.profile.space(),
                    self.profile.name(),
                    self.profile.version(),
                );
                return Ok(None);
            }

            Ok(Some(feature_distributions))
        }

        fn get_feature_alerts(
            &self,
            drift_map: &HashMap<String, f64>,
            target_feature_distributions: &FeatureDistributions,
        ) -> Vec<PsiFeatureAlert> {
            let threshold_cfg = &self.profile.config.alert_config.threshold;

            drift_map
                .iter()
                .filter_map(|(feature, drift)| {
                    let target_sample_size = target_feature_distributions
                        .distributions
                        .get(feature)?
                        .sample_size;
                    let number_of_bins = self.profile.features.get(feature)?.bins.len();
                    let threshold =
                        threshold_cfg.compute_threshold(target_sample_size, number_of_bins as u64);

                    (*drift > threshold).then(|| PsiFeatureAlert {
                        feature: feature.clone(),
                        drift: *drift,
                        threshold,
                    })
                })
                .collect()
        }

        async fn generate_alerts(
            &self,
            drift_map: &HashMap<String, f64>,
            target_feature_distributions: &FeatureDistributions,
        ) -> Result<Option<Vec<PsiFeatureAlert>>, DriftError> {
            let alert_dispatcher = AlertDispatcher::new(&self.profile.config).inspect_err(|e| {
                error!(
                    "Error creating alert dispatcher for {}/{}/{}: {}",
                    self.profile.space(),
                    self.profile.name(),
                    self.profile.version(),
                    e
                );
            })?;

            let alerts = self.get_feature_alerts(drift_map, target_feature_distributions);

            if alerts.is_empty() {
                info!(
                    "No alerts to process for {}/{}/{}",
                    self.profile.space(),
                    self.profile.name(),
                    self.profile.version(),
                );
                return Ok(None);
            }

            alert_dispatcher
                .process_alerts(&PsiFeatureAlerts {
                    alerts: alerts.clone(),
                })
                .await
                .inspect_err(|e| {
                    error!(
                        "Error processing alerts for {}/{}/{}: {}",
                        self.profile.space(),
                        self.profile.name(),
                        self.profile.version(),
                        e
                    );
                })?;
            Ok(Some(alerts))
        }

        /// organize alerts so that each alert is mapped to a single entry and feature
        /// Some features may produce multiple alerts
        ///
        /// # Arguments
        ///
        /// * `alerts` - TaskAlerts to organize
        ///
        /// # Returns
        ///
        fn organize_alerts(alerts: &[PsiFeatureAlert]) -> Vec<BTreeMap<String, String>> {
            alerts
                .iter()
                .map(|alert| {
                    let mut alert_map = BTreeMap::new();
                    alert_map.insert("entity_name".to_string(), alert.feature.clone());
                    alert_map.insert("psi".to_string(), alert.drift.to_string());
                    alert_map.insert("threshold".to_string(), alert.threshold.to_string());
                    alert_map
                })
                .collect()
        }

        fn get_drift_map(
            target_feature_distributions: &FeatureDistributions,
            profiles_to_monitor: &[PsiFeatureDriftProfile],
        ) -> Result<HashMap<String, f64>, DriftError> {
            let feature_bin_proportion_pairs = FeatureBinMapping::from_observed_bin_proportions(
                target_feature_distributions,
                profiles_to_monitor,
            )?;

            Ok(feature_bin_proportion_pairs
                .features
                .iter()
                .map(|(feature, pairs)| (feature.clone(), PsiMonitor::compute_psi(&pairs.pairs)))
                .collect())
        }

        pub async fn check_for_alerts(
            &self,
            db_pool: &Pool<Postgres>,
            previous_run: &DateTime<Utc>,
        ) -> Result<Option<Vec<BTreeMap<String, String>>>, DriftError> {
            // Check if there are any feature profiles to monitor
            let profiles_to_monitor = self.get_monitored_profiles();

            if profiles_to_monitor.is_empty() {
                return Ok(None);
            }

            // Fetch, if any, target feature distributions
            let Some(target_feature_distributions) = self
                .resolve_target_feature_distributions(previous_run, db_pool)
                .await?
            else {
                return Ok(None);
            };

            // Compute drift for each feature
            let drift_map =
                Self::get_drift_map(&target_feature_distributions, &profiles_to_monitor)?;

            // Generate alerts, if any
            let Some(alerts) = self
                .generate_alerts(&drift_map, &target_feature_distributions)
                .await
                .inspect_err(|e| {
                    error!(
                        "Error generating alerts for {}/{}/{}: {}",
                        self.profile.space(),
                        self.profile.name(),
                        self.profile.version(),
                        e
                    );
                })?
            else {
                return Ok(None);
            };

            Ok(Some(Self::organize_alerts(&alerts)))
        }

        fn create_feature_bin_proportion_pairs(
            &self,
            feature: &str,
            bin_proportions: &BTreeMap<i32, f64>,
        ) -> Result<FeatureBinProportionPairs, DriftError> {
            // get profile
            let profile = match self.profile.features.get(feature) {
                Some(profile) => profile,
                None => {
                    error!("Error: Unable to fetch profile for feature {}", feature);
                    return Err(DriftError::ProcessAlertError);
                }
            };

            let proportion_pairs =
                FeatureBinProportionPairs::from_observed_bin_proportions(bin_proportions, profile)
                    .unwrap();

            Ok(proportion_pairs)
        }

        /// Get a binned drift map. This is used on the server for vizualization purposes
        ///
        /// # Arguments
        ///
        /// * `drift_request` - DriftRequest containing the profile to monitor
        /// * `db_client` - PostgresClient to use for fetching data
        ///
        #[instrument(skip_all)]
        pub async fn get_binned_drift_map(
            &self,
            drift_request: &DriftRequest,
            db_pool: &Pool<Postgres>,
            retention_period: &i32,
            storage_settings: &ObjectStorageSettings,
            entity_id: &i32,
        ) -> Result<BinnedPsiFeatureMetrics, DriftError> {
            debug!(
                "Getting binned drift map for {}/{}/{}",
                self.profile.space(),
                self.profile.name(),
                self.profile.version(),
            );
            let binned_records = PostgresClient::get_binned_psi_drift_records(
                db_pool,
                drift_request,
                retention_period,
                storage_settings,
                entity_id,
            )
            .await?;

            if binned_records.is_empty() {
                info!(
                    "No binned drift records available for {}/{}/{}",
                    self.profile.space(),
                    self.profile.name(),
                    self.profile.version(),
                );
                return Ok(BinnedPsiFeatureMetrics::default());
            }

            // iterate over each feature and calculate psi for each time period
            let binned_map = binned_records
                .into_par_iter()
                // filter out any records that are not in the profile
                .filter(|record| self.profile.features.contains_key(&record.feature))
                // get psi for binned features and create data structure that's usable for UI
                .map(|record| -> Result<_, DriftError> {
                    let psi_vec: Result<Vec<_>, DriftError> = record
                        .bin_proportions
                        .iter()
                        .map(|bin_proportion| {
                            let proportions = self.create_feature_bin_proportion_pairs(
                                &record.feature,
                                bin_proportion,
                            )?;
                            let psi = PsiMonitor::compute_psi(&proportions.pairs);
                            Ok(psi)
                        })
                        .collect();

                    let overall_proportions = self.create_feature_bin_proportion_pairs(
                        &record.feature,
                        &record.overall_proportions,
                    )?;
                    let overall_psi = PsiMonitor::compute_psi(&overall_proportions.pairs);

                    Ok((
                        record.feature.clone(),
                        BinnedPsiMetric {
                            created_at: record.created_at,
                            psi: psi_vec?,
                            overall_psi,
                            bins: record.overall_proportions,
                        },
                    ))

                    // calculate overall psi
                })
                .collect::<Result<BTreeMap<String, BinnedPsiMetric>, DriftError>>()?;

            Ok(BinnedPsiFeatureMetrics {
                features: binned_map,
            })
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use ndarray::Array;
        use ndarray_rand::rand_distr::Uniform;
        use ndarray_rand::RandomExt;
        use scouter_types::psi::{Bin, BinType, PsiNormalThreshold, PsiThreshold};
        use scouter_types::psi::{
            DistributionData, FeatureDistributions, PsiAlertConfig, PsiDriftConfig,
        };

        fn get_test_drifter(threshold: PsiThreshold) -> PsiDrifter {
            let alert_config = PsiAlertConfig {
                features_to_monitor: vec!["feature_1".to_string(), "feature_3".to_string()],
                threshold,
                ..Default::default()
            };
            let config = PsiDriftConfig {
                space: "name".to_string(),
                name: "repo".to_string(),
                alert_config,
                ..Default::default()
            };

            let array = Array::random((1030, 3), Uniform::new(1.0, 100.0).unwrap());

            let features = vec![
                "feature_1".to_string(),
                "feature_2".to_string(),
                "feature_3".to_string(),
            ];

            let monitor = PsiMonitor::new();

            let profile = monitor
                .create_2d_drift_profile(&features, &array.view(), &config)
                .unwrap();

            PsiDrifter::new(profile)
        }

        #[test]
        fn test_get_drift_map_only_maps_matching_features() {
            // Arrange
            // Create target distributions
            let mut distributions = BTreeMap::new();

            let mut bins1 = BTreeMap::new();
            bins1.insert(0, 0.3);
            bins1.insert(1, 0.4);
            bins1.insert(2, 0.3);

            distributions.insert(
                "feature1".to_string(),
                DistributionData {
                    sample_size: 1000,
                    bins: bins1,
                },
            );

            let mut bins2 = BTreeMap::new();
            bins2.insert(0, 0.25);
            bins2.insert(1, 0.5);
            bins2.insert(2, 0.25);

            distributions.insert(
                "feature2".to_string(),
                DistributionData {
                    sample_size: 800,
                    bins: bins2,
                },
            );

            let target_distributions = FeatureDistributions { distributions };

            // Create profiles with one matching and one non-matching feature
            let profiles = vec![
                PsiFeatureDriftProfile {
                    id: "feature1".to_string(), // This exists in target_distributions
                    bins: vec![Bin {
                        id: 0,
                        lower_limit: None,
                        upper_limit: Some(10.0),
                        proportion: 0.35,
                    }],
                    timestamp: Utc::now(),
                    bin_type: BinType::Numeric,
                },
                PsiFeatureDriftProfile {
                    id: "feature2".to_string(), // This doesn't exist
                    bins: vec![Bin {
                        id: 0,
                        lower_limit: None,
                        upper_limit: Some(10.0),
                        proportion: 0.5,
                    }],
                    timestamp: Utc::now(),
                    bin_type: BinType::Numeric,
                },
            ];

            // Act
            let result = PsiDrifter::get_drift_map(&target_distributions, &profiles);

            // Assert
            assert!(result.is_ok());
            let drift_map = result.unwrap();

            // Only the matching feature should be in the result
            assert_eq!(drift_map.len(), 2);
            assert!(drift_map.contains_key("feature1"));
            assert!(drift_map.contains_key("feature2"));
        }

        #[test]
        fn test_get_feature_alerts_all_above() {
            let bin_count = 10;
            let sample_size = 10000;
            let threshold = PsiNormalThreshold { alpha: 0.05 };
            let result = threshold.compute_threshold(sample_size, bin_count);

            let drifter_with_normal_threshold = get_test_drifter(PsiThreshold::Normal(threshold));

            let feature_1 = "feature_1";
            let feature_2 = "feature_2";
            let feature_3 = "feature_3";

            let mut drift_map = HashMap::new();
            drift_map.insert(feature_1.to_string(), result + 0.1);
            drift_map.insert(feature_2.to_string(), result + 0.1);
            drift_map.insert(feature_3.to_string(), result + 0.1);

            let mut distributions = BTreeMap::new();
            let mut bins = BTreeMap::new();
            for i in 0..bin_count {
                bins.insert(i as usize, (sample_size / bin_count) as f64);
            }

            distributions.insert(
                feature_1.to_string(),
                DistributionData {
                    sample_size,
                    bins: bins.clone(),
                },
            );
            distributions.insert(
                feature_2.to_string(),
                DistributionData {
                    sample_size,
                    bins: bins.clone(),
                },
            );
            distributions.insert(
                feature_3.to_string(),
                DistributionData {
                    sample_size,
                    bins: bins.clone(),
                },
            );

            let target_feature_distributions = FeatureDistributions { distributions };

            let alerts = drifter_with_normal_threshold
                .get_feature_alerts(&drift_map, &target_feature_distributions);

            assert_eq!(alerts.len(), 3);
        }

        #[test]
        fn test_get_feature_alerts_all_below() {
            let bin_count = 10;
            let sample_size = 10000;
            let threshold = PsiNormalThreshold { alpha: 0.05 };
            let result = threshold.compute_threshold(sample_size, bin_count);

            let drifter_with_normal_threshold = get_test_drifter(PsiThreshold::Normal(threshold));

            let feature_1 = "feature_1";
            let feature_2 = "feature_2";
            let feature_3 = "feature_3";

            let mut drift_map = HashMap::new();
            drift_map.insert(feature_1.to_string(), result - 0.1); // Below threshold
            drift_map.insert(feature_2.to_string(), result - 0.1); // Below threshold
            drift_map.insert(feature_3.to_string(), result - 0.1); // Below threshold

            let mut distributions = BTreeMap::new();
            let mut bins = BTreeMap::new();
            for i in 0..bin_count {
                bins.insert(i as usize, (sample_size / bin_count) as f64);
            }

            distributions.insert(
                feature_1.to_string(),
                DistributionData {
                    sample_size,
                    bins: bins.clone(),
                },
            );
            distributions.insert(
                feature_2.to_string(),
                DistributionData {
                    sample_size,
                    bins: bins.clone(),
                },
            );
            distributions.insert(
                feature_3.to_string(),
                DistributionData {
                    sample_size,
                    bins: bins.clone(),
                },
            );

            let target_feature_distributions = FeatureDistributions { distributions };

            let alerts = drifter_with_normal_threshold
                .get_feature_alerts(&drift_map, &target_feature_distributions);

            assert_eq!(alerts.len(), 0); // No alerts expected
        }

        #[test]
        fn test_get_feature_alerts_mixed_above_below() {
            let bin_count = 10;
            let sample_size = 10000;
            let threshold = PsiNormalThreshold { alpha: 0.05 };
            let result = threshold.compute_threshold(sample_size, bin_count);

            let drifter_with_normal_threshold = get_test_drifter(PsiThreshold::Normal(threshold));

            let feature_1 = "feature_1";
            let feature_2 = "feature_2";
            let feature_3 = "feature_3";

            let mut drift_map = HashMap::new();
            drift_map.insert(feature_1.to_string(), result + 0.1); // Above threshold
            drift_map.insert(feature_2.to_string(), result - 0.1); // Below threshold
            drift_map.insert(feature_3.to_string(), result + 0.2); // Above threshold

            let mut distributions = BTreeMap::new();
            let mut bins = BTreeMap::new();
            for i in 0..bin_count {
                bins.insert(i as usize, (sample_size / bin_count) as f64);
            }

            distributions.insert(
                feature_1.to_string(),
                DistributionData {
                    sample_size,
                    bins: bins.clone(),
                },
            );
            distributions.insert(
                feature_2.to_string(),
                DistributionData {
                    sample_size,
                    bins: bins.clone(),
                },
            );
            distributions.insert(
                feature_3.to_string(),
                DistributionData {
                    sample_size,
                    bins: bins.clone(),
                },
            );

            let target_feature_distributions = FeatureDistributions { distributions };

            let alerts = drifter_with_normal_threshold
                .get_feature_alerts(&drift_map, &target_feature_distributions);

            assert_eq!(alerts.len(), 2); // Only feature_1 and feature_3 should alert

            // Verify correct features alerted
            let alert_features: Vec<String> = alerts.iter().map(|a| a.feature.clone()).collect();
            assert!(alert_features.contains(&feature_1.to_string()));
            assert!(alert_features.contains(&feature_3.to_string()));
            assert!(!alert_features.contains(&feature_2.to_string()));
        }

        #[test]
        fn test_get_feature_alerts_drift_exactly_at_threshold() {
            let bin_count = 10;
            let sample_size = 10000;
            let threshold = PsiNormalThreshold { alpha: 0.05 };
            let result = threshold.compute_threshold(sample_size, bin_count);

            let drifter_with_normal_threshold = get_test_drifter(PsiThreshold::Normal(threshold));

            let feature_1 = "feature_1";

            let mut drift_map = HashMap::new();
            drift_map.insert(feature_1.to_string(), result); // Exactly at threshold

            let mut distributions = BTreeMap::new();
            let mut bins = BTreeMap::new();
            for i in 0..bin_count {
                bins.insert(i as usize, (sample_size / bin_count) as f64);
            }

            distributions.insert(
                feature_1.to_string(),
                DistributionData {
                    sample_size,
                    bins: bins.clone(),
                },
            );

            let target_feature_distributions = FeatureDistributions { distributions };

            let alerts = drifter_with_normal_threshold
                .get_feature_alerts(&drift_map, &target_feature_distributions);

            assert_eq!(alerts.len(), 0); // Should be 0 since drift is not > threshold
        }

        #[test]
        fn test_get_monitored_profiles() {
            let drifter = get_test_drifter(PsiThreshold::default());

            let profiles_to_monitor = drifter.get_monitored_profiles();

            assert_eq!(profiles_to_monitor.len(), 2);

            assert!(
                profiles_to_monitor[0].id == "feature_1"
                    || profiles_to_monitor[0].id == "feature_3"
            );
            assert!(
                profiles_to_monitor[1].id == "feature_1"
                    || profiles_to_monitor[1].id == "feature_3"
            );
        }
    }
}
