//! The Top titles (Top Trojan, Top Warrior, Top Archer, Top Taoist) circling
//! their holder: "Top" and the profession written on a ring around the
//! waist that turns, each word bent along it, so it reads in front, thins
//! at the sides and shows reversed behind, where the hero hides it.
//!
//! The 5065 client has no art for them, so it is made here, each
//! profession in its own metal: Trojan in fire, Warrior in ice-steel,
//! Archer in emerald, Taoist in arcane violet. The words are a calligraphic
//! italic (Times New Roman Bold Italic, which Windows always has) in a
//! metallic gradient with a bevelled edge, a dark outline and a shadow,
//! drawn at twice the size and brought down so their edges stay smooth.
//! Around them: a halo that breathes, a glint that sweeps the letters now
//! and then, a ring of light with a comet's tail behind each word, sparks
//! that ride the ring and embers that rise from it, and an aura on the
//! ground that pulses under the hero.
//!
//! [`quads`] lays a frame out; the renderer draws it, and the preview test
//! paints the same quads on the CPU.

use image::RgbaImage;
use std::f32::consts::{PI, TAU};

/// The Top titles' flags (`top_classes` on the server) and professions.
pub const PROFESSIONS: [(u32, &str); 4] = [(1, "Trojan"), (2, "Warrior"), (4, "Archer"), (8, "Taoist")];

/// The words' height (1x), and the ring around the waist.
const SIZE: f32 = 32.0;
const RING: [f32; 2] = [58.0, 18.0];
/// How high the ring turns over the feet.
pub const LIFT: f32 = 50.0;
const TURN_SPEED: f32 = 1.25;
const STRIPS: usize = 16;
const RING_SEGMENTS: usize = 72;
const SPARKS: usize = 10;
const EMBERS: usize = 8;
pub const SHINE_FRAMES: usize = 10;
const SHINE_EVERY: f32 = 3.2;
const SHINE_LASTS: f32 = 0.75;
/// Supersampling of the word art.
const SS: usize = 2;
const PAD: usize = 10;

type Rgb = [f32; 3];

/// A profession's metal and light.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Theme {
    /// The metal from its highlight to its deepest.
    light: Rgb,
    main: Rgb,
    deep: Rgb,
    /// The outline, the halo and the sparks.
    edge: Rgb,
    glow: Rgb,
    spark: Rgb,
}

pub fn theme(profession: &str) -> Theme {
    match profession {
        // Ice-steel.
        "Warrior" => Theme {
            light: [0.95, 0.99, 1.0],
            main: [0.52, 0.78, 1.0],
            deep: [0.09, 0.20, 0.52],
            edge: [0.02, 0.05, 0.16],
            glow: [0.28, 0.60, 1.0],
            spark: [0.78, 0.92, 1.0],
        },
        // Emerald.
        "Archer" => Theme {
            light: [0.94, 1.0, 0.80],
            main: [0.42, 0.93, 0.36],
            deep: [0.03, 0.36, 0.13],
            edge: [0.0, 0.10, 0.03],
            glow: [0.24, 0.95, 0.45],
            spark: [0.82, 1.0, 0.62],
        },
        // Arcane violet.
        "Taoist" => Theme {
            light: [1.0, 0.92, 1.0],
            main: [0.80, 0.50, 1.0],
            deep: [0.30, 0.06, 0.55],
            edge: [0.10, 0.0, 0.18],
            glow: [0.70, 0.34, 1.0],
            spark: [0.96, 0.76, 1.0],
        },
        // Fire, the Trojan's and the default.
        _ => Theme {
            light: [1.0, 0.97, 0.76],
            main: [1.0, 0.62, 0.12],
            deep: [0.62, 0.06, 0.02],
            edge: [0.18, 0.03, 0.0],
            glow: [1.0, 0.42, 0.08],
            spark: [1.0, 0.78, 0.32],
        },
    }
}

fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb {
    std::array::from_fn(|i| a[i] + (b[i] - a[i]) * t)
}

// ------------------------------------------------------------- fields --

/// A grey field the word art is worked on.
#[derive(Clone)]
struct Field {
    width: usize,
    height: usize,
    values: Vec<f32>,
}

impl Field {
    fn new(width: usize, height: usize) -> Self {
        Self { width, height, values: vec![0.0; width * height] }
    }

    fn at(&self, x: isize, y: isize) -> f32 {
        if x < 0 || y < 0 || x >= self.width as isize || y >= self.height as isize {
            0.0
        } else {
            self.values[y as usize * self.width + x as usize]
        }
    }

