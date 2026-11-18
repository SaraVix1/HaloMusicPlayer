//! Small reusable view builders shared by pages and components (the iced
//! counterparts of `src/components/ui/*` and `album-art.tsx`).

#![allow(dead_code)]

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::LazyLock;

use iced::widget::{
    button, center, container, image, row, space, svg, text, tooltip, Space,
};
use iced::{Alignment, Color, ContentFit, Element, Length, Padding};

use crate::icons::{icon, Icon};
use crate::theme::{self, a, t};
use crate::widgets::{line, surface};

// ── Text ──────────────────────────────────────────────────────────────────────

/// Plain text at a size/colour in Geist.
pub fn txt<'a>(s: impl text::IntoFragment<'a>, size: f32) -> text::Text<'a> {
    text(s).size(size).font(theme::SANS)
}

pub fn muted<'a>(s: impl text::IntoFragment<'a>, size: f32) -> text::Text<'a> {
    text(s).size(size).font(theme::SANS).color(t().muted_fg)
}

pub fn h1<'a>(s: impl text::IntoFragment<'a>) -> text::Text<'a> {
    text(s).size(theme::TEXT_2XL).font(theme::SEMIBOLD)
}

// ── Layout bits ───────────────────────────────────────────────────────────────

pub fn hspace(w: f32) -> Space {
    space().width(Length::Fixed(w))
}

pub fn vspace(h: f32) -> Space {
    space().height(Length::Fixed(h))
}

pub fn fill() -> Space {
    space().width(Length::Fill)
}

/// 1px horizontal divider in the border colour.
pub fn hline<'a, M: 'a>() -> Element<'a, M> {
    container(space())
        .width(Length::Fill)
        .height(1)
        .style(theme::divider)
        .into()
}

pub fn hline_alpha<'a, M: 'a>(alpha: f32) -> Element<'a, M> {
    container(space())
        .width(Length::Fill)
        .height(1)
        .style(theme::divider_alpha(alpha))
        .into()
}

pub fn vline<'a, M: 'a>() -> Element<'a, M> {
    container(space())
        .width(1)
        .height(Length::Fill)
        .style(theme::divider)
        .into()
}

// ── Tooltip ───────────────────────────────────────────────────────────────────

/// shadcn tooltip: foreground bubble with background-coloured xs text,
/// opened after 400ms.
pub fn tip<'a, M: 'a>(
    content: impl Into<Element<'a, M>>,
    label: impl Into<String>,
    position: tooltip::Position,
) -> Element<'a, M> {
    let bubble = container(text(label.into()).size(theme::TEXT_XS).font(theme::SANS))
        .padding(Padding::from([6.0, 12.0]))
        .style(theme::tooltip);
    tooltip(content, bubble, position)
        .gap(6)
        .delay(std::time::Duration::from_millis(400))
        .into()
}

// ── Icons & buttons ───────────────────────────────────────────────────────────

/// Centre an icon in a fixed box.
pub fn icon_box<'a, M: 'a>(i: Icon, size: f32, w: f32, h: f32) -> Element<'a, M> {
    center(icon(i, size)).width(w).height(h).into()
}

/// Square icon button with a style function.
pub fn icon_button<'a, M: Clone + 'a>(
    i: Icon,
    size: f32,
    w: f32,
    h: f32,
    style: impl Fn(&iced::Theme, button::Status) -> button::Style + 'a,
    on_press: Option<M>,
) -> button::Button<'a, M> {
    button(icon_box(i, size, w, h))
        .padding(0)
        .style(style)
        .on_press_maybe(on_press)
}

/// Labelled button: optional leading icon + text, shadcn sizing.
pub fn label_button<'a, M: Clone + 'a>(
    leading: Option<Icon>,
    label: impl text::IntoFragment<'a>,
    size: f32,
    height: f32,
    pad_x: f32,
    style: impl Fn(&iced::Theme, button::Status) -> button::Style + 'a,
    on_press: Option<M>,
) -> button::Button<'a, M> {
    let mut content = row![].spacing(6).align_y(Alignment::Center);
    if let Some(i) = leading {
        content = content.push(icon(i, size - 1.0));
    }
    content = content.push(text(label).size(size).font(theme::MEDIUM));
    button(container(content).height(height).align_y(Alignment::Center))
        .padding(Padding::from([0.0, pad_x]))
        .style(style)
        .on_press_maybe(on_press)
}

