//! Library sort orders: the options each tab's Sort ▾ menu offers, how a
//! choice is applied (Songs: the SQL `sort` key of its route; the other
//! tabs: an in-memory sort of the loaded collection), and persistence in
//! `app_state` (`library.sort.<tab>` = `<option>:<asc|desc>`).

use std::cmp::Ordering;
use std::collections::HashMap;

use halo_backend::commands::library::{Album, AlbumArtist, Artist, Composer, Genre, TracksQuery, YearStat};

use crate::route::LibraryTab;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SortBy {
    // Collections
    Name,
    Artist,
    Year,
    Added,
    Count,
    // Songs
    Title,
    Album,
    Duration,
    Plays,
    Rating,
    TrackNumber,
}

impl SortBy {
    const ALL: [SortBy; 11] = [
        SortBy::Name,
        SortBy::Artist,
        SortBy::Year,
        SortBy::Added,
        SortBy::Count,
        SortBy::Title,
        SortBy::Album,
        SortBy::Duration,
        SortBy::Plays,
        SortBy::Rating,
        SortBy::TrackNumber,
    ];

    /// Stable id for prefs.
    fn id(self) -> &'static str {
        match self {
            SortBy::Name => "name",
            SortBy::Artist => "artist",
            SortBy::Year => "year",
            SortBy::Added => "added",
            SortBy::Count => "count",
            SortBy::Title => "title",
            SortBy::Album => "album",
            SortBy::Duration => "duration",
            SortBy::Plays => "plays",
            SortBy::Rating => "rating",
            SortBy::TrackNumber => "track",
        }
    }

    /// Direction a fresh pick starts in: "most / newest first" options go
    /// high-to-low, names and titles A–Z.
    pub fn default_desc(self) -> bool {
        matches!(self, SortBy::Added | SortBy::Count | SortBy::Plays | SortBy::Rating | SortBy::Year)
    }

    /// The `TracksQuery::sort` key (Songs tab and filtered song lists).
    fn query_key(self) -> &'static str {
        match self {
            SortBy::Artist => "artist",
            SortBy::Album => "album",
            SortBy::Duration => "duration",
            SortBy::Year => "year",
            SortBy::Added => "scanned_at", // backend maps it to added_at
            SortBy::Plays => "play_count",
            SortBy::Rating => "rating",
            SortBy::TrackNumber => "track_number",
            SortBy::Title | SortBy::Name | SortBy::Count => "title",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sort {
    pub by: SortBy,
    pub desc: bool,
}

impl Sort {
    pub const fn asc(by: SortBy) -> Self {
        Sort { by, desc: false }
    }

    /// Picking `by` in the menu: the active option flips direction, another
    /// one starts in its default direction.
    pub fn pick(self, by: SortBy) -> Sort {
        if self.by == by {
            Sort { by, desc: !self.desc }
        } else {
            Sort { by, desc: by.default_desc() }
        }
    }

    fn encode(self) -> String {
        format!("{}:{}", self.by.id(), if self.desc { "desc" } else { "asc" })
    }

    fn decode(s: &str) -> Option<Sort> {
        let (id, dir) = s.split_once(':')?;
        let by = SortBy::ALL.into_iter().find(|b| b.id() == id)?;
        let desc = match dir {
            "desc" => true,
            "asc" => false,
            _ => return None,
        };
        Some(Sort { by, desc })
    }
}

/// Menu options for a tab, in order. Empty: the tab has no Sort button.
pub fn options(tab: LibraryTab) -> &'static [(SortBy, &'static str)] {
    match tab {
        LibraryTab::Songs => SONG_OPTIONS,
        LibraryTab::Albums => &[
            (SortBy::Name, "Name"),
            (SortBy::Artist, "Artist"),
            (SortBy::Year, "Year"),
            (SortBy::Added, "Recently added"),
        ],
        LibraryTab::Artists | LibraryTab::AlbumArtists | LibraryTab::Composers | LibraryTab::Genres => {
            &[(SortBy::Name, "Name"), (SortBy::Count, "Most tracks")]
        }
        LibraryTab::Years => &[(SortBy::Year, "Year"), (SortBy::Count, "Most tracks")],
        LibraryTab::Folders => &[],
    }
}

const SONG_OPTIONS: &[(SortBy, &str)] = &[
    (SortBy::Title, "Title"),
    (SortBy::Artist, "Artist"),
    (SortBy::Album, "Album"),
    (SortBy::Duration, "Duration"),
    (SortBy::Added, "Date added"),
    (SortBy::Plays, "Most played"),
    (SortBy::Rating, "Rating"),
];

/// Options for a song list: an album's list can also go by track number.
pub fn song_options(q: &TracksQuery) -> Vec<(SortBy, &'static str)> {
    let mut v = Vec::with_capacity(SONG_OPTIONS.len() + 1);
    if q.album.is_some() {
        v.push((SortBy::TrackNumber, "Track number"));
    }
    v.extend_from_slice(SONG_OPTIONS);
    v
}

