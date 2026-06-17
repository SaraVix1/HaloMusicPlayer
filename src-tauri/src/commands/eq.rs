use crate::audio::eq::{EqState, MAX_GAIN_DB, NUM_BANDS, STEREO_WIDTH_DEFAULT, STEREO_WIDTH_MAX};
use crate::audio::PlayerHandle;
use cpal::traits::{DeviceTrait, HostTrait};
use rusqlite::Connection;
use serde::Serialize;
use std::sync::Mutex;
use tauri::State;

const KEY_BYPASS: &str = "eq.bypass";
const KEY_BANDS: &str = "eq.bands";
const KEY_STEREO: &str = "eq.stereo";
const KEY_STEREO_WIDTH: &str = "eq.stereo_width";
const KEY_DYNAMIC: &str = "eq.dynamic";

#[derive(Serialize, Clone)]
pub struct EqConfig {
    pub bypass: bool,
    pub bands: [f32; NUM_BANDS],
    pub stereo: bool,
    pub stereo_width: f32,
    pub dynamic: bool,
}

// ---------------------------------------------------------------------------
// Persistence helpers
// ---------------------------------------------------------------------------

fn read_bypass(conn: &Connection) -> bool {
    conn.query_row(
        "SELECT value FROM app_state WHERE key = ?1",
        [KEY_BYPASS],
        |row| row.get::<_, String>(0),
    )
    .ok()
    .map(|s| s == "1")
    .unwrap_or(false)
}

