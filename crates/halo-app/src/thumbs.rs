//! Cover thumbnails: downscaled JPEG copies of album art in
//! `<cache>/com.saravix.halo/thumbs`, so lists and cards never decode a
//! full-size cover (up to 1440 px, ~8 MB of pixels each) to draw it at
//! 40–200 px. Only the full-screen player and the metadata editor's crop
//! view use the original.
//!
//! [`pick`] is called from `view`: a ready thumbnail is returned at once
//! (an existing file costs two `stat`s, once per session); a missing one is
//! queued for the `halo-thumbs` worker and `None` is returned, so the caller
//! draws the placeholder. The worker wakes the UI through [`events`] when
//! a batch is done. A thumbnail is named after the source's path, size and
//! mtime, so an edited cover gets a new one.

use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Mutex, OnceLock};
use std::time::{Duration, Instant};

/// Rows, the now-playing bar, queue, palette, mini player (≤ 64 px).
pub const SMALL: u32 = 128;
/// Cards, tiles, album / folder covers (≤ 192 px).
pub const MEDIUM: u32 = 384;

/// The thumbnail size for art drawn `display` logical px wide (2× for
/// HiDPI); `None`: big enough to need the original.
pub fn tier(display: f32) -> Option<u32> {
    if display <= 64.0 {
        Some(SMALL)
    } else if display <= 192.0 {
        Some(MEDIUM)
    } else {
        None
    }
}

#[derive(Clone)]
enum State {
    Pending,
    /// The file to draw: the thumbnail, or the original when it is already
    /// small (or could not be read).
    Ready(String),
}

struct Thumbs {
    dir: PathBuf,
    map: Mutex<HashMap<(String, u32), State>>,
    jobs: Mutex<mpsc::Sender<(String, u32)>>,
}

static THUMBS: OnceLock<Thumbs> = OnceLock::new();
static READY: tokio::sync::Notify = tokio::sync::Notify::const_new();

/// Start the worker. Without it [`pick`] returns the originals.
pub fn init(cache_dir: &Path) {
    let dir = cache_dir.join("thumbs");
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let (tx, rx) = mpsc::channel();
    let t = Thumbs { dir, map: Mutex::new(HashMap::new()), jobs: Mutex::new(tx) };
    if THUMBS.set(t).is_ok() {
        let _ = std::thread::Builder::new().name("halo-thumbs".into()).spawn(move || worker(rx));
    }
}

/// The file to draw for cover `path` shown `display` px wide, or `None`
/// while its thumbnail is being made (draw the placeholder).
pub fn pick(path: &str, display: f32) -> Option<String> {
    let (Some(size), Some(t)) = (tier(display), THUMBS.get()) else {
        return Some(path.to_string());
    };
    let key = (path.to_string(), size);
    let mut map = t.map.lock().ok()?;
    match map.get(&key) {
        Some(State::Ready(p)) => return Some(p.clone()),
        Some(State::Pending) => return None,
        None => {}
    }
    if let Some(file) = thumb_file(&t.dir, path, size).filter(|f| f.is_file()) {
        let p = file.to_string_lossy().into_owned();
        map.insert(key, State::Ready(p.clone()));
        return Some(p);
    }
    map.insert(key.clone(), State::Pending);
    let _ = t.jobs.lock().map(|tx| tx.send(key));
    None
}

/// Blocking [`pick`] for worker threads (accent colour, blurred
/// background): makes the thumbnail now if needed.
pub fn ensure(path: &str, size: u32) -> String {
    let Some(t) = THUMBS.get() else { return path.to_string() };
    let key = (path.to_string(), size);
    if let Some(State::Ready(p)) = t.map.lock().ok().and_then(|m| m.get(&key).cloned()) {
        return p;
    }
    let p = make(&t.dir, path, size);
    if let Ok(mut m) = t.map.lock() {
        m.insert(key, State::Ready(p.clone()));
    }
    p
}

/// Forget resolved thumbnails (after a rescan / cover edit).
pub fn invalidate() {
    if let Some(mut m) = THUMBS.get().and_then(|t| t.map.lock().ok()) {
        m.retain(|_, s| matches!(s, State::Pending));
    }
}

/// One `()` per finished batch of thumbnails: redraw to show them.
pub fn events() -> impl iced::futures::Stream<Item = ()> {
    iced::stream::channel(1, async move |mut out| {
        use iced::futures::SinkExt;
        loop {
            READY.notified().await;
            if out.send(()).await.is_err() {
                break;
            }
        }
    })
}

