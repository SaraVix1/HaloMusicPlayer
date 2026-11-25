//! Library: shared collection state + loading, the tabbed Library page
//! (`library.tsx`), and one module per tab (each also backs its standalone
//! route such as `/artists`).

pub mod album_artists;
pub mod artists;
pub mod cards;
pub mod composers;
pub mod genres;
pub mod sort;
pub mod years;

use halo_backend::commands::library::{
    self, Album, AlbumArtist, Artist, Composer, Genre, YearStat,
};
use std::collections::HashMap;

use iced::widget::{button, column, container, row, stack, text, Space};
use iced::{Alignment, Element, Length, Padding, Task};

use sort::{Sort, SortBy};

use super::common::{self, note, GridSpec, ScrollMemory};
use super::Msg;
use crate::app::{bg, Halo, Message};
use crate::route::{LibraryTab, Route};
use crate::icons::{icon, Icon};
use crate::theme::{self, t};
use crate::widgets::{popover, Placement};
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
    /// Chosen sort per tab (missing: the tab's default). Songs' is also in
    /// its route's query.
    pub sorts: HashMap<LibraryTab, Sort>,
    /// Open dropdown in the tab bar (or a song list's Sort button).
    pub menu: Option<Menu>,
    /// Songs tab: list rows or cards (`library.songs_view` pref).
    pub songs_view: SongsView,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SongsView {
    List,
    #[default]
    Cards,
}

const SONGS_VIEW_PREF: &str = "library.songs_view";

impl SongsView {
    /// From the saved pref; anything but "list" (including nothing saved
    /// yet) is Cards, the default.
    pub fn from_pref(v: Option<&str>) -> Self {
        if v == Some("list") { SongsView::List } else { SongsView::Cards }
    }
    pub fn pref(self) -> &'static str {
        match self {
            SongsView::List => "list",
            SongsView::Cards => "cards",
        }
    }
}

impl LibraryState {
    /// Saved sorts; the Songs tab starts in its saved order.
    pub fn new(backend: &halo_backend::Backend) -> Self {
        let sorts = sort::load(backend);
        let mut songs_query = halo_backend::commands::library::TracksQuery::default();
        if let Some(s) = sorts.get(&LibraryTab::Songs) {
            songs_query = sort::apply_to_query(&songs_query, *s);
        }
        let songs_view = SongsView::from_pref(
            halo_backend::commands::ui::get_pref(SONGS_VIEW_PREF.into(), backend.db()).ok().flatten().as_deref(),
        );
        LibraryState { sorts, songs_query, songs_view, ..Default::default() }
    }

    pub fn sort_of(&self, tab: LibraryTab) -> Sort {
        self.sorts.get(&tab).copied().unwrap_or_else(|| sort::default_for(tab))
    }

