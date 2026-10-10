// Source: @digitalm1nd on x.com / _dek on Discord — Preservation Conquer project — https://discord.gg/CvKPXEHYRY
use crate::{assets::Result, character::Character};
use image::{Pixel, Rgba, RgbaImage};
use std::sync::{Arc, OnceLock};

const FONT_SIZE: f32 = 12.0;
const GUILD_COLOUR: [u8; 3] = [255, 255, 0];
const RANK_COLOUR: [u8; 3] = [145, 185, 255];
const CINZEL_DECORATIVE: &[u8] = include_bytes!("ui_assets/CinzelDecorative-Regular.ttf");

static UI_FONT: OnceLock<std::result::Result<Arc<fontdue::Font>, String>> = OnceLock::new();
static DECORATIVE_FONT: OnceLock<std::result::Result<Arc<fontdue::Font>, String>> = OnceLock::new();

fn load_ui_font() -> Result<fontdue::Font> {
    let mut database = fontdb::Database::new();
    database.load_system_fonts();
    let id = database
        .query(&fontdb::Query {
            families: &[fontdb::Family::Name("Arial"), fontdb::Family::SansSerif],
            ..Default::default()
        })
        .ok_or("No system sans-serif font available")?;
    Ok(database
        .with_face_data(id, |bytes, index| {
            fontdue::Font::from_bytes(
                bytes,
                fontdue::FontSettings {
                    collection_index: index,
                    ..Default::default()
                },
            )
        })
        .ok_or("Font unavailable")??)
}

pub(crate) fn font() -> Result<Arc<fontdue::Font>> {
    UI_FONT
        .get_or_init(|| {
            load_ui_font()
                .map(Arc::new)
                .map_err(|error| error.to_string())
        })
        .as_ref()
        .map(Arc::clone)
        .map_err(|error| std::io::Error::other(error.clone()).into())
}

pub(crate) fn decorative_font() -> Result<Arc<fontdue::Font>> {
    DECORATIVE_FONT
        .get_or_init(|| {
            fontdue::Font::from_bytes(CINZEL_DECORATIVE, fontdue::FontSettings::default())
                .map(Arc::new)
                .map_err(|error| error.to_string())
        })
        .as_ref()
        .map(Arc::clone)
        .map_err(|error| std::io::Error::other(error.clone()).into())
}

fn width(font: &fontdue::Font, text: &str) -> f32 {
    text.chars()
        .map(|c| font.metrics(c, FONT_SIZE).advance_width)
        .sum()
}

pub(crate) fn text(
    image: &mut RgbaImage,
    font: &fontdue::Font,
    spans: &[(&str, [u8; 3])],
    origin: [f32; 2],
) {
    text_sized(image, font, spans, origin, FONT_SIZE);
}

pub(crate) fn text_sized(
    image: &mut RgbaImage,
    font: &fontdue::Font,
    spans: &[(&str, [u8; 3])],
    origin: [f32; 2],
    font_size: f32,
) {
    for shadow in [true, false] {
        let mut cursor = origin[0].round();
        for &(text, colour) in spans {
            // A black name keeps a grey shadow, as the original's (0x4b5f1c).
            let dark = colour.iter().all(|channel| *channel < 0x20);
            let colour = match (shadow, dark) {
                (true, true) => [0xaa, 0xaa, 0xaa],
                (true, false) => [0, 0, 0],
                (false, _) => colour,
            };
            let offset = i32::from(shadow);
            for character in text.chars() {
                let (metrics, bitmap) = font.rasterize(character, font_size);
                for row in 0..metrics.height {
                    for column in 0..metrics.width {
                        let x = cursor.round() as i32 + metrics.xmin + column as i32 + offset;
                        let y = origin[1].round() as i32 + font_size.round() as i32
                            - metrics.ymin
                            - metrics.height as i32
                            + row as i32
                            + offset;
                        if x < 0 || y < 0 || x >= image.width() as i32 || y >= image.height() as i32
                        {
                            continue;
                        }
                        let alpha = bitmap[row * metrics.width + column];
                        let source = Rgba([colour[0], colour[1], colour[2], alpha]);
                        image.get_pixel_mut(x as u32, y as u32).blend(&source);
                    }
                }
                cursor += metrics.advance_width;
            }
        }
    }
}

const BLUE_FLASHING_NAME: u64 = 1 << 0;
const RED_NAME: u64 = 1 << 14;
const BLACK_NAME: u64 = 1 << 15;

/// The name colour the status flags give a player: black or red by PK
/// points, and while flashing (after an attack or a kill) blue blinking
/// twice a second against red, or against black for a black name.
/// A seller's name while their booth is open.
pub const VENDOR_NAME: [u8; 3] = [255, 128, 208];

/// The name's colour: pink while selling, else by PK state.
pub fn vendor_or_name_colour(status_flags: u64, selling: bool) -> [u8; 3] {
    if selling { VENDOR_NAME } else { name_colour(status_flags) }
}

