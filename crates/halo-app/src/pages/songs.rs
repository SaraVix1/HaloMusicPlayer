//! All Songs (with URL-style filters + sort), Recently Played, Most Played.

use halo_backend::commands::library::{self, TracksQuery};
use iced::widget::{button, column, row, text};
use iced::{Alignment, Element, Length, Padding, Task};

use super::common::{self, note};
use super::tracks::{self, SortKey, TrackList, Variant};
use super::Msg;
use crate::app::{bg, Halo, Message};
use crate::icons::{icon, Icon};
use crate::route::Route;
use crate::theme::{self, t};

pub fn query_for(q: &TracksQuery) -> TracksQuery {
    let mut q = q.clone();
    q.sort.get_or_insert_with(|| "title".into());
    q.direction.get_or_insert_with(|| "asc".into());
    q
}

pub fn load_songs(app: &mut Halo, q: TracksQuery) -> Task<Message> {
    app.pages.songs.loading = !app.pages.songs.loaded;
    app.pages.songs.error = None;
    let q2 = query_for(&q);
    bg(
        &app.backend,
        move |b| library::get_tracks(q2, b.db()),
        move |r| Message::Page(Msg::SongsLoaded(q, r)),
    )
}

pub fn load_recent(app: &mut Halo) -> Task<Message> {
    app.pages.recent.loading = !app.pages.recent.loaded;
    bg(
        &app.backend,
        |b| library::get_recently_played(b.db()),
        |r| Message::Page(Msg::RecentLoaded(r)),
    )
}

pub fn load_most(app: &mut Halo) -> Task<Message> {
    app.pages.most.loading = !app.pages.most.loaded;
    bg(
        &app.backend,
        |b| library::get_most_played(b.db()),
        |r| Message::Page(Msg::MostLoaded(r)),
    )
}

/// Toggle direction when clicking the active column, otherwise sort by the
/// new column ascending (`setParams(..., { replace: true })`).
pub fn sorted(q: &TracksQuery, key: SortKey) -> TracksQuery {
    let mut next = q.clone();
    let cur = q.sort.as_deref().unwrap_or("title");
    let dir = q.direction.as_deref().unwrap_or("asc");
    if cur == key.as_str() {
        next.direction = Some(if dir == "asc" { "desc" } else { "asc" }.into());
    } else {
        next.sort = Some(key.as_str().into());
        next.direction = Some("asc".into());
    }
    next
}

fn sort_state(q: &TracksQuery) -> (SortKey, bool) {
    let key = match q.sort.as_deref().unwrap_or("title") {
        "artist" => SortKey::Artist,
        "album" => SortKey::Album,
        "genre" => SortKey::Genre,
        "duration" => SortKey::Duration,
        "rating" => SortKey::Rating,
        // track_number / scanned_at / play_count have no header arrow.
        "title" => SortKey::Title,
        _ => return (SortKey::Title, q.direction.as_deref() != Some("desc")),
    };
    (key, q.direction.as_deref() != Some("desc"))
}

fn has_arrow(q: &TracksQuery) -> bool {
    matches!(
        q.sort.as_deref().unwrap_or("title"),
        "title" | "artist" | "album" | "genre" | "duration" | "rating"
    )
}

#[derive(Debug, Clone, Copy)]
pub enum Filter {
    Album,
    Artist,
    AlbumArtist,
    Composer,
    Genre,
    Year,
}

pub fn without(q: &TracksQuery, f: Filter) -> TracksQuery {
    let mut n = q.clone();
    match f {
        Filter::Album => n.album = None,
        Filter::Artist => n.artist = None,
        Filter::AlbumArtist => n.album_artist = None,
        Filter::Composer => n.composer = None,
        Filter::Genre => n.genre = None,
        Filter::Year => n.year = None,
    }
    n
}

