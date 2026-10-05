//! The HUD's emblem, bottom right: where the original shows its dragon disc
//! and "Conquer" (painted across `mainDialog2.dds` and the chat strip's
//! last piece), Gravak sets a plaque in the style of the rest of its
//! interface: navy in gold, a gold medallion with a "G" on it and steel
//! spikes round its rim like the Gravak relic's, and "Gravak" in the
//! client's decorative face (Cinzel Decorative) over a gold rule.

use image::RgbaImage;

/// The emblem's region on the composed 1024 x 141 HUD: from the "Skill"
/// button's right to the edge, from the chat strip's top to the bottom.
const REGION: [u32; 4] = [806, 66, 218, 75];

/// Supersampling of the lettering before it is reduced to the slice.
const SCALE: usize = 4;
/// Size of the letters, and where their middle sits in the region: right
/// of the medallion, on the plaque.
const LETTER_SIZE: f32 = 26.0;
const LETTER_CENTRE: [f32; 2] = [138.0, 35.0];
/// The name on the plaque.
const NAME: &str = "Gravak";
/// The plaque, in the region, and its medallion.
const PLAQUE: [i32; 4] = [2, 5, 212, 66];
const MEDALLION: ([f32; 2], f32) = ([36.0, 38.0], 24.0);
/// The "0" is narrowed so it reads as a digit, not the letter O.
const ZERO_WIDTH: f32 = 0.72;
const FILL_TOP: [u8; 3] = [255, 242, 196];
const FILL_BOTTOM: [u8; 3] = [196, 146, 64];
const LINE: [u8; 3] = [14, 30, 52];
const RIM: [u8; 3] = [40, 28, 10];
const GLOW: [u8; 3] = [255, 196, 100];

/// Draws the emblem over the composed HUD (1024 x 141): a plaque in the
/// style of the rest of Gravak's interface over the dragon disc, the
/// medallion with its "G", and "Gravak" beside it over a gold rule.
pub fn draw_emblem(hud: &mut RgbaImage) {
    let [x, y, width, height] = REGION;
    let mut slice = image::imageops::crop_imm(hud, x, y, width, height).to_image();
    draw_on(&mut slice);
    image::imageops::replace(hud, &slice, i64::from(x), i64::from(y));
}

fn draw_on(slice: &mut RgbaImage) {
    plaque(slice);
    let Ok(font) = crate::labels::decorative_font() else {
        medallion_base(slice);
        return;
    };
    monogram(slice, &font);
    letter(slice, &font, NAME, LETTER_SIZE, LETTER_CENTRE);
    // The rule under the letters, a diamond at its middle.
    let centre = LETTER_CENTRE[0] as i32;
    crate::daily_rewards_panel::rule(slice, centre - 60, centre + 60, 60);
}

fn medallion_bounds() -> crate::ui::UiRect {
    let (centre, r) = MEDALLION;
    crate::ui::UiRect::new((centre[0] - r) as i32 - 8, (centre[1] - r) as i32 - 8, (2.0 * r) as i32 + 17, (2.0 * r) as i32 + 17)
}

/// The medallion's gold ring and its deep blue face, lit at its middle.
fn medallion_base(slice: &mut RgbaImage) {
    use crate::daily_rewards_panel::{glow_in, paint_in, GOLD, GOLD_DARK, GOLD_LIGHT};
    use crate::paint;
    let (centre, r) = MEDALLION;
    let bounds = crate::ui::UiRect::new((centre[0] - r) as i32 - 3, (centre[1] - r) as i32 - 3, (2.0 * r) as i32 + 7, (2.0 * r) as i32 + 7);
    let circle = move |x: f32, y: f32, radius: f32| paint::circle(x, y, centre, radius);
    glow_in(slice, bounds, |x, y| circle(x, y, r), 3.0, paint::hex(0xffc85a), 0.3);
    paint_in(
        slice,
        bounds,
        1,
        |x, y| circle(x, y, r),
        |_, y| paint::gradient(&[(0.0, GOLD_LIGHT), (0.45, GOLD), (1.0, GOLD_DARK)], (y - centre[1] + r) / (2.0 * r)),
        1.0,
    );
    let face = r - 3.4;
    paint_in(
        slice,
        bounds,
        1,
        |x, y| circle(x, y, face),
        |x, y| {
            let distance = ((x - centre[0]).powi(2) + (y - centre[1] + 3.0).powi(2)).sqrt() / face;
            paint::mix(paint::hex(0x2a6d92), paint::hex(0x061620), distance.min(1.0))
        },
        1.0,
    );
    paint_in(slice, bounds, 1, |x, y| circle(x, y, face).abs() - 0.5, |_, _| GOLD_DARK, 1.0);
}

