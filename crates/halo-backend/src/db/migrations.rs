use rusqlite::{Connection, Result};

pub fn run(conn: &Connection) -> Result<()> {
    let version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;

    if version < 1 {
        conn.execute_batch(include_str!("../../migrations/001_initial.sql"))?;
        conn.execute_batch("PRAGMA user_version = 1;")?;
    }

    if version < 2 {
        conn.execute_batch(include_str!("../../migrations/002_stats.sql"))?;
        conn.execute_batch("PRAGMA user_version = 2;")?;
    }

    if version < 3 {
        conn.execute_batch(include_str!("../../migrations/003_lyrics.sql"))?;
        conn.execute_batch("PRAGMA user_version = 3;")?;
    }

    if version < 4 {
        conn.execute_batch(include_str!("../../migrations/004_eq_presets.sql"))?;
        conn.execute_batch("PRAGMA user_version = 4;")?;
    }

    if version < 5 {
        conn.execute_batch(include_str!("../../migrations/005_smart_playlists.sql"))?;
        conn.execute_batch("PRAGMA user_version = 5;")?;
    }

    if version < 6 {
        conn.execute_batch(include_str!("../../migrations/006_device_eq.sql"))?;
        conn.execute_batch("PRAGMA user_version = 6;")?;
    }

    if version < 7 {
        conn.execute_batch(include_str!("../../migrations/007_waveform.sql"))?;
        conn.execute_batch("PRAGMA user_version = 7;")?;
    }

    if version < 8 {
        conn.execute_batch(include_str!("../../migrations/008_waveform_rms.sql"))?;
        conn.execute_batch("PRAGMA user_version = 8;")?;
    }

    if version < 9 {
        // Offline Last.fm scrobble queue. Originally shipped as a second 007_*
        // file that the runner never executed, so the table was never created.
        conn.execute_batch(include_str!("../../migrations/009_lastfm_queue.sql"))?;
        conn.execute_batch("PRAGMA user_version = 9;")?;
    }

    if version < 10 {
        conn.execute_batch(include_str!("../../migrations/010_queue_position_index.sql"))?;
        conn.execute_batch("PRAGMA user_version = 10;")?;
    }

    Ok(())
}
