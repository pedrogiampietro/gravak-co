use crate::combat::{next_roll, CombatStats};
use crate::entities::{Entity, GameEntity, StatusEffect, StatusEffects};
use crate::packets::{ActionType, MsgAction, MsgMapInfo, MsgPlayer, MsgWeather};
use crate::shop::ShopCatalog;
use crate::systems::Screen;
use crate::utils::LoHi;
use crate::Error;
use arc_swap::ArcSwapWeak;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU16, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Weak};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tq_network::ActorHandle;

/// This struct encapsulates the game character for a player. The player
/// controls the character as the protagonist of the Conquer Online storyline.
/// The character is the persona of the player who controls it. The persona can
/// be altered using different avatars, hairstyles, and body types. The player
/// also controls the character's professions and abilities.
#[derive(Debug)]
pub struct Character {
    inner: tq_db::character::Character,
    entity: Entity,
    owner: ActorHandle,
    elevation: AtomicU16,
    silver: AtomicU64,
    nobility_donation: AtomicU64,
    equipped_title: AtomicU32,
    experience: AtomicU64,
    warehouse_silver: AtomicU64,
    active_shop_id: AtomicU32,
    active_storage_id: AtomicU32,
    /// Where the Market controller sends the player back to: the map and
    /// cell they were on when a Conductress took them to the Market.
    market_return: Mutex<Option<(u32, (u16, u16))>>,
    /// Current profession; promotions change it after login.
    class: std::sync::atomic::AtomicU8,
    /// Profession before the rebirth, and the number of rebirths.
    previous_class: std::sync::atomic::AtomicU8,
    rebirths: std::sync::atomic::AtomicU8,
    /// Virtue points, spent at Simon for the Labyrinth.
    virtue: AtomicU32,
    cps: AtomicU64,
    hair_style: AtomicU16,
    /// The spouse's name, or "None".
    spouse: Mutex<String>,
    /// Classic kill mode: 0 free, 1 safe, 2 team, 3 arrestment.
    kill_mode: AtomicU16,
    /// PK points: red name from 30, black name from 100.
    kill_points: AtomicU16,
    /// When the character died; `None` while alive.
    died_at: Mutex<Option<std::time::Instant>>,
    /// Until when the character, just revived, takes no damage.
    protected_until: Mutex<Option<std::time::Instant>>,
    /// The name over the character's booth while it is open.
    booth_name: Mutex<Option<String>>,
    /// The status timers last sent to the client.
    timers_sent: Mutex<Vec<(u64, u64)>>,
    /// The XP points the client last heard of.
    xp_sent: std::sync::atomic::AtomicU8,
    /// Lucky time left, in seconds (`lucky.rs`).
    lucky_time: AtomicU32,
    /// Monsters slain in the current Cyclone or Superman (the KO counter).
    ko_count: AtomicU32,
    /// How fast the character has been moving (`movement_pace.rs`).
    pace: crate::movement_pace::Pace,
    /// The most KOs in one Cyclone or Superman written this session.
    best_ko: AtomicU32,
    /// Where the character started praying (map, x, y).
    prayer_origin: Mutex<Option<(u32, u16, u16)>>,
    /// The blue flashing name after killing someone.
    flashing_until: Mutex<Option<std::time::Instant>>,
    /// When the next PK point wears off.
    next_pk_decay: Mutex<Option<std::time::Instant>>,
    /// Stamina (0-100, 150 with Heaven's Blessing).
    stamina: std::sync::atomic::AtomicU8,
    /// XP points; at 100 the XP circle lights up.
    xp_points: std::sync::atomic::AtomicU8,
    last_xp_up: Mutex<std::time::Instant>,
    /// Until when the lit XP circle waits for an XP skill.
    xp_ready_until: Mutex<Option<std::time::Instant>>,
    last_regeneration: Mutex<std::time::Instant>,
    inventory: Mutex<Vec<InventoryItem>>,
    equipment: Mutex<Vec<InventoryItem>>,
    warehouse: Mutex<Vec<InventoryItem>>,
    attributes: Mutex<BaseAttributes>,
    skills: Mutex<HashMap<u32, SkillProgress>>,
    weapon_proficiencies: Mutex<HashMap<u32, WeaponProficiency>>,
    statuses: StatusEffects,
    double_experience_until: Mutex<Option<SystemTime>>,
    /// How many times the experience while the boost above lasts.
    experience_multiplier: std::sync::atomic::AtomicU8,
    heaven_blessing_until: Mutex<Option<SystemTime>>,
    refineries: Mutex<HashMap<u32, crate::refinery::RefineryEffect>>,
    artifacts: Mutex<HashMap<u32, crate::refinery::ArtifactEffect>>,
    screen: ArcSwapWeak<Screen>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct InventoryItem {
    pub uid: u32,
    pub item_type: u32,
    pub position: u8,
    pub durability: u16,
    pub max_durability: u16,
    pub plus: u8,
    /// Custom rarity: 0 common, 1 rare, 2 epic, 3 legendary, 4 mythic.
    /// This is independent from classic item quality and plus/composition.
    pub rarity: u8,
    /// Classic 5065 gem codes. `0` means no gem; `255` means the socket is
    /// open.
    pub gem_one: u8,
    pub gem_two: u8,
    pub socket_one: u8,
    pub socket_two: u8,
    /// Seed used to generate the item's custom rarity bonus slots.
    pub rarity_roll: u8,
    /// Blessing: 1, 3, 5 or 7 percent less damage taken.
    pub bless: u8,
    /// Life added by a WuxingOven enchantment (0-255).
    pub enchant_hp: u8,
    /// Dye of a helmet, armor or shield (3-9; 0 is the default 3).
    pub color: u8,
}

/// Quality is encoded by the final digit of the classic equipment type and
/// is independent from the item's upgrade tier (`plus`).
fn item_rarity_bonus_percent(rarity: u8) -> u32 {
    match rarity {
        1 => 5,
        2 => 10,
        3 => 15,
        4 => 25,
        _ => 0,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TradeTransfer {
    pub left_removed: Vec<InventoryItem>,
    pub right_removed: Vec<InventoryItem>,
    pub left_received: Vec<InventoryItem>,
    pub right_received: Vec<InventoryItem>,
    pub left_silver: u64,
    pub right_silver: u64,
}

/// Normal gem families used by the 5065 composition service. The last digit
/// identifies the tier: 1 normal, 2 refined, 3 super.
pub const GEM_FAMILY_BASES: [u32; 8] = [700_000, 700_010, 700_020, 700_030, 700_040, 700_050, 700_060, 700_070];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemUpgradeOutcome {
    Upgraded(InventoryItem),
    Failed(InventoryItem),
    MaxLevel(InventoryItem),
    NotFound,
    MissingMeteor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RarityUpgradeOutcome {
    Upgraded(InventoryItem),
    Failed(InventoryItem),
    MaxRarity(InventoryItem),
    NotFound,
    MissingDragonBall,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RarityBonusOutcome {
    Rerolled(InventoryItem),
    NotFound,
    NoRarity,
    MissingDragonBall,
}

/// Meteor cost and success chance for the next classic + level.
pub fn item_upgrade_cost_and_chance(plus: u8) -> Option<(u8, u8)> {
    Some(match plus {
        0 => (1, 100),
        1 => (1, 95),
        2 => (1, 90),
        3 => (2, 80),
        4 => (2, 70),
        5 => (3, 60),
        6 => (3, 50),
        7 => (4, 40),
        8 => (5, 30),
        9 => (5, 25),
        10 => (6, 20),
        11 => (8, 15),
        _ => return None,
    })
}

/// DragonBall cost and success chance for the next custom rarity.
pub fn rarity_upgrade_cost_and_chance(rarity: u8) -> Option<(u8, u8)> {
    Some(match rarity {
        0 => (1, 75),
        1 => (2, 55),
        2 => (3, 35),
        3 => (5, 20),
        _ => return None,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QualityUpgradeOutcome {
    Upgraded(InventoryItem),
    Failed(InventoryItem),
    MaxQuality(InventoryItem),
    NotFound,
    InvalidQuality(InventoryItem),
    MissingDragonBall,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocketOutcome {
    Opened(InventoryItem),
    Inserted { item: InventoryItem, gem: InventoryItem },
    Removed(InventoryItem),
    NotFound,
    InvalidGem,
    InvalidSlot,
    SocketClosed,
    SocketOccupied,
    MissingDragonBall,
    DualSockets,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefineryOutcome {
    Applied {
        target: InventoryItem,
        material: InventoryItem,
        effect: crate::refinery::RefineryEffect,
    },
    NotFound,
    InvalidTarget,
    InvalidMaterial,
    Incompatible,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtifactOutcome {
    Applied {
        target: InventoryItem,
        material: InventoryItem,
        catalyst: InventoryItem,
        effect: crate::refinery::ArtifactEffect,
    },
    NotFound,
    InvalidTarget,
    InvalidMaterial,
    Incompatible,
    MissingMeteor,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GemComposeOutcome {
    Composed {
        consumed: Vec<InventoryItem>,
        created: InventoryItem,
        silver: u32,
    },
    InvalidGem,
    MissingGems,
    InsufficientSilver,
    InventoryFull,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttributeKind {
    Strength,
    Agility,
    Vitality,
    Spirit,
}

impl AttributeKind {
    pub fn parse(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "strength" | "str" | "forca" | "força" => Some(Self::Strength),
            "agility" | "agi" | "agilidade" => Some(Self::Agility),
            "vitality" | "vit" | "vitalidade" => Some(Self::Vitality),
            "spirit" | "spi" | "espirito" | "espírito" => Some(Self::Spirit),
            _ => None,
        }
    }

    fn column(self) -> &'static str {
        match self {
            Self::Strength => "strength",
            Self::Agility => "agility",
            Self::Vitality => "vitality",
            Self::Spirit => "spirit",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Strength => "Strength",
            Self::Agility => "Agility",
            Self::Vitality => "Vitality",
            Self::Spirit => "Spirit",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttributeAllocation {
    Allocated {
        kind: AttributeKind,
        amount: u16,
        remaining: u16,
    },
    InsufficientPoints {
        available: u16,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BaseAttributes {
    strength: u16,
    agility: u16,
    vitality: u16,
    spirit: u16,
    points: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SkillProgress {
    pub skill_id: u32,
    pub level: u16,
    pub experience: u64,
    pub proficiency: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WeaponProficiency {
    pub weapon_family: u32,
    pub level: u16,
    pub experience: u64,
    pub proficiency: u32,
}

// Classic 5065 weapon-proficiency thresholds, indexed by the current level.
// The final zero is the protocol's level-20 cap sentinel; progression stops
// before consulting it.
const WEAPON_PROFICIENCY_LEVEL_EXPERIENCE: [u32; 21] = [
    0,
    1_200,
    68_000,
    250_000,
    640_000,
    1_600_000,
    4_000_000,
    10_000_000,
    22_000_000,
    40_000_000,
    90_000_000,
    95_000_000,
    142_500_000,
    213_750_000,
    320_625_000,
    480_937_500,
    721_406_250,
    1_082_109_375,
    1_623_164_063,
    2_100_000_000,
    0,
];

fn weapon_proficiency_threshold(level: u16) -> u32 {
    WEAPON_PROFICIENCY_LEVEL_EXPERIENCE
        .get(usize::from(level))
        .copied()
        .unwrap_or(0)
}

/// Gem families by the tens digit of the gem code (Phoenix 0x, Dragon 1x,
/// Fury 2x, Rainbow 3x, Kylin 4x, Violet 5x, Moon 6x, Tortoise 7x).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GemFamily {
    /// Magic damage.
    Phoenix = 0,
    /// Hit rate.
    Fury = 2,
    /// Experience.
    Rainbow = 3,
    /// Slower durability loss.
    Kylin = 4,
    /// Weapon proficiency.
    Violet = 5,
    /// Skill experience.
    Moon = 6,
}

/// Redux `CombatStatistics.AddGemByID`: the percent a Normal, Refined or
/// Super gem (code tier 1, 2, 3) of `family` gives.
pub fn gem_percent(family: GemFamily, code: u8) -> u32 {
    if code / 10 != family as u8 || !(1..=3).contains(&(code % 10)) {
        return 0;
    }
    let tiers: [u32; 3] = match family {
        GemFamily::Phoenix | GemFamily::Fury => [5, 10, 15],
        GemFamily::Rainbow => [10, 15, 25],
        GemFamily::Kylin => [50, 100, 200],
        GemFamily::Violet => [30, 50, 100],
        GemFamily::Moon => [15, 30, 50],
    };
    tiers[usize::from(code % 10) - 1]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PurchaseOutcome {
    Bought(InventoryItem),
    InsufficientSilver,
    InventoryFull,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaleOutcome {
    Sold { item: InventoryItem, silver: u32 },
    ItemNotFound,
    NoValue,
    SilverLimit,
}

impl Character {
    pub fn new(owner: ActorHandle, inner: tq_db::character::Character) -> Self {
        let class = inner.current_class as u8;
        let previous_class = inner.previous_class as u8;
        let rebirths = inner.rebirths as u8;
        let virtue = inner.virtue.max(0) as u32;
        let cps = inner.cps.max(0) as u64;
        let hair_style = inner.hair_style.max(0) as u16;
        let kill_points = inner.kill_points.max(0) as u16;
        let persisted_double_experience_until = (inner.double_experience_until > 0)
            .then(|| UNIX_EPOCH.checked_add(Duration::from_secs(inner.double_experience_until as u64)))
            .flatten();
        let persisted_heaven_blessing_until = (inner.heaven_blessing_until > 0)
            .then(|| UNIX_EPOCH.checked_add(Duration::from_secs(inner.heaven_blessing_until as u64)))
            .flatten();
        let entity = Entity::from(&inner);
        entity.set_flags(entity.flags() | crate::death::name_flags(kill_points));
        let silver = inner.silver.max(0) as u64;
        let nobility_donation = inner.nobility_donation.max(0) as u64;
        let equipped_title = inner.equipped_title.max(0) as u32;
        let experience = inner.experience.max(0) as u64;
        let mut attributes = BaseAttributes {
            strength: inner.strength.max(0) as u16,
            agility: inner.agility.max(0) as u16,
            vitality: inner.vitality.max(0) as u16,
            spirit: inner.spirit.max(0) as u16,
            points: inner.attribute_points.max(0) as u16,
        };
        // Before the first rebirth the attributes follow the class table
        // (Redux); older characters are brought in line when they log in.
        let class_stats = (inner.rebirths == 0)
            .then(|| crate::stats::for_level(class, inner.level.max(1) as u16))
            .flatten();
        if let Some((strength, vitality, agility, spirit)) = class_stats {
            attributes.strength = strength;
            attributes.vitality = vitality;
            attributes.agility = agility;
            attributes.spirit = spirit;
        }
        let experience_multiplier = u8::try_from(inner.experience_multiplier.clamp(2, 10)).unwrap_or(2);
        let lucky_time = u32::try_from(inner.lucky_time.clamp(0, i64::from(crate::lucky::MAX_LUCKY_TIME))).unwrap_or(0);
        let now = SystemTime::now();
        let double_experience_active = persisted_double_experience_until.is_some_and(|expires_at| expires_at > now);
        let heaven_blessing_active = persisted_heaven_blessing_until.is_some_and(|expires_at| expires_at > now);
        if double_experience_active {
            entity.set_flags(entity.flags() | crate::entities::Flags::DOUBLE_EXPERIENCE);
        }
        if heaven_blessing_active {
            entity.set_flags(entity.flags() | crate::entities::Flags::HEAVEN_BLESSING);
        }
        let character = Self {
            entity,
            owner,
            inner,
            elevation: Default::default(),
            silver: AtomicU64::new(silver),
            nobility_donation: AtomicU64::new(nobility_donation),
            equipped_title: AtomicU32::new(equipped_title),
            experience: AtomicU64::new(experience),
            warehouse_silver: AtomicU64::new(0),
            active_shop_id: AtomicU32::new(0),
            active_storage_id: AtomicU32::new(0),
            market_return: Mutex::new(None),
            class: std::sync::atomic::AtomicU8::new(class),
            previous_class: std::sync::atomic::AtomicU8::new(previous_class),
            rebirths: std::sync::atomic::AtomicU8::new(rebirths),
            virtue: AtomicU32::new(virtue),
            cps: AtomicU64::new(cps),
            hair_style: AtomicU16::new(hair_style),
            spouse: Mutex::new("None".to_owned()),
            kill_points: AtomicU16::new(kill_points),
            died_at: Mutex::new(None),
            protected_until: Mutex::new(None),
            booth_name: Mutex::new(None),
            timers_sent: Mutex::new(Vec::new()),
            lucky_time: AtomicU32::new(lucky_time),
            ko_count: AtomicU32::new(0),
            pace: Default::default(),
            best_ko: AtomicU32::new(0),
            prayer_origin: Mutex::new(None),
            xp_sent: std::sync::atomic::AtomicU8::new(u8::MAX),
            flashing_until: Mutex::new(None),
            next_pk_decay: Mutex::new((kill_points > 0).then(|| std::time::Instant::now() + crate::death::PK_DECAY)),
            stamina: std::sync::atomic::AtomicU8::new(crate::vitals::MAX_STAMINA),
            xp_points: std::sync::atomic::AtomicU8::new(0),
            last_xp_up: Mutex::new(std::time::Instant::now()),
            xp_ready_until: Mutex::new(None),
            last_regeneration: Mutex::new(std::time::Instant::now()),
            kill_mode: AtomicU16::new(1),
            inventory: Mutex::new(Vec::new()),
            equipment: Mutex::new(Vec::new()),
            warehouse: Mutex::new(Vec::new()),
            attributes: Mutex::new(attributes),
            skills: Mutex::new(HashMap::new()),
            weapon_proficiencies: Mutex::new(HashMap::new()),
            statuses: StatusEffects::default(),
            double_experience_until: Mutex::new(
                double_experience_active
                    .then_some(persisted_double_experience_until)
                    .flatten(),
            ),
            experience_multiplier: std::sync::atomic::AtomicU8::new(experience_multiplier),
            heaven_blessing_until: Mutex::new(
                heaven_blessing_active
                    .then_some(persisted_heaven_blessing_until)
                    .flatten(),
            ),
            refineries: Mutex::new(HashMap::new()),
            artifacts: Mutex::new(HashMap::new()),
            screen: Default::default(),
        };
        if class_stats.is_some() {
            character.recalculate_resources(true);
        }
        character
    }

    pub fn set_screen(&self, screen: Weak<Screen>) {
        self.screen.store(screen);
    }

    pub fn try_screen(&self) -> Result<Arc<Screen>, Error> {
        self.screen.load().upgrade().ok_or(Error::ScreenNotFound)
    }

    #[inline]
    pub fn owner(&self) -> ActorHandle {
        self.owner.clone()
    }

    #[inline]
    pub fn entity(&self) -> &Entity {
        &self.entity
    }

    #[inline]
    pub fn id(&self) -> u32 {
        self.entity.id()
    }

    /// Returns the database character identifier used by persistence tables.
    /// The protocol/entity ID is offset by `CHARACTER_ID_MIN`.
    #[inline]
    pub fn database_id(&self) -> i32 {
        self.inner.character_id
    }

    pub fn elevation(&self) -> u16 {
        self.elevation.load(Ordering::Relaxed)
    }

    pub fn set_elevation(&self, value: u16) {
        self.elevation.store(value, Ordering::Relaxed);
    }

    pub fn hair_style(&self) -> u16 {
        self.hair_style.load(Ordering::Relaxed)
    }

    pub fn spouse(&self) -> String {
        self.spouse.lock().clone()
    }

    pub fn set_spouse(&self, name: impl Into<String>) {
        *self.spouse.lock() = name.into();
    }

    /// Reads the spouse from the marriages table.
    pub async fn load_spouse(&self, pool: &sqlx::PgPool) -> Result<(), Error> {
        let name = sqlx::query_scalar::<_, String>("SELECT spouse_name FROM marriages WHERE character_id = $1")
            .bind(self.inner.character_id)
            .fetch_optional(pool)
            .await?;
        self.set_spouse(name.unwrap_or_else(|| "None".to_owned()));
        Ok(())
    }

    pub fn set_hair_style(&self, hair_style: u16) {
        self.hair_style.store(hair_style, Ordering::Relaxed);
    }

    /// Resets the four attributes like a rebirth and frees the rest as
    /// points to allot (CelestialTao).
    pub fn reallot_attributes(&self) {
        let mut attributes = self.attributes.lock();
        let total = attributes.strength
            + attributes.agility
            + attributes.vitality
            + attributes.spirit
            + attributes.points;
        *attributes = BaseAttributes {
            strength: 0,
            agility: 2,
            vitality: 8,
            spirit: 0,
            points: total.saturating_sub(10),
        };
        drop(attributes);
        self.recalculate_resources(false);
    }

    /// Sets the plus (+n) of a worn item.
    pub async fn set_equipment_plus(
        &self,
        pool: &sqlx::PgPool,
        position: u8,
        plus: u8,
    ) -> Result<Option<InventoryItem>, Error> {
        let Some(mut item) = self.equipment_item(position) else {
            return Ok(None);
        };
        item.plus = plus;
        sqlx::query(
            "INSERT INTO item_upgrades (item_uid, plus, gem_one, gem_two, socket_one, socket_two, rarity)
             VALUES ($1, $2, $3, $4, $5, $6, $7)
             ON CONFLICT(item_uid) DO UPDATE SET plus = excluded.plus",
        )
        .bind(i64::from(item.uid))
        .bind(i64::from(item.plus))
        .bind(i64::from(item.gem_one))
        .bind(i64::from(item.gem_two))
        .bind(i64::from(item.socket_one))
        .bind(i64::from(item.socket_two))
        .bind(i64::from(item.rarity))
        .execute(pool)
        .await?;
        if let Some(existing) = self.equipment.lock().iter_mut().find(|owned| owned.uid == item.uid) {
            *existing = item;
        }
        Ok(Some(item))
    }

    pub fn avatar(&self) -> u16 {
        self.inner.avatar as u16
    }

    pub fn silver(&self) -> u64 {
        self.silver.load(Ordering::Relaxed)
    }

    pub fn nobility_donation(&self) -> u64 {
        self.nobility_donation.load(Ordering::Relaxed)
    }

    pub fn equipped_title(&self) -> u32 {
        self.equipped_title.load(Ordering::Relaxed)
    }

    pub fn equipped_title_name(&self) -> &'static str {
        crate::titles::name(self.equipped_title())
    }

    pub async fn equip_title(
        &self,
        pool: &sqlx::PgPool,
        title_id: u32,
    ) -> Result<crate::titles::EquipOutcome, Error> {
        use crate::titles::{EquipOutcome, NONE};
        if title_id != NONE {
            let Some(definition) = crate::titles::definition(title_id) else {
                return Ok(EquipOutcome::UnknownTitle);
            };
            if !crate::titles::is_unlocked(
                definition,
                self.entity.level().min(u16::from(u8::MAX)) as u8,
                self.nobility_status(),
            ) {
                return Ok(EquipOutcome::Locked);
            }
        }
        sqlx::query("UPDATE characters SET equipped_title = $1 WHERE character_id = $2")
            .bind(i64::from(title_id))
            .bind(self.inner.character_id)
            .execute(pool)
            .await?;
        self.equipped_title.store(title_id, Ordering::Relaxed);
        Ok(EquipOutcome::Equipped(title_id))
    }

    pub fn nobility_status(&self) -> crate::nobility::NobilityStatus {
        crate::nobility::status_for(self.nobility_donation())
    }

    /// Atomically exchanges silver for nobility donation points. The database
    /// update is guarded by both balances, so a concurrent command cannot
    /// spend the same silver twice or overflow the persisted donation.
    pub async fn donate_nobility(
        &self,
        pool: &sqlx::PgPool,
        amount: u32,
    ) -> Result<crate::nobility::DonateOutcome, Error> {
        use crate::nobility::{DonateOutcome, MIN_DONATION, MIN_LEVEL};
        if self.entity.level() < u16::from(MIN_LEVEL) {
            return Ok(DonateOutcome::InvalidLevel);
        }
        if u64::from(amount) < MIN_DONATION {
            return Ok(DonateOutcome::InvalidAmount);
        }
        let amount = i64::from(amount);
        let max_donation = i64::MAX.saturating_sub(amount);
        let Some((silver, donation)) = sqlx::query_as::<_, (i64, i64)>(
            "UPDATE characters
             SET silver = silver - $1, nobility_donation = nobility_donation + $2
             WHERE character_id = $3 AND silver >= $4 AND nobility_donation <= $5
             RETURNING silver, nobility_donation",
        )
        .bind(amount)
        .bind(amount)
        .bind(self.inner.character_id)
        .bind(amount)
        .bind(max_donation)
        .fetch_optional(pool)
        .await?
        else {
            return Ok(DonateOutcome::InsufficientSilver);
        };
        let silver = silver.max(0) as u64;
        let donation = donation.max(0) as u64;
        self.silver.store(silver, Ordering::Relaxed);
        self.nobility_donation.store(donation, Ordering::Relaxed);
        Ok(DonateOutcome::Donated(crate::nobility::status_for(donation)))
    }

    pub async fn grant_silver(&self, pool: &sqlx::PgPool, amount: u32) -> Result<u32, Error> {
        if amount == 0 {
            return Ok(self.silver().min(u64::from(u32::MAX)) as u32);
        }
        let balance = sqlx::query_scalar::<_, i64>(
            "UPDATE characters SET silver = silver + $1 WHERE character_id = $2 AND silver <= $3 RETURNING silver",
        )
        .bind(i64::from(amount))
        .bind(self.inner.character_id)
        .bind(i64::from(u32::MAX - amount))
        .fetch_optional(pool)
        .await?
        .unwrap_or_else(|| self.silver().min(u64::from(u32::MAX)) as i64);
        self.silver.store(balance.max(0) as u64, Ordering::Relaxed);
        Ok(balance.max(0).min(i64::from(u32::MAX)) as u32)
    }

    /// Debits `amount` silver only when the balance covers it. Returns the new
    /// balance, or `None` without charging anything when it does not.
    pub async fn spend_silver(&self, pool: &sqlx::PgPool, amount: u32) -> Result<Option<u32>, Error> {
        if amount == 0 {
            return Ok(Some(self.silver().min(u64::from(u32::MAX)) as u32));
        }
        let balance = sqlx::query_scalar::<_, i64>(
            "UPDATE characters SET silver = silver - $1 WHERE character_id = $2 AND silver >= $3 RETURNING silver",
        )
        .bind(i64::from(amount))
        .bind(self.inner.character_id)
        .bind(i64::from(amount))
        .fetch_optional(pool)
        .await?;
        let Some(balance) = balance else {
            return Ok(None);
        };
        self.silver.store(balance.max(0) as u64, Ordering::Relaxed);
        Ok(Some(balance.max(0).min(i64::from(u32::MAX)) as u32))
    }

    /// Each status flag under way and when it ends (seconds since 1970):
    /// the timed statuses, double experience and Heaven's Blessing.
    pub fn status_timers(&self) -> Vec<(u64, u64)> {
        let now = std::time::Instant::now();
        // In milliseconds, rounded once, so the end stays the same second
        // from one tick to the next.
        let unix_now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_millis());
        let mut timers: Vec<(u64, u64)> = self
            .statuses
            .active()
            .into_iter()
            .map(|(effect, ends)| {
                let end_ms = unix_now_ms + ends.saturating_duration_since(now).as_millis();
                (effect.flag().bits(), u64::try_from((end_ms + 500) / 1_000).unwrap_or(u64::MAX))
            })
            .collect();
        for (flag, end) in [
            (crate::entities::Flags::DOUBLE_EXPERIENCE, self.double_experience_until_unix()),
            (crate::entities::Flags::HEAVEN_BLESSING, self.heaven_blessing_until_unix()),
        ] {
            if end > 0 {
                timers.push((flag.bits(), end as u64));
            }
        }
        timers.sort_unstable();
        timers
    }

    /// Whether `timers` differ from what the client last got; they are kept
    /// as sent when they do.
    pub fn timers_changed(&self, timers: &[(u64, u64)]) -> bool {
        let mut sent = self.timers_sent.lock();
        if sent.as_slice() == timers {
            return false;
        }
        *sent = timers.to_vec();
        true
    }

    /// The XP points when the client has not heard them yet.
    pub fn xp_to_send(&self) -> Option<u8> {
        let xp = self.xp_points();
        (self.xp_sent.swap(xp, Ordering::Relaxed) != xp).then_some(xp)
    }

    /// Lucky time left, in seconds.
    pub fn lucky_time(&self) -> u32 {
        self.lucky_time.load(Ordering::Relaxed)
    }

    /// Whether a move costing `cost` keeps to a client's pace (Cyclone
    /// moves twice as fast). A move out of pace is refused, and the first
    /// and every tenth is logged as a speed hack.
    pub fn keeps_pace(&self, cost: std::time::Duration, what: &str) -> bool {
        let haste = if self.has_status(StatusEffect::Cyclone) { 2 } else { 1 };
        if self.pace.admit(cost, haste, std::time::Instant::now()) {
            return true;
        }
        if self.pace.refuse() {
            tracing::warn!(
                target: "pk",
                character = %self.entity.name(),
                id = self.entity.id(),
                what,
                "Moving faster than the client can: a speed hack?"
            );
        }
        false
    }

    /// A KO count of the Cyclone or Superman under way: a new best is
    /// written, the database keeping the greatest ever (the KO record).
    pub async fn record_ko(&self, pool: &sqlx::PgPool, ko: u32) -> Result<(), Error> {
        if self.best_ko.fetch_max(ko, Ordering::Relaxed) >= ko {
            return Ok(());
        }
        sqlx::query("UPDATE characters SET best_ko = GREATEST(best_ko, $1) WHERE character_id = $2")
            .bind(i64::from(ko))
            .bind(self.inner.character_id)
            .execute(pool)
            .await?;
        Ok(())
    }

    /// Whether Cyclone or Superman is under way.
    pub fn in_xp_skill(&self) -> bool {
        self.has_status(StatusEffect::Cyclone) || self.has_status(StatusEffect::Superman)
    }

    /// A fresh Cyclone or Superman counts its KOs from nought.
    pub fn reset_ko_count(&self) {
        self.ko_count.store(0, Ordering::Relaxed);
    }

    /// A monster slain in Cyclone or Superman: one more KO and more of it,
    /// as `Conquer.exe` counts it (a second for Cyclone, 0.8 s for
    /// Superman); the count, or `None` outside them.
    pub fn count_ko(&self) -> Option<u32> {
        use std::time::Duration;
        let extended = [
            (StatusEffect::Cyclone, Duration::from_millis(1_000)),
            (StatusEffect::Superman, Duration::from_millis(800)),
        ]
        .into_iter()
        .any(|(effect, more)| self.statuses.extend(effect, more));
        extended.then(|| self.ko_count.fetch_add(1, Ordering::Relaxed) + 1)
    }

    pub fn add_lucky_time(&self, seconds: u32) {
        let _ = self.lucky_time.try_update(Ordering::Relaxed, Ordering::Relaxed, |time| {
            Some(time.saturating_add(seconds).min(crate::lucky::MAX_LUCKY_TIME))
        });
    }

    /// One second of lucky time wears off.
    pub fn wear_lucky_time(&self) {
        let _ = self
            .lucky_time
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |time| Some(time.saturating_sub(1)));
    }

    /// Starts praying here: casting Bless (`diffuse`) or absorbing beside
    /// someone who does.
    pub fn start_praying(&self, diffuse: bool) {
        let location = self.entity.location();
        *self.prayer_origin.lock() = Some((self.entity.map_id(), location.x, location.y));
        let flags = self.entity.flags() & !(crate::entities::Flags::CASTING_PRAY | crate::entities::Flags::PRAYING);
        let flag = if diffuse { crate::entities::Flags::CASTING_PRAY } else { crate::entities::Flags::PRAYING };
        self.entity.set_flags(flags | flag);
    }

    pub fn stop_praying(&self) {
        *self.prayer_origin.lock() = None;
        self.entity
            .set_flags(self.entity.flags() & !(crate::entities::Flags::CASTING_PRAY | crate::entities::Flags::PRAYING));
    }

    /// Whether the character left the spot where they started praying.
    pub fn prayer_moved(&self) -> bool {
        let location = self.entity.location();
        self.prayer_origin
            .lock()
            .is_some_and(|origin| origin != (self.entity.map_id(), location.x, location.y))
    }

    /// The name over the character's open booth.
    pub fn booth_name(&self) -> Option<String> {
        self.booth_name.lock().clone()
    }

    /// Opens (`Some`) or closes the booth's name.
    pub fn set_booth_name(&self, name: Option<String>) {
        *self.booth_name.lock() = name;
    }

    /// Moves `amount` silver between the character and their guild's fund in
    /// one transaction: from the character when `donate`, else from the fund.
    /// The new balances (character, fund), or `None` when the giver lacks it.
    pub async fn transfer_guild_fund(
        &self,
        pool: &sqlx::PgPool,
        guild_id: u32,
        amount: u32,
        donate: bool,
    ) -> Result<Option<(u64, u64)>, Error> {
        let amount = i64::from(amount);
        let (to_character, to_fund) = if donate { (-amount, amount) } else { (amount, -amount) };
        let mut transaction = pool.begin().await?;
        let balance: Option<i64> = sqlx::query_scalar(
            "UPDATE characters SET silver = silver + $1
             WHERE character_id = $2 AND silver + $1 >= 0 AND silver + $1 <= $3 RETURNING silver",
        )
        .bind(to_character)
        .bind(self.inner.character_id)
        .bind(i64::from(u32::MAX))
        .fetch_optional(&mut *transaction)
        .await?;
        let Some(balance) = balance else {
            return Ok(None);
        };
        let fund: Option<i64> = sqlx::query_scalar(
            "UPDATE guilds SET silver = silver + $1 WHERE guild_id = $2 AND silver + $1 >= 0 RETURNING silver",
        )
        .bind(to_fund)
        .bind(i64::from(guild_id))
        .fetch_optional(&mut *transaction)
        .await?;
        let Some(fund) = fund else {
            return Ok(None);
        };
        transaction.commit().await?;
        self.silver.store(balance.max(0) as u64, Ordering::Relaxed);
        Ok(Some((balance.max(0) as u64, fund.max(0) as u64)))
    }

    pub fn set_market_return(&self, map_id: u32, position: (u16, u16)) {
        *self.market_return.lock() = Some((map_id, position));
    }

    pub fn take_market_return(&self) -> Option<(u32, (u16, u16))> {
        self.market_return.lock().take()
    }

    pub fn set_active_shop_id(&self, shop_id: u32) {
        self.active_shop_id.store(shop_id, Ordering::Relaxed);
    }

    pub fn active_shop_id(&self) -> u32 {
        self.active_shop_id.load(Ordering::Relaxed)
    }

    pub fn set_active_storage_id(&self, npc_id: u32) {
        self.active_storage_id.store(npc_id, Ordering::Relaxed);
    }

    pub fn active_storage_id(&self) -> u32 {
        self.active_storage_id.load(Ordering::Relaxed)
    }

    pub fn kill_mode(&self) -> u16 {
        self.kill_mode.load(Ordering::Relaxed)
    }

    pub fn set_kill_mode(&self, mode: u16) {
        self.kill_mode.store(mode.min(3), Ordering::Relaxed);
    }

    /// Determines whether this character may target another player with the
    /// currently implemented PK rules. Team relations are not available yet,
    /// so Team behaves as unrestricted until party/guild membership exists.
    pub fn can_attack_character(&self, target: &Character) -> bool {
        if target.revive_protected() {
            return false;
        }
        match self.kill_mode() {
            0 | 2 => true,
            3 => target.entity().flags().contains(crate::entities::Flags::BLACK_NAME),
            _ => false,
        }
    }

    pub async fn load_skills(
        &self,
        pool: &sqlx::PgPool,
        catalog: &crate::skills::SkillCatalog,
    ) -> Result<(), Error> {
        let rows = sqlx::query_as::<_, (i64, i64, i64, i64)>(
            "SELECT skill_id, level, experience, proficiency
             FROM character_skills WHERE character_id = $1 ORDER BY skill_id",
        )
        .bind(self.inner.character_id)
        .fetch_all(pool)
        .await?;
        {
            let mut skills = self.skills.lock();
            skills.clear();
            for (skill_id, level, experience, proficiency) in rows {
                let (Ok(skill_id), Ok(level), Ok(experience), Ok(proficiency)) = (
                    u32::try_from(skill_id),
                    u16::try_from(level),
                    u64::try_from(experience),
                    u32::try_from(proficiency),
                ) else {
                    continue;
                };
                // Every learned skill stays known; casting still checks the
                // catalog, but trainers teach skills the catalog does not
                // describe yet and they must survive a relog.
                skills.insert(
                    skill_id,
                    SkillProgress {
                        skill_id,
                        level,
                        experience,
                        proficiency,
                    },
                );
            }
        }

        // Only Taoists start with magic (Thunder), as the original.
        let taoist = (100..=145).contains(&self.current_class());
        for skill_id in catalog.starter_ids().filter(|_| taoist) {
            if self.has_skill(skill_id) {
                continue;
            }
            sqlx::query(
                "INSERT INTO character_skills
                 (character_id, skill_id, level, experience, proficiency)
                 VALUES ($1, $2, 1, 0, 0) ON CONFLICT DO NOTHING",
            )
            .bind(self.inner.character_id)
            .bind(i64::from(skill_id))
            .execute(pool)
            .await?;
            self.skills.lock().insert(
                skill_id,
                SkillProgress {
                    skill_id,
                    level: 1,
                    experience: 0,
                    proficiency: 0,
                },
            );
        }
        Ok(())
    }

    pub fn has_skill(&self, skill_id: u32) -> bool {
        self.skills.lock().contains_key(&skill_id)
    }

    /// Learns `skill_id` unless it is already known, like the trainers'
    /// `LearnNewSkill`: a skill that was trained further keeps its level.
    pub async fn learn_new_skill(&self, pool: &sqlx::PgPool, skill_id: u32) -> Result<Option<SkillProgress>, Error> {
        if self.has_skill(skill_id) {
            return Ok(None);
        }
        self.learn_skill(pool, skill_id, 0).await?;
        Ok(self.skill_progress(skill_id))
    }

    /// Creates one item in the inventory with its catalog durability and,
    /// when asked, an open first socket. `None` means the inventory is full.
    pub async fn grant_item(
        &self,
        pool: &sqlx::PgPool,
        catalog: &ShopCatalog,
        item_type: u32,
        open_socket: bool,
    ) -> Result<Option<InventoryItem>, Error> {
        let (durability, max_durability) = catalog
            .template(item_type)
            .map_or((100, 100), |template| (template.durability, template.max_durability));
        let Some(mut item) = self.add_ground_item(pool, item_type, durability, max_durability).await? else {
            return Ok(None);
        };
        if open_socket {
            sqlx::query(
                "INSERT INTO item_upgrades (item_uid, plus, gem_one, gem_two, socket_one, socket_two, rarity)
                 VALUES ($1, 0, 0, 0, 1, 0, 0)
                 ON CONFLICT(item_uid) DO UPDATE SET socket_one = 1",
            )
            .bind(i64::from(item.uid))
            .execute(pool)
            .await?;
            item.socket_one = 1;
            if let Some(existing) = self.inventory.lock().iter_mut().find(|owned| owned.uid == item.uid) {
                *existing = item;
            }
        }
        Ok(Some(item))
    }

    /// Consumes one inventory item of `item_type`, returning it.
    pub async fn take_item(&self, pool: &sqlx::PgPool, item_type: u32) -> Result<Option<InventoryItem>, Error> {
        let Some(item) = self
            .inventory_items()
            .into_iter()
            .find(|owned| owned.item_type == item_type)
        else {
            return Ok(None);
        };
        let deleted = sqlx::query("DELETE FROM inventory_items WHERE item_uid = $1 AND character_id = $2 AND item_type = $3")
            .bind(i64::from(item.uid))
            .bind(self.inner.character_id)
            .bind(i64::from(item_type))
            .execute(pool)
            .await?;
        if deleted.rows_affected() != 1 {
            return Ok(None);
        }
        self.inventory.lock().retain(|owned| owned.uid != item.uid);
        Ok(Some(item))
    }

    pub fn skills(&self) -> Vec<SkillProgress> {
        self.skills.lock().values().copied().collect()
    }

    pub fn skill_progress(&self, skill_id: u32) -> Option<SkillProgress> {
        self.skills.lock().get(&skill_id).copied()
    }

    pub async fn load_weapon_proficiencies(&self, pool: &sqlx::PgPool) -> Result<(), Error> {
        let rows = sqlx::query_as::<_, (i64, i64, i64, i64)>(
            "SELECT weapon_family, level, experience, proficiency
             FROM character_weapon_proficiencies WHERE character_id = $1 ORDER BY weapon_family",
        )
        .bind(self.inner.character_id)
        .fetch_all(pool)
        .await?;
        let mut proficiencies = self.weapon_proficiencies.lock();
        proficiencies.clear();
        for (weapon_family, level, experience, proficiency) in rows {
            let (Ok(weapon_family), Ok(level), Ok(experience), Ok(proficiency)) = (
                u32::try_from(weapon_family),
                u16::try_from(level),
                u64::try_from(experience),
                u32::try_from(proficiency),
            ) else {
                continue;
            };
            proficiencies.insert(
                weapon_family,
                WeaponProficiency {
                    weapon_family,
                    level: level.max(1),
                    experience,
                    proficiency,
                },
            );
        }
        Ok(())
    }

    pub fn weapon_proficiencies(&self) -> Vec<WeaponProficiency> {
        self.weapon_proficiencies.lock().values().copied().collect()
    }

    pub fn weapon_proficiency(&self, weapon_family: u32) -> Option<WeaponProficiency> {
        self.weapon_proficiencies.lock().get(&weapon_family).copied()
    }

    pub fn weapon_proficiency_bonus_percent(&self) -> u16 {
        let Some(weapon) = self.equipment_item(4).or_else(|| self.equipment_item(5)) else {
            return 0;
        };
        if weapon.durability == 0 {
            return 0;
        }
        let Some(family) = crate::shop::ShopCatalog::weapon_family(weapon.item_type) else {
            return 0;
        };
        let Some(progress) = self.weapon_proficiency(family) else {
            return 0;
        };
        u16::try_from(
            u32::from(progress.level.saturating_sub(1)).saturating_mul(2) + (progress.proficiency.min(100) / 25),
        )
        .unwrap_or(u16::MAX)
        .min(20)
    }

    pub fn meets_item_requirements(&self, item_type: u32, template: &crate::shop::ItemTemplate) -> bool {
        if u32::from(self.entity.level()) < template.level
            || self.strength() < template.required_strength
            || self.agility() < template.required_agility
            || self.vitality() < template.required_vitality
            || self.spirit() < template.required_spirit
        {
            return false;
        }

        if template.required_sex != 0 {
            let character_sex = if self.entity.mesh() >= 2_000 { 2 } else { 1 };
            if template.required_sex != character_sex {
                return false;
            }
        }

        if template.required_profession != 0 {
            let current_class = u16::from(self.current_class());
            let required_class = u16::from(template.required_profession);
            let profession_matches = if required_class >= 100 {
                current_class >= 100
            } else {
                current_class >= required_class && current_class.saturating_sub(required_class) <= 5
            };
            if !profession_matches {
                return false;
            }
        }

        if template.required_weapon_skill > 0 {
            let Some(family) = crate::shop::ShopCatalog::weapon_family(item_type) else {
                return false;
            };
            let Some(progress) = self.weapon_proficiency(family) else {
                return false;
            };
            if progress.level < u16::from(template.required_weapon_skill) {
                return false;
            }
        }
        true
    }

    pub fn apply_status(&self, effect: StatusEffect, duration: Duration) {
        self.statuses.apply(effect, duration);
        if duration.is_zero() {
            self.entity.set_flags(self.entity.flags() & !effect.flag());
        } else {
            self.entity.set_flags(self.entity.flags() | effect.flag());
        }
    }

    pub fn has_status(&self, effect: StatusEffect) -> bool {
        self.statuses.contains(effect)
    }

    pub fn tick_statuses(&self, now: std::time::Instant) -> u32 {
        let mut damage: u32 = 0;
        for effect in self.statuses.tick(now) {
            if matches!(effect, StatusEffect::Burning) {
                let max_health = u32::from(self.entity.hp().max).max(1);
                let amount = (max_health / 20).max(1);
                self.entity.apply_damage(amount);
                damage = damage.saturating_add(amount);
            }
        }
        damage
    }

    pub fn expire_statuses(&self, now: std::time::Instant) -> bool {
        let expired = self.statuses.expire(now);
        for effect in &expired {
            if !self.statuses.contains(*effect) {
                self.entity.set_flags(self.entity.flags() & !effect.flag());
            }
        }
        !expired.is_empty()
    }

    pub async fn practice_weapon_proficiency(
        &self,
        pool: &sqlx::PgPool,
        item_type: u32,
        amount: u32,
    ) -> Result<Option<WeaponProficiency>, Error> {
        let Some(weapon_family) = crate::shop::ShopCatalog::weapon_family(item_type) else {
            return Ok(None);
        };
        // Violet gems speed weapon proficiency up.
        let amount = amount.saturating_mul(100 + self.gem_bonus(GemFamily::Violet)) / 100;
        let current = self.weapon_proficiency(weapon_family).unwrap_or(WeaponProficiency {
            weapon_family,
            level: 1,
            experience: 0,
            proficiency: 0,
        });
        let mut next = current;
        next.experience = next.experience.saturating_add(u64::from(amount));
        next.proficiency = next.proficiency.saturating_add(amount);
        while next.level < 20 {
            let threshold = weapon_proficiency_threshold(next.level);
            if threshold == 0 || next.proficiency < threshold {
                break;
            }
            next.proficiency -= threshold;
            next.level = next.level.saturating_add(1);
        }
        sqlx::query(
            "INSERT INTO character_weapon_proficiencies
             (character_id, weapon_family, level, experience, proficiency)
             VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT(character_id, weapon_family) DO UPDATE SET
               level = excluded.level, experience = excluded.experience,
               proficiency = excluded.proficiency",
        )
        .bind(self.inner.character_id)
        .bind(i64::from(weapon_family))
        .bind(i64::from(next.level))
        .bind(i64::try_from(next.experience).unwrap_or(i64::MAX))
        .bind(i64::from(next.proficiency))
        .execute(pool)
        .await?;
        self.weapon_proficiencies.lock().insert(weapon_family, next);
        Ok(Some(next))
    }

    pub async fn learn_skill(&self, pool: &sqlx::PgPool, skill_id: u32, level: u16) -> Result<(), Error> {
        let level = level.max(1);
        sqlx::query(
            "INSERT INTO character_skills (character_id, skill_id, level, experience, proficiency)
             VALUES ($1, $2, $3, 0, 0)
             ON CONFLICT(character_id, skill_id) DO UPDATE SET level = excluded.level",
        )
        .bind(self.inner.character_id)
        .bind(i64::from(skill_id))
        .bind(i64::from(level))
        .execute(pool)
        .await?;
        self.skills.lock().insert(
            skill_id,
            SkillProgress {
                skill_id,
                level,
                experience: 0,
                proficiency: 0,
            },
        );
        Ok(())
    }

    /// Learns a skill and charges silver in one database transaction. `None`
    /// means the skill was already learned or the character could not pay.
    pub async fn learn_skill_with_cost(
        &self,
        pool: &sqlx::PgPool,
        skill_id: u32,
        level: u16,
        cost: u32,
    ) -> Result<Option<SkillProgress>, Error> {
        if self.has_skill(skill_id) {
            return Ok(None);
        }
        let mut transaction = pool.begin().await?;
        let inserted = sqlx::query(
            "INSERT INTO character_skills
             (character_id, skill_id, level, experience, proficiency)
             VALUES ($1, $2, $3, 0, 0) ON CONFLICT DO NOTHING",
        )
        .bind(self.inner.character_id)
        .bind(i64::from(skill_id))
        .bind(i64::from(level.max(1)))
        .execute(&mut *transaction)
        .await?;
        if inserted.rows_affected() != 1 {
            transaction.rollback().await?;
            return Ok(None);
        }
        let balance = sqlx::query_scalar::<_, i64>(
            "UPDATE characters SET silver = silver - $1
             WHERE character_id = $2 AND silver >= $3
             RETURNING silver",
        )
        .bind(i64::from(cost))
        .bind(self.inner.character_id)
        .bind(i64::from(cost))
        .fetch_optional(&mut *transaction)
        .await?;
        let Some(balance) = balance else {
            transaction.rollback().await?;
            return Ok(None);
        };
        transaction.commit().await?;

        let progress = SkillProgress {
            skill_id,
            level: level.max(1),
            experience: 0,
            proficiency: 0,
        };
        self.silver.store(balance.max(0) as u64, Ordering::Relaxed);
        self.skills.lock().insert(skill_id, progress);
        Ok(Some(progress))
    }

    /// Consumes a skill book and learns its skill in one database transaction.
    /// The item is removed from the in-memory inventory only after both the
    /// inventory delete and skill insert have committed successfully.
    pub async fn learn_skill_from_item(
        &self,
        pool: &sqlx::PgPool,
        item_uid: u32,
        skill_id: u32,
    ) -> Result<Option<(InventoryItem, SkillProgress)>, Error> {
        if self.has_skill(skill_id) {
            return Ok(None);
        }
        let mut transaction = pool.begin().await?;
        let source = sqlx::query_as::<_, (i64, i64, i64, i64, i64)>(
            "SELECT item_uid, item_type, position, durability, max_durability
             FROM inventory_items WHERE item_uid = $1 AND character_id = $2 AND position < 40",
        )
        .bind(i64::from(item_uid))
        .bind(self.inner.character_id)
        .fetch_optional(&mut *transaction)
        .await?;
        let Some((uid, item_type, position, durability, max_durability)) = source else {
            return Ok(None);
        };
        let deleted =
            sqlx::query("DELETE FROM inventory_items WHERE item_uid = $1 AND character_id = $2 AND position < 40")
                .bind(i64::from(item_uid))
                .bind(self.inner.character_id)
                .execute(&mut *transaction)
                .await?;
        if deleted.rows_affected() != 1 {
            return Ok(None);
        }
        sqlx::query(
            "INSERT INTO character_skills (character_id, skill_id, level, experience, proficiency)
             VALUES ($1, $2, 1, 0, 0)",
        )
        .bind(self.inner.character_id)
        .bind(i64::from(skill_id))
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;

        let item = InventoryItem {
            uid: u32::try_from(uid).map_err(|_| Error::Other("item ID exceeds protocol range".into()))?,
            item_type: u32::try_from(item_type).map_err(|_| Error::Other("item type exceeds protocol range".into()))?,
            position: u8::try_from(position)
                .map_err(|_| Error::Other("item position exceeds protocol range".into()))?,
            durability: u16::try_from(durability)
                .map_err(|_| Error::Other("item durability exceeds protocol range".into()))?,
            max_durability: u16::try_from(max_durability)
                .map_err(|_| Error::Other("item durability exceeds protocol range".into()))?,
            plus: 0,
            ..Default::default()
        };
        let progress = SkillProgress {
            skill_id,
            level: 1,
            experience: 0,
            proficiency: 0,
        };
        self.inventory.lock().retain(|owned| owned.uid != item.uid);
        self.skills.lock().insert(skill_id, progress);
        Ok(Some((item, progress)))
    }

    /// Registers practice for a learned skill and persists its progression.
    ///
    /// Proficiency is the progress inside the current level, while experience
    /// is the lifetime skill experience shown by the 1103 update packet.
    pub async fn practice_skill(
        &self,
        pool: &sqlx::PgPool,
        skill_id: u32,
        amount: u32,
    ) -> Result<Option<SkillProgress>, Error> {
        if amount == 0 {
            return Ok(self.skills().into_iter().find(|skill| skill.skill_id == skill_id));
        }
        // Moon gems speed skill experience up.
        let amount = amount.saturating_mul(100 + self.gem_bonus(GemFamily::Moon)) / 100;
        let Some(current) = self.skills().into_iter().find(|skill| skill.skill_id == skill_id) else {
            return Ok(None);
        };
        let mut next = current;
        next.experience = next.experience.saturating_add(u64::from(amount));
        next.proficiency = next.proficiency.saturating_add(amount);
        // The original's experience per level (MagicType `NeedExp`); skills
        // it lacks keep the old hundred per level.
        let threshold = crate::skills::experience_to_level(skill_id, next.level)
            .unwrap_or_else(|| u32::from(next.level).saturating_mul(100).max(100));
        if threshold == 0 {
            // The top level: experience no longer fills.
            next.proficiency = 0;
        } else if next.proficiency >= threshold
            && !crate::magictype::may_raise(skill_id, next.level, self.entity.level())
        {
            // Full, waiting for the character level the next one needs
            // (MagicType `NeedLevel`).
            next.proficiency = threshold;
        } else if next.proficiency >= threshold && next.level < u16::MAX {
            next.proficiency -= threshold;
            next.level = next.level.saturating_add(1);
        }
        sqlx::query(
            "UPDATE character_skills
             SET level = $1, experience = $2, proficiency = $3
             WHERE character_id = $4 AND skill_id = $5",
        )
        .bind(i64::from(next.level))
        .bind(i64::try_from(next.experience).unwrap_or(i64::MAX))
        .bind(i64::from(next.proficiency))
        .bind(self.inner.character_id)
        .bind(i64::from(skill_id))
        .execute(pool)
        .await?;
        self.skills.lock().insert(skill_id, next);
        Ok(Some(next))
    }

    pub async fn load_inventory(&self, pool: &sqlx::PgPool) -> Result<(), Error> {
        let rows = sqlx::query_as::<_, (i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64)>(
            "SELECT i.item_uid, i.item_type, i.position, i.durability, i.max_durability,
                    COALESCE(u.plus, 0), COALESCE(u.rarity, 0), COALESCE(u.gem_one, 0), COALESCE(u.gem_two, 0),
                    COALESCE(u.socket_one, 0), COALESCE(u.socket_two, 0), COALESCE(u.enchant, 0)
             FROM inventory_items i
             LEFT JOIN item_upgrades u ON u.item_uid = i.item_uid
             WHERE i.character_id = $1 ORDER BY i.position",
        )
        .bind(self.inner.character_id)
        .fetch_all(pool)
        .await?;
        {
            let mut inventory = self.inventory.lock();
            *inventory = rows
                .into_iter()
                .filter_map(
                    |(
                        uid,
                        item_type,
                        position,
                        durability,
                        max_durability,
                        plus,
                        rarity,
                        gem_one,
                        gem_two,
                        socket_one,
                        socket_two,
                        rarity_roll,
                    )| {
                        Some(InventoryItem {
                            uid: u32::try_from(uid).ok()?,
                            item_type: u32::try_from(item_type).ok()?,
                            position: u8::try_from(position).ok()?,
                            durability: u16::try_from(durability).ok()?,
                            max_durability: u16::try_from(max_durability).ok()?,
                            plus: u8::try_from(plus).ok()?,
                            rarity: u8::try_from(rarity).ok()?,
                            gem_one: u8::try_from(gem_one).ok()?,
                            gem_two: u8::try_from(gem_two).ok()?,
                            socket_one: u8::try_from(socket_one).ok()?,
                            socket_two: u8::try_from(socket_two).ok()?,
                            rarity_roll: u8::try_from(rarity_roll).ok()?,
                            bless: 0,
                            enchant_hp: 0,
                            color: 0,
                        })
                    },
                )
                .collect();
        }
        self.apply_blessings(pool).await?;
        Ok(())
    }

    /// Copies the stored blessings onto the items in memory.
    async fn apply_blessings(&self, pool: &sqlx::PgPool) -> Result<(), Error> {
        let uids = self
            .inventory
            .lock()
            .iter()
            .chain(self.equipment.lock().iter())
            .chain(self.warehouse.lock().iter())
            .map(|item| item.uid)
            .collect::<Vec<_>>();
        if uids.is_empty() {
            return Ok(());
        }
        let rows = sqlx::query_as::<_, (i64, i64)>("SELECT item_uid, bless FROM item_blessings")
            .fetch_all(pool)
            .await?;
        let blessings = rows
            .into_iter()
            .filter_map(|(uid, bless)| Some((u32::try_from(uid).ok()?, u8::try_from(bless).ok()?)))
            .filter(|(uid, _)| uids.contains(uid))
            .collect::<HashMap<_, _>>();
        let rows = sqlx::query_as::<_, (i64, i64)>("SELECT item_uid, hp FROM item_enchants")
            .fetch_all(pool)
            .await?;
        let enchants = rows
            .into_iter()
            .filter_map(|(uid, hp)| Some((u32::try_from(uid).ok()?, u8::try_from(hp).ok()?)))
            .filter(|(uid, _)| uids.contains(uid))
            .collect::<HashMap<_, _>>();
        let rows = sqlx::query_as::<_, (i64, i64)>("SELECT item_uid, color FROM item_colors")
            .fetch_all(pool)
            .await?;
        let colors = rows
            .into_iter()
            .filter_map(|(uid, color)| Some((u32::try_from(uid).ok()?, u8::try_from(color).ok()?)))
            .filter(|(uid, _)| uids.contains(uid))
            .collect::<HashMap<_, _>>();
        for items in [&self.inventory, &self.equipment, &self.warehouse] {
            for item in items.lock().iter_mut() {
                item.bless = blessings.get(&item.uid).copied().unwrap_or(0);
                item.enchant_hp = enchants.get(&item.uid).copied().unwrap_or(0);
                item.color = colors.get(&item.uid).copied().unwrap_or(0);
            }
        }
        Ok(())
    }

    /// Sets the blessing of an owned item.
    pub async fn bless_item(&self, pool: &sqlx::PgPool, item_uid: u32, bless: u8) -> Result<Option<InventoryItem>, Error> {
        sqlx::query(
            "INSERT INTO item_blessings (item_uid, bless) VALUES ($1, $2)
             ON CONFLICT(item_uid) DO UPDATE SET bless = excluded.bless",
        )
        .bind(i64::from(item_uid))
        .bind(i64::from(bless))
        .execute(pool)
        .await?;
        let mut updated = None;
        for items in [&self.inventory, &self.equipment, &self.warehouse] {
            if let Some(item) = items.lock().iter_mut().find(|item| item.uid == item_uid) {
                item.bless = bless;
                updated = Some(*item);
            }
        }
        Ok(updated)
    }

    /// Dyes an owned item.
    pub async fn dye_item(&self, pool: &sqlx::PgPool, item_uid: u32, color: u8) -> Result<Option<InventoryItem>, Error> {
        sqlx::query(
            "INSERT INTO item_colors (item_uid, color) VALUES ($1, $2)
             ON CONFLICT(item_uid) DO UPDATE SET color = excluded.color",
        )
        .bind(i64::from(item_uid))
        .bind(i64::from(color))
        .execute(pool)
        .await?;
        let mut updated = None;
        for items in [&self.inventory, &self.equipment, &self.warehouse] {
            if let Some(item) = items.lock().iter_mut().find(|item| item.uid == item_uid) {
                item.color = color;
                updated = Some(*item);
            }
        }
        Ok(updated)
    }

    /// Helmet, armor and shield dyes packed for the spawn packet.
    pub fn appearance_colors(&self) -> i32 {
        let color = |slot: u8| self.equipment_item(slot).map_or(0, |item| i32::from(item.color));
        color(3) | (color(1) << 8) | (color(5) << 16)
    }

    /// Sets the HP enchantment of an owned item.
    pub async fn enchant_item(&self, pool: &sqlx::PgPool, item_uid: u32, hp: u8) -> Result<Option<InventoryItem>, Error> {
        sqlx::query(
            "INSERT INTO item_enchants (item_uid, hp) VALUES ($1, $2)
             ON CONFLICT(item_uid) DO UPDATE SET hp = excluded.hp",
        )
        .bind(i64::from(item_uid))
        .bind(i64::from(hp))
        .execute(pool)
        .await?;
        let mut updated = None;
        for items in [&self.inventory, &self.equipment, &self.warehouse] {
            if let Some(item) = items.lock().iter_mut().find(|item| item.uid == item_uid) {
                item.enchant_hp = hp;
                updated = Some(*item);
            }
        }
        Ok(updated)
    }

    /// The summed percent of the worn gems of `family`.
    pub fn gem_bonus(&self, family: GemFamily) -> u32 {
        self.equipment_items()
            .iter()
            .filter(|item| item.durability > 0)
            .map(|item| gem_percent(family, item.gem_one) + gem_percent(family, item.gem_two))
            .sum()
    }

    /// Damage taken is lowered by the blessings of the worn items.
    pub fn bless_reduction_percent(&self) -> u16 {
        self.equipment_items()
            .iter()
            .filter(|item| item.durability > 0)
            .map(|item| u16::from(item.bless))
            .sum()
    }

    pub fn inventory_items(&self) -> Vec<InventoryItem> {
        self.inventory.lock().clone()
    }

    pub async fn load_equipment(&self, pool: &sqlx::PgPool) -> Result<(), Error> {
        let rows = sqlx::query_as::<_, (i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64)>(
            "SELECT e.item_uid, e.item_type, e.position, e.durability, e.max_durability,
                    COALESCE(u.plus, 0), COALESCE(u.rarity, 0), COALESCE(u.gem_one, 0), COALESCE(u.gem_two, 0),
                    COALESCE(u.socket_one, 0), COALESCE(u.socket_two, 0), COALESCE(u.enchant, 0)
             FROM equipment_items e
             LEFT JOIN item_upgrades u ON u.item_uid = e.item_uid
             WHERE e.character_id = $1 ORDER BY e.position",
        )
        .bind(self.inner.character_id)
        .fetch_all(pool)
        .await?;
        {
            let mut equipment = self.equipment.lock();
            *equipment = rows
                .into_iter()
                .filter_map(
                    |(
                        uid,
                        item_type,
                        position,
                        durability,
                        max_durability,
                        plus,
                        rarity,
                        gem_one,
                        gem_two,
                        socket_one,
                        socket_two,
                        rarity_roll,
                    )| {
                        Some(InventoryItem {
                            uid: u32::try_from(uid).ok()?,
                            item_type: u32::try_from(item_type).ok()?,
                            position: u8::try_from(position).ok()?,
                            durability: u16::try_from(durability).ok()?,
                            max_durability: u16::try_from(max_durability).ok()?,
                            plus: u8::try_from(plus).ok()?,
                            rarity: u8::try_from(rarity).ok()?,
                            gem_one: u8::try_from(gem_one).ok()?,
                            gem_two: u8::try_from(gem_two).ok()?,
                            socket_one: u8::try_from(socket_one).ok()?,
                            socket_two: u8::try_from(socket_two).ok()?,
                            rarity_roll: u8::try_from(rarity_roll).ok()?,
                            bless: 0,
                            enchant_hp: 0,
                            color: 0,
                        })
                    },
                )
                .collect();
        }
        self.apply_blessings(pool).await?;
        Ok(())
    }

    pub fn equipment_items(&self) -> Vec<InventoryItem> {
        self.equipment.lock().clone()
    }

    pub async fn load_refineries(&self, pool: &sqlx::PgPool) -> Result<(), Error> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|value| value.as_secs())
            .unwrap_or_default();
        sqlx::query(
            "DELETE FROM item_refineries
             WHERE item_uid IN (
                 SELECT item_uid FROM equipment_items WHERE character_id = $1
             ) AND expires_at > 0 AND expires_at <= $2",
        )
        .bind(self.inner.character_id)
        .bind(i64::try_from(now).unwrap_or(i64::MAX))
        .execute(pool)
        .await?;
        let rows = sqlx::query_as::<_, (i64, i64, i64, i64, i64)>(
            "SELECT r.item_uid, r.refinery_type, r.refinery_level, r.effect, r.expires_at
             FROM item_refineries r
             INNER JOIN equipment_items e ON e.item_uid = r.item_uid
             WHERE e.character_id = $1 AND (r.expires_at = 0 OR r.expires_at > $2)",
        )
        .bind(self.inner.character_id)
        .bind(i64::try_from(now).unwrap_or(i64::MAX))
        .fetch_all(pool)
        .await?;
        let mut refineries = self.refineries.lock();
        refineries.clear();
        for (item_uid, refinery_type, level, effect, expires_at) in rows {
            let (Ok(item_uid), Ok(refinery_type), Ok(level), Ok(effect), Ok(expires_at)) = (
                u32::try_from(item_uid),
                u32::try_from(refinery_type),
                u32::try_from(level),
                i32::try_from(effect),
                u64::try_from(expires_at),
            ) else {
                continue;
            };
            refineries.insert(
                item_uid,
                crate::refinery::RefineryEffect {
                    refinery_type,
                    level,
                    effect,
                    expires_at,
                },
            );
        }
        Ok(())
    }

    pub fn refinery_effect(&self, item_uid: u32) -> Option<crate::refinery::RefineryEffect> {
        self.refineries.lock().get(&item_uid).copied()
    }

    pub async fn load_artifacts(&self, pool: &sqlx::PgPool) -> Result<(), Error> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|value| value.as_secs())
            .unwrap_or_default();
        sqlx::query(
            "DELETE FROM item_artifacts
             WHERE item_uid IN (
                 SELECT item_uid FROM inventory_items WHERE character_id = $1
                 UNION ALL
                 SELECT item_uid FROM equipment_items WHERE character_id = $2
             ) AND expires_at > 0 AND expires_at <= $3",
        )
        .bind(self.inner.character_id)
        .bind(self.inner.character_id)
        .bind(i64::try_from(now).unwrap_or(i64::MAX))
        .execute(pool)
        .await?;
        let rows = sqlx::query_as::<_, (i64, i64, i64, i64, i64)>(
            "SELECT a.item_uid, a.artifact_type, a.artifact_level, a.effect, a.expires_at
             FROM item_artifacts a
             WHERE (a.expires_at = 0 OR a.expires_at > $1)
             AND a.item_uid IN (
                 SELECT item_uid FROM inventory_items WHERE character_id = $2
                 UNION ALL
                 SELECT item_uid FROM equipment_items WHERE character_id = $3
             )",
        )
        .bind(i64::try_from(now).unwrap_or(i64::MAX))
        .bind(self.inner.character_id)
        .bind(self.inner.character_id)
        .fetch_all(pool)
        .await?;
        let mut artifacts = self.artifacts.lock();
        artifacts.clear();
        for (item_uid, artifact_type, level, effect, expires_at) in rows {
            let (Ok(item_uid), Ok(artifact_type), Ok(level), Ok(effect), Ok(expires_at)) = (
                u32::try_from(item_uid),
                u32::try_from(artifact_type),
                u32::try_from(level),
                i32::try_from(effect),
                u64::try_from(expires_at),
            ) else {
                continue;
            };
            artifacts.insert(
                item_uid,
                crate::refinery::ArtifactEffect {
                    artifact_type,
                    level,
                    effect,
                    expires_at,
                },
            );
        }
        Ok(())
    }

    pub fn artifact_effect(&self, item_uid: u32) -> Option<crate::refinery::ArtifactEffect> {
        self.artifacts.lock().get(&item_uid).copied()
    }

    /// Returns the fire resistance supplied by active artifacts on equipped
    /// items. Artifact state is keyed by the target item UID, so inventory
    /// materials and unequipped targets never contribute to combat defense.
    pub fn fire_resistance_percent(&self) -> u16 {
        self.equipment_items()
            .into_iter()
            .filter(|item| item.durability > 0)
            .filter_map(|item| self.artifact_effect(item.uid))
            .fold(0_u16, |total, effect| {
                total.saturating_add(effect.effect.max(0) as u16).min(100)
            })
    }

    pub async fn apply_artifact(
        &self,
        pool: &sqlx::PgPool,
        item_uid: u32,
        artifact_uid: u32,
    ) -> Result<ArtifactOutcome, Error> {
        let Some(target) = self.inventory_items().into_iter().find(|item| item.uid == item_uid) else {
            return Ok(ArtifactOutcome::InvalidTarget);
        };
        let Some(material) = self.inventory_items().into_iter().find(|item| item.uid == artifact_uid) else {
            return Ok(ArtifactOutcome::NotFound);
        };
        let Some(definition) = crate::refinery::artifact_definition(material.item_type) else {
            return Ok(ArtifactOutcome::InvalidMaterial);
        };
        if crate::shop::ShopCatalog::equipment_slot_for_item(target.item_type) != Some(definition.equipment_slot) {
            return Ok(ArtifactOutcome::Incompatible);
        }
        let Some(catalyst) = self
            .inventory_items()
            .into_iter()
            .find(|item| item.item_type == 1_088_001)
        else {
            return Ok(ArtifactOutcome::MissingMeteor);
        };
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|value| value.as_secs())
            .unwrap_or_default();
        let expires_at = now.saturating_add(crate::refinery::REFINERY_DURATION_SECS);
        let mut transaction = pool.begin().await?;
        for uid in [artifact_uid, catalyst.uid] {
            let deleted = sqlx::query("DELETE FROM inventory_items WHERE item_uid = $1 AND character_id = $2")
                .bind(i64::from(uid))
                .bind(self.inner.character_id)
                .execute(&mut *transaction)
                .await?
                .rows_affected();
            if deleted != 1 {
                return Ok(ArtifactOutcome::NotFound);
            }
        }
        let target_exists: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM inventory_items WHERE item_uid = $1 AND character_id = $2)")
                .bind(i64::from(item_uid))
                .bind(self.inner.character_id)
                .fetch_one(&mut *transaction)
                .await?;
        if !target_exists {
            return Ok(ArtifactOutcome::InvalidTarget);
        }
        sqlx::query(
            "INSERT INTO item_artifacts (item_uid, artifact_type, artifact_level, effect, expires_at)
             VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT(item_uid) DO UPDATE SET
                 artifact_type = excluded.artifact_type,
                 artifact_level = excluded.artifact_level,
                 effect = excluded.effect,
                 expires_at = excluded.expires_at",
        )
        .bind(i64::from(item_uid))
        .bind(i64::from(definition.item_type))
        .bind(i64::from(definition.level))
        .bind(i64::from(definition.effect))
        .bind(i64::try_from(expires_at).unwrap_or(i64::MAX))
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;
        self.inventory
            .lock()
            .retain(|item| item.uid != artifact_uid && item.uid != catalyst.uid);
        let effect = crate::refinery::ArtifactEffect {
            artifact_type: definition.item_type,
            level: definition.level,
            effect: definition.effect,
            expires_at,
        };
        self.artifacts.lock().insert(item_uid, effect);
        Ok(ArtifactOutcome::Applied {
            target,
            material,
            catalyst,
            effect,
        })
    }

    pub async fn apply_refinery(
        &self,
        pool: &sqlx::PgPool,
        item_uid: u32,
        material_uid: u32,
    ) -> Result<RefineryOutcome, Error> {
        let Some(target) = self.equipment_items().into_iter().find(|item| item.uid == item_uid) else {
            return Ok(RefineryOutcome::InvalidTarget);
        };
        let Some(material) = self.inventory_items().into_iter().find(|item| item.uid == material_uid) else {
            return Ok(RefineryOutcome::NotFound);
        };
        let Some(definition) = crate::refinery::definition(material.item_type) else {
            return Ok(RefineryOutcome::InvalidMaterial);
        };
        if definition.equipment_slot != target.position {
            return Ok(RefineryOutcome::Incompatible);
        }

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|value| value.as_secs())
            .unwrap_or_default();
        let expires_at = now.saturating_add(crate::refinery::REFINERY_DURATION_SECS);
        let mut transaction = pool.begin().await?;
        let material_deleted = sqlx::query("DELETE FROM inventory_items WHERE item_uid = $1 AND character_id = $2")
            .bind(i64::from(material_uid))
            .bind(self.inner.character_id)
            .execute(&mut *transaction)
            .await?
            .rows_affected();
        if material_deleted != 1 {
            return Ok(RefineryOutcome::NotFound);
        }
        let target_exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(
                SELECT 1 FROM equipment_items WHERE item_uid = $1 AND character_id = $2 AND position = $3
            )",
        )
        .bind(i64::from(item_uid))
        .bind(self.inner.character_id)
        .bind(i64::from(target.position))
        .fetch_one(&mut *transaction)
        .await?;
        if !target_exists {
            return Ok(RefineryOutcome::InvalidTarget);
        }
        sqlx::query(
            "INSERT INTO item_refineries (item_uid, refinery_type, refinery_level, effect, expires_at)
             VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT(item_uid) DO UPDATE SET
                 refinery_type = excluded.refinery_type,
                 refinery_level = excluded.refinery_level,
                 effect = excluded.effect,
                 expires_at = excluded.expires_at",
        )
        .bind(i64::from(item_uid))
        .bind(i64::from(definition.refinery_type))
        .bind(i64::from(definition.level))
        .bind(i64::from(definition.effect))
        .bind(i64::try_from(expires_at).unwrap_or(i64::MAX))
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;

        self.inventory.lock().retain(|item| item.uid != material_uid);
        let effect = crate::refinery::RefineryEffect {
            refinery_type: definition.refinery_type,
            level: definition.level,
            effect: definition.effect,
            expires_at,
        };
        self.refineries.lock().insert(item_uid, effect);
        Ok(RefineryOutcome::Applied {
            target,
            material,
            effect,
        })
    }

    pub fn item_in_memory(&self, item_uid: u32) -> Option<InventoryItem> {
        for items in [&self.inventory, &self.equipment, &self.warehouse] {
            if let Some(item) = items.lock().iter().copied().find(|item| item.uid == item_uid) {
                return Some(item);
            }
        }
        None
    }

    /// Increase an owned item's classic plus level. The UID ownership check
    /// and update happen inside one transaction; the runtime mirror changes
    /// only after the database commit succeeds.
    pub async fn upgrade_item(&self, pool: &sqlx::PgPool, item_uid: u32) -> Result<ItemUpgradeOutcome, Error> {
        let Some(mut item) = self.item_in_memory(item_uid) else {
            return Ok(ItemUpgradeOutcome::NotFound);
        };
        let mut transaction = pool.begin().await?;
        let owned: bool = sqlx::query_scalar(
            "SELECT EXISTS(
                SELECT 1 FROM inventory_items WHERE item_uid = $1 AND character_id = $2
                UNION ALL
                SELECT 1 FROM equipment_items WHERE item_uid = $3 AND character_id = $4
             )",
        )
        .bind(i64::from(item_uid))
        .bind(self.inner.character_id)
        .bind(i64::from(item_uid))
        .bind(self.inner.character_id)
        .fetch_one(&mut *transaction)
        .await?;
        if !owned {
            transaction.rollback().await?;
            return Ok(ItemUpgradeOutcome::NotFound);
        }
        sqlx::query("INSERT INTO item_upgrades (item_uid, plus) VALUES ($1, 0) ON CONFLICT DO NOTHING")
            .bind(i64::from(item_uid))
            .execute(&mut *transaction)
            .await?;
        let next = sqlx::query_scalar::<_, i64>(
            "UPDATE item_upgrades SET plus = plus + 1
             WHERE item_uid = $1 AND plus < 12
             RETURNING plus",
        )
        .bind(i64::from(item_uid))
        .fetch_optional(&mut *transaction)
        .await?;
        let Some(next) = next else {
            transaction.rollback().await?;
            item.plus = 12;
            return Ok(ItemUpgradeOutcome::MaxLevel(item));
        };
        transaction.commit().await?;
        item.plus = u8::try_from(next).unwrap_or(12);
        for items in [&self.inventory, &self.equipment, &self.warehouse] {
            if let Some(existing) = items.lock().iter_mut().find(|existing| existing.uid == item_uid) {
                existing.plus = item.plus;
            }
        }
        Ok(ItemUpgradeOutcome::Upgraded(item))
    }

    /// Attempts to increase an owned item by one plus level. The required
    /// Meteors are consumed even when the configured roll fails.
    /// The catalyst, ownership check and plus update are committed together so
    /// a rejected or concurrent request cannot duplicate or destroy an item.
    pub async fn upgrade_item_with_meteor(
        &self,
        pool: &sqlx::PgPool,
        item_uid: u32,
    ) -> Result<(ItemUpgradeOutcome, Vec<InventoryItem>), Error> {
        let Some(mut item) = self.item_in_memory(item_uid) else {
            return Ok((ItemUpgradeOutcome::NotFound, Vec::new()));
        };
        let Some((meteor_cost, chance)) = item_upgrade_cost_and_chance(item.plus) else {
            item.plus = 12;
            return Ok((ItemUpgradeOutcome::MaxLevel(item), Vec::new()));
        };
        let mut transaction = pool.begin().await?;
        let owned: bool = sqlx::query_scalar(
            "SELECT EXISTS(
                SELECT 1 FROM inventory_items WHERE item_uid = $1 AND character_id = $2
                UNION ALL
                SELECT 1 FROM equipment_items WHERE item_uid = $3 AND character_id = $4
             )",
        )
        .bind(i64::from(item_uid))
        .bind(self.inner.character_id)
        .bind(i64::from(item_uid))
        .bind(self.inner.character_id)
        .fetch_one(&mut *transaction)
        .await?;
        if !owned {
            transaction.rollback().await?;
            return Ok((ItemUpgradeOutcome::NotFound, Vec::new()));
        }
        let catalysts = sqlx::query_as::<_, (i64, i64, i64, i64, i64)>(
            "SELECT item_uid, item_type, position, durability, max_durability
             FROM inventory_items
             WHERE character_id = $1 AND item_type = 1088001 AND position < 40
             ORDER BY position LIMIT $2",
        )
        .bind(self.inner.character_id)
        .bind(i64::from(meteor_cost))
        .fetch_all(&mut *transaction)
        .await?;
        if catalysts.len() < usize::from(meteor_cost) {
            transaction.rollback().await?;
            return Ok((ItemUpgradeOutcome::MissingMeteor, Vec::new()));
        }
        sqlx::query("INSERT INTO item_upgrades (item_uid, plus) VALUES ($1, 0) ON CONFLICT DO NOTHING")
            .bind(i64::from(item_uid))
            .execute(&mut *transaction)
            .await?;
        let chance = crate::lucky::lucky_chance(u32::from(chance), self.lucky_time() > 0);
        let upgraded = next_roll() % 100 < chance;
        if upgraded {
            sqlx::query(
                "UPDATE item_upgrades SET plus = plus + 1
                 WHERE item_uid = $1 AND plus < 12",
            )
            .bind(i64::from(item_uid))
            .execute(&mut *transaction)
            .await?;
        }
        for catalyst in &catalysts {
            sqlx::query("DELETE FROM inventory_items WHERE item_uid = $1 AND character_id = $2")
                .bind(catalyst.0)
                .bind(self.inner.character_id)
                .execute(&mut *transaction)
                .await?;
        }
        transaction.commit().await?;

        if upgraded {
            item.plus = item.plus.saturating_add(1).min(12);
            for items in [&self.inventory, &self.equipment, &self.warehouse] {
                if let Some(existing) = items.lock().iter_mut().find(|existing| existing.uid == item_uid) {
                    existing.plus = item.plus;
                }
            }
        }
        let catalysts: Vec<InventoryItem> = catalysts
            .into_iter()
            .map(|catalyst| {
                Ok(InventoryItem {
                    uid: u32::try_from(catalyst.0)
                        .map_err(|_| Error::Other("item ID exceeds protocol range".into()))?,
                    item_type: u32::try_from(catalyst.1).unwrap_or(1_088_001),
                    position: u8::try_from(catalyst.2).unwrap_or_default(),
                    durability: u16::try_from(catalyst.3).unwrap_or_default(),
                    max_durability: u16::try_from(catalyst.4).unwrap_or_default(),
                    plus: 0,
                    ..Default::default()
                })
            })
            .collect::<Result<_, Error>>()?;
        self.inventory
            .lock()
            .retain(|owned| !catalysts.iter().any(|catalyst| catalyst.uid == owned.uid));
        Ok((
            if upgraded {
                ItemUpgradeOutcome::Upgraded(item)
            } else {
                ItemUpgradeOutcome::Failed(item)
            },
            catalysts,
        ))
    }

    /// Returns the classic success chance for a normal/ refined/ unique/ elite
    /// quality transition. Super quality (suffix 9) cannot be upgraded.
    pub fn quality_upgrade_chance(item_type: u32) -> u8 {
        let quality = item_type % 10;
        if !(5..=8).contains(&quality) {
            return 0;
        }
        let level = (item_type / 10) % 1_000;
        85_u32
            .saturating_sub((quality - 5) * 16)
            .saturating_sub(level / 4)
            .clamp(20, 100) as u8
    }

    /// Improves an equipment item's quality with one DragonBall. The item
    /// update, catalyst consumption and failure durability penalty are
    /// committed atomically. The roll is kept injectable for deterministic
    /// domain tests while the packet path uses the server RNG.
    pub async fn upgrade_quality_with_dragonball(
        &self,
        pool: &sqlx::PgPool,
        item_uid: u32,
    ) -> Result<(QualityUpgradeOutcome, Option<InventoryItem>), Error> {
        // Luck shrinks the roll the chance is compared against.
        let roll = next_roll() % 100;
        let roll = if self.lucky_time() > 0 { roll * 9 / 10 } else { roll };
        self.upgrade_quality_with_dragonball_roll(pool, item_uid, roll)
            .await
    }

    async fn upgrade_quality_with_dragonball_roll(
        &self,
        pool: &sqlx::PgPool,
        item_uid: u32,
        roll: u32,
    ) -> Result<(QualityUpgradeOutcome, Option<InventoryItem>), Error> {
        let Some(mut item) = self.item_in_memory(item_uid) else {
            return Ok((QualityUpgradeOutcome::NotFound, None));
        };
        let quality = item.item_type % 10;
        if quality == 9 {
            return Ok((QualityUpgradeOutcome::MaxQuality(item), None));
        }
        let chance = Self::quality_upgrade_chance(item.item_type);
        if chance == 0 {
            return Ok((QualityUpgradeOutcome::InvalidQuality(item), None));
        }

        let mut transaction = pool.begin().await?;
        let inventory_owned: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM inventory_items WHERE item_uid = $1 AND character_id = $2)")
                .bind(i64::from(item_uid))
                .bind(self.inner.character_id)
                .fetch_one(&mut *transaction)
                .await?;
        let equipment_owned: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM equipment_items WHERE item_uid = $1 AND character_id = $2)")
                .bind(i64::from(item_uid))
                .bind(self.inner.character_id)
                .fetch_one(&mut *transaction)
                .await?;
        if !inventory_owned && !equipment_owned {
            transaction.rollback().await?;
            return Ok((QualityUpgradeOutcome::NotFound, None));
        }
        let Some(catalyst) = sqlx::query_as::<_, (i64, i64, i64, i64, i64)>(
            "SELECT item_uid, item_type, position, durability, max_durability
             FROM inventory_items
             WHERE character_id = $1 AND item_type = 1088000 AND position < 40
             ORDER BY position LIMIT 1",
        )
        .bind(self.inner.character_id)
        .fetch_optional(&mut *transaction)
        .await?
        else {
            transaction.rollback().await?;
            return Ok((QualityUpgradeOutcome::MissingDragonBall, None));
        };

        let upgraded = roll < u32::from(chance);
        let next_item_type = item.item_type.saturating_add(1);
        let next_durability = if upgraded {
            item.durability
        } else {
            item.max_durability / 2
        };
        let table = if equipment_owned {
            "equipment_items"
        } else {
            "inventory_items"
        };
        let updated = sqlx::query(&format!(
            "UPDATE {table} SET item_type = $1, durability = $2
             WHERE item_uid = $3 AND character_id = $4 AND item_type = $5"
        ))
        .bind(i64::from(if upgraded { next_item_type } else { item.item_type }))
        .bind(i64::from(next_durability))
        .bind(i64::from(item_uid))
        .bind(self.inner.character_id)
        .bind(i64::from(item.item_type))
        .execute(&mut *transaction)
        .await?;
        if updated.rows_affected() != 1 {
            transaction.rollback().await?;
            return Ok((QualityUpgradeOutcome::NotFound, None));
        }
        let deleted =
            sqlx::query("DELETE FROM inventory_items WHERE item_uid = $1 AND character_id = $2 AND item_type = 1088000")
                .bind(catalyst.0)
                .bind(self.inner.character_id)
                .execute(&mut *transaction)
                .await?;
        if deleted.rows_affected() != 1 {
            transaction.rollback().await?;
            return Ok((QualityUpgradeOutcome::MissingDragonBall, None));
        }
        transaction.commit().await?;

        if upgraded {
            item.item_type = next_item_type;
        }
        item.durability = next_durability;
        for items in [&self.inventory, &self.equipment, &self.warehouse] {
            if let Some(existing) = items.lock().iter_mut().find(|existing| existing.uid == item_uid) {
                *existing = item;
            }
        }
        let catalyst = InventoryItem {
            uid: u32::try_from(catalyst.0).map_err(|_| Error::Other("item ID exceeds protocol range".into()))?,
            item_type: u32::try_from(catalyst.1).unwrap_or(1_088_000),
            position: u8::try_from(catalyst.2).unwrap_or_default(),
            durability: u16::try_from(catalyst.3).unwrap_or_default(),
            max_durability: u16::try_from(catalyst.4).unwrap_or_default(),
            ..Default::default()
        };
        self.inventory.lock().retain(|owned| owned.uid != catalyst.uid);
        let outcome = if upgraded {
            QualityUpgradeOutcome::Upgraded(item)
        } else {
            QualityUpgradeOutcome::Failed(item)
        };
        Ok((outcome, Some(catalyst)))
    }

    /// Opens the next available socket on an owned equipment item. The first
    /// socket costs one DragonBall; the second costs five, matching the
    /// classic 5065 progression.
    /// Reforges a worn item at an artisan: replaces its type (next quality
    /// or next level), consumes `cost` catalysts of `catalyst_type` and, when
    /// `open_socket` is set, opens the next closed socket. Everything is one
    /// transaction; `None` means the item or the catalysts were missing.
    pub async fn reforge_equipment(
        &self,
        pool: &sqlx::PgPool,
        position: u8,
        new_type: u32,
        catalyst_type: u32,
        cost: u32,
        open_socket: bool,
    ) -> Result<Option<(InventoryItem, Vec<InventoryItem>)>, Error> {
        let Some(mut item) = self.equipment_item(position) else {
            return Ok(None);
        };
        let catalysts = self
            .inventory_items()
            .into_iter()
            .filter(|owned| owned.item_type == catalyst_type)
            .take(cost as usize)
            .collect::<Vec<_>>();
        if catalysts.len() != cost as usize {
            return Ok(None);
        }
        let mut transaction = pool.begin().await?;
        for catalyst in &catalysts {
            let deleted =
                sqlx::query("DELETE FROM inventory_items WHERE item_uid = $1 AND character_id = $2 AND item_type = $3")
                    .bind(i64::from(catalyst.uid))
                    .bind(self.inner.character_id)
                    .bind(i64::from(catalyst_type))
                    .execute(&mut *transaction)
                    .await?;
            if deleted.rows_affected() != 1 {
                transaction.rollback().await?;
                return Ok(None);
            }
        }
        let updated = sqlx::query(
            "UPDATE equipment_items SET item_type = $1
             WHERE item_uid = $2 AND character_id = $3 AND item_type = $4 AND position = $5",
        )
        .bind(i64::from(new_type))
        .bind(i64::from(item.uid))
        .bind(self.inner.character_id)
        .bind(i64::from(item.item_type))
        .bind(i64::from(position))
        .execute(&mut *transaction)
        .await?;
        if updated.rows_affected() != 1 {
            transaction.rollback().await?;
            return Ok(None);
        }
        item.item_type = new_type;
        if open_socket {
            if item.socket_one == 0 && item.gem_one == 0 {
                item.socket_one = 1;
            } else if item.socket_two == 0 && item.gem_two == 0 {
                item.socket_two = 1;
            }
            sqlx::query(
                "INSERT INTO item_upgrades (item_uid, plus, gem_one, gem_two, socket_one, socket_two, rarity)
                 VALUES ($1, $2, $3, $4, $5, $6, $7)
                 ON CONFLICT(item_uid) DO UPDATE SET socket_one = excluded.socket_one,
                   socket_two = excluded.socket_two",
            )
            .bind(i64::from(item.uid))
            .bind(i64::from(item.plus))
            .bind(i64::from(item.gem_one))
            .bind(i64::from(item.gem_two))
            .bind(i64::from(item.socket_one))
            .bind(i64::from(item.socket_two))
            .bind(i64::from(item.rarity))
            .execute(&mut *transaction)
            .await?;
        }
        transaction.commit().await?;
        if let Some(existing) = self.equipment.lock().iter_mut().find(|existing| existing.uid == item.uid) {
            *existing = item;
        }
        self.inventory
            .lock()
            .retain(|owned| !catalysts.iter().any(|catalyst| catalyst.uid == owned.uid));
        Ok(Some((item, catalysts)))
    }

    pub async fn open_item_socket(
        &self,
        pool: &sqlx::PgPool,
        item_uid: u32,
    ) -> Result<(SocketOutcome, Vec<InventoryItem>), Error> {
        let Some(mut item) = self.item_in_memory(item_uid) else {
            return Ok((SocketOutcome::NotFound, Vec::new()));
        };
        if ShopCatalog::equipment_slot_for_item(item.item_type).is_none() {
            return Ok((SocketOutcome::NotFound, Vec::new()));
        }
        let first_open = item.socket_one != 0 || item.gem_one == 255;
        let second_open = item.socket_two != 0 || item.gem_two == 255;
        let (socket_one, socket_two, cost) = if !first_open {
            (1, item.socket_two, 1_u32)
        } else if !second_open {
            (1, 1, 5_u32)
        } else {
            return Ok((SocketOutcome::DualSockets, Vec::new()));
        };
        let catalysts = self
            .inventory_items()
            .into_iter()
            .filter(|owned| owned.item_type == 1_088_000)
            .take(cost as usize)
            .collect::<Vec<_>>();
        if catalysts.len() != cost as usize {
            return Ok((SocketOutcome::MissingDragonBall, Vec::new()));
        }
        let mut transaction = pool.begin().await?;
        for catalyst in &catalysts {
            let deleted =
                sqlx::query("DELETE FROM inventory_items WHERE item_uid = $1 AND character_id = $2 AND item_type = $3")
                    .bind(i64::from(catalyst.uid))
                    .bind(self.inner.character_id)
                    .bind(1_088_000_i64)
                    .execute(&mut *transaction)
                    .await?;
            if deleted.rows_affected() != 1 {
                transaction.rollback().await?;
                return Ok((SocketOutcome::MissingDragonBall, Vec::new()));
            }
        }
        sqlx::query(
            "INSERT INTO item_upgrades (item_uid, plus, gem_one, gem_two, socket_one, socket_two, rarity)
             VALUES ($1, $2, $3, $4, $5, $6, $7)
             ON CONFLICT(item_uid) DO UPDATE SET plus = excluded.plus, gem_one = excluded.gem_one,
               gem_two = excluded.gem_two, socket_one = excluded.socket_one, socket_two = excluded.socket_two,
               rarity = excluded.rarity",
        )
        .bind(i64::from(item.uid))
        .bind(i64::from(item.plus))
        .bind(i64::from(item.gem_one))
        .bind(i64::from(item.gem_two))
        .bind(i64::from(socket_one))
        .bind(i64::from(socket_two))
        .bind(i64::from(item.rarity))
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;
        item.socket_one = socket_one;
        item.socket_two = socket_two;
        for items in [&self.inventory, &self.equipment, &self.warehouse] {
            if let Some(existing) = items.lock().iter_mut().find(|existing| existing.uid == item.uid) {
                *existing = item;
            }
        }
        self.inventory
            .lock()
            .retain(|owned| !catalysts.iter().any(|catalyst| catalyst.uid == owned.uid));
        Ok((SocketOutcome::Opened(item), catalysts))
    }

    /// Inserts or removes a gem through packet 1027. `socket` is 1 or 2 and
    /// `remove` follows the classic subtype convention (0 insert, 1 remove).
    pub async fn socket_gem(
        &self,
        pool: &sqlx::PgPool,
        item_uid: u32,
        gem_uid: u32,
        socket: u8,
        remove: bool,
    ) -> Result<SocketOutcome, Error> {
        if !matches!(socket, 1 | 2) {
            return Ok(SocketOutcome::InvalidSlot);
        }
        let Some(mut item) = self.item_in_memory(item_uid) else {
            return Ok(SocketOutcome::NotFound);
        };
        let first = socket == 1;
        let open = if first {
            item.socket_one != 0 || item.gem_one == 255
        } else {
            item.socket_two != 0 || item.gem_two == 255
        };
        let occupied = if first {
            item.gem_one > 0 && item.gem_one != 255
        } else {
            item.gem_two > 0 && item.gem_two != 255
        };
        if !open {
            return Ok(SocketOutcome::SocketClosed);
        }
        if remove {
            if !occupied {
                return Ok(SocketOutcome::SocketOccupied);
            }
            if first {
                item.gem_one = 0;
            } else {
                item.gem_two = 0;
            }
            sqlx::query(
                "INSERT INTO item_upgrades (item_uid, plus, gem_one, gem_two, socket_one, socket_two, rarity)
                 VALUES ($1, $2, $3, $4, $5, $6, $7)
                 ON CONFLICT(item_uid) DO UPDATE SET plus = excluded.plus, gem_one = excluded.gem_one,
                   gem_two = excluded.gem_two, socket_one = excluded.socket_one, socket_two = excluded.socket_two,
                   rarity = excluded.rarity",
            )
            .bind(i64::from(item.uid))
            .bind(i64::from(item.plus))
            .bind(i64::from(item.gem_one))
            .bind(i64::from(item.gem_two))
            .bind(i64::from(item.socket_one.max(if first { 1 } else { 0 })))
            .bind(i64::from(item.socket_two.max(if first { 0 } else { 1 })))
            .bind(i64::from(item.rarity))
            .execute(pool)
            .await?;
            for items in [&self.inventory, &self.equipment, &self.warehouse] {
                if let Some(existing) = items.lock().iter_mut().find(|existing| existing.uid == item.uid) {
                    *existing = item;
                }
            }
            return Ok(SocketOutcome::Removed(item));
        }
        let Some(gem) = self.inventory_items().into_iter().find(|owned| owned.uid == gem_uid) else {
            return Ok(SocketOutcome::NotFound);
        };
        if !(700_000..800_000).contains(&gem.item_type) || gem.item_type % 1000 == 0 {
            return Ok(SocketOutcome::InvalidGem);
        }
        if occupied {
            return Ok(SocketOutcome::SocketOccupied);
        }
        let code = u8::try_from(gem.item_type % 1000).unwrap_or_default();
        if first {
            item.gem_one = code;
        } else {
            item.gem_two = code;
        }
        let mut transaction = pool.begin().await?;
        let deleted = sqlx::query("DELETE FROM inventory_items WHERE item_uid = $1 AND character_id = $2")
            .bind(i64::from(gem.uid))
            .bind(self.inner.character_id)
            .execute(&mut *transaction)
            .await?;
        if deleted.rows_affected() != 1 {
            transaction.rollback().await?;
            return Ok(SocketOutcome::NotFound);
        }
        sqlx::query(
            "INSERT INTO item_upgrades (item_uid, plus, gem_one, gem_two, socket_one, socket_two, rarity)
             VALUES ($1, $2, $3, $4, $5, $6, $7)
             ON CONFLICT(item_uid) DO UPDATE SET plus = excluded.plus, gem_one = excluded.gem_one,
               gem_two = excluded.gem_two, socket_one = excluded.socket_one, socket_two = excluded.socket_two,
               rarity = excluded.rarity",
        )
        .bind(i64::from(item.uid))
        .bind(i64::from(item.plus))
        .bind(i64::from(item.gem_one))
        .bind(i64::from(item.gem_two))
        .bind(i64::from(item.socket_one.max(if first { 1 } else { 0 })))
        .bind(i64::from(item.socket_two.max(if first { 0 } else { 1 })))
        .bind(i64::from(item.rarity))
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;
        for items in [&self.inventory, &self.equipment, &self.warehouse] {
            if let Some(existing) = items.lock().iter_mut().find(|existing| existing.uid == item.uid) {
                *existing = item;
            }
        }
        self.inventory.lock().retain(|owned| owned.uid != gem.uid);
        Ok(SocketOutcome::Inserted { item, gem })
    }

    /// Composes fifteen equal normal/refined gems into the next tier. The
    /// required silver and all item removals are committed atomically.
    pub async fn compose_gems(&self, pool: &sqlx::PgPool, gem_type: u32) -> Result<GemComposeOutcome, Error> {
        let tier = gem_type % 10;
        if !matches!(tier, 1 | 2) || !GEM_FAMILY_BASES.iter().any(|base| *base + tier == gem_type) {
            return Ok(GemComposeOutcome::InvalidGem);
        }
        let price = if tier == 1 { 10_000 } else { 100_000 };
        let consumed = self
            .inventory_items()
            .into_iter()
            .filter(|item| item.item_type == gem_type)
            .take(15)
            .collect::<Vec<_>>();
        if consumed.len() != 15 {
            return Ok(GemComposeOutcome::MissingGems);
        }
        let consumed_uids = consumed
            .iter()
            .map(|item| item.uid)
            .collect::<std::collections::HashSet<_>>();
        let Some(position) = (0_u8..40).find(|position| {
            self.inventory_items()
                .iter()
                .all(|item| item.position != *position || consumed_uids.contains(&item.uid))
        }) else {
            return Ok(GemComposeOutcome::InventoryFull);
        };
        let mut transaction = pool.begin().await?;
        let balance = sqlx::query_scalar::<_, i64>(
            "UPDATE characters SET silver = silver - $1
             WHERE character_id = $2 AND silver >= $3 RETURNING silver",
        )
        .bind(i64::from(price))
        .bind(self.inner.character_id)
        .bind(i64::from(price))
        .fetch_optional(&mut *transaction)
        .await?;
        let Some(balance) = balance else {
            transaction.rollback().await?;
            return Ok(GemComposeOutcome::InsufficientSilver);
        };
        for item in &consumed {
            let deleted =
                sqlx::query("DELETE FROM inventory_items WHERE item_uid = $1 AND character_id = $2 AND item_type = $3")
                    .bind(i64::from(item.uid))
                    .bind(self.inner.character_id)
                    .bind(i64::from(gem_type))
                    .execute(&mut *transaction)
                    .await?;
            if deleted.rows_affected() != 1 {
                transaction.rollback().await?;
                return Ok(GemComposeOutcome::MissingGems);
            }
            sqlx::query("DELETE FROM item_upgrades WHERE item_uid = $1")
                .bind(i64::from(item.uid))
                .execute(&mut *transaction)
                .await?;
        }
        let created_uid: i64 = sqlx::query_scalar(
            "INSERT INTO inventory_items (character_id, item_type, position, durability, max_durability)
             VALUES ($1, $2, $3, 100, 100) RETURNING item_uid",
        )
        .bind(self.inner.character_id)
        .bind(i64::from(gem_type + 1))
        .bind(i64::from(position))
        .fetch_one(&mut *transaction)
        .await?;
        transaction.commit().await?;
        let created = InventoryItem {
            uid: u32::try_from(created_uid).unwrap_or_default(),
            item_type: gem_type + 1,
            position,
            durability: 100,
            max_durability: 100,
            ..Default::default()
        };
        self.silver.store(balance.max(0) as u64, Ordering::Relaxed);
        let mut inventory = self.inventory.lock();
        inventory.retain(|item| !consumed_uids.contains(&item.uid));
        inventory.push(created);
        inventory.sort_by_key(|item| item.position);
        Ok(GemComposeOutcome::Composed {
            consumed,
            created,
            silver: price,
        })
    }

    /// Reduces an equipped item's durability and returns its new state.
    /// Durability never becomes negative and the database update is the source
    /// of truth for concurrent combat actions.
    /// Destroys the worn item at `position` (spent arrows).
    pub async fn destroy_equipment_item(
        &self,
        pool: &sqlx::PgPool,
        position: u8,
    ) -> Result<Option<InventoryItem>, Error> {
        let Some(item) = self.equipment_item(position) else {
            return Ok(None);
        };
        let deleted = sqlx::query("DELETE FROM equipment_items WHERE character_id = $1 AND item_uid = $2")
            .bind(self.inner.character_id)
            .bind(i64::from(item.uid))
            .execute(pool)
            .await?;
        if deleted.rows_affected() != 1 {
            return Ok(None);
        }
        sqlx::query("DELETE FROM item_upgrades WHERE item_uid = $1")
            .bind(i64::from(item.uid))
            .execute(pool)
            .await?;
        self.equipment.lock().retain(|owned| owned.uid != item.uid);
        Ok(Some(item))
    }

    pub async fn degrade_equipment_item(
        &self,
        pool: &sqlx::PgPool,
        position: u8,
        amount: u16,
    ) -> Result<Option<InventoryItem>, Error> {
        if amount == 0 {
            return Ok(self.equipment_item(position));
        }
        let row = sqlx::query_as::<_, (i64, i64, i64, i64, i64)>(
            "UPDATE equipment_items
             SET durability = CASE
                 WHEN durability > $1 THEN durability - $2
                 ELSE 0
             END
             WHERE character_id = $3 AND position = $4 AND durability > 0
             RETURNING item_uid, item_type, position, durability, max_durability",
        )
        .bind(i64::from(amount))
        .bind(i64::from(amount))
        .bind(self.inner.character_id)
        .bind(i64::from(position))
        .fetch_optional(pool)
        .await?;
        let Some((uid, item_type, position, durability, max_durability)) = row else {
            return Ok(None);
        };
        let item = InventoryItem {
            uid: u32::try_from(uid).map_err(|_| Error::Other("item ID exceeds protocol range".into()))?,
            item_type: u32::try_from(item_type).map_err(|_| Error::Other("item type exceeds protocol range".into()))?,
            position: u8::try_from(position)
                .map_err(|_| Error::Other("item position exceeds protocol range".into()))?,
            durability: u16::try_from(durability).unwrap_or_default(),
            max_durability: u16::try_from(max_durability).unwrap_or_default(),
            ..self
                .item_in_memory(u32::try_from(uid).unwrap_or_default())
                .unwrap_or_default()
        };
        if let Some(existing) = self
            .equipment
            .lock()
            .iter_mut()
            .find(|existing| existing.uid == item.uid)
        {
            *existing = item;
        }
        Ok(Some(item))
    }

    /// Repairs all damaged equipment while charging silver in the same
    /// transaction. `None` means the character could not pay the cost.
    pub async fn repair_all_equipment(
        &self,
        pool: &sqlx::PgPool,
        cost: u32,
    ) -> Result<Option<Vec<InventoryItem>>, Error> {
        let mut transaction = pool.begin().await?;
        let rows = sqlx::query_as::<_, (i64, i64, i64, i64, i64)>(
            "SELECT item_uid, item_type, position, durability, max_durability
             FROM equipment_items
             WHERE character_id = $1 AND durability < max_durability
             ORDER BY position",
        )
        .bind(self.inner.character_id)
        .fetch_all(&mut *transaction)
        .await?;
        if rows.is_empty() {
            transaction.rollback().await?;
            return Ok(Some(Vec::new()));
        }
        let balance = sqlx::query_scalar::<_, i64>(
            "UPDATE characters SET silver = silver - $1
             WHERE character_id = $2 AND silver >= $3
             RETURNING silver",
        )
        .bind(i64::from(cost))
        .bind(self.inner.character_id)
        .bind(i64::from(cost))
        .fetch_optional(&mut *transaction)
        .await?;
        let Some(balance) = balance else {
            transaction.rollback().await?;
            return Ok(None);
        };
        sqlx::query(
            "UPDATE equipment_items SET durability = max_durability
             WHERE character_id = $1 AND durability < max_durability",
        )
        .bind(self.inner.character_id)
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;

        let repaired = rows
            .into_iter()
            .map(
                |(uid, item_type, position, _durability, max_durability)| InventoryItem {
                    uid: u32::try_from(uid).unwrap_or_default(),
                    item_type: u32::try_from(item_type).unwrap_or_default(),
                    position: u8::try_from(position).unwrap_or_default(),
                    durability: u16::try_from(max_durability).unwrap_or_default(),
                    max_durability: u16::try_from(max_durability).unwrap_or_default(),
                    ..self
                        .item_in_memory(u32::try_from(uid).unwrap_or_default())
                        .unwrap_or_default()
                },
            )
            .collect::<Vec<_>>();
        self.silver.store(balance.max(0) as u64, Ordering::Relaxed);
        let mut equipment = self.equipment.lock();
        for item in &repaired {
            if let Some(existing) = equipment.iter_mut().find(|existing| existing.uid == item.uid) {
                *existing = *item;
            }
        }
        Ok(Some(repaired))
    }

    pub fn equipment_item(&self, position: u8) -> Option<InventoryItem> {
        self.equipment
            .lock()
            .iter()
            .copied()
            .find(|item| item.position == position)
    }

    pub async fn equip_inventory_item(
        &self,
        pool: &sqlx::PgPool,
        item_uid: u32,
        position: u8,
    ) -> Result<Option<(InventoryItem, Option<InventoryItem>)>, Error> {
        if !(1..=9).contains(&position) {
            return Ok(None);
        }
        let mut transaction = pool.begin().await?;
        let source = sqlx::query_as::<_, (i64, i64, i64, i64, i64)>(
            "SELECT item_uid, item_type, position, durability, max_durability \
             FROM inventory_items WHERE item_uid = $1 AND character_id = $2 AND position < 40",
        )
        .bind(i64::from(item_uid))
        .bind(self.inner.character_id)
        .fetch_optional(&mut *transaction)
        .await?;
        let Some((uid, item_type, old_position, durability, max_durability)) = source else {
            return Ok(None);
        };
        let old_position =
            u8::try_from(old_position).map_err(|_| Error::Other("item position exceeds protocol range".into()))?;
        let source = InventoryItem {
            uid: u32::try_from(uid).map_err(|_| Error::Other("item uid exceeds protocol range".into()))?,
            item_type: u32::try_from(item_type).map_err(|_| Error::Other("item type exceeds protocol range".into()))?,
            position,
            durability: u16::try_from(durability).unwrap_or_default(),
            max_durability: u16::try_from(max_durability).unwrap_or_default(),
            ..self
                .item_in_memory(u32::try_from(uid).unwrap_or_default())
                .unwrap_or_default()
        };
        let destination = sqlx::query_as::<_, (i64, i64, i64, i64, i64)>(
            "SELECT item_uid, item_type, position, durability, max_durability \
             FROM equipment_items WHERE character_id = $1 AND position = $2",
        )
        .bind(self.inner.character_id)
        .bind(i64::from(position))
        .fetch_optional(&mut *transaction)
        .await?
        .map(|(uid, item_type, position, durability, max_durability)| InventoryItem {
            uid: u32::try_from(uid).unwrap_or_default(),
            item_type: u32::try_from(item_type).unwrap_or_default(),
            position: u8::try_from(position).unwrap_or_default(),
            durability: u16::try_from(durability).unwrap_or_default(),
            max_durability: u16::try_from(max_durability).unwrap_or_default(),
            ..self
                .item_in_memory(u32::try_from(uid).unwrap_or_default())
                .unwrap_or_default()
        });
        sqlx::query("DELETE FROM inventory_items WHERE item_uid = $1 AND character_id = $2")
            .bind(i64::from(item_uid))
            .bind(self.inner.character_id)
            .execute(&mut *transaction)
            .await?;
        if let Some(item) = destination {
            sqlx::query("DELETE FROM equipment_items WHERE item_uid = $1 AND character_id = $2")
                .bind(i64::from(item.uid))
                .bind(self.inner.character_id)
                .execute(&mut *transaction)
                .await?;
            sqlx::query(
                "INSERT INTO inventory_items \
                 (item_uid, character_id, item_type, position, durability, max_durability) \
                 VALUES ($1, $2, $3, $4, $5, $6)",
            )
            .bind(i64::from(item.uid))
            .bind(self.inner.character_id)
            .bind(i64::from(item.item_type))
            .bind(i64::from(old_position))
            .bind(i64::from(item.durability))
            .bind(i64::from(item.max_durability))
            .execute(&mut *transaction)
            .await?;
        }
        sqlx::query(
            "INSERT INTO equipment_items \
             (item_uid, character_id, item_type, position, durability, max_durability) \
             VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(i64::from(source.uid))
        .bind(self.inner.character_id)
        .bind(i64::from(source.item_type))
        .bind(i64::from(source.position))
        .bind(i64::from(source.durability))
        .bind(i64::from(source.max_durability))
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;

        let mut inventory = self.inventory.lock();
        inventory.retain(|item| item.uid != source.uid);
        if let Some(item) = destination {
            inventory.retain(|existing| existing.uid != item.uid);
            inventory.push(InventoryItem {
                position: old_position,
                ..item
            });
        }
        inventory.sort_by_key(|item| item.position);
        drop(inventory);
        let mut equipment = self.equipment.lock();
        equipment.retain(|item| item.position != position);
        equipment.push(source);
        equipment.sort_by_key(|item| item.position);
        Ok(Some((
            source,
            destination.map(|item| InventoryItem {
                position: old_position,
                ..item
            }),
        )))
    }

    pub async fn unequip_item(
        &self,
        pool: &sqlx::PgPool,
        item_uid: u32,
        inventory_position: u8,
    ) -> Result<Option<InventoryItem>, Error> {
        if inventory_position >= 40 {
            return Ok(None);
        }
        let mut transaction = pool.begin().await?;
        let source = sqlx::query_as::<_, (i64, i64, i64, i64, i64)>(
            "SELECT item_uid, item_type, position, durability, max_durability \
             FROM equipment_items WHERE item_uid = $1 AND character_id = $2",
        )
        .bind(i64::from(item_uid))
        .bind(self.inner.character_id)
        .fetch_optional(&mut *transaction)
        .await?;
        let Some((uid, item_type, position, durability, max_durability)) = source else {
            return Ok(None);
        };
        let occupied = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM inventory_items WHERE character_id = $1 AND position = $2)",
        )
        .bind(self.inner.character_id)
        .bind(i64::from(inventory_position))
        .fetch_one(&mut *transaction)
        .await?;
        if occupied {
            return Ok(None);
        }
        sqlx::query("DELETE FROM equipment_items WHERE item_uid = $1 AND character_id = $2")
            .bind(i64::from(item_uid))
            .bind(self.inner.character_id)
            .execute(&mut *transaction)
            .await?;
        sqlx::query(
            "INSERT INTO inventory_items \
             (item_uid, character_id, item_type, position, durability, max_durability) \
             VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(uid)
        .bind(self.inner.character_id)
        .bind(item_type)
        .bind(i64::from(inventory_position))
        .bind(durability)
        .bind(max_durability)
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;
        let item = InventoryItem {
            uid: u32::try_from(uid).unwrap_or_default(),
            item_type: u32::try_from(item_type).unwrap_or_default(),
            position: inventory_position,
            durability: u16::try_from(durability).unwrap_or_default(),
            max_durability: u16::try_from(max_durability).unwrap_or_default(),
            ..self
                .item_in_memory(u32::try_from(uid).unwrap_or_default())
                .unwrap_or_default()
        };
        self.equipment.lock().retain(|existing| existing.uid != item.uid);
        self.inventory.lock().push(item);
        self.inventory.lock().sort_by_key(|existing| existing.position);
        let _ = position;
        Ok(Some(item))
    }

    pub async fn move_inventory_item(
        &self,
        pool: &sqlx::PgPool,
        item_uid: u32,
        position: u8,
    ) -> Result<Option<(InventoryItem, Option<InventoryItem>)>, Error> {
        if position >= 40 {
            return Ok(None);
        }

        let mut transaction = pool.begin().await?;
        let source_row = sqlx::query_as::<_, (i64, i64, i64, i64, i64)>(
            "SELECT item_uid, item_type, position, durability, max_durability \
             FROM inventory_items WHERE item_uid = $1 AND character_id = $2 AND position < 40",
        )
        .bind(i64::from(item_uid))
        .bind(self.inner.character_id)
        .fetch_optional(&mut *transaction)
        .await?;
        let Some((uid, item_type, old_position, durability, max_durability)) = source_row else {
            return Ok(None);
        };
        let old_position =
            u8::try_from(old_position).map_err(|_| Error::Other("item position exceeds protocol range".into()))?;
        let source = InventoryItem {
            uid: u32::try_from(uid).map_err(|_| Error::Other("item ID exceeds protocol range".into()))?,
            item_type: u32::try_from(item_type).map_err(|_| Error::Other("item type exceeds protocol range".into()))?,
            position: old_position,
            durability: u16::try_from(durability)
                .map_err(|_| Error::Other("item durability exceeds protocol range".into()))?,
            max_durability: u16::try_from(max_durability)
                .map_err(|_| Error::Other("item durability exceeds protocol range".into()))?,
            ..self
                .item_in_memory(u32::try_from(uid).unwrap_or_default())
                .unwrap_or_default()
        };
        if old_position == position {
            return Ok(Some((source, None)));
        }

        let destination_row = sqlx::query_as::<_, (i64, i64, i64, i64, i64)>(
            "SELECT item_uid, item_type, position, durability, max_durability \
             FROM inventory_items WHERE character_id = $1 AND position = $2",
        )
        .bind(self.inner.character_id)
        .bind(i64::from(position))
        .fetch_optional(&mut *transaction)
        .await?;
        let destination = destination_row
            .map(|(uid, item_type, destination_position, durability, max_durability)| {
                Ok::<_, Error>(InventoryItem {
                    uid: u32::try_from(uid).map_err(|_| Error::Other("item ID exceeds protocol range".into()))?,
                    item_type: u32::try_from(item_type)
                        .map_err(|_| Error::Other("item type exceeds protocol range".into()))?,
                    position: u8::try_from(destination_position)
                        .map_err(|_| Error::Other("item position exceeds protocol range".into()))?,
                    durability: u16::try_from(durability)
                        .map_err(|_| Error::Other("item durability exceeds protocol range".into()))?,
                    max_durability: u16::try_from(max_durability)
                        .map_err(|_| Error::Other("item durability exceeds protocol range".into()))?,
                    ..self
                        .item_in_memory(u32::try_from(uid).unwrap_or_default())
                        .unwrap_or_default()
                })
            })
            .transpose()?;

        if let Some(destination) = destination {
            sqlx::query("DELETE FROM inventory_items WHERE item_uid = $1 AND character_id = $2")
                .bind(i64::from(destination.uid))
                .bind(self.inner.character_id)
                .execute(&mut *transaction)
                .await?;
            sqlx::query("UPDATE inventory_items SET position = $1 WHERE item_uid = $2 AND character_id = $3")
                .bind(i64::from(position))
                .bind(i64::from(source.uid))
                .bind(self.inner.character_id)
                .execute(&mut *transaction)
                .await?;
            sqlx::query(
                "INSERT INTO inventory_items \
                 (item_uid, character_id, item_type, position, durability, max_durability) \
                 VALUES ($1, $2, $3, $4, $5, $6)",
            )
            .bind(i64::from(destination.uid))
            .bind(self.inner.character_id)
            .bind(i64::from(destination.item_type))
            .bind(i64::from(old_position))
            .bind(i64::from(destination.durability))
            .bind(i64::from(destination.max_durability))
            .execute(&mut *transaction)
            .await?;
        } else {
            sqlx::query("UPDATE inventory_items SET position = $1 WHERE item_uid = $2 AND character_id = $3")
                .bind(i64::from(position))
                .bind(i64::from(source.uid))
                .bind(self.inner.character_id)
                .execute(&mut *transaction)
                .await?;
        }
        transaction.commit().await?;

        let moved = InventoryItem { position, ..source };
        let swapped = destination.map(|item| InventoryItem {
            position: old_position,
            ..item
        });
        let mut inventory = self.inventory.lock();
        for item in inventory.iter_mut() {
            if item.uid == moved.uid {
                item.position = moved.position;
            } else if let Some(swapped) = swapped {
                if item.uid == swapped.uid {
                    item.position = swapped.position;
                }
            }
        }
        inventory.sort_by_key(|item| item.position);
        Ok(Some((moved, swapped)))
    }

    /// Atomically exchanges inventory rows and silver between two characters.
    /// The caller must have already completed the two-step trade handshake;
    /// this method revalidates ownership, free slots and balances inside one
    /// database transaction so stale clients cannot duplicate an item or silver.
    pub async fn complete_trade(
        &self,
        pool: &sqlx::PgPool,
        other: &Self,
        offered_uids: &[u32],
        requested_uids: &[u32],
        offered_silver: u32,
        requested_silver: u32,
        offered_cps: u32,
        requested_cps: u32,
    ) -> Result<Option<TradeTransfer>, Error> {
        if self.database_id() == other.database_id() {
            return Ok(None);
        }
        let offered_set = offered_uids.iter().copied().collect::<std::collections::HashSet<_>>();
        let requested_set = requested_uids.iter().copied().collect::<std::collections::HashSet<_>>();
        if offered_set.len() != offered_uids.len()
            || requested_set.len() != requested_uids.len()
            || offered_set.intersection(&requested_set).next().is_some()
        {
            return Ok(None);
        }

        let mut transaction = pool.begin().await?;
        let to_item = |row: (i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64)| {
            Ok::<InventoryItem, Error>(InventoryItem {
                uid: u32::try_from(row.0).map_err(|_| Error::Other("trade item UID exceeds protocol range".into()))?,
                item_type: u32::try_from(row.1)
                    .map_err(|_| Error::Other("trade item type exceeds protocol range".into()))?,
                position: u8::try_from(row.2)
                    .map_err(|_| Error::Other("trade item position exceeds protocol range".into()))?,
                durability: u16::try_from(row.3)
                    .map_err(|_| Error::Other("trade item durability exceeds protocol range".into()))?,
                max_durability: u16::try_from(row.4)
                    .map_err(|_| Error::Other("trade item durability exceeds protocol range".into()))?,
                plus: u8::try_from(row.5).unwrap_or_default(),
                rarity: u8::try_from(row.6).unwrap_or_default(),
                gem_one: u8::try_from(row.7).unwrap_or_default(),
                gem_two: u8::try_from(row.8).unwrap_or_default(),
                socket_one: u8::try_from(row.9).unwrap_or_default(),
                socket_two: u8::try_from(row.10).unwrap_or_default(),
                rarity_roll: u8::try_from(row.11).unwrap_or_default(),
                bless: 0,
                enchant_hp: 0,
                color: 0,
            })
        };
        let mut left_items = Vec::with_capacity(offered_uids.len());
        for uid in offered_uids {
            let Some(row) = sqlx::query_as::<_, (i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64)>(
                "SELECT i.item_uid, i.item_type, i.position, i.durability, i.max_durability,
                        COALESCE(u.plus, 0), COALESCE(u.rarity, 0), COALESCE(u.gem_one, 0), COALESCE(u.gem_two, 0),
                        COALESCE(u.socket_one, 0), COALESCE(u.socket_two, 0), COALESCE(u.enchant, 0)
                 FROM inventory_items i
                 LEFT JOIN item_upgrades u ON u.item_uid = i.item_uid
                 WHERE i.item_uid = $1 AND i.character_id = $2 AND i.position < 40",
            )
            .bind(i64::from(*uid))
            .bind(self.database_id())
            .fetch_optional(&mut *transaction)
            .await?
            else {
                return Ok(None);
            };
            left_items.push(to_item(row)?);
        }
        let mut right_items = Vec::with_capacity(requested_uids.len());
        for uid in requested_uids {
            let Some(row) = sqlx::query_as::<_, (i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64)>(
                "SELECT i.item_uid, i.item_type, i.position, i.durability, i.max_durability,
                        COALESCE(u.plus, 0), COALESCE(u.rarity, 0), COALESCE(u.gem_one, 0), COALESCE(u.gem_two, 0),
                        COALESCE(u.socket_one, 0), COALESCE(u.socket_two, 0), COALESCE(u.enchant, 0)
                 FROM inventory_items i
                 LEFT JOIN item_upgrades u ON u.item_uid = i.item_uid
                 WHERE i.item_uid = $1 AND i.character_id = $2 AND i.position < 40",
            )
            .bind(i64::from(*uid))
            .bind(other.database_id())
            .fetch_optional(&mut *transaction)
            .await?
            else {
                return Ok(None);
            };
            right_items.push(to_item(row)?);
        }

        let left_positions = sqlx::query_as::<_, (i64, i64)>(
            "SELECT item_uid, position FROM inventory_items WHERE character_id = $1 AND position < 40",
        )
        .bind(self.database_id())
        .fetch_all(&mut *transaction)
        .await?;
        let right_positions = sqlx::query_as::<_, (i64, i64)>(
            "SELECT item_uid, position FROM inventory_items WHERE character_id = $1 AND position < 40",
        )
        .bind(other.database_id())
        .fetch_all(&mut *transaction)
        .await?;
        let left_outgoing = offered_set.clone();
        let right_outgoing = requested_set.clone();
        let free_positions = |rows: &[(i64, i64)], outgoing: &std::collections::HashSet<u32>, incoming: usize| {
            let occupied = rows
                .iter()
                .filter(|(uid, _)| !outgoing.contains(&u32::try_from(*uid).unwrap_or_default()))
                .filter_map(|(_, position)| u8::try_from(*position).ok())
                .collect::<std::collections::HashSet<_>>();
            let slots = (0_u8..40)
                .filter(|position| !occupied.contains(position))
                .take(incoming)
                .collect::<Vec<_>>();
            (slots.len() == incoming).then_some(slots)
        };
        let left_receive_positions = free_positions(&left_positions, &left_outgoing, right_items.len());
        let right_receive_positions = free_positions(&right_positions, &right_outgoing, left_items.len());
        let (Some(left_receive_positions), Some(right_receive_positions)) =
            (left_receive_positions, right_receive_positions)
        else {
            return Ok(None);
        };
        if offered_silver > 0 {
            let updated =
                sqlx::query("UPDATE characters SET silver = silver - $1 WHERE character_id = $2 AND silver >= $3")
                    .bind(i64::from(offered_silver))
                    .bind(self.database_id())
                    .bind(i64::from(offered_silver))
                    .execute(&mut *transaction)
                    .await?;
            if updated.rows_affected() != 1 {
                return Ok(None);
            }
        }
        if requested_silver > 0 {
            let updated =
                sqlx::query("UPDATE characters SET silver = silver - $1 WHERE character_id = $2 AND silver >= $3")
                    .bind(i64::from(requested_silver))
                    .bind(other.database_id())
                    .bind(i64::from(requested_silver))
                    .execute(&mut *transaction)
                    .await?;
            if updated.rows_affected() != 1 {
                return Ok(None);
            }
        }
        let left_credit = i64::from(requested_silver);
        let right_credit = i64::from(offered_silver);
        if left_credit > 0 {
            let updated =
                sqlx::query("UPDATE characters SET silver = silver + $1 WHERE character_id = $2 AND silver <= $3")
                    .bind(left_credit)
                    .bind(self.database_id())
                    .bind(i64::from(u32::MAX) - left_credit)
                    .execute(&mut *transaction)
                    .await?;
            if updated.rows_affected() != 1 {
                return Ok(None);
            }
        }
        if right_credit > 0 {
            let updated =
                sqlx::query("UPDATE characters SET silver = silver + $1 WHERE character_id = $2 AND silver <= $3")
                    .bind(right_credit)
                    .bind(other.database_id())
                    .bind(i64::from(u32::MAX) - right_credit)
                    .execute(&mut *transaction)
                    .await?;
            if updated.rows_affected() != 1 {
                return Ok(None);
            }
        }
        for (payer, receiver, amount) in [
            (self.database_id(), other.database_id(), offered_cps),
            (other.database_id(), self.database_id(), requested_cps),
        ] {
            if amount == 0 {
                continue;
            }
            let debited = sqlx::query("UPDATE characters SET cps = cps - $1 WHERE character_id = $2 AND cps >= $3")
                .bind(i64::from(amount))
                .bind(payer)
                .bind(i64::from(amount))
                .execute(&mut *transaction)
                .await?;
            if debited.rows_affected() != 1 {
                return Ok(None);
            }
            sqlx::query("UPDATE characters SET cps = cps + $1 WHERE character_id = $2")
                .bind(i64::from(amount))
                .bind(receiver)
                .execute(&mut *transaction)
                .await?;
        }
        // Delete and reinsert the exchanged inventory rows inside the same
        // transaction. A direct UPDATE can hit the UNIQUE(character_id,
        // position) index while two full inventories exchange their slots;
        // this keeps UID and item_upgrades intact while making the swap
        // independent of update order. Any later error rolls the deletes
        // back with the rest of the transaction.
        for item in left_items.iter().chain(right_items.iter()) {
            let owner_id = if offered_set.contains(&item.uid) {
                self.database_id()
            } else {
                other.database_id()
            };
            let deleted = sqlx::query("DELETE FROM inventory_items WHERE item_uid = $1 AND character_id = $2")
                .bind(i64::from(item.uid))
                .bind(owner_id)
                .execute(&mut *transaction)
                .await?;
            if deleted.rows_affected() != 1 {
                return Ok(None);
            }
        }
        for (item, position) in left_items.iter().zip(right_receive_positions.iter()) {
            sqlx::query(
                "INSERT INTO inventory_items
                    (item_uid, character_id, item_type, position, durability, max_durability)
                 VALUES ($1, $2, $3, $4, $5, $6)",
            )
            .bind(i64::from(item.uid))
            .bind(other.database_id())
            .bind(i64::from(item.item_type))
            .bind(i64::from(*position))
            .bind(i64::from(item.durability))
            .bind(i64::from(item.max_durability))
            .execute(&mut *transaction)
            .await?;
        }
        for (item, position) in right_items.iter().zip(left_receive_positions.iter()) {
            sqlx::query(
                "INSERT INTO inventory_items
                    (item_uid, character_id, item_type, position, durability, max_durability)
                 VALUES ($1, $2, $3, $4, $5, $6)",
            )
            .bind(i64::from(item.uid))
            .bind(self.database_id())
            .bind(i64::from(item.item_type))
            .bind(i64::from(*position))
            .bind(i64::from(item.durability))
            .bind(i64::from(item.max_durability))
            .execute(&mut *transaction)
            .await?;
        }
        transaction.commit().await?;

        let left_received = right_items
            .iter()
            .zip(left_receive_positions.iter())
            .map(|(item, position)| InventoryItem {
                position: *position,
                ..*item
            })
            .collect::<Vec<_>>();
        let right_received = left_items
            .iter()
            .zip(right_receive_positions.iter())
            .map(|(item, position)| InventoryItem {
                position: *position,
                ..*item
            })
            .collect::<Vec<_>>();
        self.inventory.lock().retain(|item| !offered_set.contains(&item.uid));
        self.inventory.lock().extend(left_received.iter().copied());
        self.inventory.lock().sort_by_key(|item| item.position);
        other.inventory.lock().retain(|item| !requested_set.contains(&item.uid));
        other.inventory.lock().extend(right_received.iter().copied());
        other.inventory.lock().sort_by_key(|item| item.position);
        let left_silver = self
            .silver()
            .saturating_sub(u64::from(offered_silver))
            .saturating_add(u64::from(requested_silver));
        let right_silver = other
            .silver()
            .saturating_sub(u64::from(requested_silver))
            .saturating_add(u64::from(offered_silver));
        self.silver.store(left_silver, Ordering::Relaxed);
        other.silver.store(right_silver, Ordering::Relaxed);
        let left_cps = self
            .cps()
            .saturating_sub(u64::from(offered_cps))
            .saturating_add(u64::from(requested_cps));
        let right_cps = other
            .cps()
            .saturating_sub(u64::from(requested_cps))
            .saturating_add(u64::from(offered_cps));
        self.cps.store(left_cps, Ordering::Relaxed);
        other.cps.store(right_cps, Ordering::Relaxed);
        Ok(Some(TradeTransfer {
            left_removed: left_items,
            right_removed: right_items,
            left_received,
            right_received,
            left_silver,
            right_silver,
        }))
    }

    pub async fn load_warehouse(&self, pool: &sqlx::PgPool) -> Result<(), Error> {
        sqlx::query("INSERT INTO warehouse_balances (character_id, silver) VALUES ($1, 0) ON CONFLICT DO NOTHING")
            .bind(self.inner.character_id)
            .execute(pool)
            .await?;
        let balance: i64 = sqlx::query_scalar("SELECT silver FROM warehouse_balances WHERE character_id = $1")
            .bind(self.inner.character_id)
            .fetch_one(pool)
            .await?;
        self.warehouse_silver.store(balance.max(0) as u64, Ordering::Relaxed);
        let rows = sqlx::query_as::<_, (i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64)>(
            "SELECT w.item_uid, w.item_type, w.position, w.durability, w.max_durability,
                    COALESCE(u.plus, 0), COALESCE(u.rarity, 0), COALESCE(u.gem_one, 0), COALESCE(u.gem_two, 0),
                    COALESCE(u.socket_one, 0), COALESCE(u.socket_two, 0), COALESCE(u.enchant, 0)
             FROM warehouse_items w
             LEFT JOIN item_upgrades u ON u.item_uid = w.item_uid
             WHERE w.character_id = $1 ORDER BY w.position",
        )
        .bind(self.inner.character_id)
        .fetch_all(pool)
        .await?;
        *self.warehouse.lock() = rows
            .into_iter()
            .filter_map(
                |(
                    uid,
                    item_type,
                    position,
                    durability,
                    max_durability,
                    plus,
                    rarity,
                    gem_one,
                    gem_two,
                    socket_one,
                    socket_two,
                    rarity_roll,
                )| {
                    Some(InventoryItem {
                        uid: u32::try_from(uid).ok()?,
                        item_type: u32::try_from(item_type).ok()?,
                        position: u8::try_from(position).ok()?,
                        durability: u16::try_from(durability).ok()?,
                        max_durability: u16::try_from(max_durability).ok()?,
                        plus: u8::try_from(plus).ok()?,
                        rarity: u8::try_from(rarity).ok()?,
                        gem_one: u8::try_from(gem_one).ok()?,
                        gem_two: u8::try_from(gem_two).ok()?,
                        socket_one: u8::try_from(socket_one).ok()?,
                        socket_two: u8::try_from(socket_two).ok()?,
                        rarity_roll: u8::try_from(rarity_roll).ok()?,
                        bless: 0,
                        enchant_hp: 0,
                        color: 0,
                    })
                },
            )
            .collect();
        self.apply_blessings(pool).await?;
        Ok(())
    }

    pub fn warehouse_items(&self) -> Vec<InventoryItem> {
        self.warehouse.lock().clone()
    }

    /// Set the custom rarity of an item owned by the character.
    ///
    /// This deliberately does not touch `item_type` (classic quality) or
    /// `plus` (composition/tier). All three progressions remain independent.
    pub async fn set_item_rarity(
        &self,
        pool: &sqlx::PgPool,
        item_uid: u32,
        rarity: u8,
    ) -> Result<Option<InventoryItem>, Error> {
        if rarity > 4 {
            return Err(Error::Other("custom item rarity must be between 0 and 4".into()));
        }
        let Some(mut item) = self.item_in_memory(item_uid) else {
            return Ok(None);
        };
        let mut transaction = pool.begin().await?;
        let owned: bool = sqlx::query_scalar(
            "SELECT EXISTS(
                SELECT 1 FROM inventory_items WHERE item_uid = $1 AND character_id = $2
                UNION ALL
                SELECT 1 FROM equipment_items WHERE item_uid = $3 AND character_id = $4
                UNION ALL
                SELECT 1 FROM warehouse_items WHERE item_uid = $5 AND character_id = $6
             )",
        )
        .bind(i64::from(item_uid))
        .bind(self.inner.character_id)
        .bind(i64::from(item_uid))
        .bind(self.inner.character_id)
        .bind(i64::from(item_uid))
        .bind(self.inner.character_id)
        .fetch_one(&mut *transaction)
        .await?;
        if !owned {
            transaction.rollback().await?;
            return Ok(None);
        }
        sqlx::query(
            "INSERT INTO item_upgrades
                (item_uid, plus, gem_one, gem_two, socket_one, socket_two, rarity)
             VALUES ($1, $2, $3, $4, $5, $6, $7)
             ON CONFLICT(item_uid) DO UPDATE SET
                plus = excluded.plus,
                gem_one = excluded.gem_one,
                gem_two = excluded.gem_two,
                socket_one = excluded.socket_one,
                socket_two = excluded.socket_two,
                rarity = excluded.rarity",
        )
        .bind(i64::from(item_uid))
        .bind(i64::from(item.plus))
        .bind(i64::from(item.gem_one))
        .bind(i64::from(item.gem_two))
        .bind(i64::from(item.socket_one))
        .bind(i64::from(item.socket_two))
        .bind(i64::from(rarity))
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;
        item.rarity = rarity;
        for items in [&self.inventory, &self.equipment, &self.warehouse] {
            if let Some(existing) = items.lock().iter_mut().find(|existing| existing.uid == item_uid) {
                existing.rarity = rarity;
            }
        }
        Ok(Some(item))
    }

    /// Attempts to advance the custom rarity by one tier. DragonBalls are
    /// consumed for every attempt, including a failed roll.
    pub async fn upgrade_item_rarity_with_dragonballs(
        &self,
        pool: &sqlx::PgPool,
        item_uid: u32,
    ) -> Result<(RarityUpgradeOutcome, Vec<InventoryItem>), Error> {
        let Some(mut item) = self.item_in_memory(item_uid) else {
            return Ok((RarityUpgradeOutcome::NotFound, Vec::new()));
        };
        let Some((dragon_ball_cost, chance)) = rarity_upgrade_cost_and_chance(item.rarity) else {
            return Ok((RarityUpgradeOutcome::MaxRarity(item), Vec::new()));
        };
        let mut transaction = pool.begin().await?;
        let owned: bool = sqlx::query_scalar(
            "SELECT EXISTS(
                SELECT 1 FROM inventory_items WHERE item_uid = $1 AND character_id = $2
                UNION ALL
                SELECT 1 FROM equipment_items WHERE item_uid = $3 AND character_id = $4
             )",
        )
        .bind(i64::from(item_uid))
        .bind(self.inner.character_id)
        .bind(i64::from(item_uid))
        .bind(self.inner.character_id)
        .fetch_one(&mut *transaction)
        .await?;
        if !owned {
            transaction.rollback().await?;
            return Ok((RarityUpgradeOutcome::NotFound, Vec::new()));
        }
        let catalysts = sqlx::query_as::<_, (i64, i64, i64, i64, i64)>(
            "SELECT item_uid, item_type, position, durability, max_durability
             FROM inventory_items
             WHERE character_id = $1 AND item_type = 1088000 AND position < 40
             ORDER BY position LIMIT $2",
        )
        .bind(self.inner.character_id)
        .bind(i64::from(dragon_ball_cost))
        .fetch_all(&mut *transaction)
        .await?;
        if catalysts.len() < usize::from(dragon_ball_cost) {
            transaction.rollback().await?;
            return Ok((RarityUpgradeOutcome::MissingDragonBall, Vec::new()));
        }
        let chance = crate::lucky::lucky_chance(u32::from(chance), self.lucky_time() > 0);
        let upgraded = next_roll() % 100 < chance;
        if upgraded {
            let rarity_roll = u8::try_from((next_roll() % 255) + 1).unwrap_or(1);
            sqlx::query(
                "INSERT INTO item_upgrades
                    (item_uid, plus, gem_one, gem_two, socket_one, socket_two, rarity, enchant)
                 VALUES ($1, $2, $3, $4, $5, $6, 0, $7)
                 ON CONFLICT(item_uid) DO UPDATE SET rarity = excluded.rarity, enchant = excluded.enchant",
            )
            .bind(i64::from(item_uid))
            .bind(i64::from(item.plus))
            .bind(i64::from(item.gem_one))
            .bind(i64::from(item.gem_two))
            .bind(i64::from(item.socket_one))
            .bind(i64::from(item.socket_two))
            .bind(i64::from(rarity_roll))
            .execute(&mut *transaction)
            .await?;
            sqlx::query("UPDATE item_upgrades SET rarity = rarity + 1, enchant = $1 WHERE item_uid = $2")
                .bind(i64::from(rarity_roll))
                .bind(i64::from(item_uid))
                .execute(&mut *transaction)
                .await?;
        }
        for catalyst in &catalysts {
            sqlx::query("DELETE FROM inventory_items WHERE item_uid = $1 AND character_id = $2")
                .bind(catalyst.0)
                .bind(self.inner.character_id)
                .execute(&mut *transaction)
                .await?;
        }
        transaction.commit().await?;

        if upgraded {
            item.rarity = item.rarity.saturating_add(1).min(4);
            item.rarity_roll = u8::try_from(
                sqlx::query_scalar::<_, i64>("SELECT enchant FROM item_upgrades WHERE item_uid = $1")
                    .bind(i64::from(item_uid))
                    .fetch_one(pool)
                    .await?,
            )
            .unwrap_or(item.rarity_roll);
            for items in [&self.inventory, &self.equipment, &self.warehouse] {
                if let Some(existing) = items.lock().iter_mut().find(|existing| existing.uid == item_uid) {
                    existing.rarity = item.rarity;
                }
            }
        }
        let catalysts: Vec<InventoryItem> = catalysts
            .into_iter()
            .map(|catalyst| {
                Ok(InventoryItem {
                    uid: u32::try_from(catalyst.0)
                        .map_err(|_| Error::Other("item ID exceeds protocol range".into()))?,
                    item_type: u32::try_from(catalyst.1).unwrap_or(1_088_000),
                    position: u8::try_from(catalyst.2).unwrap_or_default(),
                    durability: u16::try_from(catalyst.3).unwrap_or_default(),
                    max_durability: u16::try_from(catalyst.4).unwrap_or_default(),
                    plus: 0,
                    ..Default::default()
                })
            })
            .collect::<Result<_, Error>>()?;
        self.inventory
            .lock()
            .retain(|owned| !catalysts.iter().any(|catalyst| catalyst.uid == owned.uid));
        Ok((
            if upgraded {
                RarityUpgradeOutcome::Upgraded(item)
            } else {
                RarityUpgradeOutcome::Failed(item)
            },
            catalysts,
        ))
    }

    /// Rerolls the custom rarity bonus seed. One DragonBall is consumed for
    /// every reroll, while the item's rarity and classic quality remain
    /// unchanged.
    pub async fn reroll_item_rarity_bonus_with_dragonball(
        &self,
        pool: &sqlx::PgPool,
        item_uid: u32,
    ) -> Result<(RarityBonusOutcome, Option<InventoryItem>), Error> {
        let Some(mut item) = self.item_in_memory(item_uid) else {
            return Ok((RarityBonusOutcome::NotFound, None));
        };
        if item.rarity == 0 {
            return Ok((RarityBonusOutcome::NoRarity, None));
        }
        let mut transaction = pool.begin().await?;
        let Some(catalyst) = sqlx::query_as::<_, (i64, i64, i64, i64, i64)>(
            "SELECT item_uid, item_type, position, durability, max_durability
             FROM inventory_items
             WHERE character_id = $1 AND item_type = 1088000 AND position < 40
             ORDER BY position LIMIT 1",
        )
        .bind(self.inner.character_id)
        .fetch_optional(&mut *transaction)
        .await?
        else {
            transaction.rollback().await?;
            return Ok((RarityBonusOutcome::MissingDragonBall, None));
        };
        let rarity_roll = u8::try_from((next_roll() % 255) + 1).unwrap_or(1);
        sqlx::query(
            "INSERT INTO item_upgrades
                (item_uid, plus, gem_one, gem_two, socket_one, socket_two, rarity, enchant)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
             ON CONFLICT(item_uid) DO UPDATE SET enchant = excluded.enchant",
        )
        .bind(i64::from(item_uid))
        .bind(i64::from(item.plus))
        .bind(i64::from(item.gem_one))
        .bind(i64::from(item.gem_two))
        .bind(i64::from(item.socket_one))
        .bind(i64::from(item.socket_two))
        .bind(i64::from(item.rarity))
        .bind(i64::from(rarity_roll))
        .execute(&mut *transaction)
        .await?;
        sqlx::query("DELETE FROM inventory_items WHERE item_uid = $1 AND character_id = $2")
            .bind(catalyst.0)
            .bind(self.inner.character_id)
            .execute(&mut *transaction)
            .await?;
        transaction.commit().await?;

        item.rarity_roll = rarity_roll;
        for items in [&self.inventory, &self.equipment, &self.warehouse] {
            if let Some(existing) = items.lock().iter_mut().find(|existing| existing.uid == item_uid) {
                existing.rarity_roll = rarity_roll;
            }
        }
        let catalyst = InventoryItem {
            uid: u32::try_from(catalyst.0).map_err(|_| Error::Other("item ID exceeds protocol range".into()))?,
            item_type: u32::try_from(catalyst.1).unwrap_or(1_088_000),
            position: u8::try_from(catalyst.2).unwrap_or_default(),
            durability: u16::try_from(catalyst.3).unwrap_or_default(),
            max_durability: u16::try_from(catalyst.4).unwrap_or_default(),
            ..Default::default()
        };
        self.inventory.lock().retain(|owned| owned.uid != catalyst.uid);
        Ok((RarityBonusOutcome::Rerolled(item), Some(catalyst)))
    }

    pub fn warehouse_silver(&self) -> u32 {
        self.warehouse_silver.load(Ordering::Relaxed).min(u64::from(u32::MAX)) as u32
    }

    pub async fn transfer_warehouse_silver(
        &self,
        pool: &sqlx::PgPool,
        amount: u32,
        deposit: bool,
    ) -> Result<Option<u32>, Error> {
        if amount == 0 {
            return Ok(None);
        }
        let mut transaction = pool.begin().await?;
        let query = if deposit {
            "UPDATE characters SET silver = silver - $1 WHERE character_id = $2 AND silver >= $3 RETURNING silver"
        } else {
            "UPDATE warehouse_balances SET silver = silver - $1 WHERE character_id = $2 AND silver >= $3 RETURNING silver"
        };
        let debit = sqlx::query_scalar::<_, i64>(query)
            .bind(i64::from(amount))
            .bind(self.inner.character_id)
            .bind(i64::from(amount))
            .fetch_optional(&mut *transaction)
            .await?;
        let Some(new_debit_balance) = debit else {
            return Ok(None);
        };
        let query = if deposit {
            "UPDATE warehouse_balances SET silver = silver + $1 WHERE character_id = $2 AND silver <= $3 RETURNING silver"
        } else {
            "UPDATE characters SET silver = silver + $1 WHERE character_id = $2 AND silver <= $3 RETURNING silver"
        };
        let credit = sqlx::query_scalar::<_, i64>(query)
            .bind(i64::from(amount))
            .bind(self.inner.character_id)
            .bind(i64::from(u32::MAX) - i64::from(amount))
            .fetch_optional(&mut *transaction)
            .await?;
        let Some(new_credit_balance) = credit else {
            return Ok(None);
        };
        transaction.commit().await?;
        if deposit {
            self.silver.store(new_debit_balance.max(0) as u64, Ordering::Relaxed);
            self.warehouse_silver
                .store(new_credit_balance.max(0) as u64, Ordering::Relaxed);
        } else {
            self.warehouse_silver
                .store(new_debit_balance.max(0) as u64, Ordering::Relaxed);
            self.silver.store(new_credit_balance.max(0) as u64, Ordering::Relaxed);
        }
        Ok(Some(self.warehouse_silver()))
    }

    pub async fn store_inventory_item(
        &self,
        pool: &sqlx::PgPool,
        item_uid: u32,
        position: u8,
    ) -> Result<Option<(InventoryItem, InventoryItem)>, Error> {
        if position >= 40 {
            return Ok(None);
        }
        let source_upgrade = self.item_in_memory(item_uid).unwrap_or_default();
        let mut transaction = pool.begin().await?;
        let occupied: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM warehouse_items WHERE character_id = $1 AND position = $2)")
                .bind(self.inner.character_id)
                .bind(i64::from(position))
                .fetch_one(&mut *transaction)
                .await?;
        if occupied {
            return Ok(None);
        }
        let row = sqlx::query_as::<_, (i64, i64, i64, i64, i64)>(
            "DELETE FROM inventory_items WHERE item_uid = $1 AND character_id = $2 AND position < 40 \
             RETURNING item_uid, item_type, position, durability, max_durability",
        )
        .bind(i64::from(item_uid))
        .bind(self.inner.character_id)
        .fetch_optional(&mut *transaction)
        .await?;
        let Some((uid, item_type, old_position, durability, max_durability)) = row else {
            return Ok(None);
        };
        let warehouse_uid: i64 = sqlx::query_scalar(
            "INSERT INTO warehouse_items (character_id, item_type, position, durability, max_durability) \
             VALUES ($1, $2, $3, $4, $5) RETURNING item_uid",
        )
        .bind(self.inner.character_id)
        .bind(item_type)
        .bind(i64::from(position))
        .bind(durability)
        .bind(max_durability)
        .fetch_one(&mut *transaction)
        .await?;
        if source_upgrade.plus > 0
            || source_upgrade.gem_one > 0
            || source_upgrade.gem_two > 0
            || source_upgrade.socket_one > 0
            || source_upgrade.socket_two > 0
            || source_upgrade.rarity > 0
        {
            sqlx::query(
                "INSERT INTO item_upgrades (item_uid, plus, gem_one, gem_two, socket_one, socket_two, rarity)
                 VALUES ($1, $2, $3, $4, $5, $6, $7)
                 ON CONFLICT(item_uid) DO UPDATE SET plus = excluded.plus, gem_one = excluded.gem_one,
                   gem_two = excluded.gem_two, socket_one = excluded.socket_one, socket_two = excluded.socket_two,
                   rarity = excluded.rarity",
            )
            .bind(warehouse_uid)
            .bind(i64::from(source_upgrade.plus))
            .bind(i64::from(source_upgrade.gem_one))
            .bind(i64::from(source_upgrade.gem_two))
            .bind(i64::from(source_upgrade.socket_one))
            .bind(i64::from(source_upgrade.socket_two))
            .bind(i64::from(source_upgrade.rarity))
            .execute(&mut *transaction)
            .await?;
        }
        sqlx::query("DELETE FROM item_upgrades WHERE item_uid = $1")
            .bind(i64::from(item_uid))
            .execute(&mut *transaction)
            .await?;
        transaction.commit().await?;
        let removed = InventoryItem {
            uid: u32::try_from(uid).map_err(|_| Error::Other("item ID exceeds protocol range".into()))?,
            item_type: u32::try_from(item_type).map_err(|_| Error::Other("item type exceeds protocol range".into()))?,
            position: u8::try_from(old_position)
                .map_err(|_| Error::Other("item position exceeds protocol range".into()))?,
            durability: u16::try_from(durability)
                .map_err(|_| Error::Other("item durability exceeds protocol range".into()))?,
            max_durability: u16::try_from(max_durability)
                .map_err(|_| Error::Other("item durability exceeds protocol range".into()))?,
            ..source_upgrade
        };
        let stored = InventoryItem {
            uid: u32::try_from(warehouse_uid)
                .map_err(|_| Error::Other("warehouse item ID exceeds protocol range".into()))?,
            position,
            ..removed
        };
        self.inventory.lock().retain(|item| item.uid != removed.uid);
        self.warehouse.lock().push(stored);
        Ok(Some((removed, stored)))
    }

    pub async fn withdraw_warehouse_item(
        &self,
        pool: &sqlx::PgPool,
        item_uid: u32,
        position: u8,
    ) -> Result<Option<(InventoryItem, InventoryItem)>, Error> {
        if position >= 40 {
            return Ok(None);
        }
        let source_upgrade = self.item_in_memory(item_uid).unwrap_or_default();
        let mut transaction = pool.begin().await?;
        let occupied: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM inventory_items WHERE character_id = $1 AND position = $2)")
                .bind(self.inner.character_id)
                .bind(i64::from(position))
                .fetch_one(&mut *transaction)
                .await?;
        let inventory_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM inventory_items WHERE character_id = $1 AND position < 40")
                .bind(self.inner.character_id)
                .fetch_one(&mut *transaction)
                .await?;
        if occupied || inventory_count >= 40 {
            return Ok(None);
        }
        let row = sqlx::query_as::<_, (i64, i64, i64, i64, i64)>(
            "DELETE FROM warehouse_items WHERE item_uid = $1 AND character_id = $2 \
             RETURNING item_uid, item_type, position, durability, max_durability",
        )
        .bind(i64::from(item_uid))
        .bind(self.inner.character_id)
        .fetch_optional(&mut *transaction)
        .await?;
        let Some((uid, item_type, old_position, durability, max_durability)) = row else {
            return Ok(None);
        };
        let inventory_uid: i64 = sqlx::query_scalar(
            "INSERT INTO inventory_items (character_id, item_type, position, durability, max_durability) \
             VALUES ($1, $2, $3, $4, $5) RETURNING item_uid",
        )
        .bind(self.inner.character_id)
        .bind(item_type)
        .bind(i64::from(position))
        .bind(durability)
        .bind(max_durability)
        .fetch_one(&mut *transaction)
        .await?;
        if source_upgrade.plus > 0
            || source_upgrade.gem_one > 0
            || source_upgrade.gem_two > 0
            || source_upgrade.socket_one > 0
            || source_upgrade.socket_two > 0
            || source_upgrade.rarity > 0
        {
            sqlx::query(
                "INSERT INTO item_upgrades (item_uid, plus, gem_one, gem_two, socket_one, socket_two, rarity)
                 VALUES ($1, $2, $3, $4, $5, $6, $7)
                 ON CONFLICT(item_uid) DO UPDATE SET plus = excluded.plus, gem_one = excluded.gem_one,
                   gem_two = excluded.gem_two, socket_one = excluded.socket_one, socket_two = excluded.socket_two,
                   rarity = excluded.rarity",
            )
            .bind(inventory_uid)
            .bind(i64::from(source_upgrade.plus))
            .bind(i64::from(source_upgrade.gem_one))
            .bind(i64::from(source_upgrade.gem_two))
            .bind(i64::from(source_upgrade.socket_one))
            .bind(i64::from(source_upgrade.socket_two))
            .bind(i64::from(source_upgrade.rarity))
            .execute(&mut *transaction)
            .await?;
        }
        sqlx::query("DELETE FROM item_upgrades WHERE item_uid = $1")
            .bind(i64::from(item_uid))
            .execute(&mut *transaction)
            .await?;
        transaction.commit().await?;
        let stored = InventoryItem {
            uid: u32::try_from(uid).map_err(|_| Error::Other("warehouse item ID exceeds protocol range".into()))?,
            item_type: u32::try_from(item_type).map_err(|_| Error::Other("item type exceeds protocol range".into()))?,
            position: u8::try_from(old_position)
                .map_err(|_| Error::Other("warehouse position exceeds protocol range".into()))?,
            durability: u16::try_from(durability)
                .map_err(|_| Error::Other("item durability exceeds protocol range".into()))?,
            max_durability: u16::try_from(max_durability)
                .map_err(|_| Error::Other("item durability exceeds protocol range".into()))?,
            ..source_upgrade
        };
        let withdrawn = InventoryItem {
            uid: u32::try_from(inventory_uid).map_err(|_| Error::Other("item ID exceeds protocol range".into()))?,
            position,
            ..stored
        };
        self.warehouse.lock().retain(|item| item.uid != stored.uid);
        self.inventory.lock().push(withdrawn);
        Ok(Some((stored, withdrawn)))
    }

