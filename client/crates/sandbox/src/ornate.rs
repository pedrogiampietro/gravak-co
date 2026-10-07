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

/// How a button looks: as it is, under the cursor, held down or unusable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Look {
    Normal,
    Hovered,
    Pressed,
    Disabled,
}

/// The palette greyed, for a button that cannot be used.
fn greyed(palette: Palette) -> Palette {
    let grey = |colour: Colour| {
        let value = colour[0] * 0.3 + colour[1] * 0.59 + colour[2] * 0.11;
        paint::mix([value; 3], hex(0x101418), 0.35)
    };
    Palette {
        metal: palette.metal.map(grey),
        panel: palette.panel.map(grey),
        text: palette.text.map(|colour| paint::mix(grey(colour), hex(0x808080), 0.3)),
        glow: palette.glow,
    }
}

/// An ornate button of any size. From [`SIZE`]'s height up it has spikes,
/// a glow when hovered and room for the `emblem`; smaller ones are a framed
/// plate filling all of `size`. Without `assets` the HUD's pattern is left
/// out.
pub fn plate(
    mut assets: Option<&mut Assets>,
    size: [u32; 2],
    label: &str,
    palette: Palette,
    look: Look,
    emblem: Option<&str>,
) -> crate::assets::Result<RgbaImage> {
    let [width, height] = size;
    let (w, h) = (width as f32, height as f32);
    let mut image = RgbaImage::new(width.max(1), height.max(1));
    let spiked = height >= 34 && width >= 60;
    let hovered = look == Look::Hovered;
    let palette = match look {
        Look::Disabled => greyed(palette),
        Look::Pressed => Palette {
            panel: palette.panel.map(|colour| paint::mix(colour, hex(0x000000), 0.35)),
            ..palette
        },
        _ => palette,
    };
    // The plate, inside the spikes.
    let inset = if spiked { [7.0_f32, 6.0_f32] } else { [0.5_f32, 0.5_f32] };
    let plate_centre = [w / 2.0, h / 2.0];
    let plate_half = [w / 2.0 - inset[0], h / 2.0 - inset[1]];
    let plate = |x: f32, y: f32| paint::rounded_rect(x, y, plate_centre, plate_half, 3.0);
    let (top, bottom) = (inset[1], h - inset[1]);
    let metal = |_: f32, y: f32| {
        let t = (y - top) / (bottom - top);
        paint::gradient(&[(0.0, palette.metal[0]), (0.45, palette.metal[1]), (1.0, palette.metal[2])], t)
    };

    if hovered && spiked {
        paint::glow(&mut image, plate, 6.0, palette.glow, 0.5);
    }
    // Shadow, then the spikes behind the frame.
    if spiked {
        paint::fill(&mut image, |x, y| plate(x, y - 2.0) - 0.5, |_, _| hex(0x000000), 0.55);
    }
    let (left, right) = (inset[0], w - inset[0]);
    let corners = if spiked { [(left, top, -1.0, -1.0), (right, top, 1.0, -1.0), (left, bottom, -1.0, 1.0), (right, bottom, 1.0, 1.0)].to_vec() } else { Vec::new() };
    for (cx, cy, sx, sy) in corners {
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
    for (cx, sx) in if spiked { vec![(left, -1.0), (right, 1.0)] } else { Vec::new() } {
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

    // The inner panel, 3 pixels in (2 on small buttons).
    let rim = if spiked { 3.0 } else { 2.0 };
    let panel_half = [plate_half[0] - rim, plate_half[1] - rim];
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
    if let Some(Ok(art)) = assets.as_deref_mut().map(|assets| assets.image("data/main/mainDialog2.dds")) {
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
        |x, y| {
            let gloss_half = (panel_half[1] * 0.3).max(1.5);
            paint::rounded_rect(x, y, [plate_centre[0], top + rim + gloss_half + 1.0], [panel_half[0] - 4.0, gloss_half], gloss_half)
        },
        |_, _| hex(0xffffff),
        0.10,
    );

    // The medallion on the left edge.
    let mut text_left = left + if spiked { 6.0 } else { 7.0 };
    if let Some(letter) = emblem.filter(|_| spiked) {
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

    // The label: spaced serif capitals, cream to gold, over a dark drop,
    // shrunk to fit; small ones in the plain font, which reads better.
    let text_right = right - if spiked { 6.0 } else { 9.0 };
    let room = (text_right - text_left).max(1.0);
    let label = label.to_uppercase();
    let mut size = if spiked { 15.0_f32 } else { (h * 0.52).clamp(9.0, 13.0) };
    // Cinzel, its spacing given up first, down to 9px; below that the
    // plain font reads better.
    let cinzel = crate::labels::decorative_font()?;
    let mut spacing = size * 0.09;
    while size > 9.0 && paint::text_width(&cinzel, &label, size, spacing) > room {
        if spacing > 0.0 {
            spacing = (spacing - 0.4).max(0.0);
        } else {
            size -= 0.5;
        }
    }
    let font = if paint::text_width(&cinzel, &label, size, spacing) <= room {
        cinzel
    } else {
        let plain = crate::labels::font()?;
        while size > 8.0 && paint::text_width(&plain, &label, size, 0.0) > room {
            size -= 0.5;
        }
        plain
    };
    let label_width = paint::text_width(&font, &label, size, spacing);
    let origin = [
        (text_left + (text_right - text_left - label_width) / 2.0).round(),
        (h / 2.0 - size * 0.66 - if spiked { 0.0 } else { 1.0 }).round(),
    ];
    let mask = paint::text_mask(&font, &label, size, spacing, origin, [image.width(), image.height()]);
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

/// The VIP title shown over a VIP's name, at `scale` (1 over the name,
/// larger in the VIP window): "VIP" in gold Cinzel with a dark outline,
/// leaf wings on both sides, a diamond above, a warm glow and sparks.
pub fn vip_tag(scale: f32) -> crate::assets::Result<RgbaImage> {
    let (width, height) = ((78.0 * scale).round() as u32, (26.0 * scale).round() as u32);
    let (w, h) = (width as f32, height as f32);
    let mut image = RgbaImage::new(width, height);
    let centre = [w / 2.0, h * 0.56];
    let gold = |_: f32, y: f32| {
        paint::gradient(&[(0.0, hex(0xfff4c8)), (0.5, hex(0xf2c14e)), (1.0, hex(0xa86f14))], (y - h * 0.2) / (h * 0.7))
    };
    // A warm glow behind it all, strongest in the middle.
    for y in 0..height as i32 {
        for x in 0..width as i32 {
            let dx = (x as f32 + 0.5 - centre[0]) / (w * 0.42);
            let dy = (y as f32 + 0.5 - centre[1]) / (h * 0.55);
            let fade = (1.0 - (dx * dx + dy * dy).sqrt()).max(0.0);
            paint::blend(&mut image, x, y, hex(0xffbf3a), fade * fade * 0.45);
        }
    }
    // Leaf wings, three a side, sweeping outward.
    for side in [-1.0_f32, 1.0] {
        for (index, (reach, rise, size)) in [(0.20_f32, -0.10_f32, 1.0_f32), (0.30, 0.02, 0.85), (0.39, 0.14, 0.7)].into_iter().enumerate() {
            let base = [centre[0] + side * w * (0.17 + index as f32 * 0.02), centre[1] + h * 0.05];
            let tip = [centre[0] + side * w * reach * 1.25, centre[1] + h * rise];
            let thickness = h * 0.11 * size;
            let leaf = [
                base,
                [(base[0] + tip[0]) / 2.0, (base[1] + tip[1]) / 2.0 - thickness],
                tip,
                [(base[0] + tip[0]) / 2.0, (base[1] + tip[1]) / 2.0 + thickness * 0.6],
            ];
            polygon(&mut image, &leaf.map(|[x, y]| [x + 0.6 * scale, y + 0.9 * scale]), |_, _| hex(0x2a1602), 0.7);
            polygon(&mut image, &leaf, gold, 1.0);
        }
    }
    // A diamond above the word.
    let top = [centre[0], h * 0.13];
    let diamond = [
        [top[0], top[1] - h * 0.12],
        [top[0] + w * 0.035, top[1]],
        [top[0], top[1] + h * 0.12],
        [top[0] - w * 0.035, top[1]],
    ];
    polygon(&mut image, &diamond, |_, _| hex(0xfff0b8), 1.0);
    // The word: a dark outline all round, then the gold, thickened.
    let font = crate::labels::decorative_font()?;
    let (size, spacing) = (15.0 * scale, 1.0 * scale);
    let label_width = paint::text_width(&font, "VIP", size, spacing);
    let origin = [(centre[0] - label_width / 2.0).round(), (centre[1] - size * 0.62).round()];
    let mask = paint::text_mask(&font, "VIP", size, spacing, origin, [width, height]);
    let outline = (1.0 * scale).max(1.0).round() as i32;
    for dy in -outline..=outline {
        for dx in -outline..=outline {
            if dx != 0 || dy != 0 {
                paint::paint_mask(&mut image, &mask, [dx, dy + 1], |_, _| hex(0x2a1602), 0.75);
            }
        }
    }
    for (shift, alpha) in [(1, 0.5), (0, 1.0)] {
        paint::paint_mask(&mut image, &mask, [shift, 0], |_, y| gold(0.0, y), alpha);
    }
    // Sparks.
    for [fx, fy, strength] in [[0.12_f32, 0.30, 1.0], [0.88, 0.25, 0.8], [0.70, 0.86, 0.6], [0.30, 0.90, 0.5]] {
        let (x, y) = ((w * fx) as i32, (h * fy) as i32);
        paint::blend(&mut image, x, y, hex(0xffffff), strength);
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            paint::blend(&mut image, x + dx, y + dy, hex(0xffe9a0), strength * 0.45);
        }
    }
    Ok(image)
}
