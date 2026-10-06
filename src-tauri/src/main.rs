#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod cursor_probe;
mod logging;
mod settings;
mod tray;
mod windows_mgr;

fn main() {
    app::run();
}