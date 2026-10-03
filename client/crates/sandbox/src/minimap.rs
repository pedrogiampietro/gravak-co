// Source: @digitalm1nd on x.com / _dek on Discord — Preservation Conquer project — https://discord.gg/CvKPXEHYRY
use crate::{
    assets::{Assets, Result},
    map::Map,
};
use sandbox_data::ini::SectionCatalog;

pub struct Quad {
    pub texture: &'static str,
    pub rect: [f32; 4],
    pub uv: [f32; 4],
}

pub struct Minimap {
    pub images: Vec<(&'static str, image::RgbaImage)>,
    size: [f32; 2],
    single: bool,
    /// The big map over most of the screen instead of the corner one.
    expanded: bool,
    /// Index into [`ZOOMS`].
    zoom: usize,
}

/// A teammate's minimap pin and who it is, shown on hover.
pub struct TeamPin {
    pub position: [f32; 2],
    pub name: String,
}

/// Magnification over the map's own raster (corner) or over the whole map
/// fitted to the screen (expanded).
const ZOOMS: [f32; 5] = [0.5, 0.75, 1.0, 1.5, 2.0];
const DEFAULT_ZOOM: usize = 2;
const CORNER_SIZE: [f32; 2] = [170.0, 128.0];
/// Space left around the expanded map (left, top, right, bottom); the
/// bottom keeps the HUD clear.
const EXPANDED_MARGIN: [f32; 4] = [40.0, 40.0, 40.0, 150.0];
const BUTTON_STEP: f32 = 20.0;
const BUTTON_SIZE: f32 = 16.0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Button {
    ZoomIn,
    ZoomOut,
    Expand,
}

const BUTTONS: [Button; 3] = [Button::ZoomIn, Button::ZoomOut, Button::Expand];

impl Minimap {
    pub fn load(assets: &mut Assets, map_id: u32) -> Result<Option<Self>> {
        let catalog = SectionCatalog::parse(&assets.read("ani/MiniMap.Ani")?);
        let Some(section) = catalog.section(&map_id.to_string()) else {
            return Ok(None);
        };
        let count: usize = section.first("FrameAmount").unwrap_or("1").parse()?;
        if count == 0 {
            return Ok(None);
        }
        if count != 1 && count != 4 {
            return Err("Unsupported minimap frame count".into());
        }
        let frames = (0..count)
            .map(|index| {
                assets.image(
                    section
                        .first(&format!("Frame{index}"))
                        .ok_or("Missing minimap frame")?,
                )
            })
            .collect::<Result<Vec<_>>>()?;
        let full = stitch(&frames);
        let size = [full.width() as f32, full.height() as f32];
        // The corner map lets the world show through; the expanded one is
        // opaque so it reads over anything.
        let mut crop = full.clone();
        for pixel in crop.pixels_mut() {
            pixel[3] = (u16::from(pixel[3]) * 170 / 255) as u8;
        }
        let hero = catalog
            .section("hero")
            .and_then(|s| s.first("Frame0"))
            .ok_or("Missing minimap hero")?;
        let mut images = vec![
            ("@minimap-crop", crop),
            ("@minimap-full", full),
            ("@minimap-hero", assets.image(hero)?),
        ];
        for (key, color) in PIN_KEYS.into_iter().zip(PIN_COLORS) {
            images.push((key, pin_image(color)));
        }
        let controls = SectionCatalog::parse(&assets.read("ani/Control.Ani")?);
        for (section, key) in [
            ("Button00", "@minimap-plus"),
            ("Button01", "@minimap-minus"),
        ] {
            let section = controls.section(section).ok_or("Missing minimap control")?;
            images.push((
                key,
                assets.image(
                    section
                        .first("Frame0")
                        .ok_or("Missing minimap control frame")?,
                )?,
            ));
        }
        // The original third control (FullBtn) is another magnifier; one
        // holding a window glyph reads as "expand".
        let magnifier = &images.iter().find(|(key, _)| *key == "@minimap-plus").unwrap().1;
        images.push(("@minimap-expand", expand_button_image(magnifier)));
        Ok(Some(Self {
            images,
            size,
            single: count == 1,
            expanded: false,
            zoom: DEFAULT_ZOOM,
        }))
    }

    /// The bottom edge of the corner map, for what sits under it.
    pub fn bottom(&self, viewport: [f32; 2]) -> f32 {
        let [_, top, _, height] = self.frame(viewport);
        if self.expanded { 0.0 } else { top + height }
    }

    /// The box the map is drawn in; its buttons sit on its top right.
    fn frame(&self, viewport: [f32; 2]) -> [f32; 4] {
        if self.expanded {
            let [left, top, right, bottom] = EXPANDED_MARGIN;
            return [
                left,
                top,
                (viewport[0] - left - right).max(64.0),
                (viewport[1] - top - bottom).max(64.0),
            ];
        }
        let size = if self.single { self.size } else { CORNER_SIZE };
        [viewport[0] - size[0], 0.0, size[0], size[1]]
    }

