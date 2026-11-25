//! Recently Played (`recently-played.tsx`): the track table with a
//! "Last Played" relative-time column.

use halo_backend::commands::library;
use iced::{Element, Task};

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
        Some(crate::ui::h1("Recently Played").into()),
        &app.pages.recent,
        Variant::Recent,
        super::common::empty_state(
            crate::icons::Icon::History,
            "Nothing played yet",
            "Your listening history shows up here.",
            Some(("Go to Library", Message::Navigate(crate::route::Route::Library(None)))),
        ),
        true,
    )
}
