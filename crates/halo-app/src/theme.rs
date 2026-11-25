//! Design tokens and widget styles — a port of the shadcn/ui "neutral" theme
//! from the old `src/index.css` (Tailwind v4 palette, oklch values converted to
//! sRGB) plus the per-component class lists used across the React UI.
//!
//! Style closures receive iced's `&Theme`, but read colours from [`t()`], the
//! active token set, so every widget follows light/dark switches instantly.

#![allow(dead_code)]

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use iced::border::Radius;
use iced::font::Weight;
use iced::widget::{button, container, pick_list, scrollable, slider, text_input};
use iced::{Background, Border, Color, Font, Shadow, Theme, Vector};

// ── Fonts ─────────────────────────────────────────────────────────────────────

pub const SANS: Font = Font::with_name("Geist");
pub const MEDIUM: Font = Font {
    weight: Weight::Medium,
    ..SANS
};
pub const SEMIBOLD: Font = Font {
    weight: Weight::Semibold,
    ..SANS
};
pub const BOLD: Font = Font {
    weight: Weight::Bold,
    ..SANS
};
pub const MONO: Font = Font::with_name("Geist Mono");

pub const FONT_BYTES: &[&[u8]] = &[
    include_bytes!("../assets/fonts/Geist-Regular.ttf"),
    include_bytes!("../assets/fonts/Geist-Medium.ttf"),
    include_bytes!("../assets/fonts/Geist-SemiBold.ttf"),
    include_bytes!("../assets/fonts/Geist-Bold.ttf"),
    include_bytes!("../assets/fonts/GeistMono-Regular.ttf"),
    include_bytes!("../assets/fonts/GeistMono-Medium.ttf"),
    include_bytes!("../assets/fonts/lucide.ttf"),
];

// ── Type scale: three sizes ──────────────────────────────────────────────────
//
// Hierarchy comes from weight (regular / MEDIUM / SEMIBOLD), not more sizes.

/// Page and section titles (semibold).
pub const TITLE: f32 = 20.0;
/// Most text.
pub const BODY: f32 = 14.0;
/// Secondary text: artists, counts, hints, badges.
pub const SMALL: f32 = 12.0;
/// The one exception: the song title in full-screen now playing.
pub const DISPLAY: f32 = 28.0;

// ── Radii ────────────────────────────────────────────────────────────────────
//
// Controls (buttons, inputs, list items) 8 px; surfaces (cards, covers,
// popovers, dialogs, panels) 12 px; 4–6 px only for tiny things (key hints,
// chips, small thumbnails).

pub const R_XS: f32 = 4.0;
pub const R_SM: f32 = 6.0;
/// Controls.
pub const R_MD: f32 = 8.0;
/// Surfaces.
pub const R_LG: f32 = 12.0;
pub const R_XL: f32 = 12.0;
pub const R_FULL: f32 = 999.0;

// ── Colour helpers ────────────────────────────────────────────────────────────

pub const fn hex(rgb: u32) -> Color {
    Color::from_rgb8(
        ((rgb >> 16) & 0xff) as u8,
        ((rgb >> 8) & 0xff) as u8,
        (rgb & 0xff) as u8,
    )
}

/// `color / alpha` (Tailwind opacity modifier).
pub fn a(c: Color, alpha: f32) -> Color {
    Color { a: c.a * alpha, ..c }
}

pub const WHITE: Color = Color::WHITE;
pub const BLACK: Color = Color::BLACK;
pub const AMBER_400: Color = hex(0xffb900);
pub const AMBER_300: Color = hex(0xffd230);
pub const RED_500: Color = hex(0xfb2c36);
pub const RED_400: Color = hex(0xff6467);
pub const RED_900: Color = hex(0x82181a);

/// Gradient pairs (from, to) for composer / genre cards — library.tsx GRADIENTS
/// (`from-X-500/30 to-Y-500/20`).
pub const GRADIENTS: [(Color, Color); 7] = [
    (hex(0xff2056), hex(0xff6900)), // rose → orange
    (hex(0xfe9a00), hex(0xf0b100)), // amber → yellow
    (hex(0x00bc7d), hex(0x00bba7)), // emerald → teal
    (hex(0x00a6f4), hex(0x615fff)), // sky → indigo
    (hex(0x8e51ff), hex(0xe12afb)), // violet → fuchsia
    (hex(0xf6339a), hex(0xff2056)), // pink → rose
    (hex(0x00b8db), hex(0x2b7fff)), // cyan → blue
];

