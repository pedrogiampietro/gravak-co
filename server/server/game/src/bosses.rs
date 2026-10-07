//! World bosses, after the Albetros design (`Handlers/Bosses`,
//! `Monster.Kill`): each monster king of the 5065 data appears every few
//! hours where its kind roams, the whole server hears of it, it strikes
//! everyone around it, and its slayer is praised and well paid. GuildBeast
//! appears on the guild map; its slayer's guild earns a fortune.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use parking_lot::Mutex;

use crate::entities::{Character, GameEntity, Monster};
use crate::packets::{ActionType, MsgAction, MsgPlayer, MsgTalk, TalkChannel};
use crate::{Error, State};

/// Where a boss appears.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lair {
    /// Beside the biggest spawn of this monster type.
    Among(u32),
    /// At a fixed cell.
    At(u32, u16, u16),
}

/// The bosses: their monster type and lair.
pub const BOSSES: [(u32, Lair); 20] = [
    (8_300, Lair::Among(5)),  // GhostKing among the Poltergeists
    (8_301, Lair::Among(6)),  // WingedSnakeKing
    (8_302, Lair::Among(7)),  // BanditKing
    (8_303, Lair::Among(8)),  // RatKing
    (8_304, Lair::Among(9)),  // ElfBoss among the FireSpirits
    (8_305, Lair::Among(10)), // MonkeyKing among the Macaques
    (8_306, Lair::Among(11)), // GiantApeKing
    (8_307, Lair::Among(12)), // ThunderApeKing
    (8_308, Lair::Among(13)), // SnakemanKing
    (8_309, Lair::Among(14)), // SandMonsterKing
    (8_310, Lair::Among(15)), // HillMonsterKing
    (8_311, Lair::Among(16)), // RockMonsterKing
    (8_312, Lair::Among(17)), // BladeGhostKing
    (8_313, Lair::Among(18)), // BirdmanKing
    (8_314, Lair::Among(19)), // HawkBoss among the HawKings
    (8_315, Lair::Among(55)), // RobberKing among the island Bandits
    (8_316, Lair::Among(56)), // BatKing among the BloodyBats
    (8_317, Lair::Among(56)), // BloodyKing
    (8_318, Lair::Among(57)), // BullKing among the BullMonsters
    (GUILD_BEAST, Lair::At(1_038, 95, 95)),
];

/// GuildBeast (Redux 3120): its slayer's guild earns [`GUILD_BEAST_FUND`].
pub const GUILD_BEAST: u32 = 3_120;
pub const GUILD_BEAST_FUND: i64 = 1_000_000;
/// Between a boss's death and its return, at least and at most.
const RESPAWN_MIN: Duration = Duration::from_secs(2 * 60 * 60);
const RESPAWN_MAX: Duration = Duration::from_secs(4 * 60 * 60);
/// The first appearances after the server starts, at least and at most.
const FIRST_MIN: Duration = Duration::from_secs(5 * 60);
const FIRST_MAX: Duration = Duration::from_secs(60 * 60);
/// A boss's blow reaches everyone this close.
pub const AREA_BLOW: u16 = 2;

#[derive(Debug, Clone, Copy)]
struct BossState {
    next: Instant,
    /// The boss on the map now: its monster id and map.
    alive: Option<(u32, u32)>,
}

fn book() -> &'static Mutex<HashMap<u32, BossState>> {
    static BOOK: std::sync::OnceLock<Mutex<HashMap<u32, BossState>>> = std::sync::OnceLock::new();
    BOOK.get_or_init(|| {
        let now = Instant::now();
        Mutex::new(
            BOSSES
                .iter()
                .map(|(boss, _)| {
                    let next = now + between(FIRST_MIN, FIRST_MAX, crate::combat::next_roll());
                    (*boss, BossState { next, alive: None })
                })
                .collect(),
        )
    })
}

/// A duration from `low` up to `high`, picked by `roll`.
fn between(low: Duration, high: Duration, roll: u32) -> Duration {
    let span = high.saturating_sub(low).as_secs().max(1);
    low + Duration::from_secs(u64::from(roll) % span)
}

/// Whether `monster_id` is a boss on the map now.
pub fn is_boss(monster_id: u32) -> bool {
    book().lock().values().any(|state| state.alive.is_some_and(|(id, _)| id == monster_id))
}

/// What the places are called, for the announcements.
fn place_name(map_id: u32) -> &'static str {
    match map_id {
        1_002 => "Twin City",
        1_011 => "Phoenix Castle",
        1_020 => "Ape Mountain",
        1_000 => "Desert City",
        1_015 => "Bird Island",
        1_001 => "the Mystic Castle",
        1_038 => "the guild area",
        _ => "the wilds",
    }
}

