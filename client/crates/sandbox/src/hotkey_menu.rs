//! The F-key slots' right-click menu and what it opens: a learned skill, a
//! usable item, a line of text, or another F key to swap with. Drawn on
//! top of every window.

use crate::assets::{Assets, Result};
use crate::ui::UiRect;
use crate::ui_theme::{fill, outline};
use image::{imageops, RgbaImage};

/// The menu's rows.
pub const OPTIONS: [&str; 5] = ["Assign Skill", "Assign Object", "Assign Text", "Assign Hotkey", "Clear"];
const ROW: i32 = 22;
const MENU_WIDTH: i32 = 150;
const CELL: i32 = 40;
const COLUMNS: usize = 6;
const TITLE: i32 = 22;
const FOOTER: i32 = 20;
const PAD: i32 = 6;
const KEY_WIDTH: i32 = 32;
const TEXT_WIDTH: i32 = 240;
/// The longest line a slot keeps.
pub const TEXT_LIMIT: usize = 60;

const BACKGROUND: [u8; 4] = [8, 10, 14, 228];
const BORDER: [u8; 4] = [137, 115, 74, 255];
const HOVER: [u8; 4] = [96, 78, 44, 210];
const TEXT: [u8; 4] = [235, 235, 235, 255];
const LIT: [u8; 4] = [255, 236, 170, 255];
const DIM: [u8; 4] = [170, 170, 170, 255];

/// What is open over F-key `slot`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum HotkeyPopup {
    Menu { slot: usize },
    Skills { slot: usize },
    Objects { slot: usize },
    Text { slot: usize, text: String },
    Rebind { slot: usize },
}

impl HotkeyPopup {
    pub fn slot(&self) -> usize {
        match self {
            Self::Menu { slot }
            | Self::Skills { slot }
            | Self::Objects { slot }
            | Self::Text { slot, .. }
            | Self::Rebind { slot } => *slot,
        }
    }
}

/// The popup with what it lists, as drawn and hit-tested.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HotkeyPopupView {
    pub popup: HotkeyPopup,
    /// The learned skills (Skills) or the usable item types and how many
    /// the inventory holds (Objects).
    pub skills: Vec<u32>,
    pub objects: Vec<(u32, u32)>,
    /// The F key's slot rect on screen, the popup's anchor.
    pub anchor: [i32; 4],
    pub viewport: [i32; 2],
}

/// What a click on the popup picks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeyChoice {
    Option(usize),
    Skill(u32),
    Object(u32),
    Key(usize),
    SaveText,
    /// On the popup but nothing in particular.
    Nothing,
}

fn entries(view: &HotkeyPopupView) -> usize {
    match view.popup {
        HotkeyPopup::Menu { .. } => OPTIONS.len(),
        HotkeyPopup::Skills { .. } => view.skills.len(),
        HotkeyPopup::Objects { .. } => view.objects.len(),
        HotkeyPopup::Text { .. } => 1,
        HotkeyPopup::Rebind { .. } => 10,
    }
}

/// The popup's rect on screen, above its F key, and its cells inside it.
fn layout(view: &HotkeyPopupView) -> (UiRect, Vec<UiRect>) {
    let count = entries(view);
    let (width, height, cells): (i32, i32, Vec<UiRect>) = match view.popup {
        HotkeyPopup::Menu { .. } => (
            MENU_WIDTH,
            ROW * count as i32 + 8,
            (0..count).map(|index| UiRect::new(3, 4 + index as i32 * ROW, MENU_WIDTH - 6, ROW)).collect(),
        ),
        HotkeyPopup::Skills { .. } | HotkeyPopup::Objects { .. } => {
            let columns = count.clamp(1, COLUMNS) as i32;
            let rows = count.div_ceil(COLUMNS).max(1) as i32;
            let width = (columns * CELL + PAD * 2).max(180);
            (
                width,
                TITLE + rows * CELL + FOOTER + PAD,
                (0..count)
                    .map(|index| {
                        let (column, row) = ((index % COLUMNS) as i32, (index / COLUMNS) as i32);
                        UiRect::new(PAD + column * CELL, TITLE + row * CELL, CELL, CELL)
                    })
                    .collect(),
            )
        }
        HotkeyPopup::Text { .. } => (
            TEXT_WIDTH,
            TITLE + 26 + FOOTER + PAD,
            vec![UiRect::new(TEXT_WIDTH - PAD - 50, TITLE + 30, 50, 18)],
        ),
        HotkeyPopup::Rebind { .. } => (
            KEY_WIDTH * 10 + PAD * 2,
            TITLE + KEY_WIDTH + PAD,
            (0..10).map(|index| UiRect::new(PAD + index as i32 * KEY_WIDTH, TITLE, KEY_WIDTH - 2, KEY_WIDTH - 4)).collect(),
        ),
    };
    let [ax, ay, aw, _] = view.anchor;
    let x = (ax + aw / 2 - width / 2).clamp(4, (view.viewport[0] - width - 4).max(4));
    let y = (ay - height - 26).max(4);
    (UiRect::new(x, y, width, height), cells)
}

