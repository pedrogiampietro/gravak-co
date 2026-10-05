// Source: @digitalm1nd on x.com / _dek on Discord — Preservation Conquer project — https://discord.gg/CvKPXEHYRY
use crate::assets::{Assets, Result};
use sandbox_data::map::{ComposedMap, DMap};

pub use sandbox_data::map::MapSprite as Sprite;

pub struct Map {
    pub id: u32,
    pub data: DMap,
    pub sprites: Vec<Sprite>,
    pub extent: [f32; 2],
}
impl Map {
    /// The map as `sandbox_data::map::ComposedMap` lays it out, Gravak's own
    /// objects (`gravak/maps/<id>.txt` of the built content) included.
    pub fn load(assets: &mut Assets, id: u32) -> Result<Self> {
        let mut read = |name: &str| assets.read(name).map_err(|error| error.to_string());
        let map = ComposedMap::load(&mut read, id)?;
        Ok(Self {
            id: map.id,
            data: map.data,
            sprites: map.sprites,
            extent: map.extent,
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
