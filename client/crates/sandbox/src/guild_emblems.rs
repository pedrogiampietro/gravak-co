//! Guild crests: one of [`EMBLEMS`] (game-icons.net art, CC BY 3.0, see
//! `guild_emblems/CREDITS.md`) in one of [`EMBLEM_COLOURS`] over a heraldic
//! shield of one of [`FIELD_COLOURS`], with a gold rim. A crest is a `u32`:
//! the emblem, the field colour << 8 and the emblem colour << 16, as the
//! server keeps it (`guild_panel.rs`).

use std::sync::OnceLock;

use image::{GrayImage, Rgba, RgbaImage};

/// The emblems, in the order the server numbers them.
pub const EMBLEMS: [(&str, &[u8]); 36] = [
    ("Wolf", include_bytes!("guild_emblems/wolf-head.png")),
    ("Howling Wolf", include_bytes!("guild_emblems/wolf-howl.png")),
    ("Dragon", include_bytes!("guild_emblems/dragon-head.png")),
    ("Sea Dragon", include_bytes!("guild_emblems/sea-dragon.png")),
    ("Dragon Coil", include_bytes!("guild_emblems/dragon-spiral.png")),
    ("Lion", include_bytes!("guild_emblems/lion.png")),
    ("Tiger", include_bytes!("guild_emblems/tiger-head.png")),
    ("Bear", include_bytes!("guild_emblems/bear-head.png")),
    ("Fox", include_bytes!("guild_emblems/fox-head.png")),
    ("Eagle", include_bytes!("guild_emblems/eagle-emblem.png")),
    ("Hawk", include_bytes!("guild_emblems/hawk-emblem.png")),
    ("Raven", include_bytes!("guild_emblems/raven.png")),
    ("Bat", include_bytes!("guild_emblems/bat.png")),
    ("Serpent", include_bytes!("guild_emblems/snake.png")),
    ("Scorpion", include_bytes!("guild_emblems/scorpion.png")),
    ("Horse", include_bytes!("guild_emblems/horse-head.png")),
    ("Shark", include_bytes!("guild_emblems/shark-jaws.png")),
    ("Talon", include_bytes!("guild_emblems/bird-claw.png")),
    ("Skull", include_bytes!("guild_emblems/skull-crossed-bones.png")),
    ("Horned Skull", include_bytes!("guild_emblems/horned-skull.png")),
    ("Swords", include_bytes!("guild_emblems/crossed-swords.png")),
    ("Axes", include_bytes!("guild_emblems/crossed-axes.png")),
    ("Battle Axe", include_bytes!("guild_emblems/battle-axe.png")),
    ("Winged Sword", include_bytes!("guild_emblems/winged-sword.png")),
    ("Trident", include_bytes!("guild_emblems/trident.png")),
    ("Barbute", include_bytes!("guild_emblems/barbute.png")),
    ("Knight Helm", include_bytes!("guild_emblems/visored-helm.png")),
    ("Crown", include_bytes!("guild_emblems/crown.png")),
    ("Castle", include_bytes!("guild_emblems/castle.png")),
    ("Oak", include_bytes!("guild_emblems/oak.png")),
    ("Rose", include_bytes!("guild_emblems/rose.png")),
    ("Sun", include_bytes!("guild_emblems/sun.png")),
    ("Moon", include_bytes!("guild_emblems/moon.png")),
    ("Lightning", include_bytes!("guild_emblems/lightning-helix.png")),
    ("Ankh", include_bytes!("guild_emblems/ankh.png")),
    ("Arrow", include_bytes!("guild_emblems/shield-reflect.png")),
];

/// The field colours (light, dark): crimson, royal blue, emerald, purple,
/// sable, gold, teal, orange, silver, bronze.
pub const FIELD_COLOURS: [([u8; 3], [u8; 3]); 10] = [
    ([214, 52, 52], [110, 14, 20]),
    ([62, 112, 226], [18, 40, 120]),
    ([56, 178, 92], [14, 84, 40]),
    ([152, 82, 206], [64, 24, 104]),
    ([70, 70, 78], [16, 16, 20]),
    ([236, 190, 74], [150, 100, 20]),
    ([48, 170, 170], [10, 80, 86]),
    ([240, 132, 48], [140, 54, 10]),
    ([210, 214, 222], [120, 126, 140]),
    ([168, 112, 70], [80, 46, 22]),
];

