//! The city artisans' forge window (ArtisanWind in Twin City, ArtisanCloud,
//! ArtisanOu; the server's `artisan_wind`): the worn gear on the left, and
//! in the middle the chosen item, the catalyst it takes and what it becomes.
//! LEVEL raises the item's level with one Meteor, QUALITY its quality with
//! one DragonBall, each at the classic chance; a failure loses the catalyst
//! and halves the item's durability. UPGRADE may be pressed again at once.

use crate::assets::{Assets, Result};
use crate::network::OwnedItem;
use crate::ui::UiRect;
use crate::{labels, shop, ui_theme};
use image::{Rgba, RgbaImage, imageops};

/// The content's size, inside the frame (`shop::framed`).
pub const SIZE: [i32; 2] = [560, 340];
/// MsgAction OpenDialog's `data1` that opens this window.
pub const WINDOW: u32 = 9_020;
/// MsgItem actions the server knows (`artisan_wind::LEVEL`, `QUALITY`).
pub const LEVEL_ACTION: u32 = 9_011;
pub const QUALITY_ACTION: u32 = 9_012;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Craft {
    /// The item's level, with a Meteor.
    Level,
    /// The item's quality, with a DragonBall.
    Quality,
}

impl Craft {
    pub fn action(self) -> u32 {
        match self {
            Self::Level => LEVEL_ACTION,
            Self::Quality => QUALITY_ACTION,
        }
    }
}

/// What came of the last press, and when (for its flash).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Outcome {
    Upgraded,
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ForgeWindow {
    /// The chosen item's uid.
    pub selected: Option<u32>,
    pub craft: Craft,
    /// Waiting for the server's answer.
    pub busy: bool,
    pub last: Option<(Outcome, f32)>,
}

