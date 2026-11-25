//! Smooth transitions when the song changes: the cover cross-fades from the
//! previous art to the new one, and the accent colour glides from the old
//! cover's colour to the new one. A 16 ms redraw tick runs only while one of
//! them is in progress (see `subscription`).

use std::time::{Duration, Instant};

use iced::widget::{image, stack};
use iced::{Color, ContentFit, Element, Length, Subscription};

use crate::app::Message;
use crate::theme;

pub const ART: Duration = Duration::from_millis(250);
pub const ACCENT: Duration = Duration::from_millis(300);

/// Eased progress 0…1 after `elapsed` of `total` (smoothstep).
pub fn eased(elapsed: Duration, total: Duration) -> f32 {
    let x = (elapsed.as_secs_f32() / total.as_secs_f32().max(f32::EPSILON)).clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

pub fn lerp(a: Color, b: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    Color {
        r: a.r + (b.r - a.r) * t,
        g: a.g + (b.g - a.g) * t,
        b: a.b + (b.b - a.b) * t,
        a: a.a + (b.a - a.a) * t,
    }
}

#[derive(Default)]
pub struct Fades {
    /// Cover of the previous song while the new one fades in.
    prev_art: Option<String>,
    cur_art: Option<String>,
    art_start: Option<Instant>,
    /// Accent tween: from → to.
    accent: Option<(Color, Color, Instant)>,
}

impl Fades {
    /// The current song's cover changed (call on track change).
    pub fn set_art(&mut self, art: Option<String>) {
        if art == self.cur_art {
            return;
        }
        self.prev_art = std::mem::replace(&mut self.cur_art, art);
        self.art_start = Some(Instant::now());
    }

    /// Glide the accent to `to` from whatever is showing now.
    pub fn set_accent(&mut self, to: Color) {
        let from = theme::accent();
        if from == to {
            self.accent = None;
            theme::set_accent(to);
        } else {
            self.accent = Some((from, to, Instant::now()));
        }
    }

    fn art_progress(&self) -> f32 {
        self.art_start.map_or(1.0, |s| eased(s.elapsed(), ART))
    }

    pub fn active(&self) -> bool {
        self.art_start.is_some() || self.accent.is_some()
    }

    /// Advance the accent tween; finish what's done.
    pub fn tick(&mut self) {
        if let Some((from, to, start)) = self.accent {
            let p = eased(start.elapsed(), ACCENT);
            theme::set_accent(lerp(from, to, p));
            if p >= 1.0 {
                self.accent = None;
            }
        }
        if self.art_start.is_some_and(|s| s.elapsed() >= ART) {
            self.art_start = None;
            self.prev_art = None;
        }
    }

    /// The current cover at `size`, cross-fading in over the previous one.
    /// `path` is the cover to show (normally the current song's).
    pub fn cover<'a>(&self, path: Option<&str>, size: f32, radius: f32) -> Element<'a, Message> {
        let now = crate::ui::art(path, size, radius);
        let fading = self.art_start.is_some() && path.map(str::to_string) == self.cur_art;
        let Some(prev) = self.prev_art.as_deref().filter(|_| fading) else {
            return now;
        };
        let p = self.art_progress();
        // New art over the old one (no dip in brightness mid-fade); a missing
        // new cover just shows its placeholder at once.
        let top: Element<'a, Message> = match path.filter(|p| crate::ui::art_exists(p)).and_then(|p| crate::thumbs::pick(p, size)) {
            Some(p_) => image(image::Handle::from_path(p_))
                .width(Length::Fixed(size))
                .height(Length::Fixed(size))
                .content_fit(ContentFit::Cover)
                .border_radius(radius)
                .opacity(p)
                .into(),
            None => return now,
        };
        stack![crate::ui::art(Some(prev), size, radius), top].into()
    }
}

pub fn subscription(f: &Fades) -> Option<Subscription<Message>> {
    f.active().then(|| iced::time::every(Duration::from_millis(16)).map(|_| Message::FadeTick))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eased_runs_zero_to_one() {
        let t = Duration::from_millis(200);
        assert_eq!(eased(Duration::ZERO, t), 0.0);
        assert_eq!(eased(Duration::from_millis(100), t), 0.5);
        assert_eq!(eased(t, t), 1.0);
        assert_eq!(eased(Duration::from_secs(9), t), 1.0);
        // Slow start, slow end.
        assert!(eased(Duration::from_millis(20), t) < 0.1);
    }

    #[test]
    fn lerp_blends_colours() {
        let a = Color::from_rgb(0.0, 0.2, 1.0);
        let b = Color::from_rgb(1.0, 0.2, 0.0);
        assert_eq!(lerp(a, b, 0.0), a);
        assert_eq!(lerp(a, b, 1.0), b);
        let m = lerp(a, b, 0.5);
        assert!((m.r - 0.5).abs() < 1e-6 && (m.b - 0.5).abs() < 1e-6 && (m.g - 0.2).abs() < 1e-6);
        assert_eq!(lerp(a, b, 7.0), b);
    }

    #[test]
    fn set_art_starts_a_fade_only_on_change() {
        let mut f = Fades::default();
        f.set_art(Some("/a.jpg".into()));
        assert!(f.active());
        assert_eq!(f.prev_art, None);
        f.set_art(Some("/b.jpg".into()));
        assert_eq!(f.prev_art.as_deref(), Some("/a.jpg"));
        assert_eq!(f.cur_art.as_deref(), Some("/b.jpg"));
        // Same art again (next song on the same album): nothing restarts.
        let start = f.art_start;
        f.set_art(Some("/b.jpg".into()));
        assert_eq!(f.art_start, start);
    }
}
