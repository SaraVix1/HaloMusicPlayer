//! Seek bar with the track's waveform.
//!
//! [`Look::Bar`] (now-playing bar): a slim rounded line at rest, accent up
//! to the position; while hovered or dragged it grows (≈160 ms, animated by
//! the widget itself) into spaced, rounded waveform bars. [`Look::Full`]
//! (full-screen player): always the waveform, over a faint line.
//!
//! Peaks are the backend's 0..255 loudness buckets over the whole song,
//! resampled to 2 px bars with 2 px gaps. Quiet buckets draw nothing over
//! the line (a fade-out reads as a line, not as dashes). Click / drag to
//! scrub (`on_change`), release to seek (`on_commit`); hovering shows the
//! time under the cursor; ←/→ nudge ±5 s once the bar has been clicked.

use std::time::{Duration, Instant};

use iced::advanced::layout::{self, Layout};
use iced::advanced::renderer::{self, Quad, Renderer as _};
use iced::advanced::text::{self, Renderer as _};
use iced::advanced::widget::{tree, Tree, Widget};
use iced::advanced::{Clipboard, Shell};
use iced::alignment;
use iced::{keyboard, mouse, window, Element, Event, Length, Pixels, Point, Rectangle, Size};

use crate::format::duration;
use crate::theme::{self, a, t};

const SEEK_STEP_MS: u64 = 5000;
const LINE: f32 = 3.0;
const BAR_W: f32 = 2.0;
const BAR_GAP: f32 = 2.0;
const EXPAND: Duration = Duration::from_millis(160);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Look {
    /// Line at rest, waveform while hovered / dragged.
    #[default]
    Bar,
    /// Always the waveform.
    Full,
}

pub struct Waveform<'a, Message> {
    peaks: Option<&'a [u8]>,
    position: u64,
    duration: u64,
    disabled: bool,
    look: Look,
    on_change: Box<dyn Fn(u64) -> Message + 'a>,
    on_commit: Box<dyn Fn(u64) -> Message + 'a>,
}

pub fn waveform<'a, Message>(
    peaks: Option<&'a [u8]>,
    position: u64,
    duration: u64,
    disabled: bool,
    on_change: impl Fn(u64) -> Message + 'a,
    on_commit: impl Fn(u64) -> Message + 'a,
) -> Waveform<'a, Message> {
    Waveform {
        peaks,
        position,
        duration,
        disabled,
        look: Look::Bar,
        on_change: Box::new(on_change),
        on_commit: Box::new(on_commit),
    }
}

impl<Message> Waveform<'_, Message> {
    pub fn look(mut self, look: Look) -> Self {
        self.look = look;
        self
    }

    fn ms_at(&self, bounds: Rectangle, x: f32) -> u64 {
        if self.duration == 0 || bounds.width <= 0.0 {
            return 0;
        }
        let frac = ((x - bounds.x) / bounds.width).clamp(0.0, 1.0);
        (frac * self.duration as f32).round() as u64
    }
}

#[derive(Default)]
struct State {
    dragging: bool,
    focused: bool,
    /// 0 = line, 1 = waveform (Look::Bar).
    expand: f32,
    last: Option<Instant>,
}

/// `peaks` (0..255) resampled to `count` bars of 0..1: the loudest bucket
/// under each bar when shrinking, linear interpolation when stretching.
pub fn resample(peaks: &[u8], count: usize) -> Vec<f32> {
    if peaks.is_empty() || count == 0 {
        return vec![0.0; count];
    }
    let n = peaks.len();
    (0..count)
        .map(|j| {
            if count <= n {
                let lo = j * n / count;
                let hi = ((j + 1) * n / count).max(lo + 1).min(n);
                peaks[lo..hi].iter().copied().max().unwrap_or(0) as f32 / 255.0
            } else {
                let x = j as f32 * (n - 1) as f32 / (count - 1).max(1) as f32;
                let i = x.floor() as usize;
                let f = x - i as f32;
                let a_ = peaks[i] as f32;
                let b_ = peaks[(i + 1).min(n - 1)] as f32;
                (a_ + (b_ - a_) * f) / 255.0
            }
        })
        .collect()
}

