//! Fixed logical layout for the original 5065 client UI.
//!
//! The original client does not position each child relative to the current
//! window independently. It places panels in a 1024x768 logical canvas and
//! anchors only the panels that belong to an edge. Keeping that contract in a
//! small module prevents rendering and hit-testing from drifting apart.

pub const LOGICAL_WIDTH: f32 = 1024.0;
pub const LOGICAL_HEIGHT: f32 = 768.0;
pub const MAIN_HUD_WIDTH: f32 = 1024.0;
pub const MAIN_HUD_HEIGHT: f32 = 141.0;

/// The control rectangles from `R:\5065\ini\GUI.ini` `[0-130]`.
///
/// These are deliberately kept in the same logical space as the original
/// client.  The main HUD is centered horizontally at runtime, while its
/// children stay at these exact offsets.  Hit-testing must use this table too;
/// otherwise a scaled HUD can look correct and still click the wrong button.
pub const MAIN_HUD_ORIGIN_Y: f32 = 627.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MainHudControl {
    Skill,
    Items,
    Setup,
    Action,
    Chat,
    Group,
    Run,
    Pk,
    Map,
    NpcEquip,
    Exp,
    Magic,
    ScreenMove,
    Faction,
    Friend,
    Query,
    LeftOrb,
    RightOrb,
    LeftStatus,
    RightStatus,
}

impl MainHudControl {
    pub const fn id(self) -> u16 {
        match self {
            Self::Skill => 1006,
            Self::Items => 1007,
            Self::Setup => 1008,
            Self::Action => 1009,
            Self::Chat => 1010,
            Self::Group => 1011,
            Self::Run => 1012,
            Self::Pk => 1013,
            Self::Map => 1015,
            Self::NpcEquip => 1016,
            Self::Exp => 1019,
            Self::Magic => 1022,
            Self::ScreenMove => 1023,
            Self::Faction => 1024,
            Self::Friend => 1026,
            Self::Query => 1503,
            Self::LeftOrb => 1018,
            Self::RightOrb => 1025,
            Self::LeftStatus => 1501,
            Self::RightStatus => 1502,
        }
    }
}

pub const CHARACTER_WIDTH: f32 = 264.0;
pub const CHARACTER_DIALOG_HEIGHT: f32 = 406.0;
// Keep the character window compact like the original dialog. The paper-doll
// clip already fits inside this height; extending it made the panel collide
// with the HUD and the inventory at common resolutions.
pub const CHARACTER_CANVAS_HEIGHT: f32 = CHARACTER_DIALOG_HEIGHT;

pub const INVENTORY_WIDTH: f32 = 270.0;
pub const INVENTORY_HEIGHT: f32 = 502.0;

pub const CHARACTER_PAPER: [f32; 4] = [40.0, 55.0, 206.0, 332.0];
pub const INVENTORY_GRID: [f32; 4] = [26.0, 28.0, 40.0, 40.0];
pub const INVENTORY_GRID_STEP: [f32; 2] = [42.0, 42.0];
pub const INVENTORY_ICON_SIZE: f32 = 32.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn contains(self, point: [f32; 2]) -> bool {
        point[0] >= self.x
            && point[0] < self.x + self.width
            && point[1] >= self.y
            && point[1] < self.y + self.height
    }
}

pub fn character_position(viewport: [f32; 2]) -> [f32; 2] {
    [
        0.0,
        ((viewport[1] - CHARACTER_CANVAS_HEIGHT) * 0.5).max(8.0),
    ]
}

pub fn inventory_position(viewport: [f32; 2]) -> [f32; 2] {
    [
        (viewport[0] - INVENTORY_WIDTH).max(0.0),
        ((viewport[1] - INVENTORY_HEIGHT) * 0.5).max(8.0),
    ]
}

pub fn character_rect(viewport: [f32; 2]) -> Rect {
    let [x, y] = character_position(viewport);
    Rect::new(x, y, CHARACTER_WIDTH, CHARACTER_DIALOG_HEIGHT)
}

pub fn character_canvas_rect(viewport: [f32; 2]) -> Rect {
    let [x, y] = character_position(viewport);
    Rect::new(x, y, CHARACTER_WIDTH, CHARACTER_CANVAS_HEIGHT)
}

pub fn inventory_rect(viewport: [f32; 2]) -> Rect {
    let [x, y] = inventory_position(viewport);
    Rect::new(x, y, INVENTORY_WIDTH, INVENTORY_HEIGHT)
}

