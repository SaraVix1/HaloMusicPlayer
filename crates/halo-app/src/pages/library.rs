//! Albums grid (`albums.tsx`), the tabbed Library page (`library.tsx`) and the
//! standalone Artists / Album Artists / Composers / Genres / Years pages,
//! which reuse the same grid cards.

use halo_backend::commands::library::{
    self, Album, AlbumArtist, Artist, Composer, Genre, TracksQuery, YearStat,
};
use iced::widget::{button, center, column, container, row, stack, text, Space};
use iced::{gradient, Alignment, Background, Element, Length, Padding, Radians, Task};

use super::common::{self, note, GridSpec};
use super::Msg;
use crate::app::{bg, Halo, Message};
use crate::format::{initials, plural};
use crate::icons::{icon, Icon};
use crate::route::{LibraryTab, Route};
use crate::theme::{self, a, t};
use crate::ui::{self, art_sized};
use crate::widgets::{line, surface};

/// One loadable collection.
pub struct Coll<T> {
    pub items: Vec<T>,
    pub loading: bool,
    pub loaded: bool,
    pub error: Option<String>,
}

impl<T> Default for Coll<T> {
    fn default() -> Self {
        Self {
            items: vec![],
            loading: false,
            loaded: false,
            error: None,
        }
    }
}

impl<T> Coll<T> {
    pub fn set(&mut self, r: Result<Vec<T>, String>) {
        self.loading = false;
        self.loaded = true;
        match r {
            Ok(v) => {
                self.items = v;
                self.error = None;
            }
            Err(e) => self.error = Some(e),
        }
    }
}

#[derive(Default)]
pub struct LibraryState {
    pub albums: Coll<Album>,
    pub artists: Coll<Artist>,
    pub album_artists: Coll<AlbumArtist>,
    pub composers: Coll<Composer>,
    pub genres: Coll<Genre>,
    pub years: Coll<YearStat>,
    /// Last visited Library tab (tab memory).
    pub last_tab: LibraryTab,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Albums,
    Artists,
    AlbumArtists,
    Composers,
    Genres,
    Years,
}

impl From<LibraryTab> for Kind {
    fn from(t: LibraryTab) -> Self {
        match t {
            LibraryTab::Artists => Kind::Artists,
            LibraryTab::AlbumArtists => Kind::AlbumArtists,
            LibraryTab::Composers => Kind::Composers,
            LibraryTab::Genres => Kind::Genres,
            LibraryTab::Years => Kind::Years,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Loaded {
    Albums(Result<Vec<Album>, String>),
    Artists(Result<Vec<Artist>, String>),
    AlbumArtists(Result<Vec<AlbumArtist>, String>),
    Composers(Result<Vec<Composer>, String>),
    Genres(Result<Vec<Genre>, String>),
    Years(Result<Vec<YearStat>, String>),
}

pub fn load(app: &mut Halo, kind: Kind) -> Task<Message> {
    let st = &mut app.pages.library;
    macro_rules! go {
        ($field:ident, $f:path, $v:ident) => {{
            st.$field.loading = !st.$field.loaded;
            bg(&app.backend, |b| $f(b.db()), |r| {
                Message::Page(Msg::LibraryLoaded(Loaded::$v(r)))
            })
        }};
    }
    match kind {
        Kind::Albums => go!(albums, library::get_albums, Albums),
        Kind::Artists => go!(artists, library::get_artists, Artists),
        Kind::AlbumArtists => go!(album_artists, library::get_album_artists, AlbumArtists),
        Kind::Composers => go!(composers, library::get_composers, Composers),
        Kind::Genres => go!(genres, library::get_genres, Genres),
        Kind::Years => go!(years, library::get_years, Years),
    }
}

pub fn on_loaded(app: &mut Halo, l: Loaded) {
    let st = &mut app.pages.library;
    match l {
        Loaded::Albums(r) => st.albums.set(r),
        Loaded::Artists(r) => st.artists.set(r),
        Loaded::AlbumArtists(r) => st.album_artists.set(r),
        Loaded::Composers(r) => st.composers.set(r),
        Loaded::Genres(r) => st.genres.set(r),
        Loaded::Years(r) => st.years.set(r),
    }
}

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

// ── Cards ─────────────────────────────────────────────────────────────────────


fn album_card(al: &Album, side: f32) -> Element<'_, Message> {
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
        theme::R_MD,
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
            button(line(al.name.clone()).size(theme::TEXT_SM).font(theme::MEDIUM))
                .padding(0)
                .style(theme::btn_text(k.foreground, k.foreground))
                .on_press(Message::Navigate(album_route(&name))),
            line(sub).size(theme::TEXT_XS).color(k.muted_fg).width(Length::Fill),
        ]
        .spacing(0),
    ]
    .spacing(8)
    .into()
}

