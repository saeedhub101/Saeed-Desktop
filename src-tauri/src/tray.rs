use std::thread;

use rfd::FileDialog;
use tauri::{
    menu::{CheckMenuItemBuilder, MenuBuilder, MenuItemBuilder, SubmenuBuilder},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, Position, Size,
};

use crate::{
    app::{self, AppState},
    settings::CharacterScale,
};

const TRAY_ID: &str = "default";
const MENU_SHOW: &str = "tray_show";
const MENU_HIDE: &str = "tray_hide";
const MENU_CHANGE: &str = "tray_change";
const MENU_SMALL: &str = "tray_size_small";
const MENU_MEDIUM: &str = "tray_size_medium";
const MENU_LARGE: &str = "tray_size_large";
const MENU_TOP: &str = "tray_always_on_top";
const MENU_LOW: &str = "tray_low_power";
const MENU_CLICK_THROUGH: &str = "tray_click_through";
const MENU_ROTATE: &str = "tray_rotate_once";
const MENU_QUIT: &str = "tray_quit";

fn character_is_visible(app: &AppHandle) -> bool {
    if let Some(state) = app.try_state::<AppState>() {
        if state.character_destroying.load(std::sync::atomic::Ordering::SeqCst) {
            return false;
        }
    }
    app.get_webview_window("character")
        .and_then(|window| window.is_visible().ok())
        .unwrap_or(false)
}

fn state_snapshot(app: &AppHandle) -> (bool, CharacterScale, bool, bool) {
    let visible = character_is_visible(app);
    if let Some(state) = app.try_state::<AppState>() {
        if let Ok(settings) = state.settings.lock() {
            return (
                visible,
                settings.character.scale,
                settings.character.always_on_top,
                settings.performance.low_power,
            );
        }
    }
    (visible, CharacterScale::Medium, true, false)
}

fn build_menu(app: &AppHandle) -> tauri::Result<tauri::menu::Menu<tauri::Wry>> {
    let (visible, scale, always_on_top, low_power) = state_snapshot(app);
    let click_through = app.try_state::<AppState>()
        .and_then(|state| state.settings.lock().ok().map(|settings| settings.character.click_through))
        .unwrap_or(true);

    let show = MenuItemBuilder::with_id(MENU_SHOW, "Show Saeed").enabled(!visible).build(app)?;
    let hide = MenuItemBuilder::with_id(MENU_HIDE, "Hide Saeed").enabled(visible).build(app)?;
    let change = MenuItemBuilder::with_id(MENU_CHANGE, "Change Character...").build(app)?;

    let small = CheckMenuItemBuilder::with_id(MENU_SMALL, "Small")
        .checked(matches!(scale, CharacterScale::Small))
        .build(app)?;
    let medium = CheckMenuItemBuilder::with_id(MENU_MEDIUM, "Medium")
        .checked(matches!(scale, CharacterScale::Medium))
        .build(app)?;
    let large = CheckMenuItemBuilder::with_id(MENU_LARGE, "Large")
        .checked(matches!(scale, CharacterScale::Large))
        .build(app)?;
    let sizes = SubmenuBuilder::new(app, "Character Size")
        .item(&small)
        .item(&medium)
        .item(&large)
        .build()?;

    let top = CheckMenuItemBuilder::with_id(MENU_TOP, "Always on Top").checked(always_on_top).build(app)?;
    let low = CheckMenuItemBuilder::with_id(MENU_LOW, "Low Power Mode").checked(low_power).build(app)?;
    let click = CheckMenuItemBuilder::with_id(MENU_CLICK_THROUGH, "Click-through").checked(click_through).build(app)?;
    let rotate = MenuItemBuilder::with_id(MENU_ROTATE, "Rotate once").build(app)?;
    let debug = SubmenuBuilder::new(app, "Debug").item(&rotate).build()?;
    let quit = MenuItemBuilder::with_id(MENU_QUIT, "Quit").build(app)?;

    MenuBuilder::new(app)
        .item(&show)
        .item(&hide)
        .item(&change)
        .item(&sizes)
        .item(&top)
        .item(&low)
        .item(&click)
        .item(&debug)
        .item(&quit)
        .build()
}

