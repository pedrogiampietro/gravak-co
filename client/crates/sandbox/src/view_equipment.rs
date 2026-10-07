//! Another player's equipment, as the original shows it (`GUI.ini` [0-258]
//! over `data/main/DialogNpcEquip.dds`): the main bar's view-equipment
//! button ([130-1016]), then a click on a player, asks the server
//! (MsgAction 117) for what they wear (MsgItemInfo action 4).

use crate::assets::Assets;
use crate::network::OwnedItem;
use image::RgbaImage;

/// The dialog's canvas (the art keeps its margins: `GUI.ini` places the
/// controls on it).
pub const SIZE: [u32; 2] = [256, 440];
const ART: &str = "data/main/DialogNpcEquip.dds";

/// [258-1376..1386]: each slot and the server position it shows (1 head,
/// 2 neck, 3 armor, 4 right hand, 5 left hand, 6 ring, 7 bracelet,
/// 8 boots, 9 garment).
const SLOTS: [([i32; 4], u8); 9] = [
    ([187, 73, 40, 40], 1),
    ([187, 124, 40, 40], 2),
    ([187, 175, 40, 40], 6),
    ([42, 223, 40, 40], 9),
    ([42, 275, 40, 40], 7),
    ([163, 230, 65, 65], 4),
    ([90, 301, 65, 65], 5),
    ([163, 301, 65, 65], 3),
    ([40, 327, 40, 40], 8),
];
/// [258-1384] the top-right X and [258-1383] the button under the slots.
const CLOSE: [[i32; 4]; 2] = [[225, 43, 15, 13], [160, 390, 63, 20]];
/// [258-1389] the name and [258-1618] the spouse, after the art's labels.
const NAME: [i32; 2] = [100, 64];
const SPOUSE: [i32; 2] = [100, 90];
/// [258-1625] the nobility medal (64 pixels).
const NOBILITY: [i32; 2] = [35, 120];

/// The player looked at and what they wear.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct ViewedEquipment {
    pub id: u32,
    pub name: String,
    pub level: u16,
    pub nobility_rank: u32,
    pub female: bool,
    /// "None" when single, as the original's MsgName 6 answer.
    pub spouse: String,
    pub items: Vec<OwnedItem>,
}

/// Where the dialog stands: at the right, as the original's [0-258]
/// (x 768 of 1024, y 100).
pub fn position(viewport: [f32; 2]) -> [f32; 2] {
    [(viewport[0] - SIZE[0] as f32).max(0.0), 100.0]
}

fn inside([x, y, width, height]: [i32; 4], point: [f32; 2]) -> bool {
    point[0] >= x as f32 && point[1] >= y as f32 && point[0] < (x + width) as f32 && point[1] < (y + height) as f32
}

/// The item under `local` (dialog pixels).
pub fn item_at<'a>(view: &'a ViewedEquipment, local: [f32; 2]) -> Option<&'a OwnedItem> {
    let (_, position) = SLOTS.iter().find(|(rect, _)| inside(*rect, local))?;
    view.items.iter().find(|item| item.position == *position)
}

pub fn close_at(local: [f32; 2]) -> bool {
    CLOSE.iter().any(|rect| inside(*rect, local))
}

/// Whether `local` is on the dialog (its art, not the canvas's margins).
pub fn covers(local: [f32; 2]) -> bool {
    inside([30, 30, 216, 390], local)
}

fn text(image: &mut RgbaImage, value: &str, [x, y]: [i32; 2], colour: [u8; 4]) {
    if let Ok(font) = crate::main_hud::font() {
        crate::main_hud::text(image, font, value, 12.0, [x + 1, y + 1], [0, 0, 0, 255], Some(120));
        crate::main_hud::text(image, font, value, 12.0, [x, y], colour, Some(120));
    }
}

/// The dialog with the items in their slots.
pub fn image(assets: &mut Assets, view: &ViewedEquipment) -> RgbaImage {
    let mut image = RgbaImage::new(SIZE[0], SIZE[1]);
    let _ = crate::original_ui::overlay_asset(&mut image, assets, ART, 0, 0);
    let _ = crate::original_ui::overlay_asset(&mut image, assets, "data/main/CloseBtnS.dds", 223, 41);
    for ([x, y, width, height], position) in SLOTS {
        let Some(item) = view.items.iter().find(|item| item.position == position) else { continue };
        let Ok(icon) = crate::shop::item_icon_image(assets, item.item_type) else { continue };
        let scale = (width as f32 / icon.width() as f32).min(height as f32 / icon.height() as f32);
        let size = [(icon.width() as f32 * scale) as u32, (icon.height() as f32 * scale) as u32];
        let icon = image::imageops::resize(&icon, size[0].max(1), size[1].max(1), image::imageops::FilterType::Triangle);
        image::imageops::overlay(
            &mut image,
            &icon,
            i64::from(x + (width - size[0] as i32) / 2),
            i64::from(y + (height - size[1] as i32) / 2),
        );
        if item.plus > 0 {
            text(&mut image, &format!("+{}", item.plus), [x + width - 18, y + height - 15], [255, 236, 120, 255]);
        }
    }
    text(&mut image, &view.name, NAME, [255, 255, 255, 255]);
    let spouse = if view.spouse.is_empty() { "None" } else { &view.spouse };
    text(&mut image, spouse, SPOUSE, [255, 255, 255, 255]);
    // A noble's medal with its name over it; the commoner shows none.
    if crate::nobility_panel::rank_name(view.nobility_rank, view.female) != "Commoner" {
        for path in crate::nobility_panel::rank_art_paths(view.nobility_rank, view.female, 64) {
            let _ = crate::original_ui::overlay_asset(&mut image, assets, &path, i64::from(NOBILITY[0]), i64::from(NOBILITY[1]));
        }
    }
    image
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(position: u8, item_type: u32) -> OwnedItem {
        OwnedItem { uid: u32::from(position), item_type, position, ..Default::default() }
    }

    fn view() -> ViewedEquipment {
        ViewedEquipment {
            id: 1_000_007,
            name: "BrowserTest".into(),
            level: 130,
            nobility_rank: 12,
            female: false,
            spouse: "Lily".into(),
            items: vec![item(1, 118_309), item(3, 130_309), item(4, 410_339), item(5, 1_050_002), item(8, 160_249)],
        }
    }

    #[test]
    fn the_slots_hold_their_positions() {
        let view = view();
        assert_eq!(item_at(&view, [200.0, 90.0]).map(|item| item.position), Some(1));
        assert_eq!(item_at(&view, [190.0, 260.0]).map(|item| item.position), Some(4));
        assert_eq!(item_at(&view, [200.0, 140.0]), None, "nothing at the neck");
        assert!(close_at([230.0, 48.0]));
    }

    /// `BOUNTY_PREVIEW=dir cargo test -p conquer-sandbox view_equipment_preview`
    #[test]
    fn view_equipment_preview() {
        let Ok(dir) = std::env::var("BOUNTY_PREVIEW") else { return };
        let mut assets = Assets::open(std::path::Path::new("R:/5065")).expect("assets");
        image(&mut assets, &view()).save(format!("{dir}/view-equipment.png")).expect("png");
    }
}
