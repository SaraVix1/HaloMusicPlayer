//! All Songs (`all-songs.tsx`): filter chips from the URL-style query, the
//! Sort ▾ menu (`library::sort`), Play All / Shuffle Play.

use halo_backend::commands::library::{self, TracksQuery};
use iced::widget::{button, row, text};
use iced::{Alignment, Element, Padding, Task};

use super::tracks::{self, Variant};
use super::Msg;
use crate::app::{bg, Halo, Message};
use crate::icons::{icon, Icon};
use crate::route::Route;
use crate::theme::{self, t};

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

#[derive(Debug, Clone, Copy)]
pub enum Filter {
    Album,
    Artist,
    AlbumArtist,
    Composer,
    Genre,
    Year,
}

pub fn without(q: &TracksQuery, f: Filter) -> TracksQuery {
    let mut n = q.clone();
    match f {
        Filter::Album => n.album = None,
        Filter::Artist => n.artist = None,
        Filter::AlbumArtist => n.album_artist = None,
        Filter::Composer => n.composer = None,
        Filter::Genre => n.genre = None,
        Filter::Year => n.year = None,
    }
    n
}

pub fn chips<'a>(q: &TracksQuery) -> Option<Element<'a, Message>> {
    let mut out: Vec<(Filter, String)> = vec![];
    if let Some(v) = &q.album {
        out.push((Filter::Album, format!("Album: {v}")));
    }
    if let Some(v) = &q.artist {
        out.push((Filter::Artist, format!("Artist: {v}")));
    }
    if let Some(v) = &q.album_artist {
        out.push((Filter::AlbumArtist, format!("Album Artist: {v}")));
    }
    if let Some(v) = &q.composer {
        out.push((Filter::Composer, format!("Composer: {v}")));
    }
    if let Some(v) = &q.genre {
        out.push((Filter::Genre, format!("Genre: {v}")));
    }
    if let Some(v) = &q.year {
        out.push((Filter::Year, format!("Year: {v}")));
    }
    if out.is_empty() {
        return None;
    }
    let mut r = row![].spacing(8);
    for (f, label) in out {
        let target = without(q, f);
        r = r.push(
            button(
                row![text(label).size(theme::SMALL).font(theme::MEDIUM), icon(Icon::X, 12.0)]
                    .spacing(4)
                    .align_y(Alignment::Center),
            )
            .padding(Padding::from([4.0, 10.0]))
            .style(|_, st| {
                let k = t();
                iced::widget::button::Style {
                    background: Some(
                        match st {
                            iced::widget::button::Status::Hovered => theme::a(k.muted, 0.7),
                            _ => k.muted,
                        }
                        .into(),
                    ),
                    text_color: k.foreground,
                    border: theme::no_border(theme::R_FULL),
                    ..Default::default()
                }
            })
            .on_press(Message::ReplaceRoute(Route::Songs(target))),
        );
    }
    Some(r.wrap().into())
}

fn empty_library<'a>() -> Element<'a, Message> {
    let k = t();
    row![
        text("No tracks found. ").size(theme::BODY).color(k.muted_fg),
        button(text("Add a folder and scan").size(theme::BODY))
            .padding(0)
            .style(theme::btn_link)
            .on_press(Message::Navigate(Route::Settings(Some(
                crate::route::SettingsTab::Library
            )))),
        text(" to get started.").size(theme::BODY).color(k.muted_fg),
    ]
    .into()
}

pub fn view<'a>(app: &'a Halo, q: &'a TracksQuery) -> Element<'a, Message> {
    // The unfiltered list is the Library page's Songs tab (no h1 of its own);
    // a filtered one (album, artist, genre …) is a page titled "All Songs"
    // with filter chips, as before.
    let title = (!crate::route::is_all_songs(q)).then_some("All Songs");
    tracks::page(app, title, &app.pages.songs, Variant::Songs, Some(q), empty_library())
}

