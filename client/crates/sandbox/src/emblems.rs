//! Emblems drawn in code, in the style of the Reward Wall and the
//! Highscores: the compass of the top-left plate, and the HUD's round
//! feature buttons (Store, Ranking, Helper) with their coins, trophy and
//! cross.

use crate::daily_rewards_panel::{glow_in, paint_in, GOLD, GOLD_DARK, GOLD_LIGHT};
use crate::paint;
use crate::ui::UiRect;
use image::RgbaImage;

/// A diamond (rhombus) from `tip` to `tail`, `half_width` wide at its
/// middle: the points of the compass rose. Signed distance, roughly.
fn diamond(x: f32, y: f32, tip: [f32; 2], tail: [f32; 2], half_width: f32) -> f32 {
    let (dx, dy) = (tail[0] - tip[0], tail[1] - tip[1]);
    let length = (dx * dx + dy * dy).sqrt().max(f32::EPSILON);
    let (ux, uy) = (dx / length, dy / length);
    let (px, py) = (x - tip[0], y - tip[1]);
    let along = px * ux + py * uy;
    let across = (px * -uy + py * ux).abs();
    let half = length / 2.0;
    // How far from the middle along the axis, and the width there.
    let from_middle = (along - half).abs();
    let width_here = half_width * (1.0 - from_middle / half);
    (across - width_here).max(from_middle - half)
}

/// Which side of the axis from `tip` to `tail` a point lies on.
fn side(x: f32, y: f32, tip: [f32; 2], tail: [f32; 2]) -> f32 {
    (tail[0] - tip[0]) * (y - tip[1]) - (tail[1] - tip[1]) * (x - tip[0])
}

/// The compass rose of the top-left plate, `radius` pixels around `centre`.
pub fn compass(image: &mut RgbaImage, centre: [f32; 2], radius: f32) {
    let r = radius;
    let bounds = UiRect::new((centre[0] - r) as i32 - 2, (centre[1] - r) as i32 - 2, (2.0 * r) as i32 + 5, (2.0 * r) as i32 + 5);
    let circle = move |x: f32, y: f32, radius: f32| paint::circle(x, y, centre, radius);
    // The gold ring.
    glow_in(image, bounds, |x, y| circle(x, y, r), 4.0, paint::hex(0xffc85a), 0.25);
    paint_in(
        image,
        bounds,
        1,
        |x, y| circle(x, y, r),
        |_, y| paint::gradient(&[(0.0, GOLD_LIGHT), (0.45, GOLD), (1.0, GOLD_DARK)], (y - centre[1] + r) / (2.0 * r)),
        1.0,
    );
    // The face: deep blue, lighter at its middle.
    let face = r * 0.8;
    paint_in(
        image,
        bounds,
        1,
        |x, y| circle(x, y, face),
        |x, y| {
            let distance = ((x - centre[0]).powi(2) + (y - centre[1]).powi(2)).sqrt() / face;
            paint::mix(paint::hex(0x1f5a7a), paint::hex(0x081a28), distance)
        },
        1.0,
    );
    paint_in(image, bounds, 1, |x, y| circle(x, y, face).abs() - 0.5, |_, _| GOLD_DARK, 1.0);
    // Eight ticks round the face, the cardinal ones longer.
    for index in 0..8 {
        let angle = index as f32 * std::f32::consts::FRAC_PI_4;
        let (sin, cos) = angle.sin_cos();
        let outer = face - 0.8;
        let inner = if index % 2 == 0 { face - 3.6 } else { face - 2.2 };
        let a = [centre[0] + sin * outer, centre[1] - cos * outer];
        let b = [centre[0] + sin * inner, centre[1] - cos * inner];
        paint_in(
            image,
            bounds,
            0,
            |x, y| crate::daily_rewards_panel::segment(x, y, a, b) - 0.5,
            |_, _| paint::hex(0xc9a35a),
            0.9,
        );
    }
    // The rose: small steel points between, gold points on the cardinals.
    let long = face - 2.0;
    let short = face * 0.55;
    for index in 0..4 {
        let angle = (index as f32 + 0.5) * std::f32::consts::FRAC_PI_2;
        let (sin, cos) = angle.sin_cos();
        let tip = [centre[0] + sin * short, centre[1] - cos * short];
        paint_in(image, bounds, 0, |x, y| diamond(x, y, tip, centre, 1.6), |_, _| paint::hex(0x9fb0ba), 1.0);
    }
    for index in 0..4 {
        let angle = index as f32 * std::f32::consts::FRAC_PI_2;
        let (sin, cos) = angle.sin_cos();
        let tip = [centre[0] + sin * long, centre[1] - cos * long];
        let north = index == 0;
        // Lit on one side, shaded on the other.
        let (light, dark) = if north {
            (paint::hex(0xff9a6a), paint::hex(0xc0392b))
        } else {
            (GOLD_LIGHT, paint::hex(0xa77a2c))
        };
        paint_in(
            image,
            bounds,
            0,
            |x, y| diamond(x, y, tip, centre, 2.4),
            move |x, y| if side(x, y, tip, centre) > 0.0 { light } else { dark },
            1.0,
        );
    }
    // The jewel at the middle.
    paint_in(image, bounds, 0, |x, y| circle(x, y, 1.9), |_, _| GOLD_LIGHT, 1.0);
    paint_in(image, bounds, 0, |x, y| circle(x, y, 1.9).abs() - 0.4, |_, _| GOLD_DARK, 1.0);
}

