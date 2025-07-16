use crate::error::FeatureQueueError;
use crate::queue::traits::FeatureQueue;
use core::result::Result::Ok;
use scouter_types::BoxedLLMDriftServerRecord;
use scouter_types::LLMRecord;
use scouter_types::QueueExt;
use scouter_types::{
    llm::LLMDriftProfile, CustomMetricServerRecord, LLMDriftServerRecord, ServerRecord,
    ServerRecords,
};
use std::collections::HashMap;
use tracing::{error, instrument};
pub struct LLMRecordQueue {
    drift_profile: LLMDriftProfile,
    empty_queue: Vec<LLMRecord>,
}

impl LLMRecordQueue {
    pub fn new(drift_profile: LLMDriftProfile) -> Self {
        LLMRecordQueue {
            drift_profile,
            empty_queue: Vec::new(),
        }
    }

    /// Insert llm records into the queue
    ///
    /// # Arguments
    ///
    /// * `records` - A vector of llm records to insert into the queue
    ///
    /// # Returns
    ///
    /// * `Result<(), FeatureQueueError>` - A result indicating success or failure
    #[instrument(skip_all, name = "insert_custom")]
    fn insert(
        &self,
        records: Vec<&LLMRecord>,
        queue: &mut Vec<LLMRecord>,
    ) -> Result<(), FeatureQueueError> {
        for record in records {
            queue.push(record.clone());
        }
        Ok(())
    }

    fn create_drift_records(
        &self,
        queue: Vec<LLMRecord>,
    ) -> Result<ServerRecords, FeatureQueueError> {
        let records = queue
            .iter()
            .map(|record| {
                ServerRecord::LLMDrift(BoxedLLMDriftServerRecord::new(
                    LLMDriftServerRecord::new_rs(
                        self.drift_profile.config.space.clone(),
                        self.drift_profile.config.name.clone(),
                        self.drift_profile.config.version.clone(),
                        record.input.clone(),
                        record.response.clone(),
                        record.prompt.clone(),
                        record.context.clone(),
                    ),
                )) // Removed the semicolon here
            })
            .collect::<Vec<ServerRecord>>();

        Ok(ServerRecords::new(records))
    }
}

impl FeatureQueue for LLMRecordQueue {
    fn create_drift_records_from_batch<T: QueueExt>(
        &self,
        batch: Vec<T>,
    ) -> Result<ServerRecords, FeatureQueueError> {
        // clones the empty map (so we don't need to recreate it on each call)
        let mut queue = self.empty_queue.clone();

        for elem in batch {
            self.insert(elem.llm_records(), &mut queue)?;
        }

        self.create_drift_records(queue)
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use scouter_types::custom::{CustomMetric, CustomMetricAlertConfig, CustomMetricDriftConfig};
    use scouter_types::{AlertThreshold, EntityType};
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
            let one = Metric::new_rs("mae".to_string(), i as f64);
            let two = Metric::new_rs("mape".to_string(), i as f64);

            let metrics = Metrics {
                metrics: vec![one, two],
                entity_type: EntityType::Metric,
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
