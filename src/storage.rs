use rusqlite::{Connection, Result};

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
             );",
        )?;
        Ok(Self { connection })
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
}
