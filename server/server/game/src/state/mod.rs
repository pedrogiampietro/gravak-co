use crate::drops::DropCatalog;
use crate::entities::GameEntity;
use crate::guild::GuildManager;
use crate::player_shop::PlayerShopManager;
use crate::shop::ShopCatalog;
use crate::skills::SkillCatalog;
use crate::team::TeamManager;
use crate::trade::TradeManager;
use crate::world::Map;
use crate::Error;
use parking_lot::{Mutex, RwLock};
use sqlx::postgres::{PgPool, PgPoolOptions};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::debug;

mod actor_state;

pub use actor_state::ActorState;

type Maps = HashMap<u32, Map>;
type Entites = RwLock<HashMap<u32, Arc<GameEntity>>>;
type LoginTokens = Mutex<HashMap<u64, LoginToken>>;
type CreationTokens = Mutex<HashMap<u32, CreationToken>>;
type GroundItems = Mutex<HashMap<u32, GroundItem>>;
type FriendRequests = Mutex<HashMap<u32, u32>>;

#[derive(Debug, Clone, Copy)]
pub struct GroundItem {
    pub uid: u32,
    pub item_id: u32,
    pub map_id: u32,
    pub x: u16,
    pub y: u16,
    pub owner_id: u32,
    pub silver: u32,
    /// The rarity the item is picked up with (equipment that fell rare).
    pub rarity: u8,
}

#[derive(Debug)]
pub struct State {
    login_tokens: LoginTokens,
    creation_tokens: CreationTokens,
    entities: Entites,
    maps: Maps,
    shop_catalog: ShopCatalog,
    drop_catalog: DropCatalog,
    skill_catalog: SkillCatalog,
    ground_items: GroundItems,
    next_ground_item_uid: AtomicU32,
    team_manager: TeamManager,
    trade_manager: TradeManager,
    player_shop_manager: PlayerShopManager,
    guild_manager: GuildManager,
    friend_requests: FriendRequests,
    talismans: crate::bounty::TalismanBook,
    vip: crate::store::VipBook,
    /// The highscores, kept between refreshes.
    highscores: crate::highscores::Highscores,
    pool: PgPool,
}

impl State {
    /// Init The State.
    /// Should only get called once.
    pub async fn init() -> Result<Self, Error> {
        let data_dir = dotenvy::var("DATA_LOCATION")?;
        let db_url = dotenvy::var("DATABASE_URL").unwrap_or_else(|_| tq_db::DEFAULT_DATABASE_URL.to_owned());
        let pool = PgPoolOptions::new()
            .max_connections(42)
            .min_connections(4)
            .connect(&db_url)
            .await?;
        sqlx::migrate!("../../migrations")
            .run(&pool)
            .await
            .map_err(|error| Error::Other(error.to_string()))?;
        sqlx::query("SELECT sync_identity_sequences()").execute(&pool).await?;
        ensure_game_map_catalog(&pool, &data_dir).await?;
        Self::with_pool(pool).await
    }

