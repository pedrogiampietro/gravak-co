//! The Rifts' crypts (`content/rifts.toml`): maps of our own, a labyrinth of
//! narrow corridors laid out cell by cell, with our own art
//! (`content/art/cripta`).
//!
//! A maze (depth first, a few loops) of `rooms` across and down decides the
//! layout: each of its rooms is five cells of floor, the walls between them
//! two cells thick; an opening carves the wall across the corridor's width.
//! A few pairs of rooms become chambers, and the rooms around the one
//! farthest from the entrance the Guardian's hall.
//!
//! The art:
//! - the floor: the textures seen from above (`chao-*.png`), laid on the
//!   map's isometric ground and cut into the puzzle's 256 pixel tiles; the
//!   ground is periodic, so a handful of tiles cover every map;
//! - the walls: a block per wall cell along the floor, its faces of
//!   `parede-face.png` and its top of `parede-topo.png`, drawn as one of
//!   sixteen pictures by where it stands (the texture runs on from block to
//!   block);
//! - the decorations (`tocha`, `pilar`, `bau`...): placed by the layout,
//!   some closing their cell.
//!
//! The build writes, for each rift, `map/puzzle/gravak-rift-<id>.pul`,
//! `map/map/gravak-rift-<id>.DMap`, the map in `ini/GameMap.dat`, its
//! minimap in `ani/MiniMap.Ani`, its walls and decorations as Gravak objects
//! (`gravak/maps/<id>.txt`), and `gravak/rifts/<id>.txt`: where the server
//! spawns (each room's cells).

use crate::Result;
use crate::assets::Installation;
use image::{Rgba, RgbaImage, imageops};
use sandbox_data::map::{GameMapCatalog, GameMapRecord, GravakObject, GravakObjects, MapCell};
use serde::Deserialize;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::Path;

pub const CONFIG: &str = "rifts.toml";
const TILE: u32 = 256;
/// A room's floor and a wall's thickness, in cells.
const CORRIDOR: usize = 5;
const WALL: usize = 2;
const PERIOD: usize = CORRIDOR + WALL;
/// Cells of wall around the maze.
const MARGIN: usize = 9;
/// Wall blocks stand on wall cells this near the floor: the inner walls
/// whole, and around the maze a border that hides the floor's tiles past it.
const WALL_REACH: i64 = 8;
/// How tall a wall block stands, in pixels: taller than a hero. What lies
/// past the walls stays dark until the hero sees into it (the client's
/// `reveal`), so the walls need not hide it.
const WALL_HEIGHT: u32 = 136;
/// The wall's face texture shows this much of its height per 72 pixels.
const FACE_SPAN: f32 = 0.45;

const FLOOR: MapCell = MapCell { access: 0, surface: 12, elevation: 0 };
const BLOCKED: MapCell = MapCell { access: 1, surface: 12, elevation: 0 };

/// A theme's puzzle catalogue, floor tiles and objects.
fn puzzle_ani(theme: &str) -> String {
    format!("ani/gravak-{theme}.ani")
}

fn tile_dir(theme: &str) -> String {
    format!("data/map/puzzle/gravak-{theme}")
}

fn object_dir(theme: &str) -> String {
    format!("gravak/objects/{theme}")
}
const MINIMAP_ANI: &str = "ani/MiniMap.Ani";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default)]
    pub rift: Vec<RiftDef>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RiftDef {
    pub id: u32,
    pub name: String,
    pub seed: u64,
    /// Rooms of the maze across and down.
    pub rooms: [usize; 2],
    /// The art: a theme of `THEMES` (`content/art/<theme>`).
    #[serde(default = "default_theme")]
    pub theme: String,
}

fn default_theme() -> String {
    "cripta".to_owned()
}

pub fn load(content: &Path) -> Result<Config> {
    let path = content.join(CONFIG);
    if !path.exists() {
        return Ok(Config { rift: Vec::new() });
    }
    let text = std::fs::read_to_string(&path)?;
    Ok(toml::from_str(&text).map_err(|error| format!("{}: {error}", path.display()))?)
}

/// A decoration: its picture, how tall it stands in the game, and whether it
/// closes its cell.
struct Prop {
    name: &'static str,
    height: u32,
    blocks: bool,
}

const fn prop(name: &'static str, height: u32, blocks: bool) -> Prop {
    Prop { name, height, blocks }
}

/// A look for the crypts: its art (`content/art/<name>`), how big its
/// textures are on the ground, its decorations, and where each goes.
pub struct Theme {
    pub name: &'static str,
    props: &'static [Prop],
    /// World units the floor texture covers (a cell is 32).
    floor_period: f32,
    /// Cells the wall texture covers across.
    wall_period: usize,
    /// How bright the walls' faces are (a dark texture lit up).
    wall_light: f32,
    /// On the walls the camera sees, every few cells.
    wall_lights: &'static [&'static str],
    /// On some of those walls, now and then.
    wall_decor: &'static [&'static str],
    /// At the Guardian's hall's corners, and two at its far side.
    hall_corners: &'static str,
    hall_far: &'static str,
    /// Along a chamber's old wall line, and in a great room's middle.
    pillars: &'static [&'static str],
    /// What a dead end keeps.
    dead_ends: &'static [&'static str],
    /// Strewn on the floor, never in the way.
    strewn: &'static [&'static str],
}

