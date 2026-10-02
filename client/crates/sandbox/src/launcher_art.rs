//! Painting for the login screens: the night-teal backdrop with drifting
//! embers, the gold "Gravak" title, ornate gold-framed panels, buttons,
//! text fields and the profession emblems. Everything is drawn by the
//! client; nothing comes from the game's art.

use image::{Pixel, Rgba, RgbaImage};
use std::{cell::RefCell, collections::HashMap};

pub const GOLD: [u8; 4] = [232, 206, 148, 255];
pub const GOLD_BRIGHT: [u8; 4] = [255, 236, 184, 255];
pub const GOLD_DEEP: [u8; 4] = [176, 136, 66, 255];
pub const TEAL_RULE: [u8; 4] = [82, 128, 136, 255];
pub const WHITE: [u8; 4] = [240, 240, 236, 255];
pub const GREY: [u8; 4] = [150, 164, 166, 255];
pub const DIM: [u8; 4] = [104, 128, 132, 255];

const FILL_TOP: [f32; 3] = [255.0, 244.0, 204.0];
const FILL_BOTTOM: [f32; 3] = [196.0, 142.0, 58.0];

thread_local! {
    static BACKDROPS: RefCell<HashMap<[u32; 2], RgbaImage>> = RefCell::new(HashMap::new());
    static TITLES: RefCell<HashMap<(String, u32), RgbaImage>> = RefCell::new(HashMap::new());
    static EMBLEMS: RefCell<HashMap<(usize, u32, bool), RgbaImage>> = RefCell::new(HashMap::new());
}

// ---------------------------------------------------------------------------
// Pixels

pub fn blend(image: &mut RgbaImage, x: i32, y: i32, colour: [u8; 4]) {
    if x >= 0 && y >= 0 && (x as u32) < image.width() && (y as u32) < image.height() {
        image.get_pixel_mut(x as u32, y as u32).blend(&Rgba(colour));
    }
}

fn with_alpha(colour: [u8; 4], alpha: f32) -> [u8; 4] {
    [
        colour[0],
        colour[1],
        colour[2],
        (f32::from(colour[3]) * alpha.clamp(0.0, 1.0)).round() as u8,
    ]
}

fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [u8; 3] {
    let t = t.clamp(0.0, 1.0);
    [
        (a[0] + (b[0] - a[0]) * t) as u8,
        (a[1] + (b[1] - a[1]) * t) as u8,
        (a[2] + (b[2] - a[2]) * t) as u8,
    ]
}

pub fn fill(image: &mut RgbaImage, rect: [i32; 4], colour: [u8; 4]) {
    let [x, y, width, height] = rect;
    for py in y.max(0)..(y + height).min(image.height() as i32) {
        for px in x.max(0)..(x + width).min(image.width() as i32) {
            image
                .get_pixel_mut(px as u32, py as u32)
                .blend(&Rgba(colour));
        }
    }
}

/// Vertical gradient from `top` to `bottom`.
pub fn gradient(image: &mut RgbaImage, rect: [i32; 4], top: [u8; 4], bottom: [u8; 4]) {
    let [x, y, width, height] = rect;
    for row in 0..height.max(0) {
        let t = row as f32 / (height - 1).max(1) as f32;
        let lerp = |a: u8, b: u8| (f32::from(a) + (f32::from(b) - f32::from(a)) * t).round() as u8;
        let colour = [
            lerp(top[0], bottom[0]),
            lerp(top[1], bottom[1]),
            lerp(top[2], bottom[2]),
            lerp(top[3], bottom[3]),
        ];
        fill(image, [x, y + row, width, 1], colour);
    }
}

pub fn outline(image: &mut RgbaImage, rect: [i32; 4], colour: [u8; 4]) {
    let [x, y, width, height] = rect;
    fill(image, [x, y, width, 1], colour);
    fill(image, [x, y + height - 1, width, 1], colour);
    fill(image, [x, y + 1, 1, height - 2], colour);
    fill(image, [x + width - 1, y + 1, 1, height - 2], colour);
}

/// A soft halo around `rect`, fading over `radius` pixels.
pub fn glow(image: &mut RgbaImage, rect: [i32; 4], radius: i32, colour: [u8; 4]) {
    for step in 1..=radius {
        let alpha = (1.0 - step as f32 / (radius + 1) as f32).powi(2) * 0.6;
        outline(
            image,
            [
                rect[0] - step,
                rect[1] - step,
                rect[2] + step * 2,
                rect[3] + step * 2,
            ],
            with_alpha(colour, alpha),
        );
    }
}

