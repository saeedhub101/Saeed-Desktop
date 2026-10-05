//! Single application controller. UI, voice, character and tools will call
//! this boundary instead of owning competing application state.

use crate::session::{Message, Session};

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

    pub fn add_message(&mut self, role: impl Into<String>, content: impl Into<String>) {
        self.session.push(role, content);
    }

    pub fn messages(&self) -> &[Message] {
        self.session.messages()
    }
}
