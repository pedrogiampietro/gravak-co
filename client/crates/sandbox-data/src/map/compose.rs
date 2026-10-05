//! A whole map ready to draw: the puzzle background, the terrain and scene
//! objects, and Gravak's own objects, as sprites in map pixels. The client
//! and the editor draw the same thing from it.

use super::{DMap, GameMapCatalog, MapScene, PuzzleLayout, apply_fixed_scene_layers};
use crate::ini::SectionCatalog;

/// Reads a file of the installation (or of the built content before it).
pub type Read<'a> = dyn FnMut(&str) -> Result<Vec<u8>, String> + 'a;

#[derive(Clone, Debug, PartialEq)]
pub struct MapSprite {
    pub frames: Vec<String>,
    /// The top-left corner, in map pixels.
    pub origin: [f32; 2],
    /// Drawing order: the background first, then by `x + y` of the cell.
    pub depth: f32,
    pub interval: u32,
}

#[derive(Clone, Debug)]
pub struct ComposedMap {
    pub id: u32,
    pub data: DMap,
    pub sprites: Vec<MapSprite>,
    pub extent: [f32; 2],
}

/// The file the content build writes for a map's Gravak objects.
pub fn gravak_objects_path(id: u32) -> String {
    format!("gravak/maps/{id}.txt")
}

impl ComposedMap {
    pub fn load(read: &mut Read<'_>, id: u32) -> Result<Self, String> {
        let catalog =
            GameMapCatalog::parse(&read("ini/GameMap.dat")?).map_err(|error| error.to_string())?;
        let record = catalog.get(id).ok_or("Map is not in GameMap.dat")?;
        let mut data =
            DMap::parse(&read(record.dmap_path.as_str())?).map_err(|error| error.to_string())?;
        let puzzle = PuzzleLayout::parse(&read(data.puzzle_path.as_str())?)
            .map_err(|error| error.to_string())?;
        if !puzzle.cell_layers.is_empty() || !data.additional_puzzle_layers.is_empty() {
            return Err(
                "This minimal renderer supports flat PUL maps, not layered PUX maps".into(),
            );
        }
        let tile = record.tile_size as f32;
        if tile <= 0.0 {
            return Err("invalid map tile size".into());
        }
        let extent = [puzzle.columns as f32 * tile, puzzle.rows as f32 * tile];
        let mut sprites = Vec::new();
        let ani = SectionCatalog::parse(&read(puzzle.ani_path.as_str())?);
        for (i, id) in puzzle.tiles.iter().enumerate() {
            if *id < 0 {
                continue;
            }
            sprites.push(MapSprite {
                frames: frames(&ani, &format!("Puzzle{id}"))?,
                origin: [
                    (i as u32 % puzzle.columns) as f32 * tile,
                    (i as u32 / puzzle.columns) as f32 * tile,
                ],
                depth: f32::NEG_INFINITY,
                interval: 100,
            });
        }
        for object in &data.terrain_objects {
            if !object.is_renderable() {
                continue;
            }
            let ani = SectionCatalog::parse(&read(object.ani_path.as_str())?);
            let p = project(extent, data.height, [object.x as f32, object.y as f32]);
            sprites.push(MapSprite {
                frames: frames(&ani, &object.section)?,
                origin: [p[0] - object.offset_x as f32, p[1] - object.offset_y as f32],
                depth: (object.x + object.y) as f32,
                interval: object.frame_interval_ms.max(1),
            });
        }
        for object in data.scene_objects.clone() {
            let scene = MapScene::parse(&read(object.scene_path.as_str())?)
                .map_err(|error| error.to_string())?;
            for part in &scene.parts {
                let x = object.x + part.scene_offset_x;
                let y = object.y + part.scene_offset_y;
                let p = project(extent, data.height, [x as f32, y as f32]);
                let ani = SectionCatalog::parse(&read(part.ani_path.as_str())?);
                sprites.push(MapSprite {
                    frames: frames(&ani, &part.section)?,
                    origin: [p[0] + part.offset_x as f32, p[1] + part.offset_y as f32],
                    depth: (x + y) as f32,
                    interval: part.frame_interval_ms.max(1),
                });
            }
            apply_fixed_scene_layers(&mut data, object.x, object.y, &scene);
        }
        let mut map = Self {
            id,
            data,
            sprites,
            extent,
        };
        if let Ok(objects) = read(&gravak_objects_path(id)) {
            let objects = GravakObjects::parse(&String::from_utf8_lossy(&objects))?;
            map.add_gravak_objects(&objects);
        }
        map.sprites.sort_by(|a, b| a.depth.total_cmp(&b.depth));
        Ok(map)
    }

