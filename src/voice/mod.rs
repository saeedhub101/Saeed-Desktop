//! Voice boundary. Providers will implement these contracts without owning
//! application/session state.

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
