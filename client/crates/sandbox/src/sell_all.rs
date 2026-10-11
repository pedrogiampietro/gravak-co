//! Sell All at an NPC shop: every inventory item not kept back is sold in
//! one go. Two tabs: Sell All, what goes (a green check), and BlackList,
//! what is kept (a red cross). Items show once per type with how many there
//! are, and a click moves the whole type to the other tab: one arrow pack
//! on the blacklist keeps them all. The blacklist is saved per character
//! (`settings/sell_<id>.ini`). Valuable items are always kept: anything with
//! a plus, gems, blessing or enchantment, DragonBalls, Meteors, gems and
//! ExpBalls. Rare, Elite and Super gear starts kept, but a click lets its
//! kind be sold (`settings/sell_ok_<id>.ini`).
//!
//! Drawn like the Highscores and the Reward Wall: the quick panels' frame,
//! gold chips for the tabs, a teal card around the slots and the ornate
//! buttons.

use crate::assets::{Assets, Result};
use crate::components::{self, WidgetState};
use crate::daily_rewards_panel::{card, glow_in, paint_in, rounded, rule, say, segment, slot, tick, width_of, GOLD_LIGHT, GREEN};
use crate::highscores_panel::chip;
use crate::network::OwnedItem;
use crate::paint;
use crate::ui::UiRect;
use crate::{labels, ui_theme};
use image::{Rgba, RgbaImage};
use std::collections::{BTreeSet, HashMap};

