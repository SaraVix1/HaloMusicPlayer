//! Full-screen now playing (opened by clicking the cover in the now-playing
//! bar): blurred art background, large cover with title and artist · album,
//! synced lyrics around the current line (click to seek), the waveform seek
//! bar and transport controls. Esc / ✕ / clicking the cover closes it; F11
//! toggles OS full screen while it is open.
//!
//! While it's open the app renders with the dark tokens (`Halo::apply_theme`),
//! so every reused widget is light-on-dark over the art.

use halo_core::queue::RepeatMode;
use iced::widget::{button, center, column, container, image, responsive, row, scrollable, stack, text, tooltip::Position, Space};
use iced::{window, Alignment, Element, Length, Padding, Task};

use crate::app::{bg, Halo, Message, PlayerCmd};
use crate::components::{lyrics, now_playing};
use crate::format::duration_u;
use crate::icons::{icon, Icon};
use crate::theme::{self, a, t};
use crate::ui::{art, icon_box, tip};
use crate::widgets::{line, waveform};

/// Side of the shrunken cover that gets blurred (then stretched to fill).
const BG_SIDE: u32 = 48;
const BG_SIGMA: f32 = 3.0;
const COVER_MAX: f32 = 420.0;

#[derive(Default)]
pub struct State {
    pub open: bool,
    /// OS full screen (F11) entered from this view.
    pub os_fullscreen: bool,
    /// Blurred background for this track id.
    bg: Option<(Option<i64>, image::Handle)>,
}

#[derive(Debug, Clone)]
pub enum Msg {
    Open,
    Close,
    ToggleOsFullscreen,
    FindLyrics,
    Background(Option<i64>, Option<(u32, u32, Vec<u8>)>),
}

fn fm(m: Msg) -> Message {
    Message::Fullscreen(m)
}

pub fn open_message() -> Message {
    fm(Msg::Open)
}

pub fn update(app: &mut Halo, m: Msg) -> Task<Message> {
    match m {
        Msg::Open => {
            if app.fullscreen.open {
                return Task::none();
            }
            app.fullscreen.open = true;
            app.apply_theme();
            load_background(app)
        }
        Msg::Close => close(app),
        Msg::ToggleOsFullscreen => {
            let Some(id) = app.main_id else { return Task::none() };
            let st = &mut app.fullscreen;
            st.os_fullscreen = !st.os_fullscreen;
            let mode = if st.os_fullscreen { window::Mode::Fullscreen } else { window::Mode::Windowed };
            window::set_mode(id, mode)
        }
        Msg::FindLyrics => {
            let task = close(app);
            Task::batch([task, lyrics::open(app)])
        }
        Msg::Background(id, img) => {
            if app.fullscreen.open && id == current_id(app) {
                app.fullscreen.bg = img.map(|(w, h, px)| (id, image::Handle::from_rgba(w, h, px)));
            }
            Task::none()
        }
    }
}

fn close(app: &mut Halo) -> Task<Message> {
    let st = &mut app.fullscreen;
    if !st.open {
        return Task::none();
    }
    st.open = false;
    st.bg = None; // free the image
    let leave_os = std::mem::take(&mut st.os_fullscreen);
    app.apply_theme();
    match (leave_os, app.main_id) {
        (true, Some(id)) => window::set_mode(id, window::Mode::Windowed),
        _ => Task::none(),
    }
}

fn current_id(app: &Halo) -> Option<i64> {
    app.player.current_track.as_ref().map(|t| t.track_id)
}

/// Track changed: new background if the view is open.
pub fn on_track(app: &mut Halo) -> Task<Message> {
    if app.fullscreen.open {
        load_background(app)
    } else {
        Task::none()
    }
}

fn load_background(app: &mut Halo) -> Task<Message> {
    let id = current_id(app);
    if app.fullscreen.bg.as_ref().is_some_and(|(bid, _)| *bid == id) {
        return Task::none();
    }
    app.fullscreen.bg = None;
    let path = app
        .player
        .current_track
        .as_ref()
        .and_then(|t| t.album_art_path.clone())
        .filter(|p| !p.is_empty());
    match path {
        Some(p) => bg(&app.backend, move |_| blurred(&p), move |img| fm(Msg::Background(id, img))),
        None => Task::none(),
    }
}

/// The art shrunk to `BG_SIDE` px and Gaussian-blurred: a few KB that look
/// smooth when stretched over the window. `(width, height, RGBA)`.
pub fn blurred(path: &str) -> Option<(u32, u32, Vec<u8>)> {
    let img = ::image::open(path).ok()?;
    Some(blur_rgba(&img.thumbnail(BG_SIDE, BG_SIDE).into_rgba8()))
}

