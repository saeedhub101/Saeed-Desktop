use std::{
    fs,
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex,
    },
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tauri::{
    ipc::Response,
    AppHandle,
    Emitter,
    Manager,
    RunEvent,
    State,
};

use crate::{
    logging::Logger,
    settings::{AppSettings, CharacterScale},
    tray,
    windows_mgr,
};

pub struct AppState {
    pub settings: Mutex<AppSettings>,
    pub logger: Logger,
    pub position_generation: AtomicU64,
}

pub(crate) fn data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let old_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let parent = old_dir
        .parent()
        .ok_or_else(|| "Unable to resolve APPDATA directory".to_string())?;
    let new_dir = parent.join("Saeed");

    if old_dir != new_dir && old_dir.exists() && !new_dir.exists() {
        fs::rename(&old_dir, &new_dir)
            .map_err(|e| format!("Unable to migrate application data: {e}"))?;
    }

    fs::create_dir_all(&new_dir).map_err(|e| e.to_string())?;
    Ok(new_dir)
}

pub fn run() {
    let result = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            let _ = windows_mgr::show_character(app);
        }))
        .setup(|app| {
            let data_dir = data_dir(&app.handle())?;
            fs::create_dir_all(&data_dir)?;

            let logger = Logger::new(&data_dir)?;
            logger.info("Application startup");

            let settings = AppSettings::load(&data_dir).unwrap_or_default();
            settings.save(&data_dir)?;

            app.manage(AppState {
                settings: Mutex::new(settings.clone()),
                logger,
                position_generation: AtomicU64::new(0),
            });

            tray::install(&app.handle())?;

            if settings.character.visible {
                windows_mgr::create_character_window(&app.handle())?;
            }

            let cursor_app = app.handle().clone();
            thread::spawn(move || loop {
                if let Some(window) = cursor_app.get_webview_window("character") {
                    if let (Ok(cursor), Ok(position), Ok(size)) = (
                        window.cursor_position(),
                        window.inner_position(),
                        window.inner_size(),
                    ) {
                        let x = cursor.x - f64::from(position.x);
                        let y = cursor.y - f64::from(position.y);

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

                thread::sleep(Duration::from_millis(50));
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_settings,
            get_character_model,
            set_character_scale,
            set_low_power,
            import_character,
            debug_rotate_once,
            hide_character,
            show_character,
            log_error
        ])
        .on_window_event(|window, event| {
            match event {
                tauri::WindowEvent::CloseRequested { .. } => {
                    let app = window.app_handle();
                    if let Some(state) = app.try_state::<AppState>() {
                        if let Ok(mut settings) = state.settings.lock() {
                            settings.character.visible = false;
                            if let Ok(dir) = data_dir(&app) {
                                let _ = settings.save(&dir);
                            }
                            state.logger.info("Character window hidden by user close");
                        }
                    }

                    let _ = windows_mgr::destroy_character_window(&app);
                    tray::refresh(&app);
                }
                tauri::WindowEvent::Moved(position) => {
                    if window.label() != "character" {
                        return;
                    }

                    let app = window.app_handle();
                    let Some(state) = app.try_state::<AppState>() else {
                        return;
                    };

                    if let Ok(mut settings) = state.settings.lock() {
                        settings.character.position.x = position.x;
                        settings.character.position.y = position.y;
                    }

                    let generation =
                        state.position_generation.fetch_add(1, Ordering::SeqCst) + 1;
                    let app_for_save = app.clone();

                    thread::spawn(move || {
                        thread::sleep(Duration::from_millis(500));

                        let Some(state) = app_for_save.try_state::<AppState>() else {
                            return;
                        };

                        if state.position_generation.load(Ordering::SeqCst) != generation {
                            return;
                        }

                        if let Ok(settings) = state.settings.lock() {
                            if let Ok(dir) = data_dir(&app_for_save) {
                                let _ = settings.save(&dir);
                                state.logger.info("Character position saved");
                            }
                        }
                    });
                }
                _ => {}
            }
        })
        .build(tauri::generate_context!())
        .map_err(|e| e.to_string());

    match result {
        Ok(app) => {
            app.run(|_app, event| {
                if let RunEvent::ExitRequested { api, code, .. } = event {
                    if code.is_none() {
                        api.prevent_exit();
                    }
                }
            });
        }
        Err(error) => {
            eprintln!("Saeed failed to start: {error}");
        }
    }
}

#[tauri::command]
fn log_error(state: State<'_, AppState>, message: String) {
    state.logger.error(&message);
}

#[tauri::command]
fn get_settings(state: State<'_, AppState>) -> AppSettings {
    state.settings.lock().unwrap().clone()
}

#[tauri::command]
fn get_character_model(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Option<Response>, String> {
    let settings = state
        .settings
        .lock()
        .map_err(|_| "settings lock".to_string())?
        .clone();

    let path = data_dir(&app)?
        .join("characters")
        .join(&settings.character.current_id)
        .join("model.glb");

    if !path.exists() {
        return Ok(None);
    }

    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    Ok(Some(Response::new(bytes)))
}

#[tauri::command]
fn set_character_scale(
    app: AppHandle,
    scale: CharacterScale,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let dir = data_dir(&app)?;
    let mut settings = state
        .settings
        .lock()
        .map_err(|_| "settings lock".to_string())?;

    settings.character.scale = scale;
    settings.save(&dir)
}

#[tauri::command]
pub(crate) fn set_low_power(
    app: AppHandle,
    enabled: bool,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let dir = data_dir(&app)?;
    let mut settings = state
        .settings
        .lock()
        .map_err(|_| "settings lock".to_string())?;

    settings.performance.low_power = enabled;
    settings.save(&dir)?;
    state.logger.info(&format!("Low Power Mode set to {enabled}"));

    if let Some(window) = app.get_webview_window("character") {
        window
            .emit("renderer-recreate", enabled)
            .map_err(|e| e.to_string())?;
    }

    tray::refresh(&app);
    Ok(())
}

#[tauri::command]
fn import_character(
    app: AppHandle,
    source: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let source_path = PathBuf::from(source);

    if source_path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.eq_ignore_ascii_case("glb"))
        != Some(true)
    {
        return Err("Please select a .glb file.".into());
    }

    let dir = data_dir(&app)?;
    let stem = source_path
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("character");

    let safe: String = stem
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' || character == '_' {
                character
            } else {
                '_'
            }
        })
        .collect();

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_millis())
        .unwrap_or(0);

    let id = format!(
        "{}-{timestamp}",
        if safe.is_empty() {
            "character"
        } else {
            &safe
        }
    );

    let character_dir = dir.join("characters").join(&id);
    fs::create_dir_all(&character_dir).map_err(|e| e.to_string())?;
    fs::copy(source_path, character_dir.join("model.glb")).map_err(|e| e.to_string())?;

    let mut settings = state
        .settings
        .lock()
        .map_err(|_| "settings lock".to_string())?;

    settings.character.current_id = id.clone();
    settings.character.visible = true;
    settings.save(&dir)?;

    app.emit("character-reload", ())
        .map_err(|e| e.to_string())?;

    Ok(id)
}