fn write_bypass(conn: &Connection, bypass: bool) -> Result<(), String> {
    conn.execute(
        "INSERT INTO app_state (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [KEY_BYPASS, if bypass { "1" } else { "0" }],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

fn read_bands(conn: &Connection) -> [f32; NUM_BANDS] {
    conn.query_row(
        "SELECT value FROM app_state WHERE key = ?1",
        [KEY_BANDS],
        |row| row.get::<_, String>(0),
    )
    .ok()
    .and_then(|s| serde_json::from_str::<Vec<f32>>(&s).ok())
    .and_then(|v| {
        if v.len() == NUM_BANDS {
            Some(std::array::from_fn(|i| v[i].clamp(-MAX_GAIN_DB, MAX_GAIN_DB)))
        } else {
            None
        }
    })
    .unwrap_or([0.0f32; NUM_BANDS])
}

fn write_bands(conn: &Connection, bands: &[f32; NUM_BANDS]) -> Result<(), String> {
    let json = serde_json::to_string(bands.as_slice()).map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO app_state (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [KEY_BANDS, &json],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

fn read_bool(conn: &Connection, key: &str) -> bool {
    conn.query_row(
        "SELECT value FROM app_state WHERE key = ?1",
        [key],
        |row| row.get::<_, String>(0),
    )
    .ok()
    .map(|s| s == "1")
    .unwrap_or(false)
}

fn write_str(conn: &Connection, key: &str, value: &str) -> Result<(), String> {
    conn.execute(
        "INSERT INTO app_state (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [key, value],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

fn read_stereo_width(conn: &Connection) -> f32 {
    conn.query_row(
        "SELECT value FROM app_state WHERE key = ?1",
        [KEY_STEREO_WIDTH],
        |row| row.get::<_, String>(0),
    )
    .ok()
    .and_then(|s| s.parse::<f32>().ok())
    .map(|w| w.clamp(0.0, STEREO_WIDTH_MAX))
    .unwrap_or(STEREO_WIDTH_DEFAULT)
}

/// Reads the stereo/dynamic extras as (stereo_enabled, stereo_width, dynamic_enabled).
fn read_extras(conn: &Connection) -> (bool, f32, bool) {
    (
        read_bool(conn, KEY_STEREO),
        read_stereo_width(conn),
        read_bool(conn, KEY_DYNAMIC),
    )
}

// ---------------------------------------------------------------------------
// Startup restore
// ---------------------------------------------------------------------------

/// Called once at app startup to restore persisted EQ settings into the live EqState.
pub fn restore_eq_state(conn: &Connection, eq_state: &EqState) {
    eq_state.set_bypass(read_bypass(conn));
    let bands = read_bands(conn);
    for (i, &g) in bands.iter().enumerate() {
        eq_state.set_gain_db(i, g);
    }
    let (stereo, width, dynamic) = read_extras(conn);
    eq_state.set_stereo_enabled(stereo);
    eq_state.set_stereo_width(width);
    eq_state.set_dynamic_enabled(dynamic);
}

// ---------------------------------------------------------------------------
// IPC commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn get_eq(
    db: State<Mutex<Connection>>,
) -> Result<EqConfig, String> {
    let conn = db.lock().map_err(|e| e.to_string())?;
    let (stereo, stereo_width, dynamic) = read_extras(&conn);
    Ok(EqConfig {
        bypass: read_bypass(&conn),
        bands: read_bands(&conn),
        stereo,
        stereo_width,
        dynamic,
    })
}

#[tauri::command]
pub fn set_eq_stereo(
    db: State<Mutex<Connection>>,
    player: State<PlayerHandle>,
    enabled: bool,
    width: f32,
) -> Result<(), String> {
    let clamped = width.clamp(0.0, STEREO_WIDTH_MAX);
    player.eq_state.set_stereo_enabled(enabled);
    player.eq_state.set_stereo_width(clamped);
    let conn = db.lock().map_err(|e| e.to_string())?;
    write_str(&conn, KEY_STEREO, if enabled { "1" } else { "0" })?;
    write_str(&conn, KEY_STEREO_WIDTH, &clamped.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn set_eq_dynamic(
    db: State<Mutex<Connection>>,
    player: State<PlayerHandle>,
    enabled: bool,
) -> Result<(), String> {
    player.eq_state.set_dynamic_enabled(enabled);
    let conn = db.lock().map_err(|e| e.to_string())?;
    write_str(&conn, KEY_DYNAMIC, if enabled { "1" } else { "0" })?;
    Ok(())
}

#[tauri::command]
pub fn set_eq_band(
    db: State<Mutex<Connection>>,
    player: State<PlayerHandle>,
    band: usize,
    gain_db: f32,
) -> Result<(), String> {
    if band >= NUM_BANDS {
        return Err(format!("band {band} out of range (0..{NUM_BANDS})"));
    }
    let clamped = gain_db.clamp(-MAX_GAIN_DB, MAX_GAIN_DB);
    player.eq_state.set_gain_db(band, clamped);
    let conn = db.lock().map_err(|e| e.to_string())?;
    let mut bands = read_bands(&conn);
    bands[band] = clamped;
    write_bands(&conn, &bands)?;
    Ok(())
}

#[tauri::command]
pub fn set_eq_bypass(
    db: State<Mutex<Connection>>,
    player: State<PlayerHandle>,
    bypass: bool,
) -> Result<(), String> {
    player.eq_state.set_bypass(bypass);
    let conn = db.lock().map_err(|e| e.to_string())?;
    write_bypass(&conn, bypass)?;
    Ok(())
}

#[tauri::command]
pub fn set_eq_preset(
    db: State<Mutex<Connection>>,
    player: State<PlayerHandle>,
    preset: String,
) -> Result<EqConfig, String> {
    let bands = preset_gains(&preset)?;
    for (i, &g) in bands.iter().enumerate() {
        player.eq_state.set_gain_db(i, g);
    }
    let conn = db.lock().map_err(|e| e.to_string())?;
    write_bands(&conn, &bands)?;
    let bypass = read_bypass(&conn);
    let (stereo, stereo_width, dynamic) = read_extras(&conn);
    Ok(EqConfig { bypass, bands, stereo, stereo_width, dynamic })
}

// ---------------------------------------------------------------------------
// User preset IPC commands
// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub struct UserPreset {
    pub id: i64,
    pub name: String,
    pub bands: [f32; NUM_BANDS],
}

fn parse_bands_json(json: &str) -> Option<[f32; NUM_BANDS]> {
    serde_json::from_str::<Vec<f32>>(json).ok().and_then(|v| {
        if v.len() == NUM_BANDS {
            Some(std::array::from_fn(|i| v[i].clamp(-MAX_GAIN_DB, MAX_GAIN_DB)))
        } else {
            None
        }
    })
}

#[tauri::command]
pub fn list_user_presets(db: State<Mutex<Connection>>) -> Result<Vec<UserPreset>, String> {
    let conn = db.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT id, name, bands FROM eq_presets ORDER BY name COLLATE NOCASE")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(|e| e.to_string())?;
    let mut presets = Vec::new();
    for row in rows {
        let (id, name, bands_json) = row.map_err(|e| e.to_string())?;
        let bands = parse_bands_json(&bands_json).unwrap_or([0.0f32; NUM_BANDS]);
        presets.push(UserPreset { id, name, bands });
    }
    Ok(presets)
}

#[tauri::command]
pub fn save_user_preset(
    db: State<Mutex<Connection>>,
    player: State<PlayerHandle>,
    name: String,
) -> Result<UserPreset, String> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err("preset name must not be empty".into());
    }
    let bands = player.eq_state.get_all_gains();
    let json = serde_json::to_string(bands.as_slice()).map_err(|e| e.to_string())?;
    let conn = db.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO eq_presets (name, bands) VALUES (?1, ?2)
         ON CONFLICT(name) DO UPDATE SET bands = excluded.bands",
        rusqlite::params![name, json],
    )
    .map_err(|e| e.to_string())?;
    let id: i64 = conn
        .query_row(
            "SELECT id FROM eq_presets WHERE name = ?1",
            rusqlite::params![name],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    Ok(UserPreset { id, name, bands })
}

#[tauri::command]
pub fn load_user_preset(
    db: State<Mutex<Connection>>,
    player: State<PlayerHandle>,
    id: i64,
) -> Result<EqConfig, String> {
    let conn = db.lock().map_err(|e| e.to_string())?;
    let bands_json: String = conn
        .query_row(
            "SELECT bands FROM eq_presets WHERE id = ?1",
            rusqlite::params![id],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    let bands = parse_bands_json(&bands_json)
        .ok_or_else(|| "invalid bands data in preset".to_string())?;
    for (i, &g) in bands.iter().enumerate() {
        player.eq_state.set_gain_db(i, g);
    }
    write_bands(&conn, &bands)?;
    let bypass = read_bypass(&conn);
    let (stereo, stereo_width, dynamic) = read_extras(&conn);
    Ok(EqConfig { bypass, bands, stereo, stereo_width, dynamic })
}

#[tauri::command]
pub fn delete_user_preset(db: State<Mutex<Connection>>, id: i64) -> Result<(), String> {
    let conn = db.lock().map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM eq_presets WHERE id = ?1", rusqlite::params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Device EQ profiles
// ---------------------------------------------------------------------------

/// Returns the OS display name of the current default audio output device.
pub fn get_current_device_name() -> Option<String> {
    cpal::default_host()
        .default_output_device()
        .and_then(|d| d.name().ok())
}

#[derive(Serialize, Clone)]
pub struct DeviceEqProfile {
    pub device_name: String,
    pub bypass: bool,
    pub bands: [f32; NUM_BANDS],
}

fn read_device_profile(conn: &Connection, device_name: &str) -> Option<(bool, [f32; NUM_BANDS])> {
    conn.query_row(
        "SELECT bypass, bands FROM device_eq_profiles WHERE device_name = ?1",
        [device_name],
        |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
    )
    .ok()
    .and_then(|(bypass_i, bands_json)| {
        parse_bands_json(&bands_json).map(|bands| (bypass_i != 0, bands))
    })
}

fn write_device_profile(
    conn: &Connection,
    device_name: &str,
    bypass: bool,
    bands: &[f32; NUM_BANDS],
) -> Result<(), String> {
    let json = serde_json::to_string(bands.as_slice()).map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO device_eq_profiles (device_name, bypass, bands, updated_at)
         VALUES (?1, ?2, ?3, strftime('%s','now'))
         ON CONFLICT(device_name) DO UPDATE
         SET bypass = excluded.bypass, bands = excluded.bands, updated_at = excluded.updated_at",
        rusqlite::params![device_name, bypass as i64, json],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Called from the ticker when the default output device changes.
/// Loads the saved profile for `device_name` into `eq_state` and syncs
/// `app_state` so that `get_eq` returns up-to-date values.
/// Returns `Some(EqConfig)` if a profile was found, `None` otherwise.
pub fn load_device_eq_if_exists(
    conn: &Connection,
    device_name: &str,
    eq_state: &EqState,
) -> Option<EqConfig> {
    let (bypass, bands) = read_device_profile(conn, device_name)?;
    eq_state.set_bypass(bypass);
    for (i, &g) in bands.iter().enumerate() {
        eq_state.set_gain_db(i, g);
    }
    let _ = write_bypass(conn, bypass);
    let _ = write_bands(conn, &bands);
    let (stereo, stereo_width, dynamic) = read_extras(conn);
    Some(EqConfig { bypass, bands, stereo, stereo_width, dynamic })
}

#[tauri::command]
pub fn get_current_device(db: State<Mutex<Connection>>) -> Result<DeviceEqProfile, String> {
    let device_name = get_current_device_name().unwrap_or_else(|| "Unknown".to_string());
    let conn = db.lock().map_err(|e| e.to_string())?;
    let (bypass, bands) = read_device_profile(&conn, &device_name)
        .unwrap_or_else(|| (read_bypass(&conn), read_bands(&conn)));
    Ok(DeviceEqProfile { device_name, bypass, bands })
}

#[tauri::command]
pub fn save_device_eq_profile(
    db: State<Mutex<Connection>>,
    player: State<PlayerHandle>,
) -> Result<DeviceEqProfile, String> {
    let device_name = get_current_device_name()
        .ok_or_else(|| "no output device detected".to_string())?;
    let bypass = player.eq_state.is_bypass();
    let bands = player.eq_state.get_all_gains();
    let conn = db.lock().map_err(|e| e.to_string())?;
    write_device_profile(&conn, &device_name, bypass, &bands)?;
    Ok(DeviceEqProfile { device_name, bypass, bands })
}

#[tauri::command]
pub fn delete_device_eq_profile(
    db: State<Mutex<Connection>>,
    device_name: String,
) -> Result<(), String> {
    let conn = db.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "DELETE FROM device_eq_profiles WHERE device_name = ?1",
        rusqlite::params![device_name],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn list_device_eq_profiles(db: State<Mutex<Connection>>) -> Result<Vec<DeviceEqProfile>, String> {
    let conn = db.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT device_name, bypass, bands FROM device_eq_profiles \
             ORDER BY device_name COLLATE NOCASE",
        )
        .map_err(|e| e.to_string())?;
    let mut profiles = Vec::new();
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(|e| e.to_string())?;
    for row in rows {
        let (device_name, bypass_i, bands_json) = row.map_err(|e| e.to_string())?;
        let bands = parse_bands_json(&bands_json).unwrap_or([0.0f32; NUM_BANDS]);
        profiles.push(DeviceEqProfile { device_name, bypass: bypass_i != 0, bands });
    }
    Ok(profiles)
}

fn preset_gains(name: &str) -> Result<[f32; NUM_BANDS], String> {
    Ok(match name {
        "Flat" =>         [ 0.0,  0.0,  0.0,  0.0,  0.0,  0.0,  0.0,  0.0,  0.0,  0.0],
        "Rock" =>         [ 4.0,  3.0, -1.0, -1.0,  0.0,  1.0,  2.0,  3.0,  3.0,  3.0],
        "Pop" =>          [-1.5, -1.0,  0.0,  2.0,  4.0,  4.0,  2.0,  0.0, -1.0, -1.5],
        "Jazz" =>         [ 3.0,  2.0,  1.0,  2.0, -2.0, -2.0,  0.0,  1.0,  2.0,  3.0],
        "Classical" =>    [ 4.0,  3.0, -1.0, -1.0, -1.0,  0.0,  0.0,  1.0,  3.0,  4.0],
        "Electronic" =>   [ 4.0,  3.5,  1.0,  0.0, -1.0,  1.0,  0.0,  1.0,  3.0,  4.0],
        "Bass Boost" =>   [ 6.0,  5.0,  4.0,  2.0,  1.0,  0.0,  0.0,  0.0,  0.0,  0.0],
        "Treble Boost" => [ 0.0,  0.0,  0.0,  0.0,  0.0,  1.0,  2.0,  4.0,  5.0,  6.0],
        "Vocal" =>        [-2.0, -2.0, -1.0,  2.0,  4.0,  4.0,  3.0,  2.0, -1.0, -2.0],
        "Acoustic" =>     [ 4.0,  3.0,  2.0,  1.0,  2.0,  3.0,  4.0,  3.0,  2.0,  1.0],
        _ => return Err(format!("unknown preset: {name}")),
    })
}
