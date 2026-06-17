use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Mutex;
use tauri::{AppHandle, Manager, State};

#[derive(Serialize)]
pub struct LyricsLine {
    pub time_ms: u64,
    pub text: String,
}

#[derive(Serialize)]
pub struct LyricsResult {
    pub source: String,
    pub synced: bool,
    pub lines: Vec<LyricsLine>,
}

#[derive(Serialize)]
pub struct LyricsCandidate {
    pub provider: String,
    pub label: String,   // track · artist as returned by the source; may be empty
    pub synced: bool,
    pub snippet: String, // first 3 non-empty lines, timestamps stripped
    pub lyrics: String,
}

// ── LRC / plain helpers ───────────────────────────────────────────────────────

fn parse_timestamp_ms(ts: &str) -> Option<u64> {
    let (minutes, rest) = ts.trim().split_once(':')?;
    let minutes: u64 = minutes.trim().parse().ok()?;
    let (secs_str, frac_str) =
        if let Some((s, f)) = rest.split_once('.') { (s, f) } else { (rest, "0") };
    let seconds: u64 = secs_str.parse().ok()?;
    let millis: u64 = match frac_str.len() {
        0 => 0,
        1 => frac_str.parse::<u64>().ok()? * 100,
        2 => frac_str.parse::<u64>().ok()? * 10,
        _ => frac_str[..3].parse::<u64>().ok()?,
    };
    Some(minutes * 60_000 + seconds * 1_000 + millis)
}

fn parse_lrc(text: &str) -> Vec<LyricsLine> {
    let mut lines: Vec<LyricsLine> = Vec::new();
    for raw in text.lines() {
        let line = raw.trim();
        if !line.starts_with('[') { continue; }
        let Some(bracket_end) = line.find(']') else { continue };
        let timestamp = &line[1..bracket_end];
        let content = line[bracket_end + 1..].trim();
        if content.is_empty() { continue; }
        if let Some(ms) = parse_timestamp_ms(timestamp) {
            lines.push(LyricsLine { time_ms: ms, text: content.to_string() });
        }
    }
    lines.sort_by_key(|l| l.time_ms);
    lines
}

fn is_lrc(text: &str) -> bool {
    text.lines().take(20).any(|l| {
        let t = l.trim();
        t.starts_with('[') && t.contains(':') && t.contains(']')
    })
}

fn plain_to_lines(text: &str) -> Vec<LyricsLine> {
    text.lines().map(|l| LyricsLine { time_ms: 0, text: l.to_string() }).collect()
}

fn build_result(text: &str, source: &str) -> LyricsResult {
    if is_lrc(text) {
        let lines = parse_lrc(text);
        let synced = !lines.is_empty();
        LyricsResult { source: source.to_string(), synced, lines }
    } else {
        LyricsResult { source: source.to_string(), synced: false, lines: plain_to_lines(text) }
    }
}

/// 3-line plaintext preview, LRC timestamps stripped.
fn make_snippet(lyrics: &str) -> String {
    lyrics
        .lines()
        .filter_map(|raw| {
            let l = raw.trim();
            let content = if l.starts_with('[') {
                l.find(']').map(|e| l[e + 1..].trim()).unwrap_or(l)
            } else {
                l
            };
            if content.is_empty() { None } else { Some(content) }
        })
        .take(3)
        .collect::<Vec<_>>()
        .join("\n")
}

// ── LRCLIB ───────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct LrclibTrack {
    #[serde(rename = "trackName")]
    track_name: Option<String>,
    #[serde(rename = "artistName")]
    artist_name: Option<String>,
    #[serde(rename = "syncedLyrics")]
    synced_lyrics: Option<String>,
    #[serde(rename = "plainLyrics")]
    plain_lyrics: Option<String>,
    #[serde(default)]
    instrumental: bool,
}

fn lrclib_lyrics(track: &LrclibTrack) -> Option<String> {
    if track.instrumental { return None; }
    if let Some(s) = &track.synced_lyrics {
        if !s.trim().is_empty() { return Some(s.clone()); }
    }
    if let Some(p) = &track.plain_lyrics {
        if !p.trim().is_empty() { return Some(p.clone()); }
    }
    None
}

fn lrclib_label(track: &LrclibTrack) -> String {
    match (&track.track_name, &track.artist_name) {
        (Some(t), Some(a)) => format!("{} · {}", t, a),
        (Some(t), None) => t.clone(),
        _ => String::new(),
    }
}

const USER_AGENT: &str = "Halo Music Player/0.1 (https://github.com/user/halo)";

