use core::fmt;
use futures::stream::FuturesUnordered;
use futures::{StreamExt, TryFutureExt};
use num_enum::{FromPrimitive, IntoPrimitive};
use parking_lot::RwLock;
use primitives::{Location, Point, Size};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Weak};
use tq_math::SCREEN_DISTANCE;
use tq_network::{PacketEncode, PacketID};

use super::Portal;
use crate::entities::{GameEntity, Monster, Npc};
use crate::packets::{MapFlags, MsgWeather, WeatherKind};
use crate::systems::{Floor, Tile, TileType};
use crate::{constants, Error};

type Entities = RwLock<HashMap<u32, Weak<GameEntity>>>;
type Portals = HashSet<Portal>;
type Npcs = HashMap<u32, Arc<GameEntity>>;
type Monsters = HashMap<u32, Arc<GameEntity>>;
type MapRegions = RwLock<Vec<MapRegion>>;

/// This struct encapsulates map information from a compressed map and the
/// database. It includes the identification of the map, pools and methods for
/// character tracking and screen updates, and other methods for processing map
/// actions and events. It composes the floor struct which defines the map's
/// coordinate tile grid.
#[derive(Debug, Default)]
pub struct Map {
    /// The Inner map loaded from the database
    inner: tq_db::map::Map,
    /// where should the player get revived on this map
    revive_point: Point<u32>,
    /// defines the map's coordinate tile grid.
    floor: Floor,
    /// Holds all Portals in that map.
    portals: Portals,
    /// Holds all Npcs in that map.
    npcs: Npcs,
    /// Persistent monster definitions and spawn rules for this map.
    monsters: Vec<(tq_db::monster::MonsterSpawn, tq_db::monster::MonsterType)>,
    /// Runtime monster instances for this map.
    monster_entities: Monsters,
    /// Holds all MapRegions in that map.
    regions: MapRegions,
}

impl Map {
    pub fn new(
        inner: tq_db::map::Map,
        portals: Vec<tq_db::portal::Portal>,
        npcs: Vec<tq_db::npc::Npc>,
        monsters: Vec<(tq_db::monster::MonsterSpawn, tq_db::monster::MonsterType)>,
    ) -> Self {
        let portals = portals.into_iter().map(Portal::new).collect();
        let npcs = npcs
            .into_iter()
            .filter(|npc| !constants::is_terrain_npc(npc.id as _))
            .map(|v| (v.id as u32, Arc::new(GameEntity::from(Npc::from(v)))))
            .collect();
        let mut occupied_monster_tiles = HashSet::new();
        let mut monster_entities = HashMap::new();
        for (spawn, monster_type) in &monsters {
            let amount = spawn.amount.max(1) as u32;
            for index in 0..amount {
                let id_seed = spawn.id.max(0) as u32 * 1000 + index;
                let id =
                    constants::MONSTER_ID_MIN + (id_seed % (constants::MONSTER_ID_MAX - constants::MONSTER_ID_MIN));
                // Keep the first instance at the authored location and
                // spread additional instances across the spawn area. The
                // previous diagonal-only offset made an entire pack spawn
                // on top of the same visual lane.
                let mut instance_spawn = spawn.clone();
                if index > 0 && spawn.radius > 0 {
                    let radius = spawn.radius as i16;
                    let span = radius * 2 + 1;
                    let mut offset_x = ((index as i16 * 37) % span) - radius;
                    let mut offset_y = ((index as i16 * 53) % span) - radius;
                    let mut candidate = (
                        instance_spawn.x.saturating_add(offset_x),
                        instance_spawn.y.saturating_add(offset_y),
                    );
                    let mut attempts = 0;
                    while occupied_monster_tiles.contains(&candidate) && attempts < span * span {
                        offset_x = (offset_x + 1).rem_euclid(span) - radius;
                        offset_y = (offset_y + 1).rem_euclid(span) - radius;
                        candidate = (
                            instance_spawn.x.saturating_add(offset_x),
                            instance_spawn.y.saturating_add(offset_y),
                        );
                        attempts += 1;
                    }
                    instance_spawn.x = candidate.0;
                    instance_spawn.y = candidate.1;
                }
                occupied_monster_tiles.insert((instance_spawn.x, instance_spawn.y));
                let monster = Monster::new(id, monster_type.clone(), instance_spawn);
                monster_entities.insert(id, Arc::new(GameEntity::from(monster)));
            }
        }
        Self {
            floor: Floor::new(inner.path.clone()),
            revive_point: Point::new(inner.revive_point_x as u32, inner.revive_point_y as u32),
            regions: RwLock::new(Vec::new()),
            npcs,
            monsters,
            monster_entities,
            portals,
            inner,
        }
    }

