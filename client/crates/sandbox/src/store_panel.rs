//! The Store window: the categories on the left, their products in the
//! middle, and the chosen product on the right with its Buy button; the
//! CPs and VIP along the top. Drawn inside a quick panel's frame.

use crate::assets::Assets;
use crate::components::{draw_button, draw_card, draw_tab, WidgetState};
use crate::ui::UiRect;
use crate::{labels, ui_theme};
use fontdue::Font;
use gravak_protocol::{StoreProduct, StoreState};
use image::RgbaImage;

pub const CATEGORIES: [&str; 3] = ["VIP", "Consumables", "Upgrades"];

/// The catalog as the server sent it, and what is chosen on this screen.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct StoreView {
    pub products: Vec<StoreProduct>,
    pub state: StoreState,
    pub category: u32,
    /// The product chosen in the list.
    pub selected: Option<u32>,
}

impl StoreView {
    /// Adds a product as it arrives; index 0 starts the catalog over.
    pub fn receive(&mut self, product: StoreProduct) {
        if product.index == 0 {
            self.products.clear();
        }
        self.products.retain(|known| known.product_id != product.product_id);
        self.products.push(product);
    }

    pub fn in_category(&self) -> Vec<&StoreProduct> {
        self.products.iter().filter(|product| product.category == self.category).collect()
    }

    /// The product shown on the right: the chosen one, else the first.
    pub fn shown(&self) -> Option<&StoreProduct> {
        let products = self.in_category();
        self.selected
            .and_then(|id| products.iter().copied().find(|product| product.product_id == id))
            .or_else(|| products.first().copied())
    }

    pub fn can_buy(&self, product: &StoreProduct) -> bool {
        self.state.cps >= u64::from(product.price_cps) && (!product.limited() || product.stock_left > 0)
    }
}

/// What a click on the Store asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoreAction {
    Category(u32),
    Select(u32),
    Buy(u32),
}

const LIST_LEFT: i32 = 140;
const LIST_WIDTH: i32 = 180;
const ROW_HEIGHT: i32 = 54;
const LIST_TOP: i32 = 72;
const DETAIL_LEFT: i32 = 330;
const DETAIL_WIDTH: i32 = 170;
const BUY: UiRect = UiRect::new(DETAIL_LEFT + 20, 340, DETAIL_WIDTH - 40, 26);

fn category_tab(index: usize) -> UiRect {
    UiRect::new(20, LIST_TOP + index as i32 * 30, 110, 26)
}

fn product_row(row: usize) -> UiRect {
    UiRect::new(LIST_LEFT, LIST_TOP + row as i32 * (ROW_HEIGHT + 4), LIST_WIDTH, ROW_HEIGHT)
}

/// `1234567` as `1.234.567`.
fn thousands(value: u64) -> String {
    let digits = value.to_string();
    let mut out = String::new();
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index) % 3 == 0 {
            out.push('.');
        }
        out.push(digit);
    }
    out
}

