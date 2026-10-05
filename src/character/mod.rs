//! Character runtime boundary. Rendering and motion remain independent from
//! the application core and can be loaded only when the character is visible.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CharacterVisibility {
    Hidden,
    Visible,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CharacterState {
    Idle,
    Curious,
    Playful,
    Tired,
    Sleeping,
    Waking,
    Interacting,
}

pub trait CharacterRuntime {
    fn set_visibility(&mut self, visibility: CharacterVisibility);
    fn set_state(&mut self, state: CharacterState);
}
