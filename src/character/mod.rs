//! Lightweight character runtime boundary.
//!
//! Rendering is intentionally separate from motion state. The controller below
//! is dependency-free and produces on-demand procedural motion intents; a future
//! renderer can consume them without introducing an animation loop.

pub mod asset;

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

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MotionIntent {
    pub yaw: f32,
    pub pitch: f32,
    pub roll: f32,
    pub arm_wave: f32,
    pub duration_ms: u32,
}

impl MotionIntent {
    pub const IDLE: Self = Self {
        yaw: 0.0, pitch: 0.0, roll: 0.0, arm_wave: 0.0, duration_ms: 900,
    };
}

/// Owns character state and emits one-shot procedural motion intents.
/// It never starts a permanent render/update loop.
pub struct ProceduralCharacterController {
    visibility: CharacterVisibility,
    state: CharacterState,
    action_index: u32,
}

impl Default for ProceduralCharacterController {
    fn default() -> Self {
        Self::new()
    }
}

impl ProceduralCharacterController {
    pub fn new() -> Self {
        Self {
            visibility: CharacterVisibility::Hidden,
            state: CharacterState::Idle,
            action_index: 0,
        }
    }

    pub fn visibility(&self) -> CharacterVisibility {
        self.visibility
    }

    pub fn state(&self) -> CharacterState {
        self.state
    }

    pub fn set_visibility(&mut self, visibility: CharacterVisibility) {
        self.visibility = visibility;
        if visibility == CharacterVisibility::Hidden {
            self.state = CharacterState::Idle;
        }
    }

    pub fn set_state(&mut self, state: CharacterState) {
        self.state = state;
    }

    /// Returns the next procedural action only when the character is visible.
    /// Repeated calls advance through a deterministic non-repeating sequence.
    pub fn next_motion(&mut self) -> Option<MotionIntent> {
        if self.visibility != CharacterVisibility::Visible {
            return None;
        }

        let intent = match (self.state, self.action_index % 5) {
            (CharacterState::Idle, 0) => MotionIntent { yaw: -0.10, pitch: 0.02, roll: 0.0, arm_wave: 0.0, duration_ms: 700 },
            (CharacterState::Idle, 1) => MotionIntent { yaw: 0.10, pitch: -0.02, roll: 0.0, arm_wave: 0.0, duration_ms: 750 },
            (CharacterState::Curious, _) => MotionIntent { yaw: 0.18, pitch: 0.10, roll: 0.0, arm_wave: 0.0, duration_ms: 650 },
            (CharacterState::Playful, 0) => MotionIntent { yaw: -0.16, pitch: 0.06, roll: 0.04, arm_wave: 0.35, duration_ms: 800 },
            (CharacterState::Playful, 1) => MotionIntent { yaw: 0.16, pitch: -0.04, roll: -0.04, arm_wave: -0.35, duration_ms: 800 },
            (CharacterState::Interacting, _) => MotionIntent { yaw: 0.0, pitch: 0.05, roll: 0.0, arm_wave: 0.55, duration_ms: 900 },
            (CharacterState::Tired, _) => MotionIntent { yaw: 0.0, pitch: -0.08, roll: 0.0, arm_wave: -0.12, duration_ms: 1100 },
            (CharacterState::Sleeping, _) => MotionIntent { yaw: 0.0, pitch: -0.18, roll: 0.0, arm_wave: -0.20, duration_ms: 1400 },
            (CharacterState::Waking, _) => MotionIntent { yaw: 0.0, pitch: 0.10, roll: 0.0, arm_wave: 0.20, duration_ms: 900 },
            _ => MotionIntent::IDLE,
        };

        self.action_index = self.action_index.wrapping_add(1);
        Some(intent)
    }
}

pub trait CharacterRuntime {
    fn set_visibility(&mut self, visibility: CharacterVisibility);
    fn set_state(&mut self, state: CharacterState);
}

impl CharacterRuntime for ProceduralCharacterController {
    fn set_visibility(&mut self, visibility: CharacterVisibility) {
        self.set_visibility(visibility);
    }

    fn set_state(&mut self, state: CharacterState) {
        self.set_state(state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hidden_character_does_not_emit_motion() {
        let mut controller = ProceduralCharacterController::new();
        assert_eq!(controller.next_motion(), None);
    }

    #[test]
    fn visible_character_emits_one_shot_motion() {
        let mut controller = ProceduralCharacterController::new();
        controller.set_visibility(CharacterVisibility::Visible);
        assert!(controller.next_motion().is_some());
    }

    #[test]
    fn visibility_does_not_create_a_background_loop() {
        let mut controller = ProceduralCharacterController::new();
        controller.set_visibility(CharacterVisibility::Visible);
        controller.set_state(CharacterState::Playful);
        let first = controller.next_motion().unwrap();
        let second = controller.next_motion().unwrap();
        assert_ne!(first, second);
    }
}
