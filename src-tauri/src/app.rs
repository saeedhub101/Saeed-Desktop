use std::{
    fs,
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc, Mutex,
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
    windows_mgr::HideState,
};

pub struct AppState {
    pub settings: Mutex<AppSettings>,
    pub logger: Logger,
    pub position_generation: AtomicU64,
    pub position_save_scheduled: std::sync::atomic::AtomicBool,
    /// Sender the renderer's "cleanup done" acknowledgement is delivered through
    /// while a hide handshake is in progress.
    pub cleanup_ack: Mutex<Option<mpsc::Sender<()>>>,
    /// Generation id of the cursor probe thread that is allowed to run.
    pub probe_generation: AtomicU64,
    /// Tracks an in-flight hide so Show during a Hide is not lost.
    pub hide_state: Mutex<HideState>,
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
            let data_dir = data_dir(app.handle())?;
            fs::create_dir_all(&data_dir)?;

            let logger = Logger::new(&data_dir)?;
            logger.info("Application startup");

            let settings = AppSettings::load(&data_dir).unwrap_or_default();
            settings.save(&data_dir)?;

            app.manage(AppState {
                settings: Mutex::new(settings.clone()),
                logger,
                position_generation: AtomicU64::new(0),
                position_save_scheduled: std::sync::atomic::AtomicBool::new(false),
                cleanup_ack: Mutex::new(None),
                probe_generation: AtomicU64::new(0),
                hide_state: Mutex::new(HideState::Idle),
            });

            tray::install(app.handle())?;

            if settings.character.visible {
                if let Err(error) = windows_mgr::create_character_window(app.handle()) {
                    app.state::<AppState>()
                        .logger
                        .error(&format!("Character window startup failed: {error}"));
                }
            }

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
            character_cleanup_done,
            log_error
        ])
        .on_window_event(|window, event| {
            match event {
                tauri::WindowEvent::CloseRequested { api, .. } => {
                    if window.label() != "character" {
                        return;
                    }

                    // Never let the OS tear the window down directly: run the
                    // renderer cleanup handshake first, then destroy.
                    api.prevent_close();
                    windows_mgr::hide_character(window.app_handle());
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

                    state.position_generation.fetch_add(1, Ordering::SeqCst);

                    if !state.position_save_scheduled.swap(true, Ordering::SeqCst) {
                        let app_for_save = app.clone();

                        thread::spawn(move || loop {
                            thread::sleep(Duration::from_millis(500));

                            let Some(state) = app_for_save.try_state::<AppState>() else {
                                return;
                            };

                            let generation =
                                state.position_generation.load(Ordering::SeqCst);

                            thread::sleep(Duration::from_millis(500));

                            if state.position_generation.load(Ordering::SeqCst) != generation {
                                continue;
                            }

                            if let Ok(settings) = state.settings.lock() {
                                if let Ok(dir) = data_dir(&app_for_save) {
                                    let _ = settings.save(&dir);
                                    state.logger.info("Character position saved");
                                }
                            }

                            state.position_save_scheduled.store(false, Ordering::SeqCst);

                            if state.position_generation.load(Ordering::SeqCst) != generation {
                                if state.position_save_scheduled.swap(true, Ordering::SeqCst) {
                                    break;
                                }
                                continue;
                            }

                            break;
                        });
                    }
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
) -> Result<Response, String> {
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
        return Ok(Response::new(Vec::<u8>::new()));
    }

    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    Ok(Response::new(bytes))
}

#[tauri::command]
pub(crate) fn set_character_scale(
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
    drop(settings);
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
pub(crate) fn import_character(
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
pub(crate) fn debug_rotate_once(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("character") {
        window
            .emit("debug-rotate-once", ())
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

#[tauri::command]
fn hide_character(app: AppHandle) -> Result<(), String> {
    windows_mgr::hide_character(&app);
    Ok(())
}

#[tauri::command]
fn character_cleanup_done(state: State<'_, AppState>) {
    if let Ok(mut slot) = state.cleanup_ack.lock() {
        if let Some(sender) = slot.take() {
            let _ = sender.send(());
        }
    }
}

#[tauri::command]
fn show_character(app: AppHandle) -> Result<(), String> {
    windows_mgr::show_character(&app)
}

