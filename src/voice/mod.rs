//! Voice provider boundary.
//!
//! Voice adapters never own application/session state. The Core remains the
//! only owner of conversation and lifecycle. This module only turns audio
//! into text and text into audio.

use serde::Deserialize;
use ureq::unversioned::multipart::{Form, Part};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoiceState {
    Off,
    Listening,
    Speaking,
}

pub trait SpeechToText {
    fn transcribe(&self, audio: &[u8]) -> Result<String, String>;
}

pub trait TextToSpeech {
    fn synthesize(&self, text: &str) -> Result<Vec<u8>, String>;
}

/// OpenAI speech-to-text adapter.
///
/// The caller supplies a complete audio file (for example WAV). Recording,
/// lifecycle, and conversation state remain outside this provider.
pub struct OpenAiSpeechToText {
    api_key: String,
    model: String,
}

impl OpenAiSpeechToText {
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

        let model = std::env::var("OPENAI_STT_MODEL")
            .unwrap_or_else(|_| "gpt-4o-mini-transcribe".to_string());

        Ok(Self::new(api_key, model))
    }
}

#[derive(Debug, Deserialize)]
struct TranscriptionResponse {
    text: String,
}

impl SpeechToText for OpenAiSpeechToText {
    fn transcribe(&self, audio: &[u8]) -> Result<String, String> {
        if audio.is_empty() {
            return Err("Voice input is empty.".to_string());
        }

        let part = Part::bytes(audio)
            .file_name("speech.wav")
            .mime_str("audio/wav")
            .map_err(|error| format!("Could not prepare audio: {error}"))?;

        let form = Form::new()
            .part("file", part)
            .text("model", &self.model);

        let mut response = ureq::post("https://api.openai.com/v1/audio/transcriptions")
            .header("Authorization", &format!("Bearer {}", self.api_key))
            .send(form)
            .map_err(|error| format!("Speech-to-text request failed: {error}"))?;

        let payload: TranscriptionResponse = response
            .body_mut()
            .read_json()
            .map_err(|error| format!("Speech-to-text response could not be read: {error}"))?;

        let text = payload.text.trim().to_string();
        if text.is_empty() {
            return Err("Speech-to-text returned no text.".to_string());
        }

        Ok(text)
    }
}

/// OpenAI text-to-speech adapter.
///
/// The returned bytes are the audio payload from the provider. Playback is
/// intentionally owned by the future voice runtime, not by this provider.
pub struct OpenAiTextToSpeech {
    api_key: String,
    model: String,
    voice: String,
}

impl OpenAiTextToSpeech {
    pub fn new(
        api_key: impl Into<String>,
        model: impl Into<String>,
        voice: impl Into<String>,
    ) -> Self {
        Self {
            api_key: api_key.into(),
            model: model.into(),
            voice: voice.into(),
        }
    }

    pub fn from_environment() -> Result<Self, String> {
        let api_key = std::env::var("OPENAI_API_KEY")
            .map_err(|_| "OPENAI_API_KEY is not configured.".to_string())?;

        let api_key = api_key.trim().to_string();
        if api_key.is_empty() {
            return Err("OPENAI_API_KEY is empty.".to_string());
        }

        let model = std::env::var("OPENAI_TTS_MODEL")
            .unwrap_or_else(|_| "gpt-4o-mini-tts".to_string());

        let voice = std::env::var("OPENAI_TTS_VOICE")
            .unwrap_or_else(|_| "alloy".to_string());

        Ok(Self::new(api_key, model, voice))
    }
}

impl TextToSpeech for OpenAiTextToSpeech {
    fn synthesize(&self, text: &str) -> Result<Vec<u8>, String> {
        let text = text.trim();
        if text.is_empty() {
            return Err("TTS text is empty.".to_string());
        }

        let body = serde_json::json!({
            "model": self.model,
            "voice": self.voice,
            "input": text,
            "response_format": "wav",
        });

        let mut response = ureq::post("https://api.openai.com/v1/audio/speech")
            .header("Authorization", &format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .send_json(&body)
            .map_err(|error| format!("Text-to-speech request failed: {error}"))?;

        response
            .body_mut()
            .read_to_vec()
            .map_err(|error| format!("Text-to-speech audio could not be read: {error}"))
    }
}

#[cfg(test)]
mod tests {
    use super::{OpenAiSpeechToText, OpenAiTextToSpeech};

    #[test]
    fn speech_providers_keep_explicit_configuration() {
        let stt = OpenAiSpeechToText::new("test-key", "test-stt");
        assert_eq!(stt.model, "test-stt");
        assert_eq!(stt.api_key, "test-key");

        let tts = OpenAiTextToSpeech::new("test-key", "test-tts", "test-voice");
        assert_eq!(tts.model, "test-tts");
        assert_eq!(tts.voice, "test-voice");
        assert_eq!(tts.api_key, "test-key");
    }
}
