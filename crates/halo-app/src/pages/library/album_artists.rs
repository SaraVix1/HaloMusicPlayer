//! Album Artists tab / page: avatars with album + track counts.

use halo_backend::commands::library::{AlbumArtist, TracksQuery};
use iced::Element;

use super::cards::{self, PEOPLE};
use super::{coll_grid, standalone};
use crate::app::{Halo, Message};
use crate::format::plural;
use crate::route::Route;

/// The grid for this collection (used by the Library tab and `/album-artists`).
pub fn grid(app: &Halo, key: String, padding_top: Option<f32>) -> Element<'_, Message> {
    coll_grid(
        &app.pages.library.album_artists,
        "No album artists yet — scan a folder to get started.",
        PEOPLE,
        key,
        &app.pages.scroll,
        padding_top,
        |x: &AlbumArtist, w: f32| cards::person_card(
            &x.name,
            x.album_art_path.as_deref(),
            format!(
                "{} · {}",
                plural(x.album_count, "album", "albums"),
                plural(x.track_count, "track", "tracks")
            ),
            Route::Songs(TracksQuery {
                album_artist: Some(x.name.clone()),
                ..TracksQuery::default()
            }),
            w.min(160.0),
            crate::components::context_menu::Target::Artist { name: x.name.clone(), album_artist: true },
        ),
    )
}

/// Standalone page with a header and count.
pub fn view(app: &Halo) -> Element<'_, Message> {
    let n = app.pages.library.album_artists.items.len();
    standalone(app, "Album Artists", n, "album artist", "album artists", grid(app, app.route().scroll_key(), Some(0.0)))
}
