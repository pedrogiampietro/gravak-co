// Source: @digitalm1nd on x.com / _dek on Discord — Preservation Conquer project — https://discord.gg/CvKPXEHYRY
use crate::assets::{Assets, Result};
use sandbox_data::map::{ComposedMap, DMap};

pub use sandbox_data::map::MapSprite as Sprite;

pub struct Map {
    pub id: u32,
    pub data: DMap,
    pub sprites: Vec<Sprite>,
    pub extent: [f32; 2],
    /// Gravak objects a click does something with (the Reward Shrine).
    pub actions: Vec<sandbox_data::map::GravakAction>,
    /// A Rift's crypt: the cells of each of its rooms (`gravak/rifts/<id>.txt`
    /// of the built content), discovered one at a time (`crate::reveal`);
    /// empty on every other map.
    pub rooms: Vec<Vec<[i32; 2]>>,
}
impl Map {
    /// The map as `sandbox_data::map::ComposedMap` lays it out, Gravak's own
    /// objects (`gravak/maps/<id>.txt` of the built content) included.
    pub fn load(assets: &mut Assets, id: u32) -> Result<Self> {
        let mut read = |name: &str| assets.read(name).map_err(|error| error.to_string());
        let map = ComposedMap::load(&mut read, id)?;
        // Says whether our content reached the client, where a missing
        // build would otherwise go unnoticed.
        match assets.overlay_path() {
            None if assets.is_packed() => {
                eprintln!("map {id}: {} Gravak objects (packed)", map.gravak_objects)
            }
            Some(overlay) => eprintln!(
                "map {id}: {} Gravak objects (content {})",
                map.gravak_objects,
                overlay.display()
            ),
            None => eprintln!("map {id}: Gravak content off (GRAVAK_CONTENT not set or missing)"),
        }
        let rooms = assets
            .read(&format!("gravak/rifts/{id}.txt"))
            .map(|text| rooms(&String::from_utf8_lossy(&text)))
            .unwrap_or_default();
        Ok(Self {
            id: map.id,
            data: map.data,
            sprites: map.sprites,
            extent: map.extent,
            actions: map.gravak_actions,
            rooms,
        })
    }
    pub fn project(&self, p: [f32; 2]) -> [f32; 2] {
        sandbox_data::map::project(self.extent, self.data.height, p)
    }
    pub fn unproject(&self, p: [f32; 2]) -> [f32; 2] {
        sandbox_data::map::unproject(self.extent, self.data.height, p)
    }
    pub fn walkable(&self, p: [i32; 2]) -> bool {
        p[0] >= 0
            && p[1] >= 0
            && self
                .data
                .cell(p[0] as u32, p[1] as u32)
                .is_some_and(|c| c.access == 0)
    }
}

/// The rooms of a crypt's layout: `room <x> <y> <kind> <row>:<first>-<last>...`,
/// each run a row (y) of cells from x `first` to `last`.
fn rooms(text: &str) -> Vec<Vec<[i32; 2]>> {
    text.lines()
        .filter_map(|line| line.strip_prefix("room "))
        .map(|line| {
            line.split_whitespace()
                .skip(3)
                .filter_map(|run| {
                    let (row, span) = run.split_once(':')?;
                    let (first, last) = span.split_once('-')?;
                    Some((row.parse::<i32>().ok()?, first.parse::<i32>().ok()?, last.parse::<i32>().ok()?))
                })
                .flat_map(|(y, first, last)| (first..=last).map(move |x| [x, y]))
                .collect()
        })
        .collect()
}

#[cfg(test)]
#[test]
fn reads_a_crypts_rooms() {
    let rooms = rooms("# Covil
size 80
room 0 0 entrance 11:11-13 12:11-11
room 1 0 room 11:18-18
");
    assert_eq!(rooms, vec![vec![[11, 11], [12, 11], [13, 11], [11, 12]], vec![[18, 11]]]);
}