fn avatar<'a>(name: &str, art_path: Option<&str>) -> Element<'a, Message> {
    let k = t();
    match art_path.filter(|p| !p.is_empty() && std::path::Path::new(p).is_file()) {
        Some(p) => iced::widget::image(iced::widget::image::Handle::from_path(p))
            .width(Length::Fill)
            .height(Length::Fill)
            .content_fit(iced::ContentFit::Cover)
            .border_radius(theme::R_FULL)
            .into(),
        None => {
            let ini = initials(name);
            center(
                text(if ini.is_empty() { "?".to_string() } else { ini })
                    .size(theme::TEXT_3XL)
                    .font(theme::MEDIUM)
                    .color(k.muted_fg),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .style(|_| iced::widget::container::Style {
                background: Some(t().muted.into()),
                border: theme::no_border(theme::R_FULL),
                ..Default::default()
            })
            .into()
        }
    }
}

fn person_card<'a>(
    name: &'a str,
    art_path: Option<&'a str>,
    sub: String,
    route: Route,
    side: f32,
) -> Element<'a, Message> {
    let k = t();
    let content = column![
        container(avatar(name, art_path)).width(side).height(side),
        column![
            container(line(name).size(theme::TEXT_SM).font(theme::MEDIUM))
                .width(Length::Fill)
                .align_x(Alignment::Center),
            container(text(sub).size(theme::TEXT_XS).color(k.muted_fg))
                .width(Length::Fill)
                .align_x(Alignment::Center),
        ],
    ]
    .spacing(8)
    .align_x(Alignment::Center);
    surface(content)
        .width(Length::Fill)
        .on_press(Message::Navigate(route))
        .interaction(iced::mouse::Interaction::Pointer)
        .style(|s| iced::widget::container::Style {
            background: s.hovered.then(|| a(t().background, 0.1).into()),
            text_color: Some(t().foreground),
            ..Default::default()
        })
        .into()
}

fn gradient_card<'a>(name: &'a str, count: i64, route: Route) -> Element<'a, Message> {
    let (from, to) = theme::gradient_for(name);
    let content = column![
        line(name).size(theme::TEXT_BASE).font(theme::SEMIBOLD).width(Length::Fill),
        Space::new().height(Length::Fill),
        text(plural(count, "track", "tracks"))
            .size(theme::TEXT_XS)
            .color(t().muted_fg),
    ]
    .height(Length::Fill);
    surface(content)
        .width(Length::Fill)
        .height(96)
        .padding(16)
        .on_press(Message::Navigate(route))
        .interaction(iced::mouse::Interaction::Pointer)
        .style(move |s| {
            let k = t();
            let g = gradient::Linear::new(Radians(std::f32::consts::FRAC_PI_4 * 3.0))
                .add_stop(0.0, a(from, 0.3))
                .add_stop(1.0, a(to, 0.2));
            iced::widget::container::Style {
                background: Some(Background::Gradient(g.into())),
                border: theme::border(
                    if s.hovered { a(k.foreground, 0.2) } else { k.border },
                    1.0,
                    theme::R_LG,
                ),
                text_color: Some(k.foreground),
                ..Default::default()
            }
        })
        .into()
}

