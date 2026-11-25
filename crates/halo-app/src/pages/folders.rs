//! Folders tab: your music folders as cards (cover from a song inside,
//! name, "N songs · 2 h 14 m"). Click one to go in (`Route::Folder`, so ←
//! Back and the breadcrumb lead back up): its subfolders as cards, then its
//! own songs as normal song rows. Play / Shuffle play the whole folder.
//! With a single music folder the tab opens straight inside it.

use std::collections::BTreeMap;

use halo_backend::commands::library::{self, norm_path, FolderTrack, Track};
use iced::widget::{button, center, column, container, responsive, row, text};
use iced::{Alignment, Element, Length, Padding, Task};

use super::common::{self, note};
use super::library::Coll;
use super::tracks::{self, TrackList, Variant};
use super::Msg;
use crate::app::{bg, Halo, Message};
use crate::format::plural;
use crate::icons::{icon, Icon};
use crate::route::Route;
use crate::theme::{self, a, t};
use crate::ui;
use crate::widgets::{line, surface};

const CARD_MIN: f32 = 150.0;
const CARD_MAX: f32 = 196.0;
const GAP: f32 = 16.0;

#[derive(Default)]
pub struct FoldersState {
    pub rows: Coll<FolderTrack>,
    pub roots: Vec<FolderNode>,
    /// The open folder's own songs (full `Track`s, for song rows).
    pub dir_tracks: TrackList,
    dir_for: Option<String>,
}

/// One folder: totals include every subfolder.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FolderNode {
    pub path: String,
    pub name: String,
    pub children: Vec<FolderNode>,
    /// Songs directly in this folder.
    pub direct: usize,
    /// Songs here and below.
    pub songs: usize,
    pub duration_ms: i64,
    /// A cover from a song inside (its own songs first, then subfolders).
    pub art: Option<String>,
}

fn name_of(path: &str) -> String {
    path.rsplit(['/', '\\']).find(|s| !s.is_empty()).unwrap_or(path).to_string()
}

fn relative_segments(file: &str, folder: &str) -> Vec<String> {
    let f = norm_path(folder);
    let p = norm_path(file);
    let rest = if p.to_lowercase().starts_with(&(f.to_lowercase() + "/")) { p[f.len() + 1..].to_string() } else { p };
    rest.split('/').filter(|s| !s.is_empty()).map(String::from).collect()
}

#[derive(Default)]
struct Build {
    path: String,
    children: BTreeMap<String, Build>,
    tracks: Vec<FolderTrack>,
}

fn finish(b: Build) -> FolderNode {
    let mut children: Vec<FolderNode> = b.children.into_values().map(finish).collect();
    children.sort_by_key(|c| c.name.to_lowercase());
    let mut tracks = b.tracks;
    tracks.sort_by_key(|t| t.file_path.to_lowercase());
    let art = tracks
        .iter()
        .find_map(|t| t.album_art_path.clone().filter(|p| !p.is_empty()))
        .or_else(|| children.iter().find_map(|c| c.art.clone()));
    FolderNode {
        name: name_of(&b.path),
        path: b.path,
        direct: tracks.len(),
        songs: tracks.len() + children.iter().map(|c| c.songs).sum::<usize>(),
        duration_ms: tracks.iter().filter_map(|t| t.duration_ms).sum::<i64>()
            + children.iter().map(|c| c.duration_ms).sum::<i64>(),
        art,
        children,
    }
}

/// The folder tree (one root per music folder) from the flat rows.
pub fn tree(rows: &[FolderTrack]) -> Vec<FolderNode> {
    let mut roots: BTreeMap<i64, Build> = BTreeMap::new();
    let mut order: Vec<i64> = vec![];
    for r in rows {
        let root = roots.entry(r.folder_id).or_insert_with(|| {
            order.push(r.folder_id);
            Build { path: r.folder_path.clone(), ..Build::default() }
        });
        let segs = relative_segments(&r.file_path, &r.folder_path);
        if segs.is_empty() {
            continue;
        }
        let mut node = root;
        for seg in &segs[..segs.len() - 1] {
            let p = format!("{}/{seg}", node.path.trim_end_matches(['/', '\\']));
            node = node.children.entry(seg.clone()).or_insert_with(|| Build { path: p, ..Build::default() });
        }
        node.tracks.push(r.clone());
    }
    order.into_iter().filter_map(|id| roots.remove(&id)).map(finish).collect()
}