pub const WIDTH: i32 = 460;
pub const HEIGHT: i32 = 336;
const TAB_SELL: UiRect = UiRect::new(18, 42, 206, 26);
const TAB_KEPT: UiRect = UiRect::new(236, 42, 206, 26);
const HINT_Y: i32 = 76;
const GRID_CARD: UiRect = UiRect::new(14, 94, 432, 184);
const GRID: [i32; 2] = [22, 104];
const STEP: i32 = 42;
const SLOT: i32 = 38;
const COLUMNS: usize = 10;
const ROWS: usize = 4;
const VALUE_Y: i32 = 294;
const SELL_BUTTON: UiRect = UiRect::new(258, 288, 88, 28);
const CLOSE_BUTTON: UiRect = UiRect::new(354, 288, 88, 28);
/// The frame's close button (`shop::framed`).
const CLOSE_CROSS: UiRect = UiRect::new(WIDTH - 31, 6, 27, 30);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SellTab {
    Sell,
    Kept,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SellAllAction {
    Tab(SellTab),
    /// This item type moves to the other tab.
    Toggle(u32),
    SellAll,
    Close,
}

/// Items never sold by Sell All, whatever the lists say.
pub fn protected(item: &OwnedItem) -> bool {
    item.plus > 0
        || item.gem_one != 0
        || item.gem_two != 0
        || item.bless > 0
        || item.enchant_hp > 0
        || matches!(item.item_type, 1_088_000..=1_088_002 | 723_700)
        || (700_000..710_000).contains(&item.item_type)
}

/// Gear kept until its kind is let go: rare, Elite or Super.
pub fn precious(item: &OwnedItem) -> bool {
    item.rarity > 0 || item.item_type < 1_000_000 && item.item_type % 10 >= 8
}

/// What the shop pays for `item` (the server's rule): a third of the price,
/// worn down with its durability.
pub fn sale_price(base: u32, item: &OwnedItem) -> u32 {
    let price = if item.max_durability == 0 {
        u64::from(base) / 3
    } else {
        // A stack is worth its units (its durability over one unit's).
        let durability =
            if crate::shop::stackable(item.item_type) { item.durability } else { item.durability.min(item.max_durability) };
        u64::from(base) * u64::from(durability) / u64::from(item.max_durability) / 3
    };
    u32::try_from(price).unwrap_or(u32::MAX)
}

pub struct SellAllView {
    pub tab: SellTab,
    pub blacklist: BTreeSet<u32>,
    /// Kinds whose rare, Elite and Super pieces are sold too.
    pub released: BTreeSet<u32>,
    icons: HashMap<u32, RgbaImage>,
    prices: HashMap<u32, u32>,
    /// Top-left corner in window pixels.
    pub position: [i32; 2],
    character_id: Option<u32>,
    /// Opened at a shop, so it can sell; the helper opens it elsewhere for
    /// its BlackList alone.
    pub at_shop: bool,
}

impl SellAllView {
    /// Opens over the middle of the window with the icons and prices of
    /// what `inventory` holds and the character's saved blacklist.
    pub fn open(assets: &mut Assets, inventory: &[OwnedItem], character_id: Option<u32>, viewport: [f32; 2]) -> Self {
        let mut types: Vec<u32> = inventory.iter().map(|item| item.item_type).collect();
        types.sort_unstable();
        types.dedup();
        Self {
            tab: SellTab::Sell,
            blacklist: character_id.map(load_blacklist).unwrap_or_default(),
            released: character_id.map(|id| load_list(released_path(id))).unwrap_or_default(),
            icons: crate::shop::item_icons(assets, &types),
            prices: crate::shop::base_prices(assets, &types),
            position: [
                ((viewport[0] - WIDTH as f32) * 0.5).max(8.0) as i32,
                ((viewport[1] - HEIGHT as f32) * 0.5).max(8.0) as i32,
            ],
            character_id,
            at_shop: true,
        }
    }

    fn base_price(&self, item: &OwnedItem) -> u32 {
        self.prices.get(&item.item_type).copied().unwrap_or(0)
    }

    /// Whether Sell All sells `item`.
    pub fn sells(&self, item: &OwnedItem) -> bool {
        !protected(item)
            && (!precious(item) || self.released.contains(&item.item_type))
            && !self.blacklist.contains(&item.item_type)
            && sale_price(self.base_price(item), item) > 0
    }

    /// The open tab's items, one per type with how many of it there are,
    /// in inventory order.
    fn shown<'a>(&self, inventory: &'a [OwnedItem]) -> Vec<(&'a OwnedItem, usize)> {
        let mut items: Vec<&OwnedItem> = inventory
            .iter()
            .filter(|item| (self.tab == SellTab::Sell) == self.sells(item))
            .collect();
        items.sort_by_key(|item| item.position);
        let mut groups: Vec<(&OwnedItem, usize)> = Vec::new();
        for item in items {
            match groups.iter_mut().find(|(first, _)| first.item_type == item.item_type) {
                Some((_, count)) => *count += 1,
                None => groups.push((item, 1)),
            }
        }
        groups.truncate(COLUMNS * ROWS);
        groups
    }

    /// What Sell All sells now.
    pub fn to_sell(&self, inventory: &[OwnedItem]) -> Vec<OwnedItem> {
        let mut items: Vec<OwnedItem> = inventory.iter().filter(|item| self.sells(item)).copied().collect();
        items.sort_by_key(|item| item.position);
        items
    }

    /// The silver Sell All brings.
    pub fn value(&self, inventory: &[OwnedItem]) -> u64 {
        self.to_sell(inventory)
            .iter()
            .map(|item| u64::from(sale_price(self.base_price(item), item)))
            .sum()
    }

    /// Moves `item_type` to the other tab; `false` when all of it that is
    /// kept is valuable and stays kept.
    pub fn toggle(&mut self, inventory: &[OwnedItem], item_type: u32) -> bool {
        match self.tab {
            SellTab::Sell => {
                self.blacklist.insert(item_type);
                self.released.remove(&item_type);
            }
            SellTab::Kept => {
                let kept: Vec<&OwnedItem> =
                    inventory.iter().filter(|item| item.item_type == item_type && !self.sells(item)).collect();
                if !kept.is_empty() && kept.iter().all(|item| protected(item)) {
                    return false;
                }
                self.blacklist.remove(&item_type);
                if kept.iter().any(|item| precious(item)) {
                    self.released.insert(item_type);
                }
            }
        }
        if let Some(character_id) = self.character_id {
            save_blacklist(character_id, &self.blacklist);
            if let Some(path) = released_path(character_id) {
                save_list(path, &self.released);
            }
        }
        true
    }

    fn slot(index: usize) -> UiRect {
        UiRect::new(
            GRID[0] + (index % COLUMNS) as i32 * STEP,
            GRID[1] + (index / COLUMNS) as i32 * STEP,
            SLOT,
            SLOT,
        )
    }

    pub fn contains(&self, cursor: [f32; 2]) -> bool {
        let local = self.local(cursor);
        UiRect::new(0, 0, WIDTH, HEIGHT).contains_local(local)
    }

    fn local(&self, cursor: [f32; 2]) -> [f32; 2] {
        [cursor[0] - self.position[0] as f32, cursor[1] - self.position[1] as f32]
    }

    pub fn action_at(&self, inventory: &[OwnedItem], cursor: [f32; 2]) -> Option<SellAllAction> {
        let local = self.local(cursor);
        if TAB_SELL.contains_local(local) {
            return Some(SellAllAction::Tab(SellTab::Sell));
        }
        if TAB_KEPT.contains_local(local) {
            return Some(SellAllAction::Tab(SellTab::Kept));
        }
        if CLOSE_BUTTON.contains_local(local) || CLOSE_CROSS.contains_local(local) {
            return Some(SellAllAction::Close);
        }
        if self.tab == SellTab::Sell && SELL_BUTTON.contains_local(local) {
            return Some(SellAllAction::SellAll);
        }
        self.shown(inventory)
            .iter()
            .enumerate()
            .find(|(index, _)| Self::slot(*index).contains_local(local))
            .map(|(_, (item, _))| SellAllAction::Toggle(item.item_type))
    }

    /// A key that changes whenever the window looks different.
    pub fn signature(&self, inventory: &[OwnedItem], hovered: Option<SellAllAction>, can_sell: bool) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.tab.hash(&mut hasher);
        self.blacklist.hash(&mut hasher);
        self.released.hash(&mut hasher);
        for item in inventory {
            (item.uid, item.item_type, item.position, item.durability).hash(&mut hasher);
        }
        hovered.hash(&mut hasher);
        can_sell.hash(&mut hasher);
        hasher.finish()
    }

    pub fn image(&self, assets: &mut Assets, inventory: &[OwnedItem], hovered: Option<SellAllAction>, can_sell: bool) -> Result<RgbaImage> {
        // Clear: `shop::framed` puts the quick panels' frame, title and
        // close button around it.
        let mut image = RgbaImage::from_pixel(WIDTH as u32, HEIGHT as u32, Rgba([0, 0, 0, 0]));
        let font = labels::font()?;
        let selling = self.to_sell(inventory).len();
        let kept = inventory.len() - selling;
        for (rect, tab, label) in [
            (TAB_SELL, SellTab::Sell, format!("Sell All ({selling})")),
            (TAB_KEPT, SellTab::Kept, format!("BlackList ({kept})")),
        ] {
            chip(&mut image, &font, rect, &label, self.tab == tab, 12.0);
            if hovered == Some(SellAllAction::Tab(tab)) && self.tab != tab {
                let shape = rounded(rect, 5.0);
                paint_in(&mut image, rect, 1, |x, y| shape(x, y).abs() - 0.6, |_, _| paint::hex(0x9c7434), 1.0);
            }
        }
        let hint = match self.tab {
            SellTab::Sell => "Click an item to keep its kind (BlackList).",
            SellTab::Kept => "Click to sell its kind (rare and Super too). Grey: always kept.",
        };
        say(&mut image, &font, hint, ui_theme::TEXT_MUTED, 20, HINT_Y, 11.0);

        card(&mut image, GRID_CARD, false, false, false);
        let shown = self.shown(inventory);
        for index in 0..COLUMNS * ROWS {
            let rect = Self::slot(index);
            let group = shown.get(index);
            slot(&mut image, rect, group.is_none());
            let Some(&(item, count)) = group else { continue };
            if hovered == Some(SellAllAction::Toggle(item.item_type)) {
                let shape = rounded(rect, 3.0);
                glow_in(&mut image, rect, &shape, 5.0, paint::hex(0xffd36b), 0.45);
                paint_in(&mut image, rect, 1, |x, y| shape(x, y).abs() - 0.8, |_, _| GOLD_LIGHT, 1.0);
            }
            if let Some(icon) = self.icons.get(&item.item_type) {
                let room = (SLOT - 4) as u32;
                let resized;
                let icon = if icon.width() > room || icon.height() > room {
                    resized = image::imageops::resize(icon, room, room, image::imageops::FilterType::Triangle);
                    &resized
                } else {
                    icon
                };
                let x = rect.x + (SLOT - icon.width() as i32) / 2;
                let y = rect.y + (SLOT - icon.height() as i32) / 2;
                image::imageops::overlay(&mut image, icon, i64::from(x), i64::from(y));
            }
            let mark = [(rect.x + SLOT - 7) as f32, (rect.y + 7) as f32];
            if self.tab == SellTab::Sell {
                check(&mut image, mark);
            } else {
                cross(&mut image, mark, protected(item));
            }
            if count > 1 {
                let text = count.to_string();
                let x = rect.x + rect.width - 3 - width_of(&font, &text, 10.0).round() as i32;
                say(&mut image, &font, &text, [255, 246, 216], x, rect.y + rect.height - 13, 10.0);
            }
        }

        rule(&mut image, GRID_CARD.x + 8, GRID_CARD.x + GRID_CARD.width - 8, GRID_CARD.y + GRID_CARD.height + 5);
        let label = "Sale value:";
        say(&mut image, &font, label, ui_theme::TEXT_MUTED, 22, VALUE_Y + 1, 12.0);
        let value = format_silver(self.value(inventory));
        let value_x = 22 + width_of(&font, label, 12.0).round() as i32 + 8;
        say(&mut image, &font, &value, [255, 226, 143], value_x, VALUE_Y, 13.0);
        let silver_x = value_x + width_of(&font, &value, 13.0).round() as i32 + 5;
        say(&mut image, &font, "silver", ui_theme::TEXT_MUTED, silver_x, VALUE_Y + 1, 12.0);

        if self.tab == SellTab::Sell {
            let state = if !can_sell || selling == 0 {
                WidgetState::Disabled
            } else if hovered == Some(SellAllAction::SellAll) {
                WidgetState::Hovered
            } else {
                WidgetState::Normal
            };
            components::draw_button(&mut image, &font, SELL_BUTTON, "Sell All", state);
        }
        let state = if hovered == Some(SellAllAction::Close) { WidgetState::Hovered } else { WidgetState::Normal };
        components::draw_button(&mut image, &font, CLOSE_BUTTON, "Close", state);
        crate::shop::framed(assets, image, "Sell All", hovered == Some(SellAllAction::Close))
    }
}

