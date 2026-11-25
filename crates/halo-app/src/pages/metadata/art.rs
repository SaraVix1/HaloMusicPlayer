//! Metadata editor — album-art column (152px): preview plus 28×28 buttons
//! (pick from file, extract embedded art, from URL, online suggestions,
//! undo). URL mode pre-fills from the clipboard when it holds an image
//! URL; online mode searches MusicBrainz / Cover Art Archive and shows a
//! 3-column thumbnail grid. Every chosen image goes through the crop view.

use halo_backend::commands::metadata_editor::{self as me, CoverSuggestion};
use iced::widget::{button, center, column, container, image, row, stack, text, text_input, tooltip::Position};
use iced::{Alignment, ContentFit, Element, Length, Padding, Task};

use super::{crop, editor, mm, ArtMode, Draft, Editor, Msg as MMsg, URL_INPUT};
use crate::app::{bg, Halo, Message};
use crate::icons::Icon;
use crate::theme::{self, a, t};
use crate::ui::{self, spin, spin_angle, tip, Spinner};

#[derive(Debug, Clone)]
pub enum Msg {
    PickFile,
    Picked(Option<String>),
    Extract,
    Extracted(Result<String, String>),
    UrlMode,
    Clipboard(Option<String>),
    Url(String),
    LoadUrl,
    UrlLoaded(Result<String, String>),
    OnlineMode,
    Suggestions(Result<Vec<CoverSuggestion>, String>),
    Thumb(usize, Option<Vec<u8>>),
    Pick(usize),
    Picked2(Result<String, String>),
    Back,
    Undo,
}

fn am(m: Msg) -> Message {
    mm(MMsg::Art(m))
}

fn split_first(s: &str) -> Option<String> {
    s.split(',').map(|x| x.trim()).find(|x| !x.is_empty()).map(String::from)
}

/// `looksLikeImageUrl`: http(s) URL whose path ends in an image extension
/// or whose host looks like an image CDN.
pub fn looks_like_image_url(s: &str) -> bool {
    let s = s.trim();
    if !(s.starts_with("http://") || s.starts_with("https://")) || s.contains(char::is_whitespace) {
        return false;
    }
    let lower = s.to_lowercase();
    let path = lower.split(['?', '#']).next().unwrap_or("");
    let ext = [".jpg", ".jpeg", ".png", ".gif", ".bmp", ".webp"].iter().any(|e| path.ends_with(e));
    ext || lower.contains("/i.") || lower.contains("/img.") || lower.contains("/image.")
}

fn go_to_crop(app: &mut Halo, path: String) -> Task<Message> {
    if let Some(ed) = editor(app) {
        ed.crop = Some(crop::CropState::new(path));
        ed.mode = ArtMode::Crop;
    }
    Task::none()
}

fn fetch_bytes(url: String) -> Option<Vec<u8>> {
    let resp = ureq::get(&url)
        .set("User-Agent", "Halo/1.0 (desktop music player)")
        .call()
        .ok()?;
    let mut buf = Vec::new();
    use std::io::Read;
    resp.into_reader().take(8 * 1024 * 1024).read_to_end(&mut buf).ok()?;
    Some(buf)
}

