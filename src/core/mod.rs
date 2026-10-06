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

    pub fn complete<P: AiProvider>(&self, provider: &P) -> Result<AiResponse, String> {
        provider.complete(&AiRequest {
            messages: self.session.messages().to_vec(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::SaeedCore;
    use crate::session::MessageSource;

    #[test]
    fn chat_and_voice_are_one_session() {
        let mut core = SaeedCore::new();
        core.add_voice_message("user", "hello by voice");
        core.add_chat_message("user", "continue here");

        assert_eq!(core.messages().len(), 2);
        assert_eq!(core.messages()[0].source, MessageSource::Voice);
        assert_eq!(core.messages()[1].source, MessageSource::Chat);
    }
}
