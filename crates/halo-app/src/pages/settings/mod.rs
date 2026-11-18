//! Settings page (`settings.tsx`) — category nav + one file per tab.

use halo_backend::commands::eq::EqConfig;
use halo_backend::scanner::ScanProgress;
use iced::widget::{center, text};
use iced::{Element, Subscription, Task};

use super::dialogs::ConfirmAction;
use crate::app::{Halo, Message};
use crate::route::SettingsTab;

#[derive(Default)]
pub struct SettingsState {
    pub last_tab: SettingsTab,
}

#[derive(Debug, Clone)]
pub enum Msg {}

pub fn update(_app: &mut Halo, msg: Msg) -> Task<Message> {
    match msg {}
}

pub fn on_enter(_app: &mut Halo, _tab: SettingsTab) -> Task<Message> {
    Task::none()
}

pub fn on_confirm(_app: &mut Halo, _a: ConfirmAction) -> Task<Message> {
    Task::none()
}

pub fn on_scan_progress(_app: &mut Halo, _p: ScanProgress) -> Task<Message> {
    Task::none()
}

pub fn on_eq_changed(_app: &mut Halo, _cfg: EqConfig) -> Task<Message> {
    Task::none()
}

pub fn subscription(_app: &Halo) -> Option<Subscription<Message>> {
    None
}

pub fn view(_app: &Halo, tab: SettingsTab) -> Element<'_, Message> {
    center(text(tab.label())).into()
}