static SVG_HEART_FILLED: LazyLock<svg::Handle> = LazyLock::new(|| {
    svg::Handle::from_memory(include_bytes!("../assets/lucide/heart-filled.svg").as_slice())
});
static SVG_PLAY_FILLED: LazyLock<svg::Handle> = LazyLock::new(|| {
    svg::Handle::from_memory(include_bytes!("../assets/lucide/play-filled.svg").as_slice())
});
static SVG_PAUSE_FILLED: LazyLock<svg::Handle> = LazyLock::new(|| {
    svg::Handle::from_memory(include_bytes!("../assets/lucide/pause-filled.svg").as_slice())
});
static SVG_STAR: LazyLock<svg::Handle> = LazyLock::new(|| {
    svg::Handle::from_memory(include_bytes!("../assets/lucide/star.svg").as_slice())
});
static SVG_STAR_FILLED: LazyLock<svg::Handle> = LazyLock::new(|| {
    svg::Handle::from_memory(include_bytes!("../assets/lucide/star-filled.svg").as_slice())
});

#[derive(Debug, Clone, Copy)]
pub enum Filled {
    Heart,
    Play,
    Pause,
    Star,
}

/// A filled Lucide glyph (`fill="currentColor"`) tinted with `color`.
pub fn filled<'a>(which: Filled, size: f32, color: Color) -> svg::Svg<'a> {
    let handle = match which {
        Filled::Heart => SVG_HEART_FILLED.clone(),
        Filled::Play => SVG_PLAY_FILLED.clone(),
        Filled::Pause => SVG_PAUSE_FILLED.clone(),
        Filled::Star => SVG_STAR_FILLED.clone(),
    };
    svg(handle)
        .width(size)
        .height(size)
        .style(move |_, _| svg::Style { color: Some(color) })
}

// ── Star rating ───────────────────────────────────────────────────────────────

/// Five-star rating (`star-rating.tsx`). `hover` is the star currently under
/// the cursor for this instance (kept by the caller); clicking the current
/// value clears it to 0.
pub fn stars<'a, M: Clone + 'a>(
    value: i64,
    size: f32,
    readonly: bool,
    hover: Option<u8>,
    on_hover: impl Fn(Option<u8>) -> M + 'a,
    on_change: impl Fn(i64) -> M + 'a,
) -> Element<'a, M> {
    let displayed = hover.map(|h| h as i64).unwrap_or(value);
    let mut r = row![].spacing(2).align_y(Alignment::Center);
    for star in 1..=5i64 {
        let is_filled = star <= displayed;
        let hovered_this = hover == Some(star as u8);
        let s = if hovered_this && !readonly { size * 1.1 } else { size };
        let color = if is_filled {
            theme::AMBER_400
        } else {
            a(t().muted_fg, 0.25)
        };
        let glyph: Element<'a, M> = if is_filled {
            filled(Filled::Star, s, color).into()
        } else {
            svg(SVG_STAR.clone())
                .width(s)
                .height(s)
                .style(move |_, status| svg::Style {
                    color: Some(if matches!(status, svg::Status::Hovered) && !readonly {
                        theme::AMBER_300
                    } else {
                        color
                    }),
                })
                .into()
        };
        let cell = center(glyph).width(size * 1.1).height(size * 1.1);
        if readonly {
            r = r.push(cell);
        } else {
            let next = if star == value { 0 } else { star };
            r = r.push(
                surface(cell)
                    .on_press(on_change(next))
                    .on_enter(on_hover(Some(star as u8)))
                    .interaction(iced::mouse::Interaction::Pointer),
            );
        }
    }
    if readonly {
        r.into()
    } else {
        surface(r).on_exit(on_hover(None)).into()
    }
}

// ── Album art ─────────────────────────────────────────────────────────────────

thread_local! {
    static EXISTS: RefCell<HashMap<String, bool>> = RefCell::new(HashMap::new());
}

fn file_exists(path: &str) -> bool {
    EXISTS.with(|m| {
        *m.borrow_mut()
            .entry(path.to_string())
            .or_insert_with(|| std::path::Path::new(path).is_file())
    })
}

/// Forget cached existence checks (after a rescan / art edit).
pub fn invalidate_art_cache() {
    EXISTS.with(|m| m.borrow_mut().clear());
}

/// Album art square with the muted Disc fallback (`album-art.tsx`).
pub fn art<'a, M: 'a>(path: Option<&str>, size: f32, radius: f32) -> Element<'a, M> {
    art_sized(path, Length::Fixed(size), Length::Fixed(size), size, radius)
}

