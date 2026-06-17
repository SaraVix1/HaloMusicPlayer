use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::State;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Serialize, Clone)]
pub struct SmartPlaylist {
    pub id: i64,
    pub name: String,
    pub match_mode: String,
    pub sort_field: String,
    pub sort_direction: String,
    pub limit_count: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct SmartPlaylistRule {
    pub id: i64,
    pub playlist_id: i64,
    pub field: String,
    pub operator: String,
    pub value: String,
    pub position: i64,
}

/// Subset of track fields needed for the smart playlist tracks view.
#[derive(Serialize)]
pub struct SmartTrack {
    pub id: i64,
    pub title: Option<String>,
    pub album_name: Option<String>,
    pub artists: Vec<String>,
    pub duration_ms: Option<i64>,
    pub album_art_path: Option<String>,
    pub file_path: String,
    pub rating: i64,
    pub play_count: i64,
}

// ---------------------------------------------------------------------------
// Validation allowlists (guard against SQL injection via field/operator names)
// ---------------------------------------------------------------------------

fn validate_field(field: &str) -> bool {
    matches!(
        field,
        "title" | "artist" | "album_artist" | "album" | "genre" | "composer"
            | "year" | "rating" | "play_count" | "skip_count" | "duration_ms"
            | "date_added" | "last_played_at"
    )
}

fn validate_operator(op: &str) -> bool {
    matches!(
        op,
        "is" | "is_not" | "contains" | "not_contains"
            | "eq" | "not_eq" | "gt" | "gte" | "lt" | "lte"
            | "in_last_days"
    )
}

fn validate_sort_field(field: &str) -> bool {
    matches!(
        field,
        "title" | "artist" | "album" | "year" | "rating"
            | "play_count" | "skip_count" | "duration_ms"
            | "date_added" | "last_played_at"
    )
}

fn validate_match_mode(mode: &str) -> bool {
    matches!(mode, "all" | "any")
}

fn validate_direction(dir: &str) -> bool {
    matches!(dir, "asc" | "desc")
}

fn validate_limit(limit: Option<i64>) -> bool {
    limit.map_or(true, |n| n > 0 && n <= 10_000)
}

// ---------------------------------------------------------------------------
// SQL generation
// ---------------------------------------------------------------------------

/// Convert one rule into a SQL fragment + its parameters.
/// Returns None if the field/operator combination is unsupported.
fn rule_to_sql(rule: &SmartPlaylistRule) -> Option<(String, Vec<rusqlite::types::Value>)> {
    if !validate_field(&rule.field) || !validate_operator(&rule.operator) {
        return None;
    }
    let v = &rule.value;
    let mut ps: Vec<rusqlite::types::Value> = Vec::new();

    match rule.field.as_str() {
        // ── Text columns on the tracks table ──────────────────────────────
        "title" | "album" => {
            let col = if rule.field == "title" { "t.title" } else { "t.album_name" };
            let (op_sql, param) = match rule.operator.as_str() {
                "is" => ("= ?".to_string(), v.clone()),
                "is_not" => ("!= ?".to_string(), v.clone()),
                "contains" => ("LIKE ?".to_string(), format!("%{v}%")),
                "not_contains" => ("NOT LIKE ?".to_string(), format!("%{v}%")),
                _ => return None,
            };
            ps.push(rusqlite::types::Value::Text(param));
            Some((format!("{col} {op_sql}"), ps))
        }

        // ── Multi-value junction tables ───────────────────────────────────
        "artist" | "album_artist" | "genre" | "composer" => {
            let (jt, id_col, et) = match rule.field.as_str() {
                "artist" => ("track_artists", "artist_id", "artists"),
                "album_artist" => ("track_album_artists", "album_artist_id", "album_artists"),
                "genre" => ("track_genres", "genre_id", "genres"),
                "composer" => ("track_composers", "composer_id", "composers"),
                _ => unreachable!(),
            };
            let (cond_sql, neg, param) = match rule.operator.as_str() {
                "is" => ("= ?", false, v.clone()),
                "is_not" => ("= ?", true, v.clone()),
                "contains" => ("LIKE ?", false, format!("%{v}%")),
                "not_contains" => ("LIKE ?", true, format!("%{v}%")),
                _ => return None,
            };
            let not = if neg { "NOT " } else { "" };
            let sql = format!(
                "{not}EXISTS (SELECT 1 FROM {jt} _jt \
                 JOIN {et} _e ON _e.id = _jt.{id_col} \
                 WHERE _jt.track_id = t.id AND _e.name {cond_sql})"
            );
            ps.push(rusqlite::types::Value::Text(param));
            Some((sql, ps))
        }

        // ── Numeric columns ───────────────────────────────────────────────
        "year" | "rating" | "play_count" | "skip_count" | "duration_ms" => {
            let col = match rule.field.as_str() {
                "year" => "t.year",
                "rating" => "t.rating",
                "play_count" => "t.play_count",
                "skip_count" => "t.skip_count",
                "duration_ms" => "t.duration_ms",
                _ => unreachable!(),
            };
            let op_sql = match rule.operator.as_str() {
                "eq" => "=",
                "not_eq" => "!=",
                "gt" => ">",
                "gte" => ">=",
                "lt" => "<",
                "lte" => "<=",
                _ => return None,
            };
            let n: f64 = v.parse().ok()?;
            ps.push(rusqlite::types::Value::Real(n));
            Some((format!("{col} {op_sql} ?"), ps))
        }

        // ── Date added (stored as ISO TEXT in scanned_at) ─────────────────
        "date_added" => match rule.operator.as_str() {
            "in_last_days" => {
                let n: i64 = v.parse().ok()?;
                Some((format!("t.scanned_at >= datetime('now', '-{n} days')"), ps))
            }
            "gt" => {
                ps.push(rusqlite::types::Value::Text(v.clone()));
                Some(("t.scanned_at > ?".into(), ps))
            }
            "lt" => {
                ps.push(rusqlite::types::Value::Text(v.clone()));
                Some(("t.scanned_at < ?".into(), ps))
            }
            _ => None,
        },

        // ── Last played (stored as Unix INTEGER, NULL if never played) ─────
        "last_played_at" => match rule.operator.as_str() {
            "in_last_days" => {
                let n: i64 = v.parse().ok()?;
                Some((
                    format!(
                        "t.last_played_at IS NOT NULL \
                         AND t.last_played_at >= (strftime('%s','now') - {n} * 86400)"
                    ),
                    ps,
                ))
            }
            "gt" | "gte" | "lt" | "lte" => {
                let op_sql = match rule.operator.as_str() {
                    "gt" => ">",
                    "gte" => ">=",
                    "lt" => "<",
                    "lte" => "<=",
                    _ => unreachable!(),
                };
                let n: i64 = v.parse().ok()?;
                ps.push(rusqlite::types::Value::Integer(n));
                Some((format!("t.last_played_at {op_sql} ?"), ps))
            }
            _ => None,
        },

        _ => None,
    }
}

fn sort_col(field: &str) -> Option<&'static str> {
    match field {
        "title" => Some("t.title COLLATE NOCASE"),
        // Artist lives in a junction table, so sort by the track's
        // alphabetically-first artist name via a correlated subquery.
        "artist" => Some(
            "(SELECT a.name FROM track_artists ta \
              JOIN artists a ON a.id = ta.artist_id \
              WHERE ta.track_id = t.id \
              ORDER BY a.name COLLATE NOCASE LIMIT 1) COLLATE NOCASE",
        ),
        "album" => Some("t.album_name COLLATE NOCASE"),
        "year" => Some("t.year"),
        "rating" => Some("t.rating"),
        "play_count" => Some("t.play_count"),
        "skip_count" => Some("t.skip_count"),
        "duration_ms" => Some("t.duration_ms"),
        "date_added" => Some("t.scanned_at"),
        "last_played_at" => Some("t.last_played_at"),
        _ => None,
    }
}

