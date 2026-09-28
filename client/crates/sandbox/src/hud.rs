use crate::assets::{Assets, Result};
use image::{Pixel, Rgba, RgbaImage, imageops};
use std::sync::OnceLock;

// All painting and hit testing share the reference's 1704 x 228 coordinate space.
const BASE_WIDTH: u32 = 1704;
const BASE_HEIGHT: u32 = 228;
const UI_WIDTH_RATIO: f32 = 0.84;
const GOLD: [u8; 4] = [181, 139, 66, 255];
const LIGHT: [u8; 4] = [246, 219, 151, 255];
const DARK: [u8; 4] = [18, 40, 51, 255];
const STEEL: [u8; 4] = [71, 94, 101, 255];
const TEXT: [u8; 4] = [238, 221, 177, 255];
const DESIGN_WIDTH: f32 = 1920.0;
const DESIGN_HEIGHT: f32 = 1080.0;
const SAFE_MARGIN: f32 = 24.0;
const CHAT: Rect = Rect(322, 77, 731, 36);
const TALK: Rect = Rect(225, 77, 97, 36);
const CLEAR: Rect = Rect(1117, 76, 110, 39);
const CHAT_BUTTON: Rect = Rect(1235, 76, 88, 39);
const MENU_LABELS: [&str; 10] = [
    "Status", "Items", "Friend", "Capture", "Guild", "Action", "Trade", "Team", "Options",
    "Message",
];

