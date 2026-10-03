//! The feature icons along the top right of the screen, left of the
//! minimap (Bounty, Mentor Reward, Item Lock, VIP, Shopping Mall, Bug
//! Report): a round medallion with its symbol over a two-line caption, like
//! the original client's event icons. The client draws them; layout,
//! painting and hit testing share [`icon_rect`].

use crate::launcher_art::{Point, capsule, circle, mix, polygon, shape};
use image::{RgbaImage, imageops};
use std::{
    cell::RefCell,
    collections::HashMap,
    sync::{OnceLock, RwLock},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FeatureIcon {
    Bounty,
    MentorReward,
    ItemLock,
    Vip,
    ShoppingMall,
    BugReport,
}

impl FeatureIcon {
    pub const ALL: [Self; 6] = [
        Self::Bounty,
        Self::MentorReward,
        Self::ItemLock,
        Self::Vip,
        Self::ShoppingMall,
        Self::BugReport,
    ];

    fn caption(self) -> &'static [&'static str] {
        match self {
            Self::Bounty => &["Bounty"],
            Self::MentorReward => &["Mentor", "Reward"],
            Self::ItemLock => &["Item", "Lock"],
            Self::Vip => &["VIP"],
            Self::ShoppingMall => &["Shopping", "Mall"],
            Self::BugReport => &["Bug", "Report"],
        }
    }

    /// The medallion's colour, top and bottom.
    fn disc(self) -> ([f32; 3], [f32; 3]) {
        match self {
            Self::Bounty => ([196.0, 52.0, 36.0], [70.0, 10.0, 8.0]),
            Self::MentorReward => ([96.0, 170.0, 70.0], [24.0, 66.0, 22.0]),
            Self::ItemLock => ([226.0, 140.0, 48.0], [110.0, 50.0, 10.0]),
            Self::Vip => ([150.0, 70.0, 196.0], [52.0, 16.0, 86.0]),
            Self::ShoppingMall => ([70.0, 150.0, 226.0], [16.0, 52.0, 120.0]),
            Self::BugReport => ([60.0, 176.0, 196.0], [12.0, 70.0, 96.0]),
        }
    }

    fn caption_colour(self) -> [u8; 4] {
        match self {
            Self::Bounty => [255, 128, 72, 255],
            _ => [255, 222, 96, 255],
        }
    }
}

/// Medallion size, and the width each icon takes in the row.
const ICON: i32 = 40;
const CELL: i32 = 52;
/// The minimap's corner box is 170 px wide.
const MINIMAP_WIDTH: i32 = 170;
const TOP: i32 = 6;
const CAPTION_LINE: i32 = 13;

/// The icon's medallion and caption, as one clickable box.
pub fn icon_rect(viewport: [f32; 2], index: usize) -> [i32; 4] {
    let right = viewport[0] as i32 - MINIMAP_WIDTH - 10;
    let left = right - CELL * FeatureIcon::ALL.len() as i32;
    [
        left + index as i32 * CELL,
        TOP,
        CELL,
        ICON + 2 + CAPTION_LINE * 2 + 2,
    ]
}

pub fn icon_at(viewport: [f32; 2], cursor: [f32; 2]) -> Option<usize> {
    let [x, y] = [cursor[0] as i32, cursor[1] as i32];
    (0..FeatureIcon::ALL.len()).find(|&index| {
        let [left, top, width, height] = icon_rect(viewport, index);
        x >= left && x < left + width && y >= top && y < top + height
    })
}

/// The installation's own art for an icon (or the Revive plate), set from
/// `settings/hud_icons.ini`. Shared across threads: the HUD is painted on a
/// worker.
static ART: OnceLock<RwLock<HashMap<String, RgbaImage>>> = OnceLock::new();

fn art_table() -> &'static RwLock<HashMap<String, RgbaImage>> {
    ART.get_or_init(|| RwLock::new(HashMap::new()))
}

/// Uses `image` for the art named `key` (`bounty`, `mentor`, `lock`, `vip`,
/// `mall`, `bug` or `revive`).
pub fn set_art(key: &str, image: RgbaImage) {
    if let Ok(mut table) = art_table().write() {
        table.insert(key.to_ascii_lowercase(), image);
    }
}

pub fn art(key: &str) -> Option<RgbaImage> {
    art_table().read().ok()?.get(key).cloned()
}

impl FeatureIcon {
    pub fn art_key(self) -> &'static str {
        match self {
            Self::Bounty => "bounty",
            Self::MentorReward => "mentor",
            Self::ItemLock => "lock",
            Self::Vip => "vip",
            Self::ShoppingMall => "mall",
            Self::BugReport => "bug",
        }
    }
}

thread_local! {
    static MEDALLIONS: RefCell<HashMap<(FeatureIcon, bool), RgbaImage>> = RefCell::new(HashMap::new());
}