fn mark_searched(db: &Mutex<Connection>, track_id: i64) {
    if let Ok(conn) = db.lock() {
        let _ = conn.execute(
            "UPDATE tracks SET lyrics = '' WHERE id = ?1 AND (lyrics IS NULL OR lyrics = '')",
            [track_id],
        );
    }
}

/// Returns up to 5 distinct (label, lyrics) pairs from LRCLIB.
fn safe_query_lrclib(
    title: &str,
    artist: &str,
    album: &str,
    duration_secs: u64,
) -> Result<Vec<(String, String)>, String> {
    let (title, artist, album) = (title.to_owned(), artist.to_owned(), album.to_owned());
    std::panic::catch_unwind(move || query_lrclib(&title, &artist, &album, duration_secs))
        .map_err(|_| "LRCLIB: internal panic".to_string())?
}

fn query_lrclib(
    title: &str,
    artist: &str,
    album: &str,
    duration_secs: u64,
) -> Result<Vec<(String, String)>, String> {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(std::time::Duration::from_secs(8))
        .timeout_read(std::time::Duration::from_secs(12))
        .build();

    let mut results: Vec<(String, String)> = Vec::new();
    // Dedup by first 80 chars so an exact-match that also appears in fuzzy search isn't shown twice.
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

    macro_rules! push_unique {
        ($label:expr, $lyr:expr) => {{
            let lyr: String = $lyr;
            let key: String = lyr.chars().take(80).collect();
            if seen.insert(key) {
                results.push(($label, lyr));
            }
        }};
    }

    // 1. Exact match (single track)
    let exact = agent
        .get("https://lrclib.net/api/get")
        .query("track_name", title)
        .query("artist_name", artist)
        .query("album_name", album)
        .query("duration", &duration_secs.to_string())
        .set("User-Agent", USER_AGENT)
        .call();

    match exact {
        Ok(resp) => {
            if let Ok(track) = resp.into_json::<LrclibTrack>() {
                if let Some(lyr) = lrclib_lyrics(&track) {
                    push_unique!(lrclib_label(&track), lyr);
                }
            }
        }
        Err(ureq::Error::Status(404, _)) => {}
        Err(e) => return Err(format!("Network error: {e}")),
    }

    // 2. Fuzzy search (array) — add distinct results up to 5 total
    let search = agent
        .get("https://lrclib.net/api/search")
        .query("track_name", title)
        .query("artist_name", artist)
        .set("User-Agent", USER_AGENT)
        .call();

    match search {
        Ok(resp) => {
            if let Ok(tracks) = resp.into_json::<Vec<LrclibTrack>>() {
                for track in tracks {
                    if results.len() >= 5 { break; }
                    if let Some(lyr) = lrclib_lyrics(&track) {
                        push_unique!(lrclib_label(&track), lyr);
                    }
                }
            }
        }
        Err(ureq::Error::Status(_, _)) => {}
        Err(e) => return Err(format!("Network error: {e}")),
    }

    Ok(results)
}

// ── JioSaavn (saavn.dev search + jiosaavn.com internal lyrics API) ───────────

// Search results — lyricsId is often null here; we fall back to a song-detail
// fetch if it's missing (the detail endpoint always populates it).
#[derive(Deserialize)]
struct SaavnSongResult {
    id: Option<String>,
    name: Option<String>,
    #[serde(rename = "lyricsId")]
    lyrics_id: Option<String>,
}

#[derive(Deserialize)]
struct SaavnSearchData {
    results: Option<Vec<SaavnSongResult>>,
}

#[derive(Deserialize)]
struct SaavnSearchResponse {
    // saavn.dev returns `success: bool`, not the old `status: "SUCCESS"` string
    success: Option<bool>,
    data: Option<SaavnSearchData>,
}

// Song-detail response: GET /api/songs/{id} → { success, data: [SongModel] }
#[derive(Deserialize)]
struct SaavnSongDetail {
    #[serde(rename = "hasLyrics")]
    has_lyrics: Option<bool>,
    #[serde(rename = "lyricsId")]
    lyrics_id: Option<String>,
}

#[derive(Deserialize)]
struct SaavnSongDetailResponse {
    success: Option<bool>,
    data: Option<Vec<SaavnSongDetail>>,
}

// Flat response from jiosaavn.com/api.php?__call=lyrics.getLyrics
#[derive(Deserialize)]
struct SaavnInternalLyrics {
    lyrics: Option<String>,
}

fn clean_saavn_lyrics(s: &str) -> String {
    s.replace("<br />", "\n")
        .replace("<br/>", "\n")
        .replace("<br>", "\n")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&nbsp;", " ")
}

