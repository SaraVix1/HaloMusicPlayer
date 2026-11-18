//! Settings → Equalizer: bypass toggle, per-device profile bar, built-in
//! preset chips (active highlighted), user presets (chips with ×, "Save
//! current curve as…"), ten vertical ±12 dB band sliders with a dB axis,
//! and the sound enhancements (stereo expander + width, dynamic EQ).

use halo_backend::commands::eq::{self, DeviceEqProfile, EqConfig, UserPreset};
use halo_backend::Backend;
use iced::widget::{button, column, container, row, slider, text, text_input, vertical_slider, Space};
use iced::{Alignment, Element, Length, Padding, Task};

use super::{done, err, label, msg, outline_btn, setting_row, Msg as SMsg};
use crate::app::{bg, Halo, Message};
use crate::icons::{icon, Icon};
use crate::theme::{self, a, t};
use crate::ui::{self, hline, toggle};

pub const BAND_LABELS: [&str; 10] = ["32", "64", "125", "250", "500", "1k", "2k", "4k", "8k", "16k"];
pub const PRESETS: [&str; 10] = [
    "Flat", "Rock", "Pop", "Jazz", "Classical", "Electronic", "Bass Boost", "Treble Boost", "Vocal",
    "Acoustic",
];
/// Mirror of `preset_gains` in the backend so the active preset can be
/// highlighted.
pub const PRESET_GAINS: [[f32; 10]; 10] = [
    [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [4.0, 3.0, -1.0, -1.0, 0.0, 1.0, 2.0, 3.0, 3.0, 3.0],
    [-1.5, -1.0, 0.0, 2.0, 4.0, 4.0, 2.0, 0.0, -1.0, -1.5],
    [3.0, 2.0, 1.0, 2.0, -2.0, -2.0, 0.0, 1.0, 2.0, 3.0],
    [4.0, 3.0, -1.0, -1.0, -1.0, 0.0, 0.0, 1.0, 3.0, 4.0],
    [4.0, 3.5, 1.0, 0.0, -1.0, 1.0, 0.0, 1.0, 3.0, 4.0],
    [6.0, 5.0, 4.0, 2.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 2.0, 4.0, 5.0, 6.0],
    [-2.0, -2.0, -1.0, 2.0, 4.0, 4.0, 3.0, 2.0, -1.0, -2.0],
    [4.0, 3.0, 2.0, 1.0, 2.0, 3.0, 4.0, 3.0, 2.0, 1.0],
];
pub const STEREO_WIDTH_MAX: f32 = 3.0;

fn close(a_: &[f32; 10], b: &[f32; 10]) -> bool {
    a_.iter().zip(b).all(|(x, y)| (x - y).abs() < 0.05)
}

/// The built-in preset matching `bands`, if any.
pub fn match_preset(bands: &[f32; 10]) -> Option<&'static str> {
    PRESETS
        .iter()
        .zip(PRESET_GAINS.iter())
        .find(|(_, g)| close(g, bands))
        .map(|(n, _)| *n)
}

pub struct State {
    pub eq: EqConfig,
    pub user_presets: Vec<UserPreset>,
    pub device: Option<DeviceEqProfile>,
    pub saving_device: bool,
    pub preset_name: String,
    pub saving_preset: bool,
}

