use crate::error::FeatureQueueError;
use crate::queue::traits::FeatureQueue;
use core::result::Result::Ok;
use scouter_types::BoxedGenAIDriftRecord;
use scouter_types::GenAIRecord;
use scouter_types::QueueExt;
use scouter_types::{
    genai::GenAIEvalProfile, GenAIDriftRecord, MessageRecord, ServerRecord, ServerRecords,
};
use tracing::instrument;
pub struct GenAIRecordQueue {
    drift_profile: GenAIEvalProfile,
    empty_queue: Vec<GenAIRecord>,
}

impl GenAIRecordQueue {
    pub fn new(drift_profile: GenAIEvalProfile) -> Self {
        GenAIRecordQueue {
            drift_profile,
            empty_queue: Vec::new(),
        }
    }

    /// Insert genai records into the queue
    ///
    /// # Arguments
    ///
    /// * `records` - A vector of genai records to insert into the queue
    ///
    /// # Returns
    ///
    /// * `Result<(), FeatureQueueError>` - A result indicating success or failure
    #[instrument(skip_all, name = "insert_genai")]
    pub fn insert(
        &self,
        records: Vec<&GenAIRecord>,
        queue: &mut Vec<GenAIRecord>,
    ) -> Result<(), FeatureQueueError> {
        for record in records {
            queue.push(record.clone());
        }
        Ok(())
    }

    fn create_drift_records(
        &self,
        queue: Vec<GenAIRecord>,
    ) -> Result<ServerRecords, FeatureQueueError> {
        let records = queue
            .iter()
            .map(|record| {
                ServerRecord::GenAIDrift(BoxedGenAIDriftRecord::new(GenAIDriftRecord::new_rs(
                    record.prompt.clone(),
                    record.context.clone(),
                    record.created_at,
                    record.score.clone(),
                    record.uid.clone(),
                    self.drift_profile.config.uid.clone(),
                ))) // Removed the semicolon here
            })
            .collect::<Vec<ServerRecord>>();

        Ok(ServerRecords::new(records))
    }
}

impl FeatureQueue for GenAIRecordQueue {
    fn create_drift_records_from_batch<T: QueueExt>(
        &self,
        batch: Vec<T>,
    ) -> Result<MessageRecord, FeatureQueueError> {
        // clones the empty map (so we don't need to recreate it on each call)
        let mut queue = self.empty_queue.clone();

        for elem in batch {
            self.insert(elem.genai_records(), &mut queue)?;
        }

        Ok(MessageRecord::ServerRecords(
            self.create_drift_records(queue)?,
        ))
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use potato_head::mock::create_score_prompt;
    use scouter_types::genai::{
        GenAIAlertConfig, GenAIDriftConfig, GenAIDriftMetric, GenAIEvalProfile,
    };
    use scouter_types::AlertThreshold;

    async fn get_test_drift_profile() -> GenAIEvalProfile {
        let prompt = create_score_prompt(Some(vec!["input".to_string()]));
        let metric1 = GenAIDriftMetric::new(
            "coherence",
            5.0,
            AlertThreshold::Below,
            Some(0.5),
            Some(prompt.clone()),
        )
        .unwrap();

        let metric2 = GenAIDriftMetric::new(
            "relevancy",
            5.0,
            AlertThreshold::Below,
            None,
            Some(prompt.clone()),
        )
        .unwrap();

        let alert_config = GenAIAlertConfig::default();
        let drift_config =
            GenAIDriftConfig::new("scouter", "ML", "0.1.0", 25, alert_config, None).unwrap();

        GenAIEvalProfile::from_metrics(drift_config, vec![metric1, metric2])
            .await
            .unwrap()
    }

    #[test]
    fn test_feature_queue_genai_insert_record() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let drift_profile = runtime.block_on(async { get_test_drift_profile().await });
        let feature_queue = GenAIRecordQueue::new(drift_profile);

        assert_eq!(feature_queue.empty_queue.len(), 0);

        let mut record_batch = Vec::new();
        for _ in 0..1 {
            let mut new_map = serde_json::Map::new();
            // insert entry in map
            new_map.insert("input".into(), serde_json::Value::String("test".into()));
            let context = serde_json::Value::Object(new_map);

            let record = GenAIRecord::new_rs(Some(context), None);
            record_batch.push(record);
        }

        let records = feature_queue
            .create_drift_records_from_batch(record_batch)
            .unwrap();

        // empty should be excluded
        assert_eq!(records.len(), 1);
    }
}
