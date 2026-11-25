//! Custom title bar (`title-bar.tsx`): back/forward, drag region with the
//! app name, and mini-player / minimise / maximise / close buttons.

use iced::widget::{button, container, mouse_area, row, text, tooltip::Position};
use iced::{window, Alignment, Element, Length, Padding};

use crate::app::{Halo, Message};
use crate::icons::Icon;
use crate::theme::{self, a, t};
use crate::ui::{icon_box, tip};

pub fn view(app: &Halo, id: window::Id) -> Element<'_, Message> {
    let can_back = app.history.can_back();
    let can_fwd = app.history.can_forward();

    let nav = |i: Icon, enabled: bool, msg: Message, label: &'static str| -> Element<'_, Message> {
        let b = button(icon_box(i, 14.0, 28.0, 24.0))
            .padding(0)
            .style(move |th, st| {
                if enabled {
                    theme::btn_muted_square(theme::R_XS)(th, st)
                } else {
                    button::Style {
                        text_color: a(t().muted_fg, 0.3),
                        ..button::Style::default()
                    }
                }
            })
            .on_press_maybe(enabled.then_some(msg));
        tip(b, label, Position::Bottom)
    };

    let left = row![
        nav(Icon::ArrowLeft, can_back, Message::Back, "Back"),
        nav(Icon::ArrowRight, can_fwd, Message::Forward, "Forward"),
    ]
    .spacing(2)
    .padding(Padding::from([0.0, 0.0]).left(8.0))
    .align_y(Alignment::Center);

    let drag = mouse_area(
        container(text("Halo").size(theme::BODY).font(theme::SEMIBOLD))
            .padding(Padding::from([0.0, 12.0]))
            .width(Length::Fill)
            .height(Length::Fill)
            .align_y(Alignment::Center),
    )
    .on_press(Message::DragWindow(id))
    .on_double_click(Message::ToggleMaximize);

    let ctl = |i: Icon, size: f32, msg: Message, close: bool| {
        button(icon_box(i, size, 44.0, 32.0))
            .padding(0)
            .style(move |th, st| {
                if close {
                    theme::btn_close(th, st)
                } else {
                    theme::btn_muted_square(0.0)(th, st)
                }
            })
            .on_press(msg)
    };

    let right = row![
        tip(
            ctl(Icon::PictureInPicture2, 13.0, Message::OpenMini, false),
            "Mini player (Ctrl+M)",
            Position::Bottom
        ),
        ctl(Icon::Minus, 14.0, Message::Minimize, false),
        ctl(Icon::Square, 11.0, Message::ToggleMaximize, false),
        ctl(Icon::X, 14.0, Message::CloseMain, true),
    ];

    let bar = row![left, drag, right]
        .height(32)
        .width(Length::Fill)
        .align_y(Alignment::Center);

    // Over the blurred art in full-screen now playing, the bar is see-through.
    let style = if app.fullscreen.open { theme::clear } else { theme::frame };
    container(bar).style(style).into()
}
