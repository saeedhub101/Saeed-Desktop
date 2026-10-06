//! Single application controller. Chat and voice are input surfaces for one shared session.

use crate::ai::{AiProvider, AiRequest, AiResponse};
use crate::session::{Message, MessageSource, Session};

pub struct SaeedCore {
    pub version: &'static str,
    session: Session,
}

impl SaeedCore {
    pub fn new() -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION"),
            session: Session::new(),
        }
    }

    pub fn from_messages(messages: Vec<Message>) -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION"),
            session: Session::restore(messages),
        }
    }

    pub fn add_message(
        &mut self,
        role: impl Into<String>,
        content: impl Into<String>,
        source: MessageSource,
    ) {
        self.session.push(role, content, source);
    }

    pub fn add_chat_message(&mut self, role: impl Into<String>, content: impl Into<String>) {
        self.add_message(role, content, MessageSource::Chat);
    }

    pub fn add_voice_message(&mut self, role: impl Into<String>, content: impl Into<String>) {
        self.add_message(role, content, MessageSource::Voice);
    }

    pub fn messages(&self) -> &[Message] {
        self.session.messages()
    }

    pub fn snapshot_messages(&self) -> Vec<Message> {
        self.session.messages().to_vec()
    }

    pub fn add_assistant_response(&mut self, source: MessageSource, text: impl Into<String>) {
        self.add_message("assistant", text, source);
    }

    pub fn complete<P: AiProvider>(&self, provider: &P) -> Result<AiResponse, String> {
        provider.complete(&AiRequest {
            messages: self.session.messages().to_vec(),
        })
    }

    pub fn complete_chat<P: AiProvider>(&mut self, provider: &P) -> Result<AiResponse, String> {
        let response = self.complete(provider)?;
        self.add_chat_message("assistant", response.text.clone());
        Ok(response)
    }

    pub fn complete_voice<P: AiProvider>(&mut self, provider: &P) -> Result<AiResponse, String> {
        let response = self.complete(provider)?;
        self.add_voice_message("assistant", response.text.clone());
        Ok(response)
    }
}

#[cfg(test)]
mod tests {
    use super::SaeedCore;
    use crate::ai::{AiProvider, AiRequest, AiResponse};
    use crate::session::MessageSource;

    struct TestProvider;

    impl AiProvider for TestProvider {
        fn complete(&self, request: &AiRequest) -> Result<AiResponse, String> {
            Ok(AiResponse {
                text: format!("received {} messages", request.messages.len()),
            })
        }
    }

    #[test]
    fn chat_and_voice_are_one_session() {
        let mut core = SaeedCore::new();
        core.add_voice_message("user", "hello by voice");
        core.add_chat_message("user", "continue here");

        assert_eq!(core.messages().len(), 2);
        assert_eq!(core.messages()[0].source, MessageSource::Voice);
        assert_eq!(core.messages()[1].source, MessageSource::Chat);
    }

    #[test]
    fn assistant_response_is_added_to_same_session() {
        let mut core = SaeedCore::new();
        core.add_chat_message("user", "hello");

        let response = core.complete_chat(&TestProvider).expect("completion");
        assert_eq!(response.text, "received 1 messages");
        assert_eq!(core.messages().len(), 2);
        assert_eq!(core.messages()[1].role, "assistant");
        assert_eq!(core.messages()[1].source, MessageSource::Chat);
    }
}
