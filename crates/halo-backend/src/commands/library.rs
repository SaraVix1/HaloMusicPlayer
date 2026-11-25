use rusqlite::{Connection, ToSql};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;

#[derive(Debug, Clone, Serialize)]
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
    /// When the track first entered the library (`added_at`).
    pub added_at: String,
    pub rating: i64,
    pub play_count: i64,
    pub skip_count: i64,
    pub last_played_at: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, Default, PartialEq, Eq, Hash)]
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
        // Sort key kept as "scanned_at" (routes / saved sorts use it).
        "scanned_at" | "added_at" => "t.added_at",
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

pub fn get_tracks(query: TracksQuery, db: &Mutex<Connection>) -> Result<Vec<Track>, String> {
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
                t.year, t.album_art_path, t.file_path, t.folder_id,
                COALESCE(t.added_at, t.scanned_at, ''),
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
                added_at: row.get(10)?,
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

pub fn get_recently_played(db: &Mutex<Connection>) -> Result<Vec<Track>, String> {
    let conn = db.lock().map_err(|e| e.to_string())?;
    let sql = "SELECT t.id, t.title, t.album_name, t.duration_ms, t.track_number, t.disc_number,
                t.year, t.album_art_path, t.file_path, t.folder_id,
                COALESCE(t.added_at, t.scanned_at, ''),
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
                added_at: row.get(10)?,
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

pub fn get_most_played(db: &Mutex<Connection>) -> Result<Vec<Track>, String> {
    let conn = db.lock().map_err(|e| e.to_string())?;
    let sql = "SELECT t.id, t.title, t.album_name, t.duration_ms, t.track_number, t.disc_number,
                t.year, t.album_art_path, t.file_path, t.folder_id,
                COALESCE(t.added_at, t.scanned_at, ''),
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
                added_at: row.get(10)?,
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

#[derive(Debug, Clone, Serialize)]
pub struct Album {
    pub name: String,
    pub album_artists: Vec<String>,
    pub track_count: i64,
    pub year: Option<i64>,
    pub album_art_path: Option<String>,
    /// When the album's newest track entered the library (`added_at`),
    /// for "Recently added" sorting. 'YYYY-MM-DD HH:MM:SS', UTC.
    pub added: Option<String>,
}

pub fn get_albums(db: &Mutex<Connection>) -> Result<Vec<Album>, String> {
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
                     ORDER BY aa.name COLLATE NOCASE)) AS album_artists_csv,
                    MAX(COALESCE(t.added_at, t.scanned_at)) AS added
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
                added: row.get(5)?,
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

/// Tracks in the library (empty-library states).
pub fn track_count(db: &Mutex<Connection>) -> Result<i64, String> {
    let conn = db.lock().map_err(|e| e.to_string())?;
    conn.query_row("SELECT COUNT(*) FROM tracks", [], |r| r.get(0)).map_err(|e| e.to_string())
}

// ── Home shelves ──────────────────────────────────────────────────────────────

/// The three album rows on the Home page.
#[derive(Debug, Clone, Default, Serialize)]
pub struct HomeShelves {
    /// Albums by their most recent play.
    pub recently_played: Vec<Album>,
    /// Albums by total play count.
    pub most_played: Vec<Album>,
    /// Albums by when they first entered the library (`added_at`).
    pub recently_added: Vec<Album>,
    /// Tracks in the library (0 → Home shows the empty state).
    pub library_tracks: i64,
}

/// Up to `limit` albums per Home row, in one lock of the DB.
pub fn get_home_shelves(limit: usize, db: &Mutex<Connection>) -> Result<HomeShelves, String> {
    let conn = db.lock().map_err(|e| e.to_string())?;
    let limit = limit as i64;
    Ok(HomeShelves {
        recently_played: album_shelf(&conn, "MAX(t.last_played_at)", "k IS NOT NULL", limit)?,
        most_played: album_shelf(&conn, "SUM(t.play_count)", "k > 0", limit)?,
        recently_added: album_shelf(
            &conn,
            "MAX(COALESCE(t.added_at, t.scanned_at))",
            "k IS NOT NULL",
            limit,
        )?,
        library_tracks: conn
            .query_row("SELECT COUNT(*) FROM tracks", [], |r| r.get(0))
            .map_err(|e| e.to_string())?,
    })
}

/// Albums ranked by the aggregate `key` (descending; ties by name), keeping
/// those where `having` holds. Picks the top `limit` names first, then
/// looks up art / artists for just those rows. Subtitle artists are the
/// album artists, or the track artists when the album has none tagged.
fn album_shelf(conn: &Connection, key: &str, having: &str, limit: i64) -> Result<Vec<Album>, String> {
    let sql = format!(
        "WITH top AS (
             SELECT t.album_name AS name, {key} AS k
             FROM tracks t
             WHERE t.album_name IS NOT NULL AND t.album_name != ''
             GROUP BY t.album_name
             HAVING {having}
             ORDER BY k DESC, t.album_name COLLATE NOCASE
             LIMIT ?1)
         SELECT top.name,
                (SELECT COUNT(*) FROM tracks t1 WHERE t1.album_name = top.name),
                (SELECT MIN(t1.year) FROM tracks t1 WHERE t1.album_name = top.name),
                (SELECT t2.album_art_path FROM tracks t2
                 WHERE t2.album_name = top.name AND t2.album_art_path IS NOT NULL
                 LIMIT 1),
                (SELECT GROUP_CONCAT(name, '||') FROM (
                 SELECT DISTINCT aa.name FROM tracks t3
                 JOIN track_album_artists taa ON taa.track_id = t3.id
                 JOIN album_artists aa ON aa.id = taa.album_artist_id
                 WHERE t3.album_name = top.name
                 ORDER BY aa.name COLLATE NOCASE)),
                (SELECT GROUP_CONCAT(name, '||') FROM (
                 SELECT DISTINCT a.name FROM tracks t4
                 JOIN track_artists ta ON ta.track_id = t4.id
                 JOIN artists a ON a.id = ta.artist_id
                 WHERE t4.album_name = top.name
                 ORDER BY a.name COLLATE NOCASE)),
                (SELECT MAX(COALESCE(t5.added_at, t5.scanned_at)) FROM tracks t5
                 WHERE t5.album_name = top.name)
         FROM top
         ORDER BY top.k DESC, top.name COLLATE NOCASE"
    );
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([limit], |row| {
            let album_artists: Option<String> = row.get(4)?;
            let artists: Option<String> = row.get(5)?;
            Ok(Album {
                name: row.get(0)?,
                track_count: row.get(1)?,
                year: row.get(2)?,
                album_art_path: row.get(3)?,
                album_artists: split_csv(album_artists.or(artists)),
                added: row.get(6)?,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

fn split_csv(csv: Option<String>) -> Vec<String> {
    csv.map(|s| {
        s.split("||")
            .map(|p| p.trim().to_string())
            .filter(|p| !p.is_empty())
            .collect()
    })
    .unwrap_or_default()
}

// ── Artists ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct Artist {
    pub id: i64,
    pub name: String,
    pub track_count: i64,
    pub album_art_path: Option<String>,
}

pub fn get_artists(db: &Mutex<Connection>) -> Result<Vec<Artist>, String> {
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

#[derive(Debug, Clone, Serialize)]
pub struct Genre {
    pub id: i64,
    pub name: String,
    pub track_count: i64,
}

pub fn get_genres(db: &Mutex<Connection>) -> Result<Vec<Genre>, String> {
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

#[derive(Debug, Clone, Serialize)]
pub struct FolderTrack {
    pub id: i64,
    pub folder_id: i64,
    pub folder_path: String,
    pub file_path: String,
    pub title: Option<String>,
    pub duration_ms: Option<i64>,
    pub album_art_path: Option<String>,
}

pub fn get_folder_tracks(db: &Mutex<Connection>) -> Result<Vec<FolderTrack>, String> {
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

#[derive(Debug, Clone, Serialize)]
pub struct AlbumArtist {
    pub id: i64,
    pub name: String,
    pub track_count: i64,
    pub album_count: i64,
    pub album_art_path: Option<String>,
}

pub fn get_album_artists(db: &Mutex<Connection>) -> Result<Vec<AlbumArtist>, String> {
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

#[derive(Debug, Clone, Serialize)]
pub struct Composer {
    pub id: i64,
    pub name: String,
    pub track_count: i64,
}

pub fn get_composers(db: &Mutex<Connection>) -> Result<Vec<Composer>, String> {
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

#[derive(Debug, Clone, Serialize)]
pub struct YearStat {
    pub year: i64,
    pub track_count: i64,
}

pub fn get_years(db: &Mutex<Connection>) -> Result<Vec<YearStat>, String> {
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

#[cfg(test)]
mod home_tests {
    use super::*;
    use rusqlite::params;

    /// Real on-disk DB (WAL, all migrations) in a fresh temp dir.
    fn test_db(name: &str) -> Mutex<Connection> {
        let dir = std::env::temp_dir().join(format!("halo-test-home-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Mutex::new(crate::db::open(dir).unwrap())
    }

    /// (album, play_count, last_played_at, added_at, artist)
    fn add(db: &Mutex<Connection>, rows: &[(&str, i64, Option<i64>, &str, &str)]) {
        let conn = db.lock().unwrap();
        for (i, (album, plays, last, added, artist)) in rows.iter().enumerate() {
            conn.execute(
                "INSERT INTO tracks (file_path, album_name, play_count, last_played_at, added_at, scanned_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
                params![format!("/m/{album}/{i}.flac"), album, plays, last, added],
            )
            .unwrap();
            let id = conn.last_insert_rowid();
            conn.execute("INSERT OR IGNORE INTO artists (name) VALUES (?1)", [artist]).unwrap();
            conn.execute(
                "INSERT INTO track_artists (track_id, artist_id) SELECT ?1, id FROM artists WHERE name = ?2",
                params![id, artist],
            )
            .unwrap();
        }
    }

    fn names(v: &[Album]) -> Vec<&str> {
        v.iter().map(|a| a.name.as_str()).collect()
    }

    #[test]
    fn shelves_rank_albums() {
        let db = test_db("rank");
        add(
            &db,
            &[
                ("Alpha", 5, Some(100), "2024-01-01 00:00:00", "A"),
                ("Alpha", 1, Some(300), "2024-01-01 00:00:00", "A"),
                ("Beta", 9, Some(200), "2025-06-01 00:00:00", "B"),
                ("Gamma", 0, None, "2026-01-01 00:00:00", "C"),
                ("", 50, Some(999), "2026-02-01 00:00:00", "D"),
            ],
        );
        let s = get_home_shelves(10, &db).unwrap();
        // Alpha's latest play (300) beats Beta's (200); never-played and
        // untitled albums are left out.
        assert_eq!(names(&s.recently_played), ["Alpha", "Beta"]);
        // Beta 9 plays vs Alpha 6 (summed over its tracks).
        assert_eq!(names(&s.most_played), ["Beta", "Alpha"]);
        assert_eq!(names(&s.recently_added), ["Gamma", "Beta", "Alpha"]);
        let alpha = s.recently_played.iter().find(|a| a.name == "Alpha").unwrap();
        assert_eq!(alpha.track_count, 2);
        // No album-artist tags: falls back to the track artists.
        assert_eq!(alpha.album_artists, ["A"]);
        assert_eq!(s.library_tracks, 5);
        assert_eq!(get_home_shelves(10, &test_db("empty")).unwrap().library_tracks, 0);
    }

    #[test]
    fn shelves_respect_limit() {
        let db = test_db("limit");
        let rows: Vec<(String, String)> =
            (0..30).map(|i| (format!("Album {i:02}"), format!("2025-01-{:02} 00:00:00", i % 28 + 1))).collect();
        let rows: Vec<_> = rows.iter().map(|(a, d)| (a.as_str(), 1, Some(1), d.as_str(), "X")).collect();
        add(&db, &rows);
        let s = get_home_shelves(12, &db).unwrap();
        assert_eq!(s.recently_added.len(), 12);
        assert_eq!(s.most_played.len(), 12);
    }

    #[test]
    fn rescan_does_not_make_an_album_recently_added() {
        let db = test_db("rescan");
        add(
            &db,
            &[
                ("Old", 0, None, "2020-01-01 00:00:00", "A"),
                ("New", 0, None, "2025-01-01 00:00:00", "B"),
            ],
        );
        // What the scanner does to a changed file: scanned_at moves on,
        // added_at stays.
        db.lock()
            .unwrap()
            .execute("UPDATE tracks SET scanned_at = '2026-11-25 10:00:00' WHERE album_name = 'Old'", [])
            .unwrap();
        let s = get_home_shelves(10, &db).unwrap();
        assert_eq!(names(&s.recently_added), ["New", "Old"]);
    }
}
