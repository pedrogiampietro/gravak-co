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
    /// When the fire along the bottom burns out (unix ms): a lit XP
    /// circle's 20 seconds, then the XP skill's time.
    pub xp_fire_until: Option<u64>,
    /// The kill mode (`shop::KILL_MODES` order) the Capture button shows.
    pub kill_mode: u8,
    /// The top-left information lines: map and position, FPS and ping.
    pub info: Option<InfoBar>,
    /// Skill under the cursor in the bar; filled in by the renderer.
    pub hovered: Option<u32>,
    /// Timed statuses: their flag and the seconds left.
    pub timers: Vec<(u64, u32)>,
    /// The Character window's Skills tab: page, scroll and pick.
    pub skill_page: crate::shop::SkillPage,
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

/// The plate: a compass badge, the map's name, and under it the cell, the
/// frames a second and the ping with its signal bars. Its bottom.
fn draw_info_plate(img: &mut RgbaImage, info: &InfoBar) {
    use crate::daily_rewards_panel::{glow_in, paint_in, rounded, say, width_of, GOLD_DARK, GOLD_LIGHT};
    use crate::paint;
    let (Ok(ui), Ok(cinzel)) = (crate::labels::font(), crate::labels::decorative_font()) else {
        return;
    };
    let ping_text = info.ping.map_or_else(|| "-".to_owned(), |ping| format!("{ping} ms"));
    let detail = format!("{}, {}   ·   {} FPS   ·   {ping_text}", info.cell[0], info.cell[1], info.fps);
    let text_width = width_of(&cinzel, &info.map, 14.0).max(width_of(&ui, &detail, 11.0) + 20.0).round() as i32;
    let plate = crate::ui::UiRect::new(CORNER[0], CORNER[1], 52 + text_width + 14, PLATE_HEIGHT);
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
    say(img, &cinzel, &info.map, [255, 226, 143], left, plate.y + 6, 14.0);
    say(img, &ui, &detail, [196, 204, 200], left, plate.y + 27, 11.0);
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
    let bars_left = left + width_of(&ui, &detail, 11.0).round() as i32 + 6;
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

/// What a status's icon shows: a skill's icon, an item's, or the VIP title.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StatusIcon {
    Skill(u32),
    Item(u32),
    Vip,
    /// One of the original's animated status flags (`3DEffect.ini`), drawn
    /// by the renderer over the HUD.
    Flag(&'static str),
}

/// The icon of each timed status flag.
const STATUS_ICONS: [(u64, StatusIcon); 14] = [
    (1 << 18, StatusIcon::Skill(1_025)), // Superman
    (1 << 23, StatusIcon::Skill(1_110)), // Cyclone
    (1 << 9, StatusIcon::Skill(1_095)),  // Stigma
    (1 << 8, StatusIcon::Skill(1_020)),  // Shield
    (1 << 7, StatusIcon::Skill(1_015)),  // Accuracy
    (1 << 22, StatusIcon::Skill(1_075)), // Invisibility
    (1 << 27, StatusIcon::Skill(8_002)), // Fly
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
        StatusIcon::Item(item) => crate::shop::item_icon_image(assets, item).ok()?,
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
    /// The effect prefix, the 斩's centre from the bottom right and the
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
pub fn revive_button(assets: &mut Assets, label: &str) -> crate::assets::Result<RgbaImage> {
    crate::ornate::plate(Some(assets), REVIVE_BUTTON, label, crate::ornate::GRAVAK, crate::ornate::Look::Normal, None)
}

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

/// The Store button: beside the round buttons over the orb, above the chat
/// strip. Left, top, width and height in window pixels.
pub fn store_button_rect(viewport: [f32; 2]) -> [f32; 4] {
    let [x, y] = layout::main_hud_position(viewport);
    let [width, height] = crate::store_panel::BUTTON_SIZE;
    [x + 96.0, y + 30.0, width as f32, height as f32]
}

/// The Ranking button, beside the Store's.
pub fn ranking_button_rect(viewport: [f32; 2]) -> [f32; 4] {
    let [left, top, width, height] = store_button_rect(viewport);
    [left + width + 4.0, top, width, height]
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
    layout::main_hud_solid_at(viewport, cursor) || chat_tab_at(viewport, cursor).is_some()
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
        2500 => ([255, 214, 96, 255], Some(("[Broadcast]", [255, 160, 60, 255]))),
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
            xp: u32::from(skills.xp_points),
            xp_ready: skills.xp_ready,
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
    // Cyclone or Superman: their KOs in the big red count ("<n> 斩"),
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
                    xp_fire_until: None,
                    hovered: Some(1_000),
                    info: Some(InfoBar { map: "Twin City".into(), cell: [430, 380], fps: 144, ping: Some(12) }),
                    xp_ready: true,
                    xp_points: 65,
                    timers: vec![(1 << 48, 3_400), (1 << 49, 200_000), (1 << 46, 600_000), (1 << 50, 5_000)],
                    ..SkillHud::default()
                },
            )?
            .save(output.join(name))?;
        }
        Ok(())
    }
}