    pub fn id(&self) -> u32 {
        self.inner.id as u32
    }

    pub fn map_id(&self) -> u32 {
        self.inner.map_id as u32
    }

    pub fn weather(&self) -> WeatherKind {
        WeatherKind::from(self.inner.weather as u32)
    }

    pub fn flags(&self) -> MapFlags {
        MapFlags::from_bits(self.inner.flags as u32).unwrap_or_default()
    }

    pub fn color(&self) -> u32 {
        self.inner.color as u32
    }

    pub fn revive_point(&self) -> Point<u32> {
        self.revive_point
    }

    pub fn is_static(&self) -> bool {
        self.inner.id == self.inner.map_id
    }

    pub fn is_copy(&self) -> bool {
        self.inner.id == self.inner.map_id
    }

    pub fn portals(&self) -> &Portals {
        &self.portals
    }

    pub fn tile(&self, x: u16, y: u16) -> Option<Tile> {
        self.floor.tile(x, y)
    }

    /// Finds a nearby cell where a character can stand. Characters can be
    /// left on an NPC tile by older clients or by an interrupted teleport;
    /// accepting that persisted position would make the next login appear
    /// frozen because the client starts inside the NPC collision cell.
    pub fn nearest_walkable_position(&self, origin: Location, max_radius: u16) -> Option<Location> {
        let walkable = |x: i32, y: i32| {
            let (Ok(x), Ok(y)) = (u16::try_from(x), u16::try_from(y)) else {
                return false;
            };
            self.tile(x, y).is_some_and(|tile| tile.access > TileType::Npc)
        };
        nearest_walkable_candidate([i32::from(origin.x), i32::from(origin.y)], max_radius, walkable)
            .map(|[x, y]| Location::new(x as u16, y as u16, origin.direction))
    }

    pub fn npc(&self, id: u32) -> Option<&Npc> {
        self.npcs.get(&id).and_then(|v| v.as_npc())
    }

    pub fn monsters(&self) -> &[(tq_db::monster::MonsterSpawn, tq_db::monster::MonsterType)] {
        &self.monsters
    }

    pub fn monster_entity(&self, id: u32) -> Option<&Arc<GameEntity>> {
        self.monster_entities.get(&id)
    }

    pub fn monster_entities(&self) -> Vec<Arc<GameEntity>> {
        self.monster_entities.values().cloned().collect()
    }