pub const THEMES: [Theme; 2] = [
    Theme {
        name: "cripta",
        props: &[
            prop("tocha", 64, false),
            prop("pilar", 170, true),
            prop("pilar-quebrado", 110, true),
            prop("ossos", 46, false),
            prop("bau", 62, true),
            prop("caixotes", 96, true),
            prop("correntes", 140, true),
            prop("estatua", 175, true),
            prop("braseiro", 92, true),
            prop("teia", 96, false),
        ],
        floor_period: 256.0,
        wall_period: 4,
        wall_light: 1.0,
        wall_lights: &["tocha"],
        wall_decor: &["teia"],
        hall_corners: "braseiro",
        hall_far: "estatua",
        pillars: &["pilar", "pilar", "pilar-quebrado"],
        dead_ends: &["bau", "caixotes", "correntes", "ossos"],
        strewn: &["ossos"],
    },
    Theme {
        name: "fungos",
        props: &[
            prop("tocha", 66, false),
            prop("lanterna", 70, false),
            prop("estandarte", 110, false),
            prop("raizes", 110, false),
            prop("cogumelos", 64, false),
            prop("cogumelo-gigante", 175, true),
            prop("fogueira", 72, true),
            prop("barricada", 78, true),
            prop("totem", 170, true),
            prop("sacos", 58, true),
            prop("bau", 60, true),
            prop("ossos", 40, false),
            prop("barril", 76, true),
            prop("pedras", 62, true),
        ],
        floor_period: 512.0,
        wall_period: 6,
        wall_light: 1.5,
        wall_lights: &["tocha", "lanterna"],
        wall_decor: &["raizes", "estandarte", "raizes"],
        hall_corners: "cogumelo-gigante",
        hall_far: "totem",
        pillars: &["cogumelo-gigante", "pedras", "barricada", "fogueira"],
        dead_ends: &["bau", "sacos", "barril", "ossos", "sacos"],
        strewn: &["cogumelos", "ossos", "cogumelos"],
    },
];

pub fn theme(name: &str) -> Result<&'static Theme> {
    THEMES
        .iter()
        .find(|theme| theme.name == name)
        .ok_or_else(|| format!("tema {name:?} desconhecido (há: cripta, fungos)").into())
}

impl Theme {
    fn prop(&self, name: &str) -> &Prop {
        self.props.iter().find(|prop| prop.name == name).expect("a prop of the theme")
    }
}

/// A picture on a plain white or light grey background, the background made
/// clear (with a soft edge) and cropped to what is left; one already clear
/// is only cropped (`content/art/tirar_fundo.py` does the same).
fn cut_out(picture: RgbaImage) -> RgbaImage {
    let opaque = picture.pixels().all(|pixel| pixel[3] >= 250);
    let mut clear = picture;
    if opaque {
        for pixel in clear.pixels_mut() {
            let [r, g, b, _] = pixel.0.map(f32::from);
            let (low, high) = (r.min(g).min(b), r.max(g).max(b));
            pixel[3] = if high - low >= 30.0 {
                255
            } else if low > 205.0 {
                0
            } else {
                (((235.0 - low) / 30.0).clamp(0.0, 1.0) * 255.0) as u8
            };
        }
    }
    let (mut x0, mut y0, mut x1, mut y1) = (clear.width(), clear.height(), 0, 0);
    for (x, y, pixel) in clear.enumerate_pixels() {
        if pixel[3] > 8 {
            (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x + 1), y1.max(y + 1));
        }
    }
    if x1 <= x0 || y1 <= y0 {
        return clear;
    }
    imageops::crop_imm(&clear, x0, y0, x1 - x0, y1 - y0).to_image()
}

/// A theme's art, read from `content/art/<theme>`.
pub struct Art {
    floors: Vec<RgbaImage>,
    face: RgbaImage,
    top: RgbaImage,
    props: HashMap<&'static str, RgbaImage>,
}

impl Art {
    pub fn load(content: &Path, theme: &Theme) -> Result<Self> {
        let directory = content.join("art").join(theme.name);
        let open = |name: &str| -> Result<RgbaImage> {
            let path = directory.join(format!("{name}.png"));
            Ok(image::open(&path).map_err(|error| format!("{}: {error}", path.display()))?.to_rgba8())
        };
        let mut floors = vec![open("chao-1")?];
        for name in ["chao-2", "chao-3"] {
            if directory.join(format!("{name}.png")).exists() {
                floors.push(open(name)?);
            }
        }
        let mut props = HashMap::new();
        for prop in theme.props {
            let picture = cut_out(open(prop.name)?);
            let width = (picture.width() * prop.height / picture.height().max(1)).max(1);
            props.insert(prop.name, imageops::resize(&picture, width, prop.height, imageops::FilterType::Lanczos3));
        }
        Ok(Self { floors, face: open("parede-face")?, top: open("parede-topo")?, props })
    }
}

/// `texture` at (u, v) in its own pixels, repeating, bilinear.
fn sample(texture: &RgbaImage, u: f32, v: f32) -> [f32; 3] {
    let (w, h) = (texture.width() as f32, texture.height() as f32);
    let (u, v) = (u.rem_euclid(w), v.rem_euclid(h));
    let (x0, y0) = (u.floor(), v.floor());
    let (fx, fy) = (u - x0, v - y0);
    let at = |x: f32, y: f32| {
        let pixel = texture.get_pixel((x as u32) % texture.width(), (y as u32) % texture.height()).0;
        [f32::from(pixel[0]), f32::from(pixel[1]), f32::from(pixel[2])]
    };
    let (a, b, c, d) = (at(x0, y0), at(x0 + 1.0, y0), at(x0, y0 + 1.0), at(x0 + 1.0, y0 + 1.0));
    std::array::from_fn(|i| (a[i] * (1.0 - fx) + b[i] * fx) * (1.0 - fy) + (c[i] * (1.0 - fx) + d[i] * fx) * fy)
}

fn pixel(colour: [f32; 3], light: f32, alpha: u8) -> Rgba<u8> {
    Rgba([
        (colour[0] * light).clamp(0.0, 255.0) as u8,
        (colour[1] * light).clamp(0.0, 255.0) as u8,
        (colour[2] * light).clamp(0.0, 255.0) as u8,
        alpha,
    ])
}

