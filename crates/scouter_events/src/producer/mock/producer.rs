use crate::error::EventError;
use crate::producer::mock::MockConfig;
use scouter_types::MessageRecord;
use tracing::info;

#[derive(Debug, Clone)]
pub struct MockProducer {}
impl MockProducer {
    pub async fn new(_config: MockConfig) -> Result<Self, EventError> {
        Ok(MockProducer {})
    }

    pub async fn publish(&self, message: MessageRecord) -> Result<(), EventError> {
        // Mock implementation, just log the message
        info!("MockProducer publishing message: {:?}", message);
        Ok(())
    }

    pub async fn flush(&self) -> Result<(), EventError> {
        Ok(())
    }
}
