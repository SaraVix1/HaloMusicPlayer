//! Single-line text with CSS-like overflow behaviour:
//!
//! * [`Overflow::Ellipsis`] — `truncate` (`text-overflow: ellipsis`).
//! * [`Overflow::Marquee`] — now-playing bar: when the text overflows by more
//!   than 4px it slides to reveal the end and back (`marquee 7s ease-in-out
//!   1.5s infinite alternate`, holding at each end for 20% of the cycle).
//! * [`Overflow::Loop`] — mini player: the text is repeated with a 48px gap and
//!   scrolls continuously at 35 px/s (at least 8s per loop).
//! * [`Overflow::Clip`] — plain clipping.
//!
//! Animation is self-driven via redraw requests, so the app doesn't need a
//! timer subscription for it.

use std::time::Instant;

use iced::advanced::layout::{self, Layout};
use iced::advanced::renderer;
use iced::advanced::text::{self, paragraph, Paragraph as _, Renderer as _};
use iced::advanced::widget::{self, tree, Tree, Widget};
use iced::advanced::{Clipboard, Shell};
use iced::alignment;
use iced::{event, mouse, window, Color, Element, Event, Font, Length, Pixels, Point, Rectangle, Size};

use crate::theme;

type Paragraph = <iced::Renderer as text::Renderer>::Paragraph;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Overflow {
    Ellipsis,
    Marquee,
    Loop,
    Clip,
}

pub struct TextLine<'a> {
    content: std::borrow::Cow<'a, str>,
    size: f32,
    font: Font,
    color: Option<Color>,
    line_height: f32,
    width: Length,
    max_width: f32,
    overflow: Overflow,
    align_right: bool,
}

pub fn line<'a>(content: impl Into<std::borrow::Cow<'a, str>>) -> TextLine<'a> {
    TextLine {
        content: content.into(),
        size: theme::BODY,
        font: theme::SANS,
        color: None,
        line_height: 1.43,
        width: Length::Shrink,
        max_width: f32::INFINITY,
        overflow: Overflow::Ellipsis,
        align_right: false,
    }
}

impl<'a> TextLine<'a> {
    pub fn size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }
    pub fn font(mut self, font: Font) -> Self {
        self.font = font;
        self
    }
    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }
    /// Line height as a multiple of the font size.
    pub fn line_height(mut self, lh: f32) -> Self {
        self.line_height = lh;
        self
    }
    /// Absolute line height in pixels.
    pub fn line_height_px(mut self, px: f32) -> Self {
        self.line_height = px / self.size;
        self
    }
    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = width.into();
        self
    }
    pub fn max_width(mut self, w: f32) -> Self {
        self.max_width = w;
        self
    }
    pub fn overflow(mut self, overflow: Overflow) -> Self {
        self.overflow = overflow;
        self
    }
    pub fn marquee(self) -> Self {
        self.overflow(Overflow::Marquee)
    }
    pub fn looping(self) -> Self {
        self.overflow(Overflow::Loop)
    }
    pub fn align_right(mut self) -> Self {
        self.align_right = true;
        self
    }

    fn text_spec<'b>(&self, content: &'b str) -> text::Text<&'b str, Font> {
        text::Text {
            content,
            bounds: Size::new(f32::INFINITY, f32::INFINITY),
            size: Pixels(self.size),
            line_height: text::LineHeight::Relative(self.line_height),
            font: self.font,
            align_x: text::Alignment::Left,
            align_y: alignment::Vertical::Top,
            shaping: text::Shaping::Advanced,
            wrapping: text::Wrapping::None,
        }
    }
}

#[derive(Default)]
struct State {
    content: String,
    size: f32,
    font: Option<Font>,
    full: paragraph::Plain<Paragraph>,
    full_width: f32,
    /// Truncated paragraph for the ellipsis mode (None = fits).
    truncated: Option<Paragraph>,
    truncated_for: f32,
    width: f32,
    start: Option<Instant>,
    offset: f32,
}

fn ease_in_out(x: f32) -> f32 {
    // cubic-bezier(0.42, 0, 0.58, 1) ≈ smoothstep-like curve; solve with a
    // few Newton iterations for accuracy.
    let (x1, x2) = (0.42f32, 0.58f32);
    let bez = |t: f32, p1: f32, p2: f32| {
        let u = 1.0 - t;
        3.0 * u * u * t * p1 + 3.0 * u * t * t * p2 + t * t * t
    };
    let mut t = x;
    for _ in 0..6 {
        let xt = bez(t, x1, x2) - x;
        let d = {
            let u = 1.0 - t;
            3.0 * u * u * x1 + 6.0 * u * t * (x2 - x1) + 3.0 * t * t * (1.0 - x2)
        };
        if d.abs() < 1e-5 {
            break;
        }
        t = (t - xt / d).clamp(0.0, 1.0);
    }
    bez(t, 0.0, 1.0)
}