/// Everyone online hears it in the middle of the screen.
pub(crate) async fn announce(state: &State, text: String) {
    let talk = MsgTalk::from_system(0, TalkChannel::Center, text);
    for entity in state.entities() {
        if let Some(character) = entity.as_character() {
            let _ = character.owner().send(talk.clone()).await;
        }
    }
}

pub(crate) async fn monster_type(state: &State, id: u32) -> Result<Option<tq_db::monster::MonsterType>, Error> {
    Ok(sqlx::query_as::<_, tq_db::monster::MonsterType>(
        "SELECT id, name, look, level, life, attack_min, attack_max, defense, attack_speed, respawn_seconds, view_range, attack_range, attack_mode, magic_resistance \
         FROM monster_types WHERE id = $1",
    )
    .bind(i64::from(id))
    .fetch_optional(state.pool())
    .await?)
}

/// Where a lair is: map and cell.
async fn lair(state: &State, lair: Lair) -> Result<Option<(u32, u16, u16)>, Error> {
    Ok(match lair {
        Lair::At(map, x, y) => Some((map, x, y)),
        Lair::Among(kind) => {
            let spawn: Option<(i64, i64, i64)> = sqlx::query_as(
                "SELECT map_id, x, y FROM monster_spawns WHERE monster_type_id = $1 ORDER BY amount DESC, id LIMIT 1",
            )
            .bind(i64::from(kind))
            .fetch_optional(state.pool())
            .await?;
            spawn.map(|(map, x, y)| (map as u32, x as u16, y as u16))
        },
    })
}

/// Puts a monster of `monster_type` on `map_id` near `(x, y)`; it never
/// respawns (a game master's or a boss). Its id, if there was room.
pub async fn spawn_monster(
    state: &State,
    map_id: u32,
    (x, y): (u16, u16),
    monster_type: &tq_db::monster::MonsterType,
    wander: i32,
) -> Result<Option<u32>, Error> {
    let map = state.try_map(map_id)?;
    map.load().await?;
    let origin = primitives::Location::new(x, y, 0);
    let Some(spot) = map.landing_position(origin, 6) else {
        return Ok(None);
    };
    let spawn = tq_db::monster::MonsterSpawn {
        id: 0,
        monster_type_id: monster_type.id,
        map_id: map_id as i32,
        x: spot.x as i16,
        y: spot.y as i16,
        amount: 1,
        radius: wander,
    };
    let id = crate::world::next_summoned_id();
    let entity = std::sync::Arc::new(GameEntity::from(Monster::new(id, monster_type.clone(), spawn)));
    map.add_monster(entity.clone()).await?;
    if let Some(monster) = entity.as_monster() {
        let _ = map.broadcast(MsgPlayer::from_monster(monster)).await;
        let _ = map.broadcast(MsgAction::new(id, 0, 0, 0, ActionType::SpawnEffect)).await;
    }
    Ok(Some(id))
}

/// Brings out the bosses whose time has come, and notices the ones gone.
pub async fn tick(state: &State) {
    let now = Instant::now();
    let due: Vec<u32> = {
        let mut book = book().lock();
        for state_of in book.values_mut() {
            // A boss taken off the map some other way comes back later.
            if let Some((id, map_id)) = state_of.alive {
                let gone = state.try_map(map_id).map_or(true, |map| map.monster_entity(id).is_none());
                if gone {
                    state_of.alive = None;
                    state_of.next = now + between(RESPAWN_MIN, RESPAWN_MAX, crate::combat::next_roll());
                }
            }
        }
        book.iter()
            .filter(|(_, state_of)| state_of.alive.is_none() && state_of.next <= now)
            .map(|(boss, _)| *boss)
            .collect()
    };
    for boss in due {
        if let Err(error) = appear(state, boss).await {
            tracing::warn!(boss, %error, "A boss could not appear");
            if let Some(state_of) = book().lock().get_mut(&boss) {
                state_of.next = now + RESPAWN_MIN;
            }
        }
    }
}

