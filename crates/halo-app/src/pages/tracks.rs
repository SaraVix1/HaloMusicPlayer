//! Track list shared by All Songs (Library → Songs tab, and album / artist /
//! genre lists), Recently Played and Most Played: 56px virtualised rows —
//! cover · title over artist · album (or last played / plays) · rating ·
//! duration — double-click to play from a row, artist/album links, rating
//! stars shown when rated or on hover, hover actions (add to queue, add to
//! playlist, edit metadata). Sorting is the Sort ▾ menu (`library::sort`).

use halo_backend::commands::library::{Track, TracksQuery};
use iced::widget::{button, column, container, hover, row, text, tooltip::Position, Row};
use iced::{Alignment, Element, Length, Padding};

use super::common::{self, note, GridSpec, ScrollMemory};
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
    /// An album's page: track number instead of the (identical) cover,
    /// plays instead of the album; always rows.
    Album,
}

/// Track number for album rows: `3`, or `2-05` when the album has several
/// discs; `—` when untagged.
pub fn track_label(disc: Option<i64>, track: Option<i64>, multi_disc: bool) -> String {
    match (track, multi_disc) {
        (Some(n), true) => format!("{}-{n:02}", disc.unwrap_or(1)),
        (Some(n), false) => n.to_string(),
        (None, _) => "—".into(),
    }
}

fn multi_disc(list: &TrackList) -> bool {
    list.tracks.iter().any(|t| t.disc_number.unwrap_or(1) > 1)
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

/// Song cards grid (Songs tab, ▦ Cards): wide tiles (cover left, text
/// right), at least 260 px, as many columns as fit.
const CARDS: GridSpec = GridSpec {
    min_item_width: 260.0,
    row_height: CARD_H,
    gap: 12.0,
    px: 0.0,
    padding_top: 0.0,
    padding_bottom: 24.0,
};
const CARD_H: f32 = 72.0;
const CARD_ART: f32 = 56.0;

/// Virtualised grid of song cards.
pub fn cards<'a>(
    list: &'a TrackList,
    current_track: Option<i64>,
    scroll_key: String,
    memory: &'a ScrollMemory,
) -> Element<'a, Message> {
    let ids: Vec<i64> = list.ids();
    common::vgrid(scroll_key, memory, list.tracks.len(), CARDS, move |i, _w| {
        let tr = &list.tracks[i];
        song_card(tr, i, current_track == Some(tr.id), ids.clone())
    })
}

/// Tiles without virtualisation (a folder's own songs under its subfolder
/// cards, inside the page's scroll).
pub fn cards_static<'a>(list: &'a TrackList, current_track: Option<i64>) -> Element<'a, Message> {
    let ids = list.ids();
    iced::widget::responsive(move |size| {
        let cols = (((size.width + CARDS.gap) / (CARDS.min_item_width + CARDS.gap)).floor() as usize).max(1);
        let mut col = column![].spacing(CARDS.gap);
        for (r, chunk) in list.tracks.chunks(cols).enumerate() {
            let mut line_ = row![].spacing(CARDS.gap);
            for (c, tr) in chunk.iter().enumerate() {
                let i = r * cols + c;
                line_ = line_.push(container(song_card(tr, i, current_track == Some(tr.id), ids.clone())).width(Length::FillPortion(1)));
            }
            for _ in chunk.len()..cols {
                line_ = line_.push(iced::widget::Space::new().width(Length::FillPortion(1)));
            }
            col = col.push(line_);
        }
        col.into()
    })
    .height(Length::Shrink)
    .into()
}

/// Wide tile: cover (▶ over it on hover), then title / artist, and album
/// with the length at the right end. Click plays it (the rest of the list
/// queued after it); right-click: the song menu.
fn song_card<'a>(tr: &'a Track, index: usize, is_current: bool, ids: Vec<i64>) -> Element<'a, Message> {
    let k = t();
    let title = tr.title.clone().unwrap_or_else(|| file_name(&tr.file_path));
    let artists = if tr.artists.is_empty() { "Unknown artist".to_string() } else { tr.artists.join(", ") };
    let album = tr.album_name.clone().filter(|s| !s.is_empty()).unwrap_or_default();
    let play = Message::Player(PlayerCmd::PlayTracks(ids, index as i64));
    let bottom = row![
        line(album).size(theme::SMALL).color(a(k.muted_fg, 0.8)).width(Length::Fill),
        text(duration(tr.duration_ms)).size(theme::SMALL).color(k.muted_fg),
    ]
    .spacing(8)
    .align_y(Alignment::Center);
    let text_col = column![
        line(title).size(theme::BODY).font(theme::MEDIUM).color(if is_current { k.primary } else { k.foreground }).width(Length::Fill),
        line(artists).size(theme::SMALL).color(k.muted_fg).width(Length::Fill),
        bottom,
    ]
    .spacing(1)
    .width(Length::Fill);
    // ▶ over the cover on hover (the right end shows the length).
    let play_btn = button(
        iced::widget::center(container(icon(Icon::Play, 12.0)).padding(Padding::ZERO.left(2.0))).width(28).height(28),
    )
    .padding(0)
    .style(|th, st| {
        let mut s = theme::btn_primary(th, st);
        s.border.radius = theme::radius(theme::R_FULL);
        s.shadow = theme::shadow_lg();
        s
    })
    .on_press(play.clone());
    let cover = hover(
        art(tr.album_art_path.as_deref(), CARD_ART, theme::R_MD),
        iced::widget::center(play_btn).width(CARD_ART).height(CARD_ART),
    );
    let content = row![cover, text_col].spacing(12).align_y(Alignment::Center).height(Length::Fill);
    surface(content)
        .width(Length::Fill)
        .height(CARD_H)
        .padding(Padding::from([8.0, 12.0]))
        .on_press(play)
        .on_right_press(crate::components::context_menu::opener(crate::components::context_menu::Target::Track {
            id: tr.id,
            album: tr.album_name.clone(),
            artists: tr.artists.clone(),
            playlist: None,
        }))
        .interaction(iced::mouse::Interaction::Pointer)
        .style(move |s| {
            let mut st = theme::card(s.hovered);
            // Resting tiles get a faint surface so the grid reads as tiles.
            if !s.hovered {
                st.background = Some(a(t().muted, if is_current { 0.5 } else { 0.18 }).into());
            }
            st
        })
        .into()
}

