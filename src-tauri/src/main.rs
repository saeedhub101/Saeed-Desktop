#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod conversation;
mod cursor_probe;
mod logging;
mod profile;
mod settings;
mod tray;

fn main() {
    std::panic::set_hook(Box::new(|info| {
        if let Some(root) = std::env::var_os("APPDATA")
            .map(std::path::PathBuf::from)
            .map(|p| p.join("Saeed").join("logs").join("panic.log"))
        {
            if let Some(parent) = root.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(
                root,
                format!("PANIC: {info}\nBACKTRACE:\n{}\n", std::backtrace::Backtrace::force_capture()),
            );
        }
    }));

    app::run();
}