#[tauri::command]
fn debug_rotate_once(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("character") {
        window
            .emit("debug-rotate-once", ())
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

#[tauri::command]
fn hide_character(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    windows_mgr::destroy_character_window(&app)?;

    let dir = data_dir(&app)?;
    let mut settings = state
        .settings
        .lock()
        .map_err(|_| "settings lock".to_string())?;

    settings.character.visible = false;
    settings.save(&dir)?;
    state.logger.info("Character window hidden");
    tray::refresh(&app);

    Ok(())
}

#[tauri::command]
fn show_character(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    windows_mgr::create_character_window(&app)?;

    let dir = data_dir(&app)?;
    let mut settings = state
        .settings
        .lock()
        .map_err(|_| "settings lock".to_string())?;

    settings.character.visible = true;
    settings.save(&dir)?;
    state.logger.info("Character window shown");
    tray::refresh(&app);

    Ok(())
}

pub fn tray_set_character_scale(
    app: AppHandle,
    scale: CharacterScale,
    state: State<'_, AppState>,
) -> Result<(), String> {
    set_character_scale(app, scale, state)
}

pub fn tray_import_character(
    app: AppHandle,
    source: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    import_character(app, source, state)
}

pub fn tray_hide_character(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    hide_character(app, state)
}

pub fn tray_debug_rotate_once(app: AppHandle) -> Result<(), String> {
    debug_rotate_once(app)
}