impl Default for State {
    fn default() -> Self {
        Self {
            eq: EqConfig {
                bypass: false,
                bands: [0.0; 10],
                stereo: false,
                stereo_width: 1.0,
                dynamic: false,
            },
            user_presets: vec![],
            device: None,
            saving_device: false,
            preset_name: String::new(),
            saving_preset: false,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Msg {
    Loaded(Box<(Option<EqConfig>, Vec<UserPreset>, Option<DeviceEqProfile>)>),
    Eq(Result<EqConfig, String>),
    Device(Option<DeviceEqProfile>),
    Bypass,
    Preset(&'static str),
    Band(usize, f32),
    BandCommit(usize),
    Stereo(bool),
    Width(f32),
    WidthCommit,
    Dynamic(bool),
    LoadUser(i64),
    DeleteUser(i64),
    UserDeleted(i64, Result<(), String>),
    PresetName(String),
    SaveUser,
    UserSaved(Result<UserPreset, String>),
    SaveDevice,
    DeviceSaved(Result<DeviceEqProfile, String>),
    DeleteDevice,
}

fn em(m: Msg) -> Message {
    msg(SMsg::Eq(m))
}

pub fn load(app: &mut Halo) -> Task<Message> {
    bg(
        &app.backend,
        |b| {
            (
                eq::get_eq(b.db()).ok(),
                eq::list_user_presets(b.db()).unwrap_or_default(),
                eq::get_current_device(b.db()).ok(),
            )
        },
        |r| em(Msg::Loaded(Box::new(r))),
    )
}

/// `eq-state-changed` (output device switched and its profile applied).
pub fn on_changed(app: &mut Halo, cfg: EqConfig) -> Task<Message> {
    app.pages.settings.eq.eq = cfg;
    refresh_device(&app.backend)
}

fn refresh_device(b: &Backend) -> Task<Message> {
    bg(b, |b| eq::get_current_device(b.db()).ok(), |d| em(Msg::Device(d)))
}

pub fn update(app: &mut Halo, m: Msg) -> Task<Message> {
    let st = &mut app.pages.settings.eq;
    let b = &app.backend;
    match m {
        Msg::Loaded(r) => {
            let (cfg, presets, dev) = *r;
            if let Some(c) = cfg {
                st.eq = c;
            }
            st.user_presets = presets;
            st.device = dev;
            Task::none()
        }
        Msg::Eq(r) => match r {
            Ok(c) => {
                st.eq = c;
                Task::none()
            }
            Err(e) => Task::done(err(e)),
        },
        Msg::Device(d) => {
            st.device = d;
            Task::none()
        }
        Msg::Bypass => {
            let next = !st.eq.bypass;
            st.eq.bypass = next;
            bg(b, move |b| eq::set_eq_bypass(b.db(), b.player(), next), done)
        }
        Msg::Preset(p) => bg(
            b,
            move |b| eq::set_eq_preset(b.db(), b.player(), p.to_string()),
            |r| em(Msg::Eq(r)),
        ),
        Msg::Band(i, g) => {
            st.eq.bands[i] = g;
            Task::none()
        }
        Msg::BandCommit(i) => {
            let g = st.eq.bands[i];
            bg(b, move |b| eq::set_eq_band(b.db(), b.player(), i, g), done)
        }
        Msg::Stereo(on) => {
            st.eq.stereo = on;
            let w = st.eq.stereo_width;
            bg(b, move |b| eq::set_eq_stereo(b.db(), b.player(), on, w), done)
        }
        Msg::Width(w) => {
            st.eq.stereo_width = w;
            Task::none()
        }
        Msg::WidthCommit => {
            let (on, w) = (st.eq.stereo, st.eq.stereo_width);
            bg(b, move |b| eq::set_eq_stereo(b.db(), b.player(), on, w), done)
        }
        Msg::Dynamic(on) => {
            st.eq.dynamic = on;
            bg(b, move |b| eq::set_eq_dynamic(b.db(), b.player(), on), done)
        }
        Msg::LoadUser(id) => bg(
            b,
            move |b| eq::load_user_preset(b.db(), b.player(), id),
            |r| em(Msg::Eq(r)),
        ),
        Msg::DeleteUser(id) => bg(
            b,
            move |b| eq::delete_user_preset(b.db(), id),
            move |r| em(Msg::UserDeleted(id, r)),
        ),
        Msg::UserDeleted(id, r) => match r {
            Ok(()) => {
                st.user_presets.retain(|p| p.id != id);
                Task::none()
            }
            Err(e) => Task::done(err(e)),
        },
        Msg::PresetName(s) => {
            st.preset_name = s;
            Task::none()
        }
        Msg::SaveUser => {
            let name = st.preset_name.trim().to_string();
            if name.is_empty() || st.saving_preset {
                return Task::none();
            }
            st.saving_preset = true;
            bg(
                b,
                move |b| eq::save_user_preset(b.db(), b.player(), name),
                |r| em(Msg::UserSaved(r)),
            )
        }
        Msg::UserSaved(r) => {
            st.saving_preset = false;
            match r {
                Ok(p) => {
                    if let Some(x) = st.user_presets.iter_mut().find(|x| x.id == p.id) {
                        *x = p;
                    } else {
                        st.user_presets.push(p);
                        st.user_presets.sort_by_key(|p| p.name.to_lowercase());
                    }
                    st.preset_name.clear();
                    Task::none()
                }
                Err(e) => Task::done(err(e)),
            }
        }
        Msg::SaveDevice => {
            st.saving_device = true;
            bg(
                b,
                |b| eq::save_device_eq_profile(b.db(), b.player()),
                |r| em(Msg::DeviceSaved(r)),
            )
        }
        Msg::DeviceSaved(r) => {
            st.saving_device = false;
            match r {
                Ok(d) => {
                    st.device = Some(d);
                    Task::none()
                }
                Err(e) => Task::done(err(e)),
            }
        }
        Msg::DeleteDevice => {
            let Some(d) = &st.device else { return Task::none() };
            let name = d.device_name.clone();
            let b2 = b.clone();
            Task::batch([
                bg(b, move |b| eq::delete_device_eq_profile(b.db(), name), done),
                refresh_device(&b2),
            ])
        }
    }
}

fn fmt_gain(g: f32) -> String {
    if g == 0.0 {
        "0".into()
    } else if g > 0.0 {
        format!("+{g:.1}")
    } else {
        format!("{g:.1}")
    }
}

pub fn view(app: &Halo) -> Element<'_, Message> {
    let k = t();
    let st = &app.pages.settings.eq;
    let cfg = &st.eq;
    let active_preset = match_preset(&cfg.bands);
    let is_flat = cfg.bands.iter().all(|g| *g == 0.0);
    let active_user = st
        .user_presets
        .iter()
        .find(|p| close(&p.bands, &cfg.bands))
        .map(|p| p.id);

    // ── Header with bypass toggle ─────────────────────────────────────────────
    let header = column![
        row![
            column![
                text("Equalizer").size(theme::TEXT_BASE).font(theme::SEMIBOLD),
                row![
                    ui::muted("10-band graphic equalizer. ", theme::TEXT_SM),
                    text(format!("Current: {}", active_preset.unwrap_or("Custom")))
                        .size(theme::TEXT_SM)
                        .color(a(k.foreground, 0.7)),
                ],
            ]
            .spacing(2)
            .width(Length::Fill),
            row![
                ui::muted(if cfg.bypass { "Bypassed" } else { "Active" }, theme::TEXT_SM),
                toggle(!cfg.bypass, em(Msg::Bypass)),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        ]
        .align_y(Alignment::Center),
        ui::vspace(10.0),
        hline(),
    ]
    .padding(Padding::ZERO.bottom(24.0));

    let mut col = column![header].spacing(0);

    // ── Device profile ────────────────────────────────────────────────────────
    if let Some(d) = &st.device {
        col = col.push(
            container(
                row![
                    ui::muted("Device", theme::TEXT_SM),
                    crate::widgets::line(d.device_name.as_str())
                        .size(theme::TEXT_SM)
                        .font(theme::MEDIUM)
                        .width(Length::Fill),
                    outline_btn(
                        None,
                        if st.saving_device { "Saving…" } else { "Save profile" },
                        (!st.saving_device).then(|| em(Msg::SaveDevice)),
                    ),
                    crate::ui::tip(
                        button(ui::icon_box(Icon::X, 14.0, 28.0, 28.0))
                            .padding(0)
                            .style(theme::btn_text(k.muted_fg, k.destructive))
                            .on_press(em(Msg::DeleteDevice)),
                        "Remove device profile",
                        iced::widget::tooltip::Position::Top,
                    ),
                ]
                .spacing(12)
                .align_y(Alignment::Center),
            )
            .padding(Padding::from([8.0, 12.0]))
            .style(|_| iced::widget::container::Style {
                background: Some(a(t().muted, 0.3).into()),
                border: theme::border(t().border, 1.0, theme::R_MD),
                ..Default::default()
            }),
        );
        col = col.push(ui::vspace(20.0));
    }

    // ── Built-in presets ──────────────────────────────────────────────────────
    let mut chips = row![].spacing(6);
    for p in PRESETS {
        let active = Some(p) == active_preset;
        chips = chips.push(
            button(text(p).size(theme::TEXT_SM))
                .padding(Padding::from([4.0, 12.0]))
                .style(move |_, s| {
                    let k = t();
                    let hovered = matches!(s, button::Status::Hovered);
                    if active {
                        button::Style {
                            background: Some(k.primary.into()),
                            text_color: k.primary_fg,
                            border: theme::border(k.primary, 1.0, theme::R_FULL),
                            ..Default::default()
                        }
                    } else {
                        button::Style {
                            background: Some(a(k.muted, 0.4).into()),
                            text_color: if hovered { k.foreground } else { k.muted_fg },
                            border: theme::border(
                                if hovered { a(k.foreground, 0.3) } else { k.border },
                                1.0,
                                theme::R_FULL,
                            ),
                            ..Default::default()
                        }
                    }
                })
                .on_press(em(Msg::Preset(p))),
        );
    }
    col = col.push(
        column![
            row![
                label("Presets"),
                ui::fill(),
                button(text("Reset to flat").size(theme::TEXT_XS))
                    .padding(0)
                    .style(theme::btn_icon)
                    .on_press_maybe((!is_flat).then(|| em(Msg::Preset("Flat")))),
            ]
            .align_y(Alignment::Center),
            chips.wrap().vertical_spacing(6),
        ]
        .spacing(8),
    );
    col = col.push(ui::vspace(20.0));

    // ── User presets ──────────────────────────────────────────────────────────
    let mut user = column![label("My Presets")].spacing(8);
    if !st.user_presets.is_empty() {
        let mut r = row![].spacing(6);
        for p in &st.user_presets {
            let active = Some(p.id) == active_user;
            let fg = if active { k.primary_fg } else { k.muted_fg };
            r = r.push(
                container(
                    row![
                        button(text(p.name.as_str()).size(theme::TEXT_SM))
                            .padding(0)
                            .style(theme::btn_text(fg, if active { k.primary_fg } else { k.foreground }))
                            .on_press(em(Msg::LoadUser(p.id))),
                        button(icon(Icon::X, 12.0))
                            .padding(Padding::ZERO.left(4.0))
                            .style(theme::btn_text(
                                if active { a(k.primary_fg, 0.8) } else { k.muted_fg },
                                if active { k.primary_fg } else { k.destructive },
                            ))
                            .on_press(em(Msg::DeleteUser(p.id))),
                    ]
                    .spacing(4)
                    .align_y(Alignment::Center),
                )
                .padding(Padding::from([2.0, 6.0]).left(12.0))
                .style(move |_| {
                    let k = t();
                    iced::widget::container::Style {
                        background: Some(if active { k.primary } else { a(k.muted, 0.4) }.into()),
                        border: theme::border(if active { k.primary } else { k.border }, 1.0, theme::R_FULL),
                        ..Default::default()
                    }
                }),
            );
        }
        user = user.push(r.wrap().vertical_spacing(6));
    }
    let can_save = !st.preset_name.trim().is_empty() && !st.saving_preset;
    user = user.push(
        row![
            text_input("Save current curve as…", &st.preset_name)
                .on_input(|s| em(Msg::PresetName(s)))
                .on_submit(em(Msg::SaveUser))
                .size(theme::TEXT_SM)
                .padding(Padding::from([4.0, 12.0]))
                .width(176)
                .style(theme::input),
            outline_btn(None, "Save", can_save.then(|| em(Msg::SaveUser))),
        ]
        .spacing(6)
        .align_y(Alignment::Center),
    );
    col = col.push(user).push(ui::vspace(24.0));

    // ── Band sliders with dB axis ─────────────────────────────────────────────
    let axis = column![
        Space::new().height(16),
        column![
            text("+12").size(theme::TEXT_10).color(k.muted_fg),
            Space::new().height(Length::Fill),
            text("+6").size(theme::TEXT_10).color(k.muted_fg),
            Space::new().height(Length::Fill),
            text("0").size(theme::TEXT_10).color(k.muted_fg),
            Space::new().height(Length::Fill),
            text("−6").size(theme::TEXT_10).color(k.muted_fg),
            Space::new().height(Length::Fill),
            text("−12").size(theme::TEXT_10).color(k.muted_fg),
        ]
        .height(120)
        .align_x(Alignment::End),
    ]
    .width(32)
    .align_x(Alignment::End);
    let mut bands = row![axis].spacing(12);
    let bypass = cfg.bypass;
    for (i, &g) in cfg.bands.iter().enumerate() {
        // Bypassed: 50% opacity and inert (pointer-events-none).
        let vs: Element<'_, Message> = if bypass {
            vertical_slider(-12.0..=12.0_f32, g, |_| Message::Noop)
                .height(120)
                .style(|th, st| {
                    let mut s = theme::slider_style(th, st);
                    s.rail.backgrounds = (
                        a(t().primary, 0.5).into(),
                        a(t().muted, 0.5).into(),
                    );
                    s.handle.background = a(theme::WHITE, 0.5).into();
                    s
                })
                .into()
        } else {
            vertical_slider(-12.0..=12.0_f32, g, move |v| em(Msg::Band(i, v)))
                .step(0.5_f32)
                .height(120)
                .style(theme::slider_style)
                .on_release(em(Msg::BandCommit(i)))
                .into()
        };
        bands = bands.push(
            column![
                text(fmt_gain(g))
                    .size(theme::TEXT_XS)
                    .color(if g == 0.0 { k.muted_fg } else { k.foreground })
                    .height(16),
                vs,
                text(BAND_LABELS[i]).size(theme::TEXT_XS).color(k.muted_fg),
            ]
            .spacing(4)
            .width(36)
            .align_x(Alignment::Center),
        );
    }
    col = col.push(bands);

    // ── Sound enhancements ───────────────────────────────────────────────────
    col = col
        .push(ui::vspace(24.0))
        .push(hline())
        .push(ui::vspace(24.0))
        .push(label("Sound enhancements"))
        .push(setting_row(
            "Stereo expander",
            "Widen the stereo image for a more spacious sound.",
            toggle(cfg.stereo, em(Msg::Stereo(!cfg.stereo))),
        ));
    if cfg.stereo {
        col = col.push(setting_row(
            "Width",
            "Higher values push the stereo field wider. Neutral is 1.0×.",
            row![
                slider(0.0..=STEREO_WIDTH_MAX, cfg.stereo_width, |w| em(Msg::Width(w)))
                    .step(0.1_f32)
                    .on_release(em(Msg::WidthCommit))
                    .width(Length::Fill)
                    .style(theme::slider_style),
                container(
                    text(format!("{:.1}×", cfg.stereo_width))
                        .size(theme::TEXT_SM)
                        .color(k.muted_fg)
                )
                .width(40)
                .align_x(Alignment::End),
            ]
            .spacing(12)
            .align_y(Alignment::Center)
            .width(224),
        ));
    }
    col = col.push(setting_row(
        "Dynamic EQ",
        "Eases the EQ effect on loud passages and applies it fully when quiet, so boosts stay clean.",
        toggle(cfg.dynamic, em(Msg::Dynamic(!cfg.dynamic))),
    ));
    col.into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_builtin_presets() {
        assert_eq!(match_preset(&[0.0; 10]), Some("Flat"));
        assert_eq!(match_preset(&PRESET_GAINS[6]), Some("Bass Boost"));
        let mut custom = [0.0; 10];
        custom[3] = 1.0;
        assert_eq!(match_preset(&custom), None);
    }
}
