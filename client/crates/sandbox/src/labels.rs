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
/// A monster's name over its life bar; the monsters of the bounty under way
/// have a gold name behind a gold diamond.
pub fn monster_image(name: &str, health: u16, maximum_health: u16, bounty: bool) -> Result<RgbaImage> {
    let font = font()?;
    let marker = if bounty { 12 } else { 0 };
    let text_width = width(&font, name).ceil() as u32 + 8 + marker;
    let image_width = text_width.max(36);
    let mut image = RgbaImage::new(image_width, 28);
    let name_x = ((image_width as f32 - width(&font, name) + marker as f32) * 0.5).max(2.0 + marker as f32);
    let colour = if bounty { [255, 202, 64] } else { [255, 255, 255] };
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
