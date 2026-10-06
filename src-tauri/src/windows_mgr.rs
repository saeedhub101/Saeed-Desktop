use std::{sync::mpsc, thread, time::Duration};

use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

use crate::app::AppState;
use crate::settings::{CharacterScale, Position};

/// How long Rust waits for the page to report that its 3D cleanup finished.
const CLEANUP_TIMEOUT: Duration = Duration::from_millis(1000);
/// Short pause after the acknowledgement so the GPU process can act on the
/// context-loss request before the WebView is torn down.
const CLEANUP_GRACE: Duration = Duration::from_millis(100);

/// Progress of a Hide, so a Show that arrives mid-hide is not lost.
pub enum HideState {
    Idle,
    Hiding { reshow: bool },
}

fn size(scale: CharacterScale) -> u32 {
    match scale {
        CharacterScale::Small => 280,
        CharacterScale::Medium => 360,
        CharacterScale::Large => 460,
    }
}

fn default_position(app: &AppHandle, width: u32) -> Position {
    if let Ok(monitors) = app.available_monitors() {
        if let Some(monitor) = monitors.first() {
            let work_area = monitor.work_area();

            return clamp_to_work_area(
                Position {
                    x: work_area.position.x
                        + ((work_area.size.width.saturating_sub(width)) / 2) as i32,
                    y: work_area.position.y
                        + work_area.size.height.saturating_sub(width + 12) as i32,
                },
                work_area,
                width,
            );
        }
    }

    Position { x: 100, y: 100 }
}

fn clamp_position(app: &AppHandle, desired: Position, width: u32) -> Position {
    let Ok(monitors) = app.available_monitors() else {
        return desired;
    };

    let monitor = monitors
        .iter()
        .find(|monitor| {
            let area = monitor.work_area();
            desired.x >= area.position.x
                && desired.x < area.position.x + area.size.width as i32
                && desired.y >= area.position.y
                && desired.y < area.position.y + area.size.height as i32
        })
        .or_else(|| monitors.first());

    let Some(monitor) = monitor else {
        return desired;
    };

    let area = monitor.work_area();
    clamp_to_work_area(desired, area, width)
}

fn clamp_to_work_area(
    desired: Position,
    area: &tauri::PhysicalRect<i32, u32>,
    width: u32,
) -> Position {
    let max_x = area.position.x + area.size.width.saturating_sub(width) as i32;
    let max_y = area.position.y + area.size.height.saturating_sub(width) as i32;

    Position {
        x: desired.x.clamp(area.position.x, max_x.max(area.position.x)),
        y: desired.y.clamp(area.position.y, max_y.max(area.position.y)),
    }
}

pub fn create_character_window(app: &AppHandle) -> Result<(), String> {
    if let Some(state) = app.try_state::<AppState>() {
        if let Ok(mut hide) = state.hide_state.lock() {
            if let HideState::Hiding { reshow } = &mut *hide {
                // Window is being torn down; recreate it once that finishes.
                *reshow = true;
                return Ok(());
            }
        }
    }

    if let Some(window) = app.get_webview_window("character") {
        window.show().map_err(|e| e.to_string())?;
        window.set_focus().map_err(|e| e.to_string())?;
        return Ok(());
    }

    let state = app.state::<AppState>();
    let settings = state
        .settings
        .lock()
        .map_err(|_| "settings lock".to_string())?
        .clone();

    let width = size(settings.character.scale);
    let position = if settings.character.position.x != -1 || settings.character.position.y != -1 {
        clamp_position(app, settings.character.position, width)
    } else {
        default_position(app, width)
    };

    let scale_factor = app
        .monitor_from_point(position.x as f64, position.y as f64)
        .map_err(|e| e.to_string())?
        .map(|monitor| monitor.scale_factor())
        .unwrap_or(1.0);

    let logical_width = f64::from(width) / scale_factor;
    let logical_x = f64::from(position.x) / scale_factor;
    let logical_y = f64::from(position.y) / scale_factor;

    let icon = tauri::include_image!("./icons/32x32.png");

    let window = WebviewWindowBuilder::new(app, "character", WebviewUrl::App("index.html".into()))
    .title("Saeed")
    .inner_size(logical_width, logical_width)
    .position(logical_x, logical_y)
    .transparent(true)
    .decorations(false)
    .shadow(false)
    .always_on_top(settings.character.always_on_top)
    .skip_taskbar(true)
    .resizable(false)
    .visible(true)
    .focused(true)
    .icon(icon)
    .map_err(|e| e.to_string())?
    .build()
    .map_err(|e| e.to_string())?;

    window
        .set_ignore_cursor_events(false)
        .map_err(|e| e.to_string())?;

    crate::cursor_probe::start(app);
    state.logger.info("Character window created");

    if settings.character.position.x == -1 && settings.character.position.y == -1 {
        let mut saved = state
            .settings
            .lock()
            .map_err(|e| format!("settings lock: {e}"))?;

        saved.character.position = position;

        let dir = crate::app::data_dir(app)?;
        saved.save(&dir)?;
    }

    Ok(())
}