/// A small dark disc behind a mark at `centre`.
fn disc(image: &mut RgbaImage, centre: [f32; 2], ring: paint::Colour) {
    let radius = 6.0;
    let bounds = UiRect::new((centre[0] - radius) as i32 - 1, (centre[1] - radius) as i32 - 1, 15, 15);
    paint_in(image, bounds, 1, |x, y| paint::circle(x, y, centre, radius), |_, _| paint::hex(0x07131b), 0.9);
    paint_in(image, bounds, 1, |x, y| paint::circle(x, y, centre, radius).abs() - 0.5, |_, _| ring, 1.0);
}

/// A green check: the item is sold.
fn check(image: &mut RgbaImage, centre: [f32; 2]) {
    disc(image, centre, paint::hex(0x2f6a36));
    tick(image, centre, 5.5, GREEN);
}

/// A red cross: the item is kept; grey when it is valuable and cannot be
/// sold.
fn cross(image: &mut RgbaImage, centre: [f32; 2], locked: bool) {
    let colour = if locked { paint::hex(0xa8a08c) } else { paint::hex(0xe2443a) };
    disc(image, centre, if locked { paint::hex(0x4a463c) } else { paint::hex(0x7a2420) });
    let [cx, cy] = centre;
    let r = 2.6;
    let bounds = UiRect::new(cx as i32 - 5, cy as i32 - 5, 11, 11);
    paint_in(
        image,
        bounds,
        1,
        |x, y| segment(x, y, [cx - r, cy - r], [cx + r, cy + r]).min(segment(x, y, [cx + r, cy - r], [cx - r, cy + r])) - 0.9,
        |_, _| colour,
        1.0,
    );
}

