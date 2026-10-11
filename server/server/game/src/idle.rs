//! The idle hunt: Gravak hunting on while the player is away.
//!
//! The client's helper tells the server where it hunts (`MsgIdle` HUNT):
//! the spot, its reach and how far above the hero's level it fights. A
//! character who logs out hunting leaves that spot behind
//! (`character_idle`); back in, the time away (eight hours at most) is
//! hunted there at half the pace of a hunt watched over: the monsters that
//! spawn within reach, each felled in the blows the hero's attack needs
//! and no faster than they come back, with the experience, silver and drops
//! a kill gives in the world. What is worth keeping goes into the bag while
//! it has room; the rest is sold. A report tells the player what happened.
//!
//! The helper's merchant sells, repairs and buys potions without a shop at
//! hand, at the shops' own prices (`MsgIdle` SELL, REPAIR, BUY); its porter
//! carries the valuables gathered to the depot (`MsgIdle` DEPOSIT).

use std::collections::{BTreeMap, HashMap};
use std::sync::OnceLock;

use parking_lot::Mutex;

use crate::entities::{Character, InventoryItem, PurchaseOutcome, RepairOutcome, SaleOutcome};
use crate::packets::{MsgIdleLine, MsgIdleReport, MsgItemInfo, MsgTalk, MsgUserInfo, TalkChannel};
use crate::{Error, State};

/// The longest time away that is hunted.
pub const MOST_SECONDS: u64 = 8 * 60 * 60;
/// Less than this away is not worth a report.
const LEAST_SECONDS: u64 = 60;
/// Away, the hero hunts at this share of the pace of a hunt watched over.
const AWAY_PACE: f64 = 0.5;
/// Between two kills: finding the next one and walking to it.
const WALK_SECONDS: f64 = 2.0;
/// A monster that takes longer than this to fell is left alone.
const MOST_KILL_SECONDS: f64 = 120.0;
/// Kills worked out one by one at most (eight hours at a kill a second, at
/// half pace, fits).
const MOST_KILLS: u64 = 20_000;
/// The merchant buys this many of a potion at most at a time.
const MOST_BOUGHT: u32 = 20;
/// The bag's slots, and the depot's.
const BAG_SLOTS: u8 = 40;
const DEPOT_SLOTS: u8 = 40;

/// Where a character hunts, as its helper told.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Spot {
    pub map_id: u32,
    pub x: u16,
    pub y: u16,
    pub radius: u16,
    pub levels_above: u16,
}

fn spots() -> &'static Mutex<HashMap<u32, Spot>> {
    static SPOTS: OnceLock<Mutex<HashMap<u32, Spot>>> = OnceLock::new();
    SPOTS.get_or_init(Default::default)
}

/// The helper hunts at `spot`, or (`None`) stopped.
pub fn set_spot(character_id: u32, spot: Option<Spot>) {
    let mut spots = spots().lock();
    match spot {
        Some(spot) => spots.insert(character_id, spot),
        None => spots.remove(&character_id),
    };
}

/// Logging out hunting (and alive) leaves the spot behind.
pub async fn logged_out(state: &State, character: &Character) -> Result<(), Error> {
    let Some(spot) = spots().lock().remove(&character.id()) else {
        return Ok(());
    };
    if !character.entity().is_alive() {
        return Ok(());
    }
    sqlx::query(
        "INSERT INTO character_idle (character_id, map_id, x, y, radius, levels_above, since_unix)
         VALUES ($1, $2, $3, $4, $5, $6, $7)
         ON CONFLICT (character_id) DO UPDATE SET map_id = $2, x = $3, y = $4, radius = $5,
           levels_above = $6, since_unix = $7",
    )
    .bind(i64::from(character.database_id()))
    .bind(i64::from(spot.map_id))
    .bind(i64::from(spot.x))
    .bind(i64::from(spot.y))
    .bind(i64::from(spot.radius))
    .bind(i64::from(spot.levels_above))
    .bind(now_unix())
    .execute(state.pool())
    .await?;
    tracing::info!(target: "idle", character = %character.entity().name(), map = spot.map_id, "Left hunting");
    Ok(())
}

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs() as i64)
}

