//! Song lists: the Library Songs tab, and filtered lists (an album, artist,
//! genre …) shown inside their Library tab with a breadcrumb.

use halo_backend::commands::library::{self, TracksQuery};
use iced::widget::{column, row};
use iced::{Alignment, Element, Task};

use super::tracks::{self, Variant};
use super::Msg;
use crate::app::{bg, Halo, Message};
use crate::theme;

pub fn query_for(q: &TracksQuery) -> TracksQuery {
    let mut q = q.clone();
    q.sort.get_or_insert_with(|| "title".into());
    q.direction.get_or_insert_with(|| "asc".into());
    q
}

pub fn load(app: &mut Halo, q: TracksQuery) -> Task<Message> {
    app.pages.songs.loading = !app.pages.songs.loaded;
    app.pages.songs.error = None;
    let q2 = query_for(&q);
    bg(
        &app.backend,
        move |b| library::get_tracks(q2, b.db()),
        move |r| Message::Page(Msg::SongsLoaded(q, r)),
    )
}

/// The Songs tab (no filter), or a filtered list inside its Library tab:
/// `Genres › Beat`, `Artists › …`; an album gets a header with its cover
/// and album-style rows.
pub fn view<'a>(app: &'a Halo, q: &'a TracksQuery) -> Element<'a, Message> {
    let list = &app.pages.songs;
    if crate::route::is_all_songs(q) {
        return tracks::page(app, None, list, Variant::Songs, super::common::empty_or_note(app, "No songs."), false);
    }
    let tab = crate::route::filter_tab(q);
    let label = crate::route::filter_label(q).unwrap_or_default();
    if q.album.is_some() {
        let head = album_header(app, tab, label);
        return tracks::page(app, Some(head), list, Variant::Album, super::common::note("No songs on this album."), false);
    }
    let head = super::library::crumbs(app, tab, label);
    tracks::page(app, Some(head), list, Variant::Songs, super::common::note("No songs match."), false)
}

/// Album page header: cover, `Albums › Roja`, "1992 · 12 songs · 54 m".
fn album_header<'a>(app: &'a Halo, tab: crate::route::LibraryTab, label: String) -> Element<'a, Message> {
    let tracks = &app.pages.songs.tracks;
    let cover = tracks.iter().find_map(|t| t.album_art_path.as_deref().filter(|p| !p.is_empty()));
    let mut meta = vec![];
    if let Some(y) = tracks.iter().find_map(|t| t.year) {
        meta.push(y.to_string());
    }
    meta.push(crate::format::plural(tracks.len() as i64, "song", "songs"));
    let total: i64 = tracks.iter().filter_map(|t| t.duration_ms).sum();
    if total > 0 {
        meta.push(super::folders::long_duration(total));
    }
    row![
        crate::ui::art(cover, 96.0, theme::R_LG),
        column![super::library::crumbs(app, tab, label), crate::ui::muted(meta.join(" · "), theme::BODY)].spacing(8),
    ]
    .spacing(16)
    .align_y(Alignment::Center)
    .into()
}
