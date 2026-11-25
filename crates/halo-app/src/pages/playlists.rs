//! Unified playlists list (`playlists.tsx`): manual + smart playlists in one
//! grid (smart badged ✨, built-ins locked), All / Manual / Smart filter,
//! "New ▾" menu with inline creation.

use halo_backend::commands::playlists::{self, Playlist};
use halo_backend::commands::smart_playlists::{self, SmartPlaylist};
use iced::widget::{button, center, column, container, row, text, text_input};
use iced::{Alignment, Element, Length, Padding, Task};

use super::common::{self, note};
use super::Msg;
use crate::app::{bg, Halo, Message};
use crate::format::plural;
use crate::icons::{icon, Icon};
use crate::route::Route;
use crate::theme::{self, a, t};
use crate::ui::{self, small_button};
use crate::widgets::{line, popover, surface, Placement};

pub const BUILTIN_IDS: [i64; 5] = [1, 2, 3, 4, 5];
pub const NEW_NAME_INPUT: &str = "playlists-new-name";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Filter {
    #[default]
    All,
    Manual,
    Smart,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NewKind {
    Manual,
    Smart,
}

#[derive(Default)]
pub struct PlaylistsState {
    pub manual: Vec<Playlist>,
    pub smart: Vec<SmartPlaylist>,
    pub loading: bool,
    pub loaded: bool,
    pub error: Option<String>,
    pub filter: Filter,
    pub menu_open: bool,
    pub creating: Option<NewKind>,
    pub new_name: String,
}

#[derive(Debug, Clone)]
pub enum PMsg {
    Loaded(Result<(Vec<Playlist>, Vec<SmartPlaylist>), String>),
    Filter(Filter),
    ToggleMenu,
    CloseMenu,
    StartCreate(NewKind),
    CancelCreate,
    NameChanged(String),
    Create,
    Created(Result<Route, String>),
}

fn pm(m: PMsg) -> Message {
    Message::Page(Msg::Playlists(m))
}

pub fn load(app: &mut Halo) -> Task<Message> {
    app.pages.playlists.loading = !app.pages.playlists.loaded;
    bg(
        &app.backend,
        |b| {
            Ok((
                playlists::get_playlists(b.db())?,
                smart_playlists::get_smart_playlists(b.db())?,
            ))
        },
        |r| pm(PMsg::Loaded(r)),
    )
}

pub fn update(app: &mut Halo, msg: PMsg) -> Task<Message> {
    let st = &mut app.pages.playlists;
    match msg {
        PMsg::Loaded(r) => {
            st.loading = false;
            st.loaded = true;
            match r {
                Ok((m, s)) => {
                    st.manual = m;
                    st.smart = s;
                    st.error = None;
                }
                Err(e) => st.error = Some(e),
            }
            Task::none()
        }
        PMsg::Filter(f) => {
            st.filter = f;
            Task::none()
        }
        PMsg::ToggleMenu => {
            st.menu_open = !st.menu_open;
            Task::none()
        }
        PMsg::CloseMenu => {
            st.menu_open = false;
            Task::none()
        }
        PMsg::StartCreate(kind) => {
            st.menu_open = false;
            st.creating = Some(kind);
            st.new_name.clear();
            iced::widget::operation::focus(NEW_NAME_INPUT)
        }
        PMsg::CancelCreate => {
            st.creating = None;
            Task::none()
        }
        PMsg::NameChanged(s) => {
            st.new_name = s;
            Task::none()
        }
        PMsg::Create => {
            let name = st.new_name.trim().to_string();
            let Some(kind) = st.creating else {
                return Task::none();
            };
            if name.is_empty() {
                return Task::none();
            }
            bg(
                &app.backend,
                move |b| match kind {
                    NewKind::Manual => {
                        playlists::create_playlist(name, b.db()).map(|p| Route::Playlist(p.id))
                    }
                    NewKind::Smart => smart_playlists::create_smart_playlist(b.db(), name)
                        .map(|p| Route::SmartPlaylist(p.id)),
                },
                |r| pm(PMsg::Created(r)),
            )
        }
        PMsg::Created(r) => match r {
            Ok(route) => {
                st.creating = None;
                st.new_name.clear();
                app.update(Message::Navigate(route))
            }
            Err(e) => {
                st.error = Some(e);
                Task::none()
            }
        },
    }
}

fn builtin_icon(name: &str) -> Option<Icon> {
    Some(match name {
        "Recently Added" => Icon::CalendarPlus,
        "Most Played" => Icon::TrendingUp,
        "Top Rated" => Icon::Star,
        "Never Played" => Icon::VolumeX,
        "Recently Played" => Icon::History,
        _ => return None,
    })
}

fn smart_subtitle(name: &str, match_mode: &str) -> String {
    match name {
        "Recently Added" => "Added in the last 30 days".into(),
        "Most Played" => "Top 50 by play count".into(),
        "Top Rated" => "Rated 4 stars or above".into(),
        "Never Played" => "Never been played".into(),
        "Recently Played" => "Played in the last 7 days".into(),
        _ if match_mode == "all" => "Matches all rules".into(),
        _ => "Matches any rule".into(),
    }
}

fn card<'a>(
    name: &'a str,
    sub: String,
    ic: Icon,
    smart: bool,
    locked: bool,
    route: Route,
) -> Element<'a, Message> {
    let k = t();
    let mut title = row![line(name).size(theme::BODY).font(theme::MEDIUM)]
        .spacing(6)
        .align_y(Alignment::Center);
    if smart {
        title = title.push(icon(Icon::Sparkles, 11.0).color(a(k.primary, 0.7)));
    }
    let mut content = row![
        center(icon(ic, 20.0).color(a(k.muted_fg, 0.6)))
            .width(48)
            .height(48)
            .style(|_| iced::widget::container::Style {
                background: Some(t().muted.into()),
                border: theme::no_border(theme::R_LG),
                ..Default::default()
            }),
        column![
            title,
            line(sub).size(theme::SMALL).color(k.muted_fg).width(Length::Fill),
        ]
        .width(Length::Fill),
    ]
    .spacing(12)
    .align_y(Alignment::Center);
    if locked {
        content = content.push(icon(Icon::Lock, 12.0).color(a(k.muted_fg, 0.4)));
    }
    surface(content)
        .width(Length::Fill)
        .padding(12)
        .on_press(Message::Navigate(route))
        .interaction(iced::mouse::Interaction::Pointer)
        .style(|s| {
            let k = t();
            iced::widget::container::Style {
                background: s.hovered.then(|| a(k.muted, 0.3).into()),
                border: theme::border(k.border, 1.0, theme::R_LG),
                text_color: Some(k.foreground),
                ..Default::default()
            }
        })
        .into()
}

