//! Track table shared by All Songs, Recently Played and Most Played
//! (`all-songs.tsx` / `recently-played.tsx` / `most-played.tsx`):
//! `# | art | Title | Artist | Album | Genre/Last Played/Plays | Duration |
//! Rating | actions`, 44px virtualised rows, sortable headers (All Songs),
//! double-click to play from a row, artist/album/genre links, hover-only
//! actions (add to queue, add to playlist, edit metadata).

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

pub const ROW_H: f32 = 44.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Variant {
    Songs,
    Recent,
    MostPlayed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortKey {
    Title,
    Artist,
    Album,
    Genre,
    Duration,
    Rating,
}

impl SortKey {
    pub fn as_str(self) -> &'static str {
        match self {
            SortKey::Title => "title",
            SortKey::Artist => "artist",
            SortKey::Album => "album",
            SortKey::Genre => "genre",
            SortKey::Duration => "duration",
            SortKey::Rating => "rating",
        }
    }
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

/// Column widths: 48 | 48 | 2fr | 1.5fr | 1.5fr | 1fr | 80 | 100 | 72.
fn cells<'a>(items: [Element<'a, Message>; 9]) -> Row<'a, Message> {
    let widths = [
        Length::Fixed(48.0),
        Length::Fixed(48.0),
        Length::FillPortion(20),
        Length::FillPortion(15),
        Length::FillPortion(15),
        Length::FillPortion(10),
        Length::Fixed(80.0),
        Length::Fixed(100.0),
        Length::Fixed(72.0),
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

/// A muted text link that navigates to a filtered song list.
fn link<'a>(label: String, route: Route) -> Element<'a, Message> {
    button(line(label).size(theme::TEXT_SM))
        .padding(0)
        .style(theme::btn_link)
        .on_press(Message::Navigate(route))
        .into()
}

fn links<'a>(values: &[String], make: impl Fn(&str) -> TracksQuery) -> Element<'a, Message> {
    if values.is_empty() {
        return ui::muted("—", theme::TEXT_SM).into();
    }
    let mut r = row![].align_y(Alignment::Center);
    for (i, v) in values.iter().enumerate() {
        if i > 0 {
            r = r.push(ui::muted(", ", theme::TEXT_SM));
        }
        r = r.push(link(v.clone(), Route::Songs(make(v))));
    }
    container(r).clip(true).width(Length::Fill).into()
}

fn header_cell<'a>(
    label: &'static str,
    key: Option<SortKey>,
    arrow: Option<(SortKey, bool)>,
    right: bool,
) -> Element<'a, Message> {
    let mut content = row![text(label).size(theme::TEXT_SM).font(theme::MEDIUM)]
        .spacing(4)
        .align_y(Alignment::Center);
    if let (Some(k), Some((s, asc))) = (key, arrow) {
        if k == s {
            content = content.push(icon(if asc { Icon::ArrowUp } else { Icon::ArrowDown }, 12.0));
        }
    }
    let inner = container(content)
        .padding(Padding::from([8.0, 12.0]))
        .width(Length::Fill)
        .align_x(if right { Alignment::End } else { Alignment::Start });
    match key {
        Some(k) => button(inner)
            .padding(0)
            .width(Length::Fill)
            .style(theme::btn_link)
            .on_press(Message::Page(Msg::Sort(k)))
            .into(),
        _ => inner.into(),
    }
}

