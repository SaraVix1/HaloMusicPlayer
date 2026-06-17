use rusqlite::{Connection, ToSql};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::State;

#[derive(Serialize)]
pub struct Track {
    pub id: i64,
    pub title: Option<String>,
    pub album_name: Option<String>,
    pub artists: Vec<String>,
    pub genres: Vec<String>,
    pub duration_ms: Option<i64>,
    pub track_number: Option<i64>,
    pub disc_number: Option<i64>,
    pub year: Option<i64>,
    pub album_art_path: Option<String>,
    pub file_path: String,
    pub folder_id: Option<i64>,
    pub scanned_at: String,
    pub rating: i64,
    pub play_count: i64,
    pub skip_count: i64,
    pub last_played_at: Option<i64>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct TracksQuery {
    pub album: Option<String>,
    pub artist: Option<String>,
    pub album_artist: Option<String>,
    pub composer: Option<String>,
    pub genre: Option<String>,
    pub year: Option<i64>,
    pub folder_id: Option<i64>,
    pub sort: Option<String>,
    pub direction: Option<String>,
}

fn order_by_clause(sort: Option<&str>, direction: Option<&str>) -> String {
    let column = match sort.unwrap_or("title") {
        "title" => "t.title COLLATE NOCASE",
        "album" => "t.album_name COLLATE NOCASE",
        "artist" => "artists_csv COLLATE NOCASE",
        "genre" => "genres_csv COLLATE NOCASE",
        "duration" => "t.duration_ms",
        "year" => "t.year",
        "scanned_at" => "t.scanned_at",
        "track_number" => "t.disc_number, t.track_number",
        "rating" => "t.rating",
        "play_count" => "t.play_count",
        _ => "t.title COLLATE NOCASE",
    };
    let dir = if direction.unwrap_or("asc").eq_ignore_ascii_case("desc") {
        "DESC"
    } else {
        "ASC"
    };
    format!("{} {}", column, dir)
}

#[tauri::command]
pub fn get_tracks(query: TracksQuery, db: State<Mutex<Connection>>) -> Result<Vec<Track>, String> {
    let conn = db.lock().map_err(|e| e.to_string())?;

    let mut conditions: Vec<String> = Vec::new();
    let mut args: Vec<Box<dyn ToSql>> = Vec::new();

    if let Some(album) = &query.album {
        conditions.push("t.album_name = ?".into());
        args.push(Box::new(album.clone()));
    }
    if let Some(artist) = &query.artist {
        conditions.push(
            "EXISTS (SELECT 1 FROM track_artists ta_f JOIN artists a_f ON a_f.id = ta_f.artist_id WHERE ta_f.track_id = t.id AND a_f.name = ?)".into(),
        );
        args.push(Box::new(artist.clone()));
    }
    if let Some(album_artist) = &query.album_artist {
        conditions.push(
            "EXISTS (SELECT 1 FROM track_album_artists taa_f JOIN album_artists aa_f ON aa_f.id = taa_f.album_artist_id WHERE taa_f.track_id = t.id AND aa_f.name = ?)".into(),
        );
        args.push(Box::new(album_artist.clone()));
    }
    if let Some(composer) = &query.composer {
        conditions.push(
            "EXISTS (SELECT 1 FROM track_composers tc_f JOIN composers c_f ON c_f.id = tc_f.composer_id WHERE tc_f.track_id = t.id AND c_f.name = ?)".into(),
        );
        args.push(Box::new(composer.clone()));
    }
    if let Some(genre) = &query.genre {
        conditions.push(
            "EXISTS (SELECT 1 FROM track_genres tg_f JOIN genres g_f ON g_f.id = tg_f.genre_id WHERE tg_f.track_id = t.id AND g_f.name = ?)".into(),
        );
        args.push(Box::new(genre.clone()));
    }
    if let Some(year) = query.year {
        conditions.push("t.year = ?".into());
        args.push(Box::new(year));
    }
    if let Some(folder_id) = query.folder_id {
        conditions.push("t.folder_id = ?".into());
        args.push(Box::new(folder_id));
    }

    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!(" WHERE {}", conditions.join(" AND "))
    };

    let order = order_by_clause(query.sort.as_deref(), query.direction.as_deref());

