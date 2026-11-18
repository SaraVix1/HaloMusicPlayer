//! Ctrl+K command palette (`command-palette.tsx`): debounced (120ms) global
//! search over songs, albums, artists and playlists; ↑/↓/Enter/Esc keyboard
//! navigation; mouse hover selects; click / Enter activates.

use std::time::Duration;

use halo_backend::commands::library::{self, TracksQuery};
use halo_backend::commands::search::{self, SearchResults};
use iced::widget::{
    center, column, container, mouse_area, opaque, row, scrollable, stack, text, text_input,
    Space,
};
use iced::{Alignment, Element, Length, Padding, Task};

use crate::app::{bg, Halo, Message, PlayerCmd};
use crate::format::plural;
use crate::icons::{icon, Icon};
use crate::pages::albums::album_route;
use crate::route::Route;
use crate::theme::{self, a, t};
use crate::ui::{self, art, kbd};
use crate::widgets::{line, surface};

const INPUT_ID: &str = "command-palette-input";
const LIST_ID: &str = "command-palette-list";
const ROW_H: f32 = 48.0;
const GROUP_H: f32 = 26.0;

#[derive(Default)]
pub struct State {
    pub open: bool,
    pub query: String,
    pub results: Option<SearchResults>,
    pub active: usize,
    token: u64,
    scroll: f32,
    viewport: f32,
}

#[derive(Debug, Clone)]
pub enum Msg {
    Open,
    Close,
    Query(String),
    Search(u64),
    Results(u64, SearchResults),
    Move(i32),
    Hover(usize),
    Activate,
    Select(usize),
    Scrolled(f32, f32),
}

#[derive(Debug, Clone)]
enum Row {
    Track { id: i64, title: String, sub: String, art: Option<String> },
    Album { name: String, sub: String, art: Option<String> },
    Artist { name: String, sub: String },
    Playlist { id: i64, name: String, sub: String },
}

impl Row {
    fn group(&self) -> &'static str {
        match self {
            Row::Track { .. } => "Songs",
            Row::Album { .. } => "Albums",
            Row::Artist { .. } => "Artists",
            Row::Playlist { .. } => "Playlists",
        }
    }
}

fn flatten(r: &Option<SearchResults>) -> Vec<Row> {
    let Some(r) = r else { return vec![] };
    let mut out = vec![];
    for t in &r.tracks {
        let sub = [Some(t.artists.join(", ")).filter(|s| !s.is_empty()), t.album_name.clone()]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(" · ");
        out.push(Row::Track {
            id: t.track_id,
            title: t.title.clone(),
            sub,
            art: t.album_art_path.clone(),
        });
    }
    for al in &r.albums {
        out.push(Row::Album {
            name: al.name.clone(),
            sub: plural(al.track_count, "track", "tracks"),
            art: al.album_art_path.clone(),
        });
    }
    for ar in &r.artists {
        out.push(Row::Artist {
            name: ar.name.clone(),
            sub: plural(ar.track_count, "track", "tracks"),
        });
    }
    for p in &r.playlists {
        out.push(Row::Playlist {
            id: p.id,
            name: p.name.clone(),
            sub: plural(p.track_count, "track", "tracks"),
        });
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
        if last != Some(r.group()) {
            y += GROUP_H;
            last = Some(r.group());
        }
        if j == i {
            return y;
        }
        y += ROW_H;
    }
    y
}

