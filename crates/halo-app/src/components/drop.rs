//! Drag and drop files / folders onto the main window. While dragging, an
//! overlay says what will happen; on drop (paths arrive one event each,
//! collected for 200 ms) a dialog offers Play now (Enter) · Add to queue ·
//! Add to library, with a count of the music found. Esc cancels.

use std::path::PathBuf;
use std::time::Duration;

use halo_backend::open_files::{self, Dropped};
use iced::widget::{button, center, column, container, row, text, Space};
use iced::{Alignment, Element, Length, Padding, Task};

use crate::app::{bg, Halo, Message};
use crate::format::plural;
use crate::icons::{icon, Icon};
use crate::theme::{self, a, t};
use crate::ui;

const COLLECT: Duration = Duration::from_millis(200);

#[derive(Default)]
pub struct State {
    /// Files are being dragged over the window.
    pub hovering: bool,
    /// Paths dropped in the current burst (one event per path).
    pending: Vec<PathBuf>,
    token: u64,
    pub dialog: Option<Dialog>,
}

pub struct Dialog {
    /// `None` while counting.
    found: Option<Dropped>,
    /// Set while an action runs ("Importing 127 songs…").
    busy: Option<String>,
    error: Option<String>,
}

#[derive(Debug, Clone)]
pub enum Msg {
    Hovered,
    Left,
    Dropped(PathBuf),
    Collect(u64),
    Found(Dropped),
    Play,
    Queue,
    Library,
    Done(Result<(), String>),
    Cancel,
}

fn dm(m: Msg) -> Message {
    Message::Drop(m)
}

pub fn is_open(app: &Halo) -> bool {
    app.drop.dialog.is_some()
}

pub fn update(app: &mut Halo, m: Msg) -> Task<Message> {
    let st = &mut app.drop;
    match m {
        Msg::Hovered => {
            st.hovering = true;
            Task::none()
        }
        Msg::Left => {
            st.hovering = false;
            Task::none()
        }
        Msg::Dropped(p) => {
            st.hovering = false;
            st.pending.push(p);
            st.token += 1;
            let token = st.token;
            Task::perform(tokio::time::sleep(COLLECT), move |_| dm(Msg::Collect(token)))
        }
        Msg::Collect(token) => {
            if token != st.token || st.pending.is_empty() {
                return Task::none();
            }
            let paths = std::mem::take(&mut st.pending);
            st.dialog = Some(Dialog { found: None, busy: None, error: None });
            bg(&app.backend, move |_| open_files::expand(&paths), |d| dm(Msg::Found(d)))
        }
        Msg::Found(d) => {
            if let Some(dlg) = st.dialog.as_mut() {
                dlg.found = Some(d);
            }
            Task::none()
        }
        Msg::Play | Msg::Queue | Msg::Library => {
            let Some(dlg) = st.dialog.as_mut() else { return Task::none() };
            let Some(found) = dlg.found.clone().filter(|f| !f.songs.is_empty()) else { return Task::none() };
            if dlg.busy.is_some() {
                return Task::none();
            }
            let n = found.songs.len() as i64;
            dlg.busy = Some(match m {
                Msg::Library if !found.folders.is_empty() => "Adding to your library and scanning…".into(),
                _ => format!("Importing {}…", plural(n, "song", "songs")),
            });
            dlg.error = None;
            bg(
                &app.backend,
                move |b| match m {
                    Msg::Play => open_files::play_dropped(&b, &found.songs),
                    Msg::Queue => open_files::queue_dropped(&b, &found.songs),
                    _ => open_files::add_dropped_to_library(&b, &found),
                },
                |r| dm(Msg::Done(r)),
            )
        }
        Msg::Done(r) => {
            match r {
                Ok(()) => st.dialog = None,
                Err(e) => {
                    if let Some(dlg) = st.dialog.as_mut() {
                        dlg.busy = None;
                        dlg.error = Some(e);
                    }
                }
            }
            Task::none()
        }
        Msg::Cancel => {
            // A running import / scan carries on; only the dialog goes.
            st.dialog = None;
            Task::none()
        }
    }
}

/// Enter in the dialog: Play now.
pub fn on_enter(app: &mut Halo) -> Task<Message> {
    update(app, Msg::Play)
}

/// Full-window hint while files are dragged over the window.
pub fn overlay<'a>() -> Element<'a, Message> {
    let k = t();
    let card = column![
        icon(Icon::FolderOpen, 40.0).color(k.primary),
        text("Drop to play or add to your library").size(theme::TITLE).font(theme::SEMIBOLD),
        ui::muted("Songs or whole folders.", theme::BODY),
    ]
    .spacing(12)
    .align_x(Alignment::Center);
    container(center(container(card).padding(32).style(move |th| {
        let mut s = theme::dialog(th);
        s.border = iced::Border { color: a(t().primary, 0.8), width: 2.0, radius: theme::radius(theme::R_LG) };
        s
    })))
    .width(Length::Fill)
    .height(Length::Fill)
    .style(theme::backdrop(0.45))
    .into()
}

pub fn dialog(app: &Halo) -> Option<Element<'_, Message>> {
    let dlg = app.drop.dialog.as_ref()?;
    let k = t();
    let summary: Element<'_, Message> = match &dlg.found {
        None => ui::muted("Counting music…", theme::BODY).into(),
        Some(f) if f.songs.is_empty() => {
            text("No music files in what you dropped.").size(theme::BODY).into()
        }
        Some(f) => {
            let mut parts = vec![];
            if !f.folders.is_empty() {
                parts.push(plural(f.folders.len() as i64, "folder", "folders"));
            }
            parts.push(plural(f.songs.len() as i64, "song", "songs"));
            column![
                text(parts.join(" · ")).size(theme::BODY).font(theme::MEDIUM),
                ui::muted(
                    if f.folders.is_empty() {
                        "Add to library imports these songs."
                    } else {
                        "Add to library adds the folders to your library and scans them."
                    },
                    theme::SMALL
                ),
            ]
            .spacing(4)
            .into()
        }
    };
    let mut body = column![summary].spacing(12);
    if let Some(b) = &dlg.busy {
        body = body.push(ui::muted(b.as_str(), theme::BODY));
    }
    if let Some(e) = &dlg.error {
        body = body.push(text(e.as_str()).size(theme::BODY).color(k.destructive));
    }

    let ready = dlg.busy.is_none() && dlg.found.as_ref().is_some_and(|f| !f.songs.is_empty());
    let btn = |label: &'static str, primary: bool, m: Msg| {
        button(text(label).size(theme::BODY))
            .padding(Padding::from([6.0, 14.0]))
            .style(if primary { theme::btn_primary } else { theme::btn_outline })
            .on_press_maybe(ready.then_some(dm(m)))
    };
    let footer = row![
        button(text("Cancel").size(theme::BODY))
            .padding(Padding::from([6.0, 14.0]))
            .style(theme::btn_ghost)
            .on_press(dm(Msg::Cancel)),
        Space::new().width(Length::Fill),
        btn("Add to library", false, Msg::Library),
        btn("Add to queue", false, Msg::Queue),
        btn("Play now", true, Msg::Play),
    ]
    .spacing(8)
    .align_y(Alignment::Center);

    Some(ui::modal("Dropped files", body, Some(footer.into()), 520.0, dm(Msg::Cancel)))
}