    /// A box blur three times over: near enough a Gaussian of `radius`.
    fn blur(&self, radius: usize) -> Self {
        let mut field = self.clone();
        for _ in 0..3 {
            field = field.box_pass(radius, true).box_pass(radius, false);
        }
        field
    }

    fn box_pass(&self, radius: usize, horizontal: bool) -> Self {
        let mut out = Self::new(self.width, self.height);
        let r = radius as isize;
        let span = (2 * r + 1) as f32;
        for y in 0..self.height as isize {
            for x in 0..self.width as isize {
                let sum: f32 = (-r..=r)
                    .map(|d| if horizontal { self.at(x + d, y) } else { self.at(x, y + d) })
                    .sum();
                out.values[y as usize * self.width + x as usize] = sum / span;
            }
        }
        out
    }

    /// The largest value within `radius` (a round brush).
    fn dilate(&self, radius: f32) -> Self {
        let mut out = Self::new(self.width, self.height);
        let r = radius.ceil() as isize;
        let offsets: Vec<(isize, isize)> = (-r..=r)
            .flat_map(|dy| (-r..=r).map(move |dx| (dx, dy)))
            .filter(|(dx, dy)| ((dx * dx + dy * dy) as f32) <= radius * radius)
            .collect();
        for y in 0..self.height as isize {
            for x in 0..self.width as isize {
                out.values[y as usize * self.width + x as usize] =
                    offsets.iter().map(|(dx, dy)| self.at(x + dx, y + dy)).fold(0.0, f32::max);
            }
        }
        out
    }
}

/// Premultiplied colour being composed.
struct Canvas {
    width: usize,
    height: usize,
    pixels: Vec<[f32; 4]>,
}

impl Canvas {
    fn new(width: usize, height: usize) -> Self {
        Self { width, height, pixels: vec![[0.0; 4]; width * height] }
    }

    /// `colour` at `alpha` over what is there.
    fn over(&mut self, index: usize, colour: Rgb, alpha: f32) {
        let alpha = alpha.clamp(0.0, 1.0);
        let pixel = &mut self.pixels[index];
        for c in 0..3 {
            pixel[c] = colour[c] * alpha + pixel[c] * (1.0 - alpha);
        }
        pixel[3] = alpha + pixel[3] * (1.0 - alpha);
    }

    /// Down by `SS`, back to straight alpha.
    fn image(&self) -> RgbaImage {
        let (width, height) = (self.width / SS, self.height / SS);
        let mut image = RgbaImage::new(width as u32, height as u32);
        for y in 0..height {
            for x in 0..width {
                let mut sum = [0.0f32; 4];
                for dy in 0..SS {
                    for dx in 0..SS {
                        let pixel = self.pixels[(y * SS + dy) * self.width + x * SS + dx];
                        for c in 0..4 {
                            sum[c] += pixel[c];
                        }
                    }
                }
                let n = (SS * SS) as f32;
                let alpha = sum[3] / n;
                let colour: [u8; 4] = if alpha <= 0.001 {
                    [0, 0, 0, 0]
                } else {
                    [
                        (sum[0] / n / alpha * 255.0).clamp(0.0, 255.0) as u8,
                        (sum[1] / n / alpha * 255.0).clamp(0.0, 255.0) as u8,
                        (sum[2] / n / alpha * 255.0).clamp(0.0, 255.0) as u8,
                        (alpha * 255.0).clamp(0.0, 255.0) as u8,
                    ]
                };
                image.put_pixel(x as u32, y as u32, image::Rgba(colour));
            }
        }
        image
    }
}

// --------------------------------------------------------------- words --

/// Times New Roman Bold Italic from the system; `None` falls back to the
/// label face, slanted.
fn title_font() -> Option<&'static fontdue::Font> {
    static FONT: std::sync::OnceLock<Option<fontdue::Font>> = std::sync::OnceLock::new();
    FONT.get_or_init(|| {
        let mut db = fontdb::Database::new();
        db.load_system_fonts();
        let id = db.query(&fontdb::Query {
            families: &[fontdb::Family::Name("Times New Roman"), fontdb::Family::Serif],
            weight: fontdb::Weight::BOLD,
            style: fontdb::Style::Italic,
            ..Default::default()
        })?;
        if db.face(id)?.style != fontdb::Style::Italic {
            return None;
        }
        db.with_face_data(id, |bytes, index| {
            fontdue::Font::from_bytes(bytes, fontdue::FontSettings { collection_index: index, ..Default::default() }).ok()
        })?
    })
    .as_ref()
}

