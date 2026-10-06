//! AI provider boundary. Providers do not own session state or UI lifecycle.

use crate::session::Message;
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

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

pub struct OpenAiProvider {
    api_key: String,
    model: String,
}

impl OpenAiProvider {
    pub fn new(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        Self { api_key: api_key.into(), model: model.into() }
    }

    pub fn from_config(api_key: impl Into<String>, model: impl Into<String>) -> Result<Self, String> {
        let api_key = api_key.into().trim().to_string();
        if api_key.is_empty() {
            return Err("OpenAI API key is not configured. Open Settings and save your API key.".to_string());
        }
        Ok(Self::new(api_key, model))
    }

    pub fn from_environment() -> Result<Self, String> {
        let api_key = std::env::var("OPENAI_API_KEY")
            .map_err(|_| "OPENAI_API_KEY is not configured.".to_string())?;
        let api_key = api_key.trim().to_string();
        if api_key.is_empty() {
            return Err("OPENAI_API_KEY is empty.".to_string());
        }
        let model = std::env::var("OPENAI_MODEL").unwrap_or_else(|_| "gpt-6-luna".to_string());
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
        let input: Vec<_> = request.messages.iter().map(|message| {
            serde_json::json!({"role": message.role, "content": message.content})
        }).collect();

        let body = serde_json::json!({"model": self.model, "input": input});
        let mut response = ureq::post("https://api.openai.com/v1/responses")
            .header("Authorization", &format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .send_json(&body)
            .map_err(|error| format!("OpenAI request failed: {error}"))?;

        let payload: ResponsesApiResponse = response.body_mut().read_json()
            .map_err(|error| format!("OpenAI response could not be read: {error}"))?;
        let text = payload.output_text
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| "OpenAI returned no text response.".to_string())?;
        Ok(AiResponse { text })
    }
}

pub struct OpenAiCompatibleProvider {
    api_key: String,
    model: String,
    endpoint: String,
}

impl OpenAiCompatibleProvider {
    pub fn new(api_key: impl Into<String>, model: impl Into<String>, endpoint: impl Into<String>) -> Self {
        Self { api_key: api_key.into(), model: model.into(), endpoint: endpoint.into() }
    }
}
impl AiProvider for OpenAiCompatibleProvider {
    fn complete(&self, request: &AiRequest) -> Result<AiResponse, String> {
        let messages: Vec<_> = request.messages.iter().map(|m| serde_json::json!({"role":m.role,"content":m.content})).collect();
        let body = serde_json::json!({"model":self.model,"messages":messages});
        #[derive(Deserialize)] struct Choice { message: MessageContent }
        #[derive(Deserialize)] struct MessageContent { content: String }
        #[derive(Deserialize)] struct ChatResponse { choices: Vec<Choice> }
        let mut response = ureq::post(&self.endpoint)
            .header("Authorization", &format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .send_json(&body)
            .map_err(|e| format!("AI provider request failed: {e}"))?;
        let payload: ChatResponse = response.body_mut().read_json()
            .map_err(|e| format!("AI provider response could not be read: {e}"))?;
        let text = payload.choices.first().map(|c| c.message.content.trim().to_string())
            .filter(|s| !s.is_empty()).ok_or_else(|| "AI provider returned no text.".to_string())?;
        Ok(AiResponse { text })
    }
}

fn local_temp_path() -> PathBuf {
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos()).unwrap_or_default();
    std::env::temp_dir().join(format!("saeed-ai-{}-{stamp}.json", std::process::id()))
}

/// Generic local AI adapter. The configured command receives {input} as a
/// UTF-8 JSON file containing the shared conversation and must print only the
/// assistant response to stdout.
pub struct LocalCommandAiProvider {
    command: String,
}

impl LocalCommandAiProvider {
    pub fn from_environment() -> Result<Self, String> {
        let command = std::env::var("SAEED_LOCAL_AI_COMMAND")
            .map_err(|_| "Local AI is selected but SAEED_LOCAL_AI_COMMAND is not configured.".to_string())?;
        let command = command.trim().to_string();
        if command.is_empty() {
            return Err("SAEED_LOCAL_AI_COMMAND is empty.".to_string());
        }
        Ok(Self { command })
    }
}

impl AiProvider for LocalCommandAiProvider {
    fn complete(&self, request: &AiRequest) -> Result<AiResponse, String> {
        if request.messages.is_empty() {
            return Err("AI request contains no messages.".to_string());
        }

        let input = local_temp_path();
        let messages: Vec<_> = request.messages.iter().map(|message| {
            serde_json::json!({"role": message.role, "content": message.content})
        }).collect();
        fs::write(&input, serde_json::to_vec(&messages)
            .map_err(|error| format!("Could not encode local AI request: {error}"))?)
            .map_err(|error| format!("Could not create local AI input: {error}"))?;

        let command_line = self.command.replace("{input}", &input.to_string_lossy());
        let result = Command::new("cmd").args(["/C", &command_line]).output();
        let _ = fs::remove_file(&input);
        let output = result.map_err(|error| format!("Local AI command could not start: {error}"))?;

        if !output.status.success() {
            return Err(format!("Local AI command failed: {}", String::from_utf8_lossy(&output.stderr).trim()));
        }

        let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if text.is_empty() {
            return Err("Local AI returned no text.".to_string());
        }
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
