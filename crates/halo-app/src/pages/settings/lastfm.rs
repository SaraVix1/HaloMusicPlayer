//! Last.fm panel (inside Settings → Advanced): app API key + shared secret,
//! browser sign-in (Connect → Open in browser / Copy link, auto-polls
//! complete-auth every 3s for up to 2 minutes), connected state with
//! Disconnect, and "Stay signed in on this computer".

use std::time::{Duration, Instant};

use halo_backend::commands::lastfm::{self, LastFmState, LastFmStatus};
use iced::widget::{column, container, row, text, text_input};
use iced::{Alignment, Element, Length, Padding, Subscription, Task};

use super::{err, msg, outline_btn, setting_row, sm_btn, Msg as SMsg};
use crate::app::{bg, Halo, Message};
use crate::theme::{self, t};
use crate::ui::{self, toggle};

const API_CREATE_URL: &str = "https://www.last.fm/api/account/create";

#[derive(Default)]
pub struct State {
    pub status: Option<LastFmStatus>,
    pub busy: bool,
    pub editing_creds: bool,
    pub key: String,
    pub secret: String,
    pub auth_url: Option<String>,
    pub copied_at: Option<Instant>,
    pub poll_started: Option<Instant>,
}

#[derive(Debug, Clone)]
pub enum Msg {
    Status(LastFmStatus),
    Key(String),
    Secret(String),
    SaveCreds,
    CredsSaved(Result<(), String>),
    EditCreds(bool),
    OpenApiPage,
    Connect,
    AuthUrl(Result<String, String>),
    OpenInBrowser,
    CopyLink,
    Complete,
    Poll,
    Completed(Result<String, String>, bool),
    StaySignedIn(bool),
    Disconnect,
    Done(Result<(), String>),
}

fn lm(m: Msg) -> Message {
    msg(SMsg::LastFm(m))
}

fn status_task(app: &Halo) -> Task<Message> {
    bg(
        &app.backend,
        |b| lastfm::lastfm_get_status(b.clone(), b.state::<LastFmState>()),
        |s| lm(Msg::Status(s)),
    )
}

pub fn load(app: &mut Halo) -> Task<Message> {
    status_task(app)
}

/// Poll complete-auth while sign-in is pending (3s cadence, 120s budget).
pub fn subscription(app: &Halo) -> Option<Subscription<Message>> {
    let st = &app.pages.settings.lastfm;
    let pending = st.status.as_ref().is_some_and(|s| s.pending);
    let within = st
        .poll_started
        .is_none_or(|t0| t0.elapsed() < Duration::from_secs(120));
    (pending && within).then(|| iced::time::every(Duration::from_secs(3)).map(|_| lm(Msg::Poll)))
}

fn open_url(url: String) -> Task<Message> {
    Task::perform(
        async move { tokio::task::spawn_blocking(move || open::that(url)).await },
        |r| match r {
            Ok(Ok(())) => Message::Noop,
            Ok(Err(e)) => err(e.to_string()),
            Err(e) => err(e.to_string()),
        },
    )
}

