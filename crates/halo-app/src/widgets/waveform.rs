//! Waveform seek bar — port of `waveform-seek.tsx`.
//!
//! Draws the track's loudness peaks (0..255 buckets) as centred bars, split
//! into played (primary) and upcoming (muted-foreground/40) at the current
//! position. Click / drag to scrub (`on_change`), release to seek
//! (`on_commit`); hovering shows the timestamp under the cursor; ←/→ nudge
//! ±5s once the bar has been clicked (focused). Falls back to a flat
//! baseline when no peaks are available.

use iced::advanced::layout::{self, Layout};
use iced::advanced::renderer::{self, Quad, Renderer as _};
use iced::advanced::text::{self, Renderer as _};
use iced::advanced::widget::{tree, Tree, Widget};
use iced::advanced::{Clipboard, Shell};
use iced::alignment;
use iced::{keyboard, mouse, Element, Event, Length, Pixels, Point, Rectangle, Size};

use crate::format::duration;
use crate::theme::{self, a, t};

const SEEK_STEP_MS: u64 = 5000;

pub struct Waveform<'a, Message> {
    peaks: Option<&'a [u8]>,
    position: u64,
    duration: u64,
    disabled: bool,
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
        on_change: Box::new(on_change),
        on_commit: Box::new(on_commit),
    }
}

#[derive(Default)]
struct State {
    dragging: bool,
    focused: bool,
}

impl<Message> Waveform<'_, Message> {
    fn ms_at(&self, bounds: Rectangle, x: f32) -> u64 {
        if self.duration == 0 || bounds.width <= 0.0 {
            return 0;
        }
        let frac = ((x - bounds.x) / bounds.width).clamp(0.0, 1.0);
        (frac * self.duration as f32).round() as u64
    }
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
    fn layout(
        &mut self,
        _tree: &mut Tree,
        _renderer: &iced::Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
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
                shell.request_redraw();
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) if state.dragging => {
                state.dragging = false;
                let x = cursor.position().map(|p| p.x).unwrap_or(bounds.x);
                shell.publish((self.on_commit)(self.ms_at(bounds, x)));
            }
            Event::Mouse(mouse::Event::CursorLeft) => shell.request_redraw(),
            Event::Keyboard(keyboard::Event::KeyPressed {
                key: keyboard::Key::Named(named),
                ..
            }) if state.focused && !inactive => {
                let next = match named {
                    keyboard::key::Named::ArrowLeft => {
                        Some(self.position.saturating_sub(SEEK_STEP_MS))
                    }
                    keyboard::key::Named::ArrowRight => {
                        Some((self.position + SEEK_STEP_MS).min(self.duration))
                    }
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
        let played_c = a(k.primary, opacity);
        let rest_c = a(a(k.muted_fg, 0.4), opacity);

        let fraction = if self.duration > 0 {
            (self.position as f32 / self.duration as f32).clamp(0.0, 1.0)
        } else {
            0.0
        };

        let bars = self.peaks.filter(|p| !p.is_empty());
        let count = bars.map(|p| p.len()).unwrap_or(((b.width / 3.0).floor() as usize).max(1));
        let slot = b.width / count as f32;
        let bar_w = (slot * 0.6).max(1.0);
        let played_x = b.width * fraction;
        let mid = b.height / 2.0;

        for i in 0..count {
            let x = i as f32 * slot + (slot - bar_w) / 2.0;
            let amp = bars.map(|p| p[i] as f32 / 255.0).unwrap_or(0.12);
            let bar_h = (amp * (b.height - 2.0)).max(2.0);
            let color = if x + bar_w <= played_x { played_c } else { rest_c };
            renderer.fill_quad(
                Quad {
                    bounds: Rectangle {
                        x: b.x + x,
                        y: b.y + mid - bar_h / 2.0,
                        width: bar_w,
                        height: bar_h,
                    },
                    ..Quad::default()
                },
                color,
            );
        }

        // Hover timestamp, 20px above the bar.
        if !self.disabled && self.duration > 0 {
            if let Some(p) = cursor.position_over(b) {
                let hover_ms = self.ms_at(b, p.x);
                let label = duration(Some(hover_ms as i64));
                let w = label.chars().count() as f32 * 6.0 + 8.0;
                let h = 16.0;
                let r = Rectangle {
                    x: p.x - w / 2.0,
                    y: b.y - 20.0,
                    width: w,
                    height: h,
                };
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
                            size: Pixels(theme::TEXT_10),
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
        let _ = state;
    }
}

impl<'a, Message: 'a> From<Waveform<'a, Message>> for Element<'a, Message> {
    fn from(w: Waveform<'a, Message>) -> Self {
        Element::new(w)
    }
}
