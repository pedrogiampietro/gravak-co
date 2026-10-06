//! The game's cursors, drawn in code in the UI's gold so they need no
//! extra assets: an arrow for the world and HUD, a brighter arrow over
//! things that can be clicked, and target reticles with a badge for each
//! pending pick (friend, enemy, trade).

use image::{Rgba, RgbaImage};

/// Cursor images are this many pixels square.
pub const SIZE: u32 = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GameCursor {
    Arrow,
    /// Over loot, action slots and other clickable things.
    Interact,
    PickFriend,
    PickEnemy,
    PickTrade,
    /// The shop's Repair: a hammer, until an item is struck.
    Repair,
}

impl GameCursor {
    pub const ALL: [Self; 6] = [
        Self::Arrow,
        Self::Interact,
        Self::PickFriend,
        Self::PickEnemy,
        Self::PickTrade,
        Self::Repair,
    ];

    /// The pixel the click lands on.
    pub const fn hotspot(self) -> [u16; 2] {
        match self {
            Self::Arrow | Self::Interact => [2, 2],
            Self::PickFriend | Self::PickEnemy | Self::PickTrade => [RETICLE[0] as u16, RETICLE[1] as u16],
            Self::Repair => [HAMMER_FACE[0] as u16, HAMMER_FACE[1] as u16],
        }
    }

    pub fn image(self) -> RgbaImage {
        let mut image = RgbaImage::new(SIZE, SIZE);
        match self {
            Self::Arrow => arrow(&mut image, false),
            Self::Interact => arrow(&mut image, true),
            Self::PickFriend => reticle(&mut image, Badge::Plus),
            Self::PickEnemy => reticle(&mut image, Badge::Cross),
            Self::PickTrade => reticle(&mut image, Badge::Coin),
            Self::Repair => hammer(&mut image),
        }
        image
    }
}

const GOLD_LIGHT: [f32; 3] = [255.0, 238.0, 176.0];
const GOLD: [f32; 3] = [226.0, 185.0, 77.0];
const GOLD_DARK: [f32; 3] = [168.0, 116.0, 34.0];
const INK: [f32; 3] = [34.0, 20.0, 8.0];
const RETICLE: [f32; 2] = [13.0, 13.0];

/// Coverage of a shape at signed distance `d` (negative inside), with a
/// one-pixel antialiased edge.
fn coverage(d: f32) -> f32 {
    (0.5 - d).clamp(0.0, 1.0)
}

/// Blends `colour` with `alpha` over the pixel ("over" compositing on a
/// straight-alpha image).
fn blend(image: &mut RgbaImage, x: u32, y: u32, colour: [f32; 3], alpha: f32) {
    if alpha <= 0.0 {
        return;
    }
    let pixel = image.get_pixel_mut(x, y);
    let below = f32::from(pixel.0[3]) / 255.0;
    let out = alpha + below * (1.0 - alpha);
    if out <= 0.0 {
        return;
    }
    for channel in 0..3 {
        let mixed = (colour[channel] * alpha + f32::from(pixel.0[channel]) * below * (1.0 - alpha)) / out;
        pixel.0[channel] = mixed.round().clamp(0.0, 255.0) as u8;
    }
    pixel.0[3] = (out * 255.0).round() as u8;
}

/// Paints every pixel with the colour and alpha `shade` returns for its
/// centre.
fn paint(image: &mut RgbaImage, shade: impl Fn(f32, f32) -> Option<([f32; 3], f32)>) {
    for y in 0..image.height() {
        for x in 0..image.width() {
            if let Some((colour, alpha)) = shade(x as f32 + 0.5, y as f32 + 0.5) {
                blend(image, x, y, colour, alpha);
            }
        }
    }
}

fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    let t = t.clamp(0.0, 1.0);
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

