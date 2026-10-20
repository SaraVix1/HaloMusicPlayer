use crate::audio::{PlaybackStatus, PlayerHandle, PlayerState};
use crate::commands::stats;
use halo_core::audio_event::AudioEvent;
use halo_core::playback::{is_played, should_scrobble};
use halo_core::queue::{next_index, should_crossfade, RepeatMode, ShuffleOrder};
use rusqlite::{params, Connection};
use serde::Serialize;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, State};

const KEY_CURRENT_INDEX: &str = "player.current_index";
const KEY_SHUFFLE: &str = "player.shuffle";
/// Shuffled play order (JSON array of queue indices) — written only when the
/// permutation changes.
const KEY_SHUFFLE_ORDER: &str = "player.shuffle_order";
/// Cursor into the shuffle order — written on every Next / Previous.
const KEY_SHUFFLE_POS: &str = "player.shuffle_pos";
const KEY_REPEAT: &str = "player.repeat";
const KEY_VOLUME: &str = "player.volume";
const KEY_CROSSFADE_MS: &str = "player.crossfade_ms";
const KEY_POSITION_MS: &str = "player.position_ms";
const KEY_STATUS: &str = "player.status";
const KEY_RESUME_ON_LAUNCH: &str = "playback.resume_on_launch";

#[derive(Serialize)]
pub struct QueueTrack {
    pub queue_id: i64,
    pub position: i64,
    pub track_id: i64,
    pub title: Option<String>,
    pub album_name: Option<String>,
    pub artists: Vec<String>,
    pub duration_ms: Option<i64>,
    pub album_art_path: Option<String>,
    pub file_path: String,
}

#[derive(Clone, Serialize)]
pub struct CurrentTrack {
    pub track_id: i64,
    pub title: Option<String>,
    pub album_name: Option<String>,
    pub artists: Vec<String>,
    pub composers: Vec<String>,
    pub album_art_path: Option<String>,
    pub file_path: String,
    pub rating: i64,
}

#[derive(Clone, Serialize)]
pub struct FullPlayerState {
    #[serde(flatten)]
    pub player: PlayerState,
    pub current_index: Option<i64>,
    pub queue_length: i64,
    pub shuffle: bool,
    pub repeat: RepeatMode,
    pub crossfade_ms: i64,
    pub current_track: Option<CurrentTrack>,
    pub sleep_timer: crate::commands::sleep_timer::SleepTimerInfo,
}

fn read_int(conn: &Connection, key: &str) -> Option<i64> {
    conn.query_row(
        "SELECT value FROM app_state WHERE key = ?1",
        [key],
        |row| row.get::<_, String>(0),
    )
    .ok()
    .and_then(|s| s.parse::<i64>().ok())
}

fn read_string(conn: &Connection, key: &str) -> Option<String> {
    conn.query_row(
        "SELECT value FROM app_state WHERE key = ?1",
        [key],
        |row| row.get::<_, String>(0),
    )
    .ok()
}

