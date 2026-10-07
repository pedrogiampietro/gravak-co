//! Gravak's crest, and the HUD's emblem made with it.
//!
//! The crest is a heater shield: a gold rim between dark lines round a
//! glossy blue face, and a bold gold "G" on it. It is the window's icon,
//! and on the HUD it takes the place of the original's dragon disc and
//! "Conquer" (painted across `mainDialog2.dds`): the disc gives way to the
//! bar's own diamond texture, and the crest and "GRAVAK" in gold lettering
//! stand on it, the way the original's logo is painted into the bar.

use image::RgbaImage;

/// The emblem's region on the composed 1024 x 141 HUD: from the "Skill"
/// button's right to the edge, from the chat strip's top to the bottom.
const REGION: [u32; 4] = [806, 66, 218, 75];
/// The clean texture right of the "Skill" button (x, width), repeated
/// over the dragon disc from `RETEXTURE_FROM` (in the region) on.
const TEXTURE_STRIP: [u32; 2] = [800, 26];
const RETEXTURE_FROM: u32 = 30;
/// Rows of the bar's gold top edge, kept as they are.
const TOP_EDGE: u32 = 8;
/// The crest on the HUD: its shield's height and where the shield's top
/// middle stands, in the region.
const HUD_CREST_HEIGHT: f32 = 58.0;
const HUD_CREST_AT: [f32; 2] = [34.0, 11.0];
const NAME: &str = "GRAVAK";
const NAME_SIZE: f32 = 23.0;
const NAME_CENTRE: [f32; 2] = [139.0, 41.0];
const NAME_SPACING: f32 = 2.2;

/// The face of the crest's letters (SIL Open Font License 1.1).
const KATEX_BOLD: &[u8] = include_bytes!("ui_assets/KaTeX_Main-Bold.ttf");

/// Supersampling of the lettering before it is reduced.
const SCALE: usize = 4;

fn font() -> Option<&'static fontdue::Font> {
    static FONT: std::sync::OnceLock<Option<fontdue::Font>> = std::sync::OnceLock::new();
    FONT.get_or_init(|| fontdue::Font::from_bytes(KATEX_BOLD, fontdue::FontSettings::default()).ok())
        .as_ref()
}

/// Draws the emblem over the composed HUD (1024 x 141).
pub fn draw_emblem(hud: &mut RgbaImage) {
    retexture(hud);
    let [x, y, width, height] = REGION;
    let mut slice = image::imageops::crop_imm(hud, x, y, width, height).to_image();
    let [cx, top] = HUD_CREST_AT;
    draw_crest(&mut slice, [cx, top], HUD_CREST_HEIGHT);
    if let Some(font) = font() {
        gold_letters(&mut slice, font, NAME, NAME_SIZE, NAME_CENTRE, NAME_SPACING);
    }
    image::imageops::replace(hud, &slice, i64::from(x), i64::from(y));
}

/// The crest alone, `size` pixels square, on a clear background: the
/// window's icon and the installer's.
pub fn crest_icon(size: u32) -> RgbaImage {
    let mut image = RgbaImage::new(size, size);
    let side = size as f32;
    // The shield as tall as the square allows, with room for its shadow.
    let height = side * 0.92;
    draw_crest(&mut image, [side / 2.0, side * 0.03], height);
    image
}

/// Covers the dragon disc and "Conquer" with the bar's own texture: the
/// clean strip right of "Skill", repeated, every other copy mirrored so
/// the seams match.
fn retexture(hud: &mut RgbaImage) {
    let [x, y, width, height] = REGION;
    let [strip_x, strip_width] = TEXTURE_STRIP;
    let strip = image::imageops::crop_imm(hud, strip_x, y, strip_width, height).to_image();
    let mirrored = image::imageops::flip_horizontal(&strip);
    let mut at = RETEXTURE_FROM;
    let mut flip = false;
    while at < width {
        let source = if flip { &mirrored } else { &strip };
        for sy in TOP_EDGE..height {
            for sx in 0..strip_width.min(width - at) {
                hud.put_pixel(x + at + sx, y + sy, *source.get_pixel(sx, sy));
            }
        }
        at += strip_width;
        flip = !flip;
    }
}

