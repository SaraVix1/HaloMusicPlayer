//! Metadata editor dialog (`metadata-editor-dialog.tsx`) — filled in by the
//! settings/editors phase.

use iced::widget::text;
use iced::{Element, Task};

use super::dialogs::DMsg;
use crate::app::{Halo, Message};
use crate::ui::modal;

#[derive(Debug, Clone)]
pub enum Msg {}

pub fn update(_app: &mut Halo, msg: Msg) -> Task<Message> {
    match msg {}
}

pub fn on_ctrl_enter(_app: &mut Halo) -> Task<Message> {
    Task::none()
}

pub struct Editor {
    pub track_id: i64,
}

pub fn open(app: &mut Halo, track_id: i64) -> Task<Message> {
    app.pages.dialog = Some(super::dialogs::Dialog::Metadata(Box::new(Editor { track_id })));
    Task::none()
}

pub fn update_dialog_msg(_app: &mut Halo, _m: DMsg) -> Task<Message> {
    Task::none()
}

pub fn on_escape(_app: &mut Halo) -> bool {
    false
}

pub fn view<'a>(_app: &'a Halo, ed: &'a Editor) -> Element<'a, Message> {
    modal(
        "Edit metadata",
        text(format!("Track {}", ed.track_id)),
        None,
        640.0,
        Message::Page(super::Msg::Dialog(DMsg::Close)),
    )
}