fn blur_rgba(img: &::image::RgbaImage) -> (u32, u32, Vec<u8>) {
    let b = ::image::imageops::blur(img, BG_SIGMA);
    (b.width(), b.height(), b.into_raw())
}

// ── View ─────────────────────────────────────────────────────────────────────

/// The whole main window while the view is open (`title_bar` on top).
pub fn view<'a>(app: &'a Halo, title_bar: Element<'a, Message>) -> Element<'a, Message> {
    let k = t();
    // Background: blurred art (or the accent, darkened) under a scrim.
    let backdrop: Element<'a, Message> = match &app.fullscreen.bg {
        Some((_, h)) => image(h.clone())
            .width(Length::Fill)
            .height(Length::Fill)
            .content_fit(iced::ContentFit::Cover)
            .into(),
        None => container(Space::new())
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_| container::Style {
                background: Some(theme::over_bg(a(k.primary, 0.25)).into()),
                ..container::Style::default()
            })
            .into(),
    };
    let scrim = container(Space::new()).width(Length::Fill).height(Length::Fill).style(theme::scrim);

    let close = tip(
        button(icon_box(Icon::X, 18.0, 40.0, 40.0))
            .padding(0)
            .style(theme::btn_mode(false))
            .on_press(fm(Msg::Close)),
        "Close (Esc) · F11 full screen",
        Position::Left,
    );
    let top = row![Space::new().width(Length::Fill), close];

    let body = responsive(move |size| main_area(app, size)).height(Length::Fill);
    let content = column![top, body, controls(app)]
        .spacing(16)
        .padding(Padding::from([8.0, 48.0]).bottom(32.0))
        .height(Length::Fill);

    stack![backdrop, scrim, column![title_bar, content].height(Length::Fill)]
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

/// Cover + title on the left, lyrics on the right.
fn main_area(app: &Halo, size: iced::Size) -> Element<'_, Message> {
    let k = t();
    let track = app.player.current_track.as_ref();
    let side = COVER_MAX.min(size.width * 0.38).min(size.height - 96.0).max(120.0);

    let title = track.and_then(|t| t.title.clone()).unwrap_or_else(|| "No track playing".into());
    let mut sub: Vec<String> = vec![];
    if let Some(t) = track {
        if !t.artists.is_empty() {
            sub.push(t.artists.join(", "));
        }
        if let Some(al) = t.album_name.clone().filter(|s| !s.is_empty()) {
            sub.push(al);
        }
    }
    let cover = button(art(track.and_then(|t| t.album_art_path.as_deref()), side, theme::R_LG))
        .padding(0)
        .style(theme::btn_bare)
        .on_press(fm(Msg::Close));
    let left = column![
        cover,
        line(title).size(theme::DISPLAY).font(theme::SEMIBOLD).width(Length::Fill),
        line(sub.join(" · ")).size(theme::BODY).color(k.muted_fg).width(Length::Fill),
    ]
    .spacing(8)
    .width(side);

    let right = container(lyrics_side(app)).width(Length::Fill).height(side).align_y(Alignment::Center);

    center(container(row![left, right].spacing(64).align_y(Alignment::Center)).max_width(1100)).into()
}

fn lyrics_side(app: &Halo) -> Element<'_, Message> {
    let k = t();
    match lyrics::excerpt(app) {
        lyrics::Excerpt::Loading => text("Looking for lyrics…").size(theme::BODY).color(k.muted_fg).into(),
        lyrics::Excerpt::None => column![
            text("No lyrics for this song").size(theme::BODY).color(k.muted_fg),
            button(text("Find lyrics").size(theme::BODY))
                .padding(Padding::from([6.0, 14.0]))
                .style(theme::btn_outline)
                .on_press(fm(Msg::FindLyrics)),
        ]
        .spacing(12)
        .into(),
        lyrics::Excerpt::Plain(lines) => {
            let mut col = column![].spacing(6);
            for l in lines {
                col = col.push(text(l.text.as_str()).size(theme::BODY).color(a(k.foreground, 0.85)));
            }
            scrollable(col).direction(theme::vscroll()).style(theme::scroll).height(Length::Fill).into()
        }
        lyrics::Excerpt::Synced { lines, range, active } => {
            let mut col = column![].spacing(14);
            for i in range {
                let ln = &lines[i];
                let is_active = Some(i) == active;
                let past = active.is_some_and(|a_| i < a_);
                let color = if is_active {
                    k.primary
                } else if past {
                    a(k.foreground, 0.35)
                } else {
                    a(k.foreground, 0.6)
                };
                let words = if ln.text.trim().is_empty() { "♪" } else { ln.text.as_str() };
                let mut r = row![].spacing(10).align_y(Alignment::Center);
                r = r.push(container(if is_active { Element::from(icon(Icon::Play, 14.0).color(k.primary)) } else { Space::new().into() }).width(16));
                r = r.push(
                    text(words)
                        .size(theme::TITLE)
                        .font(if is_active { theme::SEMIBOLD } else { theme::SANS })
                        .color(color),
                );
                col = col.push(
                    button(r)
                        .padding(0)
                        .style(theme::btn_bare)
                        .on_press(lyrics::seek(ln.time_ms)),
                );
            }
            col.into()
        }
    }
}

