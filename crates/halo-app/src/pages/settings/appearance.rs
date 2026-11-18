//! Settings → Appearance: Light / Dark / System theme segmented control.

use halo_backend::commands::ui;
use iced::widget::{button, column, container, row, text};
use iced::{Alignment, Element, Padding, Task};

use super::{done, msg, section_header, setting_row, Msg as SMsg};
use crate::app::{bg, Halo, Message};
use crate::icons::{icon, Icon};
use crate::theme::{self, t};

#[derive(Debug, Clone)]
pub enum Msg {
    SetTheme(&'static str),
}

pub fn update(app: &mut Halo, m: Msg) -> Task<Message> {
    match m {
        Msg::SetTheme(th) => {
            app.set_theme_pref(th.to_string());
            bg(
                &app.backend,
                move |b| ui::set_theme(th.to_string(), b.clone(), b.db()),
                done,
            )
        }
    }
}

pub fn view(app: &Halo) -> Element<'_, Message> {
    let mut seg = row![].spacing(0);
    for (value, label, ic) in [
        ("light", "Light", Icon::Sun),
        ("dark", "Dark", Icon::Moon),
        ("system", "System", Icon::Monitor),
    ] {
        let active = app.theme_pref == value;
        seg = seg.push(
            button(
                row![icon(ic, 14.0), text(label).size(theme::TEXT_SM)]
                    .spacing(6)
                    .align_y(Alignment::Center),
            )
            .padding(Padding::from([6.0, 12.0]))
            .style(move |_, s| {
                let k = t();
                button::Style {
                    background: active.then(|| k.muted.into()),
                    text_color: if active || matches!(s, button::Status::Hovered) {
                        k.foreground
                    } else {
                        k.muted_fg
                    },
                    border: theme::no_border(theme::R_XS),
                    ..Default::default()
                }
            })
            .on_press(msg(SMsg::Appearance(Msg::SetTheme(value)))),
        );
    }
    column![
        section_header("Appearance", "Colour scheme and visual style."),
        setting_row(
            "Theme",
            "Choose how Halo looks.",
            container(seg).padding(2).style(theme::bordered(theme::R_MD)),
        ),
    ]
    .into()
}