/// Signed distance to a closed polygon (negative inside).
fn polygon_distance(points: &[[f32; 2]], x: f32, y: f32) -> f32 {
    let mut distance = f32::INFINITY;
    let mut inside = false;
    for index in 0..points.len() {
        let a = points[index];
        let b = points[(index + 1) % points.len()];
        let edge = [b[0] - a[0], b[1] - a[1]];
        let to = [x - a[0], y - a[1]];
        let length = edge[0] * edge[0] + edge[1] * edge[1];
        let t = ((to[0] * edge[0] + to[1] * edge[1]) / length).clamp(0.0, 1.0);
        let dx = to[0] - edge[0] * t;
        let dy = to[1] - edge[1] * t;
        distance = distance.min((dx * dx + dy * dy).sqrt());
        if (a[1] > y) != (b[1] > y) && x < a[0] + (y - a[1]) / (b[1] - a[1]) * edge[0] {
            inside = !inside;
        }
    }
    if inside { -distance } else { distance }
}

fn segment_distance(a: [f32; 2], b: [f32; 2], x: f32, y: f32) -> f32 {
    let edge = [b[0] - a[0], b[1] - a[1]];
    let to = [x - a[0], y - a[1]];
    let t = ((to[0] * edge[0] + to[1] * edge[1]) / (edge[0] * edge[0] + edge[1] * edge[1])).clamp(0.0, 1.0);
    let dx = to[0] - edge[0] * t;
    let dy = to[1] - edge[1] * t;
    (dx * dx + dy * dy).sqrt()
}

/// The classic pointer: a gold arrow with a dark rim and a soft shadow.
/// `glint` adds a small four-point star for clickable things.
fn arrow(image: &mut RgbaImage, glint: bool) {
    const POINTS: [[f32; 2]; 7] = [
        [2.0, 2.0],
        [2.0, 22.0],
        [7.5, 17.0],
        [11.5, 25.5],
        [15.0, 24.0],
        [11.0, 15.5],
        [18.0, 15.5],
    ];
    // Shadow, offset down-right.
    paint(image, |x, y| {
        let d = polygon_distance(&POINTS, x - 1.5, y - 2.0);
        Some((INK, coverage(d - 0.8) * 0.35))
    });
    // Dark rim, then the gold body with a light-to-dark gradient.
    paint(image, |x, y| {
        let d = polygon_distance(&POINTS, x, y);
        Some((INK, coverage(d - 1.0)))
    });
    paint(image, |x, y| {
        let d = polygon_distance(&POINTS, x, y);
        let t = ((x - 2.0) + (y - 2.0)) / 34.0;
        let colour = if t < 0.5 {
            mix(GOLD_LIGHT, GOLD, t * 2.0)
        } else {
            mix(GOLD, GOLD_DARK, (t - 0.5) * 2.0)
        };
        Some((colour, coverage(d + 0.2)))
    });
    // A bright bevel along the left edge.
    paint(image, |x, y| {
        let d = segment_distance([3.6, 5.0], [3.6, 18.0], x, y);
        Some((GOLD_LIGHT, coverage(d - 0.35) * 0.9))
    });
    if glint {
        let center = [23.5, 7.5];
        paint(image, |x, y| {
            let dx = (x - center[0]).abs();
            let dy = (y - center[1]).abs();
            // A thin four-point star: two tapered bars.
            let bar = |along: f32, across: f32| across - (1.4 - along * 0.25).max(0.0);
            let d = bar(dx, dy).min(bar(dy, dx)).max((dx * dx + dy * dy).sqrt() - 6.0);
            Some(([255.0, 250.0, 225.0], coverage(d)))
        });
    }
}

/// Where the hammer's striking face is: the click lands there.
const HAMMER_FACE: [f32; 2] = [4.0, 13.0];

