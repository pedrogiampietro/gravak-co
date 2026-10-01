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

/// Logical y of the main HUD in the 1024x768 canvas (`GUI.ini` `[0-130]`).
pub const MAIN_HUD_ORIGIN_Y: f32 = 627.0;

/// Clickable controls of the original bottom HUD.
///
/// Rectangles come from `R:\5065\ini\GUI.ini` `[130-*]`. The control names in
/// that file do not match the artwork the client binds to them (the
/// `IDC_MAIN_SKILL` slot shows the "Message" button, for example), so the
/// variants are named after the visible label.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MainHudButton {
    Status,
    Items,
    Friend,
    Capture,
    Guild,
    Action,
    Trade,
    Team,
    Options,
    Message,
    Skill,
    Run,
    NpcEquip,
    ScreenMove,
    Map,
}

impl MainHudButton {
    pub const ALL: [Self; 15] = [
        Self::Status,
        Self::Items,
        Self::Friend,
        Self::Capture,
        Self::Guild,
        Self::Action,
        Self::Trade,
        Self::Team,
        Self::Options,
        Self::Message,
        Self::Skill,
        Self::Run,
        Self::NpcEquip,
        Self::ScreenMove,
        Self::Map,
    ];

    /// Rectangle relative to the main HUD origin.
    pub const fn local_rect(self) -> Rect {
        match self {
            Self::Status => Rect::new(502.0, 94.0, 46.0, 22.0),
            Self::Items => Rect::new(552.0, 94.0, 46.0, 22.0),
            Self::Friend => Rect::new(602.0, 94.0, 46.0, 22.0),
            Self::Capture => Rect::new(652.0, 94.0, 46.0, 22.0),
            Self::Guild => Rect::new(702.0, 94.0, 46.0, 22.0),
            Self::Action => Rect::new(502.0, 119.0, 46.0, 22.0),
            Self::Trade => Rect::new(552.0, 119.0, 46.0, 22.0),
            Self::Team => Rect::new(602.0, 119.0, 46.0, 22.0),
            Self::Options => Rect::new(652.0, 119.0, 46.0, 22.0),
            Self::Message => Rect::new(702.0, 119.0, 46.0, 22.0),
            Self::Skill => Rect::new(753.0, 96.0, 47.0, 43.0),
            Self::Run => Rect::new(0.0, 23.0, 22.0, 22.0),
            Self::NpcEquip => Rect::new(22.0, 11.0, 22.0, 22.0),
            Self::ScreenMove => Rect::new(50.0, 11.0, 22.0, 22.0),
            Self::Map => Rect::new(72.0, 23.0, 22.0, 22.0),
        }
    }
}

/// Origin of the chat strip (`GUI.ini` `[0-145]`) relative to the main HUD.
pub const CHAT_STRIP_ORIGIN: [f32; 2] = [82.0, 70.0];

/// Controls of the chat strip, `GUI.ini` `[145-*]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatStripControl {
    List,
    Name,
    Add,
    Talk,
    Input,
    Emotion,
    Send,
    Clear,
    Log,
}

impl ChatStripControl {
    pub const ALL: [Self; 9] = [
        Self::List,
        Self::Name,
        Self::Add,
        Self::Talk,
        Self::Input,
        Self::Emotion,
        Self::Send,
        Self::Clear,
        Self::Log,
    ];

    /// Rectangle relative to the main HUD origin.
    pub const fn local_rect(self) -> Rect {
        let [x, y] = CHAT_STRIP_ORIGIN;
        match self {
            Self::List => Rect::new(x + 17.0, y + 6.0, 16.0, 17.0),
            Self::Name => Rect::new(x + 37.0, y + 1.0, 93.0, 21.0),
            Self::Add => Rect::new(x + 148.0, y + 5.0, 19.0, 15.0),
            Self::Talk => Rect::new(x + 172.0, y + 4.0, 46.0, 19.0),
            Self::Input => Rect::new(x + 230.0, y + 1.0, 327.0, 21.0),
            Self::Emotion => Rect::new(x + 573.0, y + 5.0, 19.0, 19.0),
            Self::Send => Rect::new(x + 597.0, y + 6.0, 16.0, 17.0),
            Self::Clear => Rect::new(x + 619.0, y + 4.0, 46.0, 19.0),
            Self::Log => Rect::new(x + 670.0, y + 4.0, 46.0, 19.0),
        }
    }
}

/// HP/MP orb, `GUI.ini` `[130-1018]`.
pub const ORB_RECT: Rect = Rect::new(0.0, 50.0, 92.0, 86.0);
/// Experience trough, `GUI.ini` `[130-1019]`.
pub const EXP_RECT: Rect = Rect::new(99.0, 96.0, 398.0, 4.0);
/// First F-key slot frame; slots repeat every `HOTKEY_STEP` pixels.
pub const HOTKEY_FIRST: Rect = Rect::new(92.0, 101.0, 38.0, 38.0);
pub const HOTKEY_STEP: f32 = 41.0;