/// The word's coverage at `SS` times, and the rows its letters span.
fn coverage(word: &str) -> (Field, [f32; 2]) {
    let (font, slant) = match title_font() {
        Some(font) => (font, 0.0),
        None => match crate::main_hud::label_font() {
            Ok(font) => (font, 0.28),
            Err(_) => return (Field::new(SS * 2, SS * 2), [0.0, 1.0]),
        },
    };
    let size = SIZE * SS as f32;
    let pad = (PAD * SS) as f32;
    let metrics = font.horizontal_line_metrics(size);
    let (ascent, descent) = metrics.map_or((size * 0.9, -size * 0.25), |line| (line.ascent, line.descent));
    let mut advance = 0.0;
    let mut previous = None;
    let mut glyphs = Vec::new();
    for c in word.chars() {
        if let Some(previous) = previous {
            advance += font.horizontal_kern(previous, c, size).unwrap_or(0.0);
        }
        let (glyph, bitmap) = font.rasterize(c, size);
        glyphs.push((advance, glyph, bitmap));
        advance += glyph.advance_width;
        previous = Some(c);
    }
    let width = (advance + size * (0.25 + slant) + pad * 2.0).ceil() as usize;
    let height = (ascent - descent + pad * 2.0).ceil() as usize;
    let baseline = pad + ascent;
    let mut field = Field::new(width, height);
    for (pen, glyph, bitmap) in &glyphs {
        for row in 0..glyph.height {
            let y = baseline - glyph.ymin as f32 - glyph.height as f32 + row as f32;
            let lean = (baseline - y) * slant;
            for column in 0..glyph.width {
                let x = (pad + pen + glyph.xmin as f32 + column as f32 + lean).round();
                let (x, yi) = (x as isize, y.round() as isize);
                if x >= 0 && yi >= 0 && (x as usize) < width && (yi as usize) < height {
                    let cell = &mut field.values[yi as usize * width + x as usize];
                    *cell = cell.max(f32::from(bitmap[row * glyph.width + column]) / 255.0);
                }
            }
        }
    }
    // The letters' rows: from the tallest's top to the baseline.
    let top = (0..height).find(|y| (0..width).any(|x| field.values[y * width + x] > 0.5)).unwrap_or(0) as f32;
    (field, [top, baseline])
}

/// The metal at `t` down the letters: a highlight at the top, the colour,
/// a dark horizon a little past the middle, and a reflected light at the
/// foot.
fn metal(theme: &Theme, t: f32) -> Rgb {
    let stops = [
        (0.0, theme.light),
        (0.40, theme.main),
        (0.50, mix(theme.main, theme.deep, 0.8)),
        (0.58, mix(theme.main, theme.deep, 0.25)),
        (0.90, theme.deep),
        (1.0, mix(theme.deep, theme.main, 0.45)),
    ];
    let t = t.clamp(0.0, 1.0);
    for pair in stops.windows(2) {
        let ((t0, a), (t1, b)) = (pair[0], pair[1]);
        if t <= t1 {
            return mix(a, b, (t - t0) / (t1 - t0));
        }
    }
    stops[stops.len() - 1].1
}

/// A word in its profession's metal: shadow, outline, a bright rim, and
/// the bevelled letters.
pub fn word_art(word: &str, profession: &str) -> RgbaImage {
    let theme = theme(profession);
    let (letters, [top, baseline]) = coverage(word);
    let (width, height) = (letters.width, letters.height);
    let s = SS as f32;
    let shadow = letters.dilate(1.5 * s).blur(2 * SS);
    let outline = letters.dilate(2.2 * s).blur(SS / 2);
    let rim = letters.dilate(0.9 * s);
    let bevel = letters.blur(SS + 1);
    let mut canvas = Canvas::new(width, height);
    let (shadow_dx, shadow_dy) = ((1.5 * s) as isize, (2.0 * s) as isize);
    for y in 0..height {
        for x in 0..width {
            let index = y * width + x;
            let (xi, yi) = (x as isize, y as isize);
            canvas.over(index, [0.0, 0.0, 0.0], shadow.at(xi - shadow_dx, yi - shadow_dy) * 0.6);
            canvas.over(index, theme.edge, outline.values[index]);
            canvas.over(index, mix(theme.main, theme.light, 0.5), (rim.values[index] - letters.values[index]).max(0.0) * 0.9);
            let cover = letters.values[index];
            if cover <= 0.0 {
                continue;
            }
            let fill = metal(&theme, (y as f32 - top) / (baseline - top).max(1.0));
            // Light from the upper left on the letters' edges.
            let nx = bevel.at(xi + 1, yi) - bevel.at(xi - 1, yi);
            let ny = bevel.at(xi, yi + 1) - bevel.at(xi, yi - 1);
            let shade = (nx * 0.6 + ny * 0.8) * 2.4;
            let lit: Rgb = std::array::from_fn(|c| (fill[c] * (1.0 + 0.7 * shade) + shade.max(0.0) * 0.35).clamp(0.0, 1.0));
            canvas.over(index, lit, cover);
        }
    }
    canvas.image()
}

