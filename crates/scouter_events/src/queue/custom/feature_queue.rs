use crate::queue::traits::FeatureQueue;
use core::result::Result::Ok;
use scouter_error::FeatureQueueError;
use scouter_types::Metric;
use scouter_types::QueueExt;
use scouter_types::{
    custom::CustomDriftProfile, CustomMetricServerRecord, ServerRecord, ServerRecords,
};
use std::collections::HashMap;
use tracing::error;
pub struct CustomMetricFeatureQueue {
    drift_profile: CustomDriftProfile,
    empty_queue: HashMap<String, Vec<f64>>,
    metric_names: Vec<String>,
}

impl CustomMetricFeatureQueue {
    pub fn new(drift_profile: CustomDriftProfile) -> Self {
        let empty_queue: HashMap<String, Vec<f64>> = drift_profile
            .metrics
            .keys()
            .map(|metric| (metric.clone(), Vec::new()))
            .collect();

        let metric_names = empty_queue.keys().cloned().collect();

        CustomMetricFeatureQueue {
            drift_profile,
            empty_queue,
            metric_names,
        }
    }

    /// Insert metrics into the feature queue
    ///
    /// # Arguments
    ///
    /// * `metrics` - A vector of metrics to insert into the feature queue
    ///
    /// # Returns
    ///
    /// * `Result<(), FeatureQueueError>` - A result indicating success or failure
    fn insert(
        &self,
        metrics: &Vec<Metric>,
        queue: &mut HashMap<String, Vec<f64>>,
    ) -> Result<(), FeatureQueueError> {
        for metric in metrics {
            if !self.metric_names.contains(&metric.name) {
                error!("Custom metric {} not found in drift profile", metric.name);
                continue;
            }
            if let Some(queue) = queue.get_mut(&metric.name) {
                queue.push(metric.value);
            }
        }
        Ok(())
    }

    fn create_drift_records(
        &self,
        queue: HashMap<String, Vec<f64>>,
    ) -> Result<ServerRecords, FeatureQueueError> {
        let averages = queue
            .iter()
            // filter out empty values
            .filter(|(_, values)| !values.is_empty())
            .map(|(key, values)| {
                let avg = values.iter().sum::<f64>() / values.len() as f64;
                ServerRecord::Custom(CustomMetricServerRecord::new(
                    self.drift_profile.config.space.clone(),
                    self.drift_profile.config.name.clone(),
                    self.drift_profile.config.version.clone(),
                    key.clone(),
                    avg,
                ))
            })
            .collect::<Vec<ServerRecord>>();

        Ok(ServerRecords::new(averages))
    }
}

impl FeatureQueue for CustomMetricFeatureQueue {
    fn create_drift_records_from_batch<T: QueueExt>(
        &self,
        batch: Vec<T>,
    ) -> Result<ServerRecords, FeatureQueueError> {
        // clones the empty map (so we don't need to recreate it on each call)
        let mut queue = self.empty_queue.clone();

        for elem in batch {
            self.insert(elem.metrics(), &mut queue)?;
        }

        self.create_drift_records(queue)
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use scouter_types::custom::{
        AlertThreshold, CustomMetric, CustomMetricAlertConfig, CustomMetricDriftConfig,
    };
    use scouter_types::{Metric, Metrics};

    #[test]
    fn test_feature_queue_custom_insert_metric() {
        let metric1 = CustomMetric::new("mae", 10.0, AlertThreshold::Above, None).unwrap();

        let metric2 = CustomMetric::new("mape", 10.0, AlertThreshold::Above, None).unwrap();

        let metric3 = CustomMetric::new("empty", 10.0, AlertThreshold::Above, None).unwrap();

        let custom_config = CustomMetricDriftConfig::new(
            "test",
            "test",
            "0.1.0",
            true,
            25,
            CustomMetricAlertConfig::default(),
            None,
        )
        .unwrap();
        let profile =
            CustomDriftProfile::new(custom_config, vec![metric1, metric2, metric3], None).unwrap();
        let feature_queue = CustomMetricFeatureQueue::new(profile);

        assert_eq!(feature_queue.empty_queue.len(), 3);

        let mut metric_batch = Vec::new();
        for i in 0..25 {
            let one = Metric::new("mae".to_string(), i as f64);
            let two = Metric::new("mape".to_string(), i as f64);

            let metrics = Metrics {
                metrics: vec![one, two],
            };

            metric_batch.push(metrics);
        }

        let records = feature_queue
            .create_drift_records_from_batch(metric_batch)
            .unwrap();

        // empty should be excluded
        assert_eq!(records.records.len(), 2);

        // check average of mae
        for record in records.records.iter() {
            if let ServerRecord::Custom(custom_record) = record {
                assert!(custom_record.metric.contains("ma"));
                assert_eq!(custom_record.value, 12.0);
            }
        }
    }
}
