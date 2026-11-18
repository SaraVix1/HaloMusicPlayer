//! Genres tab / page: gradient tiles linking to `/songs?genre=`.

use halo_backend::commands::library::{Genre, TracksQuery};
use iced::Element;

use super::cards::{self, TILES};
use super::{coll_grid, standalone};
use crate::app::{Halo, Message};
use crate::route::Route;

/// The grid for this collection (used by the Library tab and `/genres`).
pub fn grid(app: &Halo, key: String, padding_top: Option<f32>) -> Element<'_, Message> {
    coll_grid(
        &app.pages.library.genres,
        "No genres yet — scan a folder to get started.",
        TILES,
        key,
        &app.pages.scroll,
        padding_top,
        |x: &Genre, w: f32| {
            let _ = w;
            cards::gradient_card(
                &x.name,
                x.track_count,
                Route::Songs(TracksQuery {
                    genre: Some(x.name.clone()),
                    ..TracksQuery::default()
                }),
            )
        },
    )
}

/// Standalone page with a header and count.
pub fn view(app: &Halo) -> Element<'_, Message> {
    let n = app.pages.library.genres.items.len();
    standalone(app, "Genres", n, "genre", "genres", grid(app, app.route().scroll_key(), Some(0.0)))
}
