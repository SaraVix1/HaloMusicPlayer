//! Settings → Playback: crossfade duration (0–12 s in 0.5 s steps).

use halo_backend::commands::player;
use iced::widget::{column, container, row, slider, text};
use iced::{Alignment, Element, Length, Task};

use super::{done, msg, section_header, setting_row, Msg as SMsg};
use crate::app::{bg, Halo, Message};
use crate::theme::{self, t};

#[derive(Default)]
pub struct State {
    pub crossfade_ms: i64,
}

#[derive(Debug, Clone)]
pub enum Msg {
    Loaded(i64),
    Change(i64),
    Commit,
}

fn pm(m: Msg) -> Message {
    msg(SMsg::Playback(m))
}

pub fn load(app: &mut Halo) -> Task<Message> {
    bg(
        &app.backend,
        |b| player::get_crossfade_ms(b.db()).unwrap_or(0),
        |v| pm(Msg::Loaded(v)),
    )
}

pub fn update(app: &mut Halo, m: Msg) -> Task<Message> {
    let st = &mut app.pages.settings.playback;
    match m {
        Msg::Loaded(v) | Msg::Change(v) => {
            st.crossfade_ms = v;
            Task::none()
        }
        Msg::Commit => {
            let ms = st.crossfade_ms;
            bg(&app.backend, move |b| player::set_crossfade_ms(b, ms), done)
        }
    }
}

pub fn view(app: &Halo) -> Element<'_, Message> {
    let ms = app.pages.settings.playback.crossfade_ms;
    let control = row![
        slider(0.0..=12000.0_f64, ms as f64, |v| pm(Msg::Change(v as i64)))
            .step(500.0_f64)
            .on_release(pm(Msg::Commit))
            .width(Length::Fill)
            .style(theme::slider_style),
        container(
            text(format!("{:.1}s", ms as f64 / 1000.0))
                .size(theme::BODY)
                .color(t().muted_fg)
        )
        .width(40)
        .align_x(Alignment::End),
    ]
    .spacing(12)
    .align_y(Alignment::Center)
    .width(224);
    column![
        section_header("Playback", "Audio playback and transition settings."),
        setting_row(
            "Crossfade",
            "Fade between consecutive tracks. Set to 0 to disable.",
            control,
        ),
    ]
    .into()
}