#[derive(Clone, Copy)]
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
#[derive(Clone, Copy)]
struct UiMetrics {
    width: f32,
    height: f32,
    scale: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HudConfig {
    pub center_arcs: bool,
    pub center_bars: bool,
    pub center_scale: u8,
    pub center_arc_distance: u8,
}

impl Default for HudConfig {
    fn default() -> Self {
        Self {
            center_arcs: true,
            center_bars: true,
            center_scale: 100,
            center_arc_distance: 118,
        }
    }
}
impl UiMetrics {
    fn new(viewport: [f32; 2]) -> Self {
        Self {
            width: viewport[0].max(1.0),
            height: viewport[1].max(1.0),
            scale: (viewport[0] / DESIGN_WIDTH)
                .min(viewport[1] / DESIGN_HEIGHT)
                .clamp(0.75, 1.5),
        }
    }
    fn x(self, value: f32) -> i32 {
        (value * self.scale).round() as i32
    }
    fn rect(self, x: f32, y: f32, width: f32, height: f32) -> Rect {
        Rect(
            self.x(x),
            self.x(y),
            self.x(width).max(1),
            self.x(height).max(1),
        )
    }
    fn right(self, margin: f32, width: f32) -> i32 {
        self.x(self.width / self.scale - margin - width)
    }
    fn bottom(self, margin: f32, height: f32) -> i32 {
        self.x(self.height / self.scale - margin - height)
    }
}
fn design_player_rect(metrics: UiMetrics) -> Rect {
    metrics.rect(SAFE_MARGIN, SAFE_MARGIN, 460.0, 154.0)
}
fn design_buff_rect(metrics: UiMetrics) -> Rect {
    metrics.rect(SAFE_MARGIN, 186.0, 360.0, 44.0)
}
fn design_chat_rect(metrics: UiMetrics) -> Rect {
    Rect(
        metrics.x(SAFE_MARGIN),
        metrics.bottom(SAFE_MARGIN, 174.0),
        metrics.x(520.0),
        metrics.x(174.0),
    )
}
fn design_skill_rect(metrics: UiMetrics) -> Rect {
    let logical_width = metrics.width / metrics.scale;
    let logical_height = metrics.height / metrics.scale;
    let width = 820.0_f32.min((logical_width - SAFE_MARGIN * 2.0).max(640.0));
    let centered_x = logical_width / 2.0 - width / 2.0;
    // Keep the left ornament clear of the chat panel at the compact
    // resolutions where both controls otherwise collide.
    let chat_right = SAFE_MARGIN + 520.0;
    let x = centered_x.max(chat_right + 24.0);
    let width = width.min((logical_width - x - SAFE_MARGIN).max(640.0));
    Rect(
        metrics.x(x),
        metrics.x(logical_height - 16.0 - 138.0),
        metrics.x(width),
        metrics.x(138.0),
    )
}
fn design_quest_rect(metrics: UiMetrics) -> Rect {
    Rect(
        metrics.right(SAFE_MARGIN, 310.0),
        metrics.x(200.0),
        metrics.x(310.0),
        metrics.x(238.0),
    )
}
fn design_quick_rect(metrics: UiMetrics) -> Rect {
    let logical_width = metrics.width / metrics.scale;
    let logical_height = metrics.height / metrics.scale;
    // On compact viewports the bottom-right menu would collide with the
    // action bar. Move it above the bar while keeping the same right anchor.
    let y = if logical_width < 1500.0 {
        150.0
    } else {
        logical_height - SAFE_MARGIN - 220.0
    };
    Rect(
        metrics.right(SAFE_MARGIN, 350.0),
        metrics.x(y),
        metrics.x(350.0),
        metrics.x(220.0),
    )
}
fn design_menu_rect(metrics: UiMetrics, index: usize) -> Rect {
    let base = design_quick_rect(metrics);
    // These horizontal centers are measured from quick_menu_trim.png. The
    // vertical centers are intentionally spaced farther apart than the source
    // strip so labels can sit below the upper row without touching the lower
    // row.
    const CENTER_X: [f32; 5] = [36.0, 105.0, 175.0, 245.0, 314.0];
    const CENTER_Y: [f32; 2] = [70.0, 155.0];
    let x = base.0 as f32 / metrics.scale + CENTER_X[index % 5] - 30.0;
    let y = base.1 as f32 / metrics.scale + CENTER_Y[index / 5] - 30.0;
    metrics.rect(x, y, 60.0, 60.0)
}
fn design_skill_slot(metrics: UiMetrics, index: usize) -> Rect {
    let base = design_skill_rect(metrics);
    // Keep all ten hotkeys in one continuous row. The old layout inherited a
    // large atlas gap between F5 and F6, making the bar look unbalanced.
    let logical_width = base.2 as f32 / metrics.scale;
    let slot_width = ((logical_width - 100.0) / 10.0).clamp(44.0, 58.0);
    let gap = 4.0;
    let x = base.0 as f32 / metrics.scale + 50.0 + index.min(9) as f32 * (slot_width + gap);
    let y = base.1 as f32 / metrics.scale + 62.0;
    metrics.rect(x, y, slot_width, 54.0)
}
fn design_action_context_menu(metrics: UiMetrics, slot: usize) -> Rect {
    let skill_slot = design_skill_slot(metrics, slot.min(9));
    let menu_width = metrics.x(190.0);
    let row_height = metrics.x(29.0).max(1);
    let menu_height = metrics.x(12.0) + row_height * 4;
    let logical_width = metrics.width / metrics.scale;
    let min_x = metrics.x(6.0);
    let max_x = metrics.x(logical_width - 6.0) - menu_width;
    let centered_x = skill_slot.0 + skill_slot.2 / 2 - menu_width / 2;
    let x = centered_x.clamp(min_x, max_x.max(min_x));
    let above_y = skill_slot.1 - menu_height - metrics.x(8.0);
    let y = if above_y >= metrics.x(6.0) {
        above_y
    } else {
        skill_slot.1 + skill_slot.3 + metrics.x(8.0)
    };
    Rect(x, y, menu_width, menu_height)
}
pub fn action_slot_at(viewport: [f32; 2], cursor: [f32; 2]) -> Option<usize> {
    let metrics = UiMetrics::new(viewport);
    (0..10).find(|&index| design_skill_slot(metrics, index).contains(cursor))
}
pub fn action_context_option_at(
    viewport: [f32; 2],
    cursor: [f32; 2],
    slot: usize,
) -> Option<usize> {
    let metrics = UiMetrics::new(viewport);
    let menu = design_action_context_menu(metrics, slot);
    if !menu.contains(cursor) {
        return None;
    }
    let padding = metrics.x(6.0);
    let row_height = metrics.x(29.0).max(1);
    let y = cursor[1] as i32 - menu.1 - padding;
    let option = y.div_euclid(row_height) as usize;
    (option < 4).then_some(option)
}
fn menu_rect(index: usize) -> Rect {
    Rect(
        887 + (index % 5) as i32 * 99,
        130 + (index / 5) as i32 * 49,
        94,
        39,
    )
}

fn menu_hit_rect(index: usize) -> Rect {
    let r = menu_rect(index);
    Rect(r.0 - 4, r.1 - 4, r.2 + 8, r.3 + 8)
}
fn key_rect(index: usize) -> Rect {
    Rect(186 + index as i32 * 68, 132, 63, 85)
}
fn scale(width: f32) -> f32 {
    (width.max(1.0) * UI_WIDTH_RATIO) / BASE_WIDTH as f32
}
fn scaled_height(width: f32) -> u32 {
    (BASE_HEIGHT as f32 * scale(width)).round().max(1.0) as u32
}
pub fn position(viewport: [f32; 2]) -> [f32; 2] {
    let _ = viewport;
    [0.0, 0.0]
}
fn logical_cursor(viewport: [f32; 2], cursor: [f32; 2]) -> [f32; 2] {
    [
        (cursor[0] - position(viewport)[0]) / scale(viewport[0]),
        (cursor[1] - position(viewport)[1]) / scale(viewport[0]),
    ]
}
pub fn render_width(viewport_width: u32) -> u32 {
    viewport_width.max(1)
}
pub fn render_height(viewport_height: u32) -> u32 {
    viewport_height.max(1)
}
pub fn contains(viewport: [f32; 2], cursor: [f32; 2]) -> bool {
    if !Rect(0, 0, viewport[0] as i32, viewport[1] as i32).contains(cursor) {
        return false;
    }
    let metrics = UiMetrics::new(viewport);
    design_player_rect(metrics).contains(cursor)
        || design_buff_rect(metrics).contains(cursor)
        || design_chat_rect(metrics).contains(cursor)
        || design_skill_rect(metrics).contains(cursor)
        || design_quest_rect(metrics).contains(cursor)
        || design_quick_rect(metrics).contains(cursor)
}
pub fn items_button_at(viewport: [f32; 2], cursor: [f32; 2]) -> bool {
    quick_button_at(viewport, cursor) == Some(1)
}
pub fn status_button_at(viewport: [f32; 2], cursor: [f32; 2]) -> bool {
    quick_button_at(viewport, cursor) == Some(0)
}
pub fn quick_button_at(viewport: [f32; 2], cursor: [f32; 2]) -> Option<usize> {
    let metrics = UiMetrics::new(viewport);
    (0..10).find(|&index| design_menu_rect(metrics, index).contains(cursor))
}
pub fn chat_input_at(viewport: [f32; 2], cursor: [f32; 2]) -> bool {
    let r = design_chat_rect(UiMetrics::new(viewport));
    Rect(r.0 + r.2 / 2 - r.2 / 2 + 12, r.1 + r.3 - 44, r.2 - 24, 32).contains(cursor)
}
pub fn chat_clear_at(viewport: [f32; 2], cursor: [f32; 2]) -> bool {
    let r = design_chat_rect(UiMetrics::new(viewport));
    Rect(r.0 + r.2 - 88, r.1 + 8, 76, 24).contains(cursor)
}
pub fn chat_button_at(viewport: [f32; 2], cursor: [f32; 2]) -> bool {
    let r = design_chat_rect(UiMetrics::new(viewport));
    Rect(r.0, r.1, r.2, 46).contains(cursor)
}
pub fn talk_channel_at(viewport: [f32; 2], cursor: [f32; 2]) -> bool {
    let r = design_chat_rect(UiMetrics::new(viewport));
    Rect(r.0 + 12, r.1 + 8, 94, 24).contains(cursor)
}
pub fn chat_tab_at(viewport: [f32; 2], cursor: [f32; 2]) -> Option<u16> {
    let metrics = UiMetrics::new(viewport);
    let r = design_chat_rect(metrics);
    let channels = [0, 2021, 2000, 2004, 2003, 2007];
    channels
        .into_iter()
        .enumerate()
        .find_map(|(index, channel)| {
            let tab = Rect(
                r.0 + metrics.x(12.0 + index as f32 * 70.0),
                r.1 + metrics.x(10.0),
                metrics.x(63.0),
                metrics.x(24.0),
            );
            tab.contains(cursor).then_some(channel)
        })
}
pub fn hovered_button(viewport: [f32; 2], cursor: [f32; 2]) -> Option<u8> {
    let metrics = UiMetrics::new(viewport);
    for index in 0..10 {
        if design_menu_rect(metrics, index).contains(cursor) {
            return Some(index as u8);
        }
    }
    // The centered skillbar visually sits above the chat panel on shorter
    // viewports, so its slots receive hit-test priority in the overlap.
    for index in 0..10 {
        if design_skill_slot(metrics, index).contains(cursor) {
            return Some(20 + index as u8);
        }
    }
    if chat_clear_at(viewport, cursor) {
        return Some(11);
    }
    if chat_button_at(viewport, cursor) {
        return Some(12);
    }
    if talk_channel_at(viewport, cursor) {
        return Some(13);
    }
    if chat_input_at(viewport, cursor) {
        return Some(14);
    }
    None
}

fn pixel(img: &mut RgbaImage, x: i32, y: i32, color: [u8; 4]) {
    if x >= 0 && y >= 0 && x < img.width() as i32 && y < img.height() as i32 {
        img.get_pixel_mut(x as u32, y as u32).blend(&Rgba(color));
    }
}
fn line(img: &mut RgbaImage, a: [i32; 2], b: [i32; 2], color: [u8; 4]) {
    let (mut x, mut y) = (a[0], a[1]);
    let (dx, dy) = ((b[0] - x).abs(), -(b[1] - y).abs());
    let (sx, sy) = (if x < b[0] { 1 } else { -1 }, if y < b[1] { 1 } else { -1 });
    let mut error = dx + dy;
    loop {
        pixel(img, x, y, color);
        if x == b[0] && y == b[1] {
            break;
        }
        let twice = 2 * error;
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
fn path(img: &mut RgbaImage, points: &[[i32; 2]], color: [u8; 4]) {
    for pair in points.windows(2) {
        line(img, pair[0], pair[1], color);
    }
}
fn fill(img: &mut RgbaImage, r: Rect, color: [u8; 4]) {
    for y in r.1..r.1 + r.3 {
        for x in r.0..r.0 + r.2 {
            pixel(img, x, y, color);
        }
    }
}
fn chamfer(img: &mut RgbaImage, r: Rect, cut: i32, color: [u8; 4]) {
    let Rect(x, y, w, h) = r;
    path(
        img,
        &[
            [x + cut, y],
            [x + w - cut - 1, y],
            [x + w - 1, y + cut],
            [x + w - 1, y + h - cut - 1],
            [x + w - cut - 1, y + h - 1],
            [x + cut, y + h - 1],
            [x, y + h - cut - 1],
            [x, y + cut],
            [x + cut, y],
        ],
        color,
    );
}
fn plate(img: &mut RgbaImage, r: Rect, active: bool) {
    let Rect(x, y, w, h) = r;
    for py in 0..h {
        for px in 0..w {
            let edge = px.min(w - 1 - px) + py.min(h - 1 - py);
            if edge < 7 {
                continue;
            }
            let grain = ((px * 13 + py * 7 + px * py) % 7) as u8;
            let light = ((1.0 - py as f32 / h as f32) * 7.0) as u8;
            pixel(
                img,
                x + px,
                y + py,
                [
                    14 + grain / 2 + light,
                    34 + grain / 2 + light + if active { 9 } else { 0 },
                    47 + grain + light + if active { 10 } else { 0 },
                    255,
                ],
            );
        }
    }
    chamfer(img, r, 7, [9, 12, 15, 255]);
    chamfer(img, r.inset(1), 6, if active { LIGHT } else { STEEL });
    chamfer(img, r.inset(2), 5, DARK);
    chamfer(img, r.inset(4), 4, [47, 69, 77, 255]);
    line(img, [x + 8, y + 3], [x + w - 9, y + 3], [152, 133, 83, 170]);
    line(
        img,
        [x + 8, y + h - 3],
        [x + w - 9, y + h - 3],
        [31, 56, 66, 255],
    );
}
fn diamond(img: &mut RgbaImage, x: i32, y: i32, rx: i32, ry: i32, blue: bool) {
    for dy in -ry..=ry {
        for dx in -rx..=rx {
            if dx.abs() * ry + dy.abs() * rx <= rx * ry {
                pixel(
                    img,
                    x + dx,
                    y + dy,
                    if blue {
                        if dx < 0 {
                            [41, 155, 218, 255]
                        } else {
                            [7, 51, 88, 255]
                        }
                    } else {
                        [15, 25, 32, 255]
                    },
                );
            }
        }
    }
    path(
        img,
        &[
            [x, y - ry],
            [x + rx, y],
            [x, y + ry],
            [x - rx, y],
            [x, y - ry],
        ],
        GOLD,
    );
    path(
        img,
        &[[x - rx + 2, y], [x, y - ry + 2], [x + rx - 2, y]],
        LIGHT,
    );
}
fn corner(img: &mut RgbaImage, x: i32, y: i32, sx: i32, sy: i32) {
    let pts = [[0, 21], [0, 0], [21, 0], [15, 6], [6, 6], [6, 15], [0, 21]];
    let pts: Vec<_> = pts.iter().map(|p| [x + p[0] * sx, y + p[1] * sy]).collect();
    path(img, &pts, GOLD);
    line(
        img,
        [x + 3 * sx, y + 13 * sy],
        [x + 13 * sx, y + 3 * sy],
        LIGHT,
    );
    diamond(img, x + 7 * sx, y + 7 * sy, 2, 3, false);
}
fn corner_sized(img: &mut RgbaImage, x: i32, y: i32, sx: i32, sy: i32, size: i32) {
    let size = size.clamp(8, 21);
    let scale = size as f32 / 21.0;
    let point = |px: i32, py: i32| {
        [
            x + (px as f32 * scale).round() as i32 * sx,
            y + (py as f32 * scale).round() as i32 * sy,
        ]
    };
    let points = [
        point(0, 21),
        point(0, 0),
        point(21, 0),
        point(15, 6),
        point(6, 6),
        point(6, 15),
        point(0, 21),
    ];
    path(img, &points, GOLD);
    line(img, point(3, 13), point(13, 3), LIGHT);
    let jewel = (7.0 * scale).round() as i32;
    diamond(
        img,
        x + jewel * sx,
        y + jewel * sy,
        (2.0 * scale).round().max(1.0) as i32,
        (3.0 * scale).round().max(1.0) as i32,
        false,
    );
}
fn glow(img: &mut RgbaImage, cx: i32, cy: i32, radius: i32, blue: bool) {
    for y in -radius..=radius {
        for x in -radius..=radius {
            let d = ((x * x + y * y) as f32).sqrt() / radius as f32;
            if d < 1.0 {
                let a = ((1.0 - d).powi(3) * 130.0) as u8;
                pixel(
                    img,
                    cx + x,
                    cy + y,
                    if blue {
                        [30, 145, 255, a]
                    } else {
                        [255, 178, 50, a]
                    },
                );
            }
        }
    }
    line(
        img,
        [cx - 10, cy],
        [cx + 10, cy],
        if blue {
            [120, 210, 255, 180]
        } else {
            [255, 219, 145, 180]
        },
    );
    diamond(img, cx, cy, 2, 5, blue);
}
fn ring(img: &mut RgbaImage, radius: f32, color: [u8; 4], start: f32, end: f32) {
    let points: Vec<_> = (0..=360)
        .map(|i| {
            let angle = start + (end - start) * i as f32 / 360.0;
            [
                96 + (angle.cos() * radius) as i32,
                132 + (angle.sin() * radius) as i32,
            ]
        })
        .collect();
    path(img, &points, color);
}
fn orb_frame(img: &mut RgbaImage) {
    use std::f32::consts::TAU;
    for r in 72..=87 {
        ring(img, r as f32, [8, 18, 27, 255], 0.0, TAU);
    }
    for (r, c) in [
        (70.0, GOLD),
        (72.0, LIGHT),
        (75.0, DARK),
        (82.0, GOLD),
        (85.0, [105, 81, 42, 255]),
    ] {
        ring(img, r, c, 0.0, TAU);
    }
    for i in 0..32 {
        let a = i as f32 * TAU / 32.0;
        let p = |r: f32, t: f32| [96 + (r * t.cos()) as i32, 132 + (r * t.sin()) as i32];
        path(
            img,
            &[p(77.0, a - 0.035), p(81.0, a), p(77.0, a + 0.06)],
            if i % 3 == 0 { LIGHT } else { GOLD },
        );
    }
    for offset in 0..3 {
        ring(img, 90.0 + offset as f32, [31, 110, 192, 95], 0.2, 2.8);
        ring(img, 90.0 + offset as f32, [43, 151, 235, 120], 3.4, 5.8);
    }
    for (x, y) in [(25, 77), (160, 75), (20, 181), (173, 175)] {
        glow(img, x, y, 25, true);
    }
    for (x, y, rx, ry) in [
        (96, 43, 12, 23),
        (96, 206, 12, 21),
        (16, 132, 14, 10),
        (177, 132, 14, 10),
    ] {
        diamond(img, x, y, rx, ry, false);
        diamond(img, x, y, rx / 2, ry / 2, true);
    }
    diamond(img, 96, 26, 6, 10, false);
    line(img, [94, 61], [94, 200], LIGHT);
    line(img, [97, 61], [97, 200], GOLD);
}
fn orb_contents(img: &mut RgbaImage, health: u32, max_health: u32, mana: u32, max_mana: u32) {
    let hp = (health as f32 / max_health.max(1) as f32).clamp(0.0, 1.0);
    let mp = (mana as f32 / max_mana.max(1) as f32).clamp(0.0, 1.0);
    for y in 65..200 {
        for x in 29..164 {
            let dx = (x as f32 - 96.0) / 67.0;
            let dy = (y as f32 - 132.0) / 67.0;
            let d = dx * dx + dy * dy;
            if d > 1.0 || (94..=97).contains(&x) {
                continue;
            }
            let fraction = if x < 96 { hp } else { mp };
            let filled = y as f32 >= 199.0 - 134.0 * fraction;
            let edge = (1.0 - d).sqrt();
            let shine = (-((dx + 0.32).powi(2) + (dy + 0.5).powi(2)) * 24.0).exp();
            let shade = 0.35 + 0.65 * edge;
            let base = if !filled {
                [9.0, 18.0, 28.0]
            } else if x < 96 {
                [203.0, 12.0, 26.0]
            } else {
                [9.0, 77.0, 221.0]
            };
            let mut c = [0, 0, 0, 255];
            for channel in 0..3 {
                c[channel] = (base[channel] * shade + shine * 105.0).min(255.0) as u8;
            }
            if filled && (y as f32 - (199.0 - 134.0 * fraction)).abs() < 1.0 {
                c[0] = c[0].saturating_add(40);
                c[1] = c[1].saturating_add(40);
                c[2] = c[2].saturating_add(40);
            }
            img.put_pixel(x as u32, y as u32, Rgba(c));
        }
    }
}
fn font() -> Result<&'static fontdue::Font> {
    static FONT: OnceLock<fontdue::Font> = OnceLock::new();
    if let Some(font) = FONT.get() {
        return Ok(font);
    }
    let mut db = fontdb::Database::new();
    db.load_system_fonts();
    let id = db
        .query(&fontdb::Query {
            families: &[
                fontdb::Family::Name("Georgia"),
                fontdb::Family::Name("Times New Roman"),
                fontdb::Family::Serif,
            ],
            ..Default::default()
        })
        .ok_or("No HUD font available")?;
    let value = db
        .with_face_data(id, |bytes, index| {
            fontdue::Font::from_bytes(
                bytes,
                fontdue::FontSettings {
                    collection_index: index,
                    ..Default::default()
                },
            )
        })
        .ok_or("HUD font unavailable")??;
    let _ = FONT.set(value);
    Ok(FONT.get().expect("HUD font initialized"))
}
fn brand_font() -> Result<&'static fontdue::Font> {
    static FONT: OnceLock<fontdue::Font> = OnceLock::new();
    if let Some(font) = FONT.get() {
        return Ok(font);
    }
    let mut db = fontdb::Database::new();
    db.load_system_fonts();
    let id = db
        .query(&fontdb::Query {
            families: &[
                fontdb::Family::Name("Impact"),
                fontdb::Family::Name("Arial Black"),
                fontdb::Family::Name("Cooper Black"),
                fontdb::Family::Name("Georgia"),
            ],
            ..Default::default()
        })
        .ok_or("No brand font available")?;
    let value = db
        .with_face_data(id, |bytes, index| {
            fontdue::Font::from_bytes(
                bytes,
                fontdue::FontSettings {
                    collection_index: index,
                    ..Default::default()
                },
            )
        })
        .ok_or("Brand font unavailable")??;
    let _ = FONT.set(value);
    Ok(FONT.get().expect("brand font initialized"))
}
fn text(img: &mut RgbaImage, font: &fontdue::Font, value: &str, size: f32, r: Rect, center: bool) {
    let width: f32 = value
        .chars()
        .map(|c| font.metrics(c, size).advance_width)
        .sum();
    let mut cursor = r.0 as f32
        + if center {
            ((r.2 as f32 - width) * 0.5).max(0.0)
        } else {
            0.0
        };
    let baseline = r.1 + ((r.3 as f32 - size) * 0.5) as i32 + size as i32;
    for ch in value.chars() {
        let (m, b) = font.rasterize(ch, size);
        for row in 0..m.height {
            for col in 0..m.width {
                let x = cursor.round() as i32 + m.xmin + col as i32;
                let y = baseline - m.ymin - m.height as i32 + row as i32;
                if !r.contains([x as f32, y as f32]) {
                    continue;
                }
                let a = b[row * m.width + col];
                pixel(img, x + 1, y + 1, [0, 0, 0, a]);
                pixel(img, x, y, [TEXT[0], TEXT[1], TEXT[2], a]);
            }
        }
        cursor += m.advance_width;
    }
}
fn fitted_text_size(font: &fontdue::Font, value: &str, max_width: f32, preferred: f32) -> f32 {
    let width: f32 = value
        .chars()
        .map(|ch| font.metrics(ch, preferred).advance_width)
        .sum();
    if width <= max_width {
        return preferred;
    }
    (preferred * max_width / width).clamp(12.0, preferred)
}
fn ellipse(img: &mut RgbaImage, r: Rect, c: [u8; 4]) {
    for y in 0..r.3 {
        for x in 0..r.2 {
            if ((x as f32 + 0.5 - r.2 as f32 / 2.0) / (r.2 as f32 / 2.0)).powi(2)
                + ((y as f32 + 0.5 - r.3 as f32 / 2.0) / (r.3 as f32 / 2.0)).powi(2)
                <= 1.0
            {
                pixel(img, r.0 + x, r.1 + y, c);
            }
        }
    }
}
// Small gold pictograms stay separate from labels and use the menu's actual order.
fn glyph(img: &mut RgbaImage, id: usize, x: i32, y: i32) {
    // Match the original menu's warm gold pictograms instead of the white
    // placeholder glyphs used during the first HUD pass.
    let ink = [232, 187, 83, 255];
    let dark = [12, 22, 28, 255];
    let parchment = [245, 211, 132, 255];
    let blue = [77, 177, 255, 255];
    match id {
        0 => {
            path(
                img,
                &[
                    [x + 3, y + 12],
                    [x + 4, y + 7],
                    [x + 8, y + 2],
                    [x + 13, y + 1],
                    [x + 18, y + 4],
                    [x + 21, y + 10],
                    [x + 19, y + 13],
                    [x + 3, y + 13],
                ],
                ink,
            );
            fill(img, Rect(x + 6, y + 10, 14, 4), dark);
            line(img, [x + 7, y + 15], [x + 18, y + 15], ink);
            line(img, [x + 11, y + 3], [x + 11, y + 8], parchment);
        }
        1 => {
            ellipse(img, Rect(x + 2, y + 6, 19, 15), ink);
            path(
                img,
                &[
                    [x + 6, y + 8],
                    [x + 5, y + 2],
                    [x + 10, y + 4],
                    [x + 16, y + 2],
                    [x + 15, y + 8],
                ],
                ink,
            );
            line(img, [x + 4, y + 9], [x + 18, y + 9], dark);
            line(img, [x + 12, y + 10], [x + 12, y + 20], GOLD);
        }
        2 => {
            diamond(img, x + 11, y + 11, 4, 9, false);
            line(img, [x + 11, y], [x + 11, y + 4], blue);
            line(img, [x + 11, y + 18], [x + 11, y + 22], blue);
            line(img, [x, y + 11], [x + 4, y + 11], blue);
            line(img, [x + 18, y + 11], [x + 22, y + 11], blue);
            line(img, [x + 4, y + 4], [x + 7, y + 7], parchment);
            line(img, [x + 15, y + 15], [x + 18, y + 18], parchment);
        }
        5 => {
            for (dx, dy) in [(0, 4), (8, 0), (16, 4)] {
                ellipse(img, Rect(x + dx, y + dy, 6, 7), ink);
                ellipse(img, Rect(x + dx - 1, y + dy + 8, 8, 11), ink);
            }
        }
        3 => {
            fill(img, Rect(x + 4, y + 3, 16, 18), parchment);
            ellipse(img, Rect(x + 2, y + 3, 5, 5), ink);
            ellipse(img, Rect(x + 17, y + 16, 5, 5), ink);
            line(img, [x + 7, y + 8], [x + 17, y + 8], dark);
            line(img, [x + 7, y + 12], [x + 17, y + 12], dark);
            line(img, [x + 7, y + 16], [x + 14, y + 16], dark);
        }
        4 => {
            path(
                img,
                &[
                    [x + 2, y + 3],
                    [x + 11, y],
                    [x + 21, y + 3],
                    [x + 19, y + 15],
                    [x + 11, y + 21],
                    [x + 4, y + 15],
                    [x + 2, y + 3],
                ],
                ink,
            );
            path(
                img,
                &[
                    [x + 6, y + 6],
                    [x + 16, y + 6],
                    [x + 15, y + 13],
                    [x + 11, y + 16],
                    [x + 7, y + 13],
                    [x + 6, y + 6],
                ],
                GOLD,
            );
            line(img, [x + 11, y + 4], [x + 11, y + 15], ink);
        }
        6 => {
            fill(img, Rect(x + 1, y + 5, 21, 15), parchment);
            path(
                img,
                &[[x + 1, y + 6], [x + 11, y + 14], [x + 22, y + 6]],
                dark,
            );
            line(img, [x + 2, y + 19], [x + 8, y + 14], ink);
            line(img, [x + 20, y + 19], [x + 14, y + 14], ink);
        }
        7 => {
            line(img, [x + 11, y + 2], [x + 11, y + 20], ink);
            line(img, [x + 4, y + 6], [x + 18, y + 6], ink);
            line(img, [x + 6, y + 6], [x + 3, y + 15], ink);
            line(img, [x + 16, y + 6], [x + 19, y + 15], ink);
            path(
                img,
                &[[x + 1, y + 15], [x + 5, y + 18], [x + 9, y + 15]],
                ink,
            );
            path(
                img,
                &[[x + 13, y + 15], [x + 17, y + 18], [x + 21, y + 15]],
                ink,
            );
            line(img, [x + 7, y + 21], [x + 15, y + 21], ink);
        }
        8 => {
            for i in 0..8 {
                let a = i as f32 * std::f32::consts::TAU / 8.0;
                let dx = (a.cos() * 10.0) as i32;
                let dy = (a.sin() * 10.0) as i32;
                fill(img, Rect(x + 9 + dx, y + 9 + dy, 5, 5), ink);
            }
            ellipse(img, Rect(x + 3, y + 3, 18, 18), ink);
            ellipse(img, Rect(x + 8, y + 8, 8, 8), dark);
        }
        9 => {
            for (line_y, knob_x) in [(5, 7), (11, 16), (17, 10)] {
                line(img, [x + 2, y + line_y], [x + 22, y + line_y], ink);
                ellipse(img, Rect(x + knob_x - 2, y + line_y - 2, 5, 5), parchment);
            }
        }
        10 => {
            fill(img, Rect(x + 5, y + 5, 14, 16), ink);
            fill(img, Rect(x + 3, y + 2, 18, 2), ink);
            fill(img, Rect(x + 9, y, 6, 2), ink);
            for dx in [8, 12, 16] {
                line(img, [x + dx, y + 8], [x + dx, y + 18], dark);
            }
        }
        _ => {
            ellipse(img, Rect(x, y + 2, 24, 17), ink);
            path(
                img,
                &[[x + 5, y + 14], [x + 5, y + 22], [x + 13, y + 15]],
                ink,
            );
            for dx in [6, 11, 16] {
                fill(img, Rect(x + dx, y + 8, 2, 2), dark);
            }
        }
    }
}
fn button(
    img: &mut RgbaImage,
    font: &fontdue::Font,
    r: Rect,
    label: &str,
    icon: usize,
    active: bool,
) {
    plate(img, r, active);
    glyph(img, icon, r.0 + 8, r.1 + 8);
    let text_rect = Rect(r.0 + 40, r.1 + 4, r.2 - 42, r.3 - 8);
    let text_size = fitted_text_size(font, label, text_rect.2 as f32, 17.0);
    text(img, font, label, text_size, text_rect, true);
}
fn base_image(_assets: &mut Assets, font: &fontdue::Font) -> Result<RgbaImage> {
    static BASE: OnceLock<RgbaImage> = OnceLock::new();
    if let Some(base) = BASE.get() {
        return Ok(base.clone());
    }
    let mut img = RgbaImage::new(BASE_WIDTH, BASE_HEIGHT);
    plate(&mut img, Rect(190, 65, 1210, 162), false);
    plate(&mut img, Rect(190, 124, 685, 102), false);
    plate(&mut img, Rect(877, 123, 508, 103), false);
    plate(&mut img, Rect(218, 73, 840, 45), false);
    for (x, y, sx, sy) in [
        (197, 131, 1, 1),
        (868, 219, -1, -1),
        (880, 127, 1, 1),
        (1380, 222, -1, -1),
        (225, 66, 1, 1),
    ] {
        corner(&mut img, x, y, sx, sy);
    }
    for x in [350, 560, 768, 972] {
        diamond(&mut img, x, 68, 6, 3, false);
        line(&mut img, [x - 42, 66], [x - 8, 66], LIGHT);
    }
    for x in [210, 1354] {
        path(
            &mut img,
            &[
                [x - 26, 61],
                [x - 13, 66],
                [x, 86],
                [x + 15, 60],
                [x + 29, 55],
            ],
            GOLD,
        );
        path(
            &mut img,
            &[
                [x - 24, 65],
                [x - 12, 74],
                [x, 92],
                [x + 20, 65],
                [x + 29, 59],
            ],
            LIGHT,
        );
        diamond(&mut img, x, 76, 7, 11, false);
    }
    plate(&mut img, Rect(1395, 80, 299, 143), false);
    plate(&mut img, Rect(1404, 89, 281, 122), false);
    fill(&mut img, Rect(1406, 112, 269, 77), [9, 29, 42, 255]);
    for y in (116..188).step_by(8) {
        line(&mut img, [1410, y], [1670, y + 4], [25, 55, 67, 180]);
    }
    for (x, y) in [(1420, 122), (1658, 122), (1420, 177), (1658, 177)] {
        diamond(&mut img, x, y, 4, 7, true);
    }
    line(&mut img, [1435, 177], [1650, 177], [52, 166, 185, 180]);
    line(&mut img, [1480, 180], [1605, 180], [181, 139, 66, 150]);
    let brand = brand_font()?;
    text(
        &mut img,
        brand,
        "GRAVAK",
        40.0,
        Rect(1406, 121, 269, 54),
        true,
    );
    for (x, y, sx, sy) in [
        (1389, 83, 1, 1),
        (1690, 83, -1, 1),
        (1389, 220, 1, -1),
        (1690, 220, -1, -1),
    ] {
        corner(&mut img, x, y, sx, sy);
    }
    for y in [81, 216] {
        diamond(&mut img, 1537, y, 9, 15, false);
        diamond(&mut img, 1537, y, 4, 8, true);
    }
    for (x, y) in [(1690, 108), (1689, 195), (1423, 71), (972, 67)] {
        glow(&mut img, x, y, 26, x > 1600);
    }
    ellipse(&mut img, Rect(1065, 75, 42, 42), [13, 29, 40, 255]);
    ellipse(&mut img, Rect(1070, 79, 32, 32), GOLD);
    ellipse(&mut img, Rect(1073, 81, 26, 26), LIGHT);
    ellipse(&mut img, Rect(1078, 87, 4, 6), [11, 24, 32, 255]);
    ellipse(&mut img, Rect(1090, 87, 4, 6), [11, 24, 32, 255]);
    path(
        &mut img,
        &[
            [1078, 98],
            [1081, 101],
            [1086, 103],
            [1091, 101],
            [1094, 98],
        ],
        [11, 24, 32, 255],
    );
    orb_frame(&mut img);
    // Paint labels in the same layer as the decorative frames, never in source art.
    for i in 0..10 {
        button(&mut img, font, menu_rect(i), MENU_LABELS[i], i, false);
    }
    button(&mut img, font, TALK, "Talk", 11, false);
    button(&mut img, font, CLEAR, "Clear", 10, false);
    button(&mut img, font, CHAT_BUTTON, "Chat", 11, false);
    let _ = BASE.set(img.clone());
    Ok(img)
}

fn legacy_image(
    assets: &mut Assets,
    width: u32,
    health: u32,
    max_health: u32,
    mana: u32,
    max_mana: u32,
    level: u8,
    experience: u64,
    action_bar: &crate::action_bar::ActionBar,
    hover: Option<u8>,
    chat_text: &str,
    chat_focused: bool,
) -> Result<RgbaImage> {
    let font = font()?;
    let mut img = base_image(assets, font)?;
    orb_contents(&mut img, health, max_health, mana, max_mana);
    let next = u64::from(level.max(1)) * 100;
    let previous = u64::from(level.saturating_sub(1)) * 100;
    let fraction = if next > previous {
        ((experience.saturating_sub(previous)) as f32 / (next - previous) as f32).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let bar = Rect(186, 119, 680, 7);
    fill(&mut img, bar, [5, 15, 21, 255]);
    fill(
        &mut img,
        Rect(
            bar.0 + 1,
            bar.1 + 1,
            ((bar.2 - 2) as f32 * fraction) as i32,
            5,
        ),
        [235, 190, 52, 255],
    );
    let exp_label = format!("Level {level}  EXP {:.1}%", fraction * 100.0);
    text(
        &mut img,
        font,
        &exp_label,
        13.0,
        Rect(bar.0, 101, bar.2, 16),
        false,
    );
    for i in 0..10 {
        if hover == Some(i as u8) {
            button(&mut img, font, menu_rect(i), MENU_LABELS[i], i, true);
        }
    }
    for (r, label, id, icon) in [
        (TALK, "Talk", 13, 11),
        (CLEAR, "Clear", 11, 10),
        (CHAT_BUTTON, "Chat", 12, 11),
    ] {
        if hover == Some(id) {
            button(&mut img, font, r, label, icon, true);
        }
    }
    if chat_focused {
        chamfer(&mut img, CHAT, 3, GOLD);
    }
    let visible = if chat_focused {
        format!("{chat_text}|")
    } else {
        chat_text.to_owned()
    };
    // Keep the caret and most recent characters visible in long messages.
    let mut start = 0;
    let mut advance = 0.0;
    for (index, ch) in visible.char_indices().rev() {
        advance += font.metrics(ch, 18.0).advance_width;
        if advance > (CHAT.2 - 18) as f32 {
            start = index + ch.len_utf8();
            break;
        }
    }
    text(
        &mut img,
        font,
        &visible[start..],
        18.0,
        CHAT.inset(7),
        false,
    );
    for (index, slot) in action_bar.slots.iter().enumerate() {
        let r = key_rect(index);
        plate(&mut img, r, hover == Some(20 + index as u8));
        for (x, y, sx, sy) in [(r.0 + 2, r.1 + 2, 1, 1), (r.0 + r.2 - 3, r.1 + 2, -1, 1)] {
            corner(&mut img, x, y, sx, sy);
        }
        if let (Some(icon), Some(_)) = (&slot.icon, &slot.action_id) {
            let mut icon =
                imageops::resize(&assets.image(icon)?, 44, 44, imageops::FilterType::Lanczos3);
            if slot.disabled {
                for p in icon.pixels_mut() {
                    p.0[3] /= 2;
                }
            }
            imageops::overlay(&mut img, &icon, i64::from(r.0 + 9), i64::from(r.1 + 12));
            if let Some(seconds) = slot.cooldown.filter(|s| *s > 0) {
                fill(
                    &mut img,
                    Rect(r.0 + 5, r.1 + 5, r.2 - 10, 54),
                    [2, 6, 12, 175],
                );
                text(
                    &mut img,
                    font,
                    &seconds.to_string(),
                    21.0,
                    Rect(r.0, r.1 + 12, r.2, 40),
                    true,
                );
            }
        }
        line(
            &mut img,
            [r.0 + 8, r.1 + 63],
            [r.0 + r.2 - 9, r.1 + 63],
            [94, 76, 42, 255],
        );
        text(
            &mut img,
            font,
            &slot.hotkey,
            18.0,
            Rect(r.0, r.1 + 62, r.2, 21),
            true,
        );
    }
    if width == BASE_WIDTH {
        Ok(img)
    } else {
        Ok(imageops::resize(
            &img,
            width.max(1),
            (BASE_HEIGHT as f32 * width as f32 / BASE_WIDTH as f32)
                .round()
                .max(1.0) as u32,
            imageops::FilterType::Lanczos3,
        ))
    }
}

fn design_panel(img: &mut RgbaImage, rect: Rect) {
    // The reference HUD uses a translucent black/bronze plate with a double
    // frame. Keep this procedural so runtime text and bars never get baked
    // into an image asset.
    fill(img, rect, [8, 10, 12, 218]);
    chamfer(
        img,
        rect,
        (7.0 * (rect.2.min(rect.3) as f32 / 60.0)).round().max(3.0) as i32,
        [156, 116, 50, 255],
    );
    chamfer(img, rect.inset(2), 4, [74, 52, 26, 255]);
    chamfer(img, rect.inset(4), 3, [213, 164, 76, 190]);
    line(
        img,
        [rect.0 + 12, rect.1 + 3],
        [rect.0 + rect.2 - 13, rect.1 + 3],
        [255, 226, 151, 130],
    );
    line(
        img,
        [rect.0 + 12, rect.1 + rect.3 - 4],
        [rect.0 + rect.2 - 13, rect.1 + rect.3 - 4],
        [68, 45, 22, 255],
    );
    if rect.3 >= 58 {
        let corner_size = (rect.2.min(rect.3) / 7).clamp(8, 21);
        for (x, y, sx, sy) in [
            (rect.0 + 2, rect.1 + 2, 1, 1),
            (rect.0 + rect.2 - 2, rect.1 + 2, -1, 1),
            (rect.0 + 2, rect.1 + rect.3 - 2, 1, -1),
            (rect.0 + rect.2 - 2, rect.1 + rect.3 - 2, -1, -1),
        ] {
            corner_sized(img, x, y, sx, sy, corner_size);
        }
        let mid = rect.0 + rect.2 / 2;
        diamond(img, mid, rect.1 + 4, 4, 2, false);
        diamond(img, mid, rect.1 + rect.3 - 4, 4, 2, false);
    }
}
fn bundled_hud_asset(name: &str) -> Result<RgbaImage> {
    let bytes: &[u8] = match name {
        "player_frame" => include_bytes!("hud_assets/player_frame_trim.png"),
        "buff_bar" => include_bytes!("hud_assets/buff_bar_trim.png"),
        "minimap_frame" => include_bytes!("hud_assets/minimap_frame_trim.png"),
        "quest_tracker" => include_bytes!("hud_assets/quest_tracker_trim.png"),
        "chat_panel" => include_bytes!("hud_assets/chat_panel_trim.png"),
        "skillbar_frame" => include_bytes!("hud_assets/skillbar_frame_trim.png"),
        "quick_menu" => include_bytes!("hud_assets/quick_menu_trim.png"),
        "quick_label_spray" => include_bytes!("hud_assets/quick_label_spray.png"),
        _ => return Err(format!("unknown bundled HUD asset: {name}").into()),
    };
    Ok(image::load_from_memory(bytes)?.into_rgba8())
}
fn draw_client_panel(img: &mut RgbaImage, rect: Rect, ornate: bool) {
    let inner = rect.inset(4);
    // This is a real panel, not a glass overlay. A translucent base lets the
    // terrain bleed through and makes health bars and hotkeys unreadable.
    fill(img, inner, [7, 20, 29, 255]);
    chamfer(img, rect, 7, [224, 173, 73, 255]);
    chamfer(img, rect.inset(2), 5, [61, 39, 22, 255]);
    chamfer(img, rect.inset(4), 4, [109, 78, 34, 255]);
    line(
        img,
        [rect.0 + 12, rect.1 + 4],
        [rect.0 + rect.2 - 13, rect.1 + 4],
        [255, 226, 151, 160],
    );
    line(
        img,
        [rect.0 + 12, rect.1 + rect.3 - 4],
        [rect.0 + rect.2 - 13, rect.1 + rect.3 - 4],
        [54, 35, 21, 255],
    );
    if ornate {
        let corner_size = (rect.2.min(rect.3) / 7).clamp(8, 21);
        for (x, y, sx, sy) in [
            (rect.0 + 3, rect.1 + 3, 1, 1),
            (rect.0 + rect.2 - 3, rect.1 + 3, -1, 1),
            (rect.0 + 3, rect.1 + rect.3 - 3, 1, -1),
            (rect.0 + rect.2 - 3, rect.1 + rect.3 - 3, -1, -1),
        ] {
            corner_sized(img, x, y, sx, sy, corner_size);
        }
        diamond(img, rect.0 + rect.2 / 2, rect.1 + 4, 4, 2, false);
        diamond(img, rect.0 + rect.2 / 2, rect.1 + rect.3 - 4, 4, 2, false);
    }
}
fn draw_client_medallion(img: &mut RgbaImage, rect: Rect, active: bool) {
    let center = [rect.0 + rect.2 / 2, rect.1 + rect.3 / 2];
    let radius = (rect.2.min(rect.3) / 2 - 2).max(4);
    ellipse(
        img,
        Rect(
            center[0] - radius,
            center[1] - radius,
            radius * 2,
            radius * 2,
        ),
        if active {
            [133, 88, 32, 255]
        } else {
            [62, 42, 25, 255]
        },
    );
    ellipse(
        img,
        Rect(
            center[0] - radius + 3,
            center[1] - radius + 3,
            radius * 2 - 6,
            radius * 2 - 6,
        ),
        [10, 31, 42, 245],
    );
    chamfer(img, rect.inset(1), 6, if active { LIGHT } else { GOLD });
}
fn draw_bundled_hud_asset(img: &mut RgbaImage, name: &str, rect: Rect) -> Result<()> {
    let asset = bundled_hud_asset(name)?;
    let asset = imageops::resize(
        &asset,
        rect.2.max(1) as u32,
        rect.3.max(1) as u32,
        imageops::FilterType::Lanczos3,
    );
    imageops::overlay(img, &asset, i64::from(rect.0), i64::from(rect.1));
    Ok(())
}
fn draw_quick_menu_button(img: &mut RgbaImage, asset: &RgbaImage, rect: Rect, index: usize) {
    // quick_menu_trim.png contains ten 300px medallions in a 5x2 strip.
    // Cropping the medallion before resizing avoids the oval/distorted look
    // caused by stretching the complete strip into the compact HUD panel.
    const CENTER_X: [u32; 5] = [151, 447, 741, 1037, 1333];
    const TOP_Y: u32 = 0;
    const BOTTOM_Y: u32 = 318;
    let source_x = CENTER_X[index % 5].saturating_sub(150);
    let source_y = if index < 5 { TOP_Y } else { BOTTOM_Y };
    let sprite = imageops::crop_imm(asset, source_x, source_y, 300, 300).to_image();
    let sprite = imageops::resize(
        &sprite,
        rect.2.max(1) as u32,
        rect.3.max(1) as u32,
        imageops::FilterType::Lanczos3,
    );
    imageops::overlay(img, &sprite, i64::from(rect.0), i64::from(rect.1));
}
fn draw_minimap_frame(img: &mut RgbaImage, rect: Rect) {
    // The legacy minimap controller owns the map texture and its controls at
    // viewport-right, viewport-top. Keep this frame transparent and use the
    // controller's exact 170x128 rectangle so the artwork cannot drift away
    // from the live map.
    chamfer(img, rect, 5, [211, 160, 72, 255]);
    chamfer(img, rect.inset(2), 3, [77, 50, 24, 255]);
    line(
        img,
        [rect.0 + 8, rect.1 + 3],
        [rect.0 + rect.2 - 9, rect.1 + 3],
        [255, 225, 145, 170],
    );
    line(
        img,
        [rect.0 + 8, rect.1 + rect.3 - 3],
        [rect.0 + rect.2 - 9, rect.1 + rect.3 - 3],
        [77, 50, 24, 230],
    );
}
fn design_label(
    img: &mut RgbaImage,
    font: &fontdue::Font,
    value: &str,
    rect: Rect,
    size: f32,
    center: bool,
) {
    text(img, font, value, size, rect, center);
}
fn design_progress(img: &mut RgbaImage, rect: Rect, fraction: f32, color: [u8; 4]) {
    // Always paint the trough. Empty or zero-health states must still read as
    // a health/mana/stamina bar instead of disappearing into the background.
    fill(img, rect, [3, 9, 14, 255]);
    chamfer(img, rect, 2, [102, 77, 39, 255]);
    let inner = rect.inset(2);
    let width = (inner.2.max(0) as f32 * fraction.clamp(0.0, 1.0)) as i32;
    if width > 0 {
        fill(
            img,
            Rect(inner.0, inner.1, width.min(inner.2), inner.3.max(1)),
            color,
        );
        if width > 5 {
            fill(
                img,
                Rect(inner.0 + 1, inner.1 + 1, (width - 3).max(1), 2),
                [255, 238, 177, 90],
            );
        }
    }
}
fn design_circle_button(
    img: &mut RgbaImage,
    font: &fontdue::Font,
    rect: Rect,
    label: &str,
    icon: usize,
    active: bool,
) {
    // The frame and medallion are owned by the client theme. This function
    // only paints the runtime glyph, label and hover state.
    if active {
        ellipse(img, rect.inset(2), [255, 225, 145, 46]);
    }
    // Keep every pictogram optically centered inside the same 24px cell. A
    // few silhouettes have asymmetric bounds, so their small corrections are
    // explicit instead of letting each icon drift independently.
    let (glyph_dx, glyph_dy) = match icon {
        0 => (1, 2),     // character silhouette
        2 | 7 => (1, 2), // grouped figures
        4 => (0, 2),     // shield
        5 => (1, 1),     // friends star
        6 => (1, 0),     // mail arrows
        8 => (2, 2),     // market gear-like cluster
        _ => (0, 0),
    };
    glyph(
        img,
        icon,
        rect.0 + rect.2 / 2 - 12 + glyph_dx,
        rect.1 + rect.3 / 2 - 12 + glyph_dy,
    );
    let ui_scale = rect.2 as f32 / 60.0;
    let label_gap = (5.0 * ui_scale).round() as i32;
    let label_height = (13.5 * ui_scale).round().max(10.0) as i32;
    let label_top = rect.1 + rect.3 + label_gap;
    let label_box = Rect(rect.0 - 4, label_top, rect.2 + 8, label_height + 4);
    let label_size = fitted_text_size(
        font,
        label,
        (rect.2 - 4).max(1) as f32,
        (11.5 * ui_scale).max(8.0),
    );
    fill(img, label_box, [5, 15, 22, 205]);
    chamfer(img, label_box, 3, [85, 62, 31, 220]);
    design_label(
        img,
        font,
        label,
        // Center the label under the button with the reference's small gap.
        Rect(
            rect.0 - 4,
            label_top - (1.5 * ui_scale).round() as i32,
            rect.2 + 8,
            label_height,
        ),
        label_size,
        true,
    );
}
fn draw_design_skill_slot(
    img: &mut RgbaImage,
    assets: &mut Assets,
    font: &fontdue::Font,
    rect: Rect,
    slot: &crate::action_bar::ActionSlot,
    primary: &str,
    active: bool,
) -> Result<()> {
    // Skill slots are part of the client-owned HUD, so the frame is always
    // visible even when no skill is assigned.
    fill(img, rect, [6, 16, 24, 255]);
    chamfer(img, rect, 4, if active { LIGHT } else { GOLD });
    chamfer(img, rect.inset(2), 3, [38, 64, 73, 255]);
    if active {
        chamfer(img, rect.inset(5), 4, LIGHT);
    }
    if let (Some(icon), Some(_)) = (&slot.icon, &slot.action_id) {
        let icon_size = (rect.2 as f32 * 0.62).round().max(1.0) as u32;
        let mut icon = imageops::resize(
            &assets.image(icon)?,
            icon_size,
            icon_size,
            imageops::FilterType::Lanczos3,
        );
        if slot.disabled {
            for pixel in icon.pixels_mut() {
                pixel.0[3] /= 2;
            }
        }
        imageops::overlay(
            img,
            &icon,
            i64::from(rect.0 + (rect.2 - icon_size as i32) / 2),
            i64::from(rect.1 + 14),
        );
    }
    let action_name = slot
        .action_id
        .as_deref()
        .and_then(|action| action.strip_prefix("skill:"))
        .and_then(|id| id.parse::<u32>().ok())
        .and_then(crate::action_bar::skill_action)
        .map(|skill| skill.label)
        .unwrap_or(primary);
    design_label(
        img,
        font,
        action_name,
        Rect(rect.0 + 5, rect.1 + 3, rect.2 - 10, 18),
        13.0 * (rect.2 as f32 / 58.0),
        false,
    );
    design_label(
        img,
        font,
        &slot.hotkey,
        Rect(rect.0, rect.1 + rect.3 - 25, rect.2, 20),
        12.0 * (rect.2 as f32 / 58.0),
        true,
    );
    if let Some(seconds) = slot.cooldown.filter(|seconds| *seconds > 0) {
        fill(img, rect.inset(3), [2, 5, 8, 175]);
        design_label(
            img,
            font,
            &seconds.to_string(),
            rect.inset(4),
            18.0 * (rect.2 as f32 / 58.0),
            true,
        );
    }
    Ok(())
}
fn draw_action_context_menu(
    img: &mut RgbaImage,
    font: &fontdue::Font,
    metrics: UiMetrics,
    slot: usize,
    hover: Option<usize>,
) {
    let menu = design_action_context_menu(metrics, slot);
    fill(img, menu, [5, 12, 17, 246]);
    chamfer(img, menu, metrics.x(5.0).max(2), [212, 163, 74, 255]);
    chamfer(img, menu.inset(metrics.x(2.0).max(1)), 3, [75, 51, 27, 255]);
    let row_height = metrics.x(29.0).max(1);
    for (index, label) in [
        "Assign Spell",
        "Assign Object",
        "Assign Text",
        "Assign Hotkey",
    ]
    .into_iter()
    .enumerate()
    {
        let row = Rect(
            menu.0 + metrics.x(7.0),
            menu.1 + metrics.x(6.0) + row_height * index as i32,
            menu.2 - metrics.x(14.0),
            (row_height - metrics.x(2.0)).max(1),
        );
        if hover == Some(index) {
            fill(img, row, [109, 77, 34, 210]);
        }
        chamfer(
            img,
            row,
            metrics.x(2.0).max(1),
            if hover == Some(index) {
                LIGHT
            } else {
                [84, 63, 36, 255]
            },
        );
        design_label(
            img,
            font,
            label,
            Rect(
                row.0 + metrics.x(9.0),
                row.1,
                row.2 - metrics.x(15.0),
                row.3,
            ),
            13.0 * metrics.scale,
            false,
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
) -> Vec<(&'static str, [u8; 4], [u8; 4], f32)> {
    [
        (
            double_experience_active,
            "2X XP",
            [116, 71, 21, 245],
            [239, 190, 71, 255],
            8.0,
        ),
        (
            heaven_blessing_active,
            "BLESS",
            [73, 91, 133, 245],
            [164, 200, 255, 255],
            7.0,
        ),
        (
            burning_active,
            "BURN",
            [91, 115, 39, 245],
            [197, 219, 91, 255],
            7.0,
        ),
        (
            stunned_active,
            "STUN",
            [112, 49, 87, 245],
            [238, 151, 213, 255],
            7.0,
        ),
        (
            cyclone_active,
            "CYCL",
            [49, 108, 125, 245],
            [151, 224, 239, 255],
            6.0,
        ),
        (
            superman_active,
            "SUPER",
            [125, 76, 49, 245],
            [239, 174, 111, 255],
            6.0,
        ),
        (
            stigma_active,
            "STIG",
            [112, 84, 139, 245],
            [208, 172, 239, 255],
            7.0,
        ),
        (
            shield_active,
            "SHLD",
            [55, 105, 139, 245],
            [151, 208, 239, 255],
            6.0,
        ),
    ]
    .into_iter()
    .filter_map(|(active, label, fill, border, size)| active.then_some((label, fill, border, size)))
    .collect()
}

fn angle_in_arc(angle: f32, start: f32, end: f32) -> bool {
    let tau = std::f32::consts::TAU;
    let mut angle = angle.rem_euclid(tau);
    let start = start.rem_euclid(tau);
    let mut end = end.rem_euclid(tau);
    if end < start {
        end += tau;
    }
    if angle < start {
        angle += tau;
    }
    angle >= start && angle <= end
}

fn arc_band(
    image: &mut RgbaImage,
    center: [i32; 2],
    radius: i32,
    thickness: i32,
    start: f32,
    end: f32,
    fraction: f32,
    color: [u8; 4],
) {
    // Rasterize the band by pixels instead of sparse path points. This keeps
    // the arc solid and removes the white map pixels that showed through the
    // first dotted implementation.
    let min_x = center[0] - radius - 1;
    let max_x = center[0] + radius + 1;
    let min_y = center[1] - radius - 1;
    let max_y = center[1] + radius + 1;
    let inner_radius = (radius - thickness).max(1) as f32;
    let outer_radius = (radius + 1) as f32;
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let dx = (x - center[0]) as f32;
            let dy = (y - center[1]) as f32;
            let distance = (dx * dx + dy * dy).sqrt();
            if distance >= inner_radius
                && distance <= outer_radius
                && angle_in_arc(dy.atan2(dx), start, end)
            {
                pixel(image, x, y, [9, 15, 18, 255]);
            }
        }
    }
    let filled_end = start + (end - start) * fraction.clamp(0.0, 1.0);
    if filled_end <= start + 0.001 {
        return;
    }
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let dx = (x - center[0]) as f32;
            let dy = (y - center[1]) as f32;
            let distance = (dx * dx + dy * dy).sqrt();
            if distance >= inner_radius
                && distance <= outer_radius
                && angle_in_arc(dy.atan2(dx), start, filled_end)
            {
                pixel(image, x, y, color);
            }
        }
    }
}

