//! Views of the game world behind the login screens: a few scenic spots of
//! the real maps, composed from their tiles and scenery the way the game
//! draws them, then colour graded so the panels read over them.
//!
//! Composing a view takes a moment (the map is loaded first), so it runs on
//! a thread and each view is handed over as soon as it is ready.

use image::RgbaImage;
use std::{path::PathBuf, sync::mpsc};

/// Each view is larger than the window so the camera can drift across it.
pub const SCENE_SIZE: [u32; 2] = [2048, 1280];

/// (map, cell at the centre of the view): Bird Island's water town, the
/// Phoenix Castle tree houses, Twin City's plaza and its west gate, Desert
/// City's palm square and Ape Mountain's city.
const SPOTS: [(u32, [f32; 2]); 6] = [
    (1015, [717.0, 577.0]),
    (1011, [193.0, 266.0]),
    (1002, [430.0, 380.0]),
    (1000, [499.0, 650.0]),
    (1002, [520.0, 350.0]),
    (1020, [566.0, 565.0]),
];

/// Starts composing the views; they arrive one by one on the receiver.
pub fn start(root: PathBuf) -> mpsc::Receiver<RgbaImage> {
    let (sender, receiver) = mpsc::channel();
    // Without threads (the browser) nothing arrives and the screens go on
    // without it.
    let _ = std::thread::Builder::new().spawn(move || {
        let Ok(mut assets) = crate::assets::Assets::open(&root) else {
            return;
        };
        let mut map: Option<crate::map::Map> = None;
        for (map_id, cell) in SPOTS {
            if map.as_ref().map(|map| map.id) != Some(map_id) {
                map = crate::map::Map::load(&mut assets, map_id).ok();
            }
            let Some(map) = &map else { continue };
            let mut view = compose(&mut assets, map, cell, SCENE_SIZE);
            // A touch of blur sets the world back behind the panels.
            view = image::imageops::blur(&view, 1.4);
            grade(&mut view);
            if sender.send(view).is_err() {
                return;
            }
        }
    });
    receiver
}

/// A graded view of `map` around `cell` at any size, for pictures made
/// outside the login screens (the Instagram set in `marketing`).
#[cfg(test)]
pub fn view(assets: &mut crate::assets::Assets, map: &crate::map::Map, cell: [f32; 2], size: [u32; 2]) -> RgbaImage {
    let mut view = compose(assets, map, cell, size);
    view = image::imageops::blur(&view, 1.4);
    grade(&mut view);
    view
}

/// The map's tiles and scenery around `cell`, first animation frames only.
/// The world at `cell` without the login screens' blur, for the website.
pub fn sharp_view(assets: &mut crate::assets::Assets, map: &crate::map::Map, cell: [f32; 2], size: [u32; 2]) -> RgbaImage {
    let mut view = compose(assets, map, cell, size);
    grade(&mut view);
    view
}

fn compose(assets: &mut crate::assets::Assets, map: &crate::map::Map, cell: [f32; 2], size: [u32; 2]) -> RgbaImage {
    let [width, height] = size;
    let centre = map.project(cell);
    let origin = [centre[0] - width as f32 * 0.5, centre[1] - height as f32 * 0.5];
    let mut view = RgbaImage::from_pixel(width, height, image::Rgba([6, 14, 20, 255]));
    for sprite in &map.sprites {
        let Some(frame) = sprite.frames.first() else {
            continue;
        };
        let x = sprite.origin[0] - origin[0];
        let y = sprite.origin[1] - origin[1];
        // Scenery can reach far from its origin; skip what cannot touch.
        if x > width as f32 || y > height as f32 || x < -1600.0 || y < -1600.0 {
            continue;
        }
        let Ok(tile) = assets.image(frame) else {
            continue;
        };
        if x + tile.width() as f32 <= 0.0 || y + tile.height() as f32 <= 0.0 {
            continue;
        }
        image::imageops::overlay(&mut view, &tile, x as i64, y as i64);
    }
    view
}

/// Night grade: darker, a little desaturated, shadows pushed to teal and
/// highlights kept warm, so gold panels stand out on it.
fn grade(view: &mut RgbaImage) {
    for pixel in view.pixels_mut() {
        let [r, g, b, _] = pixel.0.map(f32::from);
        let luminance = (r * 0.3 + g * 0.59 + b * 0.11) / 255.0;
        let grey = luminance * 255.0;
        let desaturate = |channel: f32| channel * 0.82 + grey * 0.18;
        let shadow = (1.0 - luminance).powi(2);
        let light = luminance.powi(2);
        let graded = [
            desaturate(r) * 0.46 + light * 18.0,
            desaturate(g) * 0.48 + shadow * 9.0 + light * 10.0,
            desaturate(b) * 0.52 + shadow * 20.0,
        ];
        pixel.0 = [
            graded[0].clamp(0.0, 255.0) as u8,
            graded[1].clamp(0.0, 255.0) as u8,
            graded[2].clamp(0.0, 255.0) as u8,
            255,
        ];
    }
}
