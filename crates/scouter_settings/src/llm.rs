use potato_head::Provider;
use serde::Serialize;

pub trait ProviderSettings {
    fn api_key(&self) -> &str;
    fn api_url(&self) -> &str;
    fn is_configured(&self) -> bool {
        !self.api_key().is_empty() && !self.api_url().is_empty()
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct OpenAISettings {
    pub api_key: String,
    pub api_url: String,
}

impl ProviderSettings for OpenAISettings {
    fn api_key(&self) -> &str {
        &self.api_key
    }
    fn api_url(&self) -> &str {
        &self.api_url
    }
}

impl Default for OpenAISettings {
    fn default() -> Self {
        let api_key = std::env::var("OPENAI_API_KEY").unwrap_or_else(|_| "".to_string());
        let env_api_url = std::env::var("OPENAI_API_URL").ok();
        let api_url = env_api_url.unwrap_or_else(|| Provider::OpenAI.url().to_string());
        Self { api_key, api_url }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct GeminiSettings {
    pub api_key: String,
    pub api_url: String,
}

impl ProviderSettings for GeminiSettings {
    fn api_key(&self) -> &str {
        &self.api_key
    }
    fn api_url(&self) -> &str {
        &self.api_url
    }
}

impl Default for GeminiSettings {
    fn default() -> Self {
        let api_key = std::env::var("GEMINI_API_KEY").unwrap_or_else(|_| "".to_string());
        let env_api_url = std::env::var("GEMINI_API_URL").ok();
        let api_url = env_api_url.unwrap_or_else(|| Provider::Gemini.url().to_string());
        Self { api_key, api_url }
    }
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct LLMSettings {
    pub openai_settings: OpenAISettings,
    pub gemini_settings: GeminiSettings,
}

impl LLMSettings {
    /// Used by server to check if the LLM settings are configured.
    pub fn is_configured(&self) -> bool {
        self.openai_settings.is_configured() || self.gemini_settings.is_configured()
    }
}