/// `h = (h*31 + charCode) >>> 0` (unsigned 32-bit) mod 7 — same card colour
/// for the same name as the React UI.
pub fn gradient_for(name: &str) -> (Color, Color) {
    let mut h: u32 = 0;
    for u in name.encode_utf16() {
        h = h.wrapping_mul(31).wrapping_add(u as u32);
    }
    GRADIENTS[(h as usize) % GRADIENTS.len()]
}

// ── Tokens ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
pub struct Tokens {
    pub dark: bool,
    pub background: Color,
    pub foreground: Color,
    pub card: Color,
    pub popover: Color,
    pub popover_fg: Color,
    pub primary: Color,
    pub primary_fg: Color,
    pub secondary: Color,
    pub secondary_fg: Color,
    pub muted: Color,
    pub muted_fg: Color,
    pub accent: Color,
    pub accent_fg: Color,
    pub destructive: Color,
    pub border: Color,
    pub input: Color,
    pub ring: Color,
    pub sidebar: Color,
    pub sidebar_fg: Color,
    pub sidebar_accent: Color,
    pub sidebar_accent_fg: Color,
    pub sidebar_border: Color,
}

pub const LIGHT: Tokens = Tokens {
    dark: false,
    background: hex(0xffffff),
    foreground: hex(0x0a0a0a),
    card: hex(0xffffff),
    popover: hex(0xffffff),
    popover_fg: hex(0x0a0a0a),
    primary: hex(0x171717),
    primary_fg: hex(0xfafafa),
    secondary: hex(0xf5f5f5),
    secondary_fg: hex(0x171717),
    muted: hex(0xf5f5f5),
    muted_fg: hex(0x737373),
    accent: hex(0xf5f5f5),
    accent_fg: hex(0x171717),
    destructive: hex(0xe7000b),
    border: hex(0xececec),
    input: hex(0xe5e5e5),
    ring: hex(0xa1a1a1),
    sidebar: hex(0xfafafa),
    sidebar_fg: hex(0x0a0a0a),
    sidebar_accent: hex(0xf5f5f5),
    sidebar_accent_fg: hex(0x171717),
    sidebar_border: hex(0xececec),
};

pub const DARK: Tokens = Tokens {
    dark: true,
    background: hex(0x0a0a0a),
    foreground: hex(0xfafafa),
    card: hex(0x171717),
    popover: hex(0x171717),
    popover_fg: hex(0xfafafa),
    primary: hex(0xe5e5e5),
    primary_fg: hex(0x171717),
    secondary: hex(0x262626),
    secondary_fg: hex(0xfafafa),
    muted: hex(0x262626),
    muted_fg: hex(0xa1a1a1),
    accent: hex(0x262626),
    accent_fg: hex(0xfafafa),
    destructive: hex(0xff6467),
    border: Color {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 0.06,
    },
    input: Color {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 0.15,
    },
    ring: hex(0x737373),
    sidebar: hex(0x171717),
    sidebar_fg: hex(0xfafafa),
    sidebar_accent: hex(0x262626),
    sidebar_accent_fg: hex(0xfafafa),
    sidebar_border: Color {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 0.06,
    },
};

static IS_DARK: AtomicBool = AtomicBool::new(true);

pub fn set_dark(dark: bool) {
    IS_DARK.store(dark, Ordering::Relaxed);
}

pub fn is_dark() -> bool {
    IS_DARK.load(Ordering::Relaxed)
}

// ── Accent ────────────────────────────────────────────────────────────────────

/// Halo's own accent (the icon's blue): used when the current track has no
/// art, its art has no vivid colour, or "Accent from album art" is off.
pub const DEFAULT_ACCENT: Color = hex(0x3d9bde);

/// Current accent, packed `0xRRGGBB` (alpha is always 1).
static ACCENT: AtomicU32 = AtomicU32::new(0x3d9bde);