    pub async fn with_pool(pool: PgPool) -> Result<Self, Error> {
        debug!("Loading Maps from Database");
        let db_maps = tq_db::map::Map::load_all(&pool).await?;
        let mut maps = HashMap::with_capacity(db_maps.len());
        debug!("Loaded #{} Map From Database", db_maps.len());
        for map in db_maps {
            let portals = tq_db::portal::Portal::by_map(&pool, map.id).await?;
            tracing::trace!(%map.id, portals = %portals.len(), "Loaded Portals");
            let npcs = tq_db::npc::Npc::by_map(&pool, map.id).await?;
            tracing::trace!(%map.id, npcs = %npcs.len(), "Loaded Npcs");
            let monster_spawns = tq_db::monster::MonsterSpawn::by_map(&pool, map.id).await?;
            let monster_type_ids = monster_spawns
                .iter()
                .map(|spawn| spawn.monster_type_id)
                .collect::<std::collections::BTreeSet<_>>();
            let monster_types =
                tq_db::monster::MonsterType::by_ids(&pool, &monster_type_ids.into_iter().collect::<Vec<_>>()).await?;
            let monsters = monster_spawns
                .into_iter()
                .filter_map(|spawn| {
                    monster_types
                        .iter()
                        .find(|monster_type| monster_type.id == spawn.monster_type_id)
                        .cloned()
                        .map(|monster_type| (spawn, monster_type))
                })
                .collect::<Vec<_>>();
            tracing::trace!(%map.id, monsters = %monsters.len(), "Loaded monster spawns");
            let map = Map::new(map, portals, npcs, monsters);
            maps.insert(map.id(), map);
        }

        let guild_manager = GuildManager::load(&pool).await?;
        let donors = crate::nobility::load(&pool).await?;
        tracing::info!(donors, "Loaded the nobility board");
        let drop_catalog = DropCatalog::load(&pool).await?;
        tracing::info!(drops = drop_catalog.len(), "Loaded monster drop catalog");
        let state = Self {
            login_tokens: Default::default(),
            creation_tokens: Default::default(),
            entities: Default::default(),
            maps,
            shop_catalog: ShopCatalog::from_environment(),
            drop_catalog,
            skill_catalog: SkillCatalog::from_environment(),
            ground_items: Default::default(),
            next_ground_item_uid: AtomicU32::new(8_000_000),
            team_manager: TeamManager::default(),
            trade_manager: TradeManager::default(),
            player_shop_manager: PlayerShopManager::default(),
            guild_manager,
            friend_requests: Default::default(),
            talismans: Default::default(),
            vip: Default::default(),
            highscores: Default::default(),
            pool,
        };
        Ok(state)
    }

    /// Get access to the database pool
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub fn shop_catalog(&self) -> &ShopCatalog {
        &self.shop_catalog
    }

    pub fn drop_catalog(&self) -> &DropCatalog {
        &self.drop_catalog
    }

    pub fn skill_catalog(&self) -> &SkillCatalog {
        &self.skill_catalog
    }

    /// The VIP of the characters online.
    pub fn vip(&self) -> &crate::store::VipBook {
        &self.vip
    }

    /// The highscores, kept between refreshes.
    pub fn highscores(&self) -> &crate::highscores::Highscores {
        &self.highscores
    }

    /// The bounty talismans of the characters online with a task.
    pub fn talismans(&self) -> &crate::bounty::TalismanBook {
        &self.talismans
    }

    pub fn team_manager(&self) -> &TeamManager {
        &self.team_manager
    }

    pub fn trade_manager(&self) -> &TradeManager {
        &self.trade_manager
    }

    pub fn player_shop_manager(&self) -> &PlayerShopManager {
        &self.player_shop_manager
    }

    pub fn guild_manager(&self) -> &GuildManager {
        &self.guild_manager
    }

    pub fn set_friend_request(&self, target_id: u32, requester_id: u32) -> Option<u32> {
        self.friend_requests.lock().insert(target_id, requester_id)
    }

    pub fn take_friend_request(&self, target_id: u32) -> Option<u32> {
        self.friend_requests.lock().remove(&target_id)
    }

    pub fn clear_friend_requests_for(&self, character_id: u32) {
        self.friend_requests
            .lock()
            .retain(|target_id, requester_id| *target_id != character_id && *requester_id != character_id);
    }

    pub fn maps(&self) -> &Maps {
        &self.maps
    }

    pub fn try_map(&self, map_id: u32) -> Result<&Map, Error> {
        self.maps.get(&map_id).ok_or(Error::MapNotFound)
    }

    pub fn insert_ground_item(&self, item: GroundItem) {
        self.ground_items.lock().insert(item.uid, item);
    }

    pub fn ground_item(&self, uid: u32) -> Option<GroundItem> {
        self.ground_items.lock().get(&uid).copied()
    }

    pub fn take_ground_item(&self, uid: u32) -> Option<GroundItem> {
        self.ground_items.lock().remove(&uid)
    }

    pub fn next_ground_item_uid(&self) -> u32 {
        self.next_ground_item_uid.fetch_add(1, Ordering::Relaxed)
    }

    pub fn insert_entity(&self, entity: Arc<GameEntity>) {
        let mut entities = self.entities.write();
        entities.insert(entity.id(), entity);
    }

    pub fn remove_entity(&self, id: u32) {
        let mut entities = self.entities.write();
        entities.remove(&id);
    }

    pub fn with_entity<F, R>(&self, id: u32, f: F) -> Option<R>
    where
        F: FnOnce(&GameEntity) -> R,
    {
        let entities = self.entities.read();
        entities.get(&id).map(|v| f(v))
    }

