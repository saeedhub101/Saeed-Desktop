#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

slint::include_modules!();

mod app;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    app::run()
}