/// Fetches full song detail from saavn.dev to get lyricsId, which is not
/// always populated in search results.
fn saavn_lyrics_id_for_song(agent: &ureq::Agent, song_id: &str) -> Option<String> {
    let resp = agent
        .get(&format!("https://saavn.dev/api/songs/{}", song_id))
        .set("User-Agent", USER_AGENT)
        .call()
        .ok()?;
    let detail: SaavnSongDetailResponse = resp.into_json().ok()?;
    if detail.success != Some(true) {
        return None;
    }
    detail
        .data?
        .into_iter()
        .next()
        .filter(|d| d.has_lyrics.unwrap_or(false))
        .and_then(|d| d.lyrics_id)
        .filter(|id| !id.is_empty())
}

/// Returns up to 5 (label, lyrics) pairs from JioSaavn.
fn safe_query_saavn(title: &str, artist: &str) -> Result<Vec<(String, String)>, String> {
    let (title, artist) = (title.to_owned(), artist.to_owned());
    std::panic::catch_unwind(move || query_saavn(&title, &artist))
        .map_err(|_| "JioSaavn: internal panic".to_string())?
}

fn query_saavn(title: &str, artist: &str) -> Result<Vec<(String, String)>, String> {
    let query =
        if artist.is_empty() { title.to_owned() } else { format!("{} {}", title, artist) };

    let agent = ureq::AgentBuilder::new()
        .timeout_connect(std::time::Duration::from_secs(8))
        .timeout_read(std::time::Duration::from_secs(12))
        .build();

    let songs: Vec<SaavnSongResult> = match agent
        .get("https://saavn.dev/api/search/songs")
        .query("query", &query)
        .query("limit", "5")
        .set("User-Agent", USER_AGENT)
        .call()
    {
        Ok(resp) => match resp.into_json::<SaavnSearchResponse>() {
            Ok(r) if r.success == Some(true) => {
                r.data.and_then(|d| d.results).unwrap_or_default()
            }
            _ => return Ok(vec![]),
        },
        Err(ureq::Error::Status(_, _)) => return Ok(vec![]),
        Err(e) => return Err(format!("JioSaavn search: {e}")),
    };

    let mut results: Vec<(String, String)> = Vec::new();

    for song in songs.into_iter().take(5) {
        let label = song.name.unwrap_or_default();

        // lyricsId from the search result; if absent, fetch the full song detail.
        let lyrics_id = match song.lyrics_id.filter(|id| !id.is_empty()) {
            Some(id) => Some(id),
            None => song.id.as_deref().and_then(|sid| saavn_lyrics_id_for_song(&agent, sid)),
        };
        let Some(lyrics_id) = lyrics_id else { continue };

        // saavn.dev exposes no lyrics endpoint — call JioSaavn's internal API
        let url = format!(
            "https://www.jiosaavn.com/api.php?__call=lyrics.getLyrics&lyrics_id={}&_format=json&_marker=0&api_version=4&ctx=web6dot0",
            urlencoding::encode(&lyrics_id)
        );
        match agent.get(&url).set("User-Agent", USER_AGENT).call() {
            Ok(resp) => {
                if let Ok(lr) = resp.into_json::<SaavnInternalLyrics>() {
                    if let Some(lyr) = lr.lyrics {
                        let lyr = clean_saavn_lyrics(lyr.trim());
                        if !lyr.is_empty() {
                            results.push((label, lyr));
                        }
                    }
                }
            }
            Err(ureq::Error::Status(_, _)) => continue,
            Err(e) => return Err(format!("JioSaavn lyrics: {e}")),
        }
    }

    Ok(results)
}

// ── Lyrics.ovh ────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct LyricsOvhResponse {
    lyrics: Option<String>,
}

/// Returns 0 or 1 (label, lyrics) pair (single lookup by design).
fn safe_query_lyrics_ovh(title: &str, artist: &str) -> Result<Vec<(String, String)>, String> {
    let (title, artist) = (title.to_owned(), artist.to_owned());
    std::panic::catch_unwind(move || query_lyrics_ovh(&title, &artist))
        .map_err(|_| "Lyrics.ovh: internal panic".to_string())?
}

