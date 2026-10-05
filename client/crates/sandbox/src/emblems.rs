//! Emblems drawn in code, in the style of the Reward Wall and the
//! Highscores: the compass of the top-left plate, and the HUD's feature
//! buttons (Store, Ranking) with their coin and trophy.

use crate::daily_rewards_panel::{centred, glow_in, paint_in, rounded, GOLD, GOLD_DARK, GOLD_LIGHT};
use crate::paint::{self, Colour};
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
}

impl Feature {
    fn label(self) -> &'static str {
        match self {
            Self::Store => "Store",
            Self::Ranking => "Ranking",
        }
    }
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

/// A feature button `size` pixels: a rounded blue plate in gold, a gold
/// medallion with the feature's emblem on the left, the label beside it;
/// lit and glowing under the cursor.
pub fn feature_button(feature: Feature, size: [u32; 2], hovered: bool) -> crate::assets::Result<RgbaImage> {
    let mut image = RgbaImage::new(size[0], size[1]);
    let plate = UiRect::new(3, 3, size[0] as i32 - 6, size[1] as i32 - 6);
    let shape = rounded(plate, plate.height as f32 / 2.0);
    if hovered {
        glow_in(&mut image, plate, &shape, 3.0, paint::hex(0xffd36b), 0.9);
    }
    let (top, bottom): (Colour, Colour) = if hovered {
        (paint::hex(0x2f6f96), paint::hex(0x0b2536))
    } else {
        (paint::hex(0x1d4a66), paint::hex(0x071622))
    };
    let (y0, height) = (plate.y as f32, plate.height as f32);
    paint_in(&mut image, plate, 1, &shape, |_, y| paint::mix(top, bottom, (y - y0) / height), 0.96);
    paint_in(&mut image, plate, 1, |x, y| shape(x, y).abs() - 0.7, |_, _| if hovered { GOLD_LIGHT } else { GOLD }, 1.0);
    // A thin light along the top.
    let shine = UiRect::new(plate.x + 16, plate.y + 2, plate.width - 32, 2);
    let middle = (plate.x + plate.width / 2) as f32;
    let half = plate.width as f32 / 2.0;
    paint_in(
        &mut image,
        shine,
        0,
        |_, y| (y - shine.y as f32 - 0.5).abs() - 0.5,
        |x, _| paint::mix(paint::hex(0x9fd2ec), top, ((x - middle).abs() / half).powf(1.4)),
        0.5,
    );
    // The medallion and its emblem.
    let centre = [(plate.x + plate.height / 2) as f32, (plate.y + plate.height / 2) as f32];
    let radius = plate.height as f32 / 2.0 - 1.0;
    let medallion = UiRect::new(plate.x, plate.y, plate.height, plate.height);
    paint_in(
        &mut image,
        medallion,
        1,
        |x, y| paint::circle(x, y, centre, radius),
        |_, y| paint::gradient(&[(0.0, GOLD_LIGHT), (0.5, GOLD), (1.0, GOLD_DARK)], (y - centre[1] + radius) / (2.0 * radius)),
        1.0,
    );
    paint_in(&mut image, medallion, 1, |x, y| paint::circle(x, y, centre, radius - 2.8), |_, _| paint::hex(0x0d2a3c), 1.0);
    match feature {
        Feature::Store => coins(&mut image, medallion, centre),
        Feature::Ranking => trophy(&mut image, medallion, centre),
    }
    // The label, centred in what is left.
    let cinzel = crate::labels::decorative_font()?;
    let left = plate.x + plate.height + 2;
    let label_centre = left + (plate.x + plate.width - left) / 2;
    let colour = if hovered { [255, 246, 216] } else { [255, 226, 143] };
    centred(&mut image, &cinzel, feature.label(), colour, label_centre, plate.y + plate.height / 2 - 8, 14.0);
    Ok(image)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `BOUNTY_PREVIEW=dir cargo test -p conquer-sandbox emblems_preview`
    #[test]
    fn emblems_preview() {
        let Ok(dir) = std::env::var("BOUNTY_PREVIEW") else { return };
        let mut sheet = RgbaImage::from_pixel(320, 150, image::Rgba([40, 60, 45, 255]));
        let mut y = 8;
        for hovered in [false, true] {
            for (column, feature) in [Feature::Store, Feature::Ranking].into_iter().enumerate() {
                let button = feature_button(feature, [136, 40], hovered).expect("button");
                image::imageops::overlay(&mut sheet, &button, 8 + column as i64 * 146, y);
            }
            y += 48;
        }
        compass(&mut sheet, [40.0, 124.0], 17.0);
        sheet.save(format!("{dir}/emblems.png")).expect("png");
    }
}