/// Set the accent (the theme's `primary`). Takes effect on the next redraw.
pub fn set_accent(c: Color) {
    let [r, g, b, _] = c.into_rgba8();
    ACCENT.store(u32::from_be_bytes([0, r, g, b]), Ordering::Relaxed);
}

pub fn accent() -> Color {
    let [_, r, g, b] = ACCENT.load(Ordering::Relaxed).to_be_bytes();
    Color::from_rgb8(r, g, b)
}

/// `c` with its lightness clamped so it reads well on the theme's
/// background: lighter on dark, darker on light. Hue and saturation kept.
pub fn readable(c: Color, dark: bool) -> Color {
    let (h, s, l) = to_hsl(c);
    let l = if dark { l.clamp(0.55, 0.72) } else { l.clamp(0.36, 0.48) };
    from_hsl(h, s, l)
}

/// Text colour for something filled with `c`: near-black on light fills,
/// white on dark ones (WCAG relative luminance).
pub fn on(c: Color) -> Color {
    let lin = |v: f32| if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) };
    let lum = 0.2126 * lin(c.r) + 0.7152 * lin(c.g) + 0.0722 * lin(c.b);
    // Contrast with white vs. with #0a0a0a; pick the higher.
    if (1.05 / (lum + 0.05)) >= ((lum + 0.05) / 0.053) {
        WHITE
    } else {
        hex(0x0a0a0a)
    }
}

pub fn to_hsl(c: Color) -> (f32, f32, f32) {
    let max = c.r.max(c.g).max(c.b);
    let min = c.r.min(c.g).min(c.b);
    let l = (max + min) / 2.0;
    let d = max - min;
    if d.abs() < f32::EPSILON {
        return (0.0, 0.0, l);
    }
    let s = d / (1.0 - (2.0 * l - 1.0).abs());
    let h = if max == c.r {
        ((c.g - c.b) / d).rem_euclid(6.0)
    } else if max == c.g {
        (c.b - c.r) / d + 2.0
    } else {
        (c.r - c.g) / d + 4.0
    } * 60.0;
    (h, s.clamp(0.0, 1.0), l)
}

pub fn from_hsl(h: f32, s: f32, l: f32) -> Color {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((h / 60.0).rem_euclid(2.0) - 1.0).abs());
    let m = l - c / 2.0;
    let (r, g, b) = match (h.rem_euclid(360.0) / 60.0) as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    Color::from_rgb(r + m, g + m, b + m)
}

/// The active token set: light or dark, with the accent as `primary`
/// (buttons, progress, sliders, toggles, active nav, focus ring).
pub fn t() -> Tokens {
    let mut k = if is_dark() { DARK } else { LIGHT };
    let acc = readable(accent(), k.dark);
    k.primary = acc;
    k.primary_fg = on(acc);
    k.ring = a(acc, 0.7);
    k
}

/// Blend a translucent colour over the page background (for places that
/// need an opaque colour, e.g. text on top of an overlay).
pub fn over_bg(c: Color) -> Color {
    let bg = t().background;
    Color {
        r: bg.r + (c.r - bg.r) * c.a,
        g: bg.g + (c.g - bg.g) * c.a,
        b: bg.b + (c.b - bg.b) * c.a,
        a: 1.0,
    }
}

/// iced theme matching the tokens (drives defaults such as the window
/// background and the text colour inherited by widgets).
pub fn iced_theme() -> Theme {
    let k = t();
    Theme::custom(
        if k.dark { "Halo Dark" } else { "Halo Light" },
        iced::theme::Palette {
            background: k.background,
            text: k.foreground,
            primary: k.primary,
            success: hex(0x00bc7d),
            warning: AMBER_400,
            danger: k.destructive,
        },
    )
}

// ── Building blocks ───────────────────────────────────────────────────────────

pub fn radius(r: f32) -> Radius {
    Radius::from(r)
}

pub fn border(color: Color, width: f32, r: f32) -> Border {
    Border {
        color,
        width,
        radius: radius(r),
    }
}

pub fn no_border(r: f32) -> Border {
    border(Color::TRANSPARENT, 0.0, r)
}

pub fn shadow_lg() -> Shadow {
    Shadow {
        color: a(BLACK, if is_dark() { 0.5 } else { 0.12 }),
        offset: Vector::new(0.0, 8.0),
        blur_radius: 24.0,
    }
}

