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
/// The hair the heroes wear until the player picks one: style 10 in
/// colour 3.
pub const HAIR: u16 = 310;
/// A new character's look on the creation screen: body (index into
/// `launcher::BODIES`), coat dye (index into `DYES`) and hair value.
pub type LookKey = (usize, usize, u16);

/// The coat every new character is given (any profession, level 1).
pub const STARTER_COAT: u32 = 132_003;
/// The dyes offered for it (the Tinter's 3..=9; 3 is its own colour).
pub const DYES: [u8; 7] = [3, 4, 5, 6, 7, 8, 9];

/// What the loading thread hands over.
pub enum Loaded {
    /// A profession's hero in its showcase outfit.
    Hero { body: usize, class: usize, hero: Hero },
    /// A body in the starter coat with dye `DYES[dye]`, and that coat's
    /// colour for its swatch.
    Coat { body: usize, dye: usize, hero: Hero, colour: [u8; 3] },
    /// Face portraits by face number, `launcher::FACE_SIZE` square.
    Faces(Vec<(u16, RgbaImage)>),
    /// The swatch of each of `launcher::HAIR_COLOURS` for a body.
    HairColours { body: usize, colours: Vec<[u8; 3]> },
    /// A body in the starter coat with another hair, asked for by the
    /// creation screen.
    Look { key: LookKey, hero: Hero },
}

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

    /// `body` dressed as a player is seen in the world: what the website's
    /// Armory turns around. A piece the installation cannot draw is left
    /// off, the rest kept.
    pub fn load_worn(assets: &mut Assets, body: u32, [armor, helmet, right, left]: [u32; 4], hair: u16) -> Option<Self> {
        let character = [
            (armor, helmet, right, left),
            (armor, 0, right, left),
            (armor, helmet, right, 0),
            (armor, 0, right, 0),
            (armor, 0, 0, 0),
            (0, 0, 0, 0),
        ]
        .into_iter()
        .find_map(|(armor, helmet, right, left)| {
            Character::load_remote(assets, body, armor, helmet, hair, right, left, String::new(), 1).ok()
        })?;
        let textures = character
            .parts
            .iter()
            .filter_map(|part| Some((part.texture.clone(), assets.image(&part.texture).ok()?)))
            .collect();
        Some(Self { character, textures })
    }

    /// `body` wearing `armor` (a coat or a garment) and nothing else: the
    /// garment previews.
    pub fn load_armor(assets: &mut Assets, body: u32, armor: u32) -> Option<Self> {
        let character = Character::load_remote(assets, body, armor, 0, HAIR, 0, 0, String::new(), 1).ok()?;
        let textures = character
            .parts
            .iter()
            .filter_map(|part| Some((part.texture.clone(), assets.image(&part.texture).ok()?)))
            .collect();
        Some(Self { character, textures })
    }

    /// The textures of its parts, by path.
    pub fn part_textures(&self) -> Vec<(String, RgbaImage)> {
        self.character
            .parts
            .iter()
            .filter_map(|part| Some((part.texture.clone(), self.textures.get(&part.texture)?.clone())))
            .collect()
    }

    /// `body` in the starter coat dyed `dye`, with nothing else worn: what
    /// a new character looks like.
    pub fn load_coat(assets: &mut Assets, body: u32, dye: u8) -> Option<Self> {
        Self::load_look(assets, body, dye, HAIR)
    }

    /// `body` in the starter coat dyed `dye`, with hair `hair`.
    pub fn load_look(assets: &mut Assets, body: u32, dye: u8, hair: u16) -> Option<Self> {
        let armor = crate::character::dyed(STARTER_COAT, dye);
        let character = Character::load_remote(assets, body, armor, 0, hair, 0, 0, String::new(), 1).ok()?;
        let textures = character
            .parts
            .iter()
            .filter_map(|part| Some((part.texture.clone(), assets.image(&part.texture).ok()?)))
            .collect();
        Some(Self { character, textures })
    }

    /// The colour of the cloth the dye changes on part `index`: the
    /// average of the texels that differ from `other`'s (the same coat in
    /// another dye), so skin, trim and shading stay out of the swatch.
    fn dyed_colour(&self, other: &Hero, index: usize) -> Option<[u8; 3]> {
        let mine = self.textures.get(&self.character.parts.get(index)?.texture)?;
        let theirs = other.textures.get(&other.character.parts.get(index)?.texture)?;
        if mine.dimensions() != theirs.dimensions() {
            return None;
        }
        let mut sum = [0.0f32; 3];
        let mut count = 0.0;
        for (a, b) in mine.pixels().zip(theirs.pixels()) {
            let difference: i32 = (0..3).map(|c| (i32::from(a[c]) - i32::from(b[c])).abs()).sum();
            if a[3] < 128 || difference < 60 {
                continue;
            }
            sum = [0, 1, 2].map(|c| sum[c] + f32::from(a[c]));
            count += 1.0;
        }
        (count > 0.0).then(|| sum.map(|channel| (channel / count) as u8))
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

/// The face portraits the creation screen offers, both sexes.
fn faces(assets: &mut Assets) -> Vec<(u16, RgbaImage)> {
    let size = crate::launcher::FACE_SIZE;
    crate::launcher::MALE_FACES
        .chain(crate::launcher::FEMALE_FACES)
        .filter_map(|face| {
            let image = assets
                .image(&format!("data/PlayerFace/JPG/64/{face}.jpg"))
                .ok()
                .map(|image| image::imageops::resize(&image, size, size, image::imageops::FilterType::Triangle))
                .or_else(|| crate::renderer::player_face(assets, u32::from(face) * 10_000 + 1003, size))?;
            Some((face, image))
        })
        .collect()
}

/// The colour of each of `launcher::HAIR_COLOURS` on `body` (1..=4): the
/// average of the opaque, not too dark texels of the hair's texture
/// (`ini/armet.ini` entry `<body>119<colour>10`).
fn hair_colours(assets: &mut Assets, body: u32) -> Vec<[u8; 3]> {
    let catalogs = assets.read("ini/armet.ini").ok().zip(assets.read("ini/3dtexture.ini").ok());
    let Some((armet, textures)) = catalogs else {
        return vec![[128, 128, 128]; crate::launcher::HAIR_COLOURS.len()];
    };
    let armet = sandbox_data::ini::SectionCatalog::parse(&armet);
    let textures = sandbox_data::ini::IdPathCatalog::parse(&textures);
    crate::launcher::HAIR_COLOURS
        .iter()
        .map(|&colour| {
            let key = body * 1_000_000 + 119_000 + u32::from(colour) * 100 + 10;
            armet
                .section(&key.to_string())
                .and_then(|entry| entry.first("Texture0")?.parse::<u32>().ok())
                .and_then(|texture| textures.get(texture).cloned())
                .and_then(|path| assets.image(path.as_str()).ok())
                .and_then(|image| {
                    let mut sum = [0.0f32; 3];
                    let mut count = 0.0;
                    for pixel in image.pixels() {
                        let light = u32::from(pixel[0]) + u32::from(pixel[1]) + u32::from(pixel[2]);
                        if pixel[3] < 128 || light < 36 {
                            continue;
                        }
                        sum = [0, 1, 2].map(|c| sum[c] + f32::from(pixel[c]));
                        count += 1.0;
                    }
                    (count > 0.0).then(|| sum.map(|channel| (channel / count) as u8))
                })
                .unwrap_or([128, 128, 128])
        })
        .collect()
}

/// Loads the look the creation screen asked for and hands it over;
/// `false` once the screen is gone.
fn send_look(assets: &mut Assets, bodies: &[u32; 4], sender: &std::sync::mpsc::Sender<Loaded>, key: LookKey) -> bool {
    let (body, dye, hair) = key;
    let (Some(&model), Some(&dye_value)) = (bodies.get(body), DYES.get(dye)) else {
        return true;
    };
    match Hero::load_look(assets, model, dye_value, hair) {
        Some(hero) => sender.send(Loaded::Look { key, hero }).is_ok(),
        None => true,
    }
}

/// Loads the face portraits, the hair colours, every profession's hero
/// for every body, then every body in the starter coat in each dye, on a
/// thread; each arrives as it is ready. Looks with another hair are
/// loaded when the returned sender asks for them (the latest request
/// first, even while the rest is still loading).
pub fn start(root: std::path::PathBuf) -> (std::sync::mpsc::Receiver<Loaded>, std::sync::mpsc::Sender<LookKey>) {
    let (sender, receiver) = std::sync::mpsc::channel();
    let (look_sender, looks) = std::sync::mpsc::channel::<LookKey>();
    std::thread::spawn(move || {
        let Ok(mut assets) = Assets::open(&root) else {
            return;
        };
        // Bodies 1003/1004 are the male models 3 and 4, 2001/2002 the
        // female 1 and 2.
        let bodies = crate::launcher::BODIES.map(|(_, mesh)| u32::from(mesh % 10));
        // Only the latest request matters: the screen has moved on from
        // the others.
        macro_rules! serve_looks {
            () => {
                if let Some(key) = looks.try_iter().last()
                    && !send_look(&mut assets, &bodies, &sender, key)
                {
                    return;
                }
            };
        }
        if sender.send(Loaded::Faces(faces(&mut assets))).is_err() {
            return;
        }
        for (body_index, body) in bodies.into_iter().enumerate() {
            let colours = hair_colours(&mut assets, body);
            if sender.send(Loaded::HairColours { body: body_index, colours }).is_err() {
                return;
            }
        }
        for (body_index, body) in bodies.into_iter().enumerate() {
            for class in 0..crate::launcher::CLASSES.len() {
                serve_looks!();
                if let Some(hero) = Hero::load(&mut assets, body, class)
                    && sender.send(Loaded::Hero { body: body_index, class, hero }).is_err()
                {
                    return;
                }
            }
        }
        for (body_index, body) in bodies.into_iter().enumerate() {
            serve_looks!();
            let coats: Vec<Option<Hero>> = DYES.iter().map(|&dye| Hero::load_coat(&mut assets, body, dye)).collect();
            // The coat is the part whose texture changes with the dye.
            let coat_part = coats[0].as_ref().zip(coats[1].as_ref()).and_then(|(plain, dyed)| {
                (0..plain.character.parts.len().min(dyed.character.parts.len()))
                    .find(|&index| plain.character.parts[index].texture != dyed.character.parts[index].texture)
            });
            // Each dye's cloth is measured against another dye of the same
            // coat (the plain one against the first dye).
            let colours: Vec<[u8; 3]> = (0..coats.len())
                .map(|dye| {
                    let other = if dye == 0 { 1 } else { 0 };
                    coats[dye]
                        .as_ref()
                        .zip(coats.get(other).and_then(Option::as_ref))
                        .zip(coat_part)
                        .and_then(|((hero, other), index)| hero.dyed_colour(other, index))
                        .unwrap_or([160, 160, 160])
                })
                .collect();
            for (dye, hero) in coats.into_iter().enumerate() {
                let Some(hero) = hero else { continue };
                let colour = colours[dye];
                if sender.send(Loaded::Coat { body: body_index, dye, hero, colour }).is_err() {
                    return;
                }
            }
        }
        // Then wait for the screen's requests until it closes.
        while let Ok(key) = looks.recv() {
            let key = looks.try_iter().last().unwrap_or(key);
            if !send_look(&mut assets, &bodies, &sender, key) {
                return;
            }
        }
    });
    (receiver, look_sender)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `GARMENT_PREVIEW=dir cargo test -p conquer-sandbox garment_preview`:
    /// each garment (`GARMENT_IDS`, comma separated, or the originals) worn
    /// by a man and a woman, from the front and the back, in one sheet; and
    /// each one's texture.
    #[test]
    fn garment_preview() {
        let Ok(dir) = std::env::var("GARMENT_PREVIEW") else { return };
        let mut assets = Assets::open(std::path::Path::new("R:/5065")).expect("assets");
        let ids: Vec<u32> = std::env::var("GARMENT_IDS")
            .ok()
            .map(|ids| ids.split(',').filter_map(|id| id.trim().parse().ok()).collect())
            .unwrap_or_else(|| {
                [181305, 181315, 181325, 181335, 181345, 181355, 181365, 181375, 181385, 181395, 182305, 182315, 182325, 182335, 182345, 182355, 182365, 182385]
                    .to_vec()
            });
        let cell = [160u32, 220u32];
        let mut sheet = RgbaImage::from_pixel(cell[0] * 4, cell[1] * ids.len() as u32, Rgba([40, 60, 45, 255]));
        for (row, id) in ids.iter().enumerate() {
            for (column, (body, facing)) in [(1, 0.6), (1, 0.6 + std::f32::consts::PI), (3, 0.6), (3, 0.6 + std::f32::consts::PI)]
                .into_iter()
                .enumerate()
            {
                let Some(hero) = Hero::load_armor(&mut assets, body, *id) else {
                    continue;
                };
                let image = hero.render(cell, 190.0, 0.0, facing);
                image::imageops::overlay(&mut sheet, &image, i64::from(column as u32 * cell[0]), i64::from(row as u32 * cell[1]));
                if column == 0 {
                    for (index, (_, texture)) in hero.part_textures().into_iter().enumerate() {
                        texture.save(format!("{dir}/garment-{id}-texture{index}.png")).expect("texture");
                    }
                }
            }
        }
        sheet.save(format!("{dir}/garments.png")).expect("sheet");
    }

    /// `GARMENT_HEAD=dir GARMENT_IDS=181335 cargo test -p conquer-sandbox
    /// garment_head`: each body in each garment without its headdress
    /// (`Character::without_headdress`, what `hide_head` does), from the
    /// front and the back, `cut-<id>-<body>.png`; and `head-<id>-<body>.png`,
    /// the texels of the headdress (white), of the rest (grey), and where
    /// they meet (red).
    #[test]
    fn garment_head() {
        let Ok(dir) = std::env::var("GARMENT_HEAD") else { return };
        let mut assets = Assets::open(std::path::Path::new("R:/5065")).expect("assets");
        let ids: Vec<u32> =
            std::env::var("GARMENT_IDS").unwrap_or_default().split(',').filter_map(|id| id.trim().parse().ok()).collect();
        for id in ids {
            for body in [1, 2, 3, 4] {
                let Some(mut hero) = Hero::load_armor(&mut assets, body, id) else { continue };
                let texture = hero.textures[&hero.character.parts[0].texture].clone();
                let (part, headdress, rest) = hero.character.without_headdress(&hero.character.parts[0], &texture);
                let cut = (hero.character.parts[0].mesh.opaque_indices.len() + hero.character.parts[0].mesh.alpha_indices.len()
                    - part.mesh.opaque_indices.len()
                    - part.mesh.alpha_indices.len())
                    / 3;
                hero.character.parts[0] = part;
                let mut mask = RgbaImage::from_pixel(texture.width(), texture.height(), Rgba([0, 0, 0, 255]));
                for (x, y) in rest {
                    mask.put_pixel(x, y, Rgba([90, 90, 90, 255]));
                }
                let mut shared = 0;
                for (x, y) in headdress {
                    let grey = mask.get_pixel(x, y)[1] == 90;
                    shared += usize::from(grey);
                    mask.put_pixel(x, y, if grey { Rgba([255, 0, 0, 255]) } else { Rgba([255, 255, 255, 255]) });
                }
                println!("{id} body {body}: {cut} triangles cut, {shared} samples shared");
                mask.save(format!("{dir}/head-{id}-{body}.png")).expect("mask");
                let cell = [160u32, 220u32];
                let mut sheet = RgbaImage::from_pixel(cell[0] * 2, cell[1], Rgba([40, 60, 45, 255]));
                for (column, facing) in [0.6, 0.6 + std::f32::consts::PI].into_iter().enumerate() {
                    image::imageops::overlay(&mut sheet, &hero.render(cell, 190.0, 0.0, facing), i64::from(column as u32 * cell[0]), 0);
                }
                sheet.save(format!("{dir}/cut-{id}-{body}.png")).expect("sheet");
            }
        }
    }

    /// `WEAPON_PREVIEW=dir WEAPON_IDS=500229,500909 cargo test -p
    /// conquer-sandbox weapon_preview`: each weapon held by a man in the
    /// archer's coat, from the front, the side and the back, in one sheet;
    /// and each one's texture.
    #[test]
    fn weapon_preview() {
        let Ok(dir) = std::env::var("WEAPON_PREVIEW") else { return };
        let mut assets = Assets::open(std::path::Path::new("R:/5065")).expect("assets");
        let ids: Vec<u32> = std::env::var("WEAPON_IDS")
            .unwrap_or_default()
            .split(',')
            .filter_map(|id| id.trim().parse().ok())
            .collect();
        let cell: [u32; 2] = std::env::var("WEAPON_CELL").ok().and_then(|size| size.parse().ok()).map_or([200, 240], |height: u32| [height * 5 / 6, height]);
        let mut sheet = RgbaImage::from_pixel(cell[0] * 3, cell[1] * ids.len().max(1) as u32, Rgba([40, 60, 45, 255]));
        for (row, id) in ids.iter().enumerate() {
            let Ok(character) = Character::load_remote(&mut assets, 3, STARTER_COAT, 0, HAIR, *id, 0, String::new(), 1) else {
                continue;
            };
            let textures: HashMap<String, RgbaImage> = character
                .parts
                .iter()
                .filter_map(|part| Some((part.texture.clone(), assets.image(&part.texture).ok()?)))
                .collect();
            let hero = Hero { character, textures };
            for (column, facing) in [0.6, 0.6 + std::f32::consts::FRAC_PI_2, 0.6 + std::f32::consts::PI].into_iter().enumerate() {
                let image = hero.render(cell, cell[1] as f32 * 0.71, 0.0, facing);
                image::imageops::overlay(&mut sheet, &image, i64::from(column as u32 * cell[0]), i64::from(row as u32 * cell[1]));
            }
            if let Some((_, texture)) = hero.part_textures().into_iter().find(|(path, _)| path.contains("weapon") || path.contains("gravak")) {
                texture.save(format!("{dir}/weapon-{id}-texture.png")).expect("texture");
            }
        }
        sheet.save(format!("{dir}/weapons.png")).expect("sheet");
    }
}

#[cfg(test)]
mod texture_dump {
    /// `TEXTURE_DUMP=dir TEXTURE_PATHS=a.dds,b.dds cargo test -p conquer-sandbox texture_dump`
    #[test]
    fn texture_dump() {
        let Ok(dir) = std::env::var("TEXTURE_DUMP") else { return };
        let mut assets = crate::assets::Assets::open(std::path::Path::new("R:/5065")).expect("assets");
        for path in std::env::var("TEXTURE_PATHS").unwrap_or_default().split(',') {
            let image = assets.image(path).expect(path);
            let name = path.rsplit('/').next().unwrap_or(path).replace(".dds", ".png");
            image.save(format!("{dir}/{name}")).expect("png");
        }
    }
}

/// The Rifts' creatures in their poses, side by side: standing, walking,
/// striking (three moments) and fallen, to check a model from Meshy before
/// going in the game. `GRAVAK_CONTENT=... cargo test -p conquer-sandbox
/// creature_poses -- --ignored`, sheets in `marketing/criaturas`.
#[cfg(test)]
#[test]
#[ignore = "Needs the 5065 installation and the built content"]
fn creature_poses() {
    let mut assets = Assets::open(std::path::Path::new("R:/5065")).expect("assets");
    let catalog = crate::character::NpcCatalog::load(&mut assets).expect("catalog");
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../marketing/criaturas");
    std::fs::create_dir_all(&out).expect("folder");
    let looks: Vec<u32> = std::env::var("LOOKS")
        .map(|looks| looks.split(',').filter_map(|look| look.parse().ok()).collect())
        .unwrap_or_else(|_| vec![942, 943, 944, 949]);
    const CELL: [u32; 2] = [260, 300];
    for look in looks {
        let model = crate::character::NpcModel::load_monster(&mut assets, &catalog, look).expect("model");
        let texture = assets.image(&model.texture).expect("texture");
        let facing = 0.8;
        let poses = [
            model.vertices(0.5, facing),
            model.vertices_walk(0.3, facing),
            model.vertices_attack(0.15, facing),
            model.vertices_attack(0.4, facing),
            model.vertices_attack(0.65, facing),
            model.vertices_dying(0, 2.0, facing),
        ];
        // One scale for all, from the standing pose, so sizes compare.
        let (top, bottom) = poses[0].iter().fold((f32::MAX, f32::MIN), |(t, b), (p, _)| (t.min(p[1]), b.max(p[1])));
        let scale = CELL[1] as f32 * 0.6 / (bottom - top).max(1.0);
        let mut sheet = RgbaImage::from_pixel(CELL[0] * poses.len() as u32, CELL[1], Rgba([40, 44, 52, 255]));
        for (index, vertices) in poses.iter().enumerate() {
            let mut colour = vec![[0.0f32; 4]; (CELL[0] * CELL[1]) as usize];
            let mut depth = vec![f32::INFINITY; (CELL[0] * CELL[1]) as usize];
            let origin = [CELL[0] as f32 * 0.5, CELL[1] as f32 * 0.85];
            for pass in [false, true] {
                let indices = if pass { &model.mesh.alpha_indices } else { &model.mesh.opaque_indices };
                for triangle in indices.chunks_exact(3) {
                    let corners = [0, 1, 2].map(|k| {
                        let (p, uv) = vertices[triangle[k] as usize];
                        ([origin[0] + p[0] * scale, origin[1] + p[1] * scale, p[2]], uv)
                    });
                    rasterise(&mut colour, &mut depth, CELL, corners, &texture, pass);
                }
            }
            for y in 0..CELL[1] {
                for x in 0..CELL[0] {
                    let [r, g, b, a] = colour[(y * CELL[0] + x) as usize];
                    if a > 0.0 {
                        let pixel = sheet.get_pixel_mut(index as u32 * CELL[0] + x, y);
                        let mix = |c: f32, under: u8| ((c.min(255.0 * a)) + f32::from(under) * (1.0 - a)).min(255.0) as u8;
                        *pixel = Rgba([mix(r, pixel[0]), mix(g, pixel[1]), mix(b, pixel[2]), 255]);
                    }
                }
            }
        }
        sheet.save(out.join(format!("{look}.png"))).expect("save");
    }
}