/// Album art filling its width (cards), square via `size_hint` height.
pub fn art_sized<'a, M: 'a>(
    path: Option<&str>,
    width: Length,
    height: Length,
    icon_basis: f32,
    radius: f32,
) -> Element<'a, M> {
    match path.filter(|p| !p.is_empty() && file_exists(p)) {
        Some(p) => image(image::Handle::from_path(p))
            .width(width)
            .height(height)
            .content_fit(ContentFit::Cover)
            .border_radius(radius)
            .into(),
        None => art_fallback(width, height, icon_basis, radius),
    }
}

pub fn art_fallback<'a, M: 'a>(
    width: Length,
    height: Length,
    icon_basis: f32,
    radius: f32,
) -> Element<'a, M> {
    let k = t();
    center(icon(Icon::Disc, (icon_basis * 0.5).max(12.0)).color(a(k.muted_fg, 0.4)))
        .width(width)
        .height(height)
        .style(move |_| container::Style {
            background: Some(t().muted.into()),
            border: theme::no_border(radius),
            ..container::Style::default()
        })
        .into()
}

// ── Toggle switch ─────────────────────────────────────────────────────────────

/// Settings toggle: 36×20 track (primary / muted), 16px white knob.
pub fn toggle<'a, M: Clone + 'a>(on: bool, msg: M) -> Element<'a, M> {
    let knob = container(space())
        .width(16)
        .height(16)
        .style(|_| container::Style {
            background: Some(theme::WHITE.into()),
            border: theme::no_border(theme::R_FULL),
            shadow: theme::shadow_sm(),
            ..container::Style::default()
        });
    let inner = container(knob)
        .width(36)
        .height(20)
        .padding(2)
        .align_x(if on { Alignment::End } else { Alignment::Start })
        .align_y(Alignment::Center);
    button(inner)
        .padding(0)
        .style(move |_, _| button::Style {
            background: Some(if on { t().primary } else { t().muted }.into()),
            border: theme::no_border(theme::R_FULL),
            ..button::Style::default()
        })
        .on_press(msg)
        .into()
}

/// Small checkbox-like toggle with a label (sleep timer "Fade out").
pub fn check<'a, M: Clone + 'a>(checked: bool, label: &'a str, msg: M) -> Element<'a, M> {
    let k = t();
    let bx: Element<'a, M> = if checked {
        center(icon(Icon::Check, 10.0).color(k.primary_fg))
            .width(13)
            .height(13)
            .style(|_| container::Style {
                background: Some(t().primary.into()),
                border: theme::no_border(3.0),
                ..container::Style::default()
            })
            .into()
    } else {
        container(space())
            .width(13)
            .height(13)
            .style(|_| container::Style {
                border: theme::border(t().ring, 1.0, 3.0),
                ..container::Style::default()
            })
            .into()
    };
    button(
        row![bx, text(label).size(theme::TEXT_XS).color(k.muted_fg)]
            .spacing(8)
            .align_y(Alignment::Center),
    )
    .padding(0)
    .style(theme::btn_bare)
    .on_press(msg)
    .into()
}

/// Ellipsized single-line label (`truncate`).
pub fn trunc<'a>(s: impl Into<std::borrow::Cow<'a, str>>, size: f32) -> crate::widgets::textline::TextLine<'a> {
    line(s).size(size).width(Length::Fill)
}

