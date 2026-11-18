//! Lyrics panel (`lyrics-panel.tsx`) — implemented in a later phase.

use iced::widget::{center, text};
use iced::{Element, Task};

use crate::app::{Halo, Message};

#[derive(Default)]
pub struct State {
    pub open: bool,
}

#[derive(Debug, Clone)]
pub enum Msg {}

pub fn on_track(_app: &mut Halo, _id: Option<i64>) -> Task<Message> {
    Task::none()
}

pub fn on_position(_app: &mut Halo) -> Task<Message> {
    Task::none()
}

pub fn update(_app: &mut Halo, msg: Msg) -> Task<Message> {
    match msg {}
}

pub fn view(_app: &Halo) -> Element<'_, Message> {
    center(text("Lyrics")).into()
}
