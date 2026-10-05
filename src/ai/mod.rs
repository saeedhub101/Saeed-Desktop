//! AI provider boundary. The core chooses a provider; providers do not own
//! session state or UI lifecycle.

use crate::session::Message;

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
