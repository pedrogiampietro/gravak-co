use crate::{
    assets::{Assets, Result},
    labels, original_ui_layout,
    ui::{self, UiRect},
    ui_theme,
};
use image::{Rgba, RgbaImage, imageops};
use std::collections::{HashMap, HashSet};

const PANEL_WIDTH: u32 = 272;
// DialogNpcBuy.dds is drawn at the dialog origin; its frame ends at y=263.
const SHOP_HEIGHT: u32 = 264;
// Native DialogMpcGoods canvas from the 5065 atlas. The dimensions and
// anchors are owned by original_ui_layout so rendering and hit-testing use
// exactly the same coordinate system.
const INVENTORY_HEIGHT: u32 = original_ui_layout::INVENTORY_HEIGHT as u32;
const INVENTORY_WIDTH: u32 = original_ui_layout::INVENTORY_WIDTH as u32;
const WAREHOUSE_WIDTH: u32 = 270;
const WAREHOUSE_HEIGHT: u32 = 386;
// Shop slot frames painted by DialogNpcBuy ([152-1552] grid at 21,36).
const SLOT_X: u32 = 24;
const SLOT_Y: u32 = 38;
const SLOT_STEP_X: u32 = 43;
const SLOT_STEP_Y: u32 = 43;
const SLOT_SIZE: u32 = 40;
const ICON_SIZE: u32 = 32;
const ICON_OFFSET: u32 = 6;
// DialogMpcGoods.dds uses the original 5065 slot grid.
const INVENTORY_SLOT_X: u32 = original_ui_layout::INVENTORY_GRID[0] as u32;
const INVENTORY_SLOT_Y: u32 = original_ui_layout::INVENTORY_GRID[1] as u32;
const INVENTORY_SLOT_STEP_X: u32 = original_ui_layout::INVENTORY_GRID_STEP[0] as u32;
const INVENTORY_SLOT_STEP_Y: u32 = original_ui_layout::INVENTORY_GRID_STEP[1] as u32;
const INVENTORY_SLOT_SIZE: u32 = original_ui_layout::INVENTORY_GRID[2] as u32;
const INVENTORY_ICON_OFFSET: u32 = ((original_ui_layout::INVENTORY_GRID[2]
    - original_ui_layout::INVENTORY_ICON_SIZE)
    * 0.5) as u32;
// GUI.ini [153-1128] Close button and the text origins of the silver
// ([153-1127]) and CPs ([153-1129]) fields painted by the dialog.
const INVENTORY_CLOSE_RECT: [i32; 4] = [170, 447, 62, 20];
const INVENTORY_SILVER_TEXT: [i32; 2] = [121, 388];
const INVENTORY_CPS_TEXT: [i32; 2] = [121, 420];
const INVENTORY_FILTER_X: u32 = 26;
const INVENTORY_FILTER_Y: u32 = 44;
const INVENTORY_FILTER_STEP_X: u32 = 42;
const INVENTORY_FILTER_WIDTH: u32 = 40;
const INVENTORY_FILTER_HEIGHT: u32 = 22;
// Keep the action in the inventory header, but give it the same compact tab
// treatment as the rest of the client-owned UI. It sits immediately before
// the close button without competing with the centered window title.
const INVENTORY_ORGANIZE_BUTTON: UiRect = UiRect::new(182, 8, 46, 24);
const ITEMS_PER_PAGE: usize = 20;
// GUI.ini [152-1105] Repair, [152-1667] Help and [152-1104] Close.
const FOOTER_BUTTON_Y: f32 = 224.0;
const SHOP_REPAIR_X: f32 = 33.0;
const SHOP_HELP_X: f32 = 143.0;
const SHOP_CLOSE_X: f32 = 200.0;
// [152-1550]/[152-1551] page buttons.
const SHOP_PAGE_X: f32 = 243.0;
const SHOP_PAGE_UP_Y: f32 = 155.0;
const SHOP_PAGE_DOWN_Y: f32 = 190.0;
const WAREHOUSE_SLOT_X: u32 = 20;
const WAREHOUSE_SLOT_Y: u32 = 168;
const WAREHOUSE_SLOT_SIZE: u32 = 40;
const WAREHOUSE_PAGE_SIZE: u8 = 20;
// DialogDepot controls: withdraw/deposit buttons ([161-1619] row), the
// amount/savings boxes painted by the art ([161-1620]), page buttons
// ([161-1387]/[161-1388]) and the footer Help/Close text buttons.
const WAREHOUSE_WITHDRAW: [i32; 2] = [46, 54];
const WAREHOUSE_DEPOSIT: [i32; 2] = [150, 54];
const WAREHOUSE_BUTTON_SIZE: [i32; 2] = [64, 22];
const WAREHOUSE_AMOUNT_BOX: [i32; 4] = [126, 88, 82, 21];
const WAREHOUSE_SAVINGS_BOX: [i32; 4] = [126, 117, 82, 21];
const WAREHOUSE_PAGE_UP: [i32; 2] = [232, 287];
const WAREHOUSE_PAGE_DOWN: [i32; 2] = [232, 321];
const WAREHOUSE_HELP: [i32; 2] = [45, 348];
const WAREHOUSE_CLOSE: [i32; 4] = [170, 348, 62, 20];
// Native Character window dimensions from GUI.ini [0-131]. The original DDS
// components occupy this logical space; the renderer scales the whole client
// from the same 1024x768 reference instead of resizing each panel ad hoc.
const STATUS_WIDTH: u32 = original_ui_layout::CHARACTER_WIDTH as u32;
// GUI.ini [0-131] centers the Character window using its dialog height
// (406px). Some native DDS variants extend below that logical frame, so the
// drawn surface can be 435px without moving the window anchor.
const STATUS_HEIGHT: u32 = original_ui_layout::CHARACTER_DIALOG_HEIGHT as u32;
const STATUS_STANDARD_WIDTH: u32 = original_ui_layout::CHARACTER_WIDTH as u32;
const STATUS_STANDARD_HEIGHT: u32 = original_ui_layout::CHARACTER_CANVAS_HEIGHT as u32;
const STATUS_TAB_WIDTH: u32 = 96;
const QUICK_PANEL_WIDTH: u32 = 520;
const QUICK_PANEL_HEIGHT: u32 = 420;

/// Single source of truth for the Character window geometry.
///
/// The original DDS frame owns the ornamental artwork. This layout owns
/// the interactive/content areas painted over it, so rendering and
/// hit-testing cannot drift apart when the panel is adjusted.
struct StatusLayout;

fn status_surface_size(_tab: StatusTab) -> (u32, u32) {
    // Every tab shares the DialogQuery shell; only the child dialog changes.
    (STATUS_STANDARD_WIDTH, STATUS_STANDARD_HEIGHT)
}

impl StatusLayout {
    const TAB_X: i32 = 0;
    // GUI.ini [131-1504], [131-1506], [131-1505] and [131-1028].
    const TAB_Y_POSITIONS: [i32; 4] = [112, 170, 228, 288];
    const TAB_WIDTH: i32 = 37;
    const TAB_HEIGHT: i32 = 37;

    // GUI.ini [131-132], [131-134] and [131-135].
    const PAPER_X: i32 = 40;
    const PAPER_Y: i32 = 55;
    const PAPER_WIDTH: i32 = 206;
    const PAPER_HEIGHT: i32 = 332;
    // GUI.ini [135-*] equipment controls: head (1672), neck (1674), ring
    // (1673, 手饰), garment (1680, 外套), bracelet/talisman (1679, 法宝),
    // right hand (1677), left hand (1675), armor (1678, 盔甲), boots (1676)
    // and the reserved empty position. SLOT_POSITIONS holds the matching
    // server positions (1 head, 2 neck, 3 armor, 4 right, 5 left, 6 ring,
    // 7 bracelet, 8 boots, 9 garment).
    const SLOT_RECTS: [(i32, i32, i32, i32); 10] = [
        (153, 14, 40, 40),
        (153, 67, 40, 40),
        (153, 116, 40, 40),
        (7, 164, 40, 40),
        (7, 217, 40, 40),
        (129, 170, 65, 65),
        (55, 249, 65, 65),
        (129, 245, 65, 65),
        (7, 269, 40, 40),
        (0, 0, 0, 0),
    ];
    const SLOT_POSITIONS: [u8; 10] = [1, 2, 6, 9, 7, 4, 5, 3, 8, 0];

    const INFO_X: i32 = 40;
    const INFO_WIDTH: i32 = 206;
    const BASIC_Y: i32 = 55;
    const BASIC_HEIGHT: i32 = 150;
    const COMBAT_Y: i32 = 205;
    const COMBAT_HEIGHT: i32 = 170;

    const FOOTER_X: i32 = 40;
    const FOOTER_Y: i32 = 400;
    const FOOTER_WIDTH: i32 = 206;
    const FOOTER_HEIGHT: i32 = 55;

    // GUI.ini [131-1027] (top-right X) and [131-1029] (bottom Close).
    const CLOSE_ORIGIN: [i32; 2] = [239, 38];
    const CLOSE_RECTS: [(i32, i32, i32, i32); 2] = [(236, 35, 19, 19), (168, 386, 63, 20)];

    fn reputation_button_rect() -> UiRect {
        UiRect::new(
            Self::INFO_X + 7,
            Self::BASIC_Y + 180,
            Self::INFO_WIDTH - 14,
            24,
        )
    }

    fn title_button_rect(index: usize) -> UiRect {
        UiRect::new(
            Self::INFO_X + 7,
            Self::BASIC_Y + 78 + index as i32 * 24,
            Self::INFO_WIDTH - 14,
            24,
        )
    }

    fn slot_xy(slot: usize) -> (i32, i32) {
        let (x, y, _, _) = Self::SLOT_RECTS[slot];
        (x, y)
    }

    fn slot_rect(slot: usize) -> UiRect {
        let (x, y, width, height) = Self::SLOT_RECTS[slot];
        UiRect::new(x, y, width, height)
    }

    fn tab_rect(index: usize) -> UiRect {
        UiRect::new(
            Self::TAB_X,
            Self::TAB_Y_POSITIONS[index],
            Self::TAB_WIDTH,
            Self::TAB_HEIGHT,
        )
    }

    fn slot_position(slot: usize) -> u8 {
        Self::SLOT_POSITIONS[slot]
    }
}

#[derive(Debug, Clone, Default)]
pub struct ItemInfo {
    pub name: String,
    pub level: u32,
    pub attack_min: u32,
    pub attack_max: u32,
    pub defense: u32,
    pub health_add: u32,
    pub mana_add: u32,
    pub attack_speed: u32,
    pub price: u32,
    pub item_type: String,
}

/// Quality tiers used by the item presentation. In the 5065 catalog the final
/// digit of an equipment type carries the classic quality (normal, refined,
/// unique, elite or super). It is deliberately independent from `plus`, which
/// is the item's upgrade tier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ItemRarity {
    Common,
    Rare,
    Epic,
    Legendary,
    Mythic,
}

impl ItemRarity {
    pub fn from_custom_code(code: u8) -> Self {
        match code {
            1 => Self::Rare,
            2 => Self::Epic,
            3 => Self::Legendary,
            4 => Self::Mythic,
            _ => Self::Common,
        }
    }

    fn native_quality(item_type: u32) -> &'static str {
        if item_type >= 1_000_000 {
            return "Standard";
        }
        match item_type % 10 {
            5 => "Normal",
            6 => "Refined",
            7 => "Unique",
            8 => "Elite",
            9 => "Super",
            _ => "Standard",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Common => "Common",
            Self::Rare => "Rare",
            Self::Epic => "Epic",
            Self::Legendary => "Legendary",
            Self::Mythic => "Mythic",
        }
    }

    fn colour(self) -> [u8; 3] {
        match self {
            Self::Common => [190, 198, 202],
            Self::Rare => [78, 168, 255],
            Self::Epic => [207, 108, 255],
            Self::Legendary => [255, 174, 65],
            Self::Mythic => [255, 92, 92],
        }
    }

    fn bonus_percent(self) -> u32 {
        match self {
            Self::Common => 0,
            Self::Rare => 5,
            Self::Epic => 10,
            Self::Legendary => 15,
            Self::Mythic => 25,
        }
    }
}

pub fn rarity_upgrade_cost_and_chance(rarity: u8) -> Option<(u8, u8)> {
    Some(match rarity {
        0 => (1, 75),
        1 => (2, 55),
        2 => (3, 35),
        3 => (5, 20),
        _ => return None,
    })
}

pub fn rarity_bonus_slots(rarity: u8) -> u8 {
    rarity.min(4)
}

/// Generates the stable random-looking bonus attached to one rarity slot.
/// The seed is persisted by the server and changes only after an upgrade or
/// an explicit reroll, so the preview remains stable across item moves.
pub fn rarity_bonus(item_uid: u32, rarity_roll: u8, slot: u8) -> (&'static str, u16, &'static str) {
    let mut state = u32::from(rarity_roll.max(1))
        .wrapping_mul(1_103_515_245)
        .wrapping_add(item_uid)
        .wrapping_add(u32::from(slot).wrapping_mul(2_654_435_761));
    state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
    let kind = (state % 6) as usize;
    let value = 1 + ((state / 7) % 5) as u16 + u16::from(slot.min(3));
    match kind {
        0 => ("Attack", value, "%"),
        1 => ("Defense", value, "%"),
        2 => ("Max HP", value.saturating_mul(20), ""),
        3 => ("Critical", value, "%"),
        4 => ("Damage", value, "%"),
        _ => ("Damage Resist", value, "%"),
    }
}

pub fn upgrade_cost_and_chance(plus: u8) -> Option<(u8, u8)> {
    Some(match plus {
        0 => (1, 100),
        1 => (1, 95),
        2 => (1, 90),
        3 => (2, 80),
        4 => (2, 70),
        5 => (3, 60),
        6 => (3, 50),
        7 => (4, 40),
        8 => (5, 30),
        9 => (5, 25),
        10 => (6, 20),
        11 => (8, 15),
        _ => return None,
    })
}

#[derive(Debug, Clone)]
pub struct ShopState {
    pub npc_id: u32,
    pub name: String,
    pub items: Vec<u32>,
    icons: Vec<RgbaImage>,
    infos: HashMap<u32, ItemInfo>,
    pub page: usize,
    pub hover: Option<usize>,
    pub selected: Option<usize>,
    pub repair_hover: bool,
    pub scroll_hover: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum InventoryFilter {
    #[default]
    All,
    Equipment,
    Consumables,
    Materials,
    Quest,
}

impl InventoryFilter {
    pub const ALL: [Self; 5] = [
        Self::All,
        Self::Equipment,
        Self::Consumables,
        Self::Materials,
        Self::Quest,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Equipment => "Equip",
            Self::Consumables => "Use",
            Self::Materials => "Mat",
            Self::Quest => "Quest",
        }
    }

