// Last.fm scrobbling support.
//
// The Last.fm *application* credentials (API key + shared secret) identify Halo
// to Last.fm. They are entered once by whoever sets up the app, in
// Settings → Advanced → Last.fm (register an app at
// https://www.last.fm/api/account/create to obtain the pair). The key/secret are
// **persisted** in app_state so they survive restarts, but each person's account
// **session is memory-only** — never persisted, cleared on exit — so every user
// on a shared machine authorises their own Last.fm account.
//
// Note: a Last.fm API key/secret identifies the app, not a person, which is why
// it is safe for everyone on the machine to share the one key.

use rusqlite::Connection;
use serde::Serialize;
use std::collections::BTreeMap;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager, State};

const API_BASE: &str = "https://ws.audioscrobbler.com/2.0/";

// ── Managed state ─────────────────────────────────────────────────────────────

pub struct LastFmState(pub Mutex<LastFmInner>);

pub struct LastFmInner {
    /// App API key, entered by the user in Settings and persisted in app_state.
    pub api_key: Option<String>,
    /// App shared secret, entered alongside the key and persisted in app_state.
    pub api_secret: Option<String>,
    pub session_key: Option<String>,
    pub username: Option<String>,
    /// Token obtained from auth.getToken, kept until the user approves + we exchange it.
    pub pending_token: Option<String>,
    /// Unix timestamp when the currently-playing track started, for scrobble timestamp.
    pub track_start: Option<i64>,
    /// Track id we last sent a now-playing update for (avoids duplicate calls).
    pub now_playing_id: Option<i64>,
}

impl LastFmInner {
    /// The app credentials, or `None` until both key and secret are set.
    fn creds(&self) -> Option<(String, String)> {
        match (&self.api_key, &self.api_secret) {
            (Some(k), Some(s)) if !k.is_empty() && !s.is_empty() => {
                Some((k.clone(), s.clone()))
            }
            _ => None,
        }
    }
}

impl LastFmState {
    /// Initialise state at startup. The app key/secret are **persisted** (loaded
    /// here from app_state), but the per-user account **session is memory-only**
    /// — never persisted — so each person who uses this shared machine
    /// authorises their own Last.fm account and the login clears on exit rather
    /// than carrying over to the next user.
    pub fn load(conn: &Connection) -> Self {
        // The session is memory-only: purge any session rows written by older
        // builds. The app key/secret, by contrast, are persisted and restored.
        kv_del(conn, "lastfm.session_key");
        kv_del(conn, "lastfm.username");
        Self(Mutex::new(LastFmInner {
            api_key: kv_get(conn, "lastfm.api_key"),
            api_secret: kv_get(conn, "lastfm.api_secret"),
            session_key: None,
            username: None,
            pending_token: None,
            track_start: None,
            now_playing_id: None,
        }))
    }
}

// ── DB helpers ────────────────────────────────────────────────────────────────

fn kv_get(conn: &Connection, key: &str) -> Option<String> {
    conn.query_row("SELECT value FROM app_state WHERE key = ?1", [key], |r| {
        r.get::<_, String>(0)
    })
    .ok()
    .filter(|v| !v.is_empty())
}

fn kv_set(conn: &Connection, key: &str, value: &str) {
    let _ = conn.execute(
        "INSERT INTO app_state (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [key, value],
    );
}

fn kv_del(conn: &Connection, key: &str) {
    let _ = conn.execute("DELETE FROM app_state WHERE key = ?1", [key]);
}

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

// ── API helpers ───────────────────────────────────────────────────────────────

/// Build the api_sig: sort all params (excluding "format"), concatenate
/// key+value pairs, append the shared secret, MD5-hash the result.
fn sign(params: &BTreeMap<&str, String>, secret: &str) -> String {
    let mut s = String::new();
    for (k, v) in params {
        if *k != "format" {
            s.push_str(k);
            s.push_str(v);
        }
    }
    s.push_str(secret);
    format!("{:x}", md5::compute(s.as_bytes()))
}

