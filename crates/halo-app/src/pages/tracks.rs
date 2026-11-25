//! Track list shared by All Songs (Library → Songs tab, and album / artist /
//! genre lists), Recently Played and Most Played: 56px virtualised rows —
//! cover · title over artist · album (or last played / plays) · rating ·
//! duration — double-click to play from a row, artist/album links, rating
//! stars shown when rated or on hover, hover actions (add to queue, add to
//! playlist, edit metadata). Sorting is the Sort ▾ menu (`library::sort`).

use halo_backend::commands::library::{Track, TracksQuery};
use iced::widget::{button, column, container, hover, row, text, tooltip::Position, Row};
use iced::{Alignment, Element, Length, Padding};

use super::common::{self, note, ScrollMemory};
use crate::app::Halo;
use super::Msg;
use crate::app::{Message, PlayerCmd};
use crate::format::{duration, relative_time};
use crate::icons::{icon, Icon};
use crate::route::Route;
use crate::theme::{self, a, t};
use crate::ui::{self, art, stars, tip};
use crate::widgets::{line, surface};

pub const ROW_H: f32 = 56.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Variant {
    Songs,
    Recent,
    MostPlayed,
}

/// Track list state for one table.
#[derive(Default)]
pub struct TrackList {
    pub tracks: Vec<Track>,
    pub loading: bool,
    pub loaded: bool,
    pub error: Option<String>,
    pub star_hover: Option<(i64, u8)>,
}

impl TrackList {
    pub fn set_rating(&mut self, track_id: i64, rating: i64) {
        for t in &mut self.tracks {
            if t.id == track_id {
                t.rating = rating;
            }
        }
    }

    pub fn ids(&self) -> Vec<i64> {
        self.tracks.iter().map(|t| t.id).collect()
    }
}

fn file_name(path: &str) -> String {
    path.rsplit(['/', '\\']).next().unwrap_or(path).to_string()
}

/// Column widths: cover | title + artist | album (or extra) | rating |
/// duration | hover actions.
fn cells<'a>(items: [Element<'a, Message>; 6]) -> Row<'a, Message> {
    let widths = [
        Length::Fixed(56.0),
        Length::FillPortion(5),
        Length::FillPortion(3),
        Length::Fixed(96.0),
        Length::Fixed(64.0),
        Length::Fixed(96.0),
    ];
    let mut r = row![].align_y(Alignment::Center).height(Length::Fill);
    for (el, w) in items.into_iter().zip(widths) {
        r = r.push(container(el).width(w).clip(true).align_y(Alignment::Center));
    }
    r
}

fn px3<'a>(el: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    container(el)
        .padding(Padding::from([0.0, 12.0]))
        .width(Length::Fill)
        .into()
}

fn blank<'a>() -> Element<'a, Message> {
    text("").into()
}

/// A muted text link that navigates to a filtered song list.
fn link<'a>(label: String, route: Route, size: f32) -> Element<'a, Message> {
    button(line(label).size(size))
        .padding(0)
        .style(theme::btn_link)
        .on_press(Message::Navigate(route))
        .into()
}

fn links<'a>(values: &[String], size: f32, make: impl Fn(&str) -> TracksQuery) -> Element<'a, Message> {
    if values.is_empty() {
        return ui::muted("Unknown artist", size).into();
    }
    let mut r = row![].align_y(Alignment::Center);
    for (i, v) in values.iter().enumerate() {
        if i > 0 {
            r = r.push(ui::muted(", ", size));
        }
        r = r.push(link(v.clone(), Route::Songs(make(v)), size));
    }
    container(r).clip(true).width(Length::Fill).into()
}

/// Virtualised track rows.
pub fn table<'a>(
    list: &'a TrackList,
    variant: Variant,
    current_track: Option<i64>,
    scroll_key: String,
    memory: &'a ScrollMemory,
) -> Element<'a, Message> {
    let ids: Vec<i64> = list.ids();
    common::vlist(scroll_key, memory, list.tracks.len(), ROW_H, 0.0, move |i| {
        let tr = &list.tracks[i];
        track_row(tr, i, variant, current_track == Some(tr.id), list.star_hover, ids.clone())
    })
}

