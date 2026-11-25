//! Settings → General: close-button behaviour and resume-on-launch.

use halo_backend::commands::ui;
use iced::widget::{column, pick_list};
use iced::{Element, Padding, Task};

use super::{done, msg, section_header, setting_row, Msg as SMsg};
use crate::app::{bg, Halo, Message};
use crate::theme;

const CLOSE_KEY: &str = "ui.close_behavior";
const RESUME_KEY: &str = "playback.resume_on_launch";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CloseOpt(&'static str, &'static str);

impl std::fmt::Display for CloseOpt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.1)
    }
}

const CLOSE_OPTS: [CloseOpt; 2] = [
    CloseOpt("minimize", "Minimize to tray"),
    CloseOpt("quit", "Quit Halo"),
];

pub struct State {
    pub close_behavior: String,
    pub resume_on_launch: bool,
}

impl Default for State {
    fn default() -> Self {
        Self {
            close_behavior: "minimize".into(),
            resume_on_launch: false,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Msg {
    Loaded(Option<String>, Option<String>),
    CloseBehavior(CloseOpt),
    ResumeOnLaunch(bool),
}

fn gm(m: Msg) -> Message {
    msg(SMsg::General(m))
}

pub fn load(app: &mut Halo) -> Task<Message> {
    bg(
        &app.backend,
        |b| {
            (
                ui::get_pref(CLOSE_KEY.into(), b.db()).ok().flatten(),
                ui::get_pref(RESUME_KEY.into(), b.db()).ok().flatten(),
            )
        },
        |(c, r)| gm(Msg::Loaded(c, r)),
    )
}

pub fn update(app: &mut Halo, m: Msg) -> Task<Message> {
    let st = &mut app.pages.settings.general;
    match m {
        Msg::Loaded(c, r) => {
            st.close_behavior = c.unwrap_or_else(|| "minimize".into());
            st.resume_on_launch = r.as_deref() == Some("true");
            Task::none()
        }
        Msg::CloseBehavior(o) => {
            st.close_behavior = o.0.into();
            let v = o.0.to_string();
            bg(&app.backend, move |b| ui::set_pref(CLOSE_KEY.into(), v, b.db()), done)
        }
        Msg::ResumeOnLaunch(v) => {
            st.resume_on_launch = v;
            bg(
                &app.backend,
                move |b| ui::set_pref(RESUME_KEY.into(), v.to_string(), b.db()),
                done,
            )
        }
    }
}

pub fn view(app: &Halo) -> Element<'_, Message> {
    let st = &app.pages.settings.general;
    let selected = CLOSE_OPTS.iter().find(|o| o.0 == st.close_behavior).copied();
    column![
        section_header("General", "Startup behaviour and window management."),
        setting_row(
            "Close button",
            "What happens when you click the × in the title bar.",
            pick_list(&CLOSE_OPTS[..], selected, |o| gm(Msg::CloseBehavior(o)))
                .text_size(theme::BODY)
                .padding(Padding::from([4.0, 8.0]))
                .width(160)
                .style(theme::pick)
                .menu_style(theme::pick_menu),
        ),
        setting_row(
            "Resume last session on launch",
            "Continue playback from where you left off when Halo starts.",
            crate::ui::toggle(st.resume_on_launch, gm(Msg::ResumeOnLaunch(!st.resume_on_launch))),
        ),
    ]
    .into()
}
