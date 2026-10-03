//! In-game HUD: the original bottom bar, the chat log above it and the small
//! overlays (status badges, tooltips, hotkey context menu).
//!
//! The bar itself lives in `main_hud`; this module owns hit testing and the
//! full-viewport overlay texture the renderer uploads.

use crate::assets::{Assets, Result};
use crate::main_hud::{self, MainHudArt, MainHudState};
use crate::original_ui_layout::{self as layout, ChatStripControl, MainHudButton};
use image::{Pixel, Rgba, RgbaImage, imageops};

/// The player's options (the Options window), kept in
/// `settings/options.ini` beside the client. They travel with the HUD and
/// the panels, which redraw when any of them changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HudConfig {
    pub center_arcs: bool,
    pub center_bars: bool,
    pub center_scale: u8,
    pub center_arc_distance: u8,
    /// Background music and sound effect volumes, 0..=100.
    pub music_volume: u8,
    pub sound_volume: u8,
    /// Names over the other players and NPCs.
    pub show_names: bool,
    /// The kill counter beside the Skill button.
    pub kill_counter: bool,
    /// Shadows under every actor ("Full") or only under the hero.
    pub full_shadows: bool,
    /// Map, position, FPS and ping in the top-left corner.
    pub show_info: bool,
    /// Frame cap; 0 follows the monitor (VSync), `FPS_UNLIMITED` draws as
    /// fast as the machine can.
    pub max_fps: u16,
    pub fullscreen: bool,
    /// Window size: 0 the default 1366x768, 1 for 800x600, 2 for 1024x768.
    pub resolution: u8,
    /// The Options window shows its second page ("Other").
    pub options_page: u8,
}

impl Default for HudConfig {
    fn default() -> Self {
        Self {
            center_arcs: true,
            center_bars: true,
            center_scale: 100,
            center_arc_distance: 118,
            music_volume: 60,
            sound_volume: 80,
            show_names: true,
            kill_counter: true,
            full_shadows: true,
            show_info: true,
            max_fps: 0,
            fullscreen: false,
            resolution: 0,
            options_page: 0,
        }
    }
}

/// The window sizes offered by the Options window, by `HudConfig::resolution`.
pub const RESOLUTIONS: [[u32; 2]; 3] = [[1366, 768], [800, 600], [1024, 768]];
/// The frame caps offered by the Options window; 0 is VSync.
pub const FPS_CAPS: [u16; 6] = [0, 30, 60, 120, 144, FPS_UNLIMITED];
pub const FPS_UNLIMITED: u16 = u16::MAX;

/// The pause between frames for a cap: none for VSync (the monitor paces
/// it) and for Unlimited.
pub fn frame_interval(max_fps: u16) -> Option<std::time::Duration> {
    (max_fps != 0 && max_fps != FPS_UNLIMITED)
        .then(|| std::time::Duration::from_secs_f64(1.0 / f64::from(max_fps)))
}

/// Skill state shown by the HUD: the skill bar opened by the "Skill" button
/// (`GUI.ini` `[0-188]` magic row and `[0-189]` XP row), the selected skill
/// and the kill counter.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct SkillHud {
    /// Learned skills as (id, level, experience), sorted by id.
    pub skills: Vec<(u32, u16, u32)>,
    /// Weapon proficiency as (weapon type, level, experience).
    pub weapon_skills: Vec<(u32, u32, u32)>,
    /// Stamina and its cap (100, 150 with Heaven's Blessing).
    pub stamina: u8,
    pub max_stamina: u8,
    /// The XP circle is lit: XP skills can be cast.
    pub xp_ready: bool,
    /// The page selected in the status Weapon tab.
    pub weapon_filter: crate::shop::WeaponFilter,
    pub bar_open: bool,
    pub active_skill: Option<u32>,
    pub kill_count: u32,
    /// The kill mode (`shop::KILL_MODES` order) the Capture button shows.
    pub kill_mode: u8,
    /// The top-left information lines: map and position, FPS and ping.
    pub info: Vec<String>,
    /// Skill under the cursor in the bar; filled in by the renderer.
    pub hovered: Option<u32>,
    /// Dead: seconds until Revive can be pressed (0 when it can).
    pub revive_in: Option<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Rect(i32, i32, i32, i32);

impl Rect {
    fn contains(self, p: [f32; 2]) -> bool {
        p[0] >= self.0 as f32
            && p[0] < (self.0 + self.2) as f32
            && p[1] >= self.1 as f32
            && p[1] < (self.1 + self.3) as f32
    }
    fn inset(self, n: i32) -> Self {
        Self(self.0 + n, self.1 + n, self.2 - n * 2, self.3 - n * 2)
    }
}

impl From<layout::Rect> for Rect {
    fn from(r: layout::Rect) -> Self {
        Self(
            r.x.round() as i32,
            r.y.round() as i32,
            r.width.round() as i32,
            r.height.round() as i32,
        )
    }
}

/// Chat log filter tabs: label and channel (0 shows every channel).
const CHAT_TABS: [(&str, u16); 7] = [
    ("All", 0),
    ("Local", 2000),
    ("World", 2021),
    ("Team", 2003),
    ("Guild", 2004),
    ("Whisper", 2001),
    ("System", 2007),
];
const CHAT_LOG_WIDTH: i32 = 590;
const CHAT_LOG_LINES: usize = 12;
const CHAT_LINE_HEIGHT: i32 = 16;
const CHAT_TAB_HEIGHT: i32 = 20;
const CONTEXT_OPTIONS: [&str; 4] = [
    "Assign Spell",
    "Assign Object",
    "Assign Text",
    "Assign Hotkey",
];
const CONTEXT_ROW: i32 = 22;
const CONTEXT_WIDTH: i32 = 150;

