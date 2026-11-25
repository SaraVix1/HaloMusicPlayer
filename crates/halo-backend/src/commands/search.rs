//! Ctrl+K search. Every word of the query must match (in any order, any
//! field): songs by title / artist / album, albums by name / album artist,
//! artists and playlists by name. SQL narrows to candidates with `LIKE`
//! (ASCII case-insensitive); Rust then ranks them — exact > starts-with >
//! a word starts-with > contains, title above artist above album — with
//! play count and then name breaking ties, and picks the Top result.

use rusqlite::{Connection, ToSql};
use serde::Serialize;
use std::cmp::Ordering;
use std::sync::Mutex;

#[derive(Debug, Clone, Serialize)]
pub struct SearchTrackHit {
    pub track_id: i64,
    pub title: String,
    pub album_name: Option<String>,
    pub artists: Vec<String>,
    pub album_art_path: Option<String>,
    pub duration_ms: Option<i64>,
    pub play_count: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct SearchAlbumHit {
    pub name: String,
    pub album_art_path: Option<String>,
    pub track_count: i64,
    /// Album artists, or the track artists when none are tagged.
    pub artists: Vec<String>,
    pub year: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SearchArtistHit {
    pub id: i64,
    pub name: String,
    pub track_count: i64,
    /// Art from one of the artist's albums (for a round avatar).
    pub album_art_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SearchPlaylistHit {
    pub id: i64,
    pub name: String,
    pub track_count: i64,
}

/// The single best hit: an index into that kind's list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum TopHit {
    Track(usize),
    Album(usize),
    Artist(usize),
    Playlist(usize),
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct SearchResults {
    pub tracks: Vec<SearchTrackHit>,
    pub albums: Vec<SearchAlbumHit>,
    pub artists: Vec<SearchArtistHit>,
    pub playlists: Vec<SearchPlaylistHit>,
    pub top: Option<TopHit>,
}

const TRACKS: usize = 6;
const ALBUMS: usize = 4;
const ARTISTS: usize = 4;
const PLAYLISTS: usize = 3;
/// Rows each SQL query may return for ranking.
const CANDIDATES: i64 = 200;
const MAX_WORDS: usize = 6;

// ── Scoring ──────────────────────────────────────────────────────────────────

/// How well `text` matches `query` (both compared lower-case): exact 100,
/// starts with 80, a word starts with 60, contains 40, otherwise 20 per
/// cent of the query's words it contains (the rest matched elsewhere).
pub fn field_score(text: &str, query: &str) -> u32 {
    let t = text.to_lowercase();
    let q = query.to_lowercase();
    let q = q.trim();
    if q.is_empty() || t.is_empty() {
        return 0;
    }
    if t == q {
        100
    } else if t.starts_with(q) {
        80
    } else if t.split(|c: char| !c.is_alphanumeric()).any(|w| w.starts_with(q)) {
        60
    } else if t.contains(q) {
        40
    } else {
        let words: Vec<&str> = q.split_whitespace().collect();
        let hit = words.iter().filter(|w| t.contains(*w)).count();
        (20 * hit / words.len().max(1)) as u32
    }
}

/// Best score over weighted fields (`weight` in per cent).
fn best(fields: &[(&str, u32)], query: &str) -> u32 {
    fields.iter().map(|(f, w)| field_score(f, query) * w / 100).max().unwrap_or(0)
}

/// Sort by score desc, plays desc, name asc; keep `limit`. Returns the
/// top score.
fn rank<T>(v: &mut Vec<(u32, i64, String, T)>, limit: usize) -> u32 {
    v.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then(b.1.cmp(&a.1))
            .then_with(|| a.2.to_lowercase().cmp(&b.2.to_lowercase()))
    });
    v.truncate(limit);
    v.first().map_or(0, |x| x.0)
}

// ── Query ────────────────────────────────────────────────────────────────────

fn like(word: &str) -> String {
    let esc = word.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_");
    format!("%{esc}%")
}

/// `(cond for word 1) AND (cond for word 2) …`, each `cond` using `?N` for
/// its word's pattern.
fn all_words(n: usize, cond: &str) -> String {
    (1..=n).map(|i| format!("({})", cond.replace("?W", &format!("?{i}")))).collect::<Vec<_>>().join(" AND ")
}

fn split_csv(s: Option<String>) -> Vec<String> {
    s.map(|s| s.split("||").filter(|p| !p.is_empty()).map(String::from).collect()).unwrap_or_default()
}

pub fn search_library(query: String, db: &Mutex<Connection>) -> Result<SearchResults, String> {
    let q = query.trim().to_string();
    let words: Vec<String> = q.split_whitespace().take(MAX_WORDS).map(like).collect();
    if words.is_empty() {
        return Ok(SearchResults::default());
    }
    let params: Vec<&dyn ToSql> = words.iter().map(|w| w as &dyn ToSql).collect();
    let n = words.len();
    let conn = db.lock().map_err(|e| e.to_string())?;
    let err = |e: rusqlite::Error| e.to_string();

    // Songs.
    let sql = format!(
        "SELECT t.id, t.title, t.album_name, t.duration_ms, t.album_art_path, t.play_count,
                (SELECT GROUP_CONCAT(name, '||') FROM (
                 SELECT a.name FROM track_artists ta JOIN artists a ON a.id = ta.artist_id
                 WHERE ta.track_id = t.id ORDER BY a.name COLLATE NOCASE))
         FROM tracks t
         WHERE {}
         ORDER BY t.play_count DESC
         LIMIT {CANDIDATES}",
        all_words(
            n,
            "t.title LIKE ?W ESCAPE '\\' OR t.album_name LIKE ?W ESCAPE '\\'
             OR EXISTS (SELECT 1 FROM track_artists ta JOIN artists a ON a.id = ta.artist_id
                        WHERE ta.track_id = t.id AND a.name LIKE ?W ESCAPE '\\')"
        )
    );
    let mut tracks: Vec<(u32, i64, String, SearchTrackHit)> = conn
        .prepare(&sql)
        .map_err(err)?
        .query_map(params.as_slice(), |r| {
            Ok(SearchTrackHit {
                track_id: r.get(0)?,
                title: r.get::<_, Option<String>>(1)?.unwrap_or_default(),
                album_name: r.get(2)?,
                duration_ms: r.get(3)?,
                album_art_path: r.get(4)?,
                play_count: r.get(5)?,
                artists: split_csv(r.get(6)?),
            })
        })
        .map_err(err)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(err)?
        .into_iter()
        .map(|h| {
            let artists = h.artists.join(", ");
            let album = h.album_name.clone().unwrap_or_default();
            let s = best(&[(&h.title, 100), (&artists, 90), (&album, 80)], &q);
            (s, h.play_count, h.title.clone(), h)
        })
        .collect();

    // Albums.
    let sql = format!(
        "SELECT t.album_name, COUNT(t.id), MIN(t.year), SUM(t.play_count),
                (SELECT t2.album_art_path FROM tracks t2
                 WHERE t2.album_name = t.album_name AND t2.album_art_path IS NOT NULL LIMIT 1),
                (SELECT GROUP_CONCAT(name, '||') FROM (
                 SELECT DISTINCT aa.name FROM tracks t3
                 JOIN track_album_artists taa ON taa.track_id = t3.id
                 JOIN album_artists aa ON aa.id = taa.album_artist_id
                 WHERE t3.album_name = t.album_name ORDER BY aa.name COLLATE NOCASE)),
                (SELECT GROUP_CONCAT(name, '||') FROM (
                 SELECT DISTINCT a.name FROM tracks t4
                 JOIN track_artists ta ON ta.track_id = t4.id
                 JOIN artists a ON a.id = ta.artist_id
                 WHERE t4.album_name = t.album_name ORDER BY a.name COLLATE NOCASE))
         FROM tracks t
         WHERE t.album_name IS NOT NULL AND t.album_name != ''
         GROUP BY t.album_name
         HAVING {}
         LIMIT {CANDIDATES}",
        all_words(
            n,
            "t.album_name LIKE ?W ESCAPE '\\'
             OR EXISTS (SELECT 1 FROM tracks t5
                        JOIN track_album_artists taa ON taa.track_id = t5.id
                        JOIN album_artists aa ON aa.id = taa.album_artist_id
                        WHERE t5.album_name = t.album_name AND aa.name LIKE ?W ESCAPE '\\')"
        )
    );
    let mut albums: Vec<(u32, i64, String, SearchAlbumHit)> = conn
        .prepare(&sql)
        .map_err(err)?
        .query_map(params.as_slice(), |r| {
            let album_artists: Option<String> = r.get(5)?;
            let track_artists: Option<String> = r.get(6)?;
            Ok((
                r.get::<_, i64>(3)?,
                SearchAlbumHit {
                    name: r.get(0)?,
                    track_count: r.get(1)?,
                    year: r.get(2)?,
                    album_art_path: r.get(4)?,
                    artists: split_csv(album_artists.or(track_artists)),
                },
            ))
        })
        .map_err(err)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(err)?
        .into_iter()
        .map(|(plays, h)| {
            let artists = h.artists.join(", ");
            let s = best(&[(&h.name, 100), (&artists, 85)], &q);
            (s, plays, h.name.clone(), h)
        })
        .collect();

    // Artists.
    let sql = format!(
        "SELECT a.id, a.name, COUNT(DISTINCT ta.track_id), COALESCE(SUM(t.play_count), 0),
                (SELECT t2.album_art_path FROM track_artists ta2 JOIN tracks t2 ON t2.id = ta2.track_id
                 WHERE ta2.artist_id = a.id AND t2.album_art_path IS NOT NULL LIMIT 1)
         FROM artists a
         JOIN track_artists ta ON ta.artist_id = a.id
         JOIN tracks t ON t.id = ta.track_id
         WHERE {}
         GROUP BY a.id, a.name
         LIMIT {CANDIDATES}",
        all_words(n, "a.name LIKE ?W ESCAPE '\\'")
    );
    let mut artists: Vec<(u32, i64, String, SearchArtistHit)> = conn
        .prepare(&sql)
        .map_err(err)?
        .query_map(params.as_slice(), |r| {
            Ok((
                r.get::<_, i64>(3)?,
                SearchArtistHit {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    track_count: r.get(2)?,
                    album_art_path: r.get(4)?,
                },
            ))
        })
        .map_err(err)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(err)?
        .into_iter()
        .map(|(plays, h)| (field_score(&h.name, &q), plays, h.name.clone(), h))
        .collect();

    // Playlists.
    let sql = format!(
        "SELECT p.id, p.name, (SELECT COUNT(*) FROM playlist_tracks pt WHERE pt.playlist_id = p.id)
         FROM playlists p
         WHERE {}
         LIMIT {CANDIDATES}",
        all_words(n, "p.name LIKE ?W ESCAPE '\\'")
    );
    let mut playlists: Vec<(u32, i64, String, SearchPlaylistHit)> = conn
        .prepare(&sql)
        .map_err(err)?
        .query_map(params.as_slice(), |r| {
            Ok(SearchPlaylistHit { id: r.get(0)?, name: r.get(1)?, track_count: r.get(2)? })
        })
        .map_err(err)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(err)?
        .into_iter()
        .map(|h| (field_score(&h.name, &q), h.track_count, h.name.clone(), h))
        .collect();

    let scores = [
        (rank(&mut artists, ARTISTS), TopHit::Artist(0), !artists.is_empty()),
        (rank(&mut albums, ALBUMS), TopHit::Album(0), !albums.is_empty()),
        (rank(&mut tracks, TRACKS), TopHit::Track(0), !tracks.is_empty()),
        (rank(&mut playlists, PLAYLISTS), TopHit::Playlist(0), !playlists.is_empty()),
    ];
    // Highest score wins; on a tie the earlier kind (artist, album, song,
    // playlist) — an exact artist name beats songs merely by that artist.
    let top = scores
        .iter()
        .filter(|(_, _, any)| *any)
        .max_by(|a, b| a.0.cmp(&b.0).then(Ordering::Greater))
        .map(|(_, hit, _)| *hit);

    Ok(SearchResults {
        tracks: tracks.into_iter().map(|x| x.3).collect(),
        albums: albums.into_iter().map(|x| x.3).collect(),
        artists: artists.into_iter().map(|x| x.3).collect(),
        playlists: playlists.into_iter().map(|x| x.3).collect(),
        top,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::params;

    /// Fresh DB in its own temp dir (tests run in parallel: unique names).
    fn test_db(name: &str) -> Mutex<Connection> {
        static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("halo-test-search-{name}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Mutex::new(crate::db::open(dir).unwrap())
    }

    /// (title, album, artist, plays)
    fn add(db: &Mutex<Connection>, rows: &[(&str, &str, &str, i64)]) {
        let conn = db.lock().unwrap();
        for (i, (title, album, artist, plays)) in rows.iter().enumerate() {
            conn.execute(
                "INSERT INTO tracks (file_path, title, album_name, play_count) VALUES (?1, ?2, ?3, ?4)",
                params![format!("/m/{i}.mp3"), title, album, plays],
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

    fn library() -> Mutex<Connection> {
        let db = test_db("lib");
        add(
            &db,
            &[
                ("Yamunai Aatrile", "Thalapathi", "Ilaiyaraaja", 3),
                ("Rakkamma Kaiya Thattu", "Thalapathi", "Ilaiyaraaja", 9),
                ("Chinna Chinna Vanna Kuyil", "Mouna Ragam", "Ilaiyaraaja", 1),
                ("Kadhal Rojave", "Roja", "A. R. Rahman", 5),
                ("Raja Raja Chozhan", "Rettai Vaal Kuruvi", "Ilaiyaraaja", 0),
            ],
        );
        db
    }

    fn titles(r: &SearchResults) -> Vec<&str> {
        r.tracks.iter().map(|t| t.title.as_str()).collect()
    }

    #[test]
    fn field_score_orders_match_kinds() {
        assert_eq!(field_score("Roja", "roja"), 100);
        assert_eq!(field_score("Rojave", "roja"), 80);
        assert_eq!(field_score("Kadhal Rojave", "roja"), 60);
        assert_eq!(field_score("Sarojakka", "roja"), 40);
        assert_eq!(field_score("Thalapathi", "yamunai thalapathi"), 10);
        assert_eq!(field_score("Thalapathi", "xyz"), 0);
    }

    #[test]
    fn songs_match_on_artist_and_album_too() {
        let r = search_library("ilaiyaraaja".into(), &library()).unwrap();
        assert_eq!(r.tracks.len(), 4);
        // Same score (artist match): most played first.
        assert_eq!(titles(&r)[0], "Rakkamma Kaiya Thattu");
        // The artist itself is the top result.
        assert_eq!(r.top, Some(TopHit::Artist(0)));
        assert_eq!(r.artists[0].name, "Ilaiyaraaja");
        assert_eq!(r.artists[0].track_count, 4);
    }

    #[test]
    fn every_word_must_match_somewhere() {
        // Title word + album word.
        let r = search_library("yamunai thalapathi".into(), &library()).unwrap();
        assert_eq!(titles(&r), ["Yamunai Aatrile"]);
        // Artist word + album word, any order.
        let r = search_library("thalapathi ilaiyaraaja".into(), &library()).unwrap();
        assert_eq!(r.tracks.len(), 2);
        assert!(search_library("yamunai roja".into(), &library()).unwrap().tracks.is_empty());
    }

    #[test]
    fn exact_and_prefix_beat_contains() {
        let r = search_library("raja".into(), &library()).unwrap();
        // The title that starts with "raja" ranks first.
        assert_eq!(titles(&r)[0], "Raja Raja Chozhan");
        let r = search_library("roja".into(), &library()).unwrap();
        // Album "Roja" (exact) is the top result over the song "Kadhal Rojave".
        assert_eq!(r.top, Some(TopHit::Album(0)));
        assert_eq!(r.albums[0].name, "Roja");
        assert_eq!(r.albums[0].artists, ["A. R. Rahman"]);
    }

    #[test]
    fn empty_and_wildcard_queries() {
        let db = library();
        assert!(search_library("   ".into(), &db).unwrap().top.is_none());
        // % and _ are literal, not wildcards.
        assert!(search_library("%".into(), &db).unwrap().tracks.is_empty());
        assert!(search_library("_".into(), &db).unwrap().tracks.is_empty());
    }

    #[test]
    fn limits_per_kind() {
        let db = test_db("limits");
        let rows: Vec<(String, String)> = (0..20).map(|i| (format!("Song {i}"), format!("Album {i}"))).collect();
        let rows: Vec<_> = rows.iter().map(|(t, a)| (t.as_str(), a.as_str(), "Same Artist", 0)).collect();
        add(&db, &rows);
        let r = search_library("song".into(), &db).unwrap();
        assert_eq!(r.tracks.len(), TRACKS);
        let r = search_library("album".into(), &db).unwrap();
        assert_eq!(r.albums.len(), ALBUMS);
    }
}
