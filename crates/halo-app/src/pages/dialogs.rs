//! Modal dialogs owned by pages: Add to playlist (`add-to-playlist-dialog.tsx`)
//! and a generic confirm dialog (replaces `window.confirm()` and the settings
//! confirmations). The metadata editor lives in its own module.

use halo_backend::commands::playlists::{self, Playlist};
use iced::widget::{button, column, container, row, scrollable, text, text_input};
use iced::{Alignment, Element, Length, Padding, Task};

use super::Msg;
use crate::app::{bg, Halo, Message};
use crate::icons::{icon, Icon};
use crate::theme::{self, t};
use crate::ui::{self, modal, small_button};
use crate::widgets::line;

pub const NEW_PLAYLIST_INPUT: &str = "add-to-playlist-name";

#[derive(Debug, Clone)]
pub struct AddToPlaylist {
    pub track_ids: Vec<i64>,
    pub playlists: Option<Vec<Playlist>>,
    pub creating: bool,
    pub new_name: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub enum ConfirmAction {
    DeletePlaylist(i64),
    ResetStats,
    ClearDatabase,
}

#[derive(Debug, Clone)]
pub struct Confirm {
    pub title: String,
    pub body: String,
    pub confirm_label: String,
    pub destructive: bool,
    pub action: ConfirmAction,
}

pub enum Dialog {
    AddToPlaylist(AddToPlaylist),
    Confirm(Confirm),
    Metadata(Box<super::metadata::Editor>),
}

#[derive(Debug, Clone)]
pub enum DMsg {
    Close,
    PlaylistsLoaded(Result<Vec<Playlist>, String>),
    Add(i64),
    StartCreate,
    CancelCreate,
    NameChanged(String),
    Create,
    Done(Result<(), String>),
    Confirm,
}

fn dm(m: DMsg) -> Message {
    Message::Page(Msg::Dialog(m))
}

pub fn open_add_to_playlist(app: &mut Halo, track_ids: Vec<i64>) -> Task<Message> {
    app.pages.dialog = Some(Dialog::AddToPlaylist(AddToPlaylist {
        track_ids,
        playlists: None,
        creating: false,
        new_name: String::new(),
        error: None,
    }));
    bg(
        &app.backend,
        |b| playlists::get_playlists(b.db()),
        |r| dm(DMsg::PlaylistsLoaded(r)),
    )
}

pub fn open_confirm(app: &mut Halo, c: Confirm) -> Task<Message> {
    app.pages.dialog = Some(Dialog::Confirm(c));
    Task::none()
}

pub fn update(app: &mut Halo, msg: DMsg) -> Task<Message> {
    match msg {
        DMsg::Close => {
            app.pages.dialog = None;
            Task::none()
        }
        DMsg::Confirm => {
            let Some(Dialog::Confirm(c)) = app.pages.dialog.take() else {
                return Task::none();
            };
            super::on_confirm(app, c.action)
        }
        m => {
            let Some(Dialog::AddToPlaylist(st)) = app.pages.dialog.as_mut() else {
                return super::metadata::update_dialog_msg(app, m);
            };
            match m {
                DMsg::PlaylistsLoaded(r) => {
                    match r {
                        Ok(v) => st.playlists = Some(v),
                        Err(e) => {
                            st.playlists = Some(vec![]);
                            st.error = Some(e);
                        }
                    }
                    Task::none()
                }
                DMsg::Add(pid) => {
                    let ids = st.track_ids.clone();
                    bg(
                        &app.backend,
                        move |b| playlists::add_to_playlist(pid, ids, b.db()),
                        |r| dm(DMsg::Done(r)),
                    )
                }
                DMsg::StartCreate => {
                    st.creating = true;
                    st.new_name.clear();
                    iced::widget::operation::focus(NEW_PLAYLIST_INPUT)
                }
                DMsg::CancelCreate => {
                    st.creating = false;
                    Task::none()
                }
                DMsg::NameChanged(s) => {
                    st.new_name = s;
                    Task::none()
                }
                DMsg::Create => {
                    let name = st.new_name.trim().to_string();
                    if name.is_empty() {
                        return Task::none();
                    }
                    let ids = st.track_ids.clone();
                    bg(
                        &app.backend,
                        move |b| {
                            let p = playlists::create_playlist(name, b.db())?;
                            playlists::add_to_playlist(p.id, ids, b.db())
                        },
                        |r| dm(DMsg::Done(r)),
                    )
                }
                DMsg::Done(r) => {
                    match r {
                        Ok(()) => {
                            app.pages.dialog = None;
                            return super::on_playlists_changed(app);
                        }
                        Err(e) => st.error = Some(e),
                    }
                    Task::none()
                }
                DMsg::Close | DMsg::Confirm => Task::none(),
            }
        }
    }
}

/// Escape closes the topmost dialog.
pub fn on_escape(app: &mut Halo) -> bool {
    match app.pages.dialog.as_mut() {
        Some(Dialog::AddToPlaylist(st)) if st.creating => {
            st.creating = false;
            true
        }
        Some(Dialog::Metadata(_)) => {
            if !super::metadata::on_escape(app) {
                app.pages.dialog = None;
            }
            true
        }
        Some(_) => {
            app.pages.dialog = None;
            true
        }
        None => false,
    }
}

pub fn view(app: &Halo) -> Option<Element<'_, Message>> {
    match app.pages.dialog.as_ref()? {
        Dialog::AddToPlaylist(st) => Some(add_to_playlist(st)),
        Dialog::Confirm(c) => Some(confirm(c)),
        Dialog::Metadata(ed) => Some(super::metadata::view(app, ed)),
    }
}

