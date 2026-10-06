use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::Path,
    sync::Mutex,
};

const MAX_LOG_SIZE: u64 = 2 * 1024 * 1024;

pub struct Logger {
    file: Mutex<File>,
}

impl Logger {
    pub fn new(dir: &Path) -> Result<Self, String> {
        let log_dir = dir.join("logs");
        fs::create_dir_all(&log_dir).map_err(|e| e.to_string())?;

        let path = log_dir.join("saeed.log");

        if let Ok(metadata) = fs::metadata(&path) {
            if metadata.len() >= MAX_LOG_SIZE {
                let rotated = log_dir.join("saeed.log.1");

                if rotated.exists() {
                    let _ = fs::remove_file(&rotated);
                }

                fs::rename(&path, &rotated)
                    .map_err(|e| e.to_string())?;
            }
        }

        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(|e| e.to_string())?;

        Ok(Self {
            file: Mutex::new(file),
        })
    }

    pub fn info(&self, message: &str) {
        self.write("INFO", message);
    }

    pub fn error(&self, message: &str) {
        self.write("ERROR", message);
    }

    fn write(&self, level: &str, message: &str) {
        if let Ok(mut file) = self.file.lock() {
            let _ = writeln!(file, "{level} {message}");
            let _ = file.flush();
        }
    }
}