pub fn shadow_sm() -> Shadow {
    Shadow {
        color: a(BLACK, 0.12),
        offset: Vector::new(0.0, 1.0),
        blur_radius: 3.0,
    }
}

// ── Containers ────────────────────────────────────────────────────────────────

pub fn bg(color: Color) -> impl Fn(&Theme) -> container::Style {
    move |_| container::Style {
        background: Some(Background::Color(color)),
        ..container::Style::default()
    }
}

pub fn page(_: &Theme) -> container::Style {
    let k = t();
    container::Style {
        background: Some(k.background.into()),
        text_color: Some(k.foreground),
        ..container::Style::default()
    }
}

pub fn sidebar(_: &Theme) -> container::Style {
    let k = t();
    container::Style {
        background: Some(k.sidebar.into()),
        text_color: Some(k.sidebar_fg),
        ..container::Style::default()
    }
}

/// Bordered box (`rounded-lg border`), transparent fill.
pub fn bordered(r: f32) -> impl Fn(&Theme) -> container::Style {
    move |_| container::Style {
        border: border(t().border, 1.0, r),
        ..container::Style::default()
    }
}

/// `bg-muted/<alpha>` box with radius.
pub fn muted_box(alpha: f32, r: f32) -> impl Fn(&Theme) -> container::Style {
    move |_| container::Style {
        background: Some(a(t().muted, alpha).into()),
        border: no_border(r),
        ..container::Style::default()
    }
}

/// Popover / dropdown panel.
pub fn popover(_: &Theme) -> container::Style {
    let k = t();
    container::Style {
        background: Some(k.popover.into()),
        text_color: Some(k.popover_fg),
        border: no_border(R_LG),
        shadow: shadow_lg(),
        ..container::Style::default()
    }
}

/// Dialog popup (`rounded-xl ring-1 ring-foreground/10 bg-popover`).
pub fn dialog(_: &Theme) -> container::Style {
    let k = t();
    container::Style {
        background: Some(k.popover.into()),
        text_color: Some(k.popover_fg),
        border: no_border(R_XL),
        shadow: shadow_lg(),
        ..container::Style::default()
    }
}

/// Dialog footer strip (`bg-muted/50 border-t`, bottom corners rounded).
pub fn dialog_footer(_: &Theme) -> container::Style {
    let k = t();
    container::Style {
        background: Some(a(k.muted, 0.5).into()),
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: Radius {
                top_left: 0.0,
                top_right: 0.0,
                bottom_right: R_XL,
                bottom_left: R_XL,
            },
        },
        ..container::Style::default()
    }
}

/// Backdrop behind dialogs (`bg-black/10`) / palette (`bg-black/40`).
pub fn backdrop(alpha: f32) -> impl Fn(&Theme) -> container::Style {
    move |_| container::Style {
        background: Some(a(BLACK, alpha).into()),
        ..container::Style::default()
    }
}

pub fn tooltip(_: &Theme) -> container::Style {
    let k = t();
    container::Style {
        background: Some(k.foreground.into()),
        text_color: Some(k.background),
        border: no_border(R_MD),
        ..container::Style::default()
    }
}

/// 1px divider line in the border colour.
pub fn divider(_: &Theme) -> container::Style {
    container::Style {
        background: Some(t().border.into()),
        ..container::Style::default()
    }
}

pub fn divider_alpha(alpha: f32) -> impl Fn(&Theme) -> container::Style {
    move |_| container::Style {
        background: Some(a(t().border, alpha).into()),
        ..container::Style::default()
    }
}

/// Queue panel: raised surface with a soft shadow on its left edge.
pub fn side_panel(_: &Theme) -> container::Style {
    let k = t();
    container::Style {
        background: Some(k.popover.into()),
        text_color: Some(k.popover_fg),
        border: no_border(0.0),
        shadow: shadow_lg(),
        ..container::Style::default()
    }
}

/// Dark veil over the blurred art in full-screen now playing.
pub fn scrim(_: &Theme) -> container::Style {
    container::Style {
        background: Some(a(BLACK, 0.55).into()),
        ..container::Style::default()
    }
}

