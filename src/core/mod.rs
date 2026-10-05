//! Core application services live here.
//! The core owns application state and orchestration; UI, voice and character
//! remain separate modules.

pub struct SaeedCore {
    pub version: &'static str,
}

impl SaeedCore {
    pub const fn new() -> Self {
        Self { version: env!("CARGO_PKG_VERSION") }
    }
}