/// What is left of VIP: `6d 3h`, `5h 12m`, or nothing.
pub fn vip_left(vip_until: u64, now: u64) -> Option<String> {
    let left = vip_until.checked_sub(now).filter(|left| *left > 0)?;
    let (days, hours, minutes) = (left / 86_400, left / 3_600 % 24, left / 60 % 60);
    Some(if days > 0 { format!("{days}d {hours}h") } else { format!("{hours}h {minutes}m") })
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

fn width_of(font: &Font, text: &str, size: f32) -> f32 {
    text.chars().map(|c| font.metrics(c, size).advance_width).sum()
}

fn say(image: &mut RgbaImage, font: &Font, text: &str, colour: [u8; 3], x: i32, y: i32, size: f32) {
    labels::text_sized(image, font, &[(text, [10, 18, 22])], [x as f32 + 1.0, y as f32 + 1.0], size);
    labels::text_sized(image, font, &[(text, colour)], [x as f32, y as f32], size);
}

/// A small gold coin for the CP balance.
fn draw_coin(image: &mut RgbaImage, centre: [i32; 2]) {
    for dy in -7_i32..=7 {
        for dx in -7_i32..=7 {
            let distance = dx * dx + dy * dy;
            let colour = if distance > 49 {
                continue;
            } else if distance > 30 {
                [150, 100, 20, 255]
            } else {
                [236, 186, 64, 255]
            };
            image.put_pixel((centre[0] + dx) as u32, (centre[1] + dy) as u32, image::Rgba(colour));
        }
    }
}

/// Words of `text` in lines no wider than `width`.
fn wrap(font: &Font, text: &str, size: f32, width: f32) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        let candidate = if line.is_empty() { word.to_owned() } else { format!("{line} {word}") };
        if width_of(font, &candidate, size) > width && !line.is_empty() {
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

/// A product's picture: its item icon, or a gold VIP badge.
fn icon(assets: &mut Assets, product: &StoreProduct, size: u32) -> Option<RgbaImage> {
    if product.item_type > 0 {
        let icon = crate::shop::item_icon_image(assets, product.item_type).ok()?;
        return Some(image::imageops::resize(&icon, size, size, image::imageops::FilterType::Triangle));
    }
    let mut badge = RgbaImage::from_pixel(size, size, image::Rgba([40, 28, 8, 255]));
    let s = size as i32;
    crate::components::outline(&mut badge, UiRect::new(0, 0, s, s), [255, 202, 64, 255]);
    crate::components::outline(&mut badge, UiRect::new(2, 2, s - 4, s - 4), [150, 110, 40, 255]);
    let font = labels::decorative_font().ok()?;
    let text_size = size as f32 * 0.36;
    let x = (size as f32 - width_of(&font, "VIP", text_size)) / 2.0;
    labels::text_sized(&mut badge, &font, &[("VIP", [255, 214, 96])], [x, (size as f32 - text_size) / 2.0 - 1.0], text_size);
    Some(badge)
}

/// Size of the Store button by the HUD.
pub const BUTTON_SIZE: [u32; 2] = crate::ornate::SIZE;

/// The Store button: an ornate gold-framed plate with the Gravak medallion.
pub fn button_image(assets: &mut Assets, hovered: bool) -> crate::assets::Result<RgbaImage> {
    crate::ornate::button(assets, "Store", crate::ornate::GRAVAK, hovered, Some("G"))
}

/// Draws the Store onto the quick panel's contents.
pub fn draw(image: &mut RgbaImage, assets: &mut Assets, view: Option<&StoreView>) {
    let (Ok(ui), Ok(cinzel)) = (labels::font(), labels::decorative_font()) else {
        return;
    };
    let Some(view) = view.filter(|view| !view.products.is_empty()) else {
        say(image, &ui, "Opening the Store...", ui_theme::TEXT_MUTED, 24, 56, 14.0);
        return;
    };

    // The CPs and VIP.
    draw_coin(image, [30, 54]);
    say(image, &ui, &format!("{} CPs", thousands(view.state.cps)), ui_theme::TEXT_GOLD, 42, 46, 14.0);
    let vip = vip_left(view.state.vip_until, now_secs())
        .map_or_else(|| "No VIP".to_owned(), |left| format!("VIP: {left} left"));
    let vip_colour = if view.state.vip_until > now_secs() { [255, 202, 64] } else { ui_theme::TEXT_MUTED };
    let vip_x = 500 - width_of(&ui, &vip, 14.0) as i32;
    say(image, &ui, &vip, vip_colour, vip_x, 46, 14.0);

    for (index, name) in CATEGORIES.iter().enumerate() {
        draw_tab(image, &ui, category_tab(index), name, index as u32 == view.category, false);
    }

    let shown = view.shown().map(|product| product.product_id);
    for (row, product) in view.in_category().into_iter().enumerate().take(5) {
        let rect = product_row(row);
        let state = if Some(product.product_id) == shown { WidgetState::Selected } else { WidgetState::Normal };
        draw_card(image, rect, state);
        if let Some(icon) = icon(assets, product, 40) {
            image::imageops::overlay(image, &icon, i64::from(rect.x + 6), i64::from(rect.y + 7));
        }
        let text_left = rect.x + 52;
        let name_lines = wrap(&ui, &product.name, 12.0, (rect.width - 58) as f32);
        for (line, text) in name_lines.iter().take(2).enumerate() {
            say(image, &ui, text, ui_theme::TEXT, text_left, rect.y + 5 + line as i32 * 14, 12.0);
        }
        say(
            image,
            &ui,
            &format!("{} CPs", thousands(u64::from(product.price_cps))),
            ui_theme::TEXT_GOLD,
            text_left,
            rect.y + 36,
            12.0,
        );
        if product.limited() {
            let stock = format!("{} left", product.stock_left);
            let colour = if product.stock_left == 0 { [214, 91, 68] } else { ui_theme::TEXT_MUTED };
            say(image, &ui, &stock, colour, rect.x + rect.width - 8 - width_of(&ui, &stock, 11.0) as i32, rect.y + 37, 11.0);
        }
    }

    let Some(product) = view.shown() else {
        return;
    };
    draw_card(image, UiRect::new(DETAIL_LEFT, LIST_TOP, DETAIL_WIDTH, 302), WidgetState::Normal);
    let title_lines = wrap(&cinzel, &product.name, 14.0, (DETAIL_WIDTH - 16) as f32);
    for (line, text) in title_lines.iter().take(2).enumerate() {
        let x = DETAIL_LEFT + (DETAIL_WIDTH - width_of(&cinzel, text, 14.0) as i32) / 2;
        say(image, &cinzel, text, ui_theme::TEXT_GOLD, x, LIST_TOP + 8 + line as i32 * 17, 14.0);
    }
    if let Some(icon) = icon(assets, product, 64) {
        let x = DETAIL_LEFT + (DETAIL_WIDTH - 64) / 2;
        image::imageops::overlay(image, &icon, i64::from(x), i64::from(LIST_TOP + 46));
    }
    let mut y = LIST_TOP + 118;
    let price = format!("{} CPs", thousands(u64::from(product.price_cps)));
    say(image, &ui, &price, ui_theme::TEXT_GOLD, DETAIL_LEFT + (DETAIL_WIDTH - width_of(&ui, &price, 14.0) as i32) / 2, y, 14.0);
    y += 22;
    if product.limited() {
        let stock = if product.stock_left == 0 {
            "Sold out today; restocks at midnight.".to_owned()
        } else {
            format!("{} left today.", product.stock_left)
        };
        say(image, &ui, &stock, ui_theme::TEXT_MUTED, DETAIL_LEFT + 10, y, 11.0);
        y += 16;
    }
    for line in wrap(&ui, &product.description, 11.0, (DETAIL_WIDTH - 20) as f32).iter().take(8) {
        say(image, &ui, line, ui_theme::TEXT, DETAIL_LEFT + 10, y, 11.0);
        y += 14;
    }
    let state = if view.can_buy(product) { WidgetState::Normal } else { WidgetState::Disabled };
    draw_button(image, &ui, BUY, "Buy", state);
}

/// The action under `cursor`, in the panel's own coordinates.
pub fn action_at(view: &StoreView, cursor: [f32; 2]) -> Option<StoreAction> {
    if let Some(index) = (0..CATEGORIES.len()).find(|index| category_tab(*index).contains_local(cursor)) {
        return Some(StoreAction::Category(index as u32));
    }
    let products = view.in_category();
    if let Some(row) = (0..products.len().min(5)).find(|row| product_row(*row).contains_local(cursor)) {
        return Some(StoreAction::Select(products[row].product_id));
    }
    let product = view.shown()?;
    (BUY.contains_local(cursor) && view.can_buy(product)).then_some(StoreAction::Buy(product.product_id))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn product(id: u32, category: u32, price: u32) -> StoreProduct {
        StoreProduct {
            product_id: id,
            category,
            price_cps: price,
            stock_left: u32::MAX,
            name: format!("Product {id}"),
            ..Default::default()
        }
    }

    fn centre(rect: UiRect) -> [f32; 2] {
        [rect.x as f32 + rect.width as f32 / 2.0, rect.y as f32 + rect.height as f32 / 2.0]
    }

    #[test]
    fn the_store_takes_clicks() {
        let mut view = StoreView::default();
        for (index, product) in [product(1, 0, 500), product(20, 2, 13), product(22, 2, 215)].into_iter().enumerate() {
            view.receive(StoreProduct { index: index as u32, ..product });
        }
        view.state.cps = 100;
        assert_eq!(action_at(&view, centre(category_tab(2))), Some(StoreAction::Category(2)));
        assert_eq!(view.shown().map(|product| product.product_id), Some(1));
        assert_eq!(action_at(&view, centre(BUY)), None, "500 CPs is too much");

        view.category = 2;
        assert_eq!(action_at(&view, centre(product_row(1))), Some(StoreAction::Select(22)));
        assert_eq!(action_at(&view, centre(BUY)), Some(StoreAction::Buy(20)), "the first is shown");
        view.selected = Some(22);
        assert_eq!(action_at(&view, centre(BUY)), None, "215 CPs is too much");

        // A new catalog starts over.
        view.receive(StoreProduct { index: 0, ..product(30, 1, 50) });
        assert_eq!(view.products.len(), 1);
    }

    #[test]
    fn vip_time_reads_shortly() {
        assert_eq!(vip_left(0, 100), None);
        assert_eq!(vip_left(100 + 6 * 86_400 + 3 * 3_600, 100).as_deref(), Some("6d 3h"));
        assert_eq!(vip_left(100 + 5 * 3_600 + 12 * 60, 100).as_deref(), Some("5h 12m"));
    }
}

#[cfg(test)]
mod preview {
    use super::*;

    /// `BOUNTY_PREVIEW=dir cargo test -p conquer-sandbox store_preview`
    #[test]
    fn store_preview() {
        let Ok(dir) = std::env::var("BOUNTY_PREVIEW") else { return };
        let mut assets = Assets::open(std::path::Path::new("R:/5065")).expect("assets");
        let products = [
            (1, 0, 0, 7, 500, u32::MAX, "7 Days of VIP", "VIP for your whole account: +10% experience from monsters, silver, Meteors and DragonBalls from your kills go straight to your bag, and the Conductresses carry you for free."),
            (2, 0, 0, 30, 1_800, u32::MAX, "30 Days of VIP", "VIP for your whole account."),
            (10, 1, 723_017, 0, 50, u32::MAX, "Exp Potion", "Doubles the experience you gain for one hour."),
            (11, 1, 723_700, 0, 200, 7, "Exp Ball", "Grants the experience of hours of training at once."),
            (12, 1, 1_200_000, 0, 300, u32::MAX, "Heaven's Blessing (3 days)", "Offline training, and revive where you fall, for three days."),
            (20, 2, 1_088_001, 0, 13, u32::MAX, "Meteor", "Raises the level of an item with a chance of success."),
            (21, 2, 720_027, 0, 125, u32::MAX, "Meteor Scroll", "Opens into ten Meteors."),
            (22, 2, 1_088_000, 0, 215, u32::MAX, "DragonBall", "Raises the quality of an item, among many other uses."),
            (23, 2, 720_028, 0, 2_100, u32::MAX, "DragonBall Scroll", "Opens into ten DragonBalls."),
        ];
        let mut view = StoreView::default();
        for (index, (id, category, item_type, vip_days, price, stock, name, description)) in products.into_iter().enumerate() {
            view.receive(StoreProduct {
                index: index as u32,
                total: 9,
                product_id: id,
                category,
                item_type,
                amount: 1,
                vip_days,
                price_cps: price,
                stock_left: stock,
                name: name.into(),
                description: description.into(),
            });
        }
        view.state.cps = 1_250;
        view.state.vip_until = now_secs() + 6 * 86_400 + 3 * 3_600;
        for (name, category, selected) in [("vip", 0, None), ("consumables", 1, Some(11)), ("upgrades", 2, Some(22))] {
            view.category = category;
            view.selected = selected;
            let mut content = RgbaImage::new(520, 420);
            draw(&mut content, &mut assets, Some(&view));
            let mut image = crate::original_panels::framed_window(&mut assets, 520, 420).expect("frame");
            image::imageops::overlay(&mut image, &content, 0, 0);
            image.save(format!("{dir}/store-{name}.png")).expect("png");
        }
        button_image(&mut assets, false).expect("button").save(format!("{dir}/store-button.png")).expect("png");
        button_image(&mut assets, true).expect("button").save(format!("{dir}/store-button-hover.png")).expect("png");
        crate::shop::PromptDialog::new([1024.0, 768.0], "Store", "Meteor - 13 CPs (1237 left)".to_owned(), "Buy this item?")
            .with_buttons(["Buy", "Cancel"])
            .image(&mut assets, Some(true))
            .expect("prompt")
            .save(format!("{dir}/store-confirm.png"))
            .expect("png");
    }
}
