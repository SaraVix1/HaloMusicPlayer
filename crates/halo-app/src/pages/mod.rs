//! Routed pages. Each page module owns its state; this module holds the
//! shared [`Pages`] state, the page [`Msg`] enum and dispatch by route.

pub mod common;
pub mod folders;
pub mod library;
pub mod queue;
pub mod songs;
pub mod tracks;

use halo_backend::commands::eq::EqConfig;
use halo_backend::commands::library::{FolderTrack, Track, TracksQuery};
use halo_backend::commands::player::QueueTrack;
use halo_backend::scanner::ScanProgress;
use iced::widget::{center, text};
use iced::{Element, Subscription, Task};

use crate::app::{fire, Halo, Message, PlayerCmd};
use crate::route::{LibraryTab, Route};

#[derive(Default)]
pub struct Pages {
    pub scroll: common::ScrollMemory,
    pub songs: tracks::TrackList,
    pub songs_query: Option<TracksQuery>,
    pub recent: tracks::TrackList,
    pub most: tracks::TrackList,
    pub library: library::LibraryState,
    pub folders: folders::FoldersState,
    pub queue: queue::QueueState,
}

#[derive(Debug, Clone)]
pub enum Msg {
    Scrolled(String, f32, f32),
    StarHover(Option<(i64, u8)>),

    SongsLoaded(TracksQuery, Result<Vec<Track>, String>),
    RecentLoaded(Result<Vec<Track>, String>),
    MostLoaded(Result<Vec<Track>, String>),
    Sort(tracks::SortKey),
    PlayAll(Vec<i64>),
    AddToPlaylist(Vec<i64>),
    EditMetadata(i64),

    LibraryLoaded(library::Loaded),
    PlayAlbum(String),

    FoldersLoaded(Result<Vec<FolderTrack>, String>),
    SelectFolder(String),
    FolderPlay(Vec<i64>, i64, bool),

    QueueLoaded(Result<Vec<QueueTrack>, String>),
    QueueRowClick(i64),
    RemoveFromQueue(i64),
    ClearQueue,
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

