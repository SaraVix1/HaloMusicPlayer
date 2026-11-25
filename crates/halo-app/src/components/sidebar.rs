//! Icon-only navigation rail: Home, Library, Playlists, Search (opens the
//! Ctrl+K palette) and, until the queue panel replaces it, Queue; Settings
//! pinned to the bottom. Labels show as tooltips.

use iced::widget::{button, column, container, tooltip::Position};
use iced::{Alignment, Element, Length, Padding};

use crate::app::{Halo, Message};
use crate::components::palette;
use crate::icons::Icon;
use crate::route::{Route, Section};
use crate::theme;
use crate::ui::{icon_box, tip};

fn nav_button<'a>(i: Icon, label: &'static str, active: bool, msg: Message) -> Element<'a, Message> {
    let b = button(icon_box(i, 20.0, 44.0, 44.0))
        .padding(0)
        .style(theme::btn_nav(active))
        .on_press(msg);
    tip(b, label, Position::Right)
}

pub fn view(app: &Halo) -> Element<'_, Message> {
    let section = app.route().section();
    type Item = (Icon, &'static str, Option<(Section, Route)>);
    let items: [Item; 5] = [
        (Icon::House, "Home", Some((Section::Home, Route::Home))),
        // Library(None) opens the last visited tab.
        (Icon::Library, "Library", Some((Section::Library, Route::Library(None)))),
        (Icon::ListMusic, "Playlists", Some((Section::Playlists, Route::Playlists))),
        (Icon::Search, "Search (Ctrl+K)", None),
        // TODO(5d): replaced by the slide-in queue panel (≡ in the now-playing bar).
        (Icon::ListOrdered, "Queue", Some((Section::Queue, Route::Queue))),
    ];

    let mut top = column![].spacing(6).align_x(Alignment::Center);
    for (i, label, target) in items {
        let (active, msg) = match target {
            Some((s, r)) => (section == s, Message::Navigate(r)),
            None => (false, Message::Palette(palette::Msg::Open)),
        };
        top = top.push(nav_button(i, label, active, msg));
    }

    let settings = nav_button(
        Icon::Settings,
        "Settings",
        section == Section::Settings,
        Message::Navigate(Route::Settings(None)),
    );

    let rail = column![
        container(top).height(Length::Fill),
        settings,
    ]
    .align_x(Alignment::Center)
    .width(64)
    .height(Length::Fill)
    .padding(Padding::from([8.0, 0.0]));

    container(rail).style(theme::sidebar).height(Length::Fill).into()
}
