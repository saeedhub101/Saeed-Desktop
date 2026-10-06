use std::{fs, path::Path};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub schema_version: u32,
    pub character: CharacterSettings,
    pub performance: PerformanceSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CharacterSettings {
    pub visible: bool,
    pub scale: CharacterScale,
    pub always_on_top: bool,
    pub current_id: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CharacterScale {
    Small,
    Medium,
    Large,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PerformanceSettings {
    pub low_power: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            schema_version: 1,
            character: CharacterSettings {
                visible: true,
                scale: CharacterScale::Medium,
                always_on_top: true,
                current_id: "default".into(),
            },
            performance: PerformanceSettings { low_power: false },
        }
    }
}

impl AppSettings {
    pub fn load(dir: &Path) -> Result<Self, String> {
        let path = dir.join("settings.json");

        if !path.exists() {
            return Ok(Self::default());
        }

        serde_json::from_str(
            &fs::read_to_string(path).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())
    }

    pub fn save(&self, dir: &Path) -> Result<(), String> {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;

        let path = dir.join("settings.json");
        let temp = dir.join("settings.json.tmp");

        fs::write(
            &temp,
            serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;

        // Windows requires a writable handle for FlushFileBuffers/sync_all.
        // Opening the temp file read-only causes ERROR_ACCESS_DENIED (os error 5)
        // and makes Tauri abort startup while saving settings.
        fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&temp)
            .map_err(|e| e.to_string())?
            .sync_all()
            .map_err(|e| e.to_string())?;

        // Replace the destination atomically. On Windows, removing the old
        // file first creates a window where settings.json does not exist.
        // MoveFileEx with REPLACE_EXISTING + WRITE_THROUGH provides the
        // required atomic replacement semantics for the settings file.
        #[cfg(windows)]
        {
            use std::os::windows::ffi::OsStrExt;
            use windows_sys::Win32::Storage::FileSystem::{
                MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
            };

            let from: Vec<u16> = temp.as_os_str().encode_wide().chain(Some(0)).collect();
            let to: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
            let ok = unsafe {
                MoveFileExW(
                    from.as_ptr(),
                    to.as_ptr(),
                    MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
                )
            };
            if ok == 0 {
                return Err(std::io::Error::last_os_error().to_string());
            }
            return Ok(());
        }

        #[cfg(not(windows))]
        fs::rename(temp, path).map_err(|e| e.to_string())
    }
}
