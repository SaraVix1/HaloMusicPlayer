//! Artists tab / page: round avatars linking to `/songs?artist=`.

use halo_backend::commands::library::{Artist, TracksQuery};
use iced::Element;

use super::cards::{self, PEOPLE};
use super::{coll_grid, standalone};
use crate::app::{Halo, Message};
use crate::format::plural;
use crate::route::Route;

/// The grid for this collection (used by the Library tab and `/artists`).
pub fn grid(app: &Halo, key: String, padding_top: Option<f32>) -> Element<'_, Message> {
    coll_grid(
        &app.pages.library.artists,
        "No artists yet — scan a folder to get started.",
        PEOPLE,
        key,
        &app.pages.scroll,
        padding_top,
        |x: &Artist, w: f32| cards::person_card(
            &x.name,
            x.album_art_path.as_deref(),
            plural(x.track_count, "track", "tracks"),
            Route::Songs(TracksQuery {
                artist: Some(x.name.clone()),
                ..TracksQuery::default()
            }),
            w.min(160.0),
        ),
    )
}

/// Standalone page with a header and count.
pub fn view(app: &Halo) -> Element<'_, Message> {
    let n = app.pages.library.artists.items.len();
    standalone(app, "Artists", n, "artist", "artists", grid(app, app.route().scroll_key(), Some(0.0)))
}
