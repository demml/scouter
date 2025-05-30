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
    use scouter_sql::PostgresClient;
    use scouter_types::contracts::{DriftRequest, ServiceInfo};
    use scouter_types::psi::{
        BinnedPsiFeatureMetrics, BinnedPsiMetric, PsiDriftProfile, PsiFeatureAlerts,
        PsiFeatureDriftProfile,
    };
    use sqlx::{Pool, Postgres};
    use std::collections::{BTreeMap, HashMap};
    use tracing::info;
    use tracing::{debug, error, instrument};

    pub struct PsiDrifter {
        service_info: ServiceInfo,
        profile: PsiDriftProfile,
    }

    impl PsiDrifter {
        pub fn new(profile: PsiDriftProfile) -> Self {
            Self {
                service_info: ServiceInfo {
                    name: profile.config.name.clone(),
                    space: profile.config.space.clone(),
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
            limit_datetime: &DateTime<Utc>,
            db_pool: &Pool<Postgres>,
        ) -> Result<Option<FeatureBinMapping>, DriftError> {
            let profiles_to_monitor = self.get_monitored_profiles();

            let observed_bin_proportions = PostgresClient::get_feature_bin_proportions(
                db_pool,
                &self.service_info,
                limit_datetime,
                &self.profile.config.alert_config.features_to_monitor,
            )
            .await
            .inspect_err(|e| {
                error!(
                    "Error: Unable to fetch feature bin proportions from DB for {}/{}/{}: {}",
                    self.service_info.space, self.service_info.name, self.service_info.version, e
                );
            })?;

            if observed_bin_proportions.is_empty() {
                info!(
                "No observed bin proportions available for {}/{}/{}. Skipping alert processing.",
                self.service_info.space,
                self.service_info.name,
                self.service_info.version,
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
            limit_datetime: &DateTime<Utc>,
            db_pool: &Pool<Postgres>,
        ) -> Result<Option<HashMap<String, f64>>, DriftError> {
            self.get_feature_bin_proportion_pairs_map(limit_datetime, db_pool)
                .await?
                .map(|feature_bin_map| {
                    Ok(feature_bin_map
                        .features
                        .iter()
                        .map(|(feature, pairs)| {
                            (feature.clone(), PsiMonitor::compute_psi(&pairs.pairs))
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
            let alert_dispatcher = AlertDispatcher::new(&self.profile.config).inspect_err(|e| {
                error!(
                    "Error creating alert dispatcher for {}/{}/{}: {}",
                    self.service_info.space, self.service_info.name, self.service_info.version, e
                );
            })?;

            let filtered_map = self.filter_drift_map(drift_map);

            if filtered_map.is_empty() {
                info!(
                    "No alerts to process for {}/{}/{}",
                    self.service_info.space, self.service_info.name, self.service_info.version
                );
                return Ok(None);
            }

            alert_dispatcher
                .process_alerts(&PsiFeatureAlerts {
                    features: filtered_map.clone(),
                    threshold: self.profile.config.alert_config.psi_threshold,
                })
                .await
                .inspect_err(|e| {
                    error!(
                        "Error processing alerts for {}/{}/{}: {}",
                        self.service_info.space,
                        self.service_info.name,
                        self.service_info.version,
                        e
                    );
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
                alert_map.insert("entity_name".to_string(), feature.clone());
                alert_map.insert("psi".to_string(), psi.to_string());
                alert_map.insert("threshold".to_string(), threshold.to_string());
                alert_vec.push(alert_map);
            });

            alert_vec
        }

        pub async fn check_for_alerts(
            &self,
            db_pool: &Pool<Postgres>,
            previous_run: DateTime<Utc>,
        ) -> Result<Option<Vec<BTreeMap<String, String>>>, DriftError> {
            if self
                .profile
                .config
                .alert_config
                .features_to_monitor
                .is_empty()
            {
                return Ok(None);
            }

            let drift_map = self.get_drift_map(&previous_run, db_pool).await?;

            match drift_map {
                Some(drift_map) => {
                    let alerts = self.generate_alerts(&drift_map).await.inspect_err(|e| {
                        error!(
                            "Error generating alerts for {}/{}/{}: {}",
                            self.service_info.space,
                            self.service_info.name,
                            self.service_info.version,
                            e
                        );
                    })?;
                    match alerts {
                        Some(alerts) => Ok(Some(self.organize_alerts(alerts))),
                        None => Ok(None),
                    }
                }
                None => Ok(None),
            }
        }

        fn create_feature_bin_proportion_pairs(
            &self,
            feature: &str,
            bin_proportions: &BTreeMap<usize, f64>,
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
        ) -> Result<BinnedPsiFeatureMetrics, DriftError> {
            debug!(
                "Getting binned drift map for {}/{}/{}",
                self.service_info.space, self.service_info.name, self.service_info.version
            );
            let binned_records = PostgresClient::get_binned_psi_drift_records(
                db_pool,
                drift_request,
                retention_period,
                storage_settings,
            )
            .await?;

            if binned_records.is_empty() {
                info!(
                    "No binned drift records available for {}/{}/{}",
                    self.service_info.space, self.service_info.name, self.service_info.version
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
        use crate::psi::types::FeatureBinProportionPairs;
        use ndarray::Array;
        use ndarray_rand::rand_distr::Uniform;
        use ndarray_rand::RandomExt;
        use scouter_types::{
            psi::{
                Bin, BinType, FeatureBinProportion, FeatureBinProportions, PsiAlertConfig,
                PsiDriftConfig, PsiFeatureDriftProfile,
            },
            DEFAULT_VERSION,
        };

        fn get_test_drifter() -> PsiDrifter {
            let alert_config = PsiAlertConfig {
                features_to_monitor: vec!["feature_1".to_string(), "feature_3".to_string()],
                ..Default::default()
            };

            let config =
                PsiDriftConfig::new("name", "repo", DEFAULT_VERSION, alert_config, None).unwrap();

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
                bin_type: BinType::Numeric,
                bins: vec![
                    Bin {
                        id: 1,
                        lower_limit: Some(0.1),
                        upper_limit: Some(0.2),
                        proportion: training_feat1_decile1_prop,
                    },
                    Bin {
                        id: 2,
                        lower_limit: Some(0.2),
                        upper_limit: Some(0.4),
                        proportion: training_feat1_decile2_prop,
                    },
                    Bin {
                        id: 3,
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

            let mut feat1_bins = BTreeMap::new();
            feat1_bins.insert(1, observed_feat1_decile1_prop);
            feat1_bins.insert(2, observed_feat1_decile2_prop);
            feat1_bins.insert(3, observed_feat1_decile3_prop);

            let observed_proportions =
                FeatureBinProportions::from_features(vec![FeatureBinProportion {
                    feature: "feature_1".to_string(),
                    bins: feat1_bins,
                }]);

            let pairs = FeatureBinProportionPairs::from_observed_bin_proportions(
                observed_proportions.features.get("feature_1").unwrap(),
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