/// The emblem colours (light, dark): gold, silver, ivory, sable, crimson,
/// azure.
pub const EMBLEM_COLOURS: [([u8; 3], [u8; 3]); 6] = [
    ([255, 228, 120], [196, 140, 36]),
    ([242, 244, 250], [150, 156, 170]),
    ([255, 248, 228], [214, 196, 160]),
    ([60, 60, 66], [10, 10, 12]),
    ([236, 60, 56], [130, 16, 16]),
    ([120, 180, 255], [30, 70, 170]),
];

pub fn icon_of(emblem: u32) -> usize {
    (emblem & 0xff) as usize % EMBLEMS.len()
}

pub fn field_of(emblem: u32) -> usize {
    ((emblem >> 8) & 0xff) as usize % FIELD_COLOURS.len()
}

pub fn colour_of(emblem: u32) -> usize {
    ((emblem >> 16) & 0xff) as usize % EMBLEM_COLOURS.len()
}

pub fn compose(icon: usize, field: usize, colour: usize) -> u32 {
    (icon % EMBLEMS.len()) as u32 | ((field % FIELD_COLOURS.len()) as u32) << 8 | ((colour % EMBLEM_COLOURS.len()) as u32) << 16
}

/// The emblem art, decoded once.
fn mask(icon: usize) -> &'static GrayImage {
    static MASKS: OnceLock<Vec<GrayImage>> = OnceLock::new();
    let masks = MASKS.get_or_init(|| {
        EMBLEMS
            .iter()
            .map(|(_, bytes)| {
                image::load_from_memory(bytes)
                    .map(|image| {
                        let rgba = image.to_rgba8();
                        GrayImage::from_fn(rgba.width(), rgba.height(), |x, y| image::Luma([rgba.get_pixel(x, y)[3]]))
                    })
                    .unwrap_or_else(|_| GrayImage::new(1, 1))
            })
            .collect()
    });
    &masks[icon % masks.len()]
}

/// The shield half width at depth `v` (0 top, 1 point).
fn half(v: f32) -> f32 {
    if v < 0.5 {
        1.0
    } else {
        (1.0 - ((v - 0.5) / 0.5).clamp(0.0, 1.0).powf(1.8)).max(0.0)
    }
}

fn inside(x: f32, y: f32, w: f32, h: f32, inset: f32) -> bool {
    let half_w = w * 0.5;
    let u = (x - half_w) / half_w;
    if y < inset + 0.04 * h * u * u || y > h - inset * 1.5 {
        return false;
    }
    (x - half_w).abs() <= half((y / h).clamp(0.0, 1.0)) * half_w - inset
}

fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

fn rgb(colour: [u8; 3]) -> [f32; 3] {
    colour.map(f32::from)
}

