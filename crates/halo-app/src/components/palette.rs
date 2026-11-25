//! Ctrl+K quick search: debounced (120 ms) multi-word search over songs,
//! albums, artists and playlists (`search::search_library` ranks them),
//! with the best hit as a "Top result" card. ↑/↓ move, Esc closes.
//!
//! | Row | Enter | Shift+Enter | Ctrl+Enter |
//! |---|---|---|---|
//! | Song | play now, queue kept | play now | add to end of queue |
//! | Album / artist / playlist | open | play all | — |
//!
//! Hovering a row shows ▶ (and ⊕ for songs). An empty box lists recent
//! searches (`search.recent` pref; the last 6 that led to a pick).

use std::time::Duration;

use halo_backend::commands::library::{self, TracksQuery};
use halo_backend::commands::search::{self, SearchResults, TopHit};
use halo_backend::commands::{player, playlists, ui as prefs};
use iced::keyboard::Modifiers;
use iced::widget::{
    button, center, column, container, hover, mouse_area, opaque, row, scrollable, stack, text,
    text_input, tooltip::Position, Space,
};
use iced::{event, keyboard, Alignment, Element, Length, Padding, Subscription, Task};

use crate::app::{bg, fire, Halo, Message};
use crate::format::plural;
use crate::icons::{icon, Icon};
use crate::pages::albums::album_route;
use crate::route::Route;
use crate::theme::{self, a, t};
use crate::ui::{self, art, icon_box, kbd, tip};
use crate::widgets::{line, surface};

const INPUT_ID: &str = "command-palette-input";
const LIST_ID: &str = "command-palette-list";
const ROW_H: f32 = 48.0;
const TOP_H: f32 = 76.0;
const GROUP_H: f32 = 26.0;
const RECENT_PREF: &str = "search.recent";
const RECENT_MAX: usize = 6;

#[derive(Default)]
pub struct State {
    pub open: bool,
    pub query: String,
    pub results: Option<SearchResults>,
    pub active: usize,
    recent: Vec<String>,
    mods: Modifiers,
    token: u64,
    scroll: f32,
    viewport: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Enter: songs play now (queue kept); others open.
    Default,
    /// Shift+Enter / ▶: play (albums etc. replace the queue).
    Play,
    /// Ctrl+Enter / ⊕: songs go to the end of the queue.
    Queue,
}

#[derive(Debug, Clone)]
pub enum Msg {
    Open,
    Close,
    Recent(Vec<String>),
    Query(String),
    Search(u64),
    Results(u64, SearchResults),
    Move(i32),
    Hover(usize),
    Mods(Modifiers),
    /// Enter in the box: the action depends on Shift / Ctrl.
    Submit,
    Select(usize, Action),
    Scrolled(f32, f32),
}

#[derive(Debug, Clone)]
enum Kind {
    Track(i64),
    Album(String),
    Artist(String),
    Playlist(i64),
    Recent(String),
}

#[derive(Debug, Clone)]
struct Row {
    kind: Kind,
    title: String,
    sub: String,
    art: Option<String>,
    group: &'static str,
    top: bool,
}

impl Row {
    fn height(&self) -> f32 {
        if self.top { TOP_H } else { ROW_H }
    }
}

fn track_row(t: &search::SearchTrackHit, group: &'static str, top: bool) -> Row {
    let sub = [Some(t.artists.join(", ")).filter(|s| !s.is_empty()), t.album_name.clone()]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" · ");
    Row { kind: Kind::Track(t.track_id), title: t.title.clone(), sub, art: t.album_art_path.clone(), group, top }
}

fn album_row(al: &search::SearchAlbumHit, group: &'static str, top: bool) -> Row {
    let mut parts = vec![];
    if !al.artists.is_empty() {
        parts.push(al.artists.join(", "));
    }
    if let Some(y) = al.year {
        parts.push(y.to_string());
    }
    if parts.is_empty() {
        parts.push(plural(al.track_count, "track", "tracks"));
    }
    Row { kind: Kind::Album(al.name.clone()), title: al.name.clone(), sub: parts.join(" · "), art: al.album_art_path.clone(), group, top }
}

fn artist_row(ar: &search::SearchArtistHit, group: &'static str, top: bool) -> Row {
    Row {
        kind: Kind::Artist(ar.name.clone()),
        title: ar.name.clone(),
        sub: plural(ar.track_count, "track", "tracks"),
        art: ar.album_art_path.clone(),
        group,
        top,
    }
}

