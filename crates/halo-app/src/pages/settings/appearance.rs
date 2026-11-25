//! Settings → Appearance: Light / Dark / System theme segmented control and
//! the "Accent from album art" switch.

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
    SetAccentFromArt(bool),
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
        Msg::SetAccentFromArt(on) => {
            app.accent_from_art = on;
            let save = bg(
                &app.backend,
                move |b| ui::set_pref(crate::app::ACCENT_PREF.into(), on.to_string(), b.db()),
                done,
            );
            Task::batch([save, app.refresh_accent()])
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
                row![icon(ic, 14.0), text(label).size(theme::BODY)]
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
        setting_row(
            "Song lists",
            "How Library shows songs. Album pages always use the list.",
            song_view(app),
        ),
        setting_row(
            "Accent from album art",
            "Tint buttons, progress and highlights with the current cover's colour. Off: Halo blue.",
            crate::ui::toggle(
                app.accent_from_art,
                msg(SMsg::Appearance(Msg::SetAccentFromArt(!app.accent_from_art))),
            ),
        ),
    ]
    .into()
}

/// Cards / List: the same preference as the ☰ / ▦ toggle in Library.
fn song_view(app: &Halo) -> Element<'_, Message> {
    use crate::pages::library::{LMsg, SongsView};
    let cur = app.pages.library.songs_view;
    let mut seg = row![].spacing(0);
    for (v, label, ic) in [(SongsView::Cards, "Cards", Icon::LayoutGrid), (SongsView::List, "List", Icon::List)] {
        let active = cur == v;
        seg = seg.push(
            button(row![icon(ic, 14.0), text(label).size(theme::BODY)].spacing(6).align_y(Alignment::Center))
                .padding(Padding::from([6.0, 12.0]))
                .style(move |_, s| {
                    let k = t();
                    button::Style {
                        background: active.then(|| k.muted.into()),
                        text_color: if active || matches!(s, button::Status::Hovered) { k.foreground } else { k.muted_fg },
                        border: theme::no_border(theme::R_XS),
                        ..Default::default()
                    }
                })
                .on_press(Message::Page(crate::pages::Msg::Library(LMsg::SongsView(v)))),
        );
    }
    container(seg).padding(2).style(theme::bordered(theme::R_MD)).into()
}

