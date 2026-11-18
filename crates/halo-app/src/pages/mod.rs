//! Routed pages. Each page module owns its state and messages; this module
//! dispatches by route.

use halo_backend::commands::eq::EqConfig;
use halo_backend::scanner::ScanProgress;
use iced::widget::{center, text};
use iced::{Element, Subscription, Task};

use crate::app::{Halo, Message};
use crate::route::Route;

#[derive(Default)]
pub struct Pages {}

#[derive(Debug, Clone)]
pub enum Msg {}

pub fn update(_app: &mut Halo, msg: Msg) -> Task<Message> {
    match msg {}
}

pub fn on_enter(_app: &mut Halo) -> Task<Message> {
    Task::none()
}

pub fn on_navigate(_app: &mut Halo, _prev: Route) -> Task<Message> {
    Task::none()
}

pub fn on_library_changed(_app: &mut Halo) -> Task<Message> {
    Task::none()
}

pub fn on_queue_changed(_app: &mut Halo) -> Task<Message> {
    Task::none()
}

pub fn on_scan_progress(_app: &mut Halo, _p: ScanProgress) -> Task<Message> {
    Task::none()
}

pub fn on_eq_changed(_app: &mut Halo, _cfg: EqConfig) -> Task<Message> {
    Task::none()
}

pub fn on_rating_changed(_app: &mut Halo, _track_id: i64, _rating: i64) -> Task<Message> {
    Task::none()
}

pub fn on_escape(_app: &mut Halo) -> Task<Message> {
    Task::none()
}

pub fn on_ctrl_enter(_app: &mut Halo) -> Task<Message> {
    Task::none()
}

pub fn subscription(_app: &Halo) -> Option<Subscription<Message>> {
    None
}

pub fn overlay(_app: &Halo) -> Option<Element<'_, Message>> {
    None
}

pub fn view(app: &Halo) -> Element<'_, Message> {
    center(text(format!("{:?}", app.route()))).into()
}