fn add_to_playlist(st: &AddToPlaylist) -> Element<'_, Message> {
    let k = t();
    let mut body = column![].spacing(12);
    if let Some(e) = &st.error {
        body = body.push(text(e.as_str()).size(theme::BODY).color(k.destructive));
    }
    let list: Element<'_, Message> = match &st.playlists {
        None => container(ui::muted("Loading…", theme::BODY))
            .padding(Padding::from([12.0, 4.0]))
            .into(),
        Some(v) if v.is_empty() => container(ui::muted(
            "No playlists yet — create one below.",
            theme::BODY,
        ))
        .padding(Padding::from([12.0, 4.0]))
        .into(),
        Some(v) => {
            let mut col = column![];
            for p in v {
                col = col.push(
                    button(
                        row![
                            icon(Icon::ListMusic, 14.0).color(k.muted_fg),
                            line(p.name.as_str()).size(theme::BODY).width(Length::Fill),
                            text(p.track_count.to_string())
                                .size(theme::SMALL)
                                .color(k.muted_fg),
                        ]
                        .spacing(8)
                        .align_y(Alignment::Center),
                    )
                    .width(Length::Fill)
                    .padding(Padding::from([6.0, 8.0]))
                    .style(theme::btn_list_item(false, theme::R_MD))
                    .on_press(dm(DMsg::Add(p.id))),
                );
            }
            scrollable(col)
                .direction(theme::vscroll())
                .style(theme::scroll)
                .height(Length::Shrink)
                .into()
        }
    };
    body = body.push(container(list).max_height(288));

    let footer: Element<'_, Message> = if st.creating {
        let can = !st.new_name.trim().is_empty();
        row![
            text_input("Playlist name", &st.new_name)
                .id(NEW_PLAYLIST_INPUT)
                .on_input(|s| dm(DMsg::NameChanged(s)))
                .on_submit(dm(DMsg::Create))
                .size(theme::BODY)
                .padding(Padding::from([4.0, 0.0]))
                .style(theme::input_underline)
                .width(Length::Fill),
            small_button("Create + Add", true, can.then(|| dm(DMsg::Create))),
            small_button("Cancel", false, Some(dm(DMsg::CancelCreate))),
        ]
        .spacing(8)
        .align_y(Alignment::Center)
        .into()
    } else {
        button(
            row![icon(Icon::Plus, 14.0), text("New playlist…").size(theme::BODY)]
                .spacing(8)
                .align_y(Alignment::Center),
        )
        .width(Length::Fill)
        .padding(Padding::from([6.0, 8.0]))
        .style(move |th, s| {
            let mut st = theme::btn_list_item(false, theme::R_MD)(th, s);
            if !matches!(s, iced::widget::button::Status::Hovered) {
                st.text_color = t().muted_fg;
            }
            st
        })
        .on_press(dm(DMsg::StartCreate))
        .into()
    };
    let body = column![body, ui::hline(), footer].spacing(12);
    modal("Add to playlist", body, None, 384.0, dm(DMsg::Close))
}

fn confirm(c: &Confirm) -> Element<'_, Message> {
    let body = ui::muted(c.body.as_str(), theme::BODY);
    let footer = row![
        ui::fill(),
        ui::label_button(None, "Cancel", theme::BODY, 32.0, 10.0, theme::btn_outline, Some(dm(DMsg::Close))),
        ui::label_button(
            None,
            c.confirm_label.as_str(),
            theme::BODY,
            32.0,
            10.0,
            if c.destructive { theme::btn_destructive } else { theme::btn_primary },
            Some(dm(DMsg::Confirm)),
        ),
    ]
    .spacing(8);
    modal(&c.title, body, Some(footer.into()), 384.0, dm(DMsg::Close))
}
