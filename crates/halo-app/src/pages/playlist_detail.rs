//! Manual playlist detail (`playlist-detail.tsx`): header with inline
//! rename, Play / Shuffle / Delete, and the track list with drag-to-reorder
//! (grip handle, 40% opacity on the dragged row, 2px primary drop line),
//! hover remove, row play/pause and double-click to play.

use halo_backend::commands::playlists::{self, Playlist, PlaylistTrack};
use iced::widget::{button, center, column, container, hover, mouse_area, row, text, text_input, Space};
use iced::{Alignment, Element, Length, Padding, Task};

use super::common::{self, note};
use super::dialogs::{Confirm, ConfirmAction};
use super::Msg;
use crate::app::{bg, Halo, Message, PlayerCmd};
use crate::format::{duration, plural};
use crate::icons::{icon, Icon};
use crate::route::Route;
use crate::theme::{self, a, t};
use crate::ui::{self, art, icon_box, small_button};
use crate::widgets::{line, surface};

pub const RENAME_INPUT: &str = "playlist-rename";

#[derive(Default)]
pub struct DetailState {
    pub id: Option<i64>,
    pub playlist: Option<Playlist>,
    pub tracks: Vec<PlaylistTrack>,
    pub loading: bool,
    pub error: Option<String>,
    pub editing: bool,
    pub edit_name: String,
    pub drag_id: Option<i64>,
    pub over_index: Option<usize>,
}

#[derive(Debug, Clone)]
pub enum DMsg {
    Loaded(i64, Result<(Playlist, Vec<PlaylistTrack>), String>),
    StartRename,
    RenameChanged(String),
    SaveRename,
    CancelRename,
    Delete,
    Deleted(Result<(), String>),
    Remove(i64),
    RowPlay(usize, i64),
    DragStart(i64),
    DragOver(usize),
    DragEnd,
    Reordered(Result<(), String>),
    Done(Result<(), String>),
}

fn dm(m: DMsg) -> Message {
    Message::Page(Msg::PlaylistDetail(m))
}

pub fn load(app: &mut Halo, id: i64) -> Task<Message> {
    let st = &mut app.pages.playlist;
    if st.id != Some(id) {
        *st = DetailState {
            id: Some(id),
            loading: true,
            ..DetailState::default()
        };
    }
    bg(
        &app.backend,
        move |b| playlists::get_playlist(id, b.db()),
        move |r| dm(DMsg::Loaded(id, r)),
    )
}

fn refresh(app: &mut Halo) -> Task<Message> {
    match app.pages.playlist.id {
        Some(id) => load(app, id),
        None => Task::none(),
    }
}

pub fn update(app: &mut Halo, msg: DMsg) -> Task<Message> {
    let st = &mut app.pages.playlist;
    match msg {
        DMsg::Loaded(id, r) => {
            if st.id != Some(id) {
                return Task::none();
            }
            st.loading = false;
            match r {
                Ok((p, ts)) => {
                    st.playlist = Some(p);
                    st.tracks = ts;
                    st.error = None;
                }
                Err(e) => st.error = Some(e),
            }
            Task::none()
        }
        DMsg::StartRename => {
            st.editing = true;
            st.edit_name = st.playlist.as_ref().map(|p| p.name.clone()).unwrap_or_default();
            iced::widget::operation::focus(RENAME_INPUT)
        }
        DMsg::RenameChanged(s) => {
            st.edit_name = s;
            Task::none()
        }
        DMsg::CancelRename => {
            st.editing = false;
            Task::none()
        }
        DMsg::SaveRename => {
            let name = st.edit_name.trim().to_string();
            let Some(id) = st.id else { return Task::none() };
            if name.is_empty() {
                return Task::none();
            }
            st.editing = false;
            bg(
                &app.backend,
                move |b| playlists::rename_playlist(id, name, b.db()),
                |r| dm(DMsg::Done(r)),
            )
        }
        DMsg::Delete => {
            let Some(p) = &st.playlist else { return Task::none() };
            let c = Confirm {
                title: "Delete playlist".into(),
                body: format!("Delete \"{}\"?", p.name),
                confirm_label: "Delete".into(),
                destructive: true,
                action: ConfirmAction::DeletePlaylist(p.id),
            };
            super::dialogs::open_confirm(app, c)
        }
        DMsg::Deleted(r) => match r {
            Ok(()) => {
                app.pages.playlists.loaded = false;
                app.update(Message::Navigate(Route::Playlists))
            }
            Err(e) => {
                st.error = Some(e);
                Task::none()
            }
        },
        DMsg::Remove(ptid) => bg(
            &app.backend,
            move |b| playlists::remove_from_playlist(ptid, b.db()),
            |r| dm(DMsg::Done(r)),
        ),
        DMsg::RowPlay(idx, track_id) => {
            let current = app.player.current_track.as_ref().map(|t| t.track_id);
            if current == Some(track_id) {
                app.update(Message::Player(PlayerCmd::TogglePlay))
            } else {
                let ids = st.tracks.iter().map(|t| t.track_id).collect();
                app.update(Message::Player(PlayerCmd::PlayTracks(ids, idx as i64)))
            }
        }
        DMsg::DragStart(id) => {
            st.drag_id = Some(id);
            st.over_index = None;
            Task::none()
        }
        DMsg::DragOver(i) => {
            if st.drag_id.is_some() {
                st.over_index = Some(i);
            }
            Task::none()
        }
        DMsg::DragEnd => {
            let (Some(id), Some(to)) = (st.drag_id.take(), st.over_index.take()) else {
                st.drag_id = None;
                st.over_index = None;
                return Task::none();
            };
            let Some(from) = st.tracks.iter().position(|t| t.playlist_track_id == id) else {
                return Task::none();
            };
            if from == to {
                return Task::none();
            }
            // Optimistic reorder; the refresh reconciles with the DB.
            let moved = st.tracks.remove(from);
            st.tracks.insert(to, moved);
            bg(
                &app.backend,
                move |b| playlists::reorder_playlist_track(id, to as i64, b.db()),
                |r| dm(DMsg::Reordered(r)),
            )
        }
        DMsg::Reordered(r) | DMsg::Done(r) => {
            if let Err(e) = r {
                st.error = Some(e);
            }
            Task::batch([refresh(app), super::on_playlists_changed(app)])
        }
    }
}