    pub fn with_regions<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&Vec<MapRegion>) -> R,
    {
        f(&self.regions.read())
    }

    #[tracing::instrument(skip(self))]
    pub fn region(&self, x: u16, y: u16) -> Option<MapRegion> {
        let regions = self.regions.read();
        let map_size = self.floor.boundaries();
        let region_size = MapRegion::SIZE;
        let region_x = x as u32 / region_size.width;
        let region_y = y as u32 / region_size.height;
        let width = (map_size.width as f32 / region_size.width as f32).ceil() as u32;
        let region_index = region_x * width + region_y;
        tracing::trace!(%x, %y, %region_x, %region_y, %region_index, "Querying Region");
        regions.get(region_index as usize).cloned()
    }

    /// Get a list of the regions that surround the given point.
    #[tracing::instrument(skip(self))]
    pub fn surrunding_regions(&self, x: u16, y: u16) -> Vec<MapRegion> {
        let regions = self.regions.read();
        let map_size = self.floor.boundaries();
        let region_size = MapRegion::SIZE;
        let height = (map_size.height as f32 / region_size.height as f32).ceil() as u32;
        let width = (map_size.width as f32 / region_size.width as f32).ceil() as u32;
        let region_x = x as u32 / region_size.width;
        let region_y = y as u32 / region_size.height;
        let region_index = |x, y| (x * width + y) as usize;
        let mut result = Vec::new();
        // insert the current region
        if let Some(region) = regions.get(region_index(region_x, region_y)) {
            result.push(region.clone());
        }
        for i in 0..constants::WALK_XCOORDS.len() {
            let view_x = region_x as i32 + constants::WALK_XCOORDS[i] as i32;
            let view_y = region_y as i32 + constants::WALK_YCOORDS[i] as i32;
            if view_x.is_negative() || view_y.is_negative() || view_x >= width as _ || view_y >= height as _ {
                continue;
            }
            let j = region_index(view_x as u32, view_y as u32);
            if let Some(region) = regions.get(j) {
                result.push(region.clone());
            }
        }
        result
    }

    // This method loads a compressed map from the server's flat file database.
    // If the file does not exist, the
    /// server will make an attempt to find and convert a dmap version of the
    /// map into a compressed map file. After converting the map, the map
    /// will be loaded for the server.
    #[tracing::instrument(skip_all, fields(map_id = self.id()))]
    pub async fn load(&self) -> Result<(), Error> {
        if self.loaded() {
            return Ok(());
        }
        tracing::trace!("Loading into memory");
        self.floor.load().await?;
        let map_size = self.floor.boundaries();
        let region_size = MapRegion::SIZE;
        // ceil division to get the number of regions
        let height = (map_size.height as f32 / region_size.height as f32).ceil() as u32;
        let width = (map_size.width as f32 / region_size.width as f32).ceil() as u32;
        let number_of_regions = height * width;
        tracing::trace!(
            %map_size,
            %region_size,
            %height,
            %width,
            number_of_regions,
            "Building regions",
        );
        let mut regions = vec![MapRegion::default(); number_of_regions as usize];
        for y in 0..height {
            for x in 0..width {
                let start_point = Point::new(x, y);
                let region = MapRegion::new(start_point, map_size);
                let i = x * width + y;
                regions[i as usize] = region;
                tracing::trace!(%start_point, "Region created");
            }
        }
        assert_eq!(regions.len(), number_of_regions as usize);
        {
            let mut lock = self.regions.write();
            *lock = regions;
        }
        self.insert_batch(self.npcs.values().cloned()).await?;
        self.insert_batch(self.monster_entities.values().cloned()).await?;
        tracing::trace!("Map Loaded into memory");
        Ok(())
    }

    #[tracing::instrument(skip_all, fields(map_id = self.id()))]
    pub fn unload(&self) -> Result<(), Error> {
        tracing::trace!("Unload from memory");
        self.floor.unload();
        *self.regions.write() = Vec::new();
        tracing::trace!("Unloaded from memory");
        Ok(())
    }

    /// This method checks if the map is loaded in memory.
    pub fn loaded(&self) -> bool {
        self.floor.loaded() && !self.regions.read().is_empty()
    }

    /// Insert an entity into the map. If the map is not loaded in memory, it
    /// will be loaded.
    #[tracing::instrument(skip_all, fields(map_id = self.id(), entity_id = e.id()))]
    pub async fn insert_entity(&self, e: Arc<GameEntity>) -> Result<(), Error> {
        // if the map is not loaded in memory, load it.
        if !self.loaded() {
            self.load().await?;
        }
        self.update_region_for(e);
        Ok(())
    }

    #[tracing::instrument(skip(self, e), fields(map_id = self.id(), entity_id = e.id()))]
    pub fn remove_entity(&self, e: &GameEntity) -> Result<(), Error> {
        self.remove_entity_by_id_and_location(e.id(), e.basic().location())
    }

    pub fn remove_entity_by_id_and_location(&self, id: u32, Location { x, y, .. }: Location) -> Result<(), Error> {
        let region = self.region(x, y);
        if let Some(region) = region {
            region.remove_entity(id);
        }
        // if all entities are removed from the map, unload it.
        let empty = self.with_regions(|r| r.iter().all(|r| r.is_empty()));
        if empty {
            self.unload()?;
        }
        Ok(())
    }

    /// This method samples the map for elevation problems. If a player is
    /// jumping, this method will sample the map for key elevation changes
    /// and check that the player is not wall jumping. It checks all tiles
    /// in between the player and the jumping destination.
    #[tracing::instrument(skip(self), fields(map_id = self.id()))]
    pub fn sample_elevation(&self, start: (u16, u16), end: (u16, u16), elevation: u16) -> bool {
        let distance = tq_math::get_distance(start, end) as u16;
        // If the distance is 0, we are not moving.
        if distance == 0 {
            return true;
        }
        let delta = tq_math::delta(start, end);
        for i in 0..distance {
            let x = start.0 + ((i.saturating_mul(delta.0)) / distance);
            let y = start.1 + ((i.saturating_mul(delta.1)) / distance);
            let tile = self.floor.tile(x, y);
            match tile {
                Some(tile) => {
                    let within_elevation = tq_math::within_elevation(tile.elevation, elevation);
                    if !within_elevation {
                        return false;
                    }
                },
                None => return false,
            }
        }
        true
    }

    /// Updates the region for an entity. This method is called when an entity
    /// moves. It will remove the entity from the old region and insert it
    /// into the new region.
    #[tracing::instrument(skip_all, fields(map_id = self.id(), entity_id = e.as_ref().id()))]
    pub fn update_region_for(&self, e: Arc<GameEntity>) {
        let loc = e.basic().location();
        let prev_loc = e.basic().prev_location();
        let region = self.region(loc.x, loc.y);
        let old_region = self.region(prev_loc.x, prev_loc.y);
        match (region, old_region) {
            (Some(region), Some(old_region)) if region != old_region => {
                region.insert_entity(e.clone());
                old_region.remove_entity(e.id());
            },
            (Some(region), Some(_)) => {
                // During initial map loading, an entity's previous location
                // is equal to its current location. Insert it if the region
                // does not contain it yet; moving entities already present in
                // the same region remain untouched.
                if region.try_entities(e.id()).is_none() {
                    region.insert_entity(e);
                }
            },
            (Some(region), None) => {
                region.insert_entity(e.clone());
            },
            (None, Some(old_region)) => {
                old_region.remove_entity(e.id());
            },
            (None, None) => {
                tracing::warn!(
                    %loc.x,
                    %loc.y,
                    %prev_loc.x,
                    %prev_loc.y,
                    "Can not find a suitable region for character"
                )
            },
        }
    }

    /// act as "send to all" method, this method sends a packet to
    /// all characters inside this map.
    ///
    /// Internally, this method will iterate over all regions and call
    /// the `broadcast` method on each region.
    #[tracing::instrument(skip(self, packet), fields(map_id = self.id(), packet_id = P::PACKET_ID))]
    pub async fn broadcast<P>(&self, packet: P) -> Result<(), P::Error>
    where
        P: PacketEncode + PacketID + Clone,
    {
        let futs = FuturesUnordered::new();
        self.with_regions(|regions| {
            let regions = regions.iter().filter(|r| !r.is_empty()).cloned();
            for region in regions {
                let p = packet.clone();
                let f = async move { region.broadcast(p).await };
                futs.push(f);
            }
        });
        // await all futures to complete.
        futs.for_each_concurrent(None, |res| async {
            match res {
                Ok(_) => {},
                Err(e) => {
                    tracing::error!(error = ?e, "Failed to broadcast packet");
                },
            }
        })
        .await;
        Ok(())
    }

    pub async fn change_weather(&self, weather: WeatherKind) -> Result<(), Error> {
        let msg = MsgWeather::new(weather);
        self.broadcast(msg).map_err(Into::into).await
    }

    /// A batched version of [`Self::insert_entity`].
    #[tracing::instrument(skip_all, fields(map_id = self.id()))]
    async fn insert_batch<I>(&self, entities: I) -> Result<(), Error>
    where
        I: IntoIterator<Item = Arc<GameEntity>>,
    {
        for e in entities {
            self.update_region_for(e);
        }
        Ok(())
    }
}