/// The crest `emblem`, `width` by `height`: a dark outline, a gold rim lit
/// from the upper left, the field in a gradient with a shine, and the emblem
/// in its colour with a soft shadow; edges smoothed by supersampling.
pub fn crest_image(emblem: u32, width: u32, height: u32) -> RgbaImage {
    let (field_light, field_dark) = FIELD_COLOURS[field_of(emblem)];
    let (charge_light, charge_dark) = EMBLEM_COLOURS[colour_of(emblem)];
    let art = mask(icon_of(emblem));
    let (w, h) = (width as f32, height as f32);
    let scale = (w.min(h) / 40.0).max(0.5);
    let outline_px = scale.max(1.0);
    let rim_px = outline_px + 2.2 * scale;
    // The emblem square, centred a little above the shield middle.
    let side = w * 0.62;
    let (art_x, art_y) = ((w - side) * 0.5, h * 0.2);
    let sample_art = |x: f32, y: f32| -> f32 {
        let (ax, ay) = ((x - art_x) / side, (y - art_y) / side);
        if !(0.0..1.0).contains(&ax) || !(0.0..1.0).contains(&ay) {
            return 0.0;
        }
        let (mx, my) = ((ax * art.width() as f32) as u32, (ay * art.height() as f32) as u32);
        f32::from(art.get_pixel(mx.min(art.width() - 1), my.min(art.height() - 1))[0]) / 255.0
    };
    const SAMPLES: u32 = 3;
    let mut image = RgbaImage::new(width, height);
    for py in 0..height {
        for px in 0..width {
            let (mut colour, mut cover) = ([0.0_f32; 3], 0.0_f32);
            for sy in 0..SAMPLES {
                for sx in 0..SAMPLES {
                    let x = px as f32 + (sx as f32 + 0.5) / SAMPLES as f32;
                    let y = py as f32 + (sy as f32 + 0.5) / SAMPLES as f32;
                    if !inside(x, y, w, h, 0.0) {
                        continue;
                    }
                    cover += 1.0;
                    let sample = if !inside(x, y, w, h, outline_px) {
                        [24.0, 16.0, 6.0]
                    } else if !inside(x, y, w, h, rim_px) {
                        mix([255.0, 232.0, 150.0], [150.0, 98.0, 30.0], ((y / h) * 0.9 + (x / w) * 0.4).clamp(0.0, 1.0))
                    } else {
                        let base = mix(rgb(field_light), rgb(field_dark), ((y / h) * 1.05 + (x / w - 0.5) * 0.3).clamp(0.0, 1.0));
                        let shine = (1.0 - (((x / w - 0.3) / 0.24).powi(2) + ((y / h - 0.22) / 0.14).powi(2))).max(0.0);
                        let field = mix(base, [255.0, 255.0, 255.0], shine * 0.35);
                        let shadow = sample_art(x - scale * 1.2, y - scale * 1.2);
                        let field = mix(field, [0.0, 0.0, 0.0], shadow * 0.45);
                        let charge = sample_art(x, y);
                        let tone = mix(rgb(charge_light), rgb(charge_dark), ((y - art_y) / side).clamp(0.0, 1.0));
                        mix(field, tone, charge)
                    };
                    for channel in 0..3 {
                        colour[channel] += sample[channel];
                    }
                }
            }
            if cover > 0.0 {
                let alpha = cover / (SAMPLES * SAMPLES) as f32;
                let rgb = colour.map(|value| (value / cover).round().clamp(0.0, 255.0) as u8);
                image.put_pixel(px, py, Rgba([rgb[0], rgb[1], rgb[2], (alpha * 255.0).round() as u8]));
            }
        }
    }
    image
}

/// The crest drawn into `image` at `(x, y)`.
pub fn draw_crest(image: &mut RgbaImage, emblem: u32, x: i32, y: i32, width: u32, height: u32) {
    let crest = crest_image(emblem, width, height);
    image::imageops::overlay(image, &crest, i64::from(x), i64::from(y));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_emblem_decodes_and_crests_compose() {
        for icon in 0..EMBLEMS.len() {
            assert_eq!(mask(icon).width(), 128, "{}", EMBLEMS[icon].0);
        }
        let crest = compose(5, 3, 2);
        assert_eq!((icon_of(crest), field_of(crest), colour_of(crest)), (5, 3, 2));
        assert_eq!(crest_image(crest, 40, 46).dimensions(), (40, 46));
    }

    /// `GUILD_PREVIEW=dir cargo test -p conquer-sandbox crest_preview`.
    #[test]
    fn crest_preview() {
        let Ok(dir) = std::env::var("GUILD_PREVIEW") else { return };
        let mut sheet = RgbaImage::from_pixel(12 * 76, 3 * 86 + 40, Rgba([30, 44, 56, 255]));
        for icon in 0..EMBLEMS.len() {
            let crest = crest_image(compose(icon, icon % FIELD_COLOURS.len(), icon % EMBLEM_COLOURS.len()), 64, 72);
            image::imageops::overlay(&mut sheet, &crest, (icon % 12) as i64 * 76 + 6, (icon / 12) as i64 * 86 + 6);
        }
        for (index, size) in [(0_i64, [22_u32, 25]), (1, [30, 34])] {
            let crest = crest_image(compose(0, 0, 0), size[0], size[1]);
            image::imageops::overlay(&mut sheet, &crest, 10 + index * 40, 3 * 86 + 4);
        }
        sheet.save(format!("{dir}/crests.png")).expect("png");
    }
}
