//! Routed pages — one file per page / tab:
//!
//! | Route | File |
//! |---|---|
//! | Home | `home.rs` |
//! | All Songs (Library → Songs tab) | `songs.rs` (table: `tracks.rs`) |
//! | Recently Played | `recently_played.rs` |
//! | Most Played | `most_played.rs` |
//! | Albums | `albums.rs` |
//! | Library tabs + standalone pages | `library/{artists,album_artists,composers,genres,years}.rs` |
//! | Folders | `folders.rs` |
//! | Playlists | `playlists.rs` |
//! | Playlist detail | `playlist_detail.rs` |
//! | Smart playlist detail | `smart_playlist.rs` |
//! | Settings tabs | `settings/*.rs` |
//! | Dialogs | `dialogs.rs`, `metadata.rs` |
//!
//! This module holds the shared [`Pages`] state, the page [`Msg`] enum and
//! dispatch by route.

pub mod albums;
pub mod common;
pub mod dialogs;
pub mod folders;
pub mod home;
pub mod library;
pub mod metadata;
pub mod most_played;
pub mod playlist_detail;
pub mod playlists;
pub mod recently_played;
pub mod settings;
pub mod smart_playlist;
pub mod songs;
pub mod tracks;

use halo_backend::commands::eq::EqConfig;
use halo_backend::commands::library::{FolderTrack, Track, TracksQuery};
use halo_backend::scanner::ScanProgress;
use iced::{event, mouse, Element, Subscription, Task};

use crate::app::{fire, Halo, Message};
use crate::route::{LibraryTab, Route};

#[derive(Default)]
pub struct Pages {
    pub scroll: common::ScrollMemory,
    pub home: home::HomeState,
    pub songs: tracks::TrackList,
    pub songs_query: Option<TracksQuery>,
    pub recent: tracks::TrackList,
    pub most: tracks::TrackList,
    pub library: library::LibraryState,
    pub folders: folders::FoldersState,
    pub playlists: playlists::PlaylistsState,
    pub playlist: playlist_detail::DetailState,
    pub smart: smart_playlist::SmartState,
    pub settings: settings::SettingsState,
    pub dialog: Option<dialogs::Dialog>,
}

#[derive(Debug, Clone)]
pub enum Msg {
    Scrolled(String, f32, f32),
    StarHover(Option<(i64, u8)>),

    HomeLoaded(Result<halo_backend::commands::library::HomeShelves, String>),
    SongsLoaded(TracksQuery, Result<Vec<Track>, String>),
    RecentLoaded(Result<Vec<Track>, String>),
    MostLoaded(Result<Vec<Track>, String>),
    Library(library::LMsg),
    PlayAll(Vec<i64>),
    AddToPlaylist(Vec<i64>),
    EditMetadata(i64),

    LibraryLoaded(library::Loaded),
    PlayAlbum(String),

    FoldersLoaded(Result<Vec<FolderTrack>, String>),
    SelectFolder(String),
    FolderPlay(Vec<i64>, i64, bool),


    Playlists(playlists::PMsg),
    PlaylistDetail(playlist_detail::DMsg),
    Smart(smart_playlist::SMsg),
    Settings(settings::Msg),
    Dialog(dialogs::DMsg),
    Meta(metadata::Msg),
}

pub fn update(app: &mut Halo, msg: Msg) -> Task<Message> {
    match msg {
        Msg::Scrolled(key, offset, viewport) => {
            app.pages.scroll.set(key, offset, viewport);
            Task::none()
        }
        Msg::StarHover(h) => {
            app.pages.songs.star_hover = h;
            app.pages.recent.star_hover = h;
            app.pages.most.star_hover = h;
            Task::none()
        }

        Msg::HomeLoaded(r) => {
            home::on_loaded(app, r);
            Task::none()
        }
        Msg::SongsLoaded(q, r) => {
            // Ignore stale responses for a previous filter / sort.
            if let Route::Songs(cur) = app.route() {
                if *cur != q {
                    return Task::none();
                }
            }
            let first = !app.pages.songs.loaded || app.pages.songs_query.as_ref() != Some(&q);
            app.pages.songs_query = Some(q);
            set_list(&mut app.pages.songs, r);
            if first {
                app.pages.scroll.restore(&app.route().scroll_key())
            } else {
                Task::none()
            }
        }
        Msg::RecentLoaded(r) => {
            set_list(&mut app.pages.recent, r);
            Task::none()
        }
        Msg::MostLoaded(r) => {
            set_list(&mut app.pages.most, r);
            Task::none()
        }
        Msg::Library(m) => library::update(app, m),
        Msg::PlayAll(ids) => {
            if ids.is_empty() {
                return Task::none();
            }
            fire(&app.backend, move |b| {
                halo_backend::commands::player::set_shuffle(b.clone(), false)?;
                halo_backend::commands::player::set_queue_and_play(b, ids, 0)
            })
        }
        Msg::AddToPlaylist(ids) => dialogs::open_add_to_playlist(app, ids),
        Msg::EditMetadata(id) => metadata::open(app, id),

        Msg::LibraryLoaded(l) => {
            library::on_loaded(app, l);
            Task::none()
        }
        Msg::PlayAlbum(name) => albums::play_album(app, name),

        Msg::FoldersLoaded(r) => {
            folders::on_loaded(app, r);
            Task::none()
        }
        Msg::SelectFolder(p) => {
            app.pages.folders.selected = Some(p.clone());
            app.pages.scroll.restore(&format!("folders:{p}"))
        }
        Msg::FolderPlay(ids, start, shuffle) => fire(&app.backend, move |b| {
            halo_backend::commands::player::set_shuffle(b.clone(), shuffle)?;
            halo_backend::commands::player::set_queue_and_play(b, ids, start)
        }),


        Msg::Playlists(m) => playlists::update(app, m),
        Msg::PlaylistDetail(m) => playlist_detail::update(app, m),
        Msg::Smart(m) => smart_playlist::update(app, m),
        Msg::Settings(m) => settings::update(app, m),
        Msg::Dialog(m) => dialogs::update(app, m),
        Msg::Meta(m) => metadata::update(app, m),
    }
}

