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
    /// The XP circle's points, 0-100.
    pub xp_points: u8,
    /// The page selected in the status Weapon tab.
    pub weapon_filter: crate::shop::WeaponFilter,
    pub bar_open: bool,
    pub active_skill: Option<u32>,
    pub kill_count: u32,
    /// KOs of the current Cyclone or Superman.
    pub ko_count: u32,
    /// The Top titles the hero holds (`top_title::PROFESSIONS` flags).
    pub top_flags: u32,
    /// With a bow in hand, the arrows worn (0 without), for the quiver.
    pub quiver: Option<u32>,
    /// The hero's nobility rank (the server's id) and sex, for its icon.
    pub nobility_rank: u32,
    pub female: bool,
    /// When the fire along the bottom burns out (unix ms): a lit XP
    /// circle's 20 seconds, then the XP skill's time.
    pub xp_fire_until: Option<u64>,
    /// The kill mode (`shop::KILL_MODES` order) the Capture button shows.
    pub kill_mode: u8,
    /// Running rather than walking (the bar's Run checkbox).
    pub running: bool,
    /// The bar's Map and Shift Screen checkboxes, and View Equipment
    /// waiting for a player.
    pub map_shown: bool,
    pub shift_screen: bool,
    pub picking_equipment: bool,
    /// The equipment of the player looked at.
    pub viewed_equipment: Option<crate::view_equipment::ViewedEquipment>,
    /// The top-left information lines: map and position, FPS and ping.
    pub info: Option<InfoBar>,
    /// Skill under the cursor in the bar; filled in by the renderer.
    pub hovered: Option<u32>,
    /// Timed statuses: their flag and the seconds left.
    pub timers: Vec<(u64, u32)>,
    /// Flying: the Fly skill and the seconds left. `Conquer.exe` shows it in
    /// the XP dialog (189), over the KO counter; a click lands.
    pub flying: Option<(u32, u32)>,
    /// The Character window's Skills tab: page, scroll and pick.
    pub skill_page: crate::shop::SkillPage,
    /// The Attributes tab shows attack and defence (Details) instead of
    /// the attributes (Basics).
    pub attribute_details: bool,
    /// The Battle Power Evaluation is open, and the part whose Detailed
    /// Information is shown.
    pub battle_power_open: bool,
    pub battle_power_detail: Option<u8>,
    /// The Action panel is open, with the emote under the cursor.
    pub action_panel_open: bool,
    pub action_panel_hover: Option<usize>,
    /// The Action panel's emote being dragged (its index there).
    pub dragging_emote: Option<usize>,
    /// The F key being dragged to another.
    pub dragging_hotkey: Option<usize>,
    /// The F keys' open popup and the cell under the cursor.
    pub hotkey_popup: Option<(crate::hotkey_menu::HotkeyPopupView, Option<usize>)>,
    /// The chat strip's whisper name (empty: the Talk channel) and whether
    /// the keys go to it rather than to the line.
    pub chat_recipient: String,
    pub chat_name_focused: bool,
    /// The chat channel (`CHAT_CHANNELS` index), its list and the one
    /// under the cursor.
    pub chat_channel: u8,
    pub chat_channel_menu: bool,
    pub chat_channel_hover: Option<u8>,
    /// Who is saying what on the Talk channel, for the bubbles.
    pub speech: Vec<(String, String)>,
    /// The private chat window and the faces waiting over the bar.
    pub whisper_window: Option<WhisperWindowView>,
    pub whisper_faces: Vec<(crate::whisper::Conversation, Option<u32>, bool)>,
}

/// What the private chat window shows.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WhisperWindowView {
    pub conversation: crate::whisper::Conversation,
    pub own_name: String,
    pub own_mesh: u32,
    /// How the other end looks, when on screen.
    pub seen_mesh: Option<u32>,
    pub input: String,
    pub input_focused: bool,
    pub hovered: Option<crate::whisper::WhisperClick>,
}

/// Seconds since 1970 on this machine.
pub fn unix_now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_millis() as u64)
}

pub fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

/// The top-left plate: where the hero is and how the game runs.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct InfoBar {
    pub map: String,
    pub cell: [i32; 2],
    pub fps: u32,
    /// Round trip to the server, once known.
    pub ping: Option<u32>,
}

// The top-left corner: the plate, then the statuses' slots under it.
const CORNER: [i32; 2] = [8, 8];
const PLATE_HEIGHT: i32 = 46;
const STATUS_SLOT: i32 = 34;
const STATUS_STEP: i32 = 40;

/// Where the statuses' row starts: under the plate, or in the corner.
fn status_row_top(skills: &SkillHud) -> i32 {
    CORNER[1] + if skills.info.is_some() { PLATE_HEIGHT + 8 } else { 0 }
}

fn status_slot_rect(column: usize, top: i32) -> crate::ui::UiRect {
    crate::ui::UiRect::new(CORNER[0] + column as i32 * STATUS_STEP, top, STATUS_SLOT, STATUS_SLOT)
}

/// The plate's place: as wide as the map's name or the details line,
/// whichever is wider.
fn info_plate_rect(info: &InfoBar) -> crate::ui::UiRect {
    use crate::daily_rewards_panel::width_of;
    let (Ok(ui), Ok(cinzel)) = (crate::labels::font(), crate::labels::decorative_font()) else {
        return crate::ui::UiRect::new(CORNER[0], CORNER[1], 220, PLATE_HEIGHT);
    };
    let width = |text: &str| width_of(&ui, text, 11.0).round() as i32;
    // Place, frames and ping, each in a fixed slot 18 pixels apart.
    let line_width = width("0000, 0000") + 18 + width("0000") + 4 + width("FPS") + 18 + 14 + width("0000 ms");
    let text_width = (width_of(&cinzel, &info.map, 14.0).round() as i32).max(line_width);
    crate::ui::UiRect::new(CORNER[0], CORNER[1], 52 + text_width + 14, PLATE_HEIGHT)
}

