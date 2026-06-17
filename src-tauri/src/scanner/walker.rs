use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub fn walk(root: &Path, extensions: &[String]) -> Vec<PathBuf> {
    WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_file())
        .filter_map(|entry| {
            let ext = entry
                .path()
                .extension()
                .and_then(|e| e.to_str())
                .map(|s| s.to_lowercase())?;
            if extensions.iter().any(|e| e == &ext) {
                Some(entry.into_path())
            } else {
                None
            }
        })
        .collect()
}
