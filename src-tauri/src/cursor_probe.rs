//! Cursor probe: tells the page where the mouse is relative to the character
//! window so it can toggle click-through on transparent pixels.
//!
//! The thread only exists while the character window exists, sends nothing
//! while the cursor is away from the window or standing still (apart from a
//! 1 s heartbeat while hovering), and polls slowly when the cursor is far away.

use std::{
    sync::atomic::Ordering,
    thread,
    time::{Duration, Instant},
};

use tauri::{AppHandle, Emitter, Manager};

use crate::app::AppState;

/// Poll interval while the cursor is over or close to the window.
const NEAR_INTERVAL: Duration = Duration::from_millis(50);
/// Poll interval while the cursor is far away (no events are sent).
const FAR_INTERVAL: Duration = Duration::from_millis(200);
/// Distance (physical px) around the window that counts as "close".
const NEAR_MARGIN: f64 = 64.0;
/// While hovering with an unchanged position, repeat the probe this often so a
/// page that finished loading late still learns the cursor position.
const HEARTBEAT: Duration = Duration::from_secs(1);

pub fn start(app: &AppHandle) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };

    let id = state.probe_generation.fetch_add(1, Ordering::SeqCst) + 1;
    let app = app.clone();

    thread::spawn(move || run(&app, id));
}

/// Invalidates the running probe thread; it exits on its next wake-up.
pub fn stop(app: &AppHandle) {
    if let Some(state) = app.try_state::<AppState>() {
        state.probe_generation.fetch_add(1, Ordering::SeqCst);
    }
}

fn is_current(app: &AppHandle, id: u64) -> bool {
    app.try_state::<AppState>()
        .is_some_and(|state| state.probe_generation.load(Ordering::SeqCst) == id)
}

fn run(app: &AppHandle, id: u64) {
    let mut last: Option<(f64, f64, u32, u32)> = None;
    let mut last_sent = Instant::now();
    let mut was_inside = false;

    while is_current(app, id) {
        let Some(window) = app.get_webview_window("character") else {
            break;
        };

        let mut interval = FAR_INTERVAL;

        if let (Ok(cursor), Ok(position), Ok(size)) = (
            window.cursor_position(),
            window.inner_position(),
            window.inner_size(),
        ) {
            let x = cursor.x - f64::from(position.x);
            let y = cursor.y - f64::from(position.y);
            let width = f64::from(size.width);
            let height = f64::from(size.height);

            let inside = x >= 0.0 && y >= 0.0 && x < width && y < height;
            let near = x >= -NEAR_MARGIN
                && y >= -NEAR_MARGIN
                && x < width + NEAR_MARGIN
                && y < height + NEAR_MARGIN;

            if near {
                interval = NEAR_INTERVAL;
            }

            // Send while hovering, plus one final probe when the cursor leaves
            // so the page can restore click-through.
            if inside || was_inside {
                let probe = (x, y, size.width, size.height);

                if last != Some(probe) || last_sent.elapsed() >= HEARTBEAT {
                    last = Some(probe);
                    last_sent = Instant::now();

                    let _ = window.emit(
                        "cursor-probe",
                        serde_json::json!({
                            "x": x,
                            "y": y,
                            "width": size.width,
                            "height": size.height
                        }),
                    );
                }
            }

            was_inside = inside;
        }

        thread::sleep(interval);
    }
}