// ------------------------------------------------------------ the hunt

/// The hero as the hunt sees them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hunter {
    pub level: u16,
    pub attack_min: u32,
    pub attack_max: u32,
    /// Between two blows.
    pub attack_ms: u32,
}

/// A kind of monster around the spot, and how many spawn there.
#[derive(Debug, Clone, PartialEq)]
pub struct Prey {
    pub type_id: u32,
    pub name: String,
    pub level: u16,
    pub life: u32,
    pub defense: u32,
    pub respawn_seconds: u32,
    pub count: u32,
}

/// How long the hero takes to fell one: its blows, and the walk to it.
pub fn kill_seconds(hunter: &Hunter, prey: &Prey) -> f64 {
    let blow = ((hunter.attack_min + hunter.attack_max) / 2).saturating_sub(prey.defense).max(1);
    let blows = prey.life.max(1).div_ceil(blow);
    f64::from(blows) * f64::from(hunter.attack_ms.max(100)) / 1000.0 + WALK_SECONDS
}

/// How many of each prey (by its index) `seconds` away fell: the hero
/// takes them as they come, by their numbers, at the away pace; never
/// faster than they come back; never one too strong to fell.
pub fn hunt(hunter: &Hunter, prey: &[Prey], seconds: u64) -> Vec<(usize, u64)> {
    let takeable: Vec<(usize, &Prey, f64)> = prey
        .iter()
        .enumerate()
        .map(|(index, one)| (index, one, kill_seconds(hunter, one)))
        .filter(|(_, one, time)| one.count > 0 && *time <= MOST_KILL_SECONDS)
        .collect();
    let count: u64 = takeable.iter().map(|(_, one, _)| u64::from(one.count)).sum();
    if count == 0 || seconds == 0 {
        return Vec::new();
    }
    let average = takeable.iter().map(|(_, one, time)| f64::from(one.count) * time).sum::<f64>() / count as f64;
    let kills = (seconds as f64 * AWAY_PACE / average) as u64;
    takeable
        .into_iter()
        .map(|(index, one, _)| {
            let share = kills * u64::from(one.count) / count;
            // Each comes back once in its respawn time.
            let comes_back = u64::from(one.count) * (seconds / u64::from(one.respawn_seconds.max(1)) + 1);
            (index, share.min(comes_back))
        })
        .filter(|(_, kills)| *kills > 0)
        .collect()
}

/// The experience one kill gives: the life dealt, scaled by the levels
/// between them, and the kill's own share (`reward_monster_defeat`).
pub fn kill_experience(hunter_level: u16, prey: &Prey) -> u64 {
    crate::leveling::combat_experience(hunter_level, prey.level, prey.life, prey.life, false)
        + u64::from(prey.level.max(1)) * 25
}

/// A drop worth keeping: anything but gear (gems, DragonBalls, Meteors,
/// potions, materials). Gear is sold.
pub fn kept(item_type: u32) -> bool {
    item_type >= 1_000_000
}

/// The monsters around `spot` the hero fights: within its reach (and their
/// spawn's), no more than `levels_above` over the hero; never the guards.
async fn prey_at(state: &State, spot: &Spot, hero_level: u16) -> Result<Vec<Prey>, Error> {
    let spawns = tq_db::monster::MonsterSpawn::by_map(state.pool(), spot.map_id as i32).await?;
    let near: Vec<_> = spawns
        .into_iter()
        .filter(|spawn| {
            let reach = i32::from(spot.radius) + spawn.radius;
            (i32::from(spawn.x) - i32::from(spot.x)).abs() <= reach && (i32::from(spawn.y) - i32::from(spot.y)).abs() <= reach
        })
        .collect();
    let ids: Vec<i32> = near.iter().map(|spawn| spawn.monster_type_id).collect();
    let types = tq_db::monster::MonsterType::by_ids(state.pool(), &ids).await?;
    let most_level = hero_level.saturating_add(spot.levels_above);
    let mut prey: BTreeMap<i32, Prey> = BTreeMap::new();
    for spawn in near {
        let Some(kind) = types.iter().find(|kind| kind.id == spawn.monster_type_id) else {
            continue;
        };
        let level = kind.level.clamp(0, i32::from(u16::MAX)) as u16;
        if level > most_level || kind.name.starts_with("Guard") || kind.life <= 0 {
            continue;
        }
        prey.entry(kind.id)
            .or_insert_with(|| Prey {
                type_id: kind.id as u32,
                name: kind.name.clone(),
                level,
                life: kind.life as u32,
                defense: kind.defense.max(0) as u32,
                respawn_seconds: kind.respawn_seconds.max(1) as u32,
                count: 0,
            })
            .count += spawn.amount.max(0) as u32;
    }
    Ok(prey.into_values().collect())
}

