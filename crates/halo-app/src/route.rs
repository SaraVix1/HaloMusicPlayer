//! In-app routing — replaces react-router's HashRouter. Each `Route` maps to
//! one of the old URL paths (query strings become typed fields), and
//! [`History`] is the back/forward stack behind the title-bar arrows.

use halo_backend::commands::library::TracksQuery;

/// Library page tabs. Songs, Albums and Folders keep their own routes
/// (`Route::Songs`, `Route::Albums`, `Route::Folders`) and are drawn under
/// the Library tab bar; the others are `Route::Library(Some(tab))`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum LibraryTab {
    #[default]
    Songs,
    Albums,
    Artists,
    AlbumArtists,
    Composers,
    Genres,
    Years,
    Folders,
}

impl LibraryTab {
    pub const ALL: [LibraryTab; 8] = [
        LibraryTab::Songs,
        LibraryTab::Albums,
        LibraryTab::Artists,
        LibraryTab::AlbumArtists,
        LibraryTab::Composers,
        LibraryTab::Genres,
        LibraryTab::Years,
        LibraryTab::Folders,
    ];
    pub fn label(self) -> &'static str {
        match self {
            LibraryTab::Songs => "Songs",
            LibraryTab::Albums => "Albums",
            LibraryTab::Folders => "Folders",
            LibraryTab::Artists => "Artists",
            LibraryTab::AlbumArtists => "Album Artists",
            LibraryTab::Composers => "Composers",
            LibraryTab::Genres => "Genres",
            LibraryTab::Years => "Years",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SettingsTab {
    #[default]
    General,
    Appearance,
    Playback,
    Equalizer,
    Library,
    Advanced,
}

impl SettingsTab {
    pub fn label(self) -> &'static str {
        match self {
            SettingsTab::General => "General",
            SettingsTab::Appearance => "Appearance",
            SettingsTab::Playback => "Playback",
            SettingsTab::Equalizer => "Equalizer",
            SettingsTab::Library => "Library",
            SettingsTab::Advanced => "Advanced",
        }
    }
}

/// Standalone list routes (`Artists`, `Years`, …) and `RecentlyPlayed` /
/// `MostPlayed` exist like their old URL paths even though the sidebar
/// doesn't link them.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum Route {
    /// Start page: greeting + album rows.
    #[default]
    Home,
    /// `/songs?album=&artist=&album_artist=&composer=&genre=&year=&sort=&dir=`
    Songs(TracksQuery),
    Albums,
    /// `/library?tab=` (`None` = last visited tab)
    Library(Option<LibraryTab>),
    Artists,
    AlbumArtists,
    Composers,
    Genres,
    Years,
    Folders,
    /// Inside a music folder (its path).
    Folder(String),
    Playlists,
    Playlist(i64),
    SmartPlaylist(i64),
    RecentlyPlayed,
    MostPlayed,
    /// `/settings?tab=` (`None` = last visited tab)
    Settings(Option<SettingsTab>),
}

/// The Library tab a song list belongs to: the kind of its filter (an
/// album → Albums, a genre → Genres …); no filter → Songs.
pub fn filter_tab(q: &TracksQuery) -> LibraryTab {
    if q.album.is_some() {
        LibraryTab::Albums
    } else if q.artist.is_some() {
        LibraryTab::Artists
    } else if q.album_artist.is_some() {
        LibraryTab::AlbumArtists
    } else if q.composer.is_some() {
        LibraryTab::Composers
    } else if q.genre.is_some() {
        LibraryTab::Genres
    } else if q.year.is_some() {
        LibraryTab::Years
    } else if q.folder_id.is_some() {
        LibraryTab::Folders
    } else {
        LibraryTab::Songs
    }
}

/// The filter's value, for the breadcrumb (`Genres › Beat`).
pub fn filter_label(q: &TracksQuery) -> Option<String> {
    q.album
        .clone()
        .or_else(|| q.artist.clone())
        .or_else(|| q.album_artist.clone())
        .or_else(|| q.composer.clone())
        .or_else(|| q.genre.clone())
        .or_else(|| q.year.map(|y| y.to_string()))
}

/// No filter: the whole library (the Library page's Songs tab). A filtered
/// query is an album / artist / genre … track list.
pub fn is_all_songs(q: &TracksQuery) -> bool {
    q.album.is_none()
        && q.artist.is_none()
        && q.album_artist.is_none()
        && q.composer.is_none()
        && q.genre.is_none()
        && q.year.is_none()
        && q.folder_id.is_none()
}

impl Route {
    /// Sidebar section this route belongs to (`location.pathname.startsWith`).
    pub fn section(&self) -> Section {
        match self {
            Route::Home | Route::RecentlyPlayed | Route::MostPlayed => Section::Home,
            Route::Songs(_)
            | Route::Albums
            | Route::Folders
            | Route::Folder(_)
            | Route::Library(_)
            | Route::Artists
            | Route::AlbumArtists
            | Route::Composers
            | Route::Genres
            | Route::Years => Section::Library,
            Route::Playlists | Route::Playlist(_) | Route::SmartPlaylist(_) => {
                Section::Playlists
            }
            Route::Settings(_) => Section::Settings,
        }
    }

