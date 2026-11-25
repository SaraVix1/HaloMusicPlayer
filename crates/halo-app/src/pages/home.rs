//! Home: a greeting with a search button, then rows of album covers —
//! Recently played, Most played, Recently added — each with "See all".
//! The start page; an empty library shows a prompt to add a music folder.

use halo_backend::commands::library::{self, Album, HomeShelves};
use iced::widget::{button, column, container, responsive, row, text};
use iced::{Alignment, Element, Padding, Task};

use super::albums::album_card;
use super::common::{self, note};
use super::Msg;
use crate::app::{bg, Halo, Message};
use crate::components::palette;
use crate::icons::{icon, Icon};
use crate::route::Route;
use crate::theme::{self, t};
use crate::ui;

/// Albums fetched per row; a row shows as many as fit the window.
const SHELF_LIMIT: usize = 12;
/// Card width bounds and gap (as the Albums grid).
const CARD_MIN: f32 = 160.0;
const CARD_MAX: f32 = 200.0;
const GAP: f32 = 24.0;
const PX: f32 = 24.0;
/// Built-in "Recently Added" smart playlist (migration 005).
const RECENTLY_ADDED_PLAYLIST: i64 = 1;

#[derive(Default)]
pub struct HomeState {
    pub shelves: HomeShelves,
    pub loading: bool,
    pub loaded: bool,
    pub error: Option<String>,
}

pub fn load(app: &mut Halo) -> Task<Message> {
    let st = &mut app.pages.home;
    st.loading = !st.loaded;
    bg(
        &app.backend,
        |b| library::get_home_shelves(SHELF_LIMIT, b.db()),
        |r| Message::Page(Msg::HomeLoaded(r)),
    )
}

pub fn on_loaded(app: &mut Halo, r: Result<HomeShelves, String>) {
    let st = &mut app.pages.home;
    st.loading = false;
    st.loaded = true;
    match r {
        Ok(s) => {
            st.shelves = s;
            st.error = None;
        }
        Err(e) => st.error = Some(e),
    }
}

/// "Good morning" 5–11, "Good afternoon" 12–16, otherwise "Good evening".
pub fn greeting(hour: Option<u8>) -> &'static str {
    match hour {
        Some(5..=11) => "Good morning",
        Some(12..=16) => "Good afternoon",
        Some(_) => "Good evening",
        None => "Welcome back",
    }
}

/// How many cards fit in `width`, and how wide each one is.
fn columns(width: f32) -> (usize, f32) {
    let n = (((width + GAP) / (CARD_MIN + GAP)).floor() as usize).max(1);
    let w = ((width - GAP * (n as f32 - 1.0)) / n as f32).clamp(0.0, CARD_MAX);
    (n, w)
}

pub fn view(app: &Halo) -> Element<'_, Message> {
    let hello = greeting(halo_backend::platform::local_hour());
    let st = &app.pages.home;
    responsive(move |size| {
        let (n, side) = columns(size.width - PX * 2.0);
        let header = row![ui::h1(hello), ui::fill(), search_button()]
            .align_y(Alignment::Center)
            .spacing(16);
        let mut col = column![header].spacing(40);
        if st.loading {
            col = col.push(note("Loading…"));
        } else if let Some(e) = &st.error {
            col = col.push(common::error_text(e));
        } else if st.loaded && st.shelves.library_tracks == 0 {
            col = col.push(common::library_empty());
        } else {
            let s = &st.shelves;
            for (title, albums, all) in [
                ("Recently played", &s.recently_played, Route::RecentlyPlayed),
                ("Most played", &s.most_played, Route::MostPlayed),
                ("Recently added", &s.recently_added, Route::SmartPlaylist(RECENTLY_ADDED_PLAYLIST)),
            ] {
                if !albums.is_empty() {
                    col = col.push(shelf(title, albums, all, n, side));
                }
            }
        }
        common::scroll_page(app.route().scroll_key(), container(col).padding(PX))
    })
    .into()
}

/// Wide search button that opens the Ctrl+K palette.
fn search_button<'a>() -> Element<'a, Message> {
    let content = row![
        icon(Icon::Search, 14.0),
        text("Search").size(theme::BODY),
        ui::hspace(24.0),
        ui::kbd("Ctrl+K"),
    ]
    .spacing(8)
    .align_y(Alignment::Center);
    button(content)
        .padding(Padding::from([6.0, 12.0]))
        .style(theme::btn_outline)
        .on_press(Message::Palette(palette::Msg::Open))
        .into()
}

/// Title + "See all", then one row of up to `n` album cards.
fn shelf<'a>(title: &'a str, albums: &'a [Album], all: Route, n: usize, side: f32) -> Element<'a, Message> {
    let k = t();
    let see_all = button(
        row![text("See all").size(theme::BODY), icon(Icon::ArrowRight, 14.0)]
            .spacing(4)
            .align_y(Alignment::Center),
    )
    .padding(Padding::from([4.0, 8.0]))
    .style(theme::btn_text(k.muted_fg, k.foreground))
    .on_press(Message::Navigate(all));
    let head = row![
        text(title).size(theme::TITLE).font(theme::SEMIBOLD),
        ui::fill(),
        see_all,
    ]
    .align_y(Alignment::Center);

    let mut cards = row![].spacing(GAP);
    for al in albums.iter().take(n) {
        cards = cards.push(container(album_card(al, side)).width(side));
    }
    column![head, cards].spacing(12).into()
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn greeting_by_hour() {
        assert_eq!(greeting(Some(5)), "Good morning");
        assert_eq!(greeting(Some(11)), "Good morning");
        assert_eq!(greeting(Some(12)), "Good afternoon");
        assert_eq!(greeting(Some(16)), "Good afternoon");
        assert_eq!(greeting(Some(17)), "Good evening");
        assert_eq!(greeting(Some(2)), "Good evening");
        assert_eq!(greeting(None), "Welcome back");
    }

    #[test]
    fn columns_fit_width() {
        // 1100 px: 5 × 160 + 4 × 20 = 880 fits, 6 × 160 + 5 × 20 = 1060 fits.
        let (n, w) = columns(1100.0);
        assert_eq!(n, 6);
        assert!(w >= CARD_MIN && w <= CARD_MAX);
        assert_eq!(columns(100.0).0, 1);
        // Very wide: cards stop growing at CARD_MAX.
        assert_eq!(columns(5000.0).1, columns(5000.0).1.min(CARD_MAX));
    }
}
