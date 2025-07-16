use potato_head::Provider;
use serde::Serialize;
#[derive(Debug, Clone, Serialize)]
pub struct LLMSettings {
    pub openai_api_key: String,
    pub openai_api_url: String,
}

impl LLMSettings {
    /// Used by server to check if the LLM settings are configured.
    pub fn is_configured(&self) -> bool {
        !self.openai_api_key.is_empty() && !self.openai_api_url.is_empty()
    }
}

impl Default for LLMSettings {
    fn default() -> Self {
        let openai_api_key = std::env::var("OPENAI_API_KEY").unwrap_or_else(|_| "".to_string());
        let env_api_url = std::env::var("OPENAI_API_URL").ok();
        let openai_api_url = env_api_url.unwrap_or_else(|| Provider::OpenAI.url().to_string());

        Self {
            openai_api_key,
            openai_api_url,
        }
    }
}