/// The folder at `path` and the chain of folders above it (root first).
pub fn find<'a>(roots: &'a [FolderNode], path: &str) -> Option<Vec<&'a FolderNode>> {
    let want = norm_path(path);
    fn walk<'a>(n: &'a FolderNode, want: &str, chain: &mut Vec<&'a FolderNode>) -> bool {
        chain.push(n);
        let p = norm_path(&n.path);
        if p == want {
            return true;
        }
        if want.starts_with(&(p + "/")) {
            for c in &n.children {
                if walk(c, want, chain) {
                    return true;
                }
            }
        }
        chain.pop();
        false
    }
    let mut chain = vec![];
    roots.iter().any(|r| walk(r, &want, &mut chain)).then_some(chain)
}

/// "2 h 14 m" / "38 m".
pub fn long_duration(ms: i64) -> String {
    let m = (ms.max(0) + 30_000) / 60_000;
    if m >= 60 { format!("{} h {} m", m / 60, m % 60) } else { format!("{m} m") }
}

/// The open folder lists songs of its own (the tab bar then shows ☰ / ▦).
pub fn shows_songs(app: &Halo) -> bool {
    open_path(app)
        .and_then(|p| find(&app.pages.folders.roots, &p).and_then(|c| c.last().map(|n| n.direct > 0)))
        .unwrap_or(false)
}

/// The folder a route shows (`Route::Folders` with one music folder: it).
fn open_path(app: &Halo) -> Option<String> {
    match app.route() {
        Route::Folder(p) => Some(p.clone()),
        Route::Folders if app.pages.folders.roots.len() == 1 => Some(app.pages.folders.roots[0].path.clone()),
        _ => None,
    }
}

// ── Loading ──────────────────────────────────────────────────────────────────

pub fn load(app: &mut Halo) -> Task<Message> {
    app.pages.folders.rows.loading = !app.pages.folders.rows.loaded;
    let tree_task = bg(&app.backend, |b| library::get_folder_tracks(b.db()), |r| Message::Page(Msg::FoldersLoaded(r)));
    Task::batch([tree_task, load_dir(app)])
}

/// The open folder's own songs.
fn load_dir(app: &mut Halo) -> Task<Message> {
    let Some(path) = open_path(app) else { return Task::none() };
    let st = &mut app.pages.folders;
    if st.dir_for.as_deref() != Some(&path) {
        st.dir_tracks = TrackList { loading: true, ..TrackList::default() };
    }
    let p = path.clone();
    bg(
        &app.backend,
        move |b| library::get_dir_tracks(p, false, b.db()),
        move |r| Message::Page(Msg::FolderTracks(path.clone(), r)),
    )
}

pub fn on_loaded(app: &mut Halo, r: Result<Vec<FolderTrack>, String>) -> Task<Message> {
    let had_one = app.pages.folders.roots.len() == 1;
    let st = &mut app.pages.folders;
    st.rows.set(r);
    st.roots = tree(&st.rows.items);
    // First load with a single music folder: now we know which one to open.
    if !had_one && st.roots.len() == 1 && matches!(app.route(), Route::Folders) {
        return load_dir(app);
    }
    Task::none()
}

pub fn on_dir_tracks(app: &mut Halo, path: String, r: Result<Vec<Track>, String>) {
    if open_path(app).as_deref() != Some(&path) {
        return;
    }
    let st = &mut app.pages.folders;
    st.dir_for = Some(path);
    let l = &mut st.dir_tracks;
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

/// Play (or shuffle) everything in `path`, subfolders included.
pub fn play(app: &Halo, path: String, shuffle: bool) -> Task<Message> {
    crate::app::fire(&app.backend, move |b| {
        let ids: Vec<i64> = library::get_dir_tracks(path, true, b.db())?.iter().map(|t| t.id).collect();
        if ids.is_empty() {
            return Ok(());
        }
        halo_backend::commands::player::set_shuffle(b.clone(), shuffle)?;
        let start = if shuffle { crate::app::random_index(ids.len()) as i64 } else { 0 };
        halo_backend::commands::player::set_queue_and_play(b, ids, start)
    })
}

// ── View ─────────────────────────────────────────────────────────────────────

pub fn view(app: &Halo) -> Element<'_, Message> {
    let st = &app.pages.folders;
    let body: Element<'_, Message> = if st.rows.loading && st.roots.is_empty() {
        note("Loading…")
    } else if let Some(e) = &st.rows.error {
        common::error_text(e)
    } else if st.roots.is_empty() {
        common::empty_state(
            Icon::Folder,
            "No music folders yet",
            "Add one to start your library.",
            Some((
                "Add music folder",
                Message::Navigate(Route::Settings(Some(crate::route::SettingsTab::Library))),
            )),
        )
    } else {
        match open_path(app) {
            Some(path) => match find(&st.roots, &path) {
                Some(chain) => folder_page(app, chain),
                None => note("This folder isn't in your library any more."),
            },
            None => {
                let roots: Vec<&FolderNode> = st.roots.iter().collect();
                common::scroll_page(app.route().scroll_key(), container(cards(roots)).padding(Padding::from([16.0, 24.0])))
            }
        }
    };
    container(body).width(Length::Fill).height(Length::Fill).into()
}

