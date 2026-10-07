//! Booths (player shops in the Market) on their original 5065 windows:
//! the own booth ([0-233], `DialogMpcBooth.dds`) with a Remove button under
//! each item and Hawk, someone else's ([0-234], the same art) with Buy, the
//! price box an item dropped on the own booth asks for ([0-158],
//! `DialogConfirm.dds`, silver or CPs) and the hawk line ([0-235],
//! `DialogHawk.dds`), which the booth shows in a bubble over its seller.
//!
//! One image holds the booth, the price box over it and the hawk box below
//! it, where the original puts them; the same layout paints and hit-tests.

use crate::assets::{Assets, Result};
use crate::network::{OwnedItem, PlayerShopItem};
use image::{RgbaImage, imageops};

/// Items a booth page shows.
pub const PAGE: usize = 6;
/// The longest hawk line (the server's limit).
pub const HAWK_MAX: usize = 60;

const BOOTH: [i32; 2] = [455, 263];
/// [0-235] stands 329 below the booth ([0-233] y=70, [0-235] y=399).
const HAWK_TOP: i32 = 329;
const HAWK: [i32; 2] = [484, 130];
/// The image: the booth with the hawk box below it.
pub const SIZE: [u32; 2] = [484, (HAWK_TOP + HAWK[1]) as u32];

/// [233-1304..1309] the item boxes.
const GOODS: [[i32; 2]; PAGE] = [[32, 54], [223, 54], [32, 111], [223, 110], [32, 168], [223, 166]];
const GOOD: i32 = 39;
/// [233-1455..1460] the coin under the price.
const COINS: [[i32; 2]; PAGE] = [[75, 50], [270, 50], [75, 106], [270, 106], [75, 162], [270, 162]];
/// [233-1310..1315] the price.
const PRICES: [[i32; 2]; PAGE] = [[122, 52], [317, 52], [122, 110], [317, 110], [122, 168], [317, 168]];
/// [233-1558..] Remove, [234-1597..] Buy.
const BUTTONS: [[i32; 2]; PAGE] = [[133, 73], [328, 73], [133, 129], [328, 129], [133, 185], [328, 185]];
const BUTTON: [i32; 2] = [62, 20];
const CLOSE: [i32; 4] = [412, 22, 14, 13];
const CLOSE_BOTTOM: [i32; 4] = [335, 229, 42, 16];
const HAWK_BUTTON: [i32; 4] = [54, 230, 62, 20];
/// Where the original has Help ([233-1667]): Leave, which takes the own
/// booth down (Close only hides the window).
const LEAVE: [i32; 4] = [208, 229, 42, 16];
const PAGE_UP: [i32; 4] = [420, 167, 17, 17];
const PAGE_DOWN: [i32; 4] = [420, 204, 17, 17];

/// The price box ([0-158]), centred over the booth.
const PRICE_BOX: [i32; 2] = [82, 77];
const PRICE_SIZE: [i32; 2] = [290, 108];
const PRICE_SILVER: [i32; 4] = [52, 40, 84, 18];
const PRICE_CPS: [i32; 4] = [185, 40, 84, 18];
const PRICE_OK: [i32; 4] = [145, 67, 36, 19];
const PRICE_CANCEL: [i32; 4] = [215, 67, 36, 19];

/// [235-1323] close, [235-1613] the line, [235-1324] agree.
const HAWK_CLOSE: [i32; 4] = [440, 20, 16, 16];
const HAWK_FIELD: [i32; 4] = [35, 58, 410, 23];
const HAWK_AGREE: [i32; 4] = [400, 100, 70, 20];

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum PriceField {
    #[default]
    Silver,
    Cps,
}

/// The booth windows' state.
#[derive(Debug, Clone, Default, Hash)]
pub struct BoothView {
    /// The own booth is open (the hero sits by a flag).
    pub open: bool,
    /// Listings of the own booth and of the one being looked at.
    pub listings: Vec<PlayerShopItem>,
    /// Whose booth the window shows, with their name: `None` for the own.
    pub viewing: Option<(u32, String)>,
    pub page: usize,
    /// The item dropped on the own booth, waiting for its price.
    pub pricing: Option<OwnedItem>,
    pub silver_input: String,
    pub cps_input: String,
    pub price_field: PriceField,
    /// The line the own booth hawks.
    pub hawk: String,
    /// The hawk box is open.
    pub hawk_editing: bool,
}

