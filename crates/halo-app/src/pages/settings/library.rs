//! Settings → Library: managed folders (add via native picker, hover
//! remove), rescan with live progress, scan behaviour toggles, and the
//! multi-value delimiter / extension settings.

use std::time::Duration;

use halo_backend::commands::{folders, scan, ui as prefs};
use halo_backend::scanner::{ScanProgress, ScanSummary};
use iced::widget::{button, column, container, hover, row, text, text_input, Space};
use iced::{Alignment, Element, Length, Padding, Subscription, Task};

use super::{done, err, label, msg, outline_btn, section_header, setting_row, Msg as SMsg};
use crate::app::{bg, Halo, Message};
use crate::icons::{icon, Icon};
use crate::theme::{self, a, t};
use crate::ui::{self, hline, spin, spin_angle, toggle, Spinner};
use crate::widgets::line;

const AUTO_SCAN_KEY: &str = "library.auto_scan_on_add";
const WATCH_KEY: &str = "library.watch_folders";

pub struct State {
    pub folders: Vec<folders::Folder>,
    pub loading: bool,
    pub scanning: bool,
    pub progress: Option<ScanProgress>,
    pub summary: Option<ScanSummary>,
    pub override_metadata: bool,
    pub auto_scan_on_add: bool,
    pub watch_folders: bool,
    pub delimiters: String,
    pub extensions: String,
}