pub fn update(app: &mut Halo, m: Msg) -> Task<Message> {
    let backend = app.backend.clone();
    let Some(ed) = editor(app) else { return Task::none() };
    match m {
        Msg::PickFile => Task::perform(
            async {
                rfd::AsyncFileDialog::new()
                    .add_filter("Images", &["jpg", "jpeg", "png", "bmp", "gif", "webp"])
                    .pick_file()
                    .await
                    .map(|h| h.path().to_string_lossy().into_owned())
            },
            |p| am(Msg::Picked(p)),
        ),
        Msg::Picked(None) => Task::none(),
        Msg::Picked(Some(p)) => go_to_crop(app, p),
        Msg::Extract => {
            ed.extracting = true;
            let id = ed.track_id;
            bg(
                &backend,
                move |b| me::extract_track_art(id, b.clone(), b.db()),
                |r| am(Msg::Extracted(r)),
            )
        }
        Msg::Extracted(r) => {
            ed.extracting = false;
            match r {
                Ok(p) => go_to_crop(app, p),
                Err(e) => {
                    ed.error = Some(e);
                    Task::none()
                }
            }
        }
        Msg::UrlMode => {
            ed.mode = ArtMode::Url;
            Task::batch([
                iced::clipboard::read().map(|c| am(Msg::Clipboard(c))),
                iced::widget::operation::focus(URL_INPUT),
            ])
        }
        Msg::Clipboard(c) => {
            if let Some(c) = c.filter(|c| looks_like_image_url(c)) {
                if ed.url.is_empty() {
                    ed.url = c.trim().to_string();
                }
            }
            Task::none()
        }
        Msg::Url(u) => {
            ed.url = u;
            Task::none()
        }
        Msg::LoadUrl => {
            let u = ed.url.trim().to_string();
            if u.is_empty() || ed.url_loading {
                return Task::none();
            }
            ed.url_loading = true;
            ed.url_error = None;
            bg(&backend, move |b| me::fetch_art_from_url(u, b), |r| am(Msg::UrlLoaded(r)))
        }
        Msg::UrlLoaded(r) => {
            ed.url_loading = false;
            match r {
                Ok(p) => {
                    ed.url.clear();
                    go_to_crop(app, p)
                }
                Err(e) => {
                    ed.url_error = Some(e);
                    Task::none()
                }
            }
        }
        Msg::OnlineMode => {
            ed.mode = ArtMode::Online;
            ed.suggestions = None;
            ed.suggestions_error = None;
            ed.suggestions_loading = true;
            let d = ed.draft.as_ref();
            // Prefer the first album artist, fall back to the first artist.
            let artist = d
                .and_then(|d| split_first(&d.album_artists).or_else(|| split_first(&d.artists)))
                .unwrap_or_default();
            let album = d.map(|d| d.album_name.trim().to_string()).unwrap_or_default();
            bg(&backend, move |_| me::search_cover_art(artist, album), |r| am(Msg::Suggestions(r)))
        }
        Msg::Suggestions(r) => {
            ed.suggestions_loading = false;
            match r {
                Ok(list) => {
                    let tasks: Vec<Task<Message>> = list
                        .iter()
                        .enumerate()
                        .map(|(i, s)| {
                            let url = s.thumbnail_url.clone();
                            bg(&backend, move |_| fetch_bytes(url), move |b| am(Msg::Thumb(i, b)))
                        })
                        .collect();
                    ed.suggestions = Some(list.into_iter().map(|s| (s, None)).collect());
                    Task::batch(tasks)
                }
                Err(e) => {
                    ed.suggestions_error = Some(e);
                    Task::none()
                }
            }
        }
        Msg::Thumb(i, bytes) => {
            if let Some(list) = ed.suggestions.as_mut() {
                if let (Some(slot), Some(b)) = (list.get_mut(i), bytes) {
                    slot.1 = Some(image::Handle::from_bytes(b));
                }
            }
            Task::none()
        }
        Msg::Pick(i) => {
            let Some((s, _)) = ed.suggestions.as_ref().and_then(|l| l.get(i)).cloned() else {
                return Task::none();
            };
            ed.picking = Some(i);
            bg(
                &backend,
                move |b| {
                    me::fetch_art_from_url(s.full_url.clone(), b.clone())
                        .or_else(|_| me::fetch_art_from_url(s.thumbnail_url.clone(), b))
                },
                |r| am(Msg::Picked2(r)),
            )
        }
        Msg::Picked2(r) => {
            ed.picking = None;
            match r {
                Ok(p) => go_to_crop(app, p),
                Err(e) => {
                    ed.suggestions_error = Some(e);
                    Task::none()
                }
            }
        }
        Msg::Back => {
            ed.mode = ArtMode::Display;
            ed.url.clear();
            ed.url_error = None;
            Task::none()
        }
        Msg::Undo => {
            if let Some(d) = ed.draft.as_mut() {
                d.new_art_path = None;
            }
            Task::none()
        }
    }
}

