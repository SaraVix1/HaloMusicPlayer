//! Mini player window (`mini-player.tsx`) — implemented in a later phase.

use iced::widget::{center, text};
use iced::{Element, Point, Subscription, Task};

use crate::app::{Halo, Message};

#[derive(Default)]
pub struct State {}

#[derive(Debug, Clone)]
pub enum Msg {}

pub fn update(_app: &mut Halo, msg: Msg) -> Task<Message> {
    match msg {}
}

pub fn open(_app: &mut Halo) -> Task<Message> {
    Task::none()
}

pub fn on_moved(_app: &mut Halo, _pos: Point) -> Task<Message> {
    Task::none()
}

pub fn subscription(_app: &Halo) -> Option<Subscription<Message>> {
    None
}

pub fn view(_app: &Halo) -> Element<'_, Message> {
    center(text("Mini")).into()
}