/// What the time away brought.
#[derive(Debug, Default)]
struct Haul {
    kills: BTreeMap<String, u64>,
    experience: u64,
    silver: u64,
    items: BTreeMap<u32, u32>,
}

/// Back in: the time away hunted where the character left, given and told.
pub async fn welcome_back(state: &State, character: &Character) -> Result<(), Error> {
    let row: Option<(i64, i64, i64, i64, i64, i64)> = sqlx::query_as(
        "DELETE FROM character_idle WHERE character_id = $1
         RETURNING map_id, x, y, radius, levels_above, since_unix",
    )
    .bind(i64::from(character.database_id()))
    .fetch_optional(state.pool())
    .await?;
    let Some((map_id, x, y, radius, levels_above, since)) = row else {
        return Ok(());
    };
    let seconds = (now_unix() - since).clamp(0, MOST_SECONDS as i64) as u64;
    if seconds < LEAST_SECONDS {
        return Ok(());
    }
    let spot = Spot {
        map_id: map_id as u32,
        x: x.clamp(0, i64::from(u16::MAX)) as u16,
        y: y.clamp(0, i64::from(u16::MAX)) as u16,
        radius: radius.clamp(1, 30) as u16,
        levels_above: levels_above.clamp(0, 20) as u16,
    };
    let level = character.entity().level();
    let prey = prey_at(state, &spot, level).await?;
    let stats = character.combat_stats_with_catalog(Some(state.shop_catalog()));
    let hunter = Hunter {
        level,
        attack_min: stats.min_attack,
        attack_max: stats.max_attack,
        attack_ms: u32::from(character.attack_speed_with_catalog(Some(state.shop_catalog()))),
    };
    let mut haul = Haul::default();
    let mut budget = MOST_KILLS;
    for (index, kills) in hunt(&hunter, &prey, seconds) {
        let one = &prey[index];
        let kills = kills.min(budget);
        budget -= kills;
        *haul.kills.entry(one.name.clone()).or_default() += kills;
        haul.experience += kill_experience(level, one) * kills;
        for _ in 0..kills {
            let roll = crate::combat::next_roll;
            if let Some(silver) = crate::drops::roll_silver(u32::from(one.level.max(1)), roll(), roll()) {
                haul.silver += u64::from(silver);
            }
            let drops = state
                .drop_catalog()
                .roll(one.type_id, one.level)
                .into_iter()
                .chain(state.drop_catalog().roll_rares())
                .chain(crate::drops::roll_extras(one.level, roll, |item| state.shop_catalog().template(item).is_some()));
            for item in drops {
                *haul.items.entry(item).or_default() += 1;
            }
        }
    }
    give(state, character, seconds, haul).await
}

