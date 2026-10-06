//! The NPC dialog in the client's own window (`shop::framed`, like the
//! Bounty Board, the Store and the Highscores): the NPC's name for its
//! title, their face in a gold frame, the words wrapped in parchment white,
//! and the answers as cards numbered on gold medallions (the keys 1 to 9
//! pick them too); a way out ("Leave", "Just passing by") quieter than the
//! rest. Layout, painting and hit testing share one description, so a click
//! always picks the card that is lit.

use crate::assets::{Assets, Result};
use crate::daily_rewards_panel::{GOLD_DARK, GOLD_LIGHT, centred, glow_in, paint_in, rounded, say, width_of};
use crate::paint;
use crate::ui::UiRect;
use image::{RgbaImage, imageops};

/// The panel's width; its height follows the words and the answers.
const WIDTH: i32 = 560;
/// The window's frame draws nothing around; its title takes the top.
const MARGIN: i32 = 0;
const PAD: i32 = 20;
const CONTENT_TOP: i32 = 46;
const PORTRAIT: UiRect = UiRect::new(PAD, CONTENT_TOP, 84, 84);
const TEXT_LEFT: i32 = PAD + 84 + 16;
const TEXT_WIDTH: i32 = WIDTH - TEXT_LEFT - 26;
/// The frame's close button (`shop::framed`).
const CLOSE: UiRect = UiRect::new(WIDTH - 31, 6, 27, 30);
const TEXT_SIZE: f32 = 13.0;
const LINE: i32 = 18;
const ROW: i32 = 30;
const ROW_LINE: i32 = 16;
const ROW_GAP: i32 = 6;
const OPTION_TEXT: i32 = 40;
/// From the top of the screen.
const TOP: f32 = 64.0;

const PARCHMENT: [u8; 3] = [238, 228, 202];
const OPTION: [u8; 3] = [255, 232, 170];
const OPTION_LIT: [u8; 3] = [255, 250, 222];
const QUIET: [u8; 3] = [176, 184, 188];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogHit {
    Outside,
    Panel,
    Close,
    Option(usize),
}

pub struct DialogLayout {
    /// The speaker, when the words start with "Name: ".
    pub name: Option<String>,
    pub lines: Vec<String>,
    pub height: i32,
    /// The answers' cards, relative to the panel, and their wrapped words.
    pub options: Vec<[i32; 4]>,
    option_lines: Vec<Vec<String>>,
}

/// Where the picture goes: centred near the top, shadow included.
pub fn position(viewport: [f32; 2]) -> [f32; 2] {
    [((viewport[0] - WIDTH as f32) * 0.5).max(0.0).round(), TOP]
}