/// The shield's outline, its top middle at `[cx, top]`, `height` tall:
/// a flat top with raised corners and a small peak, straight sides, then
/// curves meeting in the point.
fn shield(cx: f32, top: f32, height: f32) -> Vec<[f32; 2]> {
    let half = height * 0.79 / 2.0;
    let mut points = vec![
        [cx - half, top],
        [cx - half * 0.5, top + height * 0.045],
        [cx, top - height * 0.02],
        [cx + half * 0.5, top + height * 0.045],
        [cx + half, top],
    ];
    let side = top + height * 0.42;
    let bezier = |p0: [f32; 2], p1: [f32; 2], p2: [f32; 2], t: f32| {
        let u = 1.0 - t;
        [
            u * u * p0[0] + 2.0 * u * t * p1[0] + t * t * p2[0],
            u * u * p0[1] + 2.0 * u * t * p1[1] + t * t * p2[1],
        ]
    };
    let point = [cx, top + height];
    for step in 0..=24 {
        let t = step as f32 / 24.0;
        points.push(bezier([cx + half, side], [cx + half * 0.98, top + height * 0.8], point, t));
    }
    for step in 1..=24 {
        let t = step as f32 / 24.0;
        points.push(bezier(point, [cx - half * 0.98, top + height * 0.8], [cx - half, side], t));
    }
    points
}