pub fn update(app: &mut Halo, m: Msg) -> Task<Message> {
    let st = &mut app.pages.settings.lastfm;
    match m {
        Msg::Status(s) => {
            if s.pending && st.poll_started.is_none() {
                st.poll_started = Some(Instant::now());
            }
            if !s.pending {
                st.poll_started = None;
            }
            st.status = Some(s);
            Task::none()
        }
        Msg::Key(s) => {
            st.key = s;
            Task::none()
        }
        Msg::Secret(s) => {
            st.secret = s;
            Task::none()
        }
        Msg::SaveCreds => {
            st.busy = true;
            app.pages.settings.error = None;
            let (k, s) = (st.key.trim().to_string(), st.secret.trim().to_string());
            bg(
                &app.backend,
                move |b| lastfm::lastfm_set_credentials(b.clone(), b.state::<LastFmState>(), k, s),
                |r| lm(Msg::CredsSaved(r)),
            )
        }
        Msg::CredsSaved(r) => {
            st.busy = false;
            match r {
                Ok(()) => {
                    st.key.clear();
                    st.secret.clear();
                    st.editing_creds = false;
                    st.auth_url = None;
                    status_task(app)
                }
                Err(e) => Task::done(err(e)),
            }
        }
        Msg::EditCreds(v) => {
            st.editing_creds = v;
            Task::none()
        }
        Msg::OpenApiPage => open_url(API_CREATE_URL.into()),
        Msg::Connect => {
            st.busy = true;
            app.pages.settings.error = None;
            bg(
                &app.backend,
                |b| lastfm::lastfm_start_auth(b.state::<LastFmState>()),
                |r| lm(Msg::AuthUrl(r)),
            )
        }
        Msg::AuthUrl(r) => {
            st.busy = false;
            match r {
                Ok(u) => {
                    st.auth_url = Some(u);
                    st.poll_started = Some(Instant::now());
                    status_task(app)
                }
                Err(e) => Task::done(err(e)),
            }
        }
        Msg::OpenInBrowser => match st.auth_url.clone() {
            Some(u) => open_url(u),
            // Regenerate lazily (e.g. after leaving Settings while pending).
            None => bg(
                &app.backend,
                |b| lastfm::lastfm_start_auth(b.state::<LastFmState>()),
                |r| match r {
                    Ok(u) => Message::Batch(vec![lm(Msg::AuthUrl(Ok(u.clone()))), lm(Msg::OpenInBrowser)]),
                    Err(e) => err(e),
                },
            ),
        },
        Msg::CopyLink => match st.auth_url.clone() {
            Some(u) => {
                st.copied_at = Some(Instant::now());
                let reset = Task::perform(tokio::time::sleep(Duration::from_secs(2)), |_| Message::Noop);
                Task::batch([iced::clipboard::write(u), reset])
            }
            None => bg(
                &app.backend,
                |b| lastfm::lastfm_start_auth(b.state::<LastFmState>()),
                |r| match r {
                    Ok(u) => Message::Batch(vec![lm(Msg::AuthUrl(Ok(u))), lm(Msg::CopyLink)]),
                    Err(e) => err(e),
                },
            ),
        },
        Msg::Complete | Msg::Poll => {
            let manual = matches!(m, Msg::Complete);
            if manual {
                st.busy = true;
            }
            bg(
                &app.backend,
                |b| lastfm::lastfm_complete_auth(b.clone(), b.state::<LastFmState>()),
                move |r| lm(Msg::Completed(r, manual)),
            )
        }
        Msg::Completed(r, manual) => {
            if manual {
                st.busy = false;
            }
            match r {
                Ok(_) => status_task(app),
                // Not authorised yet: polling keeps waiting silently.
                Err(e) if manual => Task::done(err(e)),
                Err(_) => Task::none(),
            }
        }
        Msg::StaySignedIn(v) => bg(
            &app.backend,
            move |b| lastfm::lastfm_set_stay_signed_in(b.clone(), b.state::<LastFmState>(), v),
            |r| lm(Msg::Done(r)),
        ),
        Msg::Disconnect => {
            st.busy = true;
            st.auth_url = None;
            bg(
                &app.backend,
                |b| lastfm::lastfm_logout(b.clone(), b.state::<LastFmState>()),
                |r| lm(Msg::Done(r)),
            )
        }
        Msg::Done(r) => {
            st.busy = false;
            match r {
                Ok(()) => status_task(app),
                Err(e) => Task::done(err(e)),
            }
        }
    }
}

fn ghost<'a>(label: impl text::IntoFragment<'a>, on_press: Option<Message>) -> Element<'a, Message> {
    sm_btn(None, label, theme::btn_ghost, on_press)
}

fn primary<'a>(label: impl text::IntoFragment<'a>, on_press: Option<Message>) -> Element<'a, Message> {
    sm_btn(None, label, theme::btn_primary, on_press)
}