    /// Where the map is drawn on screen and which part of the raster it
    /// shows. Zoomed out past the whole map, the drawn rect shrinks to it.
    fn view(
        &self,
        map: &Map,
        player: [f32; 2],
        camera: [f32; 2],
        viewport: [f32; 2],
    ) -> ([f32; 4], [f32; 4]) {
        let frame = self.frame(viewport);
        let base = if self.expanded {
            (frame[2] / self.size[0]).min(frame[3] / self.size[1])
        } else {
            1.0
        };
        let scale = base * ZOOMS[self.zoom];
        let center = if self.expanded {
            project(map, player, self.size)
        } else {
            let screen_center =
                map.unproject([camera[0] + viewport[0] * 0.5, camera[1] + viewport[1] * 0.5]);
            project(map, screen_center, self.size)
        };
        let source = crop(center, self.size, [frame[2] / scale, frame[3] / scale]);
        let size = [source[2] * scale, source[3] * scale];
        let rect = [
            frame[0] + (frame[2] - size[0]) * if self.expanded { 0.5 } else { 1.0 },
            frame[1] + if self.expanded { (frame[3] - size[1]) * 0.5 } else { 0.0 },
            size[0],
            size[1],
        ];
        (rect, source)
    }

    fn button_rect(&self, index: usize, viewport: [f32; 2]) -> [f32; 4] {
        let frame = self.frame(viewport);
        let first = frame[0] + frame[2] - BUTTON_STEP * BUTTONS.len() as f32;
        [
            first + index as f32 * BUTTON_STEP,
            frame[1] + 4.0,
            BUTTON_SIZE,
            BUTTON_SIZE,
        ]
    }

    /// A press on the map: its buttons act, and any press on the map is
    /// taken so it does not walk the hero.
    pub fn pointer(&mut self, position: [f32; 2], viewport: [f32; 2]) -> bool {
        let hit = (0..BUTTONS.len()).find(|index| inside(self.button_rect(*index, viewport), position));
        match hit.map(|index| BUTTONS[index]) {
            Some(Button::ZoomIn) => self.zoom = (self.zoom + 1).min(ZOOMS.len() - 1),
            Some(Button::ZoomOut) => self.zoom = self.zoom.saturating_sub(1),
            Some(Button::Expand) => {
                self.expanded = !self.expanded;
                self.zoom = DEFAULT_ZOOM;
            }
            None => return inside(self.frame(viewport), position),
        }
        true
    }

    /// Closes the expanded map; false when it was not open.
    pub fn collapse(&mut self) -> bool {
        if !self.expanded {
            return false;
        }
        self.expanded = false;
        self.zoom = DEFAULT_ZOOM;
        true
    }

    fn to_screen(&self, map: &Map, position: [f32; 2], rect: [f32; 4], source: [f32; 4]) -> [f32; 2] {
        let point = project(map, position, self.size);
        [
            rect[0] + (point[0] - source[0]) * rect[2] / source[2],
            rect[1] + (point[1] - source[1]) * rect[3] / source[3],
        ]
    }

    fn pin_rect(tip: [f32; 2]) -> [f32; 4] {
        [
            tip[0] - PIN_SIZE[0] as f32 * 0.5,
            tip[1] - PIN_SIZE[1] as f32,
            PIN_SIZE[0] as f32,
            PIN_SIZE[1] as f32,
        ]
    }

    /// The teammate pin under `cursor`, topmost first.
    pub fn pin_at(
        &self,
        map: &Map,
        player: [f32; 2],
        team: &[TeamPin],
        camera: [f32; 2],
        viewport: [f32; 2],
        cursor: [f32; 2],
    ) -> Option<usize> {
        let (rect, source) = self.view(map, player, camera, viewport);
        if !inside(rect, cursor) {
            return None;
        }
        (0..team.len()).rev().find(|index| {
            let tip = self.to_screen(map, team[*index].position, rect, source);
            inside(Self::pin_rect(tip), cursor)
        })
    }

