use crate::{
    assets::{Assets, Result},
    labels,
};
use image::{imageops, Rgba, RgbaImage};
use std::collections::{HashMap, HashSet};

const PANEL_WIDTH: u32 = 270;
const SHOP_HEIGHT: u32 = 250;
const INVENTORY_HEIGHT: u32 = 512;
const SLOT_X: u32 = 25;
const SLOT_Y: u32 = 28;
const SLOT_STEP_X: u32 = 48;
const SLOT_STEP_Y: u32 = 43;
const SLOT_SIZE: u32 = 36;

#[derive(Debug, Clone, Default)]
pub struct ItemInfo {
    pub name: String,
    pub level: u32,
    pub attack_min: u32,
    pub attack_max: u32,
    pub defense: u32,
    pub price: u32,
    pub item_type: String,
}

#[derive(Debug, Clone)]
pub struct ShopState {
    pub npc_id: u32,
    pub name: String,
    pub items: Vec<u32>,
    icons: Vec<RgbaImage>,
    infos: HashMap<u32, ItemInfo>,
    pub hover: Option<usize>,
    pub selected: Option<usize>,
}

impl ShopState {
    pub fn load(assets: &mut Assets, npc_id: u32) -> Result<Option<Self>> {
        let contents = String::from_utf8_lossy(&assets.read("ini/Shop.dat")?).into_owned();
        let mut current: Option<(u32, String, Vec<u32>)> = None;
        let mut found = None;
        for raw_line in contents.lines() {
            let line = raw_line.trim();
            if line.starts_with('[') && line.ends_with(']') {
                if let Some((id, name, items)) = current.take()
                    && id == npc_id
                {
                    found = Some((id, name, items));
                }
                if line.starts_with("[Shop") {
                    current = Some((0, String::new(), Vec::new()));
                }
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let Some((id, name, items)) = current.as_mut() else {
                continue;
            };
            match key.trim() {
                "ID" => *id = value.trim().parse().unwrap_or_default(),
                "Name" => *name = value.trim().to_owned(),
                key if key.strip_prefix("Item").is_some_and(|index| {
                    !index.is_empty() && index.chars().all(|c| c.is_ascii_digit())
                }) => {
                    if let Ok(item_id) = value.trim().parse() {
                        items.push(item_id);
                    }
                }
                _ => {}
            }
        }
        if found.is_none()
            && let Some((id, name, items)) = current
            && id == npc_id
        {
            found = Some((id, name, items));
        }
        let Some((npc_id, name, items)) = found else {
            return Ok(None);
        };
        let animation = assets
            .read("ani/ItemMinIcon.Ani")
            .ok()
            .map(|bytes| String::from_utf8_lossy(&bytes).into_owned());
        let icons = items
            .iter()
            .take(20)
            .map(|item_id| load_icon(assets, *item_id, animation.as_deref()))
            .collect::<Result<Vec<_>>>()?;
        let infos = load_item_infos(assets, &items);
        Ok(Some(Self {
            npc_id,
            name,
            items,
            icons,
            infos,
            hover: None,
            selected: None,
        }))
    }

    pub fn item_at(&self, viewport: [f32; 2], cursor: [f32; 2]) -> Option<usize> {
        let [left, top] = panel_position(viewport);
        if cursor[0] < left + SLOT_X as f32 || cursor[1] < top + SLOT_Y as f32 {
            return None;
        }
        let column = ((cursor[0] - left - SLOT_X as f32) / SLOT_STEP_X as f32).floor() as u32;
        let row = ((cursor[1] - top - SLOT_Y as f32) / SLOT_STEP_Y as f32).floor() as u32;
        if column >= 5 || row >= 4 {
            return None;
        }
        let x = left + SLOT_X as f32 + column as f32 * SLOT_STEP_X as f32;
        let y = top + SLOT_Y as f32 + row as f32 * SLOT_STEP_Y as f32;
        if cursor[0] > x + SLOT_SIZE as f32 || cursor[1] > y + SLOT_SIZE as f32 {
            return None;
        }
        let index = row as usize * 5 + column as usize;
        (index < self.items.len()).then_some(index)
    }

    pub fn item_info(&self, item_id: u32) -> Option<&ItemInfo> {
        self.infos.get(&item_id)
    }
}

pub fn slot_position(viewport: [f32; 2], index: usize) -> [f32; 2] {
    let [left, top] = panel_position(viewport);
    let column = index as u32 % 5;
    let row = index as u32 / 5;
    [
        left + SLOT_X as f32 + column as f32 * SLOT_STEP_X as f32 + 1.0,
        top + SLOT_Y as f32 + row as f32 * SLOT_STEP_Y as f32 + 1.0,
    ]
}

pub fn panel_position(viewport: [f32; 2]) -> [f32; 2] {
    [8.0, ((viewport[1] - SHOP_HEIGHT as f32) * 0.5).max(8.0)]
}

pub fn inventory_position(viewport: [f32; 2]) -> [f32; 2] {
    [
        (viewport[0] - PANEL_WIDTH as f32 - 8.0).max(8.0),
        ((viewport[1] - INVENTORY_HEIGHT as f32) * 0.5).max(8.0),
    ]
}

pub fn panel_image(assets: &mut Assets, state: &ShopState) -> Result<RgbaImage> {
    let source = assets.image("data/main/DialogMpcGoods.dds")?;
    let mut image = RgbaImage::new(PANEL_WIDTH, SHOP_HEIGHT);
    let top = imageops::resize(
        &imageops::crop_imm(&source, 0, 0, PANEL_WIDTH, 198).to_image(),
        PANEL_WIDTH,
        200,
        imageops::FilterType::Nearest,
    );
    let footer = imageops::crop_imm(&source, 0, 460, PANEL_WIDTH, 52).to_image();
    imageops::replace(&mut image, &top, 0, 0);
    imageops::replace(&mut image, &footer, 0, 198);

    let font = labels::font()?;
    let title = if state.name.is_empty() {
        "Shop"
    } else {
        state.name.as_str()
    };
    labels::text(&mut image, &font, &[(title, [255, 232, 160])], [90.0, 8.0]);
    for (index, _item_id) in state.items.iter().take(20).enumerate() {
        let row = index as u32 / 5;
        let column = index as u32 % 5;
        let x = SLOT_X + column * SLOT_STEP_X + 4;
        let y = SLOT_Y + row * SLOT_STEP_Y + 4;
        let Some(icon) = state.icons.get(index) else {
            continue;
        };
        imageops::overlay(&mut image, icon, x.into(), y.into());
        if state.selected == Some(index) {
            outline(
                &mut image,
                x - 3,
                y - 3,
                SLOT_SIZE + 2,
                Rgba([255, 220, 70, 255]),
            );
        }
    }
    labels::text(&mut image, &font, &[("Repair", [255, 232, 160])], [27.0, 218.0]);
    labels::text(&mut image, &font, &[("Help", [255, 232, 160])], [135.0, 218.0]);
    labels::text(&mut image, &font, &[("Close", [255, 232, 160])], [194.0, 218.0]);
    Ok(image)
}

pub fn highlight_image() -> RgbaImage {
    let mut image = RgbaImage::from_pixel(39, 39, Rgba([0, 0, 0, 0]));
    outline(&mut image, 0, 0, 38, Rgba([255, 255, 255, 255]));
    image
}

pub fn close_at(viewport: [f32; 2], cursor: [f32; 2]) -> bool {
    let [left, top] = panel_position(viewport);
    cursor[0] >= left + 185.0
        && cursor[0] <= left + PANEL_WIDTH as f32
        && cursor[1] >= top + 208.0
        && cursor[1] <= top + SHOP_HEIGHT as f32
}

pub fn inventory_image(assets: &mut Assets) -> Result<RgbaImage> {
    let source = assets.image("data/main/DialogMpcGoods.dds")?;
    let mut image = imageops::crop_imm(&source, 0, 0, PANEL_WIDTH, INVENTORY_HEIGHT).to_image();
    let font = labels::font()?;
    labels::text(&mut image, &font, &[("Help", [255, 232, 160])], [168.0, 488.0]);
    labels::text(&mut image, &font, &[("Close", [255, 232, 160])], [218.0, 488.0]);
    Ok(image)
}

pub fn tooltip_image(item_id: u32, info: Option<&ItemInfo>) -> Result<RgbaImage> {
    let font = labels::font()?;
    let mut image = RgbaImage::from_pixel(260, 138, Rgba([7, 12, 18, 238]));
    for x in 0..image.width() {
        for y in 0..image.height() {
            if x < 2 || y < 2 || x >= image.width() - 2 || y >= image.height() - 2 {
                image.put_pixel(x, y, Rgba([225, 190, 75, 255]));
            }
        }
    }
    let fallback = format!("Item {item_id}");
    let name = info.map(|item| item.name.as_str()).unwrap_or(&fallback);
    let item_type = info
        .map(|item| item.item_type.replace('`', "'"))
        .filter(|value| !value.is_empty() && value != "None");
    let title = item_type
        .as_ref()
        .map(|item_type| format!("{name} ({item_type})"))
        .unwrap_or_else(|| name.to_owned());
    let level = format!("Level: {}", info.map(|item| item.level).unwrap_or_default());
    let defense = format!(
        "Defense: {}",
        info.map(|item| format!("+{}", item.defense))
            .unwrap_or_else(|| "--".to_owned())
    );
    let attack = info
        .map(|item| {
            if item.attack_min == 0 && item.attack_max == 0 {
                "--".to_owned()
            } else if item.attack_min == item.attack_max {
                item.attack_max.to_string()
            } else {
                format!("{}-{}", item.attack_min, item.attack_max)
            }
        })
        .unwrap_or_else(|| "--".to_owned());
    let attack = format!("Attack: {attack}");
    let price = format!("Price: {}", info.map(|item| item.price).unwrap_or_default());
    labels::text(&mut image, &font, &[(&title, [255, 255, 255])], [10.0, 12.0]);
    labels::text(&mut image, &font, &[(&level, [255, 70, 70])], [10.0, 36.0]);
    labels::text(&mut image, &font, &[(&defense, [255, 255, 255])], [10.0, 58.0]);
    labels::text(&mut image, &font, &[(&attack, [255, 255, 255])], [10.0, 80.0]);
    labels::text(&mut image, &font, &[(&price, [255, 220, 70])], [10.0, 104.0]);
    Ok(image)
}

fn load_item_infos(assets: &mut Assets, item_ids: &[u32]) -> HashMap<u32, ItemInfo> {
    let requested = item_ids.iter().copied().collect::<HashSet<_>>();
    let Ok(mut bytes) = assets.read("ini/itemtype.dat") else {
        return HashMap::new();
    };
    decrypt_itemtype(&mut bytes);
    let contents = String::from_utf8_lossy(&bytes);
    let mut infos = HashMap::new();
    for line in contents.lines().skip(1) {
        let fields = line.split_ascii_whitespace().collect::<Vec<_>>();
        if fields.len() < 39 {
            continue;
        }
        let Ok(id) = fields[0].parse::<u32>() else {
            continue;
        };
        if !requested.contains(&id) {
            continue;
        }
        let number = |index: usize| fields[index].parse::<u32>().unwrap_or_default();
        infos.insert(
            id,
            ItemInfo {
                name: fields[1].replace('~', " "),
                level: number(4),
                attack_max: number(14),
                attack_min: number(15),
                defense: number(16),
                price: number(12),
                item_type: fields[37].replace('~', " "),
            },
        );
    }
    infos
}

fn decrypt_itemtype(bytes: &mut [u8]) {
    let mut seed = 9527u32;
    let mut key = [0u8; 128];
    for value in &mut key {
        seed = seed
            .wrapping_mul(0x343fdu32)
            .wrapping_add(0x269ec3u32);
        *value = ((seed >> 16) & 0x7fff) as u8;
    }
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = (*byte ^ key[index % key.len()]).rotate_right((index % 8) as u32);
    }
}

