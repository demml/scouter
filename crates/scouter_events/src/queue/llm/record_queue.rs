use crate::error::FeatureQueueError;
use crate::queue::traits::FeatureQueue;
use core::result::Result::Ok;
use scouter_types::BoxedLLMDriftServerRecord;
use scouter_types::LLMRecord;
use scouter_types::QueueExt;
use scouter_types::{
    llm::LLMDriftProfile, LLMDriftServerRecord, MessageRecord, ServerRecord, ServerRecords,
};
use tracing::instrument;
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
    #[instrument(skip_all, name = "insert_llm")]
    pub fn insert(
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
                        record.prompt.clone(),
                        record.context.clone(),
                        record.created_at,
                        record.uid.clone(),
                        record.score.clone(),
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
    ) -> Result<MessageRecord, FeatureQueueError> {
        // clones the empty map (so we don't need to recreate it on each call)
        let mut queue = self.empty_queue.clone();

        for elem in batch {
            self.insert(elem.llm_records(), &mut queue)?;
        }

        Ok(MessageRecord::ServerRecords(
            self.create_drift_records(queue)?,
        ))
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use potato_head::create_score_prompt;
    use scouter_types::llm::{LLMAlertConfig, LLMDriftConfig, LLMDriftMetric, LLMDriftProfile};
    use scouter_types::AlertThreshold;

    async fn get_test_drift_profile() -> LLMDriftProfile {
        let prompt = create_score_prompt(Some(vec!["input".to_string()]));
        let metric1 = LLMDriftMetric::new(
            "coherence",
            5.0,
            AlertThreshold::Below,
            Some(0.5),
            Some(prompt.clone()),
        )
        .unwrap();

        let metric2 = LLMDriftMetric::new(
            "relevancy",
            5.0,
            AlertThreshold::Below,
            None,
            Some(prompt.clone()),
        )
        .unwrap();

        let alert_config = LLMAlertConfig::default();
        let drift_config =
            LLMDriftConfig::new("scouter", "ML", "0.1.0", 25, alert_config, None).unwrap();

        LLMDriftProfile::from_metrics(drift_config, vec![metric1, metric2])
            .await
            .unwrap()
    }

    #[test]
    fn test_feature_queue_llm_insert_record() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let drift_profile = runtime.block_on(async { get_test_drift_profile().await });
        let feature_queue = LLMRecordQueue::new(drift_profile);

        assert_eq!(feature_queue.empty_queue.len(), 0);

        let mut record_batch = Vec::new();
        for _ in 0..1 {
            let mut new_map = serde_json::Map::new();
            // insert entry in map
            new_map.insert("input".into(), serde_json::Value::String("test".into()));
            let context = serde_json::Value::Object(new_map);

            let record = LLMRecord::new_rs(Some(context), None);
            record_batch.push(record);
        }

        let records = feature_queue
            .create_drift_records_from_batch(record_batch)
            .unwrap();

        // empty should be excluded
        assert_eq!(records.len(), 1);
    }
}