// Hover codes shared between hit testing and painting.
const HOVER_SLOT: u8 = 20;
const HOVER_CHAT: u8 = 40;
const HOVER_TAB: u8 = 60;
const HOVER_ORB: u8 = 80;
const HOVER_ICON: u8 = 100;
const HOVER_REVIVE: u8 = 120;

const SKILL_SLOT: i32 = 50;
const SKILL_ROW_CAPACITY: usize = 10;

/// Skill bar slots: XP skills on the row just above the bar, other skills
/// on the row above it (or on the lower row when there are no XP skills).
/// Rows end at the right edge of the "Skill" button, like the original.
fn skill_bar_slots(assets: &mut Assets, viewport: [f32; 2], hud: &SkillHud) -> Vec<(u32, Rect)> {
    if !hud.bar_open {
        return Vec::new();
    }
    let [x, y] = hud_origin(viewport);
    let skill = MainHudButton::Skill.local_rect();
    let right = x + (skill.x + skill.width).round() as i32;
    let (xp, magic): (Vec<u32>, Vec<u32>) = hud
        .skills
        .iter()
        .map(|(id, _, _)| *id)
        .partition(|id| crate::main_hud::is_xp_skill(assets, *id));
    let lower = y + 16;
    let rows = if xp.is_empty() {
        vec![(magic, lower)]
    } else {
        vec![(xp, lower), (magic, lower - SKILL_SLOT)]
    };
    let mut slots = Vec::new();
    for (ids, top) in rows {
        let count = ids.len().min(SKILL_ROW_CAPACITY) as i32;
        for (index, id) in ids.into_iter().take(SKILL_ROW_CAPACITY).enumerate() {
            let left = right - (count - index as i32) * SKILL_SLOT;
            slots.push((id, Rect(left, top, SKILL_SLOT, SKILL_SLOT)));
        }
    }
    slots
}

/// Skill in the open skill bar under the cursor.
pub fn skill_bar_at(
    assets: &mut Assets,
    viewport: [f32; 2],
    cursor: [f32; 2],
    hud: &SkillHud,
) -> Option<u32> {
    skill_bar_slots(assets, viewport, hud)
        .into_iter()
        .find(|(_, rect)| rect.contains(cursor))
        .map(|(id, _)| id)
}

fn draw_skill_bar(
    img: &mut RgbaImage,
    assets: &mut Assets,
    font: &fontdue::Font,
    viewport: [f32; 2],
    hud: &SkillHud,
) {
    for (id, rect) in skill_bar_slots(assets, viewport, hud) {
        if let Some(icon) = crate::main_hud::skill_icon(assets, id) {
            let size = (rect.2 - 2) as u32;
            let mut icon = imageops::resize(&icon, size, size, imageops::FilterType::Triangle);
            // XP skills stay greyed until the XP circle lights up.
            if !hud.xp_ready && crate::main_hud::is_xp_skill(assets, id) {
                for pixel in icon.pixels_mut() {
                    let grey = ((u16::from(pixel[0]) + u16::from(pixel[1]) + u16::from(pixel[2])) / 6) as u8;
                    *pixel = image::Rgba([grey, grey, grey, pixel[3]]);
                }
            }
            imageops::overlay(img, &icon, i64::from(rect.0 + 1), i64::from(rect.1 + 1));
        }
        if hud.active_skill == Some(id) {
            outline(img, rect, [255, 215, 108, 255]);
        } else if hud.hovered == Some(id) {
            outline(img, rect.inset(1), [255, 240, 190, 200]);
        }
    }
    if let Some((id, level, experience)) = hud
        .hovered
        .and_then(|id| hud.skills.iter().find(|(skill, _, _)| *skill == id))
        .copied()
        && let Some((_, rect)) = skill_bar_slots(assets, viewport, hud)
            .into_iter()
            .find(|(skill, _)| *skill == id)
    {
        tooltip(
            img,
            font,
            &[
                (&crate::magic::skill_name(assets, id), [255, 255, 0, 255]),
                (&format!("Level: {level}"), [255, 255, 255, 255]),
                (&format!("Experience: {experience}"), [255, 255, 255, 255]),
            ],
            [rect.0, rect.1 - 2],
        );
    }
}

/// The KO counter ("<n> 斩") at the right edge above the bar.
fn draw_kill_counter(img: &mut RgbaImage, viewport: [f32; 2], kills: u32) {
    let (Ok(digits), Ok(glyph)) = (crate::main_hud::italic_font(), crate::main_hud::client_font())
    else {
        return;
    };
    let [_, hud_y] = hud_origin(viewport);
    let right = viewport[0] as i32 - 10;
    let number = kills.to_string();
    let glyph_x = right - 34;
    let number_width = crate::main_hud::text_width(digits, &number, 40.0);
    crate::main_hud::glow_text(
        img,
        digits,
        &number,
        40.0,
        [glyph_x - 6 - number_width, hud_y - 4],
        [150, 110, 255],
    );
    crate::main_hud::glow_text(img, glyph, "斩", 34.0, [glyph_x, hud_y], [150, 110, 255]);
}

fn hud_origin(viewport: [f32; 2]) -> [i32; 2] {
    let [x, y] = layout::main_hud_position(viewport);
    [x.round() as i32, y.round() as i32]
}