    /// Move one owned inventory item into the persistent guild warehouse.
    /// The UID and its upgrade row are preserved, so the item cannot be
    /// duplicated or lose plus/gem/socket data while crossing the boundary.
    pub async fn store_guild_warehouse_item(
        &self,
        pool: &sqlx::PgPool,
        guild_id: u32,
        item_uid: u32,
        position: u8,
    ) -> Result<Option<(InventoryItem, InventoryItem)>, Error> {
        if position >= 40 {
            return Ok(None);
        }
        let source_upgrade = self.item_in_memory(item_uid).unwrap_or_default();
        let mut transaction = pool.begin().await?;
        let occupied: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM guild_warehouse_items WHERE guild_id = $1 AND position = $2)",
        )
        .bind(i64::from(guild_id))
        .bind(i64::from(position))
        .fetch_one(&mut *transaction)
        .await?;
        if occupied {
            return Ok(None);
        }
        let row = sqlx::query_as::<_, (i64, i64, i64, i64, i64)>(
            "DELETE FROM inventory_items WHERE item_uid = $1 AND character_id = $2 AND position < 40
             RETURNING item_uid, item_type, position, durability, max_durability",
        )
        .bind(i64::from(item_uid))
        .bind(self.inner.character_id)
        .fetch_optional(&mut *transaction)
        .await?;
        let Some((uid, item_type, old_position, durability, max_durability)) = row else {
            return Ok(None);
        };
        // Another member may fill the slot between the check above and
        // here; then nothing is stored and the item stays in the bag.
        let stored = sqlx::query(
            "INSERT INTO guild_warehouse_items
                (item_uid, guild_id, item_type, position, durability, max_durability)
             VALUES ($1, $2, $3, $4, $5, $6)
             ON CONFLICT (guild_id, position) DO NOTHING",
        )
        .bind(uid)
        .bind(i64::from(guild_id))
        .bind(item_type)
        .bind(i64::from(position))
        .bind(durability)
        .bind(max_durability)
        .execute(&mut *transaction)
        .await?;
        if stored.rows_affected() != 1 {
            return Ok(None);
        }
        transaction.commit().await?;
        let removed = InventoryItem {
            uid: u32::try_from(uid).map_err(|_| Error::Other("item ID exceeds protocol range".into()))?,
            item_type: u32::try_from(item_type).map_err(|_| Error::Other("item type exceeds protocol range".into()))?,
            position: u8::try_from(old_position)
                .map_err(|_| Error::Other("item position exceeds protocol range".into()))?,
            durability: u16::try_from(durability)
                .map_err(|_| Error::Other("item durability exceeds protocol range".into()))?,
            max_durability: u16::try_from(max_durability)
                .map_err(|_| Error::Other("item durability exceeds protocol range".into()))?,
            ..source_upgrade
        };
        let stored = InventoryItem { position, ..removed };
        self.inventory.lock().retain(|item| item.uid != removed.uid);
        Ok(Some((removed, stored)))
    }

    /// Load Guild warehouse items with the same UID upgrade state used by the
    /// inventory/equipment loaders. Slots are the Guild-local 0..39 values.
    pub async fn guild_warehouse_items(
        &self,
        pool: &sqlx::PgPool,
        guild_id: u32,
    ) -> Result<Vec<InventoryItem>, Error> {
        let rows = sqlx::query_as::<_, (i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64)>(
            "SELECT g.item_uid, g.item_type, g.position, g.durability, g.max_durability,
                    COALESCE(u.plus, 0), COALESCE(u.rarity, 0), COALESCE(u.gem_one, 0), COALESCE(u.gem_two, 0),
                    COALESCE(u.socket_one, 0), COALESCE(u.socket_two, 0), COALESCE(u.enchant, 0)
             FROM guild_warehouse_items g
             LEFT JOIN item_upgrades u ON u.item_uid = g.item_uid
             WHERE g.guild_id = $1 ORDER BY g.position",
        )
        .bind(i64::from(guild_id))
        .fetch_all(pool)
        .await?;
        rows.into_iter()
            .map(
                |(
                    uid,
                    item_type,
                    position,
                    durability,
                    max_durability,
                    plus,
                    rarity,
                    gem_one,
                    gem_two,
                    socket_one,
                    socket_two,
                    rarity_roll,
                )| {
                    Ok(InventoryItem {
                        uid: u32::try_from(uid).map_err(|_| Error::Other("item ID exceeds protocol range".into()))?,
                        item_type: u32::try_from(item_type)
                            .map_err(|_| Error::Other("item type exceeds protocol range".into()))?,
                        position: u8::try_from(position)
                            .map_err(|_| Error::Other("item position exceeds protocol range".into()))?,
                        durability: u16::try_from(durability).unwrap_or_default(),
                        max_durability: u16::try_from(max_durability).unwrap_or_default(),
                        plus: u8::try_from(plus).unwrap_or_default(),
                        rarity: u8::try_from(rarity).unwrap_or_default(),
                        gem_one: u8::try_from(gem_one).unwrap_or_default(),
                        gem_two: u8::try_from(gem_two).unwrap_or_default(),
                        socket_one: u8::try_from(socket_one).unwrap_or_default(),
                        socket_two: u8::try_from(socket_two).unwrap_or_default(),
                        rarity_roll: u8::try_from(rarity_roll).unwrap_or_default(),
                        bless: 0,
                        enchant_hp: 0,
                        color: 0,
                    })
                },
            )
            .collect()
    }

    /// Withdraw one item from a guild warehouse into a free inventory slot.
    /// The operation is atomic and keeps the same UID and item upgrades.
    pub async fn withdraw_guild_warehouse_item(
        &self,
        pool: &sqlx::PgPool,
        guild_id: u32,
        item_uid: u32,
        position: u8,
    ) -> Result<Option<(InventoryItem, InventoryItem)>, Error> {
        if position >= 40 {
            return Ok(None);
        }
        let mut transaction = pool.begin().await?;
        let occupied: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM inventory_items WHERE character_id = $1 AND position = $2)")
                .bind(self.inner.character_id)
                .bind(i64::from(position))
                .fetch_one(&mut *transaction)
                .await?;
        let inventory_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM inventory_items WHERE character_id = $1 AND position < 40")
                .bind(self.inner.character_id)
                .fetch_one(&mut *transaction)
                .await?;
        if occupied || inventory_count >= 40 {
            return Ok(None);
        }
        let row = sqlx::query_as::<_, (i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64)>(
            "SELECT g.item_uid, g.item_type, g.position, g.durability, g.max_durability,
                    COALESCE(u.plus, 0), COALESCE(u.rarity, 0), COALESCE(u.gem_one, 0), COALESCE(u.gem_two, 0),
                    COALESCE(u.socket_one, 0), COALESCE(u.socket_two, 0), COALESCE(u.enchant, 0)
             FROM guild_warehouse_items g
             LEFT JOIN item_upgrades u ON u.item_uid = g.item_uid
             WHERE g.guild_id = $1 AND g.item_uid = $2",
        )
        .bind(i64::from(guild_id))
        .bind(i64::from(item_uid))
        .fetch_optional(&mut *transaction)
        .await?;
        let Some((
            uid,
            item_type,
            old_position,
            durability,
            max_durability,
            plus,
            rarity,
            gem_one,
            gem_two,
            socket_one,
            socket_two,
            rarity_roll,
        )) = row
        else {
            return Ok(None);
        };
        // Members share the warehouse: when two take the same item at once,
        // only the one whose delete removes the row gets it.
        let taken = sqlx::query("DELETE FROM guild_warehouse_items WHERE guild_id = $1 AND item_uid = $2")
            .bind(i64::from(guild_id))
            .bind(uid)
            .execute(&mut *transaction)
            .await?;
        if taken.rows_affected() != 1 {
            return Ok(None);
        }
        sqlx::query(
            "INSERT INTO inventory_items
                (item_uid, character_id, item_type, position, durability, max_durability)
             VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(uid)
        .bind(self.inner.character_id)
        .bind(item_type)
        .bind(i64::from(position))
        .bind(durability)
        .bind(max_durability)
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;
        let stored = InventoryItem {
            uid: u32::try_from(uid).map_err(|_| Error::Other("item ID exceeds protocol range".into()))?,
            item_type: u32::try_from(item_type).map_err(|_| Error::Other("item type exceeds protocol range".into()))?,
            position: u8::try_from(old_position)
                .map_err(|_| Error::Other("item position exceeds protocol range".into()))?,
            durability: u16::try_from(durability)
                .map_err(|_| Error::Other("item durability exceeds protocol range".into()))?,
            max_durability: u16::try_from(max_durability)
                .map_err(|_| Error::Other("item durability exceeds protocol range".into()))?,
            plus: u8::try_from(plus).unwrap_or_default(),
            rarity: u8::try_from(rarity).unwrap_or_default(),
            gem_one: u8::try_from(gem_one).unwrap_or_default(),
            gem_two: u8::try_from(gem_two).unwrap_or_default(),
            socket_one: u8::try_from(socket_one).unwrap_or_default(),
            socket_two: u8::try_from(socket_two).unwrap_or_default(),
            rarity_roll: u8::try_from(rarity_roll).unwrap_or_default(),
            bless: 0,
            enchant_hp: 0,
            color: 0,
        };
        let withdrawn = InventoryItem { position, ..stored };
        self.inventory.lock().push(withdrawn);
        Ok(Some((stored, withdrawn)))
    }

    pub async fn sell_item(&self, pool: &sqlx::PgPool, item_uid: u32, silver: u32) -> Result<SaleOutcome, Error> {
        let mut transaction = pool.begin().await?;
        let item = sqlx::query_as::<_, (i64, i64, i64, i64, i64)>(
            "DELETE FROM inventory_items WHERE item_uid = $1 AND character_id = $2 AND position < 40 \
             RETURNING item_uid, item_type, position, durability, max_durability",
        )
        .bind(i64::from(item_uid))
        .bind(self.inner.character_id)
        .fetch_optional(&mut *transaction)
        .await?;
        let Some((uid, item_type, position, durability, max_durability)) = item else {
            return Ok(SaleOutcome::ItemNotFound);
        };
        if silver == 0 {
            return Ok(SaleOutcome::NoValue);
        }
        let balance_limit = i64::from(u32::MAX - silver);
        let updated = sqlx::query(
            "UPDATE characters SET silver = silver + $1 \
             WHERE character_id = $2 AND silver <= $3",
        )
        .bind(i64::from(silver))
        .bind(self.inner.character_id)
        .bind(balance_limit)
        .execute(&mut *transaction)
        .await?;
        if updated.rows_affected() == 0 {
            return Ok(SaleOutcome::SilverLimit);
        }
        sqlx::query("DELETE FROM item_upgrades WHERE item_uid = $1")
            .bind(i64::from(item_uid))
            .execute(&mut *transaction)
            .await?;
        transaction.commit().await?;

        let item = InventoryItem {
            uid: u32::try_from(uid).map_err(|_| Error::Other("item ID exceeds protocol range".into()))?,
            item_type: u32::try_from(item_type).map_err(|_| Error::Other("item type exceeds protocol range".into()))?,
            position: u8::try_from(position)
                .map_err(|_| Error::Other("item position exceeds protocol range".into()))?,
            durability: u16::try_from(durability)
                .map_err(|_| Error::Other("item durability exceeds protocol range".into()))?,
            max_durability: u16::try_from(max_durability)
                .map_err(|_| Error::Other("item durability exceeds protocol range".into()))?,
            plus: 0,
            ..Default::default()
        };
        self.silver.fetch_add(u64::from(silver), Ordering::Relaxed);
        self.inventory.lock().retain(|owned| owned.uid != item.uid);
        Ok(SaleOutcome::Sold { item, silver })
    }

    pub async fn drop_inventory_item(
        &self,
        pool: &sqlx::PgPool,
        item_uid: u32,
    ) -> Result<Option<InventoryItem>, Error> {
        let row = sqlx::query_as::<_, (i64, i64, i64, i64, i64)>(
            "DELETE FROM inventory_items WHERE item_uid = $1 AND character_id = $2 AND position < 40 \
             RETURNING item_uid, item_type, position, durability, max_durability",
        )
        .bind(i64::from(item_uid))
        .bind(self.inner.character_id)
        .fetch_optional(pool)
        .await?;
        let Some((uid, item_type, position, durability, max_durability)) = row else {
            return Ok(None);
        };
        sqlx::query("DELETE FROM item_upgrades WHERE item_uid = $1")
            .bind(uid)
            .execute(pool)
            .await?;
        let item = InventoryItem {
            uid: u32::try_from(uid).map_err(|_| Error::Other("item ID exceeds protocol range".into()))?,
            item_type: u32::try_from(item_type).map_err(|_| Error::Other("item type exceeds protocol range".into()))?,
            position: u8::try_from(position)
                .map_err(|_| Error::Other("item position exceeds protocol range".into()))?,
            durability: u16::try_from(durability)
                .map_err(|_| Error::Other("item durability exceeds protocol range".into()))?,
            max_durability: u16::try_from(max_durability)
                .map_err(|_| Error::Other("item durability exceeds protocol range".into()))?,
            plus: 0,
            ..Default::default()
        };
        self.inventory.lock().retain(|owned| owned.uid != item.uid);
        Ok(Some(item))
    }

    /// Replaces one consumable pack with its direct contents atomically. The
    /// source item is only removed after all destination slots are reserved.
    pub async fn open_consumable_bundle(
        &self,
        pool: &sqlx::PgPool,
        item_uid: u32,
        source_type: u32,
        contents_type: u32,
        amount: u8,
    ) -> Result<Option<(InventoryItem, Vec<InventoryItem>)>, Error> {
        if amount == 0 || usize::from(amount) > 40 {
            return Ok(None);
        }
        let mut transaction = pool.begin().await?;
        let source = sqlx::query_as::<_, (i64, i64, i64, i64, i64)>(
            "SELECT item_uid, item_type, position, durability, max_durability \
             FROM inventory_items WHERE item_uid = $1 AND character_id = $2 AND position < 40",
        )
        .bind(i64::from(item_uid))
        .bind(self.inner.character_id)
        .fetch_optional(&mut *transaction)
        .await?;
        let Some((uid, item_type, position, durability, max_durability)) = source else {
            return Ok(None);
        };
        if u32::try_from(item_type).ok() != Some(source_type) {
            return Ok(None);
        }
        let occupied = sqlx::query_scalar::<_, i64>(
            "SELECT position FROM inventory_items WHERE character_id = $1 AND position < 40",
        )
        .bind(self.inner.character_id)
        .fetch_all(&mut *transaction)
        .await?;
        let free_positions = (0_u8..40)
            .filter(|position| !occupied.iter().any(|occupied| *occupied == i64::from(*position)))
            .take(usize::from(amount))
            .collect::<Vec<_>>();
        if free_positions.len() != usize::from(amount) {
            return Ok(None);
        }
        sqlx::query("DELETE FROM inventory_items WHERE item_uid = $1 AND character_id = $2")
            .bind(i64::from(item_uid))
            .bind(self.inner.character_id)
            .execute(&mut *transaction)
            .await?;
        sqlx::query("DELETE FROM item_upgrades WHERE item_uid = $1")
            .bind(i64::from(item_uid))
            .execute(&mut *transaction)
            .await?;

        let mut created = Vec::with_capacity(free_positions.len());
        for position in free_positions {
            let row = sqlx::query_as::<_, (i64, i64, i64, i64, i64)>(
                "INSERT INTO inventory_items \
                 (character_id, item_type, position, durability, max_durability) \
                 VALUES ($1, $2, $3, $4, $5) \
                 RETURNING item_uid, item_type, position, durability, max_durability",
            )
            .bind(self.inner.character_id)
            .bind(i64::from(contents_type))
            .bind(i64::from(position))
            .bind(durability)
            .bind(max_durability)
            .fetch_one(&mut *transaction)
            .await?;
            created.push(InventoryItem {
                uid: u32::try_from(row.0).map_err(|_| Error::Other("item uid exceeds protocol range".into()))?,
                item_type: u32::try_from(row.1).map_err(|_| Error::Other("item type exceeds protocol range".into()))?,
                position: u8::try_from(row.2)
                    .map_err(|_| Error::Other("item position exceeds protocol range".into()))?,
                durability: u16::try_from(row.3).unwrap_or_default(),
                max_durability: u16::try_from(row.4).unwrap_or_default(),
                plus: 0,
                ..Default::default()
            });
        }
        transaction.commit().await?;

        let consumed = InventoryItem {
            uid: u32::try_from(uid).map_err(|_| Error::Other("item uid exceeds protocol range".into()))?,
            item_type: u32::try_from(item_type).map_err(|_| Error::Other("item type exceeds protocol range".into()))?,
            position: u8::try_from(position)
                .map_err(|_| Error::Other("item position exceeds protocol range".into()))?,
            durability: u16::try_from(durability).unwrap_or_default(),
            max_durability: u16::try_from(max_durability).unwrap_or_default(),
            plus: 0,
            ..Default::default()
        };
        let mut inventory = self.inventory.lock();
        inventory.retain(|owned| owned.uid != consumed.uid);
        inventory.extend(created.iter().copied());
        inventory.sort_by_key(|owned| owned.position);
        Ok(Some((consumed, created)))
    }

    pub async fn purchase_item(
        &self,
        pool: &sqlx::PgPool,
        item_type: u32,
        position: u8,
        price: u32,
        durability: u16,
        max_durability: u16,
    ) -> Result<PurchaseOutcome, Error> {
        if position >= 40 {
            return Ok(PurchaseOutcome::InventoryFull);
        }
        {
            let inventory = self.inventory.lock();
            if inventory.len() >= 40 || inventory.iter().any(|item| item.position == position) {
                return Ok(PurchaseOutcome::InventoryFull);
            }
        }
        let mut transaction = pool.begin().await?;
        let updated = sqlx::query(
            "UPDATE characters SET silver = silver - $1 \
             WHERE character_id = $2 AND silver >= $3",
        )
        .bind(i64::from(price))
        .bind(self.inner.character_id)
        .bind(i64::from(price))
        .execute(&mut *transaction)
        .await?;
        if updated.rows_affected() == 0 {
            transaction.rollback().await?;
            return Ok(PurchaseOutcome::InsufficientSilver);
        }
        let uid: i64 = sqlx::query_scalar(
            "INSERT INTO inventory_items \
             (character_id, item_type, position, durability, max_durability) \
             VALUES ($1, $2, $3, $4, $5) RETURNING item_uid",
        )
        .bind(self.inner.character_id)
        .bind(i64::from(item_type))
        .bind(i64::from(position))
        .bind(i64::from(durability))
        .bind(i64::from(max_durability))
        .fetch_one(&mut *transaction)
        .await?;
        transaction.commit().await?;

        self.silver.fetch_sub(u64::from(price), Ordering::Relaxed);
        let item = InventoryItem {
            uid: u32::try_from(uid).map_err(|_| Error::Other("item ID exceeds protocol range".into()))?,
            item_type,
            position,
            durability,
            max_durability,
            plus: 0,
            ..Default::default()
        };
        self.inventory.lock().push(item);
        Ok(PurchaseOutcome::Bought(item))
    }

    pub async fn add_ground_item(
        &self,
        pool: &sqlx::PgPool,
        item_type: u32,
        durability: u16,
        max_durability: u16,
    ) -> Result<Option<InventoryItem>, Error> {
        let position = {
            let inventory = self.inventory.lock();
            (0_u8..40).find(|position| inventory.iter().all(|item| item.position != *position))
        };
        let Some(position) = position else {
            return Ok(None);
        };
        let uid: i64 = sqlx::query_scalar(
            "INSERT INTO inventory_items \
             (character_id, item_type, position, durability, max_durability) \
             VALUES ($1, $2, $3, $4, $5) RETURNING item_uid",
        )
        .bind(self.inner.character_id)
        .bind(i64::from(item_type))
        .bind(i64::from(position))
        .bind(i64::from(durability))
        .bind(i64::from(max_durability))
        .fetch_one(pool)
        .await?;
        let item = InventoryItem {
            uid: u32::try_from(uid).map_err(|_| Error::Other("item ID exceeds protocol range".into()))?,
            item_type,
            position,
            durability,
            max_durability,
            plus: 0,
            ..Default::default()
        };
        self.inventory.lock().push(item);
        Ok(Some(item))
    }

    pub fn cps(&self) -> u64 {
        self.cps.load(Ordering::Relaxed)
    }

    /// The account the character belongs to.
    pub fn account_id(&self) -> u32 {
        u32::try_from(self.inner.account_id).unwrap_or_default()
    }

    /// Mirrors CPs the database already holds (after an atomic update).
    pub fn set_cps(&self, cps: u64) {
        self.cps.store(cps, Ordering::Relaxed);
    }

    /// Adds Conquer Points and saves them.
    pub async fn grant_cps(&self, pool: &sqlx::PgPool, amount: u64) -> Result<u64, Error> {
        let total = self.cps.fetch_add(amount, Ordering::Relaxed).saturating_add(amount);
        sqlx::query("UPDATE characters SET cps = $1 WHERE character_id = $2")
            .bind(i64::try_from(total).unwrap_or(i64::MAX))
            .bind(self.inner.character_id)
            .execute(pool)
            .await?;
        Ok(total)
    }

    pub fn experience(&self) -> u64 {
        self.experience.load(Ordering::Relaxed)
    }

    pub fn activate_double_experience(&self, duration: Duration) {
        self.activate_experience_boost(2, duration);
    }

    /// Experience times `multiplier` for `duration`, replacing the boost
    /// under way.
    pub fn activate_experience_boost(&self, multiplier: u8, duration: Duration) {
        self.experience_multiplier.store(multiplier.max(2), Ordering::Relaxed);
        *self.double_experience_until.lock() = SystemTime::now().checked_add(duration);
        self.entity
            .set_flags(self.entity.flags() | crate::entities::Flags::DOUBLE_EXPERIENCE);
    }

    pub fn activate_heaven_blessing(&self, duration: Duration) {
        *self.heaven_blessing_until.lock() = SystemTime::now().checked_add(duration);
        self.entity
            .set_flags(self.entity.flags() | crate::entities::Flags::HEAVEN_BLESSING);
    }

    fn clear_expired_double_experience(&self) -> bool {
        let expired = {
            let mut until = self.double_experience_until.lock();
            match *until {
                Some(expires_at) if expires_at <= SystemTime::now() => {
                    *until = None;
                    true
                },
                _ => false,
            }
        };
        if expired {
            self.entity
                .set_flags(self.entity.flags() & !crate::entities::Flags::DOUBLE_EXPERIENCE);
        }
        expired
    }

    pub fn double_experience_active(&self) -> bool {
        self.clear_expired_double_experience();
        self.double_experience_until.lock().is_some()
    }

    /// How many times the experience now: 1 without a boost.
    pub fn experience_multiplier(&self) -> u8 {
        if self.double_experience_active() {
            self.experience_multiplier.load(Ordering::Relaxed)
        } else {
            1
        }
    }

    fn clear_expired_heaven_blessing(&self) -> bool {
        let expired = {
            let mut until = self.heaven_blessing_until.lock();
            match *until {
                Some(expires_at) if expires_at <= SystemTime::now() => {
                    *until = None;
                    true
                },
                _ => false,
            }
        };
        if expired {
            self.entity
                .set_flags(self.entity.flags() & !crate::entities::Flags::HEAVEN_BLESSING);
        }
        expired
    }

    pub fn heaven_blessing_active(&self) -> bool {
        self.clear_expired_heaven_blessing();
        self.heaven_blessing_until.lock().is_some()
    }

    pub fn expire_temporary_effects(&self) -> bool {
        let double_experience_expired = self.clear_expired_double_experience();
        let heaven_blessing_expired = self.clear_expired_heaven_blessing();
        let pk_changed = self.expire_pk_effects();
        double_experience_expired || heaven_blessing_expired || pk_changed
    }

    fn double_experience_until_unix(&self) -> i64 {
        if !self.double_experience_active() {
            return 0;
        }
        self.double_experience_until
            .lock()
            .and_then(|expires_at| expires_at.duration_since(UNIX_EPOCH).ok())
            .map(|duration| duration.as_secs().min(i64::MAX as u64) as i64)
            .unwrap_or_default()
    }

    fn heaven_blessing_until_unix(&self) -> i64 {
        if !self.heaven_blessing_active() {
            return 0;
        }
        self.heaven_blessing_until
            .lock()
            .and_then(|expires_at| expires_at.duration_since(UNIX_EPOCH).ok())
            .map(|duration| duration.as_secs().min(i64::MAX as u64) as i64)
            .unwrap_or_default()
    }

    /// Add combat experience and advance the runtime level when the classic
    /// per-level threshold is reached. Persistence is handled separately by
    /// the character save path; the atomic value keeps combat authoritative.
    pub fn grant_experience(&self, amount: u64) -> Option<u8> {
        // Redux `GainExperience`: double experience doubles it, Heaven's
        // Blessing adds a fifth, and the server rate multiplies it all.
        let mut bonus = f64::from(self.experience_multiplier());
        if self.heaven_blessing_active() {
            bonus += 0.2;
        }
        // Rainbow gems (Redux adds them to the same bonus).
        bonus += f64::from(self.gem_bonus(GemFamily::Rainbow)) / 100.0;
        let amount = (amount as f64 * bonus) as u64 * crate::leveling::EXP_RATE;
        let level = self.entity.level().clamp(1, u16::from(u8::MAX)) as u8;
        let (next_level, experience) = crate::leveling::advance(level, self.experience(), amount);
        self.apply_level(level, next_level, experience)
    }

    /// An ExpBall worth `seconds` of the current level.
    pub fn grant_exp_ball(&self, seconds: f64) -> Option<u8> {
        let level = self.entity.level().clamp(1, u16::from(u8::MAX)) as u8;
        let (next_level, experience) = crate::leveling::exp_ball(level, self.experience(), seconds);
        self.apply_level(level, next_level, experience)
    }

    /// Stores the new experience and, on a level up, the new level.
    /// Persistence is handled separately by the character save path.
    fn apply_level(&self, level: u8, next_level: u8, experience: u64) -> Option<u8> {
        self.experience.store(experience, Ordering::Relaxed);
        if next_level <= level {
            return None;
        }
        self.change_level(u16::from(next_level));
        Some(next_level)
    }

    /// Redux `SetLevel`: before the first rebirth the attributes follow the
    /// class table; reborn characters earn three points per level to allot.
    /// Life and mana are recalculated and refilled.
    pub fn change_level(&self, level: u16) {
        let previous = self.entity.level();
        self.entity.set_level(level);
        if self.rebirths() == 0 {
            if let Some((strength, vitality, agility, spirit)) = crate::stats::for_level(self.current_class(), level) {
                let mut attributes = self.attributes.lock();
                attributes.strength = strength;
                attributes.vitality = vitality;
                attributes.agility = agility;
                attributes.spirit = spirit;
            }
        } else if level > previous {
            let mut attributes = self.attributes.lock();
            attributes.points = attributes.points.saturating_add((level - previous).saturating_mul(3));
        }
        self.recalculate_resources(true);
    }

    /// Life and mana capacities from the attributes and the class (Redux
    /// `Recalculate`), with the equipment bonus on top.
    pub fn recalculate_resources(&self, refill: bool) {
        let class = u16::from(self.current_class());
        let (strength, agility, vitality, spirit) = {
            let attributes = self.attributes.lock();
            (attributes.strength, attributes.agility, attributes.vitality, attributes.spirit)
        };
        let health = crate::entities::basic::health_from_attributes(strength, agility, vitality, spirit, class);
        let mana = crate::entities::basic::max_mana_for(spirit, class);
        self.entity.set_base_resources(health.max(1), mana, refill);
    }

    /// Spend available base-attribute points with one atomic database update.
    /// The runtime mirror is only changed after the database confirms the update.
    pub async fn allocate_attribute(
        &self,
        pool: &sqlx::PgPool,
        kind: AttributeKind,
        amount: u16,
    ) -> Result<AttributeAllocation, Error> {
        if amount == 0 {
            return Ok(AttributeAllocation::Allocated {
                kind,
                amount: 0,
                remaining: self.attribute_points(),
            });
        }
        let column = kind.column();
        let query = format!(
            "UPDATE characters SET {column} = {column} + $1, attribute_points = attribute_points - $2 \
             WHERE character_id = $3 AND attribute_points >= $4 AND {column} + $5 <= 65535 \
             RETURNING {column}, attribute_points"
        );
        let updated = sqlx::query_as::<_, (i64, i64)>(&query)
            .bind(i64::from(amount))
            .bind(i64::from(amount))
            .bind(self.inner.character_id)
            .bind(i64::from(amount))
            .bind(i64::from(amount))
            .fetch_optional(pool)
            .await?;
        let Some((_new_value, remaining)) = updated else {
            return Ok(AttributeAllocation::InsufficientPoints {
                available: self.attribute_points(),
            });
        };

        let remaining = remaining.clamp(0, i64::from(u16::MAX)) as u16;
        let profession = self.current_class() as u16;
        let (health_delta, mana_delta) = {
            let mut attributes = self.attributes.lock();
            let old_derived_health = crate::entities::basic::health_from_attributes(
                attributes.strength,
                attributes.agility,
                attributes.vitality,
                attributes.spirit,
                profession,
            );
            match kind {
                AttributeKind::Strength => attributes.strength = attributes.strength.saturating_add(amount),
                AttributeKind::Agility => attributes.agility = attributes.agility.saturating_add(amount),
                AttributeKind::Vitality => attributes.vitality = attributes.vitality.saturating_add(amount),
                AttributeKind::Spirit => attributes.spirit = attributes.spirit.saturating_add(amount),
            }
            attributes.points = remaining;

            let new_derived_health = crate::entities::basic::health_from_attributes(
                attributes.strength,
                attributes.agility,
                attributes.vitality,
                attributes.spirit,
                profession,
            );
            let health_delta = u32::from(new_derived_health.saturating_sub(old_derived_health));
            let mana_delta = if matches!(kind, AttributeKind::Spirit) {
                u32::from(amount).saturating_mul(u32::from(crate::entities::basic::mana_per_spirit(profession)))
            } else {
                0
            };
            (health_delta, mana_delta)
        };
        self.entity.adjust_base_resource_max(health_delta, mana_delta);

        Ok(AttributeAllocation::Allocated {
            kind,
            amount,
            remaining,
        })
    }

    pub fn strength(&self) -> u16 {
        self.attributes.lock().strength
    }

    pub fn agility(&self) -> u16 {
        self.attributes.lock().agility
    }

    pub fn vitality(&self) -> u16 {
        self.attributes.lock().vitality
    }

    pub fn attack_range(&self) -> u16 {
        self.attack_range_with_catalog(None)
    }

    pub fn attack_range_with_catalog(&self, catalog: Option<&ShopCatalog>) -> u16 {
        if let Some(catalog) = catalog {
            if let Some(weapon) = self
                .equipment_item(4)
                .or_else(|| self.equipment_item(5))
                .filter(|weapon| weapon.durability > 0)
            {
                if let Some(template) = catalog.template(weapon.item_type) {
                    if template.attack_range > 0 {
                        return template.attack_range.max(2);
                    }
                }
            }
        }
        if (40..50).contains(&self.current_class()) {
            14
        } else {
            2
        }
    }

    pub fn attack_speed_with_catalog(&self, catalog: Option<&ShopCatalog>) -> u16 {
        let base_speed = if let Some(catalog) = catalog {
            if let Some(weapon) = self
                .equipment_item(4)
                .or_else(|| self.equipment_item(5))
                .filter(|weapon| weapon.durability > 0)
            {
                if let Some(template) = catalog.template(weapon.item_type) {
                    if template.attack_speed > 0 {
                        template.attack_speed.clamp(250, 3_000)
                    } else {
                        800
                    }
                } else {
                    800
                }
            } else {
                800
            }
        } else {
            800
        };
        if self.has_status(StatusEffect::Cyclone) {
            base_speed.saturating_div(3).max(100)
        } else {
            base_speed
        }
    }

    /// Returns the additional magical attack supplied by usable equipped
    /// items. Spirit and the physical attributes remain handled by the skill
    /// formula; this method only exposes the itemtype magic contribution.
    pub fn magic_attack_bonus_with_catalog(&self, catalog: Option<&ShopCatalog>) -> u32 {
        let Some(catalog) = catalog else {
            return 0;
        };
        self.equipment_items()
            .into_iter()
            .filter(|item| item.durability > 0)
            .filter_map(|item| catalog.template(item.item_type))
            .map(|template| template.magic_attack)
            .fold(0_u32, u32::saturating_add)
    }

    /// Returns the magical defense supplied by usable equipped items. It is
    /// applied only by the magical damage path, leaving physical defense
    /// untouched.
    pub fn magic_defense_with_catalog(&self, catalog: Option<&ShopCatalog>) -> u32 {
        let Some(catalog) = catalog else {
            return 0;
        };
        self.equipment_items()
            .into_iter()
            .filter(|item| item.durability > 0)
            .filter_map(|item| catalog.template(item.item_type))
            .map(|template| template.magic_defense)
            .fold(0_u32, u32::saturating_add)
    }

    /// Derive combat values from this character's own attributes. Keeping
    /// this on the character makes every client/player use the same server
    /// authoritative formula instead of a level-only placeholder.
    pub fn combat_stats(&self) -> CombatStats {
        self.combat_stats_with_catalog(None)
    }

    pub fn combat_stats_with_catalog(&self, catalog: Option<&ShopCatalog>) -> CombatStats {
        let strength = u32::from(self.strength());
        let agility = u32::from(self.agility());
        let vitality = u32::from(self.vitality());
        let (mut attack_percent, damage_reduction_percent, attack_flat, penetration_percent) = self
            .equipment_items()
            .into_iter()
            .filter(|item| item.durability > 0)
            .fold(
                (0_u16, 0_u16, 0_u32, 0_u16),
                |(attack, reduction, flat, penetration), item| {
                    let gem_bonus = |code: u8| match code {
                        11 => (5, 0),
                        12 => (10, 0),
                        13 => (15, 0),
                        71 => (0, 2),
                        72 => (0, 4),
                        73 => (0, 6),
                        _ => (0, 0),
                    };
                    let (first_attack, first_reduction) = gem_bonus(item.gem_one);
                    let (second_attack, second_reduction) = gem_bonus(item.gem_two);
                    let mut next_flat = flat;
                    let mut next_penetration = penetration;
                    if let Some(effect) = self.refinery_effect(item.uid) {
                        match effect.refinery_type {
                            301 => next_flat = next_flat.saturating_add(effect.effect.max(0) as u32),
                            312 => next_penetration = next_penetration.saturating_add(effect.effect.max(0) as u16),
                            313 => {
                                // Block is represented by the existing
                                // defensive percentage
                                // until the dedicated block stat is introduced.
                            },
                            _ => {},
                        }
                    }
                    (
                        attack.saturating_add(first_attack).saturating_add(second_attack),
                        reduction
                            .saturating_add(first_reduction)
                            .saturating_add(second_reduction),
                        next_flat,
                        next_penetration,
                    )
                },
            );
        if self.has_status(StatusEffect::Stigma) {
            attack_percent = attack_percent.saturating_add(20);
        }
        let equipment = self
            .equipment_items()
            .into_iter()
            .filter(|item| item.durability > 0)
            .filter_map(|item| {
                catalog.and_then(|catalog| catalog.template(item.item_type).map(|template| (item, template)))
            })
            .fold(
                (0u32, 0u32, 0u32),
                |(min_attack, max_attack, defense), (item, template)| {
                    let scale = 100_u32
                        .saturating_add(u32::from(item.plus).saturating_mul(10))
                        .saturating_add(item_rarity_bonus_percent(item.rarity));
                    let scaled = |value: u32| value.saturating_mul(scale) / 100;
                    (
                        min_attack.saturating_add(scaled(template.attack_min)),
                        max_attack.saturating_add(scaled(template.attack_max)),
                        defense.saturating_add(scaled(template.defense)),
                    )
                },
            );
        let defense = if self.has_status(StatusEffect::Burning) {
            equipment.2.saturating_add(vitality + agility / 2).saturating_mul(90) / 100
        } else {
            vitality + agility / 2 + equipment.2
        };
        let defense = if self.has_status(StatusEffect::Shield) {
            defense.saturating_mul(120) / 100
        } else {
            defense
        };
        let damage_reduction_percent = damage_reduction_percent.saturating_add(self.bless_reduction_percent());
        CombatStats {
            // As in the original: a point of Strength is a point of
            // attack, the rest is the gear's. Level and Agility add none.
            min_attack: strength + equipment.0,
            max_attack: strength + equipment.1,
            attack_flat,
            defense,
            penetration_percent,
            max_health: primitives::Gauge::max(&self.entity.hp()) as u32,
            attack_percent,
            damage_reduction_percent,
        }
    }

    pub fn equipment_resource_bonuses(&self, catalog: &ShopCatalog) -> (u32, u32) {
        self.equipment_items()
            .into_iter()
            .filter(|item| item.durability > 0)
            .filter_map(|item| catalog.template(item.item_type).map(|template| (item, template)))
            .fold((0u32, 0u32), |(health, mana), (owned, item)| {
                let scale = 100_u32
                    .saturating_add(u32::from(owned.plus).saturating_mul(10))
                    .saturating_add(item_rarity_bonus_percent(owned.rarity));
                let scaled = |value: u16| u32::from(value).saturating_mul(scale) / 100;
                (
                    health
                        .saturating_add(scaled(item.health_add))
                        .saturating_add(u32::from(owned.enchant_hp)),
                    mana.saturating_add(scaled(item.mana_add)),
                )
            })
    }

    pub fn refresh_equipment_resources(&self, catalog: &ShopCatalog) {
        let (health, mana) = self.equipment_resource_bonuses(catalog);
        self.entity.set_equipment_resource_bonuses(health, mana);
    }

    pub fn spirit(&self) -> u16 {
        self.attributes.lock().spirit
    }

    pub fn attribute_points(&self) -> u16 {
        self.attributes.lock().points
    }

    pub fn health_points(&self) -> u16 {
        self.entity.hp().current()
    }

    pub fn maximum_health_points(&self) -> u16 {
        primitives::Gauge::max(&self.entity.hp())
    }

    pub fn mana_points(&self) -> u16 {
        self.entity.mp().current()
    }

    pub fn maximum_mana_points(&self) -> u16 {
        primitives::Gauge::max(&self.entity.mp())
    }

    pub fn kill_points(&self) -> u16 {
        self.kill_points.load(Ordering::Relaxed)
    }

    /// Sets the PK points and the red/black name that goes with them.
    pub fn set_kill_points(&self, points: u16) {
        let previous = self.kill_points.swap(points, Ordering::Relaxed);
        if previous == 0 && points > 0 {
            *self.next_pk_decay.lock() = Some(std::time::Instant::now() + crate::death::PK_DECAY);
        }
        let flags = self.entity.flags() & !(crate::entities::Flags::RED_NAME | crate::entities::Flags::BLACK_NAME);
        self.entity.set_flags(flags | crate::death::name_flags(points));
    }

    /// The blue flashing name of an attacker or a fresh killer, for at least
    /// `duration`; a longer flash already running (a kill's) is kept.
    pub fn flash_name(&self, duration: Duration) {
        let until = std::time::Instant::now() + duration;
        let mut flashing = self.flashing_until.lock();
        *flashing = Some(flashing.map_or(until, |current| current.max(until)));
        self.entity
            .set_flags(self.entity.flags() | crate::entities::Flags::BLUE_FLASHING_NAME);
    }

    pub fn stamina(&self) -> u8 {
        self.stamina.load(Ordering::Relaxed)
    }

    /// Uses `amount` stamina; `false` (and nothing used) when short.
    pub fn spend_stamina(&self, amount: u8) -> bool {
        self.stamina
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |stamina| stamina.checked_sub(amount))
            .is_ok()
    }

    pub fn xp_points(&self) -> u8 {
        self.xp_points.load(Ordering::Relaxed)
    }

    /// A kill feeds the XP circle (Redux `owner.Xp++`).
    /// Testing: stamina set outright.
    pub fn set_stamina(&self, stamina: u8) {
        self.stamina.store(stamina, Ordering::Relaxed);
    }

    /// Testing: the number of rebirths set outright.
    pub fn set_rebirths(&self, rebirths: u8) {
        self.rebirths.store(rebirths.min(2), Ordering::Relaxed);
    }

    pub fn add_xp_point(&self) {
        if self.xp_ready_until.lock().is_none() {
            let _ = self
                .xp_points
                .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |xp| Some(xp.saturating_add(1).min(crate::vitals::MAX_XP)));
            *self.last_xp_up.lock() = std::time::Instant::now();
        }
    }

    /// Roar from a teammate: the XP circle is full and lights up at the
    /// next regeneration step (when the character knows an XP skill).
    pub fn fill_xp(&self) {
        if self.xp_ready_until.lock().is_none() {
            self.xp_points.store(crate::vitals::MAX_XP, Ordering::Relaxed);
        }
    }

    pub fn xp_ready(&self) -> bool {
        self.xp_ready_until.lock().is_some()
    }

    /// Uses the lit XP circle for an XP skill; `false` when it is not lit.
    pub fn use_xp_circle(&self) -> bool {
        if self.xp_ready_until.lock().take().is_none() {
            return false;
        }
        self.reset_xp();
        true
    }

    fn reset_xp(&self) {
        *self.xp_ready_until.lock() = None;
        self.xp_points.store(0, Ordering::Relaxed);
        *self.last_xp_up.lock() = std::time::Instant::now();
        self.entity
            .set_flags(self.entity.flags() & !crate::entities::Flags::XP_CIRCLE);
    }

    /// Whether the character knows any XP skill of `catalog`.
    pub fn has_xp_skill(&self, catalog: &crate::skills::SkillCatalog) -> bool {
        self.skills
            .lock()
            .keys()
            .any(|id| catalog.get(*id).is_some_and(|skill| skill.xp))
    }

    /// The once-a-second regeneration (Redux `On_Player_Timer`): stamina
    /// +3 (+11 sitting), one XP point every 3 seconds, the XP circle at
    /// 100 for 20 seconds. Nothing happens while dead.
    pub fn regenerate(&self, has_xp_skill: bool) -> crate::vitals::Regeneration {
        let now = std::time::Instant::now();
        let mut change = crate::vitals::Regeneration::default();
        {
            let mut last = self.last_regeneration.lock();
            if now.duration_since(*last) < crate::vitals::REGENERATION_PERIOD {
                return change;
            }
            *last = now;
        }
        if self.entity.is_dead() {
            return change;
        }
        let cap = if self.heaven_blessing_active() {
            crate::vitals::BLESSED_MAX_STAMINA
        } else {
            crate::vitals::MAX_STAMINA
        };
        let stamina = self.stamina();
        if stamina < cap {
            let gain = crate::vitals::stamina_gain(self.entity.action());
            self.stamina.store(stamina.saturating_add(gain).min(cap), Ordering::Relaxed);
            change.stamina = true;
        }
        let ready_until = *self.xp_ready_until.lock();
        match ready_until {
            Some(until) if until <= now => {
                self.reset_xp();
                change.flags = true;
            },
            Some(_) => {},
            None => {
                let due = now.duration_since(*self.last_xp_up.lock()) >= crate::vitals::XP_PERIOD;
                if due {
                    let xp = self.xp_points().saturating_add(1).min(crate::vitals::MAX_XP);
                    self.xp_points.store(xp, Ordering::Relaxed);
                    *self.last_xp_up.lock() = now;
                }
                if self.xp_points() >= crate::vitals::MAX_XP && has_xp_skill {
                    *self.xp_ready_until.lock() = Some(now + crate::vitals::XP_CIRCLE_TIME);
                    self.entity
                        .set_flags(self.entity.flags() | crate::entities::Flags::XP_CIRCLE);
                    change.flags = true;
                }
            },
        }
        change
    }

    /// Revival refills stamina and empties the XP circle.
    pub fn refresh_after_revive(&self) {
        self.stamina.store(crate::vitals::MAX_STAMINA, Ordering::Relaxed);
        self.reset_xp();
        *self.protected_until.lock() = Some(std::time::Instant::now() + crate::death::REVIVE_PROTECTION);
    }

    /// Whether the character got up a moment ago and takes no damage yet
    /// (Redux `ReviveProtection`).
    pub fn revive_protected(&self) -> bool {
        self.protected_until
            .lock()
            .is_some_and(|until| std::time::Instant::now() < until)
    }

    /// Attacking gives the protection up.
    pub fn end_revive_protection(&self) {
        *self.protected_until.lock() = None;
    }

    /// Records the death; `false` when it was already recorded.
    pub fn mark_dead(&self) -> bool {
        let mut died_at = self.died_at.lock();
        if died_at.is_some() {
            return false;
        }
        *died_at = Some(std::time::Instant::now());
        true
    }

    /// Whether the dead character may press Revive already.
    pub fn can_revive(&self) -> bool {
        self.died_at
            .lock()
            .map_or(true, |died_at| died_at.elapsed() >= crate::death::REVIVE_DELAY)
    }

    pub fn clear_death(&self) {
        *self.died_at.lock() = None;
    }

    /// Ends the blue name and wears off one PK point every six minutes.
    fn expire_pk_effects(&self) -> bool {
        let now = std::time::Instant::now();
        let mut changed = false;
        {
            let mut flashing = self.flashing_until.lock();
            if flashing.is_some_and(|until| until <= now) {
                *flashing = None;
                self.entity
                    .set_flags(self.entity.flags() & !crate::entities::Flags::BLUE_FLASHING_NAME);
                changed = true;
            }
        }
        let points = self.kill_points();
        let decay_due = {
            let mut next = self.next_pk_decay.lock();
            match *next {
                Some(at) if points > 0 && at <= now => {
                    *next = (points > 1).then(|| now + crate::death::PK_DECAY);
                    true
                },
                _ => false,
            }
        };
        if decay_due {
            self.set_kill_points(points - 1);
            changed = true;
        }
        changed
    }

    pub fn current_class(&self) -> u8 {
        self.class.load(Ordering::Relaxed)
    }

    /// Promotes the character to `class` (for example 11 Trojan or 142 Fire
    /// Taoist) and persists it.
    pub async fn set_class(&self, pool: &sqlx::PgPool, class: u8) -> Result<(), Error> {
        sqlx::query("UPDATE characters SET current_class = $1 WHERE character_id = $2")
            .bind(i64::from(class))
            .bind(self.inner.character_id)
            .execute(pool)
            .await?;
        self.class.store(class, Ordering::Relaxed);
        // Trojan life and Taoist mana grow with the promotion.
        self.recalculate_resources(false);
        Ok(())
    }

    pub fn previous_class(&self) -> u8 {
        self.previous_class.load(Ordering::Relaxed)
    }

    pub fn rebirths(&self) -> u8 {
        self.rebirths.load(Ordering::Relaxed)
    }

    pub fn virtue(&self) -> u32 {
        self.virtue.load(Ordering::Relaxed)
    }

    pub fn set_virtue(&self, points: u32) {
        self.virtue.store(points, Ordering::Relaxed);
    }

    /// Spends `amount` virtue; `false` (nothing spent) when short.
    pub fn spend_virtue(&self, amount: u32) -> bool {
        self.virtue
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |virtue| virtue.checked_sub(amount))
            .is_ok()
    }

    /// The first rebirth (Redux `[3600] Alex`): the new profession, level
    /// 15 with no experience, the reborn attributes and their bonus points,
    /// every skill back to level 1, and the path's skills learned or
    /// forgotten. The caller saves the character afterwards.
    pub async fn rebirth(
        &self,
        pool: &sqlx::PgPool,
        new_class: u8,
        bonus_points: u16,
        learn: &[u32],
        forget: &[u32],
    ) -> Result<(), Error> {
        let mut transaction = pool.begin().await?;
        for skill_id in forget {
            sqlx::query("DELETE FROM character_skills WHERE character_id = $1 AND skill_id = $2")
                .bind(self.inner.character_id)
                .bind(i64::from(*skill_id))
                .execute(&mut *transaction)
                .await?;
        }
        sqlx::query("UPDATE character_skills SET level = 1, experience = 0, proficiency = 0 WHERE character_id = $1")
            .bind(self.inner.character_id)
            .execute(&mut *transaction)
            .await?;
        for skill_id in learn {
            sqlx::query(
                "INSERT INTO character_skills (character_id, skill_id, level, experience, proficiency)
                 VALUES ($1, $2, 1, 0, 0)
                 ON CONFLICT(character_id, skill_id) DO NOTHING",
            )
            .bind(self.inner.character_id)
            .bind(i64::from(*skill_id))
            .execute(&mut *transaction)
            .await?;
        }
        sqlx::query("UPDATE characters SET current_class = $1, previous_class = $2, rebirths = $3 WHERE character_id = $4")
            .bind(i64::from(new_class))
            .bind(i64::from(self.current_class()))
            .bind(i64::from(self.rebirths().saturating_add(1)))
            .bind(self.inner.character_id)
            .execute(&mut *transaction)
            .await?;
        transaction.commit().await?;

        self.previous_class.store(self.current_class(), Ordering::Relaxed);
        self.class.store(new_class, Ordering::Relaxed);
        self.rebirths.fetch_add(1, Ordering::Relaxed);
        {
            let mut skills = self.skills.lock();
            for skill_id in forget {
                skills.remove(skill_id);
            }
            for progress in skills.values_mut() {
                progress.level = 1;
                progress.experience = 0;
                progress.proficiency = 0;
            }
            for skill_id in learn {
                skills.entry(*skill_id).or_insert(SkillProgress {
                    skill_id: *skill_id,
                    level: 1,
                    experience: 0,
                    proficiency: 0,
                });
            }
        }
        *self.attributes.lock() = BaseAttributes {
            strength: 0,
            agility: 2,
            vitality: 8,
            spirit: 0,
            points: bonus_points,
        };
        self.entity.set_level(15);
        self.experience.store(0, Ordering::Relaxed);
        self.recalculate_resources(true);
        Ok(())
    }

    /// Takes `count` items of `item_type` out of the inventory at once; on
    /// `None` nothing was taken.
    pub async fn consume_items(
        &self,
        pool: &sqlx::PgPool,
        item_type: u32,
        count: usize,
    ) -> Result<Option<Vec<InventoryItem>>, Error> {
        let items = self
            .inventory_items()
            .into_iter()
            .filter(|owned| owned.item_type == item_type)
            .take(count)
            .collect::<Vec<_>>();
        if items.len() != count {
            return Ok(None);
        }
        let mut transaction = pool.begin().await?;
        for item in &items {
            let deleted =
                sqlx::query("DELETE FROM inventory_items WHERE item_uid = $1 AND character_id = $2 AND item_type = $3")
                    .bind(i64::from(item.uid))
                    .bind(self.inner.character_id)
                    .bind(i64::from(item_type))
                    .execute(&mut *transaction)
                    .await?;
            if deleted.rows_affected() != 1 {
                transaction.rollback().await?;
                return Ok(None);
            }
        }
        transaction.commit().await?;
        self.inventory
            .lock()
            .retain(|owned| !items.iter().any(|item| item.uid == owned.uid));
        Ok(Some(items))
    }

    /// Removes these inventory items, all or none.
    pub async fn consume_item_uids(
        &self,
        pool: &sqlx::PgPool,
        uids: &[u32],
    ) -> Result<Option<Vec<InventoryItem>>, Error> {
        let items = self
            .inventory_items()
            .into_iter()
            .filter(|owned| uids.contains(&owned.uid))
            .collect::<Vec<_>>();
        if items.len() != uids.len() {
            return Ok(None);
        }
        let mut transaction = pool.begin().await?;
        for item in &items {
            let deleted = sqlx::query("DELETE FROM inventory_items WHERE item_uid = $1 AND character_id = $2")
                .bind(i64::from(item.uid))
                .bind(self.inner.character_id)
                .execute(&mut *transaction)
                .await?;
            if deleted.rows_affected() != 1 {
                transaction.rollback().await?;
                return Ok(None);
            }
        }
        transaction.commit().await?;
        self.inventory.lock().retain(|owned| !uids.contains(&owned.uid));
        Ok(Some(items))
    }

    pub fn count_items(&self, item_type: u32) -> usize {
        self.inventory
            .lock()
            .iter()
            .filter(|owned| owned.item_type == item_type)
            .count()
    }

    /// The best proficiency of a weapon type (`item_type / 1000`).
    pub fn weapon_type_proficiency(&self, weapon_type: u32) -> Option<WeaponProficiency> {
        self.weapon_proficiencies()
            .into_iter()
            .filter(|progress| progress.weapon_family / 100 == weapon_type)
            .max_by_key(|progress| (progress.level, progress.proficiency))
    }

    /// One proficiency level up for a weapon type (ProficiencyGod).
    pub async fn raise_weapon_proficiency(
        &self,
        pool: &sqlx::PgPool,
        weapon_type: u32,
    ) -> Result<Option<WeaponProficiency>, Error> {
        let Some(mut progress) = self.weapon_type_proficiency(weapon_type) else {
            return Ok(None);
        };
        progress.level = progress.level.saturating_add(1);
        progress.proficiency = 0;
        sqlx::query(
            "UPDATE character_weapon_proficiencies SET level = $1, proficiency = 0
             WHERE character_id = $2 AND weapon_family = $3",
        )
        .bind(i64::from(progress.level))
        .bind(self.inner.character_id)
        .bind(i64::from(progress.weapon_family))
        .execute(pool)
        .await?;
        self.weapon_proficiencies.lock().insert(progress.weapon_family, progress);
        Ok(Some(progress))
    }

    /// Opens the first or second socket of a worn item.
    pub async fn open_equipment_socket(
        &self,
        pool: &sqlx::PgPool,
        position: u8,
        second: bool,
    ) -> Result<Option<InventoryItem>, Error> {
        let Some(mut item) = self.equipment_item(position) else {
            return Ok(None);
        };
        if second {
            item.socket_two = 1;
        } else {
            item.socket_one = 1;
        }
        sqlx::query(
            "INSERT INTO item_upgrades (item_uid, plus, gem_one, gem_two, socket_one, socket_two, rarity)
             VALUES ($1, $2, $3, $4, $5, $6, $7)
             ON CONFLICT(item_uid) DO UPDATE SET socket_one = excluded.socket_one, socket_two = excluded.socket_two",
        )
        .bind(i64::from(item.uid))
        .bind(i64::from(item.plus))
        .bind(i64::from(item.gem_one))
        .bind(i64::from(item.gem_two))
        .bind(i64::from(item.socket_one))
        .bind(i64::from(item.socket_two))
        .bind(i64::from(item.rarity))
        .execute(pool)
        .await?;
        if let Some(existing) = self.equipment.lock().iter_mut().find(|owned| owned.uid == item.uid) {
            *existing = item;
        }
        Ok(Some(item))
    }

    pub async fn kick_back(&self) -> Result<(), Error> {
        let location = self.entity.location();
        let xy = u32::constract(location.y, location.x);
        let msg = MsgAction::new(
            self.entity.id(),
            self.entity.map_id(),
            xy,
            location.direction as u16,
            ActionType::Teleport,
        );
        self.owner.send(msg).await?;
        Ok(())
    }

    #[tracing::instrument(skip(self, state), fields(me = self.entity.id()))]
    pub async fn teleport(&self, state: &crate::State, map_id: u32, (x, y): (u16, u16)) -> Result<(), Error> {
        self.pace.reset();
        let mut location = self.entity.location();
        let xy = u32::constract(y, x);
        let new_map = state.try_map(map_id)?;
        // The client loads the map's file: a copy (an event arena) names the
        // map it copies.
        let msg = MsgAction::new(
            self.entity.id(),
            new_map.map_id(),
            xy,
            location.direction as u16,
            ActionType::Teleport,
        );
        new_map.load().await?;
        let tile = new_map.tile(x, y).ok_or(Error::TileNotFound(x, y))?;
        // remove from old map
        if let Ok(old_map) = state.try_map(self.entity.map_id()) {
            old_map.remove_entity_by_id_and_location(self.id(), self.entity().location())?;
            self.try_screen()?.remove_from_observers().await?;
        }
        location.x = x;
        location.y = y;
        self.entity.set_location(location).set_map_id(map_id);
        self.set_elevation(tile.elevation);
        self.owner.send(msg).await?;
        self.owner.send(MsgWeather::new(new_map.weather())).await?;
        self.owner.send(MsgMapInfo::from_map(new_map)).await?;
        Ok(())
    }

    #[tracing::instrument(skip_all, fields(me = self.entity.id()))]
    pub async fn exchange_spawn_packets<E: AsRef<GameEntity>>(&self, observer: &E) -> Result<(), Error> {
        match observer.as_ref() {
            GameEntity::Character(c) => {
                self.send_spawn(&c.owner()).await?;
                c.send_spawn(&self.owner).await?;
            },
            _ => {
                // We only exchange spawn packets with characters
            },
        }
        Ok(())
    }

    #[tracing::instrument(skip(self, state), fields(me = self.entity.id()))]
    pub async fn save(&self, state: &crate::State) -> Result<(), Error> {
        let mut location = self.entity.location();
        // Nobody wakes up on the event arena: a return lands in Twin City.
        let map_id = if self.entity.map_id() == crate::events::EVENT_MAP {
            location.x = 430;
            location.y = 380;
            1_002
        } else {
            self.entity.map_id()
        };
        let e = tq_db::character::Character {
            character_id: self.inner.character_id,
            account_id: self.inner.account_id,
            realm_id: self.inner.realm_id,
            name: self.entity.name().to_string(),
            mesh: self.entity.mesh() as _,
            avatar: self.avatar() as _,
            hair_style: self.hair_style() as _,
            silver: self.silver() as _,
            cps: self.cps() as _,
            current_class: self.current_class() as _,
            previous_class: self.previous_class() as _,
            rebirths: self.rebirths() as _,
            level: self.entity.level() as _,
            experience: self.experience() as _,
            map_id: map_id as _,
            x: location.x as _,
            y: location.y as _,
            virtue: self.virtue() as _,
            strength: self.strength() as _,
            agility: self.agility() as _,
            vitality: self.vitality() as _,
            spirit: self.spirit() as _,
            attribute_points: self.attribute_points() as _,
            health_points: self.health_points() as _,
            mana_points: self.mana_points() as _,
            kill_points: self.kill_points() as _,
            nobility_donation: self.nobility_donation() as _,
            equipped_title: self.equipped_title() as _,
            double_experience_until: self.double_experience_until_unix(),
            heaven_blessing_until: self.heaven_blessing_until_unix(),
            experience_multiplier: i64::from(self.experience_multiplier.load(Ordering::Relaxed)),
            lucky_time: i64::from(self.lucky_time()),
        };
        e.update(state.pool()).await?;
        Ok(())
    }

    #[tracing::instrument(skip(self, to), fields(me = self.entity.id()))]
    pub(super) async fn send_spawn(&self, to: &ActorHandle) -> Result<(), Error> {
        let msg = MsgPlayer::from(self);
        to.send(msg).await?;
        // An open booth shows its name to whoever comes near.
        if let Some(name) = self.booth_name() {
            to.send(crate::packets::MsgTalk::booth(self, &name)).await?;
        }
        tracing::trace!("Sent Spawn");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tq_network::Actor;

    async fn test_character() -> Result<(sqlx::PgPool, Character, u32), Error> {
        let pool = tq_db::testing::pool().await;

        let (tx, _rx) = tokio::sync::mpsc::channel(8);
        let actor = Actor::<crate::ActorState>::new(tx);
        actor.set_id(1);
        let inner = crate::packets::MsgRegister::build_character_with(
            "item-test".to_owned(),
            crate::packets::BodyType::MuscularMale,
            crate::packets::BaseClass::Trojan,
            1,
            1,
        )?;
        inner.save(&pool).await?;
        let inner = tq_db::character::Character::from_account(&pool, 1)
            .await?
            .expect("test character should exist");
        let database_character_id = u32::try_from(inner.character_id).expect("character ID should be positive");
        let character = Character::new(actor.handle(), inner);
        Ok((pool, character, database_character_id))
    }

    async fn insert_inventory_item(
        pool: &sqlx::PgPool,
        character_id: u32,
        uid: u32,
        item_type: u32,
        position: u8,
    ) -> Result<(), Error> {
        sqlx::query(
            "INSERT INTO inventory_items \
             (item_uid, character_id, item_type, position, durability, max_durability) \
             VALUES ($1, $2, $3, $4, 100, 100)",
        )
        .bind(i64::from(uid))
        .bind(i64::from(character_id))
        .bind(i64::from(item_type))
        .bind(i64::from(position))
        .execute(pool)
        .await?;
        Ok(())
    }

    async fn insert_equipment_item(
        pool: &sqlx::PgPool,
        character_id: u32,
        uid: u32,
        item_type: u32,
        position: u8,
    ) -> Result<(), Error> {
        sqlx::query(
            "INSERT INTO equipment_items \
             (item_uid, character_id, item_type, position, durability, max_durability) \
             VALUES ($1, $2, $3, $4, 100, 100)",
        )
        .bind(i64::from(uid))
        .bind(i64::from(character_id))
        .bind(i64::from(item_type))
        .bind(i64::from(position))
        .execute(pool)
        .await?;
        Ok(())
    }

    #[tokio::test]
    async fn invalid_item_transitions_leave_inventory_and_equipment_unchanged() -> Result<(), Error> {
        let (pool, character, database_character_id) = test_character().await?;
        insert_inventory_item(&pool, database_character_id, 501, 410001, 0).await?;
        insert_equipment_item(&pool, database_character_id, 601, 111001, 1).await?;
        insert_inventory_item(&pool, database_character_id, 502, 1000000, 1).await?;
        character.load_inventory(&pool).await?;
        character.load_equipment(&pool).await?;

        let inventory_before = character.inventory_items();
        let equipment_before = character.equipment_items();
        assert!(character.equip_inventory_item(&pool, 501, 10).await?.is_none());
        assert!(character.unequip_item(&pool, 601, 1).await?.is_none());
        assert!(character.move_inventory_item(&pool, 501, 40).await?.is_none());
        assert_eq!(character.inventory_items(), inventory_before);
        assert_eq!(character.equipment_items(), equipment_before);

        let inventory_rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM inventory_items")
            .fetch_one(&pool)
            .await?;
        let equipment_rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM equipment_items")
            .fetch_one(&pool)
            .await?;
        assert_eq!(inventory_rows, 2);
        assert_eq!(equipment_rows, 1);
        Ok(())
    }

    #[tokio::test]
    async fn nobility_donation_is_atomic_and_persists_rank_progress() -> Result<(), Error> {
        let (pool, character, database_character_id) = test_character().await?;
        sqlx::query("UPDATE characters SET level = 70, silver = $1 WHERE character_id = $2")
            .bind(3_500_000_i64)
            .bind(i64::from(database_character_id))
            .execute(&pool)
            .await?;
        character.entity.set_level(70);
        character.silver.store(3_500_000, Ordering::Relaxed);

        assert_eq!(
            character.donate_nobility(&pool, 2_999_999).await?,
            crate::nobility::DonateOutcome::InvalidAmount
        );
        assert_eq!(character.silver(), 3_500_000);
        assert_eq!(character.nobility_donation(), 0);

        let outcome = character.donate_nobility(&pool, 3_000_000).await?;
        let crate::nobility::DonateOutcome::Donated(status) = outcome else {
            panic!("valid donation should succeed");
        };
        assert_eq!(status.rank, crate::nobility::NobilityRank::Serf);
        assert_eq!(status.donation, 3_000_000);
        assert_eq!(character.silver(), 500_000);
        assert_eq!(character.nobility_donation(), 3_000_000);

        assert_eq!(
            character.donate_nobility(&pool, 3_000_000).await?,
            crate::nobility::DonateOutcome::InsufficientSilver
        );
        let saved: (i64, i64) =
            sqlx::query_as("SELECT silver, nobility_donation FROM characters WHERE character_id = $1")
                .bind(i64::from(database_character_id))
                .fetch_one(&pool)
                .await?;
        assert_eq!(saved, (500_000, 3_000_000));

        let reloaded = tq_db::character::Character::by_id(&pool, database_character_id as i32).await?;
        let restored = Character::new(character.owner(), reloaded);
        assert_eq!(restored.nobility_status(), character.nobility_status());
        Ok(())
    }

    #[tokio::test]
    async fn nobility_ranking_orders_donation_and_breaks_ties_by_id() -> Result<(), Error> {
        let (pool, _character, database_character_id) = test_character().await?;
        sqlx::query("UPDATE characters SET nobility_donation = 5000000 WHERE character_id = $1")
            .bind(i64::from(database_character_id))
            .execute(&pool)
            .await?;
        sqlx::query(
            "INSERT INTO characters (account_id, realm_id, name, nobility_donation)
             VALUES (1, 1, 'rank-high', 30000000), (1, 1, 'rank-tie', 30000000)",
        )
        .execute(&pool)
        .await?;

        let entries = crate::nobility::ranking(&pool, 10).await?;
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].name, "rank-high");
        assert_eq!(entries[1].name, "rank-tie");
        assert_eq!(entries[2].name, "item-test");
        assert_eq!(entries[0].rank, crate::nobility::NobilityRank::Knight);
        assert_eq!(entries[2].rank, crate::nobility::NobilityRank::Serf);
        Ok(())
    }

    #[tokio::test]
    async fn title_equipping_validates_unlocks_and_persists() -> Result<(), Error> {
        let (pool, character, database_character_id) = test_character().await?;
        assert_eq!(character.equipped_title(), 0);
        assert_eq!(
            character.equip_title(&pool, 999).await?,
            crate::titles::EquipOutcome::UnknownTitle
        );
        assert_eq!(
            character.equip_title(&pool, 2).await?,
            crate::titles::EquipOutcome::Locked
        );

        sqlx::query("UPDATE characters SET level = 30 WHERE character_id = $1")
            .bind(i64::from(database_character_id))
            .execute(&pool)
            .await?;
        character.entity.set_level(30);
        assert_eq!(
            character.equip_title(&pool, 2).await?,
            crate::titles::EquipOutcome::Equipped(2)
        );
        assert_eq!(character.equipped_title_name(), "Veteran");
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT equipped_title FROM characters WHERE character_id = $1",)
                .bind(i64::from(database_character_id))
                .fetch_one(&pool)
                .await?,
            2
        );
        assert_eq!(
            character.equip_title(&pool, crate::titles::NONE).await?,
            crate::titles::EquipOutcome::Equipped(crate::titles::NONE)
        );
        assert_eq!(character.equipped_title(), 0);
        Ok(())
    }

    #[tokio::test]
    async fn guild_warehouse_round_trip_preserves_uid_and_upgrades() -> Result<(), Error> {
        let (pool, character, database_character_id) = test_character().await?;
        sqlx::query("INSERT INTO guilds (guild_id, name, leader_id) VALUES (1, 'Knights', $1)")
            .bind(i64::from(database_character_id))
            .execute(&pool)
            .await?;
        sqlx::query("INSERT INTO guild_members (guild_id, character_id, role) VALUES (1, $1, 1000)")
            .bind(i64::from(database_character_id))
            .execute(&pool)
            .await?;
        insert_inventory_item(&pool, database_character_id, 701, 410003, 3).await?;
        sqlx::query("INSERT INTO item_upgrades (item_uid, plus, gem_one, socket_one) VALUES (701, 4, 13, 1)")
            .execute(&pool)
            .await?;
        character.load_inventory(&pool).await?;

        let Some((removed, stored)) = character.store_guild_warehouse_item(&pool, 1, 701, 6).await? else {
            panic!("inventory item should be stored");
        };
        assert_eq!(removed.uid, 701);
        assert_eq!(removed.position, 3);
        assert_eq!(removed.plus, 4);
        assert_eq!(removed.gem_one, 13);
        assert_eq!(stored.position, 6);
        assert!(character.inventory_items().iter().all(|item| item.uid != 701));
        let guild_row: (i64, i64, i64, i64, i64, i64) = sqlx::query_as(
            "SELECT item_uid, guild_id, item_type, position, durability, max_durability
             FROM guild_warehouse_items WHERE guild_id = 1 AND item_uid = 701",
        )
        .fetch_one(&pool)
        .await?;
        assert_eq!(guild_row, (701, 1, 410003, 6, 100, 100));

        let Some((warehouse_item, withdrawn)) = character.withdraw_guild_warehouse_item(&pool, 1, 701, 2).await? else {
            panic!("guild item should be withdrawn");
        };
        assert_eq!(warehouse_item.uid, 701);
        assert_eq!(warehouse_item.position, 6);
        assert_eq!(withdrawn.uid, 701);
        assert_eq!(withdrawn.position, 2);
        assert_eq!(withdrawn.plus, 4);
        assert_eq!(withdrawn.gem_one, 13);
        let inventory_row: (i64, i64, i64, i64) = sqlx::query_as(
            "SELECT item_uid, item_type, position, durability FROM inventory_items
             WHERE character_id = $1 AND item_uid = 701",
        )
        .bind(i64::from(database_character_id))
        .fetch_one(&pool)
        .await?;
        assert_eq!(inventory_row, (701, 410003, 2, 100));
        let remaining: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM guild_warehouse_items WHERE guild_id = 1")
            .fetch_one(&pool)
            .await?;
        assert_eq!(remaining, 0);
        Ok(())
    }

    /// Another character on `pool`, of account `account`.
    async fn another_character(pool: &sqlx::PgPool, account: u32, name: &str) -> Result<(Character, u32), Error> {
        sqlx::query("INSERT INTO accounts (account_id, username, password) VALUES ($1, $2, '') ON CONFLICT DO NOTHING")
            .bind(i64::from(account))
            .bind(name)
            .execute(pool)
            .await?;
        let (tx, _rx) = tokio::sync::mpsc::channel(8);
        let actor = Actor::<crate::ActorState>::new(tx);
        actor.set_id(account as usize);
        crate::packets::MsgRegister::build_character_with(
            name.to_owned(),
            crate::packets::BodyType::MuscularMale,
            crate::packets::BaseClass::Trojan,
            account,
            1,
        )?
        .save(pool)
        .await?;
        let inner = tq_db::character::Character::from_account(pool, account)
            .await?
            .expect("the other character should exist");
        let id = u32::try_from(inner.character_id).expect("character ID should be positive");
        Ok((Character::new(actor.handle(), inner), id))
    }

    #[tokio::test]
    async fn guild_members_racing_for_one_item_get_it_once() -> Result<(), Error> {
        let (pool, first, first_id) = test_character().await?;
        let (second, _) = another_character(&pool, 2, "item-test-2").await?;
        sqlx::query("INSERT INTO guilds (guild_id, name, leader_id) VALUES (1, 'Knights', $1)")
            .bind(i64::from(first_id))
            .execute(&pool)
            .await?;
        for round in 0..20_u32 {
            let uid = 900 + round;
            sqlx::query(
                "INSERT INTO guild_warehouse_items (item_uid, guild_id, item_type, position, durability, max_durability)
                 VALUES ($1, 1, 410003, 0, 100, 100)",
            )
            .bind(i64::from(uid))
            .execute(&pool)
            .await?;
            let slot = u8::try_from(round % 40).expect("slot");
            let (left, right) = tokio::join!(
                first.withdraw_guild_warehouse_item(&pool, 1, uid, slot),
                second.withdraw_guild_warehouse_item(&pool, 1, uid, slot),
            );
            let winners = usize::from(left?.is_some()) + usize::from(right?.is_some());
            assert_eq!(winners, 1, "round {round}: exactly one member gets the item");
            let copies: i64 = sqlx::query_scalar(
                "SELECT (SELECT COUNT(*) FROM inventory_items WHERE item_uid = $1)
                      + (SELECT COUNT(*) FROM guild_warehouse_items WHERE item_uid = $1)",
            )
            .bind(i64::from(uid))
            .fetch_one(&pool)
            .await?;
            assert_eq!(copies, 1, "round {round}: the item exists once");
            sqlx::query("DELETE FROM inventory_items WHERE item_uid = $1")
                .bind(i64::from(uid))
                .execute(&pool)
                .await?;
        }
        Ok(())
    }

    #[tokio::test]
    async fn guild_members_storing_into_one_slot_keep_the_other_item() -> Result<(), Error> {
        let (pool, first, first_id) = test_character().await?;
        let (second, second_id) = another_character(&pool, 2, "item-test-2").await?;
        sqlx::query("INSERT INTO guilds (guild_id, name, leader_id) VALUES (1, 'Knights', $1)")
            .bind(i64::from(first_id))
            .execute(&pool)
            .await?;
        for round in 0..20_u32 {
            let (left_uid, right_uid) = (1000 + round * 2, 1001 + round * 2);
            insert_inventory_item(&pool, first_id, left_uid, 410003, 1).await?;
            insert_inventory_item(&pool, second_id, right_uid, 410003, 1).await?;
            first.load_inventory(&pool).await?;
            second.load_inventory(&pool).await?;
            let (left, right) = tokio::join!(
                first.store_guild_warehouse_item(&pool, 1, left_uid, 5),
                second.store_guild_warehouse_item(&pool, 1, right_uid, 5),
            );
            let stored = usize::from(left?.is_some()) + usize::from(right?.is_some());
            assert_eq!(stored, 1, "round {round}: one item takes the slot");
            let in_bags: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM inventory_items WHERE item_uid IN ($1, $2)")
                .bind(i64::from(left_uid))
                .bind(i64::from(right_uid))
                .fetch_one(&pool)
                .await?;
            assert_eq!(in_bags, 1, "round {round}: the other item stays in its bag");
            sqlx::query("DELETE FROM guild_warehouse_items WHERE guild_id = 1")
                .execute(&pool)
                .await?;
            sqlx::query("DELETE FROM inventory_items WHERE item_uid IN ($1, $2)")
                .bind(i64::from(left_uid))
                .bind(i64::from(right_uid))
                .execute(&pool)
                .await?;
        }
        Ok(())
    }

    #[tokio::test]
    async fn warehouse_silver_transfers_are_atomic_and_bounded() -> Result<(), Error> {
        let (pool, character, database_character_id) = test_character().await?;
        sqlx::query("UPDATE characters SET silver = 1000 WHERE character_id = $1")
            .bind(i64::from(database_character_id))
            .execute(&pool)
            .await?;
        character.silver.store(1_000, Ordering::Relaxed);
        character.load_warehouse(&pool).await?;

        assert_eq!(character.transfer_warehouse_silver(&pool, 250, true).await?, Some(250));
        assert_eq!(character.silver(), 750);
        assert_eq!(character.warehouse_silver(), 250);

        // An insufficient deposit must not leave the debit half committed.
        assert!(character.transfer_warehouse_silver(&pool, 1_000, true).await?.is_none());
        assert_eq!(character.silver(), 750);
        assert_eq!(character.warehouse_silver(), 250);

        // An insufficient withdrawal must not touch the warehouse balance.
        assert!(character.transfer_warehouse_silver(&pool, 251, false).await?.is_none());
        assert_eq!(character.silver(), 750);
        assert_eq!(character.warehouse_silver(), 250);

        // Credit-side overflow is rejected atomically as well.
        let near_max = u64::from(u32::MAX) - 5;
        sqlx::query("UPDATE characters SET silver = $1 WHERE character_id = $2")
            .bind(i64::try_from(near_max).expect("u32 max fits a BIGINT"))
            .bind(i64::from(database_character_id))
            .execute(&pool)
            .await?;
        sqlx::query("UPDATE warehouse_balances SET silver = 10 WHERE character_id = $1")
            .bind(i64::from(database_character_id))
            .execute(&pool)
            .await?;
        character.silver.store(near_max, Ordering::Relaxed);
        character.load_warehouse(&pool).await?;

        assert!(character.transfer_warehouse_silver(&pool, 10, false).await?.is_none());
        assert_eq!(character.silver(), near_max);
        assert_eq!(character.warehouse_silver(), 10);

        let (saved_character_silver, saved_warehouse_silver): (i64, i64) = sqlx::query_as(
            "SELECT c.silver, w.silver
             FROM characters c
             JOIN warehouse_balances w ON w.character_id = c.character_id
             WHERE c.character_id = $1",
        )
        .bind(i64::from(database_character_id))
        .fetch_one(&pool)
        .await?;
        assert_eq!(saved_character_silver, i64::try_from(near_max).unwrap());
        assert_eq!(saved_warehouse_silver, 10);

        // Direct rewards use the same bounded balance rule and remain unchanged
        // when the maximum representable silver value would be exceeded.
        assert_eq!(character.grant_silver(&pool, 10).await?, near_max as u32);
        assert_eq!(character.silver(), near_max);
        Ok(())
    }

    #[tokio::test]
    async fn equipment_durability_degrades_and_repairs_atomically() -> Result<(), Error> {
        let (pool, character, database_character_id) = test_character().await?;
        insert_equipment_item(&pool, database_character_id, 601, 410001, 4).await?;
        sqlx::query("UPDATE equipment_items SET durability = 1 WHERE item_uid = 601")
            .execute(&pool)
            .await?;
        character.load_equipment(&pool).await?;

        let broken = character
            .degrade_equipment_item(&pool, 4, 1)
            .await?
            .expect("equipped weapon should lose durability");
        assert_eq!(broken.durability, 0);
        assert_eq!(
            character.equipment_item(4).expect("weapon remains equipped").durability,
            0
        );

        let silver_before = character.silver();
        assert!(character
            .repair_all_equipment(&pool, silver_before as u32 + 1)
            .await?
            .is_none());
        assert_eq!(character.silver(), silver_before);
        let still_broken: i64 = sqlx::query_scalar("SELECT durability FROM equipment_items WHERE item_uid = 601")
            .fetch_one(&pool)
            .await?;
        assert_eq!(still_broken, 0);

        let repaired = character
            .repair_all_equipment(&pool, 100)
            .await?
            .expect("character can pay the repair cost");
        assert_eq!(repaired.len(), 1);
        assert_eq!(repaired[0].durability, 100);
        assert_eq!(character.silver(), silver_before - 100);
        let restored: i64 = sqlx::query_scalar("SELECT durability FROM equipment_items WHERE item_uid = 601")
            .fetch_one(&pool)
            .await?;
        assert_eq!(restored, 100);
        Ok(())
    }

    #[tokio::test]
    async fn refinery_consumes_material_and_reloads_for_equipped_item() -> Result<(), Error> {
        let (pool, character, database_character_id) = test_character().await?;
        insert_equipment_item(&pool, database_character_id, 601, 111003, 1).await?;
        insert_inventory_item(&pool, database_character_id, 602, 721380, 0).await?;
        character.load_inventory(&pool).await?;
        character.load_equipment(&pool).await?;
        character.load_refineries(&pool).await?;

        let outcome = character.apply_refinery(&pool, 601, 602).await?;
        let RefineryOutcome::Applied { effect, .. } = outcome else {
            panic!("compatible refinery should apply");
        };
        assert_eq!((effect.refinery_type, effect.level, effect.effect), (312, 1, 4));
        assert!(character.inventory_items().iter().all(|item| item.uid != 602));
        assert_eq!(character.refinery_effect(601), Some(effect));

        character.load_refineries(&pool).await?;
        assert_eq!(character.refinery_effect(601), Some(effect));
        let persisted: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM item_refineries WHERE item_uid = 601")
            .fetch_one(&pool)
            .await?;
        assert_eq!(persisted, 1);
        Ok(())
    }

    #[tokio::test]
    async fn artifact_consumes_material_and_meteor_before_equipping() -> Result<(), Error> {
        let (pool, character, database_character_id) = test_character().await?;
        insert_inventory_item(&pool, database_character_id, 701, 111003, 0).await?;
        insert_inventory_item(&pool, database_character_id, 702, 820001, 1).await?;
        insert_inventory_item(&pool, database_character_id, 703, 1_088_001, 2).await?;
        character.load_inventory(&pool).await?;
        character.load_artifacts(&pool).await?;

        let outcome = character.apply_artifact(&pool, 701, 702).await?;
        let ArtifactOutcome::Applied { effect, catalyst, .. } = outcome else {
            panic!("compatible artifact should apply");
        };
        assert_eq!((effect.artifact_type, effect.level, effect.effect), (820001, 1, 25));
        assert_eq!(catalyst.uid, 703);
        assert!(character
            .inventory_items()
            .iter()
            .all(|item| item.uid != 702 && item.uid != 703));
        assert_eq!(character.artifact_effect(701), Some(effect));

        character.load_artifacts(&pool).await?;
        assert_eq!(character.artifact_effect(701), Some(effect));
        let persisted: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM item_artifacts WHERE item_uid = 701")
            .fetch_one(&pool)
            .await?;
        assert_eq!(persisted, 1);
        Ok(())
    }

    #[tokio::test]
    async fn equipped_artifact_contributes_fire_resistance_only_while_usable() -> Result<(), Error> {
        let (pool, character, database_character_id) = test_character().await?;
        insert_equipment_item(&pool, database_character_id, 601, 111003, 1).await?;
        sqlx::query(
            "INSERT INTO item_artifacts (item_uid, artifact_type, artifact_level, effect, expires_at)
             VALUES (601, 820001, 1, 25, 0)",
        )
        .execute(&pool)
        .await?;

        character.load_equipment(&pool).await?;
        character.load_artifacts(&pool).await?;
        assert_eq!(character.fire_resistance_percent(), 25);

        sqlx::query("UPDATE equipment_items SET durability = 0 WHERE item_uid = 601")
            .execute(&pool)
            .await?;
        character.load_equipment(&pool).await?;
        assert_eq!(character.fire_resistance_percent(), 0);
        Ok(())
    }

    #[tokio::test]
    async fn kill_mode_controls_character_targets() -> Result<(), Error> {
        let (_pool, character, _database_character_id) = test_character().await?;
        assert!(!character.can_attack_character(&character));

        character.set_kill_mode(0);
        assert!(character.can_attack_character(&character));

        character.set_kill_mode(2);
        assert!(character.can_attack_character(&character));

        character.set_kill_mode(3);
        assert!(!character.can_attack_character(&character));
        character.entity().set_flags(crate::entities::Flags::BLACK_NAME);
        assert!(character.can_attack_character(&character));
        Ok(())
    }

    #[tokio::test]
    async fn silver_moves_between_a_member_and_the_guild_fund() -> Result<(), Error> {
        let (pool, character, database_character_id) = test_character().await?;
        sqlx::query("UPDATE characters SET silver = 50000 WHERE character_id = $1")
            .bind(i64::from(database_character_id))
            .execute(&pool)
            .await?;
        let guild_id: i64 = sqlx::query_scalar("INSERT INTO guilds (name, leader_id) VALUES ('FundTest', $1) RETURNING guild_id")
            .bind(i64::from(database_character_id))
            .fetch_one(&pool)
            .await?;
        let guild_id = guild_id as u32;
        assert_eq!(character.transfer_guild_fund(&pool, guild_id, 30_000, true).await?, Some((20_000, 30_000)));
        assert_eq!(character.silver(), 20_000);
        assert_eq!(character.transfer_guild_fund(&pool, guild_id, 30_000, true).await?, None, "not enough silver");
        assert_eq!(character.transfer_guild_fund(&pool, guild_id, 40_000, false).await?, None, "not enough in the fund");
        assert_eq!(character.transfer_guild_fund(&pool, guild_id, 10_000, false).await?, Some((30_000, 20_000)));
        assert_eq!(crate::guild::GuildManager::fund(&pool, guild_id).await?, 20_000);
        Ok(())
    }

    #[tokio::test]
    async fn a_full_xp_circle_lights_for_an_xp_skill() -> Result<(), Error> {
        let (pool, character, _database_character_id) = test_character().await?;
        let catalog = crate::skills::SkillCatalog::parse(include_str!("../../data/skills.dat")).expect("skills");
        character.learn_skill(&pool, 1_110, 1).await?;
        assert!(character.has_xp_skill(&catalog), "Cyclone is an XP skill");
        character.fill_xp();
        tokio::time::sleep(std::time::Duration::from_millis(1_100)).await;
        let change = character.regenerate(character.has_xp_skill(&catalog));
        assert!(change.flags, "the circle lights and the client hears of it");
        assert!(character.xp_ready());
        assert!(character.entity().flags().contains(crate::entities::Flags::XP_CIRCLE));
        Ok(())
    }

    #[tokio::test]
    async fn praying_stores_lucky_time_until_the_character_moves() -> Result<(), Error> {
        let (_pool, character, _database_character_id) = test_character().await?;
        character.start_praying(true);
        assert!(character.entity().flags().contains(crate::entities::Flags::CASTING_PRAY));
        assert!(!character.prayer_moved());
        character.add_lucky_time(3);
        character.add_lucky_time(crate::lucky::MAX_LUCKY_TIME);
        assert_eq!(character.lucky_time(), crate::lucky::MAX_LUCKY_TIME, "capped at two hours");
        let mut location = character.entity().location();
        location.x += 1;
        character.entity().set_location(location);
        assert!(character.prayer_moved());
        character.stop_praying();
        assert!(!character.entity().flags().intersects(crate::entities::Flags::CASTING_PRAY | crate::entities::Flags::PRAYING));
        character.wear_lucky_time();
        assert_eq!(character.lucky_time(), crate::lucky::MAX_LUCKY_TIME - 1);
        Ok(())
    }

    #[tokio::test]
    async fn a_revived_player_is_spared_until_they_attack() -> Result<(), Error> {
        let (_pool, character, _database_character_id) = test_character().await?;
        character.set_kill_mode(0);
        assert!(!character.revive_protected());
        character.refresh_after_revive();
        assert!(character.revive_protected());
        assert!(!character.can_attack_character(&character));
        character.end_revive_protection();
        assert!(character.can_attack_character(&character));
        Ok(())
    }

    #[tokio::test]
    async fn sockets_open_insert_and_remove_without_duplication() -> Result<(), Error> {
        let (pool, character, database_character_id) = test_character().await?;
        insert_equipment_item(&pool, database_character_id, 601, 410001, 4).await?;
        insert_inventory_item(&pool, database_character_id, 701, 700001, 0).await?;
        insert_inventory_item(&pool, database_character_id, 702, 1_088_000, 1).await?;
        character.load_inventory(&pool).await?;
        character.load_equipment(&pool).await?;

        let (opened, catalysts) = character.open_item_socket(&pool, 601).await?;
        let SocketOutcome::Opened(opened) = opened else {
            panic!("weapon should open a socket")
        };
        assert_eq!(opened.socket_one, 1);
        assert_eq!(catalysts.first().expect("DragonBall should be consumed").uid, 702);

        let SocketOutcome::Inserted { item, gem } = character.socket_gem(&pool, 601, 701, 1, false).await? else {
            panic!("gem should fit the open socket")
        };
        assert_eq!(item.gem_one, 1);
        assert_eq!(gem.uid, 701);
        assert!(character.inventory_items().iter().all(|owned| owned.uid != 701));

        let SocketOutcome::Removed(item) = character.socket_gem(&pool, 601, 0, 1, true).await? else {
            panic!("gem should be removable")
        };
        assert_eq!(item.gem_one, 0);
        assert_eq!(item.socket_one, 1);
        let persisted: (i64, i64, i64) =
            sqlx::query_as("SELECT gem_one, socket_one, socket_two FROM item_upgrades WHERE item_uid = 601")
                .fetch_one(&pool)
                .await?;
        assert_eq!(persisted, (0, 1, 0));

        for uid in 703..=707 {
            insert_inventory_item(
                &pool,
                database_character_id,
                uid,
                1_088_000,
                u8::try_from(uid - 700).unwrap(),
            )
            .await?;
        }
        character.load_inventory(&pool).await?;
        let (opened_again, _) = character.open_item_socket(&pool, 601).await?;
        let SocketOutcome::Opened(opened_again) = opened_again else {
            panic!("weapon should open its second socket")
        };
        assert_eq!((opened_again.socket_one, opened_again.socket_two), (1, 1));
        let remaining_dragonballs: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM inventory_items WHERE character_id = $1 AND item_type = 1088000")
                .bind(i64::from(database_character_id))
                .fetch_one(&pool)
                .await?;
        assert_eq!(remaining_dragonballs, 0);
        Ok(())
    }

    #[tokio::test]
    async fn equipped_gems_feed_authoritative_combat_stats() -> Result<(), Error> {
        let (pool, character, database_character_id) = test_character().await?;
        insert_equipment_item(&pool, database_character_id, 601, 410001, 4).await?;
        sqlx::query(
            "INSERT INTO item_upgrades (item_uid, plus, gem_one, gem_two, socket_one, socket_two)
             VALUES (601, 0, 13, 72, 1, 1)",
        )
        .execute(&pool)
        .await?;
        character.load_equipment(&pool).await?;

        let stats = character.combat_stats();
        assert_eq!(stats.attack_percent, 15);
        assert_eq!(stats.damage_reduction_percent, 4);
        Ok(())
    }

    #[tokio::test]
    async fn weapon_proficiency_persists_and_adds_a_small_attack_bonus() -> Result<(), Error> {
        let (pool, character, database_character_id) = test_character().await?;
        insert_equipment_item(&pool, database_character_id, 601, 410001, 4).await?;
        character.load_equipment(&pool).await?;
        character.load_weapon_proficiencies(&pool).await?;
        assert_eq!(character.weapon_proficiency_bonus_percent(), 0);

        let progress = character
            .practice_weapon_proficiency(&pool, 410001, 1_200)
            .await?
            .unwrap();
        assert_eq!(
            (progress.level, progress.experience, progress.proficiency),
            (2, 1_200, 0)
        );
        assert_eq!(character.weapon_proficiency_bonus_percent(), 2);
        let persisted: (i64, i64, i64, i64) = sqlx::query_as(
            "SELECT weapon_family, level, experience, proficiency
             FROM character_weapon_proficiencies WHERE character_id = $1",
        )
        .bind(i64::from(database_character_id))
        .fetch_one(&pool)
        .await?;
        assert_eq!(persisted, (41000, 2, 1_200, 0));

        character.load_weapon_proficiencies(&pool).await?;
        assert_eq!(character.weapon_proficiency(41000).unwrap().level, 2);
        Ok(())
    }

    #[tokio::test]
    async fn burning_status_reduces_defense_and_clears_after_expiration() -> Result<(), Error> {
        let (_pool, character, _database_character_id) = test_character().await?;
        let normal_defense = character.combat_stats().defense;
        character.apply_status(StatusEffect::Burning, Duration::from_secs(5));
        assert!(character.has_status(StatusEffect::Burning));
        assert!(character.entity().flags().contains(crate::entities::Flags::POISONED));
        assert_eq!(
            character.combat_stats().defense,
            normal_defense.saturating_mul(90) / 100
        );

        character.expire_statuses(std::time::Instant::now() + Duration::from_secs(6));
        assert!(!character.has_status(StatusEffect::Burning));
        assert!(!character.entity().flags().contains(crate::entities::Flags::POISONED));
        assert_eq!(character.combat_stats().defense, normal_defense);
        Ok(())
    }

    #[tokio::test]
    async fn cyclone_status_reduces_attack_interval_and_clears_after_expiration() -> Result<(), Error> {
        let (_pool, character, _database_character_id) = test_character().await?;
        let normal_speed = character.attack_speed_with_catalog(None);
        character.apply_status(StatusEffect::Cyclone, Duration::from_secs(5));
        assert!(character.has_status(StatusEffect::Cyclone));
        assert!(character.entity().flags().contains(crate::entities::Flags::CYCLONE));
        assert_eq!(character.attack_speed_with_catalog(None), normal_speed / 3);

        character.expire_statuses(std::time::Instant::now() + Duration::from_secs(6));
        assert!(!character.has_status(StatusEffect::Cyclone));
        assert!(!character.entity().flags().contains(crate::entities::Flags::CYCLONE));
        assert_eq!(character.attack_speed_with_catalog(None), normal_speed);
        Ok(())
    }

    #[tokio::test]
    async fn stigma_status_adds_attack_percent_and_clears_after_expiration() -> Result<(), Error> {
        let (_pool, character, _database_character_id) = test_character().await?;
        let normal_attack_percent = character.combat_stats().attack_percent;
        character.apply_status(StatusEffect::Stigma, Duration::from_secs(5));
        assert!(character.has_status(StatusEffect::Stigma));
        assert!(character.entity().flags().contains(crate::entities::Flags::STIGMA));
        assert_eq!(
            character.combat_stats().attack_percent,
            normal_attack_percent.saturating_add(20)
        );

        character.expire_statuses(std::time::Instant::now() + Duration::from_secs(6));
        assert!(!character.has_status(StatusEffect::Stigma));
        assert!(!character.entity().flags().contains(crate::entities::Flags::STIGMA));
        assert_eq!(character.combat_stats().attack_percent, normal_attack_percent);
        Ok(())
    }

    #[tokio::test]
    async fn shield_status_adds_defense_and_clears_after_expiration() -> Result<(), Error> {
        let (_pool, character, _database_character_id) = test_character().await?;
        let normal_defense = character.combat_stats().defense;
        character.apply_status(StatusEffect::Shield, Duration::from_secs(5));
        assert!(character.has_status(StatusEffect::Shield));
        assert!(character.entity().flags().contains(crate::entities::Flags::SHIELD));
        assert_eq!(
            character.combat_stats().defense,
            normal_defense.saturating_mul(120) / 100
        );

        character.expire_statuses(std::time::Instant::now() + Duration::from_secs(6));
        assert!(!character.has_status(StatusEffect::Shield));
        assert!(!character.entity().flags().contains(crate::entities::Flags::SHIELD));
        assert_eq!(character.combat_stats().defense, normal_defense);
        Ok(())
    }

    #[tokio::test]
    async fn item_requirements_validate_class_attributes_and_weapon_skill() -> Result<(), Error> {
        let (pool, character, _database_character_id) = test_character().await?;
        let mut template = crate::shop::ItemTemplate {
            required_profession: 10,
            required_weapon_skill: 1,
            level: 1,
            required_sex: 1,
            // A new Trojan's level 1 attributes (5 Str, 3 Vit, 2 Agi).
            required_strength: 5,
            required_agility: 2,
            required_vitality: 3,
            required_spirit: 0,
            ..Default::default()
        };
        assert!(!character.meets_item_requirements(410001, &template));
        character.practice_weapon_proficiency(&pool, 410001, 1_200).await?;
        assert!(character.meets_item_requirements(410001, &template));

        template.required_profession = 20;
        assert!(!character.meets_item_requirements(410001, &template));
        Ok(())
    }

    #[tokio::test]
    async fn quality_upgrade_consumes_dragonball_and_updates_item_atomically() -> Result<(), Error> {
        let (pool, character, database_character_id) = test_character().await?;
        insert_equipment_item(&pool, database_character_id, 601, 410005, 4).await?;
        insert_inventory_item(&pool, database_character_id, 701, 1_088_000, 0).await?;
        character.load_inventory(&pool).await?;
        character.load_equipment(&pool).await?;

        assert_eq!(Character::quality_upgrade_chance(410005), 85);
        let (outcome, catalyst) = character.upgrade_quality_with_dragonball_roll(&pool, 601, 0).await?;
        let QualityUpgradeOutcome::Upgraded(item) = outcome else {
            panic!("a zero roll should upgrade normal quality")
        };
        assert_eq!(item.item_type, 410006);
        assert_eq!(catalyst.expect("DragonBall should be consumed").uid, 701);
        assert_eq!(
            character
                .equipment_item(4)
                .expect("equipment remains equipped")
                .item_type,
            410006
        );

        insert_equipment_item(&pool, database_character_id, 602, 410006, 3).await?;
        insert_inventory_item(&pool, database_character_id, 702, 1_088_000, 1).await?;
        character.load_inventory(&pool).await?;
        character.load_equipment(&pool).await?;
        let (outcome, catalyst) = character.upgrade_quality_with_dragonball_roll(&pool, 602, 99).await?;
        let QualityUpgradeOutcome::Failed(item) = outcome else {
            panic!("a high roll should fail quality upgrade")
        };
        assert_eq!((item.item_type, item.durability), (410006, 50));
        assert_eq!(catalyst.expect("DragonBall should be consumed").uid, 702);
        let persisted: (i64, i64) =
            sqlx::query_as("SELECT item_type, durability FROM equipment_items WHERE item_uid = 602")
                .fetch_one(&pool)
                .await?;
        assert_eq!(persisted, (410006, 50));
        Ok(())
    }

    #[tokio::test]
    async fn gem_composition_consumes_fifteen_and_creates_next_tier_atomically() -> Result<(), Error> {
        let (pool, character, database_character_id) = test_character().await?;
        for index in 0..15_u32 {
            insert_inventory_item(&pool, database_character_id, 800 + index, 700_001, index as u8).await?;
        }
        sqlx::query("UPDATE characters SET silver = 100000 WHERE character_id = $1")
            .bind(i64::from(database_character_id))
            .execute(&pool)
            .await?;
        character.silver.store(100_000, Ordering::Relaxed);
        character.load_inventory(&pool).await?;
        let silver_before = character.silver();
        let GemComposeOutcome::Composed {
            consumed,
            created,
            silver,
        } = character.compose_gems(&pool, 700_001).await?
        else {
            panic!("fifteen normal gems should compose")
        };
        assert_eq!(consumed.len(), 15);
        assert_eq!(created.item_type, 700_002);
        assert_eq!(silver, 10_000);
        assert_eq!(character.silver(), silver_before - 10_000);
        let (normal_count, refined_count): (i64, i64) = sqlx::query_as(
            "SELECT
               (SELECT COUNT(*) FROM inventory_items WHERE character_id = $1 AND item_type = 700001),
               (SELECT COUNT(*) FROM inventory_items WHERE character_id = $2 AND item_type = 700002)",
        )
        .bind(i64::from(database_character_id))
        .bind(i64::from(database_character_id))
        .fetch_one(&pool)
        .await?;
        assert_eq!((normal_count, refined_count), (0, 1));
        Ok(())
    }

    #[tokio::test]
    async fn trainer_skill_learning_charges_only_after_atomic_success() -> Result<(), Error> {
        let (pool, character, database_character_id) = test_character().await?;
        let catalog = crate::skills::SkillCatalog::default();
        character.load_skills(&pool, &catalog).await?;
        let silver_before = character.silver();

        assert!(character
            .learn_skill_with_cost(&pool, crate::skills::WHIRLWIND_ID, 1, silver_before as u32 + 1)
            .await?
            .is_none());
        assert_eq!(character.silver(), silver_before);
        let missing: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM character_skills WHERE character_id = $1 AND skill_id = $2")
                .bind(i64::from(database_character_id))
                .bind(i64::from(crate::skills::WHIRLWIND_ID))
                .fetch_one(&pool)
                .await?;
        assert_eq!(missing, 0);

        let learned = character
            .learn_skill_with_cost(&pool, crate::skills::WHIRLWIND_ID, 1, 500)
            .await?
            .expect("character can pay for the trainer skill");
        assert_eq!(learned.skill_id, crate::skills::WHIRLWIND_ID);
        assert_eq!(character.silver(), silver_before - 500);
        assert!(character
            .learn_skill_with_cost(&pool, crate::skills::WHIRLWIND_ID, 1, 500)
            .await?
            .is_none());
        assert_eq!(character.silver(), silver_before - 500);
        Ok(())
    }

    #[tokio::test]
    async fn starter_skills_are_seeded_and_learned_levels_persist() -> Result<(), Error> {
        let (pool, character, database_character_id) = test_character().await?;
        let catalog = crate::skills::SkillCatalog::default();
        // Only Taoists start with Thunder; Fire waits for level 40.
        character.load_skills(&pool, &catalog).await?;
        assert!(!character.has_skill(crate::skills::BASIC_FIRE_ID), "not a Taoist");
        character.set_class(&pool, 100).await?;
        character.load_skills(&pool, &catalog).await?;
        assert!(character.has_skill(crate::skills::BASIC_FIRE_ID));
        assert!(!character.has_skill(crate::skills::FLAME_BURST_ID));

        character.learn_skill(&pool, crate::skills::BASIC_FIRE_ID, 3).await?;
        let saved_level: i64 =
            sqlx::query_scalar("SELECT level FROM character_skills WHERE character_id = $1 AND skill_id = $2")
                .bind(i64::from(database_character_id))
                .bind(i64::from(crate::skills::BASIC_FIRE_ID))
                .fetch_one(&pool)
                .await?;
        assert_eq!(saved_level, 3);

        let restored_inner = tq_db::character::Character::by_id(&pool, database_character_id as i32).await?;
        let restored = Character::new(character.owner(), restored_inner);
        restored.load_skills(&pool, &catalog).await?;
        assert_eq!(
            restored
                .skills()
                .iter()
                .find(|skill| skill.skill_id == crate::skills::BASIC_FIRE_ID)
                .map(|skill| skill.level),
            Some(3)
        );
        Ok(())
    }

    #[tokio::test]
    async fn practicing_a_skill_persists_experience_and_proficiency() -> Result<(), Error> {
        let (pool, character, database_character_id) = test_character().await?;
        let catalog = crate::skills::SkillCatalog::default();
        character.set_class(&pool, 100).await?;
        character.load_skills(&pool, &catalog).await?;

        let progress = character
            .practice_skill(&pool, crate::skills::BASIC_FIRE_ID, 2)
            .await?
            .expect("starter skill should be available");
        assert_eq!(progress.level, 1);
        assert_eq!(progress.experience, 2);
        assert_eq!(progress.proficiency, 2);

        let (level, experience, proficiency): (i64, i64, i64) = sqlx::query_as(
            "SELECT level, experience, proficiency
             FROM character_skills WHERE character_id = $1 AND skill_id = $2",
        )
        .bind(i64::from(database_character_id))
        .bind(i64::from(crate::skills::BASIC_FIRE_ID))
        .fetch_one(&pool)
        .await?;
        assert_eq!((level, experience, proficiency), (1, 2, 2));
        Ok(())
    }

    #[tokio::test]
    async fn skill_book_consumption_and_learning_are_atomic() -> Result<(), Error> {
        let (pool, character, database_character_id) = test_character().await?;
        insert_inventory_item(&pool, database_character_id, 701, crate::skills::WHIRLWIND_BOOK_ITEM, 0).await?;
        character.load_inventory(&pool).await?;

        let (consumed, progress) = character
            .learn_skill_from_item(&pool, 701, crate::skills::WHIRLWIND_ID)
            .await?
            .expect("skill book should be consumed");
        assert_eq!(consumed.item_type, crate::skills::WHIRLWIND_BOOK_ITEM);
        assert_eq!(progress.skill_id, crate::skills::WHIRLWIND_ID);
        assert!(character.has_skill(crate::skills::WHIRLWIND_ID));
        assert!(character.inventory_items().iter().all(|item| item.uid != 701));

        let item_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM inventory_items WHERE item_uid = 701 AND character_id = $1")
                .bind(i64::from(database_character_id))
                .fetch_one(&pool)
                .await?;
        let skill_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM character_skills WHERE character_id = $1 AND skill_id = $2")
                .bind(i64::from(database_character_id))
                .bind(i64::from(crate::skills::WHIRLWIND_ID))
                .fetch_one(&pool)
                .await?;
        assert_eq!(item_count, 0);
        assert_eq!(skill_count, 1);
        assert!(character
            .learn_skill_from_item(&pool, 701, crate::skills::WHIRLWIND_ID)
            .await?
            .is_none());
        Ok(())
    }

    #[tokio::test]
    async fn equipping_with_swap_preserves_one_row_per_item() -> Result<(), Error> {
        let (pool, character, database_character_id) = test_character().await?;
        insert_inventory_item(&pool, database_character_id, 501, 410001, 0).await?;
        insert_equipment_item(&pool, database_character_id, 601, 111001, 4).await?;
        character.load_inventory(&pool).await?;
        character.load_equipment(&pool).await?;

        let Some((equipped, swapped)) = character.equip_inventory_item(&pool, 501, 4).await? else {
            panic!("valid equipment transition should succeed");
        };
        assert_eq!(equipped.uid, 501);
        assert_eq!(equipped.position, 4);
        assert_eq!(swapped.map(|item| item.uid), Some(601));
        assert_eq!(character.equipment_item(4).map(|item| item.uid), Some(501));
        assert_eq!(
            character
                .inventory_items()
                .iter()
                .map(|item| item.uid)
                .collect::<Vec<_>>(),
            vec![601]
        );

        let item_rows: i64 = sqlx::query_scalar(
            "SELECT (SELECT COUNT(*) FROM inventory_items WHERE item_uid = 601) \
             + (SELECT COUNT(*) FROM equipment_items WHERE item_uid = 501)",
        )
        .fetch_one(&pool)
        .await?;
        assert_eq!(item_rows, 2);
        Ok(())
    }

    #[tokio::test]
    async fn item_plus_persists_across_reload_equipment_and_warehouse() -> Result<(), Error> {
        let (pool, character, database_character_id) = test_character().await?;
        insert_inventory_item(&pool, database_character_id, 501, 410001, 0).await?;
        character.load_inventory(&pool).await?;

        for expected in 1..=3 {
            let ItemUpgradeOutcome::Upgraded(item) = character.upgrade_item(&pool, 501).await? else {
                panic!("owned item should upgrade");
            };
            assert_eq!(item.plus, expected);
        }
        let saved_plus: i64 = sqlx::query_scalar("SELECT plus FROM item_upgrades WHERE item_uid = 501")
            .fetch_one(&pool)
            .await?;
        assert_eq!(saved_plus, 3);

        character.load_inventory(&pool).await?;
        assert_eq!(character.inventory_items()[0].plus, 3);
        let Some((equipped, _)) = character.equip_inventory_item(&pool, 501, 4).await? else {
            panic!("item should equip");
        };
        assert_eq!(equipped.plus, 3);
        character.load_equipment(&pool).await?;
        assert_eq!(character.equipment_item(4).expect("equipped item").plus, 3);

        let unequipped = character
            .unequip_item(&pool, 501, 0)
            .await?
            .expect("item should unequip");
        assert_eq!(unequipped.plus, 3);
        let Some((_removed, stored)) = character.store_inventory_item(&pool, 501, 2).await? else {
            panic!("item should move to warehouse");
        };
        assert_eq!(stored.plus, 3);
        character.load_warehouse(&pool).await?;
        assert_eq!(character.warehouse_items()[0].plus, 3);

        let Some((_stored, withdrawn)) = character.withdraw_warehouse_item(&pool, stored.uid, 0).await? else {
            panic!("item should return from warehouse");
        };
        assert_eq!(withdrawn.plus, 3);
        Ok(())
    }

    #[tokio::test]
    async fn item_upgrade_consumes_one_meteor_atomically() -> Result<(), Error> {
        let (pool, character, database_character_id) = test_character().await?;
        insert_inventory_item(&pool, database_character_id, 501, 410001, 0).await?;
        insert_inventory_item(&pool, database_character_id, 502, 1088001, 1).await?;
        character.load_inventory(&pool).await?;

        let (outcome, catalyst) = character.upgrade_item_with_meteor(&pool, 501).await?;
        let ItemUpgradeOutcome::Upgraded(item) = outcome else {
            panic!("item with a Meteor should upgrade");
        };
        assert_eq!(item.plus, 1);
        assert_eq!(catalyst.iter().map(|item| item.uid).collect::<Vec<_>>(), vec![502]);
        assert!(character.inventory_items().iter().all(|item| item.uid != 502));
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM inventory_items WHERE item_uid = 502")
                .fetch_one(&pool)
                .await?,
            0
        );

        let (outcome, catalyst) = character.upgrade_item_with_meteor(&pool, 501).await?;
        assert_eq!(outcome, ItemUpgradeOutcome::MissingMeteor);
        assert!(catalyst.is_empty());
        assert_eq!(character.item_in_memory(501).expect("item remains owned").plus, 1);
        Ok(())
    }

    #[tokio::test]
    async fn custom_rarity_is_independent_and_persists_across_warehouse_moves() -> Result<(), Error> {
        let (pool, character, database_character_id) = test_character().await?;
        insert_inventory_item(&pool, database_character_id, 501, 410001, 0).await?;
        character.load_inventory(&pool).await?;

        let item = character
            .set_item_rarity(&pool, 501, 3)
            .await?
            .expect("owned item should receive custom rarity");
        assert_eq!(item.rarity, 3);
        assert_eq!(item.plus, 0, "rarity must not change classic plus");
        let persisted: (i64, i64) = sqlx::query_as("SELECT plus, rarity FROM item_upgrades WHERE item_uid = 501")
            .fetch_one(&pool)
            .await?;
        assert_eq!(persisted, (0, 3));

        let Some((_removed, stored)) = character.store_inventory_item(&pool, 501, 2).await? else {
            panic!("item should move to warehouse");
        };
        assert_eq!(stored.rarity, 3);
        character.load_warehouse(&pool).await?;
        assert_eq!(character.warehouse_items()[0].rarity, 3);

        let Some((_stored, withdrawn)) = character.withdraw_warehouse_item(&pool, stored.uid, 0).await? else {
            panic!("item should return from warehouse");
        };
        assert_eq!(withdrawn.rarity, 3);
        Ok(())
    }

    #[tokio::test]
    async fn rarity_bonus_reroll_consumes_one_dragonball_and_persists_seed() -> Result<(), Error> {
        let (pool, character, database_character_id) = test_character().await?;
        insert_inventory_item(&pool, database_character_id, 501, 410001, 0).await?;
        insert_inventory_item(&pool, database_character_id, 701, 1_088_000, 1).await?;
        character.load_inventory(&pool).await?;
        character
            .set_item_rarity(&pool, 501, 1)
            .await?
            .expect("item should become Rare");

        let (outcome, catalyst) = character.reroll_item_rarity_bonus_with_dragonball(&pool, 501).await?;
        let RarityBonusOutcome::Rerolled(item) = outcome else {
            panic!("Rare item should be rerollable");
        };
        assert!(item.rarity_roll > 0);
        assert_eq!(catalyst.expect("one DragonBall should be consumed").uid, 701);
        assert_eq!(character.item_in_memory(501).expect("item remains owned").rarity, 1);
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM inventory_items WHERE item_uid = 701")
                .fetch_one(&pool)
                .await?,
            0
        );
        character.load_inventory(&pool).await?;
        assert_eq!(character.inventory_items()[0].rarity_roll, item.rarity_roll);
        Ok(())
    }

    #[tokio::test]
    async fn consumable_bundle_opens_atomically_into_direct_items() -> Result<(), Error> {
        let (pool, character, database_character_id) = test_character().await?;
        insert_inventory_item(&pool, database_character_id, 701, 720010, 0).await?;
        character.load_inventory(&pool).await?;

        let Some((consumed, created)) = character
            .open_consumable_bundle(&pool, 701, 720010, 1_000_030, 3)
            .await?
        else {
            panic!("bundle should open with three free slots");
        };
        assert_eq!(consumed.uid, 701);
        assert_eq!(created.len(), 3);
        assert!(created.iter().all(|item| item.item_type == 1_000_030));
        assert!(!character.inventory_items().iter().any(|item| item.uid == 701));
        assert_eq!(character.inventory_items().len(), 3);

        let bundle_rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM inventory_items WHERE item_type = 720010")
            .fetch_one(&pool)
            .await?;
        let direct_rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM inventory_items WHERE item_type = 1000030")
            .fetch_one(&pool)
            .await?;
        assert_eq!(bundle_rows, 0);
        assert_eq!(direct_rows, 3);
        Ok(())
    }

    #[tokio::test]
    async fn full_inventory_does_not_consume_a_bundle() -> Result<(), Error> {
        let (pool, character, database_character_id) = test_character().await?;
        insert_inventory_item(&pool, database_character_id, 701, 720010, 0).await?;
        for position in 1_u8..40 {
            insert_inventory_item(
                &pool,
                database_character_id,
                800 + u32::from(position),
                1_000_000,
                position,
            )
            .await?;
        }
        character.load_inventory(&pool).await?;
        let before = character.inventory_items();
        assert!(character
            .open_consumable_bundle(&pool, 701, 720010, 1_000_030, 3)
            .await?
            .is_none());
        assert_eq!(character.inventory_items(), before);
        let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM inventory_items")
            .fetch_one(&pool)
            .await?;
        assert_eq!(total, 40);
        Ok(())
    }

    #[tokio::test]
    async fn double_experience_is_authoritative_and_expires() -> Result<(), Error> {
        let (_pool, character, _database_character_id) = test_character().await?;
        character.activate_double_experience(Duration::from_secs(60));
        assert!(character.double_experience_active());
        assert!(character.entity().flags().contains(crate::entities::Flags::DOUBLE_EXPERIENCE));
        // 10 doubled, times the server rate of 5.
        character.grant_experience(10);
        assert_eq!(character.experience(), 100);

        character.activate_double_experience(Duration::ZERO);
        assert!(!character.double_experience_active());
        assert!(!character.entity().flags().contains(crate::entities::Flags::DOUBLE_EXPERIENCE));
        character.grant_experience(2);
        assert_eq!(character.experience(), 110);
        Ok(())
    }

    #[tokio::test]
    async fn attribute_points_are_spent_atomically_and_persisted() -> Result<(), Error> {
        let (pool, original, database_character_id) = test_character().await?;
        sqlx::query("UPDATE characters SET attribute_points = 5")
            .execute(&pool)
            .await?;
        let inner = tq_db::character::Character::by_id(&pool, database_character_id as i32).await?;
        let character = Character::new(original.owner(), inner);
        let old_health = character.maximum_health_points();
        let start_vitality = character.vitality();
        let outcome = character.allocate_attribute(&pool, AttributeKind::Vitality, 2).await?;
        assert_eq!(
            outcome,
            AttributeAllocation::Allocated {
                kind: AttributeKind::Vitality,
                amount: 2,
                remaining: 3,
            }
        );
        assert_eq!(character.vitality(), start_vitality + 2);
        assert_eq!(character.attribute_points(), 3);
        assert_eq!(character.maximum_health_points(), old_health + 48);

        let rejected = character.allocate_attribute(&pool, AttributeKind::Strength, 4).await?;
        assert_eq!(rejected, AttributeAllocation::InsufficientPoints { available: 3 });
        character.save(&crate::State::with_pool(pool.clone()).await?).await?;
        let persisted = tq_db::character::Character::by_id(&pool, database_character_id as i32).await?;
        assert_eq!(persisted.vitality as u16, start_vitality + 2);
        assert_eq!(persisted.attribute_points, 3);
        Ok(())
    }

    #[tokio::test]
    async fn level_up_follows_the_class_table_before_rebirth() -> Result<(), Error> {
        let (_pool, character, _database_character_id) = test_character().await?;
        assert_eq!(character.attribute_points(), 0);
        let life_before = character.maximum_health_points();
        // Level 1 needs 120 experience; 24 at the rate of 5.
        assert_eq!(character.grant_experience(24), Some(2));
        // A Trojan at level 2: the stats table, no free points.
        assert_eq!(character.attribute_points(), 0);
        assert_eq!(
            Some((character.strength(), character.vitality(), character.agility(), character.spirit())),
            crate::stats::for_level(character.current_class(), 2)
        );
        assert!(character.maximum_health_points() >= life_before);
        assert_eq!(character.health_points(), character.maximum_health_points());
        Ok(())
    }

    #[tokio::test]
    async fn double_experience_is_restored_from_persisted_expiration() -> Result<(), Error> {
        let (pool, character, database_character_id) = test_character().await?;
        character.activate_double_experience(Duration::from_secs(60));
        character.activate_heaven_blessing(Duration::from_secs(60));
        let state = crate::State::with_pool(pool).await?;
        character.save(&state).await?;
        let restored_inner = tq_db::character::Character::by_id(state.pool(), database_character_id as i32).await?;
        let restored = Character::new(character.owner(), restored_inner);
        assert!(restored.double_experience_active());
        assert_eq!(restored.experience_multiplier(), 2);
        character.activate_experience_boost(5, Duration::from_secs(60));
        character.save(&state).await?;
        let boosted = tq_db::character::Character::by_id(state.pool(), database_character_id as i32).await?;
        assert_eq!(Character::new(character.owner(), boosted).experience_multiplier(), 5);
        assert!(restored.entity().flags().contains(crate::entities::Flags::DOUBLE_EXPERIENCE));
        assert!(restored.heaven_blessing_active());
        assert!(restored
            .entity()
            .flags()
            .contains(crate::entities::Flags::HEAVEN_BLESSING));
        Ok(())
    }

    #[tokio::test]
    async fn heaven_blessing_increases_experience_and_expires() -> Result<(), Error> {
        let (_pool, character, _database_character_id) = test_character().await?;
        character.activate_heaven_blessing(Duration::from_secs(60));
        assert!(character.heaven_blessing_active());
        assert!(character
            .entity()
            .flags()
            .contains(crate::entities::Flags::HEAVEN_BLESSING));
        // A fifth more, times the server rate of 5.
        character.grant_experience(10);
        assert_eq!(character.experience(), 60);

        character.activate_heaven_blessing(Duration::ZERO);
        assert!(!character.heaven_blessing_active());
        assert!(!character
            .entity()
            .flags()
            .contains(crate::entities::Flags::HEAVEN_BLESSING));
        character.grant_experience(10);
        assert_eq!(character.experience(), 110);
        Ok(())
    }
}