/// The Nobility Rank button beside the plate (in the corner without one):
/// the empire's gold medallion in a framed square. Left, top, width and
/// height in window pixels.
pub fn nobility_button_rect(skills: &SkillHud) -> [f32; 4] {
    const SIZE: i32 = 32;
    let (x, y) = match &skills.info {
        Some(info) => {
            let plate = info_plate_rect(info);
            (plate.x + plate.width + 8, plate.y + (plate.height - SIZE) / 2)
        }
        None => (CORNER[0], CORNER[1]),
    };
    [x as f32, y as f32, SIZE as f32, SIZE as f32]
}

/// The Nobility Rank button: the original's nobility status icon for the
/// hero's rank (`ini/StatusTips.ini` 36-43, clickable, `ani/effect.ani`
/// `KnightIcon_*`), a commoner's too.
fn draw_nobility_button(img: &mut RgbaImage, assets: &mut Assets, skills: &SkillHud) {
    let [x, y, width, _] = nobility_button_rect(skills);
    let path = format!("Data/Pic/Contribute/KnightIcon_{}.dds", knight_icon(skills.nobility_rank, skills.female));
    if let Ok(icon) = assets.image(&path) {
        let icon = if icon.width() == width as u32 {
            icon
        } else {
            image::imageops::resize(&icon, width as u32, width as u32, image::imageops::FilterType::Triangle)
        };
        image::imageops::overlay(img, &icon, x as i64, y as i64);
    }
}

/// The status icon of a nobility rank: 平民 commoner, 勋爵 Knight, 子爵
/// Baron, 伯爵 Earl, 侯爵 Duke, 公爵 Prince, 王 King and 女王 Queen.
fn knight_icon(rank: u32, female: bool) -> &'static str {
    match rank {
        12 if female => "vw",
        12 => "w",
        9 => "gj",
        7 => "hj",
        5 => "bj",
        3 => "zj",
        1 => "xj",
        _ => "pm",
    }
}

/// The plate: a compass badge, the map's name, and under it the cell, the
/// frames a second and the ping with its signal bars. Its bottom.
fn draw_info_plate(img: &mut RgbaImage, info: &InfoBar) {
    use crate::daily_rewards_panel::{glow_in, paint_in, rounded, say, width_of, GOLD_DARK, GOLD_LIGHT};
    use crate::paint;
    let (Ok(ui), Ok(cinzel)) = (crate::labels::font(), crate::labels::decorative_font()) else {
        return;
    };
    // The details line in three fixed slots (place, frames, ping), so the
    // plate keeps its width as the numbers change.
    const SIZE: f32 = 11.0;
    const GAP: i32 = 18;
    let value_colour = [234, 238, 232];
    let label_colour = [132, 156, 160];
    let width = |text: &str| width_of(&ui, text, SIZE).round() as i32;
    let place_slot = width("0000, 0000");
    let fps_slot = width("0000") + 4 + width("FPS");
    let plate = info_plate_rect(info);
    let shape = rounded(plate, 10.0);
    glow_in(img, plate, &shape, 6.0, paint::hex(0x000000), 0.35);
    let (y0, height) = (plate.y as f32, plate.height as f32);
    paint_in(
        img,
        plate,
        1,
        &shape,
        |_, y| paint::mix(paint::hex(0x123247), paint::hex(0x050e14), (y - y0) / height),
        0.82,
    );
    paint_in(img, plate, 1, |x, y| shape(x, y).abs() - 0.6, |_, _| GOLD_DARK, 1.0);
    // A thin light along the top edge.
    let shine = crate::ui::UiRect::new(plate.x + 12, plate.y + 1, plate.width - 24, 2);
    paint_in(img, shine, 0, |_, y| (y - shine.y as f32 - 1.0).abs() - 0.5, |x, _| paint::mix(GOLD_LIGHT, paint::hex(0x123247), ((x - (plate.x + plate.width / 2) as f32).abs() / (plate.width as f32 / 2.0)).powf(1.5)), 0.6);

    // The compass rose.
    crate::emblems::compass(img, [(plate.x + 25) as f32, (plate.y + plate.height / 2) as f32], 17.0);

    // The map, and the details under it.
    let left = plate.x + 52;
    let line = plate.y + 27;
    say(img, &cinzel, &info.map, [255, 226, 143], left, plate.y + 6, 14.0);
    say(img, &ui, &format!("{}, {}", info.cell[0], info.cell[1]), value_colour, left, line, SIZE);
    // A small gold diamond between the slots.
    let diamond = |img: &mut RgbaImage, x: i32| {
        let centre = [x as f32, line as f32 + 7.0];
        let bounds = crate::ui::UiRect::new(x - 3, line + 4, 7, 7);
        paint_in(img, bounds, 1, |px, py| (px - centre[0]).abs() + (py - centre[1]).abs() - 2.6, |_, _| GOLD_DARK, 1.0);
    };
    let fps_left = left + place_slot + GAP;
    diamond(img, fps_left - GAP / 2);
    let fps = info.fps.to_string();
    say(img, &ui, &fps, value_colour, fps_left, line, SIZE);
    say(img, &ui, "FPS", label_colour, fps_left + width(&fps) + 4, line, SIZE);
    let ping_left = fps_left + fps_slot + GAP;
    diamond(img, ping_left - GAP / 2);
    match info.ping {
        Some(ping) => {
            let value = ping.to_string();
            say(img, &ui, &value, value_colour, ping_left + 14, line, SIZE);
            say(img, &ui, "ms", label_colour, ping_left + 14 + width(&value) + 3, line, SIZE);
        }
        None => say(img, &ui, "-", label_colour, ping_left + 14, line, SIZE),
    }
    // The signal bars, coloured by the ping.
    let colour = match info.ping {
        Some(ping) if ping < 80 => paint::hex(0x6fd36f),
        Some(ping) if ping < 160 => paint::hex(0xf0c040),
        Some(_) => paint::hex(0xe05a4a),
        None => paint::hex(0x5c6a70),
    };
    let lit = match info.ping {
        Some(ping) if ping < 80 => 3,
        Some(ping) if ping < 160 => 2,
        Some(_) => 1,
        None => 0,
    };
    // The signal bars lead the ping.
    let bars_left = ping_left;
    for bar in 0..3 {
        let bar_height = 4 + bar * 3;
        let rect = crate::ui::UiRect::new(bars_left + bar * 4, plate.y + 38 - bar_height, 3, bar_height);
        let fill = if bar < lit { colour } else { paint::hex(0x2a3a42) };
        paint_in(img, rect, 0, |x, y| {
            let dx = (x - rect.x as f32 - 1.5).abs() - 1.5;
            let dy = (y - rect.y as f32 - rect.height as f32 / 2.0).abs() - rect.height as f32 / 2.0;
            dx.max(dy)
        }, |_, _| fill, 1.0);
    }
}