impl Default for State {
    fn default() -> Self {
        Self {
            folders: vec![],
            loading: true,
            scanning: false,
            progress: None,
            summary: None,
            override_metadata: false,
            auto_scan_on_add: false,
            watch_folders: false,
            delimiters: ",;|:&".into(),
            extensions: "mp3,flac,m4a,aac,ogg,wav,opus,wma,aiff,aif".into(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Msg {
    Loaded(Box<Loaded>),
    AddFolder,
    Picked(Option<String>),
    Added(Result<folders::Folder, String>),
    Remove(i64),
    Removed(i64, Result<(), String>),
    Scan,
    Scanned(Result<ScanSummary, String>),
    Override(bool),
    AutoScan(bool),
    Watch(bool),
    Delimiters(String),
    Extensions(String),
}

#[derive(Debug, Clone)]
pub struct Loaded {
    folders: Result<Vec<folders::Folder>, String>,
    settings: Option<scan::ScanSettings>,
    auto_scan: bool,
    watch: bool,
}

fn lm(m: Msg) -> Message {
    msg(SMsg::Library(m))
}

pub fn load(app: &mut Halo) -> Task<Message> {
    bg(
        &app.backend,
        |b| Loaded {
            folders: folders::get_folders(b.db()),
            settings: scan::get_scan_settings(b.db()).ok(),
            auto_scan: prefs::get_pref(AUTO_SCAN_KEY.into(), b.db()).ok().flatten().as_deref() == Some("true"),
            watch: prefs::get_pref(WATCH_KEY.into(), b.db()).ok().flatten().as_deref() == Some("true"),
        },
        |l| lm(Msg::Loaded(Box::new(l))),
    )
}

pub fn on_progress(app: &mut Halo, p: ScanProgress) -> Task<Message> {
    app.pages.settings.library.progress = Some(p);
    Task::none()
}

/// Redraw ticks while scanning (spinning RefreshCw icon).
pub fn subscription(app: &Halo) -> Option<Subscription<Message>> {
    app.pages
        .settings
        .library
        .scanning
        .then(|| iced::time::every(Duration::from_millis(16)).map(|_| Message::Noop))
}

fn save_settings(app: &Halo) -> Task<Message> {
    let st = &app.pages.settings.library;
    let s = scan::ScanSettings {
        delimiters: st.delimiters.clone(),
        extensions: st.extensions.clone(),
    };
    bg(&app.backend, move |b| scan::set_scan_settings(s, b.db()), done)
}

pub fn update(app: &mut Halo, m: Msg) -> Task<Message> {
    let st = &mut app.pages.settings.library;
    match m {
        Msg::Loaded(l) => {
            st.loading = false;
            match l.folders {
                Ok(f) => st.folders = f,
                Err(e) => return Task::done(err(e)),
            }
            if let Some(s) = l.settings {
                st.delimiters = s.delimiters;
                st.extensions = s.extensions;
            }
            st.auto_scan_on_add = l.auto_scan;
            st.watch_folders = l.watch;
            Task::none()
        }
        Msg::AddFolder => Task::perform(
            async {
                rfd::AsyncFileDialog::new()
                    .set_title("Add music folder")
                    .pick_folder()
                    .await
                    .map(|h| h.path().to_string_lossy().into_owned())
            },
            |p| lm(Msg::Picked(p)),
        ),
        Msg::Picked(None) => Task::none(),
        Msg::Picked(Some(path)) => bg(
            &app.backend,
            move |b| folders::add_folder(path, b.clone(), b.db()),
            |r| lm(Msg::Added(r)),
        ),
        Msg::Added(r) => match r {
            Ok(f) => {
                if !st.folders.iter().any(|x| x.id == f.id) {
                    st.folders.push(f);
                }
                if st.auto_scan_on_add {
                    update(app, Msg::Scan)
                } else {
                    Task::none()
                }
            }
            Err(e) => Task::done(err(e)),
        },
        Msg::Remove(id) => bg(
            &app.backend,
            move |b| folders::remove_folder(id, b.clone(), b.db()),
            move |r| lm(Msg::Removed(id, r)),
        ),
        Msg::Removed(id, r) => match r {
            Ok(()) => {
                st.folders.retain(|f| f.id != id);
                Task::none()
            }
            Err(e) => Task::done(err(e)),
        },
        Msg::Scan => {
            if st.scanning {
                return Task::none();
            }
            app.pages.settings.error = None;
            st.summary = None;
            st.progress = Some(ScanProgress {
                current: 0,
                total: 0,
                current_file: String::new(),
                inserted: 0,
                updated: 0,
                skipped: 0,
                failed: 0,
                done: false,
            });
            st.scanning = true;
            let ov = st.override_metadata;
            bg(
                &app.backend,
                move |b| scan::scan_library(b, ov),
                |r| lm(Msg::Scanned(r)),
            )
        }
        Msg::Scanned(r) => {
            st.scanning = false;
            match r {
                Ok(s) => {
                    st.summary = Some(s);
                    Task::none()
                }
                Err(e) => Task::done(err(e)),
            }
        }
        Msg::Override(v) => {
            st.override_metadata = v;
            Task::none()
        }
        Msg::AutoScan(v) => {
            st.auto_scan_on_add = v;
            bg(
                &app.backend,
                move |b| prefs::set_pref(AUTO_SCAN_KEY.into(), v.to_string(), b.db()),
                done,
            )
        }
        Msg::Watch(v) => {
            st.watch_folders = v;
            bg(
                &app.backend,
                move |b| halo_backend::watcher::set_watch_folders(b, v),
                done,
            )
        }
        Msg::Delimiters(s) => {
            st.delimiters = s;
            save_settings(app)
        }
        Msg::Extensions(s) => {
            st.extensions = s;
            save_settings(app)
        }
    }
}

pub fn view(app: &Halo) -> Element<'_, Message> {
    let k = t();
    let st = &app.pages.settings.library;

    // ── Folders ───────────────────────────────────────────────────────────────
    let rescan_icon: Element<'_, Message> = if st.scanning {
        spin(Spinner::Refresh, 15.0, spin_angle(), k.foreground).into()
    } else {
        icon(Icon::RefreshCw, 15.0).into()
    };
    let buttons = row![
        outline_btn(Some(icon(Icon::FolderOpen, 15.0).into()), "Add folder", Some(lm(Msg::AddFolder))),
        outline_btn(
            Some(rescan_icon),
            if st.scanning { "Scanning…" } else { "Rescan library" },
            (!st.folders.is_empty() && !st.scanning).then(|| lm(Msg::Scan)),
        ),
    ]
    .spacing(12);

    let list: Element<'_, Message> = if st.loading {
        ui::muted("Loading…", theme::TEXT_SM).into()
    } else if st.folders.is_empty() {
        ui::muted("No folders added yet. Click \"Add folder\" to get started.", theme::TEXT_SM).into()
    } else {
        let mut col = column![].spacing(4);
        for f in &st.folders {
            let base = container(
                row![
                    line(f.path.as_str()).size(theme::TEXT_SM).font(theme::MONO).width(Length::Fill),
                    Space::new().width(15),
                ]
                .align_y(Alignment::Center),
            )
            .width(Length::Fill)
            .padding(Padding::from([8.0, 12.0]))
            .style(|_| iced::widget::container::Style {
                background: Some(a(t().muted, 0.4).into()),
                border: theme::no_border(theme::R_MD),
                ..Default::default()
            });
            let remove = container(
                button(icon(Icon::X, 15.0))
                    .padding(0)
                    .style(theme::btn_text(k.muted_fg, k.destructive))
                    .on_press_maybe((!st.scanning).then(|| lm(Msg::Remove(f.id)))),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(Padding::ZERO.right(12.0))
            .align_x(Alignment::End)
            .align_y(Alignment::Center);
            col = col.push(hover(base, remove));
        }
        col.into()
    };

    let mut folders_col = column![label("Music folders"), buttons, list].spacing(12);

    if let Some(p) = &st.progress {
        let pct = if p.total > 0 {
            (p.current as f32 / p.total as f32 * 100.0).round()
        } else {
            0.0
        };
        let bar = iced::widget::stack![
            container(Space::new())
                .width(Length::Fill)
                .height(6)
                .style(|_| iced::widget::container::Style {
                    background: Some(t().muted.into()),
                    border: theme::no_border(theme::R_XS),
                    ..Default::default()
                }),
            row![
                container(Space::new())
                    .width(Length::FillPortion((pct as u16).max(0)))
                    .height(6)
                    .style(|_| iced::widget::container::Style {
                        background: Some(t().primary.into()),
                        border: theme::no_border(theme::R_XS),
                        ..Default::default()
                    }),
                Space::new().width(Length::FillPortion((100 - pct as u16).max(0))),
            ],
        ];
        let mut card = column![
            row![
                text(if p.done { "Scan complete" } else { "Scanning…" })
                    .size(theme::TEXT_SM)
                    .font(theme::MEDIUM),
                ui::fill(),
                ui::muted(format!("{} / {} ({}%)", p.current, p.total, pct), theme::TEXT_XS),
            ]
            .align_y(Alignment::Center),
            bar,
        ]
        .spacing(8);
        if !p.current_file.is_empty() && !p.done {
            card = card.push(
                line(p.current_file.as_str())
                    .size(theme::TEXT_XS)
                    .font(theme::MONO)
                    .color(k.muted_fg)
                    .width(Length::Fill),
            );
        }
        card = card.push(ui::muted(
            format!(
                "Inserted {} · Updated {} · Skipped {} · Failed {}",
                p.inserted, p.updated, p.skipped, p.failed
            ),
            theme::TEXT_XS,
        ));
        folders_col = folders_col.push(
            container(card)
                .width(Length::Fill)
                .padding(12)
                .style(|_| iced::widget::container::Style {
                    background: Some(a(t().muted, 0.4).into()),
                    border: theme::no_border(theme::R_MD),
                    ..Default::default()
                }),
        );
    }
    if let (Some(s), Some(p)) = (&st.summary, &st.progress) {
        if p.done && s.total == 0 {
            folders_col = folders_col.push(ui::muted(
                "No audio files found in the managed folders.",
                theme::TEXT_SM,
            ));
        }
    }

    // ── Scan behaviour ────────────────────────────────────────────────────────
    let override_toggle: Element<'_, Message> = if st.scanning {
        container(toggle(st.override_metadata, Message::Noop))
            .style(|_| iced::widget::container::Style::default())
            .into()
    } else {
        toggle(st.override_metadata, lm(Msg::Override(!st.override_metadata)))
    };
    let behaviour = column![
        label("Scan behaviour"),
        setting_row(
            "Override existing metadata",
            "Re-read tags from files even when metadata is already in the database.",
            override_toggle,
        ),
        setting_row(
            "Auto-scan when adding a folder",
            "Start a library scan automatically after a new folder is added.",
            toggle(st.auto_scan_on_add, lm(Msg::AutoScan(!st.auto_scan_on_add))),
        ),
        setting_row(
            "Watch folders for changes",
            "Automatically update the library when files are added or removed.",
            toggle(st.watch_folders, lm(Msg::Watch(!st.watch_folders))),
        ),
    ]
    .spacing(4);

    // ── Formats ───────────────────────────────────────────────────────────────
    let formats = column![
        label("File formats"),
        field(
            "Multi-value delimiters",
            &st.delimiters,
            ",;|:&",
            "Characters used to split fields like artist, genre, and composer.",
            Msg::Delimiters,
        ),
        field(
            "File extensions",
            &st.extensions,
            "mp3,flac,m4a",
            "Comma-separated list, without dots.",
            Msg::Extensions,
        ),
    ]
    .spacing(16);

    column![
        section_header("Library", "Music folders, scan options, and file format settings."),
        folders_col,
        ui::vspace(20.0),
        hline(),
        ui::vspace(20.0),
        behaviour,
        ui::vspace(20.0),
        hline(),
        ui::vspace(20.0),
        formats,
    ]
    .into()
}

fn field<'a>(
    title: &'static str,
    value: &'a str,
    placeholder: &'static str,
    hint: &'static str,
    f: fn(String) -> Msg,
) -> iced::widget::Column<'a, Message> {
    column![
        text(title).size(theme::TEXT_SM).font(theme::MEDIUM),
        text_input(placeholder, value)
            .on_input(move |s| lm(f(s)))
            .size(theme::TEXT_SM)
            .font(theme::MONO)
            .padding(Padding::from([6.0, 12.0]))
            .style(theme::input),
        ui::muted(hint, theme::TEXT_XS),
    ]
    .spacing(6)
}
