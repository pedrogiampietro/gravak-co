//! The Battle Power windows of the original (GUI.ini dialogs 286 and 287):
//! a click on the Character window's Battle Power opens the Evaluation
//! beside it, its parts with a Details button each; Details opens the
//! Detailed Information beside that, what each worn item adds to the part.

use crate::assets::{Assets, Result};
use crate::shop::{BattlePower, BATTLE_POWER_PARTS};
use crate::ui::UiRect;
use image::{imageops, RgbaImage};

/// [0-286] and [0-287], beside the Character window ([0-131] at 0, 65).
pub const EVALUATION_SIZE: [u32; 2] = [257, 408];
pub const DETAILS_SIZE: [u32; 2] = [292, 440];
/// Measured on the original: the windows' tops a little under the orb's.
const EVALUATION_OFFSET: [f32; 2] = [262.0, 26.0];
const DETAILS_OFFSET: [f32; 2] = [519.0, 26.0];

/// [286-1466..1470]: Details for quality, sockets, super gems, plus and the
/// other bonus.
const DETAIL_BUTTONS: [[i32; 2]; BATTLE_POWER_PARTS] = [[180, 120], [180, 156], [180, 192], [180, 228], [180, 265]];
const DETAIL_BUTTON: [i32; 2] = [46, 20];
/// [286-1180] close, [286-1667] help; [287-1180] close.
const EVALUATION_CLOSE: UiRect = UiRect::new(150, 350, 63, 20);
const EVALUATION_HELP: UiRect = UiRect::new(50, 350, 63, 20);
const DETAILS_CLOSE: UiRect = UiRect::new(150, 385, 63, 20);
/// The worn slots the Detailed Information lists, in the order of its
/// art's icons: headgear, necklace, ring, armor, weapon, off hand, boots.
const DETAIL_SLOTS: [u8; 7] = [1, 2, 6, 3, 4, 5, 8];

/// The Evaluation's top left: beside the Character window.
pub fn evaluation_position(viewport: [f32; 2]) -> [f32; 2] {
    let [left, top] = crate::shop::status_position(viewport);
    [left + EVALUATION_OFFSET[0], (top + EVALUATION_OFFSET[1]).max(0.0)]
}

/// The Detailed Information's top left: beside the Evaluation.
pub fn details_position(viewport: [f32; 2]) -> [f32; 2] {
    let [left, top] = crate::shop::status_position(viewport);
    [left + DETAILS_OFFSET[0], (top + DETAILS_OFFSET[1]).max(0.0)]
}

/// What a click on the windows asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BattlePowerAction {
    Close,
    Help,
    /// Opens the Detailed Information of a part.
    Details(u8),
    CloseDetails,
}

fn local(origin: [f32; 2], cursor: [f32; 2]) -> [f32; 2] {
    [cursor[0] - origin[0], cursor[1] - origin[1]]
}

/// The Evaluation's action under the cursor; `Some(None)` on the window
/// itself, `None` off it.
pub fn evaluation_action_at(viewport: [f32; 2], cursor: [f32; 2]) -> Option<Option<BattlePowerAction>> {
    let point = local(evaluation_position(viewport), cursor);
    let [width, height] = EVALUATION_SIZE;
    if !UiRect::new(0, 0, width as i32, height as i32).contains_local(point) {
        return None;
    }
    if EVALUATION_CLOSE.contains_local(point) {
        return Some(Some(BattlePowerAction::Close));
    }
    if EVALUATION_HELP.contains_local(point) {
        return Some(Some(BattlePowerAction::Help));
    }
    Some(
        DETAIL_BUTTONS
            .iter()
            .position(|[x, y]| UiRect::new(*x, *y, DETAIL_BUTTON[0], DETAIL_BUTTON[1]).contains_local(point))
            .map(|index| BattlePowerAction::Details(index as u8)),
    )
}

/// The Detailed Information's action under the cursor, as above.
pub fn details_action_at(viewport: [f32; 2], cursor: [f32; 2]) -> Option<Option<BattlePowerAction>> {
    let point = local(details_position(viewport), cursor);
    let [width, height] = DETAILS_SIZE;
    if !UiRect::new(0, 0, width as i32, height as i32).contains_local(point) {
        return None;
    }
    Some(DETAILS_CLOSE.contains_local(point).then_some(BattlePowerAction::CloseDetails))
}

/// The original art cropped to the window.
fn art(assets: &mut Assets, path: &str, size: [u32; 2]) -> Result<RgbaImage> {
    let source = assets.image(path)?;
    Ok(imageops::crop_imm(&source, 0, 0, size[0].min(source.width()), size[1].min(source.height())).to_image())
}

fn button(image: &mut RgbaImage, assets: &mut Assets, path: &str, at: [i32; 2]) -> Result<()> {
    let source = crate::original_ui::visible_part(&assets.image(path)?)?;
    imageops::overlay(image, &source, i64::from(at[0]), i64::from(at[1]));
    Ok(())
}

fn say(image: &mut RgbaImage, text: &str, at: [i32; 2], colour: [u8; 4], width: i32) -> Result<()> {
    let font = crate::main_hud::font()?;
    crate::main_hud::text(image, font, text, 12.0, at, colour, Some(width));
    Ok(())
}

const VALUE: [u8; 4] = [255, 255, 255, 255];
const DETAIL: [u8; 4] = [255, 224, 146, 255];

