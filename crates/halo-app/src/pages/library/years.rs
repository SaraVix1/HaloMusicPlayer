//! Years tab / page: year tiles linking to `/songs?year=`.

use halo_backend::commands::library::{TracksQuery, YearStat};
use iced::widget::{column, text, Space};
use iced::{Element, Length};

use super::super::common::GridSpec;
use super::cards::TILES;
use super::{coll_grid, standalone};
use crate::app::{Halo, Message};
use crate::format::plural;
use crate::route::Route;
use crate::theme::{self, a, t};
use crate::widgets::surface;

/// `minItemWidth 160, rowHeight 108, gap 12`.
const YEARS: GridSpec = GridSpec {
    min_item_width: 160.0,
    ..TILES
};

fn year_card<'a>(y: &YearStat) -> Element<'a, Message> {
    let year = y.year;
    let content = column![
        text(year.to_string()).size(theme::TITLE).font(theme::SEMIBOLD),
        Space::new().height(Length::Fill),
        text(plural(y.track_count, "track", "tracks"))
            .size(theme::SMALL)
            .color(t().muted_fg),
    ]
    .height(Length::Fill);
    surface(content)
        .width(Length::Fill)
        .height(96)
        .padding(16)
        .on_press(Message::Navigate(Route::Songs(TracksQuery {
            year: Some(year),
            ..TracksQuery::default()
        })))
        .interaction(iced::mouse::Interaction::Pointer)
        .style(|s| {
            let k = t();
            iced::widget::container::Style {
                background: Some(a(k.muted, if s.hovered { 0.4 } else { 0.2 }).into()),
                border: theme::border(
                    if s.hovered { a(k.foreground, 0.2) } else { k.border },
                    1.0,
                    theme::R_LG,
                ),
                text_color: Some(k.foreground),
                ..Default::default()
            }
        })
        .into()
}

pub fn grid(app: &Halo, key: String, padding_top: Option<f32>) -> Element<'_, Message> {
    coll_grid(
        &app.pages.library.years,
        "No year tags found — scan a folder to get started.",
        YEARS,
        key,
        &app.pages.scroll,
        padding_top,
        |y: &YearStat, _w: f32| year_card(y),
    )
}

pub fn view(app: &Halo) -> Element<'_, Message> {
    let n = app.pages.library.years.items.len();
    standalone(app, "Years", n, "year", "years", grid(app, app.route().scroll_key(), Some(0.0)))
}