/// The HUD's feature buttons.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Feature {
    Store,
    Ranking,
    Helper,
}

/// An ellipse's signed distance, roughly (`radii` across and down).
fn ellipse(x: f32, y: f32, centre: [f32; 2], radii: [f32; 2]) -> f32 {
    let (dx, dy) = ((x - centre[0]) / radii[0], (y - centre[1]) / radii[1]);
    ((dx * dx + dy * dy).sqrt() - 1.0) * radii[0].min(radii[1])
}

/// A stack of three gold coins.
fn coins(image: &mut RgbaImage, bounds: UiRect, centre: [f32; 2]) {
    for (index, offset) in [5.0_f32, 0.5, -4.0].into_iter().enumerate() {
        let top = [centre[0] + if index == 1 { 1.0 } else { 0.0 }, centre[1] + offset];
        let radii = [7.2, 2.6];
        // The coin's edge, a band under its face.
        let edge = move |x: f32, y: f32| {
            let band = (x - top[0]).abs() - radii[0];
            let below = (y - top[1] - 1.3).abs() - 1.3;
            band.max(below).min(ellipse(x, y, [top[0], top[1] + 2.6], radii))
        };
        paint_in(image, bounds, 0, edge, |_, _| paint::hex(0x8a5a14), 1.0);
        paint_in(image, bounds, 0, move |x, y| ellipse(x, y, top, radii), |_, _| paint::hex(0xffd36b), 1.0);
        paint_in(image, bounds, 0, move |x, y| ellipse(x, y, top, [4.3, 1.4]).abs() - 0.4, |_, _| paint::hex(0xb8862c), 1.0);
    }
}

/// A trophy cup: a bowl with handles, a stem and a base.
fn trophy(image: &mut RgbaImage, bounds: UiRect, centre: [f32; 2]) {
    let [cx, cy] = centre;
    let rim = cy - 7.0;
    let gold = |_: f32, y: f32| paint::mix(paint::hex(0xffe08a), paint::hex(0xc9922c), (y - rim) / 14.0);
    // The handles, rings either side of the bowl.
    for direction in [-1.0_f32, 1.0] {
        let ring_centre = [cx + direction * 7.4, rim + 3.0];
        paint_in(
            image,
            bounds,
            0,
            move |x, y| (paint::circle(x, y, ring_centre, 2.9).abs() - 0.95).max(-(x - cx) * direction),
            |_, _| paint::hex(0xd8a648),
            1.0,
        );
    }
    // The bowl: a half disc under its rim.
    let bowl_centre = [cx, rim];
    paint_in(image, bounds, 0, move |x, y| paint::circle(x, y, bowl_centre, 7.4).max(rim - y), gold, 1.0);
    // The stem and the base.
    paint_in(image, bounds, 0, move |x, y| ((x - cx).abs() - 1.4).max((y - (cy + 2.6)).abs() - 2.4), gold, 1.0);
    paint_in(image, bounds, 0, move |x, y| ((x - cx).abs() - 5.4).max((y - (cy + 6.2)).abs() - 1.5), gold, 1.0);
    // A star on the bowl.
    let star = [cx, rim + 3.0];
    paint_in(
        image,
        bounds,
        0,
        move |x, y| {
            let (dx, dy) = ((x - star[0]).abs(), (y - star[1]).abs());
            (dx * 2.2 + dy - 2.6).min(dx + dy * 2.2 - 2.6)
        },
        |_, _| paint::hex(0xfff6d8),
        1.0,
    );
}

