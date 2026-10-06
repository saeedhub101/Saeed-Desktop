//! Voice turn orchestration.
//!
//! This module owns the voice pipeline after audio capture:
//! WAV -> STT -> shared Core session -> AI -> TTS -> playback.
//! It does not own UI lifecycle or create a second conversation/brain.

use std::sync::{Arc, Mutex};

use crate::ai::OpenAiProvider;
use crate::core::SaeedCore;
use crate::storage::AppSettings;

use super::{AudioPlayer, OpenAiSpeechToText, OpenAiTextToSpeech, SpeechToText, TextToSpeech};

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

        let api_key = settings
            .openai_api_key
            .ok_or_else(|| "Add your OpenAI API key in Settings first.".to_string())?;

        let stt = OpenAiSpeechToText::from_config(api_key.clone(), settings.stt_model)?;
        let transcript = stt.transcribe(audio)?;

        let response = {
            let mut core = core
                .lock()
                .map_err(|_| "Core state is unavailable.".to_string())?;

            core.add_voice_message("user", &transcript);

            let ai = OpenAiProvider::from_config(api_key, settings.ai_model)?;
            core.complete_voice(&ai)?.text
        };

        Ok(VoiceTurnResult {
            transcript,
            response,
        })
    }

    pub fn speak(
        settings: &Arc<Mutex<AppSettings>>,
        response: &str,
    ) -> Result<(), String> {
        let settings = settings
            .lock()
            .map_err(|_| "Settings state is unavailable.".to_string())?
            .clone();

        let api_key = settings
            .openai_api_key
            .ok_or_else(|| "Add your OpenAI API key in Settings first.".to_string())?;

        let tts = OpenAiTextToSpeech::from_config(
            api_key,
            settings.tts_model,
            settings.tts_voice,
        )?;

        let spoken_audio = tts.synthesize(response)?;
        AudioPlayer::play(&spoken_audio)
    }
}