fn load_icon(
    assets: &mut Assets,
    item_id: u32,
    animation: Option<&str>,
) -> Result<image::RgbaImage> {
    let candidates = [
        format!("data/ItemMinIcon/{item_id}.dds"),
        format!("data/MapItemIcon/{item_id}.dds"),
    ];
    for path in candidates {
        if let Ok(icon) = assets.image(&path) {
            return Ok(imageops::resize(&icon, 36, 36, imageops::FilterType::Nearest));
        }
    }
    if let Some(animation) = animation {
        for colour in 3..=9 {
            let section = format!("[Item{colour}{item_id}]");
            let Some(section_start) = animation.find(&section) else {
                continue;
            };
            let section = &animation[section_start..];
            let Some(frame) = section
                .lines()
                .skip(1)
                .take_while(|line| !line.starts_with('['))
                .find_map(|line| line.strip_prefix("Frame0="))
            else {
                continue;
            };
            if let Ok(icon) = assets.image(frame.trim()) {
                return Ok(imageops::resize(&icon, 36, 36, imageops::FilterType::Nearest));
            }
        }
    }
    Ok(placeholder_icon(item_id)?)
}

fn placeholder_icon(item_id: u32) -> Result<RgbaImage> {
    let mut image = RgbaImage::from_pixel(36, 36, Rgba([19, 28, 38, 255]));
    let accent = [
        90 + ((item_id >> 8) & 0x3f) as u8,
        90 + ((item_id >> 4) & 0x3f) as u8,
        90 + (item_id & 0x3f) as u8,
    ];
    for offset in 0..36 {
        image.put_pixel(offset, 0, Rgba([accent[0], accent[1], accent[2], 255]));
        image.put_pixel(offset, 35, Rgba([accent[0], accent[1], accent[2], 255]));
        image.put_pixel(0, offset, Rgba([accent[0], accent[1], accent[2], 255]));
        image.put_pixel(35, offset, Rgba([accent[0], accent[1], accent[2], 255]));
    }
    let short_id = item_id % 1000;
    let digits = [short_id / 100, (short_id / 10) % 10, short_id % 10];
    for (index, digit) in digits.into_iter().enumerate() {
        draw_digit(&mut image, digit as usize, 5 + index as u32 * 9, 12, accent);
    }
    Ok(image)
}