pub fn view(app: &Halo) -> Element<'_, Message> {
    let k = t();
    let st = &app.pages.playlists;
    let total = st.manual.len() + st.smart.len();

    // ── Header with "New ▾" menu ──────────────────────────────────────────────
    let new_btn = button(
        row![
            icon(Icon::Plus, 14.0),
            text("New").size(theme::SMALL),
            icon(Icon::ChevronDown, 12.0)
        ]
        .spacing(6)
        .align_y(Alignment::Center),
    )
    .padding(Padding::from([4.0, 10.0]))
    .style(|th, s| {
        let mut st = theme::btn_primary(th, s);
        st.border.radius = theme::radius(theme::R_MD);
        st
    })
    .on_press(pm(PMsg::ToggleMenu));
    let menu = st.menu_open.then(|| {
        let item = |ic: Icon, label: &'static str, kind: NewKind| {
            button(
                row![icon(ic, 14.0), text(label).size(theme::SMALL)]
                    .spacing(8)
                    .align_y(Alignment::Center),
            )
            .width(Length::Fill)
            .padding(Padding::from([6.0, 12.0]))
            .style(theme::btn_list_item(false, 0.0))
            .on_press(pm(PMsg::StartCreate(kind)))
        };
        Element::from(
            container(
                column![
                    item(Icon::ListMusic, "New playlist", NewKind::Manual),
                    item(Icon::Sparkles, "New smart playlist", NewKind::Smart),
                ]
                .width(176),
            )
            .padding(Padding::from([4.0, 0.0]))
            .style(|th| {
                let mut s = theme::popover(th);
                s.border.radius = theme::radius(theme::R_LG);
                s
            }),
        )
    });
    let new_menu = popover(new_btn, menu)
        .placement(Placement::BelowEnd)
        .gap(4.0)
        .on_dismiss(pm(PMsg::CloseMenu));

    let header = common::header(
        "Playlists",
        row![common::count(total, "playlist", "playlists"), new_menu]
            .spacing(12)
            .align_y(Alignment::Center),
    );

    // ── Filter ────────────────────────────────────────────────────────────────
    let mut filters = row![].spacing(4);
    for (f, label) in [(Filter::All, "All"), (Filter::Manual, "Manual"), (Filter::Smart, "Smart")] {
        filters = filters.push(
            button(text(label).size(theme::SMALL))
                .padding(Padding::from([4.0, 10.0]))
                .style(theme::btn_segment(st.filter == f))
                .on_press(pm(PMsg::Filter(f))),
        );
    }

    let mut col = column![header, filters].spacing(16);

    // ── Inline create ────────────────────────────────────────────────────────
    if let Some(kind) = st.creating {
        let can = !st.new_name.trim().is_empty();
        col = col.push(
            container(
                row![
                    icon(if kind == NewKind::Smart { Icon::Sparkles } else { Icon::ListMusic }, 16.0),
                    text_input(
                        if kind == NewKind::Smart { "Smart playlist name" } else { "Playlist name" },
                        &st.new_name
                    )
                    .id(NEW_NAME_INPUT)
                    .on_input(|s| pm(PMsg::NameChanged(s)))
                    .on_submit(pm(PMsg::Create))
                    .size(theme::BODY)
                    .padding(Padding::from([4.0, 0.0]))
                    .style(theme::input_underline)
                    .width(Length::Fill),
                    small_button("Create", true, can.then(|| pm(PMsg::Create))),
                    small_button("Cancel", false, Some(pm(PMsg::CancelCreate))),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
            )
            .padding(12)
            .style(theme::bordered(theme::R_MD)),
        );
    }
    if let Some(e) = &st.error {
        col = col.push(common::error_text(e));
    }

    // ── Cards ─────────────────────────────────────────────────────────────────
    let empty = (st.filter == Filter::Smart || st.manual.is_empty())
        && (st.filter == Filter::Manual || st.smart.is_empty());
    if st.loading {
        col = col.push(note("Loading…"));
    } else if empty {
        col = col.push(note(if st.filter == Filter::Smart {
            "No smart playlists yet."
        } else {
            "No playlists yet. Create one to organize tracks."
        }));
    } else {
        col = col.push(card_grid(app));
    }
    let _ = k;
    common::scroll_page("playlists".into(), container(col).padding(24))
}

