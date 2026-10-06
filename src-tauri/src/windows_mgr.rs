use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};
use crate::app::AppState;
use crate::settings::{CharacterScale, Position};

fn size(s: CharacterScale) -> f64 {
    match s {
        CharacterScale::Small => 280.0,
        CharacterScale::Medium => 360.0,
        CharacterScale::Large => 460.0,
    }
}

fn default_pos(app: &AppHandle, w: f64) -> Position {
    if let Ok(ms) = app.available_monitors() {
        if let Some(m) = ms.first() {
            let a = m.work_area();
            return Position {
                x: a.position.x + (((a.size.width as f64 - w) / 2.0).max(0.0) as i32),
                y: a.position.y + (((a.size.height as f64 - w - 12.0).max(0.0)) as i32),
            };
        }
    }
    Position { x: 100, y: 100 }
}

pub fn create_character_window(app: &AppHandle) -> Result<(), String> {
    if let Some(w) = app.get_webview_window("character") {
        let _ = w.show();
        let _ = w.set_focus();
        return Ok(());
    }

    let st = app.state::<AppState>();
    let s = st.settings.lock().map_err(|_| "settings lock".to_string())?.clone();
    let w = size(s.character.scale);
    let p = if s.character.position.x >= 0 && s.character.position.y >= 0 {
        s.character.position
    } else {
        default_pos(app, w)
    };

    let icon = tauri::image::Image::from_app_icon_resource(32)
        .map_err(|e| e.to_string())?;

    let win = WebviewWindowBuilder::new(
        app,
        "character",
        WebviewUrl::App("index.html".into()),
    )
    .title("Saeed")
    .inner_size(w, w)
    .position(p.x as f64, p.y as f64)
    .transparent(true)
    .decorations(false)
    .shadow(false)
    .always_on_top(s.character.always_on_top)
    .skip_taskbar(false)
    .resizable(false)
    .visible(true)
    .focused(true)
    .icon(icon)
    .map_err(|e| e.to_string())?
    .build()
    .map_err(|e| e.to_string())?;

    win.set_ignore_cursor_events(false).map_err(|e| e.to_string())?;

    if s.character.position.x < 0 || s.character.position.y < 0 {
        let mut x = st.settings.lock().map_err(|_| "settings lock".to_string())?;
        x.character.position = p;
        let d = app.path().app_data_dir().map_err(|e| e.to_string())?;
        let _ = x.save(&d);
    }

    Ok(())
}

pub fn destroy_character_window(app: &AppHandle) -> Result<(), String> {
    if let Some(w) = app.get_webview_window("character") {
        w.close().map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn show_character(app: &AppHandle) -> Result<(), String> {
    create_character_window(app)?;

    if let Some(st) = app.try_state::<AppState>() {
        if let Ok(mut s) = st.settings.lock() {
            s.character.visible = true;
            if let Ok(d) = app.path().app_data_dir() {
                let _ = s.save(&d);
            }
        }
    }

    Ok(())
}