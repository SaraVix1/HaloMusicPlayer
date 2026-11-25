//! Smart playlist detail + rule editor (`smart-playlist-detail.tsx`).

use halo_backend::commands::smart_playlists::{self, SmartPlaylist, SmartPlaylistRule, SmartTrack};
use iced::widget::{button, column, container, pick_list, row, text, text_input};
use iced::{Alignment, Element, Length, Padding, Task};

use super::common::{self, note};
use super::playlists::BUILTIN_IDS;
use super::Msg;
use crate::app::{bg, Halo, Message, PlayerCmd};
use crate::format::{duration, plural};
use crate::icons::{icon, Icon};
use crate::route::Route;
use crate::theme::{self, a, t};
use crate::ui::{self, art, icon_box};
use crate::widgets::{line, surface};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldType {
    Text,
    Number,
    Date,
}

/// A select option (value + label) for `pick_list`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Opt {
    pub value: &'static str,
    pub label: &'static str,
}

impl std::fmt::Display for Opt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label)
    }
}

const fn o(value: &'static str, label: &'static str) -> Opt {
    Opt { value, label }
}

pub const FIELDS: [(Opt, FieldType); 13] = [
    (o("title", "Title"), FieldType::Text),
    (o("artist", "Artist"), FieldType::Text),
    (o("album", "Album"), FieldType::Text),
    (o("album_artist", "Album artist"), FieldType::Text),
    (o("genre", "Genre"), FieldType::Text),
    (o("composer", "Composer"), FieldType::Text),
    (o("year", "Year"), FieldType::Number),
    (o("rating", "Rating (0–5)"), FieldType::Number),
    (o("play_count", "Play count"), FieldType::Number),
    (o("skip_count", "Skip count"), FieldType::Number),
    (o("duration_ms", "Duration (ms)"), FieldType::Number),
    (o("date_added", "Date added"), FieldType::Date),
    (o("last_played_at", "Last played"), FieldType::Date),
];

const OPS_TEXT: [Opt; 4] = [
    o("contains", "contains"),
    o("not_contains", "doesn't contain"),
    o("is", "is"),
    o("is_not", "is not"),
];
const OPS_NUMBER: [Opt; 6] = [
    o("eq", "="),
    o("not_eq", "≠"),
    o("gt", ">"),
    o("gte", ">="),
    o("lt", "<"),
    o("lte", "<="),
];
const OPS_DATE: [Opt; 3] = [
    o("in_last_days", "in the last (days)"),
    o("gt", "after"),
    o("lt", "before"),
];

pub const SORT_FIELDS: [Opt; 9] = [
    o("title", "Title"),
    o("artist", "Artist"),
    o("album", "Album"),
    o("year", "Year"),
    o("rating", "Rating"),
    o("play_count", "Play count"),
    o("duration_ms", "Duration"),
    o("date_added", "Date added"),
    o("last_played_at", "Last played"),
];
const SORT_DIRS: [Opt; 2] = [o("asc", "Ascending"), o("desc", "Descending")];

static FIELD_OPTS: std::sync::LazyLock<Vec<Opt>> =
    std::sync::LazyLock::new(|| FIELDS.iter().map(|(o, _)| *o).collect());

pub fn ops_for(field: &str) -> &'static [Opt] {
    match FIELDS.iter().find(|(o, _)| o.value == field).map(|(_, t)| *t) {
        Some(FieldType::Number) => &OPS_NUMBER,
        Some(FieldType::Date) => &OPS_DATE,
        _ => &OPS_TEXT,
    }
}

fn find(opts: &[Opt], v: &str) -> Option<Opt> {
    opts.iter().find(|o| o.value == v).copied()
}

#[derive(Debug, Clone)]
pub struct DraftRule {
    pub field: String,
    pub operator: String,
    pub value: String,
}

#[derive(Default)]
pub struct SmartState {
    pub id: Option<i64>,
    pub playlist: Option<SmartPlaylist>,
    pub tracks: Vec<SmartTrack>,
    pub loading: bool,
    pub error: Option<String>,
    pub editing: bool,
    pub name: String,
    pub match_mode: String,
    pub rules: Vec<DraftRule>,
    pub sort_field: String,
    pub sort_dir: String,
    pub limit: String,
    pub saving: bool,
    pub confirm_delete: bool,
}