/// The helper's cross: a green healer's cross edged in gold.
fn cross(image: &mut RgbaImage, bounds: UiRect, centre: [f32; 2]) {
    let [cx, cy] = centre;
    let plus = move |x: f32, y: f32, arm: f32, half: f32| {
        let (dx, dy) = ((x - cx).abs(), (y - cy).abs());
        ((dx - half).max(dy - arm)).min((dx - arm).max(dy - half))
    };
    paint_in(image, bounds, 0, move |x, y| plus(x, y, 8.6, 3.6), |_, _| paint::hex(0xd8a648), 1.0);
    paint_in(
        image,
        bounds,
        0,
        move |x, y| plus(x, y, 7.4, 2.4),
        move |_, y| paint::mix(paint::hex(0x9af59a), paint::hex(0x2f9a45), (y - cy + 7.4) / 14.8),
        1.0,
    );
    // A glint on the upper arm.
    paint_in(image, bounds, 0, move |x, y| ((x - cx + 0.8).abs() - 0.7).max((y - cy + 4.6).abs() - 2.0), |_, _| paint::hex(0xeaffea), 0.7);
}

/// Size of a round feature button's image, its glow's room included.
pub const ROUND_BUTTON: [u32; 2] = [40, 40];
/// The disc's radius in it.
const ROUND_RADIUS: f32 = 17.0;

/// A round feature button in the look of the client's own round buttons
/// over the orb (`RunChk1.dds`, `MapChk2.dds`...): an aged gold ring round
/// a dark disc, the feature's emblem on it; lit under the cursor.
pub fn round_button(feature: Feature, hovered: bool) -> RgbaImage {
    let [width, height] = ROUND_BUTTON;
    let mut image = RgbaImage::new(width, height);
    let centre = [width as f32 / 2.0, height as f32 / 2.0];
    let r = ROUND_RADIUS;
    let bounds = UiRect::new(0, 0, width as i32, height as i32);
    let disc = move |x: f32, y: f32, radius: f32| paint::circle(x, y, centre, radius);
    // A shadow under it, a glow under the cursor.
    if hovered {
        glow_in(&mut image, bounds, |x, y| disc(x, y, r), 3.0, paint::hex(0xffd36b), 0.9);
    } else {
        glow_in(&mut image, bounds, |x, y| disc(x, y, r), 3.0, paint::hex(0x000000), 0.45);
    }
    // The dark rim and the aged gold ring.
    paint_in(&mut image, bounds, 0, |x, y| disc(x, y, r), |_, _| paint::hex(0x1a140c), 1.0);
    let (light, dark) = if hovered {
        (paint::hex(0xfff0c4), paint::hex(0xa88a52))
    } else {
        (paint::hex(0xe6d3a6), paint::hex(0x7c6640))
    };
    paint_in(
        &mut image,
        bounds,
        0,
        |x, y| disc(x, y, r - 1.2),
        move |_, y| paint::gradient(&[(0.0, light), (0.55, paint::mix(light, dark, 0.5)), (1.0, dark)], (y - centre[1] + r) / (2.0 * r)),
        1.0,
    );
    // A dark groove inside the ring, then the disc.
    paint_in(&mut image, bounds, 0, |x, y| disc(x, y, r - 4.4), |_, _| paint::hex(0x241c10), 1.0);
    paint_in(
        &mut image,
        bounds,
        0,
        |x, y| disc(x, y, r - 5.4),
        |x, y| {
            // Lighter towards its top left, as the originals are lit.
            let distance = ((x - centre[0] + 3.0).powi(2) + (y - centre[1] + 3.0).powi(2)).sqrt() / (r - 5.4);
            paint::mix(paint::hex(0x24506a), paint::hex(0x07131c), distance.min(1.0))
        },
        1.0,
    );
    match feature {
        Feature::Store => coins(&mut image, bounds, [centre[0], centre[1] - 1.0]),
        Feature::Ranking => trophy(&mut image, bounds, [centre[0], centre[1] + 0.5]),
        Feature::Helper => cross(&mut image, bounds, centre),
    }
    // A glint on the ring's top left.
    let glint = [centre[0] - r * 0.55, centre[1] - r * 0.55];
    paint_in(&mut image, bounds, 0, |x, y| paint::circle(x, y, glint, 1.6), |_, _| paint::hex(0xfffbe8), 0.8);
    image
}

