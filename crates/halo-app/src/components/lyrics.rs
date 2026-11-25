//! Lyrics panel (`lyrics-panel.tsx`), shown over the page area: synced
//! (LRC) lyrics with the active line highlighted and kept centred,
//! click-to-seek; plain lyrics; auto-fetch online when nothing is cached;
//! manual edit (plain / LRC); and a provider search with candidate cards.

use std::time::Duration;

use halo_backend::commands::lyrics::{self, LyricsCandidate, LyricsLine, LyricsResult};
use iced::widget::{
    button, center, column, container, row, scrollable, text, text_editor, text_input, Space,
};
use iced::{Alignment, Element, Length, Padding, Subscription, Task};

use crate::app::{bg, Halo, Message, PlayerCmd};
use crate::icons::{icon, Icon};
use crate::theme::{self, a, t};
use crate::ui::{self, badge, hline, spin, spin_angle, Spinner};
use crate::widgets::{line, surface};

const SCROLL_ID: &str = "lyrics-scroll";
/// `text-lg leading-relaxed` + `gap-1` per line.
const LINE_H: f32 = 18.0 * 1.625 + 4.0;
const TOP_PAD: f32 = 48.0;

#[derive(Default)]
pub struct State {
    pub open: bool,
    track_id: Option<i64>,
    lyrics: Option<LyricsResult>,
    loading: bool,
    fetching: bool,
    not_found: bool,
    error: Option<String>,
    editing: bool,
    draft: text_editor::Content,
    saving: bool,
    searching: bool,
    form: [String; 3],
    candidates: Vec<LyricsCandidate>,
    search_done: bool,
    active: Option<usize>,
    viewport: f32,
}

#[derive(Debug, Clone)]
pub enum Msg {
    Local(i64, Result<LyricsResult, String>),
    Online(i64, Result<LyricsResult, String>),
    FetchOnline,
    EnterSearch,
    Cancel,
    Close,
    Form(usize, String),
    Search,
    Candidates(i64, Result<Vec<LyricsCandidate>, String>),
    Use(usize),
    StartEdit,
    Edit(text_editor::Action),
    Save,
    Saved(Result<(), String>),
    Seek(u64),
    Scrolled(f32),
}

fn lm(m: Msg) -> Message {
    Message::Lyrics(m)
}

fn source_label(s: &str) -> &str {
    match s {
        "database" => "From tag / saved",
        "lrc_file" => "From .lrc file",
        "lrclib" => "From LRCLIB",
        "jiosaavn" => "From JioSaavn",
        "lyricsovh" => "From Lyrics.ovh",
        other => other,
    }
}

fn provider_label(s: &str) -> &str {
    match s {
        "lrclib" => "LRCLIB",
        "jiosaavn" => "JioSaavn",
        "lyricsovh" => "Lyrics.ovh",
        other => other,
    }
}

fn lrc_time(ms: u64) -> String {
    let secs = ms / 1000;
    format!("{:02}:{:02}.{:02}", secs / 60, secs % 60, (ms % 1000) / 10)
}

fn has_lyrics(l: &Option<LyricsResult>) -> bool {
    l.as_ref()
        .is_some_and(|l| l.source != "none" && l.source != "not_found" && !l.lines.is_empty())
}

fn reset_search(st: &mut State) {
    st.candidates.clear();
    st.search_done = false;
    st.not_found = false;
    st.error = None;
}

fn load_local(app: &mut Halo, id: i64) -> Task<Message> {
    let st = &mut app.lyrics;
    st.loading = true;
    st.fetching = false;
    st.editing = false;
    st.searching = false;
    st.active = None;
    reset_search(st);
    bg(
        &app.backend,
        move |b| lyrics::get_lyrics(b, id),
        move |r| lm(Msg::Local(id, r)),
    )
}

fn fetch_online(app: &mut Halo, id: i64) -> Task<Message> {
    let st = &mut app.lyrics;
    st.fetching = true;
    st.not_found = false;
    st.error = None;
    bg(
        &app.backend,
        move |b| lyrics::fetch_lyrics_online(b, id),
        move |r| lm(Msg::Online(id, r)),
    )
}