/// Transparent title bar (full-screen now playing shows the art under it).
pub fn clear(_: &Theme) -> container::Style {
    container::Style {
        text_color: Some(t().foreground),
        ..container::Style::default()
    }
}

/// Window frame — title bar and now-playing bar — in the sidebar's colour,
/// so the chrome frames the page without divider lines.
pub fn frame(_: &Theme) -> container::Style {
    let k = t();
    container::Style {
        background: Some(k.sidebar.into()),
        text_color: Some(k.foreground),
        ..container::Style::default()
    }
}

pub fn sidebar_divider(_: &Theme) -> container::Style {
    container::Style {
        background: Some(t().sidebar_border.into()),
        ..container::Style::default()
    }
}

// ── Buttons ───────────────────────────────────────────────────────────────────

fn btn(background: Option<Color>, text: Color, border_: Border) -> button::Style {
    button::Style {
        background: background.map(Background::Color),
        text_color: text,
        border: border_,
        shadow: Shadow::default(),
        snap: true,
    }
}

fn disabled(mut s: button::Style, opacity: f32) -> button::Style {
    s.text_color = a(s.text_color, opacity);
    s.background = s.background.map(|b| match b {
        Background::Color(c) => Background::Color(a(c, opacity)),
        other => other,
    });
    s.border.color = a(s.border.color, opacity);
    s
}

/// shadcn `variant="default"` (primary fill).
pub fn btn_primary(_: &Theme, status: button::Status) -> button::Style {
    let k = t();
    let base = btn(Some(k.primary), k.primary_fg, no_border(R_MD));
    match status {
        button::Status::Hovered => btn(Some(a(k.primary, 0.8)), k.primary_fg, no_border(R_MD)),
        button::Status::Pressed => btn(Some(a(k.primary, 0.9)), k.primary_fg, no_border(R_MD)),
        button::Status::Disabled => disabled(base, 0.5),
        button::Status::Active => base,
    }
}

/// shadcn `variant="outline"`.
pub fn btn_outline(_: &Theme, status: button::Status) -> button::Style {
    let k = t();
    let fill = if k.dark { a(k.input, 0.3) } else { k.background };
    let border_c = if k.dark { k.input } else { k.border };
    let base = btn(Some(fill), k.foreground, border(border_c, 1.0, R_MD));
    match status {
        button::Status::Hovered | button::Status::Pressed => btn(
            Some(if k.dark { a(k.input, 0.5) } else { k.muted }),
            k.foreground,
            border(border_c, 1.0, R_MD),
        ),
        button::Status::Disabled => disabled(base, 0.5),
        button::Status::Active => base,
    }
}

/// shadcn `variant="secondary"`.
pub fn btn_secondary(_: &Theme, status: button::Status) -> button::Style {
    let k = t();
    let base = btn(Some(k.secondary), k.secondary_fg, no_border(R_MD));
    match status {
        button::Status::Hovered | button::Status::Pressed => {
            btn(Some(a(k.secondary, 0.8)), k.secondary_fg, no_border(R_MD))
        }
        button::Status::Disabled => disabled(base, 0.5),
        button::Status::Active => base,
    }
}

/// shadcn `variant="ghost"`.
pub fn btn_ghost(_: &Theme, status: button::Status) -> button::Style {
    let k = t();
    let base = btn(None, k.foreground, no_border(R_MD));
    match status {
        button::Status::Hovered | button::Status::Pressed => btn(
            Some(if k.dark { a(k.muted, 0.5) } else { k.muted }),
            k.foreground,
            no_border(R_MD),
        ),
        button::Status::Disabled => disabled(base, 0.5),
        button::Status::Active => base,
    }
}

/// shadcn `variant="destructive"` (`bg-destructive/10 text-destructive`).
pub fn btn_destructive(_: &Theme, status: button::Status) -> button::Style {
    let k = t();
    let base = btn(Some(a(k.destructive, 0.10)), k.destructive, no_border(R_MD));
    match status {
        button::Status::Hovered | button::Status::Pressed => {
            btn(Some(a(k.destructive, 0.20)), k.destructive, no_border(R_MD))
        }
        button::Status::Disabled => disabled(base, 0.5),
        button::Status::Active => base,
    }
}