    pub fn quads(
        &self,
        map: &Map,
        player: [f32; 2],
        team: &[TeamPin],
        camera: [f32; 2],
        viewport: [f32; 2],
    ) -> Vec<Quad> {
        let (rect, source) = self.view(map, player, camera, viewport);
        let mut quads = vec![Quad {
            texture: if self.expanded {
                "@minimap-full"
            } else {
                "@minimap-crop"
            },
            rect,
            uv: [
                source[0] / self.size[0],
                source[1] / self.size[1],
                source[2] / self.size[0],
                source[3] / self.size[1],
            ],
        }];
        // One pin per teammate, its tip on the member's cell; the hero's own
        // marker is drawn over them.
        for (index, pin) in team.iter().enumerate() {
            let tip = self.to_screen(map, pin.position, rect, source);
            if let Some(quad) = clipped(PIN_KEYS[index % PIN_KEYS.len()], Self::pin_rect(tip), rect) {
                quads.push(quad);
            }
        }
        let [x, y] = self.to_screen(map, player, rect, source);
        let hero = &self.images[2].1;
        let marker = [
            x - hero.width() as f32 * 0.5,
            y - hero.height() as f32 * 0.5,
            hero.width() as f32,
            hero.height() as f32,
        ];
        if let Some(quad) = clipped("@minimap-hero", marker, rect) {
            quads.push(quad);
        }
        for (index, button) in BUTTONS.into_iter().enumerate() {
            let key = match button {
                Button::ZoomIn => "@minimap-plus",
                Button::ZoomOut => "@minimap-minus",
                Button::Expand => "@minimap-expand",
            };
            let image = &self.images.iter().find(|(name, _)| *name == key).unwrap().1;
            let [x, y, _, _] = self.button_rect(index, viewport);
            quads.push(Quad {
                texture: key,
                rect: [x, y, image.width() as f32, image.height() as f32],
                uv: [0.0, 0.0, 1.0, 1.0],
            });
        }
        quads
    }
}

fn inside(rect: [f32; 4], point: [f32; 2]) -> bool {
    point[0] >= rect[0]
        && point[0] < rect[0] + rect[2]
        && point[1] >= rect[1]
        && point[1] < rect[1] + rect[3]
}

fn stitch(frames: &[image::RgbaImage]) -> image::RgbaImage {
    if frames.len() == 1 {
        return frames[0].clone();
    }
    let left = frames[0].width().max(frames[2].width());
    let top = frames[0].height().max(frames[1].height());
    let mut image = image::RgbaImage::new(
        left + frames[1].width().max(frames[3].width()),
        top + frames[2].height().max(frames[3].height()),
    );
    for (frame, [x, y]) in frames
        .iter()
        .zip([[0, 0], [left, 0], [0, top], [left, top]])
    {
        image::imageops::replace(&mut image, frame, x.into(), y.into());
    }
    image
}

fn project(map: &Map, cell: [f32; 2], raster: [f32; 2]) -> [f32; 2] {
    project_cell(cell, [map.data.width, map.data.height], map.extent, raster)
}

fn project_cell(cell: [f32; 2], grid: [u32; 2], drawable: [f32; 2], raster: [f32; 2]) -> [f32; 2] {
    let world = [
        (cell[0] - cell[1]) * 32.0 + grid[0] as f32 * 32.0,
        (cell[0] + cell[1]) * 16.0 + 16.0,
    ];
    let origin = [
        grid[0] as f32 * 32.0 - drawable[0] * 0.5,
        grid[1] as f32 * 16.0 - drawable[1] * 0.5 + 16.0
            - if grid[1].is_multiple_of(2) { 16.0 } else { 0.0 },
    ];
    [
        (world[0] - origin[0]) / drawable[0] * raster[0],
        (world[1] - origin[1]) / drawable[1] * raster[1],
    ]
}

fn crop(center: [f32; 2], raster: [f32; 2], destination: [f32; 2]) -> [f32; 4] {
    let width = destination[0].min(raster[0]);
    let height = destination[1].min(raster[1]);
    [
        (center[0] - width * 0.5).clamp(0.0, raster[0] - width),
        (center[1] - height * 0.5).clamp(0.0, raster[1] - height),
        width,
        height,
    ]
}

const PIN_SIZE: [u32; 2] = [13, 19];
const PIN_KEYS: [&str; 5] = [
    "@minimap-pin-0",
    "@minimap-pin-1",
    "@minimap-pin-2",
    "@minimap-pin-3",
    "@minimap-pin-4",
];
/// One colour per teammate, in team order: red, blue, green, orange, purple.
const PIN_COLORS: [[u8; 3]; 5] = [
    [226, 48, 48],
    [52, 120, 230],
    [60, 180, 75],
    [240, 150, 30],
    [160, 80, 210],
];

/// A map pin: a round head with a white dot, narrowing to a point at the
/// bottom centre, outlined in dark. Drawn with 4x4 supersampling so the
/// small sprite keeps smooth edges.
fn pin_image(color: [u8; 3]) -> image::RgbaImage {
    let [width, height] = PIN_SIZE;
    let center = [width as f32 * 0.5, 6.5];
    let radius = 6.0;
    let tip = [width as f32 * 0.5, height as f32];
    // Signed distance to the pin shape: the head circle joined to the
    // triangle that runs from its sides down to the tip.
    let distance = |x: f32, y: f32| {
        let circle = ((x - center[0]).powi(2) + (y - center[1]).powi(2)).sqrt() - radius;
        if y <= center[1] {
            return circle;
        }
        let along = (y - center[1]) / (tip[1] - center[1]);
        let half_width = radius * (1.0 - along) * 0.92;
        circle.min((x - center[0]).abs() - half_width)
    };
    image::RgbaImage::from_fn(width, height, |px, py| {
        let (mut fill, mut edge, mut dot) = (0.0, 0.0, 0.0);
        for sy in 0..4 {
            for sx in 0..4 {
                let x = px as f32 + (sx as f32 + 0.5) / 4.0;
                let y = py as f32 + (sy as f32 + 0.5) / 4.0;
                let d = distance(x, y);
                if d <= 0.0 {
                    if d > -1.2 {
                        edge += 1.0;
                    } else if ((x - center[0]).powi(2) + (y - center[1]).powi(2)).sqrt() <= 2.2 {
                        dot += 1.0;
                    } else {
                        fill += 1.0;
                    }
                }
            }
        }
        let total = fill + edge + dot;
        if total == 0.0 {
            return image::Rgba([0, 0, 0, 0]);
        }
        let mix = |channel: usize| {
            ((f32::from(color[channel]) * fill + 30.0 * edge + 255.0 * dot) / total) as u8
        };
        image::Rgba([mix(0), mix(1), mix(2), (total / 16.0 * 255.0) as u8])
    })
}

/// The expand control, cut from the zoom-in magnifier so the three match:
/// the "+" is wiped from the lens and a navy window glyph (a frame with a
/// thick title bar) takes its place.
fn expand_button_image(magnifier: &image::RgbaImage) -> image::RgbaImage {
    const NAVY: image::Rgba<u8> = image::Rgba([16, 40, 98, 255]);
    let mut image = magnifier.clone();
    for y in 3..12 {
        for x in 3..12 {
            let pixel = image.get_pixel(x, y);
            if pixel[0] == 16 && pixel[1] == 40 && (90..=98).contains(&pixel[2]) {
                // The lens glass, lighter towards the lit top left.
                let glass = if x + y < 15 { [172, 186, 197] } else { [147, 162, 175] };
                image.put_pixel(x, y, image::Rgba([glass[0], glass[1], glass[2], 255]));
            }
        }
    }
    for y in 4..=10 {
        for x in 4..=10 {
            if x == 4 || x == 10 || y <= 5 || y == 10 {
                image.put_pixel(x, y, NAVY);
            }
        }
    }
    image
}

fn clipped(texture: &'static str, rect: [f32; 4], bounds: [f32; 4]) -> Option<Quad> {
    let left = rect[0].max(bounds[0]);
    let top = rect[1].max(bounds[1]);
    let right = (rect[0] + rect[2]).min(bounds[0] + bounds[2]);
    let bottom = (rect[1] + rect[3]).min(bounds[1] + bounds[3]);
    if right <= left || bottom <= top {
        return None;
    }
    Some(Quad {
        texture,
        rect: [left, top, right - left, bottom - top],
        uv: [
            (left - rect[0]) / rect[2],
            (top - rect[1]) / rect[3],
            (right - left) / rect[2],
            (bottom - top) / rect[3],
        ],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn twin_city_projection_matches_client() {
        let point = project_cell(
            [425.0, 367.0],
            [972, 972],
            [26368.0, 17920.0],
            [753.0, 512.0],
        );
        assert!((point[0] - 429.5024).abs() < 0.001);
        assert!((point[1] - 174.1714).abs() < 0.001);
    }
    #[test]
    fn frames_are_row_major_with_unequal_widths() {
        let frames = [377, 376, 377, 376]
            .into_iter()
            .enumerate()
            .map(|(i, w)| image::RgbaImage::from_pixel(w, 256, image::Rgba([i as u8, 0, 0, 255])))
            .collect::<Vec<_>>();
        let image = stitch(&frames);
        assert_eq!(image.dimensions(), (753, 512));
        for (i, [x, y]) in [[0, 0], [377, 0], [0, 256], [377, 256]]
            .into_iter()
            .enumerate()
        {
            assert_eq!(image.get_pixel(x, y)[0], i as u8);
        }
    }
    #[test]
    fn crop_clamps_but_does_not_pin_markers() {
        assert_eq!(
            crop([0.0, 0.0], [753.0, 512.0], [170.0, 128.0]),
            [0.0, 0.0, 170.0, 128.0]
        );
        assert!(clipped("hero", [-30.0, 0.0, 16.0, 16.0], [0.0, 0.0, 170.0, 128.0]).is_none());
        assert_eq!(
            clipped("hero", [-8.0, 0.0, 16.0, 16.0], [0.0, 0.0, 170.0, 128.0])
                .unwrap()
                .uv,
            [0.5, 0.0, 0.5, 1.0]
        );
    }
}
