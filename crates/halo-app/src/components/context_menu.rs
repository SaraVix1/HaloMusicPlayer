//! Right-click menus for songs, albums and artists: one app-wide layer that
//! opens at the cursor (flipped left / up near the window edges) and closes
//! on any click elsewhere, a scroll, Esc or navigation.
//!
//! | Target | Items |
//! |---|---|
//! | Song | Play · Play Next · Add to Queue · Add to Playlist… · Go to Album · Go to Artist (≤3) · Edit Tags… · (in a playlist) Remove from Playlist |
//! | Album | Play · Play Next · Add to Queue · Add to Playlist… · Go to Artist (≤3) |
//! | Artist | Play · Play Next · Add to Queue · Add to Playlist… · Go to Artist |

use halo_backend::commands::library::{self, TracksQuery};
use halo_backend::commands::player;
use halo_backend::Backend;
use iced::widget::{button, column, container, mouse_area, row, stack, text, Space};
use iced::{Alignment, Element, Length, Padding, Point, Size, Task};

use crate::app::{bg, fire, Halo, Message};
use crate::icons::{icon, Icon};
use crate::pages::albums::album_route;
use crate::route::Route;
use crate::theme::{self, t};
use crate::ui;

const WIDTH: f32 = 230.0;
const ITEM_H: f32 = 32.0;
const SEP_H: f32 = 9.0;
const PAD: f32 = 4.0;
/// Keep this far from the window edges.
const MARGIN: f32 = 8.0;
const MAX_ARTISTS: usize = 3;

#[derive(Debug, Clone)]
pub enum Target {
    Track {
        id: i64,
        album: Option<String>,
        artists: Vec<String>,
        /// The playlist entry (playlist_track id) when right-clicked in an
        /// open playlist: adds "Remove from Playlist".
        playlist: Option<i64>,
    },
    Album { name: String, artists: Vec<String> },
    Artist { name: String, album_artist: bool },
}

#[derive(Debug, Clone)]
pub enum Item {
    Play,
    PlayNext,
    AddToQueue,
    AddToPlaylist,
    GoToAlbum(String),
    /// (name, it's an album artist)
    GoToArtist(String, bool),
    EditTags(i64),
    /// playlist_track id (in the open playlist)
    RemoveFromPlaylist(i64),
}

#[derive(Default)]
pub struct State {
    pub open: Option<(Point, Target)>,
}

#[derive(Debug, Clone)]
pub enum Msg {
    Open(Point, Target),
    Close,
    Do(Item),
}

fn cm(m: Msg) -> Message {
    Message::Context(m)
}

/// For `surface(..).on_right_press_at(...)`.
pub fn opener(target: Target) -> impl Fn(Point) -> Message {
    move |p| cm(Msg::Open(p, target.clone()))
}

pub fn is_open(app: &Halo) -> bool {
    app.context.open.is_some()
}

pub fn close(app: &mut Halo) {
    app.context.open = None;
}

/// Menu rows: `None` is a separator.
fn items(target: &Target) -> Vec<Option<(Icon, String, Item)>> {
    let mut v: Vec<Option<(Icon, String, Item)>> = vec![
        Some((Icon::Play, "Play".into(), Item::Play)),
        Some((Icon::ListOrdered, "Play Next".into(), Item::PlayNext)),
        Some((Icon::ListPlus, "Add to Queue".into(), Item::AddToQueue)),
        Some((Icon::Plus, "Add to Playlist…".into(), Item::AddToPlaylist)),
        None,
    ];
    let artist = |name: &str, aa: bool| Some((Icon::Mic2, format!("Go to {name}"), Item::GoToArtist(name.to_string(), aa)));
    match target {
        Target::Track { id, album, artists, playlist } => {
            if let Some(al) = album.as_ref().filter(|s| !s.is_empty()) {
                v.push(Some((Icon::Disc, "Go to Album".into(), Item::GoToAlbum(al.clone()))));
            }
            v.extend(artists.iter().take(MAX_ARTISTS).map(|a| artist(a, false)));
            v.push(None);
            v.push(Some((Icon::Pencil, "Edit Tags…".into(), Item::EditTags(*id))));
            if let Some(pt) = playlist {
                v.push(Some((Icon::X, "Remove from Playlist".into(), Item::RemoveFromPlaylist(*pt))));
            }
        }
        Target::Album { artists, .. } => {
            v.extend(artists.iter().take(MAX_ARTISTS).map(|a| artist(a, true)));
        }
        Target::Artist { name, album_artist } => {
            v.push(Some((Icon::Mic2, "Go to Artist".into(), Item::GoToArtist(name.clone(), *album_artist))));
        }
    }
    // No trailing / doubled separators.
    while matches!(v.last(), Some(None)) {
        v.pop();
    }
    v
}