fn track_row<'a>(
    tr: &'a Track,
    index: usize,
    variant: Variant,
    is_current: bool,
    star_hover: Option<(i64, u8)>,
    ids: Vec<i64>,
) -> Element<'a, Message> {
    let k = t();
    let id = tr.id;
    let title = tr.title.clone().unwrap_or_else(|| file_name(&tr.file_path));
    let third: Element<'a, Message> = match variant {
        Variant::Songs => match &tr.album_name {
            Some(al) => link(
                al.clone(),
                Route::Songs(TracksQuery {
                    album: Some(al.clone()),
                    ..TracksQuery::default()
                }),
                theme::TEXT_SM,
            ),
            None => ui::muted("—", theme::TEXT_SM).into(),
        },
        Variant::Recent => ui::muted(relative_time(tr.last_played_at), theme::TEXT_SM).into(),
        Variant::MostPlayed => ui::muted(crate::format::plural(tr.play_count, "play", "plays"), theme::TEXT_SM).into(),
    };
    let hover_val = star_hover.filter(|(tid, _)| *tid == id).map(|(_, s)| s);
    let rating = move || {
        stars(
            tr.rating,
            12.0,
            false,
            hover_val,
            move |h| Message::Page(Msg::StarHover(h.map(|s| (id, s)))),
            move |r| Message::Player(PlayerCmd::SetRating(id, r)),
        )
    };
    let title_color = if is_current { k.primary } else { k.foreground };

    let base = cells([
        container(art(tr.album_art_path.as_deref(), 40.0, theme::R_SM))
            .padding(Padding::ZERO.left(12.0))
            .into(),
        px3(
            column![
                line(title).size(theme::TEXT_SM).font(theme::MEDIUM).color(title_color).width(Length::Fill),
                links(&tr.artists, theme::TEXT_XS, |a_| TracksQuery {
                    artist: Some(a_.to_string()),
                    ..TracksQuery::default()
                }),
            ]
            .spacing(2),
        ),
        px3(third),
        // Unrated tracks show their stars only on hover (in the overlay).
        if tr.rating > 0 { px3(rating()) } else { blank() },
        px3(
            container(ui::muted(duration(tr.duration_ms), theme::TEXT_SM))
                .width(Length::Fill)
                .align_x(Alignment::End),
        ),
        blank(),
    ]);

    let action = |i: Icon, size: f32, label: &'static str, msg: Message| {
        tip(
            button(ui::icon_box(i, size, 28.0, 28.0))
                .padding(0)
                .style(theme::btn_icon)
                .on_press(msg),
            label,
            Position::Top,
        )
    };
    // Same columns as `base`, so the stars land exactly over their cell.
    let overlay = cells([
        blank(),
        blank(),
        blank(),
        px3(rating()),
        blank(),
        container(
            row![
                action(Icon::ListPlus, 14.0, "Add to queue", Message::Player(PlayerCmd::AddToQueue(id))),
                action(Icon::Plus, 14.0, "Add to playlist", Message::Page(Msg::AddToPlaylist(vec![id]))),
                action(Icon::Pencil, 13.0, "Edit metadata", Message::Page(Msg::EditMetadata(id))),
            ]
            .align_y(Alignment::Center),
        )
        .width(Length::Fill)
        .padding(Padding::ZERO.right(8.0))
        .align_x(Alignment::End)
        .into(),
    ]);

    let content = hover(base, overlay);
    surface(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .on_double_click(Message::Player(PlayerCmd::PlayTracks(ids, index as i64)))
        .style(move |s| container::Style {
            background: if is_current {
                Some(a(k.muted, 0.5).into())
            } else if s.hovered {
                Some(a(k.muted, 0.35).into())
            } else {
                None
            },
            border: theme::no_border(theme::R_MD),
            text_color: Some(k.foreground),
            ..container::Style::default()
        })
        .into()
}

/// "Play All" (primary, filled play) + "Shuffle Play" (outline) buttons.
pub fn play_buttons<'a>(ids: Vec<i64>) -> Element<'a, Message> {
    let k = t();
    let enabled = !ids.is_empty();
    let play_all = button(
        container(
            row![
                ui::filled(ui::Filled::Play, 13.0, k.primary_fg),
                text("Play All").size(theme::TEXT_SM).font(theme::MEDIUM)
            ]
            .spacing(6)
            .align_y(Alignment::Center),
        )
        .height(32)
        .align_y(Alignment::Center),
    )
    .padding(Padding::from([0.0, 12.0]))
    .style(|th, st| {
        let mut s = theme::btn_primary(th, st);
        s.border.radius = theme::radius(theme::R_MD);
        s
    })
    .on_press_maybe(enabled.then(|| Message::Page(Msg::PlayAll(ids.clone()))));
    let shuffle = button(
        container(
            row![icon(Icon::Shuffle, 13.0), text("Shuffle Play").size(theme::TEXT_SM).font(theme::MEDIUM)]
                .spacing(6)
                .align_y(Alignment::Center),
        )
        .height(32)
        .align_y(Alignment::Center),
    )
    .padding(Padding::from([0.0, 12.0]))
    .style(|th, st| {
        let mut s = theme::btn_outline(th, st);
        s.border.radius = theme::radius(theme::R_MD);
        s
    })
    .on_press_maybe(enabled.then(|| Message::Player(PlayerCmd::ShufflePlay(ids.clone()))));
    row![play_all, shuffle].spacing(8).align_y(Alignment::Center).into()
}

/// Page frame shared by the track-list pages: header with Play All /
/// Shuffle Play + count, optional filter chips, then the table.
/// Track list page. `title: None` leaves out the h1 (the Library page's
/// Songs tab, whose title is the tab bar).
pub fn page<'a>(
    app: &'a Halo,
    title: Option<&'a str>,
    list: &'a TrackList,
    variant: Variant,
    q: Option<&TracksQuery>,
    empty: Element<'a, Message>,
) -> Element<'a, Message> {
    let current = app.player.current_track.as_ref().map(|t| t.track_id);
    let n = list.tracks.len();
    let mut right = row![common::count(n, "track", "tracks"), play_buttons(list.ids())]
        .spacing(12)
        .align_y(Alignment::Center);
    // Filtered song lists (album, artist …) sort here; the Songs tab's Sort
    // button is in the Library tab bar.
    if let (Some(_), Variant::Songs, Some(q)) = (title, variant, q) {
        right = right.push(super::library::sort_button(
            app,
            super::library::sort::song_options(q),
            super::library::sort::of_query(q),
        ));
    }
    let header: Element<'a, Message> = match title {
        Some(title) => common::header(title, right),
        None => row![crate::ui::fill(), right].align_y(Alignment::Center).into(),
    };
    let mut col = column![header].spacing(16).height(Length::Fill);
    if let Some(c) = q.and_then(super::songs::chips) {
        col = col.push(c);
    }
    if let Some(e) = &list.error {
        col = col.push(common::error_text(e));
    }
    let key = app.route().scroll_key();
    let body: Element<'a, Message> = if list.loading {
        note("Loading…")
    } else if n == 0 {
        empty
    } else {
        table(list, variant, current, key, &app.pages.scroll)
    };
    col = col.push(body);
    common::padded(col)
}