/// Paints the row; `hovered` is lit.
pub fn draw(image: &mut RgbaImage, viewport: [f32; 2], hovered: Option<usize>) {
    for (index, icon) in FeatureIcon::ALL.into_iter().enumerate() {
        let lit = hovered == Some(index);
        let [x, y, width, height] = icon_rect(viewport, index);
        // The original icon carries its own caption: fit it in the box.
        if let Some(original) = art(icon.art_key()) {
            let scale = (width as f32 / original.width() as f32)
                .min(height as f32 / original.height() as f32);
            let (w, h) = (
                ((original.width() as f32 * scale) as u32).max(1),
                ((original.height() as f32 * scale) as u32).max(1),
            );
            let mut fitted = imageops::resize(&original, w, h, imageops::FilterType::Triangle);
            if lit {
                for pixel in fitted.pixels_mut() {
                    for channel in 0..3 {
                        pixel[channel] = (u16::from(pixel[channel]) * 5 / 4).min(255) as u8;
                    }
                }
            }
            imageops::overlay(
                image,
                &fitted,
                i64::from(x + (width - w as i32) / 2),
                i64::from(y),
            );
            continue;
        }
        let medallion = MEDALLIONS.with(|cache| {
            cache
                .borrow_mut()
                .entry((icon, lit))
                .or_insert_with(|| medallion(icon, lit))
                .clone()
        });
        let left = x + (width - ICON) / 2;
        imageops::overlay(image, &medallion, i64::from(left), i64::from(y));
        let Ok(font) = crate::main_hud::label_font() else {
            continue;
        };
        let colour = if lit {
            [255, 255, 255, 255]
        } else {
            icon.caption_colour()
        };
        for (line_index, line) in icon.caption().iter().enumerate() {
            let line_width = crate::main_hud::text_width(font, line, 12.0);
            crate::main_hud::text(
                image,
                font,
                line,
                12.0,
                [
                    x + (width - line_width) / 2,
                    y + ICON + 1 + line_index as i32 * CAPTION_LINE,
                ],
                colour,
                None,
            );
        }
    }
}

fn medallion(icon: FeatureIcon, lit: bool) -> RgbaImage {
    let size = ICON as u32;
    let mut image = RgbaImage::new(size, size);
    let half = size as f32 / 2.0;
    let unit =
        move |x: f32, y: f32| -> Point { ((x - half) / (half - 1.0), (y - half) / (half - 1.0)) };
    let rect = [0, 0, ICON, ICON];
    let (top, bottom) = icon.disc();
    let boost = if lit { 1.25 } else { 1.0 };
    shape(
        &mut image,
        rect,
        |_, y| {
            let c = mix(top, bottom, y / size as f32);
            let up = |v: u8| (f32::from(v) * boost).min(255.0) as u8;
            [up(c[0]), up(c[1]), up(c[2]), 255]
        },
        |x, y| circle(unit(x, y), (0.0, 0.0), 1.0),
    );
    // Shine on the upper half.
    shape(
        &mut image,
        rect,
        |_, _| [255, 255, 255, 46],
        |x, y| {
            let p = unit(x, y);
            circle(p, (0.0, -0.32), 0.62) && circle(p, (0.0, 0.0), 0.86)
        },
    );
    // Gold rim.
    shape(
        &mut image,
        rect,
        |_, y| {
            let c = mix([255.0, 240.0, 190.0], [176.0, 120.0, 40.0], y / size as f32);
            [c[0], c[1], c[2], 255]
        },
        |x, y| {
            let p = unit(x, y);
            let d = (p.0 * p.0 + p.1 * p.1).sqrt();
            (0.86..=1.0).contains(&d)
        },
    );
    let scale = 0.72;
    let symbol_at = move |x: f32, y: f32, dx: f32, dy: f32| {
        let p = unit(x, y);
        symbol(icon, ((p.0 - dx) / scale, (p.1 - dy) / scale))
    };
    shape(
        &mut image,
        rect,
        |_, _| [20, 8, 4, 170],
        |x, y| symbol_at(x, y, 0.05, 0.06).is_some(),
    );
    shape(
        &mut image,
        rect,
        |x, y| symbol_at(x, y, 0.0, 0.0).unwrap_or([0, 0, 0, 0]),
        |x, y| symbol_at(x, y, 0.0, 0.0).is_some(),
    );
    image
}

const IVORY: [u8; 4] = [252, 244, 222, 255];
const GOLD: [u8; 4] = [255, 210, 90, 255];
const BONE: [u8; 4] = [226, 210, 176, 255];
const DARK: [u8; 4] = [40, 26, 18, 255];

