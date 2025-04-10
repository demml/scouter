use core::result::Result::Ok;
use scouter_error::FeatureQueueError;
use scouter_types::Metrics;
use scouter_types::{
    custom::CustomDriftProfile, CustomMetricServerRecord, ServerRecord, ServerRecords,
};
use std::collections::HashMap;
use tracing::error;

pub struct CustomMetricFeatureQueue {
    pub drift_profile: CustomDriftProfile,
    pub queue: HashMap<String, Vec<f64>>,
    pub metric_names: Vec<String>,
}

impl CustomMetricFeatureQueue {
    pub fn new(drift_profile: CustomDriftProfile) -> Self {
        let queue: HashMap<String, Vec<f64>> = drift_profile
            .metrics
            .keys()
            .map(|metric| (metric.clone(), Vec::new()))
            .collect();

        let metric_names = queue.keys().cloned().collect();

        CustomMetricFeatureQueue {
            drift_profile,
            queue,
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
    pub fn insert(&mut self, metrics: Metrics) -> Result<(), FeatureQueueError> {
        for metric in metrics.metrics {
            if !self.drift_profile.metrics.contains_key(&metric.name) {
                let valid_metric_names = self
                    .drift_profile
                    .metrics
                    .keys()
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ");
                error!(
                    "Custom metric {} not found in drift profile. Valid metric names include {}",
                    metric.name, valid_metric_names
                );
                continue;
            }
            if let Some(queue) = self.queue.get_mut(&metric.name) {
                queue.push(metric.value);
            }
        }
        Ok(())
    }

    pub fn create_drift_records(&mut self) -> Result<ServerRecords, FeatureQueueError> {
        let averages = self
            .queue
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

    pub fn is_empty(&self) -> bool {
        self.queue.values().all(|vals| vals.is_empty())
    }

    pub fn clear_queue(&mut self) {
        self.queue.values_mut().for_each(|vals| {
            vals.clear();
        });
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
        let mut feature_queue = CustomMetricFeatureQueue::new(profile);

        assert_eq!(feature_queue.queue.len(), 3);

        for i in 0..25 {
            let one = Metric::new("mae".to_string(), i as f64);
            let two = Metric::new("mape".to_string(), i as f64);

            let metrics = Metrics {
                metrics: vec![one, two],
            };

            feature_queue.insert(metrics).unwrap();
        }

        assert_eq!(feature_queue.queue.get("mae").unwrap().len(), 25);
        assert_eq!(feature_queue.queue.get("mape").unwrap().len(), 25);
        assert_eq!(feature_queue.queue.get("empty").unwrap().len(), 0);

        let records = feature_queue.create_drift_records().unwrap();

        // empty should be excluded
        assert_eq!(records.records.len(), 2);

        // check average of mae
        for record in records.records.iter() {
            if let ServerRecord::Custom(custom_record) = record {
                assert!(custom_record.metric.contains("ma"));
                assert_eq!(custom_record.value, 12.0);
            }
        }

        feature_queue.clear_queue();
    }
}