fn playlist_row(p: &search::SearchPlaylistHit, group: &'static str, top: bool) -> Row {
    Row { kind: Kind::Playlist(p.id), title: p.name.clone(), sub: plural(p.track_count, "track", "tracks"), art: None, group, top }
}

/// Results as display rows: Top result first (and not repeated in its
/// group), then Songs, Albums, Artists, Playlists. Empty box: recents.
fn rows(st: &State) -> Vec<Row> {
    let Some(r) = &st.results else {
        return st
            .recent
            .iter()
            .map(|q| Row { kind: Kind::Recent(q.clone()), title: q.clone(), sub: String::new(), art: None, group: "Recent searches", top: false })
            .collect();
    };
    let mut out = vec![];
    match r.top {
        Some(TopHit::Track(i)) => out.extend(r.tracks.get(i).map(|x| track_row(x, "Top result", true))),
        Some(TopHit::Album(i)) => out.extend(r.albums.get(i).map(|x| album_row(x, "Top result", true))),
        Some(TopHit::Artist(i)) => out.extend(r.artists.get(i).map(|x| artist_row(x, "Top result", true))),
        Some(TopHit::Playlist(i)) => out.extend(r.playlists.get(i).map(|x| playlist_row(x, "Top result", true))),
        None => {}
    }
    let skip = |hit: TopHit| r.top == Some(hit);
    for (_, x) in r.tracks.iter().enumerate().filter(|(i, _)| !skip(TopHit::Track(*i))) {
        out.push(track_row(x, "Songs", false));
    }
    for (_, x) in r.albums.iter().enumerate().filter(|(i, _)| !skip(TopHit::Album(*i))) {
        out.push(album_row(x, "Albums", false));
    }
    for (_, x) in r.artists.iter().enumerate().filter(|(i, _)| !skip(TopHit::Artist(*i))) {
        out.push(artist_row(x, "Artists", false));
    }
    for (_, x) in r.playlists.iter().enumerate().filter(|(i, _)| !skip(TopHit::Playlist(*i))) {
        out.push(playlist_row(x, "Playlists", false));
    }
    out
}

fn pm(m: Msg) -> Message {
    Message::Palette(m)
}

pub fn toggle(app: &mut Halo) -> Task<Message> {
    if app.palette.open {
        update(app, Msg::Close)
    } else {
        update(app, Msg::Open)
    }
}

/// Top offset of row `i` inside the results list (rows + group headers).
fn row_top(rows: &[Row], i: usize) -> f32 {
    let mut y = 4.0;
    let mut last: Option<&str> = None;
    for (j, r) in rows.iter().enumerate() {
        if last != Some(r.group) {
            y += GROUP_H;
            last = Some(r.group);
        }
        if j == i {
            return y;
        }
        y += r.height();
    }
    y
}

/// `query` first, without case-insensitive duplicates, at most RECENT_MAX.
fn push_recent(recent: &[String], query: &str) -> Vec<String> {
    let q = query.trim();
    let mut out = vec![q.to_string()];
    out.extend(recent.iter().filter(|r| !r.eq_ignore_ascii_case(q)).cloned());
    out.truncate(RECENT_MAX);
    out
}

pub fn subscription(app: &Halo) -> Option<Subscription<Message>> {
    app.palette.open.then(|| {
        event::listen_with(|e, _, _| match e {
            iced::Event::Keyboard(keyboard::Event::ModifiersChanged(m)) => Some(pm(Msg::Mods(m))),
            _ => None,
        })
    })
}