fn folder_page<'a>(app: &'a Halo, chain: Vec<&'a FolderNode>) -> Element<'a, Message> {
    let k = t();
    let node = *chain.last().expect("non-empty chain");
    let st = &app.pages.folders;

    // Breadcrumb: Folders › Music › Ilaiyaraaja (the last one isn't a link).
    let mut crumbs = row![].spacing(6).align_y(Alignment::Center);
    let several_roots = st.roots.len() > 1;
    if several_roots {
        crumbs = crumbs.push(crumb("Folders", Some(Route::Folders))).push(sep());
    }
    for (i, n) in chain.iter().enumerate() {
        if i > 0 {
            crumbs = crumbs.push(sep());
        }
        let last = i + 1 == chain.len();
        let target = if i == 0 && !several_roots { Route::Folders } else { Route::Folder(n.path.clone()) };
        crumbs = crumbs.push(crumb(&n.name, (!last).then_some(target)));
    }

    let play_btn = |label: &'static str, ic: Icon, primary: bool, shuffle: bool| {
        button(row![icon(ic, 13.0), text(label).size(theme::BODY).font(theme::MEDIUM)].spacing(6).align_y(Alignment::Center))
            .padding(Padding::from([6.0, 12.0]))
            .style(if primary { theme::btn_primary } else { theme::btn_outline })
            .on_press_maybe((node.songs > 0).then(|| Message::Page(Msg::PlayDir(node.path.clone(), shuffle))))
    };
    let header = row![
        crumbs,
        ui::fill(),
        ui::muted(format!("{} · {}", plural(node.songs as i64, "song", "songs"), long_duration(node.duration_ms)), theme::BODY),
        play_btn("Play", Icon::Play, true, false),
        play_btn("Shuffle", Icon::Shuffle, false, true),
    ]
    .spacing(12)
    .align_y(Alignment::Center);

    let current = app.player.current_track.as_ref().map(|t| t.track_id);
    let songs = &st.dir_tracks;
    let has_songs = node.direct > 0;
    let cards_view = app.pages.library.songs_view == super::library::SongsView::Cards;
    let body: Element<'a, Message> = if node.children.is_empty() {
        // Only songs: the fast virtual list.
        if songs.loading {
            note("Loading…")
        } else if cards_view {
            tracks::cards(songs, current, app.route().scroll_key(), &app.pages.scroll)
        } else {
            tracks::table(songs, Variant::Songs, current, app.route().scroll_key(), &app.pages.scroll)
        }
    } else {
        let mut col = column![cards(node.children.iter().collect())].spacing(24);
        if has_songs {
            col = col.push(text("Songs in this folder").size(theme::TITLE).font(theme::SEMIBOLD));
            col = col.push(if songs.loading {
                note("Loading…")
            } else if cards_view {
                tracks::cards_static(songs, current)
            } else {
                tracks::rows(songs, Variant::Songs, current)
            });
        }
        common::scroll_page(app.route().scroll_key(), col)
    };
    let _ = k;
    column![container(header).padding(Padding::from([16.0, 24.0]).bottom(12.0)), container(body).padding(Padding::from([0.0, 24.0]))]
        .height(Length::Fill)
        .into()
}

fn crumb<'a>(label: &str, target: Option<Route>) -> Element<'a, Message> {
    let k = t();
    match target {
        Some(r) => button(text(label.to_string()).size(theme::BODY))
            .padding(0)
            .style(theme::btn_text(k.muted_fg, k.foreground))
            .on_press(Message::Navigate(r))
            .into(),
        None => text(label.to_string()).size(theme::TITLE).font(theme::SEMIBOLD).into(),
    }
}

fn sep<'a>() -> Element<'a, Message> {
    icon(Icon::ChevronRight, 14.0).color(a(t().muted_fg, 0.7)).into()
}