/// The chat log sits at the left edge above the bar, like the original.
fn chat_log_rect(viewport: [f32; 2]) -> Rect {
    let [hud_x, hud_y] = hud_origin(viewport);
    // Where the bar reaches under the log, keep clear of the orb's small
    // buttons (their tops sit 11px into the bar).
    let bottom = if hud_x < CHAT_LOG_WIDTH {
        hud_y + 8
    } else {
        hud_y + 18
    };
    let height = CHAT_TAB_HEIGHT + 10 + CHAT_LOG_LINES as i32 * CHAT_LINE_HEIGHT + 6;
    Rect(0, bottom - height, CHAT_LOG_WIDTH, height)
}

fn chat_tab_rect(viewport: [f32; 2], index: usize) -> Rect {
    let log = chat_log_rect(viewport);
    let font = main_hud::font().ok();
    let mut x = log.0 + 6;
    for (current, (label, _)) in CHAT_TABS.into_iter().enumerate() {
        let width = font.map_or(48, |font| main_hud::text_width(font, label, 12.0) + 18);
        if current == index {
            return Rect(x, log.1 + 6, width, CHAT_TAB_HEIGHT);
        }
        x += width + 2;
    }
    Rect(0, 0, 0, 0)
}

fn hotkey_rect(viewport: [f32; 2], index: usize) -> Rect {
    layout::hotkey_slot_rect(viewport, index).into()
}

fn action_context_menu_rect(viewport: [f32; 2], slot: usize) -> Rect {
    let anchor = hotkey_rect(viewport, slot.min(9));
    let height = CONTEXT_ROW * CONTEXT_OPTIONS.len() as i32 + 8;
    let x = (anchor.0 + anchor.2 / 2 - CONTEXT_WIDTH / 2)
        .clamp(4, (viewport[0] as i32 - CONTEXT_WIDTH - 4).max(4));
    Rect(x, anchor.1 - height - 26, CONTEXT_WIDTH, height)
}

pub fn action_slot_at(viewport: [f32; 2], cursor: [f32; 2]) -> Option<usize> {
    layout::hotkey_slot_at(viewport, cursor)
}

pub fn action_context_option_at(
    viewport: [f32; 2],
    cursor: [f32; 2],
    slot: usize,
) -> Option<usize> {
    let menu = action_context_menu_rect(viewport, slot);
    if !menu.contains(cursor) {
        return None;
    }
    let option = (cursor[1] as i32 - menu.1 - 4).div_euclid(CONTEXT_ROW);
    (0..CONTEXT_OPTIONS.len() as i32)
        .contains(&option)
        .then_some(option as usize)
}

pub fn position(_viewport: [f32; 2]) -> [f32; 2] {
    [0.0, 0.0]
}

pub fn render_width(viewport_width: u32) -> u32 {
    viewport_width.max(1)
}

pub fn render_height(viewport_height: u32) -> u32 {
    viewport_height.max(1)
}

/// True when the cursor is over HUD artwork and must not reach the world.
pub fn contains(viewport: [f32; 2], cursor: [f32; 2]) -> bool {
    if cursor[0] < 0.0 || cursor[1] < 0.0 || cursor[0] >= viewport[0] || cursor[1] >= viewport[1] {
        return false;
    }
    layout::main_hud_solid_at(viewport, cursor)
        || chat_tab_at(viewport, cursor).is_some()
        || crate::hud_icons::icon_at(viewport, cursor).is_some()
}

/// The feature icon (Bounty, VIP, ...) under the cursor.
pub fn feature_icon_at(viewport: [f32; 2], cursor: [f32; 2]) -> Option<crate::hud_icons::FeatureIcon> {
    crate::hud_icons::icon_at(viewport, cursor).map(|index| crate::hud_icons::FeatureIcon::ALL[index])
}

/// The Revive button shown while dead, above the kill counter.
fn revive_button_rect(viewport: [f32; 2]) -> Rect {
    let [_, hud_y] = hud_origin(viewport);
    Rect(viewport[0] as i32 - 10 - REVIVE_WIDTH, hud_y - 54, REVIVE_WIDTH, 40)
}

const REVIVE_WIDTH: i32 = 156;

pub fn revive_button_at(viewport: [f32; 2], cursor: [f32; 2]) -> bool {
    revive_button_rect(viewport).contains(cursor)
}

/// A plate like the original's: the panels' teal lattice in a double gold
/// frame, with "Revive" in the gold serif, or the seconds left before it.
fn draw_revive(img: &mut RgbaImage, viewport: [f32; 2], seconds_left: u32, hovered: bool) {
    let rect = revive_button_rect(viewport);
    let Rect(x, y, w, h) = rect;
    let ready = seconds_left == 0;
    let lit = ready && hovered;
    // The installation's own plate, when one is set; the label goes on it.
    let original = crate::hud_icons::art("revive");
    if let Some(plate) = &original {
        let plate = imageops::resize(plate, w as u32, h as u32, imageops::FilterType::Triangle);
        imageops::overlay(img, &plate, i64::from(x), i64::from(y));
    }
    for row in 0..if original.is_some() { 0 } else { h } {
        let t = row as f32 / h as f32;
        let base = if lit { [48.0, 104.0, 112.0] } else { [26.0, 70.0, 80.0] };
        let c = [
            (base[0] * (1.15 - t * 0.5)) as u8,
            (base[1] * (1.15 - t * 0.5)) as u8,
            (base[2] * (1.15 - t * 0.5)) as u8,
            235,
        ];
        for column in 0..w {
            // The diamond lattice of the game's panels.
            let (u, v) = ((column + row) % 10, (column - row).rem_euclid(10));
            let line = u == 0 || v == 0;
            let mut colour = c;
            if line {
                colour = [c[0].saturating_add(16), c[1].saturating_add(22), c[2].saturating_add(22), 235];
            }
            blend(img, x + column, y + row, colour);
        }
    }
    if original.is_none() || lit {
        outline(img, Rect(x - 1, y - 1, w + 2, h + 2), [10, 16, 18, 255]);
        outline(img, rect, if lit { [255, 226, 140, 255] } else { [196, 160, 86, 255] });
    }
    if original.is_none() {
        outline(img, Rect(x + 2, y + 2, w - 4, h - 4), [120, 96, 48, 220]);
    }
    let Ok(font) = main_hud::label_font() else {
        return;
    };
    let (label, size, colour) = if ready {
        ("Revive".to_owned(), 18.0, if lit { [255, 248, 220, 255] } else { [255, 226, 150, 255] })
    } else {
        (format!("Revive in {seconds_left}s"), 15.0, [190, 184, 170, 255])
    };
    let width = main_hud::text_width(font, &label, size);
    main_hud::text(img, font, &label, size, [x + (w - width) / 2, y + (h - size as i32) / 2 - 2], colour, None);
}

