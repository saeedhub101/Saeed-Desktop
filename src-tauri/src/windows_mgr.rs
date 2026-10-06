use tauri::{
    image::Image,
    runtime::dpi::{PhysicalPosition, PhysicalSize},
    AppHandle,
    Manager,
    WebviewUrl,
    WebviewWindowBuilder,
};

use crate::app::AppState;
use crate::settings::{CharacterScale, Position};

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

            return Position {
                x: work_area.position.x
                    + ((work_area.size.width.saturating_sub(width)) / 2) as i32,
                y: work_area.position.y
                    + work_area.size.height.saturating_sub(width + 12) as i32,
            };
        }
    }

    Position { x: 100, y: 100 }
}

pub fn create_character_window(app: &AppHandle) -> Result<(), String> {
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
    let position = if settings.character.position.x >= 0
        && settings.character.position.y >= 0
    {
        settings.character.position
    } else {
        default_position(app, width)
    };

    let icon = Image::from_app_icon_resource(32).map_err(|e| e.to_string())?;

    let window = WebviewWindowBuilder::new(
        app,
        "character",
        WebviewUrl::App("index.html".into()),
    )
    .title("Saeed")
    .inner_size(PhysicalSize::new(width, width))
    .position(PhysicalPosition::new(position.x, position.y))
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

    state.logger.info("Character window created");

    if settings.character.position.x < 0 || settings.character.position.y < 0 {
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

pub fn destroy_character_window(app: &AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("character") {
        window.destroy().map_err(|e| e.to_string())?;

        if let Some(state) = app.try_state::<AppState>() {
            state.logger.info("Character window destroyed");
        }
    }

    Ok(())
}

pub fn show_character(app: &AppHandle) -> Result<(), String> {
    create_character_window(app)?;

    if let Some(state) = app.try_state::<AppState>() {
        if let Ok(mut settings) = state.settings.lock() {
            settings.character.visible = true;

            if let Ok(dir) = app.path().app_data_dir() {
                let _ = settings.save(&dir);
            }

            state.logger.info("Character window shown");
        }
    }

    Ok(())
}