pub fn update(app: &mut Halo, msg: Msg) -> Task<Message> {
    let st = &mut app.palette;
    match msg {
        Msg::Open => {
            *st = State {
                open: true,
                ..State::default()
            };
            iced::widget::operation::focus(INPUT_ID)
        }
        Msg::Close => {
            *st = State::default();
            Task::none()
        }
        Msg::Query(q) => {
            st.query = q;
            st.token += 1;
            let token = st.token;
            Task::perform(tokio::time::sleep(Duration::from_millis(120)), move |_| {
                pm(Msg::Search(token))
            })
        }
        Msg::Search(token) => {
            if token != st.token {
                return Task::none();
            }
            let q = st.query.trim().to_string();
            if q.is_empty() {
                st.results = None;
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
            iced::widget::operation::scroll_to(
                LIST_ID,
                scrollable::AbsoluteOffset { x: None, y: Some(0.0) },
            )
        }
        Msg::Move(d) => {
            let rows = flatten(&st.results);
            if rows.is_empty() {
                return Task::none();
            }
            st.active = (st.active as i64 + d as i64).clamp(0, rows.len() as i64 - 1) as usize;
            // scrollIntoView({ block: "nearest" })
            let top = row_top(&rows, st.active);
            let bottom = top + ROW_H;
            let view_h = if st.viewport > 0.0 { st.viewport } else { 400.0 };
            let target = if top - GROUP_H < st.scroll {
                Some((top - GROUP_H).max(0.0))
            } else if bottom > st.scroll + view_h {
                Some(bottom - view_h + 4.0)
            } else {
                None
            };
            match target {
                Some(y) => iced::widget::operation::scroll_to(
                    LIST_ID,
                    scrollable::AbsoluteOffset { x: None, y: Some(y) },
                ),
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
        Msg::Activate => {
            let i = st.active;
            update(app, Msg::Select(i))
        }
        Msg::Select(i) => {
            let rows = flatten(&st.results);
            let Some(row) = rows.get(i).cloned() else { return Task::none() };
            *st = State::default();
            match row {
                Row::Track { id, .. } => app.update(Message::Player(PlayerCmd::PlayTracks(vec![id], 0))),
                Row::Album { name, .. } => {
                    let n2 = name.clone();
                    let play = bg(
                        &app.backend,
                        move |b| {
                            let tracks = library::get_tracks(
                                TracksQuery {
                                    album: Some(n2),
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
                    );
                    Task::batch([play, app.update(Message::Navigate(album_route(&name)))])
                }
                Row::Artist { name, .. } => app.update(Message::Navigate(Route::Songs(TracksQuery {
                    artist: Some(name),
                    ..TracksQuery::default()
                }))),
                Row::Playlist { id, .. } => app.update(Message::Navigate(Route::Playlist(id))),
            }
        }
    }
}

pub fn view(app: &Halo) -> Element<'_, Message> {
    let k = t();
    let st = &app.palette;
    let rows = flatten(&st.results);

    let input_row = row![
        icon(Icon::Search, 16.0).color(k.muted_fg),
        text_input("Search tracks, albums, artists, playlists…", &st.query)
            .id(INPUT_ID)
            .on_input(|q| pm(Msg::Query(q)))
            .on_submit(pm(Msg::Activate))
            .size(theme::TEXT_SM)
            .padding(Padding::from([6.0, 0.0]))
            .style(theme::input_bare)
            .width(Length::Fill),
        kbd("Esc"),
    ]
    .spacing(8)
    .align_y(Alignment::Center);
    let input_row = container(input_row).padding(Padding::from([8.0, 12.0]));

    let body: Element<'_, Message> = if st.results.is_none() {
        container(ui::muted("Start typing to search.", theme::TEXT_SM))
            .width(Length::Fill)
            .padding(Padding::from([24.0, 12.0]))
            .align_x(Alignment::Center)
            .into()
    } else if rows.is_empty() {
        container(ui::muted(format!("No results for \"{}\".", st.query), theme::TEXT_SM))
            .width(Length::Fill)
            .padding(Padding::from([24.0, 12.0]))
            .align_x(Alignment::Center)
            .into()
    } else {
        let mut col = column![].padding(Padding::from([4.0, 0.0]));
        let mut last: Option<&str> = None;
        for (i, r) in rows.into_iter().enumerate() {
            if last != Some(r.group()) {
                last = Some(r.group());
                col = col.push(
                    container(
                        text(r.group().to_uppercase())
                            .size(theme::TEXT_10)
                            .font(theme::MEDIUM)
                            .color(k.muted_fg),
                    )
                    .height(GROUP_H)
                    .padding(Padding::from([8.0, 12.0]).bottom(4.0)),
                );
            }
            let (lead, title, sub): (Element<'_, Message>, String, String) = match r {
                Row::Track { title, sub, art: a_, .. } => (art(a_.as_deref(), 32.0, theme::R_SM), title, sub),
                Row::Album { name, sub, art: a_ } => (art(a_.as_deref(), 32.0, theme::R_SM), name, sub),
                Row::Artist { name, sub } => (box_icon(Icon::Mic2), name, sub),
                Row::Playlist { name, sub, .. } => (box_icon(Icon::ListMusic), name, sub),
            };
            let active = i == st.active;
            let content = row![
                lead,
                column![
                    line(title).size(theme::TEXT_SM).width(Length::Fill),
                    line(if sub.is_empty() { "—".to_string() } else { sub })
                        .size(theme::TEXT_XS)
                        .color(k.muted_fg)
                        .width(Length::Fill),
                ]
                .width(Length::Fill),
            ]
            .spacing(12)
            .align_y(Alignment::Center);
            col = col.push(
                surface(content)
                    .width(Length::Fill)
                    .height(ROW_H)
                    .padding(Padding::from([6.0, 12.0]))
                    .on_press(pm(Msg::Select(i)))
                    .on_enter(pm(Msg::Hover(i)))
                    .style(move |s| iced::widget::container::Style {
                        background: if active {
                            Some(a(t().muted, 0.6).into())
                        } else if s.hovered {
                            Some(a(t().muted, 0.3).into())
                        } else {
                            None
                        },
                        ..Default::default()
                    }),
            );
        }
        scrollable(col)
            .id(LIST_ID)
            .on_scroll(|vp| pm(Msg::Scrolled(vp.absolute_offset().y, vp.bounds().height)))
            .direction(theme::vscroll())
            .style(theme::scroll)
            .into()
    };

    let max_h = (app.main_size.height * 0.6).max(120.0);
    let panel = container(column![input_row, ui::hline(), container(body).max_height(max_h)])
        .width(640.0_f32.min(app.main_size.width * 0.9))
        .clip(true)
        .style(|_| {
            let k = t();
            iced::widget::container::Style {
                background: Some(k.background.into()),
                text_color: Some(k.foreground),
                border: theme::border(k.border, 1.0, theme::R_LG),
                shadow: iced::Shadow {
                    color: a(theme::BLACK, 0.5),
                    offset: iced::Vector::new(0.0, 25.0),
                    blur_radius: 50.0,
                },
                ..Default::default()
            }
        });

    let backdrop = mouse_area(
        container(Space::new())
            .width(Length::Fill)
            .height(Length::Fill)
            .style(theme::backdrop(0.4)),
    )
    .on_press(pm(Msg::Close));
    let top = app.main_size.height * 0.10;
    stack![
        opaque(backdrop),
        container(opaque(panel))
            .width(Length::Fill)
            .align_x(Alignment::Center)
            .padding(Padding::ZERO.top(top)),
    ]
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn box_icon<'a>(i: Icon) -> Element<'a, Message> {
    center(icon(i, 14.0).color(a(t().muted_fg, 0.6)))
        .width(32)
        .height(32)
        .style(|_| iced::widget::container::Style {
            background: Some(t().muted.into()),
            border: theme::no_border(theme::R_SM),
            ..Default::default()
        })
        .into()
}