/// Brings `boss` out now; its name, when it came.
pub async fn appear(state: &State, boss: u32) -> Result<Option<String>, Error> {
    let Some((_, place)) = BOSSES.iter().find(|(id, _)| *id == boss) else {
        return Ok(None);
    };
    let (Some(kind), Some((map_id, x, y))) = (monster_type(state, boss).await?, lair(state, *place).await?) else {
        if let Some(state_of) = book().lock().get_mut(&boss) {
            state_of.next = Instant::now() + RESPAWN_MAX;
        }
        return Ok(None);
    };
    let Some(id) = spawn_monster(state, map_id, (x, y), &kind, 8).await? else {
        return Ok(None);
    };
    if let Some(state_of) = book().lock().get_mut(&boss) {
        state_of.alive = Some((id, map_id));
    }
    tracing::info!(target: "world", boss = %kind.name, map_id, x, y, "A boss appeared");
    announce(
        state,
        format!("The fearsome {} has appeared near {}! Brave souls are needed.", kind.name, place_name(map_id)),
    )
    .await;
    Ok(Some(kind.name))
}

/// The extra loot of a slain boss: a DragonBall, two Meteors and two pieces
/// of Elite or Super equipment of its level.
pub fn boss_drops(level: u16, exists: impl Fn(u32) -> bool) -> Vec<u32> {
    let mut drops = vec![crate::drops::DRAGONBALL, crate::drops::METEOR, crate::drops::METEOR];
    let mut tries = 0;
    while drops.len() < 5 && tries < 20 {
        tries += 1;
        let roll = crate::combat::next_roll();
        let gear = crate::drops::gear_id(level, crate::combat::next_roll);
        // Super one time in four, else Elite.
        let quality = if roll % 4 == 0 { 9 } else { 8 };
        let gear = gear / 10 * 10 + quality;
        if exists(gear) {
            drops.push(gear);
        }
    }
    drops
}

/// A boss fell to `slayer`: the server hears who, the boss returns in a few
/// hours, and GuildBeast pays the slayer's guild. The extra loot, if it was
/// a boss.
pub async fn slain(state: &State, slayer: &Character, monster: &Monster) -> Result<Vec<u32>, Error> {
    let was_boss = {
        let mut book = book().lock();
        let found = book
            .iter_mut()
            .find(|(_, state_of)| state_of.alive.is_some_and(|(id, _)| id == monster.id()));
        match found {
            Some((_, state_of)) => {
                state_of.alive = None;
                state_of.next = Instant::now() + between(RESPAWN_MIN, RESPAWN_MAX, crate::combat::next_roll());
                true
            },
            None => false,
        }
    };
    if !was_boss {
        return Ok(Vec::new());
    }
    let boss = &monster.monster_type().name;
    let mut line = format!("Praise {} for ridding the world of the loathsome {boss}!", slayer.entity().name());
    if monster.monster_type().id as u32 == GUILD_BEAST {
        if let Some(guild) = state.guild_manager().guild_for(slayer.id()) {
            sqlx::query("UPDATE guilds SET silver = silver + $2 WHERE guild_id = $1")
                .bind(i64::from(guild.id))
                .bind(GUILD_BEAST_FUND)
                .execute(state.pool())
                .await?;
            line = format!(
                "{} of {} slew the GuildBeast! {GUILD_BEAST_FUND} silver joins the guild's fund.",
                slayer.entity().name(),
                guild.name
            );
        }
    }
    tracing::info!(target: "world", boss = %boss, slayer = %slayer.entity().name(), "A boss was slain");
    announce(state, line).await;
    let level = u16::try_from(monster.monster_type().level.max(1)).unwrap_or(u16::MAX);
    Ok(boss_drops(level, |item| state.shop_catalog().template(item).is_some()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bosses_return_in_two_to_four_hours() {
        for roll in [0, 1, 7_199, 7_200, u32::MAX] {
            let wait = between(RESPAWN_MIN, RESPAWN_MAX, roll);
            assert!(wait >= RESPAWN_MIN && wait < RESPAWN_MAX, "{wait:?}");
        }
    }

    #[test]
    fn bosses_drop_dragonballs_meteors_and_fine_gear() {
        let drops = boss_drops(60, |_| true);
        assert_eq!(drops.len(), 5);
        assert_eq!(&drops[..3], [crate::drops::DRAGONBALL, crate::drops::METEOR, crate::drops::METEOR]);
        assert!(drops[3..].iter().all(|item| matches!(item % 10, 8 | 9)), "{drops:?}");
        assert_eq!(boss_drops(60, |item| item > 2_000_000).len(), 3, "gear that does not exist is left out");
    }

    #[test]
    fn every_boss_is_listed_once() {
        let mut ids: Vec<u32> = BOSSES.iter().map(|(id, _)| *id).collect();
        ids.dedup();
        assert_eq!(ids.len(), BOSSES.len());
    }
}
