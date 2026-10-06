use std::fs;
use std::path::PathBuf;

use keyring::Entry;
use rusqlite::{Connection, Result};

const KEYRING_SERVICE: &str = "Saeed-Desktop";
const OPENAI_KEY_USER: &str = "openai-api-key";

#[derive(Debug, Clone)]
pub struct AppSettings {
    pub openai_api_key: Option<String>,
    pub ai_model: String,
    pub stt_model: String,
    pub tts_model: String,
    pub tts_voice: String,
    pub character_paused: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            openai_api_key: None,
            ai_model: "gpt-6-luna".to_string(),
            stt_model: "gpt-4o-mini-transcribe".to_string(),
            tts_model: "gpt-4o-mini-tts".to_string(),
            tts_voice: "alloy".to_string(),
            character_paused: false,
        }
    }
}

pub struct Storage {
    connection: Connection,
}

impl Storage {
    pub fn open(path: &str) -> Result<Self> {
        let connection = Connection::open(path)?;
        connection.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA foreign_keys=ON;
             CREATE TABLE IF NOT EXISTS app_meta (
                 key TEXT PRIMARY KEY,
                 value TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS conversation_messages (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 role TEXT NOT NULL,
                 content TEXT NOT NULL,
                 source TEXT NOT NULL
             );",
        )?;
        Ok(Self { connection })
    }

    pub fn open_default() -> Result<Self> {
        let path = default_storage_path().map_err(|error| {
            rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::other(error)))
        })?;
        Self::open(path.to_string_lossy().as_ref())
    }

    pub fn set(&self, key: &str, value: &str) -> Result<()> {
        self.connection.execute(
            "INSERT INTO app_meta(key, value) VALUES(?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            (key, value),
        )?;
        Ok(())
    }

    pub fn get(&self, key: &str) -> Result<Option<String>> {
        let mut statement = self.connection.prepare(
            "SELECT value FROM app_meta WHERE key = ?1"
        )?;
        let mut rows = statement.query([key])?;
        Ok(rows.next()?.map(|row| row.get(0)).transpose()?)
    }

    pub fn load_settings(&self) -> Result<AppSettings> {
        let mut settings = AppSettings::default();
        if let Some(value) = self.get("ai_model")? {
            settings.ai_model = value;
        }
        if let Some(value) = self.get("stt_model")? {
            settings.stt_model = value;
        }
        if let Some(value) = self.get("tts_model")? {
            settings.tts_model = value;
        }
        if let Some(value) = self.get("tts_voice")? {
            settings.tts_voice = value;
        }
        if let Some(value) = self.get("character_paused")? {
            settings.character_paused = value == "true";
        }

        settings.openai_api_key = read_openai_api_key().map_err(|error| {
            rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::other(error)))
        })?;

        Ok(settings)
    }

    pub fn save_settings(&self, settings: &AppSettings) -> Result<()> {
        self.set("ai_model", &settings.ai_model)?;
        self.set("stt_model", &settings.stt_model)?;
        self.set("tts_model", &settings.tts_model)?;
        self.set("tts_voice", &settings.tts_voice)?;
        self.set("character_paused", if settings.character_paused { "true" } else { "false" })?;

        if let Some(api_key) = settings.openai_api_key.as_deref() {
            write_openai_api_key(api_key).map_err(|error| {
                rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::other(error)))
            })?;
        }

        Ok(())
    }

    pub fn load_messages(&self) -> Result<Vec<crate::session::Message>> {
        let mut statement = self.connection.prepare(
            "SELECT role, content, source FROM conversation_messages ORDER BY id ASC"
        )?;
        let rows = statement.query_map([], |row| {
            let role: String = row.get(0)?;
            let content: String = row.get(1)?;
            let source: String = row.get(2)?;
            let source = match source.as_str() {
                "chat" => crate::session::MessageSource::Chat,
                "voice" => crate::session::MessageSource::Voice,
                _ => crate::session::MessageSource::System,
            };
            Ok(crate::session::Message { role, content, source })
        })?;
        rows.collect()
    }

    pub fn append_message(&self, message: &crate::session::Message) -> Result<()> {
        let source = match message.source {
            crate::session::MessageSource::Chat => "chat",
            crate::session::MessageSource::Voice => "voice",
            crate::session::MessageSource::System => "system",
        };
        self.connection.execute(
            "INSERT INTO conversation_messages(role, content, source) VALUES(?1, ?2, ?3)",
            (&message.role, &message.content, source),
        )?;
        Ok(())
    }

    pub fn clear_conversation(&self) -> Result<()> {
        self.connection.execute("DELETE FROM conversation_messages", [])?;
        Ok(())
    }

    pub fn clear_openai_api_key(&self) -> Result<()> {
        delete_openai_api_key().map_err(|error| {
            rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::other(error)))
        })
    }
}

fn default_storage_path() -> std::result::Result<PathBuf, String> {
    let base = std::env::var_os("APPDATA")
        .ok_or_else(|| "Windows APPDATA is unavailable.".to_string())?;
    let directory = PathBuf::from(base).join("Saeed");
    fs::create_dir_all(&directory)
        .map_err(|error| format!("Could not create Saeed settings directory: {error}"))?;
    Ok(directory.join("settings.db"))
}

fn openai_key_entry() -> std::result::Result<Entry, String> {
    Entry::new(KEYRING_SERVICE, OPENAI_KEY_USER)
        .map_err(|error| format!("Windows Credential Manager is unavailable: {error}"))
}

fn read_openai_api_key() -> std::result::Result<Option<String>, String> {
    let entry = openai_key_entry()?;
    match entry.get_password() {
        Ok(value) => {
            let value = value.trim().to_string();
            if value.is_empty() {
                Ok(None)
            } else {
                Ok(Some(value))
            }
        }
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(error) => Err(format!("Could not read OpenAI API key: {error}")),
    }
}

fn write_openai_api_key(value: &str) -> std::result::Result<(), String> {
    let value = value.trim();
    if value.is_empty() {
        return Err("OpenAI API key cannot be empty.".to_string());
    }
    openai_key_entry()?
        .set_password(value)
        .map_err(|error| format!("Could not save OpenAI API key securely: {error}"))
}

fn delete_openai_api_key() -> std::result::Result<(), String> {
    let entry = openai_key_entry()?;
    match entry.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(error) => Err(format!("Could not remove OpenAI API key: {error}")),
    }
}

#[cfg(test)]
mod tests {
    use super::AppSettings;

    #[test]
    fn defaults_do_not_contain_a_secret() {
        let settings = AppSettings::default();
        assert!(settings.openai_api_key.is_none());
        assert_eq!(settings.tts_voice, "alloy");
        assert!(!settings.character_paused);
    }
}