/// The halo behind a word (drawn added), the same size as its art.
pub fn word_glow(word: &str, profession: &str) -> RgbaImage {
    let theme = theme(profession);
    let (letters, _) = coverage(word);
    let halo = letters.dilate(2.5 * SS as f32).blur(4 * SS);
    let mut canvas = Canvas::new(letters.width, letters.height);
    for (index, value) in halo.values.iter().enumerate() {
        canvas.over(index, theme.glow, (value * 1.5).min(1.0) * 0.9);
    }
    canvas.image()
}

/// A glint across a word (drawn added): frame `frame` of
/// [`SHINE_FRAMES`], the band moving left to right.
pub fn word_shine(word: &str, profession: &str, frame: usize) -> RgbaImage {
    let theme = theme(profession);
    let (letters, _) = coverage(word);
    let (width, height) = (letters.width as f32, letters.height as f32);
    let progress = frame as f32 / (SHINE_FRAMES - 1).max(1) as f32;
    let centre = -0.25 * width + progress * 1.5 * width;
    let sigma = 6.0 * SS as f32;
    let colour = mix([1.0, 1.0, 1.0], theme.light, 0.4);
    let mut canvas = Canvas::new(letters.width, letters.height);
    for (index, value) in letters.values.iter().enumerate() {
        if *value <= 0.0 {
            continue;
        }
        let (x, y) = ((index % letters.width) as f32, (index / letters.width) as f32);
        // The band leans with the letters.
        let distance = x + (y - height * 0.5) * 0.45 - centre;
        let band = (-(distance * distance) / (2.0 * sigma * sigma)).exp();
        canvas.over(index, colour, value * band);
    }
    canvas.image()
}

/// A piece of the ring of light (drawn added), lying along x.
pub fn ring_art(profession: &str) -> RgbaImage {
    let theme = theme(profession);
    let (width, height) = (32, 24);
    let mut image = RgbaImage::new(width, height);
    for y in 0..height {
        let d = y as f32 + 0.5 - height as f32 * 0.5;
        let core = (-(d * d) / (2.0 * 0.9 * 0.9)).exp();
        let halo = (-(d * d) / (2.0 * 4.0 * 4.0)).exp() * 0.35;
        let colour = mix(theme.glow, [1.0, 1.0, 1.0], core * 0.4);
        let alpha = (core * 0.8 + halo).min(1.0);
        for x in 0..width {
            image.put_pixel(x, y, image::Rgba([
                (colour[0] * 255.0) as u8,
                (colour[1] * 255.0) as u8,
                (colour[2] * 255.0) as u8,
                (alpha * 255.0) as u8,
            ]));
        }
    }
    image
}

/// The aura on the ground (drawn added): a soft disc in the profession's
/// light with a brighter rim, seen flat.
pub fn aura_art(profession: &str) -> RgbaImage {
    let theme = theme(profession);
    let (width, height) = (128u32, 48u32);
    let mut image = RgbaImage::new(width, height);
    for y in 0..height {
        for x in 0..width {
            let dx = (x as f32 + 0.5) / width as f32 * 2.0 - 1.0;
            let dy = (y as f32 + 0.5) / height as f32 * 2.0 - 1.0;
            let r = dx.hypot(dy);
            let disc = (1.0 - r).clamp(0.0, 1.0).powf(1.6) * 0.55;
            let rim = (-((r - 0.82) * (r - 0.82)) / (2.0 * 0.06 * 0.06)).exp() * 0.6;
            let alpha = (disc + rim).min(1.0) * if r > 1.0 { 0.0 } else { 1.0 };
            let colour = mix(theme.glow, theme.main, rim * 0.5);
            image.put_pixel(x, y, image::Rgba([
                (colour[0] * 255.0) as u8,
                (colour[1] * 255.0) as u8,
                (colour[2] * 255.0) as u8,
                (alpha * 255.0) as u8,
            ]));
        }
    }
    image
}