fn write_string(conn: &Connection, key: &str, value: &str) -> Result<(), String> {
    conn.execute(
        "INSERT INTO app_state (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [key, value],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

fn delete_key(conn: &Connection, key: &str) -> Result<(), String> {
    conn.execute("DELETE FROM app_state WHERE key = ?1", [key])
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn queue_length(conn: &Connection) -> Result<i64, String> {
    conn.query_row("SELECT COUNT(*) FROM queue", [], |row| row.get(0))
        .map_err(|e| e.to_string())
}

fn track_at_index(conn: &Connection, index: i64) -> Result<Option<(i64, String)>, String> {
    let row = |sql: &str| {
        conn.query_row(sql, [index], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })
        .map(Some)
        .or_else(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => Ok(None),
            other => Err(other.to_string()),
        })
    };
    // Fast path: every queue writer keeps positions contiguous (0..n-1), so the
    // index-th track is the row at that position — an indexed point lookup.
    if let Some(hit) = row(
        "SELECT q.track_id, t.file_path FROM queue q
         JOIN tracks t ON t.id = q.track_id
         WHERE q.position = ?1",
    )? {
        return Ok(Some(hit));
    }
    // Fallback for a queue with gaps in its positions (shouldn't happen, but
    // stay correct): the index-th row in position order.
    row(
        "SELECT q.track_id, t.file_path FROM queue q
         JOIN tracks t ON t.id = q.track_id
         ORDER BY q.position ASC LIMIT 1 OFFSET ?1",
    )
}

// ── Shuffle order ─────────────────────────────────────────────────────────────
//
// In shuffle mode the play order is a Fisher-Yates permutation of the queue
// (`halo_core::queue::ShuffleOrder`): Next walks forward, Previous walks back,
// so nothing repeats until every track has played and Previous retraces
// exactly what Next played. Queue edits keep it in sync; if it is ever stale
// (queue length changed some other way) it is regenerated from the current
// track.
//
// Large queues: the working copy lives in memory (`ShuffleState`) so Next /
// Previous never re-read or re-parse it. Persistence is split so the hot path
// stays tiny: the cursor (`player.shuffle_pos`) is written on every move, the
// full order (`player.shuffle_order`, ~330 KB of JSON at 50k tracks) only when
// the permutation itself changes. Lock order: DB connection first, then
// `ShuffleState` — never the other way round.

/// In-memory working copy of the shuffle order. `None` until first use (then
/// loaded + validated from the DB once) or after shuffle is turned off.
#[derive(Default)]
pub struct ShuffleState(Mutex<Option<ShuffleOrder>>);

fn shuffle_enabled(conn: &Connection) -> bool {
    read_int(conn, KEY_SHUFFLE).unwrap_or(0) != 0
}

fn read_repeat(conn: &Connection) -> RepeatMode {
    RepeatMode::parse(read_string(conn, KEY_REPEAT).as_deref().unwrap_or("off"))
}

/// The order persisted by a previous session, fully validated against the
/// queue. Only read on a cache miss (startup / after the queue changed size).
fn read_saved_shuffle(conn: &Connection, length: i64) -> Option<ShuffleOrder> {
    let order: Vec<i64> = serde_json::from_str(&read_string(conn, KEY_SHUFFLE_ORDER)?).ok()?;
    let pos = read_int(conn, KEY_SHUFFLE_POS).unwrap_or(0).max(0) as usize;
    Some(ShuffleOrder { order, pos }).filter(|o| o.is_consistent(length))
}

fn write_shuffle_order(conn: &Connection, order: &ShuffleOrder) -> Result<(), String> {
    let json = serde_json::to_string(&order.order).map_err(|e| e.to_string())?;
    write_string(conn, KEY_SHUFFLE_ORDER, &json)?;
    write_string(conn, KEY_SHUFFLE_POS, &order.pos.to_string())
}

/// A working copy of the shuffle order for a queue of `length` tracks whose
/// current index is `current`. Served from memory; on a miss (or if the queue
/// length no longer matches — an O(1) check) it is loaded from the DB, or
/// regenerated and persisted. Callers mutate the copy and hand it to
/// `commit_shuffle` once the change has actually taken effect.
fn load_shuffle(app: &AppHandle, conn: &Connection, length: i64, current: i64) -> ShuffleOrder {
    let state = app.state::<ShuffleState>();
    let mut cached = state.0.lock().unwrap_or_else(|e| e.into_inner());
    if cached.as_ref().map(|o| o.order.len() as i64) != Some(length) {
        let order = read_saved_shuffle(conn, length).unwrap_or_else(|| {
            let fresh = ShuffleOrder::new(length, current, &mut fastrand::Rng::new());
            let _ = write_shuffle_order(conn, &fresh);
            fresh
        });
        *cached = Some(order);
    }
    let mut order = cached.clone().unwrap_or_default();
    // The current track was changed outside the order (e.g. before shuffle
    // was on): re-sync the cursor without losing the rest of the pass.
    if order.current() != Some(current) {
        order.jump_to(current);
    }
    order
}

/// Make `order` the live shuffle order: always persists the cursor (tiny), and
/// the full order only if the permutation differs from the cached one.
fn commit_shuffle(app: &AppHandle, conn: &Connection, order: &ShuffleOrder) -> Result<(), String> {
    let state = app.state::<ShuffleState>();
    let mut cached = state.0.lock().unwrap_or_else(|e| e.into_inner());
    if cached.as_ref().map(|c| &c.order) != Some(&order.order) {
        write_shuffle_order(conn, order)?;
    } else {
        write_string(conn, KEY_SHUFFLE_POS, &order.pos.to_string())?;
    }
    *cached = Some(order.clone());
    Ok(())
}

fn clear_shuffle(app: &AppHandle, conn: &Connection) -> Result<(), String> {
    *app.state::<ShuffleState>().0.lock().unwrap_or_else(|e| e.into_inner()) = None;
    delete_key(conn, KEY_SHUFFLE_ORDER)?;
    delete_key(conn, KEY_SHUFFLE_POS)
}

/// The queue index that should play after `current`, honouring shuffle and
/// repeat. Changes no state: in shuffle mode the advanced order is returned so
/// the caller can `commit_shuffle` it only once the track actually loads.
fn upcoming_index(
    app: &AppHandle,
    conn: &Connection,
    current: i64,
    length: i64,
) -> (Option<i64>, Option<ShuffleOrder>) {
    let repeat = read_repeat(conn);
    if shuffle_enabled(conn) {
        let mut order = load_shuffle(app, conn, length, current);
        let next = order.advance(&repeat, &mut fastrand::Rng::new());
        (next, Some(order))
    } else {
        (next_index(current, length, &repeat), None)
    }
}

pub fn emit_state(app: &AppHandle) {
    if let Ok(state) = build_full_state(app) {
        let _ = app.emit("player-state", state);
    }
}

fn fetch_current_track(conn: &Connection, track_id: i64) -> Option<CurrentTrack> {
    conn.query_row(
        "SELECT t.id, t.title, t.album_name, t.album_art_path, t.file_path, t.rating,
                (SELECT GROUP_CONCAT(name, '||') FROM (
                 SELECT a.name FROM track_artists ta JOIN artists a ON a.id = ta.artist_id
                 WHERE ta.track_id = t.id ORDER BY a.name COLLATE NOCASE)),
                (SELECT GROUP_CONCAT(name, '||') FROM (
                 SELECT c.name FROM track_composers tc JOIN composers c ON c.id = tc.composer_id
                 WHERE tc.track_id = t.id ORDER BY c.name COLLATE NOCASE))
         FROM tracks t WHERE t.id = ?1",
        [track_id],
        |row| {
            let artists_csv: Option<String> = row.get(6)?;
            let composers_csv: Option<String> = row.get(7)?;
            Ok(CurrentTrack {
                track_id: row.get(0)?,
                title: row.get(1)?,
                album_name: row.get(2)?,
                album_art_path: row.get(3)?,
                file_path: row.get(4)?,
                rating: row.get(5)?,
                artists: artists_csv
                    .map(|s| s.split("||").map(String::from).collect())
                    .unwrap_or_default(),
                composers: composers_csv
                    .map(|s| s.split("||").map(String::from).collect())
                    .unwrap_or_default(),
            })
        },
    )
    .ok()
}

fn build_full_state(app: &AppHandle) -> Result<FullPlayerState, String> {
    let player_handle = app.state::<PlayerHandle>();
    let db = app.state::<Mutex<Connection>>();
    let mut player_snapshot = player_handle.snapshot();
    let conn = db.lock().map_err(|e| e.to_string())?;
    let current_index = read_int(&conn, KEY_CURRENT_INDEX);
    let queue_length = queue_length(&conn)?;
    let shuffle = shuffle_enabled(&conn);
    let repeat = RepeatMode::parse(
        read_string(&conn, KEY_REPEAT).as_deref().unwrap_or("off"),
    );
    let crossfade_ms = read_int(&conn, KEY_CROSSFADE_MS).unwrap_or(0).max(0);
    let current_track = if let Some(track_id) = player_snapshot.track_id {
        fetch_current_track(&conn, track_id)
    } else if let Some(index) = current_index {
        // Player is stopped (e.g. fresh start) but we know the last queue position —
        // show the track so the user can see what will play when they press play.
        track_at_index(&conn, index)
            .ok()
            .flatten()
            .and_then(|(tid, _)| fetch_current_track(&conn, tid))
    } else {
        None
    };
    // Decoder doesn't always report total duration. Fall back to the DB value from lofty.
    if player_snapshot.duration_ms.is_none() {
        if let Some(track_id) = player_snapshot.track_id {
            let db_duration: Result<Option<i64>, _> = conn.query_row(
                "SELECT duration_ms FROM tracks WHERE id = ?1",
                [track_id],
                |row| row.get(0),
            );
            if let Ok(Some(d)) = db_duration {
                if d > 0 {
                    player_snapshot.duration_ms = Some(d as u64);
                }
            }
        }
    }
    let sleep_timer = {
        let st = app.state::<crate::commands::sleep_timer::SleepTimer>();
        let guard = st.0.lock().map_err(|e| e.to_string())?;
        crate::commands::sleep_timer::get_info(&guard)
    };
    Ok(FullPlayerState {
        player: player_snapshot,
        current_index,
        queue_length,
        shuffle,
        repeat,
        crossfade_ms,
        current_track,
        sleep_timer,
    })
}

fn load_index(
    player: &PlayerHandle,
    conn: &Connection,
    index: i64,
) -> Result<bool, String> {
    let Some((track_id, file_path)) = track_at_index(conn, index)? else {
        return Ok(false);
    };
    player.load_and_play(track_id, PathBuf::from(file_path))?;
    write_string(conn, KEY_CURRENT_INDEX, &index.to_string())?;
    Ok(true)
}

/// Persist the live playback position + status so playback can resume across
/// restarts. Called periodically by the ticker. No-op when nothing is loaded,
/// so a finished/stopped session doesn't clobber the last real position.
pub fn persist_playback_progress(app: &AppHandle) {
    let player = app.state::<PlayerHandle>();
    let snap = player.snapshot();
    if snap.track_id.is_none() || snap.status == PlaybackStatus::Stopped {
        return;
    }
    let db = app.state::<Mutex<Connection>>();
    let conn = match db.lock() {
        Ok(c) => c,
        Err(_) => return,
    };
    let _ = write_string(&conn, KEY_POSITION_MS, &snap.position_ms.to_string());
    let status = match snap.status {
        PlaybackStatus::Playing => "playing",
        PlaybackStatus::Paused => "paused",
        PlaybackStatus::Stopped => "stopped",
    };
    let _ = write_string(&conn, KEY_STATUS, status);
}

/// At startup, if the resume-on-launch preference is on and a previous session
/// left a queue position, reload that track at its saved position. Restores the
/// previous play/pause state (defaults to paused to avoid surprise audio).
pub fn resume_on_launch(app: &AppHandle) {
    let db = app.state::<Mutex<Connection>>();
    let player = app.state::<PlayerHandle>();

    let (track, position_ms, was_playing) = {
        let Ok(conn) = db.lock() else { return };
        if read_string(&conn, KEY_RESUME_ON_LAUNCH).as_deref() != Some("true") {
            return;
        }
        let Some(index) = read_int(&conn, KEY_CURRENT_INDEX) else { return };
        let track = match track_at_index(&conn, index) {
            Ok(Some(t)) => t,
            _ => return,
        };
        let position_ms = read_int(&conn, KEY_POSITION_MS).unwrap_or(0).max(0) as u64;
        let was_playing = read_string(&conn, KEY_STATUS).as_deref() == Some("playing");
        (track, position_ms, was_playing)
    };

    let (track_id, file_path) = track;
    if player.load_and_play(track_id, PathBuf::from(file_path)).is_err() {
        return;
    }
    if !was_playing {
        player.pause();
    }
    if position_ms > 0 {
        let _ = player.seek(position_ms);
    }
    emit_state(app);
}

#[tauri::command]
pub fn get_player_state(app: AppHandle) -> Result<FullPlayerState, String> {
    build_full_state(&app)
}

#[tauri::command]
pub fn play_queue_index(app: AppHandle, index: i64) -> Result<(), String> {
    let player_handle = app.state::<PlayerHandle>();
    let db = app.state::<Mutex<Connection>>();
    {
        let conn = db.lock().map_err(|e| e.to_string())?;
        // Explicit pick in shuffle mode: make it the next entry in the order
        // so the rest of the shuffle pass is preserved.
        let order = if shuffle_enabled(&conn) {
            let length = queue_length(&conn)?;
            let current = read_int(&conn, KEY_CURRENT_INDEX).unwrap_or(index);
            let mut order = load_shuffle(&app, &conn, length, current);
            order.jump_to(index);
            Some(order)
        } else {
            None
        };
        if !load_index(&player_handle, &conn, index)? {
            return Err(format!("no track at index {index}"));
        }
        if let Some(order) = order {
            commit_shuffle(&app, &conn, &order)?;
        }
    }
    emit_state(&app);
    Ok(())
}

#[tauri::command]
pub fn toggle_play_pause(app: AppHandle) -> Result<(), String> {
    let player_handle = app.state::<PlayerHandle>();
    let status = player_handle.snapshot().status;
    match status {
        PlaybackStatus::Playing => player_handle.pause(),
        PlaybackStatus::Paused => player_handle.resume(),
        PlaybackStatus::Stopped => {
            let db = app.state::<Mutex<Connection>>();
            let conn = db.lock().map_err(|e| e.to_string())?;
            let index = read_int(&conn, KEY_CURRENT_INDEX).unwrap_or(0);
            load_index(&player_handle, &conn, index)?;
        }
    }
    emit_state(&app);
    Ok(())
}

#[tauri::command]
pub fn stop_playback(app: AppHandle) -> Result<(), String> {
    let player_handle = app.state::<PlayerHandle>();
    player_handle.stop();
    emit_state(&app);
    Ok(())
}

#[tauri::command]
pub fn seek_to(app: AppHandle, position_ms: u64) -> Result<(), String> {
    let player_handle = app.state::<PlayerHandle>();
    player_handle.seek(position_ms)?;
    emit_state(&app);
    Ok(())
}

/// Return the waveform peaks for a track, computing and caching them on first
/// request. The blob lives on the `tracks` row; decoding happens outside the DB
/// lock so the connection isn't held during the (potentially slow) decode.
#[tauri::command]
pub fn get_waveform(db: State<Mutex<Connection>>, track_id: i64) -> Result<Vec<u8>, String> {
    // Library tracks only — transient/negative IDs have no file row.
    if track_id < 0 {
        return Ok(Vec::new());
    }

    let (cached, file_path): (Option<Vec<u8>>, Option<String>) = {
        let conn = db.lock().map_err(|e| e.to_string())?;
        conn.query_row(
            "SELECT waveform_peaks, file_path FROM tracks WHERE id = ?1",
            [track_id],
            |row| Ok((row.get::<_, Option<Vec<u8>>>(0)?, row.get::<_, Option<String>>(1)?)),
        )
        .map_err(|e| e.to_string())?
    };

    if let Some(peaks) = cached {
        if !peaks.is_empty() {
            return Ok(peaks);
        }
    }

    let Some(path) = file_path else {
        return Ok(Vec::new());
    };

    let peaks = crate::scanner::waveform::extract_peaks(
        std::path::Path::new(&path),
        crate::scanner::waveform::DEFAULT_BUCKETS,
    )?;

    {
        let conn = db.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "UPDATE tracks SET waveform_peaks = ?2 WHERE id = ?1",
            params![track_id, peaks],
        )
        .map_err(|e| e.to_string())?;
    }

    Ok(peaks)
}

