//! Mini player window (`mini-player.tsx` + the old `commands/window.rs`):
//! a 280×30 always-on-top strip. Left: 6px volume fill bar (drag to set).
//! Middle: art + two-line looping marquee (title · album / composer ·
//! artist) that cross-fades to prev / play / next + time / seek / time when
//! the pointer moves past the art. Right: 12px expand button (click restores
//! the main window, drag moves the mini window). A 2px red progress bar
//! runs along the bottom. Drag anywhere to move; double-click restores;
//! Ctrl+M restores.

use std::time::Duration;

use halo_backend::commands::mini as prefs;
use iced::widget::{
    button, center, column, container, mouse_area, row, slider, stack, svg, text, vertical_slider, Space,
};
use iced::{window, Alignment, Element, Length, Padding, Point, Size, Subscription, Task};

use crate::app::{Halo, Message, PlayerCmd};
use crate::format::duration_u;
use crate::icons::{icon, Icon};
use crate::theme::{self, a, t};
use crate::ui::{art, icon_box};
use crate::widgets::line;

const ART: f32 = 30.0;
const DRAG_THRESHOLD: f32 = 4.0;
const W: f32 = prefs::MINI_W as f32;
const H: f32 = prefs::MINI_H as f32;

#[derive(Default)]
pub struct State {
    hovered: bool,
    scrub: Option<u64>,
    expand_press: Option<Point>,
    expand_last: Point,
    expand_dragged: bool,
    move_token: u64,
    pending_pos: Option<Point>,
    /// Waiting for the main window's scale factor before opening.
    opening: bool,
    /// Scale factor the window was opened with: positions are stored in
    /// physical pixels (as the Tauri build did) but iced works in logical.
    scale: f32,
}

#[derive(Debug, Clone)]
pub enum Msg {
    Move(Point),
    Leave,
    Drag,
    Restore,
    Volume(f32),
    Scrub(u64),
    Commit,
    ExpandPress,
    ExpandMove(Point),
    ExpandRelease,
    SavePos(u64),
    /// Second half of [`open`]: the main window's scale factor arrived.
    OpenWith(f32),
}

fn mm(m: Msg) -> Message {
    Message::Mini(m)
}

fn bottom_right(win: Size, monitor: Size) -> Point {
    // 8px from the right edge; 56px clears most taskbars / docks.
    Point::new(monitor.width - win.width - 8.0, monitor.height - win.height - 56.0)
}

/// Open the mini window (or focus it if it's already open). The saved
/// position is in physical pixels, so first fetch the display scale factor
/// (from the main window) and open in [`open_with`].
pub fn open(app: &mut Halo) -> Task<Message> {
    if let Some(id) = app.mini_id {
        return window::gain_focus(id);
    }
    if app.mini.opening {
        return Task::none();
    }
    app.mini.opening = true;
    match app.main_id {
        Some(main) => window::scale_factor(main).map(|s| mm(Msg::OpenWith(s))),
        None => Task::done(mm(Msg::OpenWith(1.0))),
    }
}

fn open_with(app: &mut Halo, scale: f32) -> Task<Message> {
    if let Some(id) = app.mini_id {
        return window::gain_focus(id);
    }
    let scale = if scale.is_finite() && scale > 0.0 { scale } else { 1.0 };
    let aot = prefs::always_on_top(app.backend.db());
    let phys = |v: f32| (v * scale).round() as i32;
    let position = match prefs::open_position_physical(app.backend.db(), phys(W), phys(H), phys(8.0)) {
        Some((x, y)) => window::Position::Specific(Point::new(x as f32 / scale, y as f32 / scale)),
        None if cfg!(target_os = "macos") => window::Position::Centered,
        None => window::Position::SpecificWith(bottom_right),
    };
    let settings = window::Settings {
        size: Size::new(W, H),
        min_size: Some(Size::new(W, H)),
        max_size: Some(Size::new(W, H)),
        position,
        resizable: false,
        decorations: false,
        // Lets GTK/Wayland honour a 30px tall window. Linux only: on Windows
        // a transparent wgpu surface can leave the window invisible.
        transparent: cfg!(target_os = "linux"),
        level: if aot { window::Level::AlwaysOnTop } else { window::Level::Normal },
        exit_on_close_request: false,
        icon: crate::app_icon(),
        #[cfg(windows)]
        platform_specific: window::settings::PlatformSpecific {
            skip_taskbar: true,
            corner_preference: window::settings::platform::CornerPreference::DoNotRound,
            ..Default::default()
        },
        #[cfg(target_os = "linux")]
        platform_specific: window::settings::PlatformSpecific {
            application_id: halo_backend::APP_IDENTIFIER.into(),
            ..Default::default()
        },
        ..window::Settings::default()
    };
    let (id, task) = window::open(settings);
    app.mini_id = Some(id);
    app.mini = State {
        scale,
        ..State::default()
    };
    // Wayland may map the window after `open`; re-apply always-on-top a few
    // times (the old Tauri build retried at ~100ms–2s for the same reason).
    let level = if aot { window::Level::AlwaysOnTop } else { window::Level::Normal };
    let retries = [100u64, 250, 500, 1000, 2000].map(move |ms| {
        Task::perform(tokio::time::sleep(Duration::from_millis(ms)), move |_| ())
            .then(move |_| window::set_level::<Message>(id, level))
    });
    Task::batch([task.then(window::gain_focus)].into_iter().chain(retries))
}