pub fn name_colour(status_flags: u64) -> [u8; 3] {
    let blink_on = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() / 500 % 2 == 0)
        .unwrap_or(true);
    let pk_colour = if status_flags & BLACK_NAME != 0 {
        [0, 0, 0]
    } else if status_flags & RED_NAME != 0 {
        [255, 0, 0]
    } else {
        [255, 255, 255]
    };
    if status_flags & BLUE_FLASHING_NAME == 0 {
        pk_colour
    } else if blink_on {
        [0, 160, 255]
    } else if status_flags & BLACK_NAME != 0 {
        pk_colour
    } else {
        [255, 0, 0]
    }
}

pub fn image(character: &Character, name_colour: [u8; 3]) -> Result<RgbaImage> {
    let font = font()?;
    let mut image = RgbaImage::new(256, 40);
    let separator = if character.guild.is_empty() || character.guild_rank.is_empty() {
        ""
    } else {
        " "
    };
    let guild_width = width(&font, &character.guild)
        + width(&font, separator)
        + width(&font, &character.guild_rank);
    text(
        &mut image,
        &font,
        &[
            (&character.guild, GUILD_COLOUR),
            (separator, GUILD_COLOUR),
            (&character.guild_rank, RANK_COLOUR),
        ],
        [(256.0 - guild_width) * 0.5, 0.0],
    );
    text(
        &mut image,
        &font,
        &[(&character.name, name_colour)],
        [(256.0 - width(&font, &character.name)) * 0.5, 13.0],
    );
    for y in 28..34 {
        for x in 102..154 {
            image.put_pixel(x, y, Rgba([0, 0, 0, 255]));
        }
    }
    let fraction =
        character.health as f32 / character.maximum_health.max(character.health).max(1) as f32;
    let filled_width = (50.0 * fraction).round() as u32;
    for y in 29..33 {
        for x in 103..103 + filled_width {
            image.put_pixel(x, y, Rgba([34, 190, 62, 255]));
        }
    }
    Ok(image)
}

/// The war shields beside a name, as Tibia's: green for the hero's own
/// guild at war, red for a guild at war with it, blue for an ally.
pub const SHIELD_OWN: u8 = 1;
pub const SHIELD_ENEMY: u8 = 2;
pub const SHIELD_ALLY: u8 = 3;
/// How far below the label's usual bottom a shield hangs.
pub const SHIELD_DROP: u32 = 5;
/// How wide a shield and its gap are.
const SHIELD_SPACE: u32 = 16;

/// The shield's size in the label, in pixels.
pub const SHIELD_SIZE: [u32; 2] = [14, 17];

/// The shield's half width at depth `v` (0 at the top, 1 at the point), in
/// units of the full half width: straight sides to 42%, then curving in to
/// the point.
fn shield_half(v: f32) -> f32 {
    if v < 0.42 {
        1.0
    } else {
        let t = ((v - 0.42) / 0.58).clamp(0.0, 1.0);
        (1.0 - t.powf(1.7)).max(0.0)
    }
}

/// Whether the point `(x, y)` (pixels from the shield box's top left, in a
/// box `w` by `h`) lies inside the shield `inset` pixels in from its edge.
/// The top arches down a little at the corners.
fn in_shield(x: f32, y: f32, w: f32, h: f32, inset: f32) -> bool {
    let half_w = w * 0.5;
    let u = (x - half_w) / half_w;
    let top = 0.6 + 1.6 * u * u + inset;
    if y < top || y > h - inset * 1.6 {
        return false;
    }
    let v = (y / h).clamp(0.0, 1.0);
    (x - half_w).abs() <= shield_half(v) * half_w - inset
}

/// The symbol: a star for the hero's guild, crossed swords for an enemy, a
/// cross for an ally; `(x, y)` from the field's centre in pixels, `r` its
/// reach.
fn in_symbol(shield: u8, x: f32, y: f32, r: f32) -> bool {
    let (x, y) = (x / r, y / r);
    match shield {
        SHIELD_OWN => {
            let angle = x.atan2(-y);
            let radius = (x * x + y * y).sqrt();
            let step = std::f32::consts::TAU / 5.0;
            let sector = (angle.rem_euclid(step) / step - 0.5).abs() * 2.0;
            radius < 0.42 + 0.58 * (1.0 - sector).powf(2.2)
        },
        SHIELD_ENEMY => {
            let blade = |a: f32, b: f32| (a - b).abs() < 0.24 && a.abs() < 0.95 && b.abs() < 0.95;
            blade(x, y) || blade(x, -y)
        },
        SHIELD_ALLY => (x.abs() < 0.26 && y.abs() < 0.95) || (y.abs() < 0.26 && x.abs() < 0.8),
        _ => false,
    }
}

fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