fn nearest_walkable_candidate(
    origin: [i32; 2],
    max_radius: u16,
    walkable: impl Fn(i32, i32) -> bool,
) -> Option<[i32; 2]> {
    if walkable(origin[0], origin[1]) {
        return None;
    }

    for radius in 1..=i32::from(max_radius) {
        for offset in -radius..=radius {
            let candidates = [
                [origin[0] + offset, origin[1] - radius],
                [origin[0] + offset, origin[1] + radius],
                [origin[0] - radius, origin[1] + offset],
                [origin[0] + radius, origin[1] + offset],
            ];
            if let Some(candidate) = candidates
                .into_iter()
                .find(|candidate| walkable(candidate[0], candidate[1]))
            {
                return Some(candidate);
            }
        }
    }

    None
}

/// A region or a block is a set of the map which will hold a collection with
/// all entities in an area. This will help us iterating over a limited
/// number of entites when trying to process AI and movement. Instead of
/// iterating a list with thousand entities in the entire map, we'll just
/// iterate the regions around us.
#[derive(Debug, Default, Clone)]
pub struct MapRegion {
    start_point: Point<u32>,
    map_size: Size<i32>,
    entities: Arc<Entities>,
}

impl Eq for MapRegion {}
impl PartialEq for MapRegion {
    fn eq(&self, other: &Self) -> bool {
        self.start_point == other.start_point
    }
}