        Msg::SongsLoaded(q, r) => {
            // Ignore stale responses for a previous filter / sort.
            if let Route::Songs(cur) = app.route() {
                if *cur != q {
                    return Task::none();
                }
            }
            let first = !app.pages.songs.loaded || app.pages.songs_query.as_ref() != Some(&q);
            app.pages.songs_query = Some(q);
            let l = &mut app.pages.songs;
            l.loading = false;
            l.loaded = true;
            match r {
                Ok(v) => {
                    l.tracks = v;
                    l.error = None;
                }
                Err(e) => l.error = Some(e),
            }
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
        Msg::Sort(key) => match app.route().clone() {
            Route::Songs(q) => app.update(Message::ReplaceRoute(Route::Songs(songs::sorted(&q, key)))),
            _ => Task::none(),
        },
        Msg::PlayAll(ids) => {
            if ids.is_empty() {
                return Task::none();
            }
            fire(&app.backend, move |b| {
                halo_backend::commands::player::set_shuffle(b.clone(), false)?;
                halo_backend::commands::player::set_queue_and_play(b, ids, 0)
            })
        }
        Msg::AddToPlaylist(_ids) => Task::none(),
        Msg::EditMetadata(_id) => Task::none(),

        Msg::LibraryLoaded(l) => {
            library::on_loaded(app, l);
            Task::none()
        }
        Msg::PlayAlbum(name) => library::play_album(app, name),

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

        Msg::QueueLoaded(r) => {
            app.pages.queue.items.set(r);
            Task::none()
        }
        Msg::QueueRowClick(i) => queue::row_click(app, i),
        Msg::RemoveFromQueue(id) => queue::remove(app, id),
        Msg::ClearQueue => queue::clear(app),
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

/// Resolve "remembered tab" routes (`/library` without `?tab=`).
fn normalize_route(app: &mut Halo) {
    match app.route().clone() {
        Route::Library(None) => {
            let tab = app.pages.library.last_tab;
            app.history.replace(Route::Library(Some(tab)));
        }
        Route::Library(Some(tab)) => app.pages.library.last_tab = tab,
        _ => {}
    }
}

/// Load whatever the current route needs and restore its scroll position.
pub fn on_enter(app: &mut Halo) -> Task<Message> {
    normalize_route(app);
    let route = app.route().clone();
    let restore = app.pages.scroll.restore(&scroll_key_for(app, &route));
    let load = match &route {
        Route::Songs(q) => songs::load_songs(app, q.clone()),
        Route::RecentlyPlayed => songs::load_recent(app),
        Route::MostPlayed => songs::load_most(app),
        Route::Albums => library::load(app, library::Kind::Albums),
        Route::Library(Some(tab)) => library::load(app, (*tab).into()),
        Route::Library(None) => Task::none(),
        Route::Artists => library::load(app, library::Kind::Artists),
        Route::AlbumArtists => library::load(app, library::Kind::AlbumArtists),
        Route::Composers => library::load(app, library::Kind::Composers),
        Route::Genres => library::load(app, library::Kind::Genres),
        Route::Years => library::load(app, library::Kind::Years),
        Route::Folders => load_folders(app),
        Route::Queue => queue::load(app),
        Route::Playlists | Route::Playlist(_) | Route::SmartPlaylist(_) | Route::Settings(_) => {
            Task::none()
        }
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

/// `library-changed`: reload what's on screen; mark the rest stale.
pub fn on_library_changed(app: &mut Halo) -> Task<Message> {
    app.pages.recent.loaded = app.pages.recent.loaded && matches!(app.route(), Route::RecentlyPlayed);
    on_enter(app)
}

pub fn on_queue_changed(app: &mut Halo) -> Task<Message> {
    if matches!(app.route(), Route::Queue) {
        queue::load(app)
    } else {
        Task::none()
    }
}

pub fn on_scan_progress(_app: &mut Halo, _p: ScanProgress) -> Task<Message> {
    Task::none()
}

pub fn on_eq_changed(_app: &mut Halo, _cfg: EqConfig) -> Task<Message> {
    Task::none()
}

pub fn on_rating_changed(app: &mut Halo, track_id: i64, rating: i64) -> Task<Message> {
    app.pages.songs.set_rating(track_id, rating);
    app.pages.recent.set_rating(track_id, rating);
    app.pages.most.set_rating(track_id, rating);
    Task::none()
}

pub fn on_escape(_app: &mut Halo) -> Task<Message> {
    Task::none()
}

pub fn on_ctrl_enter(_app: &mut Halo) -> Task<Message> {
    Task::none()
}

pub fn subscription(_app: &Halo) -> Option<Subscription<Message>> {
    None
}

pub fn overlay(_app: &Halo) -> Option<Element<'_, Message>> {
    None
}

pub fn view(app: &Halo) -> Element<'_, Message> {
    match app.route() {
        Route::Songs(q) => songs::view_songs(app, q),
        Route::RecentlyPlayed => songs::view_recent(app),
        Route::MostPlayed => songs::view_most(app),
        Route::Albums => library::view_standalone(app, library::Kind::Albums),
        Route::Library(tab) => library::view_library(app, tab.unwrap_or(LibraryTab::Artists)),
        Route::Artists => library::view_standalone(app, library::Kind::Artists),
        Route::AlbumArtists => library::view_standalone(app, library::Kind::AlbumArtists),
        Route::Composers => library::view_standalone(app, library::Kind::Composers),
        Route::Genres => library::view_standalone(app, library::Kind::Genres),
        Route::Years => library::view_standalone(app, library::Kind::Years),
        Route::Folders => folders::view(app),
        Route::Queue => queue::view(app),
        other => center(text(format!("{other:?}"))).into(),
    }
}

#[allow(dead_code)]
fn _player(_: PlayerCmd) {}