pub fn label(options: &[(SortBy, &'static str)], by: SortBy) -> &'static str {
    options.iter().find(|(b, _)| *b == by).map(|(_, l)| *l).unwrap_or("Sort")
}

/// Default order of each tab: what the backend returns.
pub fn default_for(tab: LibraryTab) -> Sort {
    match tab {
        LibraryTab::Songs => Sort::asc(SortBy::Title),
        LibraryTab::Years => Sort { by: SortBy::Year, desc: true },
        _ => Sort::asc(SortBy::Name),
    }
}

// ── Songs: the sort lives in the route's query ──────────────────────────────

/// The Sort a song query represents.
pub fn of_query(q: &TracksQuery) -> Sort {
    let key = q.sort.as_deref().unwrap_or("title");
    let by = SortBy::ALL
        .into_iter()
        .find(|b| !matches!(b, SortBy::Name | SortBy::Count) && b.query_key() == key)
        .unwrap_or(SortBy::Title);
    Sort { by, desc: q.direction.as_deref() == Some("desc") }
}

/// `q` re-sorted by `s` (filters kept).
pub fn apply_to_query(q: &TracksQuery, s: Sort) -> TracksQuery {
    TracksQuery {
        sort: Some(s.by.query_key().into()),
        direction: Some(if s.desc { "desc" } else { "asc" }.into()),
        ..q.clone()
    }
}

// ── Collections: sorted in memory ───────────────────────────────────────────

fn ci(a: &str, b: &str) -> Ordering {
    a.to_lowercase().cmp(&b.to_lowercase())
}

/// `primary`, reversed when descending; ties always by name A–Z so equal
/// counts / years read alphabetically.
fn order(primary: Ordering, desc: bool, a_name: &str, b_name: &str) -> Ordering {
    let p = if desc { primary.reverse() } else { primary };
    p.then_with(|| ci(a_name, b_name))
}

/// `None` last whichever the direction.
fn opt_cmp<T: Ord>(a: &Option<T>, b: &Option<T>, desc: bool) -> Ordering {
    match (a, b) {
        (Some(x), Some(y)) => {
            let o = x.cmp(y);
            if desc { o.reverse() } else { o }
        }
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

pub fn albums(v: &mut [Album], s: Sort) {
    v.sort_by(|a, b| match s.by {
        SortBy::Artist => order(ci(&a.album_artists.join(", "), &b.album_artists.join(", ")), s.desc, &a.name, &b.name),
        SortBy::Year => opt_cmp(&a.year, &b.year, s.desc).then_with(|| ci(&a.name, &b.name)),
        SortBy::Added => opt_cmp(&a.added, &b.added, s.desc).then_with(|| ci(&a.name, &b.name)),
        _ => order(ci(&a.name, &b.name), s.desc, "", ""),
    });
}

/// Name / track-count sort for the people and genre collections.
pub trait Named {
    fn name(&self) -> &str;
    fn count(&self) -> i64;
}

macro_rules! named {
    ($($t:ty),*) => {$(
        impl Named for $t {
            fn name(&self) -> &str { &self.name }
            fn count(&self) -> i64 { self.track_count }
        }
    )*};
}
named!(Artist, AlbumArtist, Composer, Genre);

pub fn named<T: Named>(v: &mut [T], s: Sort) {
    v.sort_by(|a, b| match s.by {
        SortBy::Count => order(a.count().cmp(&b.count()), s.desc, a.name(), b.name()),
        _ => order(ci(a.name(), b.name()), s.desc, "", ""),
    });
}

pub fn years(v: &mut [YearStat], s: Sort) {
    v.sort_by(|a, b| {
        let p = match s.by {
            SortBy::Count => a.track_count.cmp(&b.track_count),
            _ => a.year.cmp(&b.year),
        };
        let p = if s.desc { p.reverse() } else { p };
        // Equal counts: newest year first, whichever the direction.
        p.then(b.year.cmp(&a.year))
    });
}

// ── Persistence ─────────────────────────────────────────────────────────────

fn pref_key(tab: LibraryTab) -> String {
    format!("library.sort.{tab:?}")
}

/// Saved sorts for every tab (unknown / invalid values are skipped). Runs
/// once at startup: eight point lookups in `app_state`.
pub fn load(backend: &halo_backend::Backend) -> HashMap<LibraryTab, Sort> {
    let mut out = HashMap::new();
    for tab in LibraryTab::ALL {
        let opts = options(tab);
        if let Some(s) = halo_backend::commands::ui::get_pref(pref_key(tab), backend.db())
            .ok()
            .flatten()
            .and_then(|v| Sort::decode(&v))
            .filter(|s| opts.iter().any(|(b, _)| *b == s.by))
        {
            out.insert(tab, s);
        }
    }
    out
}

/// Save one tab's sort (off the UI thread).
pub fn save(backend: &halo_backend::Backend, tab: LibraryTab, s: Sort) -> iced::Task<crate::app::Message> {
    let (key, value) = (pref_key(tab), s.encode());
    crate::app::fire(backend, move |b| halo_backend::commands::ui::set_pref(key, value, b.db()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn album(name: &str, artist: &str, year: Option<i64>, added: Option<&str>) -> Album {
        Album {
            name: name.into(),
            album_artists: if artist.is_empty() { vec![] } else { vec![artist.into()] },
            track_count: 1,
            year,
            album_art_path: None,
            added: added.map(Into::into),
        }
    }

    fn names(v: &[Album]) -> Vec<&str> {
        v.iter().map(|a| a.name.as_str()).collect()
    }

    #[test]
    fn pick_flips_active_and_starts_others_in_their_default() {
        let s = Sort::asc(SortBy::Name);
        assert_eq!(s.pick(SortBy::Name), Sort { by: SortBy::Name, desc: true });
        assert_eq!(s.pick(SortBy::Count), Sort { by: SortBy::Count, desc: true });
        assert_eq!(s.pick(SortBy::Count).pick(SortBy::Count).desc, false);
    }

    #[test]
    fn encode_decode_round_trip() {
        for by in SortBy::ALL {
            for desc in [false, true] {
                let s = Sort { by, desc };
                assert_eq!(Sort::decode(&s.encode()), Some(s));
            }
        }
        assert_eq!(Sort::decode("bogus:asc"), None);
        assert_eq!(Sort::decode("name:sideways"), None);
        assert_eq!(Sort::decode("name"), None);
    }

    #[test]
    fn every_tab_default_is_one_of_its_options() {
        for tab in LibraryTab::ALL {
            let opts = options(tab);
            if !opts.is_empty() {
                assert!(opts.iter().any(|(b, _)| *b == default_for(tab).by), "{tab:?}");
            }
        }
    }

    #[test]
    fn albums_sort_by_each_option() {
        let mut v = vec![
            album("beta", "Zed", Some(2001), Some("2025-01-01 00:00:00")),
            album("Alpha", "Amy", None, Some("2026-01-01 00:00:00")),
            album("gamma", "", Some(1999), None),
        ];
        albums(&mut v, Sort::asc(SortBy::Name));
        assert_eq!(names(&v), ["Alpha", "beta", "gamma"]);
        albums(&mut v, Sort { by: SortBy::Name, desc: true });
        assert_eq!(names(&v), ["gamma", "beta", "Alpha"]);
        albums(&mut v, Sort::asc(SortBy::Artist));
        assert_eq!(names(&v), ["gamma", "Alpha", "beta"]); // "" < "amy" < "zed"
        // Missing year / date always last.
        albums(&mut v, Sort { by: SortBy::Year, desc: true });
        assert_eq!(names(&v), ["beta", "gamma", "Alpha"]);
        albums(&mut v, Sort::asc(SortBy::Year));
        assert_eq!(names(&v), ["gamma", "beta", "Alpha"]);
        albums(&mut v, Sort { by: SortBy::Added, desc: true });
        assert_eq!(names(&v), ["Alpha", "beta", "gamma"]);
    }

    #[test]
    fn named_sort_by_count_breaks_ties_by_name() {
        let g = |name: &str, n: i64| Genre { id: 0, name: name.into(), track_count: n };
        let mut v = vec![g("rock", 5), g("Jazz", 9), g("ambient", 5)];
        named(&mut v, Sort { by: SortBy::Count, desc: true });
        let got: Vec<_> = v.iter().map(|x| x.name.as_str()).collect();
        assert_eq!(got, ["Jazz", "ambient", "rock"]);
        named(&mut v, Sort::asc(SortBy::Name));
        let got: Vec<_> = v.iter().map(|x| x.name.as_str()).collect();
        assert_eq!(got, ["ambient", "Jazz", "rock"]);
    }

    #[test]
    fn years_sort() {
        let y = |year: i64, n: i64| YearStat { year, track_count: n };
        let mut v = vec![y(1999, 3), y(2020, 1), y(2005, 3)];
        years(&mut v, default_for(LibraryTab::Years));
        assert_eq!(v.iter().map(|x| x.year).collect::<Vec<_>>(), [2020, 2005, 1999]);
        years(&mut v, Sort { by: SortBy::Count, desc: true });
        assert_eq!(v.iter().map(|x| x.year).collect::<Vec<_>>(), [2005, 1999, 2020]);
    }

    #[test]
    fn song_queries_round_trip_and_keep_filters() {
        let q = TracksQuery { album: Some("X".into()), ..TracksQuery::default() };
        for (by, _) in song_options(&q) {
            for desc in [false, true] {
                let s = Sort { by, desc };
                let q2 = apply_to_query(&q, s);
                assert_eq!(q2.album.as_deref(), Some("X"));
                assert_eq!(of_query(&q2), s, "{by:?}");
            }
        }
        assert_eq!(of_query(&TracksQuery::default()), Sort::asc(SortBy::Title));
        assert_eq!(song_options(&q)[0].0, SortBy::TrackNumber);
        assert!(song_options(&TracksQuery::default()).iter().all(|(b, _)| *b != SortBy::TrackNumber));
    }
}