fn api_get(params: BTreeMap<&str, String>, secret: &str) -> Result<serde_json::Value, String> {
    let sig = sign(&params, secret);
    let mut parts: Vec<String> = params
        .iter()
        .map(|(k, v)| format!("{}={}", k, urlencoding::encode(v)))
        .collect();
    parts.push(format!("api_sig={}", sig));
    parts.push("format=json".into());
    let url = format!("{}?{}", API_BASE, parts.join("&"));
    let resp = ureq::get(&url).call().map_err(|e| e.to_string())?;
    let json: serde_json::Value = resp.into_json().map_err(|e| e.to_string())?;
    check_error(&json)?;
    Ok(json)
}

fn api_post(params: BTreeMap<&str, String>, secret: &str) -> Result<serde_json::Value, String> {
    let sig = sign(&params, secret);
    let mut form: Vec<(String, String)> = params
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();
    form.push(("api_sig".into(), sig));
    form.push(("format".into(), "json".into()));
    let form_refs: Vec<(&str, &str)> =
        form.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    let resp = ureq::post(API_BASE)
        .send_form(&form_refs)
        .map_err(|e| e.to_string())?;
    let json: serde_json::Value = resp.into_json().map_err(|e| e.to_string())?;
    check_error(&json)?;
    Ok(json)
}

fn check_error(json: &serde_json::Value) -> Result<(), String> {
    if let Some(code) = json.get("error") {
        let msg = json
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("unknown error");
        return Err(format!("Last.fm error {}: {}", code, msg));
    }
    Ok(())
}

// ── Track metadata fetched from DB ────────────────────────────────────────────

struct TrackMeta {
    title: String,
    artist: String,
    album: Option<String>,
    duration_ms: Option<i64>,
}