#[derive(Debug, Clone)]
pub enum SMsg {
    Loaded(i64, Result<(SmartPlaylist, Vec<SmartPlaylistRule>, Vec<SmartTrack>), String>),
    TracksLoaded(i64, Result<Vec<SmartTrack>, String>),
    Refresh,
    ToggleEdit,
    Name(String),
    Match(String),
    RuleField(usize, Opt),
    RuleOp(usize, Opt),
    RuleValue(usize, String),
    RemoveRule(usize),
    AddRule,
    SortField(Opt),
    SortDir(Opt),
    Limit(String),
    Save,
    Saved(Result<(SmartPlaylist, Vec<SmartTrack>), String>),
    AskDelete(bool),
    Delete,
    Deleted(Result<(), String>),
    Play(usize),
    Shuffle,
}

fn sm(m: SMsg) -> Message {
    Message::Page(Msg::Smart(m))
}

pub fn load(app: &mut Halo, id: i64) -> Task<Message> {
    let st = &mut app.pages.smart;
    if st.id != Some(id) {
        *st = SmartState {
            id: Some(id),
            ..SmartState::default()
        };
    }
    st.loading = true;
    st.error = None;
    bg(
        &app.backend,
        move |b| {
            let (p, rules) = smart_playlists::get_smart_playlist(b.db(), id)?;
            let tracks = smart_playlists::get_smart_playlist_tracks(b.db(), id)?;
            Ok((p, rules, tracks))
        },
        move |r| sm(SMsg::Loaded(id, r)),
    )
}

/// Library changed: refetch only the matched tracks (keeps an in-progress
/// rule edit).
pub fn reload_tracks(app: &mut Halo) -> Task<Message> {
    let Some(id) = app.pages.smart.id else { return Task::none() };
    bg(
        &app.backend,
        move |b| smart_playlists::get_smart_playlist_tracks(b.db(), id),
        move |r| sm(SMsg::TracksLoaded(id, r)),
    )
}

pub fn update(app: &mut Halo, msg: SMsg) -> Task<Message> {
    let st = &mut app.pages.smart;
    match msg {
        SMsg::Loaded(id, r) => {
            if st.id != Some(id) {
                return Task::none();
            }
            st.loading = false;
            match r {
                Ok((p, rules, tracks)) => {
                    st.name = p.name.clone();
                    st.match_mode = p.match_mode.clone();
                    st.rules = rules
                        .into_iter()
                        .map(|r| DraftRule {
                            field: r.field,
                            operator: r.operator,
                            value: r.value,
                        })
                        .collect();
                    st.sort_field = p.sort_field.clone();
                    st.sort_dir = p.sort_direction.clone();
                    st.limit = p.limit_count.map(|l| l.to_string()).unwrap_or_default();
                    st.playlist = Some(p);
                    st.tracks = tracks;
                }
                Err(e) => st.error = Some(e),
            }
            Task::none()
        }
        SMsg::TracksLoaded(id, r) => {
            if st.id == Some(id) {
                if let Ok(t) = r {
                    st.tracks = t;
                }
            }
            Task::none()
        }
        SMsg::Refresh => match st.id {
            Some(id) => load(app, id),
            None => Task::none(),
        },
        SMsg::ToggleEdit => {
            st.editing = !st.editing;
            Task::none()
        }
        SMsg::Name(s) => {
            st.name = s;
            Task::none()
        }
        SMsg::Match(m) => {
            st.match_mode = m;
            Task::none()
        }
        SMsg::RuleField(i, f) => {
            if let Some(r) = st.rules.get_mut(i) {
                r.field = f.value.into();
                r.operator = ops_for(f.value)[0].value.into();
                r.value.clear();
            }
            Task::none()
        }
        SMsg::RuleOp(i, op) => {
            if let Some(r) = st.rules.get_mut(i) {
                r.operator = op.value.into();
            }
            Task::none()
        }
        SMsg::RuleValue(i, v) => {
            if let Some(r) = st.rules.get_mut(i) {
                r.value = v;
            }
            Task::none()
        }
        SMsg::RemoveRule(i) => {
            if i < st.rules.len() {
                st.rules.remove(i);
            }
            Task::none()
        }
        SMsg::AddRule => {
            st.rules.push(DraftRule {
                field: "title".into(),
                operator: "contains".into(),
                value: String::new(),
            });
            Task::none()
        }
        SMsg::SortField(f) => {
            st.sort_field = f.value.into();
            Task::none()
        }
        SMsg::SortDir(d) => {
            st.sort_dir = d.value.into();
            Task::none()
        }
        SMsg::Limit(s) => {
            st.limit = s.chars().filter(|c| c.is_ascii_digit()).collect();
            Task::none()
        }
        SMsg::Save => {
            let Some(id) = st.id else { return Task::none() };
            st.saving = true;
            let name = st.name.clone();
            let mm = st.match_mode.clone();
            let sf = st.sort_field.clone();
            let sd = st.sort_dir.clone();
            let limit = st.limit.trim().parse::<i64>().ok().filter(|l| *l > 0);
            let rules: Vec<SmartPlaylistRule> = st
                .rules
                .iter()
                .enumerate()
                .map(|(i, r)| SmartPlaylistRule {
                    id: 0,
                    playlist_id: id,
                    field: r.field.clone(),
                    operator: r.operator.clone(),
                    value: r.value.clone(),
                    position: i as i64,
                })
                .collect();
            bg(
                &app.backend,
                move |b| {
                    let p = smart_playlists::update_smart_playlist(b.db(), id, name, mm, sf, sd, limit)?;
                    smart_playlists::set_smart_playlist_rules(b.db(), id, rules)?;
                    let t = smart_playlists::get_smart_playlist_tracks(b.db(), id)?;
                    Ok((p, t))
                },
                |r| sm(SMsg::Saved(r)),
            )
        }
        SMsg::Saved(r) => {
            st.saving = false;
            match r {
                Ok((p, t)) => {
                    st.playlist = Some(p);
                    st.tracks = t;
                    st.editing = false;
                    app.pages.playlists.loaded = false;
                }
                Err(e) => st.error = Some(e),
            }
            Task::none()
        }
        SMsg::AskDelete(b) => {
            st.confirm_delete = b;
            Task::none()
        }
        SMsg::Delete => {
            let Some(id) = st.id else { return Task::none() };
            bg(
                &app.backend,
                move |b| smart_playlists::delete_smart_playlist(b.db(), id),
                |r| sm(SMsg::Deleted(r)),
            )
        }
        SMsg::Deleted(r) => match r {
            Ok(()) => {
                app.pages.playlists.loaded = false;
                app.update(Message::Navigate(Route::Playlists))
            }
            Err(e) => {
                st.error = Some(e);
                Task::none()
            }
        },
        SMsg::Play(i) => {
            let ids: Vec<i64> = st.tracks.iter().map(|t| t.id).collect();
            if ids.is_empty() {
                return Task::none();
            }
            app.update(Message::Player(PlayerCmd::PlayTracks(ids, i as i64)))
        }
        SMsg::Shuffle => {
            if st.tracks.is_empty() {
                return Task::none();
            }
            let i = crate::app::random_index(st.tracks.len());
            app.update(Message::Page(Msg::Smart(SMsg::Play(i))))
        }
    }
}