fn year_card<'a>(y: &YearStat) -> Element<'a, Message> {
    let year = y.year;
    let content = column![
        text(year.to_string()).size(theme::TEXT_2XL).font(theme::SEMIBOLD),
        Space::new().height(Length::Fill),
        text(plural(y.track_count, "track", "tracks"))
            .size(theme::TEXT_XS)
            .color(t().muted_fg),
    ]
    .height(Length::Fill);
    surface(content)
        .width(Length::Fill)
        .height(96)
        .padding(16)
        .on_press(Message::Navigate(Route::Songs(TracksQuery {
            year: Some(year),
            ..TracksQuery::default()
        })))
        .interaction(iced::mouse::Interaction::Pointer)
        .style(|s| {
            let k = t();
            iced::widget::container::Style {
                background: Some(a(k.muted, if s.hovered { 0.4 } else { 0.2 }).into()),
                border: theme::border(
                    if s.hovered { a(k.foreground, 0.2) } else { k.border },
                    1.0,
                    theme::R_LG,
                ),
                text_color: Some(k.foreground),
                ..Default::default()
            }
        })
        .into()
}

const PEOPLE: GridSpec = GridSpec {
    min_item_width: 140.0,
    row_height: 210.0,
    gap: 20.0,
    px: 24.0,
    padding_top: 8.0,
    padding_bottom: 24.0,
};
const TILES: GridSpec = GridSpec {
    min_item_width: 180.0,
    row_height: 108.0,
    gap: 12.0,
    px: 24.0,
    padding_top: 8.0,
    padding_bottom: 24.0,
};
const YEARS: GridSpec = GridSpec {
    min_item_width: 160.0,
    ..TILES
};
const ALBUMS: GridSpec = GridSpec {
    min_item_width: 160.0,
    row_height: 240.0,
    gap: 20.0,
    px: 24.0,
    padding_top: 0.0,
    padding_bottom: 24.0,
};

/// The grid (or loading / empty / error state) for one collection.
fn grid<'a>(app: &'a Halo, kind: Kind, key: String, padding_top: Option<f32>) -> Element<'a, Message> {
    let st = &app.pages.library;
    let mem = &app.pages.scroll;
    let pad = |el: Element<'a, Message>| -> Element<'a, Message> {
        container(el)
            .padding(Padding::from([0.0, 24.0]).top(if padding_top.is_some() { 16.0 } else { 8.0 }))
            .into()
    };
    macro_rules! coll {
        ($c:expr, $empty:expr, $spec:expr, $card:expr) => {{
            let c = &$c;
            if c.loading {
                return pad(note("Loading…"));
            }
            if let Some(e) = &c.error {
                return pad(common::error_text(e));
            }
            if c.items.is_empty() {
                return pad(note($empty));
            }
            let mut spec = $spec;
            if let Some(p) = padding_top {
                spec.padding_top = p;
            }
            common::vgrid(key, mem, c.items.len(), spec, move |i, w| ($card)(&c.items[i], w))
        }};
    }
    match kind {
        Kind::Albums => coll!(
            st.albums,
            "No albums yet — scan a folder to populate the library.",
            ALBUMS,
            |al: &'a Album, w: f32| album_card(al, w.min(196.0))
        ),
        Kind::Artists => coll!(
            st.artists,
            "No artists yet — scan a folder to get started.",
            PEOPLE,
            |a: &'a Artist, w: f32| person_card(
                &a.name,
                a.album_art_path.as_deref(),
                plural(a.track_count, "track", "tracks"),
                Route::Songs(TracksQuery {
                    artist: Some(a.name.clone()),
                    ..TracksQuery::default()
                }),
                w.min(160.0),
            )
        ),
        Kind::AlbumArtists => coll!(
            st.album_artists,
            "No album artists yet — scan a folder to get started.",
            PEOPLE,
            |a: &'a AlbumArtist, w: f32| person_card(
                &a.name,
                a.album_art_path.as_deref(),
                format!(
                    "{} · {}",
                    plural(a.album_count, "album", "albums"),
                    plural(a.track_count, "track", "tracks")
                ),
                Route::Songs(TracksQuery {
                    album_artist: Some(a.name.clone()),
                    ..TracksQuery::default()
                }),
                w.min(160.0),
            )
        ),
        Kind::Composers => coll!(
            st.composers,
            "No composers yet — scan a folder to get started.",
            TILES,
            |c: &'a Composer, _w: f32| gradient_card(
                &c.name,
                c.track_count,
                Route::Songs(TracksQuery {
                    composer: Some(c.name.clone()),
                    ..TracksQuery::default()
                }),
            )
        ),
        Kind::Genres => coll!(
            st.genres,
            "No genres yet — scan a folder to get started.",
            TILES,
            |g: &'a Genre, _w: f32| gradient_card(
                &g.name,
                g.track_count,
                Route::Songs(TracksQuery {
                    genre: Some(g.name.clone()),
                    ..TracksQuery::default()
                }),
            )
        ),
        Kind::Years => coll!(
            st.years,
            "No year tags found — scan a folder to get started.",
            YEARS,
            |y: &'a YearStat, _w: f32| year_card(y)
        ),
    }
}