fn menu_height(rows: &[Option<(Icon, String, Item)>]) -> f32 {
    rows.iter().map(|r| if r.is_some() { ITEM_H } else { SEP_H }).sum::<f32>() + PAD * 2.0
}

/// Top-left corner for a `menu`-sized box opened at `at`: to the right of
/// and below the cursor, flipped left / up when it would leave `window`.
pub fn place(at: Point, menu: Size, window: Size) -> Point {
    let x = if at.x + menu.width > window.width - MARGIN { at.x - menu.width } else { at.x };
    let y = if at.y + menu.height > window.height - MARGIN { at.y - menu.height } else { at.y };
    Point::new(
        x.clamp(MARGIN, (window.width - menu.width - MARGIN).max(MARGIN)),
        y.clamp(MARGIN, (window.height - menu.height - MARGIN).max(MARGIN)),
    )
}

pub fn update(app: &mut Halo, m: Msg) -> Task<Message> {
    match m {
        Msg::Open(p, target) => {
            app.context.open = Some((p, target));
            Task::none()
        }
        Msg::Close => {
            close(app);
            Task::none()
        }
        Msg::Do(item) => {
            let Some((_, target)) = app.context.open.take() else { return Task::none() };
            run(app, target, item)
        }
    }
}

/// The target's songs, in album / artist order.
fn track_ids(b: &Backend, target: &Target) -> Result<Vec<i64>, String> {
    let q = match target {
        Target::Track { id, .. } => return Ok(vec![*id]),
        Target::Album { name, .. } => TracksQuery {
            album: Some(name.clone()),
            sort: Some("track_number".into()),
            direction: Some("asc".into()),
            ..TracksQuery::default()
        },
        Target::Artist { name, album_artist: true } => TracksQuery {
            album_artist: Some(name.clone()),
            sort: Some("album".into()),
            ..TracksQuery::default()
        },
        Target::Artist { name, album_artist: false } => TracksQuery {
            artist: Some(name.clone()),
            sort: Some("album".into()),
            ..TracksQuery::default()
        },
    };
    Ok(library::get_tracks(q, b.db())?.iter().map(|t| t.id).collect())
}

fn run(app: &mut Halo, target: Target, item: Item) -> Task<Message> {
    let is_track = matches!(target, Target::Track { .. });
    match item {
        Item::Play => fire(&app.backend, move |b| {
            let ids = track_ids(&b, &target)?;
            match (is_track, ids.first()) {
                // A song plays now and keeps the queue (as in Ctrl+K).
                (true, Some(&id)) => player::play_now(b, id),
                (false, Some(_)) => player::set_queue_and_play(b, ids, 0),
                (_, None) => Ok(()),
            }
        }),
        Item::PlayNext => fire(&app.backend, move |b| {
            let ids = track_ids(&b, &target)?;
            player::insert_next(b, ids).map(|_| ())
        }),
        Item::AddToQueue => fire(&app.backend, move |b| {
            let ids = track_ids(&b, &target)?;
            player::add_tracks_to_queue(b, ids)
        }),
        Item::AddToPlaylist => bg(
            &app.backend,
            move |b| track_ids(&b, &target),
            |r| match r {
                Ok(ids) if !ids.is_empty() => Message::Page(crate::pages::Msg::AddToPlaylist(ids)),
                Ok(_) => Message::Noop,
                Err(e) => Message::Error(e),
            },
        ),
        Item::GoToAlbum(name) => app.update(Message::Navigate(album_route(&name))),
        Item::GoToArtist(name, album_artist) => app.update(Message::Navigate(Route::Songs(if album_artist {
            TracksQuery { album_artist: Some(name), ..TracksQuery::default() }
        } else {
            TracksQuery { artist: Some(name), ..TracksQuery::default() }
        }))),
        Item::EditTags(id) => app.update(Message::Page(crate::pages::Msg::EditMetadata(id))),
        Item::RemoveFromPlaylist(pt) => app.update(Message::Page(crate::pages::Msg::PlaylistDetail(
            crate::pages::playlist_detail::DMsg::Remove(pt),
        ))),
    }
}