/// Mouse released anywhere while dragging.
pub fn dragging(app: &Halo) -> bool {
    app.pages.playlist.drag_id.is_some()
}

pub fn delete_now(app: &mut Halo, id: i64) -> Task<Message> {
    bg(
        &app.backend,
        move |b| playlists::delete_playlist(id, b.db()),
        |r| dm(DMsg::Deleted(r)),
    )
}

pub fn view(app: &Halo) -> Element<'_, Message> {
    let k = t();
    let st = &app.pages.playlist;
    let back = button(
        row![icon(Icon::ArrowLeft, 12.0), text("Playlists").size(theme::SMALL)]
            .spacing(4)
            .align_y(Alignment::Center),
    )
    .padding(0)
    .style(theme::btn_icon)
    .on_press(Message::Navigate(Route::Playlists));

    let mut col = column![back].spacing(12);
    if st.loading {
        col = col.push(note("Loading…"));
        return common::padded(col);
    }
    let Some(p) = &st.playlist else {
        col = col.push(common::error_text(st.error.as_deref().unwrap_or("Not found.")));
        return common::padded(col);
    };

    // ── Header ────────────────────────────────────────────────────────────────
    let title: Element<'_, Message> = if st.editing {
        row![
            text_input("", &st.edit_name)
                .id(RENAME_INPUT)
                .on_input(|s| dm(DMsg::RenameChanged(s)))
                .on_submit(dm(DMsg::SaveRename))
                .size(theme::TITLE)
                .font(theme::SEMIBOLD)
                .padding(Padding::from([4.0, 0.0]))
                .style(theme::input_underline)
                .width(Length::Fill),
            small_button("Save", true, Some(dm(DMsg::SaveRename))),
            small_button("Cancel", false, Some(dm(DMsg::CancelRename))),
        ]
        .spacing(8)
        .align_y(Alignment::Center)
        .into()
    } else {
        row![
            line(p.name.as_str()).size(theme::TITLE).font(theme::SEMIBOLD),
            button(icon(Icon::Pencil, 14.0))
                .padding(0)
                .style(theme::btn_icon)
                .on_press(dm(DMsg::StartRename)),
        ]
        .spacing(8)
        .align_y(Alignment::Center)
        .into()
    };
    let has = !st.tracks.is_empty();
    let ids: Vec<i64> = st.tracks.iter().map(|t| t.track_id).collect();
    let btn = |ic: Icon, label: &'static str, style: fn(&iced::Theme, button::Status) -> button::Style, msg: Option<Message>| {
        button(
            row![icon(ic, 14.0), text(label).size(theme::BODY)]
                .spacing(6)
                .align_y(Alignment::Center),
        )
        .padding(Padding::from([6.0, 12.0]))
        .style(move |th, s| {
            let mut st = style(th, s);
            st.border.radius = theme::radius(theme::R_MD);
            st
        })
        .on_press_maybe(msg)
    };
    fn delete_style(th: &iced::Theme, s: button::Status) -> button::Style {
        let k = t();
        let mut st = theme::btn_outline(th, s);
        if matches!(s, button::Status::Hovered | button::Status::Pressed) {
            st.background = Some(a(k.destructive, 0.1).into());
            st.text_color = k.destructive;
            st.border.color = a(k.destructive, 0.4);
        }
        st
    }
    let actions = row![
        btn(Icon::Play, "Play", theme::btn_primary, has.then(|| Message::Page(Msg::PlayAll(ids.clone())))),
        btn(Icon::Shuffle, "Shuffle", theme::btn_outline, has.then(|| Message::Player(PlayerCmd::ShufflePlay(ids.clone())))),
        btn(Icon::Trash2, "Delete", delete_style, Some(dm(DMsg::Delete))),
    ]
    .spacing(8);
    let header = row![
        center(icon(Icon::ListMusic, 40.0).color(a(k.muted_fg, 0.6)))
            .width(96)
            .height(96)
            .style(|_| iced::widget::container::Style {
                background: Some(t().muted.into()),
                border: theme::no_border(theme::R_MD),
                ..Default::default()
            }),
        column![
            title,
            ui::muted(plural(p.track_count, "track", "tracks"), theme::BODY),
            ui::vspace(8.0),
            actions,
        ]
        .spacing(4)
        .width(Length::Fill),
    ]
    .spacing(16);
    col = col.push(header).push(ui::vspace(8.0));
    if let Some(e) = &st.error {
        col = col.push(common::error_text(e));
    }

    // ── Track list ────────────────────────────────────────────────────────────
    if !has {
        col = col.push(
            row![
                ui::muted("This playlist is empty. Add songs from ", theme::BODY),
                button(text("All Songs").size(theme::BODY))
                    .padding(0)
                    .style(theme::btn_link)
                    .on_press(Message::Navigate(Route::default())),
                ui::muted(".", theme::BODY),
            ]
            .wrap(),
        );
        return common::scroll_page(app.route().scroll_key(), container(col).padding(24));
    }
    let current = app.player.current_track.as_ref().map(|t| t.track_id);
    let playing = app.is_playing();
    let mut list = column![];
    for (idx, r) in st.tracks.iter().enumerate() {
        let is_current = Some(r.track_id) == current;
        let is_playing = is_current && playing;
        let dragged = st.drag_id == Some(r.playlist_track_id);
        let drop_line = st.drag_id.is_some() && st.over_index == Some(idx);
        let title = r.title.clone().unwrap_or_else(|| {
            r.file_path.rsplit(['/', '\\']).next().unwrap_or(&r.file_path).to_string()
        });
        let mut sub = if r.artists.is_empty() { "—".to_string() } else { r.artists.join(", ") };
        if let Some(al) = &r.album_name {
            sub.push_str(&format!(" · {al}"));
        }
        let base = row![
            ui::hspace(20.0),
            container(text((idx + 1).to_string()).size(theme::SMALL).color(k.muted_fg))
                .width(24)
                .align_x(Alignment::End),
            button(icon_box(if is_playing { Icon::Pause } else { Icon::Play }, 14.0, 28.0, 28.0))
                .padding(0)
                .style(theme::btn_icon)
                .on_press(dm(DMsg::RowPlay(idx, r.track_id))),
            art(r.album_art_path.as_deref(), 32.0, theme::R_SM),
            column![
                line(title)
                    .size(theme::BODY)
                    .font(if is_current { theme::MEDIUM } else { theme::SANS })
                    .width(Length::Fill),
                line(sub).size(theme::SMALL).color(k.muted_fg).width(Length::Fill),
            ]
            .width(Length::Fill),
            container(text(duration(r.duration_ms)).size(theme::SMALL).color(k.muted_fg))
                .width(48)
                .align_x(Alignment::End),
            ui::hspace(28.0),
        ]
        .spacing(12)
        .align_y(Alignment::Center);
        let grip = mouse_area(
            center(icon(Icon::GripVertical, 14.0).color(a(k.muted_fg, 0.4)))
                .width(20)
                .height(28),
        )
        .on_press(dm(DMsg::DragStart(r.playlist_track_id)))
        .interaction(iced::mouse::Interaction::Grab);
        let top = row![
            grip,
            ui::fill(),
            button(icon_box(Icon::X, 14.0, 28.0, 28.0))
                .padding(0)
                .style(theme::btn_icon)
                .on_press(dm(DMsg::Remove(r.playlist_track_id))),
        ]
        .align_y(Alignment::Center);
        let content = hover(base, top);
        let mut item = column![];
        if idx > 0 || drop_line {
            item = item.push(
                container(Space::new())
                    .width(Length::Fill)
                    .height(if drop_line { 2 } else { 1 })
                    .style(move |_| iced::widget::container::Style {
                        background: Some(if drop_line { t().primary } else { t().border }.into()),
                        ..Default::default()
                    }),
            );
        }
        let ids2 = ids.clone();
        item = item.push(
            surface(content)
                .width(Length::Fill)
                .padding(Padding::from([6.0, 12.0]))
                .opacity(if dragged { 0.4 } else { 1.0 })
                .on_double_click(Message::Player(PlayerCmd::PlayTracks(ids2, idx as i64)))
                .on_right_press(crate::components::context_menu::opener(
                    crate::components::context_menu::Target::Track {
                        id: r.track_id,
                        album: r.album_name.clone(),
                        artists: r.artists.clone(),
                        playlist: Some(r.playlist_track_id),
                    },
                ))
                .on_enter(dm(DMsg::DragOver(idx)))
                .style(move |_| iced::widget::container::Style {
                    background: is_current.then(|| a(t().muted, 0.4).into()),
                    ..Default::default()
                }),
        );
        list = list.push(item);
    }
    let list = container(list)
        .width(Length::Fill)
        .clip(true)
        .style(move |_| iced::widget::container::Style {
            border: theme::border(k.border, 1.0, theme::R_MD),
            ..Default::default()
        });
    col = col.push(list);
    common::scroll_page(app.route().scroll_key(), container(col).padding(24))
}