/// A status's slot: a dark well in a gold frame, red when nearly out, and
/// its time on a small pill under it.
fn draw_status_slot(img: &mut RgbaImage, rect: crate::ui::UiRect, seconds: u32) {
    use crate::daily_rewards_panel::{centred, glow_in, paint_in, rounded, width_of, GOLD, GOLD_DARK};
    use crate::paint;
    let Ok(ui) = crate::labels::font() else {
        return;
    };
    let ending = seconds <= 10;
    let shape = rounded(rect, 6.0);
    glow_in(img, rect, &shape, 5.0, if ending { paint::hex(0xff5a40) } else { paint::hex(0x000000) }, if ending { 0.45 } else { 0.35 });
    let (y0, height) = (rect.y as f32, rect.height as f32);
    paint_in(img, rect, 1, &shape, |_, y| paint::mix(paint::hex(0x163a52), paint::hex(0x061420), (y - y0) / height), 0.88);
    let frame = if ending { paint::hex(0xff7a5a) } else { GOLD };
    paint_in(img, rect, 1, |x, y| shape(x, y).abs() - 0.7, |_, _| frame, 1.0);
    let label = short_time(seconds);
    let width = width_of(&ui, &label, 10.0).round() as i32 + 10;
    let pill = crate::ui::UiRect::new(rect.x + (rect.width - width) / 2, rect.y + rect.height - 6, width, 14);
    let pill_shape = rounded(pill, 7.0);
    paint_in(img, pill, 1, &pill_shape, |_, _| paint::hex(0x07131b), 0.95);
    paint_in(img, pill, 1, |x, y| pill_shape(x, y).abs() - 0.5, |_, _| if ending { paint::hex(0xff7a5a) } else { GOLD_DARK }, 1.0);
    let colour = if ending { [255, 150, 120] } else { [255, 240, 200] };
    centred(img, &ui, &label, colour, pill.x + pill.width / 2, pill.y + 1, 10.0);
}

/// A notice as a small pill (XP skills ready, poisoned...).
fn draw_badge_pill(img: &mut RgbaImage, label: &str, colour: [u8; 4], [x, y]: [i32; 2]) -> i32 {
    use crate::daily_rewards_panel::{paint_in, rounded, say, width_of};
    use crate::paint;
    let Ok(ui) = crate::labels::font() else {
        return 0;
    };
    let width = width_of(&ui, label, 11.0).round() as i32 + 18;
    let pill = crate::ui::UiRect::new(x, y, width, 18);
    let shape = rounded(pill, 9.0);
    paint_in(img, pill, 1, &shape, |_, _| paint::hex(0x07131b), 0.85);
    let edge = [f32::from(colour[0]) / 255.0, f32::from(colour[1]) / 255.0, f32::from(colour[2]) / 255.0];
    paint_in(img, pill, 1, |x, y| shape(x, y).abs() - 0.5, |_, _| edge, 0.9);
    say(img, &ui, label, [colour[0], colour[1], colour[2]], x + 9, y + 3, 11.0);
    width
}

/// What a status's icon shows: a skill's icon or the VIP title.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StatusIcon {
    Skill(u32),
    Vip,
    /// One of the original's animated status flags (`3DEffect.ini`), drawn
    /// by the renderer over the HUD.
    Flag(&'static str),
}

/// The icon of each timed status flag.
/// (Fly shows over the KO counter instead, where a click lands it.)
const STATUS_ICONS: [(u64, StatusIcon); 13] = [
    (1 << 18, StatusIcon::Skill(1_025)), // Superman
    (1 << 23, StatusIcon::Skill(1_110)), // Cyclone
    (1 << 9, StatusIcon::Skill(1_095)),  // Stigma
    (1 << 8, StatusIcon::Skill(1_020)),  // Shield
    (1 << 7, StatusIcon::Skill(1_015)),  // Accuracy
    (1 << 22, StatusIcon::Skill(1_075)), // Invisibility
    (1 << 47, StatusIcon::Skill(9_000)), // Intensify
    (1 << 1, StatusIcon::Skill(5_002)),  // Poisoned
    (1 << 5, StatusIcon::Skill(1_002)),  // Stunned
    (1 << 48, StatusIcon::Flag("DblExpFlag")), // Double experience
    (1 << 49, StatusIcon::Flag("BlessFlag")),  // Heaven's Blessing
    (1 << 46, StatusIcon::Vip),
    (1 << 50, StatusIcon::Flag("Lucky")),      // Lucky time (Bless)
];

/// Time left as the bar shows it: `3d`, `5h`, `2:05`, `45s`.
pub fn short_time(seconds: u32) -> String {
    match seconds {
        86_400.. => format!("{}d", seconds / 86_400),
        3_600.. => format!("{}h", seconds / 3_600),
        60.. => format!("{}:{:02}", seconds / 60, seconds % 60),
        _ => format!("{seconds}s"),
    }
}

/// The timed statuses with an icon, in the bar's order.
fn sorted_timers(skills: &SkillHud) -> Vec<(u64, u32)> {
    let mut timed: Vec<(u64, u32)> = skills
        .timers
        .iter()
        .copied()
        .filter(|(flag, _)| STATUS_ICONS.iter().any(|(bit, _)| bit == flag))
        .collect();
    timed.sort_by_key(|(flag, _)| STATUS_ICONS.iter().position(|(bit, _)| bit == flag).unwrap_or(usize::MAX));
    timed
}

/// The original's animated flag for a status, if it has one.
fn flag_effect(flag: u64) -> Option<&'static str> {
    STATUS_ICONS.iter().find_map(|(bit, icon)| match icon {
        StatusIcon::Flag(effect) if *bit == flag => Some(*effect),
        _ => None,
    })
}