fn fetch_meta(conn: &Connection, track_id: i64) -> Option<TrackMeta> {
    let row: Option<(Option<String>, Option<String>, Option<i64>)> = conn
        .query_row(
            "SELECT title, album_name, duration_ms FROM tracks WHERE id = ?1",
            [track_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .ok();
    let (title, album, duration_ms) = row?;
    let title = title.unwrap_or_else(|| "Unknown".into());

    let artist: Option<String> = conn
        .query_row(
            "SELECT a.name FROM track_artists ta
             JOIN artists a ON a.id = ta.artist_id
             WHERE ta.track_id = ?1
             ORDER BY a.name COLLATE NOCASE LIMIT 1",
            [track_id],
            |r| r.get(0),
        )
        .ok();
    let artist = artist.unwrap_or_else(|| "Unknown".into());

    Some(TrackMeta { title, artist, album, duration_ms })
}

// ── Offline scrobble queue ────────────────────────────────────────────────────

fn queue_push(conn: &Connection, meta: &TrackMeta, timestamp: i64) {
    let _ = conn.execute(
        "INSERT INTO lastfm_scrobble_queue (title, artist, album, timestamp)
         VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![meta.title, meta.artist, meta.album, timestamp],
    );
}

fn flush_queue(conn: &Connection, api_key: &str, api_secret: &str, sk: &str) {
    let rows: Vec<(i64, String, String, Option<String>, i64)> = conn
        .prepare(
            "SELECT id, title, artist, album, timestamp
             FROM lastfm_scrobble_queue
             ORDER BY timestamp ASC LIMIT 50",
        )
        .and_then(|mut s| {
            s.query_map([], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
            })
            .and_then(|rows| rows.collect::<Result<Vec<_>, _>>())
        })
        .unwrap_or_default();

    for (id, title, artist, album, ts) in rows {
        let mut params: BTreeMap<&str, String> = BTreeMap::new();
        params.insert("method", "track.scrobble".into());
        params.insert("api_key", api_key.into());
        params.insert("sk", sk.into());
        params.insert("track[0]", title);
        params.insert("artist[0]", artist);
        params.insert("timestamp[0]", ts.to_string());
        if let Some(al) = album {
            params.insert("album[0]", al);
        }
        if api_post(params, api_secret).is_ok() {
            let _ = conn.execute("DELETE FROM lastfm_scrobble_queue WHERE id = ?1", [id]);
        } else {
            // Increment attempt counter; give up after 5 failures.
            let _ = conn.execute(
                "UPDATE lastfm_scrobble_queue SET attempts = attempts + 1 WHERE id = ?1",
                [id],
            );
            let _ = conn.execute(
                "DELETE FROM lastfm_scrobble_queue WHERE id = ?1 AND attempts >= 5",
                [id],
            );
        }
    }
}

// ── Public hooks (called from player / ticker) ────────────────────────────────

/// Called by the ticker when the playing track changes.
/// Sends a "now playing" update to Last.fm and records the start time.
pub fn on_track_start(app: &AppHandle, track_id: i64) {
    let lastfm = match app.try_state::<LastFmState>() {
        Some(s) => s,
        None => return,
    };
    // Snapshot creds + session under lock; bail if not configured/connected.
    let (api_key, api_secret, sk, already_sent) = {
        let guard = match lastfm.0.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        let Some((api_key, api_secret)) = guard.creds() else { return };
        let Some(sk) = guard.session_key.clone() else { return };
        (api_key, api_secret, sk, guard.now_playing_id == Some(track_id))
    };
    if already_sent {
        return;
    }
    let db = match app.try_state::<Mutex<Connection>>() {
        Some(s) => s,
        None => return,
    };
    let meta = {
        let conn = match db.lock() {
            Ok(c) => c,
            Err(_) => return,
        };
        fetch_meta(&conn, track_id)
    };
    let Some(meta) = meta else { return };
    let start = now_unix();
    if let Ok(mut guard) = lastfm.0.lock() {
        guard.track_start = Some(start);
        guard.now_playing_id = Some(track_id);
    }
    let title = meta.title.clone();
    let artist = meta.artist.clone();
    let album = meta.album.clone();
    std::thread::spawn(move || {
        let mut params: BTreeMap<&str, String> = BTreeMap::new();
        params.insert("method", "track.updateNowPlaying".into());
        params.insert("api_key", api_key);
        params.insert("sk", sk);
        params.insert("track", title);
        params.insert("artist", artist);
        if let Some(al) = album {
            params.insert("album", al);
        }
        let _ = api_post(params, &api_secret);
    });
}

/// Called when a track finishes (naturally or via skip after the scrobble
/// threshold). Scrobbles the track and flushes the offline queue. The caller is
/// responsible for the play-threshold check (see `should_scrobble`).
pub fn on_track_scrobble(app: &AppHandle, track_id: i64) {
    let lastfm = match app.try_state::<LastFmState>() {
        Some(s) => s,
        None => return,
    };
    // The user may not be connected yet — in that case we still want to queue
    // the scrobble offline, so we read creds + session together under the lock.
    let (app_creds, sk, track_start) = {
        let guard = match lastfm.0.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        (guard.creds(), guard.session_key.clone(), guard.track_start)
    };
    let db = match app.try_state::<Mutex<Connection>>() {
        Some(s) => s,
        None => return,
    };
    let meta = {
        let conn = match db.lock() {
            Ok(c) => c,
            Err(_) => return,
        };
        fetch_meta(&conn, track_id)
    };
    let Some(meta) = meta else { return };
    // Last.fm rule: don't scrobble tracks shorter than 30 seconds.
    if meta.duration_ms.map(|d| d < 30_000).unwrap_or(false) {
        return;
    }
    let timestamp = track_start.unwrap_or_else(now_unix);
    let title = meta.title.clone();
    let artist = meta.artist.clone();
    let album = meta.album.clone();
    let app = app.clone();
    std::thread::spawn(move || {
        let db_arc = app.try_state::<Mutex<Connection>>();
        // Only attempt a live scrobble when both credentials and a session exist.
        if let (Some((api_key, api_secret)), Some(sk)) = (app_creds, sk) {
            let mut params: BTreeMap<&str, String> = BTreeMap::new();
            params.insert("method", "track.scrobble".into());
            params.insert("api_key", api_key.clone());
            params.insert("sk", sk.clone());
            params.insert("track[0]", title.clone());
            params.insert("artist[0]", artist.clone());
            params.insert("timestamp[0]", timestamp.to_string());
            if let Some(ref al) = album {
                params.insert("album[0]", al.clone());
            }
            match api_post(params, &api_secret) {
                Ok(_) => {
                    if let Some(db) = db_arc {
                        if let Ok(conn) = db.lock() {
                            flush_queue(&conn, &api_key, &api_secret, &sk);
                        }
                    }
                }
                Err(_) => {
                    if let Some(db) = db_arc {
                        if let Ok(conn) = db.lock() {
                            queue_push(&conn, &TrackMeta { title, artist, album, duration_ms: None }, timestamp);
                        }
                    }
                }
            }
        } else {
            // Not configured/connected — queue for when they connect later.
            if let Some(db) = db_arc {
                if let Ok(conn) = db.lock() {
                    queue_push(&conn, &TrackMeta { title, artist, album, duration_ms: None }, timestamp);
                }
            }
        }
    });
}

// ── Tauri commands ────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct LastFmStatus {
    pub connected: bool,
    pub username: Option<String>,
    pub pending: bool,
    pub configured: bool,
}

