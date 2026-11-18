use lofty::file::{AudioFile, TaggedFileExt};
use lofty::picture::Picture;
use lofty::probe::Probe;
use lofty::tag::{Accessor, ItemKey};
use std::path::Path;

#[derive(Default)]
pub struct TrackMetadata {
    pub title: Option<String>,
    pub album_name: Option<String>,
    pub track_number: Option<i64>,
    pub disc_number: Option<i64>,
    pub duration_ms: Option<i64>,
    pub year: Option<i64>,
    pub bitrate: Option<i64>,
    pub sample_rate: Option<i64>,
    pub file_format: Option<String>,
    pub artists: Vec<String>,
    pub album_artists: Vec<String>,
    pub composers: Vec<String>,
    pub genres: Vec<String>,
    pub album_art: Option<Picture>,
    pub lyrics: Option<String>,
}

pub fn extract(path: &Path) -> Result<TrackMetadata, String> {
    let tagged_file = Probe::open(path)
        .map_err(|e| e.to_string())?
        .read()
        .map_err(|e| e.to_string())?;

    let properties = tagged_file.properties();
    let file_type = tagged_file.file_type();

    let mut meta = TrackMetadata {
        duration_ms: Some(properties.duration().as_millis() as i64),
        bitrate: properties.audio_bitrate().map(|b| b as i64),
        sample_rate: properties.sample_rate().map(|r| r as i64),
        file_format: Some(format!("{:?}", file_type)),
        ..Default::default()
    };

    let tag = tagged_file
        .primary_tag()
        .or_else(|| tagged_file.first_tag());

    if let Some(tag) = tag {
        meta.title = tag.title().map(|s| s.to_string());
        meta.album_name = tag.album().map(|s| s.to_string());
        meta.track_number = tag.track().map(|n| n as i64);
        meta.disc_number = tag.disk().map(|n| n as i64);
        meta.year = tag.year().map(|y| y as i64);

        let collect_strings = |key: &ItemKey| -> Vec<String> {
            tag.get_strings(key).map(|s| s.to_string()).collect()
        };

        let mut artists = collect_strings(&ItemKey::TrackArtist);
        if artists.is_empty() {
            if let Some(a) = tag.artist() {
                artists.push(a.to_string());
            }
        }
        meta.artists = artists;

        meta.album_artists = collect_strings(&ItemKey::AlbumArtist);
        meta.composers = collect_strings(&ItemKey::Composer);

        let mut genres = collect_strings(&ItemKey::Genre);
        if genres.is_empty() {
            if let Some(g) = tag.genre() {
                genres.push(g.to_string());
            }
        }
        meta.genres = genres;

        if let Some(picture) = tag.pictures().first() {
            meta.album_art = Some(picture.clone());
        }

        meta.lyrics = tag.get_string(&ItemKey::Lyrics).map(|s| s.to_string());
    }

    Ok(meta)
}