    /// Re-sort a loaded collection after a load or a sort change.
    fn resort(&mut self, tab: LibraryTab) {
        let s = self.sort_of(tab);
        match tab {
            LibraryTab::Albums => sort::albums(&mut self.albums.items, s),
            LibraryTab::Artists => sort::named(&mut self.artists.items, s),
            LibraryTab::AlbumArtists => sort::named(&mut self.album_artists.items, s),
            LibraryTab::Composers => sort::named(&mut self.composers.items, s),
            LibraryTab::Genres => sort::named(&mut self.genres.items, s),
            LibraryTab::Years => sort::years(&mut self.years.items, s),
            LibraryTab::Songs | LibraryTab::Folders => {}
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Menu {
    More,
    Sort,
}

/// Tabs shown in the bar; the rest live under "More ▾".
const MAIN_TABS: [LibraryTab; 4] = [LibraryTab::Songs, LibraryTab::Albums, LibraryTab::Artists, LibraryTab::Genres];
const MORE_TABS: [LibraryTab; 4] =
    [LibraryTab::AlbumArtists, LibraryTab::Composers, LibraryTab::Years, LibraryTab::Folders];

#[derive(Debug, Clone)]
pub enum LMsg {
    ToggleMenu(Menu),
    CloseMenu,
    OpenTab(LibraryTab),
    SetSort(SortBy),
    SongsView(SongsView),
}

pub fn update(app: &mut Halo, msg: LMsg) -> Task<Message> {
    let st = &mut app.pages.library;
    match msg {
        LMsg::ToggleMenu(m) => {
            st.menu = if st.menu == Some(m) { None } else { Some(m) };
            Task::none()
        }
        LMsg::CloseMenu => {
            st.menu = None;
            Task::none()
        }
        LMsg::OpenTab(tab) => {
            st.menu = None;
            let route = tab_route(app, tab);
            app.update(Message::ReplaceRoute(route))
        }
        LMsg::SetSort(by) => {
            st.menu = None;
            set_sort(app, by)
        }
        LMsg::SongsView(v) => {
            st.songs_view = v;
            let value = v.pref();
            crate::app::fire(&app.backend, move |b| {
                halo_backend::commands::ui::set_pref(SONGS_VIEW_PREF.into(), value.into(), b.db())
            })
        }
    }
}

/// Apply a Sort ▾ pick to what's on screen.
fn set_sort(app: &mut Halo, by: SortBy) -> Task<Message> {
    if let Route::Songs(q) = app.route().clone() {
        let s = sort::of_query(&q).pick(by);
        let next = sort::apply_to_query(&q, s);
        // Only the Songs tab's order is remembered; album / artist lists
        // open in their usual order.
        let save = if crate::route::is_all_songs(&q) {
            app.pages.library.sorts.insert(LibraryTab::Songs, s);
            sort::save(&app.backend, LibraryTab::Songs, s)
        } else {
            Task::none()
        };
        return Task::batch([save, app.update(Message::ReplaceRoute(Route::Songs(next)))]);
    }
    let Some(tab) = app.route().library_tab().or_else(|| standalone_tab(app.route())) else {
        return Task::none();
    };
    let st = &mut app.pages.library;
    let s = st.sort_of(tab).pick(by);
    st.sorts.insert(tab, s);
    st.resort(tab);
    sort::save(&app.backend, tab, s)
}

/// Standalone collection routes (`/artists` …) share their tab's sort.
fn standalone_tab(route: &Route) -> Option<LibraryTab> {
    match route {
        Route::Artists => Some(LibraryTab::Artists),
        Route::AlbumArtists => Some(LibraryTab::AlbumArtists),
        Route::Composers => Some(LibraryTab::Composers),
        Route::Genres => Some(LibraryTab::Genres),
        Route::Years => Some(LibraryTab::Years),
        _ => None,
    }
}

/// "Sort: <option> ↑ ▾" button with its dropdown of `options`.
pub fn sort_button<'a>(app: &'a Halo, options: Vec<(SortBy, &'static str)>, cur: Sort) -> Element<'a, Message> {
    let open = app.pages.library.menu == Some(Menu::Sort);
    let lm = |m: LMsg| Message::Page(Msg::Library(m));
    let arrow = if cur.desc { Icon::ArrowDown } else { Icon::ArrowUp };
    let btn = button(
        row![
            ui::muted("Sort:", theme::BODY),
            text(sort::label(&options, cur.by)).size(theme::BODY).font(theme::MEDIUM),
            icon(arrow, 12.0),
            icon(Icon::ChevronDown, 12.0),
        ]
        .spacing(6)
        .align_y(Alignment::Center),
    )
    .padding(Padding::from([6.0, 10.0]))
    .style(theme::btn_ghost)
    .on_press(lm(LMsg::ToggleMenu(Menu::Sort)));
    let menu = open.then(|| {
        let mut col = column![].width(200);
        for (by, label) in options {
            let active = by == cur.by;
            let mut r = row![text(label).size(theme::BODY), ui::fill()].align_y(Alignment::Center);
            if active {
                r = r.push(icon(arrow, 12.0));
            }
            col = col.push(
                button(r)
                    .width(Length::Fill)
                    .padding(Padding::from([6.0, 12.0]))
                    .style(theme::btn_list_item(active, theme::R_SM))
                    .on_press(lm(LMsg::SetSort(by))),
            );
        }
        menu_box(col)
    });
    popover(btn, menu)
        .placement(Placement::BelowEnd)
        .gap(4.0)
        .on_dismiss(lm(LMsg::CloseMenu))
        .into()
}

fn menu_box<'a>(col: iced::widget::Column<'a, Message>) -> Element<'a, Message> {
    container(col)
        .padding(4)
        .style(|th| {
            let mut s = theme::popover(th);
            s.border.radius = theme::radius(theme::R_LG);
            s
        })
        .into()
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
    let tab = match l {
        Loaded::Albums(r) => {
            st.albums.set(r);
            LibraryTab::Albums
        }
        Loaded::Artists(r) => {
            st.artists.set(r);
            LibraryTab::Artists
        }
        Loaded::AlbumArtists(r) => {
            st.album_artists.set(r);
            LibraryTab::AlbumArtists
        }
        Loaded::Composers(r) => {
            st.composers.set(r);
            LibraryTab::Composers
        }
        Loaded::Genres(r) => {
            st.genres.set(r);
            LibraryTab::Genres
        }
        Loaded::Years(r) => {
            st.years.set(r);
            LibraryTab::Years
        }
    };
    st.resort(tab);
}

/// Loading / error / empty states, else the virtualised grid of `c`.
pub fn coll_grid<'a, T>(
    c: &'a Coll<T>,
    empty: Element<'a, Message>,
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
        return pad(empty);
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

/// Library page: title, then the tab bar — Songs | Albums | Artists |
/// Genres | More ▾ on the left, Sort ▾ on the right — then `body`.
pub fn tabbed<'a>(app: &'a Halo, tab: LibraryTab, body: Element<'a, Message>) -> Element<'a, Message> {
    let lm = |m: LMsg| Message::Page(Msg::Library(m));
    let mut tabs = row![].spacing(4).align_y(Alignment::End);
    for tb in MAIN_TABS {
        tabs = tabs.push(tab_button(text(tb.label()).size(theme::BODY).font(theme::MEDIUM).into(), tb == tab, lm(LMsg::OpenTab(tb))));
    }

    // "More ▾": shows the open tab's name when it is one of these.
    let in_more = MORE_TABS.contains(&tab);
    let more_label = row![
        text(if in_more { tab.label() } else { "More" }).size(theme::BODY).font(theme::MEDIUM),
        icon(Icon::ChevronDown, 12.0),
    ]
    .spacing(4)
    .align_y(Alignment::Center);
    let more_menu = (app.pages.library.menu == Some(Menu::More)).then(|| {
        let mut col = column![].width(180);
        for tb in MORE_TABS {
            col = col.push(
                button(text(tb.label()).size(theme::BODY))
                    .width(Length::Fill)
                    .padding(Padding::from([6.0, 12.0]))
                    .style(theme::btn_list_item(tb == tab, theme::R_SM))
                    .on_press(lm(LMsg::OpenTab(tb))),
            );
        }
        menu_box(col)
    });
    tabs = tabs.push(
        popover(tab_button(more_label.into(), in_more, lm(LMsg::ToggleMenu(Menu::More))), more_menu)
            .placement(Placement::BelowStart)
            .gap(4.0)
            .on_dismiss(lm(LMsg::CloseMenu)),
    );

    // Song lists (the Songs tab and filtered lists) sort their songs; the
    // collection tabs sort their cards.
    let opts = match app.route() {
        Route::Songs(q) => Some((sort::song_options(q), sort::of_query(q))),
        _ if !sort::options(tab).is_empty() => Some((sort::options(tab).to_vec(), app.pages.library.sort_of(tab))),
        _ => None,
    };
    // ☰ / ▦ wherever songs are listed (album pages are always rows).
    let show_toggle = match app.route() {
        Route::Songs(q) => q.album.is_none(),
        Route::Folders | Route::Folder(_) => super::folders::shows_songs(app),
        _ => false,
    };
    let mut bar = row![tabs, ui::fill()].spacing(8).align_y(Alignment::Center);
    if show_toggle {
        bar = bar.push(view_toggle(app.pages.library.songs_view));
    }
    if let Some((options, cur)) = opts {
        bar = bar.push(sort_button(app, options, cur));
    }

    let header = column![
        ui::h1("Library"),
        ui::vspace(16.0),
        stack![
            column![Space::new().height(Length::Fill), ui::hline::<Message>()].height(38),
            bar.height(38),
        ]
        .height(38),
    ]
    .padding(Padding::from([24.0, 24.0]).bottom(0.0));
    column![header, body].height(Length::Fill).into()
}

/// Breadcrumb for a filtered list: `Genres › Beat` (the tab links back to
/// its grid).
pub fn crumbs<'a>(app: &Halo, tab: LibraryTab, label: String) -> Element<'a, Message> {
    let k = t();
    row![
        button(text(tab.label()).size(theme::BODY))
            .padding(0)
            .style(theme::btn_text(k.muted_fg, k.foreground))
            .on_press(Message::Navigate(tab_route(app, tab))),
        icon(Icon::ChevronRight, 14.0).color(theme::a(k.muted_fg, 0.7)),
        crate::widgets::line(label).size(theme::TITLE).font(theme::SEMIBOLD),
    ]
    .spacing(6)
    .align_y(Alignment::Center)
    .into()
}

/// ☰ List | ▦ Cards (one shared preference for every song list).
pub(crate) fn view_toggle<'a>(cur: SongsView) -> Element<'a, Message> {
    let seg = |v: SongsView, ic: Icon, label: &'static str| {
        ui::tip(
            button(crate::ui::icon_box(ic, 15.0, 30.0, 28.0))
                .padding(0)
                .style(theme::btn_segment(cur == v))
                .on_press(Message::Page(Msg::Library(LMsg::SongsView(v)))),
            label,
            iced::widget::tooltip::Position::Bottom,
        )
    };
    container(row![seg(SongsView::List, Icon::List, "List"), seg(SongsView::Cards, Icon::LayoutGrid, "Cards")].spacing(2))
        .padding(2)
        .style(theme::bordered(theme::R_MD))
        .into()
}

/// A tab: label with a 2px underline when active.
fn tab_button<'a>(label: Element<'a, Message>, active: bool, msg: Message) -> Element<'a, Message> {
    let underline = container(Space::new())
        .width(Length::Fill)
        .height(2)
        .style(move |_| iced::widget::container::Style {
            background: active.then(|| t().foreground.into()),
            ..Default::default()
        });
    button(column![container(label).padding(Padding::from([8.0, 12.0])), underline].width(Length::Shrink))
        .padding(0)
        .style(move |_, st| iced::widget::button::Style {
            text_color: if active || matches!(st, iced::widget::button::Status::Hovered) {
                t().foreground
            } else {
                t().muted_fg
            },
            ..Default::default()
        })
        .on_press(msg)
        .into()
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

#[cfg(test)]
mod tests {
    use super::SongsView;

    #[test]
    fn songs_view_pref_round_trips_and_defaults_to_cards() {
        for v in [SongsView::List, SongsView::Cards] {
            assert_eq!(SongsView::from_pref(Some(v.pref())), v);
        }
        assert_eq!(SongsView::from_pref(None), SongsView::Cards);
        assert_eq!(SongsView::from_pref(Some("garbage")), SongsView::Cards);
        assert_eq!(SongsView::default(), SongsView::Cards);
    }
}

