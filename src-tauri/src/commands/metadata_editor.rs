use lofty::config::WriteOptions;
use lofty::file::{AudioFile, TaggedFileExt};
use lofty::picture::{MimeType, Picture, PictureType};
use lofty::probe::Probe;
use lofty::tag::{Accessor, ItemKey, ItemValue, TagItem};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::io::Read;
use std::path::Path;
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, State};

// ── Shared types ──────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct FullTrackMetadata {
    pub id: i64,
    pub file_path: String,
    pub album_art_path: Option<String>,
    // tag fields read from file
    pub title: Option<String>,
    pub album_name: Option<String>,
    pub artists: Vec<String>,
    pub album_artists: Vec<String>,
    pub composers: Vec<String>,
    pub genres: Vec<String>,
    pub year: Option<i64>,
    pub track_number: Option<i64>,
    pub track_total: Option<i64>,
    pub disc_number: Option<i64>,
    pub disc_total: Option<i64>,
    pub comment: Option<String>,
    pub publisher: Option<String>,
    pub copyright: Option<String>,
    pub language: Option<String>,
}

#[derive(Deserialize)]
pub struct MetadataEdit {
    pub title: Option<String>,
    pub album_name: Option<String>,
    pub artists: Vec<String>,
    pub album_artists: Vec<String>,
    pub composers: Vec<String>,
    pub genres: Vec<String>,
    pub year: Option<i64>,
    pub track_number: Option<i64>,
    pub track_total: Option<i64>,
    pub disc_number: Option<i64>,
    pub disc_total: Option<i64>,
    pub comment: Option<String>,
    pub publisher: Option<String>,
    pub copyright: Option<String>,
    pub language: Option<String>,
    pub new_art_path: Option<String>,
}

#[derive(Serialize)]
pub struct CoverSuggestion {
    pub thumbnail_url: String,
    pub full_url: String,
    pub title: String,
    pub date: Option<String>,
}