fn worker(rx: mpsc::Receiver<(String, u32)>) {
    let mut next = rx.recv().ok();
    let mut last_wake = Instant::now();
    while let Some((path, size)) = next {
        let p = make(&THUMBS.get().expect("init").dir, &path, size);
        if let Some(mut m) = THUMBS.get().and_then(|t| t.map.lock().ok()) {
            m.insert((path, size), State::Ready(p));
        }
        // Wake the UI when the queue runs dry, or every 250 ms while a
        // long batch (first visit to Albums) is running.
        next = match rx.try_recv() {
            Ok(job) => {
                if last_wake.elapsed() >= Duration::from_millis(250) {
                    READY.notify_one();
                    last_wake = Instant::now();
                }
                Some(job)
            }
            Err(mpsc::TryRecvError::Empty) => {
                READY.notify_one();
                halo_backend::mem::trim(); // the decoded originals
                last_wake = Instant::now();
                rx.recv().ok()
            }
            Err(mpsc::TryRecvError::Disconnected) => None,
        };
    }
}

/// `<dir>/<hash of path, length, mtime>-<size>.jpg`; `None` if the source
/// is gone.
fn thumb_file(dir: &Path, path: &str, size: u32) -> Option<PathBuf> {
    let meta = std::fs::metadata(path).ok()?;
    let mut h = DefaultHasher::new();
    path.hash(&mut h);
    meta.len().hash(&mut h);
    meta.modified().ok().hash(&mut h);
    Some(dir.join(format!("{:016x}-{size}.jpg", h.finish())))
}

/// Make (or find) the thumbnail; the original path when the cover is
/// already that small or can't be read.
fn make(dir: &Path, path: &str, size: u32) -> String {
    let original = || path.to_string();
    let Some(file) = thumb_file(dir, path, size) else { return original() };
    if file.is_file() {
        return file.to_string_lossy().into_owned();
    }
    match image::image_dimensions(path) {
        Ok((w, h)) if w > size || h > size => {}
        _ => return original(),
    }
    let Ok(img) = image::open(path) else { return original() };
    let rgb = img.thumbnail(size, size).into_rgb8();
    drop(img);
    // Unique temp name: `ensure` and the worker may race on one cover.
    let tmp = file.with_extension(format!("{:?}.tmp", std::thread::current().id()).replace(['(', ')'], ""));
    let ok = std::fs::File::create(&tmp).ok().is_some_and(|f| {
        let mut w = std::io::BufWriter::new(f);
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut w, 88).encode_image(&rgb).is_ok()
    });
    if ok && std::fs::rename(&tmp, &file).is_ok() {
        file.to_string_lossy().into_owned()
    } else {
        let _ = std::fs::remove_file(&tmp);
        original()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("halo-thumbs-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn tiers() {
        assert_eq!(tier(40.0), Some(SMALL));
        assert_eq!(tier(64.0), Some(SMALL));
        assert_eq!(tier(160.0), Some(MEDIUM));
        assert_eq!(tier(400.0), None);
    }

    #[test]
    fn big_covers_shrink_small_ones_stay() {
        let d = temp_dir("make");
        let big = d.join("big.png");
        image::RgbImage::from_pixel(1000, 800, image::Rgb([200, 30, 40])).save(&big).unwrap();
        let small = d.join("small.png");
        image::RgbImage::from_pixel(100, 100, image::Rgb([1, 2, 3])).save(&small).unwrap();

        let t = make(&d, big.to_str().unwrap(), SMALL);
        assert!(t.ends_with("-128.jpg"), "{t}");
        assert_eq!(image::image_dimensions(&t).unwrap(), (128, 102)); // aspect kept
        // Found again, not remade.
        assert_eq!(make(&d, big.to_str().unwrap(), SMALL), t);
        assert_eq!(make(&d, small.to_str().unwrap(), SMALL), small.to_str().unwrap());
        assert_eq!(make(&d, "/nope.jpg", SMALL), "/nope.jpg");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn edited_cover_gets_a_new_thumbnail() {
        let d = temp_dir("edit");
        let p = d.join("c.png");
        image::RgbImage::from_pixel(500, 500, image::Rgb([0, 0, 0])).save(&p).unwrap();
        let a = thumb_file(&d, p.to_str().unwrap(), SMALL).unwrap();
        image::RgbImage::from_pixel(600, 600, image::Rgb([255, 255, 255])).save(&p).unwrap();
        let b = thumb_file(&d, p.to_str().unwrap(), SMALL).unwrap();
        assert_ne!(a, b);
        let _ = std::fs::remove_dir_all(&d);
    }
}