/// Plain icon / text button: muted text, foreground on hover, no fill
/// (`text-muted-foreground hover:text-foreground`).
pub fn btn_icon(_: &Theme, status: button::Status) -> button::Style {
    let k = t();
    match status {
        button::Status::Hovered | button::Status::Pressed => {
            btn(None, k.foreground, no_border(0.0))
        }
        button::Status::Disabled => btn(None, a(k.muted_fg, 0.4), no_border(0.0)),
        button::Status::Active => btn(None, k.muted_fg, no_border(0.0)),
    }
}

/// Text-coloured button with custom resting / hover colours and no fill.
pub fn btn_text(rest: Color, hover: Color) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| match status {
        button::Status::Hovered | button::Status::Pressed => btn(None, hover, no_border(0.0)),
        button::Status::Disabled => btn(None, a(rest, 0.4), no_border(0.0)),
        button::Status::Active => btn(None, rest, no_border(0.0)),
    }
}

/// Muted icon button with a `hover:bg-muted` square (`rounded-<r>`).
pub fn btn_muted_square(r: f32) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| {
        let k = t();
        match status {
            button::Status::Hovered | button::Status::Pressed => {
                btn(Some(k.muted), k.foreground, no_border(r))
            }
            button::Status::Disabled => btn(None, a(k.muted_fg, 0.3), no_border(r)),
            button::Status::Active => btn(None, k.muted_fg, no_border(r)),
        }
    }
}

/// Title-bar close button (`hover:bg-destructive hover:text-white`).
pub fn btn_close(_: &Theme, status: button::Status) -> button::Style {
    let k = t();
    match status {
        button::Status::Hovered | button::Status::Pressed => {
            btn(Some(k.destructive), WHITE, no_border(0.0))
        }
        _ => btn(None, k.muted_fg, no_border(0.0)),
    }
}

/// Sidebar navigation button.
pub fn btn_nav(active: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| {
        let k = t();
        if active {
            return btn(Some(a(k.primary, 0.16)), k.primary, no_border(R_MD));
        }
        match status {
            button::Status::Hovered | button::Status::Pressed => btn(
                Some(a(k.sidebar_accent, 0.6)),
                k.sidebar_accent_fg,
                no_border(R_MD),
            ),
            _ => btn(None, a(k.sidebar_fg, 0.5), no_border(R_MD)),
        }
    }
}

/// Shuffle / repeat mode toggle (`bg-foreground/10` when active).
pub fn btn_mode(active: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| {
        let k = t();
        if active {
            return btn(Some(a(k.foreground, 0.10)), k.foreground, no_border(R_MD));
        }
        match status {
            button::Status::Hovered | button::Status::Pressed => {
                btn(Some(a(k.foreground, 0.05)), k.foreground, no_border(R_MD))
            }
            _ => btn(None, k.muted_fg, no_border(R_MD)),
        }
    }
}

/// Round play/pause button (`bg-foreground text-background hover:opacity-80`).
pub fn btn_play(_: &Theme, status: button::Status) -> button::Style {
    let k = t();
    let base = btn(Some(k.foreground), k.background, no_border(R_FULL));
    match status {
        button::Status::Hovered | button::Status::Pressed => {
            btn(Some(a(k.foreground, 0.8)), k.background, no_border(R_FULL))
        }
        button::Status::Disabled => disabled(base, 0.3),
        button::Status::Active => base,
    }
}

/// Big round accent button (full-screen Play / Pause).
pub fn btn_accent_round(_: &Theme, status: button::Status) -> button::Style {
    let k = t();
    let base = btn(Some(k.primary), k.primary_fg, no_border(R_FULL));
    match status {
        button::Status::Hovered | button::Status::Pressed => {
            btn(Some(a(k.primary, 0.85)), k.primary_fg, no_border(R_FULL))
        }
        button::Status::Disabled => disabled(base, 0.3),
        button::Status::Active => base,
    }
}

/// Bordered chip (sleep-timer presets, small options); filled with primary
/// when `active`.
pub fn btn_chip(active: bool, r: f32) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| {
        let k = t();
        if active {
            return btn(Some(k.primary), k.primary_fg, border(k.primary, 1.0, r));
        }
        match status {
            button::Status::Hovered | button::Status::Pressed => {
                btn(Some(k.muted), k.foreground, border(k.border, 1.0, r))
            }
            _ => btn(None, k.foreground, border(k.border, 1.0, r)),
        }
    }
}