/// Gives the haul and tells it: the experience, the silver, what is kept
/// while the bag has room, the rest sold.
async fn give(state: &State, character: &Character, seconds: u64, haul: Haul) -> Result<(), Error> {
    let owner = character.owner();
    let level_before = character.entity().level();
    let experience_before = experience_total(character);
    let silver_before = character.silver();
    if haul.experience > 0 {
        if let Some(level) = character.grant_experience(haul.experience) {
            crate::leveling::after_level_up(state, character, level).await?;
        }
    }
    let mut silver = haul.silver;
    let (mut kept_count, mut sold_count) = (0u32, 0u32);
    let mut lines: Vec<(u32, u32, u32, String)> = Vec::new();
    for (name, kills) in &haul.kills {
        lines.push((MsgIdleLine::KILLS, 0, *kills as u32, name.clone()));
    }
    for (&item_type, &amount) in &haul.items {
        let (mut into_bag, mut sold) = (0u32, 0u32);
        for _ in 0..amount {
            if kept(item_type) {
                if let Some(item) = character.grant_item(state.pool(), state.shop_catalog(), item_type, false).await? {
                    owner.send(MsgItemInfo::inventory_item(item)).await?;
                    into_bag += 1;
                    continue;
                }
            }
            let template = state.shop_catalog().template(item_type);
            let price = template.and_then(|template| {
                state.shop_catalog().sell_price(item_type, template.durability, template.max_durability)
            });
            if let Some(price) = price {
                silver += u64::from(price);
                sold += 1;
            }
        }
        if into_bag > 0 {
            lines.push((MsgIdleLine::KEPT, item_type, into_bag, String::new()));
        }
        if sold > 0 {
            lines.push((MsgIdleLine::SOLD, item_type, sold, String::new()));
        }
        kept_count += into_bag;
        sold_count += sold;
    }
    let silver = u32::try_from(silver).unwrap_or(u32::MAX);
    if silver > 0 {
        character.grant_silver(state.pool(), silver).await?;
    }
    // What was given (the server's rates and bonuses on top of the haul).
    let given = u32::try_from(character.silver().saturating_sub(silver_before)).unwrap_or(u32::MAX);
    let experience = experience_total(character).saturating_sub(experience_before);
    character.save(state).await?;
    owner.send(MsgUserInfo::from(character)).await?;
    let kills: u64 = haul.kills.values().sum();
    tracing::info!(
        target: "idle",
        character = %character.entity().name(),
        seconds,
        kills,
        experience,
        silver = given,
        "Hunted while away"
    );
    let total = u32::try_from(lines.len()).unwrap_or(u32::MAX);
    for (index, (kind, id, amount, name)) in lines.into_iter().enumerate() {
        owner
            .send(MsgIdleLine { index: index as u32, total, kind, id, amount, name })
            .await?;
    }
    owner
        .send(MsgIdleReport {
            seconds: seconds as u32,
            kills: u32::try_from(kills).unwrap_or(u32::MAX),
            experience,
            level_before: u32::from(level_before),
            level_after: u32::from(character.entity().level()),
            silver: given,
            kept: kept_count,
            sold: sold_count,
        })
        .await?;
    Ok(())
}

/// All the experience a character has earned, every level's included.
fn experience_total(character: &Character) -> u64 {
    let level = character.entity().level().clamp(1, u16::from(u8::MAX)) as u8;
    crate::leveling::total_experience(level, character.experience())
}

// ------------------------------------------------------------ the merchant

/// Whether an item may go to the merchant: nothing bound, raised or set
/// with gems; never what the hunt lives on (potions, arrows). Rare gear goes
/// only when the player's Sell All list lets its kind go (the client asks).
fn sellable(item: &InventoryItem) -> bool {
    !crate::special_items::is_bound(item.item_type)
        && !is_potion(item.item_type)
        && item.item_type / 1_000 != 1_050
        && item.plus == 0
        && item.gem_one == 0
        && item.gem_two == 0
}

/// Sells a bag item at the shops' price.
pub async fn sell(state: &State, character: &Character, uid: u32) -> Result<u32, Error> {
    let Some(item) = character.inventory_items().into_iter().find(|item| item.uid == uid) else {
        return Ok(0);
    };
    if !sellable(&item) {
        return Ok(0);
    }
    let Some(price) = state.shop_catalog().sell_price(item.item_type, item.durability, item.max_durability) else {
        return Ok(0);
    };
    match character.sell_item(state.pool(), item.uid, price).await? {
        SaleOutcome::Sold { item, silver } => {
            crate::audit::item("idle_sell", character.id(), item.uid, item.item_type, format_args!("silver={silver}"));
            crate::audit::economic("idle_sell", character.id(), 0, item.uid, silver);
            let owner = character.owner();
            owner.send(MsgItemInfo::remove_inventory_item(item)).await?;
            owner.send(MsgUserInfo::from(character)).await?;
            Ok(silver)
        },
        _ => Ok(0),
    }
}