/// Mini window moved: save its position (debounced 300ms).
pub fn on_moved(app: &mut Halo, pos: Point) -> Task<Message> {
    app.mini.move_token += 1;
    app.mini.pending_pos = Some(pos);
    let token = app.mini.move_token;
    Task::perform(tokio::time::sleep(Duration::from_millis(300)), move |_| mm(Msg::SavePos(token)))
}

pub fn subscription(_app: &Halo) -> Option<Subscription<Message>> {
    None
}

pub fn update(app: &mut Halo, m: Msg) -> Task<Message> {
    let st = &mut app.mini;
    match m {
        Msg::Move(p) => {
            if !st.hovered && p.x > ART {
                st.hovered = true;
            }
            Task::none()
        }
        Msg::Leave => {
            st.hovered = false;
            Task::none()
        }
        Msg::Drag => match app.mini_id {
            Some(id) => window::drag(id),
            None => Task::none(),
        },
        Msg::Restore => app.restore_main(),
        Msg::Volume(v) => app.update(Message::Player(PlayerCmd::SetVolume(v.clamp(0.0, 1.0)))),
        Msg::Scrub(ms) => {
            st.scrub = Some(ms);
            Task::none()
        }
        Msg::Commit => match st.scrub.take() {
            Some(ms) => app.update(Message::Player(PlayerCmd::Seek(ms))),
            None => Task::none(),
        },
        Msg::ExpandPress => {
            st.expand_press = Some(st.expand_last);
            st.expand_dragged = false;
            Task::none()
        }
        Msg::ExpandMove(p) => {
            st.expand_last = p;
            let Some(start) = st.expand_press else { return Task::none() };
            if !st.expand_dragged
                && ((p.x - start.x).abs() > DRAG_THRESHOLD || (p.y - start.y).abs() > DRAG_THRESHOLD)
            {
                st.expand_dragged = true;
                st.expand_press = None;
                return match app.mini_id {
                    Some(id) => window::drag(id),
                    None => Task::none(),
                };
            }
            Task::none()
        }
        Msg::ExpandRelease => {
            let was_pressed = st.expand_press.take().is_some();
            if was_pressed && !st.expand_dragged {
                return app.restore_main();
            }
            Task::none()
        }
        Msg::SavePos(token) => {
            if token != st.move_token {
                return Task::none();
            }
            let Some(p) = st.pending_pos.take() else { return Task::none() };
            let scale = if st.scale > 0.0 { st.scale } else { 1.0 };
            let (x, y) = ((p.x * scale).round() as i32, (p.y * scale).round() as i32);
            // Like the Tauri build: a window dropped partly off-screen or
            // under the taskbar snaps back into the work area (Windows).
            let (w, h) = ((W * scale).round() as i32, (H * scale).round() as i32);
            let (cx, cy) = match prefs::work_area_near(x + w / 2, y + h / 2) {
                Some(area) => prefs::clamp_into(x, y, w, h, area),
                None => (x, y),
            };
            let save = crate::app::fire(&app.backend, move |b| prefs::save_position(b.db(), cx, cy));
            match app.mini_id {
                Some(id) if (cx, cy) != (x, y) => Task::batch([
                    window::move_to(id, Point::new(cx as f32 / scale, cy as f32 / scale)),
                    save,
                ]),
                _ => save,
            }
        }
        Msg::OpenWith(scale) => {
            st.opening = false;
            open_with(app, scale)
        }
    }
}