fn wrap(font: &fontdue::Font, text: &str, width: i32, size: f32) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        let candidate = if line.is_empty() { word.to_owned() } else { format!("{line} {word}") };
        if !line.is_empty() && width_of(font, &candidate, size) > width as f32 {
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

/// "Blacksmith: I repair..." speaks as the Blacksmith; a short head of
/// at most three words before ": " is taken for the name.
fn speaker(message: &str) -> Option<(&str, &str)> {
    let (head, rest) = message.split_once(": ")?;
    (head.len() <= 24 && head.split_whitespace().count() <= 3 && !head.is_empty()).then_some((head.trim(), rest))
}

/// A way out of the dialog: drawn quieter.
fn leaves(text: &str) -> bool {
    let text = text.to_lowercase();
    [
        "leave", "just passing", "no thanks", "no, thanks", "never mind", "nevermind", "goodbye", "bye", "cancel",
        "not now", "maybe later", "i'll think", "i will think", "close", "nothing", "no.",
    ]
    .iter()
    .any(|word| text.starts_with(word) || text == word.trim_end_matches('.'))
}

pub fn layout(messages: &[String], options: &[(u8, String)]) -> Result<DialogLayout> {
    let font = crate::labels::font()?;
    let mut name = None;
    let mut lines = Vec::new();
    for (index, message) in messages.iter().enumerate() {
        let words = match speaker(message).filter(|_| index == 0) {
            Some((speaker, rest)) => {
                name = Some(speaker.to_owned());
                rest
            }
            None => message.as_str(),
        };
        if index > 0 && !lines.is_empty() {
            lines.push(String::new());
        }
        lines.extend(wrap(&font, words, TEXT_WIDTH, TEXT_SIZE));
    }
    let text_top = CONTENT_TOP + 4;
    let text_bottom = text_top + lines.len() as i32 * LINE;
    let mut y = text_bottom.max(PORTRAIT.y + PORTRAIT.height) + 14;
    let mut rows = Vec::new();
    let mut option_lines = Vec::new();
    for (_, text) in options {
        let wrapped = wrap(&font, text, WIDTH - PAD * 2 - OPTION_TEXT - 12, TEXT_SIZE);
        let height = ROW + (wrapped.len().max(1) as i32 - 1) * ROW_LINE;
        rows.push([PAD, y, WIDTH - PAD * 2, height]);
        option_lines.push(wrapped);
        y += height + ROW_GAP;
    }
    Ok(DialogLayout { name, lines, height: y - ROW_GAP + PAD, options: rows, option_lines })
}

fn inside(rect: [i32; 4], point: [i32; 2]) -> bool {
    point[0] >= rect[0] && point[0] < rect[0] + rect[2] && point[1] >= rect[1] && point[1] < rect[1] + rect[3]
}

pub fn hit_test(viewport: [f32; 2], cursor: [f32; 2], messages: &[String], options: &[(u8, String)]) -> Result<DialogHit> {
    let layout = layout(messages, options)?;
    let [left, top] = position(viewport);
    let point = [(cursor[0] - left) as i32 - MARGIN, (cursor[1] - top) as i32 - MARGIN];
    if !inside([0, 0, WIDTH, layout.height], point) {
        return Ok(DialogHit::Outside);
    }
    if inside([CLOSE.x, CLOSE.y, CLOSE.width, CLOSE.height], point) {
        return Ok(DialogHit::Close);
    }
    Ok(layout.options.iter().position(|&row| inside(row, point)).map_or(DialogHit::Panel, DialogHit::Option))
}

/// Whether the cursor is on the frame's close button.
pub fn close_hovered(viewport: [f32; 2], cursor: [f32; 2], messages: &[String], options: &[(u8, String)]) -> bool {
    matches!(hit_test(viewport, cursor, messages, options), Ok(DialogHit::Close))
}

/// The card under the cursor, for its light.
pub fn hovered_option(viewport: [f32; 2], cursor: [f32; 2], messages: &[String], options: &[(u8, String)]) -> Option<usize> {
    match hit_test(viewport, cursor, messages, options) {
        Ok(DialogHit::Option(index)) => Some(index),
        _ => None,
    }
}

fn shifted(rect: UiRect) -> UiRect {
    UiRect::new(rect.x + MARGIN, rect.y + MARGIN, rect.width, rect.height)
}

/// The face in a gold frame with a dark well behind it.
fn portrait(image: &mut RgbaImage, assets: &mut Assets, avatar: u16) {
    let rect = shifted(PORTRAIT);
    let shape = rounded(rect, 5.0);
    glow_in(image, rect, &shape, 6.0, paint::hex(0xffcf6a), 0.25);
    paint_in(image, rect, 1, &shape, |_, _| paint::hex(0x04090d), 1.0);
    let face_size = (rect.width - 8) as u32;
    if let Ok(face) = assets.image(&format!("data/NpcFace/{avatar}.dds")) {
        let face = crate::original_ui::visible_part(&face).unwrap_or(face);
        let face = imageops::resize(&face, face_size, face_size, imageops::FilterType::Triangle);
        imageops::overlay(image, &face, i64::from(rect.x + 4), i64::from(rect.y + 4));
    }
    paint_in(image, rect, 1, |x, y| shape(x, y).abs() - 1.4, |_, y| paint::mix(GOLD_LIGHT, GOLD_DARK, (y - rect.y as f32) / rect.height as f32), 1.0);
    let inner = UiRect::new(rect.x + 3, rect.y + 3, rect.width - 6, rect.height - 6);
    let inner_shape = rounded(inner, 3.0);
    paint_in(image, inner, 0, |x, y| inner_shape(x, y).abs() - 0.5, |_, _| GOLD_DARK, 0.9);
}

/// An answer's card: lit under the cursor, quiet for a way out.
fn option_card(image: &mut RgbaImage, cinzel: &fontdue::Font, ui: &fontdue::Font, rect: UiRect, number: usize, lines: &[String], lit: bool, quiet: bool) {
    let shape = rounded(rect, 6.0);
    if lit {
        glow_in(image, rect, &shape, 7.0, paint::hex(0xffd36b), 0.45);
    }
    let (top, bottom) = match (lit, quiet) {
        (true, _) => (paint::hex(0x2a5068), paint::hex(0x12293a)),
        (false, true) => (paint::hex(0x0e1d27), paint::hex(0x08121a)),
        (false, false) => (paint::hex(0x173a50), paint::hex(0x0b1d2a)),
    };
    let y0 = rect.y as f32;
    let h = rect.height as f32;
    paint_in(image, rect, 1, &shape, |_, y| paint::mix(top, bottom, (y - y0) / h), 0.97);
    let border = if lit { GOLD_LIGHT } else if quiet { paint::hex(0x2c3c44) } else { paint::hex(0x6a5426) };
    paint_in(image, rect, 1, |x, y| shape(x, y).abs() - if lit { 0.9 } else { 0.6 }, |_, _| border, 1.0);
    // The number on a small medallion.
    let centre = [rect.x as f32 + 20.0, rect.y as f32 + ROW as f32 / 2.0];
    let radius = 10.0;
    let bounds = UiRect::new(centre[0] as i32 - 12, centre[1] as i32 - 12, 25, 25);
    let (face_top, face_bottom) = if quiet && !lit { (paint::hex(0x5a6268), paint::hex(0x2a3034)) } else { (GOLD_LIGHT, GOLD_DARK) };
    paint_in(
        image,
        bounds,
        1,
        |x, y| paint::circle(x, y, centre, radius),
        |_, y| paint::mix(face_top, face_bottom, (y - centre[1] + radius) / (2.0 * radius)),
        1.0,
    );
    paint_in(image, bounds, 1, |x, y| paint::circle(x, y, centre, radius - 2.5), |_, _| paint::hex(0x0d2232), 1.0);
    let label = number.to_string();
    centred(image, cinzel, &label, if quiet && !lit { [190, 196, 200] } else { [255, 232, 160] }, centre[0] as i32, (centre[1] - 8.0) as i32, 12.0);
    let colour = if lit { OPTION_LIT } else if quiet { QUIET } else { OPTION };
    for (index, line) in lines.iter().enumerate() {
        say(image, ui, line, colour, rect.x + OPTION_TEXT, rect.y + 7 + index as i32 * ROW_LINE, TEXT_SIZE);
    }
}

pub fn image(
    assets: &mut Assets,
    messages: &[String],
    options: &[(u8, String)],
    avatar: u16,
    hovered: Option<usize>,
    close_hovered: bool,
) -> Result<RgbaImage> {
    let layout = layout(messages, options)?;
    let ui = crate::labels::font()?;
    let cinzel = crate::labels::decorative_font()?;
    let mut image = RgbaImage::new(WIDTH as u32, layout.height as u32);
    portrait(&mut image, assets, avatar);
    let left = TEXT_LEFT + MARGIN;
    let text_top = CONTENT_TOP + 4;
    for (index, line) in layout.lines.iter().enumerate() {
        say(&mut image, &ui, line, PARCHMENT, left, text_top + index as i32 * LINE, TEXT_SIZE);
    }
    for (index, (((_, text), row), lines)) in options.iter().zip(&layout.options).zip(&layout.option_lines).enumerate() {
        let rect = UiRect::new(row[0] + MARGIN, row[1] + MARGIN, row[2], row[3]);
        option_card(&mut image, &cinzel, &ui, rect, index + 1, lines, hovered == Some(index), leaves(text));
    }
    let title = layout.name.clone().unwrap_or_else(|| "Dialog".to_owned());
    crate::shop::framed(assets, image, &title, close_hovered)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_option_card_is_hit_where_it_is_laid_out() {
        let viewport = [1366.0, 768.0];
        let messages = vec!["Conductress: Where are you heading for? I can teleport you for 100 silver.".to_owned()];
        let options = (1..=7).map(|id| (id, format!("Destination {id}"))).collect::<Vec<_>>();
        let layout = layout(&messages, &options).expect("layout");
        assert_eq!(layout.name.as_deref(), Some("Conductress"));
        let [left, top] = position(viewport);
        for (index, row) in layout.options.iter().enumerate() {
            let point = [left + (row[0] + MARGIN + 30) as f32, top + (row[1] + MARGIN + row[3] / 2) as f32];
            assert_eq!(hit_test(viewport, point, &messages, &options).expect("hit"), DialogHit::Option(index));
        }
        let close = [left + (CLOSE.x + MARGIN + 10) as f32, top + (CLOSE.y + MARGIN + 10) as f32];
        assert_eq!(hit_test(viewport, close, &messages, &options).expect("hit"), DialogHit::Close);
        assert_eq!(hit_test(viewport, [5.0, 700.0], &messages, &options).expect("hit"), DialogHit::Outside);
        assert!(leaves("Just passing by.") && leaves("Leave.") && !leaves("Buy equipment."));
    }

    #[test]
    #[ignore = "Requires a local 5065 installation; exports the NPC dialog preview"]
    fn export_preview() -> Result<()> {
        let mut assets = Assets::open(std::path::Path::new("R:/5065"))?;
        let output = std::path::PathBuf::from(std::env::var("GRAVAK_DIALOG_PREVIEW")?);
        std::fs::create_dir_all(&output)?;
        let numbered = |texts: &[&str]| texts.iter().enumerate().map(|(index, text)| (index as u8 + 1, (*text).to_owned())).collect::<Vec<_>>();
        let messages = vec!["Conductress: Where are you heading for? I can teleport you for a price of 100 silver.".to_owned()];
        let options = numbered(&["Phoenix Castle", "Desert City", "Ape Mountain", "Bird Island.", "Mine Cave", "Market", "Just passing by."]);
        image(&mut assets, &messages, &options, 1, Some(2), false)?.save(output.join("conductress.png"))?;
        let messages = vec!["Blacksmith: I repair and sell dependable equipment. 106 items are available.".to_owned()];
        let options = numbered(&["Buy equipment.", "Sell equipment.", "Repair all equipment.", "Leave."]);
        image(&mut assets, &messages, &options, 75, Some(0), true)?.save(output.join("blacksmith.png"))?;
        Ok(())
    }
}