/// A war shield of `shield`'s colours, `scale` times [`SHIELD_SIZE`], drawn
/// at four times that and brought down so its edges are smooth: a dark
/// outline, a gold rim lit from the upper left, the field in a gradient
/// with a shine, the symbol in ivory with a dark edge, and a soft shadow.
pub fn shield_image_scaled(shield: u8, scale: u32) -> Option<RgbaImage> {
    let (light, dark) = match shield {
        SHIELD_OWN => ([120.0, 236.0, 120.0], [16.0, 104.0, 36.0]),
        SHIELD_ENEMY => ([255.0, 110.0, 92.0], [128.0, 8.0, 14.0]),
        SHIELD_ALLY => ([120.0, 180.0, 255.0], [18.0, 48.0, 146.0]),
        _ => return None,
    };
    const SAMPLES: u32 = 4;
    let [width, height] = SHIELD_SIZE.map(|value| value * scale.max(1));
    let unit = scale.max(1) as f32;
    // The shield fills its box less a pixel right and below for the shadow.
    let (w, h) = (width as f32 - unit, height as f32 - unit);
    let outline = [26.0, 16.0, 6.0];
    let rim_light = [255.0, 232.0, 150.0];
    let rim_dark = [150.0, 98.0, 30.0];
    let ivory = [255.0, 250.0, 232.0];
    let mut image = RgbaImage::new(width, height);
    for py in 0..height {
        for px in 0..width {
            let (mut colour, mut cover, mut shadow) = ([0.0_f32; 3], 0.0_f32, 0.0_f32);
            for sy in 0..SAMPLES {
                for sx in 0..SAMPLES {
                    let x = px as f32 + (sx as f32 + 0.5) / SAMPLES as f32;
                    let y = py as f32 + (sy as f32 + 0.5) / SAMPLES as f32;
                    if !in_shield(x, y, w, h, 0.0) {
                        if in_shield(x - unit, y - unit, w, h, 0.0) {
                            shadow += 1.0;
                        }
                        continue;
                    }
                    cover += 1.0;
                    let (cx, cy) = (x - w * 0.5, y - h * 0.44);
                    let reach = w * 0.3;
                    let sample = if !in_shield(x, y, w, h, 0.8 * unit) {
                        outline
                    } else if !in_shield(x, y, w, h, 1.55 * unit) {
                        mix(rim_light, rim_dark, ((y / h) * 0.9 + (x / w) * 0.4).clamp(0.0, 1.0))
                    } else if !in_shield(x, y, w, h, 1.9 * unit) {
                        mix(dark, outline, 0.5)
                    } else if in_symbol(shield, cx, cy, reach) {
                        mix(ivory, [230.0, 214.0, 170.0], (cy / reach * 0.5 + 0.5).clamp(0.0, 1.0))
                    } else if in_symbol(shield, cx, cy, reach + 0.8 * unit) {
                        mix(dark, outline, 0.55)
                    } else {
                        let base = mix(light, dark, ((y / h) * 1.05 + (x / w - 0.5) * 0.3).clamp(0.0, 1.0));
                        let shine = (1.0 - (((x / w - 0.32) / 0.2).powi(2) + ((y / h - 0.24) / 0.13).powi(2))).max(0.0);
                        mix(base, [255.0, 255.0, 255.0], shine * 0.5)
                    };
                    for channel in 0..3 {
                        colour[channel] += sample[channel];
                    }
                }
            }
            let samples = (SAMPLES * SAMPLES) as f32;
            if cover > 0.0 {
                let alpha = cover / samples;
                let shade = shadow / samples * 0.5;
                let rgb = colour.map(|value| (value / cover).round().clamp(0.0, 255.0) as u8);
                let total = alpha + shade * (1.0 - alpha);
                image.put_pixel(px, py, Rgba([rgb[0], rgb[1], rgb[2], (total * 255.0).round() as u8]));
            } else if shadow > 0.0 {
                image.put_pixel(px, py, Rgba([0, 0, 0, (shadow / samples * 0.5 * 255.0).round() as u8]));
            }
        }
    }
    Some(image)
}

/// The war shield at its size over names.
pub fn shield_image(shield: u8) -> Option<RgbaImage> {
    shield_image_scaled(shield, 1)
}

/// The war shield with its top left at `(x, y)`.
fn draw_shield(image: &mut RgbaImage, shield: u8, x: u32, y: u32) {
    if let Some(art) = shield_image(shield) {
        image::imageops::overlay(image, &art, i64::from(x), i64::from(y));
    }
}

