use crate::error::FeatureQueueError;
use crate::queue::traits::FeatureQueue;
use core::result::Result::Ok;
use scouter_types::BoxedGenAIEventRecord;
use scouter_types::GenAIEventRecord;
use scouter_types::GenAIRecord;
use scouter_types::QueueExt;
use scouter_types::{genai::GenAIEvalProfile, MessageRecord, ServerRecord, ServerRecords};
use tracing::instrument;
pub struct GenAIRecordQueue {
    drift_profile: GenAIEvalProfile,
}

impl GenAIRecordQueue {
    pub fn new(drift_profile: GenAIEvalProfile) -> Self {
        GenAIRecordQueue { drift_profile }
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
            .into_iter() // Changed from .iter() to .into_iter() to consume the vector
            .map(|genai_record| {
                ServerRecord::GenAIEvent(BoxedGenAIEventRecord::new(GenAIEventRecord::new_rs(
                    genai_record.context,
                    genai_record.created_at,
                    genai_record.uid,
                    self.drift_profile.config.uid.clone(),
                )))
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
        // Convert T to GenAIRecord using QueueExt::into_genai_record
        let genai_records: Vec<GenAIRecord> = batch
            .into_iter()
            .filter_map(|item| item.into_genai_record())
            .collect();

        Ok(MessageRecord::ServerRecords(
            self.create_drift_records(genai_records)?,
        ))
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use potato_head::mock::create_score_prompt;
    use scouter_types::genai::{
        ComparisonOperator, GenAIAlertConfig, GenAIDriftConfig, GenAIEvalProfile, LLMJudgeTask,
    };
    use serde_json::Value;

    async fn get_test_drift_profile() -> GenAIEvalProfile {
        let prompt = create_score_prompt(Some(vec!["input".to_string()]));

        let task1 = LLMJudgeTask::new_rs(
            "metric1",
            prompt.clone(),
            Value::Number(4.into()),
            None,
            ComparisonOperator::GreaterThanOrEqual,
            None,
            None,
        );

        let task2 = LLMJudgeTask::new_rs(
            "metric2",
            prompt.clone(),
            Value::Number(2.into()),
            None,
            ComparisonOperator::LessThanOrEqual,
            None,
            None,
        );

        let alert_config = GenAIAlertConfig::default();

        let drift_config =
            GenAIDriftConfig::new("scouter", "ML", "0.1.0", 25, alert_config, None).unwrap();

        GenAIEvalProfile::new(drift_config, None, Some(vec![task1, task2])).unwrap()
    }

    #[test]
    fn test_feature_queue_genai_insert_record() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let drift_profile = runtime.block_on(async { get_test_drift_profile().await });
        let feature_queue = GenAIRecordQueue::new(drift_profile);

        let mut record_batch = Vec::new();
        for _ in 0..1 {
            let mut new_map = serde_json::Map::new();
            // insert entry in map
            new_map.insert("input".into(), serde_json::Value::String("test".into()));
            let context = serde_json::Value::Object(new_map);

            let record = GenAIRecord::new_rs(Some(context));
            record_batch.push(record);
        }

        let records = feature_queue
            .create_drift_records_from_batch(record_batch)
            .unwrap();

        // empty should be excluded
        assert_eq!(records.len(), 1);
    }
}