impl BoothView {
    /// The seller whose items the window shows.
    pub fn owner(&self, own_id: Option<u32>) -> Option<u32> {
        self.viewing.as_ref().map(|(id, _)| *id).or(own_id)
    }

    /// The listings the window shows, in order.
    pub fn shown(&self, own_id: Option<u32>) -> Vec<PlayerShopItem> {
        let owner = self.owner(own_id);
        self.listings
            .iter()
            .filter(|listing| Some(listing.owner_id) == owner)
            .copied()
            .collect()
    }

    pub fn pages(&self, own_id: Option<u32>) -> usize {
        self.shown(own_id).len().div_ceil(PAGE).max(1)
    }

    /// The listing in box `slot` of the page shown.
    pub fn at_slot(&self, own_id: Option<u32>, slot: usize) -> Option<PlayerShopItem> {
        self.shown(own_id).get(self.page * PAGE + slot).copied()
    }

    /// Opens the price box for `item`.
    pub fn ask_price(&mut self, item: OwnedItem) {
        self.pricing = Some(item);
        self.silver_input.clear();
        self.cps_input.clear();
        self.price_field = PriceField::Silver;
    }

    /// A digit typed in the price box: one currency at a time, so typing
    /// in a box empties the other.
    pub fn type_digit(&mut self, digit: char) {
        let (field, other) = match self.price_field {
            PriceField::Silver => (&mut self.silver_input, &mut self.cps_input),
            PriceField::Cps => (&mut self.cps_input, &mut self.silver_input),
        };
        if field.len() < 10 && (digit != '0' || !field.is_empty()) {
            field.push(digit);
            other.clear();
        }
    }

    pub fn erase_digit(&mut self) {
        match self.price_field {
            PriceField::Silver => self.silver_input.pop(),
            PriceField::Cps => self.cps_input.pop(),
        };
    }

    /// The price typed: amount and whether in CPs.
    pub fn price(&self) -> Option<(u32, bool)> {
        let parse = |text: &str| text.parse::<u32>().ok().filter(|price| *price > 0);
        parse(&self.silver_input)
            .map(|price| (price, false))
            .or_else(|| parse(&self.cps_input).map(|price| (price, true)))
    }

