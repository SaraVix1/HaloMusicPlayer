//! Albums grid (`albums.tsx`): square art with a hover Play button, name and
//! album artists (or track count); click opens the album's songs.

use halo_backend::commands::library::{self, Album, TracksQuery};
use iced::widget::{button, center, column, container};
use iced::{Alignment, Element, Length, Padding, Task};

use super::common::GridSpec;
use super::library::coll_grid;
use super::Msg;
use crate::app::{bg, Halo, Message};
use crate::format::plural;
use crate::icons::{icon, Icon};
use crate::route::Route;
use crate::theme::{self, a, t};
use crate::ui::art_sized;
use crate::widgets::{line, surface};

const ALBUMS: GridSpec = GridSpec {
    min_item_width: 160.0,
    row_height: 240.0,
    gap: 24.0,
    px: 24.0,
    padding_top: 0.0,
    padding_bottom: 24.0,
};

/// `/songs?album=…&sort=track_number&dir=asc`
pub fn album_route(name: &str) -> Route {
    Route::Songs(TracksQuery {
        album: Some(name.to_string()),
        sort: Some("track_number".into()),
        direction: Some("asc".into()),
        ..TracksQuery::default()
    })
}

/// Play an album from its first track (Play button on album cards).
pub fn play_album(app: &Halo, name: String) -> Task<Message> {
    bg(
        &app.backend,
        move |b| {
            let tracks = library::get_tracks(
                TracksQuery {
                    album: Some(name),
                    sort: Some("track_number".into()),
                    direction: Some("asc".into()),
                    ..TracksQuery::default()
                },
                b.db(),
            )?;
            if tracks.is_empty() {
                return Ok(());
            }
            halo_backend::commands::player::set_queue_and_play(
                b,
                tracks.iter().map(|t| t.id).collect(),
                0,
            )
        },
        |r: Result<(), String>| match r {
            Ok(()) => Message::Noop,
            Err(e) => Message::Error(e),
        },
    )
}



pub(crate) fn album_card(al: &Album, side: f32) -> Element<'_, Message> {
    let k = t();
    let name = al.name.clone();
    let sub = if al.album_artists.is_empty() {
        plural(al.track_count, "track", "tracks")
    } else {
        al.album_artists.join(", ")
    };
    let cover = surface(art_sized(
        al.album_art_path.as_deref(),
        Length::Fill,
        Length::Fill,
        160.0,
        theme::R_LG,
    ))
    .width(Length::Fill)
    .height(Length::Fill)
    .on_press(Message::Navigate(album_route(&name)))
    .interaction(iced::mouse::Interaction::Pointer)
    .style(|s| iced::widget::container::Style {
        // group-hover:opacity-80 — veil with the page background.
        background: s.hovered.then(|| a(t().background, 0.2).into()),
        ..Default::default()
    });
    let play_btn = button(
        center(container(icon(Icon::Play, 16.0)).padding(Padding::ZERO.left(2.0)))
            .width(40)
            .height(40),
    )
    .padding(0)
    .style(|th, st| {
        let mut s = theme::btn_primary(th, st);
        s.border.radius = theme::radius(theme::R_FULL);
        s.shadow = theme::shadow_lg();
        s
    })
    .on_press(Message::Page(Msg::PlayAlbum(name.clone())));
    let play_layer = container(play_btn)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Alignment::End)
        .align_y(Alignment::End)
        .padding(8);
    let art_box = iced::widget::hover(
        container(cover).width(Length::Fill).height(Length::Fill),
        container(play_layer).width(Length::Fill).height(Length::Fill),
    );
    // aspect-square: the art is as tall as the column is wide.
    let art_box = container(art_box).width(Length::Fill).height(side);
    column![
        art_box,
        column![
            button(line(al.name.clone()).size(theme::BODY).font(theme::MEDIUM))
                .padding(0)
                .style(theme::btn_text(k.foreground, k.foreground))
                .on_press(Message::Navigate(album_route(&name))),
            line(sub).size(theme::SMALL).color(k.muted_fg).width(Length::Fill),
        ]
        .spacing(0),
    ]
    .spacing(8)
    .into()
}

/// Albums grid (the Library page's Albums tab).
pub fn grid(app: &Halo, key: String, padding_top: Option<f32>) -> Element<'_, Message> {
    coll_grid(
        &app.pages.library.albums,
        "No albums yet — scan a folder to populate the library.",
        ALBUMS,
        key,
        &app.pages.scroll,
        padding_top,
        |al: &Album, w| album_card(al, w.min(196.0)),
    )
}

