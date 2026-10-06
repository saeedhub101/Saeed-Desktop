use std::{
    fs,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc, Mutex,
    },
    thread,
    time::Duration,
};

use tauri::{
    ipc::Response,
    AppHandle, Emitter, Manager, RunEvent, State, WebviewUrl, WebviewWindowBuilder,
};

use crate::{
    cursor_probe,
    logging::Logger,
    settings::{AppSettings, CharacterScale},
    tray,
};

pub struct AppState {
    pub settings: Mutex<AppSettings>,
    pub logger: Logger,
    pub character_destroying: AtomicBool,
    pub cleanup_ack: Mutex<Option<mpsc::Sender<()>>>,
    pub position_generation: AtomicU64,
    pub position_save_pending: AtomicBool,
    pub probe_generation: AtomicU64,
}

pub(crate) fn data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let base = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?;
    let dir = base
        .parent()
        .ok_or_else(|| "Unable to resolve APPDATA".to_string())?
        .join("Saeed");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    fs::create_dir_all(dir.join("characters")).map_err(|e| e.to_string())?;
    Ok(dir)
}

pub fn run() {
    let result = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            let _ = show_character_core(app);
        }))
        .setup(|app| {
            let dir = data_dir(app.handle())?;
            let logger = Logger::new(&dir)?;
            let settings = AppSettings::load(&dir).unwrap_or_default();
            settings.save(&dir)?;

            app.manage(AppState {
                settings: Mutex::new(settings.clone()),
                logger,
                character_destroying: AtomicBool::new(false),
                cleanup_ack: Mutex::new(None),
                position_generation: AtomicU64::new(0),
                position_save_pending: AtomicBool::new(false),
                probe_generation: AtomicU64::new(0),
            });

            app.state::<AppState>().logger.info("Core startup");
            tray::install(app.handle())?;

            if settings.character.visible {
                if let Err(error) = create_character_window(app.handle()) {
                    app.state::<AppState>()
                        .logger
                        .error(&format!("Character startup failed: {error}"));
                }
            }
            tray::refresh(app.handle());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_settings,
            get_character_model,
            set_character_scale,
            set_low_power,
            import_character,
            hide_character,
            show_character,
            debug_rotate_once,
            character_cleanup_done,
            log_error,
        ])
        .on_window_event(|window, event| {
            let app = window.app_handle();

            match event {
                tauri::WindowEvent::CloseRequested { api, .. } if window.label() == "character" => {
                    let state = app.state::<AppState>();
                    if state.character_destroying.load(Ordering::SeqCst) {
                        return;
                    }
                    api.prevent_close();
                    hide_character(app.clone());
                }
                tauri::WindowEvent::Moved(position) if window.label() == "character" => {
                    save_position_debounced(&app, position.x, position.y);
                }
                _ => {}
            }
        })
        .build(tauri::generate_context!());

    match result {
        Ok(app) => {
            app.run(|_, event| {
                if let RunEvent::ExitRequested { api, code, .. } = event {
                    if code.is_none() {
                        api.prevent_exit();
                    }
                }
            });
        }
        Err(error) => eprintln!("Saeed failed to start: {error}"),
    }
}

fn character_size(scale: CharacterScale) -> f64 {
    match scale {
        CharacterScale::Small => 280.0,
        CharacterScale::Medium => 360.0,
        CharacterScale::Large => 460.0,
    }
}

pub(crate) fn create_character_window(app: &AppHandle) -> Result<(), String> {
    if app.get_webview_window("character").is_some() {
        return Ok(());
    }

    let settings = app
        .state::<AppState>()
        .settings
        .lock()
        .map_err(|_| "settings lock".to_string())?
        .clone();

    let size = character_size(settings.character.scale);
    let mut builder = WebviewWindowBuilder::new(
        app,
        "character",
        WebviewUrl::App("index.html".into()),
    )
    .title("Saeed")
    .inner_size(size, size)
    .transparent(true)
    .decorations(false)
    .shadow(false)
    .always_on_top(settings.character.always_on_top)
    .skip_taskbar(true)
    .resizable(false)
    .visible(true);

    if settings.character.position.x >= 0 && settings.character.position.y >= 0 {
        builder = builder.position(
            settings.character.position.x as f64,
            settings.character.position.y as f64,
        );
    }

    let window = builder.build().map_err(|e| e.to_string())?;
    cursor_probe::start(app);
    let _ = window.set_focus();
    Ok(())
}

pub(crate) fn destroy_character_window(app: &AppHandle) {
    let Some(window) = app.get_webview_window("character") else {
        set_visible(app, false);
        tray::refresh(app);
        return;
    };

    let state = app.state::<AppState>();
    cursor_probe::stop(app);
    if state.character_destroying.swap(true, Ordering::SeqCst) {
        return;
    }

    let (tx, rx) = mpsc::channel();
    if let Ok(mut slot) = state.cleanup_ack.lock() {
        *slot = Some(tx);
    }

    let _ = window.emit("prepare-destroy", ());
    let app_for_thread = app.clone();
    thread::spawn(move || {
        let _ = rx.recv_timeout(Duration::from_millis(900));
        if let Some(window) = app_for_thread.get_webview_window("character") {
            let _ = window.destroy();
        }
        let state = app_for_thread.state::<AppState>();
        state.character_destroying.store(false, Ordering::SeqCst);
        set_visible(&app_for_thread, false);
        tray::refresh(&app_for_thread);
    });
}