// ── Commands ──────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn get_track_full_metadata(
    track_id: i64,
    db: State<'_, Mutex<Connection>>,
) -> Result<FullTrackMetadata, String> {
    let (file_path, album_art_path) = {
        let conn = db.lock().map_err(|e| e.to_string())?;
        conn.query_row(
            "SELECT file_path, album_art_path FROM tracks WHERE id = ?1",
            [track_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
        )
        .map_err(|e| e.to_string())?
    };

    let path = Path::new(&file_path);
    let tagged_file = Probe::open(path)
        .map_err(|e| format!("Cannot open file: {e}"))?
        .read()
        .map_err(|e| format!("Cannot read tags: {e}"))?;

    let tag = tagged_file.primary_tag().or_else(|| tagged_file.first_tag());

    let mut title = None;
    let mut album_name = None;
    let mut artists: Vec<String> = Vec::new();
    let mut album_artists: Vec<String> = Vec::new();
    let mut composers: Vec<String> = Vec::new();
    let mut genres: Vec<String> = Vec::new();
    let mut year = None;
    let mut track_number = None;
    let mut track_total = None;
    let mut disc_number = None;
    let mut disc_total = None;
    let mut comment = None;
    let mut publisher = None;
    let mut copyright = None;
    let mut language = None;

    if let Some(tag) = tag {
        title = tag.title().map(|s| s.to_string());
        album_name = tag.album().map(|s| s.to_string());
        year = tag.year().map(|y| y as i64);
        track_number = tag.track().map(|n| n as i64);
        track_total = tag.track_total().map(|n| n as i64);
        disc_number = tag.disk().map(|n| n as i64);
        disc_total = tag.disk_total().map(|n| n as i64);

        artists = tag.get_strings(&ItemKey::TrackArtist).map(|s| s.to_string()).collect();
        if artists.is_empty() {
            if let Some(a) = tag.artist() {
                artists.push(a.to_string());
            }
        }
        album_artists = tag.get_strings(&ItemKey::AlbumArtist).map(|s| s.to_string()).collect();
        composers = tag.get_strings(&ItemKey::Composer).map(|s| s.to_string()).collect();
        genres = tag.get_strings(&ItemKey::Genre).map(|s| s.to_string()).collect();
        if genres.is_empty() {
            if let Some(g) = tag.genre() {
                genres.push(g.to_string());
            }
        }

        comment = tag.get_string(&ItemKey::Comment).map(|s| s.to_string());
        publisher = tag.get_string(&ItemKey::Label).map(|s| s.to_string());
        copyright = tag.get_string(&ItemKey::CopyrightMessage).map(|s| s.to_string());
        language = tag.get_string(&ItemKey::Language).map(|s| s.to_string());
    }

    Ok(FullTrackMetadata {
        id: track_id,
        file_path,
        album_art_path,
        title,
        album_name,
        artists,
        album_artists,
        composers,
        genres,
        year,
        track_number,
        track_total,
        disc_number,
        disc_total,
        comment,
        publisher,
        copyright,
        language,
    })
}

#[tauri::command]
pub fn save_track_metadata(
    track_id: i64,
    edit: MetadataEdit,
    app: AppHandle,
    db: State<'_, Mutex<Connection>>,
) -> Result<(), String> {
    let file_path: String = {
        let conn = db.lock().map_err(|e| e.to_string())?;
        conn.query_row(
            "SELECT file_path FROM tracks WHERE id = ?1",
            [track_id],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?
    };

    let path = Path::new(&file_path);
    let mut tagged_file = Probe::open(path)
        .map_err(|e| format!("Cannot open file: {e}"))?
        .read()
        .map_err(|e| format!("Cannot read tags: {e}"))?;

    // Read art bytes once so we can both cache and embed without a second fs::read.
    let art_material: Option<(Vec<u8>, MimeType)> = if let Some(art_src) = &edit.new_art_path {
        let bytes = std::fs::read(art_src).map_err(|e| format!("Cannot read art file: {e}"))?;
        let mime = mime_from_path(art_src);
        Some((bytes, mime))
    } else {
        None
    };

    let new_art_cache_path: Option<String> = if let Some((bytes, mime)) = &art_material {
        let picture =
            Picture::new_unchecked(PictureType::CoverFront, Some(mime.clone()), None, bytes.clone());
        let cache_dir = app
            .path()
            .app_cache_dir()
            .map_err(|e| e.to_string())?
            .join("album-art");
        std::fs::create_dir_all(&cache_dir).map_err(|e| e.to_string())?;
        let cached = crate::scanner::art::cache_picture(&picture, &cache_dir)?;
        Some(cached)
    } else {
        None
    };

    {
        let has_primary = tagged_file.primary_tag().is_some();
        let tag = if has_primary {
            tagged_file.primary_tag_mut().unwrap()
        } else {
            tagged_file
                .first_tag_mut()
                .ok_or_else(|| "No tag found in file".to_string())?
        };

        // Scalar text fields via Accessor
        match edit.title.as_deref() {
            Some(t) if !t.is_empty() => tag.set_title(t.to_string()),
            _ => tag.remove_title(),
        }
        match edit.album_name.as_deref() {
            Some(a) if !a.is_empty() => tag.set_album(a.to_string()),
            _ => tag.remove_album(),
        }
        match edit.year {
            Some(y) => tag.set_year(y as u32),
            None => tag.remove_year(),
        }
        match edit.track_number {
            Some(n) => tag.set_track(n as u32),
            None => tag.remove_track(),
        }
        match edit.track_total {
            Some(n) => tag.set_track_total(n as u32),
            None => tag.remove_track_total(),
        }
        match edit.disc_number {
            Some(n) => tag.set_disk(n as u32),
            None => tag.remove_disk(),
        }
        match edit.disc_total {
            Some(n) => tag.set_disk_total(n as u32),
            None => tag.remove_disk_total(),
        }

        // Multi-value text fields
        tag.remove_key(&ItemKey::TrackArtist);
        for name in &edit.artists {
            let _ = tag.push(TagItem::new(ItemKey::TrackArtist, ItemValue::Text(name.clone())));
        }
        tag.remove_key(&ItemKey::AlbumArtist);
        for name in &edit.album_artists {
            let _ = tag.push(TagItem::new(ItemKey::AlbumArtist, ItemValue::Text(name.clone())));
        }
        tag.remove_key(&ItemKey::Composer);
        for name in &edit.composers {
            let _ = tag.push(TagItem::new(ItemKey::Composer, ItemValue::Text(name.clone())));
        }
        tag.remove_key(&ItemKey::Genre);
        for name in &edit.genres {
            let _ = tag.push(TagItem::new(ItemKey::Genre, ItemValue::Text(name.clone())));
        }

        // Extra single-value text fields
        set_text_tag(tag, &ItemKey::Comment, edit.comment.as_deref());
        set_text_tag(tag, &ItemKey::Label, edit.publisher.as_deref());
        set_text_tag(tag, &ItemKey::CopyrightMessage, edit.copyright.as_deref());
        set_text_tag(tag, &ItemKey::Language, edit.language.as_deref());

        // Embed album art using the bytes already read above
        if let Some((bytes, mime)) = art_material {
            let picture = Picture::new_unchecked(PictureType::CoverFront, Some(mime), None, bytes);
            tag.remove_picture_type(PictureType::CoverFront);
            tag.push_picture(picture);
        }
    }

    tagged_file
        .save_to_path(path, WriteOptions::default())
        .map_err(|e| format!("Failed to save tags: {e}"))?;

    // Update DB
    let conn = db.lock().map_err(|e| e.to_string())?;

    if let Some(art_path) = &new_art_cache_path {
        conn.execute(
            "UPDATE tracks SET title=?2, album_name=?3, year=?4, track_number=?5,
             disc_number=?6, album_art_path=?7 WHERE id=?1",
            params![
                track_id,
                edit.title,
                edit.album_name,
                edit.year,
                edit.track_number,
                edit.disc_number,
                art_path
            ],
        )
        .map_err(|e| e.to_string())?;
    } else {
        conn.execute(
            "UPDATE tracks SET title=?2, album_name=?3, year=?4, track_number=?5,
             disc_number=?6 WHERE id=?1",
            params![
                track_id,
                edit.title,
                edit.album_name,
                edit.year,
                edit.track_number,
                edit.disc_number
            ],
        )
        .map_err(|e| e.to_string())?;
    }

    conn.execute("DELETE FROM track_artists WHERE track_id=?1", [track_id])
        .map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM track_album_artists WHERE track_id=?1", [track_id])
        .map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM track_composers WHERE track_id=?1", [track_id])
        .map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM track_genres WHERE track_id=?1", [track_id])
        .map_err(|e| e.to_string())?;

    for name in &edit.artists {
        let id = upsert_name(&conn, "artists", name)?;
        conn.execute(
            "INSERT OR IGNORE INTO track_artists (track_id, artist_id) VALUES (?1, ?2)",
            params![track_id, id],
        )
        .map_err(|e| e.to_string())?;
    }
    for name in &edit.album_artists {
        let id = upsert_name(&conn, "album_artists", name)?;
        conn.execute(
            "INSERT OR IGNORE INTO track_album_artists (track_id, album_artist_id) VALUES (?1, ?2)",
            params![track_id, id],
        )
        .map_err(|e| e.to_string())?;
    }
    for name in &edit.composers {
        let id = upsert_name(&conn, "composers", name)?;
        conn.execute(
            "INSERT OR IGNORE INTO track_composers (track_id, composer_id) VALUES (?1, ?2)",
            params![track_id, id],
        )
        .map_err(|e| e.to_string())?;
    }
    for name in &edit.genres {
        let id = upsert_name(&conn, "genres", name)?;
        conn.execute(
            "INSERT OR IGNORE INTO track_genres (track_id, genre_id) VALUES (?1, ?2)",
            params![track_id, id],
        )
        .map_err(|e| e.to_string())?;
    }

    drop(conn);
    app.emit("library-changed", ()).ok();
    Ok(())
}

#[tauri::command]
pub fn extract_track_art(
    track_id: i64,
    app: AppHandle,
    db: State<'_, Mutex<Connection>>,
) -> Result<String, String> {
    let file_path: String = {
        let conn = db.lock().map_err(|e| e.to_string())?;
        conn.query_row(
            "SELECT file_path FROM tracks WHERE id = ?1",
            [track_id],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?
    };

    let path = Path::new(&file_path);
    let tagged_file = Probe::open(path)
        .map_err(|e| format!("Cannot open file: {e}"))?
        .read()
        .map_err(|e| format!("Cannot read tags: {e}"))?;

    let tag = tagged_file
        .primary_tag()
        .or_else(|| tagged_file.first_tag())
        .ok_or_else(|| "No tag found in file".to_string())?;

    let picture = tag
        .pictures()
        .iter()
        .find(|p| p.pic_type() == PictureType::CoverFront)
        .or_else(|| tag.pictures().first())
        .ok_or_else(|| "No embedded art found".to_string())?;

    let cache_dir = app
        .path()
        .app_cache_dir()
        .map_err(|e| e.to_string())?
        .join("album-art");
    std::fs::create_dir_all(&cache_dir).map_err(|e| e.to_string())?;

    crate::scanner::art::cache_picture(picture, &cache_dir)
}

#[tauri::command]
pub fn fetch_art_from_url(url: String, app: AppHandle) -> Result<String, String> {
    let response = ureq::get(&url)
        .set("User-Agent", "HaloMusicPlayer/0.1")
        .timeout(Duration::from_secs(10))
        .call()
        .map_err(|e| format!("Failed to fetch URL: {e}"))?;

    let content_type = response
        .header("content-type")
        .unwrap_or("image/jpeg")
        .to_string();
    let mime = if content_type.contains("png") {
        MimeType::Png
    } else if content_type.contains("gif") {
        MimeType::Gif
    } else if content_type.contains("bmp") {
        MimeType::Bmp
    } else {
        MimeType::Jpeg
    };

    let mut bytes = Vec::new();
    response
        .into_reader()
        .take(20 * 1024 * 1024) // 20 MB safety cap
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;

    if bytes.is_empty() {
        return Err("Empty response from URL".to_string());
    }

    let picture = Picture::new_unchecked(PictureType::CoverFront, Some(mime), None, bytes);

    let cache_dir = app
        .path()
        .app_cache_dir()
        .map_err(|e| e.to_string())?
        .join("album-art");
    std::fs::create_dir_all(&cache_dir).map_err(|e| e.to_string())?;

    crate::scanner::art::cache_picture(&picture, &cache_dir)
}

#[tauri::command]
pub fn search_cover_art(artist: String, album: String) -> Result<Vec<CoverSuggestion>, String> {
    let query = format!("artist:{artist} release:{album}");

    let body: serde_json::Value = ureq::get("https://musicbrainz.org/ws/2/release")
        .query("query", &query)
        .query("limit", "8")
        .query("fmt", "json")
        .set("User-Agent", "HaloMusicPlayer/0.1 (https://github.com/halo)")
        .timeout(Duration::from_secs(10))
        .call()
        .map_err(|e| format!("MusicBrainz error: {e}"))?
        .into_json()
        .map_err(|e| e.to_string())?;

    let releases = body["releases"]
        .as_array()
        .ok_or_else(|| "No releases in response".to_string())?;

    let suggestions: Vec<CoverSuggestion> = releases
        .iter()
        .filter_map(|r| {
            let mbid = r["id"].as_str()?;
            let title = r["title"].as_str().unwrap_or("Unknown").to_string();
            let date = r["date"]
                .as_str()
                .map(|d| d.chars().take(4).collect::<String>());
            Some(CoverSuggestion {
                thumbnail_url: format!(
                    "https://coverartarchive.org/release/{mbid}/front-250"
                ),
                full_url: format!("https://coverartarchive.org/release/{mbid}/front"),
                title,
                date,
            })
        })
        .take(8)
        .collect();

    Ok(suggestions)
}

// ── Image processing ─────────────────────────────────────────────────────────

/// Crop, resize to fit within 750×750 (aspect ratio preserved), and compress to ≤ 350 KB JPEG.
/// Pass crop_w = 0 / crop_h = 0 to skip cropping (only resize + compress).
#[tauri::command]
pub fn process_art(
    source_path: String,
    crop_x: u32,
    crop_y: u32,
    crop_w: u32,
    crop_h: u32,
    app: AppHandle,
) -> Result<String, String> {
    use image::codecs::jpeg::JpegEncoder;
    use image::{ExtendedColorType, ImageEncoder};

    let img =
        image::open(&source_path).map_err(|e| format!("Cannot open image: {e}"))?;

    let cropped = if crop_w > 0 && crop_h > 0 {
        let mut tmp = img;
        tmp.crop(crop_x, crop_y, crop_w, crop_h)
    } else {
        img
    };

    let resized = if cropped.width() > 750 || cropped.height() > 750 {
        cropped.resize(750, 750, image::imageops::FilterType::Lanczos3)
    } else {
        cropped
    };

    let rgb = resized.to_rgb8();
    let max_bytes: usize = 350 * 1024;
    let mut final_bytes = Vec::new();

    for &quality in &[90u8, 80, 70, 60, 50, 40] {
        let mut buf = Vec::new();
        JpegEncoder::new_with_quality(&mut buf, quality)
            .write_image(
                rgb.as_raw(),
                rgb.width(),
                rgb.height(),
                ExtendedColorType::Rgb8,
            )
            .map_err(|e| e.to_string())?;
        final_bytes = buf;
        if final_bytes.len() <= max_bytes {
            break;
        }
    }

    let picture = Picture::new_unchecked(
        PictureType::CoverFront,
        Some(MimeType::Jpeg),
        None,
        final_bytes,
    );

    let cache_dir = app
        .path()
        .app_cache_dir()
        .map_err(|e| e.to_string())?
        .join("album-art");
    std::fs::create_dir_all(&cache_dir).map_err(|e| e.to_string())?;

    crate::scanner::art::cache_picture(&picture, &cache_dir)
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn set_text_tag(tag: &mut lofty::tag::Tag, key: &ItemKey, value: Option<&str>) {
    tag.remove_key(key);
    if let Some(v) = value {
        if !v.is_empty() {
            let _ = tag.push(TagItem::new(key.clone(), ItemValue::Text(v.to_string())));
        }
    }
}

fn upsert_name(conn: &Connection, table: &str, name: &str) -> Result<i64, String> {
    conn.execute(
        &format!("INSERT OR IGNORE INTO {table} (name) VALUES (?1)"),
        [name],
    )
    .map_err(|e| e.to_string())?;
    conn.query_row(
        &format!("SELECT id FROM {table} WHERE name = ?1"),
        [name],
        |row| row.get::<_, i64>(0),
    )
    .map_err(|e| e.to_string())
}

fn mime_from_path(path: &str) -> MimeType {
    let lower = path.to_lowercase();
    if lower.ends_with(".png") {
        MimeType::Png
    } else if lower.ends_with(".gif") {
        MimeType::Gif
    } else if lower.ends_with(".bmp") {
        MimeType::Bmp
    } else {
        MimeType::Jpeg
    }
}
