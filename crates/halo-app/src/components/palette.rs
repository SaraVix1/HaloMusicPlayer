//! Ctrl+K command palette (`command-palette.tsx`) — implemented in a later phase.

use iced::widget::{center, text};
use iced::{Element, Task};

use crate::app::{Halo, Message};

#[derive(Default)]
pub struct State {
    pub open: bool,
}

#[derive(Debug, Clone)]
pub enum Msg {
    Open,
    Close,
    Move(i32),
    Activate,
}

pub fn toggle(app: &mut Halo) -> Task<Message> {
    app.palette.open = !app.palette.open;
    Task::none()
}

pub fn update(app: &mut Halo, msg: Msg) -> Task<Message> {
    match msg {
        Msg::Open => app.palette.open = true,
        Msg::Close => app.palette.open = false,
        Msg::Move(_) | Msg::Activate => {}
    }
    Task::none()
}

pub fn view(_app: &Halo) -> Element<'_, Message> {
    center(text("Search")).into()
}
