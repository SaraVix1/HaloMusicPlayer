//! Settings → Advanced: the Last.fm panel (`lastfm.rs`) and destructive
//! actions — clear cache, reset play statistics, clear database (the last
//! two behind confirmation dialogs).

use halo_backend::commands::{scan, stats};
use iced::widget::{column, container, row};
use iced::{Element, Padding, Task};

use super::{done, msg, outline_btn, section_header, Msg as SMsg};
use crate::app::{bg, Halo, Message};
use crate::icons::{icon, Icon};
use crate::pages::dialogs::{self, Confirm, ConfirmAction};
use crate::theme;
use crate::ui::{self, hline};

#[derive(Debug, Clone)]
pub enum Msg {
    ClearCache,
    AskResetStats,
    AskClearDb,
    DbCleared(Result<(), String>),
}

fn am(m: Msg) -> Message {
    msg(SMsg::Advanced(m))
}

pub fn update(app: &mut Halo, m: Msg) -> Task<Message> {
    match m {
        Msg::ClearCache => {
            app.pages.settings.error = None;
            bg(&app.backend, scan::clear_cache, done)
        }
        Msg::AskResetStats => dialogs::open_confirm(
            app,
            Confirm {
                title: "Reset play statistics?".into(),
                body: "This clears play counts, skip counts, last played timestamps, and star \
                       ratings for every track. This cannot be undone."
                    .into(),
                confirm_label: "Reset statistics".into(),
                destructive: true,
                action: ConfirmAction::ResetStats,
            },
        ),
        Msg::AskClearDb => dialogs::open_confirm(
            app,
            Confirm {
                title: "Clear database?".into(),
                body: "This removes every scanned track, playlist, and queue entry. Managed \
                       folders are kept. This cannot be undone."
                    .into(),
                confirm_label: "Clear database".into(),
                destructive: true,
                action: ConfirmAction::ClearDatabase,
            },
        ),
        Msg::DbCleared(r) => {
            if r.is_ok() {
                app.pages.settings.library.summary = None;
                app.pages.settings.library.progress = None;
            }
            Task::done(done(r))
        }
    }
}

/// A confirmation dialog was accepted.
pub fn on_confirm(app: &mut Halo, a: ConfirmAction) -> Task<Message> {
    match a {
        ConfirmAction::ResetStats => bg(&app.backend, |b| stats::reset_all_stats(b.db()), done),
        ConfirmAction::ClearDatabase => {
            bg(&app.backend, scan::clear_database, |r| am(Msg::DbCleared(r)))
        }
        ConfirmAction::DeletePlaylist(_) => Task::none(),
    }
}

pub fn view(app: &Halo) -> Element<'_, Message> {
    let scanning = app.pages.settings.library.scanning;
    let enabled = |m: Msg| (!scanning).then(|| am(m));
    column![
        section_header("Advanced", "Optional integrations and destructive actions."),
        super::lastfm::view(app),
        ui::vspace(32.0),
        hline(),
        container(
            column![
                super::label("Destructive actions"),
                ui::muted("Use with care — these cannot be undone.", theme::SMALL),
                ui::vspace(12.0),
                row![
                    outline_btn(Some(icon(Icon::Eraser, 15.0).into()), "Clear cache", enabled(Msg::ClearCache)),
                    outline_btn(
                        Some(icon(Icon::BarChart2, 15.0).into()),
                        "Reset play statistics",
                        enabled(Msg::AskResetStats)
                    ),
                    outline_btn(Some(icon(Icon::Trash2, 15.0).into()), "Clear database", enabled(Msg::AskClearDb)),
                ]
                .spacing(12)
                .wrap(),
            ]
            .spacing(4),
        )
        .padding(Padding::ZERO.top(24.0)),
    ]
    .into()
}
