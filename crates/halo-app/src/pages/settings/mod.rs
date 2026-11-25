//! Settings page (`settings.tsx`): category nav on the left (192px), one
//! file per category on the right (max-w-2xl, own scroll position each):
//!
//! | Tab | File |
//! |---|---|
//! | General | `general.rs` |
//! | Appearance | `appearance.rs` |
//! | Playback | `playback.rs` |
//! | Equalizer | `equalizer.rs` |
//! | Library | `library.rs` |
//! | Advanced | `advanced.rs` (+ Last.fm panel in `lastfm.rs`) |
//!
//! Shared layout helpers (`section_header`, `setting_row`) live here.

pub mod advanced;
pub mod appearance;
pub mod equalizer;
pub mod general;
pub mod lastfm;
pub mod library;
pub mod playback;

use halo_backend::commands::eq::EqConfig;
use halo_backend::scanner::ScanProgress;
use iced::widget::{button, column, container, row, text};
use iced::{Alignment, Element, Length, Padding, Subscription, Task};

use super::common;
use super::dialogs::ConfirmAction;
use crate::app::{Halo, Message};
use crate::icons::{icon, Icon};
use crate::route::{Route, SettingsTab};
use crate::theme::{self, a, t};
use crate::ui::{self, hline, vline};

#[derive(Default)]
pub struct SettingsState {
    pub last_tab: SettingsTab,
    pub error: Option<String>,
    pub general: general::State,
    pub playback: playback::State,
    pub eq: equalizer::State,
    pub library: library::State,
    pub lastfm: lastfm::State,
}

#[derive(Debug, Clone)]
pub enum Msg {
    Error(String),
    General(general::Msg),
    Appearance(appearance::Msg),
    Playback(playback::Msg),
    Eq(equalizer::Msg),
    Library(library::Msg),
    Advanced(advanced::Msg),
    LastFm(lastfm::Msg),
}

pub fn msg(m: Msg) -> Message {
    Message::Page(super::Msg::Settings(m))
}

pub fn err(e: String) -> Message {
    msg(Msg::Error(e))
}

/// Map a backend result to "nothing" or a settings error.
pub fn done(r: Result<(), String>) -> Message {
    match r {
        Ok(()) => Message::Noop,
        Err(e) => err(e),
    }
}

pub fn update(app: &mut Halo, m: Msg) -> Task<Message> {
    match m {
        Msg::Error(e) => {
            app.pages.settings.error = Some(e);
            Task::none()
        }
        Msg::General(m) => general::update(app, m),
        Msg::Appearance(m) => appearance::update(app, m),
        Msg::Playback(m) => playback::update(app, m),
        Msg::Eq(m) => equalizer::update(app, m),
        Msg::Library(m) => library::update(app, m),
        Msg::Advanced(m) => advanced::update(app, m),
        Msg::LastFm(m) => lastfm::update(app, m),
    }
}

/// Entering Settings loads every category's values (as the React page did
/// on mount), so switching tabs is instant.
pub fn on_enter(app: &mut Halo, _tab: SettingsTab) -> Task<Message> {
    Task::batch([
        general::load(app),
        playback::load(app),
        equalizer::load(app),
        library::load(app),
        lastfm::load(app),
    ])
}

pub fn on_confirm(app: &mut Halo, a: ConfirmAction) -> Task<Message> {
    advanced::on_confirm(app, a)
}

pub fn on_scan_progress(app: &mut Halo, p: ScanProgress) -> Task<Message> {
    library::on_progress(app, p)
}

pub fn on_eq_changed(app: &mut Halo, cfg: EqConfig) -> Task<Message> {
    equalizer::on_changed(app, cfg)
}

pub fn subscription(app: &Halo) -> Option<Subscription<Message>> {
    let subs: Vec<_> = [library::subscription(app), lastfm::subscription(app)]
        .into_iter()
        .flatten()
        .collect();
    (!subs.is_empty()).then(|| Subscription::batch(subs))
}

// ── Shared layout helpers ─────────────────────────────────────────────────────

/// `SectionHeader`: base semibold title, sm muted description, separator.
pub fn section_header<'a>(title: &'a str, description: &'a str) -> Element<'a, Message> {
    column![
        text(title).size(theme::BODY).font(theme::SEMIBOLD),
        ui::muted(description, theme::BODY),
        ui::vspace(10.0),
        hline(),
    ]
    .spacing(2)
    .padding(Padding::ZERO.bottom(24.0))
    .into()
}

