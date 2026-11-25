//! Most Played (`most-played.tsx`): the track table with a "Plays" column.

use halo_backend::commands::library;
use iced::{Element, Task};

use super::common::note;
use super::tracks::{self, Variant};
use super::Msg;
use crate::app::{bg, Halo, Message};

pub fn load(app: &mut Halo) -> Task<Message> {
    app.pages.most.loading = !app.pages.most.loaded;
    bg(
        &app.backend,
        |b| library::get_most_played(b.db()),
        |r| Message::Page(Msg::MostLoaded(r)),
    )
}

pub fn view(app: &Halo) -> Element<'_, Message> {
    tracks::page(
        app,
        Some("Most Played"),
        &app.pages.most,
        Variant::MostPlayed,
        None,
        note("No play history yet."),
    )
}