fn local(rect: UiRect, cursor: [f32; 2]) -> [f32; 2] {
    [cursor[0] - rect.x as f32, cursor[1] - rect.y as f32]
}

fn hovered(view: &HotkeyPopupView, cursor: [f32; 2]) -> Option<usize> {
    let (rect, cells) = layout(view);
    let point = local(rect, cursor);
    cells.iter().position(|cell| cell.contains_local(point))
}

/// The choice under the cursor; `None` off the popup.
pub fn click_at(view: &HotkeyPopupView, cursor: [f32; 2]) -> Option<HotkeyChoice> {
    let (rect, _) = layout(view);
    if !UiRect::new(0, 0, rect.width, rect.height).contains_local(local(rect, cursor)) {
        return None;
    }
    let Some(index) = hovered(view, cursor) else {
        return Some(HotkeyChoice::Nothing);
    };
    Some(match &view.popup {
        HotkeyPopup::Menu { .. } => HotkeyChoice::Option(index),
        HotkeyPopup::Skills { .. } => HotkeyChoice::Skill(view.skills[index]),
        HotkeyPopup::Objects { .. } => HotkeyChoice::Object(view.objects[index].0),
        HotkeyPopup::Text { .. } => HotkeyChoice::SaveText,
        HotkeyPopup::Rebind { .. } => HotkeyChoice::Key(index),
    })
}

/// Where the popup goes on screen.
pub fn position(view: &HotkeyPopupView) -> [f32; 2] {
    let (rect, _) = layout(view);
    [rect.x as f32, rect.y as f32]
}

/// The cell under the cursor, for the texture's key.
pub fn hover_index(view: &HotkeyPopupView, cursor: [f32; 2]) -> Option<usize> {
    hovered(view, cursor)
}

fn say(image: &mut RgbaImage, text: &str, at: [i32; 2], colour: [u8; 4], width: Option<i32>) -> Result<()> {
    crate::main_hud::text(image, crate::main_hud::font()?, text, 12.0, at, colour, width);
    Ok(())
}