/// Opaque parts of the HUD. The 1024x141 canvas is mostly transparent above
/// the F-key row, and clicks there belong to the world.
const MAIN_HUD_SOLID: [Rect; 3] = [
    Rect::new(0.0, 11.0, 96.0, 130.0),
    Rect::new(82.0, 68.0, 942.0, 26.0),
    Rect::new(92.0, 84.0, 932.0, 57.0),
];

pub const CHARACTER_WIDTH: f32 = 264.0;
pub const CHARACTER_DIALOG_HEIGHT: f32 = 440.0;
pub const CHARACTER_CANVAS_HEIGHT: f32 = CHARACTER_DIALOG_HEIGHT;

pub const INVENTORY_WIDTH: f32 = 270.0;
pub const INVENTORY_HEIGHT: f32 = 502.0;

pub const CHARACTER_PAPER: [f32; 4] = [40.0, 55.0, 206.0, 332.0];
pub const INVENTORY_GRID: [f32; 4] = [26.0, 28.0, 40.0, 40.0];
pub const INVENTORY_GRID_STEP: [f32; 2] = [42.0, 42.0];
pub const INVENTORY_ICON_SIZE: f32 = 38.0;

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

fn main_hud_local(viewport: [f32; 2], rect: Rect) -> Rect {
    let [x, y] = main_hud_position(viewport);
    Rect::new(x + rect.x, y + rect.y, rect.width, rect.height)
}

pub fn main_hud_button_rect(viewport: [f32; 2], button: MainHudButton) -> Rect {
    main_hud_local(viewport, button.local_rect())
}

pub fn chat_strip_rect(viewport: [f32; 2], control: ChatStripControl) -> Rect {
    main_hud_local(viewport, control.local_rect())
}

pub fn orb_rect(viewport: [f32; 2]) -> Rect {
    main_hud_local(viewport, ORB_RECT)
}

pub fn hotkey_slot_rect(viewport: [f32; 2], index: usize) -> Rect {
    main_hud_local(
        viewport,
        Rect::new(
            HOTKEY_FIRST.x + index as f32 * HOTKEY_STEP,
            HOTKEY_FIRST.y,
            HOTKEY_FIRST.width,
            HOTKEY_FIRST.height,
        ),
    )
}

pub fn main_hud_button_at(viewport: [f32; 2], point: [f32; 2]) -> Option<MainHudButton> {
    MainHudButton::ALL
        .into_iter()
        .find(|&button| main_hud_button_rect(viewport, button).contains(point))
}

pub fn chat_strip_control_at(viewport: [f32; 2], point: [f32; 2]) -> Option<ChatStripControl> {
    ChatStripControl::ALL
        .into_iter()
        .find(|&control| chat_strip_rect(viewport, control).contains(point))
}

pub fn hotkey_slot_at(viewport: [f32; 2], point: [f32; 2]) -> Option<usize> {
    (0..10).find(|&index| hotkey_slot_rect(viewport, index).contains(point))
}

/// True when `point` is over painted HUD artwork rather than the world.
pub fn main_hud_solid_at(viewport: [f32; 2], point: [f32; 2]) -> bool {
    MAIN_HUD_SOLID
        .into_iter()
        .any(|rect| main_hud_local(viewport, rect).contains(point))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn original_panels_use_edge_anchors() {
        assert_eq!(character_position([1912.0, 914.0]), [0.0, 237.0]);
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
            main_hud_button_rect([1024.0, 768.0], MainHudButton::Items),
            Rect::new(552.0, 721.0, 46.0, 22.0)
        );
        assert_eq!(
            main_hud_button_rect([1920.0, 1080.0], MainHudButton::Map),
            Rect::new(520.0, 962.0, 22.0, 22.0)
        );
        assert_eq!(
            chat_strip_rect([1024.0, 768.0], ChatStripControl::Clear),
            Rect::new(701.0, 701.0, 46.0, 19.0)
        );
    }

    #[test]
    fn hit_testing_matches_the_painted_controls() {
        let viewport = [1366.0, 768.0];
        for button in MainHudButton::ALL {
            let r = main_hud_button_rect(viewport, button);
            let center = [r.x + r.width * 0.5, r.y + r.height * 0.5];
            assert_eq!(main_hud_button_at(viewport, center), Some(button));
            assert!(main_hud_solid_at(viewport, center));
        }
        for index in 0..10 {
            let r = hotkey_slot_rect(viewport, index);
            let center = [r.x + r.width * 0.5, r.y + r.height * 0.5];
            assert_eq!(hotkey_slot_at(viewport, center), Some(index));
        }
        let [x, y] = main_hud_position(viewport);
        // Above the F-key row the HUD canvas is transparent.
        assert!(!main_hud_solid_at(viewport, [x + 600.0, y + 20.0]));
    }
}