/// Plain (non-virtual) rows: a folder's own few songs under its subfolder
/// cards, inside the page's scroll.
pub fn rows<'a>(list: &'a TrackList, variant: Variant, current_track: Option<i64>) -> Element<'a, Message> {
    let ids = list.ids();
    let multi = multi_disc(list);
    let mut col = column![];
    for (i, tr) in list.tracks.iter().enumerate() {
        col = col.push(
            container(track_row(tr, i, variant, current_track == Some(tr.id), list.star_hover, ids.clone(), multi))
                .height(ROW_H),
        );
    }
    col.into()
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
    let multi = multi_disc(list);
    common::vlist(scroll_key, memory, list.tracks.len(), ROW_H, 0.0, move |i| {
        let tr = &list.tracks[i];
        track_row(tr, i, variant, current_track == Some(tr.id), list.star_hover, ids.clone(), multi)
    })
}

fn track_row<'a>(
    tr: &'a Track,
    index: usize,
    variant: Variant,
    is_current: bool,
    star_hover: Option<(i64, u8)>,
    ids: Vec<i64>,
    multi_disc: bool,
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
                theme::BODY,
            ),
            None => ui::muted("—", theme::BODY).into(),
        },
        Variant::Recent => ui::muted(relative_time(tr.last_played_at), theme::BODY).into(),
        Variant::MostPlayed => ui::muted(crate::format::plural(tr.play_count, "play", "plays"), theme::BODY).into(),
        Variant::Album if tr.play_count > 0 => {
            ui::muted(crate::format::plural(tr.play_count, "play", "plays"), theme::BODY).into()
        }
        Variant::Album => blank(),
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

    let lead: Element<'a, Message> = if variant == Variant::Album {
        container(ui::muted(track_label(tr.disc_number, tr.track_number, multi_disc), theme::BODY))
            .width(Length::Fill)
            .align_x(Alignment::Center)
            .into()
    } else {
        container(art(tr.album_art_path.as_deref(), 40.0, theme::R_SM)).padding(Padding::ZERO.left(12.0)).into()
    };
    let base = cells([
        lead,
        px3(
            column![
                line(title).size(theme::BODY).font(theme::MEDIUM).color(title_color).width(Length::Fill),
                links(&tr.artists, theme::SMALL, |a_| TracksQuery {
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
            container(ui::muted(duration(tr.duration_ms), theme::BODY))
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
        .on_right_press(crate::components::context_menu::opener(
            crate::components::context_menu::Target::Track {
                id,
                album: tr.album_name.clone(),
                artists: tr.artists.clone(),
                playlist: None,
            },
        ))
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
                text("Play All").size(theme::BODY).font(theme::MEDIUM)
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
            row![icon(Icon::Shuffle, 13.0), text("Shuffle Play").size(theme::BODY).font(theme::MEDIUM)]
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

/// Track list page: `head` (a breadcrumb, an h1, or nothing — the Songs
/// tab, whose title is the tab bar) on the left of the count and Play All /
/// Shuffle Play; `toggle` adds ☰ / ▦ there (pages outside the Library tab
/// bar). Album pages are always rows; the rest follow the Cards / List view.
pub fn page<'a>(
    app: &'a Halo,
    head: Option<Element<'a, Message>>,
    list: &'a TrackList,
    variant: Variant,
    empty: Element<'a, Message>,
    toggle: bool,
) -> Element<'a, Message> {
    let current = app.player.current_track.as_ref().map(|t| t.track_id);
    let n = list.tracks.len();
    let mut right = row![common::count(n, "track", "tracks"), play_buttons(list.ids())]
        .spacing(12)
        .align_y(Alignment::Center);
    if toggle && variant != Variant::Album {
        right = right.push(super::library::view_toggle(app.pages.library.songs_view));
    }
    let header: Element<'a, Message> = match head {
        Some(h) => row![h, crate::ui::fill(), right].spacing(16).align_y(Alignment::Center).into(),
        None => row![crate::ui::fill(), right].align_y(Alignment::Center).into(),
    };
    let mut col = column![header].spacing(16).height(Length::Fill);
    if let Some(e) = &list.error {
        col = col.push(common::error_text(e));
    }
    let key = app.route().scroll_key();
    let body: Element<'a, Message> = if list.loading {
        note("Loading…")
    } else if n == 0 {
        empty
    } else if variant != Variant::Album && app.pages.library.songs_view == super::library::SongsView::Cards {
        cards(list, current, key, &app.pages.scroll)
    } else {
        table(list, variant, current, key, &app.pages.scroll)
    };
    col = col.push(body);
    common::padded(col)
}

#[cfg(test)]
mod tests {
    use super::track_label;

    #[test]
    fn album_track_numbers() {
        assert_eq!(track_label(Some(1), Some(3), false), "3");
        assert_eq!(track_label(None, Some(12), false), "12");
        assert_eq!(track_label(Some(2), Some(5), true), "2-05");
        assert_eq!(track_label(None, Some(7), true), "1-07");
        assert_eq!(track_label(Some(1), None, true), "—");
    }
}