#[tauri::command]
pub fn lastfm_get_status(lastfm: State<LastFmState>) -> LastFmStatus {
    let guard = lastfm.0.lock().unwrap();
    LastFmStatus {
        connected: guard.session_key.is_some(),
        username: guard.username.clone(),
        pending: guard.pending_token.is_some(),
        configured: guard.creds().is_some(),
    }
}

/// Save the app API key + shared secret (entered in Settings) and persist them.
/// Pass empty strings to clear. Disconnects any active session since the app
/// identity changed.
#[tauri::command]
pub fn lastfm_set_credentials(
    app: AppHandle,
    lastfm: State<LastFmState>,
    api_key: String,
    api_secret: String,
) -> Result<(), String> {
    let key = api_key.trim().to_string();
    let secret = api_secret.trim().to_string();
    let db = app.state::<Mutex<Connection>>();
    let conn = db.lock().map_err(|e| e.to_string())?;
    if key.is_empty() || secret.is_empty() {
        kv_del(&conn, "lastfm.api_key");
        kv_del(&conn, "lastfm.api_secret");
    } else {
        kv_set(&conn, "lastfm.api_key", &key);
        kv_set(&conn, "lastfm.api_secret", &secret);
    }
    let mut guard = lastfm.0.lock().map_err(|e| e.to_string())?;
    guard.api_key = if key.is_empty() { None } else { Some(key) };
    guard.api_secret = if secret.is_empty() { None } else { Some(secret) };
    // App identity changed — drop any in-progress or established session.
    guard.session_key = None;
    guard.username = None;
    guard.pending_token = None;
    Ok(())
}

/// Step 1 of auth: get a token from Last.fm and return the auth URL for the user to visit.
#[tauri::command]
pub fn lastfm_start_auth(lastfm: State<LastFmState>) -> Result<String, String> {
    let (api_key, api_secret) = {
        let guard = lastfm.0.lock().map_err(|e| e.to_string())?;
        guard
            .creds()
            .ok_or("Enter your Last.fm API key and secret first.")?
    };
    let mut params: BTreeMap<&str, String> = BTreeMap::new();
    params.insert("method", "auth.getToken".into());
    params.insert("api_key", api_key.clone());
    let json = api_get(params, &api_secret)?;
    let token = json["token"]
        .as_str()
        .ok_or("No token in response")?
        .to_string();
    let auth_url = format!(
        "https://www.last.fm/api/auth/?api_key={}&token={}",
        api_key, token
    );
    if let Ok(mut guard) = lastfm.0.lock() {
        guard.pending_token = Some(token);
    }
    Ok(auth_url)
}