#[tauri::command]
pub fn set_volume(app: AppHandle, volume: f32) -> Result<(), String> {
    let player_handle = app.state::<PlayerHandle>();
    player_handle.set_volume(volume);
    {
        let db = app.state::<Mutex<Connection>>();
        let conn = db.lock().map_err(|e| e.to_string())?;
        write_string(&conn, KEY_VOLUME, &volume.to_string())?;
    }
    emit_state(&app);
    Ok(())
}

#[tauri::command]
pub fn next_track(app: AppHandle) -> Result<(), String> {
    let player_handle = app.state::<PlayerHandle>();
    let snap = player_handle.snapshot();
    let db = app.state::<Mutex<Connection>>();
    let scrobble_id: Option<i64>;
    {
        let conn = db.lock().map_err(|e| e.to_string())?;
        scrobble_id = if let Some(tid) = snap.track_id {
            // Local play/skip stats use the lenient 30s-or-50% rule…
            if is_played(snap.position_ms, snap.duration_ms) {
                stats::record_play(&conn, tid);
            } else {
                stats::record_skip(&conn, tid);
            }
            // …but scrobbling follows Last.fm's stricter 50%-or-4min rule.
            if should_scrobble(snap.position_ms, snap.duration_ms) {
                Some(tid)
            } else {
                None
            }
        } else {
            None
        };
        let length = queue_length(&conn)?;
        if length == 0 {
            player_handle.stop();
        } else {
            let current = read_int(&conn, KEY_CURRENT_INDEX).unwrap_or(0);
            match upcoming_index(&app, &conn, current, length) {
                (Some(idx), order) => {
                    load_index(&player_handle, &conn, idx)?;
                    if let Some(order) = order {
                        commit_shuffle(&app, &conn, &order)?;
                    }
                }
                (None, _) => player_handle.stop(),
            }
        }
    }
    if let Some(tid) = scrobble_id {
        crate::commands::lastfm::on_track_scrobble(&app, tid);
    }
    emit_state(&app);
    Ok(())
}