    pub fn entity(&self, id: u32) -> Option<Arc<GameEntity>> {
        self.entities.read().get(&id).cloned()
    }

    pub fn entities(&self) -> Vec<Arc<GameEntity>> {
        let lock = self.entities.read();
        let values = lock.values();
        values.cloned().collect()
    }

    /// Persist the runtime state of every connected character without removing
    /// the entities from the world. This is intentionally separate from
    /// `clean_up`, which drains the state during a full server shutdown.
    pub async fn autosave_characters(&self) -> Result<usize, Error> {
        let mut saved = 0;
        for entity in self.entities() {
            if let GameEntity::Character(character) = entity.as_ref() {
                character.save(self).await?;
                saved += 1;
            }
        }
        Ok(saved)
    }

    /// Generate a new Login Token.
    ///
    /// The token will be stored internally, and can be later removed by calling
    /// [`TokenStore::remove_login_token`].
    pub fn generate_login_token(&self, account_id: u32, realm_id: u32) -> Result<GeneratedLoginToken, crate::Error> {
        let token = rand::random();
        self.login_tokens
            .lock()
            .insert(token, LoginToken { account_id, realm_id });
        Ok(GeneratedLoginToken { token })
    }

    /// Remove a Login Token.
    pub fn remove_login_token(&self, token: u64) -> Result<LoginToken, crate::Error> {
        self.login_tokens
            .lock()
            .remove(&token)
            .ok_or(crate::Error::LoginTokenNotFound)
    }

    /// Store a new CreationToken.
    /// The token will be stored internally, and can be later removed by calling
    /// [`TokenStore::remove_creation_token`].
    pub fn store_creation_token(&self, token: u32, account_id: u32, realm_id: u32) -> Result<(), crate::Error> {
        self.creation_tokens
            .lock()
            .insert(token, CreationToken { account_id, realm_id });
        Ok(())
    }

    /// Remove a CreationToken.
    pub fn remove_creation_token(&self, token: u32) -> Result<CreationToken, crate::Error> {
        self.creation_tokens
            .lock()
            .remove(&token)
            .ok_or(crate::Error::CreationTokenNotFound)
    }

    fn drain_entities(&self) -> Vec<Arc<GameEntity>> {
        let mut entities = self.entities.write();
        let values = entities.drain();
        values.map(|(_, v)| v).collect()
    }