static SVG_TRIANGLE: std::sync::LazyLock<svg::Handle> = std::sync::LazyLock::new(|| {
    svg::Handle::from_memory(
        br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 6 8"><path d="M0 0 L6 4 L0 8 Z" fill="currentColor"/></svg>"#
            .as_slice(),
    )
});

pub fn view(app: &Halo) -> Element<'_, Message> {
    let k = t();
    let st = &app.mini;
    let p = &app.player;
    let track = p.current_track.as_ref();
    let has_track = track.is_some();
    let is_playing = app.is_playing();
    let duration = p.player.duration_ms.unwrap_or(0);
    let pos = st.scrub.unwrap_or(p.player.position_ms);

    // ── Volume fill bar (6px) ─────────────────────────────────────────────────
    let vol = vertical_slider(0.0..=1.0_f32, p.player.volume, |v| mm(Msg::Volume(v)))
        .step(0.01_f32)
        .width(6)
        .height(H)
        .style(|_, _| {
            let k = t();
            iced::widget::slider::Style {
                rail: iced::widget::slider::Rail {
                    backgrounds: (k.primary.into(), k.muted.into()),
                    width: 6.0,
                    border: theme::no_border(0.0),
                },
                handle: iced::widget::slider::Handle {
                    shape: iced::widget::slider::HandleShape::Rectangle {
                        width: 0,
                        border_radius: 0.0.into(),
                    },
                    background: iced::Color::TRANSPARENT.into(),
                    border_width: 0.0,
                    border_color: iced::Color::TRANSPARENT,
                },
            }
        });

    // ── Default layer: art + two marquee lines ───────────────────────────────
    let title = track
        .and_then(|t| t.title.clone())
        .unwrap_or_else(|| "No track playing".into());
    let album = track.and_then(|t| t.album_name.clone()).unwrap_or_default();
    let composer = track.map(|t| t.composers.join(", ")).filter(|s| !s.is_empty());
    let artist = track
        .map(|t| t.artists.join(", "))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "—".into());
    let dot = || text("·").size(theme::SMALL).color(a(k.muted_fg, 0.6));
    let mut l1 = row![line(title)
        .size(theme::SMALL)
        .font(theme::MEDIUM)
        .line_height_px(13.0)
        .color(if has_track { k.foreground } else { a(k.foreground, 0.4) })
        .max_width(120.0)]
    .spacing(4)
    .align_y(Alignment::Center);
    if !album.is_empty() {
        l1 = l1.push(dot()).push(
            line(album)
                .size(theme::SMALL)
                .line_height_px(13.0)
                .color(k.muted_fg)
                .width(Length::Fill)
                .looping(),
        );
    }
    let l2: Element<'_, Message> = match composer {
        Some(c) => {
            let mut r = row![line(c)
                .size(theme::SMALL)
                .line_height_px(11.0)
                .color(k.muted_fg)
                .max_width(120.0)]
            .spacing(4)
            .align_y(Alignment::Center);
            if artist != "—" {
                r = r.push(dot()).push(
                    line(artist)
                        .size(theme::SMALL)
                        .line_height_px(11.0)
                        .color(k.muted_fg)
                        .width(Length::Fill)
                        .looping(),
                );
            }
            r.into()
        }
        None => line(artist)
            .size(theme::SMALL)
            .line_height_px(11.0)
            .color(k.muted_fg)
            .width(Length::Fill)
            .looping()
            .into(),
    };
    let default_layer = row![
        art(track.and_then(|t| t.album_art_path.as_deref()), ART, 0.0),
        column![l1, l2].spacing(1).width(Length::Fill).padding(Padding::ZERO.bottom(2.0)),
    ]
    .spacing(8)
    .align_y(Alignment::Center)
    .height(H);

    // ── Control layer ─────────────────────────────────────────────────────────
    let skip = |i: Icon, c: PlayerCmd| {
        button(icon(i, 13.0))
            .padding(0)
            .style(theme::btn_icon)
            .on_press_maybe(has_track.then_some(Message::Player(c)))
    };
    let play_icon: Element<'_, Message> = if is_playing {
        icon(Icon::Pause, 10.0).into()
    } else {
        container(icon(Icon::Play, 10.0)).padding(Padding::ZERO.left(2.0)).into()
    };
    let play_enabled = has_track || p.player.status != halo_backend::audio::PlaybackStatus::Stopped;
    let controls = row![
        skip(Icon::SkipBack, PlayerCmd::Previous),
        button(center(play_icon).width(20).height(20))
            .padding(0)
            .style(theme::btn_play)
            .on_press_maybe(play_enabled.then_some(Message::Player(PlayerCmd::TogglePlay))),
        skip(Icon::SkipForward, PlayerCmd::Next),
    ]
    .spacing(6)
    .align_y(Alignment::Center);
    let max = duration.max(1) as f64;
    let seek_enabled = has_track && duration > 0;
    let seek = slider(0.0..=max, (pos as f64).min(max), move |v| mm(Msg::Scrub(v as u64)))
        .on_release(mm(Msg::Commit))
        .width(Length::Fill)
        .style(theme::slider_style);
    let seek: Element<'_, Message> = if seek_enabled {
        seek.into()
    } else {
        slider(0.0..=1.0_f64, 0.0, |_| Message::Noop)
            .width(Length::Fill)
            .style(theme::slider_style)
            .into()
    };
    let time = |s: String, right: bool| {
        let tx = text(s).size(theme::SMALL).color(k.muted_fg).width(28);
        if right { tx.align_x(iced::alignment::Horizontal::Right) } else { tx }
    };
    let control_layer = row![
        controls,
        row![time(duration_u(Some(pos)), true), seek, time(duration_u(p.player.duration_ms), false)]
            .spacing(4)
            .align_y(Alignment::Center)
            .width(Length::Fill),
    ]
    .spacing(8)
    .padding(Padding::ZERO.left(6.0))
    .align_y(Alignment::Center)
    .height(H);

    // Hover gate: only once the pointer moves past the art (ART px).
    let content: Element<'_, Message> = if st.hovered {
        control_layer.into()
    } else {
        default_layer.into()
    };
    let content = mouse_area(container(content).width(Length::Fill).height(H))
        .on_move(|p| mm(Msg::Move(p)))
        .on_exit(mm(Msg::Leave))
        .on_press(mm(Msg::Drag))
        .on_double_click(mm(Msg::Restore));

    // ── Expand button (12px) ──────────────────────────────────────────────────
    let expand = mouse_area(
        crate::widgets::surface(
            center(
                svg(SVG_TRIANGLE.clone())
                    .width(6)
                    .height(8)
                    .style(|_, s| iced::widget::svg::Style {
                        color: Some(if matches!(s, iced::widget::svg::Status::Hovered) {
                            t().foreground
                        } else {
                            t().muted_fg
                        }),
                    }),
            )
            .width(12)
            .height(H),
        )
        .style(|s| iced::widget::container::Style {
            background: Some(a(t().muted, if s.hovered { 0.7 } else { 0.4 }).into()),
            ..Default::default()
        }),
    )
    .on_press(mm(Msg::ExpandPress))
    .on_move(|p| mm(Msg::ExpandMove(p)))
    .on_release(mm(Msg::ExpandRelease));
    let expand = crate::ui::tip(expand, "Restore full window (Ctrl+M)", iced::widget::tooltip::Position::Top);

    let strip = row![vol, content, expand].height(H).width(W);

    // ── Red progress bar (default layer only) ────────────────────────────────
    let mut layers = stack![container(strip).style(theme::page)];
    if !st.hovered {
        let pct = if duration > 0 { (pos as f32 / duration as f32).clamp(0.0, 1.0) } else { 0.0 };
        let bar = crate::ui::progress(pct, 2.0, theme::RED_500, 0.0);
        layers = layers.push(
            column![
                Space::new().height(H - 2.0),
                row![
                    Space::new().width(6.0 + ART),
                    container(bar).width(Length::Fill).height(2).style(|_| iced::widget::container::Style {
                        background: Some(a(theme::RED_900, 0.3).into()),
                        ..Default::default()
                    }),
                ],
            ]
            .width(W)
            .height(H),
        );
    }
    let _ = icon_box::<Message>;
    layers.width(W).height(H).into()
}
