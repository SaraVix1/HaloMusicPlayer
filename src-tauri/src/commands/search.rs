use rusqlite::{params, Connection};
use serde::Serialize;
use std::sync::Mutex;
use tauri::State;

#[derive(Serialize)]
pub struct SearchTrackHit {
    pub track_id: i64,
    pub title: String,
    pub album_name: Option<String>,
    pub artists: Vec<String>,
    pub album_art_path: Option<String>,
    pub duration_ms: Option<i64>,
}

#[derive(Serialize)]
pub struct SearchAlbumHit {
    pub name: String,
    pub album_art_path: Option<String>,
    pub track_count: i64,
}

#[derive(Serialize)]
pub struct SearchArtistHit {
    pub id: i64,
    pub name: String,
    pub track_count: i64,
}

#[derive(Serialize)]
pub struct SearchPlaylistHit {
    pub id: i64,
    pub name: String,
    pub track_count: i64,
}

#[derive(Serialize, Default)]
pub struct SearchResults {
    pub tracks: Vec<SearchTrackHit>,
    pub albums: Vec<SearchAlbumHit>,
    pub artists: Vec<SearchArtistHit>,
    pub playlists: Vec<SearchPlaylistHit>,
}

const PER_TYPE_LIMIT: i64 = 8;

#[tauri::command]
pub fn search_library(
    query: String,
    db: State<Mutex<Connection>>,
) -> Result<SearchResults, String> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return Ok(SearchResults::default());
    }
    let like = format!("%{}%", trimmed.replace('%', "\\%").replace('_', "\\_"));
    let conn = db.lock().map_err(|e| e.to_string())?;

    let tracks = {
        let mut stmt = conn
            .prepare(
                "SELECT t.id, t.title, t.album_name, t.duration_ms, t.album_art_path,
                        (SELECT GROUP_CONCAT(name, '||') FROM (
                         SELECT a.name FROM track_artists ta JOIN artists a ON a.id = ta.artist_id
                         WHERE ta.track_id = t.id ORDER BY a.name COLLATE NOCASE))
                 FROM tracks t
                 WHERE t.title LIKE ?1 ESCAPE '\\'
                 ORDER BY t.title COLLATE NOCASE
                 LIMIT ?2",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![like, PER_TYPE_LIMIT], |row| {
                let title: Option<String> = row.get(1)?;
                let artists_csv: Option<String> = row.get(5)?;
                Ok(SearchTrackHit {
                    track_id: row.get(0)?,
                    title: title.unwrap_or_default(),
                    album_name: row.get(2)?,
                    duration_ms: row.get(3)?,
                    album_art_path: row.get(4)?,
                    artists: artists_csv
                        .map(|s| s.split("||").map(String::from).collect())
                        .unwrap_or_default(),
                })
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?
    };

    let albums = {
        let mut stmt = conn
            .prepare(
                "SELECT t.album_name, COUNT(t.id),
                        (SELECT t2.album_art_path FROM tracks t2
                         WHERE t2.album_name = t.album_name AND t2.album_art_path IS NOT NULL
                         LIMIT 1)
                 FROM tracks t
                 WHERE t.album_name IS NOT NULL AND t.album_name != ''
                   AND t.album_name LIKE ?1 ESCAPE '\\'
                 GROUP BY t.album_name
                 ORDER BY t.album_name COLLATE NOCASE
                 LIMIT ?2",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![like, PER_TYPE_LIMIT], |row| {
                Ok(SearchAlbumHit {
                    name: row.get(0)?,
                    track_count: row.get(1)?,
                    album_art_path: row.get(2)?,
                })
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?
    };

    let artists = {
        let mut stmt = conn
            .prepare(
                "SELECT a.id, a.name, COUNT(DISTINCT ta.track_id)
                 FROM artists a
                 LEFT JOIN track_artists ta ON ta.artist_id = a.id
                 WHERE a.name LIKE ?1 ESCAPE '\\'
                 GROUP BY a.id, a.name
                 HAVING COUNT(DISTINCT ta.track_id) > 0
                 ORDER BY a.name COLLATE NOCASE
                 LIMIT ?2",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![like, PER_TYPE_LIMIT], |row| {
                Ok(SearchArtistHit {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    track_count: row.get(2)?,
                })
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?
    };

    let playlists = {
        let mut stmt = conn
            .prepare(
                "SELECT p.id, p.name,
                        (SELECT COUNT(*) FROM playlist_tracks pt WHERE pt.playlist_id = p.id)
                 FROM playlists p
                 WHERE p.name LIKE ?1 ESCAPE '\\'
                 ORDER BY p.name COLLATE NOCASE
                 LIMIT ?2",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![like, PER_TYPE_LIMIT], |row| {
                Ok(SearchPlaylistHit {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    track_count: row.get(2)?,
                })
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?
    };

    Ok(SearchResults {
        tracks,
        albums,
        artists,
        playlists,
    })
}