fn art_btn<'a>(i: Icon, label: &'static str, msg: Option<Message>) -> Element<'a, Message> {
    tip(
        button(ui::icon_box(i, 13.0, 28.0, 28.0))
            .padding(0)
            .style(|th, s| {
                let mut st = theme::btn_outline(th, s);
                st.background = matches!(s, button::Status::Hovered).then(|| a(t().muted, 0.4).into());
                st.text_color = if matches!(s, button::Status::Hovered) { t().foreground } else { t().muted_fg };
                st.border.color = t().border;
                st.border.radius = theme::radius(theme::R_MD);
                st
            })
            .on_press_maybe(msg),
        label,
        Position::Top,
    )
}

pub fn view<'a>(ed: &'a Editor, d: &'a Draft) -> Element<'a, Message> {
    let k = t();
    let current = d
        .new_art_path
        .as_deref()
        .or(ed.meta.as_ref().and_then(|m| m.album_art_path.as_deref()));
    let mut preview = stack![ui::art(current, 152.0, theme::R_LG)];
    if ed.extracting {
        preview = preview.push(
            center(spin(Spinner::Loader, 20.0, spin_angle(), theme::WHITE))
                .width(152)
                .height(152)
                .style(theme::backdrop(0.4)),
        );
    }
    let mut col = column![preview].spacing(8).width(152);

    match ed.mode {
        ArtMode::Display | ArtMode::Crop => {
            let mut btns = row![
                art_btn(Icon::FolderOpen, "Pick from file", Some(am(Msg::PickFile))),
                art_btn(Icon::ImageOff, "Extract embedded art", (!ed.extracting).then(|| am(Msg::Extract))),
                art_btn(Icon::Link2, "From URL", Some(am(Msg::UrlMode))),
                art_btn(Icon::Search, "Online suggestions", Some(am(Msg::OnlineMode))),
            ]
            .spacing(4);
            if d.new_art_path.is_some() {
                btns = btns.push(art_btn(Icon::X, "Undo art change", Some(am(Msg::Undo))));
            }
            col = col.push(container(btns.wrap().vertical_spacing(4)).width(Length::Fill).align_x(Alignment::Center));
        }
        ArtMode::Url => {
            col = col.push(
                text_input("Paste image URL…", &ed.url)
                    .id(URL_INPUT)
                    .on_input(|u| am(Msg::Url(u)))
                    .on_submit(am(Msg::LoadUrl))
                    .size(theme::SMALL)
                    .padding(Padding::from([4.0, 8.0]))
                    .style(theme::input_muted),
            );
            if let Some(e) = &ed.url_error {
                col = col.push(text(e.as_str()).size(theme::SMALL).color(k.destructive));
            }
            let mut load = row![].spacing(4).align_y(Alignment::Center);
            if ed.url_loading {
                load = load.push(spin(Spinner::Loader, 11.0, spin_angle(), k.primary_fg));
            }
            load = load.push(text("Load").size(theme::SMALL));
            let can = !ed.url.trim().is_empty() && !ed.url_loading;
            col = col.push(
                row![
                    button(container(load).width(Length::Fill).align_x(Alignment::Center))
                        .width(Length::Fill)
                        .padding(Padding::from([4.0, 0.0]))
                        .style(|th, s| {
                            let mut st = theme::btn_primary(th, s);
                            st.border.radius = theme::radius(theme::R_MD);
                            st
                        })
                        .on_press_maybe(can.then(|| am(Msg::LoadUrl))),
                    button(container(text("Cancel").size(theme::SMALL)).width(Length::Fill).align_x(Alignment::Center))
                        .width(Length::Fill)
                        .padding(Padding::from([4.0, 0.0]))
                        .style(|th, s| {
                            let mut st = theme::btn_outline(th, s);
                            st.border.radius = theme::radius(theme::R_MD);
                            st
                        })
                        .on_press(am(Msg::Back)),
                ]
                .spacing(6),
            );
        }
        ArtMode::Online => {
            col = col.push(text("Suggestions").size(theme::SMALL).font(theme::MEDIUM).color(k.muted_fg));
            if ed.suggestions_loading {
                col = col.push(center(spin(Spinner::Loader, 18.0, spin_angle(), k.muted_fg)).height(80));
            }
            if let Some(e) = &ed.suggestions_error {
                col = col.push(text(e.as_str()).size(theme::SMALL).color(k.destructive));
            }
            if let (Some(list), false) = (&ed.suggestions, ed.suggestions_loading) {
                if list.is_empty() {
                    col = col.push(ui::muted("No results.", theme::SMALL));
                } else {
                    let mut grid = column![].spacing(4);
                    for (ri, chunk) in list.chunks(3).enumerate() {
                        let mut r = row![].spacing(4);
                        for (ci, (s, handle)) in chunk.iter().enumerate() {
                            let i = ri * 3 + ci;
                            let thumb: Element<'a, Message> = match handle {
                                Some(h) => image(h.clone())
                                    .width(Length::Fill)
                                    .height(48)
                                    .content_fit(ContentFit::Cover)
                                    .border_radius(theme::R_XS)
                                    .into(),
                                None => container(iced::widget::Space::new())
                                    .width(Length::Fill)
                                    .height(48)
                                    .style(|_| iced::widget::container::Style {
                                        background: Some(t().muted.into()),
                                        border: theme::no_border(theme::R_XS),
                                        ..Default::default()
                                    })
                                    .into(),
                            };
                            let mut cell = stack![thumb];
                            if ed.picking == Some(i) {
                                cell = cell.push(
                                    center(spin(Spinner::Loader, 14.0, spin_angle(), theme::WHITE))
                                        .height(48)
                                        .style(theme::backdrop(0.5)),
                                );
                            }
                            let label = match &s.date {
                                Some(dt) => format!("{} ({dt})", s.title),
                                None => s.title.clone(),
                            };
                            r = r.push(
                                container(tip(
                                    button(cell)
                                        .padding(0)
                                        .width(Length::Fill)
                                        .style(|_, st| iced::widget::button::Style {
                                            border: theme::border(
                                                if matches!(st, button::Status::Hovered) { t().ring } else { iced::Color::TRANSPARENT },
                                                2.0,
                                                theme::R_XS,
                                            ),
                                            ..Default::default()
                                        })
                                        .on_press_maybe(ed.picking.is_none().then(|| am(Msg::Pick(i)))),
                                    label,
                                    Position::Top,
                                ))
                                .width(Length::FillPortion(1)),
                            );
                        }
                        for _ in chunk.len()..3 {
                            r = r.push(ui::fill());
                        }
                        grid = grid.push(r);
                    }
                    col = col.push(grid);
                }
            }
            col = col.push(
                button(text("← Back").size(theme::SMALL))
                    .padding(0)
                    .style(theme::btn_icon)
                    .on_press(am(Msg::Back)),
            );
        }
    }
    col.into()
}

#[cfg(test)]
mod tests {
    use super::looks_like_image_url;

    #[test]
    fn image_urls() {
        assert!(looks_like_image_url("https://x.com/a/b.JPG?w=10"));
        assert!(looks_like_image_url("https://i.imgur.com/abc"));
        assert!(!looks_like_image_url("hello world"));
        assert!(!looks_like_image_url("https://example.com/page"));
    }
}