    /// The Library tab this route shows, if it is one.
    pub fn library_tab(&self) -> Option<LibraryTab> {
        match self {
            Route::Songs(q) => Some(filter_tab(q)),
            Route::Albums => Some(LibraryTab::Albums),
            Route::Folders | Route::Folder(_) => Some(LibraryTab::Folders),
            Route::Library(tab) => *tab,
            _ => None,
        }
    }

    /// Key used for per-route scroll memory (the old `location.key`-ish path).
    pub fn scroll_key(&self) -> String {
        format!("{self:?}")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    Home,
    Library,
    Playlists,
    Settings,
}

/// Linear navigation history with a cursor.
#[derive(Debug, Clone)]
pub struct History {
    stack: Vec<Route>,
    index: usize,
}

impl Default for History {
    fn default() -> Self {
        Self {
            stack: vec![Route::default()],
            index: 0,
        }
    }
}

impl History {
    pub fn current(&self) -> &Route {
        &self.stack[self.index]
    }

    /// Navigate to `route` (PUSH): drops forward entries. Pushing the current
    /// route again is a no-op.
    pub fn push(&mut self, route: Route) {
        if *self.current() == route {
            return;
        }
        self.stack.truncate(self.index + 1);
        self.stack.push(route);
        self.index += 1;
    }

    /// Replace the current entry (REPLACE), e.g. changing a sort order.
    pub fn replace(&mut self, route: Route) {
        self.stack[self.index] = route;
    }

    pub fn can_back(&self) -> bool {
        self.index > 0
    }

    pub fn can_forward(&self) -> bool {
        self.index + 1 < self.stack.len()
    }

    pub fn back(&mut self) -> bool {
        if self.can_back() {
            self.index -= 1;
            true
        } else {
            false
        }
    }

    pub fn forward(&mut self) -> bool {
        if self.can_forward() {
            self.index += 1;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_back_forward() {
        let mut h = History::default();
        assert!(!h.can_back());
        h.push(Route::Albums);
        h.push(Route::Playlists);
        assert!(h.back());
        assert_eq!(*h.current(), Route::Albums);
        assert!(h.can_forward());
        h.push(Route::Folders);
        assert!(!h.can_forward());
        assert!(h.back());
        assert_eq!(*h.current(), Route::Albums);
    }

    #[test]
    fn library_tabs_map_to_routes() {
        assert_eq!(Route::Songs(TracksQuery::default()).library_tab(), Some(LibraryTab::Songs));
        let sorted = TracksQuery { sort: Some("album".into()), ..TracksQuery::default() };
        assert_eq!(Route::Songs(sorted).library_tab(), Some(LibraryTab::Songs));
        let album = TracksQuery { album: Some("X".into()), ..TracksQuery::default() };
        assert_eq!(Route::Songs(album.clone()).library_tab(), Some(LibraryTab::Albums));
        assert_eq!(Route::Songs(album).section(), Section::Library);
        assert_eq!(Route::Albums.library_tab(), Some(LibraryTab::Albums));
        assert_eq!(Route::Folders.library_tab(), Some(LibraryTab::Folders));
        assert_eq!(Route::Home.section(), Section::Home);
        assert_eq!(Route::default(), Route::Home);
    }

    #[test]
    fn filters_belong_to_their_tab() {
        let q = |f: fn(&mut TracksQuery)| {
            let mut q = TracksQuery::default();
            f(&mut q);
            q
        };
        let genre = q(|q| q.genre = Some("Beat".into()));
        assert_eq!(filter_tab(&genre), LibraryTab::Genres);
        assert_eq!(filter_label(&genre).as_deref(), Some("Beat"));
        assert_eq!(filter_tab(&q(|q| q.artist = Some("A".into()))), LibraryTab::Artists);
        assert_eq!(filter_tab(&q(|q| q.album_artist = Some("A".into()))), LibraryTab::AlbumArtists);
        assert_eq!(filter_tab(&q(|q| q.composer = Some("C".into()))), LibraryTab::Composers);
        let year = q(|q| q.year = Some(1992));
        assert_eq!(filter_tab(&year), LibraryTab::Years);
        assert_eq!(filter_label(&year).as_deref(), Some("1992"));
        assert_eq!(filter_tab(&TracksQuery::default()), LibraryTab::Songs);
        assert_eq!(filter_label(&TracksQuery::default()), None);
    }

    #[test]
    fn push_same_is_noop() {
        let mut h = History::default();
        h.push(Route::default());
        assert!(!h.can_back());
    }
}