impl fmt::Display for MapRegion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let id = self.id();
        let n = self.with_entities(|c| c.len());
        write!(f, "Region #{id} with {n} entity")
    }
}

impl MapRegion {
    /// WIDTH and HEIGHT are the number of tiles in a region.
    pub const SIZE: Size<u32> = Size::new(SCREEN_DISTANCE as _, SCREEN_DISTANCE as _);

    pub fn new(start_point: Point<u32>, map_size: Size<i32>) -> Self {
        Self {
            start_point,
            map_size,
            entities: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn id(&self) -> usize {
        let width = (self.map_size.width as f32 / Self::SIZE.width as f32).ceil() as u32;
        let Point { x, y } = self.start_point;
        (x * width + y) as usize
    }

    pub fn is_empty(&self) -> bool {
        self.with_entities(|c| c.is_empty())
    }

    pub fn try_entities(&self, id: u32) -> Option<Weak<GameEntity>> {
        self.with_entities(|c| c.get(&id).cloned())
    }

    pub fn with_entities<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&HashMap<u32, Weak<GameEntity>>) -> R,
    {
        f(&self.entities.read())
    }

    pub fn with_entities_mut<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut HashMap<u32, Weak<GameEntity>>) -> R,
    {
        f(&mut self.entities.write())
    }

    #[tracing::instrument(skip_all, fields(map_id = self.id(), entity_id = entity.as_ref().id()))]
    pub fn insert_entity(&self, entity: Arc<GameEntity>) {
        self.with_entities_mut(|c| c.insert(entity.id(), Arc::downgrade(&entity)));
    }

    #[tracing::instrument(skip_all, fields(map_id = self.id(), entity_id = id))]
    pub fn remove_entity(&self, id: u32) -> Option<Weak<GameEntity>> {
        self.with_entities_mut(|c| c.remove(&id))
    }

    #[tracing::instrument(skip(self, packet), fields(region_id = self.id(), packet_id = P::PACKET_ID))]
    pub async fn broadcast<P>(&self, packet: P) -> Result<(), P::Error>
    where
        P: PacketEncode + PacketID + Clone,
    {
        let futs = FuturesUnordered::new();
        self.with_entities(|entities| {
            for character in entities.values() {
                let p = packet.clone();
                let Some(owner) = character.upgrade().and_then(|c| c.owner()) else {
                    continue;
                };
                let f = async move { owner.send(p).await };
                futs.push(f);
            }
        });
        // await all futures to complete.
        futs.for_each_concurrent(None, |_| async {}).await;
        Ok(())
    }
}