fn set_list(l: &mut tracks::TrackList, r: Result<Vec<Track>, String>) {
    l.loading = false;
    l.loaded = true;
    match r {
        Ok(v) => {
            l.tracks = v;
            l.error = None;
        }
        Err(e) => l.error = Some(e),
    }
}

/// Resolve "remembered tab" routes (`/library`, `/settings` without `?tab=`)
/// and remember the Library tab being shown.
fn normalize_route(app: &mut Halo) {
    match app.route().clone() {
        Route::Library(None) => {
            let route = library::tab_route(app, app.pages.library.last_tab);
            app.history.replace(route);
        }
        // Tabs with their own routes.
        Route::Library(Some(tab @ (LibraryTab::Songs | LibraryTab::Albums | LibraryTab::Folders))) => {
            let route = library::tab_route(app, tab);
            app.history.replace(route);
        }
        Route::Settings(None) => {
            let tab = app.pages.settings.last_tab;
            app.history.replace(Route::Settings(Some(tab)));
        }
        Route::Settings(Some(tab)) => app.pages.settings.last_tab = tab,
        _ => {}
    }
    if let Some(tab) = app.route().library_tab() {
        app.pages.library.last_tab = tab;
    }
    if let Route::Songs(q) = app.route() {
        if crate::route::is_all_songs(q) {
            app.pages.library.songs_query = q.clone();
        }
    }
}

/// Load whatever the current route needs and restore its scroll position.
pub fn on_enter(app: &mut Halo) -> Task<Message> {
    app.pages.library.menu = None;
    normalize_route(app);
    let route = app.route().clone();
    let restore = app.pages.scroll.restore(&scroll_key_for(app, &route));
    let load = match &route {
        Route::Home => home::load(app),
        Route::Songs(q) => songs::load(app, q.clone()),
        Route::RecentlyPlayed => recently_played::load(app),
        Route::MostPlayed => most_played::load(app),
        Route::Albums => library::load(app, library::Kind::Albums),
        Route::Library(Some(tab)) => match library::Kind::of(*tab) {
            Some(kind) => library::load(app, kind),
            None => Task::none(),
        },
        Route::Library(None) => Task::none(),
        Route::Artists => library::load(app, library::Kind::Artists),
        Route::AlbumArtists => library::load(app, library::Kind::AlbumArtists),
        Route::Composers => library::load(app, library::Kind::Composers),
        Route::Genres => library::load(app, library::Kind::Genres),
        Route::Years => library::load(app, library::Kind::Years),
        Route::Folders => load_folders(app),
        Route::Playlists => playlists::load(app),
        Route::Playlist(id) => playlist_detail::load(app, *id),
        Route::SmartPlaylist(id) => smart_playlist::load(app, *id),
        Route::Settings(tab) => settings::on_enter(app, tab.unwrap_or_default()),
    };
    Task::batch([load, restore])
}

fn scroll_key_for(app: &Halo, route: &Route) -> String {
    match route {
        Route::Library(Some(tab)) => format!("library:{tab:?}"),
        Route::Folders => app
            .pages
            .folders
            .selected
            .as_ref()
            .map(|p| format!("folders:{p}"))
            .unwrap_or_else(|| route.scroll_key()),
        _ => route.scroll_key(),
    }
}

fn load_folders(app: &mut Halo) -> Task<Message> {
    app.pages.folders.rows.loading = !app.pages.folders.rows.loaded;
    crate::app::bg(
        &app.backend,
        |b| halo_backend::commands::library::get_folder_tracks(b.db()),
        |r| Message::Page(Msg::FoldersLoaded(r)),
    )
}