pub(crate) fn show_character_core(app: &AppHandle) -> Result<(), String> {
    if app.get_webview_window("character").is_none() {
        create_character_window(app)?;
    }
    set_visible(app, true);
    tray::refresh(app);
    Ok(())
}

pub(crate) fn hide_character_core(app: AppHandle) {
    destroy_character_window(&app);
}

fn set_visible(app: &AppHandle, visible: bool) {
    if let Some(state) = app.try_state::<AppState>() {
        if let Ok(mut settings) = state.settings.lock() {
            settings.character.visible = visible;
            if let Ok(dir) = data_dir(app) {
                let _ = settings.save(&dir);
            }
        }
    }
}

fn save_position_debounced(app: &AppHandle, x: i32, y: i32) {
    let Some(state) = app.try_state::<AppState>() else { return };
    if let Ok(mut settings) = state.settings.lock() {
        settings.character.position.x = x;
        settings.character.position.y = y;
    }

    state.position_generation.fetch_add(1, Ordering::SeqCst);
    if state.position_save_pending.swap(true, Ordering::SeqCst) {
        return;
    }

    let app = app.clone();
    thread::spawn(move || {
        loop {
            thread::sleep(Duration::from_millis(350));
            let Some(state) = app.try_state::<AppState>() else { return };
            let generation = state.position_generation.load(Ordering::SeqCst);
            thread::sleep(Duration::from_millis(250));
            if state.position_generation.load(Ordering::SeqCst) != generation {
                continue;
            }
            if let Ok(settings) = state.settings.lock() {
                if let Ok(dir) = data_dir(&app) {
                    let _ = settings.save(&dir);
                }
            }
            state.position_save_pending.store(false, Ordering::SeqCst);
            break;
        }
    });
}

#[tauri::command]
fn get_settings(state: State<'_, AppState>) -> AppSettings {
    state.settings.lock().map(|s| s.clone()).unwrap_or_default()
}

#[tauri::command]
fn get_character_model(app: AppHandle, state: State<'_, AppState>) -> Result<Response, String> {
    let id = state
        .settings
        .lock()
        .map_err(|_| "settings lock".to_string())?
        .character
        .current_id
        .clone();

    let path = data_dir(&app)?.join("characters").join(id).join("model.glb");
    if !path.exists() {
        return Ok(Response::new(Vec::<u8>::new()));
    }
    Ok(Response::new(fs::read(path).map_err(|e| e.to_string())?))
}

#[tauri::command]
pub(crate) fn set_character_scale_core(
    app: AppHandle,
    scale: CharacterScale,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut settings = state.settings.lock().map_err(|_| "settings lock".to_string())?;
    settings.character.scale = scale;
    settings.save(&data_dir(&app)?)?;
    if let Some(window) = app.get_webview_window("character") {
        let size = character_size(scale);
        let _ = window.set_size(tauri::PhysicalSize::new(size as u32, size as u32));
        let _ = window.emit("character-refit", ());
    }
    tray::refresh(&app);
    Ok(())
}

#[tauri::command]
pub(crate) fn set_low_power_core(
    app: AppHandle,
    enabled: bool,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut settings = state.settings.lock().map_err(|_| "settings lock".to_string())?;
    settings.performance.low_power = enabled;
    settings.save(&data_dir(&app)?)?;
    drop(settings);
    if let Some(window) = app.get_webview_window("character") {
        let _ = window.emit("renderer-recreate", enabled);
    }
    tray::refresh(&app);
    Ok(())
}

#[tauri::command]
pub(crate) fn import_character_core(
    app: AppHandle,
    source: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let source_path = PathBuf::from(source);
    if source_path.extension().and_then(|e| e.to_str()).map(|e| e.eq_ignore_ascii_case("glb")) != Some(true) {
        return Err("Please select a .glb file.".into());
    }

    let stem = source_path.file_stem().and_then(|s| s.to_str()).unwrap_or("character");
    let safe: String = stem.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect();
    let id = format!("{}-{}", if safe.is_empty() { "character" } else { &safe }, std::process::id());

    let dir = data_dir(&app)?.join("characters").join(&id);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    fs::copy(source_path, dir.join("model.glb")).map_err(|e| e.to_string())?;

    {
        let mut settings = state.settings.lock().map_err(|_| "settings lock".to_string())?;
        settings.character.current_id = id.clone();
        settings.character.visible = true;
        settings.save(&data_dir(&app)?)?;
    }

    if app.get_webview_window("character").is_some() {
        let _ = app.emit("character-reload", ());
    } else {
        create_character_window(&app)?;
    }
    tray::refresh(&app);
    Ok(id)
}

#[tauri::command]
fn hide_character(app: AppHandle) -> Result<(), String> {
    destroy_character_window(&app);
    Ok(())
}

#[tauri::command]
fn show_character(app: AppHandle) -> Result<(), String> {
    show_character_core(&app)
}

#[tauri::command]
pub(crate) fn debug_rotate_once_core(app: AppHandle) -> Result<(), String> {
    debug_rotate_once_core(app)
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
fn log_error(state: State<'_, AppState>, message: String) {
    state.logger.error(&message);
}