    let sql = format!(
        "SELECT t.id, t.title, t.album_name, t.duration_ms, t.track_number, t.disc_number,
                t.year, t.album_art_path, t.file_path, t.folder_id, t.scanned_at,
                t.rating, t.play_count, t.skip_count, t.last_played_at,
                (SELECT GROUP_CONCAT(name, '||') FROM (
                 SELECT a.name FROM track_artists ta JOIN artists a ON a.id = ta.artist_id
                 WHERE ta.track_id = t.id ORDER BY a.name COLLATE NOCASE)) AS artists_csv,
                (SELECT GROUP_CONCAT(name, '||') FROM (
                 SELECT g.name FROM track_genres tg JOIN genres g ON g.id = tg.genre_id
                 WHERE tg.track_id = t.id ORDER BY g.name COLLATE NOCASE)) AS genres_csv
         FROM tracks t{}
         ORDER BY {}",
        where_clause, order
    );

    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let params_iter = rusqlite::params_from_iter(args.iter().map(|b| b.as_ref()));

    let rows = stmt
        .query_map(params_iter, |row| {
            let artists_csv: Option<String> = row.get(15)?;
            let genres_csv: Option<String> = row.get(16)?;
            Ok(Track {
                id: row.get(0)?,
                title: row.get(1)?,
                album_name: row.get(2)?,
                duration_ms: row.get(3)?,
                track_number: row.get(4)?,
                disc_number: row.get(5)?,
                year: row.get(6)?,
                album_art_path: row.get(7)?,
                file_path: row.get(8)?,
                folder_id: row.get(9)?,
                scanned_at: row.get(10)?,
                rating: row.get(11)?,
                play_count: row.get(12)?,
                skip_count: row.get(13)?,
                last_played_at: row.get(14)?,
                artists: artists_csv
                    .map(|s| s.split("||").map(String::from).collect())
                    .unwrap_or_default(),
                genres: genres_csv
                    .map(|s| s.split("||").map(String::from).collect())
                    .unwrap_or_default(),
            })
        })
        .map_err(|e| e.to_string())?;

    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

// ── Recently played ───────────────────────────────────────────────────────────

#[tauri::command]
pub fn get_recently_played(db: State<Mutex<Connection>>) -> Result<Vec<Track>, String> {
    let conn = db.lock().map_err(|e| e.to_string())?;
    let sql = "SELECT t.id, t.title, t.album_name, t.duration_ms, t.track_number, t.disc_number,
                t.year, t.album_art_path, t.file_path, t.folder_id, t.scanned_at,
                t.rating, t.play_count, t.skip_count, t.last_played_at,
                (SELECT GROUP_CONCAT(name, '||') FROM (
                 SELECT a.name FROM track_artists ta JOIN artists a ON a.id = ta.artist_id
                 WHERE ta.track_id = t.id ORDER BY a.name COLLATE NOCASE)) AS artists_csv,
                (SELECT GROUP_CONCAT(name, '||') FROM (
                 SELECT g.name FROM track_genres tg JOIN genres g ON g.id = tg.genre_id
                 WHERE tg.track_id = t.id ORDER BY g.name COLLATE NOCASE)) AS genres_csv
         FROM tracks t
         WHERE t.last_played_at IS NOT NULL
         ORDER BY t.last_played_at DESC
         LIMIT 100";
    let mut stmt = conn.prepare(sql).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            let artists_csv: Option<String> = row.get(15)?;
            let genres_csv: Option<String> = row.get(16)?;
            Ok(Track {
                id: row.get(0)?,
                title: row.get(1)?,
                album_name: row.get(2)?,
                duration_ms: row.get(3)?,
                track_number: row.get(4)?,
                disc_number: row.get(5)?,
                year: row.get(6)?,
                album_art_path: row.get(7)?,
                file_path: row.get(8)?,
                folder_id: row.get(9)?,
                scanned_at: row.get(10)?,
                rating: row.get(11)?,
                play_count: row.get(12)?,
                skip_count: row.get(13)?,
                last_played_at: row.get(14)?,
                artists: artists_csv
                    .map(|s| s.split("||").map(String::from).collect())
                    .unwrap_or_default(),
                genres: genres_csv
                    .map(|s| s.split("||").map(String::from).collect())
                    .unwrap_or_default(),
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

// ── Most played ───────────────────────────────────────────────────────────────

#[tauri::command]
pub fn get_most_played(db: State<Mutex<Connection>>) -> Result<Vec<Track>, String> {
    let conn = db.lock().map_err(|e| e.to_string())?;
    let sql = "SELECT t.id, t.title, t.album_name, t.duration_ms, t.track_number, t.disc_number,
                t.year, t.album_art_path, t.file_path, t.folder_id, t.scanned_at,
                t.rating, t.play_count, t.skip_count, t.last_played_at,
                (SELECT GROUP_CONCAT(name, '||') FROM (
                 SELECT a.name FROM track_artists ta JOIN artists a ON a.id = ta.artist_id
                 WHERE ta.track_id = t.id ORDER BY a.name COLLATE NOCASE)) AS artists_csv,
                (SELECT GROUP_CONCAT(name, '||') FROM (
                 SELECT g.name FROM track_genres tg JOIN genres g ON g.id = tg.genre_id
                 WHERE tg.track_id = t.id ORDER BY g.name COLLATE NOCASE)) AS genres_csv
         FROM tracks t
         WHERE t.play_count > 0
         ORDER BY t.play_count DESC
         LIMIT 100";
    let mut stmt = conn.prepare(sql).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            let artists_csv: Option<String> = row.get(15)?;
            let genres_csv: Option<String> = row.get(16)?;
            Ok(Track {
                id: row.get(0)?,
                title: row.get(1)?,
                album_name: row.get(2)?,
                duration_ms: row.get(3)?,
                track_number: row.get(4)?,
                disc_number: row.get(5)?,
                year: row.get(6)?,
                album_art_path: row.get(7)?,
                file_path: row.get(8)?,
                folder_id: row.get(9)?,
                scanned_at: row.get(10)?,
                rating: row.get(11)?,
                play_count: row.get(12)?,
                skip_count: row.get(13)?,
                last_played_at: row.get(14)?,
                artists: artists_csv
                    .map(|s| s.split("||").map(String::from).collect())
                    .unwrap_or_default(),
                genres: genres_csv
                    .map(|s| s.split("||").map(String::from).collect())
                    .unwrap_or_default(),
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

// ── Albums ────────────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct Album {
    pub name: String,
    pub album_artists: Vec<String>,
    pub track_count: i64,
    pub year: Option<i64>,
    pub album_art_path: Option<String>,
}

#[tauri::command]
pub fn get_albums(db: State<Mutex<Connection>>) -> Result<Vec<Album>, String> {
    let conn = db.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT t.album_name,
                    COUNT(t.id) AS track_count,
                    MIN(t.year) AS year,
                    (SELECT t2.album_art_path FROM tracks t2
                     WHERE t2.album_name = t.album_name AND t2.album_art_path IS NOT NULL
                     LIMIT 1) AS album_art_path,
                    (SELECT GROUP_CONCAT(name) FROM (
                     SELECT DISTINCT aa.name FROM tracks t3
                     JOIN track_album_artists taa ON taa.track_id = t3.id
                     JOIN album_artists aa ON aa.id = taa.album_artist_id
                     WHERE t3.album_name = t.album_name
                     ORDER BY aa.name COLLATE NOCASE)) AS album_artists_csv
             FROM tracks t
             WHERE t.album_name IS NOT NULL AND t.album_name != ''
             GROUP BY t.album_name
             ORDER BY t.album_name COLLATE NOCASE",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([], |row| {
            let csv: Option<String> = row.get(4)?;
            Ok(Album {
                name: row.get(0)?,
                track_count: row.get(1)?,
                year: row.get(2)?,
                album_art_path: row.get(3)?,
                album_artists: csv
                    .map(|s| {
                        s.split(',')
                            .map(|p| p.trim().to_string())
                            .filter(|p| !p.is_empty())
                            .collect()
                    })
                    .unwrap_or_default(),
            })
        })
        .map_err(|e| e.to_string())?;

    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

// ── Artists ───────────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct Artist {
    pub id: i64,
    pub name: String,
    pub track_count: i64,
    pub album_art_path: Option<String>,
}

#[tauri::command]
pub fn get_artists(db: State<Mutex<Connection>>) -> Result<Vec<Artist>, String> {
    let conn = db.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT a.id, a.name,
                    COUNT(DISTINCT ta.track_id) AS track_count,
                    (SELECT t.album_art_path FROM tracks t
                     JOIN track_artists ta2 ON ta2.track_id = t.id
                     WHERE ta2.artist_id = a.id AND t.album_art_path IS NOT NULL
                     LIMIT 1) AS album_art_path
             FROM artists a
             LEFT JOIN track_artists ta ON ta.artist_id = a.id
             GROUP BY a.id, a.name
             HAVING track_count > 0
             ORDER BY a.name COLLATE NOCASE",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([], |row| {
            Ok(Artist {
                id: row.get(0)?,
                name: row.get(1)?,
                track_count: row.get(2)?,
                album_art_path: row.get(3)?,
            })
        })
        .map_err(|e| e.to_string())?;

    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

// ── Genres ────────────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct Genre {
    pub id: i64,
    pub name: String,
    pub track_count: i64,
}

#[tauri::command]
pub fn get_genres(db: State<Mutex<Connection>>) -> Result<Vec<Genre>, String> {
    let conn = db.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT g.id, g.name, COUNT(DISTINCT tg.track_id) AS track_count
             FROM genres g
             LEFT JOIN track_genres tg ON tg.genre_id = g.id
             GROUP BY g.id, g.name
             HAVING track_count > 0
             ORDER BY g.name COLLATE NOCASE",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([], |row| {
            Ok(Genre {
                id: row.get(0)?,
                name: row.get(1)?,
                track_count: row.get(2)?,
            })
        })
        .map_err(|e| e.to_string())?;

    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

// ── Folder tracks ─────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct FolderTrack {
    pub id: i64,
    pub folder_id: i64,
    pub folder_path: String,
    pub file_path: String,
    pub title: Option<String>,
    pub duration_ms: Option<i64>,
    pub album_art_path: Option<String>,
}