fn smooth(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

impl<'a, Message> Widget<Message, iced::Theme, iced::Renderer> for Waveform<'a, Message> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }
    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }
    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fill)
    }
    fn layout(&mut self, _tree: &mut Tree, _renderer: &iced::Renderer, limits: &layout::Limits) -> layout::Node {
        layout::Node::new(limits.max())
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &iced::Renderer,
        _clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_mut::<State>();
        let bounds = layout.bounds();
        let inactive = self.disabled || self.duration == 0;

        // Line ↔ waveform animation (Bar look only).
        if self.look == Look::Bar {
            let target = if !inactive && (state.dragging || cursor.is_over(bounds)) { 1.0 } else { 0.0 };
            match event {
                Event::Window(window::Event::RedrawRequested(now)) if state.expand != target => {
                    let dt = state.last.map_or(0.0, |l| now.saturating_duration_since(l).as_secs_f32());
                    let step = dt / EXPAND.as_secs_f32();
                    state.expand = if target > state.expand {
                        (state.expand + step).min(1.0)
                    } else {
                        (state.expand - step).max(0.0)
                    };
                    state.last = Some(*now);
                    shell.request_redraw();
                }
                Event::Window(window::Event::RedrawRequested(_)) => state.last = None,
                Event::Mouse(_) if state.expand != target => shell.request_redraw(),
                _ => {}
            }
        }

        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if let Some(p) = cursor.position_over(bounds) {
                    if inactive {
                        return;
                    }
                    state.dragging = true;
                    state.focused = true;
                    shell.publish((self.on_change)(self.ms_at(bounds, p.x)));
                    shell.capture_event();
                } else {
                    state.focused = false;
                }
            }
            Event::Mouse(mouse::Event::CursorMoved { position }) => {
                if state.dragging {
                    shell.publish((self.on_change)(self.ms_at(bounds, position.x)));
                }
                if state.dragging || bounds.contains(*position) {
                    shell.request_redraw();
                }
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) if state.dragging => {
                state.dragging = false;
                let x = cursor.position().map(|p| p.x).unwrap_or(bounds.x);
                shell.publish((self.on_commit)(self.ms_at(bounds, x)));
                shell.request_redraw();
            }
            Event::Mouse(mouse::Event::CursorLeft) => shell.request_redraw(),
            Event::Keyboard(keyboard::Event::KeyPressed { key: keyboard::Key::Named(named), .. })
                if state.focused && !inactive =>
            {
                let next = match named {
                    keyboard::key::Named::ArrowLeft => Some(self.position.saturating_sub(SEEK_STEP_MS)),
                    keyboard::key::Named::ArrowRight => Some((self.position + SEEK_STEP_MS).min(self.duration)),
                    _ => None,
                };
                if let Some(ms) = next {
                    shell.publish((self.on_commit)(ms));
                    shell.capture_event();
                }
            }
            _ => {}
        }
    }

    fn mouse_interaction(
        &self,
        _tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        if !self.disabled && self.duration > 0 && cursor.is_over(layout.bounds()) {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::None
        }
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut iced::Renderer,
        _theme: &iced::Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_ref::<State>();
        let b = layout.bounds();
        if b.width <= 0.0 || b.height <= 0.0 {
            return;
        }
        let k = t();
        let opacity = if self.disabled { 0.5 } else { 1.0 };
        let fraction = if self.duration > 0 {
            (self.position as f32 / self.duration as f32).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let played_x = b.width * fraction;
        let mid = b.y + b.height / 2.0;
        let e = match self.look {
            Look::Full => 1.0,
            Look::Bar => smooth(state.expand),
        };

        // The line: full strength at rest, faint under the waveform.
        let line_a = opacity * (1.0 - 0.7 * e);
        for (w, c) in [(b.width, a(a(k.muted_fg, 0.35), line_a)), (played_x, a(k.primary, line_a))] {
            if w > 0.0 {
                renderer.fill_quad(
                    Quad {
                        bounds: Rectangle { x: b.x, y: mid - LINE / 2.0, width: w, height: LINE },
                        border: theme::no_border(LINE / 2.0),
                        ..Quad::default()
                    },
                    c,
                );
            }
        }

        // The waveform, growing out of the line.
        if e > 0.01 {
            if let Some(peaks) = self.peaks.filter(|p| !p.is_empty()) {
                let count = ((b.width + BAR_GAP) / (BAR_W + BAR_GAP)).floor().max(1.0) as usize;
                let stride = b.width / count as f32;
                let amps = resample(peaks, count);
                let played_c = a(k.primary, opacity * e);
                let rest_c = a(a(k.muted_fg, 0.35), opacity * e);
                for (i, amp) in amps.into_iter().enumerate() {
                    let h = amp * b.height * e;
                    if h < 1.0 {
                        continue; // quiet: just the line
                    }
                    let x = i as f32 * stride + (stride - BAR_W) / 2.0;
                    renderer.fill_quad(
                        Quad {
                            bounds: Rectangle { x: b.x + x, y: mid - h / 2.0, width: BAR_W, height: h },
                            border: theme::no_border(BAR_W / 2.0),
                            ..Quad::default()
                        },
                        if x + BAR_W <= played_x { played_c } else { rest_c },
                    );
                }
            }
        }

        // Hover timestamp, 20px above the bar.
        if !self.disabled && self.duration > 0 {
            if let Some(p) = cursor.position_over(b) {
                let label = duration(Some(self.ms_at(b, p.x) as i64));
                let w = label.chars().count() as f32 * 6.5 + 10.0;
                let h = 18.0;
                let r = Rectangle { x: p.x - w / 2.0, y: b.y - 22.0, width: w, height: h };
                renderer.with_layer(*viewport, |renderer| {
                    renderer.fill_quad(
                        Quad {
                            bounds: r,
                            border: theme::border(k.border, 0.0, theme::R_XS),
                            shadow: theme::shadow_sm(),
                            ..Quad::default()
                        },
                        k.popover,
                    );
                    renderer.fill_text(
                        text::Text {
                            content: label,
                            bounds: r.size(),
                            size: Pixels(theme::SMALL),
                            line_height: text::LineHeight::Absolute(Pixels(h)),
                            font: theme::SANS,
                            align_x: text::Alignment::Center,
                            align_y: alignment::Vertical::Center,
                            shaping: text::Shaping::Basic,
                            wrapping: text::Wrapping::None,
                        },
                        Point::new(r.center_x(), r.center_y()),
                        k.popover_fg,
                        *viewport,
                    );
                });
            }
        }
    }
}

impl<'a, Message: 'a> From<Waveform<'a, Message>> for Element<'a, Message> {
    fn from(w: Waveform<'a, Message>) -> Self {
        Element::new(w)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resample_shrinks_by_max_and_stretches_by_interpolation() {
        let p = [0u8, 255, 0, 51, 0, 0];
        // 6 → 3: the loudest of each pair.
        assert_eq!(resample(&p, 3), vec![1.0, 0.2, 0.0]);
        // 2 → 3: interpolated.
        let s = resample(&[0, 255], 3);
        assert_eq!(s.len(), 3);
        assert!((s[1] - 0.5).abs() < 0.01);
        assert_eq!(resample(&[], 4), vec![0.0; 4]);
        assert!(resample(&p, 0).is_empty());
    }

    #[test]
    fn resample_covers_every_bucket_once() {
        // 200 buckets → 37 bars: each bar is the max of its slice, and a lone
        // loud bucket shows up in exactly one bar wherever it is.
        for loud in [0usize, 57, 199] {
            let mut p = vec![10u8; 200];
            p[loud] = 255;
            let bars = resample(&p, 37);
            assert_eq!(bars.iter().filter(|&&x| x == 1.0).count(), 1, "bucket {loud}");
        }
    }
}