/// Header + virtualised rows inside a rounded bordered box.
pub fn table<'a>(
    list: &'a TrackList,
    variant: Variant,
    sortable: bool,
    arrow: Option<(SortKey, bool)>,
    current_track: Option<i64>,
    scroll_key: String,
    memory: &'a ScrollMemory,
) -> Element<'a, Message> {
    let k = t();
    let third_label = match variant {
        Variant::Songs => "Genre",
        Variant::Recent => "Last Played",
        Variant::MostPlayed => "Plays",
    };
    let sk = |key| if sortable { Some(key) } else { None };
    let head = cells([
        header_cell("#", None, arrow, false),
        text("").into(),
        header_cell("Title", sk(SortKey::Title), arrow, false),
        header_cell("Artist", sk(SortKey::Artist), arrow, false),
        header_cell("Album", sk(SortKey::Album), arrow, false),
        header_cell(third_label, sk(SortKey::Genre), arrow, false),
        header_cell("Duration", sk(SortKey::Duration), arrow, true),
        header_cell("Rating", sk(SortKey::Rating), arrow, false),
        text("").into(),
    ])
    .height(36);
    let head = container(head).width(Length::Fill).style(move |_| container::Style {
        background: Some(a(t().muted, 0.4).into()),
        text_color: Some(t().muted_fg),
        ..container::Style::default()
    });

    let ids: Vec<i64> = list.ids();
    let rows = common::vlist(scroll_key, memory, list.tracks.len(), ROW_H, 0.0, move |i| {
        let tr = &list.tracks[i];
        track_row(tr, i, variant, current_track == Some(tr.id), list.star_hover, ids.clone())
    });

    container(column![head, rows])
        .width(Length::Fill)
        .height(Length::Fill)
        .clip(true)
        .style(move |_| container::Style {
            border: theme::border(k.border, 1.0, theme::R_MD),
            ..container::Style::default()
        })
        .into()
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
        Variant::Songs => links(&tr.genres, |g| TracksQuery {
            genre: Some(g.to_string()),
            ..TracksQuery::default()
        }),
        Variant::Recent => ui::muted(relative_time(tr.last_played_at), theme::TEXT_SM).into(),
        Variant::MostPlayed => ui::muted(tr.play_count.to_string(), theme::TEXT_SM).into(),
    };
    let album: Element<'a, Message> = match &tr.album_name {
        Some(al) => link(
            al.clone(),
            Route::Songs(TracksQuery {
                album: Some(al.clone()),
                ..TracksQuery::default()
            }),
        ),
        None => ui::muted("—", theme::TEXT_SM).into(),
    };
    let hover_val = star_hover.filter(|(tid, _)| *tid == id).map(|(_, s)| s);

    let base = cells([
        px3(ui::muted((index + 1).to_string(), theme::TEXT_SM)),
        art(tr.album_art_path.as_deref(), 32.0, theme::R_SM),
        px3(line(title).size(theme::TEXT_SM).width(Length::Fill)),
        px3(links(&tr.artists, |a_| TracksQuery {
            artist: Some(a_.to_string()),
            ..TracksQuery::default()
        })),
        px3(album),
        px3(third),
        px3(
            container(ui::muted(duration(tr.duration_ms), theme::TEXT_SM))
                .width(Length::Fill)
                .align_x(Alignment::End),
        ),
        px3(stars(
            tr.rating,
            12.0,
            false,
            hover_val,
            move |h| Message::Page(Msg::StarHover(h.map(|s| (id, s)))),
            move |r| Message::Player(PlayerCmd::SetRating(id, r)),
        )),
        text("").into(),
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
    let actions = row![
        ui::fill(),
        container(
            row![
                action(Icon::ListPlus, 14.0, "Add to queue", Message::Player(PlayerCmd::AddToQueue(id))),
                action(Icon::Plus, 14.0, "Add to playlist", Message::Page(Msg::AddToPlaylist(vec![id]))),
                action(Icon::Pencil, 13.0, "Edit metadata", Message::Page(Msg::EditMetadata(id))),
            ]
            .align_y(Alignment::Center)
        )
        .width(72)
        .padding(Padding::ZERO.right(8.0))
        .align_x(Alignment::End),
    ]
    .height(Length::Fill)
    .align_y(Alignment::Center);

    let content = hover(base, actions);
    column![
        ui::hline(),
        surface(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .on_double_click(Message::Player(PlayerCmd::PlayTracks(ids, index as i64)))
            .style(move |s| container::Style {
                background: if is_current {
                    Some(a(k.muted, 0.4).into())
                } else if s.hovered {
                    Some(a(k.muted, 0.3).into())
                } else {
                    None
                },
                text_color: Some(k.foreground),
                ..container::Style::default()
            }),
    ]
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
    let right = row![play_buttons(list.ids()), common::count(n, "track", "tracks")]
        .spacing(8)
        .align_y(Alignment::Center);
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
        let sortable = variant == Variant::Songs;
        let arrow = q.filter(|q| super::songs::has_arrow(q)).map(super::songs::sort_state);
        table(list, variant, sortable, arrow, current, key, &app.pages.scroll)
    };
    col = col.push(body);
    common::padded(col)
}