/// The ground under a screen point of the map's picture, in world units (a
/// cell is 32 along each axis).
fn world(x: f32, y: f32) -> (f32, f32) {
    (y + x / 2.0, y - x / 2.0)
}

/// The floor's tiles: the ground repeats every two tiles across and every
/// tile down, so each texture gives two; the variations blend a second
/// texture into a tile's middle, leaving its edges to the first.
fn floor_tiles(art: &Art, period: f32) -> Vec<RgbaImage> {
    let scale = |texture: &RgbaImage| texture.width() as f32 / period;
    let mut tiles = Vec::new();
    for variant in 0..art.floors.len() {
        for column in 0..2u32 {
            tiles.push(RgbaImage::from_fn(TILE, TILE, |x, y| {
                let (u, v) = world((column * TILE + x) as f32, y as f32);
                let base = &art.floors[0];
                let mut colour = sample(base, u * scale(base), v * scale(base));
                if variant > 0 {
                    let other = &art.floors[variant];
                    let blend = {
                        let (dx, dy) = (x as f32 / TILE as f32 - 0.5, y as f32 / TILE as f32 - 0.5);
                        (1.0 - (dx.hypot(dy) / 0.45)).clamp(0.0, 1.0).powf(0.6)
                    };
                    let second = sample(other, u * scale(other), v * scale(other));
                    colour = std::array::from_fn(|i| colour[i] * (1.0 - blend) + second[i] * blend);
                }
                pixel(colour, 0.9, 255)
            }));
        }
    }
    // The dark beyond the crypt.
    tiles.push(RgbaImage::from_pixel(TILE, TILE, Rgba([6, 7, 10, 255])));
    tiles
}

/// A wall block for the cell at (`x`, `y`) of the texture's period: the
/// cell's diamond (64 by 32) at its foot, raised `WALL_HEIGHT`. The image's
/// point (32, 16 + WALL_HEIGHT) is the cell's.
fn wall_block(art: &Art, period: usize, brightness: f32, x: usize, y: usize) -> RgbaImage {
    let height = WALL_HEIGHT as f32;
    let (fw, fh) = (art.face.width() as f32, art.face.height() as f32);
    let (tw, th) = (art.top.width() as f32, art.top.height() as f32);
    let face_scale = fw / (period as f32);
    RgbaImage::from_fn(64, 32 + WALL_HEIGHT, |ix, iy| {
        let px = ix as f32 + 0.5 - 32.0;
        let py = iy as f32 + 0.5 - 16.0 - height;
        // The top: the cell's diamond raised.
        let (tx, ty) = (px, py + height);
        if tx.abs() / 32.0 + ty.abs() / 16.0 <= 1.0 {
            let along_x = (tx / 32.0 + ty / 16.0) / 2.0 + 0.5;
            let along_y = (ty / 16.0 - tx / 32.0) / 2.0 + 0.5;
            let u = (x as f32 + along_x) / period as f32 * tw;
            let v = (y as f32 + along_y) / period as f32 * th;
            let rim = 1.0 - (tx.abs() / 32.0 + ty.abs() / 16.0);
            let light = if rim < 0.06 { 0.6 } else { 1.0 };
            return pixel(sample(&art.top, u, v), light, 255);
        }
        if (-32.0..=0.0).contains(&px) {
            // The face toward +y, along x: from the left corner to the foot.
            let foot = 16.0 + px / 2.0;
            if py <= foot && py >= foot - height {
                let rise = (foot - py) / height;
                let along = (px + 32.0) / 32.0;
                let u = (x as f32 + along) * face_scale;
                let v = (1.0 - rise) * fh * FACE_SPAN * height / 72.0;
                let light = brightness * 0.82 * (0.62 + 0.38 * rise.min(1.0)) * if px > -1.0 { 0.8 } else { 1.0 };
                return pixel(sample(&art.face, u, v), light, 255);
            }
        } else if (0.0..=32.0).contains(&px) {
            // The face toward +x, along y.
            let foot = 16.0 - px / 2.0;
            if py <= foot && py >= foot - height {
                let rise = (foot - py) / height;
                let along = 1.0 - px / 32.0;
                let u = (y as f32 + along) * face_scale + fw * 0.37;
                let v = (1.0 - rise) * fh * FACE_SPAN * height / 72.0;
                let light = brightness * 0.55 * (0.62 + 0.38 * rise.min(1.0));
                return pixel(sample(&art.face, u, v), light, 255);
            }
        }
        Rgba([0, 0, 0, 0])
    })
}

/// A small deterministic generator (SplitMix64), so a seed makes the same
/// rift on every build.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }

    fn chance(&mut self, p: f64) -> bool {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64 <= p
    }
}

/// A wall between two rooms, by the room on its right or below.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Opening {
    /// Between room (x - 1, y) and (x, y).
    Across(usize, usize),
    /// Between room (x, y - 1) and (x, y).
    Down(usize, usize),
}