pub fn on_navigate(app: &mut Halo, _prev: Route) -> Task<Message> {
    on_enter(app)
}

/// `library-changed`: reload what's on screen.
pub fn on_library_changed(app: &mut Halo) -> Task<Message> {
    match app.route() {
        Route::SmartPlaylist(_) => smart_playlist::reload_tracks(app),
        Route::Settings(_) => Task::none(),
        _ => on_enter(app),
    }
}

/// A playlist was created / renamed / changed: refresh the list next time.
pub fn on_playlists_changed(app: &mut Halo) -> Task<Message> {
    app.pages.playlists.loaded = false;
    if matches!(app.route(), Route::Playlists) {
        playlists::load(app)
    } else {
        Task::none()
    }
}

/// A confirm dialog was accepted.
pub fn on_confirm(app: &mut Halo, action: dialogs::ConfirmAction) -> Task<Message> {
    match action {
        dialogs::ConfirmAction::DeletePlaylist(id) => playlist_detail::delete_now(app, id),
        dialogs::ConfirmAction::ResetStats | dialogs::ConfirmAction::ClearDatabase => {
            settings::on_confirm(app, action)
        }
    }
}


pub fn on_scan_progress(app: &mut Halo, p: ScanProgress) -> Task<Message> {
    settings::on_scan_progress(app, p)
}

pub fn on_eq_changed(app: &mut Halo, cfg: EqConfig) -> Task<Message> {
    settings::on_eq_changed(app, cfg)
}

pub fn on_rating_changed(app: &mut Halo, track_id: i64, rating: i64) -> Task<Message> {
    app.pages.songs.set_rating(track_id, rating);
    app.pages.recent.set_rating(track_id, rating);
    app.pages.most.set_rating(track_id, rating);
    Task::none()
}

/// A dialog (add to playlist, confirm, metadata editor) is open.
pub fn has_dialog(app: &Halo) -> bool {
    app.pages.dialog.is_some()
}

pub fn on_escape(app: &mut Halo) -> Task<Message> {
    if dialogs::on_escape(app) {
        return Task::none();
    }
    if app.pages.library.menu.take().is_some() {
        return Task::none();
    }
    let pl = &mut app.pages.playlists;
    if pl.menu_open || pl.creating.is_some() {
        pl.menu_open = false;
        pl.creating = None;
        return Task::none();
    }
    if app.pages.playlist.editing {
        app.pages.playlist.editing = false;
    }
    if app.now_playing.sleep_open {
        app.now_playing.sleep_open = false;
    }
    Task::none()
}

pub fn on_ctrl_enter(app: &mut Halo) -> Task<Message> {
    metadata::on_ctrl_enter(app)
}

pub fn subscription(app: &Halo) -> Option<Subscription<Message>> {
    let mut subs = vec![];
    if playlist_detail::dragging(app) {
        subs.push(event::listen_with(|e, _, _| match e {
            iced::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => Some(
                Message::Page(Msg::PlaylistDetail(playlist_detail::DMsg::DragEnd)),
            ),
            _ => None,
        }));
    }
    if metadata::busy(app) {
        subs.push(iced::time::every(std::time::Duration::from_millis(16)).map(|_| Message::Noop));
    }
    if let Some(s) = settings::subscription(app) {
        subs.push(s);
    }
    (!subs.is_empty()).then(|| Subscription::batch(subs))
}

pub fn overlay(app: &Halo) -> Option<Element<'_, Message>> {
    dialogs::view(app)
}

pub fn view(app: &Halo) -> Element<'_, Message> {
    match app.route() {
        Route::Home => home::view(app),
        Route::Songs(q) if crate::route::is_all_songs(q) => {
            library::tabbed(app, LibraryTab::Songs, songs::view(app, q))
        }
        Route::Songs(q) => songs::view(app, q),
        Route::RecentlyPlayed => recently_played::view(app),
        Route::MostPlayed => most_played::view(app),
        Route::Albums => library::tabbed(
            app,
            LibraryTab::Albums,
            albums::grid(app, app.route().scroll_key(), None),
        ),
        Route::Library(tab) => library::view_library(app, tab.unwrap_or_default()),
        Route::Artists => library::artists::view(app),
        Route::AlbumArtists => library::album_artists::view(app),
        Route::Composers => library::composers::view(app),
        Route::Genres => library::genres::view(app),
        Route::Years => library::years::view(app),
        Route::Folders => library::tabbed(app, LibraryTab::Folders, folders::view(app)),
        Route::Playlists => playlists::view(app),
        Route::Playlist(_) => playlist_detail::view(app),
        Route::SmartPlaylist(_) => smart_playlist::view(app),
        Route::Settings(tab) => settings::view(app, tab.unwrap_or_default()),
    }
}