/// Repairs what the hero wears, as far as the silver goes.
pub async fn repair(state: &State, character: &Character) -> Result<u32, Error> {
    let owner = character.owner();
    let mut paid = 0u32;
    for item in character.equipment_items() {
        if item.max_durability == 0 || item.durability >= item.max_durability {
            continue;
        }
        let Some(cost) = state.shop_catalog().repair_price(item.item_type, item.durability, item.max_durability) else {
            continue;
        };
        if let RepairOutcome::Repaired { item, worn } = character.repair_item(state.pool(), item.uid, cost).await? {
            paid = paid.saturating_add(cost);
            let info = if worn { MsgItemInfo::equipment_item(item) } else { MsgItemInfo::inventory_item(item) };
            owner.send(info).await?;
        }
    }
    if paid > 0 {
        owner.send(MsgUserInfo::from(character)).await?;
        owner
            .send(MsgTalk::from_system(character.id(), TalkChannel::TopLeft, format!("Helper: repaired for {paid} silver.")))
            .await?;
    }
    Ok(paid)
}

/// A potion the merchant sells: a life or mana potion the shops have.
pub fn is_potion(item_type: u32) -> bool {
    (1_000_000..1_003_000).contains(&item_type)
}

/// Buys up to `amount` of a potion into the free slots, as far as the
/// silver goes.
pub async fn buy(state: &State, character: &Character, item_type: u32, amount: u32) -> Result<u32, Error> {
    if !is_potion(item_type) {
        return Ok(0);
    }
    let Some(template) = state.shop_catalog().item(item_type) else {
        return Ok(0);
    };
    let owner = character.owner();
    let mut bought = 0;
    for _ in 0..amount.min(MOST_BOUGHT) {
        // Onto a stack with room, or into the first free slot.
        let taken: Vec<u8> = character.inventory_items().iter().map(|item| item.position).collect();
        let slot = (0..BAG_SLOTS).find(|slot| !taken.contains(slot)).unwrap_or(0);
        match character
            .purchase_item(state.pool(), item_type, slot, template.price, template.durability, template.max_durability)
            .await?
        {
            PurchaseOutcome::Bought(item) => {
                owner.send(MsgItemInfo::inventory_item(item)).await?;
                bought += 1;
            },
            _ => break,
        }
    }
    if bought > 0 {
        owner.send(MsgUserInfo::from(character)).await?;
        owner
            .send(MsgTalk::from_system(
                character.id(),
                TalkChannel::TopLeft,
                format!("Helper: bought {bought} potions for {} silver.", u64::from(template.price) * u64::from(bought)),
            ))
            .await?;
    }
    Ok(bought)
}

// ------------------------------------------------------------ the porter

/// What the porter carries to the depot: DragonBalls, Meteors and their
/// tears, gems and ExpBalls.
pub fn depositable(item_type: u32) -> bool {
    matches!(item_type, 1_088_000..=1_088_002 | crate::leveling::EXP_BALL) || (700_000..710_000).contains(&item_type)
}