/// The openings of a maze of `width` by `height` rooms: a depth first
/// spanning tree from the entrance (0, 0), with `loops` of the other walls
/// opened.
fn maze(width: usize, height: usize, rng: &mut Rng, loops: f64) -> HashSet<Opening> {
    let mut open = HashSet::new();
    let mut seen = HashSet::from([(0, 0)]);
    let mut stack = vec![(0usize, 0usize)];
    while let Some(&(x, y)) = stack.last() {
        let next: Vec<(usize, usize)> = [(x + 1, y), (x.wrapping_sub(1), y), (x, y + 1), (x, y.wrapping_sub(1))]
            .into_iter()
            .filter(|&(nx, ny)| nx < width && ny < height && !seen.contains(&(nx, ny)))
            .collect();
        if next.is_empty() {
            stack.pop();
            continue;
        }
        let (nx, ny) = next[rng.below(next.len())];
        seen.insert((nx, ny));
        stack.push((nx, ny));
        open.insert(if nx != x { Opening::Across(x.max(nx), y) } else { Opening::Down(x, y.max(ny)) });
    }
    for x in 0..width {
        for y in 0..height {
            if y > 0 && rng.chance(loops) {
                open.insert(Opening::Down(x, y));
            }
            if x > 0 && rng.chance(loops) {
                open.insert(Opening::Across(x, y));
            }
        }
    }
    open
}

/// The rooms next to `room` through an opening.
fn neighbours(room: (usize, usize), size: [usize; 2], open: &HashSet<Opening>) -> Vec<(usize, usize)> {
    let (x, y) = room;
    let mut next = Vec::new();
    if x + 1 < size[0] && open.contains(&Opening::Across(x + 1, y)) {
        next.push((x + 1, y));
    }
    if x > 0 && open.contains(&Opening::Across(x, y)) {
        next.push((x - 1, y));
    }
    if y + 1 < size[1] && open.contains(&Opening::Down(x, y + 1)) {
        next.push((x, y + 1));
    }
    if y > 0 && open.contains(&Opening::Down(x, y)) {
        next.push((x, y - 1));
    }
    next
}

/// How many steps each room is from the entrance.
fn distances(size: [usize; 2], open: &HashSet<Opening>) -> HashMap<(usize, usize), usize> {
    let mut distance = HashMap::from([((0, 0), 0)]);
    let mut queue = VecDeque::from([(0usize, 0usize)]);
    while let Some(room) = queue.pop_front() {
        let steps = distance[&room] + 1;
        for next in neighbours(room, size, open) {
            distance.entry(next).or_insert_with(|| {
                queue.push_back(next);
                steps
            });
        }
    }
    distance
}

/// A rift built.
pub struct Rift {
    pub def: RiftDef,
    /// Cells across and down.
    pub size: u32,
    pub cells: Vec<MapCell>,
    /// The puzzle's tiles (indexes into the floor tiles).
    columns: u32,
    rows: u32,
    tiles: Vec<i16>,
    /// Walls and decorations: (cell x, cell y, picture, offset of its
    /// top-left from the cell's point).
    objects: Vec<(i32, i32, String, [i32; 2])>,
    /// The Guardian's hall: its top-left room.
    pub hall: (usize, usize),
    /// Each room's walkable cells; the hall's are under its top-left room.
    pub rooms: Vec<(usize, usize, Vec<(u32, u32)>)>,
}

/// Where a room's floor starts, in cells.
fn room_origin(room: (usize, usize)) -> (usize, usize) {
    (MARGIN + room.0 * PERIOD + WALL, MARGIN + room.1 * PERIOD + WALL)
}