/// What the full-screen now-playing view shows beside the cover.
pub enum Excerpt<'a> {
    Loading,
    /// Synced lyrics: `lines[range]` around the `active` line (None: the
    /// song hasn't reached the first line yet).
    Synced {
        lines: &'a [LyricsLine],
        range: std::ops::Range<usize>,
        active: Option<usize>,
    },
    Plain(&'a [LyricsLine]),
    None,
}

/// Lines shown before / after the active synced line.
pub const EXCERPT_BEFORE: usize = 2;
pub const EXCERPT_AFTER: usize = 3;

pub fn excerpt(app: &Halo) -> Excerpt<'_> {
    let st = &app.lyrics;
    if st.loading || (st.fetching && !has_lyrics(&st.lyrics)) {
        return Excerpt::Loading;
    }
    match st.lyrics.as_ref().filter(|_| has_lyrics(&st.lyrics)) {
        Some(l) if l.synced => Excerpt::Synced {
            lines: &l.lines,
            range: excerpt_range(l.lines.len(), st.active, EXCERPT_BEFORE, EXCERPT_AFTER),
            active: st.active,
        },
        Some(l) => Excerpt::Plain(&l.lines),
        None => Excerpt::None,
    }
}

/// Indices around `active`: `before` lines above, `after` below, clipped to
/// `0..len`. Before the first line: the first `after + 1` lines.
pub fn excerpt_range(len: usize, active: Option<usize>, before: usize, after: usize) -> std::ops::Range<usize> {
    match active {
        Some(i) => i.saturating_sub(before).min(len)..(i + after + 1).min(len),
        None => 0..(after + 1).min(len),
    }
}

/// Open the panel (the full-screen view's "Find lyrics").
pub fn open(app: &mut Halo) -> Task<Message> {
    if app.lyrics.open {
        return Task::none();
    }
    toggle(app)
}

/// Show / hide the panel; on open, re-centre the active synced line.
pub fn toggle(app: &mut Halo) -> Task<Message> {
    app.lyrics.open = !app.lyrics.open;
    app.lyrics.active = None;
    if app.lyrics.open {
        on_position(app)
    } else {
        Task::none()
    }
}

/// The current track changed.
pub fn on_track(app: &mut Halo, id: Option<i64>) -> Task<Message> {
    app.lyrics.track_id = id;
    match id {
        Some(id) => load_local(app, id),
        None => {
            let st = &mut app.lyrics;
            st.lyrics = None;
            st.editing = false;
            st.searching = false;
            reset_search(st);
            Task::none()
        }
    }
}

/// Player position update: move the active synced line and keep it centred.
pub fn on_position(app: &mut Halo) -> Task<Message> {
    let st = &mut app.lyrics;
    let Some(l) = st.lyrics.as_ref().filter(|l| l.synced && !l.lines.is_empty()) else {
        return Task::none();
    };
    let pos = app.player.player.position_ms;
    let idx = l.lines.iter().rposition(|ln| ln.time_ms <= pos);
    if idx == st.active {
        return Task::none();
    }
    st.active = idx;
    if !st.open || st.editing || st.searching {
        return Task::none();
    }
    let Some(i) = idx else { return Task::none() };
    let vp = if st.viewport > 0.0 { st.viewport } else { app.main_size.height - 88.0 - 32.0 - 45.0 };
    let y = (TOP_PAD + i as f32 * LINE_H + LINE_H / 2.0 - vp / 2.0).max(0.0);
    iced::widget::operation::scroll_to(
        SCROLL_ID,
        scrollable::AbsoluteOffset { x: None, y: Some(y) },
    )
}

/// Seek to a lyric line (also used by the full-screen view).
pub fn seek(ms: u64) -> Message {
    lm(Msg::Seek(ms))
}

pub fn subscription(app: &Halo) -> Option<Subscription<Message>> {
    let st = &app.lyrics;
    (st.open && (st.loading || st.fetching || st.saving))
        .then(|| iced::time::every(Duration::from_millis(16)).map(|_| Message::Noop))
}

