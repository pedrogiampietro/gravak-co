//! Ornate buttons in the game's own style: a plate in a metal frame with
//! spikes at its corners and diamonds on its sides, an inner panel with a
//! glow and the HUD's diamond pattern, serif capitals, and an optional
//! medallion riveted on its left edge. Drawn with `paint`, so smooth.

use crate::assets::Assets;
use crate::paint::{self, hex, Colour};
use image::RgbaImage;

/// The colours of one kind of button.
#[derive(Debug, Clone, Copy)]
pub struct Palette {
    /// The frame's metal, from its light top to its dark bottom.
    pub metal: [Colour; 3],
    /// The inner panel: its glowing centre, its body and its dark edge.
    pub panel: [Colour; 3],
    /// The label, from its top to its bottom.
    pub text: [Colour; 2],
    /// Around the button under the cursor.
    pub glow: Colour,
}

/// Gold on the deep teal of the HUD.
pub const GRAVAK: Palette = Palette {
    metal: [hex(0xfbe7a8), hex(0xc9963c), hex(0x6e4612)],
    panel: [hex(0x2a6a96), hex(0x123651), hex(0x061826)],
    text: [hex(0xfff6d8), hex(0xe7bd62)],
    glow: hex(0xffd36b),
};

/// Size of an ornate button, spikes included.
pub const SIZE: [u32; 2] = [136, 40];

/// A filled polygon, edges smoothed by 4x4 samples a pixel.
fn polygon(image: &mut RgbaImage, points: &[[f32; 2]], colour: impl Fn(f32, f32) -> Colour, alpha: f32) {
    let inside = |x: f32, y: f32| {
        let mut crossings = false;
        for index in 0..points.len() {
            let [ax, ay] = points[index];
            let [bx, by] = points[(index + 1) % points.len()];
            if (ay > y) != (by > y) && x < ax + (y - ay) * (bx - ax) / (by - ay) {
                crossings = !crossings;
            }
        }
        crossings
    };
    let (min_x, max_x) = points.iter().fold((f32::MAX, f32::MIN), |(lo, hi), p| (lo.min(p[0]), hi.max(p[0])));
    let (min_y, max_y) = points.iter().fold((f32::MAX, f32::MIN), |(lo, hi), p| (lo.min(p[1]), hi.max(p[1])));
    for y in (min_y.floor() as i32).max(0)..=(max_y.ceil() as i32).min(image.height() as i32 - 1) {
        for x in (min_x.floor() as i32).max(0)..=(max_x.ceil() as i32).min(image.width() as i32 - 1) {
            let mut hits = 0;
            for sy in 0..4 {
                for sx in 0..4 {
                    if inside(x as f32 + (sx as f32 + 0.5) / 4.0, y as f32 + (sy as f32 + 0.5) / 4.0) {
                        hits += 1;
                    }
                }
            }
            if hits > 0 {
                paint::blend(image, x, y, colour(x as f32 + 0.5, y as f32 + 0.5), alpha * hits as f32 / 16.0);
            }
        }
    }
}

