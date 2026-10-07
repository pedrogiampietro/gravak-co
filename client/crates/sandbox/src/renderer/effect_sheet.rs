//! Contact sheets of the weapon skills' effects, one per skill: a row per
//! level (MagicType's, from 0), its frames across, drawn by a software
//! rasteriser from the same C3 animation the renderer plays (meshes; the
//! particles and ribbons some effects add are left out). The caster's
//! effect stands left, the target's right, the trail's steps between.
//!
//! `EFFECT_SHEETS=dir cargo test -p conquer-sandbox effect_sheets -- --ignored`

use super::{draw_key_visible, sheet_cell, transparency_at, turned};
use crate::assets::Assets;
use image::{Rgba, RgbaImage};
use sandbox_data::c3::{C3Document, Matrix4};

const SKILLS: [(u32, &str); 15] = [
    (1250, "Glaive"),
    (1260, "Spear"),
    (1290, "Dagger"),
    (1300, "LongHammer"),
    (5000, "Bow"),
    (5010, "Wand"),
    (5020, "Halberd"),
    (5030, "Sword"),
    (5040, "Hammer"),
    (5050, "Poleaxe"),
    (7000, "Hook"),
    (7010, "Axe"),
    (7020, "Club"),
    (7030, "Scepter"),
    (7040, "Whip"),
];
const FRAMES: usize = 6;
const CELL: [u32; 2] = [300, 220];
const LABEL: u32 = 210;
/// The caster and the target, two cells apart as in a fight.
const CASTER: [f32; 2] = [90.0, 150.0];
const TARGET: [f32; 2] = [210.0, 150.0];

struct Layer {
    document: C3Document,
    texture: RgbaImage,
}

type Loaded = (Vec<Layer>, f32, f32);

fn load(assets: &mut Assets, name: &str) -> Option<Loaded> {
    let definition = crate::magic::effect(assets, name)?;
    let layers: Vec<Layer> = definition
        .layers
        .iter()
        .filter_map(|layer| {
            let texture = assets.image(&layer.texture).ok()?;
            Some(Layer {
                document: C3Document::parse(&assets.read(&layer.mesh).ok()?).ok()?,
                texture: if layer.additive { crate::magic::additive_image(texture) } else { texture },
            })
        })
        .collect();
    let frames = layers
        .iter()
        .flat_map(|layer| {
            layer.document.motions.iter().map(|motion| motion.frame_count as f32).chain(
                layer.document.meshes.iter().flat_map(|mesh| {
                    mesh.transparency_keys
                        .last()
                        .map(|key| key.frame as f32)
                        .into_iter()
                        .chain(mesh.draw_keys.last().map(|key| key.frame as f32))
                }),
            )
        })
        .fold(1.0_f32, f32::max);
    (!layers.is_empty()).then_some((layers, definition.frame_interval, frames))
}

fn duration(effect: &Option<Loaded>) -> f32 {
    effect.as_ref().map_or(0.0, |(_, interval, frames)| frames * interval / 1000.0)
}

/// The effect's frame at `seconds` over `canvas` around `origin`, projected
/// as the renderer does (the world's 45-degree view, three quarters size).
fn draw(canvas: &mut RgbaImage, effect: &Loaded, origin: [f32; 2], seconds: f32) {
    let (layers, interval, frames) = effect;
    let frame = seconds * 1000.0 / interval;
    if frame < 0.0 || frame >= *frames {
        return;
    }
    let (sin, cos) = (-std::f32::consts::FRAC_PI_4).sin_cos();
    let scale = 0.75;
    for layer in layers {
        for (index, mesh) in layer.document.meshes.iter().enumerate() {
            if !draw_key_visible(&mesh.draw_keys, frame) {
                continue;
            }
            let Some(motion) = layer.document.motions.get(index).or_else(|| layer.document.motions.first()) else {
                continue;
            };
            let alpha = transparency_at(&mesh.transparency_keys, frame);
            let cell = sheet_cell(mesh, frame);
            let points: Vec<([f32; 2], [f32; 2])> = mesh
                .vertices
                .iter()
                .map(|vertex| {
                    let p = turned(mesh.transformed_vertex_position(vertex, motion, Matrix4::IDENTITY, frame), None);
                    (
                        [origin[0] + p.x * scale, origin[1] + (p.y * sin + p.z * cos) * scale],
                        [vertex.texture_coordinate.x + cell[0], vertex.texture_coordinate.y + cell[1]],
                    )
                })
                .collect();
            for triangle in mesh.opaque_indices.chunks_exact(3).chain(mesh.alpha_indices.chunks_exact(3)) {
                let corners = [0, 1, 2].map(|k| points[triangle[k] as usize]);
                fill(canvas, corners, &layer.texture, alpha);
            }
        }
    }
}

