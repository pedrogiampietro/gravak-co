//! The heroes on the character creation screen: the game's own 3D models,
//! drawn by a small software rasteriser (the login screens are painted
//! without the GPU). The geometry comes from `Character::vertices`, so the
//! models stand and breathe exactly as they do in the world.

use crate::{action::Action, assets::Assets, character::Character};
use image::{Rgba, RgbaImage};
use std::collections::HashMap;

/// What a profession wears on the creation screen, for each `CLASSES` row:
/// armor, helmet, right and left weapon (0 for none).
pub const OUTFITS: [[u32; 4]; 4] = [
    // Trojan: armor, coronet, two blades.
    [130_100, 118_100, 410_100, 410_100],
    // Warrior: armor, helmet, a blade and a shield.
    [131_100, 111_100, 410_100, 900_100],
    // Archer: coat, hat and a bow.
    [133_100, 113_100, 500_100, 0],
    // Taoist: robe, cap and a backsword.
    [134_100, 114_100, 421_100, 0],
];
const HAIR: u16 = 310;

/// A model ready to draw: the character and the textures of its parts.
pub struct Hero {
    character: Character,
    textures: HashMap<String, RgbaImage>,
}

impl Hero {
    /// Loads `body` (1..=4) in profession `class`'s outfit; a piece the
    /// installation lacks is left off rather than failing the whole hero.
    pub fn load(assets: &mut Assets, body: u32, class: usize) -> Option<Self> {
        let [armor, helmet, right, left] = OUTFITS[class];
        let character = [
            (armor, helmet, right, left),
            (armor, 0, right, left),
            (armor, 0, right, 0),
            (0, 0, 0, 0),
        ]
        .into_iter()
        .find_map(|(armor, helmet, right, left)| {
            Character::load_remote(assets, body, armor, helmet, HAIR, right, left, String::new(), 1).ok()
        })?;
        let textures = character
            .parts
            .iter()
            .filter_map(|part| Some((part.texture.clone(), assets.image(&part.texture).ok()?)))
            .collect();
        Some(Self { character, textures })
    }

    /// The hero at `time` seconds into its stand animation, turned to
    /// `facing` radians, `height` pixels tall from feet to the top of the
    /// head, centred in a `size` image.
    pub fn render(&self, size: [u32; 2], height: f32, time: f32, facing: f32) -> RgbaImage {
        // Drawn at twice the size and reduced, for smooth edges.
        const SUPERSAMPLE: u32 = 2;
        let [width, image_height] = size.map(|value| value * SUPERSAMPLE);
        let parts: Vec<_> = self
            .character
            .parts
            .iter()
            .map(|part| (part, self.character.vertices(part, time, Action::Idle, facing, None)))
            .collect();
        // Fit the stand pose: the feet sit low, the head below the top.
        let (mut top, mut bottom) = (f32::MAX, f32::MIN);
        for (_, vertices) in &parts {
            for (position, _) in vertices {
                top = top.min(position[1]);
                bottom = bottom.max(position[1]);
            }
        }
        let scale = height * SUPERSAMPLE as f32 / (bottom - top).max(1.0);
        let origin = [width as f32 * 0.5, image_height as f32 * 0.93 - bottom * scale];
        let mut colour = vec![[0.0f32; 4]; (width * image_height) as usize];
        let mut depth = vec![f32::INFINITY; (width * image_height) as usize];
        for pass in [false, true] {
            for (part, vertices) in &parts {
                let Some(texture) = self.textures.get(&part.texture) else {
                    continue;
                };
                let indices = if pass { &part.mesh.alpha_indices } else { &part.mesh.opaque_indices };
                for triangle in indices.chunks_exact(3) {
                    let corners = [0, 1, 2].map(|k| {
                        let (p, uv) = vertices[triangle[k] as usize];
                        ([origin[0] + p[0] * scale, origin[1] + p[1] * scale, p[2]], uv)
                    });
                    rasterise(&mut colour, &mut depth, [width, image_height], corners, texture, pass);
                }
            }
        }
        let mut image = RgbaImage::new(size[0], size[1]);
        for (x, y, pixel) in image.enumerate_pixels_mut() {
            let mut sum = [0.0f32; 4];
            for sy in 0..SUPERSAMPLE {
                for sx in 0..SUPERSAMPLE {
                    let at = ((y * SUPERSAMPLE + sy) * width + x * SUPERSAMPLE + sx) as usize;
                    for c in 0..4 {
                        sum[c] += colour[at][c];
                    }
                }
            }
            let samples = (SUPERSAMPLE * SUPERSAMPLE) as f32;
            let alpha = sum[3] / samples;
            *pixel = if alpha <= 0.0 {
                Rgba([0, 0, 0, 0])
            } else {
                // Colours are stored premultiplied.
                Rgba([
                    (sum[0] / samples / alpha).min(255.0) as u8,
                    (sum[1] / samples / alpha).min(255.0) as u8,
                    (sum[2] / samples / alpha).min(255.0) as u8,
                    (alpha * 255.0) as u8,
                ])
            };
        }
        image
    }
}