pub fn items_button_at(viewport: [f32; 2], cursor: [f32; 2]) -> bool {
    layout::main_hud_button_at(viewport, cursor) == Some(MainHudButton::Items)
}

pub fn capture_button_at(viewport: [f32; 2], cursor: [f32; 2]) -> bool {
    layout::main_hud_button_at(viewport, cursor) == Some(MainHudButton::Capture)
}

pub fn status_button_at(viewport: [f32; 2], cursor: [f32; 2]) -> bool {
    layout::main_hud_button_at(viewport, cursor) == Some(MainHudButton::Status)
}

/// Index understood by `main.rs` and `shop::QuickPanel::from_button`.
pub fn quick_button_at(viewport: [f32; 2], cursor: [f32; 2]) -> Option<usize> {
    Some(match layout::main_hud_button_at(viewport, cursor)? {
        MainHudButton::Status => 0,
        MainHudButton::Items => 1,
        MainHudButton::Skill => 2,
        MainHudButton::Guild => 4,
        MainHudButton::Friend => 5,
        MainHudButton::Trade => 6,
        MainHudButton::Team => 7,
        MainHudButton::Options => 9,
        MainHudButton::Message => 10,
        _ => return None,
    })
}

pub fn chat_input_at(viewport: [f32; 2], cursor: [f32; 2]) -> bool {
    matches!(
        layout::chat_strip_control_at(viewport, cursor),
        Some(ChatStripControl::Input | ChatStripControl::Name)
    )
}

pub fn chat_clear_at(viewport: [f32; 2], cursor: [f32; 2]) -> bool {
    layout::chat_strip_control_at(viewport, cursor) == Some(ChatStripControl::Clear)
}

pub fn chat_send_at(viewport: [f32; 2], cursor: [f32; 2]) -> bool {
    layout::chat_strip_control_at(viewport, cursor) == Some(ChatStripControl::Send)
}

pub fn chat_button_at(viewport: [f32; 2], cursor: [f32; 2]) -> bool {
    layout::chat_strip_control_at(viewport, cursor) == Some(ChatStripControl::Log)
}

pub fn talk_channel_at(viewport: [f32; 2], cursor: [f32; 2]) -> bool {
    layout::chat_strip_control_at(viewport, cursor) == Some(ChatStripControl::Talk)
}

pub fn chat_tab_at(viewport: [f32; 2], cursor: [f32; 2]) -> Option<u16> {
    (0..CHAT_TABS.len())
        .find(|&index| chat_tab_rect(viewport, index).contains(cursor))
        .map(|index| CHAT_TABS[index].1)
}

pub fn hovered_button(viewport: [f32; 2], cursor: [f32; 2]) -> Option<u8> {
    if let Some(index) = crate::hud_icons::icon_at(viewport, cursor) {
        return Some(HOVER_ICON + index as u8);
    }
    if revive_button_at(viewport, cursor) {
        return Some(HOVER_REVIVE);
    }
    if let Some(slot) = layout::hotkey_slot_at(viewport, cursor) {
        return Some(HOVER_SLOT + slot as u8);
    }
    if let Some(button) = layout::main_hud_button_at(viewport, cursor) {
        return MainHudButton::ALL
            .iter()
            .position(|&candidate| candidate == button)
            .map(|index| index as u8);
    }
    if let Some(control) = layout::chat_strip_control_at(viewport, cursor) {
        return ChatStripControl::ALL
            .iter()
            .position(|&candidate| candidate == control)
            .map(|index| HOVER_CHAT + index as u8);
    }
    if let Some(index) = (0..CHAT_TABS.len()).find(|&i| chat_tab_rect(viewport, i).contains(cursor))
    {
        return Some(HOVER_TAB + index as u8);
    }
    layout::orb_rect(viewport)
        .contains(cursor)
        .then_some(HOVER_ORB)
}

fn blend(img: &mut RgbaImage, x: i32, y: i32, color: [u8; 4]) {
    if x >= 0 && y >= 0 && x < img.width() as i32 && y < img.height() as i32 {
        img.get_pixel_mut(x as u32, y as u32).blend(&Rgba(color));
    }
}

fn fill(img: &mut RgbaImage, r: Rect, color: [u8; 4]) {
    for y in r.1..r.1 + r.3 {
        for x in r.0..r.0 + r.2 {
            blend(img, x, y, color);
        }
    }
}

fn outline(img: &mut RgbaImage, r: Rect, color: [u8; 4]) {
    for x in r.0..r.0 + r.2 {
        blend(img, x, r.1, color);
        blend(img, x, r.1 + r.3 - 1, color);
    }
    for y in r.1 + 1..r.1 + r.3 - 1 {
        blend(img, r.0, y, color);
        blend(img, r.0 + r.2 - 1, y, color);
    }
}

