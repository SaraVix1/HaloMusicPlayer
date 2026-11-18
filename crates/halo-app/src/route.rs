//! In-app routing — replaces react-router's HashRouter. Each `Route` maps to
//! one of the old URL paths (query strings become typed fields), and
//! [`History`] is the back/forward stack behind the title-bar arrows.

use halo_backend::commands::library::TracksQuery;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum LibraryTab {
    #[default]
    Artists,
    AlbumArtists,
    Composers,
    Genres,
    Years,
}

impl LibraryTab {
    pub const ALL: [LibraryTab; 5] = [
        LibraryTab::Artists,
        LibraryTab::AlbumArtists,
        LibraryTab::Composers,
        LibraryTab::Genres,
        LibraryTab::Years,
    ];
    pub fn label(self) -> &'static str {
        match self {
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
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Route {
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
    Playlists,
    Playlist(i64),
    SmartPlaylist(i64),
    RecentlyPlayed,
    MostPlayed,
    Queue,
    /// `/settings?tab=` (`None` = last visited tab)
    Settings(Option<SettingsTab>),
}

impl Default for Route {
    fn default() -> Self {
        Route::Songs(TracksQuery::default())
    }
}

impl Route {
    /// Sidebar section this route belongs to (`location.pathname.startsWith`).
    pub fn section(&self) -> Section {
        match self {
            Route::Songs(_) | Route::RecentlyPlayed | Route::MostPlayed => Section::Songs,
            Route::Albums => Section::Albums,
            Route::Library(_)
            | Route::Artists
            | Route::AlbumArtists
            | Route::Composers
            | Route::Genres
            | Route::Years => Section::Library,
            Route::Folders => Section::Folders,
            Route::Playlists | Route::Playlist(_) | Route::SmartPlaylist(_) => {
                Section::Playlists
            }
            Route::Queue => Section::Queue,
            Route::Settings(_) => Section::Settings,
        }
    }

    /// Key used for per-route scroll memory (the old `location.key`-ish path).
    pub fn scroll_key(&self) -> String {
        format!("{self:?}")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    Songs,
    Albums,
    Library,
    Folders,
    Playlists,
    Queue,
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
        h.push(Route::Queue);
        assert!(h.back());
        assert_eq!(*h.current(), Route::Albums);
        assert!(h.can_forward());
        h.push(Route::Folders);
        assert!(!h.can_forward());
        assert!(h.back());
        assert_eq!(*h.current(), Route::Albums);
    }

    #[test]
    fn push_same_is_noop() {
        let mut h = History::default();
        h.push(Route::default());
        assert!(!h.can_back());
    }
}
