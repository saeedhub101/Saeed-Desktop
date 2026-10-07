use std::thread;

use rfd::FileDialog;
use tauri::{
    menu::{CheckMenuItemBuilder, MenuBuilder, MenuItemBuilder, SubmenuBuilder},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, Position, Size,
};

use crate::{
    app::{self, AppState},
    settings::CharacterScale,
};

fn state_snapshot(app: &AppHandle) -> (bool, CharacterScale, bool, bool) {
    let visible = app
        .get_webview_window("character")
        .and_then(|window| window.is_visible().ok())
        .unwrap_or(false);
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

    let show = MenuItemBuilder::with_id("show", "Show Saeed").enabled(!visible).build(app)?;
    let hide = MenuItemBuilder::with_id("hide", "Hide Saeed").enabled(visible).build(app)?;
    let change = MenuItemBuilder::with_id("change", "Change Character...").build(app)?;

    let small = CheckMenuItemBuilder::with_id("small", "Small")
        .checked(matches!(scale, CharacterScale::Small))
        .build(app)?;
    let medium = CheckMenuItemBuilder::with_id("medium", "Medium")
        .checked(matches!(scale, CharacterScale::Medium))
        .build(app)?;
    let large = CheckMenuItemBuilder::with_id("large", "Large")
        .checked(matches!(scale, CharacterScale::Large))
        .build(app)?;
    let sizes = SubmenuBuilder::new(app, "Character Size")
        .item(&small)
        .item(&medium)
        .item(&large)
        .build()?;

    let top = CheckMenuItemBuilder::with_id("top", "Always on Top").checked(always_on_top).build(app)?;
    let low = CheckMenuItemBuilder::with_id("low", "Low Power Mode").checked(low_power).build(app)?;
    let rotate = MenuItemBuilder::with_id("rotate", "Rotate once").build(app)?;
    let debug = SubmenuBuilder::new(app, "Debug").item(&rotate).build()?;
    let quit = MenuItemBuilder::with_id("quit", "Quit").build(app)?;

    MenuBuilder::new(app)
        .item(&show)
        .item(&hide)
        .item(&change)
        .item(&sizes)
        .item(&top)
        .item(&low)
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

    TrayIconBuilder::with_id("default")
        .icon(icon)
        .tooltip("Saeed")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "show" => {
                let _ = app::show_character_core(app);
                refresh(app);
            }
            "hide" => {
                let _ = app::hide_character_core(app.clone());
                refresh(app);
            }
            "change" => choose_character(app.clone()),
            "small" => set_scale(app, CharacterScale::Small),
            "medium" => set_scale(app, CharacterScale::Medium),
            "large" => set_scale(app, CharacterScale::Large),
            "top" => toggle_top(app),
            "low" => toggle_low(app),
            "rotate" => {
                let _ = app::debug_rotate_once_core(app.clone());
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                rect,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(state) = app.try_state::<AppState>() {
                    let (x, y) = position_xy(rect.position);
                    let (w, h) = size_wh(rect.size);
                    state.logger.info(&format!(
                        "Tray icon rect: x={} y={} width={} height={}",
                        x, y, w, h
                    ));
                }
                let visible = app
                    .get_webview_window("character")
                    .and_then(|window| window.is_visible().ok())
                    .unwrap_or(false);
                if visible {
                    let _ = app::hide_character_core(app.clone());
                } else {
                    let _ = app::show_character_core(app);
                }
                refresh(app);
            }
        })
        .build(app)?;

    // Windows may need a short interval to register the tray icon with the
    // shell. Retry instead of assuming rect() is populated immediately.
    let app_for_rect = app.clone();
    thread::spawn(move || {
        for _ in 0..20 {
            if let Some(tray) = app_for_rect.tray_by_id("default") {
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
            let _ = app::import_character_core(app.clone(), path.to_string_lossy().to_string(), app.state());
            refresh(&app);
        }
    });
}

fn set_scale(app: &AppHandle, scale: CharacterScale) {
    let _ = app::set_character_scale_core(app.clone(), scale, app.state());
    refresh(app);
}

fn toggle_top(app: &AppHandle) {
    let state = app.state::<AppState>();
    if let Ok(mut settings) = state.settings.lock() {
        settings.character.always_on_top = !settings.character.always_on_top;
        let value = settings.character.always_on_top;
        if let Ok(dir) = app::data_dir(app) {
            let _ = settings.save(&dir);
        }
        if let Some(window) = app.get_webview_window("character") {
            let _ = window.set_always_on_top(value);
        }
    }
    refresh(app);
}

fn toggle_low(app: &AppHandle) {
    let state = app.state::<AppState>();
    let enabled = state.settings.lock().map(|s| !s.performance.low_power).unwrap_or(false);
    let _ = app::set_low_power_core(app.clone(), enabled, app.state());
    refresh(app);
}

pub fn refresh(app: &AppHandle) {
    if let Ok(menu) = build_menu(app) {
        if let Some(tray) = app.tray_by_id("default") {
            let _ = tray.set_menu(Some(menu));
        }
    }
}
