//! Now-playing bar (`now-playing.tsx` + `sleep-timer-button.tsx`): track
//! info with marquee, star rating, transport + shuffle/repeat, waveform seek
//! with time labels, Last.fm love, EQ shortcut, lyrics toggle, sleep timer,
//! mute and volume.

use halo_backend::commands::{self, lastfm::LastFmState};
use halo_backend::Backend;
use halo_core::queue::RepeatMode;
use iced::widget::{button, column, container, row, slider, text, tooltip::Position, Space};
use iced::{Alignment, Element, Length, Padding, Task};

use crate::app::{bg, fire, Halo, Message, PlayerCmd};
use crate::format::{countdown, duration_u};
use crate::icons::{icon, Icon};
use crate::route::{Route, SettingsTab};
use crate::theme::{self, a, t};
use crate::ui::{self, art, filled, icon_box, tip, Filled};
use crate::widgets::{line, popover, waveform, Placement};

const PRESETS: [u64; 6] = [5, 10, 15, 30, 45, 60];

#[derive(Default)]
pub struct State {
    /// Position under the pointer while scrubbing the waveform.
    pub scrub: Option<u64>,
    pub peaks: Option<Vec<u8>>,
    pub lfm_connected: bool,
    pub loved: Option<bool>,
    pub star_hover: Option<u8>,
    pub sleep_open: bool,
    pub sleep_fade: bool,
    pub volume_drag: Option<f32>,
}

#[derive(Debug, Clone)]
pub enum Msg {
    Peaks(i64, Vec<u8>),
    LastFm(i64, bool, Option<bool>),
    ToggleLove,
    LoveFailed(bool),
    Scrub(u64),
    Commit(u64),
    StarHover(Option<u8>),
    Rate(i64),
    Volume(f32),
    VolumeRelease,
    OpenEq,
    ToggleLyrics,
    SleepToggle,
    SleepClose,
    SleepPreset(u64),
    SleepEndOfSong,
    SleepCancel,
    SleepFade,
}

fn lastfm_status(b: Backend, track_id: i64) -> (bool, Option<bool>) {
    let status = commands::lastfm::lastfm_get_status(b.clone(), b.state::<LastFmState>());
    if !status.connected {
        return (false, None);
    }
    let loved =
        commands::lastfm::lastfm_is_loved(b.clone(), b.state::<LastFmState>(), track_id).ok();
    (true, loved)
}

/// Called when the current track changes.
pub fn on_track(app: &mut Halo, track_id: Option<i64>) -> Task<Message> {
    app.now_playing.peaks = None;
    app.now_playing.star_hover = None;
    let Some(id) = track_id else {
        app.now_playing.loved = None;
        return Task::none();
    };
    Task::batch([
        bg(
            &app.backend,
            move |b| commands::player::get_waveform(b.db(), id).unwrap_or_default(),
            move |p| Message::NowPlaying(Msg::Peaks(id, p)),
        ),
        bg(
            &app.backend,
            move |b| lastfm_status(b, id),
            move |(c, l)| Message::NowPlaying(Msg::LastFm(id, c, l)),
        ),
    ])
}

fn current_id(app: &Halo) -> Option<i64> {
    app.player.current_track.as_ref().map(|t| t.track_id)
}

pub fn update(app: &mut Halo, msg: Msg) -> Task<Message> {
    let st = &mut app.now_playing;
    match msg {
        Msg::Peaks(id, p) => {
            if current_id(app) == Some(id) {
                app.now_playing.peaks = (!p.is_empty()).then_some(p);
            }
            Task::none()
        }
        Msg::LastFm(id, connected, loved) => {
            if current_id(app) == Some(id) {
                app.now_playing.lfm_connected = connected;
                app.now_playing.loved = if connected { loved } else { None };
            }
            Task::none()
        }
        Msg::ToggleLove => {
            let (Some(id), Some(loved)) = (current_id(app), app.now_playing.loved) else {
                return Task::none();
            };
            let next = !loved;
            app.now_playing.loved = Some(next); // optimistic; revert on failure
            bg(
                &app.backend,
                move |b| commands::lastfm::lastfm_love(b.clone(), b.state::<LastFmState>(), id, next),
                move |r| match r {
                    Ok(()) => Message::Noop,
                    Err(_) => Message::NowPlaying(Msg::LoveFailed(!next)),
                },
            )
        }
        Msg::LoveFailed(prev) => {
            app.now_playing.loved = Some(prev);
            Task::none()
        }
        Msg::Scrub(ms) => {
            st.scrub = Some(ms);
            Task::none()
        }
        Msg::Commit(ms) => {
            st.scrub = None;
            app.update(Message::Player(PlayerCmd::Seek(ms)))
        }
        Msg::StarHover(h) => {
            st.star_hover = h;
            Task::none()
        }
        Msg::Rate(r) => match current_id(app) {
            Some(id) => app.update(Message::Player(PlayerCmd::SetRating(id, r))),
            None => Task::none(),
        },
        Msg::Volume(v) => {
            st.volume_drag = Some(v);
            app.update(Message::Player(PlayerCmd::SetVolume(v)))
        }
        Msg::VolumeRelease => {
            st.volume_drag = None;
            Task::none()
        }
        Msg::OpenEq => app.update(Message::Navigate(Route::Settings(Some(SettingsTab::Equalizer)))),
        Msg::ToggleLyrics => crate::components::lyrics::toggle(app),
        Msg::SleepToggle => {
            st.sleep_open = !st.sleep_open;
            Task::none()
        }
        Msg::SleepClose => {
            st.sleep_open = false;
            Task::none()
        }
        Msg::SleepPreset(m) => {
            st.sleep_open = false;
            let fade = st.sleep_fade;
            fire(&app.backend, move |b| {
                commands::sleep_timer::set_sleep_timer(b, m, fade)
            })
        }
        Msg::SleepEndOfSong => {
            st.sleep_open = false;
            let fade = st.sleep_fade;
            fire(&app.backend, move |b| {
                commands::sleep_timer::set_sleep_timer_end_of_song(b, fade)
            })
        }
        Msg::SleepCancel => fire(&app.backend, commands::sleep_timer::cancel_sleep_timer),
        Msg::SleepFade => {
            st.sleep_fade = !st.sleep_fade;
            Task::none()
        }
    }
}