/// A spark (drawn added): a white core in the profession's light with a
/// four-pointed flare.
pub fn spark_art(profession: &str) -> RgbaImage {
    let theme = theme(profession);
    let size = 32;
    let mut image = RgbaImage::new(size, size);
    let centre = size as f32 * 0.5;
    for y in 0..size {
        for x in 0..size {
            let (dx, dy) = (x as f32 + 0.5 - centre, y as f32 + 0.5 - centre);
            let r2 = dx * dx + dy * dy;
            let core = (-r2 / (2.0 * 1.6 * 1.6)).exp();
            let halo = (-r2 / (2.0 * 5.5 * 5.5)).exp() * 0.55;
            let flare = ((-dx.abs() / 6.0).exp() * (-(dy * dy) / 0.8).exp()
                + (-dy.abs() / 6.0).exp() * (-(dx * dx) / 0.8).exp())
                * 0.6;
            let colour = mix(theme.spark, [1.0, 1.0, 1.0], core);
            let alpha = (core + halo + flare).min(1.0);
            image.put_pixel(x, y, image::Rgba([
                (colour[0] * 255.0) as u8,
                (colour[1] * 255.0) as u8,
                (colour[2] * 255.0) as u8,
                (alpha * 255.0) as u8,
            ]));
        }
    }
    image
}

// -------------------------------------------------------------- layout --

/// What a quad shows.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Piece {
    Word(String),
    Glow(String),
    Shine(String, usize),
    Ring,
    Spark,
    Aura,
}

impl Piece {
    /// Drawn added onto the scene rather than over it.
    pub fn added(&self) -> bool {
        !matches!(self, Self::Word(_))
    }