fn query_lyrics_ovh(title: &str, artist: &str) -> Result<Vec<(String, String)>, String> {
    if artist.is_empty() || title.is_empty() {
        return Ok(vec![]);
    }
    let url = format!(
        "https://api.lyrics.ovh/v1/{}/{}",
        urlencoding::encode(artist),
        urlencoding::encode(title)
    );
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(std::time::Duration::from_secs(8))
        .timeout_read(std::time::Duration::from_secs(12))
        .build();
    match agent.get(&url).set("User-Agent", USER_AGENT).call() {
        Ok(resp) => {
            if let Ok(r) = resp.into_json::<LyricsOvhResponse>() {
                if let Some(lyr) = r.lyrics {
                    let lyr = lyr.trim().to_string();
                    if !lyr.is_empty() {
                        // Label is the query itself — Lyrics.ovh is a point lookup
                        let label = format!("{} - {}", artist, title);
                        return Ok(vec![(label, lyr)]);
                    }
                }
            }
            Ok(vec![])
        }
        Err(ureq::Error::Status(404, _)) | Err(ureq::Error::Status(503, _)) => Ok(vec![]),
        Err(e) => Err(format!("Lyrics.ovh: {e}")),
    }
}

// ── Tauri commands ────────────────────────────────────────────────────────────