fn build_tracks_query(
    playlist: &SmartPlaylist,
    rules: &[SmartPlaylistRule],
) -> (String, Vec<rusqlite::types::Value>) {
    let mut conditions: Vec<String> = Vec::new();
    let mut all_params: Vec<rusqlite::types::Value> = Vec::new();

    for rule in rules {
        if let Some((cond, params)) = rule_to_sql(rule) {
            conditions.push(cond);
            all_params.extend(params);
        }
    }

    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        let joiner = if playlist.match_mode == "any" { " OR " } else { " AND " };
        format!("WHERE ({})", conditions.join(joiner))
    };

    let dir = if playlist.sort_direction == "desc" { "DESC" } else { "ASC" };
    let order_clause = sort_col(&playlist.sort_field)
        .map(|col| format!("ORDER BY {col} {dir}"))
        .unwrap_or_default();

    let limit_clause = playlist.limit_count
        .map(|n| format!("LIMIT {n}"))
        .unwrap_or_default();

    let sql = format!(
        "SELECT t.id, t.title, t.album_name, t.duration_ms,
                t.album_art_path, t.file_path, t.rating, t.play_count,
                (SELECT GROUP_CONCAT(name, '||') FROM (
                 SELECT a.name FROM track_artists ta JOIN artists a ON a.id = ta.artist_id
                 WHERE ta.track_id = t.id ORDER BY a.name COLLATE NOCASE))
         FROM tracks t
         {where_clause}
         {order_clause}
         {limit_clause}"
    );

    (sql, all_params)
}