#[tauri::command]
pub fn get_folder_tracks(db: State<Mutex<Connection>>) -> Result<Vec<FolderTrack>, String> {
    let conn = db.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT t.id, t.folder_id, f.path, t.file_path, t.title, t.duration_ms, t.album_art_path
             FROM tracks t
             JOIN folders f ON f.id = t.folder_id
             ORDER BY f.path, t.file_path",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([], |row| {
            Ok(FolderTrack {
                id: row.get(0)?,
                folder_id: row.get(1)?,
                folder_path: row.get(2)?,
                file_path: row.get(3)?,
                title: row.get(4)?,
                duration_ms: row.get(5)?,
                album_art_path: row.get(6)?,
            })
        })
        .map_err(|e| e.to_string())?;

    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

// ── Album artists ─────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct AlbumArtist {
    pub id: i64,
    pub name: String,
    pub track_count: i64,
    pub album_count: i64,
    pub album_art_path: Option<String>,
}

#[tauri::command]
pub fn get_album_artists(db: State<Mutex<Connection>>) -> Result<Vec<AlbumArtist>, String> {
    let conn = db.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT aa.id, aa.name,
                    COUNT(DISTINCT taa.track_id) AS track_count,
                    COUNT(DISTINCT t.album_name) AS album_count,
                    (SELECT t2.album_art_path FROM tracks t2
                     JOIN track_album_artists taa2 ON taa2.track_id = t2.id
                     WHERE taa2.album_artist_id = aa.id AND t2.album_art_path IS NOT NULL
                     LIMIT 1) AS album_art_path
             FROM album_artists aa
             LEFT JOIN track_album_artists taa ON taa.album_artist_id = aa.id
             LEFT JOIN tracks t ON t.id = taa.track_id
             GROUP BY aa.id, aa.name
             HAVING track_count > 0
             ORDER BY aa.name COLLATE NOCASE",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([], |row| {
            Ok(AlbumArtist {
                id: row.get(0)?,
                name: row.get(1)?,
                track_count: row.get(2)?,
                album_count: row.get(3)?,
                album_art_path: row.get(4)?,
            })
        })
        .map_err(|e| e.to_string())?;

    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