fn np(m: Msg) -> Message {
    Message::NowPlaying(m)
}

pub fn view(app: &Halo) -> Element<'_, Message> {
    let k = t();
    let p = &app.player;
    let st = &app.now_playing;
    let track = p.current_track.as_ref();
    let has_track = track.is_some();
    let is_playing = app.is_playing();

    // ── Track info (320px) ────────────────────────────────────────────────────
    let title = track
        .and_then(|t| t.title.clone())
        .unwrap_or_else(|| "No track playing".into());
    let composer = track
        .map(|t| t.composers.join(", "))
        .filter(|s| !s.is_empty());
    let artist = track
        .map(|t| t.artists.join(", "))
        .filter(|s| !s.is_empty());
    let dot = || text("·").size(theme::SMALL).color(a(k.muted_fg, 0.5));

    let mut line1 = row![line(title)
        .size(theme::BODY)
        .font(theme::MEDIUM)
        .color(if has_track { k.foreground } else { a(k.foreground, 0.4) })
        .max_width(155.0)]
    .spacing(4)
    .align_y(Alignment::Center);
    if let Some(album) = track.and_then(|t| t.album_name.clone()) {
        line1 = line1.push(dot()).push(
            line(album)
                .size(theme::SMALL)
                .color(k.muted_fg)
                .width(Length::Fill)
                .marquee(),
        );
    }

    let line2: Element<'_, Message> = match (composer, artist) {
        (Some(c), artist) => {
            let mut r = row![line(c).size(theme::SMALL).color(k.muted_fg).max_width(155.0)]
                .spacing(4)
                .align_y(Alignment::Center);
            if let Some(a_) = artist {
                r = r.push(dot()).push(
                    line(a_)
                        .size(theme::SMALL)
                        .color(k.muted_fg)
                        .width(Length::Fill)
                        .marquee(),
                );
            }
            r.into()
        }
        (None, Some(a_)) => line(a_)
            .size(theme::SMALL)
            .color(k.muted_fg)
            .width(Length::Fill)
            .marquee()
            .into(),
        (None, None) => text("—").size(theme::SMALL).color(k.muted_fg).into(),
    };

    // Last.fm love, beside the title.
    let love: Option<Element<'_, Message>> = (st.lfm_connected && has_track).then(|| {
        let loved = st.loved.unwrap_or(false);
        let heart: Element<'_, Message> = if loved {
            filled(Filled::Heart, 16.0, theme::RED_500).into()
        } else {
            icon(Icon::Heart, 16.0).into()
        };
        tip(
            button(heart)
                .padding(0)
                .style(if loved {
                    Box::new(theme::btn_text(theme::RED_500, theme::RED_400))
                        as Box<dyn Fn(&iced::Theme, button::Status) -> button::Style>
                } else {
                    Box::new(theme::btn_icon)
                })
                .on_press_maybe(st.loved.is_some().then_some(np(Msg::ToggleLove))),
            if loved { "Loved on Last.fm" } else { "Love on Last.fm" },
            Position::Top,
        )
    });

    // Clicking the cover opens full-screen now playing.
    let cover = button(art(track.and_then(|t| t.album_art_path.as_deref()), 48.0, theme::R_SM))
        .padding(0)
        .style(theme::btn_bare)
        .on_press_maybe(has_track.then(crate::components::fullscreen::open_message));
    let info = row![
        tip(cover, "Full-screen player", Position::Top),
        column![line1, line2].spacing(2).width(Length::Fill),
    ]
    .spacing(12)
    .align_y(Alignment::Center)
    .width(Length::Fill);
    let info = match love {
        Some(h) => row![info, h].spacing(12).align_y(Alignment::Center),
        None => row![info],
    }
    .width(320);

    // ── Transport ─────────────────────────────────────────────────────────────
    let mode_btn = |i: Icon, active: bool, label: &'static str, msg: Message| {
        let content = iced::widget::stack![
            icon_box(i, 15.0, 28.0, 28.0),
            container(if active {
                Element::from(
                    container(Space::new())
                        .width(4)
                        .height(4)
                        .style(|_| container::Style {
                            background: Some(t().primary.into()),
                            border: theme::no_border(theme::R_FULL),
                            ..container::Style::default()
                        }),
                )
            } else {
                Space::new().into()
            })
            .width(28)
            .height(28)
            .align_x(Alignment::Center)
            .align_y(Alignment::End)
            .padding(Padding::ZERO.bottom(2.0)),
        ];
        tip(
            button(content).padding(0).style(theme::btn_mode(active)).on_press(msg),
            label,
            Position::Top,
        )
    };

    let skip = |i: Icon, msg: PlayerCmd| {
        button(icon_box(i, 18.0, 22.0, 28.0))
            .padding(0)
            .style(theme::btn_icon)
            .on_press_maybe(has_track.then_some(Message::Player(msg)))
    };

    let play_icon: Element<'_, Message> = if is_playing {
        icon(Icon::Pause, 14.0).into()
    } else {
        container(icon(Icon::Play, 14.0)).padding(Padding::ZERO.left(2.0)).into()
    };
    let play_enabled = has_track || p.player.status != halo_backend::audio::PlaybackStatus::Stopped;
    let play = button(iced::widget::center(play_icon).width(32).height(32))
        .padding(0)
        .style(theme::btn_play)
        .on_press_maybe(play_enabled.then_some(Message::Player(PlayerCmd::TogglePlay)));

    let repeat_label = match p.repeat {
        RepeatMode::Off => "Repeat off",
        RepeatMode::All => "Repeat all",
        RepeatMode::One => "Repeat one",
    };
    let controls = row![
        mode_btn(
            Icon::Shuffle,
            p.shuffle,
            "Shuffle",
            Message::Player(PlayerCmd::SetShuffle(!p.shuffle))
        ),
        tip(skip(Icon::SkipBack, PlayerCmd::Previous), "Previous", Position::Top),
        play,
        tip(skip(Icon::SkipForward, PlayerCmd::Next), "Next", Position::Top),
        mode_btn(
            if p.repeat == RepeatMode::One { Icon::Repeat1 } else { Icon::Repeat },
            p.repeat != RepeatMode::Off,
            repeat_label,
            Message::Player(PlayerCmd::CycleRepeat)
        ),
    ]
    .spacing(8)
    .align_y(Alignment::Center);

    let duration = p.player.duration_ms.unwrap_or(0);
    let display_pos = st.scrub.unwrap_or(p.player.position_ms);
    let time_label = |s: String, right: bool| {
        let t_ = text(s)
            .size(theme::SMALL)
            .color(k.muted_fg)
            .line_height(1.0)
            .width(36);
        if right {
            t_.align_x(iced::alignment::Horizontal::Right)
        } else {
            t_
        }
    };
    let seek = row![
        time_label(duration_u(Some(display_pos)), true),
        container(waveform(
            st.peaks.as_deref(),
            display_pos,
            duration,
            !has_track || duration == 0,
            |ms| np(Msg::Scrub(ms)),
            |ms| np(Msg::Commit(ms)),
        ))
        .width(Length::Fill)
        .height(28),
        time_label(duration_u(p.player.duration_ms), false),
    ]
    .spacing(8)
    .align_y(Alignment::Center);
    let center_col = container(controls).width(Length::Fill).align_x(Alignment::Center);

    // ── Right controls (176px) ────────────────────────────────────────────────
    let mut right = row![].spacing(12).align_y(Alignment::Center);
    right = right.push(tip(
        button(icon(Icon::Mic2, 16.0))
            .padding(0)
            .style(if app.lyrics.open {
                Box::new(theme::btn_text(k.primary, a(k.primary, 0.8)))
                    as Box<dyn Fn(&iced::Theme, button::Status) -> button::Style>
            } else {
                Box::new(theme::btn_icon)
            })
            .on_press(np(Msg::ToggleLyrics)),
        "Lyrics",
        Position::Top,
    ));
    right = right.push(tip(
        button(icon(Icon::ListMusic, 16.0))
            .padding(0)
            .style(if app.queue_panel.open {
                Box::new(theme::btn_text(k.primary, a(k.primary, 0.8)))
                    as Box<dyn Fn(&iced::Theme, button::Status) -> button::Style>
            } else {
                Box::new(theme::btn_icon)
            })
            .on_press(crate::components::queue_panel::toggle_message()),
        "Queue",
        Position::Top,
    ));
    right = right.push(sleep_timer(app));
    right = right.push(tip(
        button(icon(Icon::SlidersHorizontal, 16.0))
            .padding(0)
            .style(theme::btn_icon)
            .on_press(np(Msg::OpenEq)),
        "Equalizer",
        Position::Top,
    ));
    let vol = st.volume_drag.unwrap_or(p.player.volume);
    right = right.push(tip(
        button(icon(if vol > 0.0 { Icon::Volume2 } else { Icon::VolumeX }, 16.0))
            .padding(0)
            .style(theme::btn_icon)
            .on_press(Message::Player(PlayerCmd::ToggleMute)),
        if vol > 0.0 { "Mute" } else { "Unmute" },
        Position::Top,
    ));
    right = right.push(
        slider(0.0..=100.0, (vol * 100.0).round(), |v| np(Msg::Volume(v / 100.0)))
            .on_release(np(Msg::VolumeRelease))
            .step(1.0_f32)
            .width(80)
            .height(16)
            .style(theme::slider_style),
    );
    let right = container(right).width(320).align_x(Alignment::End);

    // Row 1: track · transport · tools; row 2: the seek bar, full width.
    let top = row![info, center_col, right]
        .spacing(16)
        .height(60)
        .align_y(Alignment::Center);
    let bar = column![top, seek]
        .spacing(2)
        .padding(Padding::from([4.0, 16.0]).bottom(8.0));

    container(bar).width(Length::Fill).style(theme::frame).into()
}

