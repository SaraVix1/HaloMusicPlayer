//! Metadata editor dialog (`metadata-editor-dialog.tsx`), split into:
//!
//! - `mod.rs`   — state, messages, loading / saving
//! - `fields.rs`— the tag form (title … language)
//! - `art.rs`   — album-art column (pick file, extract, URL, online
//!   suggestions, undo)
//! - `crop.rs`  — crop view (drag / resize box, Apply crop / Use full image)
//!
//! Ctrl+Enter saves; Escape leaves the URL / online / crop sub-modes first.

pub mod art;
pub mod crop;
pub mod fields;

use halo_backend::commands::metadata_editor::{self as me, CoverSuggestion, FullTrackMetadata, MetadataEdit};
use iced::widget::{column, container, row, text};
use iced::{Alignment, Element, Length, Task};

use super::dialogs::{DMsg, Dialog};
use crate::app::{bg, Halo, Message};
use crate::theme::{self, t};
use crate::ui::{self, modal, spin, spin_angle, Spinner};

pub const TITLE_INPUT: &str = "metadata-title";
pub const URL_INPUT: &str = "metadata-art-url";

pub struct Draft {
    pub title: String,
    pub album_name: String,
    pub artists: String,
    pub album_artists: String,
    pub composers: String,
    pub genres: String,
    pub year: String,
    pub track_number: String,
    pub track_total: String,
    pub disc_number: String,
    pub disc_total: String,
    pub comment: iced::widget::text_editor::Content,
    pub publisher: String,
    pub copyright: String,
    pub language: String,
    pub new_art_path: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtMode {
    Display,
    Url,
    Online,
    Crop,
}

pub struct Editor {
    pub track_id: i64,
    pub meta: Option<FullTrackMetadata>,
    pub draft: Option<Draft>,
    pub loading: bool,
    pub saving: bool,
    pub error: Option<String>,
    pub mode: ArtMode,
    pub url: String,
    pub url_loading: bool,
    pub url_error: Option<String>,
    pub extracting: bool,
    pub suggestions: Option<Vec<(CoverSuggestion, Option<iced::widget::image::Handle>)>>,
    pub suggestions_loading: bool,
    pub suggestions_error: Option<String>,
    pub picking: Option<usize>,
    pub crop: Option<crop::CropState>,
}

/// Which text field changed.
#[derive(Debug, Clone, Copy)]
pub enum Field {
    Title,
    Artists,
    AlbumArtists,
    Album,
    Genres,
    Composers,
    Year,
    TrackNumber,
    TrackTotal,
    DiscNumber,
    DiscTotal,
    Publisher,
    Copyright,
    Language,
}

// One-shot load payload; boxing it would only add noise at the call sites.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone)]
pub enum Msg {
    Loaded(i64, Result<FullTrackMetadata, String>),
    Field(Field, String),
    Comment(iced::widget::text_editor::Action),
    Save,
    Saved(Result<(), String>),
    Art(art::Msg),
    Crop(crop::Msg),
}

pub fn mm(m: Msg) -> Message {
    Message::Page(super::Msg::Meta(m))
}

fn close_msg() -> Message {
    Message::Page(super::Msg::Dialog(DMsg::Close))
}

pub fn editor(app: &mut Halo) -> Option<&mut Editor> {
    match app.pages.dialog.as_mut() {
        Some(Dialog::Metadata(ed)) => Some(ed),
        _ => None,
    }
}

fn join(v: &[String]) -> String {
    v.join(", ")
}

fn split(s: &str) -> Vec<String> {
    s.split(',').map(|x| x.trim()).filter(|x| !x.is_empty()).map(String::from).collect()
}

fn num(s: &str) -> Option<i64> {
    s.trim().parse::<i64>().ok().filter(|n| *n >= 1)
}

fn opt(s: &str) -> Option<String> {
    let t = s.trim();
    (!t.is_empty()).then(|| t.to_string())
}

fn n2s(v: Option<i64>) -> String {
    v.map(|n| n.to_string()).unwrap_or_default()
}

fn to_draft(m: &FullTrackMetadata) -> Draft {
    Draft {
        title: m.title.clone().unwrap_or_default(),
        album_name: m.album_name.clone().unwrap_or_default(),
        artists: join(&m.artists),
        album_artists: join(&m.album_artists),
        composers: join(&m.composers),
        genres: join(&m.genres),
        year: n2s(m.year),
        track_number: n2s(m.track_number),
        track_total: n2s(m.track_total),
        disc_number: n2s(m.disc_number),
        disc_total: n2s(m.disc_total),
        comment: iced::widget::text_editor::Content::with_text(m.comment.as_deref().unwrap_or("")),
        publisher: m.publisher.clone().unwrap_or_default(),
        copyright: m.copyright.clone().unwrap_or_default(),
        language: m.language.clone().unwrap_or_default(),
        new_art_path: None,
    }
}

pub fn open(app: &mut Halo, track_id: i64) -> Task<Message> {
    app.pages.dialog = Some(Dialog::Metadata(Box::new(Editor {
        track_id,
        meta: None,
        draft: None,
        loading: true,
        saving: false,
        error: None,
        mode: ArtMode::Display,
        url: String::new(),
        url_loading: false,
        url_error: None,
        extracting: false,
        suggestions: None,
        suggestions_loading: false,
        suggestions_error: None,
        picking: None,
        crop: None,
    })));
    bg(
        &app.backend,
        move |b| me::get_track_full_metadata(track_id, b.db()),
        move |r| mm(Msg::Loaded(track_id, r)),
    )
}

pub fn update(app: &mut Halo, m: Msg) -> Task<Message> {
    let backend = app.backend.clone();
    let Some(ed) = editor(app) else { return Task::none() };
    match m {
        Msg::Loaded(id, r) => {
            if ed.track_id != id {
                return Task::none();
            }
            ed.loading = false;
            match r {
                Ok(meta) => {
                    ed.draft = Some(to_draft(&meta));
                    ed.meta = Some(meta);
                    iced::widget::operation::focus(TITLE_INPUT)
                }
                Err(e) => {
                    ed.error = Some(e);
                    Task::none()
                }
            }
        }
        Msg::Field(f, v) => {
            if let Some(d) = ed.draft.as_mut() {
                let slot = match f {
                    Field::Title => &mut d.title,
                    Field::Artists => &mut d.artists,
                    Field::AlbumArtists => &mut d.album_artists,
                    Field::Album => &mut d.album_name,
                    Field::Genres => &mut d.genres,
                    Field::Composers => &mut d.composers,
                    Field::Year => &mut d.year,
                    Field::TrackNumber => &mut d.track_number,
                    Field::TrackTotal => &mut d.track_total,
                    Field::DiscNumber => &mut d.disc_number,
                    Field::DiscTotal => &mut d.disc_total,
                    Field::Publisher => &mut d.publisher,
                    Field::Copyright => &mut d.copyright,
                    Field::Language => &mut d.language,
                };
                *slot = v;
            }
            Task::none()
        }
        Msg::Comment(a) => {
            if let Some(d) = ed.draft.as_mut() {
                d.comment.perform(a);
            }
            Task::none()
        }
        Msg::Save => {
            let Some(d) = ed.draft.as_ref() else { return Task::none() };
            if ed.saving || ed.mode == ArtMode::Crop {
                return Task::none();
            }
            ed.saving = true;
            ed.error = None;
            let edit = MetadataEdit {
                title: opt(&d.title),
                album_name: opt(&d.album_name),
                artists: split(&d.artists),
                album_artists: split(&d.album_artists),
                composers: split(&d.composers),
                genres: split(&d.genres),
                year: num(&d.year),
                track_number: num(&d.track_number),
                track_total: num(&d.track_total),
                disc_number: num(&d.disc_number),
                disc_total: num(&d.disc_total),
                comment: opt(&d.comment.text()),
                publisher: opt(&d.publisher),
                copyright: opt(&d.copyright),
                language: opt(&d.language),
                new_art_path: d.new_art_path.clone(),
            };
            let id = ed.track_id;
            bg(
                &backend,
                move |b| me::save_track_metadata(id, edit, b.clone(), b.db()),
                |r| mm(Msg::Saved(r)),
            )
        }
        Msg::Saved(r) => {
            ed.saving = false;
            match r {
                // The backend emits library-changed, which refreshes the page.
                Ok(()) => {
                    app.pages.dialog = None;
                    Task::none()
                }
                Err(e) => {
                    ed.error = Some(e);
                    Task::none()
                }
            }
        }
        Msg::Art(m) => art::update(app, m),
        Msg::Crop(m) => crop::update(app, m),
    }
}

/// DMsg fallback (none are metadata-specific).
pub fn update_dialog_msg(_app: &mut Halo, _m: DMsg) -> Task<Message> {
    Task::none()
}

/// Escape inside the editor: leave a sub-mode first. Returns `true` if it
/// was handled (dialog stays open).
pub fn on_escape(app: &mut Halo) -> bool {
    let Some(ed) = editor(app) else { return false };
    if ed.saving {
        return true;
    }
    match ed.mode {
        ArtMode::Display => false,
        ArtMode::Crop => {
            ed.mode = ArtMode::Display;
            ed.crop = None;
            true
        }
        _ => {
            ed.mode = ArtMode::Display;
            ed.url.clear();
            ed.url_error = None;
            true
        }
    }
}

pub fn on_ctrl_enter(app: &mut Halo) -> Task<Message> {
    if editor(app).is_some() {
        update(app, Msg::Save)
    } else {
        Task::none()
    }
}

pub fn view<'a>(app: &'a Halo, ed: &'a Editor) -> Element<'a, Message> {
    let k = t();
    let mut body = column![].spacing(12);
    if let Some(e) = &ed.error {
        body = body.push(text(e.as_str()).size(theme::TEXT_SM).color(k.destructive));
    }

    if ed.mode == ArtMode::Crop {
        if let Some(c) = &ed.crop {
            body = body.push(crop::view(c));
        }
        return modal("Edit metadata", body, None, 640.0, close_msg());
    }

    match &ed.draft {
        Some(d) if !ed.loading => {
            body = body.push(
                row![art::view(ed, d), fields::view(app, d)]
                    .spacing(16)
                    .align_y(Alignment::Start),
            );
        }
        _ => {
            body = body.push(
                container(ui::muted("Loading…", theme::TEXT_SM))
                    .width(Length::Fill)
                    .padding(24)
                    .align_x(Alignment::Center),
            );
        }
    }

    let lead: Option<Element<'a, Message>> =
        ed.saving.then(|| spin(Spinner::Loader, 13.0, spin_angle(), k.primary_fg).into());
    let can_save = !ed.saving && !ed.loading && ed.draft.is_some();
    let footer = row![
        ui::fill(),
        ui::label_button(None, "Cancel", theme::TEXT_SM, 32.0, 16.0, theme::btn_outline, (!ed.saving).then(close_msg)),
        {
            let mut r = row![].spacing(6).align_y(Alignment::Center);
            if let Some(l) = lead {
                r = r.push(l);
            }
            r = r.push(text(if ed.saving { "Saving…" } else { "Save" }).size(theme::TEXT_SM).font(theme::MEDIUM));
            iced::widget::button(container(r).height(32).align_y(Alignment::Center))
                .padding(iced::Padding::from([0.0, 16.0]))
                .style(theme::btn_primary)
                .on_press_maybe(can_save.then(|| mm(Msg::Save)))
        },
    ]
    .spacing(8);
    modal("Edit metadata", body, Some(footer.into()), 640.0, close_msg())
}

/// Animation ticks while something is loading (spinners).
pub fn busy(app: &Halo) -> bool {
    match app.pages.dialog.as_ref() {
        Some(Dialog::Metadata(ed)) => {
            ed.saving
                || ed.extracting
                || ed.url_loading
                || ed.suggestions_loading
                || ed.picking.is_some()
                || ed.crop.as_ref().is_some_and(|c| c.processing)
        }
        _ => false,
    }
}
