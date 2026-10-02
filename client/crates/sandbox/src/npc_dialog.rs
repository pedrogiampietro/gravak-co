//! The original NPC dialog (`GUI.ini` `[0-173]`, `Control.Ani`
//! `[Dialog23]` `DialogTask.dds`): a translucent gold-framed box in the
//! top-left corner with the NPC face (`ani/NpcFace.Ani`), the text beside it
//! and the answers listed below. Layout, painting and hit testing share one
//! description so a click always selects the row that is highlighted.

use crate::assets::{Assets, Result};
use image::{Rgba, RgbaImage, imageops};

const ART: &str = "data/main/DialogTask.dds";
const CLOSE: &str = "data/main/CloseBtnS.dds";
/// Native size of `DialogTask.dds`; taller dialogs stretch its middle row.
const WIDTH: i32 = 544;
const ART_HEIGHT: i32 = 201;
/// [173-1180] close button.
const CLOSE_RECT: [i32; 4] = [512, 8, 20, 20];
const FACE: [i32; 2] = [16, 16];
const TEXT_LEFT: i32 = 92;
const TEXT_WIDTH: i32 = 430;
const TOP: i32 = 16;
const LINE: i32 = 16;
const OPTION_GAP: i32 = 6;
const BOTTOM_MARGIN: i32 = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogHit {
    Outside,
    Panel,
    Close,
    Option(usize),
}

pub struct DialogLayout {
    pub lines: Vec<String>,
    pub height: i32,
    /// Option rows relative to the dialog origin.
    pub options: Vec<[i32; 4]>,
}

/// The dialog sits at the top-left corner, like the original.
pub fn position(_viewport: [f32; 2]) -> [f32; 2] {
    [0.0, 0.0]
}