fn position_xy(p: Position) -> (f64, f64) {
    match p {
        Position::Physical(p) => (p.x as f64, p.y as f64),
        Position::Logical(p) => (p.x, p.y),
    }
}

fn size_wh(s: Size) -> (f64, f64) {
    match s {
        Size::Physical(s) => (s.width as f64, s.height as f64),
        Size::Logical(s) => (s.width, s.height),
    }
}

pub fn install(app: &AppHandle) -> tauri::Result<()> {
    let menu = build_menu(app)?;
    let icon = tauri::include_image!("./icons/32x32.png");

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon)
        .tooltip("Saeed")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| {
            let id = event.id().as_ref();
            if let Some(state) = app.try_state::<AppState>() {
                state.logger.info(&format!("Tray menu event received: id={}", id));
            }
            match id {
            MENU_SHOW => {
                match app::show_character_core(app) {
                    Ok(_) => refresh(app),
                    Err(err) => app.state::<AppState>().logger.error(&format!("Tray show failed: {}", err)),
                }
            }
            MENU_HIDE => {
                match app::hide_character_core(app.clone()) {
                    Ok(_) => refresh(app),
                    Err(err) => app.state::<AppState>().logger.error(&format!("Tray hide failed: {}", err)),
                }
            }
            MENU_CHANGE => choose_character(app.clone()),
            MENU_SMALL => set_scale(app, CharacterScale::Small),
            MENU_MEDIUM => set_scale(app, CharacterScale::Medium),
            MENU_LARGE => set_scale(app, CharacterScale::Large),
            MENU_TOP => toggle_top(app),
            MENU_LOW => toggle_low(app),
            MENU_CLICK_THROUGH => toggle_click_through(app),
            MENU_ROTATE => {
                match app::debug_rotate_once_core(app.clone()) {
                    Ok(_) => {}
                    Err(err) => app.state::<AppState>().logger.error(&format!("Tray rotate failed: {}", err)),
                }
            }
            MENU_QUIT => {
                if let Some(state) = app.try_state::<AppState>() {
                    state.logger.info("Tray Quit command received");
                }
                app.exit(0);
            }
            _ => {
                if let Some(state) = app.try_state::<AppState>() {
                    state.logger.error(&format!("Unhandled tray menu event: id={}", id));
                }
            }
            }
        })
        .on_tray_icon_event(|tray, event| {
            let app = tray.app_handle();

            if let TrayIconEvent::Click {
                button,
                button_state: MouseButtonState::Up,
                rect,
                ..
            } = event
            {
                if let Some(state) = app.try_state::<AppState>() {
                    let button_name = match button {
                        MouseButton::Left => "left",
                        MouseButton::Right => "right",
                        MouseButton::Middle => "middle",
                    };
                    state.logger.info(&format!("Tray click event: button={}", button_name));
                    let (x, y) = position_xy(rect.position);
                    let (w, h) = size_wh(rect.size);
                    state.logger.info(&format!(
                        "Tray icon rect: x={} y={} width={} height={}",
                        x, y, w, h
                    ));
                }

                match button {
                    MouseButton::Right => {
                        if let Some(tray_icon) = app.tray_by_id(TRAY_ID) {
                            match tray_icon.with_inner_tray_icon(|inner| inner.show_menu()) {
                                Ok(Ok(())) => {
                                    if let Some(state) = app.try_state::<AppState>() {
                                        state.logger.info("Tray context menu explicitly shown");
                                    }
                                }
                                Ok(Err(err)) | Err(err) => {
                                    if let Some(state) = app.try_state::<AppState>() {
                                        state.logger.error(&format!("Tray context menu show failed: {}", err));
                                    }
                                }
                            }
                        }
                    }
                    MouseButton::Left => {
                        let visible = character_is_visible(app);
                        if visible {
                            let _ = app::hide_character_core(app.clone());
                        } else {
                            let _ = app::show_character_core(app);
                        }
                        refresh(app);
                    }
                    MouseButton::Middle => {}
                }
            }
        })
        .build(app)?;

    // Windows may need a short interval to register the tray icon with the
    // shell. Retry instead of assuming rect() is populated immediately.
    let app_for_rect = app.clone();
    thread::spawn(move || {
        for _ in 0..20 {
            if let Some(tray) = app_for_rect.tray_by_id(TRAY_ID) {
                if let Ok(Some(rect)) = tray.rect() {
                    let (x, y) = position_xy(rect.position);
                    let (w, h) = size_wh(rect.size);
                    if let Some(state) = app_for_rect.try_state::<AppState>() {
                        state.logger.info(&format!(
                            "Tray icon rect: x={} y={} width={} height={}",
                            x, y, w, h
                        ));
                    }
                    break;
                }
            }
            thread::sleep(std::time::Duration::from_millis(250));
        }
    });

    Ok(())
}

