//! Voice turn orchestration.
//!
//! This module owns the voice pipeline after audio capture:
//! WAV -> STT -> shared Core session -> AI -> TTS -> playback.
//! It does not own UI lifecycle or create a second conversation/brain.

use std::sync::{Arc, Mutex};

use crate::ai::{AiProvider, LocalCommandAiProvider, OpenAiProvider};
use crate::core::SaeedCore;
use crate::storage::AppSettings;

use super::{
    AudioPlayer, LocalCommandSpeechToText, LocalCommandTextToSpeech,
    OpenAiSpeechToText, OpenAiTextToSpeech, SpeechToText, TextToSpeech,
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

        let transcript = if settings.stt_model.trim().eq_ignore_ascii_case("local") {
            let stt = LocalCommandSpeechToText::from_environment()?;
            stt.transcribe(audio)?
        } else {
            let api_key = settings.openai_api_key.clone()
                .ok_or_else(|| "OpenAI STT selected: add your OpenAI API key in Settings.".to_string())?;
            let stt = OpenAiSpeechToText::from_config(api_key, settings.stt_model)?;
            stt.transcribe(audio)?
        };

        let response = {
            let mut core = core
                .lock()
                .map_err(|_| "Core state is unavailable.".to_string())?;

            core.add_voice_message("user", &transcript);

            let api_key = settings.openai_api_key.clone()
                .ok_or_else(|| "AI currently requires an OpenAI API key.".to_string())?;
            let ai = OpenAiProvider::from_config(api_key, settings.ai_model)?;
            core.complete_voice(&ai)?.text
        };

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

        let spoken_audio = if settings.tts_model.trim().eq_ignore_ascii_case("local") {
            let tts = LocalCommandTextToSpeech::from_environment()?;
            tts.synthesize(response)?
        } else {
            let api_key = settings.openai_api_key
                .ok_or_else(|| "OpenAI TTS selected: add your OpenAI API key in Settings.".to_string())?;
            let tts = OpenAiTextToSpeech::from_config(api_key, settings.tts_model, settings.tts_voice)?;
            tts.synthesize(response)?
        };

        AudioPlayer::play(&spoken_audio)
    }
}
