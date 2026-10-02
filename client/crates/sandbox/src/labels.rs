// Source: @digitalm1nd on x.com / _dek on Discord — Preservation Conquer project — https://discord.gg/CvKPXEHYRY
use crate::{assets::Result, character::Character};
use image::{Pixel, Rgba, RgbaImage};
use std::sync::{Arc, OnceLock};

const FONT_SIZE: f32 = 12.0;
const GUILD_COLOUR: [u8; 3] = [255, 255, 0];
const RANK_COLOUR: [u8; 3] = [145, 185, 255];
pub const BUILD_TEXT: &str =
    "[Preservation] Skeleton test build by @digitalm1nd | Discord: _dek | discord.gg/CvKPXEHYRY";
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

fn damage_font() -> Result<fontdue::Font> {
    let mut database = fontdb::Database::new();
    database.load_system_fonts();
    let id = database
        .query(&fontdb::Query {
            families: &[
                fontdb::Family::Name("Times New Roman"),
                fontdb::Family::Name("Georgia"),
                fontdb::Family::Serif,
            ],
            weight: fontdb::Weight::BOLD,
            style: fontdb::Style::Italic,
            ..Default::default()
        })
        .ok_or("No damage font available")?;
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
        .ok_or("Damage font unavailable")??)
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
            let colour = if shadow { [0, 0, 0] } else { colour };
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

pub fn actor_image(name: &str, name_colour: [u8; 3]) -> Result<RgbaImage> {
    let font = font()?;
    let width = width(&font, name).ceil() as u32 + 8;
    let mut image = RgbaImage::new(width.max(16), 16);
    text(&mut image, &font, &[(name, name_colour)], [4.0, 2.0]);
    Ok(image)
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

/// Compact monster nameplate: the name followed by a small health bar.
pub fn monster_image(name: &str, health: u16, maximum_health: u16) -> Result<RgbaImage> {
    let font = font()?;
    let text_width = width(&font, name).ceil() as u32 + 8;
    let image_width = text_width.max(36);
    let mut image = RgbaImage::new(image_width, 28);
    text(
        &mut image,
        &font,
        &[(name, [255, 255, 255])],
        [
            ((image_width as f32 - width(&font, name)) * 0.5).max(2.0),
            1.0,
        ],
    );
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

pub fn damage_image(value: u32) -> Result<RgbaImage> {
    let font = damage_font()?;
    let text_value = if value == 0 { "Miss".to_owned() } else { value.to_string() };
    let size = 46.0;
    let text_width: f32 = text_value
        .chars()
        .map(|character| font.metrics(character, size).advance_width)
        .sum();
    let mut image = RgbaImage::new(text_width.ceil() as u32 + 24, 58);
    for (outline, colour) in [(true, [20, 12, 65]), (false, [224, 216, 255])] {
        let mut cursor = ((image.width() as f32 - text_width) * 0.5).round();
        for character in text_value.chars() {
            let (metrics, bitmap) = font.rasterize(character, size);
            for row in 0..metrics.height {
                for column in 0..metrics.width {
                    let offset = if outline { 3 } else { 0 };
                    let x = cursor as i32 + metrics.xmin + column as i32 + offset;
                    let y = 48 - metrics.ymin - metrics.height as i32 + row as i32 + offset;
                    if x < 0 || y < 0 || x >= image.width() as i32 || y >= image.height() as i32 {
                        continue;
                    }
                    let alpha = bitmap[row * metrics.width + column];
                    image.put_pixel(
                        x as u32,
                        y as u32,
                        Rgba([colour[0], colour[1], colour[2], alpha]),
                    );
                }
            }
            cursor += metrics.advance_width;
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

pub fn loot_name_image(name: &str) -> Result<RgbaImage> {
    let font = font()?;
    let width = width(&font, name).ceil() as u32 + 8;
    let mut image = RgbaImage::new(width.max(24), 18);
    text(&mut image, &font, &[(name, [255, 236, 150])], [4.0, 2.0]);
    Ok(image)
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

pub fn hit_flash_image() -> Result<RgbaImage> {
    let mut image = RgbaImage::new(64, 64);
    let center = 31.5_f32;
    for y in 0..64 {
        for x in 0..64 {
            let dx = x as f32 - center;
            let dy = y as f32 - center;
            let distance = (dx * dx + dy * dy).sqrt();
            let alpha = if (20.0..=30.0).contains(&distance) {
                ((30.0 - distance) * 22.0).clamp(0.0, 210.0) as u8
            } else {
                0
            };
            image.put_pixel(x, y, Rgba([255, 235, 120, alpha]));
        }
    }
    Ok(image)
}

pub fn build_banner() -> Result<RgbaImage> {
    let font = font()?;
    let mut image = RgbaImage::new(width(&font, BUILD_TEXT).ceil() as u32 + 2, 16);
    text(&mut image, &font, &[(BUILD_TEXT, GUILD_COLOUR)], [0.0, 0.0]);
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