// ── Composers ─────────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct Composer {
    pub id: i64,
    pub name: String,
    pub track_count: i64,
}

#[tauri::command]
pub fn get_composers(db: State<Mutex<Connection>>) -> Result<Vec<Composer>, String> {
    let conn = db.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT c.id, c.name,
                    COUNT(DISTINCT tc.track_id) AS track_count
             FROM composers c
             LEFT JOIN track_composers tc ON tc.composer_id = c.id
             GROUP BY c.id, c.name
             HAVING track_count > 0
             ORDER BY c.name COLLATE NOCASE",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([], |row| {
            Ok(Composer {
                id: row.get(0)?,
                name: row.get(1)?,
                track_count: row.get(2)?,
            })
        })
        .map_err(|e| e.to_string())?;

    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

// ── Years ─────────────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct YearStat {
    pub year: i64,
    pub track_count: i64,
}

#[tauri::command]
pub fn get_years(db: State<Mutex<Connection>>) -> Result<Vec<YearStat>, String> {
    let conn = db.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT year, COUNT(*) AS track_count
             FROM tracks
             WHERE year IS NOT NULL
             GROUP BY year
             ORDER BY year DESC",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([], |row| {
            Ok(YearStat {
                year: row.get(0)?,
                track_count: row.get(1)?,
            })
        })
        .map_err(|e| e.to_string())?;

    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}
