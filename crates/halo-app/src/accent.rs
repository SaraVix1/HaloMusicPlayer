//! Accent colour from album art: the most vivid hue in a 64 px thumbnail.
//! The theme turns it into `primary` (see `theme::t`), clamping lightness
//! for the light / dark background.

use iced::Color;
use image::RgbaImage;

/// Hue buckets (15° each).
const BINS: usize = 24;
/// Pixels duller / darker than this don't count as "colour".
const MIN_SAT: f32 = 0.25;
const MIN_VAL: f32 = 0.20;
/// Fraction of the image (by vividness weight) the winning hue must cover;
/// below it the cover is treated as grey / black-and-white.
const MIN_SHARE: f32 = 0.03;

/// Load `path`, shrink it and pick its accent. `None`: unreadable, or no
/// vivid colour (the caller falls back to Halo's default accent).
pub fn from_path(path: &str) -> Option<Color> {
    let img = image::open(path).ok()?;
    from_image(&img.thumbnail(64, 64).into_rgba8())
}

/// The dominant vivid hue of `img`: pixels are bucketed by hue and
/// weighted by saturation × value, so a small bright patch can beat a large
/// dull area; the winner is the weighted mean colour of its bucket.
pub fn from_image(img: &RgbaImage) -> Option<Color> {
    let mut weight = [0f32; BINS];
    let mut sum = [[0f32; 3]; BINS];
    let mut n = 0usize;
    for p in img.pixels() {
        let [r, g, b, alpha] = p.0;
        if alpha < 128 {
            continue;
        }
        n += 1;
        let (r, g, b) = (r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0);
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let v = max;
        let s = if max > 0.0 { (max - min) / max } else { 0.0 };
        if s < MIN_SAT || v < MIN_VAL {
            continue;
        }
        let (h, _, _) = crate::theme::to_hsl(Color::from_rgb(r, g, b));
        let bin = ((h / 360.0 * BINS as f32) as usize).min(BINS - 1);
        let w = s * v;
        weight[bin] += w;
        sum[bin][0] += r * w;
        sum[bin][1] += g * w;
        sum[bin][2] += b * w;
    }
    if n == 0 {
        return None;
    }
    // Neighbouring bins count towards a hue (a gradient across a bucket
    // edge shouldn't split its vote).
    let score = |i: usize| weight[i] + 0.5 * (weight[(i + BINS - 1) % BINS] + weight[(i + 1) % BINS]);
    let best = (0..BINS).max_by(|&a, &b| score(a).total_cmp(&score(b)))?;
    if weight[best] <= 0.0 || score(best) / (n as f32) < MIN_SHARE {
        return None;
    }
    let w = weight[best];
    Some(Color::from_rgb(sum[best][0] / w, sum[best][1] / w, sum[best][2] / w))
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    fn solid(w: u32, h: u32, c: [u8; 3]) -> RgbaImage {
        RgbaImage::from_pixel(w, h, Rgba([c[0], c[1], c[2], 255]))
    }

    fn hue(c: Color) -> f32 {
        crate::theme::to_hsl(c).0
    }

    #[test]
    fn red_cover_gives_red() {
        let c = from_image(&solid(64, 64, [200, 30, 40])).unwrap();
        assert!(hue(c) < 10.0 || hue(c) > 350.0, "{c:?}");
    }

    #[test]
    fn grey_and_black_covers_give_none() {
        assert_eq!(from_image(&solid(64, 64, [128, 128, 128])), None);
        assert_eq!(from_image(&solid(64, 64, [5, 5, 8])), None);
        assert_eq!(from_image(&solid(64, 64, [250, 250, 250])), None);
        assert_eq!(from_image(&RgbaImage::new(8, 8)), None); // fully transparent
    }

    #[test]
    fn vivid_patch_beats_dull_background() {
        // Mostly dark grey-brown with a bright blue square.
        let mut img = solid(64, 64, [60, 55, 50]);
        for y in 20..36 {
            for x in 20..36 {
                img.put_pixel(x, y, Rgba([30, 120, 230, 255]));
            }
        }
        let c = from_image(&img).unwrap();
        assert!((200.0..230.0).contains(&hue(c)), "{c:?}");
    }

    #[test]
    fn tiny_speck_of_colour_is_ignored() {
        let mut img = solid(64, 64, [128, 128, 128]);
        img.put_pixel(0, 0, Rgba([255, 0, 0, 255]));
        assert_eq!(from_image(&img), None);
    }

    #[test]
    fn readable_and_on_keep_contrast() {
        use crate::theme::{on, readable, to_hsl, WHITE};
        let navy = Color::from_rgb8(10, 20, 90);
        let (h0, _, _) = to_hsl(navy);
        // On dark: lifted to a readable lightness, hue kept.
        let d = readable(navy, true);
        let (h1, _, l1) = to_hsl(d);
        assert!((0.549..=0.721).contains(&l1), "{l1}"); // float round trip
        assert!((h0 - h1).abs() < 1.0);
        // On light: a pale yellow gets darkened.
        let (_, _, l2) = to_hsl(readable(Color::from_rgb8(255, 250, 180), false));
        assert!((0.359..=0.481).contains(&l2), "{l2}");
        // Button text: white on dark fills, black on light ones.
        assert_eq!(on(Color::from_rgb8(20, 40, 120)), WHITE);
        assert_ne!(on(Color::from_rgb8(250, 230, 120)), WHITE);
    }
}