/// Folder cards, as many columns as fit (not virtualised: folder counts
/// are small).
fn cards(nodes: Vec<&FolderNode>) -> Element<'_, Message> {
    responsive(move |size| {
        let n = (((size.width + GAP) / (CARD_MIN + GAP)).floor() as usize).max(1);
        let w = ((size.width - GAP * (n as f32 - 1.0)) / n as f32).min(CARD_MAX);
        let mut col = column![].spacing(GAP);
        for chunk in nodes.chunks(n) {
            let mut r = row![].spacing(GAP);
            for node in chunk {
                r = r.push(container(folder_card(node, w)).width(w));
            }
            col = col.push(r);
        }
        col.into()
    })
    .height(Length::Shrink)
    .into()
}

fn folder_card(n: &FolderNode, side: f32) -> Element<'_, Message> {
    let k = t();
    let inner = (side - 2.0 * theme::CARD_PAD).max(0.0);
    let cover: Element<'_, Message> = match &n.art {
        Some(p) => ui::art(Some(p), inner, theme::R_LG),
        None => center(icon(Icon::Folder, (inner * 0.35).max(20.0)).color(a(k.muted_fg, 0.6)))
            .width(inner)
            .height(inner)
            .style(|_| container::Style {
                background: Some(t().muted.into()),
                border: theme::no_border(theme::R_LG),
                ..container::Style::default()
            })
            .into(),
    };
    let mut sub = plural(n.songs as i64, "song", "songs");
    if n.duration_ms > 0 {
        sub = format!("{sub} · {}", long_duration(n.duration_ms));
    }
    let content = column![
        cover,
        column![
            line(n.name.clone()).size(theme::BODY).font(theme::MEDIUM).width(Length::Fill),
            line(sub).size(theme::SMALL).color(k.muted_fg).width(Length::Fill),
        ],
    ]
    .spacing(8);
    surface(content)
        .width(Length::Fill)
        .padding(theme::CARD_PAD)
        .on_press(Message::Navigate(Route::Folder(n.path.clone())))
        .on_right_press(crate::components::context_menu::opener(crate::components::context_menu::Target::Folder {
            path: n.path.clone(),
        }))
        .interaction(iced::mouse::Interaction::Pointer)
        .style(|s| theme::card(s.hovered))
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ft(folder_id: i64, root: &str, file: &str, ms: i64, art: Option<&str>) -> FolderTrack {
        FolderTrack {
            id: 0,
            folder_id,
            folder_path: root.into(),
            file_path: file.into(),
            title: None,
            duration_ms: Some(ms),
            album_art_path: art.map(Into::into),
        }
    }

    fn sample() -> Vec<FolderNode> {
        tree(&[
            ft(1, "/m", "/m/b.mp3", 60_000, None),
            ft(1, "/m", "/m/Raja/1980s/x.mp3", 120_000, Some("/art/x.jpg")),
            ft(1, "/m", "/m/Raja/y.mp3", 180_000, None),
            ft(1, "/m", "/m/arr/z.mp3", 60_000, Some("/art/z.jpg")),
        ])
    }

    #[test]
    fn tree_counts_durations_and_covers() {
        let roots = sample();
        assert_eq!(roots.len(), 1);
        let m = &roots[0];
        assert_eq!((m.name.as_str(), m.direct, m.songs, m.duration_ms), ("m", 1, 4, 420_000));
        // Children sorted case-insensitively.
        let names: Vec<&str> = m.children.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["arr", "Raja"]);
        let raja = &m.children[1];
        assert_eq!((raja.direct, raja.songs, raja.duration_ms), (1, 2, 300_000));
        // No cover of its own: taken from the 1980s subfolder.
        assert_eq!(raja.art.as_deref(), Some("/art/x.jpg"));
        assert_eq!(raja.children[0].path, "/m/Raja/1980s");
        // Root: its own song has no cover, so the first child's ("arr").
        assert_eq!(m.art.as_deref(), Some("/art/z.jpg"));
    }

    #[test]
    fn find_returns_the_chain_from_the_root() {
        let roots = sample();
        let chain = find(&roots, "/m/Raja/1980s/").unwrap();
        let names: Vec<&str> = chain.iter().map(|n| n.name.as_str()).collect();
        assert_eq!(names, ["m", "Raja", "1980s"]);
        assert!(find(&roots, "/m/Rajax").is_none());
        assert!(find(&roots, "/elsewhere").is_none());
    }

    #[test]
    fn durations_read_naturally() {
        assert_eq!(long_duration(0), "0 m");
        assert_eq!(long_duration(38 * 60_000 + 10_000), "38 m");
        assert_eq!(long_duration(134 * 60_000), "2 h 14 m");
    }
}