pub fn update(app: &mut Halo, m: Msg) -> Task<Message> {
    let current = app.player.current_track.as_ref().map(|t| t.track_id);
    let st = &mut app.lyrics;
    match m {
        Msg::Local(id, r) => {
            if st.track_id != Some(id) {
                return Task::none();
            }
            st.loading = false;
            let res = r.unwrap_or(LyricsResult {
                source: "none".into(),
                synced: false,
                lines: vec![],
            });
            // Auto-fetch only when never searched ("" sentinel = searched).
            let auto = res.source == "none";
            st.lyrics = Some(res);
            if auto {
                fetch_online(app, id)
            } else {
                Task::none()
            }
        }
        Msg::Online(id, r) => {
            if st.track_id != Some(id) {
                return Task::none();
            }
            st.fetching = false;
            match r {
                Ok(res) => {
                    if res.source == "none" || res.source == "not_found" {
                        st.not_found = true;
                    }
                    st.lyrics = Some(res);
                }
                Err(e) => st.error = Some(e),
            }
            Task::none()
        }
        Msg::FetchOnline => match current {
            Some(id) => fetch_online(app, id),
            None => Task::none(),
        },
        Msg::EnterSearch => {
            let tr = app.player.current_track.as_ref();
            st.form = [
                tr.and_then(|t| t.title.clone()).unwrap_or_default(),
                tr.map(|t| t.artists.join(", ")).unwrap_or_default(),
                tr.and_then(|t| t.album_name.clone()).unwrap_or_default(),
            ];
            st.searching = true;
            reset_search(st);
            Task::none()
        }
        Msg::Cancel => {
            st.editing = false;
            st.searching = false;
            reset_search(st);
            Task::none()
        }
        Msg::Close => {
            st.open = false;
            Task::none()
        }
        Msg::Form(i, v) => {
            st.form[i] = v;
            Task::none()
        }
        Msg::Search => {
            let Some(id) = current else { return Task::none() };
            if st.form[0].trim().is_empty() || st.fetching {
                return Task::none();
            }
            st.fetching = true;
            st.candidates.clear();
            st.search_done = false;
            st.not_found = false;
            st.error = None;
            let [t_, a_, al] = st.form.clone();
            bg(
                &app.backend,
                move |b| lyrics::search_lyrics_providers(b, id, t_, a_, al),
                move |r| lm(Msg::Candidates(id, r)),
            )
        }
        Msg::Candidates(id, r) => {
            if st.track_id != Some(id) {
                return Task::none();
            }
            st.fetching = false;
            match r {
                Ok(c) => {
                    st.not_found = c.is_empty();
                    st.candidates = c;
                    st.search_done = true;
                }
                Err(e) => st.error = Some(e),
            }
            Task::none()
        }
        Msg::Use(i) => {
            let (Some(id), Some(c)) = (current, st.candidates.get(i)) else {
                return Task::none();
            };
            let text = c.lyrics.clone();
            bg(
                &app.backend,
                move |b| lyrics::save_lyrics(b.db(), id, text),
                |r| lm(Msg::Saved(r)),
            )
        }
        Msg::StartEdit => {
            let raw = st
                .lyrics
                .as_ref()
                .map(|l| {
                    l.lines
                        .iter()
                        .map(|ln| {
                            if l.synced {
                                format!("[{}]{}", lrc_time(ln.time_ms), ln.text)
                            } else {
                                ln.text.clone()
                            }
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                })
                .unwrap_or_default();
            st.draft = text_editor::Content::with_text(&raw);
            st.editing = true;
            Task::none()
        }
        Msg::Edit(action) => {
            st.draft.perform(action);
            Task::none()
        }
        Msg::Save => {
            let Some(id) = current else { return Task::none() };
            st.saving = true;
            let text = st.draft.text();
            bg(
                &app.backend,
                move |b| lyrics::save_lyrics(b.db(), id, text),
                |r| lm(Msg::Saved(r)),
            )
        }
        Msg::Saved(r) => {
            st.saving = false;
            match r {
                Ok(()) => match current {
                    // Reload (leaves edit / search mode).
                    Some(id) => load_local(app, id),
                    None => Task::none(),
                },
                Err(e) => {
                    // Keep editing open on error.
                    st.error = Some(e);
                    Task::none()
                }
            }
        }
        Msg::Seek(ms) => app.update(Message::Player(PlayerCmd::Seek(ms))),
        Msg::Scrolled(h) => {
            st.viewport = h;
            Task::none()
        }
    }
}

fn hbtn<'a>(i: Icon, label: &'static str, msg: Option<Message>) -> Element<'a, Message> {
    ui::tip(
        button(ui::icon_box(i, 14.0, 26.0, 26.0))
            .padding(0)
            .style(theme::btn_muted_square(theme::R_XS))
            .on_press_maybe(msg),
        label,
        iced::widget::tooltip::Position::Bottom,
    )
}