/// A smith's hammer: a steel head with a gold band across the top left,
/// its wooden handle running down to the right.
fn hammer(image: &mut RgbaImage) {
    const HEAD_CENTRE: [f32; 2] = [10.0, 9.0];
    // Along the handle, and across it (the head's length).
    let along = [std::f32::consts::FRAC_1_SQRT_2, std::f32::consts::FRAC_1_SQRT_2];
    let across = [-std::f32::consts::FRAC_1_SQRT_2, std::f32::consts::FRAC_1_SQRT_2];
    let point = |a: f32, b: f32| [HEAD_CENTRE[0] + across[0] * a + along[0] * b, HEAD_CENTRE[1] + across[1] * a + along[1] * b];
    let head = [point(-8.0, -4.0), point(8.5, -4.0), point(8.5, 4.0), point(-8.0, 4.0)];
    let handle = |x: f32, y: f32| segment_distance(point(0.0, 3.0), point(0.0, 22.0), x, y);
    // Shadow, then dark rims, so it reads on any ground.
    paint(image, |x, y| {
        let d = polygon_distance(&head, x - 1.5, y - 2.0).min(handle(x - 1.5, y - 2.0) - 2.2);
        Some((INK, coverage(d - 0.8) * 0.35))
    });
    paint(image, |x, y| Some((INK, coverage(handle(x, y) - 3.2))));
    paint(image, |x, y| Some((INK, coverage(polygon_distance(&head, x, y) - 1.0))));
    // The handle: warm wood, lighter along its middle.
    paint(image, |x, y| {
        let d = handle(x, y);
        let wood = mix([186.0, 120.0, 62.0], [104.0, 60.0, 26.0], (d / 2.2).clamp(0.0, 1.0));
        Some((wood, coverage(d - 2.1)))
    });
    // The head: steel, light on top, with a gold band in its middle.
    paint(image, |x, y| {
        let d = polygon_distance(&head, x, y);
        let a = (x - HEAD_CENTRE[0]) * across[0] + (y - HEAD_CENTRE[1]) * across[1];
        let b = (x - HEAD_CENTRE[0]) * along[0] + (y - HEAD_CENTRE[1]) * along[1];
        let steel = mix([236.0, 240.0, 246.0], [112.0, 120.0, 134.0], (b + 4.0) / 8.0);
        let colour = if a.abs() < 2.2 { mix(GOLD_LIGHT, GOLD_DARK, (b + 4.0) / 8.0) } else { steel };
        Some((colour, coverage(d + 0.2)))
    });
}

#[derive(Clone, Copy)]
enum Badge {
    /// Add friend.
    Plus,
    /// Enemy.
    Cross,
    /// Trade.
    Coin,
}

/// A gold targeting ring with ticks and a centre dot, and a round badge at
/// the bottom right naming the pick.
fn reticle(image: &mut RgbaImage, badge: Badge) {
    let [cx, cy] = RETICLE;
    let radius = 8.5;
    let ring = |x: f32, y: f32| ((x - cx).hypot(y - cy) - radius).abs();
    let ticks = |x: f32, y: f32| {
        [
            ([cx, cy - 12.5], [cx, cy - 6.5]),
            ([cx, cy + 6.5], [cx, cy + 12.5]),
            ([cx - 12.5, cy], [cx - 6.5, cy]),
            ([cx + 6.5, cy], [cx + 12.5, cy]),
        ]
        .into_iter()
        .map(|(a, b)| segment_distance(a, b, x, y))
        .fold(f32::INFINITY, f32::min)
    };
    // Dark rims first so the gold reads on any ground.
    paint(image, |x, y| Some((INK, coverage(ring(x, y) - 2.2).max(coverage(ticks(x, y) - 1.9)))));
    paint(image, |x, y| {
        let t = ((y - (cy - radius)) / (radius * 2.0)).clamp(0.0, 1.0);
        let colour = mix(GOLD_LIGHT, GOLD_DARK, t);
        Some((colour, coverage(ring(x, y) - 1.0).max(coverage(ticks(x, y) - 0.8))))
    });
    paint(image, |x, y| {
        let d = (x - cx).hypot(y - cy);
        Some((INK, coverage(d - 2.2)))
    });
    paint(image, |x, y| {
        let d = (x - cx).hypot(y - cy);
        Some((GOLD_LIGHT, coverage(d - 1.2)))
    });

    let center = [25.0, 25.0];
    let (fill, rim) = match badge {
        Badge::Plus => ([62.0, 160.0, 84.0], [16.0, 52.0, 24.0]),
        Badge::Cross => ([196.0, 52.0, 44.0], [64.0, 12.0, 10.0]),
        Badge::Coin => (GOLD, [96.0, 60.0, 14.0]),
    };
    let badge_distance = |x: f32, y: f32| (x - center[0]).hypot(y - center[1]) - 6.0;
    paint(image, |x, y| Some((INK, coverage(badge_distance(x, y) - 1.2))));
    paint(image, |x, y| {
        let t = (y - (center[1] - 6.0)) / 12.0;
        Some((mix(mix(fill, [255.0; 3], 0.35), fill, t), coverage(badge_distance(x, y))))
    });
    paint(image, |x, y| {
        let d = badge_distance(x, y).abs();
        Some((rim, coverage(d - 0.5) * 0.9))
    });
    let [bx, by] = center;
    match badge {
        Badge::Plus => paint(image, |x, y| {
            let d = segment_distance([bx - 3.2, by], [bx + 3.2, by], x, y)
                .min(segment_distance([bx, by - 3.2], [bx, by + 3.2], x, y));
            Some(([255.0; 3], coverage(d - 0.9)))
        }),
        Badge::Cross => paint(image, |x, y| {
            let d = segment_distance([bx - 2.6, by - 2.6], [bx + 2.6, by + 2.6], x, y)
                .min(segment_distance([bx - 2.6, by + 2.6], [bx + 2.6, by - 2.6], x, y));
            Some(([255.0; 3], coverage(d - 0.85)))
        }),
        // A coin: an inner ring and a vertical mark.
        Badge::Coin => paint(image, |x, y| {
            let inner = ((x - bx).hypot(y - by) - 3.6).abs();
            let mark = segment_distance([bx, by - 1.8], [bx, by + 1.8], x, y);
            Some(([110.0, 70.0, 16.0], coverage(inner - 0.5).max(coverage(mark - 0.6))))
        }),
    }
}