pub fn update(app: &mut Halo, msg: Msg) -> Task<Message> {
    let st = &mut app.palette;
    match msg {
        Msg::Open => {
            *st = State { open: true, ..State::default() };
            let load = bg(
                &app.backend,
                |b| {
                    prefs::get_pref(RECENT_PREF.into(), b.db())
                        .ok()
                        .flatten()
                        .and_then(|s| serde_json::from_str::<Vec<String>>(&s).ok())
                        .unwrap_or_default()
                },
                |v| pm(Msg::Recent(v)),
            );
            Task::batch([iced::widget::operation::focus(INPUT_ID), load])
        }
        Msg::Close => {
            *st = State::default();
            Task::none()
        }
        Msg::Recent(v) => {
            st.recent = v;
            Task::none()
        }
        Msg::Mods(m) => {
            st.mods = m;
            Task::none()
        }
        Msg::Query(q) => {
            st.query = q;
            st.token += 1;
            let token = st.token;
            Task::perform(tokio::time::sleep(Duration::from_millis(120)), move |_| pm(Msg::Search(token)))
        }
        Msg::Search(token) => {
            if token != st.token {
                return Task::none();
            }
            let q = st.query.trim().to_string();
            if q.is_empty() {
                st.results = None;
                st.active = 0;
                return Task::none();
            }
            bg(
                &app.backend,
                move |b| search::search_library(q, b.db()).unwrap_or_default(),
                move |r| pm(Msg::Results(token, r)),
            )
        }
        Msg::Results(token, r) => {
            if token == st.token {
                st.results = Some(r);
                st.active = 0;
            }
            iced::widget::operation::scroll_to(LIST_ID, scrollable::AbsoluteOffset { x: None, y: Some(0.0) })
        }
        Msg::Move(d) => {
            let rows = rows(st);
            if rows.is_empty() {
                return Task::none();
            }
            st.active = (st.active as i64 + d as i64).clamp(0, rows.len() as i64 - 1) as usize;
            // scrollIntoView({ block: "nearest" })
            let top = row_top(&rows, st.active);
            let bottom = top + rows[st.active].height();
            let view_h = if st.viewport > 0.0 { st.viewport } else { 400.0 };
            let target = if top - GROUP_H < st.scroll {
                Some((top - GROUP_H).max(0.0))
            } else if bottom > st.scroll + view_h {
                Some(bottom - view_h + 4.0)
            } else {
                None
            };
            match target {
                Some(y) => iced::widget::operation::scroll_to(LIST_ID, scrollable::AbsoluteOffset { x: None, y: Some(y) }),
                None => Task::none(),
            }
        }
        Msg::Hover(i) => {
            st.active = i;
            Task::none()
        }
        Msg::Scrolled(y, h) => {
            st.scroll = y;
            st.viewport = h;
            Task::none()
        }
        Msg::Submit => {
            let action = if st.mods.command() || st.mods.control() {
                Action::Queue
            } else if st.mods.shift() {
                Action::Play
            } else {
                Action::Default
            };
            let i = st.active;
            update(app, Msg::Select(i, action))
        }
        Msg::Select(i, action) => {
            let rows = rows(st);
            let Some(row) = rows.get(i).cloned() else { return Task::none() };
            if let Kind::Recent(q) = &row.kind {
                let q = q.clone();
                return update(app, Msg::Query(q));
            }
            // Remember what was searched for.
            let query = st.query.trim().to_string();
            let save = if query.is_empty() {
                Task::none()
            } else {
                let json = serde_json::to_string(&push_recent(&st.recent, &query)).unwrap_or_default();
                bg(&app.backend, move |b| prefs::set_pref(RECENT_PREF.into(), json, b.db()), |_| Message::Noop)
            };
            *st = State::default();
            Task::batch([save, run(app, row.kind, action)])
        }
    }
}

/// Carry out `action` on a picked row.
fn run(app: &mut Halo, kind: Kind, action: Action) -> Task<Message> {
    let play_all = |app: &Halo, q: TracksQuery| {
        fire(&app.backend, move |b| {
            let ids: Vec<i64> = library::get_tracks(q, b.db())?.iter().map(|t| t.id).collect();
            if ids.is_empty() {
                return Ok(());
            }
            player::set_queue_and_play(b, ids, 0)
        })
    };
    match (kind, action) {
        (Kind::Track(id), Action::Queue) => fire(&app.backend, move |b| player::add_to_queue(b, id)),
        (Kind::Track(id), _) => fire(&app.backend, move |b| player::play_now(b, id)),
        (Kind::Album(name), Action::Play) => play_all(
            app,
            TracksQuery {
                album: Some(name),
                sort: Some("track_number".into()),
                direction: Some("asc".into()),
                ..TracksQuery::default()
            },
        ),
        (Kind::Album(name), _) => app.update(Message::Navigate(album_route(&name))),
        (Kind::Artist(name), Action::Play) => play_all(app, TracksQuery { artist: Some(name), ..TracksQuery::default() }),
        (Kind::Artist(name), _) => app.update(Message::Navigate(Route::Songs(TracksQuery {
            artist: Some(name),
            ..TracksQuery::default()
        }))),
        (Kind::Playlist(id), Action::Play) => fire(&app.backend, move |b| {
            let (_, tracks) = playlists::get_playlist(id, b.db())?;
            let ids: Vec<i64> = tracks.iter().map(|t| t.track_id).collect();
            if ids.is_empty() {
                return Ok(());
            }
            player::set_queue_and_play(b, ids, 0)
        }),
        (Kind::Playlist(id), _) => app.update(Message::Navigate(Route::Playlist(id))),
        (Kind::Recent(_), _) => Task::none(),
    }
}

