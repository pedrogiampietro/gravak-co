//! The HUD emblem reads "v0x" instead of "Conquer".
//!
//! The original lettering ("onquer", beside the dragon disc that forms the
//! "C") is painted into `data/main/mainDialog2.dds`. The client edits that
//! slice when it loads: the letters are wiped by copying the background's
//! diamond pattern over them, then "v0x" is set in the client's decorative
//! face (Cinzel Decorative): a gold gradient lined in navy, rimmed dark and
//! lit by a soft glow. Nothing of the original art is stored in the
//! repository.

use image::RgbaImage;

/// The background diamond pattern repeats every 19 rows.
const PATTERN_PERIOD: i64 = 19;
/// Where the "onquer" letters sit inside the 256x55 emblem slice.
const LETTERS: [u32; 4] = [95, 21, 214, 55];

/// Supersampling of the lettering before it is reduced to the slice.
const SCALE: usize = 4;
/// Size of the letters, and where their middle sits in the slice.
const LETTER_SIZE: f32 = 33.0;
const LETTER_CENTRE: [f32; 2] = [154.0, 33.0];
/// The "0" is narrowed so it reads as a digit, not the letter O.
const ZERO_WIDTH: f32 = 0.72;
const FILL_TOP: [u8; 3] = [255, 242, 196];
const FILL_BOTTOM: [u8; 3] = [196, 146, 64];
const LINE: [u8; 3] = [14, 30, 52];
const RIM: [u8; 3] = [40, 28, 10];
const GLOW: [u8; 3] = [255, 196, 100];

/// Replaces the lettering of the emblem slice (`mainDialog2.dds` rows
/// 64..119) with "v0x".
pub fn replace_lettering(slice: &mut RgbaImage) {
    let mask = lettering_mask(slice);
    wipe(slice, &mask);
    let Ok(font) = crate::labels::decorative_font() else {
        return;
    };
    letter_v0x(slice, &font);
}

/// The gold rim and pale fill of the letters, closed and grown so their navy
/// lining goes with them.
fn lettering_mask(slice: &RgbaImage) -> Vec<bool> {
    let (width, height) = slice.dimensions();
    let mut mask = vec![false; (width * height) as usize];
    for y in LETTERS[1]..LETTERS[3].min(height) {
        for x in LETTERS[0]..LETTERS[2].min(width) {
            let [r, g, b, _] = slice.get_pixel(x, y).0.map(i32::from);
            // The tail of the "q" below the line is a darker gold.
            let gold = if y >= 44 {
                r > 90 && r >= b + 5 && g > 70
            } else {
                r > 100 && r >= b + 10 && g > 85
            };
            let pale = r > 140 && g > 150 && b > 140;
            mask[(y * width + x) as usize] = gold || pale;
        }
    }
    let mask = grow(&mask, width, height, 2);
    let mask = shrink(&mask, width, height, 2);
    grow(&mask, width, height, 2)
}

fn grow(mask: &[bool], width: u32, height: u32, steps: u32) -> Vec<bool> {
    let mut mask = mask.to_vec();
    for _ in 0..steps {
        let previous = mask.clone();
        for y in 0..height as i64 {
            for x in 0..width as i64 {
                let set = [(0, 0), (1, 0), (-1, 0), (0, 1), (0, -1)].iter().any(|(dx, dy)| {
                    let (nx, ny) = (x + dx, y + dy);
                    nx >= 0
                        && ny >= 0
                        && nx < width as i64
                        && ny < height as i64
                        && previous[(ny * width as i64 + nx) as usize]
                });
                mask[(y * width as i64 + x) as usize] = set;
            }
        }
    }
    mask
}

fn shrink(mask: &[bool], width: u32, height: u32, steps: u32) -> Vec<bool> {
    let inverted: Vec<bool> = mask.iter().map(|set| !set).collect();
    grow(&inverted, width, height, steps)
        .into_iter()
        .map(|set| !set)
        .collect()
}

/// Covers the masked pixels with the background one pattern period above or
/// below, whichever is clear of the letters.
fn wipe(slice: &mut RgbaImage, mask: &[bool]) {
    let source = slice.clone();
    let (width, height) = slice.dimensions();
    for y in 0..height as i64 {
        for x in 0..width {
            if !mask[(y * width as i64 + x as i64) as usize] {
                continue;
            }
            let clear = [-1, 1, -2, 2]
                .into_iter()
                .map(|periods| y + periods * PATTERN_PERIOD)
                .find(|&sy| sy >= 0 && sy < height as i64 && !mask[(sy * width as i64 + x as i64) as usize]);
            if let Some(sy) = clear {
                slice.put_pixel(x, y as u32, *source.get_pixel(x, sy as u32));
            }
        }
    }
}

/// Coverage (0..=1) of the lettering in slice pixels, `width` x `height`.
fn lettering_coverage(font: &fontdue::Font, width: usize, height: usize) -> Vec<f32> {
    let size = LETTER_SIZE * SCALE as f32;
    // Each glyph, emboldened by 2 supersampled pixels as a stroke would.
    let glyphs: Vec<(usize, usize, Vec<f32>)> = [('v', 1.0), ('0', ZERO_WIDTH), ('x', 1.0)]
        .into_iter()
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
    let mut left = (LETTER_CENTRE[0] * SCALE as f32 - total as f32 / 2.0).max(0.0) as usize;
    // Bottoms on a common baseline.
    let baseline = (LETTER_CENTRE[1] * SCALE as f32 + tallest as f32 / 2.0) as usize;
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

/// Sets "v0x" over the wiped emblem: glow, dark rim, navy line, then the
/// letters in a gold gradient from their top to their bottom.
fn letter_v0x(slice: &mut RgbaImage, font: &fontdue::Font) {
    let (width, height) = (slice.width() as usize, slice.height() as usize);
    let letters = lettering_coverage(font, width, height);
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
