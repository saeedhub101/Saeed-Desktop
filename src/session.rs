#[derive(Debug, Clone, Default)]
pub struct Message {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Default)]
pub struct Session {
    messages: Vec<Message>,
}

impl Session {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, role: impl Into<String>, content: impl Into<String>) {
        self.messages.push(Message {
            role: role.into(),
            content: content.into(),
        });
    }

    pub fn messages(&self) -> &[Message] {
        &self.messages
    }
}

#[cfg(test)]
mod tests {
    use super::Session;

    #[test]
    fn preserves_shared_conversation_order() {
        let mut session = Session::new();
        session.push("user", "open the salary file");
        session.push("assistant", "I will open it");
        session.push("user", "calculate Ahmed's total");

        assert_eq!(session.messages().len(), 3);
        assert_eq!(session.messages()[2].content, "calculate Ahmed's total");
    }
}