#[tauri::command]
pub fn previous_track(app: AppHandle) -> Result<(), String> {
    let player_handle = app.state::<PlayerHandle>();
    let snap = player_handle.snapshot();
    let db = app.state::<Mutex<Connection>>();
    if snap.position_ms > 3000 {
        let _ = player_handle.seek(0);
    } else {
        let scrobble_id: Option<i64>;
        {
            let conn = db.lock().map_err(|e| e.to_string())?;
            scrobble_id = if let Some(tid) = snap.track_id {
                if is_played(snap.position_ms, snap.duration_ms) {
                    stats::record_play(&conn, tid);
                } else {
                    stats::record_skip(&conn, tid);
                }
                if should_scrobble(snap.position_ms, snap.duration_ms) {
                    Some(tid)
                } else {
                    None
                }
            } else {
                None
            };
            let length = queue_length(&conn)?;
            if length > 0 {
                let current = read_int(&conn, KEY_CURRENT_INDEX).unwrap_or(0);
                if shuffle_enabled(&conn) {
                    // Step back through the shuffle order — retraces exactly
                    // what Next played.
                    let mut order = load_shuffle(&app, &conn, length, current);
                    match order.prev() {
                        Some(prev) => {
                            load_index(&player_handle, &conn, prev)?;
                            commit_shuffle(&app, &conn, &order)?;
                        }
                        None => {
                            // Start of the shuffle pass — restart the current track.
                            let _ = player_handle.seek(0);
                            emit_state(&app);
                            return Ok(());
                        }
                    }
                } else {
                    let prev = if current == 0 { length - 1 } else { current - 1 };
                    load_index(&player_handle, &conn, prev)?;
                }
            }
        }
        if let Some(tid) = scrobble_id {
            crate::commands::lastfm::on_track_scrobble(&app, tid);
        }
    }
    emit_state(&app);
    Ok(())
}

