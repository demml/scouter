#[cfg(feature = "sql")]
pub mod psi_drifter {

    use crate::psi::monitor::PsiMonitor;
    use crate::psi::types::FeatureBinMapping;
    use chrono::NaiveDateTime;
    use scouter_contracts::ServiceInfo;
    use scouter_dispatch::AlertDispatcher;
    use scouter_error::DriftError;
    use scouter_sql::PostgresClient;
    use scouter_types::psi::{PsiDriftProfile, PsiFeatureAlerts, PsiFeatureDriftProfile};
    use std::collections::{BTreeMap, HashMap};
    use tracing::error;
    use tracing::info;

    pub struct PsiDrifter {
        service_info: ServiceInfo,
        profile: PsiDriftProfile,
    }

    impl PsiDrifter {
        pub fn new(profile: PsiDriftProfile) -> Self {
            Self {
                service_info: ServiceInfo {
                    name: profile.config.name.clone(),
                    repository: profile.config.repository.clone(),
                    version: profile.config.version.clone(),
                },
                profile,
            }
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

        async fn get_feature_bin_proportion_pairs_map(
            &self,
            limit_datetime: &NaiveDateTime,
            db_client: &PostgresClient,
        ) -> Result<Option<FeatureBinMapping>, DriftError> {
            let profiles_to_monitor = self.get_monitored_profiles();

            let observed_bin_proportions = db_client
                .get_feature_bin_proportions(
                    &self.service_info,
                    limit_datetime,
                    &self.profile.config.alert_config.features_to_monitor,
                )
                .await
                .map_err(|e| {
                    error!(
                        "Error: Unable to fetch feature bin proportions from DB for {}/{}/{}: {}",
                        self.service_info.repository,
                        self.service_info.name,
                        self.service_info.version,
                        e
                    );
                    DriftError::Error("Error processing alerts".to_string())
                })?;

            if observed_bin_proportions.is_empty() {
                info!(
                "No observed bin proportions available for {}/{}/{}. This indicates that no real-world data values have been recorded in the database for the monitored features as of {}. Skipping alert processing.",
                self.service_info.repository,
                self.service_info.name,
                self.service_info.version,
                limit_datetime
            );
                return Ok(None);
            }

            Ok(Some(FeatureBinMapping::from_observed_bin_proportions(
                &observed_bin_proportions,
                &profiles_to_monitor,
            )?))
        }

        pub async fn get_drift_map(
            &self,
            limit_datetime: &NaiveDateTime,
            db_client: &PostgresClient,
        ) -> Result<Option<HashMap<String, f64>>, DriftError> {
            self.get_feature_bin_proportion_pairs_map(limit_datetime, db_client)
                .await?
                .map(|feature_bin_map| {
                    Ok(feature_bin_map
                        .features
                        .iter()
                        .map(|(feature, pairs)| {
                            (feature.clone(), PsiMonitor::new().compute_psi(&pairs.pairs))
                        })
                        .collect())
                })
                .transpose()
        }

        fn filter_drift_map(&self, drift_map: &HashMap<String, f64>) -> HashMap<String, f64> {
            let psi_threshold = self.profile.config.alert_config.psi_threshold;
            let filtered_drift_map: HashMap<String, f64> = drift_map
                .iter()
                .filter(|(_, &value)| value > psi_threshold)
                .map(|(key, &value)| (key.clone(), value))
                .collect();

            filtered_drift_map
        }

        pub async fn generate_alerts(
            &self,
            drift_map: &HashMap<String, f64>,
        ) -> Result<Option<HashMap<String, f64>>, DriftError> {
            let alert_dispatcher = AlertDispatcher::new(&self.profile.config).map_err(|e| {
                error!(
                    "Error creating alert dispatcher for {}/{}/{}: {}",
                    self.service_info.repository,
                    self.service_info.name,
                    self.service_info.version,
                    e
                );
                DriftError::Error("Error creating alert dispatcher".to_string())
            })?;

            let filtered_map = self.filter_drift_map(drift_map);

            if filtered_map.is_empty() {
                info!(
                    "No alerts to process for {}/{}/{}",
                    self.service_info.repository, self.service_info.name, self.service_info.version
                );
                return Ok(None);
            }

            alert_dispatcher
                .process_alerts(&PsiFeatureAlerts {
                    features: filtered_map.clone(),
                    threshold: self.profile.config.alert_config.psi_threshold,
                })
                .await
                .map_err(|e| {
                    error!(
                        "Error processing alerts for {}/{}/{}: {}",
                        self.service_info.repository,
                        self.service_info.name,
                        self.service_info.version,
                        e
                    );
                    DriftError::Error("Error processing alerts".to_string())
                })?;
            Ok(Some(filtered_map))
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
        fn organize_alerts(
            &self,
            mut alerts: HashMap<String, f64>,
        ) -> Vec<BTreeMap<String, String>> {
            let threshold = self.profile.config.alert_config.psi_threshold;
            let mut alert_vec = Vec::new();
            alerts.iter_mut().for_each(|(feature, psi)| {
                let mut alert_map = BTreeMap::new();
                alert_map.insert("feature".to_string(), feature.clone());
                alert_map.insert("psi".to_string(), psi.to_string());
                alert_map.insert("threshold".to_string(), threshold.to_string());
                alert_vec.push(alert_map);
            });

            alert_vec
        }

        pub async fn check_for_alerts(
            &self,
            db_client: &PostgresClient,
            previous_run: NaiveDateTime,
        ) -> Result<Option<Vec<BTreeMap<String, String>>>, DriftError> {
            info!(
                "Processing drift task for profile: {}/{}/{}",
                self.service_info.repository, self.service_info.name, self.service_info.version
            );

            if self
                .profile
                .config
                .alert_config
                .features_to_monitor
                .is_empty()
            {
                info!(
                    "No PSI profiles to process for {}/{}/{}",
                    self.service_info.repository, self.service_info.name, self.service_info.version
                );
                return Ok(None);
            }

            let drift_map = self.get_drift_map(&previous_run, db_client).await?;

            match drift_map {
                Some(drift_map) => {
                    let alerts = self.generate_alerts(&drift_map).await.map_err(|e| {
                        error!(
                            "Error generating alerts for {}/{}/{}: {}",
                            self.service_info.repository,
                            self.service_info.name,
                            self.service_info.version,
                            e
                        );
                        DriftError::Error("Error processing alerts".to_string())
                    })?;
                    match alerts {
                        Some(alerts) => Ok(Some(self.organize_alerts(alerts))),
                        None => Ok(None),
                    }
                }
                None => Ok(None),
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::psi::types::FeatureBinProportionPairs;
        use ndarray::Array;
        use ndarray_rand::rand_distr::Uniform;
        use ndarray_rand::RandomExt;
        use scouter_types::psi::{
            Bin, FeatureBinProportion, FeatureBinProportions, PsiAlertConfig, PsiDriftConfig,
            PsiFeatureDriftProfile,
        };

        fn get_test_drifter() -> PsiDrifter {
            let config = PsiDriftConfig::new(
                Some("name".to_string()),
                Some("repo".to_string()),
                None,
                None,
                None,
                Some(PsiAlertConfig::new(
                    None,
                    None,
                    Some(vec!["feature_1".to_string(), "feature_3".to_string()]),
                    None,
                    None,
                )),
                None,
            )
            .unwrap();

            let array = Array::random((1030, 3), Uniform::new(1.0, 100.0));

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
        fn test_get_monitored_profiles() {
            let drifter = get_test_drifter();

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

        #[test]
        fn test_get_feature_bin_proportion_pairs() {
            let training_feat1_decile1_prop = 0.2;
            let training_feat1_decile2_prop = 0.5;
            let training_feat1_decile3_prop = 0.3;

            let feature_drift_profile = PsiFeatureDriftProfile {
                id: "feature_1".to_string(),
                bins: vec![
                    Bin {
                        id: "decile_1".to_string(),
                        lower_limit: Some(0.1),
                        upper_limit: Some(0.2),
                        proportion: training_feat1_decile1_prop,
                    },
                    Bin {
                        id: "decile_2".to_string(),
                        lower_limit: Some(0.2),
                        upper_limit: Some(0.4),
                        proportion: training_feat1_decile2_prop,
                    },
                    Bin {
                        id: "decile_3".to_string(),
                        lower_limit: Some(0.4),
                        upper_limit: Some(0.8),
                        proportion: training_feat1_decile3_prop,
                    },
                ],
                timestamp: Default::default(),
            };

            let observed_feat1_decile1_prop = 0.6;
            let observed_feat1_decile2_prop = 0.3;
            let observed_feat1_decile3_prop = 0.1;

            let observed_proportions = FeatureBinProportions::from_bins(vec![
                FeatureBinProportion {
                    feature: "feature_1".to_string(),
                    bin_id: "decile_1".to_string(),
                    proportion: observed_feat1_decile1_prop,
                },
                FeatureBinProportion {
                    feature: "feature_1".to_string(),
                    bin_id: "decile_2".to_string(),
                    proportion: observed_feat1_decile2_prop,
                },
                FeatureBinProportion {
                    feature: "feature_1".to_string(),
                    bin_id: "decile_3".to_string(),
                    proportion: observed_feat1_decile3_prop,
                },
            ]);

            let pairs = FeatureBinProportionPairs::from_observed_bin_proportions(
                &observed_proportions,
                &feature_drift_profile,
            )
            .unwrap();

            pairs.pairs.iter().for_each(|(a, b)| {
                if *a == training_feat1_decile1_prop {
                    assert_eq!(*b, observed_feat1_decile1_prop);
                } else if *a == training_feat1_decile2_prop {
                    assert_eq!(*b, observed_feat1_decile2_prop);
                } else if *a == training_feat1_decile3_prop {
                    assert_eq!(*b, observed_feat1_decile3_prop);
                } else {
                    panic!("test failed: proportion mismatch!");
                }
            })
        }

        #[test]
        fn test_filter_drift_map() {
            let drifter = get_test_drifter();

            let mut drift_map = HashMap::new();

            let feature_with_drift = "feature_4".to_string();

            drift_map.insert("feature_1".to_string(), 0.07);
            drift_map.insert("feature_2".to_string(), 0.2);
            drift_map.insert("feature_3".to_string(), 0.23);
            drift_map.insert(feature_with_drift.clone(), 0.3);
            drift_map.insert("feature_5".to_string(), 0.12);

            // we did not specify a custom psi threshold and thus will be using the default of 0.25
            let filtered_drift_map = drifter.filter_drift_map(&drift_map);
            assert_eq!(filtered_drift_map.len(), 1);
            assert!(filtered_drift_map.contains_key(&feature_with_drift));
        }
    }
}
