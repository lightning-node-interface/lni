use rusqlite::{Connection, Result};
use std::sync::{Arc, Mutex};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DbError {
    #[error("SqlError: {reason}")]
    SqlErr { reason: String },
    #[error("ConnectionError: {reason}")]
    ConnectionErr { reason: String },
    #[error("QueryError: {reason}")]
    QueryErr { reason: String },
}

impl From<rusqlite::Error> for DbError {
    fn from(e: rusqlite::Error) -> Self {
        Self::SqlErr {
            reason: e.to_string(),
        }
    }
}

pub struct Db {
    conn: Arc<Mutex<Connection>>,
}

impl Db {
    pub fn new(path: String) -> Result<Self, DbError> {
        let conn = Connection::open(path).map_err(DbError::from)?;
        
        // Mozilla recommended pragmas (simplified)
        conn.execute_batch(
            r#"
            PRAGMA journal_mode=WAL;
            PRAGMA synchronous=NORMAL;
            PRAGMA foreign_keys=ON;
            "#,
        )
        .map_err(DbError::from)?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn create_user(&self, user_id: String, name: String) -> Result<(), DbError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "CREATE TABLE IF NOT EXISTS users (id TEXT PRIMARY KEY, name TEXT)",
            [],
        )
        .map_err(DbError::from)?;

        conn.execute(
            "INSERT OR REPLACE INTO users (id, name) VALUES (?1, ?2)",
            &[&user_id, &name],
        )
        .map_err(DbError::from)?;
        Ok(())
    }

    pub fn get_user(&self, user_id: String) -> Result<Option<String>, DbError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT name FROM users WHERE id = ?1")
            .map_err(DbError::from)?;

        let name_opt = stmt
            .query_row([&user_id], |row| row.get(0))
            .map_err(DbError::from)?;

        Ok(name_opt)
    }
}