pub fn generate(def: &RiftDef, theme: &Theme, floor_variants: usize) -> Result<Rift> {
    let [width, height] = def.rooms;
    if width < 4 || height < 4 {
        return Err(format!("rift {}: at least 4 by 4 rooms", def.id).into());
    }
    let size_rooms = [width, height];
    let mut rng = Rng(def.seed ^ u64::from(def.id).wrapping_mul(0x2545_F491_4F6C_DD1D));
    let mut open = maze(width, height, &mut rng, 0.1);
    let distance = distances(size_rooms, &open);
    let farthest = *distance.iter().max_by_key(|(room, steps)| (**steps, room.1, room.0)).expect("rooms").0;
    let hall = (farthest.0.min(width - 2).max(1), farthest.1.min(height - 2).max(1));
    let (hx, hy) = hall;
    open.extend([Opening::Across(hx + 1, hy), Opening::Across(hx + 1, hy + 1), Opening::Down(hx, hy + 1), Opening::Down(hx + 1, hy + 1)]);
    // Great rooms: a few blocks of two by two rooms opened into one, as the
    // hall is (its walls' crossing gone too).
    let mut blocks = vec![hall];
    for _ in 0..(width * height / 20) {
        let (x, y) = (rng.below(width - 1), rng.below(height - 1));
        let touches = |(bx, by): (usize, usize)| x + 2 > bx && bx + 2 > x && y + 2 > by && by + 2 > y;
        if (x, y) == (0, 0) || x + y == 0 || blocks.iter().any(|block| touches(*block)) || (x <= 1 && y <= 1) {
            continue;
        }
        open.extend([Opening::Across(x + 1, y), Opening::Across(x + 1, y + 1), Opening::Down(x, y + 1), Opening::Down(x + 1, y + 1)]);
        blocks.push((x, y));
    }
    let in_block = |room: (usize, usize)| blocks.iter().any(|(bx, by)| (*bx..bx + 2).contains(&room.0) && (*by..by + 2).contains(&room.1));
    // Chambers: a few pairs of rooms opened into one, with the wall between
    // them gone entirely.
    let mut chambers = HashSet::new();
    for _ in 0..(width * height / 12) {
        let (x, y) = (rng.below(width - 1), rng.below(height));
        let pair = [(x, y), (x + 1, y)];
        if pair.iter().any(|room| *room == (0, 0) || in_block(*room)) {
            continue;
        }
        open.insert(Opening::Across(x + 1, y));
        chambers.insert((x + 1, y));
    }

    // Cells: walls everywhere, then the floor carved.
    let span = MARGIN * 2 + width.max(height) * PERIOD + WALL;
    let size = span.div_ceil(8) * 8;
    let mut floor = vec![false; size * size];
    let mut carve = |x0: usize, y0: usize, x1: usize, y1: usize| {
        for y in y0..y1 {
            for x in x0..x1 {
                floor[y * size + x] = true;
            }
        }
    };
    for ry in 0..height {
        for rx in 0..width {
            let (x0, y0) = room_origin((rx, ry));
            carve(x0, y0, x0 + CORRIDOR, y0 + CORRIDOR);
            if rx > 0 && open.contains(&Opening::Across(rx, ry)) {
                let whole = chambers.contains(&(rx, ry)) || blocks.iter().any(|(bx, by)| bx + 1 == rx && (*by..by + 2).contains(&ry));
                let (from, to) = if whole { (y0, y0 + CORRIDOR) } else { (y0 + 1, y0 + CORRIDOR - 1) };
                carve(x0 - WALL, from, x0, to);
            }
            if ry > 0 && open.contains(&Opening::Down(rx, ry)) {
                let whole = blocks.iter().any(|(bx, by)| by + 1 == ry && (*bx..bx + 2).contains(&rx));
                let (from, to) = if whole { (x0, x0 + CORRIDOR) } else { (x0 + 1, x0 + CORRIDOR - 1) };
                carve(from, y0 - WALL, to, y0);
            }
        }
    }
    // A great room's middle: the walls' crossing goes too.
    for (bx, by) in &blocks {
        let (cx0, cy0) = room_origin((bx + 1, by + 1));
        carve(cx0 - WALL, cy0 - WALL, cx0, cy0);
    }
    let is_floor = |x: i64, y: i64| x >= 0 && y >= 0 && (x as usize) < size && (y as usize) < size && floor[y as usize * size + x as usize];

    let mut cells = vec![BLOCKED; size * size];
    for (index, cell) in cells.iter_mut().enumerate() {
        if floor[index] {
            *cell = FLOOR;
        }
    }

    // Decorations, closing their cells where they stand.
    let objects_dir = object_dir(theme.name);
    let mut objects = Vec::new();
    let place = |objects: &mut Vec<(i32, i32, String, [i32; 2])>, cells: &mut Vec<MapCell>, name: &str, x: usize, y: usize, nudge: [i32; 2]| {
        objects.push((x as i32, y as i32, format!("{objects_dir}/{name}.png"), nudge));
        if theme.prop(name).blocks {
            cells[y * size + x] = BLOCKED;
        }
    };
    let pick = |rng: &mut Rng, list: &[&'static str]| list[rng.below(list.len())];
    let centre = |room: (usize, usize)| {
        let (x0, y0) = room_origin(room);
        (x0 + CORRIDOR / 2, y0 + CORRIDOR / 2)
    };
    // The hall: its corners, and two at its far side.
    let (hx0, hy0) = room_origin(hall);
    let hall_end = (hx0 + 2 * CORRIDOR + WALL - 1, hy0 + 2 * CORRIDOR + WALL - 1);
    for (x, y) in [(hx0 + 1, hy0 + 1), (hall_end.0 - 1, hy0 + 1), (hx0 + 1, hall_end.1 - 1), (hall_end.0 - 1, hall_end.1 - 1)] {
        place(&mut objects, &mut cells, theme.hall_corners, x, y, [0, 0]);
    }
    for (x, y) in [(hall_end.0 - 1, hy0 + 4), (hall_end.0 - 1, hall_end.1 - 4)] {
        place(&mut objects, &mut cells, theme.hall_far, x, y, [0, 0]);
    }
    let in_hall = |room: (usize, usize)| (hx..hx + 2).contains(&room.0) && (hy..hy + 2).contains(&room.1);
    // The other great rooms: something in the middle, where the walls met.
    for (bx, by) in blocks.iter().skip(1) {
        let (cx0, cy0) = room_origin((bx + 1, by + 1));
        let name = pick(&mut rng, theme.pillars);
        place(&mut objects, &mut cells, name, cx0 - 1, cy0 - 1, [0, 0]);
    }
    for ry in 0..height {
        for rx in 0..width {
            let room = (rx, ry);
            if room == (0, 0) || in_hall(room) {
                continue;
            }
            let (x, y) = centre(room);
            let exits = neighbours(room, size_rooms, &open).len();
            if chambers.contains(&room) {
                // A chamber: two along the old wall's line.
                let (x0, y0) = room_origin(room);
                for py in [y0, y0 + CORRIDOR - 1] {
                    let name = pick(&mut rng, theme.pillars);
                    place(&mut objects, &mut cells, name, x0 - 1, py, [0, 0]);
                }
            } else if exits == 1 && !in_block(room) {
                // A dead end keeps something.
                let name = pick(&mut rng, theme.dead_ends);
                place(&mut objects, &mut cells, name, x, y, [0, 0]);
            }
            // Something strewn on the floor, in the way of nothing.
            if rng.chance(0.3) {
                let (dx, dy) = (rng.below(CORRIDOR), rng.below(CORRIDOR));
                let (x0, y0) = room_origin(room);
                let name = pick(&mut rng, theme.strewn);
                if !theme.prop(name).blocks && floor[(y0 + dy) * size + x0 + dx] {
                    place(&mut objects, &mut cells, name, x0 + dx, y0 + dy, [0, 0]);
                }
            }
        }
    }

    // Walls: a block on every wall cell beside the floor; lights and the
    // theme's wall decorations on some of the faces the camera sees.
    let near_floor = |x: usize, y: usize| {
        (-WALL_REACH..=WALL_REACH).any(|dy| (-WALL_REACH..=WALL_REACH).any(|dx| is_floor(x as i64 + dx, y as i64 + dy)))
    };
    let mut walls = Vec::new();
    for y in 0..size {
        for x in 0..size {
            if !floor[y * size + x] && near_floor(x, y) {
                walls.push((x, y));
            }
        }
    }
    for &(x, y) in &walls {
        let name = format!("{objects_dir}/parede-{}-{}.png", x % theme.wall_period, y % theme.wall_period);
        objects.push((x as i32, y as i32, name, [0, 0]));
        let faces_y = is_floor(x as i64, y as i64 + 1) && !is_floor(x as i64 - 1, y as i64 + 1);
        let faces_x = is_floor(x as i64 + 1, y as i64) && !is_floor(x as i64 + 1, y as i64 - 1);
        if !(faces_y || faces_x) {
            continue;
        }
        let side = if faces_y { -14 } else { 14 };
        if (x + 3 * y) % 9 == 0 {
            let light = pick(&mut rng, theme.wall_lights);
            objects.push((x as i32, y as i32, format!("{objects_dir}/{light}.png"), [side, -64]));
        } else if (x + 5 * y) % 7 == 0 && rng.chance(0.45) {
            let decor = pick(&mut rng, theme.wall_decor);
            objects.push((x as i32, y as i32, format!("{objects_dir}/{decor}.png"), [side, -40]));
        }
    }

    // Every room must still be reached from the entrance.
    let entrance = centre((0, 0));
    let reached = reach(&cells, size as u32, (entrance.0 as u32, entrance.1 as u32));
    let mut rooms = Vec::new();
    for ry in 0..height {
        for rx in 0..width {
            if in_hall((rx, ry)) && (rx, ry) != hall {
                continue;
            }
            let (x0, y0) = room_origin((rx, ry));
            let (x1, y1) = if (rx, ry) == hall { (hall_end.0 + 1, hall_end.1 + 1) } else { (x0 + CORRIDOR, y0 + CORRIDOR) };
            let mut room_cells = Vec::new();
            for y in y0..y1 {
                for x in x0..x1 {
                    if reached.contains(&(x as u32, y as u32)) {
                        room_cells.push((x as u32, y as u32));
                    }
                }
            }
            if room_cells.is_empty() {
                return Err(format!("rift {}: room {rx},{ry} cannot be reached", def.id).into());
            }
            rooms.push((rx, ry, room_cells));
        }
    }

    // The puzzle: the floor's tiles where the crypt is, the dark elsewhere.
    let (columns, rows) = ((size / 4) as u32, (size / 8) as u32);
    let picture = [columns * TILE, rows * TILE];
    let dark = (floor_variants * 2) as i16;
    let mut tiles = vec![dark; (columns * rows) as usize];
    for y in 0..size {
        for x in 0..size {
            if !floor[y * size + x] {
                continue;
            }
            let p = project(picture, size as u32, x as f32, y as f32);
            for (dx, dy) in [(-32.0, 0.0), (32.0, 0.0), (0.0, -16.0), (0.0, 16.0), (0.0, 0.0)] {
                let (tx, ty) = (((p[0] + dx) / TILE as f32) as i64, ((p[1] + dy) / TILE as f32) as i64);
                if tx < 0 || ty < 0 || tx >= i64::from(columns) || ty >= i64::from(rows) {
                    continue;
                }
                let index = (ty as u32 * columns + tx as u32) as usize;
                if tiles[index] == dark {
                    let variant = if rng.chance(0.7) || floor_variants == 1 { 0 } else { 1 + rng.below(floor_variants - 1) };
                    tiles[index] = (variant * 2 + (tx as usize & 1)) as i16;
                }
            }
        }
    }
    Ok(Rift {
        def: def.clone(),
        size: size as u32,
        cells,
        columns,
        rows,
        tiles,
        objects,
        hall,
        rooms,
    })
}

/// Where a cell's point is on a picture of `picture` pixels, as the client
/// lays it out (`sandbox_data::map::project`).
fn project(picture: [u32; 2], size: u32, x: f32, y: f32) -> [f32; 2] {
    [
        (x - y) * 32.0 + picture[0] as f32 * 0.5,
        (x + y - (size - 1) as f32) * 16.0 + picture[1] as f32 * 0.5,
    ]
}

fn reach(cells: &[MapCell], size: u32, from: (u32, u32)) -> HashSet<(u32, u32)> {
    let mut seen = HashSet::from([from]);
    let mut queue = VecDeque::from([from]);
    while let Some((x, y)) = queue.pop_front() {
        for (dx, dy) in [(-1, -1), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1)] {
            let (nx, ny) = (i64::from(x) + dx, i64::from(y) + dy);
            if nx < 0 || ny < 0 || nx >= i64::from(size) || ny >= i64::from(size) {
                continue;
            }
            let next = (nx as u32, ny as u32);
            if cells[(next.1 * size + next.0) as usize].access == 0 && seen.insert(next) {
                queue.push_back(next);
            }
        }
    }
    seen
}

