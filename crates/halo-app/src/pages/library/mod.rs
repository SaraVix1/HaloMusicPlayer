//! Library: shared collection state + loading, the tabbed Library page
//! (`library.tsx`), and one module per tab (each also backs its standalone
//! route such as `/artists`).

pub mod album_artists;
pub mod artists;
pub mod cards;
pub mod composers;
pub mod genres;
pub mod years;

use halo_backend::commands::library::{
    self, Album, AlbumArtist, Artist, Composer, Genre, YearStat,
};
use iced::widget::{button, column, container, row, stack, text, Space};
use iced::{Element, Length, Padding, Task};

use super::common::{self, note, GridSpec, ScrollMemory};
use super::Msg;
use crate::app::{bg, Halo, Message};
use crate::route::{LibraryTab, Route};
use crate::theme::{self, t};
use crate::ui;

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
    /// Last unfiltered Songs query (keeps the Songs tab's sort order).
    pub songs_query: halo_backend::commands::library::TracksQuery,
}

/// Where a Library tab lives: Songs, Albums and Folders have their own
/// routes, the rest are `Route::Library(Some(tab))`.
pub fn tab_route(app: &Halo, tab: LibraryTab) -> Route {
    match tab {
        LibraryTab::Songs => Route::Songs(app.pages.library.songs_query.clone()),
        LibraryTab::Albums => Route::Albums,
        LibraryTab::Folders => Route::Folders,
        _ => Route::Library(Some(tab)),
    }
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

impl Kind {
    /// The collection a Library tab shows (Songs and Folders have their own
    /// loaders).
    pub fn of(tab: LibraryTab) -> Option<Kind> {
        match tab {
            LibraryTab::Albums => Some(Kind::Albums),
            LibraryTab::Artists => Some(Kind::Artists),
            LibraryTab::AlbumArtists => Some(Kind::AlbumArtists),
            LibraryTab::Composers => Some(Kind::Composers),
            LibraryTab::Genres => Some(Kind::Genres),
            LibraryTab::Years => Some(Kind::Years),
            LibraryTab::Songs | LibraryTab::Folders => None,
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

/// Loading / error / empty states, else the virtualised grid of `c`.
pub fn coll_grid<'a, T>(
    c: &'a Coll<T>,
    empty: &'a str,
    mut spec: GridSpec,
    key: String,
    memory: &'a ScrollMemory,
    padding_top: Option<f32>,
    card: impl Fn(&'a T, f32) -> Element<'a, Message> + 'a,
) -> Element<'a, Message> {
    let pad = |el: Element<'a, Message>| -> Element<'a, Message> {
        container(el)
            .padding(Padding::from([0.0, 24.0]).top(if padding_top.is_some() { 16.0 } else { 8.0 }))
            .into()
    };
    if c.loading {
        return pad(note("Loading…"));
    }
    if let Some(e) = &c.error {
        return pad(common::error_text(e));
    }
    if c.items.is_empty() {
        return pad(note(empty));
    }
    if let Some(p) = padding_top {
        spec.padding_top = p;
    }
    common::vgrid(key, memory, c.items.len(), spec, move |i, w| card(&c.items[i], w))
}

/// Standalone collection page: `h1` + count header, then `grid`.
pub fn standalone<'a>(
    _app: &'a Halo,
    title: &'a str,
    n: usize,
    one: &'a str,
    many: &'a str,
    grid: Element<'a, Message>,
) -> Element<'a, Message> {
    let header = container(common::header(title, common::count(n, one, many)))
        .padding(Padding::from([24.0, 24.0]).bottom(16.0));
    column![header, grid].height(Length::Fill).into()
}

/// Tabbed Library page for the collection tabs.
pub fn view_library(app: &Halo, tab: LibraryTab) -> Element<'_, Message> {
    tabbed(app, tab, tab_grid(app, tab, format!("library:{tab:?}")))
}

/// Library page: title and tab bar, then `body` (the current tab's content).
pub fn tabbed<'a>(app: &'a Halo, tab: LibraryTab, body: Element<'a, Message>) -> Element<'a, Message> {
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
                .on_press(Message::ReplaceRoute(tab_route(app, tb))),
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
    column![header, body].height(Length::Fill).into()
}

fn tab_grid(app: &Halo, tab: LibraryTab, key: String) -> Element<'_, Message> {
    match tab {
        LibraryTab::Artists => artists::grid(app, key, None),
        LibraryTab::AlbumArtists => album_artists::grid(app, key, None),
        LibraryTab::Composers => composers::grid(app, key, None),
        LibraryTab::Genres => genres::grid(app, key, None),
        LibraryTab::Years => years::grid(app, key, None),
        LibraryTab::Albums => super::albums::grid(app, key, None),
        // Own routes, drawn through `tabbed` by `pages::view`.
        LibraryTab::Songs | LibraryTab::Folders => Space::new().into(),
    }
}
