use std::{
    sync::atomic::Ordering,
    thread,
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter, Manager};
use crate::app::AppState;

const NEAR: Duration = Duration::from_millis(50);
const FAR: Duration = Duration::from_millis(200);
const HEARTBEAT: Duration = Duration::from_secs(1);
const MARGIN: f64 = 64.0;

pub fn start(app: &AppHandle) {
    let Some(state) = app.try_state::<AppState>() else { return };
    let id = state.probe_generation.fetch_add(1, Ordering::SeqCst) + 1;
    let app = app.clone();
    thread::spawn(move || run(&app, id));
}

pub fn stop(app: &AppHandle) {
    if let Some(state) = app.try_state::<AppState>() {
        state.probe_generation.fetch_add(1, Ordering::SeqCst);
    }
}

fn current(app: &AppHandle, id: u64) -> bool {
    app.try_state::<AppState>()
        .is_some_and(|s| s.probe_generation.load(Ordering::SeqCst) == id)
}

fn run(app: &AppHandle, id: u64) {
    let mut last: Option<(f64, f64, u32, u32)> = None;
    let mut last_sent = Instant::now();
    let mut was_inside = false;

    while current(app, id) {
        let Some(window) = app.get_webview_window("character") else { break };
        let mut delay = FAR;
        let click_through = app
            .try_state::<AppState>()
            .and_then(|s| s.settings.lock().ok().map(|settings| settings.character.click_through))
            .unwrap_or(true);
        let interaction_active = app
            .try_state::<AppState>()
            .is_some_and(|s| s.interaction_active.load(Ordering::SeqCst));
        if interaction_active || !click_through {
            thread::sleep(Duration::from_millis(100));
            continue;
        }

        if let (Ok(cursor), Ok(position), Ok(size)) =
            (window.cursor_position(), window.inner_position(), window.inner_size())
        {
            let x = cursor.x - f64::from(position.x);
            let y = cursor.y - f64::from(position.y);
            let w = f64::from(size.width);
            let h = f64::from(size.height);
            let inside = x >= 0.0 && y >= 0.0 && x < w && y < h;
            let near = x >= -MARGIN && y >= -MARGIN && x < w + MARGIN && y < h + MARGIN;
            if near { delay = NEAR; }

            if inside || was_inside {
                let sample = (x, y, size.width, size.height);
                if last != Some(sample) || last_sent.elapsed() >= HEARTBEAT {
                    last = Some(sample);
                    last_sent = Instant::now();
                    let _ = window.emit("cursor-probe", serde_json::json!({
                        "x": x, "y": y, "width": size.width, "height": size.height
                    }));
                }
            }
            was_inside = inside;
        }
        thread::sleep(delay);
    }
}