impl ForgeWindow {
    pub fn new(equipment: &[OwnedItem]) -> Self {
        // The weapon first, else the first item that can be worked on.
        let selected = equipment
            .iter()
            .filter(|item| upgradable(item))
            .min_by_key(|item| if item.position == 4 { 0 } else { item.position })
            .map(|item| item.uid);
        Self { selected, craft: Craft::Level, busy: false, last: None }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ForgeAction {
    Select(u32),
    Craft(Craft),
    Upgrade,
    Close,
}

/// The worn gear's slots, head to boots (positions 1..=8), in a 2x4 grid.
const SLOT_ORDER: [u8; 8] = [1, 2, 3, 6, 4, 5, 7, 8];
const SLOT_NAMES: [&str; 8] = ["Head", "Neck", "Armor", "Ring", "Weapon", "Left hand", "Bracelet", "Boots"];
const GEAR_BOX: UiRect = UiRect { x: 16, y: 46, width: 168, height: 278 };
const TAB_LEVEL: UiRect = UiRect { x: 200, y: 46, width: 168, height: 26 };
const TAB_QUALITY: UiRect = UiRect { x: 376, y: 46, width: 168, height: 26 };
const ITEM_SLOT: UiRect = UiRect { x: 212, y: 104, width: 64, height: 64 };
const CATALYST_SLOT: UiRect = UiRect { x: 324, y: 104, width: 64, height: 64 };
const RESULT_SLOT: UiRect = UiRect { x: 466, y: 104, width: 64, height: 64 };
const UPGRADE: UiRect = UiRect { x: 262, y: 296, width: 120, height: 30 };
const CLOSE: UiRect = UiRect { x: 394, y: 296, width: 120, height: 30 };
/// The frame's close button (`shop::framed`).
const CLOSE_X: UiRect = UiRect { x: SIZE[0] - 31, y: 6, width: 27, height: 30 };

fn gear_slot(index: usize) -> UiRect {
    let (column, row) = (index % 2, index / 2);
    UiRect::new(GEAR_BOX.x + 14 + column as i32 * 76, GEAR_BOX.y + 30 + row as i32 * 60, 64, 52)
}

/// Gear the artisans work on: the paper doll's head to boots, not the
/// garment, arrows or weapon accessories (the server's `upgradable`).
pub fn upgradable(item: &OwnedItem) -> bool {
    (1..=8).contains(&item.position) && item.item_type / 1_000 != 1_050
}

fn worn(equipment: &[OwnedItem], position: u8) -> Option<&OwnedItem> {
    equipment.iter().find(|item| item.position == position)
}

fn inside(rect: UiRect, [x, y]: [i32; 2]) -> bool {
    x >= rect.x && x < rect.x + rect.width && y >= rect.y && y < rect.y + rect.height
}

/// Where the window stands: in the middle of the screen.
pub fn position(viewport: [f32; 2]) -> [f32; 2] {
    [
        ((viewport[0] - SIZE[0] as f32) * 0.5).max(0.0).round(),
        ((viewport[1] - SIZE[1] as f32) * 0.5).max(0.0).round(),
    ]
}

pub fn covers(local: [f32; 2]) -> bool {
    local[0] >= 0.0 && local[1] >= 0.0 && local[0] < SIZE[0] as f32 && local[1] < SIZE[1] as f32
}

/// What a click at `local` (in the window) asks for.
pub fn action_at(equipment: &[OwnedItem], local: [f32; 2]) -> Option<ForgeAction> {
    let point = [local[0] as i32, local[1] as i32];
    if inside(CLOSE, point) || inside(CLOSE_X, point) {
        return Some(ForgeAction::Close);
    }
    if inside(UPGRADE, point) {
        return Some(ForgeAction::Upgrade);
    }
    if inside(TAB_LEVEL, point) {
        return Some(ForgeAction::Craft(Craft::Level));
    }
    if inside(TAB_QUALITY, point) {
        return Some(ForgeAction::Craft(Craft::Quality));
    }
    SLOT_ORDER.iter().enumerate().find_map(|(index, position)| {
        let item = worn(equipment, *position).filter(|item| upgradable(item))?;
        inside(gear_slot(index), point).then_some(ForgeAction::Select(item.uid))
    })
}

/// What the chosen item would become, the catalyst and the chance; or why
/// it cannot be done.
pub struct Offer {
    pub next: u32,
    pub catalyst: u32,
    pub chance: u32,
}

pub fn offer(assets: &mut Assets, craft: Craft, item: &OwnedItem) -> std::result::Result<Offer, &'static str> {
    if !upgradable(item) {
        return Err("The artisan does not work on this item.");
    }
    let requirement = shop::item_info(assets, item.item_type).map_or(0, |info| info.level);
    let candidates: Vec<u32> = (6..=9)
        .map(|quality| item.item_type / 10 * 10 + quality)
        .chain((1..10).map(|step| item.item_type + 10 * step))
        .filter(|id| shop::item_info(assets, *id).is_some())
        .collect();
    let exists = |id: u32| candidates.contains(&id);
    match craft {
        Craft::Level => {
            if requirement >= 120 {
                return Err("Above level 120 only the WeaponMaster raises an item's level.");
            }
            let next = crate::forge_rules::next_level(item.item_type, exists).ok_or("This item is at its highest level.")?;
            Ok(Offer { next, catalyst: crate::forge_rules::METEOR, chance: crate::forge_rules::level_chance(item.item_type) })
        }
        Craft::Quality => {
            let next = crate::forge_rules::next_quality(item.item_type, exists).ok_or("This item's quality cannot be raised.")?;
            Ok(Offer {
                next,
                catalyst: crate::forge_rules::DRAGONBALL,
                chance: crate::forge_rules::quality_chance(item.item_type, requirement),
            })
        }
    }
}

/// Whether UPGRADE can be pressed now.
pub fn ready(assets: &mut Assets, view: &ForgeWindow, equipment: &[OwnedItem], bag: &[OwnedItem]) -> bool {
    let Some(item) = view.selected.and_then(|uid| equipment.iter().find(|item| item.uid == uid)) else {
        return false;
    };
    !view.busy
        && offer(assets, view.craft, item)
            .is_ok_and(|offer| crate::forge_rules::count(bag, offer.catalyst, item.uid) >= 1)
}

const QUALITIES: [&str; 10] = ["", "", "", "Normal", "Normal", "Normal", "Refined", "Unique", "Elite", "Super"];

fn quality_name(item_type: u32) -> &'static str {
    QUALITIES[(item_type % 10) as usize]
}