/// A player's name with their guild and rank over it, as the hero's own
/// label: 13 pixels taller when there is a guild (the name stays at the
/// bottom, where a plain one is). A war shield goes right of the name.
pub fn actor_image_with_guild(name: &str, name_colour: [u8; 3], guild: &str, rank: &str, shield: u8) -> Result<RgbaImage> {
    let font = font()?;
    let shield_space = if shield == 0 { 0 } else { SHIELD_SPACE };
    if guild.is_empty() {
        // A shield hangs a little below the name: the label is
        // [`SHIELD_DROP`] pixels taller at the bottom (the renderer keeps the
        // name in place).
        let drop = if shield == 0 { 0 } else { SHIELD_DROP };
        let name_width = width(&font, name).ceil() as u32;
        let mut image = RgbaImage::new((name_width + 11 + shield_space).max(16), 16 + drop);
        text(&mut image, &font, &[(name, name_colour)], [4.0, 2.0]);
        draw_shield(&mut image, shield, name_width + 9, 4);
        return Ok(image);
    }
    let separator = if rank.is_empty() { "" } else { " " };
    let guild_width = width(&font, guild) + width(&font, separator) + width(&font, rank);
    let name_width = width(&font, name) + shield_space as f32;
    let image_width = (guild_width.max(name_width).ceil() as u32 + 8).max(16);
    let drop = if shield == 0 { 0 } else { SHIELD_DROP };
    let image_width = image_width + if shield == 0 { 0 } else { 6 };
    let mut image = RgbaImage::new(image_width, 29 + drop);
    text(
        &mut image,
        &font,
        &[(guild, GUILD_COLOUR), (separator, GUILD_COLOUR), (rank, RANK_COLOUR)],
        [(image_width as f32 - guild_width) * 0.5, 2.0],
    );
    let name_left = (image_width as f32 - name_width) * 0.5;
    text(&mut image, &font, &[(name, name_colour)], [name_left, 15.0]);
    draw_shield(&mut image, shield, (name_left + name_width - shield_space as f32 + 5.0).max(0.0) as u32, 17);
    Ok(image)
}

/// How wide a name is written over its owner.
pub fn name_width(name: &str) -> f32 {
    font().map_or(0.0, |font| width(&font, name))
}

/// Side of the face portrait on the hovered player's nameplate.
pub const PLAYER_FACE_SIZE: u32 = 32;

/// Nameplate shown over a hovered player: their face portrait on the left,
/// the name and a small health bar beside it.
pub fn player_plate(
    face: Option<&RgbaImage>,
    name: &str,
    name_colour: [u8; 3],
    health: u16,
    maximum_health: u16,
) -> Result<RgbaImage> {
    let font = font()?;
    let face_box = PLAYER_FACE_SIZE + 2;
    let bar_width = 50u32;
    let text_left = face_box + 4;
    let image_width = text_left + (width(&font, name).ceil() as u32 + 4).max(bar_width + 2);
    let mut image = RgbaImage::new(image_width, face_box);
    for y in 0..face_box {
        for x in 0..face_box {
            let edge = x == 0 || y == 0 || x == face_box - 1 || y == face_box - 1;
            image.put_pixel(
                x,
                y,
                Rgba(if edge { [0, 0, 0, 255] } else { [24, 30, 34, 220] }),
            );
        }
    }
    if let Some(face) = face {
        image::imageops::overlay(&mut image, face, 1, 1);
    }
    text(&mut image, &font, &[(name, name_colour)], [text_left as f32, 2.0]);
    let bar_top = 19;
    for y in bar_top..bar_top + 6 {
        for x in text_left..text_left + bar_width + 2 {
            image.put_pixel(x, y, Rgba([0, 0, 0, 255]));
        }
    }
    let fraction = health as f32 / maximum_health.max(health).max(1) as f32;
    let filled_width = (bar_width as f32 * fraction).round() as u32;
    for y in bar_top + 1..bar_top + 5 {
        for x in text_left + 1..text_left + 1 + filled_width {
            image.put_pixel(x, y, Rgba([34, 190, 62, 255]));
        }
    }
    Ok(image)
}

/// The hovered player's plate with their guild and rank written over it;
/// 14 pixels taller when there is a guild, the plate itself at the bottom.
pub fn player_plate_with_guild(
    face: Option<&RgbaImage>,
    name: &str,
    name_colour: [u8; 3],
    health: u16,
    maximum_health: u16,
    guild: &str,
    rank: &str,
) -> Result<RgbaImage> {
    let plate = player_plate(face, name, name_colour, health, maximum_health)?;
    if guild.is_empty() {
        return Ok(plate);
    }
    let font = font()?;
    let separator = if rank.is_empty() { "" } else { " " };
    let guild_width = width(&font, guild) + width(&font, separator) + width(&font, rank);
    let image_width = plate.width().max(guild_width.ceil() as u32 + 4);
    let mut image = RgbaImage::new(image_width, plate.height() + 14);
    text(
        &mut image,
        &font,
        &[(guild, GUILD_COLOUR), (separator, GUILD_COLOUR), (rank, RANK_COLOUR)],
        [2.0, 0.0],
    );
    image::imageops::overlay(&mut image, &plate, 0, 14);
    Ok(image)
}

