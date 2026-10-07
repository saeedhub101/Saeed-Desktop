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
    let visible = app.get_webview_window("character").is_some();
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
