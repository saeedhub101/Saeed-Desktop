use std::thread;

use rfd::FileDialog;
use tauri::{
    image::Image,
    menu::{
        CheckMenuItemBuilder, MenuBuilder, MenuItemBuilder, SubmenuBuilder,
    },
    PhysicalSize,
    tray::{
        MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent,
    },
    AppHandle, Emitter, Manager,
};

use crate::{app::AppState, settings::CharacterScale, windows_mgr};

fn snapshot(
    app: &AppHandle,
) -> (bool, CharacterScale, bool, bool) {
    if let Some(state) = app.try_state::<AppState>() {
        if let Ok(settings) = state.settings.lock() {
            return (
                app.get_webview_window("character").is_some(),
                settings.character.scale,
                settings.character.always_on_top,
                settings.performance.low_power,
            );
        }
    }

    (
        false,
        CharacterScale::Medium,
        true,
        false,
    )
}

fn menu(app: &AppHandle) -> tauri::Result<tauri::menu::Menu<tauri::Wry>> {
    let (visible, scale, always_on_top, low_power) = snapshot(app);

    let show = MenuItemBuilder::with_id("show", "Show Saeed")
        .enabled(!visible)
        .build(app)?;
    let hide = MenuItemBuilder::with_id("hide", "Hide Saeed")
        .enabled(visible)
        .build(app)?;
    let change =
        MenuItemBuilder::with_id("change", "Change Character...").build(app)?;

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

    let always_on_top_item =
        CheckMenuItemBuilder::with_id("top", "Always on Top")
            .checked(always_on_top)
            .build(app)?;

    let low_power_item =
        CheckMenuItemBuilder::with_id("low", "Low Power Mode")
            .checked(low_power)
            .build(app)?;

    let rotate =
        MenuItemBuilder::with_id("rotate", "Rotate once").build(app)?;
    let debug = SubmenuBuilder::new(app, "Debug")
        .item(&rotate)
        .build()?;

    let quit = MenuItemBuilder::with_id("quit", "Quit").build(app)?;

    MenuBuilder::new(app)
        .item(&show)
        .item(&hide)
        .item(&change)
        .item(&sizes)
        .item(&always_on_top_item)
        .item(&low_power_item)
        .item(&debug)
        .item(&quit)
        .build()
}

pub fn install(app: &AppHandle) -> tauri::Result<()> {
    let menu = menu(app)?;
    let icon = Image::from_app_icon_resource(32)?;

    TrayIconBuilder::with_id("default")
        .icon(icon)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "show" => {
                let _ = windows_mgr::show_character(app);
                refresh(app);
            }
            "hide" => {
                // The tray menu is refreshed once the window is really gone.
                let _ = crate::app::tray_hide_character(app.clone());
            }
            "change" => choose(app.clone()),
            "small" => scale(app, CharacterScale::Small),
            "medium" => scale(app, CharacterScale::Medium),
            "large" => scale(app, CharacterScale::Large),
            "top" => {
                let state = app.state::<AppState>();

                if let Ok(mut settings) = state.settings.lock() {
                    settings.character.always_on_top =
                        !settings.character.always_on_top;

                    if let Ok(dir) = crate::app::data_dir(app) {
                        let _ = settings.save(&dir);
                    }

                    if let Some(window) =
                        app.get_webview_window("character")
                    {
                        let _ = window.set_always_on_top(
                            settings.character.always_on_top,
                        );
                    }
                }

                refresh(app);
            }
            "low" => {
                let state = app.state::<AppState>();

                if let Ok(settings) = state.settings.lock() {
                    let enabled = !settings.performance.low_power;
                    drop(settings);

                    let _ = crate::app::set_low_power(
                        app.clone(),
                        enabled,
                        app.state(),
                    );
                }

                refresh(app);
            }
            "rotate" => {
                let _ =
                    crate::app::tray_debug_rotate_once(app.clone());
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();

                if app
                    .get_webview_window("character")
                    .is_some()
                {
                    let _ = crate::app::tray_hide_character(app.clone());
                } else {
                    let _ = windows_mgr::show_character(app);
                    refresh(app);
                }
            }
        })
        .build(app)?;

    Ok(())
}

fn choose(app: AppHandle) {
    let _ = thread::spawn(move || {
        if let Some(path) = FileDialog::new()
            .add_filter("GLB model", &["glb"])
            .pick_file()
        {
            let _ = crate::app::tray_import_character(
                app.clone(),
                path.to_string_lossy().to_string(),
                app.state(),
            );

            let _ = windows_mgr::create_character_window(&app);
            refresh(&app);
        }
    });
}

fn scale(app: &AppHandle, scale: CharacterScale) {
    let _ = crate::app::tray_set_character_scale(
        app.clone(),
        scale,
        app.state(),
    );

    if let Some(window) =
        app.get_webview_window("character")
    {
        let size = match scale {
            CharacterScale::Small => 280,
            CharacterScale::Medium => 360,
            CharacterScale::Large => 460,
        };

        let _ = window.set_size(PhysicalSize::new(size, size));
        let _ = app.emit("character-reload", ());
    }

    refresh(app);
}

pub fn refresh(app: &AppHandle) {
    if let Ok(menu) = menu(app) {
        if let Some(tray) = app.tray_by_id("default") {
            let _ = tray.set_menu(Some(menu));
        }
    }
}