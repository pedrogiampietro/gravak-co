use crate::{assets::{Assets, Result}, labels};
use image::{imageops, Rgba, RgbaImage};

const PANEL_WIDTH: u32 = 270;
const SHOP_HEIGHT: u32 = 270;
const INVENTORY_HEIGHT: u32 = 512;
const SLOT_X: u32 = 25;
const SLOT_Y: u32 = 27;
const SLOT_STEP_X: u32 = 48;
const SLOT_STEP_Y: u32 = 50;
const SLOT_SIZE: u32 = 40;

#[derive(Debug, Clone)]
pub struct ShopState {
    pub npc_id: u32,
    pub name: String,
    pub items: Vec<u32>,
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
        Ok(found.map(|(npc_id, name, items)| Self {
            npc_id,
            name,
            items,
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
        &imageops::crop_imm(&source, 0, 0, PANEL_WIDTH, 238).to_image(),
        PANEL_WIDTH,
        220,
        imageops::FilterType::Nearest,
    );
    let footer = imageops::crop_imm(&source, 0, 460, PANEL_WIDTH, 52).to_image();
    imageops::replace(&mut image, &top, 0, 0);
    imageops::replace(&mut image, &footer, 0, 218);

    let font = labels::font()?;
    let title = if state.name.is_empty() {
        "Shop"
    } else {
        state.name.as_str()
    };
    labels::text(&mut image, &font, &[(title, [255, 232, 160])], [90.0, 8.0]);
    for (index, item_id) in state.items.iter().take(20).enumerate() {
        let row = index as u32 / 5;
        let column = index as u32 % 5;
        let x = SLOT_X + column * SLOT_STEP_X + 4;
        let y = SLOT_Y + row * SLOT_STEP_Y + 4;
        let icon = load_icon(assets, *item_id)?;
        imageops::overlay(&mut image, &icon, x.into(), y.into());
        if state.hover == Some(index) || state.selected == Some(index) {
            let colour = if state.selected == Some(index) {
                Rgba([255, 220, 70, 255])
            } else {
                Rgba([255, 255, 255, 255])
            };
            outline(&mut image, x - 3, y - 3, SLOT_SIZE + 2, colour);
        }
    }
    labels::text(&mut image, &font, &[("Repair", [255, 232, 160])], [27.0, 238.0]);
    labels::text(&mut image, &font, &[("Help", [255, 232, 160])], [135.0, 238.0]);
    labels::text(&mut image, &font, &[("Close", [255, 232, 160])], [194.0, 238.0]);
    Ok(image)
}

pub fn close_at(viewport: [f32; 2], cursor: [f32; 2]) -> bool {
    let [left, top] = panel_position(viewport);
    cursor[0] >= left + 185.0
        && cursor[0] <= left + PANEL_WIDTH as f32
        && cursor[1] >= top + 228.0
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

pub fn tooltip_image(item_id: u32) -> Result<RgbaImage> {
    let font = labels::font()?;
    let mut image = RgbaImage::from_pixel(220, 126, Rgba([7, 12, 18, 238]));
    for x in 0..image.width() {
        for y in 0..image.height() {
            if x < 2 || y < 2 || x >= image.width() - 2 || y >= image.height() - 2 {
                image.put_pixel(x, y, Rgba([225, 190, 75, 255]));
            }
        }
    }
    let id = format!("Item {item_id}");
    labels::text(&mut image, &font, &[(&id, [255, 255, 255])], [10.0, 12.0]);
    labels::text(&mut image, &font, &[("Level: --", [255, 70, 70])], [10.0, 34.0]);
    labels::text(&mut image, &font, &[("Defense: --", [255, 255, 255])], [10.0, 52.0]);
    labels::text(&mut image, &font, &[("Attack: --", [255, 255, 255])], [10.0, 70.0]);
    labels::text(&mut image, &font, &[("Price: --", [255, 220, 70])], [10.0, 96.0]);
    Ok(image)
}

fn load_icon(assets: &mut Assets, item_id: u32) -> Result<image::RgbaImage> {
    let candidates = [
        format!("data/ItemMinIcon/{item_id}.dds"),
        format!("data/MapItemIcon/{item_id}.dds"),
    ];
    for path in candidates {
        if let Ok(icon) = assets.image(&path) {
            return Ok(imageops::resize(&icon, 36, 36, imageops::FilterType::Nearest));
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
    let font = labels::font()?;
    let short_id = format!("{:03}", item_id % 1000);
    labels::text(&mut image, &font, &[(&short_id, [230, 220, 170])], [5.0, 13.0]);
    Ok(image)
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
