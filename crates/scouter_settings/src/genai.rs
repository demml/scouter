use potato_head::{GoogleAuth, OpenAIAuth};
use serde::Serialize;

#[derive(Debug, Clone, Serialize, Default)]
pub struct GenAISettings {
    is_configured: bool,
}

impl GenAISettings {
    pub async fn new() -> Self {
        // Check if either OpenAI or Google authentication is configured
        let openai_configured = !matches!(OpenAIAuth::from_env(), OpenAIAuth::NotSet);
        let google_configured = !matches!(GoogleAuth::from_env().await, GoogleAuth::NotSet);

        Self {
            is_configured: openai_configured || google_configured,
        }
    }
    /// Used by server to check if the LLM settings are configured.
    pub fn is_configured(&self) -> bool {
        self.is_configured
    }
}