/// Compact monster nameplate: the name followed by a small health bar.
/// A monster's name over its life bar; the monsters of the bounty under way
/// have a gold name behind a gold diamond.
/// A monster's name by its level against the hero's, as the original
/// (0x4b5d9e): black five or more above, red above, white down to four
/// below, green five or more below.
pub fn monster_name_colour(monster_level: u16, hero_level: u16) -> [u8; 3] {
    if monster_level >= hero_level + 5 {
        [0, 0, 0]
    } else if monster_level > hero_level {
        [255, 0, 0]
    } else if hero_level < monster_level + 5 {
        [255, 255, 255]
    } else {
        [0, 255, 0]
    }
}

pub fn monster_image(name: &str, name_colour: [u8; 3], health: u16, maximum_health: u16, bounty: bool) -> Result<RgbaImage> {
    let font = font()?;
    let marker = if bounty { 12 } else { 0 };
    let text_width = width(&font, name).ceil() as u32 + 8 + marker;
    let image_width = text_width.max(36);
    let mut image = RgbaImage::new(image_width, 28);
    let name_x = ((image_width as f32 - width(&font, name) + marker as f32) * 0.5).max(2.0 + marker as f32);
    let colour = if bounty { [255, 202, 64] } else { name_colour };
    text(&mut image, &font, &[(name, colour)], [name_x, 1.0]);
    if bounty {
        // A small diamond before the name.
        let (cx, cy) = (name_x as i32 - 8, 8_i32);
        for dy in -4_i32..=4 {
            for dx in -(4 - dy.abs())..=(4 - dy.abs()) {
                let (x, y) = (cx + dx, cy + dy);
                if x >= 0 && (x as u32) < image_width {
                    let edge = dx.abs() + dy.abs() == 4;
                    let shade = if edge { [120, 82, 20, 255] } else { [255, 214, 96, 255] };
                    image.put_pixel(x as u32, y as u32, Rgba(shade));
                }
            }
        }
    }
    let bar_width = image_width.saturating_sub(8).max(24);
    for y in 18..24 {
        for x in 4..4 + bar_width {
            image.put_pixel(x, y, Rgba([0, 0, 0, 255]));
        }
    }
    let fraction = health as f32 / maximum_health.max(health).max(1) as f32;
    let filled_width = (bar_width as f32 * fraction).round() as u32;
    for y in 19..23 {
        for x in 5..5 + filled_width.saturating_sub(1) {
            image.put_pixel(x, y, Rgba([54, 203, 70, 255]));
        }
    }
    Ok(image)
}

pub fn loot_image(value: u32) -> Result<RgbaImage> {
    let font = font()?;
    let label = format!("Silver {value}");
    let size = 13.0;
    let width = label
        .chars()
        .map(|ch| font.metrics(ch, size).advance_width)
        .sum::<f32>()
        .ceil() as u32;
    let mut image = RgbaImage::new(width + 8, 18);
    text(
        &mut image,
        &font,
        &[(label.as_str(), [255, 225, 120])],
        [4.0, 1.0],
    );
    Ok(image)
}

/// A small dark tooltip with a gold border, such as a minimap pin's name.
pub fn tooltip_image(label: &str) -> Result<RgbaImage> {
    let font = font()?;
    let width = width(&font, label).ceil() as u32 + 12;
    let mut image = RgbaImage::from_pixel(width.max(24), 20, image::Rgba([12, 10, 8, 220]));
    let (w, h) = image.dimensions();
    for (x, y, pixel) in image.enumerate_pixels_mut() {
        if x == 0 || y == 0 || x == w - 1 || y == h - 1 {
            *pixel = image::Rgba([196, 160, 90, 255]);
        }
    }
    text(&mut image, &font, &[(label, [255, 236, 170])], [6.0, 3.0]);
    Ok(image)
}

/// A dropped item's name in `colour` (its quality's, `loot_colour`).
pub fn loot_name_image_in(name: &str, colour: [u8; 3]) -> Result<RgbaImage> {
    let font = font()?;
    let width = width(&font, name).ceil() as u32 + 8;
    let mut image = RgbaImage::new(width.max(24), 18);
    text(&mut image, &font, &[(name, colour)], [4.0, 2.0]);
    Ok(image)
}

/// The colour of a dropped item's name, by quality: Super gold, Elite
/// red, Unique blue, Refined cyan; DragonBalls and Meteors yellow; the rest
/// as before.
pub fn loot_colour(item_type: u32) -> [u8; 3] {
    match crate::shop::item_light(item_type) {
        Some("FireLight") => [255, 190, 60],
        Some("RedLight") => [255, 110, 120],
        Some("BlueLight") => [120, 170, 255],
        Some("RoyalBlueLight") => [120, 225, 255],
        Some("YellowLight" | "GoldEnLight") => [255, 230, 90],
        _ => [255, 236, 150],
    }
}