/// Seek bar and transport, centred at the bottom.
fn controls(app: &Halo) -> Element<'_, Message> {
    let k = t();
    let p = &app.player;
    let st = &app.now_playing;
    let has_track = p.current_track.is_some();

    let duration = p.player.duration_ms.unwrap_or(0);
    let pos = st.scrub.unwrap_or(p.player.position_ms);
    let time = |s: String, right: bool| {
        let t_ = text(s).size(theme::SMALL).color(k.muted_fg).width(44);
        if right { t_.align_x(iced::alignment::Horizontal::Right) } else { t_ }
    };
    let seek = row![
        time(duration_u(Some(pos)), true),
        container(waveform(
            st.peaks.as_deref(),
            pos,
            duration,
            !has_track || duration == 0,
            |ms| Message::NowPlaying(now_playing::Msg::Scrub(ms)),
            |ms| Message::NowPlaying(now_playing::Msg::Commit(ms)),
        ))
        .width(Length::Fill)
        .height(32),
        time(duration_u(p.player.duration_ms), false),
    ]
    .spacing(12)
    .align_y(Alignment::Center);

    let mode = |i: Icon, active: bool, label: &'static str, msg: Message| {
        tip(
            button(icon_box(i, 18.0, 40.0, 40.0))
                .padding(0)
                .style(move |th, s| {
                    let mut st = theme::btn_mode(active)(th, s);
                    if active {
                        st.text_color = t().primary;
                    }
                    st
                })
                .on_press(msg),
            label,
            Position::Top,
        )
    };
    let skip = |i: Icon, label: &'static str, cmd: PlayerCmd| {
        tip(
            button(icon_box(i, 22.0, 44.0, 44.0))
                .padding(0)
                .style(theme::btn_icon)
                .on_press_maybe(has_track.then_some(Message::Player(cmd))),
            label,
            Position::Top,
        )
    };
    let play_icon: Element<'_, Message> = if app.is_playing() {
        icon(Icon::Pause, 22.0).into()
    } else {
        container(icon(Icon::Play, 22.0)).padding(Padding::ZERO.left(3.0)).into()
    };
    let play = button(center(play_icon).width(56).height(56))
        .padding(0)
        .style(theme::btn_accent_round)
        .on_press_maybe(has_track.then_some(Message::Player(PlayerCmd::TogglePlay)));
    let repeat_label = match p.repeat {
        RepeatMode::Off => "Repeat off",
        RepeatMode::All => "Repeat all",
        RepeatMode::One => "Repeat one",
    };
    let transport = row![
        mode(Icon::Shuffle, p.shuffle, "Shuffle", Message::Player(PlayerCmd::SetShuffle(!p.shuffle))),
        skip(Icon::SkipBack, "Previous", PlayerCmd::Previous),
        play,
        skip(Icon::SkipForward, "Next", PlayerCmd::Next),
        mode(
            if p.repeat == RepeatMode::One { Icon::Repeat1 } else { Icon::Repeat },
            p.repeat != RepeatMode::Off,
            repeat_label,
            Message::Player(PlayerCmd::CycleRepeat),
        ),
    ]
    .spacing(20)
    .align_y(Alignment::Center);

    column![container(seek).max_width(720), transport]
        .spacing(12)
        .align_x(Alignment::Center)
        .width(Length::Fill)
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blur_keeps_size_and_smooths() {
        // Hard black / white halves become a gradient across the middle.
        let mut img = ::image::RgbaImage::from_pixel(48, 48, ::image::Rgba([0, 0, 0, 255]));
        for y in 0..48 {
            for x in 24..48 {
                img.put_pixel(x, y, ::image::Rgba([255, 255, 255, 255]));
            }
        }
        let (w, h, px) = blur_rgba(&img);
        assert_eq!((w, h), (48, 48));
        assert_eq!(px.len(), 48 * 48 * 4);
        let at = |x: usize| px[(24 * 48 + x) * 4];
        // Edges of the halves stay dark / light; the seam is in between.
        assert!(at(2) < 40 && at(45) > 215, "{} {}", at(2), at(45));
        assert!((40..215).contains(&at(23)) && (40..215).contains(&at(24)));
    }
}