fn select<'a>(
    opts: &'a [Opt],
    selected: Option<Opt>,
    disabled: bool,
    f: impl Fn(Opt) -> Message + 'a,
) -> Element<'a, Message> {
    let pl = pick_list(opts, selected, f)
        .text_size(theme::BODY)
        .padding(Padding::from([4.0, 8.0]))
        .style(theme::pick)
        .menu_style(theme::pick_menu);
    if disabled {
        // A pick_list can't be disabled; render it inert at 60% opacity.
        let label = selected.map(|o| o.label).unwrap_or("");
        container(text(label).size(theme::BODY).color(a(t().foreground, 0.6)))
            .padding(Padding::from([4.0, 8.0]))
            .style(theme::bordered(theme::R_MD))
            .into()
    } else {
        pl.into()
    }
}

fn input<'a>(
    placeholder: &'a str,
    value: &'a str,
    disabled: bool,
    f: impl Fn(String) -> Message + 'a,
) -> text_input::TextInput<'a, Message> {
    let mut i = text_input(placeholder, value)
        .size(theme::BODY)
        .padding(Padding::from([4.0, 8.0]))
        .style(theme::input);
    if !disabled {
        i = i.on_input(f);
    }
    i
}

pub fn view(app: &Halo) -> Element<'_, Message> {
    let k = t();
    let st = &app.pages.smart;
    if st.loading && st.playlist.is_none() {
        return common::padded(column![note("Loading…")]);
    }
    let Some(p) = &st.playlist else {
        return common::padded(column![common::error_text(
            st.error.as_deref().unwrap_or("Not found")
        )]);
    };
    let builtin = BUILTIN_IDS.contains(&p.id);

    // ── Header ────────────────────────────────────────────────────────────────
    let mut sub = plural(st.tracks.len() as i64, "track", "tracks");
    if let Some(l) = p.limit_count {
        sub.push_str(&format!(" · limit {l}"));
    }
    let edit_label = match (builtin, st.editing) {
        (false, false) => "Edit rules",
        (false, true) => "Cancel",
        (true, false) => "View rules",
        (true, true) => "Close",
    };
    let editing = st.editing;
    let header = row![
        button(icon(Icon::ChevronLeft, 20.0))
            .padding(0)
            .style(theme::btn_icon)
            .on_press(Message::Navigate(Route::Playlists)),
        column![
            line(p.name.as_str()).size(theme::TITLE).font(theme::SEMIBOLD).width(Length::Fill),
            ui::muted(sub, theme::BODY),
        ]
        .width(Length::Fill),
        button(icon_box(Icon::RefreshCw, 16.0, 28.0, 28.0))
            .padding(0)
            .style(theme::btn_icon)
            .on_press(sm(SMsg::Refresh)),
        button(text(edit_label).size(theme::SMALL))
            .padding(Padding::from([4.0, 12.0]))
            .style(move |th, s| {
                let k = t();
                let mut st = theme::btn_outline(th, s);
                st.background = None;
                st.border.radius = theme::radius(theme::R_MD);
                if editing {
                    st.border.color = k.primary;
                    st.text_color = k.primary;
                } else {
                    st.border.color = k.border;
                    if matches!(s, button::Status::Hovered) {
                        st.background = Some(a(k.muted, 0.4).into());
                    }
                }
                st
            })
            .on_press(sm(SMsg::ToggleEdit)),
    ]
    .spacing(12)
    .align_y(Alignment::Center);

    let mut col = column![header].spacing(24);
    if let Some(e) = &st.error {
        col = col.push(common::error_text(e));
    }

    // ── Rule editor ───────────────────────────────────────────────────────────
    if st.editing {
        let mut match_btns = row![ui::muted("Match", theme::BODY)]
            .spacing(8)
            .align_y(Alignment::Center);
        for (m, label) in [("all", "All rules"), ("any", "Any rule")] {
            let active = st.match_mode == m;
            match_btns = match_btns.push(
                button(text(label).size(theme::SMALL))
                    .padding(Padding::from([4.0, 10.0]))
                    .style(theme::btn_chip(active, theme::R_MD))
                    .on_press_maybe((!builtin).then(|| sm(SMsg::Match(m.into())))),
            );
        }
        let name_row = row![
            input("Playlist name", &st.name, builtin, |s| sm(SMsg::Name(s))).width(Length::Fill),
            match_btns,
        ]
        .spacing(12)
        .align_y(Alignment::Center);

        let mut rules = column![].spacing(8);
        for (i, r) in st.rules.iter().enumerate() {
            let fields: &'static [Opt] = FIELD_OPTS.as_slice();
            let ops = ops_for(&r.field);
            let mut rr = row![
                select(fields, find(fields, &r.field), builtin, move |f| sm(SMsg::RuleField(i, f))),
                select(ops, find(ops, &r.operator), builtin, move |op| sm(SMsg::RuleOp(i, op))),
                input("value", &r.value, builtin, move |v| sm(SMsg::RuleValue(i, v))).width(112),
            ]
            .spacing(8)
            .align_y(Alignment::Center);
            if !builtin {
                rr = rr.push(
                    button(icon(Icon::Trash2, 14.0))
                        .padding(0)
                        .style(theme::btn_text(k.muted_fg, k.destructive))
                        .on_press(sm(SMsg::RemoveRule(i))),
                );
            }
            rules = rules.push(rr);
        }
        if st.rules.is_empty() {
            rules = rules.push(ui::muted("No rules — all tracks will match.", theme::BODY));
        }

        let mut editor = column![name_row, rules].spacing(16);
        if !builtin {
            editor = editor.push(
                button(
                    row![icon(Icon::Plus, 14.0), text("Add rule").size(theme::BODY)]
                        .spacing(6)
                        .align_y(Alignment::Center),
                )
                .padding(0)
                .style(theme::btn_icon)
                .on_press(sm(SMsg::AddRule)),
            );
        }
        let sort_row = row![
            ui::muted("Sort by", theme::BODY),
            select(&SORT_FIELDS, find(&SORT_FIELDS, &st.sort_field), builtin, |f| sm(SMsg::SortField(f))),
            select(&SORT_DIRS, find(&SORT_DIRS, &st.sort_dir), builtin, |d| sm(SMsg::SortDir(d))),
            ui::muted("Limit", theme::BODY),
            input("None", &st.limit, builtin, |s| sm(SMsg::Limit(s))).width(80),
        ]
        .spacing(12)
        .align_y(Alignment::Center);
        editor = editor.push(ui::hline()).push(sort_row);

        if !builtin {
            let can_save = !st.saving && !st.name.trim().is_empty();
            let save = ui::label_button(
                None,
                if st.saving { "Saving…" } else { "Save" },
                theme::BODY,
                32.0,
                16.0,
                |th, s| {
                    let mut st = theme::btn_primary(th, s);
                    st.border.radius = theme::radius(theme::R_MD);
                    st
                },
                can_save.then(|| sm(SMsg::Save)),
            );
            let del: Element<'_, Message> = if !st.confirm_delete {
                button(text("Delete playlist").size(theme::BODY))
                    .padding(0)
                    .style(theme::btn_text(k.muted_fg, k.destructive))
                    .on_press(sm(SMsg::AskDelete(true)))
                    .into()
            } else {
                row![
                    text("Delete?").size(theme::BODY).color(k.destructive),
                    button(text("Yes").size(theme::BODY))
                        .padding(0)
                        .style(theme::btn_text(k.destructive, a(k.destructive, 0.8)))
                        .on_press(sm(SMsg::Delete)),
                    button(text("Cancel").size(theme::BODY))
                        .padding(0)
                        .style(theme::btn_link)
                        .on_press(sm(SMsg::AskDelete(false))),
                ]
                .spacing(8)
                .align_y(Alignment::Center)
                .into()
            };
            editor = editor
                .push(ui::hline())
                .push(row![save, del].spacing(12).align_y(Alignment::Center));
        }
        col = col.push(
            container(editor)
                .padding(16)
                .width(Length::Fill)
                .style(theme::bordered(theme::R_LG)),
        );
    }

    // ── Playback buttons + tracks ────────────────────────────────────────────
    if !st.tracks.is_empty() {
        let k2 = k;
        col = col.push(
            row![
                button(
                    row![
                        ui::filled(ui::Filled::Play, 14.0, k2.primary_fg),
                        text("Play all").size(theme::BODY)
                    ]
                    .spacing(6)
                    .align_y(Alignment::Center),
                )
                .padding(Padding::from([6.0, 12.0]))
                .style(|th, s| {
                    let mut st = theme::btn_primary(th, s);
                    st.border.radius = theme::radius(theme::R_MD);
                    st
                })
                .on_press(sm(SMsg::Play(0))),
                button(
                    row![icon(Icon::Shuffle, 14.0), text("Shuffle").size(theme::BODY)]
                        .spacing(6)
                        .align_y(Alignment::Center),
                )
                .padding(Padding::from([6.0, 12.0]))
                .style(|th, s| {
                    let mut st = theme::btn_outline(th, s);
                    st.border.radius = theme::radius(theme::R_MD);
                    st
                })
                .on_press(sm(SMsg::Shuffle)),
            ]
            .spacing(8),
        );
        let mut list = column![].spacing(2);
        for (i, tr) in st.tracks.iter().enumerate() {
            let title = tr.title.clone().unwrap_or_else(|| {
                tr.file_path.rsplit(['/', '\\']).next().unwrap_or(&tr.file_path).to_string()
            });
            let mut sub = tr.artists.join(", ");
            if let Some(al) = &tr.album_name {
                sub.push_str(&format!(" · {al}"));
            }
            let content = row![
                art(tr.album_art_path.as_deref(), 36.0, theme::R_XS),
                column![
                    line(title).size(theme::BODY).width(Length::Fill),
                    line(sub).size(theme::SMALL).color(k.muted_fg).width(Length::Fill),
                ]
                .width(Length::Fill),
                text(duration(tr.duration_ms)).size(theme::SMALL).color(k.muted_fg),
            ]
            .spacing(12)
            .align_y(Alignment::Center);
            list = list.push(
                surface(content)
                    .width(Length::Fill)
                    .padding(Padding::from([6.0, 8.0]))
                    .on_double_click(sm(SMsg::Play(i)))
                    .on_right_press_at(crate::components::context_menu::opener(
                        crate::components::context_menu::Target::Track {
                            id: tr.id,
                            album: tr.album_name.clone(),
                            artists: tr.artists.clone(),
                            playlist: None,
                        },
                    ))
                    .style(|s| iced::widget::container::Style {
                        background: s.hovered.then(|| a(t().muted, 0.3).into()),
                        border: theme::no_border(theme::R_MD),
                        ..Default::default()
                    }),
            );
        }
        col = col.push(list);
    } else {
        col = col.push(note("No tracks match this playlist's rules."));
    }

    common::scroll_page(
        app.route().scroll_key(),
        container(container(col).max_width(768)).padding(24),
    )
}