pub fn view(app: &Halo) -> Element<'_, Message> {
    let k = t();
    let st = &app.pages.settings.lastfm;
    let configured = st.status.as_ref().is_some_and(|s| s.configured);
    let show_form = !configured || st.editing_creds;

    let mut col = column![
        super::label("Last.fm scrobbling"),
        container(ui::muted(
            "Optional. Scrobbles the tracks you play to your Last.fm profile. Enter the app’s \
             API key once, then each person signs in to their own account — sign-in clears when \
             the app closes unless “Stay signed in” is on.",
            theme::TEXT_XS,
        ))
        .max_width(448),
        ui::vspace(12.0),
    ]
    .spacing(4);

    let body: Element<'_, Message> = if show_form {
        let can = !st.busy && !st.key.trim().is_empty() && !st.secret.trim().is_empty();
        let mut actions = row![primary(if st.busy { "Saving…" } else { "Save" }, can.then(|| lm(Msg::SaveCreds)))]
            .spacing(8);
        if configured {
            actions = actions.push(ghost("Cancel", Some(lm(Msg::EditCreds(false)))));
        }
        column![
            row![
                ui::muted("Create an API account at ", theme::TEXT_XS),
                iced::widget::button(text("last.fm/api/account/create").size(theme::TEXT_XS))
                    .padding(0)
                    .style(theme::btn_link)
                    .on_press(lm(Msg::OpenApiPage)),
                ui::muted(" and paste the key and shared secret here.", theme::TEXT_XS),
            ]
            .wrap(),
            field("API key", &st.key, "32-character API key", false, Msg::Key),
            field("Shared secret", &st.secret, "Shared secret", true, Msg::Secret),
            actions,
        ]
        .spacing(12)
        .max_width(448)
        .into()
    } else if let Some(s) = st.status.as_ref().filter(|s| s.connected) {
        row![
            row![
                text("Connected as ").size(theme::TEXT_SM),
                text(format!("@{}", s.username.clone().unwrap_or_default()))
                    .size(theme::TEXT_SM)
                    .font(theme::MEDIUM),
            ],
            outline_btn(None, "Disconnect", (!st.busy).then(|| lm(Msg::Disconnect))),
            ghost("Change API key", Some(lm(Msg::EditCreds(true)))),
        ]
        .spacing(16)
        .align_y(Alignment::Center)
        .into()
    } else if st.status.as_ref().is_some_and(|s| s.pending) {
        let copied = st.copied_at.is_some_and(|t0| t0.elapsed() < Duration::from_secs(2));
        column![
            text("Authenticate").size(theme::TEXT_SM).font(theme::MEDIUM),
            ui::muted(
                "Authorise Halo on Last.fm and click Allow — Halo finishes connecting \
                 automatically. Open the page here, or copy the link to open it in another browser.",
                theme::TEXT_XS,
            ),
            row![
                primary("Open in browser", Some(lm(Msg::OpenInBrowser))),
                outline_btn(None, if copied { "Copied!" } else { "Copy link" }, Some(lm(Msg::CopyLink))),
            ]
            .spacing(8),
            ghost(
                if st.busy { "Connecting…" } else { "I've authorised — complete setup" },
                (!st.busy).then(|| lm(Msg::Complete)),
            ),
        ]
        .spacing(12)
        .max_width(448)
        .into()
    } else {
        column![
            ui::muted("Your password never touches this app — you authorise it on Last.fm.", theme::TEXT_XS),
            row![
                outline_btn(
                    None,
                    if st.busy { "Preparing…" } else { "Connect with browser" },
                    (!st.busy).then(|| lm(Msg::Connect)),
                ),
                ghost("Change API key", Some(lm(Msg::EditCreds(true)))),
            ]
            .spacing(8),
        ]
        .spacing(8)
        .max_width(448)
        .into()
    };
    col = col.push(body);

    if configured && !st.editing_creds {
        let stay = st.status.as_ref().is_some_and(|s| s.stay_signed_in);
        col = col.push(
            container(setting_row(
                "Stay signed in on this computer",
                "Off by default so others using this computer sign in to their own account.",
                toggle(stay, lm(Msg::StaySignedIn(!stay))),
            ))
            .max_width(448)
            .padding(Padding::ZERO.top(16.0)),
        );
    }
    let _ = (k, Length::Fill);
    col.into()
}

fn field<'a>(
    title: &'static str,
    value: &'a str,
    ph: &'static str,
    secure: bool,
    f: fn(String) -> Msg,
) -> iced::widget::Column<'a, Message> {
    column![
        text(title).size(theme::TEXT_XS).font(theme::MEDIUM),
        text_input(ph, value)
            .on_input(move |s| lm(f(s)))
            .secure(secure)
            .size(theme::TEXT_SM)
            .font(theme::MONO)
            .padding(Padding::from([6.0, 12.0]))
            .style(theme::input),
    ]
    .spacing(4)
}