#[derive(Debug, FromPrimitive, IntoPrimitive)]
#[repr(u32)]
#[allow(unused)]
pub enum Maps {
    #[num_enum(default)]
    Unknwon = 0,
    Desert = 1000,
    Newplain = 1002,
    Mine01 = 1003,
    Forum = 1004,
    Arena = 1005,
    Horse = 1006,
    Star01 = 1100,
    Star02 = 1101,
    Star03 = 1102,
    Star04 = 1103,
    Star05 = 1104,
    Star10 = 1105,
    Star06 = 1106,
    Star07 = 1107,
    Star08 = 1108,
    Star09 = 1109,
    Smith = 1007,
    Grocery = 1008,
    Newbie = 1010,
    Woods = 1011,
    Sky = 1012,
    Tiger = 1013,
    Dragon = 1014,
    Island = 1015,
    Qiling = 1016,
    Canyon = 1020,
    Mine = 1021,
    Brave = 1022,
    MineOne = 1025,
    MineTwo = 1026,
    MineThree = 1027,
    MineFour = 1028,
    MineOne2 = 1029,
    MineTwo2 = 1030,
    MineThree2 = 1031,
    MineFour2 = 1032,
    Prison = 6000,
    Street = 1036,
    FactionBlack = 1037,
    Faction = 1038,
    Playground = 1039,
    Skycut = 1040,
    Skymaze = 1041,
    LineupPass = 1042,
    Lineup = 1043,
    Riskisland = 1051,
    Skymaze1 = 1060,
    Skymaze2 = 1061,
    Skymaze3 = 1062,
    Star = 1064,
    Boa = 1070,
    PArena = 1080,
    Newcanyon = 1075,
    Newwoods = 1076,
    Newdesert = 1077,
    Newisland = 1078,
    MysIsland = 1079,
    IdlandMap = 1082,
    ParenaM = 1090,
    ParenaS = 1091,
    House01 = 1098,
    House03 = 1099,
    Sanctuary = 1601,
    Task01 = 1201,
    Task02 = 1202,
    Task04 = 1204,
    Task05 = 1205,
    Task07 = 1207,
    Task08 = 1208,
    Task10 = 1210,
    Task11 = 1211,
    IslandSnail = 1212,
    DesertSnail = 1213,
    CanyonFairy = 1214,
    WoodsFairy = 1215,
    NewplainFairy = 1216,
    MineA = 1500,
    MineB = 1501,
    MineC = 1502,
    MineD = 1503,
    STask01 = 1351,
    STask02 = 1352,
    STask03 = 1353,
    STask04 = 1354,
    Slpk = 1505,
    Hhpk = 1506,
    Blpk = 1507,
    Ympk = 1508,
    Mfpk = 1509,
    Faction01 = 1550,
    Jokul01 = 1615,
    Tiemfiles = 1616,
    Dgate = 2021,
    Dsquare = 2022,
    Dcloister = 2023,
    Dsigil = 2024,
    Cordiform = 1645,
    Nhouse04 = 601,
    ArenaNone = 700,
    Fairylandpk07 = 1760,
    Halloween2007A = 1766,
    Halloween2007Boss = 1767,
    WoodsZ = 1066,
    QilingZ = 1067,
    Fairylandpk03 = 1764,
    IcecryptLev1 = 1762,
}

#[cfg(test)]
mod tests {
    use futures::FutureExt;

    use super::*;
    use crate::test_utils::*;

    #[test]
    fn nearest_walkable_candidate_recovers_from_a_blocked_spawn() {
        let candidate = nearest_walkable_candidate([10, 10], 4, |x, y| [x, y] == [8, 9]);
        assert_eq!(candidate, Some([8, 9]));
    }

    #[test]
    fn nearest_walkable_candidate_keeps_a_valid_spawn() {
        let candidate = nearest_walkable_candidate([10, 10], 4, |x, y| [x, y] == [10, 10]);
        assert_eq!(candidate, None);
    }

    #[tokio::test]
    async fn map_regions() -> Result<(), Error> {
        with_test_env(tracing::Level::TRACE, |state, _actors| {
            async move {
                let test_map_id = Maps::Arena;
                let map = state.try_map(test_map_id.into())?;
                let data_dir = std::env::var("DATA_LOCATION").map(std::path::PathBuf::from)?;
                let map_asset = data_dir.join("GameMaps").join("map").join("arena.DMap");
                let converted_asset = data_dir.join("Maps").join("arena.cmap");
                if !map_asset.exists() && !converted_asset.exists() {
                    tracing::warn!(path = ?map_asset, "Skipping map region test because arena map assets are unavailable");
                    return Ok(());
                }
                map.load().await?;
                let my_region = map.region(50, 50);
                assert!(my_region.is_some(), "Can't find a region on (50, 50)");
                let my_region = map.region(67, 50);
                assert!(my_region.is_some(), "Can't find a region on (67, 50)");
                Ok(())
            }
            .boxed()
        })
        .await
    }
}