/// Original-style tooltip: translucent black box, thin bronze border.
fn tooltip(img: &mut RgbaImage, font: &fontdue::Font, lines: &[(&str, [u8; 4])], anchor: [i32; 2]) {
    let width = lines
        .iter()
        .map(|(line, _)| main_hud::text_width(font, line, 12.0))
        .max()
        .unwrap_or(0)
        + 12;
    let height = lines.len() as i32 * 15 + 8;
    let x = anchor[0].clamp(2, (img.width() as i32 - width - 2).max(2));
    let y = (anchor[1] - height).max(2);
    let r = Rect(x, y, width, height);
    fill(img, r, [0, 0, 0, 185]);
    outline(img, r, [137, 115, 74, 255]);
    for (index, (line, color)) in lines.iter().enumerate() {
        main_hud::text(
            img,
            font,
            line,
            12.0,
            [x + 6, y + 4 + index as i32 * 15],
            *color,
            None,
        );
    }
}

fn chat_color(channel: u16) -> ([u8; 4], Option<(&'static str, [u8; 4])>) {
    match channel {
        2001 => (
            [255, 128, 255, 255],
            Some(("[Whisper]", [255, 128, 255, 255])),
        ),
        2003 => ([128, 255, 255, 255], Some(("[Team]", [128, 255, 255, 255]))),
        2004 => (
            [128, 255, 128, 255],
            Some(("[Guild]", [128, 255, 128, 255])),
        ),
        2021 => (
            [255, 200, 120, 255],
            Some(("[World]", [255, 200, 120, 255])),
        ),
        2005 | 2007 | 2011 => ([255, 255, 0, 255], Some(("[System]", [255, 64, 255, 255]))),
        _ => ([255, 255, 255, 255], None),
    }
}

fn draw_chat_log(
    img: &mut RgbaImage,
    font: &fontdue::Font,
    viewport: [f32; 2],
    history: &[crate::network::ChatMessage],
    filter: u16,
    hover: Option<u8>,
) {
    let log = chat_log_rect(viewport);
    fill(img, log, [0, 0, 0, 70]);
    for (index, (label, channel)) in CHAT_TABS.into_iter().enumerate() {
        let tab = chat_tab_rect(viewport, index);
        let active = channel == filter;
        let hovered = hover == Some(HOVER_TAB + index as u8);
        fill(
            img,
            tab,
            if active {
                [92, 96, 104, 235]
            } else if hovered {
                [70, 74, 82, 225]
            } else {
                [44, 47, 54, 215]
            },
        );
        outline(
            img,
            tab,
            if active {
                [176, 190, 214, 255]
            } else {
                [74, 79, 88, 255]
            },
        );
        let color = if active {
            [255, 255, 255, 255]
        } else {
            [205, 208, 214, 255]
        };
        let text_x = tab.0 + (tab.2 - main_hud::text_width(font, label, 12.0)) / 2;
        main_hud::text(img, font, label, 12.0, [text_x, tab.1 + 3], color, None);
    }
    let messages = history
        .iter()
        .filter(|message| filter == 0 || message.channel == filter)
        .rev()
        .take(CHAT_LOG_LINES)
        .collect::<Vec<_>>();
    let max_width = log.2 - 12;
    for (row, message) in messages.into_iter().enumerate() {
        let y = log.1 + log.3 - 6 - (row as i32 + 1) * CHAT_LINE_HEIGHT;
        let (color, tag) = chat_color(message.channel);
        let mut x = log.0 + 6;
        if let Some((tag, tag_color)) = tag {
            x += main_hud::text(img, font, tag, 12.0, [x, y], tag_color, None) + 4;
        }
        let body = if message.channel == 2007 || message.channel == 2005 {
            message.message.clone()
        } else {
            format!("{}: {}", message.sender_name, message.message)
        };
        main_hud::text(
            img,
            font,
            &body,
            12.0,
            [x, y],
            color,
            Some(max_width - (x - log.0)),
        );
    }
}

fn draw_hotkeys(
    img: &mut RgbaImage,
    assets: &mut Assets,
    font: &fontdue::Font,
    viewport: [f32; 2],
    action_bar: &crate::action_bar::ActionBar,
    hover: Option<u8>,
) -> Result<()> {
    for (index, slot) in action_bar.slots.iter().enumerate() {
        let frame = hotkey_rect(viewport, index);
        let inner = frame.inset(3);
        if let Some(mut icon) = slot
            .icon_path()
            .and_then(|path| assets.image(&path).ok())
        {
            // Skill textures keep their artwork in a larger transparent
            // canvas; crop it so the icon fills the slot like the original.
            icon = crate::original_ui::visible_part(&icon)?;
            let size = (frame.2 - 4) as u32;
            if icon.dimensions() != (size, size) {
                icon = imageops::resize(&icon, size, size, imageops::FilterType::Triangle);
            }
            if slot.disabled {
                for pixel in icon.pixels_mut() {
                    pixel.0[3] /= 2;
                }
            }
            imageops::overlay(img, &icon, i64::from(frame.0 + 2), i64::from(frame.1 + 2));
        }
        if slot.cooldown_left > 0 {
            cooldown_sweep(img, inner, slot.cooldown_left);
        }
        if let Some(seconds) = slot.cooldown.filter(|seconds| *seconds > 0) {
            let label = seconds.to_string();
            let x = inner.0 + (inner.2 - main_hud::text_width(font, &label, 13.0)) / 2;
            main_hud::text(
                img,
                font,
                &label,
                13.0,
                [x, inner.1 + 9],
                [255, 255, 255, 255],
                None,
            );
        }
        if hover == Some(HOVER_SLOT + index as u8) {
            outline(img, inner, [255, 230, 150, 220]);
        }
    }
    Ok(())
}

/// Darkens the part of the slot still cooling down, as a clock hand that
/// sweeps clockwise from twelve o'clock (the 5065 hotkey cooldown).
fn cooldown_sweep(img: &mut RgbaImage, area: Rect, left: u8) {
    let ready = 1.0 - f32::from(left) / f32::from(crate::action_bar::COOLDOWN_STEPS);
    let cx = area.0 as f32 + area.2 as f32 / 2.0;
    let cy = area.1 as f32 + area.3 as f32 / 2.0;
    for y in area.1.max(0)..(area.1 + area.3).min(img.height() as i32) {
        for x in area.0.max(0)..(area.0 + area.2).min(img.width() as i32) {
            let dx = x as f32 + 0.5 - cx;
            let dy = y as f32 + 0.5 - cy;
            // 0 at twelve o'clock, growing clockwise to 1.
            let turn = (dx.atan2(-dy) / std::f32::consts::TAU).rem_euclid(1.0);
            if turn >= ready {
                let pixel = img.get_pixel_mut(x as u32, y as u32);
                for channel in &mut pixel.0[..3] {
                    *channel = (u16::from(*channel) * 2 / 5) as u8;
                }
            }
        }
    }
}

fn draw_action_context_menu(
    img: &mut RgbaImage,
    font: &fontdue::Font,
    viewport: [f32; 2],
    slot: usize,
    hover: Option<usize>,
) {
    let menu = action_context_menu_rect(viewport, slot);
    fill(img, menu, [0, 0, 0, 200]);
    outline(img, menu, [137, 115, 74, 255]);
    for (index, label) in CONTEXT_OPTIONS.into_iter().enumerate() {
        let row = Rect(
            menu.0 + 3,
            menu.1 + 4 + index as i32 * CONTEXT_ROW,
            menu.2 - 6,
            CONTEXT_ROW,
        );
        if hover == Some(index) {
            fill(img, row, [96, 78, 44, 200]);
        }
        main_hud::text(
            img,
            font,
            label,
            12.0,
            [row.0 + 8, row.1 + 4],
            if hover == Some(index) {
                [255, 236, 170, 255]
            } else {
                [235, 235, 235, 255]
            },
            None,
        );
    }
}

fn active_buff_badges(
    double_experience_active: bool,
    heaven_blessing_active: bool,
    burning_active: bool,
    stunned_active: bool,
    cyclone_active: bool,
    superman_active: bool,
    stigma_active: bool,
    shield_active: bool,
) -> Vec<(&'static str, [u8; 4])> {
    [
        (double_experience_active, "2X XP", [239, 190, 71, 255]),
        (heaven_blessing_active, "Blessing", [164, 200, 255, 255]),
        (burning_active, "Poisoned", [197, 219, 91, 255]),
        (stunned_active, "Stunned", [238, 151, 213, 255]),
        (cyclone_active, "Cyclone", [151, 224, 239, 255]),
        (superman_active, "Superman", [239, 174, 111, 255]),
        (stigma_active, "Stigma", [208, 172, 239, 255]),
        (shield_active, "Shield", [151, 208, 239, 255]),
    ]
    .into_iter()
    .filter_map(|(active, label, color)| active.then_some((label, color)))
    .collect()
}

/// XP progress within the current level. Mirrors the server's placeholder
/// curve, where reaching level `n + 1` needs `n * 100` total experience.
/// Server `levexp` table: experience to leave each level (1-140), as in
/// the reference server. The server keeps experience inside the level.
const LEVEL_EXPERIENCE: [u64; 140] = [
    120, 180, 240, 360, 600, 960, 1200, 2400,
    3600, 8400, 12000, 14400, 18000, 21600, 22646, 32203,
    37433, 47556, 56609, 68772, 70515, 75936, 97733, 114836,
    120853, 123981, 126720, 145878, 173436, 197646, 202451, 212160,
    244190, 285823, 305986, 312864, 324480, 366168, 433959, 460590,
    506738, 569994, 728527, 850829, 916479, 935118, 940800, 1076593,
    1272780, 1357994, 1384861, 1478400, 1632438, 1903104, 2066042, 2104924,
    1921085, 2417202, 2853462, 3054574, 3111217, 3225600, 3810962, 4437896,
    4880605, 4970962, 5107200, 5652518, 6579162, 6877991, 7100700, 7157657,
    9106860, 10596398, 11220549, 11409192, 11424000, 12882952, 15172807, 15896990,
    16163799, 16800000, 19230280, 22365208, 23819312, 24219528, 24864000, 27200077,
    32033165, 33723801, 34291317, 34944000, 39463523, 45878567, 48924236, 49729220,
    51072000, 55808379, 64870058, 68391931, 69537026, 76422968, 96950789, 112676755,
    120090482, 121798280, 127680000, 137446887, 193715970, 408832150, 454674685, 461125885,
    469189885, 477253885, 480479485, 485317885, 493381885, 580580046, 717424987, 282274058,
    338728870, 406474644, 487769572, 585323487, 702388184, 842865821, 1011438985, 1073741823,
    1073741823, 8589134588, 25767403764, 77302211292, 231906633876, 347859950814, 521789926221, 782684889332,
    1174027333998, 1761041000997, 2641561501496, 0,
];

fn experience_fraction(level: u8, experience: u64) -> f32 {
    let need = LEVEL_EXPERIENCE
        .get(usize::from(level.max(1)) - 1)
        .copied()
        .unwrap_or(0);
    if need == 0 {
        return 0.0;
    }
    (experience as f32 / need as f32).clamp(0.0, 1.0)
}

pub fn image(
    assets: &mut Assets,
    width: u32,
    height: u32,
    _character_name: &str,
    health: u32,
    max_health: u32,
    mana: u32,
    max_mana: u32,
    level: u8,
    experience: u64,
    action_bar: &crate::action_bar::ActionBar,
    hover: Option<u8>,
    action_context_slot: Option<usize>,
    action_context_hover: Option<usize>,
    chat_text: &str,
    chat_focused: bool,
    chat_history: &[crate::network::ChatMessage],
    chat_filter: u16,
    double_experience_active: bool,
    heaven_blessing_active: bool,
    burning_active: bool,
    stunned_active: bool,
    cyclone_active: bool,
    superman_active: bool,
    stigma_active: bool,
    shield_active: bool,
    hud_config: HudConfig,
    skills: &SkillHud,
) -> Result<RgbaImage> {
    let font = main_hud::font()?;
    let viewport = [width as f32, height as f32];
    let mut img = RgbaImage::new(width.max(1), height.max(1));

    draw_chat_log(&mut img, font, viewport, chat_history, chat_filter, hover);

    let art = MainHudArt::get(assets)?;
    let [x, y] = hud_origin(viewport);
    let hovered_button = hover
        .filter(|&code| usize::from(code) < MainHudButton::ALL.len())
        .map(|code| MainHudButton::ALL[usize::from(code)]);
    let hovered_chat = hover
        .and_then(|code| code.checked_sub(HOVER_CHAT))
        .and_then(|index| ChatStripControl::ALL.get(usize::from(index)).copied());
    main_hud::draw(
        &mut img,
        art,
        [i64::from(x), i64::from(y)],
        &MainHudState {
            health,
            max_health,
            mana,
            max_mana,
            stamina: u32::from(skills.stamina),
            max_stamina: u32::from(skills.max_stamina),
            experience: experience_fraction(level, experience),
            hovered_button,
            hovered_chat,
            chat_text,
            chat_focused,
            kill_mode: skills.kill_mode,
            active_skill_icon: skills
                .active_skill
                .and_then(|id| crate::main_hud::skill_icon(assets, id)),
        },
        font,
    );
    draw_skill_bar(&mut img, assets, font, viewport, skills);
    if hud_config.kill_counter {
        draw_kill_counter(&mut img, viewport, skills.kill_count);
    }
    crate::hud_icons::draw(
        &mut img,
        viewport,
        hover
            .and_then(|code| code.checked_sub(HOVER_ICON))
            .map(usize::from)
            .filter(|index| *index < crate::hud_icons::FeatureIcon::ALL.len()),
    );
    if let Some(seconds_left) = skills.revive_in {
        draw_revive(&mut img, viewport, seconds_left, hover == Some(HOVER_REVIVE));
    }
    draw_hotkeys(&mut img, assets, font, viewport, action_bar, hover)?;

    // Status effects, listed under the top-left information line.
    let mut badges = active_buff_badges(
        double_experience_active,
        heaven_blessing_active,
        burning_active,
        stunned_active,
        cyclone_active,
        superman_active,
        stigma_active,
        shield_active,
    );
    if skills.xp_ready {
        badges.insert(0, ("XP Skills Ready", [255, 220, 90, 255]));
    }
    for (row, line) in skills.info.iter().enumerate() {
        main_hud::text(
            &mut img,
            font,
            line,
            12.0,
            [4, 4 + row as i32 * 15],
            [255, 255, 255, 255],
            None,
        );
    }
    let badges_top = 6 + skills.info.len().max(2) as i32 * 15;
    for (row, (label, color)) in badges.into_iter().enumerate() {
        main_hud::text(
            &mut img,
            font,
            label,
            12.0,
            [4, badges_top + row as i32 * 15],
            color,
            None,
        );
    }

    if hover == Some(HOVER_ORB) {
        let orb = Rect::from(layout::orb_rect(viewport));
        tooltip(
            &mut img,
            font,
            &[
                (&format!("HP: {health}/{max_health}"), [255, 120, 120, 255]),
                (&format!("MP: {mana}/{max_mana}"), [130, 170, 255, 255]),
                (&format!("Level: {level}"), [255, 255, 255, 255]),
            ],
            [orb.0 + orb.2 + 4, orb.1 + 10],
        );
    } else if let Some(index) = hover
        .and_then(|code| code.checked_sub(HOVER_SLOT))
        .map(usize::from)
        .filter(|&index| index < action_bar.slots.len())
        && action_context_slot.is_none()
    {
        let slot = &action_bar.slots[index];
        let name = slot
            .action_id
            .as_deref()
            .and_then(|action| action.strip_prefix("skill:"))
            .and_then(|id| id.parse::<u32>().ok())
            .and_then(crate::action_bar::skill_action)
            .map(|skill| skill.name);
        if let Some(name) = name {
            let frame = hotkey_rect(viewport, index);
            tooltip(
                &mut img,
                font,
                &[
                    (name, [255, 255, 255, 255]),
                    (&slot.hotkey, [200, 200, 200, 255]),
                ],
                [frame.0, frame.1 - 4],
            );
        }
    }
    if let Some(slot) = action_context_slot {
        draw_action_context_menu(&mut img, font, viewport, slot, action_context_hover);
    }
    Ok(img)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hit_tests_follow_the_original_layout() {
        for viewport in [[1024.0, 768.0], [1366.0, 768.0], [2560.0, 1440.0]] {
            let center = |r: layout::Rect| [r.x + r.width * 0.5, r.y + r.height * 0.5];
            let items = center(layout::main_hud_button_rect(viewport, MainHudButton::Items));
            assert!(items_button_at(viewport, items));
            assert_eq!(quick_button_at(viewport, items), Some(1));
            let status = center(layout::main_hud_button_rect(
                viewport,
                MainHudButton::Status,
            ));
            assert!(status_button_at(viewport, status));
            assert_eq!(quick_button_at(viewport, status), Some(0));
            for (index, button) in MainHudButton::ALL.into_iter().enumerate() {
                let p = center(layout::main_hud_button_rect(viewport, button));
                assert_eq!(hovered_button(viewport, p), Some(index as u8));
                assert!(contains(viewport, p));
            }
            for index in 0..10 {
                let p = center(layout::hotkey_slot_rect(viewport, index));
                assert_eq!(action_slot_at(viewport, p), Some(index));
                assert_eq!(hovered_button(viewport, p), Some(HOVER_SLOT + index as u8));
            }
            let clear = center(layout::chat_strip_rect(viewport, ChatStripControl::Clear));
            assert!(chat_clear_at(viewport, clear));
            let input = center(layout::chat_strip_rect(viewport, ChatStripControl::Input));
            assert!(chat_input_at(viewport, input));
            for (index, (_, channel)) in CHAT_TABS.into_iter().enumerate() {
                let tab = chat_tab_rect(viewport, index);
                let p = [(tab.0 + tab.2 / 2) as f32, (tab.1 + tab.3 / 2) as f32];
                assert_eq!(chat_tab_at(viewport, p), Some(channel));
            }
            assert!(!contains(viewport, [viewport[0] * 0.5, viewport[1] * 0.5]));
            assert!(!contains(viewport, [-1.0, viewport[1] - 1.0]));
        }
    }

    #[test]
    fn context_menu_rows_are_hit_tested() {
        let viewport = [1366.0, 768.0];
        let menu = action_context_menu_rect(viewport, 3);
        for index in 0..CONTEXT_OPTIONS.len() {
            let y = menu.1 + 4 + index as i32 * CONTEXT_ROW + CONTEXT_ROW / 2;
            assert_eq!(
                action_context_option_at(viewport, [(menu.0 + 20) as f32, y as f32], 3),
                Some(index)
            );
        }
        assert_eq!(action_context_option_at(viewport, [0.0, 0.0], 3), None);
    }

    #[test]
    fn experience_tracks_the_current_level() {
        // Level 2 needs 180 experience.
        assert_eq!(experience_fraction(2, 90), 0.5);
        assert_eq!(experience_fraction(1, 0), 0.0);
        assert_eq!(experience_fraction(3, 10_000), 1.0);
    }

    #[test]
    fn combat_buffs_are_listed_in_order() {
        let none = active_buff_badges(false, false, false, false, false, false, false, false);
        assert!(none.is_empty());
        let all = active_buff_badges(true, true, true, true, true, true, true, true);
        assert_eq!(all.len(), 8);
        assert_eq!(all[0].0, "2X XP");
        assert_eq!(all[7].0, "Shield");
    }

    #[test]
    #[ignore = "Requires a local 5065 installation; exports HUD images for visual inspection"]
    fn export_preview() -> Result<()> {
        let root = std::env::var("GRAVAK_ASSETS")?;
        let output = std::path::PathBuf::from(std::env::var("GRAVAK_HUD_PREVIEW")?);
        std::fs::create_dir_all(&output)?;
        let mut assets = Assets::open(std::path::Path::new(&root))?;
        let chat_history = vec![
            crate::network::ChatMessage {
                channel: 2007,
                sender_name: "SYSTEM".into(),
                recipient_name: "ALLUSERS".into(),
                message: "Ikzordep Has logged onto the server!".into(),
            },
            crate::network::ChatMessage {
                channel: 2000,
                sender_name: "CloudStrider".into(),
                recipient_name: "ALLUSERS".into(),
                message: "Olá, Twin City!".into(),
            },
            crate::network::ChatMessage {
                channel: 2001,
                sender_name: "Sheko".into(),
                recipient_name: "CloudStrider".into(),
                message: "Mensagem privada de teste".into(),
            },
        ];
        let action_bar = crate::action_bar::ActionBar::default();
        for (name, width, height, hp, mp, hover, chat) in [
            (
                "hud-1366.png",
                1366,
                768,
                65,
                40,
                Some(1),
                "Olá, Twin City!",
            ),
            ("hud-1024.png", 1024, 768, 25, 0, Some(HOVER_ORB), ""),
            ("hud-1904.png", 1904, 915, 100, 100, Some(HOVER_SLOT), ""),
        ] {
            image(
                &mut assets,
                width,
                height,
                "CloudStrider",
                hp,
                100,
                mp,
                100,
                2,
                160,
                &action_bar,
                hover,
                None,
                None,
                chat,
                !chat.is_empty(),
                &chat_history,
                0,
                true,
                false,
                false,
                false,
                false,
                false,
                false,
                false,
                HudConfig::default(),
                &SkillHud {
                    skills: vec![(1_000, 3, 120), (1_001, 1, 15), (1_110, 1, 0)],
                    bar_open: true,
                    active_skill: Some(1_001),
                    kill_count: 7,
                    hovered: Some(1_000),
                    ..SkillHud::default()
                },
            )?
            .save(output.join(name))?;
        }
        Ok(())
    }
}