/// An ornate button with `label`; `emblem` is a letter on a gold medallion
/// riveted on the left edge.
pub fn button(
    assets: &mut Assets,
    label: &str,
    palette: Palette,
    hovered: bool,
    emblem: Option<&str>,
) -> crate::assets::Result<RgbaImage> {
    let [width, height] = SIZE;
    let (w, h) = (width as f32, height as f32);
    let mut image = RgbaImage::new(width, height);
    // The plate, inside the spikes.
    let inset = [7.0_f32, 6.0_f32];
    let plate_centre = [w / 2.0, h / 2.0];
    let plate_half = [w / 2.0 - inset[0], h / 2.0 - inset[1]];
    let plate = |x: f32, y: f32| paint::rounded_rect(x, y, plate_centre, plate_half, 3.0);
    let (top, bottom) = (inset[1], h - inset[1]);
    let metal = |_: f32, y: f32| {
        let t = (y - top) / (bottom - top);
        paint::gradient(&[(0.0, palette.metal[0]), (0.45, palette.metal[1]), (1.0, palette.metal[2])], t)
    };

    if hovered {
        paint::glow(&mut image, plate, 6.0, palette.glow, 0.5);
    }
    // Shadow, then the spikes behind the frame.
    paint::fill(&mut image, |x, y| plate(x, y - 2.0) - 0.5, |_, _| hex(0x000000), 0.55);
    let (left, right) = (inset[0], w - inset[0]);
    for (cx, cy, sx, sy) in [(left, top, -1.0, -1.0), (right, top, 1.0, -1.0), (left, bottom, -1.0, 1.0), (right, bottom, 1.0, 1.0)] {
        // A thorn grown out of each corner: it leaves the frame along both
        // edges and curls out to a point.
        let thorn = [
            [cx - sx * 13.0, cy - sy * 0.5],
            [cx - sx * 6.0, cy + sy * 1.2],
            [cx + sx * 1.5, cy + sy * 3.0],
            [cx + sx * 6.5, cy + sy * 6.0],
            [cx + sx * 3.0, cy + sy * 1.5],
            [cx + sx * 1.2, cy - sy * 6.0],
            [cx - sx * 0.5, cy - sy * 13.0],
        ];
        polygon(&mut image, &thorn.map(|[x, y]| [x + 0.6, y + 1.2]), |_, _| hex(0x000000), 0.45);
        polygon(&mut image, &thorn, metal, 1.0);
    }
    for (cx, sx) in [(left, -1.0), (right, 1.0)] {
        // A diamond on each side, half over the frame.
        let cy = h / 2.0;
        let diamond = [[cx + sx * 5.5, cy], [cx + sx * 1.0, cy - 5.5], [cx - sx * 3.5, cy], [cx + sx * 1.0, cy + 5.5]];
        polygon(&mut image, &diamond.map(|[x, y]| [x + 0.6, y + 1.2]), |_, _| hex(0x000000), 0.45);
        polygon(&mut image, &diamond, metal, 1.0);
    }

    // The frame: dark outline, metal, then a thin light line inside it.
    paint::fill(&mut image, |x, y| plate(x, y) - 1.0, |_, _| hex(0x1c1004), 1.0);
    paint::fill(&mut image, plate, metal, 1.0);
    paint::stroke(&mut image, |x, y| plate(x, y) + 0.8, 0.8, |_, y| if y < h / 2.0 { hex(0xffffff) } else { palette.metal[2] }, 0.45);

    // The inner panel, 3 pixels in.
    let panel_half = [plate_half[0] - 3.0, plate_half[1] - 3.0];
    let panel = |x: f32, y: f32| paint::rounded_rect(x, y, plate_centre, panel_half, 1.5);
    let lift = if hovered { 0.18 } else { 0.0 };
    paint::fill(
        &mut image,
        panel,
        |x, y| {
            // A glow in the middle, darker toward the edges.
            let dx = (x - plate_centre[0]) / panel_half[0];
            let dy = (y - plate_centre[1]) / panel_half[1];
            let t = (dx * dx * 0.55 + dy * dy).sqrt();
            let base = paint::gradient(&[(0.0, palette.panel[0]), (0.6, palette.panel[1]), (1.1, palette.panel[2])], t);
            paint::mix(base, palette.panel[0], lift)
        },
        1.0,
    );
    // The HUD's diamond pattern, faint.
    if let Ok(art) = assets.image("data/main/mainDialog2.dds") {
        let tile = image::imageops::crop_imm(&art, 33, 66, 18, 44).to_image();
        for y in 0..height as i32 {
            for x in 0..width as i32 {
                let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                let cover = paint::coverage(panel(px, py));
                if cover <= 0.0 {
                    continue;
                }
                let texel = tile.get_pixel(x as u32 % tile.width(), y as u32 % tile.height()).0;
                let luminance = (texel[0] as f32 * 0.3 + texel[1] as f32 * 0.6 + texel[2] as f32 * 0.1) / 255.0;
                paint::blend(&mut image, x, y, hex(0x9fd4ff), cover * (luminance - 0.15).max(0.0) * 0.35);
            }
        }
    }
    // A bevel on the panel: shade at its top edge, light at its bottom.
    paint::stroke(&mut image, |x, y| panel(x, y) + 0.6, 1.0, |_, y| if y < h / 2.0 { hex(0x02080e) } else { hex(0x6fb6e6) }, 0.5);
    // Gloss across the upper part.
    paint::fill(
        &mut image,
        |x, y| paint::rounded_rect(x, y, [plate_centre[0], top + 7.5], [panel_half[0] - 4.0, 3.0], 3.0),
        |_, _| hex(0xffffff),
        0.10,
    );

    // The medallion on the left edge.
    let mut text_left = left + 6.0;
    if let Some(letter) = emblem {
        let centre = [left + 3.0, h / 2.0];
        let radius = 12.0;
        paint::fill(&mut image, |x, y| paint::circle(x, y, [centre[0] + 1.0, centre[1] + 1.5], radius), |_, _| hex(0x000000), 0.5);
        paint::fill(&mut image, |x, y| paint::circle(x, y, centre, radius), |_, _| hex(0x3a2206), 1.0);
        let light = [centre[0] - 4.0, centre[1] - 5.0];
        paint::fill(
            &mut image,
            |x, y| paint::circle(x, y, centre, radius - 1.2),
            |x, y| {
                let t = ((x - light[0]).powi(2) + (y - light[1]).powi(2)).sqrt() / 20.0;
                paint::gradient(&[(0.0, hex(0xfff6cf)), (0.3, hex(0xf2c862)), (0.7, hex(0xc78a22)), (1.0, hex(0x7f4c10))], t)
            },
            1.0,
        );
        paint::stroke(&mut image, |x, y| paint::circle(x, y, centre, 8.4), 0.9, |_, _| hex(0x8a5812), 0.8);
        let font = crate::labels::font()?;
        let size = 14.0;
        let glyph_width = paint::text_width(&font, letter, size, 0.0);
        let origin = [centre[0] - glyph_width / 2.0, centre[1] - size * 0.62];
        let mask = paint::text_mask(&font, letter, size, 0.0, origin, [width, height]);
        paint::paint_mask(&mut image, &mask, [0, 1], |_, _| hex(0xfff0bd), 0.7);
        paint::paint_mask(&mut image, &mask, [0, 0], |_, _| hex(0x5a3504), 1.0);
        paint::blend(&mut image, light[0] as i32 - 1, light[1] as i32 - 1, hex(0xffffff), 0.9);
        text_left = centre[0] + radius + 2.0;
    }

    // The label: spaced serif capitals, cream to gold, over a dark drop.
    let font = crate::labels::decorative_font()?;
    let (size, spacing) = (15.0_f32, 1.4_f32);
    let label = label.to_uppercase();
    let label_width = paint::text_width(&font, &label, size, spacing);
    let text_right = right - 6.0;
    let origin = [
        (text_left + (text_right - text_left - label_width) / 2.0).round(),
        (h / 2.0 - size * 0.66).round(),
    ];
    let mask = paint::text_mask(&font, &label, size, spacing, origin, [width, height]);
    paint::paint_mask(&mut image, &mask, [1, 1], |_, _| hex(0x000000), 0.9);
    let text = |_: f32, y: f32| {
        let colour = paint::mix(palette.text[0], palette.text[1], (y - origin[1]) / size);
        if hovered { paint::mix(colour, hex(0xffffff), 0.25) } else { colour }
    };
    for (shift, alpha) in [(1, 0.4), (0, 1.0)] {
        paint::paint_mask(&mut image, &mask, [shift, 0], text, alpha);
    }
    Ok(image)
}
