// Source: @digitalm1nd on x.com / _dek on Discord — Preservation Conquer project — https://discord.gg/CvKPXEHYRY
use crate::{assets::Result, character::Character};
use image::{Pixel, Rgba, RgbaImage};

const FONT_SIZE: f32 = 12.0;
const GUILD_COLOUR: [u8; 3] = [255, 255, 0];
const RANK_COLOUR: [u8; 3] = [145, 185, 255];
pub const BUILD_TEXT: &str =
    "[Preservation] Skeleton test build by @digitalm1nd | Discord: _dek | discord.gg/CvKPXEHYRY";

pub(crate) fn font() -> Result<fontdue::Font> {
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
    for shadow in [true, false] {
        let mut cursor = origin[0].round();
        for &(text, colour) in spans {
            let colour = if shadow { [0, 0, 0] } else { colour };
            let offset = i32::from(shadow);
            for character in text.chars() {
                let (metrics, bitmap) = font.rasterize(character, FONT_SIZE);
                for row in 0..metrics.height {
                    for column in 0..metrics.width {
                        let x = cursor.round() as i32 + metrics.xmin + column as i32 + offset;
                        let y =
                            origin[1].round() as i32 + 12 - metrics.ymin - metrics.height as i32
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

pub fn image(character: &Character) -> Result<RgbaImage> {
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
        &[(&character.name, [255, 255, 255])],
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

pub fn actor_image(name: &str) -> Result<RgbaImage> {
    let font = font()?;
    let width = width(&font, name).ceil() as u32 + 8;
    let mut image = RgbaImage::new(width.max(16), 16);
    text(&mut image, &font, &[(name, [255, 255, 255])], [4.0, 2.0]);
    Ok(image)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogHit {
    Outside,
    Panel,
    Close,
    Option(usize),
}

pub struct DialogLayout {
    pub lines: Vec<String>,
    pub left: f32,
    pub top: f32,
    pub height: u32,
    pub option_top: f32,
}

pub fn dialog_layout(
    messages: &[String],
    options: &[(u8, String)],
    viewport: [f32; 2],
) -> Result<DialogLayout> {
    let font = font()?;
    let mut lines = Vec::new();
    for message in messages {
        let mut line = String::new();
        for word in message.split_whitespace() {
            let candidate = if line.is_empty() {
                word.to_owned()
            } else {
                format!("{line} {word}")
            };
            if !line.is_empty() && width(&font, &candidate) > 668.0 {
                lines.push(std::mem::take(&mut line));
            }
            if line.is_empty() {
                line = word.to_owned();
            } else if width(&font, &candidate) <= 668.0 {
                line = candidate;
            }
        }
        if !line.is_empty() {
            lines.push(line);
        }
    }
    if lines.is_empty() {
        lines.push("(sem texto)".to_owned());
    }
    let height = (74 + lines.len() * 20 + options.len() * 28).clamp(104, 300) as u32;
    let left = ((viewport[0] - 720.0) * 0.5).max(8.0);
    let top = (viewport[1] - height as f32 - 24.0).max(8.0);
    let option_top = top + 48.0 + lines.len() as f32 * 20.0;
    Ok(DialogLayout {
        lines,
        left,
        top,
        height,
        option_top,
    })
}

pub fn dialog_hit_test(
    viewport: [f32; 2],
    cursor: [f32; 2],
    messages: &[String],
    options: &[(u8, String)],
) -> Result<DialogHit> {
    let layout = dialog_layout(messages, options, viewport)?;
    if cursor[0] < layout.left
        || cursor[0] > layout.left + 720.0
        || cursor[1] < layout.top
        || cursor[1] > layout.top + layout.height as f32
    {
        return Ok(DialogHit::Outside);
    }
    if cursor[0] >= layout.left + 684.0
        && cursor[0] <= layout.left + 707.0
        && cursor[1] >= layout.top + 9.0
        && cursor[1] <= layout.top + 29.0
    {
        return Ok(DialogHit::Close);
    }
    if cursor[0] < layout.left + 15.0 || cursor[0] > layout.left + 705.0 {
        return Ok(DialogHit::Panel);
    }
    let index = ((cursor[1] - layout.option_top) / 27.0).floor() as usize;
    let row_offset = cursor[1] - layout.option_top - index as f32 * 27.0;
    if cursor[1] >= layout.option_top && index < options.len() && row_offset < 24.0 {
        return Ok(DialogHit::Option(index));
    }
    Ok(DialogHit::Panel)
}

pub fn dialog_image(messages: &[String], options: &[(u8, String)]) -> Result<RgbaImage> {
    dialog_image_with_hover(messages, options, None)
}

pub fn dialog_image_with_hover(
    messages: &[String],
    options: &[(u8, String)],
    hovered: Option<usize>,
) -> Result<RgbaImage> {
    let layout = dialog_layout(messages, options, [720.0, 360.0])?;
    let font = font()?;
    let height = layout.height;
    let mut image = RgbaImage::from_pixel(720, height, Rgba([0, 0, 0, 0]));
    for y in 0..height {
        for x in 0..720 {
            let border = x < 2 || x >= 718 || y < 2 || y >= height - 2;
            let header = (3..38).contains(&y);
            let stripe = (y % 4) == 0;
            image.put_pixel(
                x,
                y,
                if border {
                    Rgba([224, 190, 94, 250])
                } else if header {
                    Rgba([22, 51, 68, 247])
                } else {
                    Rgba([if stripe { 15 } else { 11 }, 22, 34, 242])
                },
            );
        }
    }
    dialog_outline(
        &mut image,
        5,
        41,
        710,
        height.saturating_sub(47),
        Rgba([61, 91, 103, 255]),
    );
    text(
        &mut image,
        &font,
        &[("NPC DIALOG", [255, 225, 143])],
        [18.0, 12.0],
    );
    dialog_round_rect(&mut image, 685, 9, 22, 20, 4, Rgba([67, 39, 40, 255]));
    text(&mut image, &font, &[("X", [255, 220, 205])], [692.0, 13.0]);
    for (index, line) in layout.lines.iter().enumerate() {
        text(
            &mut image,
            &font,
            &[(line, [255, 255, 255])],
            [20.0, 48.0 + index as f32 * 20.0],
        );
    }
    for (index, (_, option)) in options.iter().enumerate() {
        let y = 48 + layout.lines.len() as u32 * 20 + index as u32 * 27;
        let highlighted = hovered == Some(index);
        dialog_round_rect(
            &mut image,
            15,
            y,
            690,
            24,
            5,
            if highlighted {
                Rgba([247, 210, 115, 255])
            } else {
                Rgba([38, 65, 78, 255])
            },
        );
        dialog_round_rect(
            &mut image,
            16,
            y + 1,
            688,
            22,
            4,
            if highlighted {
                Rgba([47, 81, 91, 255])
            } else {
                Rgba([17, 39, 53, 255])
            },
        );
        let label = format!("{}.  {option}", index + 1);
        text(
            &mut image,
            &font,
            &[(
                &label,
                if highlighted {
                    [255, 243, 190]
                } else {
                    [255, 222, 132]
                },
            )],
            [24.0, y as f32 + 5.0],
        );
    }
    text(
        &mut image,
        &font,
        &[(
            "Select an option or click outside to close",
            [157, 179, 190],
        )],
        [20.0, height as f32 - 18.0],
    );
    Ok(image)
}

fn dialog_round_rect(
    image: &mut RgbaImage,
    left: u32,
    top: u32,
    width: u32,
    height: u32,
    radius: u32,
    colour: Rgba<u8>,
) {
    let right = left + width - 1;
    let bottom = top + height - 1;
    for y in top..=bottom {
        for x in left..=right {
            let near_x = x.clamp(left + radius, right.saturating_sub(radius));
            let near_y = y.clamp(top + radius, bottom.saturating_sub(radius));
            let dx = x.abs_diff(near_x);
            let dy = y.abs_diff(near_y);
            if dx * dx + dy * dy <= radius * radius {
                image.put_pixel(x, y, colour);
            }
        }
    }
}

fn dialog_outline(
    image: &mut RgbaImage,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    colour: Rgba<u8>,
) {
    for dx in x..(x + width).min(image.width()) {
        image.put_pixel(dx, y.min(image.height() - 1), colour);
        image.put_pixel(dx, (y + height - 1).min(image.height() - 1), colour);
    }
    for dy in y..(y + height).min(image.height()) {
        image.put_pixel(x.min(image.width() - 1), dy, colour);
        image.put_pixel((x + width - 1).min(image.width() - 1), dy, colour);
    }
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
}