fn chips<'a>(q: &TracksQuery) -> Option<Element<'a, Message>> {
    let mut out: Vec<(Filter, String)> = vec![];
    if let Some(v) = &q.album {
        out.push((Filter::Album, format!("Album: {v}")));
    }
    if let Some(v) = &q.artist {
        out.push((Filter::Artist, format!("Artist: {v}")));
    }
    if let Some(v) = &q.album_artist {
        out.push((Filter::AlbumArtist, format!("Album Artist: {v}")));
    }
    if let Some(v) = &q.composer {
        out.push((Filter::Composer, format!("Composer: {v}")));
    }
    if let Some(v) = &q.genre {
        out.push((Filter::Genre, format!("Genre: {v}")));
    }
    if let Some(v) = &q.year {
        out.push((Filter::Year, format!("Year: {v}")));
    }
    if out.is_empty() {
        return None;
    }
    let mut r = row![].spacing(8);
    for (f, label) in out {
        let target = without(q, f);
        r = r.push(
            button(
                row![text(label).size(theme::TEXT_XS).font(theme::MEDIUM), icon(Icon::X, 12.0)]
                    .spacing(4)
                    .align_y(Alignment::Center),
            )
            .padding(Padding::from([4.0, 10.0]))
            .style(|_, st| {
                let k = t();
                iced::widget::button::Style {
                    background: Some(
                        match st {
                            iced::widget::button::Status::Hovered => theme::a(k.muted, 0.7),
                            _ => k.muted,
                        }
                        .into(),
                    ),
                    text_color: k.foreground,
                    border: theme::no_border(theme::R_FULL),
                    ..Default::default()
                }
            })
            .on_press(Message::ReplaceRoute(Route::Songs(target))),
        );
    }
    Some(r.wrap().into())
}

fn empty_library<'a>() -> Element<'a, Message> {
    let k = t();
    row![
        text("No tracks found. ").size(theme::TEXT_SM).color(k.muted_fg),
        button(text("Add a folder and scan").size(theme::TEXT_SM))
            .padding(0)
            .style(theme::btn_link)
            .on_press(Message::Navigate(Route::Settings(Some(
                crate::route::SettingsTab::Library
            )))),
        text(" to get started.").size(theme::TEXT_SM).color(k.muted_fg),
    ]
    .into()
}

fn page<'a>(
    app: &'a Halo,
    title: &'a str,
    list: &'a TrackList,
    variant: Variant,
    q: Option<&TracksQuery>,
    empty: Element<'a, Message>,
) -> Element<'a, Message> {
    let current = app.player.current_track.as_ref().map(|t| t.track_id);
    let n = list.tracks.len();
    let header = common::header(
        title,
        row![
            tracks::play_buttons(list.ids()),
            common::count(n, "track", "tracks"),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    );
    let mut col = column![header].spacing(16).height(Length::Fill);
    if let Some(c) = q.and_then(chips) {
        col = col.push(c);
    }
    if let Some(e) = &list.error {
        col = col.push(common::error_text(e));
    }
    let key = app.route().scroll_key();
    let body: Element<'a, Message> = if list.loading {
        note("Loading…")
    } else if n == 0 {
        empty
    } else {
        let sortable = variant == Variant::Songs;
        let arrow = q.filter(|q| has_arrow(q)).map(sort_state);
        tracks::table(list, variant, sortable, arrow, current, key, &app.pages.scroll)
    };
    col = col.push(body);
    common::padded(col)
}

pub fn view_songs<'a>(app: &'a Halo, q: &'a TracksQuery) -> Element<'a, Message> {
    page(app, "All Songs", &app.pages.songs, Variant::Songs, Some(q), empty_library())
}

pub fn view_recent(app: &Halo) -> Element<'_, Message> {
    page(
        app,
        "Recently Played",
        &app.pages.recent,
        Variant::Recent,
        None,
        note("No recently played tracks."),
    )
}

pub fn view_most(app: &Halo) -> Element<'_, Message> {
    page(
        app,
        "Most Played",
        &app.pages.most,
        Variant::MostPlayed,
        None,
        note("No play history yet."),
    )
}