fn center_status_overlay(
    image: &mut RgbaImage,
    font: &fontdue::Font,
    width: u32,
    height: u32,
    health: u32,
    max_health: u32,
    mana: u32,
    max_mana: u32,
    config: HudConfig,
) {
    if !config.center_arcs && !config.center_bars {
        return;
    }
    let scale = f32::from(config.center_scale.clamp(75, 140)) / 100.0;
    let center = [width as i32 / 2, height as i32 / 2 - (18.0 * scale) as i32];
    let radius = (f32::from(config.center_arc_distance.clamp(80, 180)) * scale).round() as i32;
    let thickness = (25.0 * scale).round() as i32;
    let health_fraction = health as f32 / max_health.max(1) as f32;
    let mana_fraction = mana as f32 / max_mana.max(1) as f32;

    if config.center_arcs {
        let pi = std::f32::consts::PI;
        arc_band(
            image,
            center,
            radius,
            thickness,
            0.65 * pi,
            1.35 * pi,
            health_fraction,
            [54, 190, 74, 255],
        );
        arc_band(
            image,
            center,
            radius,
            thickness,
            -0.35 * pi,
            0.35 * pi,
            mana_fraction,
            [57, 106, 224, 255],
        );
    }

    if config.center_bars {
        let bar_width = (360.0 * scale).round() as i32;
        let bar_height = (16.0 * scale).round().max(11.0) as i32;
        let x = center[0] - bar_width / 2;
        // The bars belong just above the action bar, not directly below the
        // arcs in the middle of the play field.
        let y = height as i32 - (170.0 * scale).round() as i32;
        for (index, (value, maximum, color)) in [
            (health, max_health, [54, 190, 74, 255]),
            (mana, max_mana, [57, 106, 224, 255]),
        ]
        .into_iter()
        .enumerate()
        {
            let trough = Rect(
                x,
                y + index as i32 * (bar_height + 6),
                bar_width,
                bar_height,
            );
            fill(image, trough, [5, 10, 14, 220]);
            chamfer(image, trough, 2, [109, 84, 43, 235]);
            let inner = trough.inset(2);
            let filled = (inner.2 as f32 * value as f32 / maximum.max(1) as f32) as i32;
            if filled > 0 {
                fill(image, Rect(inner.0, inner.1, filled, inner.3), color);
            }
            design_label(
                image,
                font,
                &format!("{} {value}/{maximum}", if index == 0 { "HP" } else { "MP" }),
                trough.inset(2),
                (12.0 * scale).max(10.0),
                true,
            );
        }
    }
}