#[tauri::command]
pub fn set_shuffle(app: AppHandle, enabled: bool) -> Result<(), String> {
    {
        let db = app.state::<Mutex<Connection>>();
        let conn = db.lock().map_err(|e| e.to_string())?;
        write_string(&conn, KEY_SHUFFLE, if enabled { "1" } else { "0" })?;
        // Fresh order starting from the current track, or drop it.
        if enabled {
            let length = queue_length(&conn)?;
            let current = read_int(&conn, KEY_CURRENT_INDEX).unwrap_or(0);
            let order = ShuffleOrder::new(length, current, &mut fastrand::Rng::new());
            commit_shuffle(&app, &conn, &order)?;
        } else {
            clear_shuffle(&app, &conn)?;
        }
    }
    emit_state(&app);
    Ok(())
}

#[tauri::command]
pub fn get_crossfade_ms(db: State<Mutex<Connection>>) -> Result<i64, String> {
    let conn = db.lock().map_err(|e| e.to_string())?;
    Ok(read_int(&conn, KEY_CROSSFADE_MS).unwrap_or(0).max(0))
}

#[tauri::command]
pub fn set_crossfade_ms(app: AppHandle, ms: i64) -> Result<(), String> {
    let clamped = ms.clamp(0, 12_000);
    {
        let db = app.state::<Mutex<Connection>>();
        let conn = db.lock().map_err(|e| e.to_string())?;
        write_string(&conn, KEY_CROSSFADE_MS, &clamped.to_string())?;
    }
    emit_state(&app);
    Ok(())
}