    /// Its layer, back to front: ring, halo, word, glint, sparks.
    pub fn layer(&self) -> u8 {
        match self {
            Self::Aura => 0,
            Self::Ring => 0,
            Self::Glow(_) => 1,
            Self::Word(_) => 2,
            Self::Shine(..) => 3,
            Self::Spark => 4,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TopQuad {
    pub piece: Piece,
    /// Top left, top right, bottom right, bottom left.
    pub corners: [[f32; 2]; 4],
    pub uv: [[f32; 2]; 4],
    pub alpha: f32,
    /// In front of the hero (else behind).
    pub front: bool,
}

fn ring_point(centre: [f32; 2], angle: f32, scale: f32) -> [f32; 2] {
    [centre[0] + angle.cos() * RING[0] * scale, centre[1] + angle.sin() * RING[1] * scale]
}

/// The profession shown at `time` among those `flags` holds (they take
/// turns, six seconds each).
pub fn profession(flags: u32, time: f32) -> Option<&'static str> {
    let held: Vec<&str> = PROFESSIONS.iter().filter(|(flag, _)| flags & flag != 0).map(|(_, name)| *name).collect();
    (!held.is_empty()).then(|| held[(time.max(0.0) / 6.0) as usize % held.len()])
}

/// One frame of a title around a hero whose feet are at `origin`;
/// `size_of` gives a word's art size.
pub fn quads(profession: &str, origin: [f32; 2], time: f32, size_of: impl Fn(&str) -> [f32; 2]) -> Vec<TopQuad> {
    let centre = [origin[0], origin[1] - LIFT];
    let turn = time * TURN_SPEED;
    let words = [("Top".to_owned(), turn), (profession.to_owned(), turn + PI)];
    let mut quads = Vec::new();
    let spans: Vec<f32> = words.iter().map(|(word, _)| size_of(word)[0] / RING[0]).collect();

    // The aura on the ground, pulsing under the hero.
    let pulse = 0.5 + 0.5 * (time * 2.2).sin();
    let [aw, ah] = [RING[0] * 2.1 * (1.0 + 0.05 * pulse), RING[1] * 2.4 * (1.0 + 0.05 * pulse)];
    quads.push(TopQuad {
        piece: Piece::Aura,
        corners: [
            [origin[0] - aw * 0.5, origin[1] - ah * 0.5],
            [origin[0] + aw * 0.5, origin[1] - ah * 0.5],
            [origin[0] + aw * 0.5, origin[1] + ah * 0.5],
            [origin[0] - aw * 0.5, origin[1] + ah * 0.5],
        ],
        uv: [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
        alpha: 0.3 + 0.2 * pulse,
        front: false,
    });

    // The ring: dim all round, bright under each word with a comet's tail
    // behind it.
    for segment in 0..RING_SEGMENTS {
        let (a0, a1) = (segment as f32 / RING_SEGMENTS as f32 * TAU, (segment + 1) as f32 / RING_SEGMENTS as f32 * TAU);
        let middle = (a0 + a1) * 0.5;
        let mut light: f32 = 0.06;
        for ((_, angle), span) in words.iter().zip(&spans) {
            // How far behind the word's middle (it moves to larger angles).
            let behind = (angle - middle).rem_euclid(TAU);
            let behind = if behind > PI { behind - TAU } else { behind };
            let half = span * 0.5;
            light += if behind.abs() <= half {
                0.45
            } else if behind > half {
                0.85 * (-(behind - half) / 0.9).exp()
            } else {
                0.3 * (-(-half - behind) / 0.2).exp()
            };
        }
        let sin = middle.sin();
        let front = sin > 0.0;
        let light = light.min(1.0) * if front { 1.0 } else { 0.45 };
        let (p0, p1) = (ring_point(centre, a0, 1.0), ring_point(centre, a1, 1.0));
        // Thick across the line, a little more in front.
        let (dx, dy) = (p1[0] - p0[0], p1[1] - p0[1]);
        let length = dx.hypot(dy).max(0.001);
        let thickness = 7.0 + 3.0 * sin.max(0.0);
        let normal = [-dy / length * thickness, dx / length * thickness];
        quads.push(TopQuad {
            piece: Piece::Ring,
            corners: [
                [p0[0] - normal[0], p0[1] - normal[1]],
                [p1[0] - normal[0], p1[1] - normal[1]],
                [p1[0] + normal[0], p1[1] + normal[1]],
                [p0[0] + normal[0], p0[1] + normal[1]],
            ],
            uv: [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
            alpha: light,
            front,
        });
    }

    // The words, each bent along the ring in strips, with their halo
    // breathing and, now and then, a glint.
    let breath = 0.55 + 0.3 * (0.5 + 0.5 * (time * 2.6).sin());
    let glint = time.rem_euclid(SHINE_EVERY);
    let shine = (glint < SHINE_LASTS)
        .then(|| ((glint / SHINE_LASTS) * (SHINE_FRAMES - 1) as f32).round() as usize);
    for ((word, angle), span) in words.iter().zip(&spans) {
        let [_, height] = size_of(word);
        let at = |u: f32| {
            let a = angle - (u - 0.5) * span;
            (ring_point(centre, a, 1.0), a.sin())
        };
        for strip in 0..STRIPS {
            let (u0, u1) = (strip as f32 / STRIPS as f32, (strip + 1) as f32 / STRIPS as f32);
            let ((left, sin0), (right, sin1)) = (at(u0), at(u1));
            let near = (sin0 + sin1) * 0.5;
            let half = |sin: f32| height * (0.84 + 0.16 * sin) * 0.5;
            let corners = [
                [left[0], left[1] - half(sin0)],
                [right[0], right[1] - half(sin1)],
                [right[0], right[1] + half(sin1)],
                [left[0], left[1] + half(sin0)],
            ];
            let uv = [[u0, 0.0], [u1, 0.0], [u1, 1.0], [u0, 1.0]];
            let front = near > 0.0;
            let dim = 0.55 + 0.45 * near.max(0.0);
            let mut pieces = vec![(Piece::Glow(word.clone()), breath * dim), (Piece::Word(word.clone()), dim)];
            if let Some(frame) = shine.filter(|_| front) {
                pieces.push((Piece::Shine(word.clone(), frame), 1.0));
            }
            for (piece, alpha) in pieces {
                quads.push(TopQuad { piece, corners, uv, alpha, front });
            }
        }
    }

    // Sparks riding the ring, twinkling.
    let spark = |quads: &mut Vec<TopQuad>, at: [f32; 2], size: f32, alpha: f32, front: bool| {
        let h = size * 0.5;
        quads.push(TopQuad {
            piece: Piece::Spark,
            corners: [[at[0] - h, at[1] - h], [at[0] + h, at[1] - h], [at[0] + h, at[1] + h], [at[0] - h, at[1] + h]],
            uv: [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
            alpha,
            front,
        });
    };
    for index in 0..SPARKS {
        let i = index as f32;
        let angle = turn * 1.35 + i * TAU / SPARKS as f32 + 0.25 * (time * 1.7 + i).sin();
        let scale = 1.0 + 0.08 * (time * 2.3 + i * 1.3).sin();
        let mut at = ring_point(centre, angle, scale);
        at[1] -= 4.0 * (time * 3.0 + i).sin();
        let twinkle = 0.5 + 0.5 * (time * 7.0 + i * 2.1).sin();
        let front = angle.sin() > 0.0;
        spark(&mut quads, at, 12.0 + 8.0 * twinkle, (0.35 + 0.65 * twinkle) * if front { 1.0 } else { 0.5 }, front);
    }
    // Embers rising off the ring, each from its own place every cycle.
    for index in 0..EMBERS {
        let i = index as f32;
        let cycle = time * 0.55 + i / EMBERS as f32;
        let phase = cycle.fract();
        let angle = i * 2.4 + cycle.floor() * 1.7;
        let mut at = ring_point(centre, angle, 1.0);
        at[1] -= phase * 38.0;
        at[0] += (phase * 6.0 + i).sin() * 3.0;
        let front = angle.sin() > 0.0;
        spark(&mut quads, at, 7.0 + 5.0 * (1.0 - phase), (phase * PI).sin() * 0.8, front);
    }
    quads
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_professions_take_turns() {
        assert_eq!(profession(0, 0.0), None);
        assert_eq!(profession(4, 99.0), Some("Archer"));
        assert_eq!(profession(1 | 8, 1.0), Some("Trojan"));
        assert_eq!(profession(1 | 8, 7.0), Some("Taoist"));
    }

    #[test]
    fn a_frame_has_the_ring_both_words_and_the_sparks() {
        let quads = quads("Archer", [100.0, 200.0], 0.2, |_| [80.0, 40.0]);
        assert_eq!(quads.iter().filter(|quad| quad.piece == Piece::Ring).count(), RING_SEGMENTS);
        assert_eq!(quads.iter().filter(|quad| quad.piece == Piece::Word("Top".into())).count(), STRIPS);
        assert_eq!(quads.iter().filter(|quad| quad.piece == Piece::Spark).count(), SPARKS + EMBERS);
        // At 0.2 s the glint is on: front strips carry it.
        assert!(quads.iter().any(|quad| matches!(quad.piece, Piece::Shine(..))));
        // Something is in front and something behind.
        assert!(quads.iter().any(|quad| quad.front) && quads.iter().any(|quad| !quad.front));
    }

    /// Paints quads on the CPU as the renderer would: behind the hero,
    /// the hero (a stand-in), then in front.
    fn paint(canvas: &mut image::RgbaImage, quads: &[TopQuad], textures: &std::collections::HashMap<Piece, RgbaImage>, hero: [u32; 4]) {
        let mut ordered: Vec<&TopQuad> = quads.iter().collect();
        ordered.sort_by_key(|quad| (quad.front, quad.piece.layer()));
        let mut hero_drawn = false;
        for quad in ordered {
            if quad.front && !hero_drawn {
                hero_drawn = true;
                let [x, y, w, h] = hero;
                for py in y..y + h {
                    for px in x..x + w {
                        canvas.put_pixel(px, py, image::Rgba([196, 64, 52, 255]));
                    }
                }
            }
            let Some(texture) = textures.get(&quad.piece) else { continue };
            for [a, b, c] in [[0, 1, 2], [0, 2, 3]] {
                triangle(canvas, texture, [quad.corners[a], quad.corners[b], quad.corners[c]], [quad.uv[a], quad.uv[b], quad.uv[c]], quad.alpha, quad.piece.added());
            }
        }
    }

    fn triangle(canvas: &mut image::RgbaImage, texture: &RgbaImage, p: [[f32; 2]; 3], uv: [[f32; 2]; 3], alpha: f32, added: bool) {
        let area = (p[1][0] - p[0][0]) * (p[2][1] - p[0][1]) - (p[2][0] - p[0][0]) * (p[1][1] - p[0][1]);
        if area.abs() < 1e-4 {
            return;
        }
        let min_x = p.iter().map(|v| v[0]).fold(f32::MAX, f32::min).floor().max(0.0) as u32;
        let max_x = p.iter().map(|v| v[0]).fold(f32::MIN, f32::max).ceil().min(canvas.width() as f32 - 1.0) as u32;
        let min_y = p.iter().map(|v| v[1]).fold(f32::MAX, f32::min).floor().max(0.0) as u32;
        let max_y = p.iter().map(|v| v[1]).fold(f32::MIN, f32::max).ceil().min(canvas.height() as f32 - 1.0) as u32;
        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                let w0 = ((p[1][0] - px) * (p[2][1] - py) - (p[2][0] - px) * (p[1][1] - py)) / area;
                let w1 = ((p[2][0] - px) * (p[0][1] - py) - (p[0][0] - px) * (p[2][1] - py)) / area;
                let w2 = 1.0 - w0 - w1;
                if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                    continue;
                }
                let u = w0 * uv[0][0] + w1 * uv[1][0] + w2 * uv[2][0];
                let v = w0 * uv[0][1] + w1 * uv[1][1] + w2 * uv[2][1];
                let tx = ((u * texture.width() as f32) as u32).min(texture.width() - 1);
                let ty = ((v * texture.height() as f32) as u32).min(texture.height() - 1);
                let source = texture.get_pixel(tx, ty).0;
                let a = f32::from(source[3]) / 255.0 * alpha;
                let target = canvas.get_pixel_mut(x, y);
                for c in 0..3 {
                    let s = f32::from(source[c]);
                    let d = f32::from(target.0[c]);
                    target.0[c] = if added { (d + s * a).min(255.0) } else { s * a + d * (1.0 - a) } as u8;
                }
            }
        }
    }

    /// `TOP_PREVIEW=dir cargo test -p conquer-sandbox top_title_preview`:
    /// each profession's words, then frames of the title turning.
    #[test]
    fn top_title_preview() {
        let Ok(dir) = std::env::var("TOP_PREVIEW") else { return };
        let professions = ["Trojan", "Warrior", "Archer", "Taoist"];
        let times = [0.25, 0.9, 1.7, 2.4, 3.3];
        let (cell_w, cell_h) = (200u32, 170u32);
        let mut sheet = image::RgbaImage::from_pixel(cell_w * times.len() as u32, cell_h * professions.len() as u32, image::Rgba([96, 104, 98, 255]));
        let mut words = image::RgbaImage::from_pixel(560, 70 * professions.len() as u32, image::Rgba([60, 66, 62, 255]));
        for (row, profession) in professions.iter().enumerate() {
            let mut textures = std::collections::HashMap::new();
            for word in ["Top", *profession] {
                textures.insert(Piece::Word(word.into()), word_art(word, profession));
                textures.insert(Piece::Glow(word.into()), word_glow(word, profession));
                for frame in 0..SHINE_FRAMES {
                    textures.insert(Piece::Shine(word.into(), frame), word_shine(word, profession, frame));
                }
            }
            textures.insert(Piece::Ring, ring_art(profession));
            textures.insert(Piece::Spark, spark_art(profession));
            textures.insert(Piece::Aura, aura_art(profession));
            // The words flat: halo added under them.
            let mut x = 10i64;
            for word in ["Top", *profession] {
                let glow = &textures[&Piece::Glow(word.into())];
                let art = &textures[&Piece::Word(word.into())];
                let mut glow_canvas = image::RgbaImage::new(glow.width(), glow.height());
                image::imageops::overlay(&mut glow_canvas, glow, 0, 0);
                for (gx, gy, pixel) in glow_canvas.enumerate_pixels() {
                    let target = words.get_pixel_mut((x as u32 + gx).min(559), row as u32 * 70 + gy.min(69));
                    let a = f32::from(pixel.0[3]) / 255.0 * 0.8;
                    for c in 0..3 {
                        target.0[c] = (f32::from(target.0[c]) + f32::from(pixel.0[c]) * a).min(255.0) as u8;
                    }
                }
                image::imageops::overlay(&mut words, art, x, row as i64 * 70);
                x += i64::from(art.width()) + 10;
            }
            for (column, time) in times.iter().enumerate() {
                let mut cell = image::RgbaImage::from_pixel(cell_w, cell_h, image::Rgba([96, 104, 98, 255]));
                let origin = [cell_w as f32 * 0.5, cell_h as f32 - 20.0];
                let quads = quads(profession, origin, *time, |word| {
                    let art = &textures[&Piece::Word(word.into())];
                    [art.width() as f32, art.height() as f32]
                });
                paint(&mut cell, &quads, &textures, [cell_w / 2 - 14, cell_h - 110, 28, 92]);
                image::imageops::overlay(&mut sheet, &cell, i64::from(cell_w * column as u32), i64::from(cell_h * row as u32));
            }
        }
        sheet.save(format!("{dir}/top-turning.png")).expect("png");
        words.save(format!("{dir}/top-words.png")).expect("png");
    }
}
