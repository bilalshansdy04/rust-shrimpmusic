pub mod models;
pub mod repository;
pub mod schema;

use std::path::{Path, PathBuf};
use directories::ProjectDirs;
use rusqlite::Connection;

pub use models::*;
pub use repository::Database;

pub fn default_db_path() -> PathBuf {
    if let Some(proj_dirs) = ProjectDirs::from("com", "ShrimpMusic", "ShrimpMusic") {
        let dir = proj_dirs.data_local_dir();
        std::fs::create_dir_all(dir).ok();
        dir.join("library.db")
    } else {
        PathBuf::from("library.db")
    }
}

pub fn open_database<P: AsRef<Path>>(path: P) -> rusqlite::Result<Database> {
    if let Some(parent) = path.as_ref().parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let conn = Connection::open(path)?;
    Database::new(conn)
}

pub fn open_memory_database() -> rusqlite::Result<Database> {
    let conn = Connection::open_in_memory()?;
    Database::new(conn)
}
