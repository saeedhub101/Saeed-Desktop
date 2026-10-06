//! Voice turn orchestration.
//!
//! This module owns the voice pipeline after audio capture:
//! WAV -> STT -> shared Core session -> AI -> TTS -> playback.
//! It does not own UI lifecycle or create a second conversation/brain.

use std::sync::{Arc, Mutex};

use crate::ai::{AiProvider, AiRequest, LocalCommandAiProvider, OpenAiProvider, OpenAiCompatibleProvider};
use crate::core::SaeedCore;
use crate::storage::{AppSettings, Storage};

use super::{
    AudioPlayer, LocalCommandSpeechToText, LocalCommandTextToSpeech,
    OpenAiSpeechToText, OpenAiTextToSpeech, OpenAiCompatibleSpeechToText, ElevenLabsTextToSpeech, SpeechToText, TextToSpeech,
};

#[derive(Debug, Clone)]
pub struct VoiceTurnResult {
    pub transcript: String,
    pub response: String,
}

pub struct VoiceController;

impl VoiceController {
    pub fn process_turn(
        core: &Arc<Mutex<SaeedCore>>,
        settings: &Arc<Mutex<AppSettings>>,
        audio: &[u8],
    ) -> Result<VoiceTurnResult, String> {
        let settings = settings
            .lock()
            .map_err(|_| "Settings state is unavailable.".to_string())?
            .clone();

        let transcript = match settings.stt_provider.trim().to_ascii_lowercase().as_str() {
            "local" => LocalCommandSpeechToText::from_environment()?.transcribe(audio)?,
            "groq" => {
                let key=settings.groq_api_key.clone().ok_or_else(|| "Groq STT selected: add the Groq API key in Settings.".to_string())?;
                OpenAiCompatibleSpeechToText::new(key, settings.stt_model, "https://api.groq.com/openai/v1/audio/transcriptions").transcribe(audio)?
            }
            _ => {
                let key=settings.openai_api_key.clone().ok_or_else(|| "OpenAI STT selected: add your OpenAI API key in Settings.".to_string())?;
                OpenAiSpeechToText::from_config(key, settings.stt_model)?.transcribe(audio)?
            }
        };

        let messages = {
            let mut core = core
                .lock()
                .map_err(|_| "Core state is unavailable.".to_string())?;
            core.add_voice_message("user", &transcript);
            if let Some(message) = core.messages().last() {
                let storage = Storage::open_default()
                    .map_err(|error| format!("Could not open conversation storage: {error}"))?;
                storage.append_message(message)
                    .map_err(|error| format!("Could not persist voice message: {error}"))?;
            }
            core.snapshot_messages()
        };

        let response = match settings.ai_provider.trim().to_ascii_lowercase().as_str() {
            "local" => LocalCommandAiProvider::from_environment()?.complete(&AiRequest { messages })?.text,
            "groq" => {
                let key=settings.groq_api_key.clone().ok_or_else(|| "Groq AI selected: add the Groq API key in Settings.".to_string())?;
                OpenAiCompatibleProvider::new(key, settings.ai_model, "https://api.groq.com/openai/v1/chat/completions").complete(&AiRequest { messages })?.text
            }
            _ => {
                let key=settings.openai_api_key.clone().ok_or_else(|| "OpenAI AI selected: add your OpenAI API key in Settings.".to_string())?;
                OpenAiProvider::from_config(key, settings.ai_model)?.complete(&AiRequest { messages })?.text
            }
        };

        {
            let mut core = core
                .lock()
                .map_err(|_| "Core state is unavailable.".to_string())?;
            core.add_assistant_response(crate::session::MessageSource::Voice, response.clone());
            if let Some(message) = core.messages().last() {
                let storage = Storage::open_default()
                    .map_err(|error| format!("Could not open conversation storage: {error}"))?;
                storage.append_message(message)
                    .map_err(|error| format!("Could not persist voice response: {error}"))?;
            }
        }

        Ok(VoiceTurnResult { transcript, response })
    }

    pub fn speak(
        settings: &Arc<Mutex<AppSettings>>,
        response: &str,
    ) -> Result<(), String> {
        let settings = settings
            .lock()
            .map_err(|_| "Settings state is unavailable.".to_string())?
            .clone();

        let spoken_audio = match settings.tts_provider.trim().to_ascii_lowercase().as_str() {
            "local" => LocalCommandTextToSpeech::from_environment()?.synthesize(response)?,
            "elevenlabs" => {
                let key=settings.elevenlabs_api_key.ok_or_else(|| "ElevenLabs TTS selected: add the ElevenLabs API key in Settings.".to_string())?;
                ElevenLabsTextToSpeech::new(key, settings.tts_voice, settings.tts_model).synthesize(response)?
            }
            _ => {
                let key=settings.openai_api_key.ok_or_else(|| "OpenAI TTS selected: add your OpenAI API key in Settings.".to_string())?;
                OpenAiTextToSpeech::from_config(key, settings.tts_model, settings.tts_voice)?.synthesize(response)?
            }
        };

        AudioPlayer::play(&spoken_audio)
    }
}
