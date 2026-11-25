//! Card widgets and grid presets shared by the Library tabs.

use iced::widget::{center, column, container, text, Space};
use iced::{gradient, Alignment, Background, Element, Length, Radians};

use super::super::common::GridSpec;
use crate::app::Message;
use crate::format::{initials, plural};
use crate::route::Route;
use crate::theme::{self, a, t};
use crate::widgets::{line, surface};

/// Artists / Album Artists grid (`minItemWidth 140, rowHeight 210, gap 20`).
pub const PEOPLE: GridSpec = GridSpec {
    min_item_width: 140.0,
    row_height: 210.0,
    gap: 24.0,
    px: 24.0,
    padding_top: 8.0,
    padding_bottom: 24.0,
};

/// Composers / Genres grid (`minItemWidth 180, rowHeight 108, gap 12`).
pub const TILES: GridSpec = GridSpec {
    min_item_width: 180.0,
    row_height: 108.0,
    gap: 12.0,
    px: 24.0,
    padding_top: 8.0,
    padding_bottom: 24.0,
};

/// Round artist avatar: album art, or initials on muted.
pub fn avatar<'a>(name: &str, art_path: Option<&str>) -> Element<'a, Message> {
    let k = t();
    match art_path.filter(|p| !p.is_empty() && std::path::Path::new(p).is_file()) {
        Some(p) => iced::widget::image(iced::widget::image::Handle::from_path(p))
            .width(Length::Fill)
            .height(Length::Fill)
            .content_fit(iced::ContentFit::Cover)
            .border_radius(theme::R_FULL)
            .into(),
        None => {
            let ini = initials(name);
            center(
                text(if ini.is_empty() { "?".to_string() } else { ini })
                    .size(theme::TITLE)
                    .font(theme::MEDIUM)
                    .color(k.muted_fg),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .style(|_| iced::widget::container::Style {
                background: Some(t().muted.into()),
                border: theme::no_border(theme::R_FULL),
                ..Default::default()
            })
            .into()
        }
    }
}

/// Artist / album-artist card: round avatar, name, subtitle.
pub fn person_card<'a>(
    name: &'a str,
    art_path: Option<&'a str>,
    sub: String,
    route: Route,
    side: f32,
    menu: crate::components::context_menu::Target,
) -> Element<'a, Message> {
    let k = t();
    let content = column![
        container(avatar(name, art_path)).width(side).height(side),
        column![
            container(line(name).size(theme::BODY).font(theme::MEDIUM))
                .width(Length::Fill)
                .align_x(Alignment::Center),
            container(text(sub).size(theme::SMALL).color(k.muted_fg))
                .width(Length::Fill)
                .align_x(Alignment::Center),
        ],
    ]
    .spacing(8)
    .align_x(Alignment::Center);
    surface(content)
        .width(Length::Fill)
        .on_press(Message::Navigate(route))
        .on_right_press(crate::components::context_menu::opener(menu))
        .interaction(iced::mouse::Interaction::Pointer)
        .style(|s| iced::widget::container::Style {
            background: s.hovered.then(|| a(t().background, 0.1).into()),
            text_color: Some(t().foreground),
            ..Default::default()
        })
        .into()
}

/// Composer / genre card: hashed gradient, name, track count.
pub fn gradient_card<'a>(name: &'a str, count: i64, route: Route) -> Element<'a, Message> {
    let (from, to) = theme::gradient_for(name);
    let content = column![
        line(name).size(theme::BODY).font(theme::SEMIBOLD).width(Length::Fill),
        Space::new().height(Length::Fill),
        text(plural(count, "track", "tracks"))
            .size(theme::SMALL)
            .color(t().muted_fg),
    ]
    .height(Length::Fill);
    surface(content)
        .width(Length::Fill)
        .height(96)
        .padding(16)
        .on_press(Message::Navigate(route))
        .interaction(iced::mouse::Interaction::Pointer)
        .style(move |s| {
            let k = t();
            let lift = if s.hovered { 0.15 } else { 0.0 };
            let g = gradient::Linear::new(Radians(std::f32::consts::FRAC_PI_4 * 3.0))
                .add_stop(0.0, a(from, 0.3 + lift))
                .add_stop(1.0, a(to, 0.2 + lift));
            iced::widget::container::Style {
                background: Some(Background::Gradient(g.into())),
                border: theme::no_border(theme::R_LG),
                text_color: Some(k.foreground),
                ..Default::default()
            }
        })
        .into()
}