    /// Draws the objects over the map and closes the cells they block.
    pub fn add_gravak_objects(&mut self, objects: &GravakObjects) {
        for object in &objects.objects {
            let p = self.project([object.x as f32, object.y as f32]);
            self.sprites.push(MapSprite {
                frames: vec![object.image.clone()],
                origin: [
                    p[0] + object.offset[0] as f32,
                    p[1] + object.offset[1] as f32,
                ],
                depth: (object.x + object.y) as f32,
                interval: 100,
            });
        }
        for &[x, y] in &objects.blocked {
            let (Ok(x), Ok(y)) = (u32::try_from(x), u32::try_from(y)) else {
                continue;
            };
            if x < self.data.width && y < self.data.height {
                let index = y as usize * self.data.width as usize + x as usize;
                self.data.cells[index].access = 1;
            }
        }
    }

    pub fn project(&self, p: [f32; 2]) -> [f32; 2] {
        project(self.extent, self.data.height, p)
    }

    pub fn unproject(&self, p: [f32; 2]) -> [f32; 2] {
        unproject(self.extent, self.data.height, p)
    }
}

/// The map pixel of a cell (fractional cells give points between them).
pub fn project(extent: [f32; 2], height: u32, p: [f32; 2]) -> [f32; 2] {
    [
        (p[0] - p[1]) * 32.0 + extent[0] * 0.5,
        (p[0] + p[1] - (height - 1) as f32) * 16.0 + extent[1] * 0.5,
    ]
}

/// The cell under a map pixel, the inverse of [`project`].
pub fn unproject(extent: [f32; 2], height: u32, p: [f32; 2]) -> [f32; 2] {
    let d = (p[0] - extent[0] * 0.5) / 32.0;
    let s = (p[1] - extent[1] * 0.5) / 16.0 + (height - 1) as f32;
    [(s + d) * 0.5, (s - d) * 0.5]
}

fn frames(ani: &SectionCatalog, name: &str) -> Result<Vec<String>, String> {
    let section = ani
        .section(name)
        .ok_or_else(|| format!("Missing ANI section {name}"))?;
    let count = section
        .first("FrameAmount")
        .unwrap_or("1")
        .parse::<usize>()
        .map_err(|error| error.to_string())?;
    if count == 0 || count > 4096 {
        return Err("invalid ANI frame count".into());
    }
    (0..count)
        .map(|i| {
            section
                .first(&format!("Frame{i}"))
                .map(|s| s.replace('\\', "/"))
                .ok_or_else(|| format!("Missing {name}/Frame{i}"))
        })
        .collect()
}

/// Gravak's objects of one map, as the content build writes them
/// (`gravak/maps/<id>.txt`), one per line:
///
/// ```text
/// object <x> <y> <offset x> <offset y> <image>
/// block <x> <y>
/// ```
///
/// The offset goes from the cell's point to the image's top-left corner.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GravakObjects {
    pub objects: Vec<GravakObject>,
    pub blocked: Vec<[i32; 2]>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GravakObject {
    pub x: i32,
    pub y: i32,
    pub offset: [i32; 2],
    pub image: String,
}