/// Auto-fetch: LRCLIB first (synced preferred), JioSaavn as fallback.
/// Called automatically on track load when no lyrics are cached.
#[tauri::command]
pub async fn fetch_lyrics_online(app: AppHandle, track_id: i64) -> Result<LyricsResult, String> {
    let db = app.state::<Mutex<Connection>>();

    let (title, artist, album, duration_ms): (
        Option<String>,
        Option<String>,
        Option<String>,
        Option<i64>,
    ) = {
        let conn = db.lock().map_err(|e| e.to_string())?;
        conn.query_row(
            "SELECT t.title,
                    (SELECT a.name FROM track_artists ta
                     JOIN artists a ON a.id = ta.artist_id
                     WHERE ta.track_id = t.id LIMIT 1),
                    t.album_name,
                    t.duration_ms
             FROM tracks t WHERE t.id = ?1",
            [track_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .map_err(|e| e.to_string())?
    };

    let title = title.unwrap_or_default();
    let artist = artist.unwrap_or_default();
    let album = album.unwrap_or_default();
    let duration_secs = duration_ms.unwrap_or(0) as u64 / 1000;

    if title.is_empty() {
        return Ok(LyricsResult { source: "none".into(), synced: false, lines: vec![] });
    }

    let first = tokio::task::spawn_blocking(move || {
        // LRCLIB first — purpose-built for lyrics, supports LRC synced format
        let lrclib = safe_query_lrclib(&title, &artist, &album, duration_secs)
            .unwrap_or_default();
        if !lrclib.is_empty() {
            // Prefer synced (LRC) over plain
            let pick = lrclib
                .iter()
                .find(|(_, lyr)| is_lrc(lyr))
                .or_else(|| lrclib.first());
            if let Some((_, lyrics)) = pick {
                return Some((lyrics.clone(), "lrclib".to_string()));
            }
        }

        // Fallback: JioSaavn
        safe_query_saavn(&title, &artist)
            .unwrap_or_default()
            .into_iter()
            .next()
            .map(|(_, lyrics)| (lyrics, "jiosaavn".to_string()))
    })
    .await
    .map_err(|e| e.to_string())?;

    match first {
        Some((lyrics, source)) => {
            let conn = db.lock().map_err(|e| e.to_string())?;
            conn.execute(
                "UPDATE tracks SET lyrics = ?1 WHERE id = ?2",
                rusqlite::params![&lyrics, track_id],
            )
            .map_err(|e| e.to_string())?;
            Ok(build_result(&lyrics, &source))
        }
        None => {
            mark_searched(&db, track_id);
            Ok(LyricsResult { source: "not_found".into(), synced: false, lines: vec![] })
        }
    }
}

/// Manual search: queries all providers concurrently and returns every distinct
/// candidate so the user can pick the right one.
#[tauri::command]
pub async fn search_lyrics_providers(
    app: AppHandle,
    track_id: i64,
    title: String,
    artist: String,
    album: String,
) -> Result<Vec<LyricsCandidate>, String> {
    if title.trim().is_empty() {
        return Ok(vec![]);
    }

    let db = app.state::<Mutex<Connection>>();
    let duration_secs: u64 = {
        let conn = db.lock().map_err(|e| e.to_string())?;
        conn.query_row(
            "SELECT duration_ms FROM tracks WHERE id = ?1",
            [track_id],
            |row| row.get::<_, Option<i64>>(0),
        )
        .ok()
        .flatten()
        .unwrap_or(0) as u64
            / 1000
    };

    let (t1, a1, al1) = (title.clone(), artist.clone(), album.clone());
    let (t2, a2) = (title.clone(), artist.clone());
    let (t3, a3) = (title, artist);

    // Tasks start immediately on the blocking thread pool; awaiting in order is fine.
    let lrclib_task = tokio::task::spawn_blocking(move || -> Vec<(String, String, String)> {
        safe_query_lrclib(&t1, &a1, &al1, duration_secs)
            .unwrap_or_default()
            .into_iter()
            .map(|(label, lyrics)| ("lrclib".to_string(), label, lyrics))
            .collect()
    });
    let saavn_task = tokio::task::spawn_blocking(move || -> Vec<(String, String, String)> {
        safe_query_saavn(&t2, &a2)
            .unwrap_or_default()
            .into_iter()
            .map(|(label, lyrics)| ("jiosaavn".to_string(), label, lyrics))
            .collect()
    });
    let ovh_task = tokio::task::spawn_blocking(move || -> Vec<(String, String, String)> {
        safe_query_lyrics_ovh(&t3, &a3)
            .unwrap_or_default()
            .into_iter()
            .map(|(label, lyrics)| ("lyricsovh".to_string(), label, lyrics))
            .collect()
    });

    let lrclib_results = lrclib_task.await.unwrap_or_default();
    let saavn_results = saavn_task.await.unwrap_or_default();
    let ovh_results = ovh_task.await.unwrap_or_default();

    let candidates = lrclib_results
        .into_iter()
        .chain(saavn_results)
        .chain(ovh_results)
        .map(|(provider, label, lyrics)| {
            let synced = is_lrc(&lyrics);
            let snippet = make_snippet(&lyrics);
            LyricsCandidate { provider, label, synced, snippet, lyrics }
        })
        .collect();

    Ok(candidates)
}

// ── DB / file commands ────────────────────────────────────────────────────────

#[tauri::command]
pub fn get_lyrics(app: AppHandle, track_id: i64) -> Result<LyricsResult, String> {
    let db = app.state::<Mutex<Connection>>();
    let conn = db.lock().map_err(|e| e.to_string())?;

    let row: Option<(Option<String>, String)> = conn
        .query_row(
            "SELECT lyrics, file_path FROM tracks WHERE id = ?1",
            [track_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .ok();

    let Some((db_lyrics, file_path)) = row else {
        return Ok(LyricsResult { source: "none".into(), synced: false, lines: vec![] });
    };

    // "" = already searched, nothing found; NULL = never searched
    let already_searched =
        db_lyrics.as_deref().map(|s| s.trim().is_empty()).unwrap_or(false);

    if let Some(lyrics) = &db_lyrics {
        if !lyrics.trim().is_empty() {
            return Ok(build_result(lyrics, "database"));
        }
    }

    let lrc_path = Path::new(&file_path).with_extension("lrc");
    if lrc_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&lrc_path) {
            if !content.trim().is_empty() {
                return Ok(build_result(&content, "lrc_file"));
            }
        }
    }

    let source = if already_searched { "not_found" } else { "none" };
    Ok(LyricsResult { source: source.into(), synced: false, lines: vec![] })
}

#[tauri::command]
pub fn save_lyrics(
    db: State<Mutex<Connection>>,
    track_id: i64,
    lyrics: String,
) -> Result<(), String> {
    let conn = db.lock().map_err(|e| e.to_string())?;
    let text: Option<&str> = if lyrics.trim().is_empty() { None } else { Some(&lyrics) };
    conn.execute(
        "UPDATE tracks SET lyrics = ?1 WHERE id = ?2",
        rusqlite::params![text, track_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_timestamp_hundredths() {
        assert_eq!(parse_timestamp_ms("00:12.50"), Some(12_500));
    }

    #[test]
    fn parse_timestamp_milliseconds() {
        assert_eq!(parse_timestamp_ms("01:23.456"), Some(83_456));
    }

    #[test]
    fn parse_lrc_sorts_and_skips_empty() {
        let lrc = "[00:05.00]Second\n[00:01.00]First\n[00:03.00]\n";
        let lines = parse_lrc(lrc);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].text, "First");
        assert_eq!(lines[0].time_ms, 1_000);
        assert_eq!(lines[1].text, "Second");
    }

    #[test]
    fn is_lrc_detects_format() {
        assert!(is_lrc("[00:01.00]Hello"));
        assert!(!is_lrc("Just plain\nlyrics here"));
    }

    #[test]
    fn make_snippet_strips_timestamps() {
        let lrc = "[00:01.00]First\n[00:02.00]Second\n[00:03.00]Third\n[00:04.00]Fourth\n";
        let s = make_snippet(lrc);
        assert_eq!(s, "First\nSecond\nThird");
    }
}