/// Pill (`rounded-full`) — primary when active, muted otherwise.
pub fn btn_pill(active: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| {
        let k = t();
        if active {
            return btn(Some(k.primary), k.primary_fg, no_border(R_FULL));
        }
        match status {
            button::Status::Hovered | button::Status::Pressed => {
                btn(Some(a(k.muted, 0.8)), k.foreground, no_border(R_FULL))
            }
            _ => btn(Some(k.muted), k.foreground, no_border(R_FULL)),
        }
    }
}

/// Segmented control item (`bg-secondary` when active).
pub fn btn_segment(active: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| {
        let k = t();
        if active {
            return btn(Some(k.secondary), k.secondary_fg, no_border(R_MD));
        }
        match status {
            button::Status::Hovered | button::Status::Pressed => {
                btn(Some(a(k.muted, 0.6)), k.foreground, no_border(R_MD))
            }
            _ => btn(None, k.muted_fg, no_border(R_MD)),
        }
    }
}

/// List item with a hover fill; `selected` uses the accent colour.
pub fn btn_list_item(selected: bool, r: f32) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| {
        let k = t();
        if selected {
            return btn(Some(k.accent), k.accent_fg, no_border(r));
        }
        match status {
            button::Status::Hovered | button::Status::Pressed => {
                btn(Some(a(k.muted, 0.5)), k.foreground, no_border(r))
            }
            _ => btn(None, k.foreground, no_border(r)),
        }
    }
}

/// Link-style text button (`hover:underline` approximated with a colour shift).
pub fn btn_link(_: &Theme, status: button::Status) -> button::Style {
    let k = t();
    match status {
        button::Status::Hovered | button::Status::Pressed => btn(None, k.foreground, no_border(0.0)),
        _ => btn(None, k.muted_fg, no_border(0.0)),
    }
}

/// Fully transparent button (content draws its own visuals).
pub fn btn_bare(_: &Theme, _: button::Status) -> button::Style {
    btn(None, t().foreground, no_border(0.0))
}

// ── Inputs ────────────────────────────────────────────────────────────────────

/// shadcn input: transparent, bordered, ring on focus.
pub fn input(_: &Theme, status: text_input::Status) -> text_input::Style {
    let k = t();
    let fill = if k.dark { a(k.input, 0.3) } else { Color::TRANSPARENT };
    let border_c = match status {
        text_input::Status::Focused { .. } => k.ring,
        _ => k.input,
    };
    text_input::Style {
        background: Background::Color(fill),
        border: border(border_c, 1.0, R_MD),
        icon: k.muted_fg,
        placeholder: k.muted_fg,
        value: k.foreground,
        selection: a(k.ring, 0.4),
    }
}

/// Metadata-editor style input (`bg-muted/40`).
pub fn input_muted(_: &Theme, status: text_input::Status) -> text_input::Style {
    let k = t();
    let border_c = match status {
        text_input::Status::Focused { .. } => k.ring,
        _ => k.border,
    };
    text_input::Style {
        background: Background::Color(a(k.muted, 0.4)),
        border: border(border_c, 1.0, R_MD),
        icon: k.muted_fg,
        placeholder: a(k.muted_fg, 0.7),
        value: k.foreground,
        selection: a(k.ring, 0.4),
    }
}

/// Borderless input (command palette, inline rename).
pub fn input_bare(_: &Theme, _: text_input::Status) -> text_input::Style {
    let k = t();
    text_input::Style {
        background: Background::Color(Color::TRANSPARENT),
        border: no_border(0.0),
        icon: k.muted_fg,
        placeholder: k.muted_fg,
        value: k.foreground,
        selection: a(k.ring, 0.4),
    }
}

/// Inline rename input with only a bottom-ish border (approximated by a
/// subtle full border).
pub fn input_underline(_: &Theme, _: text_input::Status) -> text_input::Style {
    let k = t();
    text_input::Style {
        background: Background::Color(Color::TRANSPARENT),
        border: border(k.border, 1.0, R_XS),
        icon: k.muted_fg,
        placeholder: k.muted_fg,
        value: k.foreground,
        selection: a(k.ring, 0.4),
    }
}