/// The medallion with a gold "G", steel spikes round its rim as on the
/// Gravak relic's.
fn monogram(slice: &mut RgbaImage, font: &fontdue::Font) {
    use crate::daily_rewards_panel::paint_in;
    use crate::paint;
    let (centre, r) = MEDALLION;
    let bounds = medallion_bounds();
    let [cx, cy] = centre;
    medallion_base(slice);
    // Steel spikes round the ring, as on the relic's rim.
    for index in 0..6 {
        let angle = (index as f32 * 60.0 + 30.0).to_radians();
        let (sin, cos) = angle.sin_cos();
        let base = r - 1.0;
        let tip = [cx + sin * (r + 8.0), cy - cos * (r + 8.0)];
        let left = [cx + (angle - 0.2).sin() * base, cy - (angle - 0.2).cos() * base];
        let right = [cx + (angle + 0.2).sin() * base, cy - (angle + 0.2).cos() * base];
        let spike = move |x: f32, y: f32| {
            // Inside the triangle: on the inner side of its three edges.
            let edge = |a: [f32; 2], b: [f32; 2]| {
                let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
                let length = (dx * dx + dy * dy).sqrt().max(f32::EPSILON);
                ((x - a[0]) * dy - (y - a[1]) * dx) / length
            };
            edge(left, tip).max(edge(tip, right)).max(edge(right, left))
        };
        paint_in(slice, bounds, 0, spike, |x, _| paint::mix(paint::hex(0xd6dee4), paint::hex(0x6c7a84), ((x - cx) / r + 1.0) / 2.0), 1.0);
    }
    letter(slice, font, "G", 27.0, [cx + 0.5, cy + 0.5]);
}

/// A rounded navy plaque in gold over the emblem, lit along its top.
fn plaque(slice: &mut RgbaImage) {
    use crate::daily_rewards_panel::{glow_in, paint_in, rounded, GOLD, GOLD_DARK, GOLD_LIGHT};
    use crate::paint;
    let [x, y, width, height] = PLAQUE;
    let rect = crate::ui::UiRect::new(x, y, width, height);
    let shape = rounded(rect, 12.0);
    glow_in(slice, rect, &shape, 3.0, paint::hex(0x000000), 0.55);
    let (y0, h) = (y as f32, height as f32);
    paint_in(slice, rect, 1, &shape, |_, py| paint::mix(paint::hex(0x15405a), paint::hex(0x040e15), (py - y0) / h), 1.0);
    paint_in(slice, rect, 1, |px, py| shape(px, py).abs() - 0.7, |_, _| GOLD, 1.0);
    let inner = crate::ui::UiRect::new(x + 3, y + 3, width - 6, height - 6);
    let inner_shape = rounded(inner, 9.5);
    paint_in(slice, inner, 0, |px, py| inner_shape(px, py).abs() - 0.4, |_, _| GOLD_DARK, 0.7);
    let shine = crate::ui::UiRect::new(x + 18, y + 2, width - 36, 2);
    let middle = (x + width / 2) as f32;
    paint_in(
        slice,
        shine,
        0,
        |_, py| (py - shine.y as f32 - 0.5).abs() - 0.5,
        |px, _| paint::mix(GOLD_LIGHT, paint::hex(0x15405a), ((px - middle).abs() / (width as f32 / 2.0)).powf(1.3)),
        0.55,
    );
}

/// Coverage (0..=1) of the lettering in slice pixels, `width` x `height`.
fn lettering_coverage(
    font: &fontdue::Font,
    text: &str,
    letter_size: f32,
    letter_centre: [f32; 2],
    width: usize,
    height: usize,
) -> Vec<f32> {
    let size = letter_size * SCALE as f32;
    // Each glyph, emboldened by 2 supersampled pixels as a stroke would.
    // The "0" is narrowed so it reads as a digit, not the letter O.
    let glyphs: Vec<(usize, usize, Vec<f32>)> = text
        .chars()
        .map(|character| (character, if character == '0' { ZERO_WIDTH } else { 1.0 }))
        .map(|(character, x_scale)| {
            let (metrics, bitmap) = font.rasterize(character, size);
            let pad = 2;
            let (w, h) = (metrics.width + pad * 2, metrics.height + pad * 2);
            let mut coverage = vec![0.0; w * h];
            for y in 0..metrics.height {
                for x in 0..metrics.width {
                    coverage[(y + pad) * w + x + pad] =
                        f32::from(bitmap[y * metrics.width + x]) / 255.0;
                }
            }
            let coverage = max_filter(&coverage, w, h, 2);
            let scaled_w = ((w as f32) * x_scale).round().max(1.0) as usize;
            let mut scaled = vec![0.0; scaled_w * h];
            for y in 0..h {
                for x in 0..scaled_w {
                    let source = ((x as f32 + 0.5) / x_scale - 0.5).clamp(0.0, (w - 1) as f32);
                    let (left, t) = (source.floor() as usize, source.fract());
                    let right = (left + 1).min(w - 1);
                    scaled[y * scaled_w + x] =
                        coverage[y * w + left] * (1.0 - t) + coverage[y * w + right] * t;
                }
            }
            (scaled_w, h, scaled)
        })
        .collect();
    let gap = SCALE;
    let total = glyphs.iter().map(|(w, _, _)| w).sum::<usize>() + gap * (glyphs.len() - 1);
    let tallest = glyphs.iter().map(|(_, h, _)| *h).max().unwrap_or(0);
    let (big_w, big_h) = (width * SCALE, height * SCALE);
    let mut big = vec![0.0_f32; big_w * big_h];
    let mut left = (letter_centre[0] * SCALE as f32 - total as f32 / 2.0).max(0.0) as usize;
    // Bottoms on a common baseline.
    let baseline = (letter_centre[1] * SCALE as f32 + tallest as f32 / 2.0) as usize;
    for (w, h, coverage) in &glyphs {
        let top = baseline.saturating_sub(*h);
        for y in 0..*h {
            for x in 0..*w {
                let (bx, by) = (left + x, top + y);
                if bx < big_w && by < big_h {
                    let cell = &mut big[by * big_w + bx];
                    *cell = cell.max(coverage[y * w + x]);
                }
            }
        }
        left += w + gap;
    }
    let mut coverage = vec![0.0; width * height];
    for y in 0..height {
        for x in 0..width {
            let mut sum = 0.0;
            for sy in 0..SCALE {
                for sx in 0..SCALE {
                    sum += big[(y * SCALE + sy) * big_w + x * SCALE + sx];
                }
            }
            coverage[y * width + x] = sum / (SCALE * SCALE) as f32;
        }
    }
    coverage
}