// ---------------------------------------------------------------------------
// CRUD helpers
// ---------------------------------------------------------------------------

fn row_to_playlist(row: &rusqlite::Row<'_>) -> rusqlite::Result<SmartPlaylist> {
    Ok(SmartPlaylist {
        id: row.get(0)?,
        name: row.get(1)?,
        match_mode: row.get(2)?,
        sort_field: row.get(3)?,
        sort_direction: row.get(4)?,
        limit_count: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
    })
}

fn fetch_rules(conn: &Connection, playlist_id: i64) -> Result<Vec<SmartPlaylistRule>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, playlist_id, field, operator, value, position
             FROM smart_playlist_rules
             WHERE playlist_id = ?1
             ORDER BY position ASC",
        )
        .map_err(|e| e.to_string())?;
    let result = stmt.query_map([playlist_id], |row| {
        Ok(SmartPlaylistRule {
            id: row.get(0)?,
            playlist_id: row.get(1)?,
            field: row.get(2)?,
            operator: row.get(3)?,
            value: row.get(4)?,
            position: row.get(5)?,
        })
    })
    .map_err(|e| e.to_string())?
    .collect::<Result<Vec<_>, _>>()
    .map_err(|e| e.to_string())?;
    Ok(result)
}

// ---------------------------------------------------------------------------
// IPC commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn get_smart_playlists(db: State<Mutex<Connection>>) -> Result<Vec<SmartPlaylist>, String> {
    let conn = db.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT id, name, match_mode, sort_field, sort_direction,
                    limit_count, created_at, updated_at
             FROM smart_playlists ORDER BY id ASC",
        )
        .map_err(|e| e.to_string())?;
    let result = stmt.query_map([], row_to_playlist)
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(result)
}

#[tauri::command]
pub fn get_smart_playlist(
    db: State<Mutex<Connection>>,
    id: i64,
) -> Result<(SmartPlaylist, Vec<SmartPlaylistRule>), String> {
    let conn = db.lock().map_err(|e| e.to_string())?;
    let playlist = conn
        .query_row(
            "SELECT id, name, match_mode, sort_field, sort_direction,
                    limit_count, created_at, updated_at
             FROM smart_playlists WHERE id = ?1",
            [id],
            row_to_playlist,
        )
        .map_err(|e| e.to_string())?;
    let rules = fetch_rules(&conn, id)?;
    Ok((playlist, rules))
}

