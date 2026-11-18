//! Icon-only navigation rail (`sidebar.tsx`): Search, All Songs, Albums,
//! Library, Folders, Playlists, Queue; Settings pinned to the bottom.

use halo_backend::commands::library::TracksQuery;
use iced::widget::{button, column, container, row, tooltip::Position};
use iced::{Alignment, Element, Length, Padding};

use crate::app::{Halo, Message};
use crate::components::palette;
use crate::icons::Icon;
use crate::route::{Route, Section};
use crate::theme;
use crate::ui::{icon_box, tip, vline};

fn nav_button<'a>(i: Icon, label: &'static str, active: bool, msg: Message) -> Element<'a, Message> {
    let b = button(icon_box(i, 20.0, 40.0, 40.0))
        .padding(0)
        .style(theme::btn_nav(active))
        .on_press(msg);
    tip(b, label, Position::Right)
}

pub fn view(app: &Halo) -> Element<'_, Message> {
    let section = app.route().section();
    let items: [(Icon, &'static str, Option<(Section, Route)>); 7] = [
        (Icon::Search, "Search (Ctrl+K)", None),
        (
            Icon::Music,
            "All Songs",
            Some((Section::Songs, Route::Songs(TracksQuery::default()))),
        ),
        (Icon::Disc, "Albums", Some((Section::Albums, Route::Albums))),
        (Icon::Library, "Library", Some((Section::Library, Route::Library(None)))),
        (Icon::Folder, "Folders", Some((Section::Folders, Route::Folders))),
        (
            Icon::ListMusic,
            "Playlists",
            Some((Section::Playlists, Route::Playlists)),
        ),
        (Icon::ListOrdered, "Queue", Some((Section::Queue, Route::Queue))),
    ];

    let mut top = column![].spacing(4).align_x(Alignment::Center);
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
    .width(55)
    .height(Length::Fill)
    .padding(Padding::from([8.0, 0.0]));

    row![
        container(rail).style(theme::sidebar).height(Length::Fill),
        container(vline::<Message>()).style(theme::sidebar_divider),
    ]
    .height(Length::Fill)
    .into()
}