/// Stores every bag item of `item_type` in the depot's free slots.
pub async fn deposit(state: &State, character: &Character, item_type: u32) -> Result<u32, Error> {
    if !depositable(item_type) {
        return Ok(0);
    }
    let taken: Vec<i64> = sqlx::query_scalar("SELECT position FROM warehouse_items WHERE character_id = $1")
        .bind(i64::from(character.database_id()))
        .fetch_all(state.pool())
        .await?;
    let mut free = (0..DEPOT_SLOTS).filter(|slot| !taken.contains(&i64::from(*slot)));
    let owner = character.owner();
    let mut stored = 0u32;
    let items: Vec<InventoryItem> =
        character.inventory_items().into_iter().filter(|item| item.item_type == item_type).collect();
    for item in items {
        let Some(slot) = free.next() else {
            break;
        };
        if let Some((removed, _)) = character.store_inventory_item(state.pool(), item.uid, slot).await? {
            crate::audit::item("idle_deposit", character.id(), removed.uid, removed.item_type, format_args!("slot={slot}"));
            owner.send(MsgItemInfo::remove_inventory_item(removed)).await?;
            stored += 1;
        }
    }
    let words = if stored > 0 {
        format!("Helper: {stored} stored in the depot.")
    } else {
        "Helper: the depot is full.".to_owned()
    };
    owner.send(MsgTalk::from_system(character.id(), TalkChannel::TopLeft, words)).await?;
    Ok(stored)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hunter() -> Hunter {
        Hunter { level: 20, attack_min: 90, attack_max: 110, attack_ms: 1_000 }
    }

    fn prey(name: &str, level: u16, life: u32, defense: u32, count: u32) -> Prey {
        Prey { type_id: 1, name: name.into(), level, life, defense, respawn_seconds: 10, count }
    }

    #[test]
    fn a_kill_takes_its_blows_and_the_walk() {
        // 100 a blow against no defense: three blows for 250 life.
        assert_eq!(kill_seconds(&hunter(), &prey("Pheasant", 1, 250, 0, 1)), 3.0 + WALK_SECONDS);
        // Defense takes its share off each blow, and a blow is never nothing.
        assert_eq!(kill_seconds(&hunter(), &prey("Turtle", 1, 100, 50, 1)), 2.0 + WALK_SECONDS);
        assert_eq!(kill_seconds(&hunter(), &prey("Wall", 1, 10, 500, 1)), 10.0 + WALK_SECONDS);
    }

    #[test]
    fn the_time_away_is_hunted_at_half_pace_by_their_numbers() {
        // Five seconds a kill, an hour away at half pace: 360 kills, three
        // to one as they spawn.
        let found = hunt(&hunter(), &[prey("Pheasant", 1, 300, 0, 30), prey("Robin", 1, 300, 0, 10)], 3_600);
        assert_eq!(found, vec![(0, 270), (1, 90)]);
    }

    #[test]
    fn never_faster_than_they_come_back_nor_one_too_strong() {
        // One monster coming back once a minute: at most 61 in an hour.
        let slow = Prey { respawn_seconds: 60, ..prey("Lone", 1, 100, 0, 1) };
        assert_eq!(hunt(&hunter(), &[slow], 3_600), vec![(0, 61)]);
        // Far too strong to fell (millions of life): left alone.
        assert!(hunt(&hunter(), &[prey("Titan", 1, 9_000_000, 0, 5)], 3_600).is_empty());
        assert!(hunt(&hunter(), &[], 3_600).is_empty());
    }

    #[test]
    fn a_kill_gives_its_life_by_levels_and_its_share() {
        // Equal levels: the life, and 25 a level.
        assert_eq!(kill_experience(20, &prey("Robin", 20, 400, 0, 1)), 400 + 500);
        // Far below: a twentieth of the life.
        assert_eq!(kill_experience(60, &prey("Pheasant", 1, 400, 0, 1)), 20 + 25);
    }

    #[test]
    fn gear_is_sold_and_the_rest_kept() {
        assert!(!kept(410_301), "a blade");
        assert!(!kept(130_005), "an armour");
        assert!(kept(1_088_000), "a Meteor");
        assert!(kept(1_000_030), "a potion");
        assert!(is_potion(1_002_020) && !is_potion(1_088_000) && !is_potion(410_301));
    }

    #[test]
    fn the_porter_carries_only_valuables() {
        assert!(depositable(1_088_000) && depositable(1_088_001), "a DragonBall, a Meteor");
        assert!(depositable(700_013) && depositable(723_700), "a gem, an ExpBall");
        assert!(!depositable(410_301) && !depositable(1_000_010), "a blade, a potion");
    }

    #[test]
    fn the_merchant_keeps_what_the_hunt_lives_on() {
        let item = |item_type| InventoryItem { item_type, ..InventoryItem::default() };
        assert!(sellable(&item(410_301)), "a blade goes");
        assert!(!sellable(&item(1_000_010)), "a potion stays");
        assert!(!sellable(&item(1_050_000)), "arrows stay");
        assert!(!sellable(&InventoryItem { plus: 3, ..item(410_301) }), "a +3 stays");
    }
}