    pub fn type_hawk(&mut self, text: &str) {
        for ch in text.chars().filter(|ch| !ch.is_control()) {
            if self.hawk.chars().count() < HAWK_MAX {
                self.hawk.push(ch);
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoothAction {
    /// Inside a window, on nothing.
    Nothing,
    /// Hides the window; the own booth stays open.
    Close,
    /// Takes the own booth down.
    Leave,
    Hawk,
    PageUp,
    PageDown,
    /// Takes back the listing in this box of the page.
    Remove(usize),
    Buy(usize),
    PriceField(PriceField),
    PriceOk,
    PriceCancel,
    HawkOk,
    HawkClose,
}

/// Where the image goes: the left edge, the booth centred as the other
/// left-hand windows, the hawk box kept on screen.
pub fn position(viewport: [f32; 2]) -> [f32; 2] {
    let centred = ((viewport[1] - BOOTH[1] as f32) * 0.5).max(8.0);
    [0.0, centred.min((viewport[1] - SIZE[1] as f32).max(8.0))]
}

fn local(viewport: [f32; 2], cursor: [f32; 2]) -> [i32; 2] {
    let [left, top] = position(viewport);
    [(cursor[0] - left).floor() as i32, (cursor[1] - top).floor() as i32]
}

fn inside([x, y, width, height]: [i32; 4], [px, py]: [i32; 2]) -> bool {
    px >= x && px < x + width && py >= y && py < y + height
}

fn offset([x, y, width, height]: [i32; 4], [dx, dy]: [i32; 2]) -> [i32; 4] {
    [x + dx, y + dy, width, height]
}

fn hawk_rect(rect: [i32; 4]) -> [i32; 4] {
    offset(rect, [0, HAWK_TOP])
}

fn price_rect(rect: [i32; 4]) -> [i32; 4] {
    offset(rect, PRICE_BOX)
}

fn good_rect(slot: usize) -> [i32; 4] {
    let [x, y] = GOODS[slot];
    [x, y, GOOD, GOOD]
}

fn button_rect(slot: usize) -> [i32; 4] {
    let [x, y] = BUTTONS[slot];
    [x, y, BUTTON[0], BUTTON[1]]
}

/// Whether the cursor is on one of the windows shown.
pub fn window_at(viewport: [f32; 2], cursor: [f32; 2], view: &BoothView) -> bool {
    let point = local(viewport, cursor);
    inside([0, 0, BOOTH[0], BOOTH[1]], point)
        || (view.hawk_editing && inside([0, HAWK_TOP, HAWK[0], HAWK[1]], point))
}

/// What a click at `cursor` does; `None` outside the windows.
pub fn action_at(viewport: [f32; 2], cursor: [f32; 2], view: &BoothView, own_id: Option<u32>) -> Option<BoothAction> {
    use BoothAction as A;
    let point = local(viewport, cursor);
    if view.hawk_editing && inside([0, HAWK_TOP, HAWK[0], HAWK[1]], point) {
        return Some(if inside(hawk_rect(HAWK_CLOSE), point) {
            A::HawkClose
        } else if inside(hawk_rect(HAWK_AGREE), point) {
            A::HawkOk
        } else {
            A::Nothing
        });
    }
    if !inside([0, 0, BOOTH[0], BOOTH[1]], point) {
        return None;
    }
    // The price box holds the window until it is answered.
    if view.pricing.is_some() {
        return Some(if inside(price_rect(PRICE_SILVER), point) {
            A::PriceField(PriceField::Silver)
        } else if inside(price_rect(PRICE_CPS), point) {
            A::PriceField(PriceField::Cps)
        } else if inside(price_rect(PRICE_OK), point) {
            A::PriceOk
        } else if inside(price_rect(PRICE_CANCEL), point) {
            A::PriceCancel
        } else {
            A::Nothing
        });
    }
    if inside(CLOSE, point) || inside(CLOSE_BOTTOM, point) {
        return Some(A::Close);
    }
    if view.viewing.is_none() && view.open && inside(HAWK_BUTTON, point) {
        return Some(A::Hawk);
    }
    if view.viewing.is_none() && view.open && inside(LEAVE, point) {
        return Some(A::Leave);
    }
    if inside(PAGE_UP, point) {
        return Some(A::PageUp);
    }
    if inside(PAGE_DOWN, point) {
        return Some(A::PageDown);
    }
    for slot in 0..PAGE {
        if inside(button_rect(slot), point) && view.at_slot(own_id, slot).is_some() {
            return Some(if view.viewing.is_some() { A::Buy(slot) } else { A::Remove(slot) });
        }
    }
    Some(A::Nothing)
}

/// The listing under the cursor, for its tooltip.
pub fn item_at(viewport: [f32; 2], cursor: [f32; 2], view: &BoothView, own_id: Option<u32>) -> Option<PlayerShopItem> {
    if view.pricing.is_some() {
        return None;
    }
    let point = local(viewport, cursor);
    (0..PAGE)
        .find(|slot| inside(good_rect(*slot), point))
        .and_then(|slot| view.at_slot(own_id, slot))
}

const WHITE: [u8; 4] = [255, 255, 255, 255];
const GREY: [u8; 4] = [170, 170, 170, 255];
const GOLD: [u8; 4] = [232, 206, 148, 255];

fn text(image: &mut RgbaImage, value: &str, [x, y]: [i32; 2], colour: [u8; 4], width: i32) {
    if let Ok(font) = crate::main_hud::font() {
        crate::main_hud::text(image, font, value, 12.0, [x, y], colour, Some(width));
    }
}

/// A gold label centred in `rect`, as the original's text buttons.
fn label(image: &mut RgbaImage, value: &str, [x, y, width, height]: [i32; 4]) -> Result<()> {
    let font = crate::main_hud::label_font()?;
    let text_width = crate::main_hud::text_width(font, value, 13.0);
    crate::main_hud::crisp_text(image, font, value, 13.0, [x + (width - text_width) / 2, y + (height - 15) / 2], GOLD);
    Ok(())
}

fn art(image: &mut RgbaImage, assets: &mut Assets, path: &str, [x, y]: [i32; 2]) -> Result<()> {
    crate::original_ui::overlay_asset(image, assets, path, x.into(), y.into())
}

fn coin(cps: bool) -> &'static str {
    if cps { "data/main/PublicBtnMoney.dds" } else { "data/main/MpcGoodsMoneyBtn.dds" }
}

pub fn image(assets: &mut Assets, view: &BoothView, own_id: Option<u32>) -> Result<RgbaImage> {
    let mut image = RgbaImage::new(SIZE[0], SIZE[1]);
    art(&mut image, assets, "data/main/DialogMpcBooth.dds", [0, 0])?;
    art(&mut image, assets, "data/main/CloseBtnS.dds", [CLOSE[0] - 1, CLOSE[1] - 1])?;
    label(&mut image, "Close", CLOSE_BOTTOM)?;
    let own = view.viewing.is_none();
    let shown = view.shown(own_id);
    for (slot, listing) in shown.iter().skip(view.page * PAGE).take(PAGE).enumerate() {
        let [x, y] = GOODS[slot];
        if let Ok(icon) = crate::shop::item_icon_native(assets, listing.item.item_type) {
            let size = crate::original_ui_layout::INVENTORY_ICON_SIZE as u32;
            let icon = imageops::resize(&icon, size, size, imageops::FilterType::Triangle);
            imageops::overlay(&mut image, &icon, (x + 1).into(), (y + 1).into());
        }
        if listing.item.plus > 0 {
            text(&mut image, &format!("+{}", listing.item.plus), [x + 3, y + 24], WHITE, 30);
        }
        art(&mut image, assets, coin(listing.cps), COINS[slot])?;
        let [px, py] = PRICES[slot];
        text(&mut image, &crate::store_panel::thousands(u64::from(listing.price)), [px + 6, py + 2], WHITE, 80);
        let button = if own { "data/main/MpcBoothBtnGetBack.dds" } else { "data/main/NpcBoothBtnBuy.dds" };
        art(&mut image, assets, button, BUTTONS[slot])?;
    }
    let pages = view.pages(own_id);
    if pages > 1 {
        art(&mut image, assets, "data/main/FriendBtnUp.dds", [PAGE_UP[0], PAGE_UP[1]])?;
        art(&mut image, assets, "data/main/FriendBtnDown.dds", [PAGE_DOWN[0], PAGE_DOWN[1]])?;
        text(&mut image, &format!("{}/{pages}", view.page + 1), [418, 188], GREY, 30);
    }
    match &view.viewing {
        None if view.open => {
            art(&mut image, assets, "data/main/MpcBoothBtnHawk.dds", [HAWK_BUTTON[0], HAWK_BUTTON[1]])?;
            label(&mut image, "Leave", LEAVE)?;
            if shown.is_empty() {
                text(&mut image, "Drag items from your inventory here to sell them.", [60, 120], GREY, 340);
            }
        },
        None => {
            text(&mut image, "Click a flag in the Market to set up your booth.", [70, 120], GREY, 340);
        },
        Some((_, name)) => {
            text(&mut image, &format!("{name}'s booth"), [24, 232], GOLD, 170);
            if shown.is_empty() {
                text(&mut image, "Nothing for sale.", [170, 120], GREY, 200);
            }
        },
    }
    if let Some(item) = view.pricing {
        draw_price_box(&mut image, assets, view, item)?;
    }
    if view.hawk_editing {
        draw_hawk_box(&mut image, assets, view)?;
    }
    Ok(image)
}

fn draw_price_box(image: &mut RgbaImage, assets: &mut Assets, view: &BoothView, item: OwnedItem) -> Result<()> {
    let [left, top] = PRICE_BOX;
    // The art is half see-through: darken the booth under it first.
    crate::original_panels::fill(image, [left, top, PRICE_SIZE[0], PRICE_SIZE[1]], [0, 0, 0, 150]);
    art(image, assets, "data/main/DialogConfirm.dds", PRICE_BOX)?;
    let name = crate::shop::item_info(assets, item.item_type).map_or_else(|| "this item".to_owned(), |info| info.name);
    let line = format!("Sell {name} for:");
    if let Ok(font) = crate::main_hud::font() {
        let width = crate::main_hud::text_width(font, &line, 12.0).min(275);
        crate::main_hud::text(image, font, &line, 12.0, [left + 5 + (275 - width) / 2, top + 11], WHITE, Some(275));
    }
    for (field, rect, cps) in [(PriceField::Silver, PRICE_SILVER, false), (PriceField::Cps, PRICE_CPS, true)] {
        let [x, y, width, height] = price_rect(rect);
        art(image, assets, coin(cps), [x - 44, y])?;
        art(image, assets, "data/main/ConfirmEdt.dds", [x, y])?;
        let typed = if cps { &view.cps_input } else { &view.silver_input };
        let focused = view.price_field == field;
        let shown = if focused { format!("{typed}_") } else { typed.clone() };
        text(image, &shown, [x + 4, y + 2], WHITE, width - 6);
        if focused {
            crate::original_panels::outline_rect(image, [x, y, width, height], [255, 215, 108, 255]);
        }
    }
    art(image, assets, "data/main/ConfirmOkBtn.dds", [left + PRICE_OK[0], top + PRICE_OK[1]])?;
    art(image, assets, "data/main/ConfirmCancelBtn.dds", [left + PRICE_CANCEL[0], top + PRICE_CANCEL[1]])?;
    Ok(())
}

fn draw_hawk_box(image: &mut RgbaImage, assets: &mut Assets, view: &BoothView) -> Result<()> {
    art(image, assets, "data/main/DialogHawk.dds", [0, HAWK_TOP])?;
    let [x, y] = [HAWK_CLOSE[0], HAWK_CLOSE[1] + HAWK_TOP];
    art(image, assets, "data/main/CloseBtnS.dds", [x, y])?;
    let [fx, fy, width, _] = hawk_rect(HAWK_FIELD);
    text(image, &format!("{}_", view.hawk), [fx + 6, fy + 5], WHITE, width - 10);
    let [ax, ay, _, _] = hawk_rect(HAWK_AGREE);
    art(image, assets, "data/main/ConfirmOkBtn.dds", [ax + 17, ay])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn listing(owner_id: u32, uid: u32) -> PlayerShopItem {
        PlayerShopItem {
            owner_id,
            price: 100,
            cps: false,
            item: OwnedItem { uid, ..OwnedItem::default() },
        }
    }

    fn at(rect: [i32; 4]) -> [f32; 2] {
        let [left, top] = position(VIEWPORT);
        [left + rect[0] as f32 + rect[2] as f32 * 0.5, top + rect[1] as f32 + rect[3] as f32 * 0.5]
    }

    const VIEWPORT: [f32; 2] = [1366.0, 768.0];

    #[test]
    fn own_booth_removes_and_others_buy() {
        let mut view = BoothView {
            open: true,
            listings: vec![listing(1, 10), listing(2, 20), listing(1, 11)],
            ..BoothView::default()
        };
        assert_eq!(view.shown(Some(1)).len(), 2);
        assert_eq!(action_at(VIEWPORT, at(button_rect(1)), &view, Some(1)), Some(BoothAction::Remove(1)));
        // An empty box has no button.
        assert_eq!(action_at(VIEWPORT, at(button_rect(2)), &view, Some(1)), Some(BoothAction::Nothing));
        assert_eq!(action_at(VIEWPORT, at(HAWK_BUTTON), &view, Some(1)), Some(BoothAction::Hawk));
        assert_eq!(action_at(VIEWPORT, at(LEAVE), &view, Some(1)), Some(BoothAction::Leave));
        view.viewing = Some((2, "Sheko".into()));
        assert_eq!(action_at(VIEWPORT, at(button_rect(0)), &view, Some(1)), Some(BoothAction::Buy(0)));
        assert_eq!(view.at_slot(Some(1), 0).map(|listing| listing.item.uid), Some(20));
        assert_eq!(action_at(VIEWPORT, at(HAWK_BUTTON), &view, Some(1)), Some(BoothAction::Nothing));
        assert_eq!(action_at(VIEWPORT, at(CLOSE_BOTTOM), &view, Some(1)), Some(BoothAction::Close));
        assert_eq!(action_at(VIEWPORT, [900.0, 700.0], &view, Some(1)), None);
    }

    #[test]
    fn the_price_box_takes_one_currency() {
        let mut view = BoothView::default();
        view.ask_price(OwnedItem::default());
        for digit in "0150".chars() {
            view.type_digit(digit);
        }
        assert_eq!(view.price(), Some((150, false)));
        view.price_field = PriceField::Cps;
        view.type_digit('7');
        assert!(view.silver_input.is_empty());
        assert_eq!(view.price(), Some((7, true)));
        // While it is open only its controls answer.
        assert_eq!(action_at(VIEWPORT, at(price_rect(PRICE_OK)), &view, None), Some(BoothAction::PriceOk));
        assert_eq!(
            action_at(VIEWPORT, at(price_rect(PRICE_SILVER)), &view, None),
            Some(BoothAction::PriceField(PriceField::Silver))
        );
        assert_eq!(action_at(VIEWPORT, at(CLOSE_BOTTOM), &view, None), Some(BoothAction::Nothing));
    }

    #[test]
    fn the_hawk_box_sits_below_the_booth() {
        let mut view = BoothView::default();
        let agree = at(hawk_rect(HAWK_AGREE));
        assert!(!window_at(VIEWPORT, agree, &view));
        view.hawk_editing = true;
        assert!(window_at(VIEWPORT, agree, &view));
        assert_eq!(action_at(VIEWPORT, agree, &view, None), Some(BoothAction::HawkOk));
        view.type_hawk(&"x".repeat(80));
        assert_eq!(view.hawk.chars().count(), HAWK_MAX);
        // On a 600 pixel high window the hawk box still fits.
        assert!(position([800.0, 600.0])[1] + SIZE[1] as f32 <= 600.0);
    }

    #[test]
    fn pages_hold_six_items() {
        let view = BoothView {
            listings: (0..10).map(|uid| listing(1, uid)).collect(),
            page: 1,
            ..BoothView::default()
        };
        assert_eq!(view.pages(Some(1)), 2);
        assert_eq!(view.at_slot(Some(1), 3).map(|listing| listing.item.uid), Some(9));
        assert_eq!(view.at_slot(Some(1), 4), None);
    }

    /// `BOOTH_PREVIEW=dir cargo test -p conquer-sandbox booth_preview`
    #[test]
    fn booth_preview() {
        let Ok(dir) = std::env::var("BOOTH_PREVIEW") else { return };
        let mut assets = Assets::open(std::path::Path::new("R:/5065")).expect("assets");
        let item = |uid, item_type, price, cps| PlayerShopItem {
            owner_id: 1,
            price,
            cps,
            item: OwnedItem { uid, item_type, plus: (uid % 3) as u8, ..OwnedItem::default() },
        };
        let mut view = BoothView {
            open: true,
            listings: vec![
                item(1, 410_077, 25_000, false),
                item(2, 1_088_000, 215, true),
                item(3, 130_037, 9_800, false),
                item(4, 1_088_001, 18, true),
                item(5, 500_067, 4_000, false),
                item(6, 723_700, 120_000, false),
                item(7, 700_011, 50_000, false),
            ],
            hawk: "Cheap gear! DragonBalls for CPs".into(),
            ..BoothView::default()
        };
        image(&mut assets, &view, Some(1)).expect("own").save(format!("{dir}/booth-own.png")).expect("png");
        view.ask_price(OwnedItem { item_type: 410_077, ..OwnedItem::default() });
        view.type_digit('3');
        view.type_digit('0');
        view.hawk_editing = true;
        image(&mut assets, &view, Some(1)).expect("price").save(format!("{dir}/booth-price.png")).expect("png");
        view.pricing = None;
        view.hawk_editing = false;
        view.viewing = Some((1, "Sheko".into()));
        image(&mut assets, &view, Some(2)).expect("other").save(format!("{dir}/booth-other.png")).expect("png");
    }
}
