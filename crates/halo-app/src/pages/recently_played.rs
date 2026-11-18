//! Recently Played (`recently-played.tsx`): the track table with a
//! "Last Played" relative-time column.

use halo_backend::commands::library;
use iced::{Element, Task};

use super::common::note;
use super::tracks::{self, Variant};
use super::Msg;
use crate::app::{bg, Halo, Message};

pub fn load(app: &mut Halo) -> Task<Message> {
    app.pages.recent.loading = !app.pages.recent.loaded;
    bg(
        &app.backend,
        |b| library::get_recently_played(b.db()),
        |r| Message::Page(Msg::RecentLoaded(r)),
    )
}

pub fn view(app: &Halo) -> Element<'_, Message> {
    tracks::page(
        app,
        "Recently Played",
        &app.pages.recent,
        Variant::Recent,
        None,
        note("No recently played tracks."),
    )
}
