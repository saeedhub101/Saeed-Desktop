#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod cursor_probe;
mod logging;
mod settings;
mod tray;
mod windows_mgr;

fn main() {
    std::panic::set_hook(Box::new(|info| {
        let root = std::env::var_os("APPDATA")
            .map(std::path::PathBuf::from)
            .map(|p| p.join("Saeed").join("logs").join("panic.log"));

        if let Some(path) = root {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }

            let message = format!(
                "PANIC: {info}\nBACKTRACE:\n{}\n",
                std::backtrace::Backtrace::force_capture()
            );

            let _ = std::fs::write(path, message);
        }
    }));

    app::run();
}
