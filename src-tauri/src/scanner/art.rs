use lofty::picture::{MimeType, Picture};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

pub fn cache_picture(picture: &Picture, cache_dir: &Path) -> Result<String, String> {
    let data = picture.data();
    let mut hasher = Sha256::new();
    hasher.update(data);
    let hash = hex::encode(hasher.finalize());

    let extension = match picture.mime_type() {
        Some(MimeType::Png) => "png",
        Some(MimeType::Jpeg) => "jpg",
        Some(MimeType::Gif) => "gif",
        Some(MimeType::Bmp) => "bmp",
        Some(MimeType::Tiff) => "tiff",
        _ => "img",
    };

    let filename = format!("{}.{}", hash, extension);
    let path: PathBuf = cache_dir.join(&filename);

    if !path.exists() {
        std::fs::write(&path, data).map_err(|e| e.to_string())?;
    }

    Ok(path.to_string_lossy().to_string())
}