/// The Helper's button: the cross, with a light on its lower right, green
/// while the helper is on.
pub fn helper_button(hovered: bool, on: bool) -> RgbaImage {
    let mut image = round_button(Feature::Helper, hovered);
    let bounds = UiRect::new(0, 0, ROUND_BUTTON[0] as i32, ROUND_BUTTON[1] as i32);
    let light = [ROUND_BUTTON[0] as f32 - 10.0, ROUND_BUTTON[1] as f32 - 10.0];
    paint_in(&mut image, bounds, 0, |x, y| paint::circle(x, y, light, 4.4), |_, _| paint::hex(0x1a140c), 1.0);
    let colour = if on { paint::hex(0x5cf06c) } else { paint::hex(0x8a3a30) };
    if on {
        glow_in(&mut image, bounds, |x, y| paint::circle(x, y, light, 3.2), 3.0, colour, 0.8);
    }
    paint_in(&mut image, bounds, 0, |x, y| paint::circle(x, y, light, 3.2), |_, _| colour, 1.0);
    image
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `BOUNTY_PREVIEW=dir cargo test -p conquer-sandbox emblems_preview`
    #[test]
    fn emblems_preview() {
        let Ok(dir) = std::env::var("BOUNTY_PREVIEW") else { return };
        let mut sheet = RgbaImage::from_pixel(320, 150, image::Rgba([40, 60, 45, 255]));
        compass(&mut sheet, [40.0, 124.0], 17.0);
        for (column, (feature, hovered)) in
            [(Feature::Store, false), (Feature::Ranking, false), (Feature::Store, true), (Feature::Ranking, true)]
                .into_iter()
                .enumerate()
        {
            image::imageops::overlay(&mut sheet, &round_button(feature, hovered), 80 + column as i64 * 46, 104);
        }
        sheet.save(format!("{dir}/emblems.png")).expect("png");
    }

    /// `BOUNTY_PREVIEW=dir cargo test -p conquer-sandbox helper_button_preview`:
    /// the HUD's left with the Store, Ranking and Helper buttons, at twice
    /// the size.
    #[test]
    fn helper_button_preview() {
        let Ok(dir) = std::env::var("BOUNTY_PREVIEW") else { return };
        let mut assets = crate::assets::Assets::open(std::path::Path::new("R:/5065")).expect("assets");
        let viewport = [1024.0, 141.0];
        let mut hud = RgbaImage::new(1024, 141);
        crate::main_hud::preview_into(&mut hud, &mut assets);
        let place = |hud: &mut RgbaImage, button: &RgbaImage, rect: [f32; 4]| {
            image::imageops::overlay(hud, button, rect[0] as i64, rect[1] as i64);
        };
        let store = crate::store_panel::button_image(&mut assets, false).expect("store");
        place(&mut hud, &store, crate::hud::store_button_rect(viewport));
        place(&mut hud, &round_button(Feature::Ranking, false), crate::hud::ranking_button_rect(viewport));
        place(&mut hud, &helper_button(false, true), crate::hud::helper_button_rect(viewport));
        let crop = image::imageops::crop_imm(&hud, 100, 0, 360, 141).to_image();
        image::imageops::resize(&crop, 720, 282, image::imageops::FilterType::Nearest)
            .save(format!("{dir}/hud-helper.png"))
            .expect("png");
        let mut sheet = RgbaImage::from_pixel(200, 50, image::Rgba([40, 60, 45, 255]));
        for (column, (hovered, on)) in [(false, false), (false, true), (true, false), (true, true)].into_iter().enumerate() {
            image::imageops::overlay(&mut sheet, &helper_button(hovered, on), 5 + column as i64 * 48, 5);
        }
        sheet.save(format!("{dir}/helper-buttons.png")).expect("png");
    }
}