/// Copies a pixel's colour into `Rgba` for tests and debugging.
#[allow(dead_code)]
fn pixel(image: &RgbaImage, x: u32, y: u32) -> Rgba<u8> {
    *image.get_pixel(x, y)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_cursor_is_drawn_with_an_opaque_hotspot_area() {
        for cursor in GameCursor::ALL {
            let image = cursor.image();
            assert_eq!(image.dimensions(), (SIZE, SIZE));
            let [hx, hy] = cursor.hotspot();
            assert!(u32::from(hx) < SIZE && u32::from(hy) < SIZE);
            let opaque = image.pixels().filter(|pixel| pixel.0[3] > 200).count();
            assert!(opaque > 40, "{cursor:?} has {opaque} opaque pixels");
            // The corners stay clear so the cursor is not a square.
            assert_eq!(pixel(&image, SIZE - 1, 0).0[3], 0, "{cursor:?}");
        }
        if let Ok(dir) = std::env::var("CURSOR_PREVIEW_DIR") {
            let mut sheet = RgbaImage::from_pixel(SIZE * 6 * 4 + 48, SIZE * 4 * 2 + 20, Rgba([0, 0, 0, 0]));
            for (index, cursor) in GameCursor::ALL.into_iter().enumerate() {
                let big = image::imageops::resize(&cursor.image(), SIZE * 4, SIZE * 4, image::imageops::FilterType::Nearest);
                for (row, bg) in [[150u8, 140, 125], [40, 70, 50]].into_iter().enumerate() {
                    for y in 0..SIZE * 4 {
                        for x in 0..SIZE * 4 {
                            let p = big.get_pixel(x, y).0;
                            let a = f32::from(p[3]) / 255.0;
                            let c = [0, 1, 2].map(|i| (f32::from(p[i]) * a + f32::from(bg[i]) * (1.0 - a)) as u8);
                            sheet.put_pixel(index as u32 * (SIZE * 4 + 8) + x, row as u32 * (SIZE * 4 + 10) + y, Rgba([c[0], c[1], c[2], 255]));
                        }
                    }
                }
            }
            sheet.save(format!("{dir}/cursors.png")).unwrap();
        }
        // The arrow's tip sits on its hotspot.
        let arrow = GameCursor::Arrow.image();
        assert!(pixel(&arrow, 3, 4).0[3] > 200);
    }
}
