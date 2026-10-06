//! Voice hardware and provider boundary.
//!
//! Audio hardware is active only while recording or playing a response.
//! Providers never own application/session state; Core remains the owner of
//! conversation state and lifecycle.

use std::io::Cursor;
use std::sync::{Arc, Mutex};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream, StreamConfig};
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

/// Captures from the Windows default microphone while the stream is alive.
/// Starting and stopping the recorder are explicit; there is no background
/// recording loop.
pub struct MicrophoneRecorder {
    stream: Option<Stream>,
    samples: Arc<Mutex<Vec<i16>>>,
    config: Option<StreamConfig>,
    sample_format: Option<SampleFormat>,
}

impl MicrophoneRecorder {
    pub fn new() -> Self {
        Self {
            stream: None,
            samples: Arc::new(Mutex::new(Vec::new())),
            config: None,
            sample_format: None,
        }
    }

    pub fn is_recording(&self) -> bool {
        self.stream.is_some()
    }

    pub fn start(&mut self) -> Result<(), String> {
        if self.stream.is_some() {
            return Err("Microphone is already listening.".to_string());
        }

        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or_else(|| "No default microphone was found.".to_string())?;

        let supported = device
            .default_input_config()
            .map_err(|error| format!("Could not open the default microphone: {error}"))?;

        let sample_format = supported.sample_format();
        let config: StreamConfig = supported.into();
        let samples = Arc::clone(&self.samples);

        {
            let mut buffer = samples
                .lock()
                .map_err(|_| "Microphone buffer is unavailable.".to_string())?;
            buffer.clear();
        }

        let stream = match sample_format {
            SampleFormat::F32 => device
                .build_input_stream(
                    &config,
                    move |data: &[f32], _| {
                        if let Ok(mut buffer) = samples.lock() {
                            buffer.extend(data.iter().map(|sample| {
                                let value = sample.clamp(-1.0, 1.0);
                                (value * i16::MAX as f32) as i16
                            }));
                        }
                    },
                    |error| eprintln!("Saeed microphone stream error: {error}"),
                    None,
                )
                .map_err(|error| format!("Could not start microphone: {error}"))?,
            SampleFormat::I16 => {
                let samples = Arc::clone(&self.samples);
                device
                    .build_input_stream(
                        &config,
                        move |data: &[i16], _| {
                            if let Ok(mut buffer) = samples.lock() {
                                buffer.extend_from_slice(data);
                            }
                        },
                        |error| eprintln!("Saeed microphone stream error: {error}"),
                        None,
                    )
                    .map_err(|error| format!("Could not start microphone: {error}"))?
            }
            SampleFormat::U16 => {
                let samples = Arc::clone(&self.samples);
                device
                    .build_input_stream(
                        &config,
                        move |data: &[u16], _| {
                            if let Ok(mut buffer) = samples.lock() {
                                buffer.extend(data.iter().map(|sample| {
                                    (*sample as i32 - 32_768) as i16
                                }));
                            }
                        },
                        |error| eprintln!("Saeed microphone stream error: {error}"),
                        None,
                    )
                    .map_err(|error| format!("Could not start microphone: {error}"))?
            }
            other => {
                return Err(format!(
                    "The default microphone format is not supported: {other:?}"
                ));
            }
        };

        stream
            .play()
            .map_err(|error| format!("Could not activate microphone: {error}"))?;

        self.config = Some(config);
        self.sample_format = Some(sample_format);
        self.stream = Some(stream);
        Ok(())
    }

    /// Stops recording and returns a complete PCM WAV file.
    pub fn stop(&mut self) -> Result<Vec<u8>, String> {
        let Some(_) = self.stream.take() else {
            return Err("Microphone is not listening.".to_string());
        };

        let config = self
            .config
            .take()
            .ok_or_else(|| "Microphone configuration is unavailable.".to_string())?;
        self.sample_format = None;

        let samples = {
            let buffer = self
                .samples
                .lock()
                .map_err(|_| "Microphone buffer is unavailable.".to_string())?;
            buffer.clone()
        };

        if samples.is_empty() {
            return Err("No microphone audio was captured.".to_string());
        }

        let mut bytes = Cursor::new(Vec::new());
        let spec = hound::WavSpec {
            channels: config.channels,
            sample_rate: config.sample_rate.0,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };

        {
            let mut writer = hound::WavWriter::new(&mut bytes, spec)
                .map_err(|error| format!("Could not create WAV audio: {error}"))?;

            for sample in samples {
                writer
                    .write_sample(sample)
                    .map_err(|error| format!("Could not encode microphone audio: {error}"))?;
            }

            writer
                .finalize()
                .map_err(|error| format!("Could not finalize microphone audio: {error}"))?;
        }

        Ok(bytes.into_inner())
    }
}

impl Default for MicrophoneRecorder {
    fn default() -> Self {
        Self::new()
    }
}

/// Plays provider audio through the Windows default output device.
pub struct AudioPlayer;

impl AudioPlayer {
    pub fn play(audio: &[u8]) -> Result<(), String> {
        if audio.is_empty() {
            return Err("TTS returned empty audio.".to_string());
        }

        let stream = rodio::DeviceSinkBuilder::open_default_sink()
            .map_err(|error| format!("Could not open the default speaker: {error}"))?;
        let player = rodio::Player::connect_new(stream.mixer());
        let decoder = rodio::Decoder::new(Cursor::new(audio.to_vec()))
            .map_err(|error| format!("Could not decode TTS audio: {error}"))?;

        player.append(decoder);
        player.sleep_until_end();
        Ok(())
    }
}

/// OpenAI speech-to-text adapter.
///
/// The caller supplies a complete audio file. Recording, lifecycle, and
/// conversation state remain outside this provider.
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
/// The returned bytes are the provider audio payload. Playback is handled by
/// AudioPlayer, keeping provider and hardware responsibilities separate.
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
    use super::{MicrophoneRecorder, OpenAiSpeechToText, OpenAiTextToSpeech};

    #[test]
    fn recorder_starts_off() {
        let recorder = MicrophoneRecorder::new();
        assert!(!recorder.is_recording());
    }

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