/// Builds the card grid with a fixed column count derived from the window
/// width (auto-fill, minmax(220px, 1fr), gap 12).
fn card_grid(app: &Halo) -> Element<'_, Message> {
    let st = &app.pages.playlists;
    let mut items: Vec<Element<'_, Message>> = vec![];
    if st.filter != Filter::Smart {
        for p in &st.manual {
            items.push(card(
                &p.name,
                plural(p.track_count, "track", "tracks"),
                Icon::ListMusic,
                false,
                false,
                Route::Playlist(p.id),
            ));
        }
    }
    if st.filter != Filter::Manual {
        for p in &st.smart {
            items.push(card(
                &p.name,
                smart_subtitle(&p.name, &p.match_mode),
                builtin_icon(&p.name).unwrap_or(Icon::Sparkles),
                true,
                BUILTIN_IDS.contains(&p.id),
                Route::SmartPlaylist(p.id),
            ));
        }
    }
    // Content width = window − sidebar (56) − page padding (48).
    let width = app.main_size.width - 56.0 - 48.0;
    let cols = (((width + 12.0) / (220.0 + 12.0)).floor() as usize).max(1);
    let mut grid = column![].spacing(12);
    let mut it = items.into_iter().peekable();
    while it.peek().is_some() {
        let mut r = row![].spacing(12);
        for _ in 0..cols {
            match it.next() {
                Some(el) => r = r.push(container(el).width(Length::FillPortion(1))),
                None => r = r.push(ui::fill()),
            }
        }
        grid = grid.push(r);
    }
    grid.into()
}