/// One textured triangle into the premultiplied colour and depth buffers.
/// Opaque triangles write depth and drop nearly clear texels; alpha ones
/// blend over what is in front-to-back order already drawn.
fn rasterise(
    colour: &mut [[f32; 4]],
    depth: &mut [f32],
    size: [u32; 2],
    corners: [([f32; 3], [f32; 2]); 3],
    texture: &RgbaImage,
    alpha_pass: bool,
) {
    let [(a, ta), (b, tb), (c, tc)] = corners;
    let area = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
    if area.abs() < 1e-6 {
        return;
    }
    let min_x = a[0].min(b[0]).min(c[0]).floor().max(0.0) as u32;
    let max_x = a[0].max(b[0]).max(c[0]).ceil().min(size[0] as f32 - 1.0).max(0.0) as u32;
    let min_y = a[1].min(b[1]).min(c[1]).floor().max(0.0) as u32;
    let max_y = a[1].max(b[1]).max(c[1]).ceil().min(size[1] as f32 - 1.0).max(0.0) as u32;
    let (tw, th) = (texture.width() as f32, texture.height() as f32);
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            let w0 = ((b[0] - px) * (c[1] - py) - (b[1] - py) * (c[0] - px)) / area;
            let w1 = ((c[0] - px) * (a[1] - py) - (c[1] - py) * (a[0] - px)) / area;
            let w2 = 1.0 - w0 - w1;
            if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                continue;
            }
            let z = w0 * a[2] + w1 * b[2] + w2 * c[2];
            let at = (y * size[0] + x) as usize;
            if z > depth[at] {
                continue;
            }
            let u = (w0 * ta[0] + w1 * tb[0] + w2 * tc[0]).rem_euclid(1.0);
            let v = (w0 * ta[1] + w1 * tb[1] + w2 * tc[1]).rem_euclid(1.0);
            let texel = texture.get_pixel(((u * tw) as u32).min(texture.width() - 1), ((v * th) as u32).min(texture.height() - 1));
            let alpha = f32::from(texel[3]) / 255.0;
            if alpha_pass {
                let pixel = &mut colour[at];
                for channel in 0..3 {
                    pixel[channel] = f32::from(texel[channel]) * alpha + pixel[channel] * (1.0 - alpha);
                }
                pixel[3] = alpha + pixel[3] * (1.0 - alpha);
            } else {
                if alpha < 0.5 {
                    continue;
                }
                depth[at] = z;
                colour[at] = [f32::from(texel[0]), f32::from(texel[1]), f32::from(texel[2]), 1.0];
            }
        }
    }
}

/// Loads every profession's hero for every body on a thread; each arrives
/// as (index into `launcher::BODIES`, index into `launcher::CLASSES`, hero).
pub fn start(root: std::path::PathBuf) -> std::sync::mpsc::Receiver<(usize, usize, Hero)> {
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let Ok(mut assets) = Assets::open(&root) else {
            return;
        };
        for (body_index, (_, mesh)) in crate::launcher::BODIES.into_iter().enumerate() {
            // Bodies 1003/1004 are the male models 3 and 4, 2001/2002 the
            // female 1 and 2.
            let body = u32::from(mesh % 10);
            for class in 0..crate::launcher::CLASSES.len() {
                if let Some(hero) = Hero::load(&mut assets, body, class)
                    && sender.send((body_index, class, hero)).is_err()
                {
                    return;
                }
            }
        }
    });
    receiver
}