pub fn image(
    assets: &mut Assets,
    width: u32,
    height: u32,
    character_name: &str,
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
) -> Result<RgbaImage> {
    let font = font()?;
    let metrics = UiMetrics::new([width as f32, height as f32]);
    let mut img = RgbaImage::new(width.max(1), height.max(1));

    // Player status — anchored TOP_LEFT. Keep the live values inside the
    // logical panel bounds; decorative HUD assets can extend beyond their
    // source rectangle at compact resolutions.
    let player = design_player_rect(metrics);
    draw_client_panel(&mut img, player, true);
    let portrait = Rect(
        player.0 + metrics.x(14.0),
        player.1 + metrics.x(19.0),
        metrics.x(104.0),
        metrics.x(104.0),
    );
    ellipse(&mut img, portrait, [12, 29, 37, 255]);
    ellipse(
        &mut img,
        portrait.inset(metrics.x(5.0)),
        [191, 147, 64, 255],
    );
    ellipse(&mut img, portrait.inset(metrics.x(9.0)), [32, 58, 66, 255]);
    let initial = character_name.chars().next().unwrap_or('?').to_string();
    design_label(
        &mut img,
        font,
        &initial,
        portrait,
        42.0 * metrics.scale,
        true,
    );
    design_label(
        &mut img,
        font,
        character_name,
        Rect(
            player.0 + metrics.x(132.0),
            player.1 + metrics.x(14.0),
            metrics.x(310.0),
            metrics.x(25.0),
        ),
        20.0 * metrics.scale,
        false,
    );
    design_label(
        &mut img,
        font,
        &format!("Lv. {level}"),
        Rect(
            player.0 + metrics.x(132.0),
            player.1 + metrics.x(38.0),
            metrics.x(80.0),
            metrics.x(22.0),
        ),
        16.0 * metrics.scale,
        false,
    );
    // The artwork reserves a little more room between the portrait and the
    // live bars than the first pass did. Keep the name/level left aligned but
    // start the status rows on the actual troughs in the frame.
    // Keep a dedicated value column inside the panel. Shorter troughs prevent
    // current/maximum values from colliding with the right frame at compact
    // resolutions.
    let bar_x = player.0 + metrics.x(132.0);
    // Keep a dedicated value column on the right. The previous 35px column
    // clipped values such as 318/318 at compact resolutions.
    let bar_w = metrics.x(310.0);
    for (index, (label, current, maximum, color)) in [
        ("HP", health, max_health, [221, 32, 37, 255]),
        ("MP", mana, max_mana, [38, 113, 231, 255]),
        ("STA", 100_u32, 100_u32, [33, 175, 112, 255]),
    ]
    .into_iter()
    .enumerate()
    {
        let row = player.1 + metrics.x(64.0 + index as f32 * 20.0);
        design_label(
            &mut img,
            font,
            label,
            Rect(bar_x, row, metrics.x(36.0), metrics.x(18.0)),
            12.0 * metrics.scale,
            false,
        );
        design_progress(
            &mut img,
            Rect(
                bar_x + metrics.x(38.0),
                row + metrics.x(2.0),
                bar_w - metrics.x(114.0),
                metrics.x(8.0),
            ),
            current as f32 / maximum.max(1) as f32,
            color,
        );
        design_label(
            &mut img,
            font,
            &format!("{current}/{maximum}"),
            Rect(
                bar_x + bar_w - metrics.x(60.0),
                row,
                metrics.x(60.0),
                metrics.x(18.0),
            ),
            10.0 * metrics.scale,
            true,
        );
    }
    let previous = u64::from(level.saturating_sub(1)) * 100;
    let next = u64::from(level.max(1)) * 100;
    let exp = if next > previous {
        ((experience.saturating_sub(previous)) as f32 / (next - previous) as f32).clamp(0.0, 1.0)
    } else {
        0.0
    };
    design_label(
        &mut img,
        font,
        "EXP",
        Rect(
            bar_x,
            player.1 + metrics.x(126.0),
            metrics.x(36.0),
            metrics.x(18.0),
        ),
        12.0 * metrics.scale,
        false,
    );
    design_progress(
        &mut img,
        Rect(
            bar_x + metrics.x(38.0),
            player.1 + metrics.x(128.0),
            bar_w - metrics.x(114.0),
            metrics.x(8.0),
        ),
        exp,
        [237, 189, 54, 255],
    );
    design_label(
        &mut img,
        font,
        &format!("{:.1}%", exp * 100.0),
        Rect(
            bar_x + bar_w - metrics.x(60.0),
            player.1 + metrics.x(126.0),
            metrics.x(60.0),
            metrics.x(18.0),
        ),
        10.0 * metrics.scale,
        true,
    );

    // Buff bar and quest tracker are intentionally empty until their runtime
    // data exists; the reusable frames are still present and anchored.
    let buff = design_buff_rect(metrics);
    draw_client_panel(&mut img, buff, false);
    design_label(
        &mut img,
        font,
        "BUFFS",
        Rect(buff.0 + metrics.x(12.0), buff.1, metrics.x(60.0), buff.3),
        13.0 * metrics.scale,
        false,
    );
    for (index, (label, fill_color, border_color, text_size)) in active_buff_badges(
        double_experience_active,
        heaven_blessing_active,
        burning_active,
        stunned_active,
        cyclone_active,
        superman_active,
        stigma_active,
        shield_active,
    )
    .into_iter()
    .take(5)
    .enumerate()
    {
        let slot = Rect(
            buff.0 + metrics.x(78.0 + index as f32 * 62.0),
            buff.1 + metrics.x(8.0),
            metrics.x(28.0),
            metrics.x(28.0),
        );
        chamfer(&mut img, slot, 3, [97, 77, 43, 255]);
        let active_slot = slot.inset(metrics.x(2.0));
        fill(&mut img, active_slot, fill_color);
        chamfer(&mut img, active_slot, 2, border_color);
        design_label(&mut img, font, label, slot, text_size * metrics.scale, true);
    }

    let quest = design_quest_rect(metrics);
    draw_client_panel(&mut img, quest, true);
    let quest_header = Rect(
        quest.0 + metrics.x(5.0),
        quest.1 + metrics.x(5.0),
        quest.2 - metrics.x(10.0),
        metrics.x(32.0),
    );
    fill(&mut img, quest_header, [66, 20, 16, 220]);
    chamfer(&mut img, quest_header, 4, [172, 116, 44, 255]);
    line(
        &mut img,
        [
            quest_header.0 + metrics.x(7.0),
            quest_header.1 + quest_header.3 - 3,
        ],
        [
            quest_header.0 + quest_header.2 - metrics.x(7.0),
            quest_header.1 + quest_header.3 - 3,
        ],
        [235, 184, 82, 190],
    );
    design_label(
        &mut img,
        font,
        "QUEST TRACKER",
        Rect(
            quest.0 + metrics.x(14.0),
            quest.1 + metrics.x(10.0),
            quest.2 - metrics.x(28.0),
            metrics.x(24.0),
        ),
        16.0 * metrics.scale,
        false,
    );
    for (index, label) in ["Main", "Daily", "Side", "Guild", "Event"]
        .into_iter()
        .enumerate()
    {
        let tab = Rect(
            quest.0 + metrics.x(12.0 + index as f32 * 57.0),
            quest.1 + metrics.x(42.0),
            metrics.x(52.0),
            metrics.x(23.0),
        );
        chamfer(
            &mut img,
            tab,
            3,
            if index == 0 { LIGHT } else { [94, 74, 42, 255] },
        );
        design_label(&mut img, font, label, tab, 10.0 * metrics.scale, true);
    }
    design_label(
        &mut img,
        font,
        "Nenhuma missão ativa",
        Rect(
            quest.0 + metrics.x(16.0),
            quest.1 + metrics.x(87.0),
            quest.2 - metrics.x(32.0),
            metrics.x(28.0),
        ),
        13.0 * metrics.scale,
        false,
    );

    // Chat — anchored BOTTOM_LEFT.
    let chat = design_chat_rect(metrics);
    draw_client_panel(&mut img, chat, true);
    let chat_channels = [0, 2021, 2000, 2004, 2003, 2007];
    for (index, label) in ["All", "World", "Nearby", "Guild", "Team", "System"]
        .into_iter()
        .enumerate()
    {
        let tab = Rect(
            chat.0 + metrics.x(12.0 + index as f32 * 70.0),
            chat.1 + metrics.x(10.0),
            metrics.x(63.0),
            metrics.x(24.0),
        );
        chamfer(
            &mut img,
            tab,
            3,
            if chat_channels[index] == chat_filter {
                LIGHT
            } else {
                [88, 68, 39, 255]
            },
        );
        design_label(&mut img, font, label, tab, 11.0 * metrics.scale, true);
    }
    let mut visible_messages = chat_history
        .iter()
        .filter(|message| chat_filter == 0 || message.channel == chat_filter)
        .rev()
        .take(5)
        .collect::<Vec<_>>();
    visible_messages.reverse();
    for (index, message) in visible_messages.into_iter().enumerate() {
        let prefix = if message.channel == 2001 {
            format!("[W] {}: ", message.sender_name)
        } else if message.channel == 2007 {
            "[SYS] ".to_owned()
        } else {
            format!("{}: ", message.sender_name)
        };
        let mut line = format!("{prefix}{}", message.message);
        let max_chars = 58;
        if line.chars().count() > max_chars {
            line = line.chars().take(max_chars - 1).collect();
            line.push('…');
        }
        let row = Rect(
            chat.0 + metrics.x(16.0),
            chat.1 + metrics.x(39.0 + index as f32 * 20.0),
            chat.2 - metrics.x(32.0),
            metrics.x(19.0),
        );
        design_label(&mut img, font, &line, row, 12.0 * metrics.scale, false);
    }
    let input = Rect(
        chat.0 + metrics.x(12.0),
        chat.1 + chat.3 - metrics.x(44.0),
        chat.2 - metrics.x(24.0),
        metrics.x(32.0),
    );
    fill(&mut img, input, [5, 11, 16, 245]);
    chamfer(
        &mut img,
        input,
        3,
        if chat_focused {
            LIGHT
        } else {
            [94, 74, 42, 255]
        },
    );
    let visible = if chat_focused {
        format!("{chat_text}|")
    } else {
        chat_text.to_owned()
    };
    design_label(
        &mut img,
        font,
        if visible.is_empty() {
            "Type a message..."
        } else {
            &visible
        },
        input.inset(7),
        14.0 * metrics.scale,
        false,
    );
    let clear = Rect(
        chat.0 + chat.2 - metrics.x(88.0),
        chat.1 + metrics.x(10.0),
        metrics.x(76.0),
        metrics.x(24.0),
    );
    design_label(&mut img, font, "Clear", clear, 11.0 * metrics.scale, true);

    // Skill bar — anchored BOTTOM_CENTER. The existing ActionBar remains the
    // source of truth; this layer only changes presentation.
    let skill = design_skill_rect(metrics);
    draw_client_panel(&mut img, skill, true);
    for (index, slot) in action_bar.slots.iter().enumerate() {
        draw_design_skill_slot(
            &mut img,
            assets,
            font,
            design_skill_slot(metrics, index),
            slot,
            &((index + 1) % 10).to_string(),
            hover == Some(20 + index as u8),
        )?;
    }
    if let Some(slot) = action_context_slot {
        draw_action_context_menu(&mut img, font, metrics, slot, action_context_hover);
    }

    // Quick menu — anchored BOTTOM_RIGHT.
    for (index, label) in [
        "Character",
        "Inventory",
        "Skills",
        "Quest",
        "Guild",
        "Friends",
        "Mail",
        "Market",
        "System",
        "Settings",
    ]
    .into_iter()
    .enumerate()
    {
        let button = design_menu_rect(metrics, index);
        draw_client_medallion(&mut img, button, hover == Some(index as u8));
        design_circle_button(
            &mut img,
            font,
            button,
            label,
            index,
            hover == Some(index as u8),
        );
    }

    center_status_overlay(
        &mut img, font, width, height, health, max_health, mana, max_mana, hud_config,
    );

    // The minimap is rendered by its own controller. This border is only a
    // visual frame and does not paint over the map texture.
    // Minimap::quads uses a physical 170x128 viewport rectangle, independent
    // of the HUD scale. Keep the frame in that same coordinate space or it
    // drifts inward on 1366px/compact windows.
    let mini = Rect(width as i32 - 170, 0, 170, 128);
    draw_minimap_frame(&mut img, mini);
    Ok(img)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scaled_controls_match_visible_layout() {
        for viewport in [
            [1366.0, 768.0],
            [2560.0, 1440.0],
            [2560.0, 1080.0],
            [3840.0, 2160.0],
        ] {
            let metrics = UiMetrics::new(viewport);
            for i in 0..10 {
                let r = design_menu_rect(metrics, i);
                let p = [(r.0 + r.2 / 2) as f32, (r.1 + r.3 / 2) as f32];
                assert_eq!(hovered_button(viewport, p), Some(i as u8));
                assert_eq!(items_button_at(viewport, p), i == 1);
                assert!(contains(viewport, p));
                let slot = design_skill_slot(metrics, i);
                let slot_center = [(slot.0 + slot.2 / 2) as f32, (slot.1 + slot.3 / 2) as f32];
                assert_eq!(hovered_button(viewport, slot_center), Some(20 + i as u8));
            }
            let chat = design_chat_rect(metrics);
            assert!(chat_input_at(
                viewport,
                [(chat.0 + 40) as f32, (chat.1 + chat.3 - 20) as f32]
            ));
            assert!(chat_clear_at(
                viewport,
                [(chat.0 + chat.2 - 50) as f32, (chat.1 + 20) as f32]
            ));
            assert!(chat_button_at(
                viewport,
                [(chat.0 + 20) as f32, (chat.1 + 20) as f32]
            ));
            assert!(talk_channel_at(
                viewport,
                [(chat.0 + 25) as f32, (chat.1 + 20) as f32]
            ));
            for (index, channel) in [0, 2021, 2000, 2004, 2003, 2007].into_iter().enumerate() {
                let center = [
                    (chat.0 + metrics.x(12.0 + index as f32 * 70.0 + 31.5)) as f32,
                    (chat.1 + metrics.x(22.0)) as f32,
                ];
                assert_eq!(chat_tab_at(viewport, center), Some(channel));
            }
            assert!(!contains(viewport, [viewport[0] * 0.5, viewport[1] * 0.5]));
            assert!(!contains(viewport, [-1.0, viewport[1] - 1.0]));
            assert!(!contains(viewport, [50.0, viewport[1] + 1.0]));
        }
    }
    #[test]
    fn resources_fill_independently_and_handle_zero_maximum() {
        let mut full = RgbaImage::new(BASE_WIDTH, BASE_HEIGHT);
        orb_contents(&mut full, 100, 100, 100, 100);
        let mut partial = RgbaImage::new(BASE_WIDTH, BASE_HEIGHT);
        orb_contents(&mut partial, 25, 100, 100, 100);
        assert_ne!(full.get_pixel(65, 100), partial.get_pixel(65, 100));
        assert_eq!(full.get_pixel(125, 100), partial.get_pixel(125, 100));
        assert_eq!(full.get_pixel(65, 180), partial.get_pixel(65, 180));
        orb_contents(&mut partial, 0, 0, 0, 0);
        assert_eq!(partial.get_pixel(96, 132).0[3], 0);
    }

    #[test]
    fn combat_buffs_fill_slots_without_overwriting_active_states() {
        let none = active_buff_badges(false, false, false, false, false, false, false, false);
        assert!(none.is_empty());
        let all = active_buff_badges(true, true, true, true, true, true, true, true);
        assert_eq!(all.len(), 8);
        assert_eq!(all[0].0, "2X XP");
        assert_eq!(all[2].0, "BURN");
        assert_eq!(all[4].0, "CYCL");
        assert_eq!(all[7].0, "SHLD");
        assert_eq!(all.iter().take(5).count(), 5);
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
            crate::network::ChatMessage {
                channel: 2007,
                sender_name: "SYSTEM".into(),
                recipient_name: "ALLUSERS".into(),
                message: "Item equipado com sucesso".into(),
            },
        ];
        for (name, width, hp, mp, hover, chat) in [
            ("hud-reference.png", 1704, 100, 100, None, ""),
            ("hud-1366.png", 1366, 65, 40, Some(1), "Olá, Twin City!"),
            (
                "hud-1024.png",
                1024,
                25,
                0,
                Some(25),
                "Uma mensagem longa para verificar o campo de conversa e o cursor no final do texto.",
            ),
        ] {
            image(
                &mut assets,
                width,
                (768.0 * width as f32 / 1366.0).round() as u32,
                "CloudStrider",
                hp,
                100,
                mp,
                100,
                2,
                125,
                &Default::default(),
                hover,
                None,
                None,
                chat,
                !chat.is_empty(),
                &chat_history,
                0,
                false,
                false,
                false,
                false,
                false,
                false,
                false,
                false,
                HudConfig::default(),
            )?
            .save(output.join(name))?;
        }
        image(
            &mut assets,
            1366,
            768,
            "CloudStrider",
            65,
            100,
            40,
            100,
            24,
            2_343,
            &Default::default(),
            None,
            None,
            None,
            "",
            false,
            &chat_history,
            0,
            true,
            true,
            true,
            true,
            true,
            true,
            true,
            true,
            HudConfig::default(),
        )?
        .save(output.join("hud-combat.png"))?;
        Ok(())
    }
}