fn wrap(font: &fontdue::Font, text: &str, width: i32) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        let candidate = if line.is_empty() {
            word.to_owned()
        } else {
            format!("{line} {word}")
        };
        if !line.is_empty() && crate::main_hud::text_width(font, &candidate, 12.0) > width {
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

pub fn layout(messages: &[String], options: &[(u8, String)]) -> Result<DialogLayout> {
    let font = crate::main_hud::font()?;
    let lines = messages
        .iter()
        .flat_map(|message| wrap(font, message, TEXT_WIDTH))
        .collect::<Vec<_>>();
    let mut y = TOP + lines.len() as i32 * LINE + OPTION_GAP;
    let mut rows = Vec::new();
    for (_, text) in options {
        let height = wrap(font, text, TEXT_WIDTH - 16).len().max(1) as i32 * LINE;
        rows.push([TEXT_LEFT - 4, y, TEXT_WIDTH + 8, height + 2]);
        y += height + 3;
    }
    let height = (y + BOTTOM_MARGIN).max(ART_HEIGHT).max(FACE[1] + 64 + BOTTOM_MARGIN);
    Ok(DialogLayout {
        lines,
        height,
        options: rows,
    })
}

fn inside(rect: [i32; 4], point: [i32; 2]) -> bool {
    point[0] >= rect[0]
        && point[0] < rect[0] + rect[2]
        && point[1] >= rect[1]
        && point[1] < rect[1] + rect[3]
}

pub fn hit_test(
    viewport: [f32; 2],
    cursor: [f32; 2],
    messages: &[String],
    options: &[(u8, String)],
) -> Result<DialogHit> {
    let layout = layout(messages, options)?;
    let [left, top] = position(viewport);
    let point = [(cursor[0] - left) as i32, (cursor[1] - top) as i32];
    if !inside([0, 0, WIDTH, layout.height], point) {
        return Ok(DialogHit::Outside);
    }
    if inside(CLOSE_RECT, point) {
        return Ok(DialogHit::Close);
    }
    Ok(layout
        .options
        .iter()
        .position(|&row| inside(row, point))
        .map_or(DialogHit::Panel, DialogHit::Option))
}

/// Row under the cursor, for the hover highlight.
pub fn hovered_option(
    viewport: [f32; 2],
    cursor: [f32; 2],
    messages: &[String],
    options: &[(u8, String)],
) -> Option<usize> {
    match hit_test(viewport, cursor, messages, options) {
        Ok(DialogHit::Option(index)) => Some(index),
        _ => None,
    }
}

/// `DialogTask.dds` at `height`: the top and bottom halves of the art with
/// its middle row repeated in between.
fn frame(assets: &mut Assets, height: i32) -> Result<RgbaImage> {
    let art = assets.image(ART)?;
    let art = imageops::crop_imm(&art, 0, 0, WIDTH as u32, ART_HEIGHT as u32).to_image();
    if height <= ART_HEIGHT {
        return Ok(art);
    }
    let half = ART_HEIGHT / 2;
    let mut image = RgbaImage::new(WIDTH as u32, height as u32);
    let top = imageops::crop_imm(&art, 0, 0, WIDTH as u32, half as u32).to_image();
    let bottom = imageops::crop_imm(&art, 0, half as u32, WIDTH as u32, (ART_HEIGHT - half) as u32)
        .to_image();
    let middle = imageops::crop_imm(&art, 0, half as u32, WIDTH as u32, 1).to_image();
    imageops::overlay(&mut image, &top, 0, 0);
    for y in half..height - (ART_HEIGHT - half) {
        imageops::overlay(&mut image, &middle, 0, i64::from(y));
    }
    imageops::overlay(&mut image, &bottom, 0, i64::from(height - (ART_HEIGHT - half)));
    Ok(image)
}

pub fn image(
    assets: &mut Assets,
    messages: &[String],
    options: &[(u8, String)],
    avatar: u16,
    hovered: Option<usize>,
) -> Result<RgbaImage> {
    let layout = layout(messages, options)?;
    let font = crate::main_hud::font()?;
    let mut image = frame(assets, layout.height)?;
    if let Ok(face) = assets.image(&format!("data/NpcFace/{avatar}.dds")) {
        imageops::overlay(&mut image, &face, FACE[0].into(), FACE[1].into());
    }
    crate::original_ui::overlay_asset(
        &mut image,
        assets,
        CLOSE,
        CLOSE_RECT[0].into(),
        CLOSE_RECT[1].into(),
    )?;
    for (index, line) in layout.lines.iter().enumerate() {
        crate::main_hud::text(
            &mut image,
            font,
            line,
            12.0,
            [TEXT_LEFT, TOP + index as i32 * LINE],
            [255, 255, 255, 255],
            None,
        );
    }
    for (index, ((_, text), row)) in options.iter().zip(&layout.options).enumerate() {
        let hovered = hovered == Some(index);
        if hovered {
            for y in row[1]..row[1] + row[3] {
                for x in row[0]..row[0] + row[2] {
                    image::Pixel::blend(
                        image.get_pixel_mut(x as u32, y as u32),
                        &Rgba([255, 220, 120, 45]),
                    );
                }
            }
        }
        let colour = if hovered {
            [255, 255, 0, 255]
        } else {
            [255, 230, 160, 255]
        };
        crate::main_hud::text(&mut image, font, "»", 12.0, [row[0] + 4, row[1] + 1], colour, None);
        for (line_index, line) in wrap(font, text, TEXT_WIDTH - 16).iter().enumerate() {
            crate::main_hud::text(
                &mut image,
                font,
                line,
                12.0,
                [row[0] + 16, row[1] + 1 + line_index as i32 * LINE],
                colour,
                None,
            );
        }
    }
    Ok(image)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_option_row_is_hit_where_it_is_laid_out() {
        let messages = vec!["Where are you heading for? I can teleport you for 100 silver.".to_owned()];
        let options = (1..=7)
            .map(|id| (id, format!("Destination {id}")))
            .collect::<Vec<_>>();
        let layout = layout(&messages, &options).expect("layout");
        assert!(layout.height >= ART_HEIGHT);
        for (index, row) in layout.options.iter().enumerate() {
            let point = [(row[0] + 10) as f32, (row[1] + row[3] / 2) as f32];
            assert_eq!(
                hit_test([1366.0, 768.0], point, &messages, &options).expect("hit"),
                DialogHit::Option(index)
            );
        }
        assert_eq!(
            hit_test([1366.0, 768.0], [520.0, 14.0], &messages, &options).expect("hit"),
            DialogHit::Close
        );
        assert_eq!(
            hit_test([1366.0, 768.0], [800.0, 600.0], &messages, &options).expect("hit"),
            DialogHit::Outside
        );
    }

    #[test]
    #[ignore = "Requires a local 5065 installation; exports the NPC dialog preview"]
    fn export_preview() -> Result<()> {
        let mut assets = Assets::open(std::path::Path::new("R:/5065"))?;
        let output = std::path::PathBuf::from(std::env::var("GRAVAK_DIALOG_PREVIEW")?);
        std::fs::create_dir_all(&output)?;
        let messages =
            vec!["Where are you heading for? I can teleport you for a price of 100 silver.".to_owned()];
        let options = [
            "Phoenix Castle",
            "Desert City",
            "Ape Mountain",
            "Bird Island.",
            "Mine Cave",
            "Market",
            "Just passing by.",
        ]
        .into_iter()
        .enumerate()
        .map(|(index, text)| (index as u8 + 1, text.to_owned()))
        .collect::<Vec<_>>();
        image(&mut assets, &messages, &options, 1, Some(2))?.save(output.join("conductress.png"))?;
        Ok(())
    }

}