#[tauri::command]
pub fn create_smart_playlist(
    db: State<Mutex<Connection>>,
    name: String,
) -> Result<SmartPlaylist, String> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err("name must not be empty".into());
    }
    let conn = db.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO smart_playlists (name) VALUES (?1)",
        params![name],
    )
    .map_err(|e| e.to_string())?;
    let id = conn.last_insert_rowid();
    conn.query_row(
        "SELECT id, name, match_mode, sort_field, sort_direction,
                limit_count, created_at, updated_at
         FROM smart_playlists WHERE id = ?1",
        [id],
        row_to_playlist,
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_smart_playlist(
    db: State<Mutex<Connection>>,
    id: i64,
    name: String,
    match_mode: String,
    sort_field: String,
    sort_direction: String,
    limit_count: Option<i64>,
) -> Result<SmartPlaylist, String> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err("name must not be empty".into());
    }
    if !validate_match_mode(&match_mode) {
        return Err(format!("invalid match_mode: {match_mode}"));
    }
    if !validate_sort_field(&sort_field) {
        return Err(format!("invalid sort_field: {sort_field}"));
    }
    if !validate_direction(&sort_direction) {
        return Err(format!("invalid sort_direction: {sort_direction}"));
    }
    if !validate_limit(limit_count) {
        return Err("limit_count out of range".into());
    }
    let conn = db.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE smart_playlists
         SET name = ?1, match_mode = ?2, sort_field = ?3,
             sort_direction = ?4, limit_count = ?5,
             updated_at = strftime('%s','now')
         WHERE id = ?6",
        params![name, match_mode, sort_field, sort_direction, limit_count, id],
    )
    .map_err(|e| e.to_string())?;
    conn.query_row(
        "SELECT id, name, match_mode, sort_field, sort_direction,
                limit_count, created_at, updated_at
         FROM smart_playlists WHERE id = ?1",
        [id],
        row_to_playlist,
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_smart_playlist(db: State<Mutex<Connection>>, id: i64) -> Result<(), String> {
    let conn = db.lock().map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM smart_playlists WHERE id = ?1", [id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Replace all rules for a playlist in one atomic operation.
#[tauri::command]
pub fn set_smart_playlist_rules(
    db: State<Mutex<Connection>>,
    playlist_id: i64,
    rules: Vec<SmartPlaylistRule>,
) -> Result<(), String> {
    for r in &rules {
        if !validate_field(&r.field) {
            return Err(format!("invalid field: {}", r.field));
        }
        if !validate_operator(&r.operator) {
            return Err(format!("invalid operator: {}", r.operator));
        }
    }
    let conn = db.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "DELETE FROM smart_playlist_rules WHERE playlist_id = ?1",
        [playlist_id],
    )
    .map_err(|e| e.to_string())?;
    for (i, rule) in rules.iter().enumerate() {
        conn.execute(
            "INSERT INTO smart_playlist_rules (playlist_id, field, operator, value, position)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![playlist_id, rule.field, rule.operator, rule.value, i as i64],
        )
        .map_err(|e| e.to_string())?;
    }
    conn.execute(
        "UPDATE smart_playlists SET updated_at = strftime('%s','now') WHERE id = ?1",
        [playlist_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn get_smart_playlist_tracks(
    db: State<Mutex<Connection>>,
    id: i64,
) -> Result<Vec<SmartTrack>, String> {
    let conn = db.lock().map_err(|e| e.to_string())?;
    let playlist = conn
        .query_row(
            "SELECT id, name, match_mode, sort_field, sort_direction,
                    limit_count, created_at, updated_at
             FROM smart_playlists WHERE id = ?1",
            [id],
            row_to_playlist,
        )
        .map_err(|e| e.to_string())?;
    let rules = fetch_rules(&conn, id)?;
    let (sql, params) = build_tracks_query(&playlist, &rules);

    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let result = stmt.query_map(rusqlite::params_from_iter(&params), |row| {
        let artists_csv: Option<String> = row.get(8)?;
        Ok(SmartTrack {
            id: row.get(0)?,
            title: row.get(1)?,
            album_name: row.get(2)?,
            duration_ms: row.get(3)?,
            album_art_path: row.get(4)?,
            file_path: row.get(5)?,
            rating: row.get(6)?,
            play_count: row.get(7)?,
            artists: artists_csv
                .map(|s| s.split("||").map(String::from).collect())
                .unwrap_or_default(),
        })
    })
    .map_err(|e| e.to_string())?
    .collect::<Result<Vec<_>, _>>()
    .map_err(|e| e.to_string())?;
    Ok(result)
}