/// Where the renderer draws the animated status flags: their effect and
/// the centre of their slot, in window pixels.
pub fn status_flag_slots(skills: &SkillHud) -> Vec<(&'static str, [f32; 2])> {
    let top = status_row_top(skills);
    sorted_timers(skills)
        .iter()
        .enumerate()
        .filter_map(|(column, (flag, _))| {
            let effect = flag_effect(*flag)?;
            let rect = status_slot_rect(column, top);
            Some((effect, [(rect.x + rect.width / 2) as f32, (rect.y + rect.height / 2 - 2) as f32]))
        })
        .collect()
}

/// A status's icon, `size` pixels square.
fn status_icon(assets: &mut Assets, flag: u64, size: u32) -> Option<RgbaImage> {
    let icon = STATUS_ICONS.iter().find(|(bit, _)| *bit == flag).map(|(_, icon)| *icon)?;
    let image = match icon {
        StatusIcon::Skill(skill) => crate::main_hud::skill_icon(assets, skill)?,
        StatusIcon::Vip => crate::ornate::vip_tag(1.0).ok()?,
        StatusIcon::Flag(_) => return None,
    };
    let (width, height) = image.dimensions();
    let scale = size as f32 / width.max(height).max(1) as f32;
    Some(image::imageops::resize(
        &image,
        ((width as f32 * scale).round() as u32).max(1),
        ((height as f32 * scale).round() as u32).max(1),
        image::imageops::FilterType::Triangle,
    ))
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


// Hover codes shared between hit testing and painting.
const HOVER_SLOT: u8 = 20;
const HOVER_CHAT: u8 = 40;
const HOVER_ORB: u8 = 80;

const SKILL_SLOT: i32 = 50;
const SKILL_ROW_CAPACITY: usize = 10;

/// Skill bar slots: XP skills on the row just above the bar, other skills
/// on the row above it (or on the lower row when there are no XP skills).
/// Rows end at the right edge of the "Skill" button, like the original.
///
/// A lit XP circle brings its skills up on their own at the right edge,
/// as `Conquer.exe` places its XP dialog (189): `W - n * 50` across,
/// `H - 173` down, 50 pixels each, ten at most. A click picks one for the
/// right button.
fn skill_bar_slots(assets: &mut Assets, viewport: [f32; 2], hud: &SkillHud) -> Vec<(u32, Rect)> {
    const XP_SLOT: i32 = 50;
    const XP_SHOWN: usize = 10;
    let (xp, magic): (Vec<u32>, Vec<u32>) = hud
        .skills
        .iter()
        .map(|(id, _, _)| *id)
        .partition(|id| crate::main_hud::is_xp_skill(assets, *id));
    let mut slots = Vec::new();
    if let Some((fly, _)) = hud.flying.filter(|_| !hud.xp_ready) {
        let top = viewport[1] as i32 - 173;
        slots.push((fly, Rect(viewport[0] as i32 - XP_SLOT, top, XP_SLOT, XP_SLOT)));
    }
    if hud.xp_ready {
        let shown = xp.len().min(XP_SHOWN) as i32;
        let left = viewport[0] as i32 - shown * XP_SLOT;
        let top = viewport[1] as i32 - 173;
        for (index, id) in xp.iter().take(XP_SHOWN).enumerate() {
            slots.push((*id, Rect(left + index as i32 * XP_SLOT, top, XP_SLOT, XP_SLOT)));
        }
    }
    if !hud.bar_open {
        return slots;
    }
    let [x, y] = hud_origin(viewport);
    let skill = MainHudButton::Skill.local_rect();
    let right = x + (skill.x + skill.width).round() as i32;
    let lower = y + 16;
    // Lit, the XP skills are in their own row at the right edge.
    let rows = if xp.is_empty() || hud.xp_ready {
        vec![(magic, lower)]
    } else {
        vec![(xp, lower), (magic, lower - SKILL_SLOT)]
    };
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

/// The quiver with a bow in hand (`Conquer.exe` 0x4aa6c8): `[NoArrow]`
/// of `ani/common.ani` 135 from the bar's left and 126 over the bottom,
/// and the arrows worn
/// in white over it, with a black shadow a pixel up and left.
fn draw_quiver(img: &mut RgbaImage, assets: &mut Assets, font: &fontdue::Font, viewport: [f32; 2], arrows: u32) {
    // 135 from the bar's left (the original's bar spans its 1024 pixels;
    // ours is centred in wider windows).
    let [x, y] = [hud_origin(viewport)[0] + 135, viewport[1] as i32 - 126];
    if let Ok(icon) = assets.image("data/pic/arrow.dds") {
        image::imageops::overlay(img, &icon, i64::from(x), i64::from(y));
    }
    let count = arrows.to_string();
    crate::main_hud::text(img, font, &count, 12.0, [x - 1, y - 1], [0, 0, 0, 255], None);
    crate::main_hud::text(img, font, &count, 12.0, [x, y], [255, 255, 255, 255], None);
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
            let flying = hud.flying.is_some_and(|(fly, _)| fly == id);
            if !hud.xp_ready && !flying && crate::main_hud::is_xp_skill(assets, id) {
                for pixel in icon.pixels_mut() {
                    let grey = ((u16::from(pixel[0]) + u16::from(pixel[1]) + u16::from(pixel[2])) / 6) as u8;
                    *pixel = image::Rgba([grey, grey, grey, pixel[3]]);
                }
            }
            imageops::overlay(img, &icon, i64::from(rect.0 + 1), i64::from(rect.1 + 1));
        }
        // The flier's time left under its icon.
        if let Some((_, seconds)) = hud.flying.filter(|(fly, _)| *fly == id) {
            let label = format!("{seconds}s");
            let x = rect.0 + (rect.2 - main_hud::text_width(font, &label, 12.0)) / 2;
            main_hud::text(img, font, &label, 12.0, [x + 1, rect.1 + rect.3 - 14], [0, 0, 0, 220], None);
            main_hud::text(img, font, &label, 12.0, [x, rect.1 + rect.3 - 15], [255, 255, 255, 255], None);
            outline(img, rect, [214, 176, 96, 255]);
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

/// Cyclone (1 << 23) and Superman (1 << 18).
const XP_SKILL_FLAGS: u64 = (1 << 23) | (1 << 18);

/// The counts at the bottom right, drawn with the original's
/// `CountB`/`CountK` effects (`3DEffect.ini`): the kills, small and blue,
/// and an XP skill's KOs, big and red.
#[derive(Clone, Copy)]
enum Count {
    Kills,
    Ko,
}

impl Count {
    /// The effect prefix, the æ–©'s centre from the bottom right and the
    /// digits' spacing, as the original places them (`W - 30, H - 111`
    /// and `W - 57, H - 138`).
    fn layout(self) -> (&'static str, [i32; 2], i32) {
        match self {
            Self::Kills => ("CountB", [30, 111], 26),
            Self::Ko => ("CountK", [57, 138], 53),
        }
    }

    /// Each glyph's drawn size: the effect's quad (93 x 123) at the
    /// original's scale, 1.2 for the KOs; the kills a little smaller than
    /// its 0.6, under the XP skills as the original shows them.
    fn glyph_size(self) -> [u32; 2] {
        match self {
            Self::Kills => [40, 52],
            Self::Ko => [112, 148],
        }
    }
}

fn draw_count(img: &mut RgbaImage, assets: &mut Assets, viewport: [f32; 2], count: u32, kind: Count) {
    let (prefix, [from_right, from_bottom], spacing) = kind.layout();
    let [width, height] = kind.glyph_size();
    let mut glyph = |assets: &mut Assets, name: String, centre: [i32; 2]| {
        let Some(texture) = crate::magic::effect(assets, &name).and_then(|effect| effect.layers.first().cloned())
        else {
            return;
        };
        let Ok(image) = assets.image(&texture.texture) else {
            return;
        };
        let image = imageops::resize(&image, width, height, imageops::FilterType::Triangle);
        let left = centre[0] - width as i32 / 2;
        let top = centre[1] - height as i32 / 2;
        imageops::overlay(img, &image, i64::from(left), i64::from(top));
    };
    let mark = [viewport[0] as i32 - from_right, viewport[1] as i32 - from_bottom];
    glyph(assets, format!("{prefix}Z"), mark);
    let digits = count.to_string();
    let first = mark[0] - 10 - digits.len() as i32 * spacing;
    for (index, digit) in digits.chars().enumerate() {
        glyph(assets, format!("{prefix}{digit}"), [first + index as i32 * spacing, mark[1]]);
    }
}

/// The revive countdown and button sit above the KO counter, against the
/// right edge: this is their bottom right corner.
pub fn revive_anchor(viewport: [f32; 2]) -> [f32; 2] {
    let [_, hud_y] = hud_origin(viewport);
    [viewport[0] - 10.0, hud_y as f32 - 8.0]
}

/// While dead, the countdown: "Revive 14" in the KO counter's glow.
pub fn revive_countdown_image(remaining: u32) -> crate::assets::Result<RgbaImage> {
    let (digits, glyph) = (crate::main_hud::italic_font()?, crate::main_hud::client_font()?);
    let label = "Revive";
    let number = remaining.to_string();
    let label_width = crate::main_hud::text_width(glyph, label, 22.0);
    let number_width = crate::main_hud::text_width(digits, &number, 34.0);
    let width = (label_width + number_width + 22).max(1) as u32;
    let mut image = RgbaImage::new(width, 42);
    let glow = [150, 110, 255];
    crate::main_hud::glow_text(&mut image, glyph, label, 22.0, [4, 12], glow);
    crate::main_hud::glow_text(&mut image, digits, &number, 34.0, [label_width + 14, 2], glow);
    Ok(image)
}

/// Size of a revive button.
#[cfg(test)]
pub const REVIVE_BUTTON: [u32; 2] = [136, 40];

/// A button in the style of the HUD's "Skill" button: the HUD's diamond
/// pattern on deep teal, a thin gold frame with corner brackets, and the
/// client's serif text in pale teal.
/// The panel of the HUD-style buttons: the HUD's diamond pattern on deep
/// teal, a thin `frame` coloured frame with corner brackets.
pub fn hud_style_panel(assets: &mut Assets, width: u32, height: u32, frame: [u8; 3]) -> RgbaImage {
    let mut image = RgbaImage::from_pixel(width, height, image::Rgba([12, 34, 44, 255]));
    // The diamond pattern beside the HUD's emblem (mainDialog2.dds).
    if let Ok(art) = assets.image("data/main/mainDialog2.dds") {
        let tile = image::imageops::crop_imm(&art, 33, 66, 18, 44).to_image();
        let mut y = 0;
        while y < height {
            let mut x = 0;
            while x < width {
                image::imageops::overlay(&mut image, &tile, i64::from(x), i64::from(y));
                x += tile.width();
            }
            y += tile.height();
        }
        // Darken it so the text reads, as on the Skill button.
        for pixel in image.pixels_mut() {
            for channel in 0..3 {
                pixel.0[channel] = (u16::from(pixel.0[channel]) * 7 / 10) as u8;
            }
            pixel.0[3] = 255;
        }
    }
    let gold = image::Rgba([frame[0], frame[1], frame[2], 255]);
    let shade = image::Rgba([40, 34, 20, 255]);
    let (w, h) = (width as i32, height as i32);
    let mut put = |x: i32, y: i32, colour: image::Rgba<u8>| {
        if x >= 0 && y >= 0 && x < w && y < h {
            image.put_pixel(x as u32, y as u32, colour);
        }
    };
    // The frame, inset like the original, with a dark line outside it.
    for x in 3..w - 3 {
        put(x, 3, gold);
        put(x, h - 4, gold);
        put(x, 2, shade);
        put(x, h - 3, shade);
    }
    for y in 3..h - 3 {
        put(3, y, gold);
        put(w - 4, y, gold);
        put(2, y, shade);
        put(w - 3, y, shade);
    }
    // Corner brackets: a small square bracket hooked on each corner.
    for (cx, cy, sx, sy) in [(3, 3, 1, 1), (w - 4, 3, -1, 1), (3, h - 4, 1, -1), (w - 4, h - 4, -1, -1)] {
        for step in 0..5 {
            put(cx - sx * 2 + sx * step, cy - sy * 2, gold);
            put(cx - sx * 2, cy - sy * 2 + sy * step, gold);
        }
        put(cx + sx, cy + sy, gold);
    }
    image
}

/// The revive buttons: ornate plates like the Store's.
/// The original revive plates (`data/main/Relive01.dds`, lit
/// `Relive02.dds`, the art of `[XpSkillType5]` in `ani/Magic.Ani`).
/// The art says "Revive Here"; plain Revive is the same plate with "Here"
/// taken away and "Revive" moved to the middle. A Revive Here the player
/// cannot use (no Heaven's Blessing) is dimmed.
pub fn revive_plate(assets: &mut Assets, here: bool, lit: bool, usable: bool) -> crate::assets::Result<RgbaImage> {
    let art = assets.image(if lit { "data/main/Relive02.dds" } else { "data/main/Relive01.dds" })?;
    let mut plate = crate::original_ui::visible_part(&art)?;
    if !here {
        only_first_line(&mut plate);
    }
    if !usable {
        for pixel in plate.pixels_mut() {
            let grey = (u16::from(pixel.0[0]) + u16::from(pixel.0[1]) + u16::from(pixel.0[2])) / 3;
            for channel in 0..3 {
                pixel.0[channel] = ((u16::from(pixel.0[channel]) + grey) / 4) as u8;
            }
        }
    }
    Ok(plate)
}

/// Keeps the first line of a plate's two-line caption, centred: the
/// caption's pixels (bright, with their dark outline) are lifted out, the
/// plate under them is filled in from around, and the first line is put
/// back in the middle.
fn only_first_line(plate: &mut RgbaImage) {
    let (width, height) = (plate.width() as i32, plate.height() as i32);
    // The frame and its corner pieces stay as they are.
    let border = 5;
    let inside = |x: i32, y: i32| x >= border && y >= border && x < width - border && y < height - border;
    let bright = |pixel: &image::Rgba<u8>| {
        pixel.0[3] > 0 && (u16::from(pixel.0[0]) + u16::from(pixel.0[1]) + u16::from(pixel.0[2])) / 3 > 95
    };
    let mut rows = vec![0; height as usize];
    let mut mask = vec![false; (width * height) as usize];
    for y in 0..height {
        for x in 0..width {
            if inside(x, y) && bright(plate.get_pixel(x as u32, y as u32)) {
                rows[y as usize] += 1;
                for dy in -2..=2 {
                    for dx in -2..=2 {
                        if inside(x + dx, y + dy) {
                            mask[((y + dy) * width + x + dx) as usize] = true;
                        }
                    }
                }
            }
        }
    }
    let Some(top) = (border..height - border).find(|y| rows[*y as usize] >= 2) else {
        return;
    };
    let end = (top..height - border).find(|y| rows[*y as usize] == 0).unwrap_or(height - border) - 1;
    let line: Vec<(i32, i32, image::Rgba<u8>)> = (top - 2..=end + 2)
        .flat_map(|y| (0..width).map(move |x| (x, y)))
        .filter(|(x, y)| inside(*x, *y) && mask[(y * width + x) as usize])
        .map(|(x, y)| (x, y, *plate.get_pixel(x as u32, y as u32)))
        .collect();
    // Fill the caption's place from its edges inwards.
    let mut todo: Vec<(i32, i32)> = (0..width * height)
        .filter(|index| mask[*index as usize])
        .map(|index| (index % width, index / width))
        .collect();
    while !todo.is_empty() {
        let mut filled = Vec::new();
        for &(x, y) in &todo {
            let (mut sum, mut count) = ([0u32; 4], 0);
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let (nx, ny) = (x + dx, y + dy);
                    if nx >= 0 && ny >= 0 && nx < width && ny < height && !mask[(ny * width + nx) as usize] {
                        let pixel = plate.get_pixel(nx as u32, ny as u32);
                        for channel in 0..4 {
                            sum[channel] += u32::from(pixel.0[channel]);
                        }
                        count += 1;
                    }
                }
            }
            if count > 0 {
                filled.push((x, y, image::Rgba(sum.map(|total| (total / count) as u8))));
            }
        }
        if filled.is_empty() {
            break;
        }
        for &(x, y, pixel) in &filled {
            plate.put_pixel(x as u32, y as u32, pixel);
            mask[(y * width + x) as usize] = false;
        }
        todo.retain(|(x, y)| mask[(y * width + x) as usize]);
    }
    let shift = (height - 1 - top - end) / 2;
    for (x, y, pixel) in line {
        if inside(x, y + shift) {
            plate.put_pixel(x as u32, (y + shift) as u32, pixel);
        }
    }
}

#[cfg(test)]
pub fn hud_style_button(assets: &mut Assets, label: &str) -> crate::assets::Result<RgbaImage> {
    let [width, height] = REVIVE_BUTTON;
    let mut image = hud_style_panel(assets, width, height, [176, 158, 104]);
    let (w, h) = (width as i32, height as i32);
    // Cinzel, the decorative font of the titles, with a one-pixel shadow.
    let font = crate::labels::decorative_font()?;
    let text_width = |size: f32| label.chars().map(|c| font.metrics(c, size).advance_width).sum::<f32>();
    let mut size = 17.0;
    while size > 11.0 && text_width(size) > (w - 20) as f32 {
        size -= 0.5;
    }
    let x = ((w as f32 - text_width(size)) / 2.0).max(4.0).round();
    let y = ((h as f32 - size) / 2.0 - 1.0).round();
    crate::labels::text_sized(&mut image, &font, &[(label, [12, 22, 28])], [x + 1.0, y + 1.0], size);
    crate::labels::text_sized(&mut image, &font, &[(label, [150, 214, 216])], [x, y], size);
    Ok(image)
}

/// The Store button: round, past the original's quiver (which shows from
/// 137 to 167 of the bar with a bow in hand, `draw_quiver`) and before the
/// skill bar's ten slots (from 300). Left, top, width and height in window
/// pixels, its glow's room included.
pub fn store_button_rect(viewport: [f32; 2]) -> [f32; 4] {
    let [x, y] = layout::main_hud_position(viewport);
    let [width, height] = crate::emblems::ROUND_BUTTON;
    [x + 170.0, y + 14.0, width as f32, height as f32]
}

/// The Ranking button, beside the Store's.
pub fn ranking_button_rect(viewport: [f32; 2]) -> [f32; 4] {
    let [left, top, width, height] = store_button_rect(viewport);
    [left + width - 2.0, top, width, height]
}

/// The Helper button, beside the Ranking's.
pub fn helper_button_rect(viewport: [f32; 2]) -> [f32; 4] {
    let [left, top, width, height] = ranking_button_rect(viewport);
    [left + width - 2.0, top, width, height]
}

fn hud_origin(viewport: [f32; 2]) -> [i32; 2] {
    let [x, y] = layout::main_hud_position(viewport);
    [x.round() as i32, y.round() as i32]
}



fn hotkey_rect(viewport: [f32; 2], index: usize) -> Rect {
    layout::hotkey_slot_rect(viewport, index).into()
}

pub fn action_slot_at(viewport: [f32; 2], cursor: [f32; 2]) -> Option<usize> {
    layout::hotkey_slot_at(viewport, cursor)
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
}

pub fn items_button_at(viewport: [f32; 2], cursor: [f32; 2]) -> bool {
    layout::main_hud_button_at(viewport, cursor) == Some(MainHudButton::Items)
}

pub fn capture_button_at(viewport: [f32; 2], cursor: [f32; 2]) -> bool {
    layout::main_hud_button_at(viewport, cursor) == Some(MainHudButton::Capture)
}

pub fn action_button_at(viewport: [f32; 2], cursor: [f32; 2]) -> bool {
    layout::main_hud_button_at(viewport, cursor) == Some(MainHudButton::Action)
}

pub fn run_button_at(viewport: [f32; 2], cursor: [f32; 2]) -> bool {
    layout::main_hud_button_at(viewport, cursor) == Some(MainHudButton::Run)
}

pub fn npc_equip_button_at(viewport: [f32; 2], cursor: [f32; 2]) -> bool {
    layout::main_hud_button_at(viewport, cursor) == Some(MainHudButton::NpcEquip)
}

pub fn map_button_at(viewport: [f32; 2], cursor: [f32; 2]) -> bool {
    layout::main_hud_button_at(viewport, cursor) == Some(MainHudButton::Map)
}

pub fn screen_move_button_at(viewport: [f32; 2], cursor: [f32; 2]) -> bool {
    layout::main_hud_button_at(viewport, cursor) == Some(MainHudButton::ScreenMove)
}

/// The original's hint for the round buttons over the orb (`Conquer.exe`
/// 0x46ef5d, `StrRes.ini` 10070, 10072, 10073/10074, 10370).
pub fn round_button_tip(button: MainHudButton, shift_screen: bool) -> Option<&'static str> {
    Some(match button {
        MainHudButton::Run => "Walk/Run",
        MainHudButton::Map => "Map On/Off",
        MainHudButton::ScreenMove if shift_screen => "Shift Screen: On",
        MainHudButton::ScreenMove => "Shift Screen: Off",
        MainHudButton::NpcEquip => "View Equipment",
        _ => return None,
    })
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

pub fn chat_name_at(viewport: [f32; 2], cursor: [f32; 2]) -> bool {
    layout::chat_strip_control_at(viewport, cursor) == Some(ChatStripControl::Name)
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

pub fn hovered_button(viewport: [f32; 2], cursor: [f32; 2]) -> Option<u8> {
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
        let binding = slot.binding();
        let art = match &binding {
            Some(crate::action_bar::Binding::Item(item_type)) => crate::shop::item_icon_image(assets, *item_type).ok(),
            // A skill's art is wherever `ani/Magic.Ani` names it (Scatter is
            // skill29.dds), already cropped.
            Some(crate::action_bar::Binding::Skill(id)) if slot.icon.is_none() => crate::main_hud::skill_icon(assets, *id),
            // Skill textures keep their artwork in a larger transparent
            // canvas; crop it so the icon fills the slot like the original.
            _ => slot
                .icon_path()
                .and_then(|path| assets.image(&path).ok())
                .map(|icon| crate::original_ui::visible_part(&icon))
                .transpose()?,
        };
        if let Some(mut icon) = art {
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
        if let Some(crate::action_bar::Binding::Text(text)) = &binding {
            // A line of text: its first letters on a dark plate.
            fill(img, inner, [20, 24, 30, 230]);
            let short: String = text.chars().take(5).collect();
            let x = inner.0 + (inner.2 - main_hud::text_width(font, &short, 11.0)).max(0) / 2;
            main_hud::text(img, font, &short, 11.0, [x, inner.1 + 10], [255, 236, 170, 255], Some(inner.2));
        }
        if let Some(count) = slot.count {
            let label = count.to_string();
            let x = inner.0 + inner.2 - main_hud::text_width(font, &label, 11.0) - 1;
            main_hud::text(img, font, &label, 11.0, [x, inner.1 + inner.3 - 13], [255, 255, 255, 255], None);
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

fn experience_fraction(level: u8, experience: u64) -> f32 {
    let need = crate::leveling::required(level);
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

    let _ = (chat_history, chat_filter);

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
            xp: u32::from(skills.xp_points),
            xp_ready: skills.xp_ready,
            experience: experience_fraction(level, experience),
            hovered_button,
            hovered_chat,
            chat_text,
            chat_focused,
            chat_name: &skills.chat_recipient,
            chat_name_focused: skills.chat_name_focused,
            chat_channel: skills.chat_channel,
            chat_channel_menu: skills.chat_channel_menu,
            chat_channel_hover: skills.chat_channel_hover,
            kill_mode: skills.kill_mode,
            running: skills.running,
            map_shown: skills.map_shown,
            shift_screen: skills.shift_screen,
            picking_equipment: skills.picking_equipment,
            active_skill_icon: skills
                .active_skill
                .and_then(|id| crate::main_hud::skill_icon(assets, id)),
        },
        font,
    );
    draw_skill_bar(&mut img, assets, font, viewport, skills);
    if let Some(arrows) = skills.quiver {
        draw_quiver(&mut img, assets, font, viewport, arrows);
    }
    // Cyclone or Superman: their KOs in the big red count ("<n> æ–©"),
    // otherwise the kills of the last kind of monster in the small blue one.
    let in_xp_skill = skills.timers.iter().any(|(flag, _)| *flag & XP_SKILL_FLAGS != 0);
    if in_xp_skill {
        draw_count(&mut img, assets, viewport, skills.ko_count, Count::Ko);
    } else if hud_config.kill_counter {
        draw_count(&mut img, assets, viewport, skills.kill_count, Count::Kills);
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
    if let Some(info) = &skills.info {
        draw_info_plate(&mut img, info);
        draw_nobility_button(&mut img, assets, skills);
    }
    let mut badges_top = status_row_top(skills);
    // Timed statuses in framed slots with the time left on a pill under
    // each; their text badges give way.
    let timed = sorted_timers(skills);
    let mut column = 0;
    for (flag, seconds) in &timed {
        let rect = status_slot_rect(column, badges_top);
        draw_status_slot(&mut img, rect, *seconds);
        // The animated flags are left for the renderer to draw here.
        if flag_effect(*flag).is_none()
            && let Some(icon) = status_icon(assets, *flag, 26)
        {
            let (icon_width, icon_height) = icon.dimensions();
            image::imageops::overlay(
                &mut img,
                &icon,
                i64::from(rect.x + (rect.width - icon_width as i32) / 2),
                i64::from(rect.y + (rect.height - 4 - icon_height as i32) / 2),
            );
        }
        column += 1;
    }
    if column > 0 {
        badges_top += STATUS_SLOT + 16;
        let shown = |label: &str| {
            let flag = match label {
                "2X XP" => 1 << 48,
                "Blessing" => 1 << 49,
                "Poisoned" => 1 << 1,
                "Stunned" => 1 << 5,
                "Cyclone" => 1 << 23,
                "Superman" => 1 << 18,
                "Stigma" => 1 << 9,
                "Shield" => 1 << 8,
                _ => 0,
            };
            timed.iter().any(|(bit, _)| *bit == flag)
        };
        badges.retain(|(label, _)| !shown(label));
    }
    for (row, (label, color)) in badges.into_iter().enumerate() {
        draw_badge_pill(&mut img, label, color, [CORNER[0], badges_top + row as i32 * 22]);
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
        let name = slot.binding().map(|binding| match binding {
            crate::action_bar::Binding::Skill(id) => crate::magic::skill_name(assets, id),
            crate::action_bar::Binding::Emote(action) => format!("{action:?}"),
            crate::action_bar::Binding::Item(item_type) => crate::shop::item_name(assets, item_type),
            crate::action_bar::Binding::Text(text) => text,
        });
        if let Some(name) = name.as_deref() {
            let frame = hotkey_rect(viewport, index);
            // A skill shows its level and experience, as in the skill list.
            let progress = match slot.binding() {
                Some(crate::action_bar::Binding::Skill(id)) => {
                    skills.skills.iter().find(|(skill, _, _)| *skill == id).map(|(_, level, experience)| {
                        (format!("Level: {level}"), format!("Experience: {experience}"))
                    })
                }
                _ => None,
            };
            let mut lines: Vec<(&str, [u8; 4])> = vec![(name, [255, 255, 0, 255])];
            if let Some((level, experience)) = &progress {
                lines.push((level, [255, 255, 255, 255]));
                lines.push((experience, [255, 255, 255, 255]));
            }
            lines.push((&slot.hotkey, [200, 200, 200, 255]));
            tooltip(&mut img, font, &lines, [frame.0, frame.1 - 4]);
        }
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
            assert!(!contains(viewport, [viewport[0] * 0.5, viewport[1] * 0.5]));
            assert!(!contains(viewport, [-1.0, viewport[1] - 1.0]));
        }
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
        assert_eq!(
            (short_time(45), short_time(125), short_time(7_300), short_time(200_000)),
            ("45s".to_owned(), "2:05".to_owned(), "2h".to_owned(), "2d".to_owned())
        );
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
        let mut action_bar = crate::action_bar::ActionBar::default();
        // An emote, an object and a line of text on F4 to F6.
        use crate::action_bar::Binding;
        action_bar.set(3, Some(Binding::Emote(crate::action::Action::Dance)));
        action_bar.set(4, Some(Binding::Item(1_000_000)));
        action_bar.slots[4].count = Some(12);
        action_bar.set(5, Some(Binding::Text("Hello all".into())));
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
            ("hud-1704.png", 1704, 960, 100, 100, Some(HOVER_SLOT), ""),
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
                    skills: vec![(1_000, 3, 120), (1_001, 1, 15), (1_025, 1, 0), (1_110, 1, 0)],
                    bar_open: true,
                    active_skill: Some(1_001),
                    kill_count: 7,
                    ko_count: 37,
                    quiver: Some(457),
                    xp_fire_until: None,
                    hovered: Some(1_000),
                    info: Some(InfoBar { map: "Twin City".into(), cell: [430, 380], fps: 144, ping: Some(12) }),
                    xp_ready: true,
                    xp_points: 65,
                    chat_recipient: "Tom411".into(),
                    timers: vec![(1 << 48, 3_400), (1 << 49, 200_000), (1 << 46, 600_000), (1 << 50, 5_000)],
                    ..SkillHud::default()
                },
            )?
            .save(output.join(name))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod revive_plate_preview {
    /// Paints the four revive plates side by side into GRAVAK_PREVIEW_OUT.
    #[test]
    #[ignore = "Needs the 5065 installation"]
    fn revive_plates() {
        let mut assets = crate::assets::Assets::open(std::path::Path::new("R:/5065")).unwrap();
        let plates: Vec<_> = [(false, false, true), (false, true, true), (true, false, true), (true, true, true), (true, false, false)]
            .into_iter()
            .map(|(here, lit, usable)| super::revive_plate(&mut assets, here, lit, usable).unwrap())
            .collect();
        assert!(plates.iter().all(|plate| plate.dimensions() == plates[0].dimensions()));
        let (width, height) = plates[0].dimensions();
        let mut sheet = image::RgbaImage::from_pixel((width + 4) * plates.len() as u32, height, image::Rgba([40, 60, 40, 255]));
        for (index, plate) in plates.iter().enumerate() {
            image::imageops::overlay(&mut sheet, plate, i64::from((width + 4) * index as u32), 0);
        }
        let out = std::env::var("GRAVAK_PREVIEW_OUT").unwrap_or_else(|_| ".".into());
        sheet.save(std::path::Path::new(&out).join("revive-plates.png")).unwrap();
    }
}