/// The symbol's colour at `p` (unit disc, y down), if `p` is on it.
fn symbol(icon: FeatureIcon, p: Point) -> Option<[u8; 4]> {
    match icon {
        FeatureIcon::Bounty => {
            // A skull over crossed bones, with a gold coin.
            if circle(p, (0.5, 0.52), 0.24) {
                return Some(if circle(p, (0.5, 0.52), 0.16) {
                    GOLD
                } else {
                    [190, 130, 40, 255]
                });
            }
            let head = circle(p, (0.0, -0.16), 0.46);
            let jaw = polygon(
                p,
                &[(-0.26, 0.12), (0.26, 0.12), (0.22, 0.44), (-0.22, 0.44)],
            );
            if head || jaw {
                let eye = circle(p, (-0.17, -0.12), 0.12) || circle(p, (0.17, -0.12), 0.12);
                let nose = polygon(p, &[(0.0, 0.02), (-0.06, 0.14), (0.06, 0.14)]);
                let teeth = jaw && p.1 > 0.26 && ((p.0 * 10.0).rem_euclid(2.0) < 0.35);
                return Some(if eye || nose || teeth { DARK } else { IVORY });
            }
            let bones = capsule(p, (-0.62, 0.62), (0.62, -0.02), 0.08)
                || capsule(p, (-0.62, -0.02), (0.62, 0.62), 0.08)
                || [(-0.62, 0.62), (0.62, -0.02), (-0.62, -0.02), (0.62, 0.62)]
                    .iter()
                    .any(|&end| circle(p, end, 0.12));
            bones.then_some(BONE)
        }
        FeatureIcon::MentorReward => {
            // A scroll with a gold seal.
            if circle(p, (0.22, 0.3), 0.18) {
                return Some([200, 40, 30, 255]);
            }
            let sheet = polygon(
                p,
                &[(-0.42, -0.42), (0.42, -0.42), (0.42, 0.42), (-0.42, 0.42)],
            );
            let rolls = capsule(p, (-0.52, -0.46), (0.52, -0.46), 0.1)
                || capsule(p, (-0.52, 0.46), (0.52, 0.46), 0.1);
            if rolls {
                return Some(BONE);
            }
            if sheet {
                let line = [-0.2, -0.02, 0.16].iter().any(|y| (p.1 - y).abs() < 0.035)
                    && p.0 > -0.3
                    && p.0 < 0.1;
                return Some(if line { DARK } else { IVORY });
            }
            None
        }
        FeatureIcon::ItemLock => {
            // A padlock.
            let shackle = {
                let d = ((p.0).powi(2) + (p.1 + 0.12).powi(2)).sqrt();
                (0.24..=0.38).contains(&d) && p.1 < -0.08
            } || (p.1 >= -0.12 && p.1 < 0.05 && ((p.0.abs() - 0.31).abs() < 0.07));
            if shackle {
                return Some([210, 214, 220, 255]);
            }
            let body = polygon(p, &[(-0.46, 0.0), (0.46, 0.0), (0.46, 0.58), (-0.46, 0.58)]);
            if body {
                let keyhole = circle(p, (0.0, 0.22), 0.09)
                    || polygon(
                        p,
                        &[(-0.04, 0.24), (0.04, 0.24), (0.07, 0.44), (-0.07, 0.44)],
                    );
                return Some(if keyhole { DARK } else { GOLD });
            }
            None
        }
        FeatureIcon::Vip => {
            // A crown with jewels.
            for (centre, colour) in [
                ((-0.56, -0.3), [90, 200, 255, 255]),
                ((0.0, -0.52), [255, 70, 90, 255]),
                ((0.56, -0.3), [90, 200, 255, 255]),
            ] {
                if circle(p, centre, 0.11) {
                    return Some(colour);
                }
            }
            let crown = polygon(
                p,
                &[
                    (-0.6, 0.36),
                    (-0.6, -0.24),
                    (-0.28, 0.06),
                    (0.0, -0.46),
                    (0.28, 0.06),
                    (0.6, -0.24),
                    (0.6, 0.36),
                ],
            );
            if crown {
                let band = p.1 > 0.18;
                return Some(if band { [230, 170, 50, 255] } else { GOLD });
            }
            None
        }
        FeatureIcon::ShoppingMall => {
            // A shopping bag.
            let handle = {
                let d = ((p.0).powi(2) + (p.1 + 0.12).powi(2)).sqrt();
                (0.18..=0.27).contains(&d) && p.1 < -0.1
            };
            if handle {
                return Some(GOLD);
            }
            let bag = polygon(
                p,
                &[(-0.42, -0.12), (0.42, -0.12), (0.5, 0.58), (-0.5, 0.58)],
            );
            if bag {
                let star = circle(p, (0.0, 0.24), 0.13);
                return Some(if star { GOLD } else { IVORY });
            }
            None
        }
        FeatureIcon::BugReport => {
            // A ladybird.
            if circle(p, (0.0, -0.4), 0.2) {
                return Some(DARK);
            }
            let legs = [(-0.3, -0.05), (-0.36, 0.2), (-0.3, 0.45)]
                .iter()
                .any(|&(x, y)| {
                    capsule(p, (x, y), (x - 0.3, y - 0.05), 0.04)
                        || capsule(p, (-x, y), (-x + 0.3, y - 0.05), 0.04)
                });
            if legs {
                return Some(DARK);
            }
            if circle(p, (0.0, 0.18), 0.44) {
                let seam = p.0.abs() < 0.03;
                let spot = circle(p, (-0.2, 0.06), 0.08)
                    || circle(p, (0.2, 0.06), 0.08)
                    || circle(p, (-0.18, 0.34), 0.08)
                    || circle(p, (0.18, 0.34), 0.08);
                return Some(if seam || spot {
                    DARK
                } else {
                    [226, 40, 34, 255]
                });
            }
            None
        }
    }
}