fn padded(text: &str, length: usize) -> Vec<u8> {
    let mut bytes = text.as_bytes().to_vec();
    bytes.resize(length, 0);
    bytes
}

impl Rift {
    pub fn puzzle_path(&self) -> String {
        format!("map/puzzle/gravak-rift-{}.pul", self.def.id)
    }

    pub fn dmap_path(&self) -> String {
        format!("map/map/gravak-rift-{}.DMap", self.def.id)
    }

    pub fn puzzle_bytes(&self) -> Vec<u8> {
        let mut bytes = padded("PUZZLE2", 8);
        bytes.extend(padded(&puzzle_ani(&self.def.theme).replace('/', "\\"), 256));
        bytes.extend((self.columns as i32).to_le_bytes());
        bytes.extend((self.rows as i32).to_le_bytes());
        for tile in &self.tiles {
            bytes.extend(tile.to_le_bytes());
        }
        bytes.extend([0; 8]);
        bytes
    }

    pub fn dmap_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(self.cells.len() * 6 + 1024);
        bytes.extend(1004u32.to_le_bytes());
        bytes.extend(0u32.to_le_bytes());
        bytes.extend(padded(&self.puzzle_path().replace('/', "\\"), 260));
        bytes.extend(self.size.to_le_bytes());
        bytes.extend(self.size.to_le_bytes());
        for y in 0..self.size {
            let mut checksum: u32 = 0;
            for x in 0..self.size {
                let cell = self.cells[(y * self.size + x) as usize];
                bytes.extend(cell.access.to_le_bytes());
                bytes.extend(cell.surface.to_le_bytes());
                bytes.extend(cell.elevation.to_le_bytes());
                let (access, surface, elevation) = (i64::from(cell.access), i64::from(cell.surface), i64::from(cell.elevation));
                let (x, y) = (i64::from(x), i64::from(y));
                checksum = checksum
                    .wrapping_add((access * (surface + y + 1) + (elevation + 2) * (x + 1 + surface)) as u32);
            }
            bytes.extend(checksum.to_le_bytes());
        }
        // No portals, no objects.
        bytes.extend(0i32.to_le_bytes());
        bytes.extend(0i32.to_le_bytes());
        bytes
    }

    /// The walls and decorations as the client draws them; `sizes` gives
    /// each picture's size, to stand it on its cell.
    pub fn gravak_objects(&self, sizes: &HashMap<String, [u32; 2]>) -> GravakObjects {
        let mut objects = GravakObjects::default();
        for (x, y, image, nudge) in &self.objects {
            let [width, height] = sizes.get(image).copied().unwrap_or([64, 64]);
            // A wall block's foot is its diamond's bottom; a decoration's,
            // its picture's bottom middle, a little below the cell's point.
            let foot = if image.contains("/parede-") { 32 } else { 8 };
            objects.objects.push(GravakObject {
                x: *x,
                y: *y,
                offset: [-(width as i32) / 2 + nudge[0], -(height as i32) + foot + nudge[1]],
                image: image.clone(),
                action: None,
            });
        }
        objects
    }

    /// The minimap: the crypt's floor light on the dark, at an eighth of
    /// the picture.
    pub fn minimap(&self) -> RgbaImage {
        let picture = [self.columns * TILE, self.rows * TILE];
        let scale = 8.0;
        let mut image = RgbaImage::from_pixel(
            (picture[0] as f32 / scale) as u32,
            (picture[1] as f32 / scale) as u32,
            Rgba([8, 9, 12, 255]),
        );
        for y in 0..self.size {
            for x in 0..self.size {
                let cell = self.cells[(y * self.size + x) as usize];
                let p = project(picture, self.size, x as f32, y as f32);
                let colour = if cell.access == 0 { Rgba([120, 116, 104, 255]) } else { continue };
                let (cx, cy) = (p[0] / scale, p[1] / scale);
                for dy in -2..=2 {
                    for dx in -4..=4 {
                        if (dx as f32).abs() / 4.0 + (dy as f32).abs() / 2.0 > 1.0 {
                            continue;
                        }
                        let (ix, iy) = (cx as i64 + dx, cy as i64 + dy);
                        if ix >= 0 && iy >= 0 && (ix as u32) < image.width() && (iy as u32) < image.height() {
                            image.put_pixel(ix as u32, iy as u32, colour);
                        }
                    }
                }
            }
        }
        // The walls' line around the floor.
        let lit = image.clone();
        for y in 1..image.height() - 1 {
            for x in 1..image.width() - 1 {
                if lit.get_pixel(x, y)[0] > 50 {
                    continue;
                }
                let beside = [(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)].iter().any(|&(a, b)| lit.get_pixel(a, b)[0] > 50);
                if beside {
                    image.put_pixel(x, y, Rgba([62, 56, 50, 255]));
                }
            }
        }
        image
    }

    /// For the server: the size and each room's walkable cells, the
    /// entrance's and the hall's marked, as runs along a row (`room <x> <y>
    /// <kind> <cell y>:<first x>-<last x> ...`).
    pub fn layout_text(&self) -> String {
        let mut text = format!("# {} (gerado por gravak-content)\nsize {}\n", self.def.name, self.size);
        for (x, y, cells) in &self.rooms {
            let kind = if (*x, *y) == (0, 0) {
                "entrance"
            } else if (*x, *y) == self.hall {
                "hall"
            } else {
                "room"
            };
            text.push_str(&format!("room {x} {y} {kind}"));
            let mut sorted: Vec<(u32, u32)> = cells.iter().map(|&(cx, cy)| (cy, cx)).collect();
            sorted.sort_unstable();
            let mut index = 0;
            while index < sorted.len() {
                let (row, first) = sorted[index];
                let mut last = first;
                while index + 1 < sorted.len() && sorted[index + 1] == (row, last + 1) {
                    index += 1;
                    last += 1;
                }
                text.push_str(&format!(" {row}:{first}-{last}"));
                index += 1;
            }
            text.push('\n');
        }
        text
    }

    /// The entrance's middle cell.
    pub fn entrance(&self) -> (u32, u32) {
        let (x0, y0) = room_origin((0, 0));
        ((x0 + CORRIDOR / 2) as u32, (y0 + CORRIDOR / 2) as u32)
    }
}

