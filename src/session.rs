#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageSource {
    Chat,
    Voice,
    System,
}

impl Default for MessageSource {
    fn default() -> Self {
        Self::System
    }
}

#[derive(Debug, Clone, Default)]
pub struct Message {
    pub role: String,
    pub content: String,
    pub source: MessageSource,
}

#[derive(Debug, Default)]
pub struct Session {
    messages: Vec<Message>,
}

impl Session {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(
        &mut self,
        role: impl Into<String>,
        content: impl Into<String>,
        source: MessageSource,
    ) {
        self.messages.push(Message {
            role: role.into(),
            content: content.into(),
            source,
        });
    }

    pub fn messages(&self) -> &[Message] {
        &self.messages
    }

    pub fn restore(messages: Vec<Message>) -> Self {
        Self { messages }
    }
}

#[cfg(test)]
mod tests {
    use super::{MessageSource, Session};

    #[test]
    fn chat_and_voice_share_one_conversation() {
        let mut session = Session::new();
        session.push("user", "start with voice", MessageSource::Voice);
        session.push("assistant", "I heard you", MessageSource::Voice);
        session.push("user", "continue in chat", MessageSource::Chat);

        assert_eq!(session.messages().len(), 3);
        assert_eq!(session.messages()[0].source, MessageSource::Voice);
        assert_eq!(session.messages()[2].source, MessageSource::Chat);
        assert_eq!(session.messages()[2].content, "continue in chat");
    }
}