pub fn pick(_: &Theme, status: pick_list::Status) -> pick_list::Style {
    let k = t();
    let border_c = match status {
        pick_list::Status::Opened { .. } => k.ring,
        pick_list::Status::Hovered => a(k.foreground, 0.3),
        pick_list::Status::Active => k.input,
    };
    pick_list::Style {
        text_color: k.foreground,
        placeholder_color: k.muted_fg,
        handle_color: k.muted_fg,
        background: Background::Color(if k.dark { a(k.input, 0.3) } else { k.background }),
        border: border(border_c, 1.0, R_MD),
    }
}

pub fn pick_menu(_: &Theme) -> iced::overlay::menu::Style {
    let k = t();
    iced::overlay::menu::Style {
        background: Background::Color(k.popover),
        border: border(k.border, 1.0, R_MD),
        text_color: k.popover_fg,
        selected_text_color: k.accent_fg,
        selected_background: Background::Color(k.accent),
        shadow: shadow_lg(),
    }
}

/// shadcn slider: 4px muted track, primary fill, 12px white thumb with a
/// ring-coloured border.
pub fn slider_style(_: &Theme, status: slider::Status) -> slider::Style {
    let k = t();
    let ring_w = match status {
        slider::Status::Hovered | slider::Status::Dragged => 3.0,
        slider::Status::Active => 1.0,
    };
    let ring_c = match status {
        slider::Status::Hovered | slider::Status::Dragged => a(k.ring, 0.5),
        slider::Status::Active => k.ring,
    };
    slider::Style {
        rail: slider::Rail {
            backgrounds: (k.primary.into(), k.muted.into()),
            width: 4.0,
            border: no_border(R_FULL),
        },
        handle: slider::Handle {
            shape: slider::HandleShape::Circle { radius: 6.0 },
            background: WHITE.into(),
            border_width: ring_w,
            border_color: ring_c,
        },
    }
}

// ── Scrollbars ────────────────────────────────────────────────────────────────

/// Thin theme-aware scrollbar (index.css `::-webkit-scrollbar`, 5px).
pub fn scroll(_: &Theme, status: scrollable::Status) -> scrollable::Style {
    let k = t();
    let hovered = matches!(
        status,
        scrollable::Status::Hovered {
            is_vertical_scrollbar_hovered: true,
            ..
        } | scrollable::Status::Hovered {
            is_horizontal_scrollbar_hovered: true,
            ..
        } | scrollable::Status::Dragged { .. }
    );
    let thumb = a(k.foreground, if hovered { 0.32 } else { 0.18 });
    let rail = scrollable::Rail {
        background: None,
        border: no_border(R_FULL),
        scroller: scrollable::Scroller {
            background: thumb.into(),
            border: no_border(R_FULL),
        },
    };
    scrollable::Style {
        container: container::Style::default(),
        vertical_rail: rail,
        horizontal_rail: rail,
        gap: None,
        auto_scroll: scrollable::AutoScroll {
            background: k.popover.into(),
            border: border(k.border, 1.0, R_FULL),
            shadow: shadow_sm(),
            icon: k.foreground,
        },
    }
}

/// Scrollbar configuration matching the 5px web scrollbar.
pub fn scrollbar() -> scrollable::Scrollbar {
    scrollable::Scrollbar::new().width(5).scroller_width(5).margin(0)
}

pub fn vscroll() -> scrollable::Direction {
    scrollable::Direction::Vertical(scrollbar())
}

/// Multi-line editor (`textarea`): `bg-muted/40 border rounded-md`,
/// ring on focus.
pub fn editor(_: &Theme, status: iced::widget::text_editor::Status) -> iced::widget::text_editor::Style {
    let k = t();
    let border_c = match status {
        iced::widget::text_editor::Status::Focused { .. } => k.ring,
        _ => k.border,
    };
    iced::widget::text_editor::Style {
        background: Background::Color(a(k.muted, 0.4)),
        border: border(border_c, 1.0, R_MD),
        placeholder: k.muted_fg,
        value: k.foreground,
        selection: a(k.ring, 0.4),
    }
}