/// A round halo of `radius` pixels centred on (cx, cy).
pub fn radial_glow(image: &mut RgbaImage, cx: i32, cy: i32, radius: i32, colour: [u8; 4]) {
    for dy in -radius..=radius {
        for dx in -radius..=radius {
            let d = ((dx * dx + dy * dy) as f32).sqrt() / radius as f32;
            if d < 1.0 {
                blend(
                    image,
                    cx + dx,
                    cy + dy,
                    with_alpha(colour, (1.0 - d).powi(2)),
                );
            }
        }
    }
}

/// Paints the pixels of `rect` for which `shape` (in pixel space) is
/// inside, antialiased with 4x4 supersampling.
fn shape(
    image: &mut RgbaImage,
    rect: [i32; 4],
    colour: impl Fn(f32, f32) -> [u8; 4],
    inside: impl Fn(f32, f32) -> bool,
) {
    const SAMPLES: i32 = 4;
    let [x0, y0, width, height] = rect;
    for py in y0..y0 + height {
        for px in x0..x0 + width {
            let mut hits = 0;
            for sy in 0..SAMPLES {
                for sx in 0..SAMPLES {
                    let fx = px as f32 + (sx as f32 + 0.5) / SAMPLES as f32;
                    let fy = py as f32 + (sy as f32 + 0.5) / SAMPLES as f32;
                    if inside(fx, fy) {
                        hits += 1;
                    }
                }
            }
            if hits > 0 {
                let colour = colour(px as f32 + 0.5, py as f32 + 0.5);
                blend(
                    image,
                    px,
                    py,
                    with_alpha(colour, hits as f32 / (SAMPLES * SAMPLES) as f32),
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Geometry in emblem space: the unit disc, y pointing down.

type Point = (f32, f32);

fn segment_distance(p: Point, a: Point, b: Point) -> f32 {
    let (px, py) = (p.0 - a.0, p.1 - a.1);
    let (bx, by) = (b.0 - a.0, b.1 - a.1);
    let t = ((px * bx + py * by) / (bx * bx + by * by)).clamp(0.0, 1.0);
    ((px - bx * t).powi(2) + (py - by * t).powi(2)).sqrt()
}

fn capsule(p: Point, a: Point, b: Point, radius: f32) -> bool {
    segment_distance(p, a, b) <= radius
}

fn circle(p: Point, centre: Point, radius: f32) -> bool {
    (p.0 - centre.0).powi(2) + (p.1 - centre.1).powi(2) <= radius * radius
}

fn polygon(p: Point, points: &[Point]) -> bool {
    let mut inside = false;
    let mut j = points.len() - 1;
    for i in 0..points.len() {
        let (a, b) = (points[i], points[j]);
        if (a.1 > p.1) != (b.1 > p.1) && p.0 < (b.0 - a.0) * (p.1 - a.1) / (b.1 - a.1) + a.0 {
            inside = !inside;
        }
        j = i;
    }
    inside
}

fn rotate(p: Point, angle: f32) -> Point {
    let (sin, cos) = angle.sin_cos();
    (p.0 * cos - p.1 * sin, p.0 * sin + p.1 * cos)
}

/// A sword pointing up, hilt at the bottom.
fn sword(p: Point) -> bool {
    polygon(
        p,
        &[
            (-0.075, 0.22),
            (-0.075, -0.5),
            (0.0, -0.66),
            (0.075, -0.5),
            (0.075, 0.22),
        ],
    ) || capsule(p, (-0.27, 0.26), (0.27, 0.26), 0.05)
        || capsule(p, (0.0, 0.3), (0.0, 0.5), 0.04)
        || circle(p, (0.0, 0.56), 0.07)
}

/// The profession symbols: crossed swords, a shield, a drawn bow and the
/// Tao.
fn symbol(class: usize, p: Point) -> bool {
    match class {
        0 => sword(rotate(p, -0.62)) || sword(rotate(p, 0.62)),
        1 => {
            let outer = [
                (-0.48, -0.52),
                (0.48, -0.52),
                (0.48, 0.02),
                (0.0, 0.66),
                (-0.48, 0.02),
            ];
            let inner = [
                (-0.36, -0.41),
                (0.36, -0.41),
                (0.36, -0.0),
                (0.0, 0.5),
                (-0.36, -0.0),
            ];
            let cross = capsule(p, (0.0, -0.36), (0.0, 0.44), 0.06)
                || capsule(p, (-0.3, -0.1), (0.3, -0.1), 0.06);
            polygon(p, &outer) && (!polygon(p, &inner) || cross)
        }
        2 => {
            let (dx, dy) = (p.0 - 0.42, p.1);
            let distance = (dx * dx + dy * dy).sqrt();
            let limb = (distance - 0.86).abs() <= 0.055 && dx < -0.2 && dy.abs() < 0.66;
            let string = capsule(p, (-0.2, -0.64), (-0.2, 0.64), 0.018);
            let shaft = capsule(p, (-0.62, 0.0), (0.5, 0.0), 0.028);
            let head = polygon(p, &[(0.5, -0.1), (0.7, 0.0), (0.5, 0.1)]);
            let fletch = polygon(
                p,
                &[(-0.66, -0.12), (-0.5, 0.0), (-0.66, 0.12), (-0.56, 0.0)],
            );
            limb || string || shaft || head || fletch
        }
        _ => {
            let r = 0.6;
            if !circle(p, (0.0, 0.0), r) {
                return false;
            }
            if circle(p, (0.0, -r / 2.0), r / 8.0) {
                return false;
            }
            if circle(p, (0.0, r / 2.0), r / 8.0) {
                return true;
            }
            if circle(p, (0.0, -r / 2.0), r / 2.0) {
                return true;
            }
            if circle(p, (0.0, r / 2.0), r / 2.0) {
                return false;
            }
            p.0 < 0.0
        }
    }
}

/// A round medallion with the profession's symbol, `size` pixels across.
pub fn emblem(class: usize, size: u32, lit: bool) -> RgbaImage {
    EMBLEMS.with(|cache| {
        cache
            .borrow_mut()
            .entry((class, size, lit))
            .or_insert_with(|| paint_emblem(class, size, lit))
            .clone()
    })
}

fn paint_emblem(class: usize, size: u32, lit: bool) -> RgbaImage {
    let mut image = RgbaImage::new(size, size);
    let half = size as f32 / 2.0;
    let unit = |x: f32, y: f32| ((x - half) / (half - 1.0), (y - half) / (half - 1.0));
    let rect = [0, 0, size as i32, size as i32];
    // Disc: deep teal, lighter towards the top.
    shape(
        &mut image,
        rect,
        |_, y| {
            let t = y / size as f32;
            let c = mix([46.0, 92.0, 104.0], [10.0, 28.0, 36.0], t);
            [c[0], c[1], c[2], 255]
        },
        |x, y| circle(unit(x, y), (0.0, 0.0), 1.0),
    );
    // Rim: double gold ring.
    let gold = |_: f32, y: f32| {
        let c = mix(FILL_TOP, FILL_BOTTOM, y / size as f32);
        [c[0], c[1], c[2], 255]
    };
    shape(&mut image, rect, gold, |x, y| {
        let p = unit(x, y);
        let d = (p.0 * p.0 + p.1 * p.1).sqrt();
        (0.9..=1.0).contains(&d) || (0.8..=0.83).contains(&d)
    });
    if lit {
        shape(
            &mut image,
            rect,
            |_, _| [255, 220, 140, 60],
            |x, y| circle(unit(x, y), (0.0, 0.0), 0.8),
        );
    }
    // Symbol: dark shadow then gold.
    let scale = 0.8;
    shape(
        &mut image,
        rect,
        |_, _| [4, 12, 16, 200],
        |x, y| {
            let p = unit(x, y);
            symbol(class, ((p.0 - 0.04) / scale, (p.1 - 0.05) / scale))
        },
    );
    shape(&mut image, rect, gold, |x, y| {
        let p = unit(x, y);
        symbol(class, (p.0 / scale, p.1 / scale))
    });
    image
}

// ---------------------------------------------------------------------------
// Backdrop

/// The static backdrop: night-teal gradient, a warm glow behind the title,
/// a faint diamond lattice and a vignette.
pub fn backdrop(size: [u32; 2]) -> RgbaImage {
    BACKDROPS.with(|cache| {
        let mut cache = cache.borrow_mut();
        if cache.len() > 4 {
            cache.clear();
        }
        cache
            .entry(size)
            .or_insert_with(|| paint_backdrop(size))
            .clone()
    })
}

fn paint_backdrop(size: [u32; 2]) -> RgbaImage {
    let [width, height] = size;
    let mut image = RgbaImage::new(width, height);
    let (w, h) = (width as f32, height as f32);
    let glow_centre = (w * 0.5, h * 0.2);
    for y in 0..height {
        for x in 0..width {
            let (fx, fy) = (x as f32, y as f32);
            let t = fy / h;
            let mut c = [6.0 + 14.0 * t, 16.0 + 30.0 * t, 24.0 + 34.0 * t];
            // Warm glow behind the title.
            let gd = (((fx - glow_centre.0) / (w * 0.42)).powi(2)
                + ((fy - glow_centre.1) / (h * 0.3)).powi(2))
            .sqrt();
            let g = (1.0 - gd).max(0.0).powi(2) * 0.55;
            c[0] += 80.0 * g;
            c[1] += 58.0 * g;
            c[2] += 22.0 * g;
            // Diamond lattice, like the game panels' background.
            let u = (fx + fy) / 38.0;
            let v = (fx - fy) / 38.0;
            let line = ((u - u.round()).abs().min((v - v.round()).abs())) * 38.0;
            if line < 0.7 {
                c[0] += 7.0;
                c[1] += 11.0;
                c[2] += 12.0;
            }
            // Vignette.
            let vd = (((fx / w) - 0.5).powi(2) + ((fy / h) - 0.5).powi(2)).sqrt();
            let vignette = 1.0 - (vd - 0.35).max(0.0) * 1.4;
            let px = [
                (c[0] * vignette).clamp(0.0, 255.0) as u8,
                (c[1] * vignette).clamp(0.0, 255.0) as u8,
                (c[2] * vignette).clamp(0.0, 255.0) as u8,
                255,
            ];
            image.put_pixel(x, y, Rgba(px));
        }
    }
    image
}

/// Embers drifting up through the backdrop.
pub fn embers(image: &mut RgbaImage, time: f32) {
    let (w, h) = (image.width() as f32, image.height() as f32);
    for index in 0..70u32 {
        let seed = |k: u32| {
            let mut v = index
                .wrapping_mul(2_654_435_761)
                .wrapping_add(k.wrapping_mul(40_503));
            v ^= v >> 13;
            v = v.wrapping_mul(1_274_126_177);
            (v >> 8) as f32 / (1u32 << 24) as f32
        };
        let speed = 12.0 + seed(1) * 26.0;
        let travel = h + 60.0;
        let y = h + 30.0 - ((time * speed + seed(2) * travel) % travel);
        let x = seed(3) * w + (time * (0.3 + seed(4)) + seed(5) * 6.3).sin() * 18.0;
        let life = 1.0 - ((h - y) / h).clamp(0.0, 1.0);
        let twinkle = 0.6 + 0.4 * (time * (2.0 + seed(6) * 3.0) + seed(7) * 6.3).sin();
        let alpha = (life * twinkle * (0.35 + seed(8) * 0.5)).clamp(0.0, 1.0);
        let radius = 1.0 + seed(9) * 1.6;
        let colour = if seed(10) < 0.7 {
            [255, 196, 110, 255]
        } else {
            [150, 220, 230, 255]
        };
        let r = radius.ceil() as i32 + 2;
        for dy in -r..=r {
            for dx in -r..=r {
                let d = ((dx * dx + dy * dy) as f32).sqrt();
                let falloff = (1.0 - d / (radius + 2.0)).max(0.0).powi(2);
                if falloff > 0.0 {
                    blend(
                        image,
                        x as i32 + dx,
                        y as i32 + dy,
                        with_alpha(colour, alpha * falloff),
                    );
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Text

/// Coverage mask of `text` in `font` at `size`: (width, height, alpha).
fn mask(font: &fontdue::Font, text: &str, size: f32) -> (usize, usize, Vec<f32>) {
    let metrics = font.horizontal_line_metrics(size);
    let ascent = metrics.map_or(size, |m| m.ascent).ceil() as i32;
    let descent = metrics.map_or(0.0, |m| -m.descent).ceil() as i32;
    let height = (ascent + descent + 2).max(1) as usize;
    let width = text
        .chars()
        .map(|c| font.metrics(c, size).advance_width)
        .sum::<f32>()
        .ceil() as usize
        + 4;
    let mut alpha = vec![0.0f32; width * height];
    let mut pen = 1.0f32;
    for c in text.chars() {
        let (glyph, bitmap) = font.rasterize(c, size);
        let left = (pen + glyph.xmin as f32).round() as i32;
        let top = ascent - glyph.ymin - glyph.height as i32 + 1;
        for gy in 0..glyph.height {
            for gx in 0..glyph.width {
                let (x, y) = (left + gx as i32, top + gy as i32);
                if x >= 0 && y >= 0 && (x as usize) < width && (y as usize) < height {
                    let value = f32::from(bitmap[gy * glyph.width + gx]) / 255.0;
                    let cell = &mut alpha[y as usize * width + x as usize];
                    *cell = (*cell).max(value);
                }
            }
        }
        pen += glyph.advance_width;
    }
    (width, height, alpha)
}

/// Gold lettering with a dark rim and a warm glow, centred on `centre_x`.
pub fn gold_title(image: &mut RgbaImage, text: &str, size: f32, centre_x: i32, y: i32) {
    let rendered = TITLES.with(|cache| {
        cache
            .borrow_mut()
            .entry((text.to_owned(), size as u32))
            .or_insert_with(|| paint_title(text, size))
            .clone()
    });
    let left = centre_x - rendered.width() as i32 / 2;
    image::imageops::overlay(image, &rendered, i64::from(left), i64::from(y));
}

fn paint_title(text: &str, size: f32) -> RgbaImage {
    let Ok(font) = crate::labels::decorative_font() else {
        return RgbaImage::new(1, 1);
    };
    let (w, h, alpha) = mask(&font, text, size);
    let pad = (size * 0.4) as usize;
    let (width, height) = (w + pad * 2, h + pad * 2);
    let at = |x: i64, y: i64| -> f32 {
        let (x, y) = (x - pad as i64, y - pad as i64);
        if x < 0 || y < 0 || x as usize >= w || y as usize >= h {
            0.0
        } else {
            alpha[y as usize * w + x as usize]
        }
    };
    let mut image = RgbaImage::new(width as u32, height as u32);
    // Glow: a wide blur of the letters.
    let radius = (size * 0.22) as i64;
    let mut blurred = vec![0.0f32; width * height];
    for y in 0..height as i64 {
        for x in 0..width as i64 {
            let mut sum = 0.0;
            let mut count = 0.0;
            for dy in (-radius..=radius).step_by(2) {
                for dx in (-radius..=radius).step_by(2) {
                    sum += at(x + dx, y + dy);
                    count += 1.0;
                }
            }
            blurred[y as usize * width + x as usize] = sum / count;
        }
    }
    for y in 0..height as i64 {
        for x in 0..width as i64 {
            let glow = blurred[y as usize * width + x as usize];
            let rim = [
                (-2, 0),
                (2, 0),
                (0, -2),
                (0, 2),
                (-1, -1),
                (1, 1),
                (-1, 1),
                (1, -1),
            ]
            .iter()
            .map(|(dx, dy)| at(x + dx, y + dy))
            .fold(0.0f32, f32::max);
            let fill = at(x, y);
            let mut px = Rgba([255, 190, 96, (glow * 150.0) as u8]);
            px.blend(&Rgba([30, 18, 4, (rim * 235.0) as u8]));
            let t = (y as f32 - pad as f32) / h as f32;
            let c = mix(FILL_TOP, FILL_BOTTOM, t * 1.15);
            px.blend(&Rgba([c[0], c[1], c[2], (fill * 255.0) as u8]));
            image.put_pixel(x as u32, y as u32, px);
        }
    }
    image
}

/// Serif label (the game's Times face) centred on `centre_x`.
pub fn label(image: &mut RgbaImage, text: &str, size: f32, centre_x: i32, y: i32, colour: [u8; 4]) {
    if let Ok(font) = crate::main_hud::label_font() {
        let width = crate::main_hud::text_width(font, text, size);
        crate::main_hud::crisp_text(image, font, text, size, [centre_x - width / 2, y], colour);
    }
}

/// Serif label starting at `x`.
pub fn label_left(image: &mut RgbaImage, text: &str, size: f32, x: i32, y: i32, colour: [u8; 4]) {
    if let Ok(font) = crate::main_hud::label_font() {
        crate::main_hud::crisp_text(image, font, text, size, [x, y], colour);
    }
}

/// Decorative (Cinzel) heading starting at `x`, or centred when `centred`.
pub fn heading(
    image: &mut RgbaImage,
    text: &str,
    size: f32,
    x: i32,
    y: i32,
    colour: [u8; 4],
    centred: bool,
) {
    let Ok(font) = crate::labels::decorative_font() else {
        return label(image, text, size, x, y, colour);
    };
    let width = crate::main_hud::text_width(&font, text, size);
    let left = if centred { x - width / 2 } else { x };
    crate::main_hud::text(image, &font, text, size, [left, y], colour, None);
}

pub fn text_width(text: &str, size: f32) -> i32 {
    crate::main_hud::font().map_or(0, |font| crate::main_hud::text_width(font, text, size))
}

/// Body text in the UI sans face; returns the advance.
pub fn body(
    image: &mut RgbaImage,
    text: &str,
    size: f32,
    x: i32,
    y: i32,
    colour: [u8; 4],
    max_width: i32,
) -> i32 {
    crate::main_hud::font().map_or(0, |font| {
        crate::main_hud::text(image, font, text, size, [x, y], colour, Some(max_width))
    })
}

/// Splits `text` into lines no wider than `width` in the body face.
pub fn wrap(text: &str, size: f32, width: i32) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        let candidate = if line.is_empty() {
            word.to_owned()
        } else {
            format!("{line} {word}")
        };
        if text_width(&candidate, size) > width && !line.is_empty() {
            lines.push(std::mem::replace(&mut line, word.to_owned()));
        } else {
            line = candidate;
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

// ---------------------------------------------------------------------------
// Widgets

/// A small gold diamond centred on (x, y).
pub fn diamond(image: &mut RgbaImage, x: i32, y: i32, radius: i32, colour: [u8; 4]) {
    for dy in -radius..=radius {
        let span = radius - dy.abs();
        fill(image, [x - span, y + dy, span * 2 + 1, 1], colour);
    }
}

/// A gold rule with diamond ends, `width` wide, centred on `centre_x`.
pub fn ornament_rule(image: &mut RgbaImage, centre_x: i32, y: i32, width: i32) {
    let half = width / 2;
    for x in -half..=half {
        let t = 1.0 - (x.abs() as f32 / half.max(1) as f32);
        blend(image, centre_x + x, y, with_alpha(GOLD, 0.25 + t * 0.75));
    }
    diamond(image, centre_x, y, 3, GOLD_BRIGHT);
    diamond(image, centre_x - half - 4, y, 2, GOLD_DEEP);
    diamond(image, centre_x + half + 4, y, 2, GOLD_DEEP);
}

/// An ornate panel: dark glass body, double gold frame with corner pieces
/// and, when `title` is set, a title band.
pub fn panel(image: &mut RgbaImage, rect: [i32; 4], title: Option<&str>) {
    let [x, y, w, h] = rect;
    // Drop shadow.
    for step in 1..=10 {
        let alpha = (1.0 - step as f32 / 11.0).powi(2) * 0.5;
        outline(
            image,
            [x - step, y - step + 4, w + step * 2, h + step * 2],
            [0, 0, 0, (alpha * 255.0) as u8],
        );
    }
    gradient(image, rect, [22, 50, 60, 238], [10, 28, 36, 245]);
    outline(image, [x - 1, y - 1, w + 2, h + 2], [4, 10, 14, 255]);
    outline(image, rect, GOLD_DEEP);
    outline(image, [x + 1, y + 1, w - 2, h - 2], [120, 92, 44, 255]);
    outline(image, [x + 4, y + 4, w - 8, h - 8], TEAL_RULE);
    // Corner pieces: an L of gold with a diamond.
    for (cx, cy, sx, sy) in [
        (x, y, 1, 1),
        (x + w - 1, y, -1, 1),
        (x, y + h - 1, 1, -1),
        (x + w - 1, y + h - 1, -1, -1),
    ] {
        for i in 0..18 {
            for t in 0..3 {
                blend(image, cx + sx * i, cy + sy * t, GOLD);
                blend(image, cx + sx * t, cy + sy * i, GOLD);
            }
        }
        diamond(image, cx + sx * 8, cy + sy * 8, 3, GOLD_BRIGHT);
    }
    if let Some(title) = title {
        let band = [x + 5, y + 5, w - 10, 40];
        gradient(image, band, [40, 84, 96, 255], [18, 46, 56, 255]);
        fill(image, [x + 5, y + 45, w - 10, 1], GOLD_DEEP);
        fill(image, [x + 5, y + 46, w - 10, 1], [4, 12, 16, 200]);
        heading(image, title, 19.0, x + w / 2, y + 13, GOLD_BRIGHT, true);
        ornament_rule(image, x + w / 2, y + 45, (w / 3).min(160));
    }
}

pub enum ButtonStyle {
    Primary,
    Secondary,
    Choice { selected: bool },
}

pub fn button(
    image: &mut RgbaImage,
    rect: [i32; 4],
    text: &str,
    style: ButtonStyle,
    hot: bool,
    disabled: bool,
) {
    let [x, y, w, h] = rect;
    let (top, bottom, border, colour) = match style {
        ButtonStyle::Primary => (
            if hot {
                [196, 150, 66, 255]
            } else {
                [156, 116, 48, 255]
            },
            if hot {
                [118, 82, 28, 255]
            } else {
                [86, 58, 20, 255]
            },
            GOLD_BRIGHT,
            [255, 246, 222, 255],
        ),
        ButtonStyle::Secondary => (
            if hot {
                [52, 98, 110, 255]
            } else {
                [30, 66, 78, 255]
            },
            if hot {
                [20, 48, 58, 255]
            } else {
                [10, 30, 38, 255]
            },
            if hot { GOLD_BRIGHT } else { GOLD_DEEP },
            if hot { GOLD_BRIGHT } else { GOLD },
        ),
        ButtonStyle::Choice { selected: true } => (
            [132, 98, 40, 255],
            [70, 48, 16, 255],
            GOLD_BRIGHT,
            GOLD_BRIGHT,
        ),
        ButtonStyle::Choice { selected: false } => (
            if hot {
                [40, 80, 92, 255]
            } else {
                [18, 42, 52, 255]
            },
            if hot {
                [16, 40, 48, 255]
            } else {
                [8, 22, 28, 255]
            },
            if hot { GOLD } else { [96, 84, 56, 255] },
            if hot { GOLD_BRIGHT } else { GOLD },
        ),
    };
    if hot && !disabled {
        glow(image, rect, 6, [255, 200, 110, 120]);
    }
    gradient(image, rect, top, bottom);
    fill(image, [x + 1, y + 1, w - 2, 1], [255, 255, 255, 40]);
    outline(image, rect, border);
    outline(image, [x - 1, y - 1, w + 2, h + 2], [0, 0, 0, 140]);
    let size = if h >= 30 { 15.0 } else { 13.0 };
    let colour = if disabled {
        with_alpha(colour, 0.5)
    } else {
        colour
    };
    label(
        image,
        text,
        size,
        x + w / 2,
        y + (h - size as i32 - 2) / 2,
        colour,
    );
}

/// A text field: dark inset with a gold frame when focused.
pub fn field(
    image: &mut RgbaImage,
    rect: [i32; 4],
    caption: &str,
    value: &str,
    focused: bool,
    hot: bool,
    caret: bool,
) {
    let [x, y, w, h] = rect;
    label_left(image, caption, 13.0, x + 2, y - 19, GOLD);
    if focused {
        glow(image, rect, 5, [255, 210, 120, 110]);
    }
    gradient(image, rect, [2, 8, 12, 255], [12, 26, 32, 255]);
    fill(image, [x + 1, y + 1, w - 2, 2], [0, 0, 0, 120]);
    outline(
        image,
        rect,
        if focused {
            GOLD_BRIGHT
        } else if hot {
            GOLD
        } else {
            [64, 94, 100, 255]
        },
    );
    let advance = body(image, value, 13.0, x + 10, y + (h - 15) / 2, WHITE, w - 20);
    if focused && caret {
        fill(image, [x + 11 + advance, y + 7, 1, h - 14], GOLD_BRIGHT);
    }
}

/// A horizontal bar of `value` out of five pips.
pub fn stat_bar(image: &mut RgbaImage, x: i32, y: i32, caption: &str, value: u8) {
    label_left(image, caption, 13.0, x, y - 1, GOLD);
    for pip in 0..5 {
        let rect = [x + 78 + pip * 26, y + 2, 22, 9];
        if pip < i32::from(value) {
            gradient(image, rect, [255, 226, 150, 255], [180, 128, 50, 255]);
            outline(image, rect, [80, 56, 20, 255]);
        } else {
            fill(image, rect, [6, 16, 20, 255]);
            outline(image, rect, [52, 74, 80, 255]);
        }
    }
}