pub fn inventory_slot_rect(viewport: [f32; 2], slot: u8) -> Rect {
    let [left, top] = inventory_position(viewport);
    let column = f32::from(slot % 5);
    let row = f32::from(slot / 5);
    Rect::new(
        left + INVENTORY_GRID[0] + column * INVENTORY_GRID_STEP[0],
        top + INVENTORY_GRID[1] + row * INVENTORY_GRID_STEP[1],
        INVENTORY_GRID[2],
        INVENTORY_GRID[3],
    )
}

pub fn main_hud_position(viewport: [f32; 2]) -> [f32; 2] {
    [
        ((viewport[0] - MAIN_HUD_WIDTH) * 0.5).max(0.0),
        (viewport[1] - MAIN_HUD_HEIGHT).max(0.0),
    ]
}

pub fn main_hud_rect(viewport: [f32; 2]) -> Rect {
    let [x, y] = main_hud_position(viewport);
    Rect::new(x, y, MAIN_HUD_WIDTH, MAIN_HUD_HEIGHT)
}

pub fn main_hud_control_rect(viewport: [f32; 2], control: MainHudControl) -> Option<Rect> {
    let [origin_x, origin_y] = main_hud_position(viewport);
    let rect = match control {
        MainHudControl::Skill => Rect::new(702.0, 119.0, 46.0, 22.0),
        MainHudControl::Items => Rect::new(552.0, 94.0, 46.0, 22.0),
        MainHudControl::Setup => Rect::new(652.0, 119.0, 46.0, 22.0),
        MainHudControl::Action => Rect::new(502.0, 119.0, 46.0, 22.0),
        MainHudControl::Chat => Rect::new(552.0, 119.0, 46.0, 22.0),
        MainHudControl::Group => Rect::new(602.0, 119.0, 46.0, 22.0),
        MainHudControl::Run => Rect::new(0.0, 23.0, 22.0, 22.0),
        MainHudControl::Pk => Rect::new(652.0, 94.0, 46.0, 22.0),
        MainHudControl::Map => Rect::new(72.0, 23.0, 22.0, 22.0),
        MainHudControl::NpcEquip => Rect::new(22.0, 11.0, 22.0, 22.0),
        MainHudControl::Exp => Rect::new(99.0, 97.0, 398.0, 8.0),
        MainHudControl::Magic => Rect::new(753.0, 96.0, 47.0, 46.0),
        MainHudControl::ScreenMove => Rect::new(50.0, 11.0, 22.0, 22.0),
        MainHudControl::Faction => Rect::new(702.0, 94.0, 46.0, 22.0),
        MainHudControl::Friend => Rect::new(602.0, 94.0, 46.0, 22.0),
        MainHudControl::Query => Rect::new(502.0, 94.0, 46.0, 22.0),
        MainHudControl::LeftOrb => Rect::new(0.0, 50.0, 92.0, 86.0),
        MainHudControl::RightOrb => Rect::new(42.0, 58.0, 12.0, 78.0),
        MainHudControl::LeftStatus => Rect::new(4.0, 54.0, 40.0, 86.0),
        MainHudControl::RightStatus => Rect::new(52.0, 54.0, 40.0, 86.0),
    };
    Some(Rect::new(
        origin_x + rect.x,
        origin_y + rect.y,
        rect.width,
        rect.height,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn original_panels_use_edge_anchors() {
        assert_eq!(character_position([1912.0, 914.0]), [0.0, 254.0]);
        assert_eq!(inventory_position([1912.0, 914.0]), [1642.0, 206.0]);
    }

    #[test]
    fn inventory_slot_geometry_is_shared() {
        assert_eq!(
            inventory_slot_rect([1024.0, 768.0], 0),
            Rect::new(780.0, 161.0, 40.0, 40.0)
        );
        assert_eq!(
            inventory_slot_rect([1024.0, 768.0], 6),
            Rect::new(822.0, 203.0, 40.0, 40.0)
        );
    }

    #[test]
    fn main_hud_uses_original_bottom_center_anchor() {
        assert_eq!(main_hud_position([1920.0, 1080.0]), [448.0, 939.0]);
        assert_eq!(
            main_hud_rect([1024.0, 768.0]),
            Rect::new(0.0, 627.0, 1024.0, 141.0)
        );
    }

    #[test]
    fn main_hud_controls_keep_gui_ini_offsets() {
        assert_eq!(
            main_hud_control_rect([1024.0, 768.0], MainHudControl::Items),
            Some(Rect::new(552.0, 721.0, 46.0, 22.0))
        );
        assert_eq!(
            main_hud_control_rect([1920.0, 1080.0], MainHudControl::Map),
            Some(Rect::new(520.0, 962.0, 22.0, 22.0))
        );
    }
}