impl GravakObjects {
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut objects = Self::default();
        for (number, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let fields = line.split_whitespace().collect::<Vec<_>>();
            let int = |index: usize| {
                fields
                    .get(index)
                    .and_then(|value| value.parse::<i32>().ok())
                    .ok_or_else(|| format!("gravak objects line {}: invalid number", number + 1))
            };
            match fields[0] {
                "object" if fields.len() == 6 => objects.objects.push(GravakObject {
                    x: int(1)?,
                    y: int(2)?,
                    offset: [int(3)?, int(4)?],
                    image: fields[5].to_owned(),
                }),
                "block" if fields.len() == 3 => objects.blocked.push([int(1)?, int(2)?]),
                _ => return Err(format!("gravak objects line {}: {line:?}", number + 1)),
            }
        }
        Ok(objects)
    }

    pub fn write(&self) -> String {
        let mut out = String::from(
            "# Gravak objects: object <x> <y> <offset x> <offset y> <image> / block <x> <y>\n",
        );
        for object in &self.objects {
            out.push_str(&format!(
                "object {} {} {} {} {}\n",
                object.x, object.y, object.offset[0], object.offset[1], object.image
            ));
        }
        for [x, y] in &self.blocked {
            out.push_str(&format!("block {x} {y}\n"));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projection_round_trips() {
        let extent = [2048.0, 1024.0];
        let cell = [17.0, 40.0];
        let pixel = project(extent, 64, cell);
        let back = unproject(extent, 64, pixel);
        assert!((back[0] - cell[0]).abs() < 1e-3 && (back[1] - cell[1]).abs() < 1e-3);
    }

    fn fixed(text: &str, length: usize) -> Vec<u8> {
        let mut bytes = text.as_bytes().to_vec();
        bytes.resize(length, 0);
        bytes
    }

    /// A 10x10 map with one background tile, one terrain object and our
    /// statue, as files of an installation.
    fn installation() -> std::collections::HashMap<String, Vec<u8>> {
        let mut files = std::collections::HashMap::new();
        let record = crate::map::GameMapRecord {
            id: 7,
            dmap_path: crate::VirtualPath::try_from("map/map/test.DMap").unwrap(),
            tile_size: 256,
        };
        files.insert(
            "ini/GameMap.dat".to_owned(),
            GameMapCatalog::encode(&[record]).unwrap(),
        );
        let mut pul = fixed("PUZZLE2", 8);
        pul.extend(fixed("ani/test.ani", 256));
        pul.extend(2_i32.to_le_bytes());
        pul.extend(1_i32.to_le_bytes());
        pul.extend(0_i16.to_le_bytes());
        pul.extend((-1_i16).to_le_bytes());
        files.insert("map/puzzle/test.pul".to_owned(), pul);
        files.insert(
            "ani/test.ani".to_owned(),
            b"[Puzzle0]\nFrame0=data/tile.dds\n[House]\nFrame0=data/house.dds\n".to_vec(),
        );
        let mut dmap = Vec::new();
        dmap.extend(1004_u32.to_le_bytes());
        dmap.extend(0_u32.to_le_bytes());
        dmap.extend(fixed("map/puzzle/test.pul", 260));
        dmap.extend(10_u32.to_le_bytes());
        dmap.extend(10_u32.to_le_bytes());
        for _ in 0..10 {
            for _ in 0..10 {
                dmap.extend([0_u8; 6]);
            }
            dmap.extend(0_u32.to_le_bytes());
        }
        dmap.extend(0_i32.to_le_bytes());
        dmap.extend(1_i32.to_le_bytes());
        dmap.extend(4_i32.to_le_bytes());
        dmap.extend(fixed("ani/test.ani", 260));
        dmap.extend(fixed("House", 128));
        for value in [2_i32, 3, 1, 1, 10, 20, 100] {
            dmap.extend(i32::to_le_bytes(value));
        }
        files.insert("map/map/test.DMap".to_owned(), dmap);
        files
    }

    #[test]
    fn maps_compose_with_our_objects() {
        let mut files = installation();
        let mut read = |name: &str| {
            files
                .get(name)
                .cloned()
                .ok_or_else(|| format!("missing {name}"))
        };
        let plain = ComposedMap::load(&mut read, 7).unwrap();
        assert_eq!(plain.sprites.len(), 2);
        assert_eq!(plain.sprites[0].frames, ["data/tile.dds"]);
        assert_eq!(plain.data.cell(4, 5).unwrap().access, 0);

        files.insert(
            gravak_objects_path(7),
            b"object 4 5 -30 -60 gravak/objects/statue.png\nblock 4 5\n".to_vec(),
        );
        let mut read = |name: &str| {
            files
                .get(name)
                .cloned()
                .ok_or_else(|| format!("missing {name}"))
        };
        let map = ComposedMap::load(&mut read, 7).unwrap();
        assert_eq!(map.sprites.len(), 3);
        let statue = map
            .sprites
            .iter()
            .find(|sprite| sprite.frames[0] == "gravak/objects/statue.png")
            .unwrap();
        let base = map.project([4.0, 5.0]);
        assert_eq!(statue.origin, [base[0] - 30.0, base[1] - 60.0]);
        assert_eq!(statue.depth, 9.0);
        // Drawn in depth order: the house (2 + 3) before the statue.
        assert_eq!(map.sprites.last().unwrap(), statue);
        assert_eq!(map.data.cell(4, 5).unwrap().access, 1);
    }

    #[test]
    fn gravak_objects_round_trip() {
        let objects = GravakObjects {
            objects: vec![GravakObject {
                x: 430,
                y: 380,
                offset: [-64, -120],
                image: "gravak/objects/statue.png".into(),
            }],
            blocked: vec![[430, 380], [431, 380]],
        };
        assert_eq!(GravakObjects::parse(&objects.write()).unwrap(), objects);
        assert!(GravakObjects::parse("object 1 2\n").is_err());
    }
}