/// Signed distance from `p` to the polygon (negative inside).
fn polygon_distance(points: &[[f32; 2]], p: [f32; 2]) -> f32 {
    let mut distance = f32::MAX;
    let mut inside = false;
    for index in 0..points.len() {
        let a = points[index];
        let b = points[(index + 1) % points.len()];
        let edge = [b[0] - a[0], b[1] - a[1]];
        let offset = [p[0] - a[0], p[1] - a[1]];
        let length = edge[0] * edge[0] + edge[1] * edge[1];
        let t = if length > 0.0 {
            ((offset[0] * edge[0] + offset[1] * edge[1]) / length).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let (dx, dy) = (offset[0] - edge[0] * t, offset[1] - edge[1] * t);
        distance = distance.min((dx * dx + dy * dy).sqrt());
        if (a[1] > p[1]) != (b[1] > p[1]) && p[0] < a[0] + (p[1] - a[1]) / (b[1] - a[1]) * edge[0] {
            inside = !inside;
        }
    }
    if inside { -distance } else { distance }
}

fn mix(from: [f32; 3], to: [f32; 3], t: f32) -> [f32; 3] {
    let t = t.clamp(0.0, 1.0);
    std::array::from_fn(|channel| from[channel] + (to[channel] - from[channel]) * t)
}

fn blend(image: &mut RgbaImage, x: u32, y: u32, colour: [f32; 3], alpha: f32) {
    if alpha <= 0.0 {
        return;
    }
    let alpha = alpha.min(1.0);
    let pixel = image.get_pixel_mut(x, y);
    let below = f32::from(pixel[3]) / 255.0;
    let out = alpha + below * (1.0 - alpha);
    for channel in 0..3 {
        let value = (colour[channel] * alpha + f32::from(pixel[channel]) * below * (1.0 - alpha)) / out.max(f32::EPSILON);
        pixel[channel] = value.round().clamp(0.0, 255.0) as u8;
    }
    pixel[3] = (out * 255.0).round() as u8;
}

/// Coverage of the area `inset` pixels inside the outline, for a pixel at
/// signed distance `distance`.
fn inside(distance: f32, inset: f32) -> f32 {
    (0.5 - (distance + inset)).clamp(0.0, 1.0)
}

/// The crest: its shield's top middle at `[cx, top]`, `height` tall.
fn draw_crest(image: &mut RgbaImage, [cx, top]: [f32; 2], height: f32) {
    let unit = height / 58.0;
    let points = shield(cx, top, height);
    let half = height * 0.79 / 2.0;
    let face_centre = [cx, top + height * 0.33];
    // The gloss: an ellipse over the upper left.
    let gloss_centre = [cx - half * 0.55, top - height * 0.065];
    let gloss_radius = [half * 1.25, height * 0.485];
    for y in 0..image.height() {
        for x in 0..image.width() {
            let p = [x as f32 + 0.5, y as f32 + 0.5];
            // Its shadow, a little below.
            let shadow = polygon_distance(&points, [p[0], p[1] - 1.5 * unit]);
            if shadow > -2.0 * unit && shadow < 3.5 * unit {
                let fade = 1.0 - (shadow / (3.5 * unit)).clamp(0.0, 1.0);
                blend(image, x, y, [0.0; 3], 0.6 * fade * fade);
            }
            let distance = polygon_distance(&points, p);
            if distance > 0.5 {
                continue;
            }
            let t = ((p[1] - top) / height).clamp(0.0, 1.0);
            blend(image, x, y, [30.0, 20.0, 6.0], inside(distance, 0.0));
            let gold = mix([255.0, 236.0, 160.0], [150.0, 96.0, 30.0], t);
            blend(image, x, y, gold, inside(distance, 1.0 * unit));
            blend(image, x, y, [40.0, 26.0, 8.0], inside(distance, 3.4 * unit));
            let face = inside(distance, 4.4 * unit);
            if face > 0.0 {
                let reach = (((p[0] - face_centre[0]) / (half * 1.5)).powi(2)
                    + ((p[1] - face_centre[1]) / (height * 0.8)).powi(2))
                .sqrt();
                blend(image, x, y, mix([80.0, 165.0, 245.0], [8.0, 32.0, 90.0], reach), face);
                let gloss = ((p[0] - gloss_centre[0]) / gloss_radius[0]).powi(2)
                    + ((p[1] - gloss_centre[1]) / gloss_radius[1]).powi(2);
                let shine = ((1.0 - gloss) / 0.15).clamp(0.0, 1.0);
                blend(image, x, y, [255.0; 3], 0.27 * shine * face);
            }
        }
    }
    if let Some(font) = font() {
        gold_letters(image, font, "G", height * 0.59, [cx, top + height * 0.466], 0.0);
    }
}

/// Coverage (0..=1) of `text` in pixels of a `width` x `height` image,
/// centred on `centre`, `spacing` pixels between letters.
fn lettering_coverage(
    font: &fontdue::Font,
    text: &str,
    letter_size: f32,
    centre: [f32; 2],
    spacing: f32,
    width: usize,
    height: usize,
) -> Vec<f32> {
    let size = letter_size * SCALE as f32;
    let glyphs: Vec<(fontdue::Metrics, Vec<u8>)> = text.chars().map(|character| font.rasterize(character, size)).collect();
    let gap = spacing * SCALE as f32;
    let total = glyphs.iter().map(|(metrics, _)| metrics.advance_width).sum::<f32>() + gap * (glyphs.len().saturating_sub(1)) as f32;
    // Bottoms on a common baseline, the capitals' middle on `centre`.
    let cap = glyphs.iter().map(|(metrics, _)| metrics.height as i32 + metrics.ymin).max().unwrap_or(0) as f32;
    let baseline = centre[1] * SCALE as f32 + cap / 2.0;
    let (big_w, big_h) = (width * SCALE, height * SCALE);
    let mut big = vec![0.0_f32; big_w * big_h];
    let mut pen = centre[0] * SCALE as f32 - total / 2.0;
    for (metrics, bitmap) in &glyphs {
        let left = (pen + metrics.xmin as f32).round() as i64;
        let top = (baseline - metrics.height as f32 - metrics.ymin as f32).round() as i64;
        for gy in 0..metrics.height {
            for gx in 0..metrics.width {
                let (bx, by) = (left + gx as i64, top + gy as i64);
                if bx >= 0 && by >= 0 && (bx as usize) < big_w && (by as usize) < big_h {
                    let cell = &mut big[by as usize * big_w + bx as usize];
                    *cell = cell.max(f32::from(bitmap[gy * metrics.width + gx]) / 255.0);
                }
            }
        }
        pen += metrics.advance_width + gap;
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

/// Sets `text` in polished gold: a soft shadow, a dark outline, the
/// letters from pale gold at their top to deep gold at their bottom with a
/// bright band above the middle, and a lit top left edge. The outline and
/// the shadow grow with the letters past the HUD's sizes.
fn gold_letters(image: &mut RgbaImage, font: &fontdue::Font, text: &str, size: f32, centre: [f32; 2], spacing: f32) {
    let (width, height) = (image.width() as usize, image.height() as usize);
    let weight = (size / 34.0).round().max(1.0) as usize;
    let letters = lettering_coverage(font, text, size, centre, spacing, width, height);
    let near = max_filter(&letters, width, height, weight);
    let far = max_filter(&letters, width, height, weight + weight.div_ceil(2));
    let ring: Vec<f32> = near.iter().zip(&far).map(|(near, far)| near.max(far * 0.55)).collect();
    let shadow = blur(&ring, width, height, weight + 1);
    let rows: Vec<usize> = (0..height)
        .filter(|&y| (0..width).any(|x| letters[y * width + x] > 0.16))
        .collect();
    let (Some(&first), Some(&last)) = (rows.first(), rows.last()) else {
        return;
    };
    let at = |x: usize, y: usize| letters[y * width + x];
    for y in 0..height {
        let t = ((y as f32 - first as f32) / (last - first).max(1) as f32).clamp(0.0, 1.0);
        let band = (1.0 - (t - 0.38).abs() * 1.6).max(0.0) * 30.0;
        let fill = mix([255.0, 244.0, 200.0], [206.0, 140.0, 40.0], t).map(|channel| (channel + band).min(255.0));
        for x in 0..width {
            let index = y * width + x;
            // The shadow falls a little right and down.
            if x >= weight && y >= weight {
                blend(image, x as u32, y as u32, [0.0; 3], shadow[(y - weight) * width + x - weight] * 0.8);
            }
            blend(image, x as u32, y as u32, [28.0, 16.0, 4.0], ring[index]);
            blend(image, x as u32, y as u32, fill, letters[index]);
            let lit = if x >= weight && y >= weight { at(x, y) * (1.0 - at(x - weight, y - weight)) } else { 0.0 };
            blend(image, x as u32, y as u32, [255.0, 252.0, 230.0], lit * 0.9);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shield_is_closed_and_its_middle_inside() {
        let points = shield(30.0, 5.0, 58.0);
        assert!(polygon_distance(&points, [30.0, 30.0]) < -10.0);
        assert!(polygon_distance(&points, [2.0, 60.0]) > 0.0);
        // The point is at the bottom middle.
        assert!(points.iter().any(|point| (point[1] - 63.0).abs() < 0.01 && (point[0] - 30.0).abs() < 0.01));
    }

    #[test]
    fn the_icon_keeps_its_corners_clear() {
        let icon = crest_icon(64);
        assert_eq!(icon.get_pixel(0, 63)[3], 0);
        assert_eq!(icon.get_pixel(63, 63)[3], 0);
        assert!(icon.get_pixel(32, 30)[3] == 255);
    }

    /// Navy behind the crest for the profile pictures: lighter in the
    /// middle, deep at the edges.
    fn navy(width: u32, height: u32) -> RgbaImage {
        RgbaImage::from_fn(width, height, |x, y| {
            let (dx, dy) = (x as f32 / width as f32 - 0.5, y as f32 / height as f32 - 0.45);
            let reach = ((dx * dx + dy * dy).sqrt() * 1.7).min(1.0);
            let [r, g, b] = mix([30.0, 66.0, 104.0], [5.0, 14.0, 26.0], reach);
            image::Rgba([r as u8, g as u8, b as u8, 255])
        })
    }

    /// `GRAVAK_BRAND=dir cargo test --release -p conquer-sandbox brand_kit`:
    /// the crest and the name for the community's pages (Instagram,
    /// Discord): transparent crest, square avatars kept inside the circle
    /// those sites cut, the crest with the name, and a banner.
    #[test]
    fn brand_kit() {
        let Ok(dir) = std::env::var("GRAVAK_BRAND") else { return };
        let font = font().expect("font");
        crest_icon(1024).save(format!("{dir}/gravak-crest-1024.png")).expect("png");
        for (size, name) in [(1080, "gravak-instagram-1080"), (512, "gravak-discord-512")] {
            let mut avatar = navy(size, size);
            let side = size as f32;
            draw_crest(&mut avatar, [side / 2.0, side * 0.17], side * 0.64);
            avatar.save(format!("{dir}/{name}.png")).expect("png");
        }
        // The crest with "GRAVAK" beside it, on a clear background.
        let mut wordmark = RgbaImage::new(1600, 480);
        draw_crest(&mut wordmark, [250.0, 40.0], 400.0);
        gold_letters(&mut wordmark, font, NAME, 190.0, [1000.0, 250.0], 18.0);
        wordmark.save(format!("{dir}/gravak-wordmark.png")).expect("png");
        // A banner (Discord's 960 x 540 crops it well too).
        let mut banner = navy(1920, 1080);
        draw_crest(&mut banner, [960.0, 150.0], 520.0);
        gold_letters(&mut banner, font, NAME, 150.0, [960.0, 860.0], 16.0);
        banner.save(format!("{dir}/gravak-banner-1920x1080.png")).expect("png");
    }

    /// `BOUNTY_PREVIEW=dir cargo test -p conquer-sandbox crest_preview`:
    /// the crest at the icon sizes, and the installer's icon.
    #[test]
    fn crest_preview() {
        let Ok(dir) = std::env::var("BOUNTY_PREVIEW") else { return };
        for size in [16, 24, 32, 48, 64, 128, 256] {
            crest_icon(size).save(format!("{dir}/crest-{size}.png")).expect("png");
        }
    }
}