/// Each pixel takes the largest value within `radius` (a square window).
fn max_filter(values: &[f32], width: usize, height: usize, radius: usize) -> Vec<f32> {
    let mut out = vec![0.0; values.len()];
    for y in 0..height {
        for x in 0..width {
            let mut best = 0.0_f32;
            for ny in y.saturating_sub(radius)..(y + radius + 1).min(height) {
                for nx in x.saturating_sub(radius)..(x + radius + 1).min(width) {
                    best = best.max(values[ny * width + nx]);
                }
            }
            out[y * width + x] = best;
        }
    }
    out
}

/// Three box blurs, close to a Gaussian about `radius` wide.
fn blur(values: &[f32], width: usize, height: usize, radius: usize) -> Vec<f32> {
    let mut current = values.to_vec();
    for _ in 0..3 {
        let mut next = vec![0.0; current.len()];
        for y in 0..height {
            for x in 0..width {
                let (mut sum, mut count) = (0.0, 0.0);
                for ny in y.saturating_sub(radius)..(y + radius + 1).min(height) {
                    for nx in x.saturating_sub(radius)..(x + radius + 1).min(width) {
                        sum += current[ny * width + nx];
                        count += 1.0;
                    }
                }
                next[y * width + x] = sum / count;
            }
        }
        current = next;
    }
    current
}

fn blend(slice: &mut RgbaImage, x: usize, y: usize, colour: [u8; 3], alpha: f32) {
    if alpha <= 0.0 {
        return;
    }
    let alpha = alpha.min(1.0);
    let pixel = slice.get_pixel_mut(x as u32, y as u32);
    for channel in 0..3 {
        pixel[channel] = (f32::from(pixel[channel]) * (1.0 - alpha)
            + f32::from(colour[channel]) * alpha)
            .round() as u8;
    }
    pixel[3] = pixel[3].max((alpha * 255.0) as u8);
}

/// Sets `text` in gold: glow, dark rim, navy line, then the letters in a
/// gold gradient from their top to their bottom.
fn letter(slice: &mut RgbaImage, font: &fontdue::Font, text: &str, size: f32, centre: [f32; 2]) {
    let (width, height) = (slice.width() as usize, slice.height() as usize);
    let letters = lettering_coverage(font, text, size, centre, width, height);
    let line = max_filter(&letters, width, height, 1);
    let rim = max_filter(&letters, width, height, 2);
    let glow = blur(&rim, width, height, 3);
    let rows: Vec<usize> = (0..height)
        .filter(|&y| (0..width).any(|x| letters[y * width + x] > 0.16))
        .collect();
    let top = rows.first().copied().unwrap_or(0);
    let bottom = rows.last().copied().unwrap_or(height);
    for y in 0..height {
        let t = ((y as f32 - top as f32) / bottom.saturating_sub(top).max(1) as f32).clamp(0.0, 1.0);
        let fill: [u8; 3] = std::array::from_fn(|channel| {
            (f32::from(FILL_TOP[channel]) * (1.0 - t) + f32::from(FILL_BOTTOM[channel]) * t).round()
                as u8
        });
        for x in 0..width {
            let at = y * width + x;
            blend(slice, x, y, GLOW, glow[at] * 0.55);
            blend(slice, x, y, RIM, rim[at]);
            blend(slice, x, y, LINE, line[at]);
            blend(slice, x, y, fill, letters[at]);
        }
    }
}

#[cfg(test)]
mod emblem_preview {
    use super::*;

    /// `BOUNTY_PREVIEW=dir cargo test -p conquer-sandbox logo_emblem_preview`
    #[test]
    fn logo_emblem_preview() {
        let Ok(dir) = std::env::var("BOUNTY_PREVIEW") else { return };
        let [_, _, width, height] = REGION;
        let mut slice = RgbaImage::from_pixel(width, height, image::Rgba([14, 34, 46, 255]));
        draw_on(&mut slice);
        slice.save(format!("{dir}/logo-emblem.png")).expect("png");
    }
}
