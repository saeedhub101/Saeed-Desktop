//! Character runtime boundary: one owner for visibility, state, motion and renderer lifetime.

pub mod asset;
pub mod renderer;

pub use renderer::GlbCharacterRenderer;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CharacterVisibility { Hidden, Visible }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CharacterState { Idle, Curious, Playful, Tired, Sleeping, Waking, Interacting }

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MotionIntent {
    pub yaw: f32,
    pub pitch: f32,
    pub roll: f32,
    pub arm_wave: f32,
    pub duration_ms: u32,
}
impl MotionIntent {
    pub const IDLE: Self = Self { yaw: 0.0, pitch: 0.0, roll: 0.0, arm_wave: 0.0, duration_ms: 900 };
}

pub struct CharacterRuntime {
    visibility: CharacterVisibility,
    state: CharacterState,
    action_index: u32,
    asset_path: Option<std::path::PathBuf>,
    renderer: Option<GlbCharacterRenderer>,
    width: u32,
    height: u32,
    paused: bool,
    manual_pose: MotionIntent,
    rest_pose: MotionIntent,
}

impl CharacterRuntime {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            visibility: CharacterVisibility::Hidden,
            state: CharacterState::Idle,
            action_index: 0,
            asset_path: asset::find_default_asset(),
            renderer: None,
            width: width.max(64),
            height: height.max(64),
            paused: false,
            manual_pose: MotionIntent::IDLE,
            rest_pose: MotionIntent::IDLE,
        }
    }

    pub fn visibility(&self) -> CharacterVisibility { self.visibility }
    pub fn state(&self) -> CharacterState { self.state }
    pub fn set_state(&mut self, state: CharacterState) { self.state = state; }
    pub fn is_paused(&self) -> bool { self.paused }
    pub fn set_paused(&mut self, paused: bool) { self.paused = paused; }
    pub fn set_manual_pose(&mut self, yaw: f32, pitch: f32, roll: f32, arm_wave: f32) {
        self.manual_pose = MotionIntent { yaw, pitch, roll, arm_wave, duration_ms: 0 };
    }
    pub fn manual_pose(&self) -> MotionIntent { self.manual_pose }
    pub fn save_rest_pose(&mut self) { self.rest_pose = self.manual_pose; }
    pub fn restore_rest_pose(&mut self) { self.manual_pose = self.rest_pose; }
    pub fn rest_pose(&self) -> MotionIntent { self.rest_pose }

    pub fn set_visibility(&mut self, visibility: CharacterVisibility) -> Result<(), String> {
        if visibility == CharacterVisibility::Hidden {
            self.visibility = CharacterVisibility::Hidden;
            self.state = CharacterState::Idle;
            self.renderer = None;
            return Ok(());
        }

        self.ensure_renderer()?;
        self.visibility = CharacterVisibility::Visible;
        Ok(())
    }

    fn ensure_renderer(&mut self) -> Result<(), String> {
        if self.renderer.is_some() { return Ok(()); }
        let path = self.asset_path.clone().ok_or_else(|| "No Saeed GLB character asset was found.".to_string())?;
        self.renderer = Some(GlbCharacterRenderer::from_path(path, self.width, self.height)?);
        Ok(())
    }

    pub fn render_next(&mut self) -> Result<Option<slint::SharedPixelBuffer<slint::Rgba8Pixel>>, String> {
        if self.visibility != CharacterVisibility::Visible { return Ok(None); }
        self.ensure_renderer()?;
        let motion = if self.paused { self.manual_pose } else { self.next_motion().unwrap_or(MotionIntent::IDLE) };
        Ok(self.renderer.as_ref().map(|renderer| renderer.render(motion)))
    }

    pub fn render_idle(&mut self) -> Result<Option<slint::SharedPixelBuffer<slint::Rgba8Pixel>>, String> {
        if self.visibility != CharacterVisibility::Visible { return Ok(None); }
        self.ensure_renderer()?;
        Ok(self.renderer.as_ref().map(|renderer| renderer.render(if self.paused { self.manual_pose } else { MotionIntent::IDLE })))
    }

    pub fn next_motion(&mut self) -> Option<MotionIntent> {
        if self.visibility != CharacterVisibility::Visible || self.paused { return None; }
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

pub trait CharacterRuntimeApi {
    fn set_visibility(&mut self, visibility: CharacterVisibility) -> Result<(), String>;
    fn set_state(&mut self, state: CharacterState);
}

impl CharacterRuntimeApi for CharacterRuntime {
    fn set_visibility(&mut self, visibility: CharacterVisibility) -> Result<(), String> { self.set_visibility(visibility) }
    fn set_state(&mut self, state: CharacterState) { self.set_state(state); }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hidden_runtime_drops_renderer_and_emits_no_motion() {
        let mut runtime = CharacterRuntime::new(64, 64);
        assert_eq!(runtime.next_motion(), None);
        runtime.set_visibility(CharacterVisibility::Hidden).unwrap();
        assert_eq!(runtime.next_motion(), None);
    }

    #[test]
    fn visible_runtime_owns_renderer_lifecycle() {
        let runtime = CharacterRuntime::new(64, 64);
        assert!(runtime.renderer.is_none());
        assert!(!runtime.is_paused());
    }

    #[test]
    fn procedural_actions_change_without_a_loop() {
        let mut runtime = CharacterRuntime::new(64, 64);
        runtime.visibility = CharacterVisibility::Visible;
        runtime.state = CharacterState::Playful;
        let a = runtime.next_motion().unwrap();
        let b = runtime.next_motion().unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn pause_and_rest_pose_are_real_runtime_state() {
        let mut runtime = CharacterRuntime::new(64, 64);
        runtime.visibility = CharacterVisibility::Visible;
        runtime.set_manual_pose(0.2, 0.1, 0.0, 0.3);
        runtime.save_rest_pose();
        runtime.set_manual_pose(0.0, 0.0, 0.0, 0.0);
        runtime.restore_rest_pose();
        assert_eq!(runtime.manual_pose().yaw, 0.2);
        runtime.set_paused(true);
        assert_eq!(runtime.next_motion(), None);
    }
}