/// The Battle Power Evaluation ([0-286]): the level, the rebirths and the
/// parts with their Details, and the battle power.
pub fn evaluation_image(assets: &mut Assets, power: &BattlePower, rebirths: u8, level: u8) -> Result<RgbaImage> {
    let mut image = art(assets, "data/main/BattleDetailAssBG.dds", EVALUATION_SIZE)?;
    // [286-1471/1493] the level, [286-1472/1494] the rebirths.
    say(&mut image, &power.level.to_string(), [140, 48], VALUE, 38)?;
    say(&mut image, &format!("Level {level}"), [180, 48], DETAIL, 70)?;
    say(&mut image, &power.rebirths.to_string(), [140, 84], VALUE, 38)?;
    say(&mut image, &rebirths.to_string(), [180, 84], DETAIL, 70)?;
    // [286-1474..1477]: the parts, each beside its Details.
    for (part, [_, y]) in power.parts.iter().zip(DETAIL_BUTTONS) {
        say(&mut image, &part.to_string(), [140, y], VALUE, 38)?;
    }
    for [x, y] in DETAIL_BUTTONS {
        button(&mut image, assets, "data/main/BattleBtnDetail.dds", [x, y])?;
    }
    // [286-1485]: the battle power.
    let font = crate::main_hud::font()?;
    crate::main_hud::text(&mut image, font, &power.total().to_string(), 14.0, [140, 309], [255, 236, 170, 255], Some(100));
    button(&mut image, assets, "data/main/HelpBtn2.dds", [EVALUATION_HELP.x, EVALUATION_HELP.y])?;
    button(&mut image, assets, "data/main/CloseBtn2.dds", [EVALUATION_CLOSE.x, EVALUATION_CLOSE.y])?;
    Ok(image)
}

/// The Detailed Information ([0-287]) of part `part`: each worn item's name
/// and what it adds, and their sum.
pub fn details_image(
    assets: &mut Assets,
    equipment: &[crate::network::OwnedItem],
    power: &BattlePower,
    part: u8,
) -> Result<RgbaImage> {
    let mut image = art(assets, "data/main/BattleDetailConBG.dds", DETAILS_SIZE)?;
    let items = crate::shop::battle_power_items(equipment);
    for (row, slot) in DETAIL_SLOTS.into_iter().enumerate() {
        let y = 55 + row as i32 * 45;
        let Some((item, parts)) = items.iter().find(|(item, _)| item.position == slot) else {
            continue;
        };
        let name = crate::shop::item_name(assets, item.item_type);
        let name = if item.plus > 0 { format!("{name}(+{})", item.plus) } else { name };
        say(&mut image, &name, [72, y], VALUE, 98)?;
        say(&mut image, &parts[usize::from(part)].to_string(), [180, y], DETAIL, 44)?;
    }
    // [287-1485]: the part's sum (the other bonus has no items: nobility).
    let total = power.parts.get(usize::from(part)).copied().unwrap_or(0);
    let font = crate::main_hud::font()?;
    crate::main_hud::text(&mut image, font, &total.to_string(), 14.0, [200, 354], [255, 236, 170, 255], Some(80));
    button(&mut image, assets, "data/main/CloseBtn2.dds", [DETAILS_CLOSE.x, DETAILS_CLOSE.y])?;
    Ok(image)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_windows_sit_beside_the_character_window() {
        let viewport = [1024.0, 768.0];
        let [left, top] = crate::shop::status_position(viewport);
        assert_eq!(evaluation_position(viewport), [left + 262.0, (top + 26.0).max(0.0)]);
        let [x, y] = evaluation_position(viewport);
        let details = [x + 180.0 + 5.0, y + 156.0 + 5.0];
        assert_eq!(evaluation_action_at(viewport, details), Some(Some(BattlePowerAction::Details(1))));
        assert_eq!(evaluation_action_at(viewport, [x + 160.0, y + 355.0]), Some(Some(BattlePowerAction::Close)));
        assert_eq!(evaluation_action_at(viewport, [x + 20.0, y + 20.0]), Some(None));
        assert_eq!(evaluation_action_at(viewport, [x - 400.0, y]), None);
    }

    /// `BOUNTY_PREVIEW=dir cargo test -p conquer-sandbox battle_power_preview`
    #[test]
    fn battle_power_preview() {
        let Ok(dir) = std::env::var("BOUNTY_PREVIEW") else { return };
        let mut assets = Assets::open(std::path::Path::new("R:/5065")).expect("assets");
        let item = |uid, item_type, position, plus, gem_one, gem_two| crate::network::OwnedItem {
            uid,
            item_type,
            position,
            durability: 100,
            max_durability: 100,
            plus,
            rarity: 0,
            gem_one,
            gem_two,
            rarity_roll: 0,
            bless: 0,
            enchant_hp: 0,
            color: 0,
        };
        let equipment = [
            item(1, 118_309, 1, 3, 13, 0),
            item(2, 120_009, 2, 0, 0, 0),
            item(3, 130_008, 3, 5, 13, 3),
            item(4, 410_309, 4, 7, 3, 3),
            item(5, 900_007, 5, 2, 0, 0),
            item(6, 150_006, 6, 0, 0, 0),
            item(7, 160_009, 8, 4, 0, 0),
        ];
        let power = BattlePower::of(24, 1, &equipment, 5);
        let mut sheet = RgbaImage::from_pixel(EVALUATION_SIZE[0] + DETAILS_SIZE[0] + 10, DETAILS_SIZE[1], image::Rgba([40, 60, 45, 255]));
        let evaluation = evaluation_image(&mut assets, &power, 1, 24).expect("evaluation");
        let details = details_image(&mut assets, &equipment, &power, 0).expect("details");
        imageops::overlay(&mut sheet, &evaluation, 0, 0);
        imageops::overlay(&mut sheet, &details, i64::from(EVALUATION_SIZE[0]) + 10, 0);
        sheet.save(format!("{dir}/battle-power.png")).expect("png");
    }
}