fn len(app: &Halo, kind: Kind) -> usize {
    let st = &app.pages.library;
    match kind {
        Kind::Albums => st.albums.items.len(),
        Kind::Artists => st.artists.items.len(),
        Kind::AlbumArtists => st.album_artists.items.len(),
        Kind::Composers => st.composers.items.len(),
        Kind::Genres => st.genres.items.len(),
        Kind::Years => st.years.items.len(),
    }
}

/// Albums page and the standalone list pages: header with a count, then
/// the grid.
pub fn view_standalone(app: &Halo, kind: Kind) -> Element<'_, Message> {
    let (title, one, many) = match kind {
        Kind::Albums => ("Albums", "album", "albums"),
        Kind::Artists => ("Artists", "artist", "artists"),
        Kind::AlbumArtists => ("Album Artists", "album artist", "album artists"),
        Kind::Composers => ("Composers", "composer", "composers"),
        Kind::Genres => ("Genres", "genre", "genres"),
        Kind::Years => ("Years", "year", "years"),
    };
    let header = container(common::header(title, common::count(len(app, kind), one, many)))
        .padding(Padding::from([24.0, 24.0]).bottom(16.0));
    column![header, grid(app, kind, app.route().scroll_key(), Some(0.0))]
        .height(Length::Fill)
        .into()
}

/// Tabbed Library page.
pub fn view_library(app: &Halo, tab: LibraryTab) -> Element<'_, Message> {
    let k = t();
    let mut tabs = row![].spacing(4);
    for tb in LibraryTab::ALL {
        let active = tb == tab;
        let label = container(text(tb.label()).size(theme::TEXT_SM).font(theme::MEDIUM))
            .padding(Padding::from([8.0, 12.0]));
        let underline = container(Space::new())
            .width(Length::Fill)
            .height(2)
            .style(move |_| iced::widget::container::Style {
                background: active.then(|| t().foreground.into()),
                ..Default::default()
            });
        tabs = tabs.push(
            button(column![label, underline].width(Length::Shrink))
                .padding(0)
                .style(move |_, st| iced::widget::button::Style {
                    text_color: if active || matches!(st, iced::widget::button::Status::Hovered) {
                        t().foreground
                    } else {
                        t().muted_fg
                    },
                    ..Default::default()
                })
                .on_press(Message::ReplaceRoute(Route::Library(Some(tb)))),
        );
    }
    let header = column![
        ui::h1("Library"),
        ui::vspace(16.0),
        stack![
            column![Space::new().height(Length::Fill), ui::hline::<Message>()].height(38),
            tabs,
        ]
        .height(38),
    ]
    .padding(Padding::from([24.0, 24.0]).bottom(0.0));
    let _ = k;
    column![
        header,
        grid(app, tab.into(), format!("library:{tab:?}"), None),
    ]
    .height(Length::Fill)
    .into()
}