    /// Cleanup the state, close all connections and updating the database.
    pub async fn clean_up(self) -> Result<(), Error> {
        debug!("Clean up ..");
        debug!("Saving Entities data ..");
        let entities = self.drain_entities();
        let mut saved = 0;
        for e in entities {
            match e.as_ref() {
                GameEntity::Character(character) => {
                    character.save(&self).await?;
                    saved += 1;
                },
                GameEntity::Npc(_) => {
                    // Do nothing for now
                },
                GameEntity::Monster(_) => {
                    // Monsters are runtime entities; their definitions and
                    // spawns are persisted separately.
                },
            }
        }
        tracing::info!(target: "server", characters = saved, "Saved everyone online");
        self.pool().close().await;
        debug!("Closed Database Connection ..");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::Character;
    use crate::packets::{BaseClass, BodyType, MsgRegister};
    use crate::systems::Screen;
    use tq_network::Actor;

    #[tokio::test]
    async fn autosave_persists_connected_character_runtime_state() -> Result<(), Error> {
        let pool = tq_db::testing::pool().await;
        let state = State::with_pool(pool.clone()).await?;

        let (tx, _rx) = tokio::sync::mpsc::channel(8);
        let actor = Actor::<ActorState>::new(tx);
        actor.set_id(1);
        let inner = MsgRegister::build_character_with(
            "autosave-test".to_owned(),
            BodyType::MuscularMale,
            BaseClass::Trojan,
            1,
            1,
        )?;
        inner.save(&pool).await?;
        let inner = tq_db::character::Character::from_account(&pool, 1)
            .await?
            .expect("test character should exist");
        let database_id = inner.character_id;
        let character = Character::new(actor.handle(), inner);
        character.entity().set_level(12);
        actor.update(character, Screen::new(actor.handle()));
        state.insert_entity(actor.entity());

        assert_eq!(state.autosave_characters().await?, 1);
        let persisted = tq_db::character::Character::by_id(&pool, database_id).await?;
        assert_eq!(persisted.level, 12);

        Ok(())
    }
}

async fn ensure_game_map_catalog(pool: &PgPool, data_dir: &str) -> Result<(), Error> {
    fn read_u32(bytes: &[u8], offset: &mut usize) -> Result<u32, Error> {
        let end = offset.saturating_add(4);
        let raw = bytes
            .get(*offset..end)
            .ok_or_else(|| Error::Other("GameMap.dat ended unexpectedly".to_owned()))?;
        *offset = end;
        Ok(u32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]]))
    }

    let catalog_path = PathBuf::from(data_dir).join("GameMaps").join("GameMap.dat");
    let bytes = tokio::fs::read(&catalog_path).await?;
    let mut offset = 0;
    let count = read_u32(&bytes, &mut offset)?;
    if count > 4096 {
        return Err(Error::Other("GameMap.dat contains too many maps".to_owned()));
    }

    let mut imported = 0_u32;
    for _ in 0..count {
        let map_id = read_u32(&bytes, &mut offset)?;
        let path_length = read_u32(&bytes, &mut offset)? as usize;
        if path_length == 0 || path_length > 512 {
            return Err(Error::Other("GameMap.dat contains an invalid path length".to_owned()));
        }
        let raw_path = bytes
            .get(offset..offset + path_length)
            .ok_or_else(|| Error::Other("GameMap.dat path is truncated".to_owned()))?;
        offset += path_length;
        let _tile_size = read_u32(&bytes, &mut offset)?;
        let path = String::from_utf8(raw_path.to_vec())
            .map_err(|error| Error::Other(format!("GameMap.dat path is not UTF-8: {error}")))?
            .trim_end_matches('\0')
            .replace('\\', "/");
        if !path.starts_with("map/") || !path.ends_with(".DMap") {
            continue;
        }
        let dmap_path = PathBuf::from(data_dir).join("GameMaps").join(&path);
        if !dmap_path.try_exists()? {
            continue;
        }
        let compressed_path = path.strip_suffix(".DMap").unwrap_or(&path).to_owned() + ".cmap";
        sqlx::query(
            "INSERT INTO maps \
             (id, map_id, path, revive_point_x, revive_point_y, flags, weather, reborn_map, color) \
             VALUES ($1, $2, $3, 430, 380, 0, 0, $4, 4294967295) ON CONFLICT DO NOTHING",
        )
        .bind(map_id as i32)
        .bind(map_id as i32)
        .bind(compressed_path)
        .bind(map_id as i32)
        .execute(pool)
        .await?;
        imported += 1;
    }
    tracing::info!(maps = imported, "Validated GameMap.dat catalog");
    Ok(())
}

#[derive(Clone, Debug)]
pub struct LoginToken {
    pub account_id: u32,
    pub realm_id: u32,
}

#[derive(Clone, Debug)]
pub struct CreationToken {
    pub account_id: u32,
    pub realm_id: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GeneratedLoginToken {
    pub token: u64,
}

/// Listens for the server shutdown signal.
///
/// Shutdown is signalled using a `broadcast::Receiver`. Only a single value is
/// ever sent. Once a value has been sent via the broadcast channel, the server
/// should shutdown.
///
/// The `Shutdown` struct listens for the signal and tracks that the signal has
/// been received. Callers may query for whether the shutdown signal has been
/// received or not.
#[derive(Debug)]
pub struct Shutdown {
    /// `true` if the shutdown signal has been received
    shutdown: bool,

    /// The receive half of the channel used to listen for shutdown.
    notify: broadcast::Receiver<()>,
}

impl Shutdown {
    /// Create a new `Shutdown` backed by the given `broadcast::Receiver`.
    pub fn new(notify: broadcast::Receiver<()>) -> Shutdown {
        Shutdown {
            shutdown: false,
            notify,
        }
    }

    /// Receive the shutdown notice, waiting if necessary.
    pub async fn recv(&mut self) {
        // If the shutdown signal has already been received, then return
        // immediately.
        if self.shutdown {
            return;
        }

        // Cannot receive a "lag error" as only one value is ever sent.
        let _ = self.notify.recv().await;

        // Remember that the signal has been received.
        self.shutdown = true;
    }
}
