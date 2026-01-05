use crate::error::FeatureQueueError;
use crate::queue::traits::FeatureQueue;
use core::result::Result::Ok;
use scouter_types::BoxedGenAIEvalRecord;
use scouter_types::QueueExt;
use scouter_types::{MessageRecord, ServerRecord, ServerRecords};

pub struct GenAIEvalRecordQueue {}

impl GenAIEvalRecordQueue {
    pub fn new() -> Self {
        GenAIEvalRecordQueue {}
    }
}

impl FeatureQueue for GenAIEvalRecordQueue {
    fn create_drift_records_from_batch<T: QueueExt>(
        &self,
        batch: Vec<T>,
    ) -> Result<MessageRecord, FeatureQueueError> {
        // Convert T to GenAIEvalRecord using QueueExt::into_genai_record
        let genai_records: Vec<ServerRecord> = batch
            .into_iter()
            .filter_map(|item| {
                let record = item.into_genai_record();
                record.map(|r| ServerRecord::GenAIEval(BoxedGenAIEvalRecord::new(r)))
            })
            .collect();

        Ok(MessageRecord::ServerRecords(ServerRecords::new(
            genai_records,
        )))
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use scouter_types::GenAIEvalRecord;

    #[test]
    fn test_feature_queue_genai_insert_record() {
        let feature_queue = GenAIEvalRecordQueue::new();

        let mut record_batch = Vec::new();
        for _ in 0..1 {
            let mut new_map = serde_json::Map::new();
            // insert entry in map
            new_map.insert("input".into(), serde_json::Value::String("test".into()));
            let context = serde_json::Value::Object(new_map);

            let record = GenAIEvalRecord {
                context,
                ..Default::default()
            };
            record_batch.push(record);
        }

        let records = feature_queue
            .create_drift_records_from_batch(record_batch)
            .unwrap();

        // empty should be excluded
        assert_eq!(records.len(), 1);
    }
}