// ── View ─────────────────────────────────────────────────────────────────────

pub fn view(app: &Halo) -> Element<'_, Message> {
    let k = t();
    let st = &app.palette;
    let rows = rows(st);

    let input_row = row![
        icon(Icon::Search, 16.0).color(k.muted_fg),
        text_input("Search songs, albums, artists, playlists…", &st.query)
            .id(INPUT_ID)
            .on_input(|q| pm(Msg::Query(q)))
            .on_submit(pm(Msg::Submit))
            .size(theme::BODY)
            .padding(Padding::from([8.0, 0.0]))
            .style(theme::input_bare)
            .width(Length::Fill),
        kbd("Esc"),
    ]
    .spacing(10)
    .align_y(Alignment::Center);
    let input_row = container(input_row).padding(Padding::from([8.0, 16.0]));

    let footer = footer(rows.get(st.active));
    let note = |s: String| -> Element<'_, Message> {
        container(ui::muted(s, theme::BODY))
            .width(Length::Fill)
            .padding(Padding::from([24.0, 16.0]))
            .align_x(Alignment::Center)
            .into()
    };
    let body: Element<'_, Message> = if st.results.is_none() && rows.is_empty() {
        note("Search your library. Several words narrow it down: an artist and an album, say.".into())
    } else if rows.is_empty() {
        note(format!("No results for \"{}\".", st.query.trim()))
    } else {
        let mut col = column![].padding(Padding::from([4.0, 8.0]));
        let mut last: Option<&str> = None;
        for (i, r) in rows.into_iter().enumerate() {
            if last != Some(r.group) {
                last = Some(r.group);
                col = col.push(
                    container(text(r.group).size(theme::SMALL).font(theme::MEDIUM).color(k.muted_fg))
                        .height(GROUP_H)
                        .padding(Padding::from([8.0, 8.0]).bottom(4.0)),
                );
            }
            let active = i == st.active;
            col = col.push(result_row(r, i, active));
        }
        scrollable(col)
            .id(LIST_ID)
            .on_scroll(|vp| pm(Msg::Scrolled(vp.absolute_offset().y, vp.bounds().height)))
            .direction(theme::vscroll())
            .style(theme::scroll)
            .into()
    };

    let max_h = (app.main_size.height * 0.6).max(160.0);
    let panel = container(column![input_row, ui::hline(), container(body).max_height(max_h), ui::hline(), footer])
        .width(640.0_f32.min(app.main_size.width * 0.9))
        .clip(true)
        .style(|th| {
            let mut s = theme::popover(th);
            s.border.radius = theme::radius(theme::R_LG);
            s
        });

    let backdrop = mouse_area(container(Space::new()).width(Length::Fill).height(Length::Fill).style(theme::backdrop(0.4)))
        .on_press(pm(Msg::Close));
    let top = app.main_size.height * 0.10;
    stack![
        opaque(backdrop),
        container(opaque(panel)).width(Length::Fill).align_x(Alignment::Center).padding(Padding::ZERO.top(top)),
    ]
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn result_row<'a>(r: Row, i: usize, active: bool) -> Element<'a, Message> {
    let k = t();
    let side = if r.top { 56.0 } else { 32.0 };
    let lead: Element<'_, Message> = match &r.kind {
        Kind::Track(_) | Kind::Album(_) => art(r.art.as_deref(), side, theme::R_SM),
        Kind::Artist(_) => art(r.art.as_deref(), side, theme::R_FULL),
        Kind::Playlist(_) => box_icon(Icon::ListMusic, side),
        Kind::Recent(_) => box_icon(Icon::History, side),
    };
    let kind_label = match r.kind {
        Kind::Track(_) => "Song",
        Kind::Album(_) => "Album",
        Kind::Artist(_) => "Artist",
        Kind::Playlist(_) => "Playlist",
        Kind::Recent(_) => "",
    };
    let sub = match (r.top, r.sub.is_empty()) {
        (true, true) => kind_label.to_string(),
        (true, false) => format!("{kind_label} · {}", r.sub),
        (false, _) => r.sub.clone(),
    };
    let height = r.height();
    let mut lines = column![line(r.title)
        .size(if r.top { theme::TITLE } else { theme::BODY })
        .font(if r.top { theme::SEMIBOLD } else { theme::SANS })
        .width(Length::Fill)]
    .spacing(2)
    .width(Length::Fill);
    if !sub.is_empty() {
        lines = lines.push(line(sub).size(theme::SMALL).color(k.muted_fg).width(Length::Fill));
    }
    let content = row![lead, lines].spacing(12).align_y(Alignment::Center);

    // ▶ (and ⊕ for songs) on hover.
    let mini = |ic: Icon, label: &'static str, action: Action| {
        tip(
            button(icon_box(ic, 14.0, 28.0, 28.0))
                .padding(0)
                .style(theme::btn_mode(false))
                .on_press(pm(Msg::Select(i, action))),
            label,
            Position::Top,
        )
    };
    let mut tools = row![ui::fill()].spacing(4).align_y(Alignment::Center);
    match r.kind {
        Kind::Track(_) => {
            tools = tools.push(mini(Icon::Play, "Play now", Action::Play));
            tools = tools.push(mini(Icon::ListPlus, "Add to queue", Action::Queue));
        }
        Kind::Album(_) | Kind::Artist(_) | Kind::Playlist(_) => {
            tools = tools.push(mini(Icon::Play, "Play all", Action::Play));
        }
        Kind::Recent(_) => {}
    }
    let tools = container(tools).height(Length::Fill).align_y(Alignment::Center);

    surface(hover(content, tools))
        .width(Length::Fill)
        .height(height)
        .padding(Padding::from([6.0, 8.0]))
        .on_press(pm(Msg::Select(i, Action::Default)))
        .on_enter(pm(Msg::Hover(i)))
        .style(move |s| iced::widget::container::Style {
            background: if active {
                Some(a(t().muted, 0.6).into())
            } else if s.hovered {
                Some(a(t().muted, 0.3).into())
            } else {
                None
            },
            border: theme::no_border(theme::R_MD),
            ..Default::default()
        })
        .into()
}

