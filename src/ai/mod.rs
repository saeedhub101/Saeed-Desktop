//! AI provider boundary. Providers do not own session state or UI lifecycle.

use crate::session::Message;
use serde::Deserialize;

#[derive(Debug, Clone)]
pub struct AiRequest {
    pub messages: Vec<Message>,
}

#[derive(Debug, Clone)]
pub struct AiResponse {
    pub text: String,
}

pub trait AiProvider {
    fn complete(&self, request: &AiRequest) -> Result<AiResponse, String>;
}

/// OpenAI Responses API provider.
///
/// The API key is read from OPENAI_API_KEY so secrets never enter source control.
/// The model can be overridden with OPENAI_MODEL; otherwise a low-cost current
/// model is used.
pub struct OpenAiProvider {
    api_key: String,
    model: String,
}

impl OpenAiProvider {
    pub fn new(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            model: model.into(),
        }
    }

    pub fn from_environment() -> Result<Self, String> {
        let api_key = std::env::var("OPENAI_API_KEY")
            .map_err(|_| "OPENAI_API_KEY is not configured.".to_string())?;

        let api_key = api_key.trim().to_string();
        if api_key.is_empty() {
            return Err("OPENAI_API_KEY is empty.".to_string());
        }

        let model = std::env::var("OPENAI_MODEL")
            .unwrap_or_else(|_| "gpt-6-luna".to_string());

        Ok(Self::new(api_key, model))
    }
}

#[derive(Debug, Deserialize)]
struct ResponsesApiResponse {
    #[serde(default)]
    output_text: Option<String>,
}

impl AiProvider for OpenAiProvider {
    fn complete(&self, request: &AiRequest) -> Result<AiResponse, String> {
        let input: Vec<_> = request
            .messages
            .iter()
            .map(|message| {
                serde_json::json!({
                    "role": message.role,
                    "content": message.content,
                })
            })
            .collect();

        let body = serde_json::json!({
            "model": self.model,
            "input": input,
        });

        let mut response = ureq::post("https://api.openai.com/v1/responses")
            .header("Authorization", &format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .send_json(&body)
            .map_err(|error| format!("OpenAI request failed: {error}"))?;

        let payload: ResponsesApiResponse = response
            .body_mut()
            .read_json()
            .map_err(|error| format!("OpenAI response could not be read: {error}"))?;

        let text = payload
            .output_text
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| "OpenAI returned no text response.".to_string())?;

        Ok(AiResponse { text })
    }
}

#[cfg(test)]
mod tests {
    use super::OpenAiProvider;

    #[test]
    fn provider_constructor_keeps_credentials_out_of_environment_tests() {
        let provider = OpenAiProvider::new("test-key", "gpt-6-luna");
        assert_eq!(provider.model, "gpt-6-luna");
        assert_eq!(provider.api_key, "test-key");
    }
}