/// The menu layer (over everything in the main window).
pub fn view(app: &Halo) -> Option<Element<'_, Message>> {
    let (at, target) = app.context.open.as_ref()?;
    let rows = items(target);
    let size = Size::new(WIDTH, menu_height(&rows));
    let pos = place(*at, size, app.main_size);

    let mut col = column![].width(WIDTH - PAD * 2.0);
    for r in rows {
        col = col.push(match r {
            Some((ic, label, item)) => Element::from(
                button(
                    row![icon(ic, 14.0), text(label).size(theme::BODY)]
                        .spacing(10)
                        .align_y(Alignment::Center),
                )
                .width(Length::Fill)
                .height(ITEM_H)
                .padding(Padding::from([0.0, 10.0]))
                .style(theme::btn_list_item(false, theme::R_SM))
                .on_press(cm(Msg::Do(item))),
            ),
            None => container(ui::hline::<Message>()).height(SEP_H).align_y(Alignment::Center).into(),
        });
    }
    let menu = container(col).padding(PAD).style(|th| {
        let mut s = theme::popover(th);
        s.border.radius = theme::radius(theme::R_LG);
        s.text_color = Some(t().popover_fg);
        s
    });

    // Any click / scroll outside closes it.
    let catcher = mouse_area(Space::new().width(Length::Fill).height(Length::Fill))
        .on_press(cm(Msg::Close))
        .on_right_press(cm(Msg::Close))
        .on_scroll(|_| cm(Msg::Close));
    Some(
        stack![
            catcher,
            container(iced::widget::opaque(menu)).padding(Padding::ZERO.left(pos.x).top(pos.y)),
        ]
        .width(Length::Fill)
        .height(Length::Fill)
        .into(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const WIN: Size = Size::new(1200.0, 750.0);
    const MENU: Size = Size::new(230.0, 300.0);

    #[test]
    fn opens_below_right_of_the_cursor() {
        assert_eq!(place(Point::new(100.0, 100.0), MENU, WIN), Point::new(100.0, 100.0));
    }

    #[test]
    fn flips_left_and_up_near_the_edges() {
        // Bottom-right corner: up and to the left of the cursor.
        assert_eq!(place(Point::new(1150.0, 700.0), MENU, WIN), Point::new(920.0, 400.0));
        // Right edge only.
        assert_eq!(place(Point::new(1100.0, 50.0), MENU, WIN), Point::new(870.0, 50.0));
    }

    #[test]
    fn stays_inside_a_small_window() {
        let p = place(Point::new(50.0, 50.0), MENU, Size::new(200.0, 200.0));
        assert!(p.x >= MARGIN && p.y >= MARGIN);
    }

    #[test]
    fn items_per_target() {
        let labels = |t: &Target| -> Vec<String> { items(t).into_iter().map(|r| r.map(|(_, l, _)| l).unwrap_or("—".into())).collect() };
        let song = Target::Track {
            id: 1,
            album: Some("Roja".into()),
            artists: vec!["A".into(), "B".into(), "C".into(), "D".into()],
            playlist: Some(70),
        };
        assert_eq!(
            labels(&song),
            [
                "Play", "Play Next", "Add to Queue", "Add to Playlist…", "—", "Go to Album", "Go to A", "Go to B",
                "Go to C", "—", "Edit Tags…", "Remove from Playlist"
            ]
        );
        // Album without artists: no dangling separator.
        let album = Target::Album { name: "X".into(), artists: vec![] };
        assert_eq!(labels(&album), ["Play", "Play Next", "Add to Queue", "Add to Playlist…"]);
        let artist = Target::Artist { name: "A".into(), album_artist: false };
        assert_eq!(labels(&artist).last().map(String::as_str), Some("Go to Artist"));
    }
}