/// Key hints for the selected row.
fn footer<'a>(active: Option<&Row>) -> Element<'a, Message> {
    let hint = |k: &'a str, label: &'a str| row![kbd(k), ui::muted(label, theme::SMALL)].spacing(6).align_y(Alignment::Center);
    let mut r = row![hint("↑↓", "move")].spacing(16).align_y(Alignment::Center);
    match active.map(|r| &r.kind) {
        Some(Kind::Track(_)) => {
            r = r.push(hint("↵", "play now")).push(hint("Ctrl ↵", "add to queue"));
        }
        Some(Kind::Album(_) | Kind::Artist(_) | Kind::Playlist(_)) => {
            r = r.push(hint("↵", "open")).push(hint("⇧ ↵", "play all"));
        }
        Some(Kind::Recent(_)) => {
            r = r.push(hint("↵", "search again"));
        }
        None => {}
    }
    container(r).padding(Padding::from([8.0, 16.0])).width(Length::Fill).into()
}

fn box_icon<'a>(i: Icon, side: f32) -> Element<'a, Message> {
    center(icon(i, (side * 0.45).max(14.0)).color(a(t().muted_fg, 0.6)))
        .width(side)
        .height(side)
        .style(|_| iced::widget::container::Style {
            background: Some(t().muted.into()),
            border: theme::no_border(theme::R_SM),
            ..Default::default()
        })
        .into()
}

#[cfg(test)]
mod tests {
    use super::push_recent;

    #[test]
    fn recent_searches_dedupe_and_cap() {
        let v = push_recent(&[], " roja ");
        assert_eq!(v, ["roja"]);
        let v = push_recent(&v, "Ilaiyaraaja");
        assert_eq!(v, ["Ilaiyaraaja", "roja"]);
        // Same query again (any case) moves to the front, not duplicated.
        let v = push_recent(&v, "ROJA");
        assert_eq!(v, ["ROJA", "Ilaiyaraaja"]);
        let many: Vec<String> = (0..10).map(|i| format!("q{i}")).collect();
        assert_eq!(push_recent(&many, "new").len(), 6);
    }
}