/// One textured triangle blended over the canvas (straight alpha).
fn fill(canvas: &mut RgbaImage, corners: [([f32; 2], [f32; 2]); 3], texture: &RgbaImage, alpha: f32) {
    let [(a, ta), (b, tb), (c, tc)] = corners;
    let area = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
    if area.abs() < 1e-6 {
        return;
    }
    let (w, h) = (canvas.width() as f32, canvas.height() as f32);
    let min_x = a[0].min(b[0]).min(c[0]).floor().max(0.0) as u32;
    let max_x = a[0].max(b[0]).max(c[0]).ceil().min(w - 1.0).max(0.0) as u32;
    let min_y = a[1].min(b[1]).min(c[1]).floor().max(0.0) as u32;
    let max_y = a[1].max(b[1]).max(c[1]).ceil().min(h - 1.0).max(0.0) as u32;
    let (tw, th) = (texture.width() as f32, texture.height() as f32);
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            let w0 = ((b[0] - px) * (c[1] - py) - (b[1] - py) * (c[0] - px)) / area;
            let w1 = ((c[0] - px) * (a[1] - py) - (c[1] - py) * (a[0] - px)) / area;
            let w2 = 1.0 - w0 - w1;
            if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                continue;
            }
            let u = (w0 * ta[0] + w1 * tb[0] + w2 * tc[0]).rem_euclid(1.0);
            let v = (w0 * ta[1] + w1 * tb[1] + w2 * tc[1]).rem_euclid(1.0);
            let texel = texture.get_pixel(((u * tw) as u32).min(texture.width() - 1), ((v * th) as u32).min(texture.height() - 1));
            let k = f32::from(texel[3]) / 255.0 * alpha;
            let pixel = canvas.get_pixel_mut(x, y);
            for channel in 0..3 {
                pixel[channel] = (f32::from(texel[channel]) * k + f32::from(pixel[channel]) * (1.0 - k)).min(255.0) as u8;
            }
        }
    }
}

fn label(canvas: &mut RgbaImage, text: &str, at: [i32; 2], colour: [u8; 4]) {
    if let Ok(font) = crate::main_hud::font() {
        crate::main_hud::text(canvas, font, text, 13.0, at, colour, None);
    }
}

#[test]
#[ignore]
fn effect_sheets() {
    let Ok(dir) = std::env::var("EFFECT_SHEETS") else { return };
    let mut assets = Assets::open(std::path::Path::new("R:/5065")).expect("assets");
    for (skill_id, weapon) in SKILLS {
        let levels: Vec<_> = (0..10_u16)
            .filter_map(|level| crate::magic::magic_type(&mut assets, skill_id, level).filter(|magic| magic.level == level).cloned())
            .collect();
        let name = levels.first().map(|magic| magic.name.clone()).unwrap_or_default();
        let width = LABEL + CELL[0] * FRAMES as u32;
        let height = 40 + CELL[1] * levels.len() as u32;
        let mut sheet = RgbaImage::from_pixel(width, height, Rgba([18, 20, 26, 255]));
        label(&mut sheet, &format!("{name} ({skill_id}) - {weapon}: a row per level, frames across"), [12, 12], [255, 226, 143, 255]);
        for (row, magic) in levels.iter().enumerate() {
            let top = 40 + row as u32 * CELL[1];
            let sender = magic.sender_effect.as_deref().and_then(|name| load(&mut assets, name));
            let target = magic.target_effect.as_deref().and_then(|name| load(&mut assets, name));
            let steps = magic.trail.as_deref().map(|trail| crate::magic::trail(&mut assets, trail).to_vec()).unwrap_or_default();
            let trail: Vec<_> = steps
                .into_iter()
                .filter_map(|step| load(&mut assets, &step.effect).map(|effect| (step, effect)))
                .collect();
            let trail_end = trail
                .iter()
                .map(|(step, (_, interval, frames))| step.delay_ms as f32 / 1000.0 + frames * interval / 1000.0)
                .fold(0.0_f32, f32::max);
            let span = duration(&sender).max(duration(&target)).max(trail_end).max(0.1);
            let lines = [
                format!("level {} (server {})", magic.level, magic.level + 1),
                format!("caster: {}", magic.sender_effect.as_deref().unwrap_or("-")),
                format!("target: {}", magic.target_effect.as_deref().unwrap_or("-")),
                format!("trail: {}", magic.trail.as_deref().unwrap_or("-")),
                format!("{span:.2} s"),
            ];
            for (index, line) in lines.iter().enumerate() {
                label(&mut sheet, line, [10, top as i32 + 30 + index as i32 * 20], [220, 220, 220, 255]);
            }
            for column in 0..FRAMES {
                let left = LABEL + column as u32 * CELL[0];
                let mut cell = RgbaImage::from_pixel(CELL[0], CELL[1], Rgba([28, 32, 40, 255]));
                // Stand-ins for who strikes (blue) and who is struck (red).
                for (spot, colour) in [(CASTER, [90, 140, 220, 255]), (TARGET, [220, 90, 90, 255])] {
                    for dy in -3..=3_i32 {
                        for dx in -3..=3_i32 {
                            cell.put_pixel((spot[0] as i32 + dx) as u32, (spot[1] as i32 + dy) as u32, Rgba(colour));
                        }
                    }
                }
                let t = span * (column as f32 + 0.5) / FRAMES as f32;
                if let Some(effect) = &sender {
                    draw(&mut cell, effect, CASTER, t);
                }
                for (step, effect) in &trail {
                    let origin = [CASTER[0] + step.along as f32, CASTER[1] + step.across as f32];
                    draw(&mut cell, effect, origin, t - step.delay_ms as f32 / 1000.0);
                }
                if let Some(effect) = &target {
                    draw(&mut cell, effect, TARGET, t);
                }
                label(&mut cell, &format!("{t:.2}s"), [6, 4], [150, 150, 150, 255]);
                image::imageops::overlay(&mut sheet, &cell, i64::from(left), i64::from(top));
            }
        }
        sheet.save(format!("{dir}/{skill_id}-{name}.png")).expect("png");
    }
}