/// Pill-shaped small badge (`rounded-full bg-muted px-2 py-0.5 text-[10px]`).
pub fn badge<'a, M: 'a>(label: impl text::IntoFragment<'a>) -> Element<'a, M> {
    container(text(label).size(theme::TEXT_10).font(theme::MEDIUM))
        .padding(Padding::from([2.0, 8.0]))
        .style(|_| container::Style {
            background: Some(t().muted.into()),
            text_color: Some(t().muted_fg),
            border: theme::no_border(theme::R_FULL),
            ..container::Style::default()
        })
        .into()
}

/// Keyboard key hint (`kbd`).
pub fn kbd<'a, M: 'a>(label: &'a str) -> Element<'a, M> {
    container(text(label).size(theme::TEXT_10).font(theme::MONO))
        .padding(Padding::from([1.0, 6.0]))
        .style(|_| container::Style {
            text_color: Some(t().muted_fg),
            border: theme::border(t().border, 1.0, theme::R_XS),
            background: Some(t().muted.into()),
            ..container::Style::default()
        })
        .into()
}

// ── Dialog ────────────────────────────────────────────────────────────────────

/// shadcn `Dialog`: `bg-black/10` backdrop (click closes), centred popup
/// (`max-w-sm` = 384px by default, `p-4 gap-4 rounded-xl`), title and a
/// ghost close button in the top-right corner.
pub fn modal<'a, M: Clone + 'a>(
    title: &'a str,
    body: impl Into<Element<'a, M>>,
    footer: Option<Element<'a, M>>,
    width: f32,
    on_close: M,
) -> Element<'a, M> {
    use iced::widget::{column, mouse_area, opaque, stack};
    let close = button(icon_box(Icon::X, 16.0, 28.0, 28.0))
        .padding(0)
        .style(theme::btn_ghost)
        .on_press(on_close.clone());
    let head = row![
        text(title).size(theme::TEXT_BASE).font(theme::MEDIUM).width(Length::Fill),
        close,
    ]
    .align_y(Alignment::Center);
    let mut card = column![
        container(column![head, body.into()].spacing(16)).padding(Padding::from([12.0, 16.0]).bottom(16.0)),
    ];
    if let Some(f) = footer {
        card = card.push(
            container(f)
                .width(Length::Fill)
                .padding(16)
                .style(theme::dialog_footer),
        );
    }
    let card = container(card).width(width).style(theme::dialog);
    let backdrop = mouse_area(
        container(space())
            .width(Length::Fill)
            .height(Length::Fill)
            .style(theme::backdrop(0.10)),
    )
    .on_press(on_close);
    stack![
        opaque(backdrop),
        center(opaque(card)).padding(16),
    ]
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

/// Small text button used in dialogs / inline editors: primary fill or
/// bordered outline, `px-3 py-1 text-xs rounded-md`.
pub fn small_button<'a, M: Clone + 'a>(
    label: impl text::IntoFragment<'a>,
    primary: bool,
    on_press: Option<M>,
) -> Element<'a, M> {
    button(text(label).size(theme::TEXT_XS))
        .padding(Padding::from([4.0, 12.0]))
        .style(move |th, st| {
            let mut s = if primary {
                theme::btn_primary(th, st)
            } else {
                theme::btn_outline(th, st)
            };
            s.border.radius = theme::radius(theme::R_MD);
            s
        })
        .on_press_maybe(on_press)
        .into()
}

// ── Spinners ──────────────────────────────────────────────────────────────────

static SVG_REFRESH: LazyLock<svg::Handle> = LazyLock::new(|| {
    svg::Handle::from_memory(include_bytes!("../assets/lucide/refresh-cw.svg").as_slice())
});
static SVG_LOADER: LazyLock<svg::Handle> = LazyLock::new(|| {
    svg::Handle::from_memory(include_bytes!("../assets/lucide/loader-circle.svg").as_slice())
});

#[derive(Debug, Clone, Copy)]
pub enum Spinner {
    Refresh,
    Loader,
}

/// `animate-spin` icon: `angle` (radians) is advanced by the caller's
/// animation tick. `color` tints the stroke.
pub fn spin<'a>(which: Spinner, size: f32, angle: f32, color: Color) -> svg::Svg<'a> {
    let handle = match which {
        Spinner::Refresh => SVG_REFRESH.clone(),
        Spinner::Loader => SVG_LOADER.clone(),
    };
    svg(handle)
        .width(size)
        .height(size)
        .rotation(iced::Rotation::Floating(iced::Radians(angle)))
        .style(move |_, _| svg::Style { color: Some(color) })
}

/// Current spinner angle derived from wall-clock time (1 turn per second).
pub fn spin_angle() -> f32 {
    let ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() % 1000)
        .unwrap_or(0);
    ms as f32 / 1000.0 * std::f32::consts::TAU
}

/// Horizontal progress fill: `fraction` (0..=1) of the width in `fill`
/// colour (no zero-width `FillPortion` artefacts at 0 / 100%).
pub fn progress<'a, M: 'a>(fraction: f32, height: f32, fill: Color, radius: f32) -> Element<'a, M> {
    let f = (fraction.clamp(0.0, 1.0) * 1000.0).round() as u16;
    let bar = move || {
        container(space())
            .width(Length::Fill)
            .height(height)
            .style(move |_| container::Style {
                background: Some(fill.into()),
                border: theme::no_border(radius),
                ..container::Style::default()
            })
    };
    match f {
        0 => space().width(Length::Fill).height(height).into(),
        1000 => bar().into(),
        f => row![
            container(bar()).width(Length::FillPortion(f)),
            space().width(Length::FillPortion(1000 - f)),
        ]
        .height(height)
        .into(),
    }
}