fn format_silver(value: u64) -> String {
    let digits = value.to_string();
    let mut out = String::new();
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index) % 3 == 0 {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

fn blacklist_path(character_id: u32) -> Option<std::path::PathBuf> {
    Some(std::env::current_exe().ok()?.parent()?.join("settings").join(format!("sell_{character_id}.ini")))
}

fn released_path(character_id: u32) -> Option<std::path::PathBuf> {
    Some(std::env::current_exe().ok()?.parent()?.join("settings").join(format!("sell_ok_{character_id}.ini")))
}

/// The item types the character keeps from Sell All.
pub fn load_blacklist(character_id: u32) -> BTreeSet<u32> {
    load_list(blacklist_path(character_id))
}

fn load_list(path: Option<std::path::PathBuf>) -> BTreeSet<u32> {
    path.and_then(|path| std::fs::read_to_string(path).ok())
        .map(|text| {
            text.split(|c: char| c == ',' || c.is_whitespace())
                .filter_map(|value| value.trim().parse().ok())
                .collect()
        })
        .unwrap_or_default()
}

fn save_blacklist(character_id: u32, blacklist: &BTreeSet<u32>) {
    if let Some(path) = blacklist_path(character_id) {
        save_list(path, blacklist);
    }
}

fn save_list(path: std::path::PathBuf, list: &BTreeSet<u32>) {
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let text = list.iter().map(u32::to_string).collect::<Vec<_>>().join(",");
    if let Err(error) = std::fs::write(&path, text) {
        eprintln!("Could not save the Sell All list: {error}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(uid: u32, item_type: u32, position: u8) -> OwnedItem {
        OwnedItem {
            uid,
            item_type,
            position,
            durability: 10,
            max_durability: 10,
            plus: 0,
            rarity: 0,
            gem_one: 0,
            gem_two: 0,
            affixes: [0; 4],
            bless: 0,
            enchant_hp: 0,
            color: 0,
            skin: 0,
        }
    }

    fn view(prices: &[(u32, u32)]) -> SellAllView {
        SellAllView {
            tab: SellTab::Sell,
            blacklist: BTreeSet::new(),
            released: BTreeSet::new(),
            icons: HashMap::new(),
            prices: prices.iter().copied().collect(),
            position: [0, 0],
            character_id: None,
            at_shop: true,
        }
    }

    #[test]
    fn valuables_and_blacklisted_types_stay() {
        let mut view = view(&[(1_000_000, 30), (410_105, 900), (410_109, 900), (1_088_000, 9_000)]);
        let mut plus = item(4, 410_105, 3);
        plus.plus = 1;
        let inventory = [
            item(1, 1_000_000, 0),
            item(2, 410_105, 1),
            item(3, 410_109, 2),
            plus,
            item(5, 1_088_000, 4),
        ];
        let sold: Vec<u32> = view.to_sell(&inventory).iter().map(|item| item.uid).collect();
        assert_eq!(sold, vec![1, 2]);
        assert_eq!(view.value(&inventory), 10 + 300);
        assert!(view.toggle(&inventory, 410_105));
        assert_eq!(view.to_sell(&inventory).len(), 1);
        // A protected item cannot be moved to the sale.
        view.tab = SellTab::Kept;
        assert!(!view.toggle(&inventory, 1_088_000));
        assert!(view.toggle(&inventory, 410_105));
        assert_eq!(view.to_sell(&inventory).len(), 2);
        // Super (…9) and rare gear starts kept; a click lets its kind go.
        assert!(!view.sells(&inventory[2]));
        assert!(view.toggle(&inventory, 410_109));
        assert!(view.sells(&inventory[2]));
        let rare = OwnedItem { rarity: 2, ..item(6, 410_105, 6) };
        assert!(!view.sells(&rare), "410105 was let go plain, not rare");
        assert!(view.toggle(&[rare], 410_105));
        assert!(view.sells(&rare));
        // Back to the BlackList: kept again, rare and all.
        view.tab = SellTab::Sell;
        assert!(view.toggle(&inventory, 410_109));
        assert!(!view.sells(&inventory[2]));
    }

    #[test]
    fn one_slot_per_kind_and_one_click_keeps_them_all() {
        let mut view = view(&[(1_050_002, 30), (1_000_000, 20)]);
        let inventory = [item(1, 1_050_002, 0), item(2, 1_000_000, 1), item(3, 1_050_002, 2), item(4, 1_050_002, 5)];
        let shown: Vec<(u32, usize)> = view.shown(&inventory).iter().map(|(item, count)| (item.item_type, *count)).collect();
        assert_eq!(shown, vec![(1_050_002, 3), (1_000_000, 1)]);
        let first = SellAllView::slot(0);
        let cursor = [(first.x + 5) as f32, (first.y + 5) as f32];
        assert_eq!(view.action_at(&inventory, cursor), Some(SellAllAction::Toggle(1_050_002)));
        assert!(view.toggle(&inventory, 1_050_002));
        assert_eq!(view.to_sell(&inventory).len(), 1);
    }

    #[test]
    fn worn_items_sell_for_less() {
        let mut worn = item(1, 410_105, 0);
        worn.durability = 5;
        assert_eq!(sale_price(900, &worn), 150);
        assert_eq!(format_silver(37_425), "37,425");
    }
}