/// Compact, readable status banner used for important client/server feedback.
/// The banner deliberately owns its background so it remains legible over any
/// map texture and does not depend on the chat panel being open.
pub fn status_image(message: &str) -> Result<RgbaImage> {
    let font = font()?;
    let mut visible = message.trim().to_owned();
    if visible.len() > 96 {
        visible = visible.chars().take(95).collect();
        visible.push('…');
    }
    let size = 16.0;
    let text_width = visible
        .chars()
        .map(|character| font.metrics(character, size).advance_width)
        .sum::<f32>();
    let width = (text_width.ceil() as u32 + 28).clamp(220, 760);
    let height = 34;
    let is_error = ["error", "rejected", "falha", "indisponível", "inválido"]
        .iter()
        .any(|marker| visible.to_lowercase().contains(marker));
    let border = if is_error {
        [214, 91, 68, 255]
    } else {
        [219, 174, 81, 255]
    };
    let fill = if is_error {
        [50, 22, 24, 242]
    } else {
        [13, 29, 38, 242]
    };
    let mut image = RgbaImage::from_pixel(width, height, Rgba(fill));
    for x in 0..width {
        image.put_pixel(x, 0, Rgba(border));
        image.put_pixel(x, height - 1, Rgba(border));
    }
    for y in 0..height {
        image.put_pixel(0, y, Rgba(border));
        image.put_pixel(width - 1, y, Rgba(border));
    }
    text(
        &mut image,
        &font,
        &[(
            &visible,
            if is_error {
                [255, 190, 170]
            } else {
                [246, 222, 164]
            },
        )],
        [14.0, 8.0],
    );
    Ok(image)
}

/// Compact diagnostics panel shown by the F11 performance overlay.
pub fn performance_image(
    frame_ms: f64,
    draws: usize,
    vertices: usize,
    textures: usize,
) -> Result<RgbaImage> {
    let font = font()?;
    let fps = if frame_ms > 0.0 {
        1_000.0 / frame_ms
    } else {
        0.0
    };
    let mut image = RgbaImage::from_pixel(232, 94, Rgba([5, 14, 21, 238]));
    for x in 0..image.width() {
        image.put_pixel(x, 0, Rgba([213, 163, 58, 255]));
        image.put_pixel(x, image.height() - 1, Rgba([91, 59, 27, 255]));
    }
    for y in 0..image.height() {
        image.put_pixel(0, y, Rgba([213, 163, 58, 255]));
        image.put_pixel(image.width() - 1, y, Rgba([91, 59, 27, 255]));
    }
    text_sized(
        &mut image,
        &font,
        &[("PERFORMANCE", [255, 224, 146])],
        [10.0, 7.0],
        12.0,
    );
    let lines = [
        format!("Frame: {frame_ms:.2} ms  FPS: {fps:.0}"),
        format!("Draws: {draws}  Vertices: {vertices}"),
        format!("Textures: {textures}"),
    ];
    for (index, line) in lines.iter().enumerate() {
        text_sized(
            &mut image,
            &font,
            &[(line.as_str(), [214, 222, 213])],
            [10.0, 27.0 + index as f32 * 15.0],
            10.0,
        );
    }
    Ok(image)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translucent_text_preserves_the_shadow_under_it() {
        let mut pixel = Rgba([0_u8, 0, 0, 255]);
        pixel.blend(&Rgba([255, 255, 255, 128]));
        assert!(pixel[3] >= 254);
        assert!((127..=129).contains(&pixel[0]));
    }

    #[test]
    fn monster_names_tell_their_level_as_the_original() {
        assert_eq!(monster_name_colour(55, 50), [0, 0, 0]);
        assert_eq!(monster_name_colour(54, 50), [255, 0, 0]);
        assert_eq!(monster_name_colour(51, 50), [255, 0, 0]);
        assert_eq!(monster_name_colour(50, 50), [255, 255, 255]);
        assert_eq!(monster_name_colour(46, 50), [255, 255, 255]);
        assert_eq!(monster_name_colour(45, 50), [0, 255, 0]);
        assert_eq!(monster_name_colour(1, 130), [0, 255, 0]);
    }

    #[test]
    fn guild_and_rank_use_different_client_colours() {
        assert_eq!(GUILD_COLOUR, [255, 255, 0]);
        assert_eq!(RANK_COLOUR, [145, 185, 255]);
    }

    #[test]
    fn status_banner_has_a_bounded_size_for_long_feedback() {
        let short = status_image("online: item equipped").expect("status image");
        let long = status_image(&"x".repeat(400)).expect("status image");
        assert!(short.width() >= 220);
        assert!(long.width() <= 760);
        assert_eq!(short.height(), 34);
    }
}

/// The bubble over someone who speaks on the Talk channel, drawn as the
/// original draws it: a box of faint dark glass at least 153 by 71 with
/// slightly rounded corners and a double edge (dark outside, light inside),
/// a narrow tail from its bottom down to the speaker, all one outline, and
/// the words in yellow with a black shadow from the top left. Edges are
/// smoothed. The tail's tip is [`SPEECH_TIP`] from the image's bottom right.
pub fn speech_bubble_image(message: &str) -> Result<RgbaImage> {
    bubble(message, &SPEECH)
}