#[tauri::command]
pub fn set_repeat(app: AppHandle, mode: RepeatMode) -> Result<(), String> {
    {
        let db = app.state::<Mutex<Connection>>();
        let conn = db.lock().map_err(|e| e.to_string())?;
        write_string(&conn, KEY_REPEAT, mode.as_str())?;
    }
    emit_state(&app);
    Ok(())
}

#[tauri::command]
pub fn get_queue(db: State<Mutex<Connection>>) -> Result<Vec<QueueTrack>, String> {
    let conn = db.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT q.id, q.position, t.id, t.title, t.album_name, t.duration_ms,
                    t.album_art_path, t.file_path,
                    (SELECT GROUP_CONCAT(name, '||') FROM (
                     SELECT a.name FROM track_artists ta JOIN artists a ON a.id = ta.artist_id
                     WHERE ta.track_id = t.id ORDER BY a.name COLLATE NOCASE))
             FROM queue q
             JOIN tracks t ON t.id = q.track_id
             ORDER BY q.position ASC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            let artists_csv: Option<String> = row.get(8)?;
            Ok(QueueTrack {
                queue_id: row.get(0)?,
                position: row.get(1)?,
                track_id: row.get(2)?,
                title: row.get(3)?,
                album_name: row.get(4)?,
                duration_ms: row.get(5)?,
                album_art_path: row.get(6)?,
                file_path: row.get(7)?,
                artists: artists_csv
                    .map(|s| s.split("||").map(String::from).collect())
                    .unwrap_or_default(),
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

/// Replace the whole queue in one transaction. Row-by-row autocommit took ~7 s
/// for a 50k-track queue (one WAL commit per row) while holding the DB lock;
/// a single transaction with a prepared insert takes well under 100 ms.
fn replace_queue(conn: &Connection, track_ids: &[i64]) -> Result<(), String> {
    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM queue", []).map_err(|e| e.to_string())?;
    {
        let mut insert = tx
            .prepare("INSERT INTO queue (track_id, position) VALUES (?1, ?2)")
            .map_err(|e| e.to_string())?;
        for (idx, tid) in track_ids.iter().enumerate() {
            insert
                .execute(params![tid, idx as i64])
                .map_err(|e| e.to_string())?;
        }
    }
    tx.commit().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_queue_and_play(
    app: AppHandle,
    track_ids: Vec<i64>,
    start_index: i64,
) -> Result<(), String> {
    if track_ids.is_empty() {
        return Err("track_ids must not be empty".into());
    }
    let player_handle = app.state::<PlayerHandle>();
    let db = app.state::<Mutex<Connection>>();
    {
        let conn = db.lock().map_err(|e| e.to_string())?;
        replace_queue(&conn, &track_ids)?;
        let idx = start_index.clamp(0, track_ids.len() as i64 - 1);
        load_index(&player_handle, &conn, idx)?;
        // New queue → new shuffle pass starting from the chosen track.
        if shuffle_enabled(&conn) {
            let order = ShuffleOrder::new(track_ids.len() as i64, idx, &mut fastrand::Rng::new());
            commit_shuffle(&app, &conn, &order)?;
        } else {
            clear_shuffle(&app, &conn)?;
        }
    }
    emit_state(&app);
    Ok(())
}

#[tauri::command]
pub fn add_to_queue(app: AppHandle, track_id: i64) -> Result<(), String> {
    {
        let db = app.state::<Mutex<Connection>>();
        let conn = db.lock().map_err(|e| e.to_string())?;
        let next_pos: i64 = conn
            .query_row(
                "SELECT COALESCE(MAX(position), -1) + 1 FROM queue",
                [],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        let length_before = queue_length(&conn)?;
        conn.execute(
            "INSERT INTO queue (track_id, position) VALUES (?1, ?2)",
            params![track_id, next_pos],
        )
        .map_err(|e| e.to_string())?;
        // Slot the new track in among the unplayed part of the shuffle order.
        if shuffle_enabled(&conn) && length_before > 0 {
            let current = read_int(&conn, KEY_CURRENT_INDEX).unwrap_or(0);
            let mut order = load_shuffle(&app, &conn, length_before, current);
            order.on_insert(length_before, &mut fastrand::Rng::new());
            commit_shuffle(&app, &conn, &order)?;
        }
    }
    emit_state(&app);
    Ok(())
}

#[tauri::command]
pub fn remove_from_queue(app: AppHandle, queue_id: i64) -> Result<(), String> {
    let player_handle = app.state::<PlayerHandle>();
    let db = app.state::<Mutex<Connection>>();
    {
        let conn = db.lock().map_err(|e| e.to_string())?;
        let removed_position: Option<i64> = conn
            .query_row(
                "SELECT position FROM queue WHERE id = ?1",
                [queue_id],
                |row| row.get(0),
            )
            .ok();
        // Capture the shuffle order against the queue as it was before removal.
        let mut order = match (shuffle_enabled(&conn), removed_position) {
            (true, Some(_)) => {
                let length_before = queue_length(&conn)?;
                let current = read_int(&conn, KEY_CURRENT_INDEX).unwrap_or(0);
                Some(load_shuffle(&app, &conn, length_before, current))
            }
            _ => None,
        };
        // Delete + renumber atomically so positions stay contiguous (the
        // indexed lookup in track_at_index relies on it).
        {
            let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
            tx.execute("DELETE FROM queue WHERE id = ?1", [queue_id])
                .map_err(|e| e.to_string())?;
            if let Some(removed) = removed_position {
                tx.execute(
                    "UPDATE queue SET position = position - 1 WHERE position > ?1",
                    [removed],
                )
                .map_err(|e| e.to_string())?;
            }
            tx.commit().map_err(|e| e.to_string())?;
        }
        if let Some(removed) = removed_position {
            if let Some(order) = order.as_mut() {
                order.on_remove(removed);
                commit_shuffle(&app, &conn, order)?;
            }
            let current = read_int(&conn, KEY_CURRENT_INDEX);
            if let Some(curr) = current {
                if removed < curr {
                    write_string(&conn, KEY_CURRENT_INDEX, &(curr - 1).to_string())?;
                } else if removed == curr {
                    player_handle.stop();
                    let length = queue_length(&conn)?;
                    if length > 0 {
                        // Shuffle: continue with the next track in the order.
                        let new_idx = order
                            .as_ref()
                            .and_then(|o| o.current())
                            .unwrap_or(curr.min(length - 1));
                        load_index(&player_handle, &conn, new_idx)?;
                    } else {
                        delete_key(&conn, KEY_CURRENT_INDEX)?;
                    }
                }
            }
        }
    }
    emit_state(&app);
    Ok(())
}

#[tauri::command]
pub fn clear_queue(app: AppHandle) -> Result<(), String> {
    let player_handle = app.state::<PlayerHandle>();
    player_handle.stop();
    {
        let db = app.state::<Mutex<Connection>>();
        let conn = db.lock().map_err(|e| e.to_string())?;
        conn.execute("DELETE FROM queue", [])
            .map_err(|e| e.to_string())?;
        delete_key(&conn, KEY_CURRENT_INDEX)?;
        clear_shuffle(&app, &conn)?;
    }
    emit_state(&app);
    Ok(())
}

/// Drain the audio event channel and react to each event.
/// Called once per ticker iteration; replaces the old `advance_if_finished` +
/// `try_start_crossfade` poll pair.
/// Returns true if any state change occurred (so the caller can emit player-state).
pub fn handle_audio_events(app: &AppHandle) -> bool {
    let player_handle = app.state::<PlayerHandle>();
    let events = player_handle.drain_events();
    if events.is_empty() {
        return false;
    }
    let mut changed = false;
    for event in events {
        match event {
            AudioEvent::TrackFinished { track_id } => {
                changed |= on_track_finished(app, track_id);
            }
            AudioEvent::NearEnd { track_id, remaining_ms } => {
                changed |= on_near_end(app, track_id, remaining_ms);
            }
        }
    }
    changed
}

/// React to a `TrackFinished` event: record the play, honour the sleep timer,
/// then advance the queue. The `track_id` is the track that just finished
/// (captured by the Worker before clearing its internal state).
fn on_track_finished(app: &AppHandle, finished_track_id: i64) -> bool {
    // Stats and scrobble only apply to real library tracks (non-negative IDs).
    if finished_track_id >= 0 {
        let db = app.state::<Mutex<Connection>>();
        if let Ok(conn) = db.lock() {
            stats::record_play(&conn, finished_track_id);
        }
        crate::commands::lastfm::on_track_scrobble(app, finished_track_id);
    }

    // Sleep timer end-of-song mode: stop here instead of loading the next track.
    if crate::commands::sleep_timer::handle_track_end(app) {
        emit_state(app);
        return true;
    }

    if finished_track_id < 0 {
        emit_state(app);
        return true;
    }

    let db = app.state::<Mutex<Connection>>();
    let Ok(conn) = db.lock() else { return true };
    let length = match queue_length(&conn) {
        Ok(n) => n,
        Err(_) => return true,
    };
    if length == 0 {
        return true;
    }
    let current = read_int(&conn, KEY_CURRENT_INDEX).unwrap_or(0);
    if let (Some(idx), order) = upcoming_index(app, &conn, current, length) {
        let player_handle = app.state::<PlayerHandle>();
        if load_index(&player_handle, &conn, idx).unwrap_or(false) {
            if let Some(order) = order {
                let _ = commit_shuffle(app, &conn, &order);
            }
        }
    }
    drop(conn);
    emit_state(app);
    true
}

/// React to a `NearEnd` event: if crossfade is configured and the remaining
/// time falls within the crossfade window, start crossfading the next track.
fn on_near_end(app: &AppHandle, _track_id: i64, remaining_ms: u64) -> bool {
    let player_handle = app.state::<PlayerHandle>();

    // Only crossfade when actively playing.
    if player_handle.snapshot().status != PlaybackStatus::Playing {
        return false;
    }

    let db = app.state::<Mutex<Connection>>();
    let Ok(conn) = db.lock() else { return false };
    let crossfade_ms = read_int(&conn, KEY_CROSSFADE_MS).unwrap_or(0).max(0) as u64;

    if !should_crossfade(remaining_ms, crossfade_ms) {
        return false;
    }

    let length = match queue_length(&conn) {
        Ok(n) => n,
        Err(_) => return false,
    };
    if length == 0 {
        return false;
    }
    let current = read_int(&conn, KEY_CURRENT_INDEX).unwrap_or(0);
    // Peek: the advanced shuffle order is only saved once the crossfade starts.
    let (Some(next_idx), order) = upcoming_index(app, &conn, current, length) else {
        return false;
    };
    if next_idx == current && !matches!(read_repeat(&conn), RepeatMode::One) {
        return false;
    }
    let Ok(Some((next_track_id, file_path))) = track_at_index(&conn, next_idx) else {
        return false;
    };

    if player_handle
        .load_and_crossfade(next_track_id, PathBuf::from(file_path), crossfade_ms)
        .is_ok()
    {
        let _ = write_string(&conn, KEY_CURRENT_INDEX, &next_idx.to_string());
        if let Some(order) = order {
            let _ = commit_shuffle(app, &conn, &order);
        }
        drop(conn);
        emit_state(app);
        true
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    /// Real on-disk DB (WAL, real migrations) with `n` tracks, in a fresh temp dir.
    fn test_db(name: &str, n: i64) -> Connection {
        let dir = std::env::temp_dir().join(format!("halo-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let conn = crate::db::open(dir).unwrap();
        let tx = conn.unchecked_transaction().unwrap();
        {
            let mut ins = tx
                .prepare("INSERT INTO tracks (id, file_path) VALUES (?1, ?2)")
                .unwrap();
            for i in 0..n {
                ins.execute(params![i, format!("/music/{i}.flac")]).unwrap();
            }
        }
        tx.commit().unwrap();
        conn
    }

    #[test]
    fn replace_queue_and_lookup_by_index() {
        let conn = test_db("lookup", 100);
        let ids: Vec<i64> = (0..100).rev().collect(); // queue order ≠ id order
        replace_queue(&conn, &ids).unwrap();
        assert_eq!(queue_length(&conn).unwrap(), 100);
        for idx in [0, 1, 50, 99] {
            let (tid, path) = track_at_index(&conn, idx).unwrap().unwrap();
            assert_eq!(tid, ids[idx as usize]);
            assert_eq!(path, format!("/music/{tid}.flac"));
        }
        assert_eq!(track_at_index(&conn, 100).unwrap(), None);
        // Replacing again fully swaps the contents.
        replace_queue(&conn, &[7, 8]).unwrap();
        assert_eq!(queue_length(&conn).unwrap(), 2);
        assert_eq!(track_at_index(&conn, 1).unwrap().unwrap().0, 8);
    }

    #[test]
    fn lookup_falls_back_when_positions_have_gaps() {
        let conn = test_db("gaps", 10);
        replace_queue(&conn, &[0, 1, 2, 3]).unwrap();
        // Simulate a legacy queue with a gap: positions 0, 1, 5, 6.
        conn.execute("UPDATE queue SET position = position + 3 WHERE position >= 2", [])
            .unwrap();
        assert_eq!(track_at_index(&conn, 2).unwrap().unwrap().0, 2);
        assert_eq!(track_at_index(&conn, 3).unwrap().unwrap().0, 3);
        assert_eq!(track_at_index(&conn, 4).unwrap(), None);
    }

    #[test]
    fn queue_position_index_exists() {
        let conn = test_db("index", 0);
        let n: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = 'idx_queue_position'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 1);
    }

    /// Timing check for large queues. Ignored by default (timing varies by
    /// machine); run with:
    /// `cargo test -p halo --lib large_queue_timing -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn large_queue_timing() {
        const N: i64 = 50_000;
        let conn = test_db("timing", N);
        let ids: Vec<i64> = (0..N).collect();

        let t = Instant::now();
        replace_queue(&conn, &ids).unwrap();
        println!("replace_queue ({N} tracks):        {:>8.2} ms", t.elapsed().as_secs_f64() * 1e3);

        let t = Instant::now();
        for i in 0..1000 {
            track_at_index(&conn, (i * 47) % N).unwrap().unwrap();
        }
        println!("track_at_index (avg of 1000):      {:>8.3} ms", t.elapsed().as_secs_f64());

        let mut order = ShuffleOrder::new(N, 0, &mut fastrand::Rng::new());
        let t = Instant::now();
        write_shuffle_order(&conn, &order).unwrap();
        println!("write full shuffle order (rare):   {:>8.2} ms", t.elapsed().as_secs_f64() * 1e3);

        let t = Instant::now();
        let loaded = read_saved_shuffle(&conn, N).unwrap();
        println!("read + validate order (startup):   {:>8.2} ms", t.elapsed().as_secs_f64() * 1e3);
        assert_eq!(loaded, order);

        let t = Instant::now();
        for _ in 0..1000 {
            let mut copy = order.clone();
            copy.advance(&RepeatMode::Off, &mut fastrand::Rng::new());
            write_string(&conn, KEY_SHUFFLE_POS, &copy.pos.to_string()).unwrap();
            order = copy;
        }
        println!("Next in shuffle (avg of 1000):     {:>8.3} ms", t.elapsed().as_secs_f64());
    }
}
