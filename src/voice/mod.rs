//! Voice hardware and provider boundary.
//!
//! The microphone is active only while the user explicitly enables Mic mode.
//! Speech detection is local; network work happens outside the audio callback.

use std::io::Cursor;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

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

/// Captures microphone samples while the stream is alive.
/// Speech detection runs outside the CPAL callback.
pub struct MicrophoneRecorder {
    stream: Option<Stream>,
    samples: Arc<Mutex<Vec<i16>>>,
    config: Option<StreamConfig>,
}

impl MicrophoneRecorder {
    pub fn new() -> Self {
        Self {
            stream: None,
            samples: Arc::new(Mutex::new(Vec::new())),
            config: None,
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
                                (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16
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
        self.stream = Some(stream);
        Ok(())
    }

    pub fn stop(&mut self) -> Result<Vec<u8>, String> {
        self.stop_stream()?;

        let config = self
            .config
            .take()
            .ok_or_else(|| "Microphone configuration is unavailable.".to_string())?;

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

        encode_wav(&samples, &config)
    }

    fn stop_stream(&mut self) -> Result<(), String> {
        self.stream.take();
        Ok(())
    }

    /// Waits for one spoken utterance using local RMS + silence detection.
    ///
    /// Returns None when Mic mode is disabled before speech is completed.
    pub fn capture_utterance(
        &mut self,
        active: &AtomicBool,
        activation_threshold: f32,
        silence_timeout: Duration,
        minimum_speech: Duration,
    ) -> Result<Option<Vec<u8>>, String> {
        if !self.is_recording() {
            return Err("Microphone is not listening.".to_string());
        }

        let started_at = Instant::now();
        let mut last_checked = 0usize;
        let mut speech_started: Option<Instant> = None;
        let mut last_voice = Instant::now();

        while active.load(Ordering::Acquire) {
            thread::sleep(Duration::from_millis(40));

            let samples = self
                .samples
                .lock()
                .map_err(|_| "Microphone buffer is unavailable.".to_string())?
                .clone();

            if samples.len() <= last_checked {
                continue;
            }

            let chunk = &samples[last_checked..];
            last_checked = samples.len();

            let rms = rms(chunk);
            if rms >= activation_threshold {
                let now = Instant::now();
                if speech_started.is_none() {
                    speech_started = Some(now);
                }
                last_voice = now;
            }

            if let Some(start) = speech_started {
                let now = Instant::now();
                if now.duration_since(start) >= minimum_speech
                    && now.duration_since(last_voice) >= silence_timeout
                {
                    let audio = self.stop()?;
                    return Ok(Some(audio));
                }
            }

            if speech_started.is_none() && started_at.elapsed() > Duration::from_millis(200) {
                // Keep only a short rolling pre-speech buffer to avoid unbounded
                // memory growth during silence.
                let keep = 48_000usize;
                if let Ok(mut buffer) = self.samples.lock() {
                    if buffer.len() > keep {
                        let remove = buffer.len() - keep;
                        buffer.drain(..remove);
                        last_checked = last_checked.saturating_sub(remove);
                    }
                }
            }
        }

        self.stop_stream()?;
        self.config.take();
        Ok(None)
    }
}

fn rms(samples: &[i16]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }

    let sum = samples
        .iter()
        .map(|sample| {
            let normalized = *sample as f32 / i16::MAX as f32;
            normalized * normalized
        })
        .sum::<f32>();

    (sum / samples.len() as f32).sqrt()
}

fn encode_wav(samples: &[i16], config: &StreamConfig) -> Result<Vec<u8>, String> {
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
                .write_sample(*sample)
                .map_err(|error| format!("Could not encode microphone audio: {error}"))?;
        }

        writer
            .finalize()
            .map_err(|error| format!("Could not finalize microphone audio: {error}"))?;
    }

    Ok(bytes.into_inner())
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

pub struct OpenAiSpeechToText {
    api_key: String,
    model: String,
}

impl OpenAiSpeechToText {
    pub fn new(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        Self { api_key: api_key.into(), model: model.into() }
    }

    pub fn from_config(api_key: impl Into<String>, model: impl Into<String>) -> Result<Self, String> {\n        let api_key = api_key.into().trim().to_string();\n        if api_key.is_empty() {\n            return Err("OpenAI API key is not configured. Open Settings and save your API key.".to_string());\n        }\n        Ok(Self::new(api_key, model))\n    }\n\n    pub fn from_config(\n        api_key: impl Into<String>,\n        model: impl Into<String>,\n        voice: impl Into<String>,\n    ) -> Result<Self, String> {\n        let api_key = api_key.into().trim().to_string();\n        if api_key.is_empty() {\n            return Err("OpenAI API key is not configured. Open Settings and save your API key.".to_string());\n        }\n        Ok(Self::new(api_key, model, voice))\n    }\n\n    pub fn from_environment() -> Result<Self, String> {
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

        let form = Form::new().part("file", part).text("model", &self.model);
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
        Self { api_key: api_key.into(), model: model.into(), voice: voice.into() }
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