fn draw_digit(image: &mut RgbaImage, digit: usize, left: u32, top: u32, colour: [u8; 3]) {
    const GLYPHS: [[u8; 15]; 10] = [
        [1, 1, 1, 1, 0, 1, 1, 0, 1, 1, 0, 1, 1, 1, 1],
        [0, 1, 0, 1, 1, 0, 0, 1, 0, 0, 1, 0, 1, 1, 1],
        [1, 1, 1, 0, 0, 1, 1, 1, 1, 1, 0, 0, 1, 1, 1],
        [1, 1, 1, 0, 0, 1, 0, 1, 1, 0, 0, 1, 1, 1, 1],
        [1, 0, 1, 1, 0, 1, 1, 1, 1, 0, 0, 1, 0, 0, 1],
        [1, 1, 1, 1, 0, 0, 1, 1, 1, 0, 0, 1, 1, 1, 1],
        [1, 1, 1, 1, 0, 0, 1, 1, 1, 1, 0, 1, 1, 1, 1],
        [1, 1, 1, 0, 0, 1, 0, 1, 0, 1, 0, 0, 1, 0, 0],
        [1, 1, 1, 1, 0, 1, 1, 1, 1, 1, 0, 1, 1, 1, 1],
        [1, 1, 1, 1, 0, 1, 1, 1, 1, 0, 0, 1, 1, 1, 1],
    ];
    for row in 0..5 {
        for column in 0..3 {
            if GLYPHS[digit][row * 3 + column] != 0 {
                image.put_pixel(
                    left + column as u32,
                    top + row as u32,
                    Rgba([colour[0], colour[1], colour[2], 255]),
                );
            }
        }
    }
}

fn outline(image: &mut RgbaImage, x: u32, y: u32, size: u32, colour: Rgba<u8>) {
    for offset in 0..size {
        if x + offset < image.width() && y < image.height() {
            image.put_pixel(x + offset, y, colour);
        }
        if x + offset < image.width() && y + size < image.height() {
            image.put_pixel(x + offset, y + size, colour);
        }
        if x < image.width() && y + offset < image.height() {
            image.put_pixel(x, y + offset, colour);
        }
        if x + size < image.width() && y + offset < image.height() {
            image.put_pixel(x + size, y + offset, colour);
        }
    }
}