/// The window, framed, with its title.
pub fn image(
    assets: &mut Assets,
    view: &ForgeWindow,
    equipment: &[OwnedItem],
    bag: &[OwnedItem],
    hovered: Option<ForgeAction>,
    title: &str,
    now: f32,
) -> Result<RgbaImage> {
    let mut image = RgbaImage::from_pixel(SIZE[0] as u32, SIZE[1] as u32, Rgba([0, 0, 0, 0]));
    let font = labels::font()?;
    let text = |image: &mut RgbaImage, value: &str, colour: [u8; 3], at: [i32; 2], size: f32| {
        labels::text_sized(image, &font, &[(value, colour)], [at[0] as f32, at[1] as f32], size);
    };
    let panel = |image: &mut RgbaImage, rect: UiRect| {
        ui_theme::fill(image, rect, [7, 18, 27, 255]);
        ui_theme::outline(image, rect, [126, 91, 39, 255]);
    };

    // The worn gear: each upgradable item in its slot, the chosen one lit.
    panel(&mut image, GEAR_BOX);
    text(&mut image, "YOUR EQUIPMENT", ui_theme::TEXT_GOLD, [GEAR_BOX.x + 12, GEAR_BOX.y + 10], 10.0);
    for (index, position) in SLOT_ORDER.iter().enumerate() {
        let rect = gear_slot(index);
        let item = worn(equipment, *position);
        let usable = item.is_some_and(upgradable);
        let selected = item.is_some_and(|item| Some(item.uid) == view.selected);
        let over = item.is_some_and(|item| hovered == Some(ForgeAction::Select(item.uid)));
        ui_theme::slot(&mut image, UiRect::new(rect.x + 10, rect.y, 44, 44), over, selected, usable);
        match item {
            Some(item) if usable => {
                if let Ok(icon) = shop::item_icon_image(assets, item.item_type) {
                    let icon = imageops::resize(&icon, 40, 40, imageops::FilterType::Triangle);
                    imageops::overlay(&mut image, &icon, i64::from(rect.x + 12), i64::from(rect.y + 2));
                }
            }
            _ => text(&mut image, SLOT_NAMES[index], ui_theme::TEXT_MUTED, [rect.x + 12, rect.y + 16], 8.0),
        }
    }

    // LEVEL and QUALITY.
    for (rect, craft, label) in [(TAB_LEVEL, Craft::Level, "LEVEL  ·  Meteor"), (TAB_QUALITY, Craft::Quality, "QUALITY  ·  DragonBall")] {
        let active = view.craft == craft;
        let over = hovered == Some(ForgeAction::Craft(craft));
        ui_theme::fill(&mut image, rect, if active { [58, 44, 20, 255] } else if over { [34, 32, 26, 255] } else { [16, 22, 26, 255] });
        ui_theme::outline(&mut image, rect, if active { [255, 214, 102, 255] } else { [126, 91, 39, 255] });
        let width = crate::main_hud::text_width(&font, label, 10.0);
        text(&mut image, label, if active { ui_theme::TEXT_GOLD } else { ui_theme::TEXT_MUTED }, [rect.x + (rect.width - width) / 2, rect.y + 8], 10.0);
    }

    // The forge: item + catalyst >> result.
    let forge = UiRect::new(200, 80, 344, 120);
    panel(&mut image, forge);
    let item = view.selected.and_then(|uid| equipment.iter().find(|item| item.uid == uid)).copied();
    let offer = item.map(|item| offer(assets, view.craft, &item));
    let catalyst = match view.craft {
        Craft::Level => crate::forge_rules::METEOR,
        Craft::Quality => crate::forge_rules::DRAGONBALL,
    };
    let have = crate::forge_rules::count(bag, catalyst, item.map_or(0, |item| item.uid));
    ui_theme::slot(&mut image, ITEM_SLOT, false, item.is_some(), item.is_some());
    ui_theme::slot(&mut image, CATALYST_SLOT, false, false, true);
    let can = matches!(offer, Some(Ok(_)));
    ui_theme::slot(&mut image, RESULT_SLOT, false, can, can);
    let icon_at = |image: &mut RgbaImage, assets: &mut Assets, item_type: u32, slot: UiRect| {
        if let Ok(icon) = shop::item_icon_image(assets, item_type) {
            imageops::overlay(image, &icon, i64::from(slot.x + 16), i64::from(slot.y + 16));
        }
    };
    if let Some(item) = item {
        icon_at(&mut image, assets, item.item_type, ITEM_SLOT);
    }
    icon_at(&mut image, assets, catalyst, CATALYST_SLOT);
    text(&mut image, "+", ui_theme::TEXT_GOLD, [ITEM_SLOT.x + ITEM_SLOT.width + 16, ITEM_SLOT.y + 22], 18.0);
    text(&mut image, ">>", ui_theme::TEXT_GOLD, [CATALYST_SLOT.x + CATALYST_SLOT.width + 22, CATALYST_SLOT.y + 22], 18.0);
    if let Some(Ok(offer)) = &offer {
        icon_at(&mut image, assets, offer.next, RESULT_SLOT);
    }
    let needs = format!("1  (you have {have})");
    text(&mut image, &needs, if have > 0 { ui_theme::TEXT } else { [255, 174, 65] }, [CATALYST_SLOT.x - 4, CATALYST_SLOT.y + 72], 9.0);

    // What it is, and what it becomes.
    let describe = |assets: &mut Assets, item_type: u32, plus: u8| {
        let level = shop::item_info(assets, item_type).map_or(0, |info| info.level);
        let plus = if plus > 0 { format!(" +{plus}") } else { String::new() };
        (format!("{}{plus}", shop::item_name(assets, item_type)), format!("{}, level {level}", quality_name(item_type)))
    };
    let details = UiRect::new(200, 208, 344, 78);
    panel(&mut image, details);
    match (item, &offer) {
        (None, _) => text(&mut image, "Choose an item on the left.", ui_theme::TEXT_MUTED, [details.x + 12, details.y + 12], 10.0),
        (Some(item), Some(Ok(offer))) => {
            let (name, line) = describe(assets, item.item_type, item.plus);
            text(&mut image, &name, ui_theme::TEXT, [details.x + 12, details.y + 10], 10.0);
            text(&mut image, &line, ui_theme::TEXT_MUTED, [details.x + 12, details.y + 26], 9.0);
            let (next_name, next_line) = describe(assets, offer.next, item.plus);
            text(&mut image, &next_name, ui_theme::TEXT_GOLD, [details.x + 186, details.y + 10], 10.0);
            text(&mut image, &next_line, ui_theme::TEXT_GOLD, [details.x + 186, details.y + 26], 9.0);
            let chance = format!("Chance {}%", offer.chance);
            let colour = match offer.chance {
                70.. => [99, 221, 133],
                35.. => [255, 214, 102],
                _ => [232, 100, 82],
            };
            text(&mut image, &chance, colour, [details.x + 12, details.y + 48], 12.0);
            text(
                &mut image,
                "On failure the catalyst is lost and durability halves.",
                ui_theme::TEXT_MUTED,
                [details.x + 106, details.y + 51],
                8.5,
            );
        }
        (Some(item), Some(Err(why))) => {
            let (name, line) = describe(assets, item.item_type, item.plus);
            text(&mut image, &name, ui_theme::TEXT, [details.x + 12, details.y + 10], 10.0);
            text(&mut image, &line, ui_theme::TEXT_MUTED, [details.x + 12, details.y + 26], 9.0);
            text(&mut image, why, [255, 174, 65], [details.x + 12, details.y + 50], 9.5);
        }
        (Some(_), None) => {}
    }

    // The last press, flashing for two seconds.
    if let Some((outcome, at)) = view.last
        && now - at < 2.0
    {
        let (label, colour) = match outcome {
            Outcome::Upgraded => ("SUCCESS!", [99, 221, 133]),
            Outcome::Failed => ("FAILED", [232, 100, 82]),
        };
        let width = crate::main_hud::text_width(&font, label, 16.0);
        text(&mut image, label, colour, [forge.x + (forge.width - width) / 2, forge.y + 8], 16.0);
    }

    let ready = ready(assets, view, equipment, bag);
    let label = if view.busy { "FORGING..." } else { "UPGRADE" };
    shop::draw_upgrade_action_button(&mut image, &font, UPGRADE, label, hovered == Some(ForgeAction::Upgrade), ready, true);
    shop::draw_upgrade_action_button(&mut image, &font, CLOSE, "CLOSE", hovered == Some(ForgeAction::Close), true, false);
    shop::framed(assets, image, title, hovered == Some(ForgeAction::Close))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(uid: u32, item_type: u32, position: u8) -> OwnedItem {
        OwnedItem { uid, item_type, position, durability: 10, max_durability: 10, ..Default::default() }
    }

    #[test]
    fn it_offers_the_gear_but_not_arrows_garments_or_accessories() {
        let gear = [item(1, 500_197, 4), item(2, 1_050_002, 5), item(3, 181_305, 9), item(4, 370_001, 15), item(5, 130_003, 3)];
        assert_eq!(ForgeWindow::new(&gear).selected, Some(1), "the weapon first");
        assert!(upgradable(&gear[0]) && upgradable(&gear[4]));
        assert!(!upgradable(&gear[1]) && !upgradable(&gear[2]) && !upgradable(&gear[3]));
        // The weapon's slot (index 4 of the grid) selects it; the arrows'
        // (index 5) do not.
        let weapon = gear_slot(4);
        let at = [(weapon.x + 20) as f32, (weapon.y + 20) as f32];
        assert_eq!(action_at(&gear, at), Some(ForgeAction::Select(1)));
        let arrows = gear_slot(5);
        assert_eq!(action_at(&gear, [(arrows.x + 20) as f32, (arrows.y + 20) as f32]), None);
        assert_eq!(action_at(&gear, [(UPGRADE.x + 5) as f32, (UPGRADE.y + 5) as f32]), Some(ForgeAction::Upgrade));
    }

    /// `GRAVAK_FORGE_WINDOW=dir cargo test -p conquer-sandbox forge_window_preview`.
    #[test]
    fn forge_window_preview() {
        let Ok(dir) = std::env::var("GRAVAK_FORGE_WINDOW") else { return };
        let mut assets = Assets::open(std::path::Path::new("R:/5065")).expect("assets");
        let gear = [item(1, 500_197, 4), item(2, 1_050_002, 5), item(5, 130_003, 3), item(6, 120_183, 2), item(7, 150_213, 6), item(8, 160_213, 8)];
        let bag: Vec<OwnedItem> = (0..4).map(|index| item(20 + index, crate::forge_rules::METEOR, index as u8)).collect();
        let mut view = ForgeWindow::new(&gear);
        view.last = Some((Outcome::Upgraded, 0.0));
        image(&mut assets, &view, &gear, &bag, Some(ForgeAction::Upgrade), "ArtisanWind", 0.5).expect("level").save(format!("{dir}/forge-level.png")).expect("png");
        view.craft = Craft::Quality;
        view.last = None;
        image(&mut assets, &view, &gear, &bag, Some(ForgeAction::Select(5)), "ArtisanWind", 0.5).expect("quality").save(format!("{dir}/forge-quality.png")).expect("png");
    }
}