/// The popup with the cell under `hover` lit.
pub fn image(assets: &mut Assets, view: &HotkeyPopupView, hover: Option<usize>) -> Result<RgbaImage> {
    let (rect, cells) = layout(view);
    let mut image = RgbaImage::new(rect.width as u32, rect.height as u32);
    let whole = UiRect::new(0, 0, rect.width, rect.height);
    fill(&mut image, whole, BACKGROUND);
    outline(&mut image, whole, BORDER);
    let key = format!("F{}", view.popup.slot() + 1);
    match &view.popup {
        HotkeyPopup::Menu { .. } => {
            for (index, (label, cell)) in OPTIONS.iter().zip(&cells).enumerate() {
                if hover == Some(index) {
                    fill(&mut image, *cell, HOVER);
                }
                say(&mut image, label, [cell.x + 8, cell.y + 4], if hover == Some(index) { LIT } else { TEXT }, None)?;
            }
        }
        HotkeyPopup::Skills { .. } | HotkeyPopup::Objects { .. } => {
            let skills = matches!(view.popup, HotkeyPopup::Skills { .. });
            say(&mut image, &format!("{} for {key}", if skills { "Skill" } else { "Object" }), [PAD + 2, 5], LIT, None)?;
            if cells.is_empty() {
                let none = if skills { "No skills learned yet" } else { "No usable items in the bag" };
                say(&mut image, none, [PAD + 2, TITLE + 12], DIM, None)?;
            }
            for (index, cell) in cells.iter().enumerate() {
                let frame = UiRect::new(cell.x + 1, cell.y + 1, cell.width - 2, cell.height - 2);
                fill(&mut image, frame, [24, 28, 34, 255]);
                outline(&mut image, frame, if hover == Some(index) { LIT } else { [70, 64, 50, 255] });
                let size = (frame.width - 4) as u32;
                let art = if skills {
                    crate::main_hud::skill_icon(assets, view.skills[index])
                        .map(|art| imageops::resize(&art, size, size, imageops::FilterType::Triangle))
                } else {
                    crate::shop::item_icon_image(assets, view.objects[index].0)
                        .ok()
                        .map(|art| imageops::resize(&art, size, size, imageops::FilterType::Triangle))
                };
                if let Some(art) = art {
                    imageops::overlay(&mut image, &art, i64::from(frame.x + 2), i64::from(frame.y + 2));
                }
                if !skills {
                    let count = view.objects[index].1.to_string();
                    let font = crate::main_hud::font()?;
                    let width = crate::main_hud::text_width(font, &count, 11.0);
                    crate::main_hud::text(&mut image, font, &count, 11.0, [frame.x + frame.width - width - 3, frame.y + frame.height - 14], [255, 255, 255, 255], None);
                }
            }
            if let Some(index) = hover.filter(|index| *index < cells.len()) {
                let name = if skills {
                    crate::magic::skill_name(assets, view.skills[index])
                } else {
                    crate::shop::item_name(assets, view.objects[index].0)
                };
                say(&mut image, &name, [PAD + 2, rect.height - FOOTER], TEXT, Some(rect.width - PAD * 2))?;
            }
        }
        HotkeyPopup::Text { text, .. } => {
            say(&mut image, &format!("Text for {key}"), [PAD + 2, 5], LIT, None)?;
            let field = UiRect::new(PAD, TITLE, rect.width - PAD * 2, 22);
            fill(&mut image, field, [24, 28, 34, 255]);
            outline(&mut image, field, BORDER);
            say(&mut image, &format!("{text}|"), [field.x + 4, field.y + 5], [255, 255, 255, 255], Some(field.width - 8))?;
            say(&mut image, "Enter saves, Esc cancels", [PAD + 2, TITLE + 32], DIM, None)?;
            let save = cells[0];
            fill(&mut image, save, if hover == Some(0) { HOVER } else { [40, 34, 22, 255] });
            outline(&mut image, save, BORDER);
            say(&mut image, "Save", [save.x + 12, save.y + 3], if hover == Some(0) { LIT } else { TEXT }, None)?;
        }
        HotkeyPopup::Rebind { slot } => {
            say(&mut image, &format!("Move {key} to"), [PAD + 2, 5], LIT, None)?;
            for (index, cell) in cells.iter().enumerate() {
                let current = index == *slot;
                fill(&mut image, *cell, if hover == Some(index) { HOVER } else { [24, 28, 34, 255] });
                outline(&mut image, *cell, if current { LIT } else { [70, 64, 50, 255] });
                let label = format!("F{}", index + 1);
                let font = crate::main_hud::font()?;
                let width = crate::main_hud::text_width(font, &label, 12.0);
                crate::main_hud::text(&mut image, font, &label, 12.0, [cell.x + (cell.width - width) / 2, cell.y + 8], if current { LIT } else { TEXT }, None);
            }
        }
    }
    Ok(image)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view(popup: HotkeyPopup) -> HotkeyPopupView {
        HotkeyPopupView {
            popup,
            skills: vec![1000, 1001, 1002],
            objects: vec![(1_000_000, 5)],
            anchor: [400, 700, 38, 38],
            viewport: [1024, 768],
        }
    }

    #[test]
    fn popups_sit_above_their_key_and_hit_their_cells() {
        let menu = view(HotkeyPopup::Menu { slot: 3 });
        let [x, y] = position(&menu);
        assert!(y + ((ROW * OPTIONS.len() as i32) as f32) < 700.0);
        assert_eq!(click_at(&menu, [x + 20.0, y + 4.0 + ROW as f32 * 2.0 + 5.0]), Some(HotkeyChoice::Option(2)));
        assert_eq!(click_at(&menu, [x - 10.0, y]), None);
        let skills = view(HotkeyPopup::Skills { slot: 3 });
        let [x, y] = position(&skills);
        let second = [x + (PAD + CELL + 5) as f32, y + (TITLE + 5) as f32];
        assert_eq!(click_at(&skills, second), Some(HotkeyChoice::Skill(1001)));
        let keys = view(HotkeyPopup::Rebind { slot: 0 });
        let [x, y] = position(&keys);
        assert_eq!(click_at(&keys, [x + (PAD + KEY_WIDTH * 9 + 5) as f32, y + (TITLE + 5) as f32]), Some(HotkeyChoice::Key(9)));
    }

    /// `BOUNTY_PREVIEW=dir cargo test -p conquer-sandbox hotkey_popup_preview`
    #[test]
    fn hotkey_popup_preview() {
        let Ok(dir) = std::env::var("BOUNTY_PREVIEW") else { return };
        let mut assets = Assets::open(std::path::Path::new("R:/5065")).expect("assets");
        let mut objects = view(HotkeyPopup::Objects { slot: 2 });
        objects.objects = vec![(1_000_000, 12), (1_001_000, 3), (1_050_000, 1), (1_060_020, 2)];
        let popups = [
            (view(HotkeyPopup::Menu { slot: 2 }), Some(1)),
            (view(HotkeyPopup::Skills { slot: 2 }), Some(1)),
            (objects, Some(0)),
            (view(HotkeyPopup::Text { slot: 2, text: "/w Friend hi".into() }), None),
            (view(HotkeyPopup::Rebind { slot: 2 }), Some(5)),
        ];
        let images: Vec<RgbaImage> = popups.iter().map(|(view, hover)| image(&mut assets, view, *hover).expect("popup")).collect();
        let width = images.iter().map(RgbaImage::width).sum::<u32>() + 10 * images.len() as u32;
        let height = images.iter().map(RgbaImage::height).max().unwrap_or(1);
        let mut sheet = RgbaImage::from_pixel(width, height, image::Rgba([60, 70, 60, 255]));
        let mut x = 0;
        for image in &images {
            imageops::overlay(&mut sheet, image, x, 0);
            x += i64::from(image.width()) + 10;
        }
        sheet.save(format!("{dir}/hotkey-popups.png")).expect("png");
    }
}