/// Step 2 of auth: exchange the approved token for a session key.
#[tauri::command]
pub fn lastfm_complete_auth(
    lastfm: State<LastFmState>,
) -> Result<String, String> {
    let (api_key, api_secret, token) = {
        let guard = lastfm.0.lock().map_err(|e| e.to_string())?;
        let (api_key, api_secret) = guard
            .creds()
            .ok_or("Enter your Last.fm API key and secret first.")?;
        let token = guard
            .pending_token
            .clone()
            .ok_or("No pending token — call lastfm_start_auth first")?;
        (api_key, api_secret, token)
    };
    let mut params: BTreeMap<&str, String> = BTreeMap::new();
    params.insert("method", "auth.getSession".into());
    params.insert("api_key", api_key);
    params.insert("token", token);
    let json = api_get(params, &api_secret)?;
    let session = json["session"]
        .as_object()
        .ok_or("No session in response")?;
    let sk = session["key"].as_str().ok_or("No key in session")?.to_string();
    let name = session["name"]
        .as_str()
        .ok_or("No name in session")?
        .to_string();
    {
        // Session stays in memory only — never persisted, so it clears on exit.
        let mut guard = lastfm.0.lock().map_err(|e| e.to_string())?;
        guard.session_key = Some(sk);
        guard.username = Some(name.clone());
        guard.pending_token = None;
    }
    Ok(name)
}

#[tauri::command]
pub fn lastfm_logout(app: AppHandle, lastfm: State<LastFmState>) -> Result<(), String> {
    {
        let mut guard = lastfm.0.lock().map_err(|e| e.to_string())?;
        guard.session_key = None;
        guard.username = None;
        guard.pending_token = None;
    }
    let db = app.state::<Mutex<Connection>>();
    let conn = db.lock().map_err(|e| e.to_string())?;
    kv_del(&conn, "lastfm.session_key");
    kv_del(&conn, "lastfm.username");
    Ok(())
}

/// Love or unlove a track on Last.fm. Requires credentials + an active session.
#[tauri::command]
pub fn lastfm_love(
    app: AppHandle,
    lastfm: State<LastFmState>,
    track_id: i64,
    love: bool,
) -> Result<(), String> {
    let (api_key, api_secret, sk) = {
        let guard = lastfm.0.lock().map_err(|e| e.to_string())?;
        let (api_key, api_secret) = guard.creds().ok_or("Last.fm not configured.")?;
        let sk = guard
            .session_key
            .clone()
            .ok_or("Not connected to Last.fm.")?;
        (api_key, api_secret, sk)
    };
    let db = app.state::<Mutex<Connection>>();
    let meta = {
        let conn = db.lock().map_err(|e| e.to_string())?;
        fetch_meta(&conn, track_id).ok_or("Track not found.")?
    };
    let mut params: BTreeMap<&str, String> = BTreeMap::new();
    params.insert("method", if love { "track.love".into() } else { "track.unlove".into() });
    params.insert("api_key", api_key);
    params.insert("sk", sk);
    params.insert("track", meta.title);
    params.insert("artist", meta.artist);
    api_post(params, &api_secret)?;
    Ok(())
}

/// Whether the given track is currently loved on the connected Last.fm account.
/// Returns false when not connected or on any API error (best-effort).
#[tauri::command]
pub fn lastfm_is_loved(
    app: AppHandle,
    lastfm: State<LastFmState>,
    track_id: i64,
) -> Result<bool, String> {
    let (api_key, api_secret, username) = {
        let guard = lastfm.0.lock().map_err(|e| e.to_string())?;
        let Some((api_key, api_secret)) = guard.creds() else { return Ok(false) };
        match guard.username.clone() {
            Some(u) => (api_key, api_secret, u),
            None => return Ok(false),
        }
    };
    let db = app.state::<Mutex<Connection>>();
    let meta = {
        let conn = db.lock().map_err(|e| e.to_string())?;
        match fetch_meta(&conn, track_id) {
            Some(m) => m,
            None => return Ok(false),
        }
    };
    let mut params: BTreeMap<&str, String> = BTreeMap::new();
    params.insert("method", "track.getInfo".into());
    params.insert("api_key", api_key);
    params.insert("username", username);
    params.insert("track", meta.title);
    params.insert("artist", meta.artist);
    match api_get(params, &api_secret) {
        Ok(json) => Ok(json["track"]["userloved"].as_str() == Some("1")),
        Err(_) => Ok(false),
    }
}
