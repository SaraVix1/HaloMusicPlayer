pub mod migrations;

use rusqlite::{Connection, Result};
use std::path::PathBuf;

pub fn open(app_data_dir: PathBuf) -> Result<Connection> {
    let db_path = app_data_dir.join("halo.db");
    let conn = Connection::open(db_path)?;
    conn.execute_batch(
        "PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON; PRAGMA synchronous=NORMAL;",
    )?;
    migrations::run(&conn)?;
    Ok(conn)
}