pub fn view(app: &Halo) -> Element<'_, Message> {
    let k = t();
    let st = &app.lyrics;
    let track = app.player.current_track.as_ref();
    let has = has_lyrics(&st.lyrics);

    // ── Header ────────────────────────────────────────────────────────────────
    let title = track
        .map(|t| t.title.clone().unwrap_or_else(|| "Unknown track".into()))
        .unwrap_or_else(|| "No track playing".into());
    let mut left = row![line(title).size(theme::BODY).font(theme::MEDIUM)]
        .spacing(12)
        .align_y(Alignment::Center);
    if has && !st.searching {
        left = left.push(ui::muted(
            source_label(&st.lyrics.as_ref().unwrap().source).to_string(),
            theme::SMALL,
        ));
    }
    let mut actions = row![].spacing(4).align_y(Alignment::Center);
    if track.is_some() && !st.editing && !st.searching {
        let fetch_icon: Element<'_, Message> = if st.fetching {
            center(spin(Spinner::Loader, 14.0, spin_angle(), a(k.muted_fg, 0.4)))
                .width(26)
                .height(26)
                .into()
        } else {
            hbtn(Icon::CloudDownload, "Fetch lyrics online", Some(lm(Msg::FetchOnline)))
        };
        actions = actions
            .push(fetch_icon)
            .push(hbtn(Icon::Search, "Search all providers", Some(lm(Msg::EnterSearch))))
            .push(hbtn(Icon::Pencil, "Edit lyrics", Some(lm(Msg::StartEdit))));
    }
    if st.editing || st.searching {
        actions = actions.push(hbtn(Icon::X, "Cancel", Some(lm(Msg::Cancel))));
    }
    if st.editing {
        let save: Element<'_, Message> = if st.saving {
            center(spin(Spinner::Loader, 14.0, spin_angle(), k.primary)).width(26).height(26).into()
        } else {
            button(ui::icon_box(Icon::Check, 14.0, 26.0, 26.0))
                .padding(0)
                .style(theme::btn_text(k.primary, a(k.primary, 0.8)))
                .on_press(lm(Msg::Save))
                .into()
        };
        actions = actions.push(save);
    }
    if !st.editing && !st.searching {
        actions = actions.push(hbtn(Icon::X, "Close lyrics", Some(lm(Msg::Close))));
    }
    let header = container(
        row![container(left).width(Length::Fill), actions]
            .spacing(16)
            .align_y(Alignment::Center),
    )
    .padding(Padding::from([12.0, 24.0]))
    .height(45);

    // ── Body ──────────────────────────────────────────────────────────────────
    let body: Element<'_, Message> = if st.loading {
        center(spin(Spinner::Loader, 20.0, spin_angle(), k.muted_fg)).into()
    } else if st.editing {
        column![
            row![
                ui::muted("Plain text or LRC format (e.g. ", theme::SMALL),
                text("[00:12.50]Line text").size(theme::SMALL).font(theme::MONO).color(k.muted_fg),
                ui::muted(")", theme::SMALL),
            ],
            text_editor(&st.draft)
                .on_action(|a| lm(Msg::Edit(a)))
                .placeholder("Paste lyrics here...")
                .font(theme::MONO)
                .size(theme::BODY)
                .padding(12)
                .height(Length::Fill)
                .style(theme::editor),
        ]
        .spacing(12)
        .padding(24)
        .height(Length::Fill)
        .into()
    } else if st.searching {
        search_view(app)
    } else if !has {
        let mut col = column![].spacing(12).align_x(Alignment::Center);
        if st.fetching {
            col = col
                .push(spin(Spinner::Loader, 20.0, spin_angle(), k.muted_fg))
                .push(ui::muted("Searching for lyrics…", theme::BODY));
        } else {
            let nf = st.not_found || st.lyrics.as_ref().is_some_and(|l| l.source == "not_found");
            col = col.push(ui::muted(
                if nf { "No lyrics found online." } else { "No lyrics found." },
                theme::BODY,
            ));
            if let Some(e) = &st.error {
                col = col.push(
                    container(text(e.as_str()).size(theme::SMALL).color(k.destructive))
                        .max_width(320),
                );
            }
            if track.is_some() {
                col = col.push(
                    column![
                        button(
                            row![icon(Icon::Search, 14.0), text("Search all providers").size(theme::BODY)]
                                .spacing(8)
                                .align_y(Alignment::Center),
                        )
                        .padding(Padding::from([8.0, 16.0]))
                        .style(|th, s| {
                            let mut st = theme::btn_outline(th, s);
                            st.background = matches!(s, button::Status::Hovered).then(|| t().muted.into());
                            st.border.color = t().border;
                            st.border.radius = theme::radius(theme::R_FULL);
                            st
                        })
                        .on_press(lm(Msg::EnterSearch)),
                        button(text("Add lyrics manually").size(theme::SMALL))
                            .padding(0)
                            .style(theme::btn_link)
                            .on_press(lm(Msg::StartEdit)),
                    ]
                    .spacing(8)
                    .align_x(Alignment::Center),
                );
            }
        }
        center(col).into()
    } else {
        lines_view(app)
    };

    column![header, hline(), container(body).width(Length::Fill).height(Length::Fill)]
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn lines_view(app: &Halo) -> Element<'_, Message> {
    let k = t();
    let st = &app.lyrics;
    let l = st.lyrics.as_ref().unwrap();
    let mut col = column![].spacing(4).align_x(Alignment::Center).width(Length::Fill);
    for (i, ln) in l.lines.iter().enumerate() {
        let active = l.synced && Some(i) == st.active;
        let past = l.synced && st.active.is_some_and(|a_| i < a_);
        let color = if active {
            k.foreground
        } else if past {
            a(k.muted_fg, 0.5)
        } else {
            k.muted_fg
        };
        let txt = text(ln.text.as_str())
            .size(if active { theme::TITLE * 1.05 } else { theme::TITLE })
            .font(if active { theme::SEMIBOLD } else { theme::SANS })
            .line_height(1.625)
            .color(color)
            .align_x(iced::alignment::Horizontal::Center);
        let el: Element<'_, Message> = if l.synced {
            let ms = ln.time_ms;
            surface(container(txt).max_width(672))
                .on_press(lm(Msg::Seek(ms)))
                .interaction(iced::mouse::Interaction::Pointer)
                .into()
        } else {
            container(txt).max_width(672).into()
        };
        col = col.push(el);
    }
    col = col.push(Space::new().height(256));
    scrollable(container(col).padding(Padding::from([TOP_PAD, 32.0])).width(Length::Fill))
        .id(SCROLL_ID)
        .on_scroll(|vp| lm(Msg::Scrolled(vp.bounds().height)))
        .direction(theme::vscroll())
        .style(theme::scroll)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn search_view(app: &Halo) -> Element<'_, Message> {
    let k = t();
    let st = &app.lyrics;
    let mut form = column![].spacing(12);
    for (i, label) in ["Title", "Artist", "Album (optional)"].into_iter().enumerate() {
        form = form.push(
            column![
                ui::muted(label, theme::SMALL),
                text_input("", &st.form[i])
                    .on_input(move |v| lm(Msg::Form(i, v)))
                    .on_submit(lm(Msg::Search))
                    .size(theme::BODY)
                    .padding(Padding::from([8.0, 12.0]))
                    .style(theme::input_muted),
            ]
            .spacing(4),
        );
    }
    let can = !st.fetching && !st.form[0].trim().is_empty();
    let lead: Element<'_, Message> = if st.fetching {
        spin(Spinner::Loader, 14.0, spin_angle(), k.primary_fg).into()
    } else {
        icon(Icon::Search, 14.0).into()
    };
    form = form.push(
        button(
            row![
                lead,
                text(if st.fetching { "Searching all providers…" } else { "Search all providers" })
                    .size(theme::BODY)
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        )
        .padding(Padding::from([8.0, 16.0]))
        .style(|th, s| {
            let mut st = theme::btn_primary(th, s);
            st.border.radius = theme::radius(theme::R_FULL);
            st
        })
        .on_press_maybe(can.then(|| lm(Msg::Search))),
    );

    let mut col = column![form].spacing(16);
    if let Some(e) = &st.error {
        col = col.push(text(e.as_str()).size(theme::SMALL).color(k.destructive));
    }
    if st.search_done && st.candidates.is_empty() && st.error.is_none() {
        col = col.push(ui::muted("No results found — try different values.", theme::BODY));
    }
    if !st.candidates.is_empty() {
        let n = st.candidates.len();
        let mut list = column![ui::muted(
            format!("{n} result{} — pick one to use:", if n != 1 { "s" } else { "" }),
            theme::SMALL,
        )]
        .spacing(12);
        for (i, c) in st.candidates.iter().enumerate() {
            let mut tags = row![badge(provider_label(&c.provider).to_string())]
                .spacing(8)
                .align_y(Alignment::Center);
            if c.synced {
                tags = tags.push(text("Synced").size(theme::SMALL).font(theme::MEDIUM).color(k.primary));
            }
            if !c.label.is_empty() {
                tags = tags.push(
                    line(c.label.as_str())
                        .size(theme::SMALL)
                        .color(a(k.foreground, 0.8))
                        .width(Length::Fill),
                );
            }
            let snippet = if c.snippet.is_empty() {
                "(no preview)".to_string()
            } else {
                format!("{}…", c.snippet)
            };
            list = list.push(
                container(
                    column![
                        tags,
                        text(snippet)
                            .size(theme::SMALL)
                            .font(theme::MONO)
                            .line_height(1.625)
                            .color(k.muted_fg),
                        container(
                            button(text("Use this").size(theme::SMALL))
                                .padding(Padding::from([6.0, 12.0]))
                                .style(|th, s| {
                                    let mut st = theme::btn_primary(th, s);
                                    st.border.radius = theme::radius(theme::R_FULL);
                                    st
                                })
                                .on_press(lm(Msg::Use(i))),
                        )
                        .width(Length::Fill)
                        .align_x(Alignment::End),
                    ]
                    .spacing(8),
                )
                .padding(12)
                .style(theme::bordered(theme::R_LG)),
            );
        }
        col = col.push(list);
    }
    scrollable(container(col).padding(20))
        .direction(theme::vscroll())
        .style(theme::scroll)
        .height(Length::Fill)
        .into()
}

#[cfg(test)]
mod tests {
    use super::excerpt_range;

    #[test]
    fn excerpt_range_around_active_line() {
        // Before the first line: the first four lines.
        assert_eq!(excerpt_range(10, None, 2, 3), 0..4);
        // Start: nothing above line 0.
        assert_eq!(excerpt_range(10, Some(0), 2, 3), 0..4);
        // Middle: two above, three below.
        assert_eq!(excerpt_range(10, Some(5), 2, 3), 3..9);
        // End: clipped at the last line.
        assert_eq!(excerpt_range(10, Some(9), 2, 3), 7..10);
        // Short song.
        assert_eq!(excerpt_range(2, None, 2, 3), 0..2);
        assert_eq!(excerpt_range(0, None, 2, 3), 0..0);
    }
}
