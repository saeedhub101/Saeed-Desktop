#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod app;mod logging;mod icon;mod settings;mod tray;mod windows_mgr;
fn main(){app::run()}