    pub fn accepts(self, item_type: u32) -> bool {
        match self {
            Self::All => true,
            // Classic Conquer equipment IDs are below the currency/material
            // ranges and are grouped by their leading type digits.
            Self::Equipment => item_type < 1_000_000,
            Self::Consumables => {
                (1_000_000..1_080_000).contains(&item_type)
                    || consumable_bundle_for_item(item_type).is_some()
            }
            Self::Materials => (1_080_000..1_090_000).contains(&item_type),
            Self::Quest => item_type >= 1_090_000,
        }
    }
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
                }) =>
                {
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
        let map_animation = assets
            .read("ani/MapItemIcon.Ani")
            .ok()
            .map(|bytes| String::from_utf8_lossy(&bytes).into_owned());
        let size = original_ui_layout::INVENTORY_ICON_SIZE as u32;
        let icons = items
            .iter()
            .map(|item_id| {
                load_icon_native(
                    assets,
                    *item_id,
                    animation.as_deref(),
                    map_animation.as_deref(),
                )
                .map(|icon| {
                    if icon.dimensions() == (size, size) {
                        icon
                    } else {
                        imageops::resize(&icon, size, size, imageops::FilterType::Triangle)
                    }
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let infos = load_item_infos(assets, &items);
        Ok(Some(Self {
            npc_id,
            name,
            items,
            icons,
            infos,
            page: 0,
            hover: None,
            selected: None,
            repair_hover: false,
            scroll_hover: None,
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
        let index = self.page * 5 + row as usize * 5 + column as usize;
        (index < self.items.len()).then_some(index)
    }

    pub fn item_info(&self, item_id: u32) -> Option<&ItemInfo> {
        self.infos.get(&item_id)
    }

    pub fn icon_image(&self, item_id: u32) -> Option<&RgbaImage> {
        self.items
            .iter()
            .position(|candidate| *candidate == item_id)
            .and_then(|index| self.icons.get(index))
    }

    pub fn visible_slot(&self, index: usize) -> Option<usize> {
        let start = self.page * 5;
        (index >= start && index < (start + ITEMS_PER_PAGE).min(self.items.len()))
            .then_some(index - start)
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
    original_ui_layout::inventory_position(viewport)
}

pub fn inventory_slot_position(viewport: [f32; 2], slot: u8) -> [f32; 2] {
    let [left, top] = inventory_position(viewport);
    let rect = inventory_slot_rect(slot);
    [left + rect.x as f32 + 1.0, top + rect.y as f32 + 1.0]
}

pub fn status_position(viewport: [f32; 2]) -> [f32; 2] {
    original_ui_layout::character_position(viewport)
}

pub fn status_equipment_slot_at(viewport: [f32; 2], cursor: [f32; 2]) -> Option<u8> {
    let [left, top] = status_position(viewport);
    for slot in 0..StatusLayout::SLOT_POSITIONS.len() {
        let position = StatusLayout::slot_position(slot);
        let rect = StatusLayout::slot_rect(slot);
        let local = [
            cursor[0] - left - StatusLayout::PAPER_X as f32,
            cursor[1] - top - StatusLayout::PAPER_Y as f32,
        ];
        if rect.contains_local(local) {
            return (position != 0).then_some(position);
        }
    }
    None
}

pub fn status_character_clip(viewport: [f32; 2]) -> [u32; 4] {
    let [left, top] = status_position(viewport);
    [
        (left + StatusLayout::PAPER_X as f32).max(0.0) as u32,
        (top + StatusLayout::PAPER_Y as f32).max(0.0) as u32,
        StatusLayout::PAPER_WIDTH as u32,
        StatusLayout::PAPER_HEIGHT as u32,
    ]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QuickPanel {
    Skills,
    Quest,
    Guild,
    Friends,
    Trade,
    Mail,
    Market,
    Team,
    System,
    Settings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsPanelAction {
    ToggleArcs,
    ToggleBars,
    ScaleDown,
    ScaleUp,
    DistanceDown,
    DistanceUp,
}

/// The Options window is the original `DialogSetup`, which has no HUD
/// tuning controls; the HUD-centre settings belonged to the removed overlay.
pub fn settings_panel_action_at(
    _viewport: [f32; 2],
    _cursor: [f32; 2],
) -> Option<SettingsPanelAction> {
    None
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TeamPanelAction {
    Create,
    AcceptInvite,
    RequestJoin,
    AcceptJoin,
    Invite,
    Leave,
    Leader,
    Kick,
    Dismiss,
    ToggleJoin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FriendsPanelAction {
    AddFriend,
    AddIgnore,
    Whisper(usize),
    RemoveFriend(usize),
    RemoveIgnore(usize),
    AcceptRequest,
    RefuseRequest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TradePanelAction {
    Request,
    AcceptRequest,
    AddItem,
    RemoveOwnItem(usize),
    SetMoney,
    Confirm,
    Cancel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerShopPanelAction {
    Open,
    Close,
    QueryTarget,
    AddSelectedItem,
    Buy(usize),
    Remove(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuildPanelAction {
    AcceptInvite,
    Warehouse,
    Invite,
    Leave,
    Kick,
}

impl QuickPanel {
    pub fn from_button(index: usize) -> Option<Self> {
        Some(match index {
            2 => Self::Skills,
            3 => Self::Quest,
            4 => Self::Guild,
            5 => Self::Friends,
            6 => Self::Trade,
            7 => Self::Team,
            8 => Self::Market,
            9 => Self::Settings,
            10 => Self::Mail,
            _ => return None,
        })
    }

    fn title(self) -> &'static str {
        match self {
            Self::Skills => "SKILLS",
            Self::Quest => "QUESTS",
            Self::Guild => "GUILD",
            Self::Friends => "FRIENDS",
            Self::Trade => "TRADE",
            Self::Mail => "MAIL",
            Self::Market => "MARKET",
            Self::Team => "TEAM",
            Self::System => "SYSTEM",
            Self::Settings => "SETTINGS",
        }
    }
}

pub fn trade_panel_action_at(
    viewport: [f32; 2],
    cursor: [f32; 2],
    trade: &crate::network::TradeView,
) -> Option<TradePanelAction> {
    crate::original_panels::trade_action_at(viewport, cursor, trade)
}

pub fn trade_panel_drop_at(viewport: [f32; 2], cursor: [f32; 2], active: bool) -> bool {
    active && crate::original_panels::trade_drop_at(viewport, cursor)
}

pub fn player_shop_panel_action_at(
    viewport: [f32; 2],
    cursor: [f32; 2],
    open: bool,
    listings: &[crate::network::PlayerShopItem],
    own_character_id: Option<u32>,
    has_selected_target: bool,
) -> Option<PlayerShopPanelAction> {
    let [left, top] = quick_panel_position(viewport, QuickPanel::Market);
    let x = cursor[0] - left;
    let y = cursor[1] - top;
    if !(18.0..=502.0).contains(&x) {
        return None;
    }
    if (112.0..140.0).contains(&y) {
        if open {
            return Some(if x < 260.0 {
                PlayerShopPanelAction::Close
            } else {
                PlayerShopPanelAction::AddSelectedItem
            });
        }
        return (x < 260.0).then_some(PlayerShopPanelAction::Open);
    }
    if has_selected_target && (146.0..174.0).contains(&y) && x < 260.0 {
        return Some(PlayerShopPanelAction::QueryTarget);
    }
    for (index, listing) in listings.iter().take(5).enumerate() {
        let row = 190.0 + index as f32 * 26.0;
        if (row..row + 22.0).contains(&y) {
            return Some(if own_character_id == Some(listing.owner_id) {
                PlayerShopPanelAction::Remove(index)
            } else {
                PlayerShopPanelAction::Buy(index)
            });
        }
    }
    None
}

pub fn player_shop_price_at(viewport: [f32; 2], cursor: [f32; 2]) -> bool {
    let [left, top] = quick_panel_position(viewport, QuickPanel::Market);
    (left + 268.0..=left + 420.0).contains(&cursor[0])
        && (top + 76.0..=top + 104.0).contains(&cursor[1])
}

pub fn quick_panel_position(viewport: [f32; 2], panel: QuickPanel) -> [f32; 2] {
    crate::original_panels::position(panel, viewport).unwrap_or([
        ((viewport[0] - QUICK_PANEL_WIDTH as f32) * 0.5).max(8.0),
        ((viewport[1] - QUICK_PANEL_HEIGHT as f32) * 0.5).max(8.0),
    ])
}

pub fn quick_panel_at(viewport: [f32; 2], cursor: [f32; 2], panel: QuickPanel) -> bool {
    if crate::original_panels::is_original(panel) {
        return crate::original_panels::panel_at(panel, viewport, cursor);
    }
    let [left, top] = quick_panel_position(viewport, panel);
    cursor[0] >= left
        && cursor[0] < left + QUICK_PANEL_WIDTH as f32
        && cursor[1] >= top
        && cursor[1] < top + QUICK_PANEL_HEIGHT as f32
}

pub fn quick_panel_close_at(viewport: [f32; 2], cursor: [f32; 2], panel: QuickPanel) -> bool {
    if crate::original_panels::is_original(panel) {
        return crate::original_panels::close_at(panel, viewport, cursor);
    }
    let [left, top] = quick_panel_position(viewport, panel);
    cursor[0] >= left + QUICK_PANEL_WIDTH as f32 - 32.0
        && cursor[0] < left + QUICK_PANEL_WIDTH as f32 - 8.0
        && cursor[1] >= top + 8.0
        && cursor[1] < top + 32.0
}

pub fn team_panel_member_at(
    viewport: [f32; 2],
    cursor: [f32; 2],
    team_members: &[crate::network::TeamMember],
) -> Option<u32> {
    crate::original_panels::team_portrait_at(viewport, cursor)
        .and_then(|index| team_members.get(index))
        .map(|member| member.member_id)
}

pub fn guild_panel_member_at(
    viewport: [f32; 2],
    cursor: [f32; 2],
    guild_members: &[crate::network::GuildMember],
) -> Option<u32> {
    crate::original_panels::guild_member_at(viewport, cursor)
        .and_then(|index| guild_members.get(index))
        .map(|member| member.character_id)
}

pub fn guild_panel_action_at(
    viewport: [f32; 2],
    cursor: [f32; 2],
    has_guild: bool,
    pending_invite: bool,
) -> Option<GuildPanelAction> {
    crate::original_panels::guild_action_at(viewport, cursor, has_guild, pending_invite)
}

pub fn team_panel_action_at(
    viewport: [f32; 2],
    cursor: [f32; 2],
    has_team: bool,
    pending_invite: bool,
    pending_join_request: bool,
) -> Option<TeamPanelAction> {
    crate::original_panels::team_action_at(
        viewport,
        cursor,
        &crate::original_panels::TeamState {
            has_team,
            pending_invite,
            pending_join_request,
            join_enabled: true,
        },
    )
}

pub fn friends_panel_action_at(
    viewport: [f32; 2],
    cursor: [f32; 2],
    friends: &[crate::network::SocialEntry],
    ignores: &[crate::network::SocialEntry],
    pending_request: bool,
) -> Option<FriendsPanelAction> {
    crate::original_panels::friends_action_at(
        viewport,
        cursor,
        friends.len(),
        ignores.len(),
        pending_request,
    )
}

fn team_button(image: &mut RgbaImage, font: &fontdue::Font, x: u32, y: u32, label: &str) {
    let rect = UiRect::new(x as i32, y as i32, 228, 28);
    crate::components::draw_action_button(
        image,
        font,
        rect,
        label,
        crate::components::WidgetState::Normal,
    );
}

fn settings_button(
    image: &mut RgbaImage,
    font: &fontdue::Font,
    x: u32,
    y: u32,
    label: &str,
    active: bool,
) {
    let width = 150;
    let fill = if active {
        [39, 91, 67, 255]
    } else {
        [31, 37, 39, 255]
    };
    for py in y..y + 28 {
        for px in x..x + width {
            image.put_pixel(px, py, Rgba(fill));
        }
    }
    for px in x..x + width {
        image.put_pixel(px, y, Rgba([171, 132, 58, 255]));
        image.put_pixel(px, y + 27, Rgba([171, 132, 58, 255]));
    }
    for py in y..y + 28 {
        image.put_pixel(x, py, Rgba([171, 132, 58, 255]));
        image.put_pixel(x + width - 1, py, Rgba([171, 132, 58, 255]));
    }
    labels::text(
        image,
        font,
        &[(label, [230, 218, 181])],
        [x as f32 + 10.0, y as f32 + 6.0],
    );
}

fn settings_small_button(image: &mut RgbaImage, font: &fontdue::Font, x: u32, y: u32, label: &str) {
    let width = 60;
    for py in y..y + 28 {
        for px in x..x + width {
            image.put_pixel(px, py, Rgba([31, 37, 39, 255]));
        }
    }
    for px in x..x + width {
        image.put_pixel(px, y, Rgba([171, 132, 58, 255]));
        image.put_pixel(px, y + 27, Rgba([171, 132, 58, 255]));
    }
    for py in y..y + 28 {
        image.put_pixel(x, py, Rgba([171, 132, 58, 255]));
        image.put_pixel(x + width - 1, py, Rgba([171, 132, 58, 255]));
    }
    labels::text(
        image,
        font,
        &[(label, [230, 218, 181])],
        [x as f32 + 25.0, y as f32 + 6.0],
    );
}

fn team_health_fraction(health: u16, maximum_health: u16) -> f32 {
    if maximum_health == 0 {
        return 0.0;
    }
    (health as f32 / maximum_health as f32).clamp(0.0, 1.0)
}

fn team_health_bar(
    image: &mut RgbaImage,
    x: u32,
    y: u32,
    width: u32,
    health: u16,
    maximum_health: u16,
) {
    let fraction = team_health_fraction(health, maximum_health);
    for py in y..y + 8 {
        for px in x..x + width {
            image.put_pixel(px, py, Rgba([7, 13, 17, 255]));
        }
    }
    let filled = (width as f32 * fraction).round() as u32;
    let fill = if fraction <= 0.25 {
        [214, 83, 61, 255]
    } else if fraction <= 0.5 {
        [221, 174, 66, 255]
    } else {
        [58, 184, 112, 255]
    };
    for py in y + 1..y + 7 {
        for px in x + 1..x + filled.saturating_sub(1) {
            image.put_pixel(px, py, Rgba(fill));
        }
    }
    for px in x..x + width {
        image.put_pixel(px, y, Rgba([116, 91, 47, 255]));
        image.put_pixel(px, y + 7, Rgba([116, 91, 47, 255]));
    }
}

fn social_button(
    image: &mut RgbaImage,
    font: &fontdue::Font,
    x: u32,
    y: u32,
    width: u32,
    label: &str,
) {
    let rect = UiRect::new(x as i32, y as i32, width as i32, 22);
    crate::components::draw_button(
        image,
        font,
        rect,
        label,
        crate::components::WidgetState::Normal,
    );
}

pub fn quick_panel_image(
    assets: &mut Assets,
    panel: QuickPanel,
    character: &crate::character::Character,
    action_bar: &crate::action_bar::ActionBar,
    known_skills: &HashMap<u32, (u16, u32)>,
    team_members: &[crate::network::TeamMember],
    team_leader_id: Option<u32>,
    pending_invite: Option<u32>,
    pending_join_request: Option<u32>,
    team_join_enabled: bool,
    guild_members: &[crate::network::GuildMember],
    pending_guild_invite: Option<u32>,
    friends: &[crate::network::SocialEntry],
    ignores: &[crate::network::SocialEntry],
    pending_friend_name: Option<&str>,
    selected_target_name: Option<&str>,
    inventory: &[crate::network::OwnedItem],
    trade: &crate::network::TradeView,
    player_shop_items: &[crate::network::PlayerShopItem],
    own_character_id: Option<u32>,
    player_shop_open: bool,
    player_shop_price: &str,
    player_shop_price_focused: bool,
    hud_config: crate::hud::HudConfig,
) -> Result<RgbaImage> {
    if crate::original_panels::is_original(panel) {
        return crate::original_panels::image(
            assets,
            panel,
            &crate::original_panels::PanelData {
                character,
                team_members,
                team_leader_id,
                team: crate::original_panels::TeamState {
                    has_team: !team_members.is_empty(),
                    pending_invite: pending_invite.is_some(),
                    pending_join_request: pending_join_request.is_some(),
                    join_enabled: team_join_enabled,
                },
                guild_members,
                pending_guild_invite,
                friends,
                ignores,
                pending_friend_name,
                selected_target_name,
                trade,
            },
        );
    }
    let mut image =
        RgbaImage::from_pixel(QUICK_PANEL_WIDTH, QUICK_PANEL_HEIGHT, Rgba([0, 0, 0, 0]));
    crate::components::draw_window_rect(
        &mut image,
        UiRect::new(0, 0, QUICK_PANEL_WIDTH as i32, QUICK_PANEL_HEIGHT as i32),
        panel.title(),
        crate::components::WidgetState::Normal,
    )?;
    let font = labels::font()?;
    match panel {
        QuickPanel::Skills => {
            labels::text(
                &mut image,
                &font,
                &[("Habilidades aprendidas", [255, 224, 146])],
                [22.0, 54.0],
            );
            let mut skills = known_skills.iter().collect::<Vec<_>>();
            skills.sort_by_key(|(skill_id, _)| **skill_id);
            if skills.is_empty() {
                labels::text(
                    &mut image,
                    &font,
                    &[("Nenhuma habilidade aprendida", [150, 150, 150])],
                    [24.0, 84.0],
                );
            } else {
                for (index, (skill_id, (level, experience))) in skills.iter().take(8).enumerate() {
                    let row = 84 + (index as u32) * 30;
                    let name = match **skill_id {
                        1_000 => "Basic Fire",
                        1_001 => "Flame Burst",
                        1_002 => "Whirlwind",
                        _ => "Unknown skill",
                    };
                    let line = format!("{name}  Lv.{level}  EXP {experience}");
                    labels::text(
                        &mut image,
                        &font,
                        &[(line.as_str(), [220, 210, 175])],
                        [24.0, row as f32],
                    );
                }
            }
            labels::text(
                &mut image,
                &font,
                &[("Action bar", [255, 224, 146])],
                [24.0, 330.0],
            );
            for (index, slot) in action_bar.slots.iter().take(2).enumerate() {
                let label = slot.action_id.as_deref().unwrap_or("Empty slot");
                let line = format!("F{}  {label}", index + 1);
                labels::text(
                    &mut image,
                    &font,
                    &[(line.as_str(), [170, 164, 143])],
                    [24.0 + index as f32 * 246.0, 350.0],
                );
            }
        }
        QuickPanel::Quest => {
            labels::text(
                &mut image,
                &font,
                &[("No active quests", [220, 210, 175])],
                [24.0, 64.0],
            );
            labels::text(
                &mut image,
                &font,
                &[("Quest progress will appear here.", [150, 150, 150])],
                [24.0, 94.0],
            );
        }
        QuickPanel::Guild => {
            crate::components::draw_card(
                &mut image,
                UiRect::new(20, 50, 480, 96),
                crate::components::WidgetState::Normal,
            );
            labels::text(
                &mut image,
                &font,
                &[("Informações da Guild", ui_theme::TEXT_GOLD)],
                [32.0, 58.0],
            );
            let guild = if character.guild.is_empty() {
                "Sem guild"
            } else {
                character.guild.as_str()
            };
            let rank = if character.guild_rank.is_empty() {
                "Membro"
            } else {
                character.guild_rank.as_str()
            };
            labels::text(
                &mut image,
                &font,
                &[
                    ("Guild:", [220, 210, 175]),
                    (" ", [220, 210, 175]),
                    (guild, [255, 255, 255]),
                ],
                [32.0, 84.0],
            );
            labels::text(
                &mut image,
                &font,
                &[
                    ("Cargo:", [220, 210, 175]),
                    (" ", [220, 210, 175]),
                    (rank, [255, 255, 255]),
                ],
                [32.0, 110.0],
            );

            crate::components::draw_card(
                &mut image,
                UiRect::new(20, 156, 480, 185),
                crate::components::WidgetState::Normal,
            );
            labels::text(
                &mut image,
                &font,
                &[("Membros da Guild", ui_theme::TEXT_GOLD)],
                [32.0, 164.0],
            );
            if guild_members.is_empty() && character.guild.is_empty() {
                if let Some(inviter_id) = pending_guild_invite {
                    labels::text(
                        &mut image,
                        &font,
                        &[(
                            format!("Convite recebido de {inviter_id}").as_str(),
                            [220, 210, 175],
                        )],
                        [32.0, 192.0],
                    );
                    team_button(&mut image, &font, 24, 112, "ACEITAR CONVITE");
                } else {
                    labels::text(
                        &mut image,
                        &font,
                        &[("Você não está em uma guild", [150, 150, 150])],
                        [32.0, 192.0],
                    );
                }
            } else if guild_members.is_empty() {
                labels::text(
                    &mut image,
                    &font,
                    &[("Nenhum membro online no momento", [150, 150, 150])],
                    [32.0, 192.0],
                );
            } else {
                for (index, member) in guild_members.iter().take(8).enumerate() {
                    let member_name = member.name.chars().take(18).collect::<String>();
                    let rank_label = if member.rank == 1000 {
                        "Leader"
                    } else {
                        "Member"
                    };
                    let line = format!("{}  Lv.{} {}", member_name, member.level, rank_label);
                    let status = if member.online { "ONLINE" } else { "OFFLINE" };
                    crate::components::draw_list_item(
                        &mut image,
                        &font,
                        UiRect::new(24, 184 + index as i32 * 21, 472, 20),
                        &line,
                        Some(status),
                        if member.online {
                            crate::components::WidgetState::Normal
                        } else {
                            crate::components::WidgetState::Disabled
                        },
                    );
                }
                team_button(&mut image, &font, 24, 350, "ARMAZÉM");
                team_button(&mut image, &font, 268, 350, "SAIR");
                team_button(&mut image, &font, 24, 382, "CONVIDAR");
                team_button(&mut image, &font, 268, 382, "EXPULSAR");
            }
        }
        QuickPanel::Friends => {
            labels::text(
                &mut image,
                &font,
                &[("Selected target", [255, 224, 146])],
                [24.0, 48.0],
            );
            if let Some(name) = selected_target_name {
                let selected = name.chars().take(25).collect::<String>();
                labels::text(
                    &mut image,
                    &font,
                    &[(selected.as_str(), [235, 225, 190])],
                    [24.0, 67.0],
                );
                social_button(&mut image, &font, 300, 61, 84, "ADD");
                social_button(&mut image, &font, 394, 61, 106, "IGNORE");
            } else {
                labels::text(
                    &mut image,
                    &font,
                    &[("Select a nearby player first", [150, 150, 150])],
                    [24.0, 67.0],
                );
            }
            labels::text(
                &mut image,
                &font,
                &[("Friends", [255, 224, 146])],
                [24.0, 94.0],
            );
            if friends.is_empty() {
                labels::text(
                    &mut image,
                    &font,
                    &[("No friends to display", [150, 150, 150])],
                    [24.0, 110.0],
                );
            } else {
                for (index, entry) in friends.iter().take(6).enumerate() {
                    let row = 110 + index as u32 * 26;
                    let status = if entry.online { "ONLINE" } else { "OFFLINE" };
                    let color = if entry.online {
                        [140, 225, 165]
                    } else {
                        [160, 155, 145]
                    };
                    labels::text(
                        &mut image,
                        &font,
                        &[(entry.name.as_str(), [235, 225, 190])],
                        [24.0, row as f32],
                    );
                    labels::text(&mut image, &font, &[(status, color)], [252.0, row as f32]);
                    social_button(&mut image, &font, 410, row.saturating_sub(1), 90, "REMOVE");
                }
            }
            labels::text(
                &mut image,
                &font,
                &[("Ignored", [255, 224, 146])],
                [24.0, 260.0],
            );
            if ignores.is_empty() {
                labels::text(
                    &mut image,
                    &font,
                    &[("No ignored characters", [150, 150, 150])],
                    [24.0, 286.0],
                );
            } else {
                for (index, entry) in ignores.iter().take(3).enumerate() {
                    let row = 286 + index as u32 * 22;
                    labels::text(
                        &mut image,
                        &font,
                        &[(entry.name.as_str(), [220, 210, 175])],
                        [24.0, row as f32],
                    );
                    social_button(&mut image, &font, 410, row.saturating_sub(1), 90, "REMOVE");
                }
            }
        }
        QuickPanel::Trade => {
            labels::text(
                &mut image,
                &font,
                &[("Troca segura entre jogadores", ui_theme::TEXT_GOLD)],
                [24.0, 48.0],
            );
            if !trade.active {
                crate::components::draw_card(
                    &mut image,
                    UiRect::new(20, 72, 480, 110),
                    crate::components::WidgetState::Normal,
                );
                if let Some(requester) = trade.pending_request {
                    labels::text(
                        &mut image,
                        &font,
                        &[(
                            format!("Convite recebido de {requester}").as_str(),
                            [235, 225, 190],
                        )],
                        [32.0, 84.0],
                    );
                    team_button(&mut image, &font, 24, 112, "ACEITAR TROCA");
                    team_button(&mut image, &font, 24, 146, "RECUSAR");
                } else if selected_target_name.is_some() {
                    labels::text(
                        &mut image,
                        &font,
                        &[(
                            format!("Alvo: {}", selected_target_name.unwrap_or_default()).as_str(),
                            [235, 225, 190],
                        )],
                        [32.0, 84.0],
                    );
                    team_button(&mut image, &font, 24, 112, "SOLICITAR TROCA");
                } else {
                    labels::text(
                        &mut image,
                        &font,
                        &[(
                            "Selecione um jogador próximo para negociar",
                            [150, 150, 150],
                        )],
                        [32.0, 84.0],
                    );
                }
            } else {
                crate::components::draw_card(
                    &mut image,
                    UiRect::new(20, 52, 236, 192),
                    crate::components::WidgetState::Normal,
                );
                crate::components::draw_card(
                    &mut image,
                    UiRect::new(264, 52, 236, 192),
                    crate::components::WidgetState::Normal,
                );
                labels::text(
                    &mut image,
                    &font,
                    &[("Seu lado", ui_theme::TEXT_GOLD)],
                    [28.0, 58.0],
                );
                let peer_label = format!("Outro lado ({})", trade.peer_id.unwrap_or_default());
                labels::text(
                    &mut image,
                    &font,
                    &[(peer_label.as_str(), ui_theme::TEXT_GOLD)],
                    [272.0, 58.0],
                );
                for (index, item) in trade.own_items.iter().take(5).enumerate() {
                    let row = 84 + index as u32 * 28;
                    labels::text(
                        &mut image,
                        &font,
                        &[(
                            format!("{}  +{}", item.item_type, item.plus).as_str(),
                            [220, 210, 175],
                        )],
                        [28.0, row as f32],
                    );
                }
                for (index, item) in trade.peer_items.iter().take(5).enumerate() {
                    let row = 84 + index as u32 * 28;
                    labels::text(
                        &mut image,
                        &font,
                        &[(
                            format!("{}  +{}", item.item_type, item.plus).as_str(),
                            [220, 210, 175],
                        )],
                        [272.0, row as f32],
                    );
                }
                labels::text(
                    &mut image,
                    &font,
                    &[(
                        format!("Silver: {}", trade.own_silver).as_str(),
                        [220, 210, 175],
                    )],
                    [28.0, 224.0],
                );
                labels::text(
                    &mut image,
                    &font,
                    &[(
                        format!("Silver: {}", trade.peer_silver).as_str(),
                        [220, 210, 175],
                    )],
                    [272.0, 224.0],
                );
                team_button(&mut image, &font, 24, 250, "ADICIONAR ITEM");
                team_button(&mut image, &font, 268, 250, "+1 SILVER");
                team_button(
                    &mut image,
                    &font,
                    24,
                    290,
                    if trade.own_confirmed {
                        "CONFIRMADO"
                    } else {
                        "CONFIRMAR"
                    },
                );
                team_button(&mut image, &font, 268, 290, "CANCELAR");
                let other_status = if trade.peer_confirmed {
                    "Status do parceiro: CONFIRMADO"
                } else {
                    "Status do parceiro: Aguardando confirmação"
                };
                crate::components::draw_status_banner(
                    &mut image,
                    &font,
                    UiRect::new(20, 326, 480, 22),
                    other_status,
                    !trade.peer_confirmed,
                );
                let available = inventory
                    .iter()
                    .filter(|item| {
                        !trade
                            .own_items
                            .iter()
                            .any(|offered| offered.uid == item.uid)
                    })
                    .count();
                labels::text(
                    &mut image,
                    &font,
                    &[(
                        format!("Itens disponíveis: {available}").as_str(),
                        [150, 150, 150],
                    )],
                    [24.0, 356.0],
                );
            }
        }
        QuickPanel::Mail => {
            labels::text(
                &mut image,
                &font,
                &[("Mailbox", [255, 224, 146])],
                [24.0, 64.0],
            );
            labels::text(
                &mut image,
                &font,
                &[("Your mailbox is empty", [220, 210, 175])],
                [24.0, 98.0],
            );
        }
        QuickPanel::Market => {
            labels::text(
                &mut image,
                &font,
                &[("Banca de jogadores", [255, 224, 146])],
                [24.0, 64.0],
            );
            let target = selected_target_name.unwrap_or("nenhum alvo");
            labels::text(
                &mut image,
                &font,
                &[(target, [220, 210, 175])],
                [24.0, 84.0],
            );
            labels::text(
                &mut image,
                &font,
                &[("Preço", [255, 224, 146])],
                [268.0, 64.0],
            );
            let price_colour = if player_shop_price_focused {
                [255, 255, 255]
            } else {
                [220, 210, 175]
            };
            labels::text(
                &mut image,
                &font,
                &[(player_shop_price, price_colour)],
                [268.0, 84.0],
            );
            if player_shop_open {
                labels::text(
                    &mut image,
                    &font,
                    &[("Sua banca está aberta", [140, 225, 165])],
                    [24.0, 104.0],
                );
                team_button(&mut image, &font, 24, 112, "FECHAR BANCA");
                team_button(&mut image, &font, 268, 112, "ADICIONAR ITEM");
            } else {
                labels::text(
                    &mut image,
                    &font,
                    &[("Preço padrão: 1000 silver", [170, 164, 143])],
                    [24.0, 104.0],
                );
                team_button(&mut image, &font, 24, 112, "ABRIR BANCA");
            }
            if selected_target_name.is_some() {
                team_button(&mut image, &font, 24, 146, "CONSULTAR ALVO");
            } else {
                labels::text(
                    &mut image,
                    &font,
                    &[(
                        "Selecione um jogador próximo para consultar",
                        [150, 150, 150],
                    )],
                    [24.0, 154.0],
                );
            }
            labels::text(
                &mut image,
                &font,
                &[(
                    "Itens anunciados — clique para comprar/remover",
                    [255, 224, 146],
                )],
                [24.0, 178.0],
            );
            if player_shop_items.is_empty() {
                labels::text(
                    &mut image,
                    &font,
                    &[("Nenhum anúncio carregado", [150, 150, 150])],
                    [24.0, 204.0],
                );
            } else {
                for (index, listing) in player_shop_items.iter().take(5).enumerate() {
                    let row = 190 + index as u32 * 26;
                    let action = if own_character_id == Some(listing.owner_id) {
                        "REMOVER"
                    } else {
                        "COMPRAR"
                    };
                    let line = format!("{}  | {} silver", listing.item.item_type, listing.price);
                    labels::text(
                        &mut image,
                        &font,
                        &[(line.as_str(), [220, 210, 175])],
                        [24.0, row as f32],
                    );
                    labels::text(
                        &mut image,
                        &font,
                        &[(action, [255, 224, 146])],
                        [410.0, row as f32],
                    );
                }
            }
        }
        QuickPanel::Team => {
            labels::text(
                &mut image,
                &font,
                &[("Membros do grupo", [255, 224, 146])],
                [24.0, 64.0],
            );
            if team_members.is_empty() {
                labels::text(
                    &mut image,
                    &font,
                    &[("Você não está em um grupo", [170, 164, 143])],
                    [24.0, 98.0],
                );
                if let Some(leader_id) = pending_invite {
                    labels::text(
                        &mut image,
                        &font,
                        &[(format!("Convite de {leader_id}").as_str(), [220, 210, 175])],
                        [24.0, 110.0],
                    );
                    team_button(&mut image, &font, 24, 112, "ACEITAR CONVITE");
                }
                team_button(&mut image, &font, 24, 146, "CRIAR GRUPO");
                team_button(&mut image, &font, 24, 180, "SOLICITAR ENTRADA");
            } else {
                for (index, member) in team_members.iter().take(5).enumerate() {
                    let row = 84 + index as u32 * 26;
                    let name = if member.name.is_empty() {
                        format!("Character {}", member.member_id)
                    } else {
                        member.name.chars().take(21).collect()
                    };
                    labels::text(
                        &mut image,
                        &font,
                        &[(name.as_str(), [235, 225, 190])],
                        [24.0, row as f32],
                    );
                    if team_leader_id == Some(member.member_id) {
                        labels::text(
                            &mut image,
                            &font,
                            &[("LÍDER", [255, 215, 108])],
                            [176.0, row as f32],
                        );
                    }
                    team_health_bar(
                        &mut image,
                        250,
                        row + 2,
                        154,
                        member.health,
                        member.maximum_health,
                    );
                    let hp = format!(
                        "{}/{}",
                        member.health,
                        member.maximum_health.max(member.health)
                    );
                    labels::text(
                        &mut image,
                        &font,
                        &[(hp.as_str(), [190, 220, 185])],
                        [410.0, row as f32],
                    );
                }
                if let Some(requester_id) = pending_join_request {
                    labels::text(
                        &mut image,
                        &font,
                        &[(
                            format!("Pedido de entrada: {requester_id}").as_str(),
                            [220, 210, 175],
                        )],
                        [24.0, 204.0],
                    );
                    team_button(&mut image, &font, 24, 216, "ACEITAR PEDIDO");
                }
                team_button(&mut image, &font, 24, 246, "CONVIDAR");
                team_button(&mut image, &font, 268, 246, "SAIR");
                team_button(&mut image, &font, 24, 278, "PASSAR LIDERANÇA");
                team_button(&mut image, &font, 268, 278, "EXPULSAR");
                team_button(&mut image, &font, 24, 310, "DISPENSAR");
                team_button(
                    &mut image,
                    &font,
                    268,
                    310,
                    if team_join_enabled {
                        "BLOQUEAR ENTRADA"
                    } else {
                        "LIBERAR ENTRADA"
                    },
                );
            }
        }
        QuickPanel::System => {
            labels::text(
                &mut image,
                &font,
                &[("System information", [255, 224, 146])],
                [24.0, 64.0],
            );
            labels::text(
                &mut image,
                &font,
                &[
                    ("ESC", [255, 224, 146]),
                    (" closes the active panel", [220, 210, 175]),
                ],
                [24.0, 98.0],
            );
            labels::text(
                &mut image,
                &font,
                &[
                    ("F1-F10", [255, 224, 146]),
                    (" activate the action bar", [220, 210, 175]),
                ],
                [24.0, 128.0],
            );
        }
        QuickPanel::Settings => {
            labels::text(
                &mut image,
                &font,
                &[("HUD SETTINGS", [255, 224, 146])],
                [24.0, 64.0],
            );
            labels::text(
                &mut image,
                &font,
                &[("CENTER HUD", [220, 210, 175])],
                [24.0, 98.0],
            );
            settings_button(
                &mut image,
                &font,
                330,
                92,
                if hud_config.center_arcs {
                    "ARCS  ON"
                } else {
                    "ARCS OFF"
                },
                hud_config.center_arcs,
            );
            settings_button(
                &mut image,
                &font,
                330,
                132,
                if hud_config.center_bars {
                    "BARS  ON"
                } else {
                    "BARS OFF"
                },
                hud_config.center_bars,
            );
            labels::text(
                &mut image,
                &font,
                &[("CENTER SCALE", [220, 210, 175])],
                [24.0, 180.0],
            );
            settings_small_button(&mut image, &font, 330, 172, "-");
            labels::text(
                &mut image,
                &font,
                &[(&format!("{}%", hud_config.center_scale), [230, 218, 181])],
                [400.0, 180.0],
            );
            settings_small_button(&mut image, &font, 420, 172, "+");
            labels::text(
                &mut image,
                &font,
                &[("ARC DISTANCE", [220, 210, 175])],
                [24.0, 230.0],
            );
            settings_small_button(&mut image, &font, 330, 222, "-");
            labels::text(
                &mut image,
                &font,
                &[(
                    &format!("{}px", hud_config.center_arc_distance),
                    [230, 218, 181],
                )],
                [400.0, 230.0],
            );
            settings_small_button(&mut image, &font, 420, 222, "+");
            labels::text(
                &mut image,
                &font,
                &[("Arcs and bars appear around the player.", [150, 150, 150])],
                [24.0, 292.0],
            );
        }
    }
    labels::text(
        &mut image,
        &font,
        &[("Click X or press ESC to close", [150, 150, 150])],
        [24.0, 340.0],
    );
    Ok(image)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StatusTab {
    Equipment,
    Attributes,
    Weapon,
    Skills,
    // Kept as data states for the already implemented server features. They
    // are not part of the native 5065 Character button strip.
    Title,
    Reputation,
    Fashion,
}

impl StatusTab {
    pub const ALL: [StatusTab; 4] = [
        StatusTab::Attributes,
        StatusTab::Equipment,
        StatusTab::Weapon,
        StatusTab::Skills,
    ];

    pub fn label(self) -> &'static str {
        match self {
            StatusTab::Equipment => "Equipamentos",
            StatusTab::Attributes => "Atributos",
            StatusTab::Weapon => "Armas",
            StatusTab::Skills => "Skills",
            StatusTab::Title => "Título",
            StatusTab::Reputation => "Reputação",
            StatusTab::Fashion => "Fashion",
        }
    }
}

/// The Weapon tab's three pages: one-handed (4xx), two-handed (5xx) and
/// the rest (shields, boxing...).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum WeaponFilter {
    #[default]
    OneHand,
    TwoHand,
    Others,
}

impl WeaponFilter {
    pub const ALL: [WeaponFilter; 3] = [Self::OneHand, Self::TwoHand, Self::Others];

    pub fn of(weapon_type: u32) -> Self {
        match weapon_type / 100 {
            4 => Self::OneHand,
            5 => Self::TwoHand,
            _ => Self::Others,
        }
    }

    /// `Control.Ani` `[Button90..92]`: normal and pressed frames.
    fn frames(self) -> [&'static str; 2] {
        match self {
            Self::OneHand => ["data/main/WeaponBtnS.dds", "data/main/WeaponBtnSClick.dds"],
            Self::TwoHand => ["data/main/WeaponBtnD.dds", "data/main/WeaponBtnDClick.dds"],
            Self::Others => ["data/main/WeaponBtnO.dds", "data/main/WeaponBtnOClick.dds"],
        }
    }

    /// GUI.ini `[134-1519..1521]`, relative to the paper.
    fn rect(self) -> UiRect {
        let x = match self {
            Self::OneHand => 4,
            Self::TwoHand => 70,
            Self::Others => 136,
        };
        UiRect::new(StatusLayout::PAPER_X + x, StatusLayout::PAPER_Y + 297, 64, 21)
    }
}

pub fn status_weapon_filter_at(viewport: [f32; 2], cursor: [f32; 2]) -> Option<WeaponFilter> {
    let [left, top] = status_position(viewport);
    let local = [cursor[0] - left, cursor[1] - top];
    WeaponFilter::ALL
        .into_iter()
        .find(|filter| filter.rect().contains_local(local))
}

pub fn status_tab_at(viewport: [f32; 2], cursor: [f32; 2]) -> Option<StatusTab> {
    let [left, top] = status_position(viewport);
    for (i, tab) in StatusTab::ALL.iter().enumerate() {
        if StatusLayout::tab_rect(i).contains_local([cursor[0] - left, cursor[1] - top]) {
            return Some(*tab);
        }
    }
    None
}

pub fn status_close_at(viewport: [f32; 2], cursor: [f32; 2]) -> bool {
    let [left, top] = status_position(viewport);
    let local = [cursor[0] - left, cursor[1] - top];
    StatusLayout::CLOSE_RECTS
        .into_iter()
        .any(|(x, y, width, height)| UiRect::new(x, y, width, height).contains_local(local))
}

pub fn status_panel_at(viewport: [f32; 2], cursor: [f32; 2]) -> bool {
    let [left, top] = status_position(viewport);
    cursor[0] >= left
        && cursor[0] < left + STATUS_WIDTH as f32
        && cursor[1] >= top
        && cursor[1] < top + STATUS_STANDARD_HEIGHT as f32
}

/// Returns the base attribute represented by a `+` button in the Attributes
/// tab. The server still validates the available points; this is only the
/// client hitbox for the visible action.
pub fn status_attribute_button_at(viewport: [f32; 2], cursor: [f32; 2]) -> Option<&'static str> {
    let [left, top] = status_position(viewport);
    // GUI.ini [132-1562..1565]: these are the native plus-button hitboxes
    // inside DialogQuery_Role, which is overlaid at (40, 55).
    let role_left = left + StatusLayout::PAPER_X as f32;
    let role_top = top + StatusLayout::PAPER_Y as f32;
    for (attribute, x, y) in [
        ("strength", 81.0, 199.0),
        ("agility", 175.0, 199.0),
        ("vitality", 81.0, 230.0),
        ("spirit", 175.0, 230.0),
    ] {
        if cursor[0] >= role_left + x
            && cursor[0] < role_left + x + 19.0
            && cursor[1] >= role_top + y
            && cursor[1] < role_top + y + 19.0
        {
            return Some(attribute);
        }
    }
    None
}

pub fn status_reputation_donate_at(viewport: [f32; 2], cursor: [f32; 2]) -> bool {
    let [left, top] = status_position(viewport);
    let rect = StatusLayout::reputation_button_rect();
    rect.contains_local([cursor[0] - left, cursor[1] - top])
}

/// Returns the title catalog entry under the cursor. Title `0` clears the
/// equipped title; the server remains authoritative for unlock requirements.
pub fn status_title_at(viewport: [f32; 2], cursor: [f32; 2]) -> Option<u32> {
    let [left, top] = status_position(viewport);
    for (index, title_id) in [0_u32, 1, 2, 3, 4, 5].into_iter().enumerate() {
        if StatusLayout::title_button_rect(index)
            .contains_local([cursor[0] - left, cursor[1] - top])
        {
            return Some(title_id);
        }
    }
    None
}

fn generated_panel_surface(name: &str, width: u32, height: u32) -> Result<RgbaImage> {
    let bytes: &[u8] = match name {
        "inventory" => include_bytes!("hud_assets/inventory_panel_generated.png"),
        "character" => include_bytes!("hud_assets/character_panel_generated.png"),
        _ => return Err(format!("unknown generated panel asset: {name}").into()),
    };
    let source = image::load_from_memory(bytes)?.into_rgba8();
    Ok(imageops::resize(
        &source,
        width.max(1),
        height.max(1),
        imageops::FilterType::Lanczos3,
    ))
}

/// Composes the Character window from the installed 5065 components. The
/// frame and slot art come from the client; only live character values are
/// painted by the game.
fn original_character_image(
    assets: &mut Assets,
    level: u8,
    nobility_rank: u32,
    character: &crate::character::Character,
    equipment: &[crate::network::OwnedItem],
    hidden_default_slots: &[bool; 10],
    active_tab: StatusTab,
    close_hovered: bool,
) -> Result<RgbaImage> {
    let (surface_width, surface_height) = status_surface_size(active_tab);
    let mut image = RgbaImage::from_pixel(surface_width, surface_height, Rgba([0, 0, 0, 0]));
    let shell =
        crate::original_ui::component(assets, crate::original_ui::CharacterComponent::Shell)?;
    // The player Character window is the shell plus the native Mission
    // paper-doll panel. DialogNpcEquip is a separate inspect-NPC dialog;
    // using it here removes the Character tabs and shifts every slot.
    let source = shell.clone();
    imageops::overlay(&mut image, &source, 0, 0);

    match active_tab {
        StatusTab::Attributes => {
            let role = crate::original_ui::component(
                assets,
                crate::original_ui::CharacterComponent::Role,
            )?;
            imageops::overlay(&mut image, &role, 40, 55);
            // GUI.ini [132-1150] and [132-1151]. Keep the native Base/Details
            // buttons visible below the original role panel.
            imageops::overlay(
                &mut image,
                &crate::original_ui::role_button(assets, false)?,
                64,
                337,
            );
            imageops::overlay(
                &mut image,
                &crate::original_ui::role_button(assets, true)?,
                159,
                337,
            );
        }
        StatusTab::Weapon => {
            let weapon = crate::original_ui::component(
                assets,
                crate::original_ui::CharacterComponent::Weapon,
            )?;
            imageops::overlay(&mut image, &weapon, 40, 55);
            // GUI.ini [134-1522..1527] reserves six native weapon rows.
            // Keep the original row artwork and add only live item icons.
            for (index, item) in equipment
                .iter()
                .filter(|item| item.position == 4 || item.position == 5)
                .take(6)
                .enumerate()
            {
                if let Ok(icon) = item_icon_image(assets, item.item_type) {
                    imageops::overlay(
                        &mut image,
                        &icon,
                        59_i64,
                        124_i64 + i64::from(index as i32 * 38),
                    );
                }
            }
        }
        StatusTab::Skills => {
            let mission = crate::original_ui::component(
                assets,
                crate::original_ui::CharacterComponent::Mission,
            )?;
            imageops::overlay(&mut image, &mission, 40, 55);
        }
        StatusTab::Equipment => {
            let equipment_panel = crate::original_ui::component(
                assets,
                crate::original_ui::CharacterComponent::Mission,
            )?;
            imageops::overlay(
                &mut image,
                &equipment_panel,
                i64::from(StatusLayout::PAPER_X),
                i64::from(StatusLayout::PAPER_Y),
            );
        }
        StatusTab::Title | StatusTab::Reputation | StatusTab::Fashion => {}
    }
    imageops::overlay(
        &mut image,
        &crate::original_ui::close_button(assets, close_hovered)?,
        239,
        38,
    );

    let font = labels::font()?;
    let value_colour = [235, 225, 193];
    let name = if character.name.is_empty() {
        "Character"
    } else {
        character.name.as_str()
    };
    let guild = if character.guild.is_empty() {
        "None"
    } else {
        character.guild.as_str()
    };
    match active_tab {
        StatusTab::Equipment => {
            // Equipment.dds belongs to the inspect-NPC dialog and contains
            // Name/Spouse controls. The player Character equipment view uses
            // the shell header and the native Mission slot panel instead.
            let rank = nobility_rank_label(nobility_rank);
            labels::text(
                &mut image,
                &font,
                &[(rank, [255, 224, 146])],
                [centered_text_x(&font, rank, 128.0), 43.0],
            );
            let _ = (name, equipment, hidden_default_slots);
        }
        StatusTab::Attributes => {
            let identity = [
                name.to_owned(),
                level.to_string(),
                class_name(character.current_class).to_owned(),
                "0".to_owned(),
                "None".to_owned(),
                guild.to_owned(),
                if character.guild_rank.is_empty() {
                    "None".to_owned()
                } else {
                    character.guild_rank.clone()
                },
            ];
            // GUI.ini [132-1130..1136] positions the value controls relative
            // to DialogQuery_Role, which is composited at (40, 55).
            for (text, y) in identity
                .iter()
                .zip([63.0, 91.0, 117.0, 143.0, 169.0, 195.0, 221.0])
            {
                labels::text_sized(
                    &mut image,
                    &font,
                    &[(text.as_str(), value_colour)],
                    [112.0, y],
                    8.5,
                );
            }
            for (text, x, y) in [
                (character.strength.to_string(), 128.0, 257.0),
                (character.agility.to_string(), 188.0, 254.0),
                (character.vitality.to_string(), 128.0, 276.0),
                (character.spirit.to_string(), 188.0, 284.0),
                (character.attribute_points.to_string(), 142.0, 314.0),
            ] {
                labels::text_sized(
                    &mut image,
                    &font,
                    &[(text.as_str(), value_colour)],
                    [x, y],
                    8.5,
                );
            }
        }
        StatusTab::Weapon | StatusTab::Skills => {}
        StatusTab::Title => {
            labels::text(
                &mut image,
                &font,
                &[("Titles", [255, 224, 146])],
                [58.0, 78.0],
            );
            labels::text(
                &mut image,
                &font,
                &[("Use the original title controls", value_colour)],
                [58.0, 110.0],
            );
        }
        StatusTab::Reputation => {
            labels::text(
                &mut image,
                &font,
                &[("Reputation", [255, 224, 146])],
                [58.0, 78.0],
            );
            labels::text(
                &mut image,
                &font,
                &[("Donation and ranking", value_colour)],
                [58.0, 110.0],
            );
        }
        StatusTab::Fashion => {
            labels::text(
                &mut image,
                &font,
                &[("Fashion", [255, 224, 146])],
                [58.0, 78.0],
            );
        }
    }
    Ok(image)
}

fn new_panel_surface(width: u32, height: u32) -> RgbaImage {
    let mut image = RgbaImage::from_pixel(width, height, Rgba([8, 20, 29, 248]));
    for x in 0..width {
        image.put_pixel(x, 0, Rgba([225, 179, 82, 255]));
        image.put_pixel(x, height - 1, Rgba([102, 66, 25, 255]));
    }
    for y in 0..height {
        image.put_pixel(0, y, Rgba([225, 179, 82, 255]));
        image.put_pixel(width - 1, y, Rgba([102, 66, 25, 255]));
    }
    for x in 3..width.saturating_sub(3) {
        image.put_pixel(x, 3, Rgba([72, 48, 24, 255]));
        image.put_pixel(x, height.saturating_sub(4), Rgba([231, 187, 91, 220]));
    }
    for y in 3..height.saturating_sub(3) {
        image.put_pixel(3, y, Rgba([72, 48, 24, 255]));
        image.put_pixel(width.saturating_sub(4), y, Rgba([231, 187, 91, 220]));
    }
    for x in 10..width.saturating_sub(10) {
        image.put_pixel(x, 43, Rgba([76, 52, 26, 255]));
    }
    image
}

pub fn status_image(
    assets: &mut Assets,
    level: u8,
    experience: u64,
    character: &crate::character::Character,
    equipment: &[crate::network::OwnedItem],
    hidden_default_slots: &[bool; 10],
    active_tab: StatusTab,
    mana: u32,
    maximum_mana: u32,
    close_hovered: bool,
    nobility_donation: u64,
    nobility_rank: u32,
    nobility_remaining: u64,
    equipped_title: u32,
    nobility_ranking: &[crate::network::NobilityEntry],
    reputation_donate_hovered: bool,
    title_hovered: Option<u32>,
) -> Result<RgbaImage> {
    return original_character_image(
        assets,
        level,
        nobility_rank,
        character,
        equipment,
        hidden_default_slots,
        active_tab,
        close_hovered,
    );

    #[allow(unreachable_code)]
    let mut image = generated_panel_surface("character", STATUS_WIDTH, STATUS_HEIGHT)?;
    // The generated frame has transparent cut-outs in the content area. Fill
    // the paper-doll interior first so the world cannot bleed through below
    // the character while preserving the ornamental frame around it.
    status_fill(
        &mut image,
        StatusLayout::PAPER_X + 4,
        StatusLayout::PAPER_Y + 4,
        StatusLayout::PAPER_WIDTH - 8,
        StatusLayout::PAPER_HEIGHT - 8,
        [8, 20, 29, 255],
    );
    let font = labels::font()?;
    let decorative_font = labels::decorative_font()?;
    labels::text_sized(
        &mut image,
        &decorative_font,
        &[("CHARACTER", [255, 224, 146])],
        [
            centered_text_x(&decorative_font, "CHARACTER", STATUS_WIDTH as f32 * 0.5),
            43.0,
        ],
        13.0,
    );
    draw_close_button(&mut image, &font, close_hovered);
    for (i, tab) in StatusTab::ALL.iter().enumerate() {
        let is_active = *tab == active_tab;
        let rect = StatusLayout::tab_rect(i);
        draw_sidebar_tab(
            &mut image,
            &decorative_font,
            rect.x as u32,
            rect.y as u32,
            tab.label(),
            is_active,
        );
    }
    let content_top = StatusLayout::PAPER_Y;
    let display_equipment = status_equipment_items(character, equipment, hidden_default_slots);
    let equipment_stats = equipment_bonuses(assets, &display_equipment);
    let battle_power = derived_battle_power(
        level,
        character,
        (equipment_stats.0, equipment_stats.1, equipment_stats.2),
    );
    match active_tab {
        StatusTab::Equipment => {
            draw_equipment_tab(
                &mut image,
                &font,
                &decorative_font,
                assets,
                level,
                character,
                equipment,
                hidden_default_slots,
                mana,
                maximum_mana,
                battle_power,
            )?;
        }
        StatusTab::Attributes => draw_attributes_tab(
            &mut image,
            &font,
            &decorative_font,
            level,
            experience,
            character,
            mana,
            maximum_mana,
            content_top,
            battle_power,
        ),
        StatusTab::Weapon | StatusTab::Skills => {}
        StatusTab::Title => {
            draw_title_tab(
                &mut image,
                &font,
                &decorative_font,
                equipped_title,
                nobility_rank,
                title_hovered,
            );
        }
        StatusTab::Reputation => {
            draw_reputation_tab(
                &mut image,
                &font,
                &decorative_font,
                nobility_donation,
                nobility_remaining,
                nobility_ranking,
                reputation_donate_hovered,
            );
        }
        StatusTab::Fashion => draw_simple_status_tab(
            &mut image,
            &font,
            &decorative_font,
            "Fashion",
            "Nenhum visual equipado",
        ),
    }
    Ok(image)
}

/// Static layer for the original Character window. The atlas components and
/// close button are independent from live data and can remain resident.
/// Side tab textures (`Control.Ani` `[Check61]`, `[Check64]`, `[Check63]`,
/// `[Check65]`); frame 1 is the selected, gold state.
fn status_tab_frames(tab: StatusTab) -> [&'static str; 2] {
    match tab {
        StatusTab::Equipment => [
            "data/main/QueryMissionBtn.dds",
            "data/main/QueryMissionBtnClick.dds",
        ],
        StatusTab::Weapon => [
            "data/main/QueryWeaponBtn.dds",
            "data/main/QueryWeaponBtnClick.dds",
        ],
        StatusTab::Skills => [
            "data/main/QuerySkillBtn.dds",
            "data/main/QuerySkillBtnClick.dds",
        ],
        _ => [
            "data/main/QueryRoleBtn.dds",
            "data/main/QueryRoleBtnClick.dds",
        ],
    }
}

/// Character window frame: `DialogQuery` with the active child dialog and
/// its native controls, all at their `GUI.ini` `[131-*]`..`[135-*]` offsets.
pub fn status_base_image(
    assets: &mut Assets,
    active_tab: StatusTab,
    weapon_filter: WeaponFilter,
    close_hovered: bool,
) -> Result<RgbaImage> {
    use crate::original_ui::{CharacterComponent, overlay_asset};
    let (width, height) = status_surface_size(active_tab);
    let mut image = RgbaImage::from_pixel(width, height, Rgba([0, 0, 0, 0]));
    let shell = crate::original_ui::component(assets, CharacterComponent::Shell)?;
    imageops::overlay(&mut image, &shell, 0, 0);
    draw_battle_power_label(&mut image)?;
    let child = match active_tab {
        StatusTab::Equipment => CharacterComponent::Mission,
        StatusTab::Weapon | StatusTab::Skills => CharacterComponent::Weapon,
        _ => CharacterComponent::Role,
    };
    let child = crate::original_ui::component(assets, child)?;
    let (paper_x, paper_y) = (
        i64::from(StatusLayout::PAPER_X),
        i64::from(StatusLayout::PAPER_Y),
    );
    imageops::overlay(&mut image, &child, paper_x, paper_y);
    if active_tab == StatusTab::Equipment {
        // `[Image160]`: the left-hand weapon frame is a separate texture
        // around the 65x65 control [135-1675].
        overlay_asset(
            &mut image,
            assets,
            "data/main/DialogQuery_Mission1.dds",
            paper_x + 54,
            paper_y + 248,
        )?;
    }

    for (index, tab) in StatusTab::ALL.iter().enumerate() {
        let rect = StatusLayout::tab_rect(index);
        let frame = status_tab_frames(*tab)[usize::from(*tab == active_tab)];
        overlay_asset(&mut image, assets, frame, rect.x.into(), rect.y.into())?;
    }
    let close = if close_hovered {
        "data/main/CloseBtnSclick.dds"
    } else {
        "data/main/CloseBtnS.dds"
    };
    let [close_x, close_y] = StatusLayout::CLOSE_ORIGIN;
    overlay_asset(&mut image, assets, close, close_x.into(), close_y.into())?;
    overlay_asset(&mut image, assets, "data/main/HelpBtn2.dds", 75, 386)?;
    overlay_asset(&mut image, assets, "data/main/CloseBtn2.dds", 168, 386)?;

    let controls: &[(&str, i64, i64)] = match active_tab {
        // [132-1150], [132-1151] and the four [132-156x] plus buttons.
        StatusTab::Attributes => &[
            ("data/main/RoleBtnBase.dds", 24, 282),
            ("data/main/RoleBtnMore.dds", 119, 282),
            ("data/main/RoleBtnI.dds", 81, 199),
            ("data/main/RoleBtnI.dds", 175, 199),
            ("data/main/RoleBtnI.dds", 81, 230),
            ("data/main/RoleBtnI.dds", 175, 230),
        ],
        // [134-1519..1521] and the [134-1063/1064] page buttons.
        StatusTab::Weapon => &[
            ("data/main/QuerySkillBtnU.dds", 183, 230),
            ("data/main/QuerySkillBtnD.dds", 183, 264),
        ],
        StatusTab::Skills => &[
            ("data/main/QuerySkillBtnZ.dds", 4, 297),
            ("data/main/QuerySkillBtnM.dds", 70, 297),
            ("data/main/QuerySkillBtnJ.dds", 136, 297),
            ("data/main/QuerySkillBtnU.dds", 183, 230),
            ("data/main/QuerySkillBtnD.dds", 183, 264),
        ],
        _ => &[],
    };
    for (path, x, y) in controls {
        overlay_asset(&mut image, assets, path, paper_x + x, paper_y + y)?;
    }
    if active_tab == StatusTab::Weapon {
        for filter in WeaponFilter::ALL {
            let rect = filter.rect();
            let frame = filter.frames()[usize::from(filter == weapon_filter)];
            overlay_asset(&mut image, assets, frame, rect.x.into(), rect.y.into())?;
        }
    }
    Ok(image)
}

/// The 5065 shell bakes "Battle Power" into its plate with a blurry serif;
/// the client paints the plate clean and writes the label with the system
/// font. Repaint the plate interior with its vertical gradient and redraw.
fn draw_battle_power_label(image: &mut RgbaImage) -> Result<()> {
    const PLATE: [u32; 4] = [58, 28, 71, 12];
    const TOP: [f32; 3] = [18.0, 36.0, 46.0];
    const BOTTOM: [f32; 3] = [58.0, 84.0, 98.0];
    let [x0, y0, width, height] = PLATE;
    for row in 0..height {
        let t = row as f32 / (height - 1) as f32;
        let colour = std::array::from_fn::<u8, 3, _>(|i| (TOP[i] + (BOTTOM[i] - TOP[i]) * t) as u8);
        for x in x0..x0 + width {
            image.put_pixel(x, y0 + row, Rgba([colour[0], colour[1], colour[2], 255]));
        }
    }
    let font = crate::main_hud::client_font()?;
    let label = "Battle Power";
    let text_width = crate::main_hud::text_width(font, label, 12.0);
    crate::main_hud::crisp_text(
        image,
        font,
        label,
        12.0,
        [(x0 + width / 2) as i32 - text_width / 2, y0 as i32],
        [240, 232, 205, 255],
    );
    Ok(())
}

/// Transparent live-data layer for Character. Only values change here; the
/// original frame, tabs, and native panels never need to be regenerated.
pub fn status_dynamic_image(
    assets: &mut Assets,
    level: u8,
    _experience: u64,
    nobility_rank: u32,
    character: &crate::character::Character,
    active_tab: StatusTab,
    _mana: u32,
    _maximum_mana: u32,
    known_skills: &[(u32, u16, u32)],
    equipment: &[crate::network::OwnedItem],
    weapon_skills: &[(u32, u32, u32)],
    weapon_filter: WeaponFilter,
    selected_skill: Option<u32>,
) -> Result<RgbaImage> {
    let (width, height) = status_surface_size(active_tab);
    let mut image = RgbaImage::from_pixel(width, height, Rgba([0, 0, 0, 0]));
    let font = crate::main_hud::font()?;
    const WHITE: [u8; 4] = [255, 255, 255, 255];
    let paper = [StatusLayout::PAPER_X, StatusLayout::PAPER_Y];
    let value = |image: &mut RgbaImage, text: &str, x: i32, y: i32| {
        crate::main_hud::text(
            image,
            font,
            text,
            12.0,
            [paper[0] + x, paper[1] + y + 1],
            WHITE,
            Some(122),
        );
    };

    // The orb beside "Battle Power" shows the battle power (potency), in
    // white italic with a soft blue glow, spilling over the orb like the
    // original.
    let power = battle_power(level, character.rebirths, equipment, nobility_rank).to_string();
    let power_font = crate::main_hud::italic_font().unwrap_or(font);
    let size = if power.len() > 2 { 24.0 } else { 30.0 };
    let power_width = crate::main_hud::text_width(power_font, &power, size);
    crate::main_hud::glow_text(
        &mut image,
        power_font,
        &power,
        size,
        [28 - power_width / 2, 27 - (size * 0.62) as i32],
        [90, 104, 234],
    );

    let or_none = |text: &str| {
        if text.is_empty() {
            "None".to_owned()
        } else {
            text.to_owned()
        }
    };
    match active_tab {
        StatusTab::Attributes => {
            // [132-1130..1136]: name, level, job, PK, spouse, guild, position.
            for (text, y) in [
                (character.name.clone(), 8),
                (level.to_string(), 36),
                (class_name(character.current_class), 62),
                (character.kill_points.to_string(), 88),
                (or_none(&character.spouse), 114),
                (or_none(&character.guild), 140),
                (character.guild_rank.clone(), 166),
            ] {
                value(&mut image, &text, 72, y);
            }
            // [132-1137], [132-1566..1569].
            for (number, x, y) in [
                (character.strength, 53, 199),
                (character.agility, 148, 199),
                (character.vitality, 53, 229),
                (character.spirit, 148, 229),
                (character.attribute_points, 102, 259),
            ] {
                value(&mut image, &number.to_string(), x, y);
            }
        }
        StatusTab::Equipment => {
            // [135-1619]: nobility rank above the character model.
            let rank = nobility_rank_label(nobility_rank);
            let rank_width = crate::main_hud::text_width(font, rank, 13.0);
            crate::main_hud::text(
                &mut image,
                font,
                rank,
                13.0,
                [paper[0] + 82 - rank_width / 2, paper[1] + 6],
                [255, 236, 150, 255],
                None,
            );
        }
        StatusTab::Weapon => {
            let mut rows = weapon_skills
                .iter()
                .copied()
                .filter(|(weapon_type, _, _)| WeaponFilter::of(*weapon_type) == weapon_filter)
                .collect::<Vec<_>>();
            rows.sort_unstable_by_key(|(weapon_type, _, _)| *weapon_type);
            let equipped = equipment
                .iter()
                .find(|item| item.position == 4)
                .map(|item| item.item_type / 1_000);
            let selected = rows
                .iter()
                .find(|(weapon_type, _, _)| Some(*weapon_type) == equipped)
                .or(rows.first())
                .copied();
            if let Some((weapon_type, level, experience)) = selected {
                let name = weapon_type_name(assets, weapon_type);
                let needed = weapon_level_experience(level);
                let lines = [
                    format!("{name}: Level {level}"),
                    if needed == 0 {
                        "EXP: Max".to_owned()
                    } else {
                        format!("EXP: {experience}/{needed}")
                    },
                    format!("Attack +{}%", weapon_attack_bonus(level, experience)),
                ];
                // Inside the description box at the top of the paper.
                for (index, line) in lines.iter().enumerate() {
                    let y = paper[1] + 6 + index as i32 * 17;
                    crate::main_hud::text(&mut image, font, line, 12.0, [paper[0] + 24, y], WHITE, Some(180));
                }
            }
            for (row, &(weapon_type, level, _)) in rows.iter().take(STATUS_LIST_ROWS).enumerate() {
                let y = paper[1] + 67 + row as i32 * 38;
                if let Some(icon) = weapon_type_icon(assets, weapon_type) {
                    draw_list_icon(&mut image, &icon, [paper[0] + 17, y]);
                }
                draw_level_stars(&mut image, assets, level, [paper[0] + 82, y + 18]);
            }
        }
        StatusTab::Skills => {
            let selected = known_skills
                .iter()
                .find(|(id, _, _)| Some(*id) == selected_skill)
                .or(known_skills.first())
                .copied();
            if let Some((skill_id, level, experience)) = selected {
                let kind = crate::magic::magic_type(assets, skill_id, level)
                    .map(|magic| magic.kind.clone())
                    .filter(|kind| !kind.is_empty())
                    .unwrap_or_else(|| format!("Level {level}  Exp {experience}"));
                let name = crate::magic::skill_name(assets, skill_id);
                for (index, line) in [name, kind].iter().enumerate() {
                    let y = paper[1] + 8 + index as i32 * 18;
                    crate::main_hud::text(&mut image, font, line, 12.0, [paper[0] + 12, y], WHITE, Some(180));
                }
            }
            // [134-1522..1526]/[134-1165] icons and the level boxes beside.
            for (row, &(skill_id, skill_level, _)) in
                known_skills.iter().take(STATUS_LIST_ROWS).enumerate()
            {
                let y = paper[1] + 67 + row as i32 * 38;
                if let Some(icon) = crate::main_hud::skill_icon(assets, skill_id) {
                    draw_list_icon(&mut image, &icon, [paper[0] + 17, y]);
                }
                draw_level_stars(
                    &mut image,
                    assets,
                    u32::from(skill_level),
                    [paper[0] + 82, y + 18],
                );
            }
        }
        _ => {}
    }
    Ok(image)
}

const STATUS_LIST_ROWS: usize = 6;

/// Mirrors the server: two points per level above the first, plus up to
/// four from progress inside the level, capped at 20%.
fn weapon_attack_bonus(level: u32, experience: u32) -> u32 {
    (level.saturating_sub(1) * 2 + experience.min(100) / 25).min(20)
}

/// The server's (and Redux's) proficiency needed to leave `level`; 0 at
/// the top level.
fn weapon_level_experience(level: u32) -> u32 {
    const TABLE: [u32; 21] = [
        0, 1_200, 68_000, 250_000, 640_000, 1_600_000, 4_000_000, 10_000_000, 22_000_000,
        40_000_000, 90_000_000, 95_000_000, 142_500_000, 213_750_000, 320_625_000, 480_937_500,
        721_406_250, 1_082_109_375, 1_623_164_063, 2_100_000_000, 0,
    ];
    TABLE.get(level as usize).copied().unwrap_or(0)
}

/// `ini/WeaponSkillName.ini`: "410,Blade" lines.
fn weapon_type_name(assets: &mut Assets, weapon_type: u32) -> String {
    assets
        .read("ini/WeaponSkillName.ini")
        .ok()
        .and_then(|bytes| {
            String::from_utf8_lossy(&bytes).lines().find_map(|line| {
                let (id, name) = line.split_once(',')?;
                (id.trim().parse::<u32>().ok()? == weapon_type).then(|| name.trim().to_owned())
            })
        })
        .unwrap_or_else(|| format!("Weapon {weapon_type}"))
}

/// `ani/Control.Ani` `[Weapon{type}]`: the weapon type icon.
fn weapon_type_icon(assets: &mut Assets, weapon_type: u32) -> Option<RgbaImage> {
    let ani = assets.read("ani/Control.Ani").ok()?;
    let ani = String::from_utf8_lossy(&ani);
    let path = animation_frame(&ani, &format!("[Weapon{weapon_type}]"))?.to_owned();
    crate::original_ui::visible_part(&assets.image(&path).ok()?).ok()
}

/// Icon centred in a 37x37 list box ([134-1522..]).
fn draw_list_icon(image: &mut RgbaImage, icon: &RgbaImage, origin: [i32; 2]) {
    let icon = imageops::resize(icon, 33, 33, imageops::FilterType::Triangle);
    imageops::overlay(image, &icon, i64::from(origin[0] + 2), i64::from(origin[1] + 2));
}

/// Level as the original stars: a gold mark per 20 levels
/// (`[Weaponlevel20]`), silver per 5 (`[Weaponlevel5]`), copper per level
/// (`[Weaponlevel1]`), vertically centred on `centre[1]`.
fn draw_level_stars(image: &mut RgbaImage, assets: &mut Assets, level: u32, centre: [i32; 2]) {
    let marks = [
        ("data/main/LevelGold.dds", level / 20),
        ("data/main/LevelSiller.dds", level % 20 / 5),
        ("data/main/LevelCopper.dds", level % 5),
    ];
    let mut x = centre[0];
    for (path, count) in marks {
        let Some(mark) = assets
            .image(path)
            .ok()
            .and_then(|mark| crate::original_ui::visible_part(&mark).ok())
        else {
            continue;
        };
        for _ in 0..count {
            let y = centre[1] - mark.height() as i32 / 2;
            imageops::overlay(image, &mark, i64::from(x), i64::from(y));
            x += mark.width() as i32 + 1;
        }
    }
}

/// Battle power ("Potency" in the 5065 art): level, five per reborn, the
/// nobility bonus and, for each worn item except the garment, its quality
/// above Normal, its plus and one per gem (two for a super gem). A
/// two-handed weapon without an off-hand item counts twice.
pub fn battle_power(
    level: u8,
    rebirths: u8,
    equipment: &[crate::network::OwnedItem],
    nobility_rank: u32,
) -> u32 {
    let nobility = match nobility_rank {
        12 => 5,
        9 => 4,
        7 => 3,
        5 => 2,
        3 => 1,
        _ => 0,
    };
    let item_power = |item: &crate::network::OwnedItem| {
        let quality = (item.item_type % 10).saturating_sub(5);
        let gems = [item.gem_one, item.gem_two]
            .into_iter()
            .filter(|gem| *gem != 0 && *gem != 255)
            .map(|gem| if gem % 10 == 3 { 2 } else { 1 })
            .sum::<u32>();
        quality + u32::from(item.plus) + gems
    };
    let has_off_hand = equipment.iter().any(|item| item.position == 5);
    let items = equipment
        .iter()
        .filter(|item| (1..=8).contains(&item.position))
        .map(|item| {
            let two_handed = item.position == 4 && (500..600).contains(&(item.item_type / 1_000));
            item_power(item) * if two_handed && !has_off_hand { 2 } else { 1 }
        })
        .sum::<u32>();
    u32::from(level) + u32::from(rebirths) * 5 + nobility + items
}

fn draw_reputation_tab(
    image: &mut RgbaImage,
    font: &fontdue::Font,
    title_font: &fontdue::Font,
    donation: u64,
    remaining: u64,
    ranking: &[crate::network::NobilityEntry],
    donate_hovered: bool,
) {
    let x = StatusLayout::INFO_X;
    let y = StatusLayout::BASIC_Y;
    let width = StatusLayout::INFO_WIDTH;
    let height = StatusLayout::BASIC_HEIGHT + StatusLayout::COMBAT_HEIGHT + 5;
    draw_info_card(image, font, title_font, x, y, width, height, "Reputação");
    labels::text_sized(
        image,
        font,
        &[(format!("Doação: {donation}").as_str(), [235, 225, 193])],
        [x as f32 + 7.0, (y + 43) as f32],
        10.0,
    );
    labels::text_sized(
        image,
        font,
        &[(format!("Próximo: {remaining}").as_str(), [235, 225, 193])],
        [x as f32 + 7.0, (y + 61) as f32],
        10.0,
    );
    labels::text_sized(
        image,
        title_font,
        &[("Ranking", [255, 224, 146])],
        [x as f32 + 7.0, (y + 84) as f32],
        10.0,
    );
    if ranking.is_empty() {
        labels::text_sized(
            image,
            font,
            &[("Sem registros", [170, 164, 143])],
            [x as f32 + 7.0, (y + 104) as f32],
            9.0,
        );
    } else {
        for entry in ranking.iter().take(4) {
            let name: String = entry.name.chars().take(11).collect();
            let line = format!("#{} {} {}", entry.position, name, entry.donation);
            let line_y = y + 104 + ((entry.position.saturating_sub(1).min(3)) as i32 * 18);
            labels::text_sized(
                image,
                font,
                &[(line.as_str(), [190, 184, 162])],
                [x as f32 + 7.0, line_y as f32],
                8.5,
            );
        }
    }
    draw_status_action_button(
        image,
        font,
        StatusLayout::reputation_button_rect(),
        "DOAR 3M",
        donate_hovered,
    );
}

fn draw_status_action_button(
    image: &mut RgbaImage,
    font: &fontdue::Font,
    rect: UiRect,
    label: &str,
    hovered: bool,
) {
    ui::draw_text_button(image, font, rect, label, hovered, false);
}

fn draw_sidebar_tab(
    image: &mut RgbaImage,
    font: &fontdue::Font,
    x: u32,
    y: u32,
    label: &str,
    active: bool,
) {
    // The frame's corners and side ornaments reduce the usable area. Fit the
    // actual rendered label inside that inner area, rather than centering it
    // against the nominal tab width.
    let inner_left = 14.0;
    let inner_right = 14.0;
    let max_label_width = STATUS_TAB_WIDTH as f32 - inner_left - inner_right;
    let label_width = |size: f32| {
        label
            .chars()
            .map(|character| font.metrics(character, size).advance_width)
            .sum::<f32>()
    };
    let mut label_size = 12.0;
    while label_size > 7.5 && label_width(label_size) > max_label_width {
        label_size -= 0.25;
    }
    let rendered_width = label_width(label_size);
    let label_x = x as f32 + inner_left + (max_label_width - rendered_width) * 0.5;
    labels::text_sized(
        image,
        font,
        &[(
            label,
            if active {
                [255, 232, 160]
            } else {
                [204, 190, 153]
            },
        )],
        [label_x, y as f32 + 15.0],
        label_size,
    );
}

fn draw_tab(
    image: &mut RgbaImage,
    font: &fontdue::Font,
    x: u32,
    y: u32,
    label: &str,
    active: bool,
) {
    let bg_color = if active {
        [92, 65, 29, 255]
    } else {
        [12, 28, 38, 255]
    };
    let border_color = if active {
        [246, 211, 132, 255]
    } else {
        [113, 82, 38, 255]
    };
    let text_color = if active {
        [255, 232, 160]
    } else {
        [204, 190, 153]
    };
    for tx in x..x + StatusLayout::TAB_WIDTH as u32 {
        for ty in y..y + StatusLayout::TAB_HEIGHT as u32 {
            image.put_pixel(tx, ty, Rgba(bg_color));
        }
    }
    for tx in x..x + StatusLayout::TAB_WIDTH as u32 {
        image.put_pixel(tx, y, Rgba(border_color));
        image.put_pixel(
            tx,
            y + StatusLayout::TAB_HEIGHT as u32 - 1,
            Rgba(border_color),
        );
    }
    for ty in y..y + StatusLayout::TAB_HEIGHT as u32 {
        image.put_pixel(x, ty, Rgba(border_color));
        image.put_pixel(
            x + StatusLayout::TAB_WIDTH as u32 - 1,
            ty,
            Rgba(border_color),
        );
    }
    if active {
        for tx in x + 2..x + StatusLayout::TAB_WIDTH as u32 - 2 {
            image.put_pixel(
                tx,
                y + StatusLayout::TAB_HEIGHT as u32 - 1,
                Rgba([8, 20, 29, 248]),
            );
        }
    }
    // The ornamental corners and side jewels occupy part of the nominal tab
    // width, so center labels inside the quiet inner panel instead of using
    // the full 96 px rectangle.
    let inner_left = 13.0;
    let inner_right = 13.0;
    let max_label_width = StatusLayout::TAB_WIDTH as f32 - inner_left - inner_right;
    let label_width = |size: f32| {
        label
            .chars()
            .map(|character| font.metrics(character, size).advance_width)
            .sum::<f32>()
    };
    let mut label_size = 11.0;
    while label_size > 8.0 && label_width(label_size) > max_label_width {
        label_size -= 0.25;
    }
    let label_width = label_width(label_size);
    let label_x = x as f32 + inner_left + (max_label_width - label_width) * 0.5;
    labels::text_sized(
        image,
        font,
        &[(label, text_color)],
        [label_x, y as f32 + 6.0],
        label_size,
    );
}

fn draw_close_button(image: &mut RgbaImage, font: &fontdue::Font, hovered: bool) {
    // The generated Character frame already contains the large red close
    // medallion.  Do not paint a second rectangular X over it: that was the
    // small stray button visible above the panel in the previous layout.
    // The generated frame owns the red medallion and its hover-safe artwork.
    // Do not add a second circle on top of it.
    let _ = (font, hovered, image);
}

fn draw_info_card(
    image: &mut RgbaImage,
    _font: &fontdue::Font,
    title_font: &fontdue::Font,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    title: &str,
) {
    // Keep content on a quiet, translucent surface. The generated frame is
    // deliberately ornate, so a controlled inner card makes text readable
    // without fighting the decorative gold separators underneath it.
    status_fill(image, x, y, width, height, [5, 14, 21, 255]);
    status_line(image, [x, y], [x + width - 1, y], [205, 160, 70, 230]);
    status_line(
        image,
        [x, y + height - 1],
        [x + width - 1, y + height - 1],
        [102, 66, 28, 220],
    );
    status_line(image, [x, y], [x, y + height - 1], [154, 111, 48, 210]);
    status_line(
        image,
        [x + width - 1, y],
        [x + width - 1, y + height - 1],
        [154, 111, 48, 210],
    );
    labels::text_sized(
        image,
        title_font,
        &[(title, [255, 224, 146])],
        [x as f32 + 7.0, y as f32 + 8.0],
        11.5,
    );
    status_line(
        image,
        [x + 7, y + 27],
        [x + width - 8, y + 27],
        [111, 77, 33, 210],
    );
}

fn draw_attributes_tab(
    image: &mut RgbaImage,
    font: &fontdue::Font,
    title_font: &fontdue::Font,
    level: u8,
    experience: u64,
    character: &crate::character::Character,
    mana: u32,
    maximum_mana: u32,
    _content_top: i32,
    battle_power: u32,
) {
    let x = StatusLayout::INFO_X;
    let y = StatusLayout::BASIC_Y;
    let width = StatusLayout::INFO_WIDTH;
    draw_info_card(image, font, title_font, x, y, width, 333, "Attributes");
    let name = if character.name.is_empty() {
        "Character"
    } else {
        character.name.as_str()
    };
    let identity = [
        format!("Name: {name}"),
        format!("Level: {level}"),
        format!("HP: {} / {}", character.health, character.maximum_health),
        format!("MP: {mana} / {maximum_mana}"),
    ];
    for (index, line) in identity.iter().enumerate() {
        labels::text(
            image,
            font,
            &[(line.as_str(), [235, 225, 193])],
            [x as f32 + 7.0, (y + 41 + index as i32 * 20) as f32],
        );
    }
    labels::text(
        image,
        font,
        &[("Core attributes", [255, 224, 146])],
        [x as f32 + 7.0, (y + 128) as f32],
    );
    let attribute_lines = [
        format!("Strength: {}", character.strength),
        format!("Agility: {}", character.agility),
        format!("Vitality: {}", character.vitality),
        format!("Spirit: {}", character.spirit),
        format!("Points: {}", character.attribute_points),
    ];
    for (index, line) in attribute_lines.iter().enumerate() {
        let line_y = y + 151 + index as i32 * 20;
        labels::text(
            image,
            font,
            &[(line.as_str(), [220, 210, 175])],
            [x as f32 + 7.0, line_y as f32],
        );
        if index < 4 {
            draw_attribute_plus_button(
                image,
                font,
                x + 112,
                line_y - 11,
                character.attribute_points > 0,
            );
        }
    }
    /*
    labels::text(
        image,
        font,
        &[
            ("Nome:", [220, 210, 175]),
            (&character.name, [255, 255, 255]),
        ],
        [x as f32, y as f32],
    );
    y += 28;
    labels::text(
        image,
        font,
        &[
            ("Nível:", [220, 210, 175]),
            (&level.to_string(), [255, 255, 255]),
        ],
        [x as f32, y as f32],
    );
    y += 28;
    labels::text(
        image,
        font,
        &[
            ("HP:", [220, 210, 175]),
            (
                &format!("{} / {}", character.health, character.maximum_health),
                [255, 255, 255],
            ),
        ],
        [x as f32, y as f32],
    );
    y += 28;
    labels::text(
        image,
        font,
        &[
            ("MP:", [220, 210, 175]),
            (&format!("{} / {}", mana, maximum_mana), [255, 255, 255]),
        ],
        [x as f32, y as f32],
    );
    y += 36;
    let attribute_lines = [
        format!("Força: {}", character.strength),
        format!("Agilidade: {}", character.agility),
        format!("Vitalidade: {}", character.vitality),
        format!("Espírito: {}", character.spirit),
        format!("Pontos: {}", character.attribute_points),
    ];
    for line in &attribute_lines {
        labels::text(
            image,
            font,
            &[(line.as_str(), [220, 210, 175])],
            [x as f32, y as f32],
        );
        y += 23;
    }
    y += 13;*/
    let next = u64::from(level.max(1)) * 100;
    let previous = u64::from(level.saturating_sub(1)) * 100;
    let pct = if next > previous {
        ((experience.saturating_sub(previous)) as f32 / (next - previous) as f32).clamp(0.0, 1.0)
    } else {
        0.0
    };
    labels::text(
        image,
        font,
        &[("Experiência", [255, 224, 146])],
        [x as f32 + 7.0, (y + 262) as f32],
    );
    let bar_x = x + 7;
    let bar_y = y + 284;
    let experience_width = (width - 14).max(1) as u32;
    let experience_fill = ((experience_width.saturating_sub(2) as f32) * pct) as u32;
    status_fill(
        image,
        bar_x,
        bar_y,
        experience_width as i32,
        12,
        [4, 12, 18, 255],
    );
    status_fill(
        image,
        bar_x + 1,
        bar_y + 2,
        experience_fill as i32,
        8,
        [235, 190, 52, 255],
    );
    labels::text(
        image,
        font,
        &[(
            format!("{:.1}% próximo nível", pct * 100.0).as_str(),
            [255, 255, 220],
        )],
        [x as f32 + 7.0, (y + 300) as f32],
    );
    draw_footer(image, font, character, battle_power);
}

fn draw_attribute_plus_button(
    image: &mut RgbaImage,
    font: &fontdue::Font,
    x: i32,
    y: i32,
    enabled: bool,
) {
    let fill = if enabled {
        [31, 73, 77, 245]
    } else {
        [14, 28, 34, 220]
    };
    let border = if enabled {
        [213, 163, 58, 255]
    } else {
        [82, 69, 44, 210]
    };
    status_fill(image, x, y, 20, 18, fill);
    rect_outline(image, x as u32, y as u32, 20, 18, Rgba(border));
    labels::text_sized(
        image,
        font,
        &[(
            "+",
            if enabled {
                [255, 224, 146]
            } else {
                [104, 97, 77]
            },
        )],
        [x as f32 + 6.0, y as f32 + 2.0],
        12.0,
    );
}

/// Profession names from the original `ini/ProfessionalName.ini`.
fn class_name(class_id: u8) -> String {
    match class_id {
        10 => "InternTrojan",
        11 => "Trojan",
        12 => "VeteranTrojan",
        13 => "TigerTrojan",
        14 => "DragonTrojan",
        15 => "TrojanMaster",
        20 => "InternWarrior",
        21 => "Warrior",
        22 => "BrassWarrior",
        23 => "SilverWarrior",
        24 => "GoldWarrior",
        25 => "WarriorMaster",
        40 => "InternArcher",
        41 => "Archer",
        42 => "EagleArcher",
        43 => "TigerArcher",
        44 => "DragonArcher",
        45 => "ArcherMaster",
        100 => "InternTaoist",
        101 => "Taoist",
        112 => "MetalTaoist",
        113 => "MetalWizard",
        114 => "MetalMaster",
        115 => "MetalSaint",
        122 => "WoodTaoist",
        123 => "WoodWizard",
        124 => "WoodMaster",
        125 => "WoodSaint",
        132 => "WaterTaoist",
        133 => "WaterWizard",
        134 => "WaterMaster",
        135 => "WaterSaint",
        142 => "FireTaoist",
        143 => "FireWizard",
        144 => "FireMaster",
        145 => "FireSaint",
        152 => "EarthTaoist",
        153 => "EarthWizard",
        154 => "EarthMaster",
        155 => "EarthSaint",
        190 => "InternTaoist",
        191 => "Taosit",
        192 => "GeneralTaoist",
        193 => "Wizard",
        194 => "Master",
        195 => "Saint",
        30 => "Knight",
        _ => return "Unknown".to_owned(),
    }
    .to_owned()
}

fn status_pixel(image: &mut RgbaImage, x: i32, y: i32, color: [u8; 4]) {
    if x >= 0 && y >= 0 && x < image.width() as i32 && y < image.height() as i32 {
        image.put_pixel(x as u32, y as u32, Rgba(color));
    }
}

fn status_line(image: &mut RgbaImage, a: [i32; 2], b: [i32; 2], color: [u8; 4]) {
    let (mut x, mut y) = (a[0], a[1]);
    let (dx, dy) = ((b[0] - x).abs(), -(b[1] - y).abs());
    let (sx, sy) = (if x < b[0] { 1 } else { -1 }, if y < b[1] { 1 } else { -1 });
    let mut error = dx + dy;
    loop {
        status_pixel(image, x, y, color);
        if x == b[0] && y == b[1] {
            break;
        }
        let twice = error * 2;
        if twice >= dy {
            error += dy;
            x += sx;
        }
        if twice <= dx {
            error += dx;
            y += sy;
        }
    }
}

fn status_fill(image: &mut RgbaImage, x: i32, y: i32, width: i32, height: i32, color: [u8; 4]) {
    for py in y..y + height {
        for px in x..x + width {
            status_pixel(image, px, py, color);
        }
    }
}

fn status_ellipse(image: &mut RgbaImage, cx: i32, cy: i32, rx: i32, ry: i32, color: [u8; 4]) {
    for y in -ry..=ry {
        let ratio = 1.0 - (y * y) as f32 / (ry * ry).max(1) as f32;
        let half = (rx as f32 * ratio.max(0.0).sqrt()) as i32;
        for x in -half..=half {
            status_pixel(image, cx + x, cy + y, color);
        }
    }
}

fn status_slot(image: &mut RgbaImage, x: i32, y: i32, position: u8, occupied: bool, hovered: bool) {
    status_fill(image, x, y, 38, 38, [5, 13, 19, 232]);
    status_line(image, [x + 3, y], [x + 34, y], [224, 174, 73, 255]);
    status_line(image, [x + 3, y + 37], [x + 34, y + 37], [114, 70, 26, 255]);
    status_line(image, [x, y + 3], [x, y + 34], [224, 174, 73, 255]);
    status_line(image, [x + 37, y + 3], [x + 37, y + 34], [114, 70, 26, 255]);
    status_line(image, [x + 5, y + 5], [x + 32, y + 5], [74, 53, 29, 255]);
    if !occupied {
        if let Some(mut icon) = equipment_slot_placeholder_image(position) {
            // Keep the category readable while making it unmistakably an
            // empty slot. Real equipment icons are drawn later at full
            // opacity and are therefore visually distinct.
            for pixel in icon.pixels_mut() {
                pixel.0[0] = (u16::from(pixel.0[0]) * 60 / 100) as u8;
                pixel.0[1] = (u16::from(pixel.0[1]) * 65 / 100) as u8;
                pixel.0[2] = (u16::from(pixel.0[2]) * 70 / 100) as u8;
                pixel.0[3] = (u16::from(pixel.0[3]) * 72 / 100) as u8;
            }
            imageops::overlay(image, &icon, i64::from(x + 3), i64::from(y + 3));
        } else {
            let color = [93, 108, 112, 190];
            status_line(image, [x + 11, y + 19], [x + 27, y + 19], color);
            status_line(image, [x + 19, y + 11], [x + 19, y + 27], color);
        }
    }
    if hovered {
        rect_outline(
            image,
            (x - 1).max(0) as u32,
            (y - 1).max(0) as u32,
            40,
            40,
            Rgba([255, 236, 154, 255]),
        );
    }
}

fn quality_outline(image: &mut RgbaImage, rect: UiRect, rarity_code: u8) {
    // Common items keep the plain slot of the original client; only the
    // custom rarity tiers are called out.
    let rarity = ItemRarity::from_custom_code(rarity_code);
    if rarity == ItemRarity::Common {
        return;
    }
    let colour = rarity.colour();
    let colour = [colour[0], colour[1], colour[2], 255];
    ui_theme::outline(image, rect, colour);
    if rect.width > 5 && rect.height > 5 {
        ui_theme::outline(
            image,
            UiRect::new(rect.x + 2, rect.y + 2, rect.width - 4, rect.height - 4),
            [colour[0], colour[1], colour[2], 150],
        );
    }
}

fn equipment_slot_placeholder_image(position: u8) -> Option<RgbaImage> {
    let bytes: &[u8] = match position {
        1 => include_bytes!("ui_assets/equipment_placeholders/slot_1.png"),
        2 => include_bytes!("ui_assets/equipment_placeholders/slot_2.png"),
        3 => include_bytes!("ui_assets/equipment_placeholders/slot_3.png"),
        4 => include_bytes!("ui_assets/equipment_placeholders/slot_4.png"),
        5 => include_bytes!("ui_assets/equipment_placeholders/slot_5.png"),
        6 => include_bytes!("ui_assets/equipment_placeholders/slot_6.png"),
        7 => include_bytes!("ui_assets/equipment_placeholders/slot_7.png"),
        8 => include_bytes!("ui_assets/equipment_placeholders/slot_8.png"),
        9 => include_bytes!("ui_assets/equipment_placeholders/slot_9.png"),
        _ => return None,
    };
    image::load_from_memory(bytes)
        .ok()
        .map(|image| image.into_rgba8())
}

fn draw_character_preview(
    image: &mut RgbaImage,
    _font: &fontdue::Font,
    _character: &crate::character::Character,
) -> Result<()> {
    status_ellipse(image, 224, 446, 61, 10, [22, 28, 31, 255]);
    Ok(())
}

fn draw_equipment_tab(
    image: &mut RgbaImage,
    font: &fontdue::Font,
    title_font: &fontdue::Font,
    assets: &mut Assets,
    level: u8,
    character: &crate::character::Character,
    equipment: &[crate::network::OwnedItem],
    hidden_default_slots: &[bool; 10],
    mana: u32,
    maximum_mana: u32,
    battle_power: u32,
) -> Result<()> {
    /* legacy coordinate-heavy implementation retained below while the new
       layout is defined in one place.
    let display_equipment = status_equipment_items(character, equipment, hidden_default_slots);
    let slot_y = [128, 179, 230, 281, 332];
    let slot_positions = [1_u8, 2, 3, 6, 8, 4, 5, 7, 9, 0];
    for (index, y) in slot_y.into_iter().enumerate() {
        let left_position = slot_positions[index];
        let right_position = slot_positions[index + 5];
        status_slot(
            image,
            font,
            119,
            y,
            display_equipment
                .iter()
                .any(|item| item.position == left_position),
            false,
        );
        status_slot(
            image,
            font,
            291,
            y,
            display_equipment
                .iter()
                .any(|item| item.position == right_position),
            false,
        );
    }
    for (slot, position) in slot_positions.into_iter().enumerate() {
        let item_id = display_equipment
            .iter()
            .find(|item| item.position == position)
            .map(|item| item.item_type)
            .unwrap_or_default();
        if item_id == 0 {
            continue;
        }
        let (x, y) = if slot < 5 {
            (119, slot_y[slot])
        } else {
            (291, slot_y[slot - 5])
        };
        if let Ok(icon) = item_icon_image(assets, item_id) {
            imageops::overlay(image, &icon, i64::from(x + 3), i64::from(y + 3));
        }
    }
    draw_character_preview(image, font, character)?;

    labels::text(
        image,
        font,
        &[("Basic Attributes", [255, 224, 146])],
        [360.0, 112.0],
    );
    let name = if character.name.is_empty() {
        "Character"
    } else {
        &character.name
    };
    let guild = if character.guild.is_empty() {
        "None"
    } else {
        &character.guild
    };
    let name_line = format!("Name: {name}");
    let class_line = format!("Class: {}", class_name(character.current_class));
    let level_line = format!("Level: {level}");
    let guild_line = format!("Guild: {guild}");
    labels::text(
        image,
        font,
        &[(name_line.as_str(), [235, 225, 193])],
        [360.0, 138.0],
    );
    labels::text(
        image,
        font,
        &[(class_line.as_str(), [235, 225, 193])],
        [360.0, 157.0],
    );
    labels::text(
        image,
        font,
        &[(level_line.as_str(), [235, 225, 193])],
        [360.0, 176.0],
    );
    labels::text(
        image,
        font,
        &[(guild_line.as_str(), [235, 225, 193])],
        [360.0, 195.0],
    );
    let hp_line = format!("HP: {} / {}", character.health, character.maximum_health);
    labels::text(
        image,
        font,
        &[(hp_line.as_str(), [220, 210, 175])],
        [360.0, 225.0],
    );
    status_fill(image, 398, 219, 102, 10, [10, 18, 23, 255]);
    let hp = (102.0 * character.health as f32 / character.maximum_health.max(1) as f32) as i32;
    status_fill(image, 399, 220, hp.max(1), 8, [186, 46, 53, 255]);
    let mp_line = format!("MP: {mana} / {maximum_mana}");
    labels::text(
        image,
        font,
        &[(mp_line.as_str(), [220, 210, 175])],
        [360.0, 243.0],
    );
    status_fill(image, 398, 237, 102, 10, [10, 18, 23, 255]);
    let mp = (102.0 * mana as f32 / maximum_mana.max(mana).max(1) as f32) as i32;
    status_fill(image, 399, 238, mp.max(1), 8, [49, 121, 208, 255]);
    labels::text(image, font, &[("Stamina", [220, 210, 175])], [360.0, 261.0]);
    status_fill(image, 398, 255, 102, 10, [10, 18, 23, 255]);
    status_fill(image, 399, 256, 102, 8, [44, 162, 104, 255]);
    let strength = u32::from(character.strength);
    let agility = u32::from(character.agility);
    let vitality = u32::from(character.vitality);
    let attack_min = u32::from(level.max(1)) + strength * 2 + agility / 2;
    let attack_max = u32::from(level.max(1)) * 2 + strength * 3 + agility;
    let defense = vitality + agility / 2;
    let stat_lines = [
        format!("Attack: {attack_min} - {attack_max}"),
        format!("Defense: {defense}"),
        format!("Strength: {}", character.strength),
        format!("Agility: {}", character.agility),
        format!("Vitality: {}", character.vitality),
        format!("Spirit: {}", character.spirit),
    ];
    // These are the centers of the actual decorative cards in the generated
    // frame: [278..312], [312..335], [335..359], [359..382],
    // [382..406] and [406..429]. Keep each label in its own card.
    for (y, line) in [295.0, 324.0, 347.0, 371.0, 394.0, 418.0]
        .into_iter()
        .zip(stat_lines.iter())
    {
        labels::text(image, font, &[(line.as_str(), [230, 220, 188])], [360.0, y]);
    }
    labels::text(
        image,
        font,
        &[("Battle Power 356,820", [255, 224, 146])],
        [164.0, 527.0],
    );
    labels::text(image, font, &[("Details", [255, 224, 146])], [237.0, 548.0]);
    */

    let display_equipment = status_equipment_items(character, equipment, hidden_default_slots);
    for slot in 0..StatusLayout::SLOT_POSITIONS.len() {
        let position = StatusLayout::slot_position(slot);
        let rect = StatusLayout::slot_rect(slot);
        let occupied = display_equipment
            .iter()
            .any(|item| item.position == position);
        status_slot(image, rect.x, rect.y, position, occupied, false);
        if let Some(item) = display_equipment
            .iter()
            .find(|item| item.position == position)
            && item.item_type != 0
        {
            quality_outline(image, rect, item.rarity);
            if let Ok(icon) = item_icon_image(assets, item.item_type) {
                imageops::overlay(image, &icon, i64::from(rect.x + 3), i64::from(rect.y + 3));
            }
        }
    }
    draw_character_preview(image, font, character)?;

    let info_x = StatusLayout::INFO_X;
    draw_info_card(
        image,
        font,
        title_font,
        info_x,
        StatusLayout::BASIC_Y,
        StatusLayout::INFO_WIDTH,
        StatusLayout::BASIC_HEIGHT,
        "Basic Attributes",
    );
    let name = if character.name.is_empty() {
        "Character"
    } else {
        character.name.as_str()
    };
    let guild = if character.guild.is_empty() {
        "None"
    } else {
        character.guild.as_str()
    };
    let identity = [
        format!("Name: {name}"),
        format!("Class: {}", class_name(character.current_class)),
        format!("Level: {level}"),
        format!("Guild: {guild}"),
    ];
    for (index, line) in identity.iter().enumerate() {
        labels::text(
            image,
            font,
            &[(line.as_str(), [235, 225, 193])],
            [
                info_x as f32 + 7.0,
                (StatusLayout::BASIC_Y + 35 + index as i32 * 18) as f32,
            ],
        );
    }
    let hp_line = format!("HP: {} / {}", character.health, character.maximum_health);
    labels::text(
        image,
        font,
        &[(hp_line.as_str(), [220, 210, 175])],
        [info_x as f32 + 7.0, 208.0],
    );
    draw_resource_bar(
        image,
        info_x + 7,
        220,
        128,
        character.health,
        character.maximum_health,
        [186, 46, 53, 255],
    );
    let mp_line = format!("MP: {mana} / {maximum_mana}");
    labels::text(
        image,
        font,
        &[(mp_line.as_str(), [220, 210, 175])],
        [info_x as f32 + 7.0, 226.0],
    );
    draw_resource_bar(
        image,
        info_x + 7,
        238,
        128,
        mana,
        maximum_mana,
        [49, 121, 208, 255],
    );
    labels::text(
        image,
        font,
        &[("Stamina", [220, 210, 175])],
        [info_x as f32 + 7.0, 244.0],
    );
    draw_resource_bar(image, info_x + 7, 256, 128, 100, 100, [44, 162, 104, 255]);

    draw_info_card(
        image,
        font,
        title_font,
        info_x,
        StatusLayout::COMBAT_Y,
        StatusLayout::INFO_WIDTH,
        StatusLayout::COMBAT_HEIGHT,
        "Combat Attributes",
    );
    let strength = u32::from(character.strength);
    let agility = u32::from(character.agility);
    let vitality = u32::from(character.vitality);
    let (equipment_attack_min, equipment_attack_max, equipment_defense, _, _) =
        equipment_bonuses(assets, &display_equipment);
    let stat_lines = [
        format!(
            "Attack: {} - {}",
            u32::from(level.max(1)) + strength * 2 + agility / 2 + equipment_attack_min,
            u32::from(level.max(1)) * 2 + strength * 3 + agility + equipment_attack_max
        ),
        format!("Defense: {}", vitality + agility / 2 + equipment_defense),
        format!("Strength: {}", character.strength),
        format!("Agility: {}", character.agility),
        format!("Vitality: {}", character.vitality),
        format!("Spirit: {}", character.spirit),
    ];
    for (index, line) in stat_lines.iter().enumerate() {
        labels::text(
            image,
            font,
            &[(line.as_str(), [230, 220, 188])],
            [
                info_x as f32 + 7.0,
                (StatusLayout::COMBAT_Y + 39 + index as i32 * 21) as f32,
            ],
        );
    }
    draw_footer(image, font, character, battle_power);
    Ok(())
}

fn draw_resource_bar(
    image: &mut RgbaImage,
    x: i32,
    y: i32,
    width: i32,
    value: u32,
    maximum: u32,
    color: [u8; 4],
) {
    let width = width.max(4);
    status_fill(image, x, y, width, 10, [8, 16, 22, 255]);
    let fraction = value as f32 / maximum.max(1) as f32;
    let fill = ((width - 2) as f32 * fraction.clamp(0.0, 1.0)).round() as i32;
    if fill > 0 {
        status_fill(image, x + 1, y + 1, fill, 8, color);
    }
}

fn equipment_bonuses(
    assets: &mut Assets,
    equipment: &[crate::network::OwnedItem],
) -> (u32, u32, u32, u32, u32) {
    let item_ids = equipment
        .iter()
        .filter(|item| item.max_durability > 0 && item.durability > 0)
        .map(|item| item.item_type)
        .collect::<Vec<_>>();
    let infos = load_item_infos(assets, &item_ids);
    equipment
        .iter()
        .filter(|item| item.max_durability > 0 && item.durability > 0)
        .fold(
            (0, 0, 0, 0, 0),
            |(min_attack, max_attack, defense, health, mana), item| {
                let Some(info) = infos.get(&item.item_type) else {
                    return (min_attack, max_attack, defense, health, mana);
                };
                let scale = item_stat_scale(item.plus, item.rarity);
                let scaled = |value: u32| value.saturating_mul(scale) / 100;
                (
                    min_attack.saturating_add(scaled(info.attack_min)),
                    max_attack.saturating_add(scaled(info.attack_max)),
                    defense.saturating_add(scaled(info.defense)),
                    health.saturating_add(scaled(info.health_add)),
                    mana.saturating_add(scaled(info.mana_add)),
                )
            },
        )
}

fn derived_battle_power(
    level: u8,
    character: &crate::character::Character,
    bonuses: (u32, u32, u32),
) -> u32 {
    let strength = u32::from(character.strength);
    let agility = u32::from(character.agility);
    let vitality = u32::from(character.vitality);
    let spirit = u32::from(character.spirit);
    let base_min_attack = u32::from(level.max(1)) + strength * 2 + agility / 2;
    let base_max_attack = u32::from(level.max(1)) * 2 + strength * 3 + agility;
    let attack_min = base_min_attack.saturating_add(bonuses.0);
    let attack_max = base_max_attack.saturating_add(bonuses.1);
    let defense = vitality + agility / 2 + bonuses.2;
    level.max(1) as u32 * 10
        + strength * 2
        + agility * 2
        + vitality * 2
        + spirit * 2
        + attack_min
        + attack_max
        + defense
}

fn draw_footer(
    image: &mut RgbaImage,
    font: &fontdue::Font,
    character: &crate::character::Character,
    battle_power: u32,
) {
    let x = StatusLayout::FOOTER_X;
    let y = StatusLayout::FOOTER_Y;
    status_fill(
        image,
        x,
        y,
        StatusLayout::FOOTER_WIDTH,
        StatusLayout::FOOTER_HEIGHT,
        [5, 14, 21, 255],
    );
    status_line(
        image,
        [x, y],
        [x + StatusLayout::FOOTER_WIDTH - 1, y],
        [205, 160, 70, 230],
    );
    status_line(
        image,
        [x, y + StatusLayout::FOOTER_HEIGHT - 1],
        [
            x + StatusLayout::FOOTER_WIDTH - 1,
            y + StatusLayout::FOOTER_HEIGHT - 1,
        ],
        [102, 66, 28, 220],
    );
    let name = if character.name.is_empty() {
        "Character"
    } else {
        character.name.as_str()
    };
    labels::text(
        image,
        font,
        &[(name, [248, 211, 124])],
        [
            centered_text_x(font, name, (x + StatusLayout::FOOTER_WIDTH / 2) as f32),
            (y + 7) as f32,
        ],
    );
    let battle_power = format!("Battle Power {battle_power}");
    labels::text(
        image,
        font,
        &[(battle_power.as_str(), [255, 224, 146])],
        [
            centered_text_x(
                font,
                battle_power.as_str(),
                (x + StatusLayout::FOOTER_WIDTH / 2) as f32,
            ),
            (y + 32) as f32,
        ],
    );
    labels::text(
        image,
        font,
        &[("Details", [255, 224, 146])],
        [
            centered_text_x(font, "Details", (x + StatusLayout::FOOTER_WIDTH / 2) as f32),
            (y + 51) as f32,
        ],
    );
}

pub fn status_equipment_overlay_image(
    assets: &mut Assets,
    equipment: &[crate::network::OwnedItem],
    hovered: Option<u8>,
) -> Result<RgbaImage> {
    let (width, height) = status_surface_size(StatusTab::Equipment);
    let mut image = RgbaImage::from_pixel(width, height, Rgba([0, 0, 0, 0]));
    for slot in 0..StatusLayout::SLOT_POSITIONS.len() {
        let rect = StatusLayout::slot_rect(slot);
        if rect.width <= 0 || rect.height <= 0 {
            continue;
        }
        let absolute = UiRect::new(
            StatusLayout::PAPER_X + rect.x,
            StatusLayout::PAPER_Y + rect.y,
            rect.width,
            rect.height,
        );
        let position = StatusLayout::slot_position(slot);
        let item = equipment.iter().find(|item| item.position == position);
        // DialogQuery_Mission already paints the slot frames; only the
        // hover state and the items are drawn here.
        if hovered == Some(position) {
            for x in absolute.x..absolute.x + absolute.width {
                for y in [absolute.y, absolute.y + absolute.height - 1] {
                    status_pixel(&mut image, x, y, [255, 230, 150, 220]);
                }
            }
            for y in absolute.y..absolute.y + absolute.height {
                for x in [absolute.x, absolute.x + absolute.width - 1] {
                    status_pixel(&mut image, x, y, [255, 230, 150, 220]);
                }
            }
        }
        if let Some(item) = item
            && let Ok(icon) = item_icon_native(assets, item.item_type)
        {
            // Like the original, the icon fills its frame (40px or 65px).
            let size = (absolute.width - 4).max(1) as u32;
            let icon = imageops::resize(&icon, size, size, imageops::FilterType::Triangle);
            let icon_x = absolute.x + 2;
            let icon_y = absolute.y + 2;
            imageops::overlay(&mut image, &icon, i64::from(icon_x), i64::from(icon_y));
            draw_durability_indicator(
                &mut image,
                absolute.x + 4,
                absolute.y + absolute.height - 6,
                (absolute.width - 8).max(1),
                item,
            );
        }
    }
    Ok(image)
}

/// Returns the equipment that should be represented by the Character panel.
///
/// The server equipment table contains real, persisted items.  A newly
/// created character can still have the default outfit/weapon appearance from
/// the client profile before any item has been persisted in that table.  Keep
/// those appearance values visible in the panel as virtual entries (uid 0),
/// while real server items always win and remain the only draggable entries.
pub fn status_equipment_items(
    character: &crate::character::Character,
    equipment: &[crate::network::OwnedItem],
    hidden_default_slots: &[bool; 10],
) -> Vec<crate::network::OwnedItem> {
    let mut items = equipment.to_vec();
    for (position, item_type) in [
        (3_u8, character.armor_id),
        (4_u8, character.right_weapon_id),
        (5_u8, character.left_weapon_id),
    ] {
        if item_type != 0
            && !hidden_default_slots[usize::from(position)]
            && !items.iter().any(|item| item.position == position)
        {
            items.push(crate::network::OwnedItem {
                uid: 0,
                item_type,
                position,
                durability: 0,
                max_durability: 0,
                plus: 0,
                rarity: 0,
                gem_one: 0,
                gem_two: 0,
                rarity_roll: 0,
                bless: 0,
                enchant_hp: 0,
                color: 0,
            });
        }
    }
    items.sort_by_key(|item| item.position);
    items
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ItemContextAction {
    Use,
    Equip,
    Unequip,
    Upgrade,
    Rarity,
    ImproveQuality,
    ApplyRefinery,
    ApplyArtifact,
    GemSocket,
    OpenSocket,
    InsertGem,
    RemoveGem,
}

impl ItemContextAction {
    pub fn label(self) -> &'static str {
        match self {
            Self::Use => "USE",
            Self::Equip => "EQUIP",
            Self::Unequip => "UNEQUIP",
            Self::Upgrade => "UPGRADE",
            Self::Rarity => "RARITY",
            Self::ImproveQuality => "QUALITY",
            Self::ApplyRefinery => "REFINERY",
            Self::ApplyArtifact => "ARTIFACT",
            Self::GemSocket => "GEM SOCKET",
            Self::OpenSocket => "OPEN SOCKET",
            Self::InsertGem => "INSERT GEM",
            Self::RemoveGem => "REMOVE GEM",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ItemContext {
    pub item: crate::network::OwnedItem,
    pub actions: Vec<ItemContextAction>,
    pub anchor: [f32; 2],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ItemActionPanel {
    pub item: crate::network::OwnedItem,
    pub action: ItemContextAction,
    pub selected_rarity: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ItemActionPanelButton {
    Confirm,
    Cancel,
    Reroll,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GemSelectionPanel {
    pub item: crate::network::OwnedItem,
    pub socket: u8,
    pub selected_gem: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GemSelectionPanelButton {
    Socket(u8),
    Gem(u32),
    OpenSocket(u8),
    RemoveGem(u8),
}

const ITEM_CONTEXT_WIDTH: f32 = 224.0;
const ITEM_CONTEXT_HEADER_HEIGHT: f32 = 30.0;
const ITEM_CONTEXT_ROW_HEIGHT: f32 = 34.0;
const ITEM_ACTION_PANEL_WIDTH: f32 = 286.0;
const ITEM_ACTION_PANEL_HEIGHT: f32 = 214.0;
const RARITY_PANEL_WIDTH: f32 = 520.0;
const RARITY_PANEL_HEIGHT: f32 = 360.0;
const GEM_SELECTION_PANEL_WIDTH: f32 = 440.0;
const GEM_SELECTION_PANEL_HEIGHT: f32 = 380.0;
const UPGRADE_PANEL_WIDTH: f32 = 520.0;
const UPGRADE_PANEL_HEIGHT: f32 = 330.0;

fn item_action_panel_size(action: ItemContextAction) -> (f32, f32) {
    if action == ItemContextAction::Upgrade {
        (UPGRADE_PANEL_WIDTH, UPGRADE_PANEL_HEIGHT)
    } else if action == ItemContextAction::Rarity {
        (RARITY_PANEL_WIDTH, RARITY_PANEL_HEIGHT)
    } else {
        (ITEM_ACTION_PANEL_WIDTH, ITEM_ACTION_PANEL_HEIGHT)
    }
}

pub fn rarity_label(rarity: u8) -> &'static str {
    match rarity {
        1 => "RARE",
        2 => "EPIC",
        3 => "LEGENDARY",
        4 => "MYTHIC",
        _ => "COMMON",
    }
}

pub fn rarity_colour(rarity: u8) -> [u8; 3] {
    ItemRarity::from_custom_code(rarity).colour()
}

fn rarity_rgba(rarity: u8) -> [u8; 4] {
    let colour = rarity_colour(rarity);
    [colour[0], colour[1], colour[2], 255]
}

pub fn gem_selection_panel_size() -> (f32, f32) {
    (GEM_SELECTION_PANEL_WIDTH, GEM_SELECTION_PANEL_HEIGHT)
}

pub fn gem_selection_panel_position(viewport: [f32; 2]) -> [f32; 2] {
    let (width, height) = gem_selection_panel_size();
    [
        ((viewport[0] - width) * 0.5).max(8.0),
        ((viewport[1] - height) * 0.5).max(8.0),
    ]
}

pub fn gem_selection_panel_close_at(viewport: [f32; 2], cursor: [f32; 2]) -> bool {
    let [left, top] = gem_selection_panel_position(viewport);
    UiRect::new(GEM_SELECTION_PANEL_WIDTH as i32 - 31, 6, 27, 30)
        .contains_local([cursor[0] - left, cursor[1] - top])
}

pub fn gem_selection_panel_button_at(
    viewport: [f32; 2],
    cursor: [f32; 2],
    inventory: &[crate::network::OwnedItem],
) -> Option<GemSelectionPanelButton> {
    let [left, top] = gem_selection_panel_position(viewport);
    let local = [cursor[0] - left, cursor[1] - top];
    for socket in 1..=2u8 {
        let x = 20 + (i32::from(socket) - 1) * 204;
        if UiRect::new(x, 112, 194, 48).contains_local(local) {
            return Some(GemSelectionPanelButton::Socket(socket));
        }
    }
    let gems = inventory
        .iter()
        .filter(|item| (700_000..800_000).contains(&item.item_type))
        .take(16);
    for (index, gem) in gems.enumerate() {
        let column = (index % 4) as i32;
        let row = (index / 4) as i32;
        let rect = UiRect::new(24 + column * 102, 180 + row * 43, 94, 38);
        if rect.contains_local(local) {
            return Some(GemSelectionPanelButton::Gem(gem.uid));
        }
    }
    None
}

pub fn gem_selection_panel_action_at(
    viewport: [f32; 2],
    cursor: [f32; 2],
    panel: GemSelectionPanel,
) -> Option<GemSelectionPanelButton> {
    let [left, top] = gem_selection_panel_position(viewport);
    let local = [cursor[0] - left, cursor[1] - top];
    for socket in 1..=2u8 {
        let x = 20 + (i32::from(socket) - 1) * 204;
        let action = if socket == 1 {
            if panel.item.gem_one == 0 || panel.item.gem_one == 255 {
                Some(GemSelectionPanelButton::OpenSocket(socket))
            } else if panel.item.gem_one > 0 {
                Some(GemSelectionPanelButton::RemoveGem(socket))
            } else {
                None
            }
        } else if panel.item.gem_two == 0 || panel.item.gem_two == 255 {
            Some(GemSelectionPanelButton::OpenSocket(socket))
        } else if panel.item.gem_two > 0 {
            Some(GemSelectionPanelButton::RemoveGem(socket))
        } else {
            None
        };
        if action.is_some() && UiRect::new(x + 128, 130, 58, 22).contains_local(local) {
            return action;
        }
    }
    None
}

pub fn item_context_position(viewport: [f32; 2], anchor: [f32; 2], rows: usize) -> [f32; 2] {
    let height = ITEM_CONTEXT_HEADER_HEIGHT + ITEM_CONTEXT_ROW_HEIGHT * rows.max(1) as f32;
    [
        (anchor[0] + 8.0).min((viewport[0] - ITEM_CONTEXT_WIDTH - 8.0).max(8.0)),
        (anchor[1] + 8.0).min((viewport[1] - height - 8.0).max(8.0)),
    ]
}

pub fn item_context_button_at(
    viewport: [f32; 2],
    cursor: [f32; 2],
    anchor: [f32; 2],
    rows: usize,
) -> Option<usize> {
    let [left, top] = item_context_position(viewport, anchor, rows);
    if cursor[0] < left
        || cursor[0] >= left + ITEM_CONTEXT_WIDTH
        || cursor[1] < top + ITEM_CONTEXT_HEADER_HEIGHT
    {
        return None;
    }
    let row =
        ((cursor[1] - top - ITEM_CONTEXT_HEADER_HEIGHT) / ITEM_CONTEXT_ROW_HEIGHT).floor() as usize;
    (row < rows).then_some(row)
}

pub fn item_context_image(
    actions: &[ItemContextAction],
    hovered: Option<usize>,
) -> Result<RgbaImage> {
    let font = labels::font()?;
    let height = ITEM_CONTEXT_HEADER_HEIGHT + ITEM_CONTEXT_ROW_HEIGHT * actions.len().max(1) as f32;
    let mut image = RgbaImage::from_pixel(
        ITEM_CONTEXT_WIDTH as u32,
        height as u32,
        Rgba([4, 10, 14, 252]),
    );
    let width = image.width();
    let height = image.height();
    ui_theme::fill(
        &mut image,
        UiRect::new(1, 1, width as i32 - 2, ITEM_CONTEXT_HEADER_HEIGHT as i32),
        [10, 30, 40, 255],
    );
    rect_outline(&mut image, 0, 0, width, height, Rgba([226, 185, 77, 255]));
    labels::text_sized(
        &mut image,
        &font,
        &[("ITEM ACTIONS", ui_theme::TEXT_GOLD)],
        [12.0, 9.0],
        10.0,
    );
    for (index, action) in actions.iter().enumerate() {
        let top = (ITEM_CONTEXT_HEADER_HEIGHT + index as f32 * ITEM_CONTEXT_ROW_HEIGHT) as u32;
        let row_rect = UiRect::new(
            4,
            top as i32 + 2,
            width as i32 - 8,
            ITEM_CONTEXT_ROW_HEIGHT as i32 - 4,
        );
        if hovered == Some(index) {
            ui_theme::fill(&mut image, row_rect, [39, 67, 78, 255]);
            ui_theme::outline(&mut image, row_rect, [222, 178, 66, 255]);
        } else if index % 2 == 0 {
            ui_theme::fill(&mut image, row_rect, [7, 19, 27, 230]);
        }
        let label = action.label();
        let marker = match action {
            ItemContextAction::Equip | ItemContextAction::Unequip => "E",
            ItemContextAction::Upgrade => "+",
            ItemContextAction::Rarity => "◆",
            ItemContextAction::ImproveQuality => "Q",
            ItemContextAction::ApplyRefinery => "R",
            ItemContextAction::ApplyArtifact => "A",
            ItemContextAction::GemSocket => "S",
            ItemContextAction::OpenSocket
            | ItemContextAction::InsertGem
            | ItemContextAction::RemoveGem => "S",
            ItemContextAction::Use => "U",
        };
        labels::text_sized(
            &mut image,
            &font,
            &[(
                marker,
                if hovered == Some(index) {
                    ui_theme::TEXT
                } else {
                    ui_theme::TEXT_GOLD
                },
            )],
            [14.0, top as f32 + 9.0],
            10.0,
        );
        labels::text_sized(
            &mut image,
            &font,
            &[(
                label,
                if hovered == Some(index) {
                    ui_theme::TEXT
                } else {
                    ui_theme::TEXT_MUTED
                },
            )],
            [42.0, top as f32 + 9.0],
            10.0,
        );
    }
    Ok(image)
}

/// MsgAction 96 kill modes, indexed by the mode the server expects:
/// free PK, peace (monsters only), team and capture (black names only).
pub const KILL_MODES: [&str; 4] = ["PK", "PEACE", "TEAM", "CAPTURE"];

const CHOICE_MENU_WIDTH: f32 = 150.0;

/// A small list of choices drawn like the item action menu: the kill-mode
/// menu above Capture and the Shift+click player menu.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ChoiceMenu {
    pub title: String,
    pub labels: Vec<&'static str>,
    /// A row drawn as the current choice.
    pub marked: Option<usize>,
    /// Top-left corner in window pixels.
    pub position: [i32; 2],
}

impl ChoiceMenu {
    fn height(rows: usize) -> f32 {
        ITEM_CONTEXT_HEADER_HEIGHT + ITEM_CONTEXT_ROW_HEIGHT * rows.max(1) as f32
    }

    /// The kill-mode menu, opened above the Capture button.
    pub fn kill_modes(viewport: [f32; 2], current: u8) -> Self {
        let button = crate::original_ui_layout::main_hud_button_rect(
            viewport,
            crate::original_ui_layout::MainHudButton::Capture,
        );
        let height = Self::height(KILL_MODES.len());
        Self {
            title: "PK MODE".into(),
            labels: KILL_MODES.to_vec(),
            marked: Some(usize::from(current)),
            position: [
                button.x.min(viewport[0] - CHOICE_MENU_WIDTH - 8.0).max(8.0) as i32,
                (button.y - height - 4.0).max(8.0) as i32,
            ],
        }
    }

    /// A menu opened at `anchor` (the click), kept inside the window.
    pub fn at(viewport: [f32; 2], anchor: [f32; 2], title: String, labels: Vec<&'static str>) -> Self {
        let height = Self::height(labels.len());
        Self {
            title,
            labels,
            marked: None,
            position: [
                (anchor[0] + 8.0).min((viewport[0] - CHOICE_MENU_WIDTH - 8.0).max(8.0)) as i32,
                (anchor[1] + 8.0).min((viewport[1] - height - 8.0).max(8.0)) as i32,
            ],
        }
    }

    pub fn contains(&self, cursor: [f32; 2]) -> bool {
        let [left, top] = self.position.map(|value| value as f32);
        cursor[0] >= left
            && cursor[0] < left + CHOICE_MENU_WIDTH
            && cursor[1] >= top
            && cursor[1] < top + Self::height(self.labels.len())
    }

    /// The row under the cursor, if any.
    pub fn row_at(&self, cursor: [f32; 2]) -> Option<usize> {
        let [left, top] = self.position.map(|value| value as f32);
        if !self.contains(cursor) || cursor[1] < top + ITEM_CONTEXT_HEADER_HEIGHT || cursor[0] < left {
            return None;
        }
        let row = ((cursor[1] - top - ITEM_CONTEXT_HEADER_HEIGHT) / ITEM_CONTEXT_ROW_HEIGHT).floor()
            as usize;
        (row < self.labels.len()).then_some(row)
    }

    pub fn image(&self, hovered: Option<usize>) -> Result<RgbaImage> {
        let font = labels::font()?;
        let mut image = RgbaImage::from_pixel(
            CHOICE_MENU_WIDTH as u32,
            Self::height(self.labels.len()) as u32,
            Rgba([4, 10, 14, 252]),
        );
        let width = image.width();
        let height = image.height();
        ui_theme::fill(
            &mut image,
            UiRect::new(1, 1, width as i32 - 2, ITEM_CONTEXT_HEADER_HEIGHT as i32),
            [10, 30, 40, 255],
        );
        rect_outline(&mut image, 0, 0, width, height, Rgba([226, 185, 77, 255]));
        labels::text_sized(
            &mut image,
            &font,
            &[(self.title.as_str(), ui_theme::TEXT_GOLD)],
            [12.0, 9.0],
            10.0,
        );
        for (index, label) in self.labels.iter().enumerate() {
            let top = (ITEM_CONTEXT_HEADER_HEIGHT + index as f32 * ITEM_CONTEXT_ROW_HEIGHT) as u32;
            let row_rect = UiRect::new(
                4,
                top as i32 + 2,
                width as i32 - 8,
                ITEM_CONTEXT_ROW_HEIGHT as i32 - 4,
            );
            let marked = self.marked == Some(index);
            if hovered == Some(index) {
                ui_theme::fill(&mut image, row_rect, [39, 67, 78, 255]);
                ui_theme::outline(&mut image, row_rect, [222, 178, 66, 255]);
            } else if index % 2 == 0 {
                ui_theme::fill(&mut image, row_rect, [7, 19, 27, 230]);
            }
            if marked {
                labels::text_sized(
                    &mut image,
                    &font,
                    &[("*", ui_theme::TEXT_GOLD)],
                    [14.0, top as f32 + 9.0],
                    10.0,
                );
            }
            labels::text_sized(
                &mut image,
                &font,
                &[(
                    label,
                    if hovered == Some(index) || marked {
                        ui_theme::TEXT
                    } else {
                        ui_theme::TEXT_MUTED
                    },
                )],
                [32.0, top as f32 + 9.0],
                10.0,
            );
        }
        Ok(image)
    }
}

/// A one-line notice drawn at the top of the screen, such as "click a
/// player" while a Friend/Trade button waits for its target.
pub fn notice_image(text: &str) -> Result<RgbaImage> {
    let font = labels::font()?;
    let width = (text.chars().count() as u32 * 8 + 32).clamp(160, 640);
    let mut image = RgbaImage::from_pixel(width, 30, Rgba([4, 10, 14, 235]));
    rect_outline(&mut image, 0, 0, width, 30, Rgba([226, 185, 77, 255]));
    labels::text_sized(&mut image, &font, &[(text, ui_theme::TEXT_GOLD)], [16.0, 9.0], 11.0);
    Ok(image)
}

pub fn item_action_panel_position(viewport: [f32; 2], action: ItemContextAction) -> [f32; 2] {
    let (width, height) = item_action_panel_size(action);
    [
        ((viewport[0] - width) * 0.5).max(8.0),
        ((viewport[1] - height) * 0.5).max(8.0),
    ]
}

pub fn item_action_panel_button_at(
    viewport: [f32; 2],
    cursor: [f32; 2],
    action: ItemContextAction,
) -> Option<ItemActionPanelButton> {
    let [left, top] = item_action_panel_position(viewport, action);
    let local = [cursor[0] - left, cursor[1] - top];
    if action == ItemContextAction::Rarity {
        if UiRect::new(24, 318, 150, 28).contains_local(local) {
            return Some(ItemActionPanelButton::Reroll);
        }
        if UiRect::new(190, 318, 150, 28).contains_local(local) {
            return Some(ItemActionPanelButton::Confirm);
        }
        if UiRect::new(356, 318, 140, 28).contains_local(local) {
            return Some(ItemActionPanelButton::Cancel);
        }
        return None;
    }
    let (confirm, cancel) = if action == ItemContextAction::Upgrade {
        (
            UiRect::new(134, 279, 120, 30),
            UiRect::new(266, 279, 120, 30),
        )
    } else {
        (
            UiRect::new(24, 176, 112, 25),
            UiRect::new(150, 176, 112, 25),
        )
    };
    if confirm.contains_local(local) {
        Some(ItemActionPanelButton::Confirm)
    } else if cancel.contains_local(local) {
        Some(ItemActionPanelButton::Cancel)
    } else {
        None
    }
}

pub fn item_action_panel_close_at(
    viewport: [f32; 2],
    cursor: [f32; 2],
    action: ItemContextAction,
) -> bool {
    let [left, top] = item_action_panel_position(viewport, action);
    let (width, _) = item_action_panel_size(action);
    UiRect::new(width as i32 - 31, 6, 27, 30).contains_local([cursor[0] - left, cursor[1] - top])
}

pub fn item_action_panel_confirm_enabled(
    panel: ItemActionPanel,
    inventory: &[crate::network::OwnedItem],
) -> bool {
    match panel.action {
        ItemContextAction::Upgrade => {
            let Some((cost, _chance)) = upgrade_cost_and_chance(panel.item.plus) else {
                return false;
            };
            inventory
                .iter()
                .filter(|item| item.position < 40 && item.item_type == 1_088_001)
                .count()
                >= usize::from(cost)
        }
        ItemContextAction::ImproveQuality => {
            matches!(panel.item.item_type % 10, 5..=8)
                && inventory
                    .iter()
                    .any(|item| item.position < 40 && item.item_type == 1_088_000)
        }
        ItemContextAction::OpenSocket => {
            let first_open = panel.item.gem_one != 0 || panel.item.gem_one == 255;
            let second_open = panel.item.gem_two != 0 || panel.item.gem_two == 255;
            let cost = if !first_open {
                1
            } else if !second_open {
                5
            } else {
                return false;
            };
            inventory
                .iter()
                .filter(|item| item.position < 40 && item.item_type == 1_088_000)
                .count()
                >= cost
        }
        ItemContextAction::Rarity => {
            let Some((cost, _chance)) = rarity_upgrade_cost_and_chance(panel.item.rarity) else {
                return false;
            };
            panel.selected_rarity == panel.item.rarity.saturating_add(1)
                && inventory
                    .iter()
                    .filter(|item| item.position < 40 && item.item_type == 1_088_000)
                    .count()
                    >= usize::from(cost)
        }
        ItemContextAction::InsertGem => {
            (panel.item.gem_one == 255 || panel.item.gem_two == 255)
                && inventory
                    .iter()
                    .any(|item| item.position < 40 && (700_000..800_000).contains(&item.item_type))
        }
        ItemContextAction::RemoveGem => {
            (panel.item.gem_one > 0 && panel.item.gem_one != 255)
                || (panel.item.gem_two > 0 && panel.item.gem_two != 255)
        }
        ItemContextAction::ApplyRefinery | ItemContextAction::ApplyArtifact => true,
        _ => true,
    }
}

pub fn item_action_panel_reroll_enabled(
    panel: ItemActionPanel,
    inventory: &[crate::network::OwnedItem],
) -> bool {
    panel.action == ItemContextAction::Rarity
        && panel.item.rarity > 0
        && inventory
            .iter()
            .any(|item| item.position < 40 && item.item_type == 1_088_000)
}

pub fn item_action_panel_image(
    assets: &mut Assets,
    panel: ItemActionPanel,
    hovered: Option<ItemActionPanelButton>,
    inventory: &[crate::network::OwnedItem],
    close_hovered: bool,
    action_animation_progress: Option<f32>,
) -> Result<RgbaImage> {
    if panel.action == ItemContextAction::Upgrade {
        return upgrade_action_panel_image(
            assets,
            panel,
            hovered,
            inventory,
            close_hovered,
            action_animation_progress,
        );
    }
    if panel.action == ItemContextAction::Rarity {
        return rarity_action_panel_image(
            assets,
            panel,
            hovered,
            inventory,
            close_hovered,
            action_animation_progress,
        );
    }
    let mut image = RgbaImage::from_pixel(
        ITEM_ACTION_PANEL_WIDTH as u32,
        ITEM_ACTION_PANEL_HEIGHT as u32,
        Rgba([0, 0, 0, 0]),
    );
    ui_theme::window(&mut image, panel.action.label(), close_hovered)?;
    let font = labels::font()?;
    ui_theme::slot(&mut image, UiRect::new(22, 53, 52, 52), false, false, true);
    if let Ok(icon) = item_icon_image(assets, panel.item.item_type) {
        imageops::overlay(&mut image, &icon, 32, 63);
    }
    let target_name = item_name(assets, panel.item.item_type);
    let item_title = if panel.item.plus > 0 {
        format!("{target_name}  +{}", panel.item.plus)
    } else {
        target_name.clone()
    };
    labels::text_sized(
        &mut image,
        &font,
        &[("TARGET ITEM", ui_theme::TEXT_GOLD)],
        [88.0, 51.0],
        10.0,
    );
    labels::text_sized(
        &mut image,
        &font,
        &[(item_title.as_str(), ui_theme::TEXT)],
        [88.0, 67.0],
        9.0,
    );
    let (hint, confirm_enabled) = match panel.action {
        ItemContextAction::Upgrade => {
            let next_plus = panel.item.plus.saturating_add(1).min(12);
            let meteor_count = inventory
                .iter()
                .filter(|item| item.position < 40 && item.item_type == 1_088_001)
                .count();
            let transition = if panel.item.plus >= 12 {
                "MAXIMUM +12".to_owned()
            } else {
                format!("Upgrade: +{}  ->  +{next_plus}", panel.item.plus)
            };
            labels::text_sized(
                &mut image,
                &font,
                &[(transition.as_str(), ui_theme::TEXT_GOLD)],
                [22.0, 96.0],
                10.0,
            );
            labels::text_sized(
                &mut image,
                &font,
                &[("Success chance: 100%", ui_theme::TEXT)],
                [22.0, 115.0],
                10.0,
            );
            let cost = format!("Cost: 1 Meteor   Available: {meteor_count}");
            labels::text_sized(
                &mut image,
                &font,
                &[(cost.as_str(), ui_theme::TEXT)],
                [22.0, 134.0],
                10.0,
            );
            let hint = if panel.item.plus >= 12 {
                "This item is already at maximum level."
            } else if meteor_count == 0 {
                "You need a Meteor to continue."
            } else {
                "Confirm to consume one Meteor."
            };
            (hint, panel.item.plus < 12 && meteor_count > 0)
        }
        ItemContextAction::Rarity => unreachable!("rarity uses its dedicated panel"),
        ItemContextAction::ImproveQuality => (
            "Uses one DragonBall.",
            item_action_panel_confirm_enabled(panel, inventory),
        ),
        ItemContextAction::ApplyRefinery => ("Uses a compatible refinery.", true),
        ItemContextAction::ApplyArtifact => ("Uses a compatible artifact.", true),
        ItemContextAction::OpenSocket => (
            "Uses 1 or 5 DragonBalls.",
            item_action_panel_confirm_enabled(panel, inventory),
        ),
        ItemContextAction::InsertGem => (
            "Uses the next open socket.",
            item_action_panel_confirm_enabled(panel, inventory),
        ),
        ItemContextAction::RemoveGem => (
            "Removes the selected gem.",
            item_action_panel_confirm_enabled(panel, inventory),
        ),
        _ => ("Confirm this item action.", true),
    };
    if !matches!(panel.action, ItemContextAction::Upgrade) {
        labels::text_sized(
            &mut image,
            &font,
            &[(hint, ui_theme::TEXT_MUTED)],
            [22.0, 128.0],
            10.0,
        );
    } else {
        labels::text_sized(
            &mut image,
            &font,
            &[(
                hint,
                if confirm_enabled {
                    ui_theme::TEXT_MUTED
                } else {
                    [232, 100, 82]
                },
            )],
            [22.0, 153.0],
            9.0,
        );
    }
    ui::draw_text_button(
        &mut image,
        &font,
        UiRect::new(24, 176, 112, 25),
        "CONFIRM",
        hovered == Some(ItemActionPanelButton::Confirm),
        !confirm_enabled,
    );
    ui::draw_text_button(
        &mut image,
        &font,
        UiRect::new(150, 176, 112, 25),
        "CANCEL",
        hovered == Some(ItemActionPanelButton::Cancel),
        false,
    );
    Ok(image)
}

fn rarity_action_panel_image(
    assets: &mut Assets,
    panel: ItemActionPanel,
    hovered: Option<ItemActionPanelButton>,
    inventory: &[crate::network::OwnedItem],
    close_hovered: bool,
    animation_progress: Option<f32>,
) -> Result<RgbaImage> {
    let mut image = RgbaImage::from_pixel(
        RARITY_PANEL_WIDTH as u32,
        RARITY_PANEL_HEIGHT as u32,
        Rgba([5, 17, 25, 255]),
    );
    ui_theme::window(&mut image, "ITEM RARITY", close_hovered)?;
    let font = labels::font()?;
    let next_rarity = panel.item.rarity.saturating_add(1).min(4);
    let (dragon_ball_cost, success_chance) =
        rarity_upgrade_cost_and_chance(panel.item.rarity).unwrap_or((0, 0));
    let dragon_ball_count = inventory
        .iter()
        .filter(|item| item.position < 40 && item.item_type == 1_088_000)
        .count();
    let item_name = item_name(assets, panel.item.item_type);
    let title = if panel.item.plus > 0 {
        format!("{item_name} +{}", panel.item.plus)
    } else {
        item_name
    };
    for (x, rarity, caption) in [
        (20_i32, panel.item.rarity, "CURRENT ITEM"),
        (270, next_rarity, "AFTER UPGRADE"),
    ] {
        let card = UiRect::new(x, 48, 230, 238);
        ui_theme::fill(&mut image, card, [5, 17, 25, 255]);
        ui_theme::outline(&mut image, card, rarity_rgba(rarity));
        labels::text_sized(
            &mut image,
            &font,
            &[(caption, ui_theme::TEXT_GOLD)],
            [x as f32 + 12.0, 58.0],
            9.0,
        );
        let slot = UiRect::new(x + 89, 76, 52, 52);
        ui_theme::slot(&mut image, slot, false, false, true);
        ui_theme::outline(
            &mut image,
            UiRect::new(x + 87, 74, 56, 56),
            rarity_rgba(rarity),
        );
        if let Ok(icon) = item_icon_image(assets, panel.item.item_type) {
            imageops::overlay(&mut image, &icon, i64::from(x + 99), 86);
        }
        labels::text_sized(
            &mut image,
            &font,
            &[(title.as_str(), ui_theme::TEXT)],
            [x as f32 + 12.0, 140.0],
            9.0,
        );
        labels::text_sized(
            &mut image,
            &font,
            &[(rarity_label(rarity), rarity_colour(rarity))],
            [x as f32 + 12.0, 158.0],
            10.0,
        );
        let slots = format!("BONUS SLOTS  {}/4", rarity_bonus_slots(rarity));
        labels::text_sized(
            &mut image,
            &font,
            &[(slots.as_str(), ui_theme::TEXT_MUTED)],
            [x as f32 + 12.0, 178.0],
            8.0,
        );
        for slot_index in 0..4_u8 {
            let y = 195.0 + f32::from(slot_index) * 19.0;
            if slot_index < rarity_bonus_slots(rarity) {
                let (bonus_name, value, unit) = rarity_bonus(
                    panel.item.uid,
                    if x == 20 {
                        panel.item.rarity_roll
                    } else {
                        panel.item.rarity_roll.wrapping_add(37)
                    },
                    slot_index,
                );
                let text = if x == 20 {
                    format!("{bonus_name}  +{value}{unit}")
                } else {
                    format!("{bonus_name}  RANDOM")
                };
                labels::text_sized(
                    &mut image,
                    &font,
                    &[(text.as_str(), rarity_colour(rarity))],
                    [x as f32 + 12.0, y],
                    8.0,
                );
            } else {
                labels::text_sized(
                    &mut image,
                    &font,
                    &[("LOCKED BONUS SLOT", [93, 108, 114])],
                    [x as f32 + 12.0, y],
                    8.0,
                );
            }
        }
    }
    labels::text_sized(
        &mut image,
        &font,
        &[(">", ui_theme::TEXT_GOLD)],
        [253.0, 155.0],
        18.0,
    );
    let cost = if panel.item.rarity < 4 {
        format!("UPGRADE COST: {dragon_ball_cost} DB   AVAILABLE: {dragon_ball_count}")
    } else {
        "MAXIMUM RARITY".to_owned()
    };
    labels::text_sized(
        &mut image,
        &font,
        &[(cost.as_str(), ui_theme::TEXT)],
        [24.0, 288.0],
        9.0,
    );
    let chance = if panel.item.rarity < 4 {
        format!("CHANCE: {success_chance}%")
    } else {
        String::new()
    };
    labels::text_sized(
        &mut image,
        &font,
        &[(
            &chance,
            if success_chance >= 60 {
                [99, 221, 133]
            } else {
                [255, 174, 65]
            },
        )],
        [362.0, 288.0],
        9.0,
    );
    if let Some(progress) = animation_progress {
        let progress = progress.clamp(0.0, 1.0);
        let pulse = (progress * std::f32::consts::PI * 10.0).sin().abs();
        let travel_x = 178.0 + 164.0 * progress;
        let glow = (90.0 + pulse * 110.0) as u8;
        ui_theme::fill(
            &mut image,
            UiRect::new(176, 301, 168, 4),
            [103, 76, 34, 255],
        );
        ui_theme::fill(
            &mut image,
            UiRect::new(176, 301, (168.0 * progress).round() as i32, 4),
            [glow, 184, 75, 255],
        );
        ui_theme::fill(
            &mut image,
            UiRect::new(travel_x as i32 - 5, 295, 10, 16),
            [255, 218, 109, 230],
        );
        ui_theme::outline(
            &mut image,
            UiRect::new(travel_x as i32 - 8, 292, 16, 22),
            [255, 241, 176, 255],
        );
        labels::text_sized(
            &mut image,
            &font,
            &[("UPGRADING RARITY...", [255, 224, 139])],
            [184.0, 300.0],
            8.0,
        );
    }
    let reroll_enabled = item_action_panel_reroll_enabled(panel, inventory);
    let confirm_enabled = item_action_panel_confirm_enabled(panel, inventory);
    let animating = animation_progress.is_some();
    ui::draw_text_button(
        &mut image,
        &font,
        UiRect::new(24, 318, 150, 28),
        "REROLL BONUS (1 DB)",
        hovered == Some(ItemActionPanelButton::Reroll),
        !reroll_enabled || animating,
    );
    ui::draw_text_button(
        &mut image,
        &font,
        UiRect::new(190, 318, 150, 28),
        "UPGRADE RARITY",
        hovered == Some(ItemActionPanelButton::Confirm),
        !confirm_enabled || animating,
    );
    ui::draw_text_button(
        &mut image,
        &font,
        UiRect::new(356, 318, 140, 28),
        "CANCEL",
        hovered == Some(ItemActionPanelButton::Cancel),
        animating,
    );
    Ok(image)
}

pub fn gem_selection_panel_image(
    assets: &mut Assets,
    panel: GemSelectionPanel,
    inventory: &[crate::network::OwnedItem],
    hovered: Option<GemSelectionPanelButton>,
    close_hovered: bool,
) -> Result<RgbaImage> {
    let mut image = RgbaImage::from_pixel(
        GEM_SELECTION_PANEL_WIDTH as u32,
        GEM_SELECTION_PANEL_HEIGHT as u32,
        Rgba([0, 0, 0, 0]),
    );
    ui_theme::window(&mut image, "GEM SOCKET", close_hovered)?;
    let font = labels::font()?;
    ui_theme::fill(&mut image, UiRect::new(18, 44, 404, 64), [7, 19, 28, 255]);
    ui_theme::outline(&mut image, UiRect::new(18, 44, 404, 64), [69, 94, 104, 255]);
    ui_theme::slot(&mut image, UiRect::new(24, 52, 52, 52), false, false, true);
    if let Ok(icon) = item_icon_image(assets, panel.item.item_type) {
        imageops::overlay(&mut image, &icon, 34, 62);
    }
    let target_name = item_name(assets, panel.item.item_type);
    labels::text_sized(
        &mut image,
        &font,
        &[(target_name.as_str(), ui_theme::TEXT)],
        [88.0, 53.0],
        11.0,
    );
    labels::text_sized(
        &mut image,
        &font,
        &[("Select a socket, then choose a gem", ui_theme::TEXT_MUTED)],
        [88.0, 69.0],
        8.0,
    );
    labels::text_sized(
        &mut image,
        &font,
        &[("SOCKETS", ui_theme::TEXT_GOLD)],
        [88.0, 99.0],
        8.0,
    );
    for socket in 1..=2 {
        let rect = UiRect::new(20 + (socket - 1) as i32 * 204, 112, 194, 48);
        let gem_code = if socket == 1 {
            panel.item.gem_one
        } else {
            panel.item.gem_two
        };
        let open = gem_code == 255;
        let occupied = gem_code > 0 && !open;
        let active = panel.socket == socket;
        ui_theme::fill(
            &mut image,
            rect,
            if active {
                [35, 61, 72, 255]
            } else {
                [7, 18, 27, 255]
            },
        );
        ui_theme::outline(
            &mut image,
            rect,
            if open {
                if active {
                    [
                        ui_theme::TEXT_GOLD[0],
                        ui_theme::TEXT_GOLD[1],
                        ui_theme::TEXT_GOLD[2],
                        255,
                    ]
                } else {
                    [120, 160, 145, 255]
                }
            } else {
                [80, 80, 80, 255]
            },
        );
        if occupied {
            if let Ok(icon) = item_icon_image(assets, 700_000 + u32::from(gem_code)) {
                imageops::overlay(&mut image, &icon, rect.x as i64 + 8, rect.y as i64 + 8);
            }
        }
        let label = format!("SOCKET {socket}");
        labels::text_sized(
            &mut image,
            &font,
            &[(
                &label,
                if open {
                    ui_theme::TEXT
                } else {
                    ui_theme::TEXT_MUTED
                },
            )],
            [rect.x as f32 + 47.0, rect.y as f32 + 8.0],
            8.0,
        );
        let closed = gem_code == 0;
        let state = if open {
            "SELECT GEM"
        } else if occupied {
            "GEM INSTALLED"
        } else {
            "CLOSED"
        };
        labels::text_sized(
            &mut image,
            &font,
            &[(
                state,
                if open {
                    [120, 190, 155]
                } else if occupied {
                    ui_theme::TEXT_GOLD
                } else {
                    ui_theme::TEXT_MUTED
                },
            )],
            [rect.x as f32 + 47.0, rect.y as f32 + 24.0],
            7.0,
        );
        if open || occupied || closed {
            let action_rect = UiRect::new(rect.x + 128, rect.y + 18, 58, 22);
            ui_theme::fill(
                &mut image,
                action_rect,
                if active {
                    [76, 58, 30, 255]
                } else {
                    [24, 31, 35, 255]
                },
            );
            ui_theme::outline(
                &mut image,
                action_rect,
                if active {
                    [226, 185, 77, 255]
                } else {
                    [112, 101, 65, 255]
                },
            );
            let action_label = if occupied {
                "REMOVE"
            } else if open {
                "+"
            } else {
                "OPEN"
            };
            labels::text_sized(
                &mut image,
                &font,
                &[(action_label, ui_theme::TEXT)],
                [
                    action_rect.x as f32 + if action_label == "+" { 25.0 } else { 9.0 },
                    action_rect.y as f32 + 7.0,
                ],
                7.0,
            );
        }
    }
    labels::text_sized(
        &mut image,
        &font,
        &[(
            &format!("GEMS IN INVENTORY  •  SOCKET {} SELECTED", panel.socket),
            ui_theme::TEXT_GOLD,
        )],
        [24.0, 170.0],
        8.0,
    );
    let gems: Vec<_> = inventory
        .iter()
        .filter(|item| (700_000..800_000).contains(&item.item_type))
        .take(16)
        .collect();
    for (index, gem) in gems.iter().enumerate() {
        let column = (index % 4) as i32;
        let row = (index / 4) as i32;
        let rect = UiRect::new(24 + column * 102, 180 + row * 43, 94, 38);
        let active = panel.selected_gem == Some(gem.uid)
            || hovered == Some(GemSelectionPanelButton::Gem(gem.uid));
        let card_colour = if active {
            [35, 61, 72, 255]
        } else {
            [7, 18, 27, 255]
        };
        ui_theme::fill(&mut image, rect, card_colour);
        ui_theme::outline(
            &mut image,
            rect,
            if active {
                [
                    ui_theme::TEXT_GOLD[0],
                    ui_theme::TEXT_GOLD[1],
                    ui_theme::TEXT_GOLD[2],
                    255,
                ]
            } else {
                [120, 101, 54, 255]
            },
        );
        if let Ok(icon) = item_icon_image(assets, gem.item_type) {
            imageops::overlay(&mut image, &icon, rect.x as i64 + 4, rect.y as i64 + 3);
        }
        let name = item_name(assets, gem.item_type);
        labels::text_sized(
            &mut image,
            &font,
            &[(
                name.as_str(),
                if active {
                    ui_theme::TEXT
                } else {
                    ui_theme::TEXT_MUTED
                },
            )],
            [rect.x as f32 + 39.0, rect.y as f32 + 6.0],
            8.0,
        );
        let tier = format!("Type {}  •  #{}", gem.item_type % 10, gem.uid);
        labels::text_sized(
            &mut image,
            &font,
            &[(&tier, ui_theme::TEXT_MUTED)],
            [rect.x as f32 + 39.0, rect.y as f32 + 21.0],
            7.0,
        );
    }
    if gems.is_empty() {
        labels::text_sized(
            &mut image,
            &font,
            &[("No gems available in the inventory.", [232, 100, 82])],
            [24.0, 198.0],
            9.0,
        );
    }
    tooltip_rule(&mut image, 340);
    let hint = "Select a gem, then click + on the socket to insert  •  Esc to cancel";
    labels::text_sized(
        &mut image,
        &font,
        &[(hint, ui_theme::TEXT_MUTED)],
        [24.0, 347.0],
        8.0,
    );
    Ok(image)
}

fn upgrade_action_panel_image(
    assets: &mut Assets,
    panel: ItemActionPanel,
    hovered: Option<ItemActionPanelButton>,
    inventory: &[crate::network::OwnedItem],
    close_hovered: bool,
    animation_progress: Option<f32>,
) -> Result<RgbaImage> {
    let mut image = RgbaImage::from_pixel(
        UPGRADE_PANEL_WIDTH as u32,
        UPGRADE_PANEL_HEIGHT as u32,
        Rgba([0, 0, 0, 0]),
    );
    ui_theme::window(&mut image, "UPGRADE ITEM", close_hovered)?;
    let font = labels::font()?;
    let confirm_enabled = item_action_panel_confirm_enabled(panel, inventory);
    let meteor_count = inventory
        .iter()
        .filter(|item| item.position < 40 && item.item_type == 1_088_001)
        .count();
    let (meteor_cost, success_chance) = upgrade_cost_and_chance(panel.item.plus).unwrap_or((0, 0));
    let next_plus = panel.item.plus.saturating_add(1).min(12);
    let animating = animation_progress.is_some();
    let current_name = item_name(assets, panel.item.item_type);
    let current_title = if panel.item.plus > 0 {
        format!("{current_name}  +{}", panel.item.plus)
    } else {
        format!("{current_name}  +0")
    };
    let next_title = format!("{current_name}  +{next_plus}");

    for (rect, title) in [
        (UiRect::new(24, 52, 210, 178), "CURRENT ITEM"),
        (UiRect::new(286, 52, 210, 178), "AFTER UPGRADE"),
    ] {
        ui_theme::fill(&mut image, rect, [7, 18, 27, 255]);
        ui_theme::outline(&mut image, rect, [126, 91, 39, 255]);
        labels::text_sized(
            &mut image,
            &font,
            &[(title, ui_theme::TEXT_GOLD)],
            [rect.x as f32 + 12.0, rect.y as f32 + 10.0],
            10.0,
        );
    }
    ui_theme::slot(&mut image, UiRect::new(97, 82, 64, 64), false, false, true);
    ui_theme::slot(&mut image, UiRect::new(359, 82, 64, 64), false, true, true);
    if let Ok(icon) = item_icon_image(assets, panel.item.item_type) {
        imageops::overlay(&mut image, &icon, 113, 98);
        imageops::overlay(&mut image, &icon, 375, 98);
    }
    if let Some(progress) = animation_progress {
        let progress = progress.clamp(0.0, 1.0);
        let eased = progress * progress * (3.0 - 2.0 * progress);
        let travel_x = 156.0 + 208.0 * eased;
        let pulse = (progress * std::f32::consts::PI * 10.0).sin().abs();
        let glow = (90.0 + pulse * 110.0) as u8;
        ui_theme::fill(
            &mut image,
            UiRect::new(174, 112, 172, 4),
            [103, 76, 34, 255],
        );
        ui_theme::fill(
            &mut image,
            UiRect::new(174, 112, (172.0 * progress).round() as i32, 4),
            [glow, 184, 75, 255],
        );
        ui_theme::fill(
            &mut image,
            UiRect::new(travel_x as i32 - 5, 106, 10, 16),
            [255, 218, 109, 230],
        );
        ui_theme::outline(
            &mut image,
            UiRect::new(travel_x as i32 - 8, 103, 16, 22),
            [255, 241, 176, 255],
        );
        labels::text_sized(
            &mut image,
            &font,
            &[("UPGRADING...", [255, 224, 139])],
            [214.0, 132.0],
            9.0,
        );
    }
    labels::text_sized(
        &mut image,
        &font,
        &[(current_title.as_str(), ui_theme::TEXT)],
        [36.0, 158.0],
        10.0,
    );
    labels::text_sized(
        &mut image,
        &font,
        &[(next_title.as_str(), ui_theme::TEXT_GOLD)],
        [298.0, 158.0],
        10.0,
    );
    let current_type = format!("Item ID: {}", panel.item.item_type);
    let current_level = format!("Current level: +{}", panel.item.plus);
    let next_level = format!("New level: +{next_plus}");
    labels::text_sized(
        &mut image,
        &font,
        &[(current_type.as_str(), ui_theme::TEXT_MUTED)],
        [36.0, 184.0],
        9.0,
    );
    labels::text_sized(
        &mut image,
        &font,
        &[(current_level.as_str(), ui_theme::TEXT_MUTED)],
        [36.0, 200.0],
        9.0,
    );
    labels::text_sized(
        &mut image,
        &font,
        &[(next_level.as_str(), ui_theme::TEXT_GOLD)],
        [298.0, 184.0],
        9.0,
    );
    labels::text_sized(
        &mut image,
        &font,
        &[(
            "Same item; only the plus level changes.",
            ui_theme::TEXT_MUTED,
        )],
        [298.0, 200.0],
        9.0,
    );
    labels::text_sized(
        &mut image,
        &font,
        &[(">>", ui_theme::TEXT_GOLD)],
        [250.0, 114.0],
        18.0,
    );

    ui_theme::fill(&mut image, UiRect::new(24, 241, 472, 28), [34, 45, 47, 255]);
    ui_theme::outline(
        &mut image,
        UiRect::new(24, 241, 472, 28),
        [126, 91, 39, 255],
    );
    let cost = format!(
        "COST: {meteor_cost} METEOR{}     AVAILABLE: {meteor_count}",
        if meteor_cost == 1 { "" } else { "S" }
    );
    labels::text_sized(
        &mut image,
        &font,
        &[(cost.as_str(), ui_theme::TEXT)],
        [40.0, 250.0],
        10.0,
    );
    let chance = if panel.item.plus >= 12 {
        "MAXIMUM LEVEL".to_owned()
    } else {
        format!("SUCCESS CHANCE: {success_chance}%")
    };
    labels::text_sized(
        &mut image,
        &font,
        &[(
            chance.as_str(),
            if panel.item.plus >= 12 {
                [232, 100, 82]
            } else if success_chance >= 70 {
                [99, 221, 133]
            } else {
                [255, 174, 65]
            },
        )],
        [318.0, 250.0],
        10.0,
    );
    draw_upgrade_action_button(
        &mut image,
        &font,
        UiRect::new(134, 279, 120, 30),
        "UPGRADE",
        hovered == Some(ItemActionPanelButton::Confirm),
        confirm_enabled && !animating,
        true,
    );
    draw_upgrade_action_button(
        &mut image,
        &font,
        UiRect::new(266, 279, 120, 30),
        "CANCEL",
        hovered == Some(ItemActionPanelButton::Cancel),
        !animating,
        false,
    );
    Ok(image)
}

fn draw_upgrade_action_button(
    image: &mut RgbaImage,
    font: &fontdue::Font,
    rect: UiRect,
    label: &str,
    hovered: bool,
    enabled: bool,
    primary: bool,
) {
    let fill = if !enabled {
        [31, 34, 32, 255]
    } else if primary {
        if hovered {
            [57, 139, 79, 255]
        } else {
            [35, 91, 57, 255]
        }
    } else if hovered {
        [111, 82, 37, 255]
    } else {
        [49, 40, 29, 255]
    };
    let border = if !enabled {
        [91, 96, 87, 255]
    } else if primary {
        [108, 224, 135, 255]
    } else {
        [205, 160, 70, 230]
    };
    let text = if !enabled {
        [133, 138, 124]
    } else if primary {
        [228, 255, 233]
    } else {
        [235, 225, 193]
    };
    ui_theme::fill(image, rect, fill);
    ui_theme::outline(image, rect, border);
    ui_theme::outline(
        image,
        UiRect::new(rect.x + 2, rect.y + 2, rect.width - 4, rect.height - 4),
        [border[0], border[1], border[2], 130],
    );
    let width = text_width(font, label);
    labels::text_sized(
        image,
        font,
        &[(label, text)],
        [
            rect.x as f32 + ((rect.width as f32 - width) * 0.5).max(4.0),
            rect.y as f32 + 9.0,
        ],
        10.0,
    );
}

/// Resolves the classic item type into the equipment position used by the
/// server protocol.  The text classification covers the normal itemtype.dat
/// names; the numeric ranges keep the action useful when an older data file
/// does not contain a descriptive type string.
pub fn equipment_slot_for_item(assets: &mut Assets, item_type: u32) -> Option<u8> {
    // The itemtype.dat text is not reliable for every legacy item family: in
    // particular, some weapons are exposed with an accessory description.
    // Resolve the numeric family first so a weapon can never be mistaken for
    // a ring/bracelet just because of that text.
    let family = item_type / 1_000;
    match family {
        110..=119 => return Some(1), // headgear
        120..=129 => return Some(2), // necklace / amulet
        130..=139 => return Some(3), // armor
        150..=151 => return Some(6), // ring
        152..=159 => return Some(7), // bracelet
        160..=169 => return Some(8), // boots
        170..=179 => return Some(7), // bracelet variants
        180..=189 => return Some(9), // garment / fashion
        410..=499 => return Some(4), // weapon
        500..=599 => return Some(4), // two-handed weapon
        900..=909 => return Some(5), // shield / off-hand
        1_050 => return Some(5),     // arrows
        _ => {}
    }

    let kind = item_info(assets, item_type)
        .map(|info| info.item_type.to_ascii_lowercase())
        .unwrap_or_default();
    if kind.contains("armor") {
        return Some(3);
    }
    if kind.contains("armet")
        || kind.contains("helmet")
        || kind.contains("head")
        || kind.contains("hat")
    {
        return Some(1);
    }
    if kind.contains("neck") || kind.contains("necklace") {
        return Some(2);
    }
    if kind.contains("ring") {
        return Some(6);
    }
    if kind.contains("boot") || kind.contains("shoe") {
        return Some(8);
    }
    if kind.contains("bracelet") || kind.contains("bangle") {
        return Some(7);
    }
    if kind.contains("tower") || kind.contains("fan") || kind.contains("shield") {
        return Some(5);
    }
    if kind.contains("blade")
        || kind.contains("sword")
        || kind.contains("weapon")
        || kind.contains("club")
        || kind.contains("bow")
        || kind.contains("dagger")
        || kind.contains("spear")
    {
        return Some(4);
    }
    match item_type / 10_000 {
        11 => Some(1),
        12 => Some(2),
        13 => Some(3),
        15 => Some(6),
        16 => Some(8),
        17 => Some(7),
        18 => Some(9),
        41..=49 => Some(4),
        90 => Some(5),
        _ => None,
    }
}

pub fn equipment_slot_accepts_item(assets: &mut Assets, item_type: u32, slot: u8) -> bool {
    let Some(primary) = equipment_slot_for_item(assets, item_type) else {
        return false;
    };
    primary == slot || (primary == 4 && slot == 5)
}

/// Returns the paper-doll slot accepted by the refinery material IDs used by
/// the current compatibility slice.
pub fn refinery_material_slot(item_type: u32) -> Option<u8> {
    match item_type {
        721_380..=721_384 | 724_440..=724_444 => Some(1),
        721_405..=721_409 | 721_420..=721_424 | 721_445 => Some(4),
        _ => None,
    }
}

pub fn artifact_material_slot(item_type: u32) -> Option<u8> {
    match item_type {
        800_401 => Some(5),
        820_001 => Some(1),
        822_001 => Some(3),
        824_001 => Some(8),
        _ => None,
    }
}

pub fn health_recovery_for_item(item_type: u32) -> Option<u16> {
    match item_type {
        1_000_000 => Some(70),
        1_000_010 => Some(100),
        1_000_020 => Some(250),
        1_000_030 => Some(500),
        1_002_000 => Some(800),
        1_002_010 => Some(1_200),
        1_002_020 => Some(2_000),
        1_002_050 => Some(3_000),
        _ => None,
    }
}

pub fn mana_recovery_for_item(item_type: u32) -> Option<u16> {
    match item_type {
        1_001_000 => Some(70),
        1_001_010 => Some(200),
        1_001_020 => Some(450),
        1_001_030 => Some(1_000),
        1_001_040 => Some(2_000),
        1_002_030 => Some(3_000),
        1_002_040 => Some(4_500),
        _ => None,
    }
}

pub fn consumable_bundle_for_item(item_type: u32) -> Option<(u32, u8)> {
    match item_type {
        720_010 => Some((1_000_030, 3)),
        720_011 => Some((1_002_000, 3)),
        720_012 => Some((1_002_010, 3)),
        720_013 => Some((1_002_020, 3)),
        721_330 => Some((1_002_050, 3)),
        720_014 => Some((1_001_030, 3)),
        720_015 => Some((1_001_040, 3)),
        720_016 => Some((1_002_030, 3)),
        720_017 | 721_332 => Some((1_002_040, 3)),
        720_027 => Some((1_088_001, 10)),
        720_028 => Some((1_088_000, 10)),
        _ => None,
    }
}

pub fn temporary_effect_for_item(item_type: u32) -> bool {
    // 723700 is the ExpBall.
    matches!(item_type, 722_700 | 723_017 | 723_700 | 1_200_000..=1_200_002)
}

pub fn is_consumable(item_type: u32) -> bool {
    health_recovery_for_item(item_type).is_some()
        || mana_recovery_for_item(item_type).is_some()
        || consumable_bundle_for_item(item_type).is_some()
        || temporary_effect_for_item(item_type)
}

fn draw_simple_status_tab(
    image: &mut RgbaImage,
    font: &fontdue::Font,
    title_font: &fontdue::Font,
    title: &str,
    body: &str,
) {
    let x = StatusLayout::INFO_X;
    let y = StatusLayout::BASIC_Y;
    let width = StatusLayout::INFO_WIDTH;
    let height = StatusLayout::BASIC_HEIGHT + StatusLayout::COMBAT_HEIGHT + 5;
    draw_info_card(image, font, title_font, x, y, width, height, title);

    // Keep descriptive text inside the same right-hand card used by the
    // attributes/equipment tabs. Long messages are split at a word boundary
    // so they cannot cross the frame or overlap the paper-doll area.
    let mut line = String::new();
    let mut row = 0;
    for word in body.split_whitespace() {
        let candidate = if line.is_empty() {
            word.to_owned()
        } else {
            format!("{line} {word}")
        };
        let candidate_width = candidate
            .chars()
            .map(|character| font.metrics(character, 10.0).advance_width)
            .sum::<f32>();
        if candidate_width > (width - 14) as f32 && !line.is_empty() {
            labels::text_sized(
                image,
                font,
                &[(line.as_str(), [170, 164, 143])],
                [x as f32 + 7.0, (y + 43 + row * 20) as f32],
                10.0,
            );
            row += 1;
            line = word.to_owned();
        } else {
            line = candidate;
        }
    }
    if !line.is_empty() {
        labels::text_sized(
            image,
            font,
            &[(line.as_str(), [170, 164, 143])],
            [x as f32 + 7.0, (y + 43 + row * 20) as f32],
            10.0,
        );
    }
}

fn nobility_rank_label(rank: u32) -> &'static str {
    match rank {
        1 => "Knight",
        3 => "Baron",
        5 => "Earl",
        7 => "Duke",
        9 => "Prince",
        12 => "King",
        _ => "Serf",
    }
}

fn title_label(title_id: u32) -> &'static str {
    match title_id {
        1 => "Novice",
        2 => "Veteran",
        3 => "Knight",
        4 => "Baron",
        5 => "Duke",
        _ => "Nenhum",
    }
}

fn draw_title_tab(
    image: &mut RgbaImage,
    font: &fontdue::Font,
    title_font: &fontdue::Font,
    equipped_title: u32,
    nobility_rank: u32,
    hovered: Option<u32>,
) {
    let x = StatusLayout::INFO_X;
    let y = StatusLayout::BASIC_Y;
    let width = StatusLayout::INFO_WIDTH;
    let height = StatusLayout::BASIC_HEIGHT + StatusLayout::COMBAT_HEIGHT + 5;
    draw_info_card(image, font, title_font, x, y, width, height, "Títulos");
    labels::text_sized(
        image,
        font,
        &[(
            format!("Equipado: {}", title_label(equipped_title)).as_str(),
            [235, 225, 193],
        )],
        [x as f32 + 7.0, (y + 43) as f32],
        9.5,
    );
    labels::text_sized(
        image,
        font,
        &[(
            format!("Patente: {}", nobility_rank_label(nobility_rank)).as_str(),
            [190, 184, 162],
        )],
        [x as f32 + 7.0, (y + 56) as f32],
        9.0,
    );
    for (index, title_id) in [0_u32, 1, 2, 3, 4, 5].into_iter().enumerate() {
        draw_status_action_button(
            image,
            font,
            StatusLayout::title_button_rect(index),
            title_label(title_id),
            hovered == Some(title_id),
        );
    }
}

fn draw_skills_tab(image: &mut RgbaImage, font: &fontdue::Font, content_top: i32) {
    let x = 124;
    let y = content_top;
    labels::text(
        image,
        font,
        &[("Habilidades", [255, 224, 146])],
        [x as f32, y as f32],
    );
    labels::text(
        image,
        font,
        &[("Nenhuma habilidade aprendida", [150, 150, 150])],
        [x as f32, (y + 30) as f32],
    );
}

fn draw_magic_tab(image: &mut RgbaImage, font: &fontdue::Font, content_top: i32) {
    let x = 124;
    let y = content_top;
    labels::text(
        image,
        font,
        &[("Magias", [255, 224, 146])],
        [x as f32, y as f32],
    );
    labels::text(
        image,
        font,
        &[("Nenhuma magia aprendida", [150, 150, 150])],
        [x as f32, (y + 30) as f32],
    );
}

fn draw_status_tab(
    image: &mut RgbaImage,
    font: &fontdue::Font,
    character: &crate::character::Character,
    content_top: i32,
) {
    let x = 124;
    let mut y = content_top;
    labels::text(
        image,
        font,
        &[("Status Geral", [255, 224, 146])],
        [x as f32, y as f32],
    );
    y += 30;
    labels::text(
        image,
        font,
        &[
            ("Guilda:", [220, 210, 175]),
            (&character.guild, [255, 255, 255]),
        ],
        [x as f32, y as f32],
    );
    y += 26;
    labels::text(
        image,
        font,
        &[
            ("Cargo:", [220, 210, 175]),
            (&character.guild_rank, [255, 255, 255]),
        ],
        [x as f32, y as f32],
    );
    y += 26;
    labels::text(
        image,
        font,
        &[("PK Points:", [220, 210, 175]), ("0", [255, 255, 255])],
        [x as f32, y as f32],
    );
}

pub fn inventory_slot_at(viewport: [f32; 2], cursor: [f32; 2]) -> Option<u8> {
    let [left, top] = inventory_position(viewport);
    let local = [cursor[0] - left, cursor[1] - top];
    for slot in 0..40_u8 {
        if inventory_slot_rect(slot).contains_local(local) {
            return Some(slot);
        }
    }
    None
}

fn inventory_slot_rect(slot: u8) -> UiRect {
    let column = i32::from(slot % 5);
    let row = i32::from(slot / 5);
    UiRect::new(
        INVENTORY_SLOT_X as i32 + column * INVENTORY_SLOT_STEP_X as i32,
        INVENTORY_SLOT_Y as i32 + row * INVENTORY_SLOT_STEP_Y as i32,
        INVENTORY_SLOT_SIZE as i32,
        INVENTORY_SLOT_SIZE as i32,
    )
}

pub fn inventory_item_at(
    inventory: &[crate::network::OwnedItem],
    filter: InventoryFilter,
    viewport: [f32; 2],
    cursor: [f32; 2],
) -> Option<crate::network::OwnedItem> {
    let slot = inventory_slot_at(viewport, cursor)?;
    inventory
        .iter()
        .find(|item| item.position == slot && item.position < 40 && filter.accepts(item.item_type))
        .copied()
}

pub fn inventory_filter_at(viewport: [f32; 2], cursor: [f32; 2]) -> Option<InventoryFilter> {
    let [left, top] = inventory_position(viewport);
    for (index, filter) in InventoryFilter::ALL.into_iter().enumerate() {
        if inventory_filter_rect(index).contains_local([cursor[0] - left, cursor[1] - top]) {
            return Some(filter);
        }
    }
    None
}

pub fn inventory_organize_at(viewport: [f32; 2], cursor: [f32; 2]) -> bool {
    let [left, top] = inventory_position(viewport);
    INVENTORY_ORGANIZE_BUTTON.contains_local([cursor[0] - left, cursor[1] - top])
}

fn inventory_filter_rect(index: usize) -> UiRect {
    UiRect::new(
        INVENTORY_FILTER_X as i32 + index as i32 * INVENTORY_FILTER_STEP_X as i32,
        INVENTORY_FILTER_Y as i32,
        INVENTORY_FILTER_WIDTH as i32,
        INVENTORY_FILTER_HEIGHT as i32,
    )
}

pub fn inventory_panel_at(viewport: [f32; 2], cursor: [f32; 2]) -> bool {
    let [left, top] = inventory_position(viewport);
    cursor[0] >= left
        && cursor[0] < left + INVENTORY_WIDTH as f32
        && cursor[1] >= top
        && cursor[1] < top + INVENTORY_HEIGHT as f32
}

pub fn inventory_close_at(viewport: [f32; 2], cursor: [f32; 2]) -> bool {
    let [left, top] = inventory_position(viewport);
    let [x, y, width, height] = INVENTORY_CLOSE_RECT.map(|value| value as f32);
    (left + x..left + x + width).contains(&cursor[0])
        && (top + y..top + y + height).contains(&cursor[1])
}

pub fn warehouse_position(viewport: [f32; 2]) -> [f32; 2] {
    [
        8.0,
        ((viewport[1] - WAREHOUSE_HEIGHT as f32) * 0.5).max(8.0),
    ]
}

pub fn warehouse_slot_at(viewport: [f32; 2], cursor: [f32; 2], page: usize) -> Option<u8> {
    let [left, top] = warehouse_position(viewport);
    if cursor[0] < left + WAREHOUSE_SLOT_X as f32 || cursor[1] < top + WAREHOUSE_SLOT_Y as f32 {
        return None;
    }
    let column = ((cursor[0] - left - WAREHOUSE_SLOT_X as f32) / SLOT_STEP_X as f32).floor() as u32;
    let row = ((cursor[1] - top - WAREHOUSE_SLOT_Y as f32) / SLOT_STEP_Y as f32).floor() as u32;
    if column >= 5 || row >= 4 {
        return None;
    }
    let x = left + WAREHOUSE_SLOT_X as f32 + column as f32 * SLOT_STEP_X as f32;
    let y = top + WAREHOUSE_SLOT_Y as f32 + row as f32 * SLOT_STEP_Y as f32;
    (cursor[0] <= x + WAREHOUSE_SLOT_SIZE as f32 && cursor[1] <= y + WAREHOUSE_SLOT_SIZE as f32)
        .then_some((page.min(1) as u8) * WAREHOUSE_PAGE_SIZE + (row * 5 + column) as u8)
}

pub fn warehouse_panel_at(viewport: [f32; 2], cursor: [f32; 2]) -> bool {
    let [left, top] = warehouse_position(viewport);
    cursor[0] >= left
        && cursor[0] < left + WAREHOUSE_WIDTH as f32
        && cursor[1] >= top
        && cursor[1] < top + WAREHOUSE_HEIGHT as f32
}

fn warehouse_local(viewport: [f32; 2], cursor: [f32; 2]) -> [f32; 2] {
    let [left, top] = warehouse_position(viewport);
    [cursor[0] - left, cursor[1] - top]
}

fn warehouse_hit(viewport: [f32; 2], cursor: [f32; 2], rect: [i32; 4]) -> bool {
    let [x, y, width, height] = rect.map(|value| value as f32);
    within(warehouse_local(viewport, cursor), x, y, width, height)
}

pub fn warehouse_close_at(viewport: [f32; 2], cursor: [f32; 2]) -> bool {
    warehouse_hit(viewport, cursor, WAREHOUSE_CLOSE)
}

/// `Some(false)` withdraws, `Some(true)` deposits.
pub fn warehouse_action_at(viewport: [f32; 2], cursor: [f32; 2]) -> Option<bool> {
    let [width, height] = WAREHOUSE_BUTTON_SIZE;
    for (origin, deposit) in [(WAREHOUSE_WITHDRAW, false), (WAREHOUSE_DEPOSIT, true)] {
        if warehouse_hit(viewport, cursor, [origin[0], origin[1], width, height]) {
            return Some(deposit);
        }
    }
    None
}

/// `Some(true)` pages forward.
pub fn warehouse_page_at(viewport: [f32; 2], cursor: [f32; 2]) -> Option<bool> {
    for (origin, forward) in [(WAREHOUSE_PAGE_UP, false), (WAREHOUSE_PAGE_DOWN, true)] {
        if warehouse_hit(viewport, cursor, [origin[0], origin[1], 17, 17]) {
            return Some(forward);
        }
    }
    None
}

pub fn warehouse_amount_at(viewport: [f32; 2], cursor: [f32; 2]) -> bool {
    warehouse_hit(viewport, cursor, WAREHOUSE_AMOUNT_BOX)
}

pub fn shop_panel_at(viewport: [f32; 2], cursor: [f32; 2]) -> bool {
    let [left, top] = panel_position(viewport);
    cursor[0] >= left
        && cursor[0] < left + PANEL_WIDTH as f32
        && cursor[1] >= top
        && cursor[1] < top + SHOP_HEIGHT as f32
}

/// Item icon at its native 64px resolution.
pub fn item_icon_native(assets: &mut Assets, item_id: u32) -> Result<RgbaImage> {
    let animation = assets
        .read("ani/ItemMinIcon.Ani")
        .ok()
        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned());
    let map_animation = assets
        .read("ani/MapItemIcon.Ani")
        .ok()
        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned());
    load_icon_native(
        assets,
        item_id,
        animation.as_deref(),
        map_animation.as_deref(),
    )
}

pub fn item_icon_image(assets: &mut Assets, item_id: u32) -> Result<RgbaImage> {
    let animation = assets
        .read("ani/ItemMinIcon.Ani")
        .ok()
        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned());
    let map_animation = assets
        .read("ani/MapItemIcon.Ani")
        .ok()
        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned());
    load_icon(
        assets,
        item_id,
        animation.as_deref(),
        map_animation.as_deref(),
    )
}

/// Ground items use the original map sprites.  Some classic rare-item IDs
/// are animations whose first frame has a different DDS ID, so resolve the
/// MapItemIcon catalog before falling back to the generic inventory loader.
pub fn ground_item_icon_image(assets: &mut Assets, item_id: u32) -> Result<RgbaImage> {
    let direct = format!("data/MapItemIcon/{item_id}.dds");
    if let Ok(icon) = assets.image(&direct) {
        return Ok(icon);
    }
    let animation = assets
        .read("ani/MapItemIcon.Ani")
        .ok()
        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned());
    if let Some(animation) = animation
        && let Some(frame) = animation_frame(&animation, &format!("[{item_id}]"))
        && let Ok(icon) = assets.image(frame)
    {
        return Ok(icon);
    }
    item_icon_image(assets, item_id)
}

pub fn item_name(assets: &mut Assets, item_id: u32) -> String {
    load_item_infos(assets, &[item_id])
        .remove(&item_id)
        .map(|info| info.name)
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| match item_id {
            1_090_000 | 1_090_010 | 1_090_020 => "Silver".to_owned(),
            1_091_000 | 1_091_010 | 1_091_020 => "Gold".to_owned(),
            1_088_000 => "DragonBall".to_owned(),
            1_088_001 => "Meteor".to_owned(),
            _ => gem_item_name(item_id).unwrap_or_else(|| format!("Item {item_id}")),
        })
}

fn gem_item_name(item_id: u32) -> Option<String> {
    if !(700_000..800_000).contains(&item_id) || item_id % 1000 == 0 {
        return None;
    }
    let family = ((item_id - 700_000) / 10) as usize;
    let tier = item_id % 10;
    let names = [
        "PhoenixGem",
        "DragonGem",
        "FuryGem",
        "RainbowGem",
        "ThunderGem",
        "GloryGem",
        "KylinGem",
        "VioletGem",
    ];
    let family_name = names.get(family).copied().unwrap_or("CustomGem");
    Some(if tier <= 3 {
        family_name.to_owned()
    } else {
        format!("{family_name} T{tier}")
    })
}

pub fn panel_image(assets: &mut Assets, state: &ShopState) -> Result<RgbaImage> {
    let mut image = RgbaImage::from_pixel(PANEL_WIDTH, SHOP_HEIGHT, Rgba([0, 0, 0, 0]));
    imageops::overlay(&mut image, &panel_base_image(assets)?, 0, 0);
    imageops::overlay(&mut image, &panel_overlay_image(assets, state)?, 0, 0);
    Ok(image)
}

/// Static NPC shop window: `DialogNpcBuy` and its footer buttons.
pub fn panel_base_image(assets: &mut Assets) -> Result<RgbaImage> {
    use crate::original_ui::overlay_asset;
    let mut image = RgbaImage::from_pixel(PANEL_WIDTH, SHOP_HEIGHT, Rgba([0, 0, 0, 0]));
    overlay_asset(&mut image, assets, "data/main/DialogNpcBuy.dds", 0, 0)?;
    let y = FOOTER_BUTTON_Y as i64;
    overlay_asset(
        &mut image,
        assets,
        "data/main/HelpBtn2.dds",
        SHOP_HELP_X as i64,
        y,
    )?;
    overlay_asset(
        &mut image,
        assets,
        "data/main/CloseBtn2.dds",
        SHOP_CLOSE_X as i64,
        y,
    )?;
    // The client has no texture for the page buttons and shows them as
    // plain outlined squares.
    for page_y in [SHOP_PAGE_UP_Y, SHOP_PAGE_DOWN_Y] {
        shop_square(
            &mut image,
            SHOP_PAGE_X as u32 + 3,
            page_y as u32 + 3,
            [222, 205, 150, 255],
        );
    }
    Ok(image)
}

fn shop_square(image: &mut RgbaImage, x: u32, y: u32, colour: [u8; 4]) {
    outline(image, x, y, 10, Rgba(colour));
}

pub fn panel_overlay_image(assets: &mut Assets, state: &ShopState) -> Result<RgbaImage> {
    let mut image = RgbaImage::from_pixel(PANEL_WIDTH, SHOP_HEIGHT, Rgba([0, 0, 0, 0]));
    let first_item = state.page * 5;
    let icon_offset = (SLOT_SIZE - original_ui_layout::INVENTORY_ICON_SIZE as u32) / 2;
    for (slot, item_index) in
        (first_item..(first_item + ITEMS_PER_PAGE).min(state.items.len())).enumerate()
    {
        let row = slot as u32 / 5;
        let column = slot as u32 % 5;
        let x = SLOT_X + column * SLOT_STEP_X;
        let y = SLOT_Y + row * SLOT_STEP_Y;
        let Some(icon) = state.icons.get(item_index) else {
            continue;
        };
        imageops::overlay(
            &mut image,
            icon,
            (x + icon_offset).into(),
            (y + icon_offset).into(),
        );
        if state.selected == Some(item_index) {
            outline(&mut image, x, y, SLOT_SIZE, Rgba([255, 221, 96, 255]));
        } else if state.hover == Some(item_index) {
            outline(&mut image, x, y, SLOT_SIZE, Rgba([255, 236, 170, 200]));
        }
    }
    let repair = if state.repair_hover {
        "data/main/NpcBuyBtnFixClick.dds"
    } else {
        "data/main/NpcBuyBtnFix.dds"
    };
    crate::original_ui::overlay_asset(
        &mut image,
        assets,
        repair,
        SHOP_REPAIR_X as i64,
        FOOTER_BUTTON_Y as i64,
    )?;
    if let Some(down) = state.scroll_hover {
        let y = if down {
            SHOP_PAGE_DOWN_Y
        } else {
            SHOP_PAGE_UP_Y
        };
        shop_square(
            &mut image,
            SHOP_PAGE_X as u32 + 3,
            y as u32 + 3,
            [255, 255, 255, 255],
        );
    }
    Ok(image)
}

fn rounded_rect(
    image: &mut RgbaImage,
    left: u32,
    top: u32,
    width: u32,
    height: u32,
    radius: u32,
    colour: Rgba<u8>,
) {
    let right = left + width - 1;
    let bottom = top + height - 1;
    let radius = radius.min(width / 2).min(height / 2);
    for y in top..=bottom {
        for x in left..=right {
            let nearest_x = x.clamp(left + radius, right.saturating_sub(radius));
            let nearest_y = y.clamp(top + radius, bottom.saturating_sub(radius));
            let dx = x.abs_diff(nearest_x);
            let dy = y.abs_diff(nearest_y);
            if dx * dx + dy * dy <= radius * radius {
                image.put_pixel(x, y, colour);
            }
        }
    }
}

pub fn item_info(assets: &mut Assets, item_id: u32) -> Option<ItemInfo> {
    load_item_infos(assets, &[item_id]).remove(&item_id)
}

/// Returns the same basic-attack cadence used by the authoritative server.
/// Weapon speeds are stored in milliseconds in `itemtype.dat`; the default
/// matches the server fallback when the catalog has no weapon value.
pub fn attack_cooldown_seconds(
    assets: &mut Assets,
    equipment: &[crate::network::OwnedItem],
) -> f32 {
    let weapon_ids = equipment
        .iter()
        .filter(|item| {
            matches!(item.position, 4 | 5) && item.max_durability > 0 && item.durability > 0
        })
        .map(|item| item.item_type)
        .collect::<Vec<_>>();
    let infos = load_item_infos(assets, &weapon_ids);
    let speed = weapon_ids
        .iter()
        .find_map(|item_id| infos.get(item_id).map(|info| info.attack_speed))
        .filter(|speed| *speed > 0)
        .unwrap_or(800);
    (speed as f32 / 1_000.0).clamp(0.25, 3.0)
}

pub fn highlight_image() -> RgbaImage {
    let mut image = RgbaImage::from_pixel(39, 39, Rgba([0, 0, 0, 0]));
    outline(&mut image, 0, 0, 38, Rgba([255, 255, 255, 255]));
    image
}

fn shop_local(viewport: [f32; 2], cursor: [f32; 2]) -> [f32; 2] {
    let [left, top] = panel_position(viewport);
    [cursor[0] - left, cursor[1] - top]
}

fn within(point: [f32; 2], x: f32, y: f32, width: f32, height: f32) -> bool {
    (x..x + width).contains(&point[0]) && (y..y + height).contains(&point[1])
}

pub fn close_at(viewport: [f32; 2], cursor: [f32; 2]) -> bool {
    within(
        shop_local(viewport, cursor),
        SHOP_CLOSE_X,
        FOOTER_BUTTON_Y,
        43.0,
        20.0,
    )
}

pub fn repair_button_at(viewport: [f32; 2], cursor: [f32; 2]) -> bool {
    within(
        shop_local(viewport, cursor),
        SHOP_REPAIR_X,
        FOOTER_BUTTON_Y,
        62.0,
        20.0,
    )
}

pub fn scroll_direction_at(viewport: [f32; 2], cursor: [f32; 2]) -> Option<bool> {
    let local = shop_local(viewport, cursor);
    if within(local, SHOP_PAGE_X, SHOP_PAGE_UP_Y, 16.0, 16.0) {
        Some(false)
    } else if within(local, SHOP_PAGE_X, SHOP_PAGE_DOWN_Y, 16.0, 16.0) {
        Some(true)
    } else {
        None
    }
}

pub fn inventory_image(
    assets: &mut Assets,
    inventory: &[crate::network::OwnedItem],
    silver: u32,
    cps: u32,
    selected_uid: Option<u32>,
    filter: InventoryFilter,
    hovered_filter: Option<InventoryFilter>,
    close_hovered: bool,
) -> Result<RgbaImage> {
    let mut image = RgbaImage::from_pixel(INVENTORY_WIDTH, INVENTORY_HEIGHT, Rgba([0, 0, 0, 0]));
    let base = inventory_base_image(assets)?;
    let overlay = inventory_overlay_image(
        assets,
        inventory,
        silver,
        cps,
        selected_uid,
        filter,
        hovered_filter,
        close_hovered,
        false,
    )?;
    imageops::overlay(&mut image, &base, 0, 0);
    imageops::overlay(&mut image, &overlay, 0, 0);
    Ok(image)
}

/// Static layer for the original inventory. It is uploaded once and never
/// regenerated when an item, hover state, or currency value changes.
pub fn inventory_base_image(assets: &mut Assets) -> Result<RgbaImage> {
    crate::original_ui::inventory_base(assets)
}

/// Dynamic layer for the original inventory. It is transparent everywhere
/// except for item icons, durability, selection, values, and close hover.
pub fn inventory_overlay_image(
    assets: &mut Assets,
    inventory: &[crate::network::OwnedItem],
    silver: u32,
    cps: u32,
    selected_uid: Option<u32>,
    filter: InventoryFilter,
    hovered_filter: Option<InventoryFilter>,
    close_hovered: bool,
    organize_hovered: bool,
) -> Result<RgbaImage> {
    let mut image = RgbaImage::from_pixel(INVENTORY_WIDTH, INVENTORY_HEIGHT, Rgba([0, 0, 0, 0]));
    let font = crate::main_hud::font()?;
    let _ = (filter, hovered_filter, organize_hovered);
    if close_hovered {
        let [x, y, _, _] = INVENTORY_CLOSE_RECT;
        crate::original_ui::overlay_asset(
            &mut image,
            assets,
            "data/main/CloseBtn2Click.dds",
            x.into(),
            y.into(),
        )?;
    }
    let animation = assets
        .read("ani/ItemMinIcon.Ani")
        .ok()
        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned());
    let map_animation = assets
        .read("ani/MapItemIcon.Ani")
        .ok()
        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned());
    for item in inventory
        .iter()
        .filter(|item| item.position < 40 && filter.accepts(item.item_type))
    {
        let slot = inventory_slot_rect(item.position);
        quality_outline(&mut image, slot, item.rarity);
        let icon = load_icon_native(
            assets,
            item.item_type,
            animation.as_deref(),
            map_animation.as_deref(),
        )?;
        // The original scales the 64px icons down to fill the slot.
        let size = original_ui_layout::INVENTORY_ICON_SIZE as u32;
        let icon = if icon.dimensions() == (size, size) {
            icon
        } else {
            imageops::resize(&icon, size, size, imageops::FilterType::Triangle)
        };
        imageops::overlay(
            &mut image,
            &icon,
            (slot.x as u32 + INVENTORY_ICON_OFFSET).into(),
            (slot.y as u32 + INVENTORY_ICON_OFFSET).into(),
        );
        // Like the original, durability is only called out once it is low.
        if item.item_type < 1_000_000
            && item.max_durability > 0
            && u32::from(item.durability) * 4 <= u32::from(item.max_durability)
        {
            draw_durability_indicator(&mut image, slot.x + 4, slot.y + 34, 31, item);
        }
        if selected_uid == Some(item.uid) {
            outline(
                &mut image,
                slot.x as u32 + 1,
                slot.y as u32 + 1,
                INVENTORY_SLOT_SIZE - 2,
                Rgba([255, 221, 96, 255]),
            );
        }
    }
    let [x, y] = INVENTORY_SILVER_TEXT;
    crate::main_hud::text(
        &mut image,
        font,
        &format_silver(silver),
        12.0,
        [x, y],
        [255, 255, 255, 255],
        Some(80),
    );
    let [x, y] = INVENTORY_CPS_TEXT;
    crate::main_hud::text(
        &mut image,
        font,
        &format_silver(cps),
        12.0,
        [x, y],
        [255, 255, 255, 255],
        Some(80),
    );
    Ok(image)
}

fn draw_durability_indicator(
    image: &mut RgbaImage,
    x: i32,
    y: i32,
    width: i32,
    item: &crate::network::OwnedItem,
) {
    if item.max_durability == 0 {
        return;
    }
    let ratio = (f32::from(item.durability) / f32::from(item.max_durability)).clamp(0.0, 1.0);
    let color = if item.durability == 0 {
        [204, 55, 55, 255]
    } else if ratio <= 0.25 {
        [225, 151, 48, 255]
    } else {
        [59, 183, 99, 255]
    };
    status_fill(image, x, y, width, 4, [8, 12, 15, 235]);
    let inner_width = (width - 2).max(1);
    let filled = (inner_width as f32 * ratio).round() as i32;
    if filled > 0 {
        status_fill(image, x + 1, y + 1, filled, 2, color);
    }
    if item.durability == 0 {
        status_line(image, [x, y], [x + width - 1, y], color);
        status_line(image, [x, y + 3], [x + width - 1, y + 3], color);
    }
}

fn centered_text_x(font: &fontdue::Font, text: &str, center: f32) -> f32 {
    center - text_width(font, text) * 0.5
}

fn draw_inventory_close_button(image: &mut RgbaImage, hovered: bool) {
    const LEFT: u32 = 236;
    const TOP: u32 = 14;
    let outer = if hovered {
        [255, 224, 121, 255]
    } else {
        [239, 187, 71, 255]
    };
    let inner = if hovered {
        [139, 51, 28, 255]
    } else {
        [105, 37, 24, 255]
    };
    rounded_rect(image, LEFT, TOP, 25, 25, 4, Rgba([48, 24, 19, 255]));
    rounded_rect(image, LEFT + 2, TOP + 2, 21, 21, 3, Rgba(inner));
    rounded_rect(image, LEFT + 3, TOP + 3, 19, 19, 2, Rgba(inner));
    rect_outline(image, LEFT + 2, TOP + 2, 21, 21, Rgba(outer));

    for offset in 0..13 {
        for thickness in 0..3 {
            let points = [
                (LEFT + 6 + offset, TOP + 6 + offset + thickness),
                (LEFT + 6 + offset + thickness, TOP + 6 + offset),
                (LEFT + 18 - offset, TOP + 6 + offset + thickness),
                (LEFT + 18 - offset - thickness, TOP + 6 + offset),
            ];
            for (x, y) in points {
                if x < image.width() && y < image.height() {
                    image.put_pixel(x, y, Rgba(outer));
                }
            }
        }
    }
}

fn right_aligned_text_x(font: &fontdue::Font, text: &str, right: f32) -> f32 {
    right - text_width(font, text)
}

fn text_width(font: &fontdue::Font, text: &str) -> f32 {
    text.chars()
        .map(|character| font.metrics(character, 12.0).advance_width)
        .sum::<f32>()
}

pub fn warehouse_image(
    assets: &mut Assets,
    items: &[crate::network::OwnedItem],
    page: usize,
    selected_uid: Option<u32>,
    saved_silver: u32,
    amount_text: &str,
    amount_focused: bool,
    guild_warehouse: bool,
) -> Result<RgbaImage> {
    let mut image = RgbaImage::from_pixel(WAREHOUSE_WIDTH, WAREHOUSE_HEIGHT, Rgba([0, 0, 0, 0]));
    imageops::overlay(
        &mut image,
        &warehouse_base_image(assets, guild_warehouse)?,
        0,
        0,
    );
    imageops::overlay(
        &mut image,
        &warehouse_overlay_image(
            assets,
            items,
            page,
            selected_uid,
            saved_silver,
            amount_text,
            amount_focused,
            guild_warehouse,
        )?,
        0,
        0,
    );
    Ok(image)
}

/// Static Warehouse window: `DialogDepot` with its native buttons.
pub fn warehouse_base_image(assets: &mut Assets, guild_warehouse: bool) -> Result<RgbaImage> {
    use crate::original_ui::overlay_asset;
    let mut image = RgbaImage::from_pixel(WAREHOUSE_WIDTH, WAREHOUSE_HEIGHT, Rgba([0, 0, 0, 0]));
    overlay_asset(&mut image, assets, "data/main/DialogDepot.dds", 0, 0)?;
    for (path, [x, y]) in [
        ("data/main/DepotBtnGet.dds", WAREHOUSE_WITHDRAW),
        ("data/main/DepotBtnAdd.dds", WAREHOUSE_DEPOSIT),
        ("data/main/QuerySkillBtnU.dds", WAREHOUSE_PAGE_UP),
        ("data/main/QuerySkillBtnD.dds", WAREHOUSE_PAGE_DOWN),
        ("data/main/HelpBtn2.dds", WAREHOUSE_HELP),
        (
            "data/main/CloseBtn2.dds",
            [WAREHOUSE_CLOSE[0], WAREHOUSE_CLOSE[1]],
        ),
    ] {
        overlay_asset(&mut image, assets, path, x.into(), y.into())?;
    }
    let _ = guild_warehouse;
    Ok(image)
}

pub fn warehouse_overlay_image(
    assets: &mut Assets,
    items: &[crate::network::OwnedItem],
    page: usize,
    selected_uid: Option<u32>,
    saved_silver: u32,
    amount_text: &str,
    amount_focused: bool,
    guild_warehouse: bool,
) -> Result<RgbaImage> {
    let mut image = RgbaImage::from_pixel(WAREHOUSE_WIDTH, WAREHOUSE_HEIGHT, Rgba([0, 0, 0, 0]));
    let font = crate::main_hud::font()?;
    let animation = assets
        .read("ani/ItemMinIcon.Ani")
        .ok()
        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned());
    let map_animation = assets
        .read("ani/MapItemIcon.Ani")
        .ok()
        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned());
    let size = original_ui_layout::INVENTORY_ICON_SIZE as u32;
    let icon_offset = (WAREHOUSE_SLOT_SIZE - size) / 2;
    let first_slot = (page.min(1) as u8) * WAREHOUSE_PAGE_SIZE;
    for item in items
        .iter()
        .filter(|item| (first_slot..first_slot + WAREHOUSE_PAGE_SIZE).contains(&item.position))
    {
        let slot = item.position - first_slot;
        let slot_x = WAREHOUSE_SLOT_X + u32::from(slot % 5) * SLOT_STEP_X;
        let slot_y = WAREHOUSE_SLOT_Y + u32::from(slot / 5) * SLOT_STEP_Y;
        let rect = UiRect::new(
            slot_x as i32,
            slot_y as i32,
            WAREHOUSE_SLOT_SIZE as i32,
            WAREHOUSE_SLOT_SIZE as i32,
        );
        quality_outline(&mut image, rect, item.rarity);
        let icon = load_icon_native(
            assets,
            item.item_type,
            animation.as_deref(),
            map_animation.as_deref(),
        )?;
        let icon = if icon.dimensions() == (size, size) {
            icon
        } else {
            imageops::resize(&icon, size, size, imageops::FilterType::Triangle)
        };
        imageops::overlay(
            &mut image,
            &icon,
            (slot_x + icon_offset).into(),
            (slot_y + icon_offset).into(),
        );
        if selected_uid == Some(item.uid) {
            outline(
                &mut image,
                slot_x,
                slot_y,
                WAREHOUSE_SLOT_SIZE,
                Rgba([255, 221, 96, 255]),
            );
        }
    }
    const WHITE: [u8; 4] = [255, 255, 255, 255];
    if !guild_warehouse {
        let [x, y, width, height] = WAREHOUSE_AMOUNT_BOX;
        if amount_focused {
            rect_outline(
                &mut image,
                x as u32,
                y as u32,
                width as u32,
                height as u32,
                Rgba([150, 190, 205, 255]),
            );
        }
        let amount = if amount_focused {
            format!("{amount_text}|")
        } else if amount_text.is_empty() {
            "0".to_owned()
        } else {
            amount_text.to_owned()
        };
        crate::main_hud::text(
            &mut image,
            font,
            &amount,
            12.0,
            [x + 5, y + 4],
            WHITE,
            Some(width - 8),
        );
        let [x, y, width, _] = WAREHOUSE_SAVINGS_BOX;
        crate::main_hud::text(
            &mut image,
            font,
            &format_silver(saved_silver),
            12.0,
            [x + 5, y + 4],
            WHITE,
            Some(width - 8),
        );
    }
    Ok(image)
}

fn format_silver(silver: u32) -> String {
    let digits = silver.to_string();
    let mut formatted = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, digit) in digits.chars().enumerate() {
        if index != 0 && (digits.len() - index) % 3 == 0 {
            formatted.push(',');
        }
        formatted.push(digit);
    }
    formatted
}

pub fn tooltip_image(item_id: u32, info: Option<&ItemInfo>) -> Result<RgbaImage> {
    item_tooltip_image(item_id, item_id, info, 0, 0, 0, 0, 0, 0, 0)
}

/// Rich tooltip used for inventory items. The wrapper above remains available
/// for shop previews, while this variant includes the live item's plus and
/// sockets so the panel cannot become stale after an upgrade.
pub fn item_tooltip_image(
    item_uid: u32,
    item_id: u32,
    info: Option<&ItemInfo>,
    plus: u8,
    gem_one: u8,
    gem_two: u8,
    rarity_code: u8,
    rarity_roll: u8,
    bless: u8,
    enchant_hp: u8,
) -> Result<RgbaImage> {
    classic_item_tooltip(
        item_uid,
        item_id,
        info,
        None,
        plus,
        [gem_one, gem_two],
        rarity_code,
        rarity_roll,
        bless,
        enchant_hp,
    )
}

pub fn equipment_tooltip_image(
    item_uid: u32,
    item_id: u32,
    info: Option<&ItemInfo>,
    durability: u16,
    max_durability: u16,
    plus: u8,
    gem_one: u8,
    gem_two: u8,
    rarity_code: u8,
    rarity_roll: u8,
    bless: u8,
    enchant_hp: u8,
) -> Result<RgbaImage> {
    classic_item_tooltip(
        item_uid,
        item_id,
        info,
        Some((durability, max_durability)),
        plus,
        [gem_one, gem_two],
        rarity_code,
        rarity_roll,
        bless,
        enchant_hp,
    )
}

/// The original item tooltip: a translucent black box sized to its lines,
/// with the name in yellow, attributes in white, gems in green and bonuses
/// in magenta. The custom rarity system keeps its colour for its lines.
fn classic_item_tooltip(
    item_uid: u32,
    item_id: u32,
    info: Option<&ItemInfo>,
    durability: Option<(u16, u16)>,
    plus: u8,
    gems: [u8; 2],
    rarity_code: u8,
    rarity_roll: u8,
    bless: u8,
    enchant_hp: u8,
) -> Result<RgbaImage> {
    const YELLOW: [u8; 4] = [255, 255, 0, 255];
    const WHITE: [u8; 4] = [255, 255, 255, 255];
    const GREEN: [u8; 4] = [0, 255, 0, 255];
    const MAGENTA: [u8; 4] = [255, 64, 255, 255];
    const RED: [u8; 4] = [255, 80, 64, 255];
    let rarity = ItemRarity::from_custom_code(rarity_code);
    let rarity_colour = {
        let [r, g, b] = rarity.colour();
        [r, g, b, 255]
    };
    let mut lines: Vec<(String, [u8; 4])> = Vec::new();

    let fallback = format!("Item {item_id}");
    let name = info.map(|item| item.name.as_str()).unwrap_or(&fallback);
    let mut title = name.to_owned();
    if plus > 0 {
        title.push_str(&format!("(+{plus})"));
    }
    if bless > 0 {
        title.push_str(&format!(" -{bless}%"));
    }
    if let Some(kind) = info.map(|item| item.item_type.trim()).filter(|kind| !kind.is_empty()) {
        title.push_str(&format!(" ({kind})"));
    }
    lines.push((title, YELLOW));
    if rarity != ItemRarity::Common {
        lines.push((format!("Rarity: {}", rarity.label()), rarity_colour));
    }
    let scale = item_stat_scale(plus, rarity_code);
    if let Some(info) = info {
        if info.level > 0 {
            lines.push((format!("Level: {}", info.level), WHITE));
        }
        if info.attack_max > 0 {
            let attack = format_attack(Some((info.attack_min, info.attack_max)), scale);
            lines.push((format!("Attack: {attack}"), WHITE));
        }
        if info.defense > 0 {
            lines.push((
                format!("Defense: {}", scale_item_value(info.defense, scale)),
                WHITE,
            ));
        }
        if info.health_add > 0 {
            lines.push((format!("HP: +{}", info.health_add), WHITE));
        }
        if info.mana_add > 0 {
            lines.push((format!("MP: +{}", info.mana_add), WHITE));
        }
        if info.attack_speed > 0 {
            lines.push((format!("Attack Rate: {}", info.attack_speed), WHITE));
        }
    }
    let sockets = gems.iter().filter(|gem| **gem != 0).count();
    if sockets > 0 {
        lines.push((format!("Socket {sockets} gem(s)"), WHITE));
        for (index, gem) in gems.iter().enumerate() {
            if *gem != 0 && *gem != 255 {
                lines.push((
                    format!("SocketGem {}:{}", index + 1, gem_code_name(*gem)),
                    GREEN,
                ));
            }
        }
    }
    if let Some((current, maximum)) = durability.filter(|(_, maximum)| *maximum > 0) {
        lines.push((
            format!("Dura.: {current}/{maximum}"),
            if current == 0 { RED } else { WHITE },
        ));
    }
    if enchant_hp > 0 {
        lines.push((format!("Enchant: +{enchant_hp} HP"), MAGENTA));
    }
    if plus > 0 {
        lines.push((format!("Upgrade: +{}%", u32::from(plus) * 10), MAGENTA));
    }
    for slot in 0..rarity_bonus_slots(rarity_code) {
        let (bonus, value, unit) = rarity_bonus(item_uid, rarity_roll, slot);
        lines.push((format!("{bonus}: +{value}{unit}"), rarity_colour));
    }
    if let Some(price) = info.map(|item| item.price).filter(|price| *price > 0) {
        lines.push((format!("Price: {}", format_silver(price)), WHITE));
    }
    classic_tooltip(&lines)
}

/// Translucent black box with one 15px line per entry.
fn classic_tooltip(lines: &[(String, [u8; 4])]) -> Result<RgbaImage> {
    let font = crate::main_hud::font()?;
    let width = lines
        .iter()
        .map(|(line, _)| crate::main_hud::text_width(font, line, 12.0))
        .max()
        .unwrap_or(0)
        + 12;
    let height = lines.len() as u32 * 15 + 8;
    let mut image = RgbaImage::from_pixel(width.max(40) as u32, height, Rgba([0, 0, 0, 200]));
    let [w, h] = [image.width(), image.height()];
    for x in 0..w {
        image.put_pixel(x, 0, Rgba([70, 70, 70, 230]));
        image.put_pixel(x, h - 1, Rgba([70, 70, 70, 230]));
    }
    for y in 0..h {
        image.put_pixel(0, y, Rgba([70, 70, 70, 230]));
        image.put_pixel(w - 1, y, Rgba([70, 70, 70, 230]));
    }
    for (index, (line, colour)) in lines.iter().enumerate() {
        crate::main_hud::text(
            &mut image,
            font,
            line,
            12.0,
            [6, 4 + index as i32 * 15],
            *colour,
            None,
        );
    }
    Ok(image)
}

fn tooltip_rule(image: &mut RgbaImage, y: u32) {
    for x in 12..image.width().saturating_sub(12) {
        image.put_pixel(x, y, Rgba([80, 59, 34, 255]));
    }
}

fn item_stat_scale(plus: u8, rarity_code: u8) -> u32 {
    100_u32
        .saturating_add(u32::from(plus).saturating_mul(10))
        .saturating_add(ItemRarity::from_custom_code(rarity_code).bonus_percent())
}

fn scale_item_value(value: u32, scale: u32) -> u32 {
    value.saturating_mul(scale) / 100
}

fn format_attack(values: Option<(u32, u32)>, scale: u32) -> String {
    match values {
        Some((0, 0)) | None => "--".to_owned(),
        Some((min, max)) if min == max => scale_item_value(max, scale).to_string(),
        Some((min, max)) => format!(
            "{}-{}",
            scale_item_value(min, scale),
            scale_item_value(max, scale)
        ),
    }
}

fn gem_code_name(code: u8) -> String {
    let family_names = [
        "PhoenixGem",
        "DragonGem",
        "FuryGem",
        "RainbowGem",
        "KylinGem",
        "VioletGem",
        "MoonGem",
        "TortoiseGem",
    ];
    let family = usize::from(code / 10);
    let tier = code % 10;
    let name = family_names.get(family).copied().unwrap_or("CustomGem");
    if tier == 0 {
        name.to_owned()
    } else {
        format!("{name} T{tier}")
    }
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
                health_add: number(19),
                mana_add: number(20),
                attack_speed: number(32),
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
        seed = seed.wrapping_mul(0x343fdu32).wrapping_add(0x269ec3u32);
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
    map_animation: Option<&str>,
) -> Result<image::RgbaImage> {
    let icon = load_icon_native(assets, item_id, animation, map_animation)?;
    Ok(if icon.dimensions() == (ICON_SIZE, ICON_SIZE) {
        icon
    } else {
        imageops::resize(&icon, ICON_SIZE, ICON_SIZE, imageops::FilterType::Nearest)
    })
}

/// Item icon at its native resolution (64x64 for `ItemMinIcon`).
fn load_icon_native(
    assets: &mut Assets,
    item_id: u32,
    animation: Option<&str>,
    map_animation: Option<&str>,
) -> Result<image::RgbaImage> {
    let candidates = [
        format!("data/ItemMinIcon/{item_id}.dds"),
        format!("data/MapItemIcon/{item_id}.dds"),
    ];
    for path in candidates {
        if let Ok(icon) = assets.image(&path) {
            return Ok(icon);
        }
    }
    if let Some(animation) = animation {
        let sections = std::iter::once(format!("[Item{item_id}]"))
            .chain((3..=9).map(|colour| format!("[Item{colour}{item_id}]")));
        for section in sections {
            let Some(frame) = animation_frame(animation, &section) else {
                continue;
            };
            if let Ok(icon) = assets.image(frame.trim()) {
                return Ok(icon);
            }
        }
    }
    if let Some(animation) = map_animation
        && let Some(frame) = animation_frame(animation, &format!("[{item_id}]"))
        && let Ok(icon) = assets.image(frame.trim())
    {
        return Ok(icon);
    }
    if (700_000..800_000).contains(&item_id) && item_id % 1000 != 0 {
        return Ok(gem_placeholder_icon(item_id));
    }
    Ok(placeholder_icon(item_id)?)
}

fn animation_frame<'a>(animation: &'a str, section: &str) -> Option<&'a str> {
    let mut inside_section = false;
    for line in animation.lines().map(str::trim) {
        if line.starts_with('[') && line.ends_with(']') {
            if inside_section {
                return None;
            }
            inside_section = line.eq_ignore_ascii_case(section);
            continue;
        }
        if inside_section && let Some(frame) = line.strip_prefix("Frame0=") {
            return Some(frame.trim());
        }
    }
    None
}

fn placeholder_icon(item_id: u32) -> Result<RgbaImage> {
    let mut image = RgbaImage::from_pixel(ICON_SIZE, ICON_SIZE, Rgba([19, 28, 38, 255]));
    let accent = [
        90 + ((item_id >> 8) & 0x3f) as u8,
        90 + ((item_id >> 4) & 0x3f) as u8,
        90 + (item_id & 0x3f) as u8,
    ];
    for offset in 0..ICON_SIZE {
        image.put_pixel(offset, 0, Rgba([accent[0], accent[1], accent[2], 255]));
        image.put_pixel(
            offset,
            ICON_SIZE - 1,
            Rgba([accent[0], accent[1], accent[2], 255]),
        );
        image.put_pixel(0, offset, Rgba([accent[0], accent[1], accent[2], 255]));
        image.put_pixel(
            ICON_SIZE - 1,
            offset,
            Rgba([accent[0], accent[1], accent[2], 255]),
        );
    }
    let short_id = item_id % 1000;
    let digits = [short_id / 100, (short_id / 10) % 10, short_id % 10];
    for (index, digit) in digits.into_iter().enumerate() {
        draw_digit(&mut image, digit as usize, 4 + index as u32 * 8, 11, accent);
    }
    Ok(image)
}

fn gem_placeholder_icon(item_id: u32) -> RgbaImage {
    let family = ((item_id.saturating_sub(700_000) / 10) % 8) as usize;
    let tier = (item_id % 10).max(1);
    let colours = [
        [255, 206, 72],
        [87, 194, 255],
        [190, 102, 255],
        [107, 255, 190],
        [255, 114, 114],
        [255, 166, 72],
        [132, 220, 255],
        [220, 116, 255],
    ];
    let colour = colours[family];
    let mut image = RgbaImage::from_pixel(ICON_SIZE, ICON_SIZE, Rgba([9, 18, 26, 255]));
    for y in 4..28 {
        let distance = (y as i32 - 16).unsigned_abs().min(10);
        let half = 2 + (10 - distance) / 2;
        let left = 16_u32.saturating_sub(half);
        let right = (16 + half).min(ICON_SIZE - 1);
        for x in left..=right {
            let shade = if x <= 16 { 255 } else { 205 };
            image.put_pixel(x, y, Rgba([colour[0], colour[1], colour[2], shade]));
        }
    }
    for offset in 0..ICON_SIZE {
        image.put_pixel(offset, 0, Rgba([colour[0], colour[1], colour[2], 220]));
        image.put_pixel(
            offset,
            ICON_SIZE - 1,
            Rgba([colour[0], colour[1], colour[2], 220]),
        );
        image.put_pixel(0, offset, Rgba([colour[0], colour[1], colour[2], 220]));
        image.put_pixel(
            ICON_SIZE - 1,
            offset,
            Rgba([colour[0], colour[1], colour[2], 220]),
        );
    }
    let tier_colour = [255, 245, 190, 255];
    for y in 13..=18 {
        image.put_pixel(16, y, Rgba(tier_colour));
    }
    if tier >= 2 {
        image.put_pixel(13, 16, Rgba(tier_colour));
        image.put_pixel(19, 16, Rgba(tier_colour));
    }
    if tier >= 3 {
        image.put_pixel(10, 16, Rgba(tier_colour));
        image.put_pixel(22, 16, Rgba(tier_colour));
    }
    image
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

fn rect_outline(image: &mut RgbaImage, x: u32, y: u32, width: u32, height: u32, colour: Rgba<u8>) {
    for offset in 0..width {
        if x + offset < image.width() {
            if y < image.height() {
                image.put_pixel(x + offset, y, colour);
            }
            if y + height - 1 < image.height() {
                image.put_pixel(x + offset, y + height - 1, colour);
            }
        }
    }
    for offset in 0..height {
        if y + offset < image.height() {
            if x < image.width() {
                image.put_pixel(x, y + offset, colour);
            }
            if x + width - 1 < image.width() {
                image.put_pixel(x + width - 1, y + offset, colour);
            }
        }
    }
}

#[cfg(test)]
mod icon_tests {
    use super::{
        INVENTORY_FILTER_HEIGHT, INVENTORY_FILTER_STEP_X, INVENTORY_FILTER_WIDTH,
        INVENTORY_FILTER_X, INVENTORY_FILTER_Y, INVENTORY_SLOT_SIZE, INVENTORY_SLOT_STEP_X,
        INVENTORY_SLOT_STEP_Y, INVENTORY_SLOT_X, INVENTORY_SLOT_Y, INVENTORY_WIDTH,
        InventoryFilter, PlayerShopPanelAction, STATUS_STANDARD_HEIGHT, StatusLayout, StatusTab,
        animation_frame, consumable_bundle_for_item, health_recovery_for_item, inventory_close_at,
        inventory_filter_at, inventory_image, inventory_item_at, inventory_organize_at,
        inventory_panel_at, inventory_position, inventory_slot_at, inventory_slot_position,
        is_consumable, mana_recovery_for_item, player_shop_panel_action_at, player_shop_price_at,
        status_base_image, status_character_clip, status_close_at, status_dynamic_image,
        status_equipment_items, status_equipment_overlay_image, status_equipment_slot_at,
        status_position, status_reputation_donate_at, status_tab_at, status_title_at,
        team_health_fraction, temporary_effect_for_item,
    };

    #[test]
    fn character_layout_keeps_visual_and_interactive_regions_together() {
        for viewport in [[1024.0, 768.0], [1366.0, 768.0], [1704.0, 960.0]] {
            let [left, top] = status_position(viewport);
            assert_eq!(left, 0.0);
            assert_eq!(
                top,
                ((viewport[1] - STATUS_STANDARD_HEIGHT as f32) * 0.5).max(8.0)
            );
            for (x, y, width, height) in StatusLayout::CLOSE_RECTS {
                assert!(status_close_at(
                    viewport,
                    [left + (x + width / 2) as f32, top + (y + height / 2) as f32]
                ));
            }

            for (index, tab) in StatusTab::ALL.into_iter().enumerate() {
                let tab_y = top + StatusLayout::TAB_Y_POSITIONS[index] as f32;
                assert_eq!(
                    status_tab_at(
                        viewport,
                        [
                            left + StatusLayout::TAB_X as f32
                                + StatusLayout::TAB_WIDTH as f32 / 2.0,
                            tab_y + StatusLayout::TAB_HEIGHT as f32 / 2.0,
                        ],
                    ),
                    Some(tab)
                );
            }

            let clip = status_character_clip(viewport);
            assert_eq!(clip[2], StatusLayout::PAPER_WIDTH as u32);
            assert_eq!(clip[3], StatusLayout::PAPER_HEIGHT as u32);
            assert!(clip[1] + clip[3] <= (top + StatusLayout::FOOTER_Y as f32) as u32);

            for slot in 0..StatusLayout::SLOT_POSITIONS.len() {
                let position = StatusLayout::slot_position(slot);
                if position == 0 {
                    continue;
                }
                let rect = StatusLayout::slot_rect(slot);
                assert_eq!(
                    status_equipment_slot_at(
                        viewport,
                        [
                            left + StatusLayout::PAPER_X as f32 + rect.x as f32 + 19.0,
                            top + StatusLayout::PAPER_Y as f32 + rect.y as f32 + 19.0,
                        ],
                    ),
                    Some(position)
                );
            }
        }
    }

    #[test]
    fn item_context_menu_reserves_header_and_maps_every_row() {
        let viewport = [1024.0, 768.0];
        let anchor = [300.0, 200.0];
        let rows = 5;
        let [left, top] = super::item_context_position(viewport, anchor, rows);
        assert_eq!(
            super::item_context_button_at(viewport, [left + 20.0, top + 12.0], anchor, rows),
            None
        );
        for index in 0..rows {
            let y = top + 30.0 + index as f32 * 34.0 + 17.0;
            assert_eq!(
                super::item_context_button_at(viewport, [left + 40.0, y], anchor, rows),
                Some(index)
            );
        }
    }

    #[test]
    fn choice_menu_maps_rows_and_stays_in_the_window() {
        let viewport = [1024.0, 768.0];
        // Opened near the bottom-right corner, it moves back inside.
        let menu = super::ChoiceMenu::at(viewport, [1020.0, 760.0], "x".into(), vec!["A", "B", "C"]);
        let [left, top] = menu.position.map(|value| value as f32);
        assert!(left + 150.0 <= viewport[0] && top + 30.0 + 3.0 * 34.0 <= viewport[1]);
        assert_eq!(menu.row_at([left + 20.0, top + 12.0]), None);
        for row in 0..3 {
            let y = top + 30.0 + row as f32 * 34.0 + 17.0;
            assert_eq!(menu.row_at([left + 40.0, y]), Some(row));
        }
        assert_eq!(menu.row_at([left - 5.0, top + 47.0]), None);
        assert!(!menu.contains([left + 40.0, top + 30.0 + 3.0 * 34.0 + 1.0]));
        let modes = super::ChoiceMenu::kill_modes(viewport, 1);
        assert_eq!((modes.labels.len(), modes.marked), (4, Some(1)));
    }

    #[test]
    fn gem_selection_panel_exposes_both_sockets_and_inventory_gems() {
        let viewport = [1024.0, 768.0];
        let [left, top] = super::gem_selection_panel_position(viewport);
        let inventory = [
            crate::network::OwnedItem {
                uid: 701,
                item_type: 700001,
                position: 0,
                durability: 0,
                max_durability: 0,
                plus: 0,
                rarity: 0,
                gem_one: 0,
                gem_two: 0,
                rarity_roll: 0,
                bless: 0,
                enchant_hp: 0,
                color: 0,
            },
            crate::network::OwnedItem {
                uid: 702,
                item_type: 700002,
                position: 1,
                durability: 0,
                max_durability: 0,
                plus: 0,
                rarity: 0,
                gem_one: 0,
                gem_two: 0,
                rarity_roll: 0,
                bless: 0,
                enchant_hp: 0,
                color: 0,
            },
        ];
        assert_eq!(
            super::gem_selection_panel_button_at(viewport, [left + 45.0, top + 130.0], &inventory,),
            Some(super::GemSelectionPanelButton::Socket(1))
        );
        assert_eq!(
            super::gem_selection_panel_button_at(viewport, [left + 245.0, top + 130.0], &inventory,),
            Some(super::GemSelectionPanelButton::Socket(2))
        );
        assert_eq!(
            super::gem_selection_panel_button_at(viewport, [left + 50.0, top + 200.0], &inventory,),
            Some(super::GemSelectionPanelButton::Gem(701))
        );
        assert_eq!(
            super::gem_selection_panel_button_at(viewport, [left + 152.0, top + 200.0], &inventory,),
            Some(super::GemSelectionPanelButton::Gem(702))
        );
        assert!(super::gem_selection_panel_close_at(
            viewport,
            [left + 420.0, top + 15.0]
        ));
    }

    #[test]
    fn rarity_panel_has_upgrade_and_reroll_actions_without_tier_buttons() {
        let viewport = [1024.0, 768.0];
        let [left, top] =
            super::item_action_panel_position(viewport, super::ItemContextAction::Rarity);
        assert_eq!(
            super::item_action_panel_button_at(
                viewport,
                [left + 90.0, top + 332.0],
                super::ItemContextAction::Rarity,
            ),
            Some(super::ItemActionPanelButton::Reroll)
        );
        assert_eq!(
            super::item_action_panel_button_at(
                viewport,
                [left + 260.0, top + 332.0],
                super::ItemContextAction::Rarity,
            ),
            Some(super::ItemActionPanelButton::Confirm)
        );
        assert_eq!(super::rarity_label(2), "EPIC");
        assert_eq!(super::rarity_label(3), "LEGENDARY");
    }

    #[test]
    fn every_item_action_panel_has_confirm_cancel_and_close_targets() {
        let viewport = [1024.0, 768.0];
        let actions = [
            super::ItemContextAction::Upgrade,
            super::ItemContextAction::Rarity,
            super::ItemContextAction::ImproveQuality,
            super::ItemContextAction::ApplyRefinery,
            super::ItemContextAction::ApplyArtifact,
            super::ItemContextAction::OpenSocket,
            super::ItemContextAction::InsertGem,
            super::ItemContextAction::RemoveGem,
        ];
        let item = crate::network::OwnedItem {
            uid: 1,
            item_type: 410005,
            position: 0,
            durability: 100,
            max_durability: 100,
            plus: 0,
            rarity: 0,
            gem_one: 255,
            gem_two: 0,
            rarity_roll: 0,
            bless: 0,
            enchant_hp: 0,
            color: 0,
        };
        for action in actions {
            let [left, top] = super::item_action_panel_position(viewport, action);
            assert_eq!(
                super::item_action_panel_button_at(
                    viewport,
                    [
                        left + if action == super::ItemContextAction::Upgrade {
                            194.0
                        } else if action == super::ItemContextAction::Rarity {
                            260.0
                        } else {
                            80.0
                        },
                        top + if action == super::ItemContextAction::Upgrade {
                            294.0
                        } else if action == super::ItemContextAction::Rarity {
                            332.0
                        } else {
                            188.0
                        },
                    ],
                    action,
                ),
                Some(super::ItemActionPanelButton::Confirm)
            );
            assert!(
                super::item_action_panel_close_at(viewport, [left + 10.0, top + 10.0], action)
                    == false
            );
            let close_x = if action == super::ItemContextAction::Upgrade {
                500.0
            } else if action == super::ItemContextAction::Rarity {
                490.0
            } else {
                270.0
            };
            assert!(super::item_action_panel_close_at(
                viewport,
                [left + close_x, top + 15.0],
                action
            ));
            let _ = super::ItemActionPanel {
                item,
                action,
                selected_rarity: 0,
            };
        }
    }

    #[test]
    fn action_panel_rejects_invalid_quality_without_catalyst() {
        let item = crate::network::OwnedItem {
            uid: 1,
            item_type: 410003,
            position: 0,
            durability: 100,
            max_durability: 100,
            plus: 0,
            rarity: 0,
            gem_one: 0,
            gem_two: 0,
            rarity_roll: 0,
            bless: 0,
            enchant_hp: 0,
            color: 0,
        };
        let panel = super::ItemActionPanel {
            item,
            action: super::ItemContextAction::ImproveQuality,
            selected_rarity: 0,
        };
        assert!(!super::item_action_panel_confirm_enabled(panel, &[]));
        let valid = super::ItemActionPanel {
            item: crate::network::OwnedItem {
                item_type: 410005,
                ..item
            },
            action: super::ItemContextAction::ImproveQuality,
            selected_rarity: 0,
        };
        assert!(!super::item_action_panel_confirm_enabled(valid, &[]));
        let dragon_ball = crate::network::OwnedItem {
            item_type: 1_088_000,
            ..item
        };
        assert!(super::item_action_panel_confirm_enabled(
            valid,
            &[dragon_ball]
        ));
    }

    #[test]
    fn title_buttons_have_stable_hitboxes() {
        let viewport = [1366.0, 768.0];
        let [left, top] = status_position(viewport);
        let x = left + StatusLayout::INFO_X as f32 + 12.0;
        let first_y = top + StatusLayout::BASIC_Y as f32 + 86.0;
        assert_eq!(status_title_at(viewport, [x, first_y]), Some(0));
        assert_eq!(
            status_title_at(viewport, [x, first_y + 24.0 * 5.0]),
            Some(5)
        );
        assert_eq!(status_title_at(viewport, [x, first_y - 20.0]), None);
    }

    #[test]
    fn battle_power_counts_level_reborns_nobility_and_items() {
        let item = |position, item_type, plus, gem_one, gem_two| crate::network::OwnedItem {
            uid: 1,
            item_type,
            position,
            durability: 0,
            max_durability: 0,
            plus,
            rarity: 0,
            gem_one,
            gem_two,
            rarity_roll: 0,
            bless: 0,
            enchant_hp: 0,
            color: 0,
        };
        assert_eq!(super::battle_power(24, 1, &[], 0), 29);
        // Super armor (+4), plus 3, one normal and one super gem (+3).
        let armor = item(3, 130_009, 3, 1, 13);
        assert_eq!(super::battle_power(24, 1, &[armor.clone()], 12), 29 + 5 + 10);
        // Garments do not count; an empty socket (255) does not either.
        let garment = item(9, 181_309, 9, 255, 0);
        assert_eq!(super::battle_power(1, 1, &[garment], 0), 6);
        // A two-handed weapon counts twice unless an off-hand is worn.
        let glaive = item(4, 510_009, 0, 0, 0);
        assert_eq!(super::battle_power(1, 1, &[glaive.clone()], 0), 6 + 8);
        let shield = item(5, 900_005, 0, 0, 0);
        assert_eq!(super::battle_power(1, 1, &[glaive, shield], 0), 6 + 4);
    }

    #[test]
    fn role_attribute_buttons_follow_native_gui_coordinates() {
        let viewport = [1024.0, 768.0];
        let [left, top] = status_position(viewport);
        let role_left = left + StatusLayout::PAPER_X as f32;
        let role_top = top + StatusLayout::PAPER_Y as f32;
        // GUI.ini [132-1565] strength, [132-1562] agility, [132-1563]
        // vitality and [132-1564] spirit; each sits right of its value.
        for (attribute, x, y) in [
            ("strength", 81.0, 199.0),
            ("agility", 175.0, 199.0),
            ("vitality", 81.0, 230.0),
            ("spirit", 175.0, 230.0),
        ] {
            assert_eq!(
                super::status_attribute_button_at(
                    viewport,
                    [role_left + x + 9.0, role_top + y + 9.0,]
                ),
                Some(attribute)
            );
        }
        assert_eq!(
            super::status_attribute_button_at(viewport, [role_left + 10.0, role_top + 10.0]),
            None
        );
    }

    #[test]
    fn reputation_donation_button_has_a_stable_hitbox() {
        let viewport = [1024.0, 768.0];
        let [left, top] = status_position(viewport);
        assert!(status_reputation_donate_at(
            viewport,
            [
                left + StatusLayout::INFO_X as f32 + 71.0,
                top + StatusLayout::BASIC_Y as f32 + 192.0
            ]
        ));
        assert!(!status_reputation_donate_at(
            viewport,
            [left + 1.0, top + 1.0]
        ));
    }

    #[test]
    fn inventory_filters_match_classic_item_ranges() {
        assert!(InventoryFilter::Equipment.accepts(410339));
        assert!(InventoryFilter::Consumables.accepts(1_050_000));
        assert!(InventoryFilter::Consumables.accepts(720_010));
        assert!(InventoryFilter::Materials.accepts(1_088_001));
        assert!(InventoryFilter::Quest.accepts(1_090_000));
        assert!(!InventoryFilter::Equipment.accepts(1_088_001));
    }

    #[test]
    fn inventory_layout_keeps_close_filters_and_slots_in_the_panel() {
        for viewport in [[1024.0, 768.0], [1366.0, 768.0], [1704.0, 960.0]] {
            let [left, top] = inventory_position(viewport);
            assert_eq!(left, (viewport[0] - INVENTORY_WIDTH as f32).max(0.0));
            assert!(inventory_panel_at(viewport, [left + 136.0, top + 256.0]));
            // The original Close button at the bottom ([153-1128]).
            assert!(inventory_close_at(viewport, [left + 200.0, top + 457.0]));
            assert!(!inventory_close_at(viewport, [left + 248.0, top + 26.0]));
            assert!(inventory_organize_at(viewport, [left + 201.0, top + 20.0]));
            for (index, filter) in InventoryFilter::ALL.into_iter().enumerate() {
                assert_eq!(
                    inventory_filter_at(
                        viewport,
                        [
                            left + INVENTORY_FILTER_X as f32
                                + index as f32 * INVENTORY_FILTER_STEP_X as f32
                                + INVENTORY_FILTER_WIDTH as f32 / 2.0,
                            top + INVENTORY_FILTER_Y as f32 + INVENTORY_FILTER_HEIGHT as f32 / 2.0,
                        ],
                    ),
                    Some(filter)
                );
            }
            assert_eq!(
                inventory_slot_at(
                    viewport,
                    [
                        left + INVENTORY_SLOT_X as f32 + INVENTORY_SLOT_SIZE as f32 / 2.0,
                        top + INVENTORY_SLOT_Y as f32
                            + 7.0 * INVENTORY_SLOT_STEP_Y as f32
                            + INVENTORY_SLOT_SIZE as f32 / 2.0,
                    ]
                ),
                Some(35)
            );
        }
    }

    #[test]
    fn inventory_slot_hitbox_excludes_the_gap_between_cells() {
        let viewport = [1024.0, 768.0];
        let [left, top] = inventory_position(viewport);
        assert_eq!(
            inventory_slot_at(
                viewport,
                [
                    left + INVENTORY_SLOT_X as f32 + 19.0,
                    top + INVENTORY_SLOT_Y as f32 + 19.0,
                ]
            ),
            Some(0)
        );
        assert_eq!(
            inventory_slot_at(
                viewport,
                [
                    left + INVENTORY_SLOT_X as f32 + INVENTORY_SLOT_STEP_X as f32 - 0.5,
                    top + INVENTORY_SLOT_Y as f32 + 19.0,
                ]
            ),
            None
        );
    }

    #[test]
    fn inventory_hover_only_resolves_visible_items_and_keeps_slot_geometry_shared() {
        let viewport = [1024.0, 768.0];
        let item = crate::network::OwnedItem {
            uid: 7,
            item_type: 410_339,
            position: 6,
            durability: 100,
            max_durability: 100,
            plus: 0,
            rarity: 0,
            gem_one: 0,
            gem_two: 0,
            rarity_roll: 0,
            bless: 0,
            enchant_hp: 0,
            color: 0,
        };
        let [left, top] = inventory_position(viewport);
        let cursor = [
            left + INVENTORY_SLOT_X as f32 + INVENTORY_SLOT_STEP_X as f32 + 19.5,
            top + INVENTORY_SLOT_Y as f32 + INVENTORY_SLOT_STEP_Y as f32 + 19.5,
        ];
        assert_eq!(
            inventory_item_at(&[item], InventoryFilter::All, viewport, cursor),
            Some(item)
        );
        assert_eq!(
            inventory_slot_position(viewport, item.position),
            [
                left + INVENTORY_SLOT_X as f32 + INVENTORY_SLOT_STEP_X as f32 + 1.0,
                top + INVENTORY_SLOT_Y as f32 + INVENTORY_SLOT_STEP_Y as f32 + 1.0,
            ]
        );
        assert_eq!(
            inventory_item_at(&[item], InventoryFilter::Materials, viewport, cursor),
            None
        );
    }

    #[test]
    fn consumable_catalog_separates_health_and_mana_potions() {
        assert_eq!(health_recovery_for_item(1_000_000), Some(70));
        assert_eq!(health_recovery_for_item(720_010), None);
        assert_eq!(mana_recovery_for_item(1_001_000), Some(70));
        assert_eq!(mana_recovery_for_item(1_002_040), Some(4_500));
        assert_eq!(mana_recovery_for_item(720_013), None);
        assert_eq!(consumable_bundle_for_item(720_010), Some((1_000_030, 3)));
        assert_eq!(consumable_bundle_for_item(720_017), Some((1_002_040, 3)));
        assert!(is_consumable(1_000_030));
        assert!(is_consumable(1_001_030));
        assert!(is_consumable(720_010));
        assert!(temporary_effect_for_item(723_017));
        assert!(temporary_effect_for_item(1_200_000));
        assert!(temporary_effect_for_item(1_200_002));
        assert!(!is_consumable(410_003));
    }

    #[test]
    fn finds_item_min_icon_sections_without_a_quality_prefix() {
        let animation = "[Item725005]\nFrameAmount=1\nFrame0=data/ItemMinIcon/1030010.dds\n[Item725010]\nFrame0=data/ItemMinIcon/1030020.dds";
        assert_eq!(
            animation_frame(animation, "[Item725005]"),
            Some("data/ItemMinIcon/1030010.dds")
        );
        assert_eq!(
            animation_frame(animation, "[Item72501]"),
            None,
            "section lookup must not match a longer item ID"
        );
    }

    #[test]
    fn finds_map_item_icon_sections() {
        let animation = "[725005]\nFrameAmount=1\nFrame0=data/MapItemIcon/1030010.dds";
        assert_eq!(
            animation_frame(animation, "[725005]"),
            Some("data/MapItemIcon/1030010.dds")
        );
    }

    #[test]
    fn team_health_fraction_is_safe_for_empty_and_overflowing_values() {
        assert_eq!(team_health_fraction(0, 100), 0.0);
        assert_eq!(team_health_fraction(50, 100), 0.5);
        assert_eq!(team_health_fraction(150, 100), 1.0);
        assert_eq!(team_health_fraction(10, 0), 0.0);
    }

    #[test]
    fn player_shop_panel_actions_keep_open_query_and_listing_zones_stable() {
        let viewport = [1000.0, 800.0];
        let [left, top] = super::quick_panel_position(viewport, super::QuickPanel::Market);
        assert!(player_shop_price_at(viewport, [left + 300.0, top + 90.0]));
        assert!(!player_shop_price_at(viewport, [left + 300.0, top + 120.0]));
        let listings = vec![crate::network::PlayerShopItem {
            owner_id: 7,
            price: 1_000,
            item: crate::network::OwnedItem {
                uid: 42,
                item_type: 410003,
                position: 0,
                durability: 100,
                max_durability: 100,
                plus: 0,
                rarity: 0,
                gem_one: 0,
                gem_two: 0,
                rarity_roll: 0,
                bless: 0,
                enchant_hp: 0,
                color: 0,
            },
        }];
        assert_eq!(
            player_shop_panel_action_at(
                viewport,
                [left + 30.0, top + 125.0],
                false,
                &[],
                Some(9),
                false,
            ),
            Some(PlayerShopPanelAction::Open)
        );
        assert_eq!(
            player_shop_panel_action_at(
                viewport,
                [left + 30.0, top + 158.0],
                true,
                &[],
                Some(9),
                true,
            ),
            Some(PlayerShopPanelAction::QueryTarget)
        );
        assert_eq!(
            player_shop_panel_action_at(
                viewport,
                [left + 30.0, top + 195.0],
                true,
                &listings,
                Some(9),
                false,
            ),
            Some(PlayerShopPanelAction::Buy(0))
        );
        assert_eq!(
            player_shop_panel_action_at(
                viewport,
                [left + 30.0, top + 195.0],
                true,
                &listings,
                Some(7),
                false,
            ),
            Some(PlayerShopPanelAction::Remove(0))
        );
    }

    #[test]
    #[ignore = "Requires a local 5065 installation; exports the character UI preview"]
    fn export_character_preview() -> super::Result<()> {
        let root = std::env::var("GRAVAK_ASSETS")?;
        let output = std::path::PathBuf::from(std::env::var("GRAVAK_CHARACTER_PREVIEW")?);
        std::fs::create_dir_all(&output)?;
        let mut assets = crate::assets::Assets::open(std::path::Path::new(&root))?;
        let mut character = crate::character::Character::load(&mut assets, 2)?;
        character.name = "Ikzordep".into();
        character.strength = 40;
        character.agility = 15;
        character.vitality = 24;
        let equipment = status_equipment_items(&character, &[], &[false; 10]);
        for (tab, name) in [
            (StatusTab::Equipment, "equipment"),
            (StatusTab::Attributes, "attributes"),
            (StatusTab::Weapon, "weapon"),
            (StatusTab::Skills, "skills"),
        ] {
            // Same layers, in the same order, as the renderer.
            let mut image = status_base_image(&mut assets, tab, super::WeaponFilter::OneHand, false)?;
            let skills = [(1_000, 3, 120), (1_001, 1, 15)];
            let values = status_dynamic_image(
                &mut assets,
                24,
                0,
                0,
                &character,
                tab,
                0,
                0,
                &skills,
                &equipment,
                &[(410, 1, 110), (420, 12, 3_500), (480, 3, 90)],
                super::WeaponFilter::OneHand,
                Some(1_000),
            )?;
            image::imageops::overlay(&mut image, &values, 0, 0);
            if tab == StatusTab::Equipment {
                let items = status_equipment_overlay_image(&mut assets, &equipment, Some(4))?;
                image::imageops::overlay(&mut image, &items, 0, 0);
            }
            image.save(output.join(format!("character-{name}.png")))?;
        }
        Ok(())
    }

    #[test]
    #[ignore = "Requires a local 5065 installation; exports the NPC shop preview"]
    fn export_shop_preview() -> super::Result<()> {
        let root = std::env::var("GRAVAK_ASSETS")?;
        let output = std::path::PathBuf::from(std::env::var("GRAVAK_SHOP_PREVIEW")?);
        std::fs::create_dir_all(&output)?;
        let mut assets = crate::assets::Assets::open(std::path::Path::new(&root))?;
        let mut state = super::ShopState::load(&mut assets, 3)?.ok_or("shop 3 is missing")?;
        state.hover = Some(1);
        state.selected = Some(2);
        super::panel_image(&mut assets, &state)?.save(output.join("shop.png"))?;
        let info = super::item_info(&mut assets, 410_339);
        super::equipment_tooltip_image(7, 410_339, info.as_ref(), 70, 70, 12, 13, 13, 2, 9, 3, 120)?
            .save(output.join("tooltip.png"))?;
        Ok(())
    }

    #[test]
    #[ignore = "Requires a local 5065 installation; exports the inventory UI preview"]
    fn export_inventory_preview() -> super::Result<()> {
        let root = std::env::var("GRAVAK_ASSETS")?;
        let output = std::path::PathBuf::from(std::env::var("GRAVAK_INVENTORY_PREVIEW")?);
        std::fs::create_dir_all(&output)?;
        let mut assets = crate::assets::Assets::open(std::path::Path::new(&root))?;
        inventory_image(
            &mut assets,
            &[],
            123_456,
            0,
            None,
            InventoryFilter::All,
            None,
            false,
        )?
        .save(output.join("inventory-empty.png"))?;
        let filled = [
            crate::network::OwnedItem {
                uid: 1,
                item_type: 410_339,
                position: 0,
                durability: 100,
                max_durability: 100,
                plus: 0,
                rarity: 0,
                gem_one: 0,
                gem_two: 0,
                rarity_roll: 0,
                bless: 0,
                enchant_hp: 0,
                color: 0,
            },
            crate::network::OwnedItem {
                uid: 2,
                item_type: 410_003,
                position: 1,
                durability: 70,
                max_durability: 100,
                plus: 2,
                rarity: 0,
                gem_one: 0,
                gem_two: 0,
                rarity_roll: 0,
                bless: 0,
                enchant_hp: 0,
                color: 0,
            },
            crate::network::OwnedItem {
                uid: 3,
                item_type: 1_088_001,
                position: 2,
                durability: 0,
                max_durability: 0,
                plus: 0,
                rarity: 0,
                gem_one: 0,
                gem_two: 0,
                rarity_roll: 0,
                bless: 0,
                enchant_hp: 0,
                color: 0,
            },
            crate::network::OwnedItem {
                uid: 4,
                item_type: 1_088_000,
                position: 6,
                durability: 0,
                max_durability: 0,
                plus: 0,
                rarity: 0,
                gem_one: 0,
                gem_two: 0,
                rarity_roll: 0,
                bless: 0,
                enchant_hp: 0,
                color: 0,
            },
            crate::network::OwnedItem {
                uid: 5,
                item_type: 181_305,
                position: 17,
                durability: 100,
                max_durability: 100,
                plus: 1,
                rarity: 0,
                gem_one: 0,
                gem_two: 0,
                rarity_roll: 0,
                bless: 0,
                enchant_hp: 0,
                color: 0,
            },
        ];
        inventory_image(
            &mut assets,
            &filled,
            987_654,
            1_250,
            Some(2),
            InventoryFilter::All,
            None,
            false,
        )?
        .save(output.join("inventory-filled.png"))?;
        Ok(())
    }
}