/// `SettingRow`: label + description on the left, control on the right,
/// `py-3`, bottom border at 50%.
pub fn setting_row<'a>(
    label: &'a str,
    description: &'a str,
    control: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    let left = column![text(label).size(theme::BODY).font(theme::MEDIUM)]
        .spacing(2)
        .width(Length::Fill);
    let left = if description.is_empty() {
        left
    } else {
        left.push(ui::muted(description, theme::SMALL))
    };
    column![
        row![left, control.into()]
            .spacing(24)
            .align_y(Alignment::Center)
            .padding(Padding::from([12.0, 0.0])),
        ui::hline_alpha(0.5),
    ]
    .into()
}

/// Bold sub-section label (`text-sm font-medium`).
pub fn label<'a>(s: &'a str) -> Element<'a, Message> {
    text(s).size(theme::BODY).font(theme::MEDIUM).into()
}

/// shadcn `Button variant="outline" size="sm"` with an optional icon.
pub fn outline_btn<'a>(
    lead: Option<Element<'a, Message>>,
    label: impl text::IntoFragment<'a>,
    on_press: Option<Message>,
) -> Element<'a, Message> {
    sm_btn(lead, label, theme::btn_outline, on_press)
}

pub fn sm_btn<'a>(
    lead: Option<Element<'a, Message>>,
    label: impl text::IntoFragment<'a>,
    style: fn(&iced::Theme, button::Status) -> button::Style,
    on_press: Option<Message>,
) -> Element<'a, Message> {
    let mut r = row![].spacing(8).align_y(Alignment::Center);
    if let Some(l) = lead {
        r = r.push(l);
    }
    r = r.push(text(label).size(theme::BODY).font(theme::MEDIUM));
    button(container(r).height(28).align_y(Alignment::Center))
        .padding(Padding::from([0.0, 10.0]))
        .style(move |th, s| {
            let mut st = style(th, s);
            st.border.radius = theme::radius(theme::R_MD);
            st
        })
        .on_press_maybe(on_press)
        .into()
}

const CATEGORIES: [(SettingsTab, Icon); 6] = [
    (SettingsTab::General, Icon::Settings2),
    (SettingsTab::Appearance, Icon::Palette),
    (SettingsTab::Playback, Icon::Play),
    (SettingsTab::Equalizer, Icon::SlidersHorizontal),
    (SettingsTab::Library, Icon::LibraryBig),
    (SettingsTab::Advanced, Icon::Wrench),
];

pub fn view(app: &Halo, tab: SettingsTab) -> Element<'_, Message> {
    let k = t();
    let mut nav = column![].spacing(2).padding(8).width(192);
    for (c, ic) in CATEGORIES {
        let active = c == tab;
        nav = nav.push(
            button(
                row![
                    icon(ic, 15.0),
                    text(c.label())
                        .size(theme::BODY)
                        .font(if active { theme::MEDIUM } else { theme::SANS })
                ]
                .spacing(10)
                .align_y(Alignment::Center),
            )
            .width(Length::Fill)
            .padding(Padding::from([8.0, 12.0]))
            .style(move |_, s| {
                let k = t();
                let hovered = matches!(s, button::Status::Hovered | button::Status::Pressed);
                button::Style {
                    background: if active {
                        Some(k.muted.into())
                    } else if hovered {
                        Some(a(k.muted, 0.6).into())
                    } else {
                        None
                    },
                    text_color: if active || hovered { k.foreground } else { k.muted_fg },
                    border: theme::no_border(theme::R_MD),
                    ..Default::default()
                }
            })
            .on_press(Message::ReplaceRoute(Route::Settings(Some(c)))),
        );
    }

    let section: Element<'_, Message> = match tab {
        SettingsTab::General => general::view(app),
        SettingsTab::Appearance => appearance::view(app),
        SettingsTab::Playback => playback::view(app),
        SettingsTab::Equalizer => equalizer::view(app),
        SettingsTab::Library => library::view(app),
        SettingsTab::Advanced => advanced::view(app),
    };
    let mut body = column![].spacing(0);
    if let Some(e) = &app.pages.settings.error {
        body = body.push(
            container(text(e.as_str()).size(theme::BODY).color(k.destructive))
                .padding(Padding::ZERO.bottom(16.0)),
        );
    }
    body = body.push(section);
    let content = common::scroll_page(
        format!("settings:{tab:?}"),
        container(container(body).max_width(672)).padding(24),
    );

    row![
        container(common::scroll_page("settings:nav".into(), nav)).width(192),
        vline(),
        container(content).width(Length::Fill).height(Length::Fill),
    ]
    .height(Length::Fill)
    .into()
}