fn sleep_timer(app: &Halo) -> Element<'_, Message> {
    let k = t();
    let s = &app.player.sleep_timer;
    let active = s.active;

    let mut label = row![icon(Icon::Moon, 15.0)].spacing(4).align_y(Alignment::Center);
    if active && s.end_of_song {
        label = label.push(text("EOS").size(theme::SMALL).line_height(1.0));
    } else if active {
        if let Some(rem) = s.remaining_secs {
            label = label.push(text(countdown(rem)).size(theme::SMALL).line_height(1.0));
        }
    }
    let anchor = button(label)
        .padding(0)
        .style(if active {
            Box::new(theme::btn_text(k.primary, a(k.primary, 0.8)))
                as Box<dyn Fn(&iced::Theme, button::Status) -> button::Style>
        } else {
            Box::new(theme::btn_icon)
        })
        .on_press(np(Msg::SleepToggle));

    let popup = app.now_playing.sleep_open.then(|| {
        let mut header = row![text("Sleep timer").size(theme::SMALL).font(theme::MEDIUM), ui::fill()]
            .align_y(Alignment::Center);
        if active {
            header = header.push(
                button(
                    row![icon(Icon::X, 11.0), text("Cancel").size(theme::SMALL)]
                        .spacing(4)
                        .align_y(Alignment::Center),
                )
                .padding(0)
                .style(theme::btn_text(k.destructive, a(k.destructive, 0.8)))
                .on_press(np(Msg::SleepCancel)),
            );
        }
        let mut grid = column![].spacing(6);
        for chunk in PRESETS.chunks(3) {
            let mut r = row![].spacing(6);
            for &m in chunk {
                let is_active = active
                    && !s.end_of_song
                    && s
                        .remaining_secs
                        .is_some_and(|rem| (rem as i64 - m as i64 * 60).abs() < 30);
                r = r.push(
                    button(
                        container(text(format!("{m}m")).size(theme::SMALL))
                            .width(Length::Fill)
                            .align_x(Alignment::Center),
                    )
                    .width(Length::Fill)
                    .padding(Padding::from([4.0, 8.0]))
                    .style(theme::btn_chip(is_active, theme::R_XS))
                    .on_press(np(Msg::SleepPreset(m))),
                );
            }
            grid = grid.push(r);
        }
        let eos = button(text("End of current song").size(theme::SMALL))
            .width(Length::Fill)
            .padding(Padding::from([6.0, 8.0]))
            .style(theme::btn_chip(active && s.end_of_song, theme::R_XS))
            .on_press(np(Msg::SleepEndOfSong));
        let fade = ui::check(app.now_playing.sleep_fade, "Fade out", np(Msg::SleepFade));

        Element::from(
            container(column![header, grid, eos, fade].spacing(10))
                .width(208)
                .padding(12)
                .style(theme::popover),
        )
    });

    popover(anchor, popup)
        .placement(Placement::AboveEnd)
        .gap(8.0)
        .on_dismiss(np(Msg::SleepClose))
        .into()
}