/// The bubble over an open booth, with the line its seller hawks: the
/// speech bubble's shape, smaller, in warm amber glass with a gold edge
/// and pale gold words, so a shop's call reads apart from what is said.
pub fn booth_bubble_image(message: &str) -> Result<RgbaImage> {
    bubble(message, &BOOTH)
}

struct BubbleStyle {
    min_size: [u32; 2],
    /// Opacity of the outer dark edge, the inner light edge and the glass.
    alpha: [f32; 3],
    edge: [f32; 3],
    light: [f32; 3],
    glass: [f32; 3],
    text: [u8; 3],
}

const SPEECH: BubbleStyle = BubbleStyle {
    min_size: [153, 71],
    alpha: [0.42, 0.36, 0.13],
    edge: [0.0; 3],
    light: [236.0, 236.0, 242.0],
    glass: [0.0; 3],
    text: [255, 255, 0],
};

const BOOTH: BubbleStyle = BubbleStyle {
    min_size: [96, 30],
    alpha: [0.7, 0.85, 0.62],
    edge: [36.0, 22.0, 4.0],
    light: [226.0, 178.0, 84.0],
    glass: [44.0, 26.0, 8.0],
    text: [255, 228, 150],
};

fn bubble(message: &str, style: &BubbleStyle) -> Result<RgbaImage> {
    const MAX_WIDTH: f32 = 190.0;
    const LINE: u32 = 15;
    const TAIL: u32 = 18;
    let font = font()?;
    let mut lines: Vec<String> = Vec::new();
    let mut line = String::new();
    for word in message.split_whitespace() {
        let candidate = if line.is_empty() { word.to_owned() } else { format!("{line} {word}") };
        if width(&font, &candidate) > MAX_WIDTH && !line.is_empty() {
            lines.push(std::mem::replace(&mut line, word.to_owned()));
        } else {
            line = candidate;
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines.truncate(6);
    let text_width = lines.iter().map(|line| width(&font, line)).fold(0.0_f32, f32::max);
    let [min_width, min_height] = style.min_size;
    let (w, h) = ((text_width.ceil() as u32 + 18).max(min_width), (lines.len() as u32 * LINE + 14).max(min_height));
    let mut image = RgbaImage::new(w, h + TAIL);
    let (wf, hf) = (w as f32, h as f32);
    // The tail: feet on the bottom edge, the tip down by the right corner.
    let feet = [[wf - 64.0, hf - 3.0], [wf - 33.0, hf - 3.0]];
    let tip = [wf - SPEECH_TIP[0] as f32, (h + TAIL) as f32 - SPEECH_TIP[1] as f32];
    for y in 0..image.height() {
        for x in 0..image.width() {
            let p = [x as f32 + 0.5, y as f32 + 0.5];
            let distance = round_rect_distance(p, [0.5, 0.5], [wf - 0.5, hf - 0.5], 3.0)
                .min(triangle_distance(p, feet[0], feet[1], tip));
            // Coverage of the shape shrunk by `inset` pixels.
            let inside = |inset: f32| (0.5 - (distance + inset)).clamp(0.0, 1.0);
            let dark = inside(0.0) - inside(1.0);
            let light = inside(1.0) - inside(2.0);
            let glass = inside(2.0);
            let [a_dark, a_light, a_glass] = style.alpha;
            let alpha = dark * a_dark + light * a_light + glass * a_glass;
            if alpha <= 0.0 {
                continue;
            }
            let colour: [u8; 3] = std::array::from_fn(|channel| {
                ((dark * a_dark * style.edge[channel] + light * a_light * style.light[channel] + glass * a_glass * style.glass[channel])
                    / alpha)
                    .round()
                    .clamp(0.0, 255.0) as u8
            });
            image.put_pixel(x, y, Rgba([colour[0], colour[1], colour[2], (alpha * 255.0).round() as u8]));
        }
    }
    for (index, line) in lines.iter().enumerate() {
        let y = 6.0 + (index as u32 * LINE) as f32;
        text(&mut image, &font, &[(line.as_str(), [0, 0, 0])], [9.0, y + 1.0]);
        text(&mut image, &font, &[(line.as_str(), style.text)], [8.0, y]);
    }
    Ok(image)
}

/// Signed distance from `p` to the rounded box `min`..`max` (negative
/// inside).
fn round_rect_distance(p: [f32; 2], min: [f32; 2], max: [f32; 2], radius: f32) -> f32 {
    let centre = [(min[0] + max[0]) * 0.5, (min[1] + max[1]) * 0.5];
    let half = [(max[0] - min[0]) * 0.5 - radius, (max[1] - min[1]) * 0.5 - radius];
    let q = [(p[0] - centre[0]).abs() - half[0], (p[1] - centre[1]).abs() - half[1]];
    let outside = (q[0].max(0.0).powi(2) + q[1].max(0.0).powi(2)).sqrt();
    outside + q[0].max(q[1]).min(0.0) - radius
}

/// Signed distance from `p` to the triangle `a`, `b`, `c` (negative
/// inside).
fn triangle_distance(p: [f32; 2], a: [f32; 2], b: [f32; 2], c: [f32; 2]) -> f32 {
    let segment = |from: [f32; 2], to: [f32; 2]| {
        let edge = [to[0] - from[0], to[1] - from[1]];
        let offset = [p[0] - from[0], p[1] - from[1]];
        let t = ((offset[0] * edge[0] + offset[1] * edge[1]) / (edge[0].powi(2) + edge[1].powi(2))).clamp(0.0, 1.0);
        ((offset[0] - edge[0] * t).powi(2) + (offset[1] - edge[1] * t).powi(2)).sqrt()
    };
    let distance = segment(a, b).min(segment(b, c)).min(segment(c, a));
    let side = |from: [f32; 2], to: [f32; 2]| (to[0] - from[0]) * (p[1] - from[1]) - (to[1] - from[1]) * (p[0] - from[0]);
    let (ab, bc, ca) = (side(a, b), side(b, c), side(c, a));
    let inside = (ab >= 0.0 && bc >= 0.0 && ca >= 0.0) || (ab <= 0.0 && bc <= 0.0 && ca <= 0.0);
    if inside { -distance } else { distance }
}

/// Where the bubble's tail ends, from the image's bottom right.
pub const SPEECH_TIP: [u32; 2] = [3, 2];

#[cfg(test)]
mod speech_preview {
    /// `BOUNTY_PREVIEW=dir cargo test -p conquer-sandbox speech_bubble_preview`
    #[test]
    fn speech_bubble_preview() {
        let Ok(dir) = std::env::var("BOUNTY_PREVIEW") else { return };
        let mut sheet = image::RgbaImage::new(460, 120);
        let short = super::speech_bubble_image("teste").expect("bubble");
        let long = super::speech_bubble_image("alguém vende uma Super Dragon Ball? pago bem, chama no whisper").expect("bubble");
        image::imageops::overlay(&mut sheet, &short, 10, 10);
        image::imageops::overlay(&mut sheet, &long, 140, 10);
        sheet.save(format!("{dir}/speech.png")).expect("png");
        let mut booth = image::RgbaImage::from_pixel(460, 90, image::Rgba([150, 150, 150, 255]));
        let short = super::booth_bubble_image("VENDO BALLS EXP").expect("bubble");
        let long = super::booth_bubble_image("Cheap gear! DragonBalls for CPs, Meteors and +2 stones").expect("bubble");
        image::imageops::overlay(&mut booth, &short, 10, 10);
        image::imageops::overlay(&mut booth, &long, 160, 10);
        booth.save(format!("{dir}/booth-bubble.png")).expect("png");
    }
}

#[cfg(test)]
mod shield_preview {
    /// `GUILD_PREVIEW=dir cargo test -p conquer-sandbox shield_preview`.
    #[test]
    fn shield_preview() {
        let Ok(dir) = std::env::var("GUILD_PREVIEW") else { return };
        let mut sheet = image::RgbaImage::from_pixel(260, 140, image::Rgba([150, 140, 130, 255]));
        let labels = [
            ("Gravak790942391", "Super Amiguinhos", "Member", super::SHIELD_OWN),
            ("TesteCreation", "Enemy Leader", "Leader", super::SHIELD_ENEMY),
            ("Aliado", "Blue Moon", "Deputy Leader", super::SHIELD_ALLY),
            ("SemGuild", "", "", super::SHIELD_ENEMY),
        ];
        for (index, (name, guild, rank, shield)) in labels.into_iter().enumerate() {
            let label = super::actor_image_with_guild(name, [255, 255, 255], guild, rank, shield).expect("label");
            image::imageops::overlay(&mut sheet, &label, 10, 4 + index as i64 * 33);
        }
        sheet.save(format!("{dir}/shields.png")).expect("png");
        // The shields alone, eight times as big, to judge the art.
        let mut big = image::RgbaImage::from_pixel(3 * 8 * 16 + 3 * 30, 8 * 18, image::Rgba([150, 140, 130, 255]));
        for (index, shield) in [super::SHIELD_OWN, super::SHIELD_ENEMY, super::SHIELD_ALLY].into_iter().enumerate() {
            let art = super::shield_image_scaled(shield, 8).expect("art");
            image::imageops::overlay(&mut big, &art, index as i64 * 8 * 16, 0);
            // And at its real size, beside.
            let small = super::shield_image(shield).expect("art");
            image::imageops::overlay(&mut big, &small, 3 * 8 * 16 + index as i64 * 30 + 6, 20);
        }
        big.save(format!("{dir}/shields-big.png")).expect("png");
    }
}