fn choose_character(app: AppHandle) {
    thread::spawn(move || {
        if let Some(path) = FileDialog::new().add_filter("GLB model", &["glb"]).pick_file() {
            match app::import_character_core(app.clone(), path.to_string_lossy().to_string(), app.state()) {
                Ok(_) => refresh(&app),
                Err(err) => app.state::<AppState>().logger.error(&format!("Tray Change Character failed: {}", err)),
            }
        } else if let Some(state) = app.try_state::<AppState>() {
            state.logger.info("Tray Change Character dialog cancelled");
        }
    });
}

fn set_scale(app: &AppHandle, scale: CharacterScale) {
    match app::set_character_scale_core(app.clone(), scale, app.state()) {
        Ok(_) => refresh(app),
        Err(err) => app.state::<AppState>().logger.error(&format!("Tray set scale {:?} failed: {}", scale, err)),
    }
}

fn toggle_top(app: &AppHandle) {
    let state = app.state::<AppState>();
    if let Ok(mut settings) = state.settings.lock() {
        settings.character.always_on_top = !settings.character.always_on_top;
        let value = settings.character.always_on_top;
        if let Ok(dir) = app::data_dir(app) {
            if let Err(err) = settings.save(&dir) {
                state.logger.error(&format!("Tray Always on Top save failed: {}", err));
            }
        }
        drop(settings);
        if let Some(window) = app.get_webview_window("character") {
            if let Err(err) = window.set_always_on_top(value) {
                state.logger.error(&format!("Tray Always on Top window update failed: {}", err));
            }
        }
    }
    refresh(app);
}

fn toggle_low(app: &AppHandle) {
    let state = app.state::<AppState>();
    let enabled = state.settings.lock().map(|s| !s.performance.low_power).unwrap_or(false);
    if let Err(err) = app::set_low_power_core(app.clone(), enabled, app.state()) {
        state.logger.error(&format!("Tray Low Power Mode failed: {}", err));
    } else {
        refresh(app);
    }
}

fn toggle_click_through(app: &AppHandle) {
    let state = app.state::<AppState>();
    let enabled = state.settings.lock().map(|s| !s.character.click_through).unwrap_or(true);
    if let Ok(mut settings) = state.settings.lock() {
        settings.character.click_through = enabled;
        if let Ok(dir) = app::data_dir(app) {
            if let Err(err) = settings.save(&dir) {
                state.logger.error(&format!("Tray Click-through save failed: {}", err));
            }
        }
    }
    if let Some(window) = app.get_webview_window("character") {
        let interaction_active = state.interaction_active.load(std::sync::atomic::Ordering::SeqCst);
        let effective_ignore = enabled && !interaction_active;
        if let Err(err) = window.set_ignore_cursor_events(effective_ignore) {
            state.logger.error(&format!("Tray Click-through window update failed: {}", err));
        }
        if let Err(err) = window.emit("click-through-changed", enabled) {
            state.logger.error(&format!("Tray Click-through event failed: {}", err));
        }
    }
    refresh(app);
}

pub fn refresh(app: &AppHandle) {
    if let Ok(menu) = build_menu(app) {
        if let Some(tray) = app.tray_by_id(TRAY_ID) {
            if let Err(err) = tray.set_menu(Some(menu)) {
                if let Some(state) = app.try_state::<AppState>() {
                    state.logger.error(&format!("Tray menu refresh failed: {}", err));
                }
            } else if let Some(state) = app.try_state::<AppState>() {
                state.logger.info("Tray menu refreshed");
            }
        } else if let Some(state) = app.try_state::<AppState>() {
            state.logger.error("Tray menu refresh failed: tray not found");
        }
    } else if let Some(state) = app.try_state::<AppState>() {
        state.logger.error("Tray menu rebuild failed");
    }
}