impl<'a, Message> Widget<Message, iced::Theme, iced::Renderer> for TextLine<'a> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }

    fn size(&self) -> Size<Length> {
        Size::new(self.width, Length::Shrink)
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        _renderer: &iced::Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let state = tree.state.downcast_mut::<State>();
        let changed = state.content != self.content
            || state.size != self.size
            || state.font != Some(self.font);
        if changed {
            state.content = self.content.to_string();
            state.size = self.size;
            state.font = Some(self.font);
            state.full.update(self.text_spec(&self.content));
            state.full_width = state.full.min_bounds().width;
            state.truncated = None;
            state.truncated_for = -1.0;
            state.start = None;
            state.offset = 0.0;
        }
        let h = (self.size * self.line_height).ceil();
        let natural = Size::new(state.full_width.min(self.max_width), h);
        let size = limits
            .width(self.width)
            .height(Length::Fixed(h))
            .max_width(self.max_width)
            .resolve(self.width, Length::Fixed(h), natural);
        state.width = size.width;

        if self.overflow == Overflow::Ellipsis
            && state.full_width > size.width + 0.5
            && (state.truncated_for - size.width).abs() > 0.5
        {
            state.truncated_for = size.width;
            let chars: Vec<(usize, char)> = self.content.char_indices().collect();
            // Binary search the longest prefix that fits with "…".
            let (mut lo, mut hi) = (0usize, chars.len());
            let mut best = Paragraph::with_text(self.text_spec("…"));
            while lo < hi {
                let mid = (lo + hi).div_ceil(2);
                let end = chars.get(mid).map(|c| c.0).unwrap_or(self.content.len());
                let candidate = format!("{}…", self.content[..end].trim_end());
                let p = Paragraph::with_text(self.text_spec(&candidate));
                if p.min_bounds().width <= size.width {
                    best = p;
                    lo = mid;
                } else {
                    hi = mid - 1;
                }
            }
            if lo == 0 {
                best = Paragraph::with_text(self.text_spec("…"));
            }
            state.truncated = Some(best);
        } else if state.full_width <= size.width + 0.5 {
            state.truncated = None;
            state.truncated_for = -1.0;
        }

        layout::Node::new(size)
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        _layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _renderer: &iced::Renderer,
        _clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        if !matches!(self.overflow, Overflow::Marquee | Overflow::Loop) {
            return;
        }
        let Event::Window(window::Event::RedrawRequested(now)) = event else {
            return;
        };
        let state = tree.state.downcast_mut::<State>();
        let overflow = state.full_width - state.width;
        let animate = match self.overflow {
            Overflow::Marquee => overflow > 4.0,
            _ => overflow > 0.5,
        };
        if !animate {
            state.offset = 0.0;
            state.start = None;
            return;
        }
        let start = *state.start.get_or_insert(*now);
        let t = now.saturating_duration_since(start).as_secs_f32();
        state.offset = match self.overflow {
            Overflow::Marquee => {
                const DELAY: f32 = 1.5;
                const CYCLE: f32 = 7.0;
                if t < DELAY {
                    0.0
                } else {
                    let tt = (t - DELAY) / CYCLE;
                    let n = tt.floor() as i64;
                    let mut p = tt - tt.floor();
                    if n % 2 == 1 {
                        p = 1.0 - p;
                    }
                    let local = ((p - 0.2) / 0.6).clamp(0.0, 1.0);
                    ease_in_out(local) * overflow
                }
            }
            _ => {
                let span = state.full_width + 48.0;
                let duration = (span / 35.0).max(8.0);
                (t * span / duration) % span
            }
        };
        shell.request_redraw();
        let _ = event::Status::Ignored;
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut iced::Renderer,
        _theme: &iced::Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_ref::<State>();
        let bounds = layout.bounds();
        if !bounds.intersects(viewport) {
            return;
        }
        let color = self.color.unwrap_or(style.text_color);
        // Clip to our bounds within the parent's visible area (a layer would
        // escape the parent's clip, e.g. a scrollable).
        let Some(clip) = bounds.intersection(viewport) else {
            return;
        };

        match self.overflow {
            Overflow::Ellipsis if state.truncated.is_some() => {
                renderer.fill_paragraph(
                    state.truncated.as_ref().unwrap(),
                    bounds.position(),
                    color,
                    clip,
                );
            }
            Overflow::Marquee | Overflow::Loop if state.full_width > bounds.width + 0.5 => {
                let p = Point::new(bounds.x - state.offset, bounds.y);
                renderer.fill_paragraph(state.full.raw(), p, color, clip);
                if self.overflow == Overflow::Loop {
                    let p2 = Point::new(p.x + state.full_width + 48.0, bounds.y);
                    renderer.fill_paragraph(state.full.raw(), p2, color, clip);
                }
            }
            _ => {
                let x = if self.align_right {
                    bounds.x + (bounds.width - state.full_width).max(0.0)
                } else {
                    bounds.x
                };
                renderer.fill_paragraph(state.full.raw(), Point::new(x, bounds.y), color, clip);
            }
        }
    }

    fn operate(
        &mut self,
        _tree: &mut Tree,
        layout: Layout<'_>,
        _renderer: &iced::Renderer,
        operation: &mut dyn widget::Operation,
    ) {
        operation.text(None, layout.bounds(), &self.content);
    }
}

impl<'a, Message: 'a> From<TextLine<'a>> for Element<'a, Message> {
    fn from(t: TextLine<'a>) -> Self {
        Element::new(t)
    }
}