fn write(output: &Path, path: &str, bytes: &[u8]) -> Result<()> {
    let path = output.join(path);
    std::fs::create_dir_all(path.parent().expect("a folder"))?;
    std::fs::write(path, bytes)?;
    Ok(())
}

fn save_png(output: &Path, path: &str, image: &RgbaImage) -> Result<()> {
    let target = output.join(path);
    std::fs::create_dir_all(target.parent().expect("a folder"))?;
    image.save(&target)?;
    Ok(())
}

/// Builds every rift of `content/rifts.toml` into `output`; how many.
pub fn build(installation: &mut Installation, content: &Path, output: &Path) -> Result<usize> {
    let config = load(content)?;
    if config.rift.is_empty() {
        return Ok(0);
    }
    // Each theme the rifts use: its floor's tiles and their ani, its wall
    // blocks and its decorations, shared by its rifts.
    let mut sizes = HashMap::new();
    let mut art_of: HashMap<&'static str, usize> = HashMap::new();
    for def in &config.rift {
        let theme = theme(&def.theme)?;
        if art_of.contains_key(theme.name) {
            continue;
        }
        let art = Art::load(content, theme)?;
        let floor = floor_tiles(&art, theme.floor_period);
        let mut ani = String::new();
        for (index, tile) in floor.iter().enumerate() {
            let path = format!("{}/{index}.png", tile_dir(theme.name));
            save_png(output, &path, tile)?;
            ani.push_str(&format!("[Puzzle{index}]\r\nFrameAmount=1\r\nFrame0={path}\r\n\r\n"));
        }
        write(output, &puzzle_ani(theme.name), ani.as_bytes())?;
        for y in 0..theme.wall_period {
            for x in 0..theme.wall_period {
                let path = format!("{}/parede-{x}-{y}.png", object_dir(theme.name));
                let block = wall_block(&art, theme.wall_period, theme.wall_light, x, y);
                sizes.insert(path.clone(), [block.width(), block.height()]);
                save_png(output, &path, &block)?;
            }
        }
        for (name, picture) in &art.props {
            let path = format!("{}/{name}.png", object_dir(theme.name));
            sizes.insert(path.clone(), [picture.width(), picture.height()]);
            save_png(output, &path, picture)?;
        }
        art_of.insert(theme.name, art.floors.len());
    }

    let mut catalog = GameMapCatalog::parse(&installation.read("ini/GameMap.dat")?)?;
    let mut minimaps = String::from_utf8_lossy(&installation.read(MINIMAP_ANI)?).into_owned();
    if !minimaps.ends_with('\n') {
        minimaps.push_str("\r\n");
    }
    let mut records = Vec::new();
    for def in &config.rift {
        if catalog.get(def.id).is_some() {
            return Err(format!("rift {}: the installation already has map {}", def.id, def.id).into());
        }
        let theme = theme(&def.theme)?;
        let floor_variants = art_of[theme.name];
        let rift = generate(def, theme, floor_variants)?;
        write(output, &rift.puzzle_path(), &rift.puzzle_bytes())?;
        write(output, &rift.dmap_path(), &rift.dmap_bytes())?;
        write(output, &format!("gravak/rifts/{}.txt", def.id), rift.layout_text().as_bytes())?;
        write(output, &sandbox_data::map::gravak_objects_path(def.id), rift.gravak_objects(&sizes).write().as_bytes())?;
        let minimap = format!("data/MiniMap/gravak-rift-{}.png", def.id);
        save_png(output, &minimap, &rift.minimap())?;
        minimaps.push_str(&format!("[{}]\r\nFrameAmount=1\r\nFrame0={minimap}\r\n", def.id));
        records.push(GameMapRecord {
            id: def.id,
            dmap_path: rift.dmap_path().as_str().try_into()?,
            tile_size: TILE,
        });
    }
    write(output, MINIMAP_ANI, minimaps.as_bytes())?;
    let extension = GameMapCatalog::parse(&GameMapCatalog::encode(&records)?)?;
    catalog.merge_distinct(extension)?;
    let all: Vec<GameMapRecord> = catalog.records().cloned().collect();
    write(output, "ini/GameMap.dat", &GameMapCatalog::encode(&all)?)?;
    Ok(config.rift.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sandbox_data::map::{DMap, PuzzleLayout};

    fn def() -> RiftDef {
        RiftDef { id: 9101, name: "test".into(), seed: 1, rooms: [9, 9], theme: "cripta".into() }
    }

    #[test]
    fn mazes_reach_every_room() {
        let mut rng = Rng(7);
        let open = maze(6, 4, &mut rng, 0.0);
        // A spanning tree: one opening fewer than rooms.
        assert_eq!(open.len(), 6 * 4 - 1);
        assert_eq!(distances([6, 4], &open).len(), 6 * 4);
    }

    #[test]
    fn corridors_are_five_cells_wide() {
        let rift = generate(&def(), &THEMES[0], 3).unwrap();
        // Across the entrance room's middle: wall, five of floor, wall.
        let (x0, y0) = room_origin((0, 0));
        let y = y0 + CORRIDOR / 2;
        let walkable = |x: usize| rift.cells[y * rift.size as usize + x].access == 0;
        assert!(!walkable(x0 - 1));
        assert!((x0..x0 + CORRIDOR).all(walkable));
        // Every room but the hall's other three is reached.
        assert_eq!(rift.rooms.len(), 9 * 9 - 3);
    }

    #[test]
    fn rift_files_read_back_as_the_client_reads_them() {
        let rift = generate(&def(), &THEMES[0], 3).unwrap();
        let puzzle = PuzzleLayout::parse(&rift.puzzle_bytes()).unwrap();
        assert_eq!(puzzle.tiles, rift.tiles);
        let map = DMap::parse(&rift.dmap_bytes()).unwrap();
        assert_eq!((map.width, map.height), (rift.size, rift.size));
        assert_eq!(map.cells, rift.cells);
        // The picture covers every cell.
        assert_eq!(map.width, puzzle.columns * 4);
        assert_eq!(map.width, puzzle.rows * 8);
        // The same seed, the same rift.
        assert_eq!(generate(&def(), &THEMES[0], 3).unwrap().tiles, rift.tiles);
    }
}
