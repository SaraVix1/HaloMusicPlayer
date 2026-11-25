//! Settings → Appearance: Light / Dark / System theme, song list view,
//! "Accent from album art" and the renderer (GPU / Software).

use halo_backend::commands::ui;
use iced::widget::{button, column, container, row, text};
use iced::{Alignment, Element, Padding, Task};

use super::{done, msg, section_header, setting_row, Msg as SMsg};
use crate::app::{bg, Halo, Message};
use crate::icons::{icon, Icon};
use crate::renderer::{self, Renderer};
use crate::theme::{self, t};

#[derive(Debug, Clone)]
pub enum Msg {
    Theme(&'static str),
    AccentFromArt(bool),
    Renderer(Renderer),
}

pub fn update(app: &mut Halo, m: Msg) -> Task<Message> {
    match m {
        Msg::Theme(th) => {
            app.set_theme_pref(th.to_string());
            bg(
                &app.backend,
                move |b| ui::set_theme(th.to_string(), b.clone(), b.db()),
                done,
            )
        }
        Msg::AccentFromArt(on) => {
            app.accent_from_art = on;
            let save = bg(
                &app.backend,
                move |b| ui::set_pref(crate::app::ACCENT_PREF.into(), on.to_string(), b.db()),
                done,
            );
            Task::batch([save, app.refresh_accent()])
        }
        Msg::Renderer(r) => {
            app.renderer = r;
            bg(&app.backend, move |b| ui::set_pref(renderer::PREF.into(), r.as_pref().into(), b.db()), done)
        }
    }
}

pub fn view(app: &Halo) -> Element<'_, Message> {
    let theme_seg = segmented(
        [("light", "Light", Icon::Sun), ("dark", "Dark", Icon::Moon), ("system", "System", Icon::Monitor)]
            .into_iter()
            .map(|(value, label, ic)| {
                (label, Some(ic), app.theme_pref == value, msg(SMsg::Appearance(Msg::Theme(value))))
            })
            .collect(),
    );
    let render_seg = segmented(
        [(Renderer::Gpu, "GPU"), (Renderer::Software, "Software")]
            .into_iter()
            .map(|(r, label)| (label, None, app.renderer == r, msg(SMsg::Appearance(Msg::Renderer(r)))))
            .collect(),
    );
    let render_desc = if app.renderer == renderer::at_launch() {
        "GPU: smooth animations, more memory. Software: about a third of the memory, more CPU while things animate. Applies the next time Halo starts."
    } else {
        "Restart Halo to switch renderers."
    };
    column![
        section_header("Appearance", "Colour scheme and visual style."),
        setting_row("Theme", "Choose how Halo looks.", theme_seg),
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
                msg(SMsg::Appearance(Msg::AccentFromArt(!app.accent_from_art))),
            ),
        ),
        setting_row("Renderer", render_desc, render_seg),
    ]
    .into()
}

/// Cards / List: the same preference as the ☰ / ▦ toggle in Library.
fn song_view(app: &Halo) -> Element<'_, Message> {
    use crate::pages::library::{LMsg, SongsView};
    let cur = app.pages.library.songs_view;
    segmented(
        [(SongsView::Cards, "Cards", Icon::LayoutGrid), (SongsView::List, "List", Icon::List)]
            .into_iter()
            .map(|(v, label, ic)| {
                (label, Some(ic), cur == v, Message::Page(crate::pages::Msg::Library(LMsg::SongsView(v))))
            })
            .collect(),
    )
}

/// Segmented control: one `(label, icon, active, on_press)` per option.
fn segmented<'a>(items: Vec<(&'static str, Option<Icon>, bool, Message)>) -> Element<'a, Message> {
    let mut seg = row![].spacing(0);
    for (label, ic, active, on_press) in items {
        let mut content = row![].spacing(6).align_y(Alignment::Center);
        if let Some(ic) = ic {
            content = content.push(icon(ic, 14.0));
        }
        seg = seg.push(
            button(content.push(text(label).size(theme::BODY)))
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
                .on_press(on_press),
        );
    }
    container(seg).padding(2).style(theme::bordered(theme::R_MD)).into()
}