/// Destroys the character window after asking the page to release its 3D
/// resources first (geometries, materials, textures, renderer, GL context).
///
/// Blocks while waiting for the page, so it must NOT run on the main thread:
/// the acknowledgement is delivered through the main thread's event loop.
pub fn destroy_character_window(app: &AppHandle) -> Result<(), String> {
    let Some(window) = app.get_webview_window("character") else {
        return Ok(());
    };

    crate::cursor_probe::stop(app);

    let state = app.try_state::<AppState>();

    if let Some(state) = &state {
        let (sender, receiver) = mpsc::channel();

        if let Ok(mut slot) = state.cleanup_ack.lock() {
            *slot = Some(sender);
        }

        if window.emit("prepare-destroy", ()).is_ok() {
            match receiver.recv_timeout(CLEANUP_TIMEOUT) {
                Ok(()) => {
                    state.logger.info("Character renderer cleanup acknowledged");
                    thread::sleep(CLEANUP_GRACE);
                }
                Err(_) => state
                    .logger
                    .error("Character renderer cleanup timed out; destroying anyway"),
            }
        }

        if let Ok(mut slot) = state.cleanup_ack.lock() {
            *slot = None;
        }
    }

    window.destroy().map_err(|e| e.to_string())?;

    if let Some(state) = &state {
        state.logger.info("Character window destroyed");
    }

    Ok(())
}

/// Starts a graceful Hide on a worker thread and returns immediately.
pub fn hide_character(app: &AppHandle) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };

    {
        let Ok(mut hide) = state.hide_state.lock() else {
            return;
        };

        if let HideState::Hiding { reshow } = &mut *hide {
            // Latest intent wins: a second Hide cancels a pending re-show.
            *reshow = false;
            return;
        }

        *hide = HideState::Hiding { reshow: false };
    }

    let app = app.clone();

    thread::spawn(move || {
        let result = destroy_character_window(&app);
        finish_hide(&app, result);
    });
}

fn finish_hide(app: &AppHandle, result: Result<(), String>) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };

    let reshow = match state.hide_state.lock() {
        Ok(mut hide) => {
            let reshow = matches!(*hide, HideState::Hiding { reshow: true });
            *hide = HideState::Idle;
            reshow
        }
        Err(_) => false,
    };

    if let Err(error) = &result {
        state
            .logger
            .error(&format!("Character window destroy failed: {error}"));
    }

    if reshow {
        let _ = show_character(app);
    } else if result.is_ok() {
        if let Ok(mut settings) = state.settings.lock() {
            settings.character.visible = false;

            if let Ok(dir) = crate::app::data_dir(app) {
                let _ = settings.save(&dir);
            }

            state.logger.info("Character window hidden");
        }
    }

    crate::tray::refresh(app);
}

pub fn show_character(app: &AppHandle) -> Result<(), String> {
    create_character_window(app)?;

    if let Some(state) = app.try_state::<AppState>() {
        if let Ok(mut settings) = state.settings.lock() {
            settings.character.visible = true;

            if let Ok(dir) = crate::app::data_dir(app) {
                let _ = settings.save(&dir);
            }

            state.logger.info("Character window shown");
        }
    }

    Ok(())
}