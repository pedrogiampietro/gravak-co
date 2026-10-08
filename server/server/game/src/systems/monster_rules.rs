//! The special rules of monster AI, beyond seeing, chasing and striking
//! (`monster_ai.rs`): casters strike from afar with their magic, a monster
//! leaves alone heroes far above it and hits them softer, a weak one flees
//! once when nearly dead, a pack answers when one of its own is hit, guards
//! walk a round about their post, and the champions (world and field
//! bosses) rage, call their kind and strike all around with their magic.
//! What each monster has done of these lives here, until it dies.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use primitives::Location;

/// How far a caster's magic reaches, in cells.
pub const MAGIC_RANGE: i16 = 7;
/// A monster's magic hits for this share of its attack.
pub const MAGIC_PERCENT: u32 = 80;
/// A monster does not go after a hero this many levels above it (it still
/// fights back when hit).
pub const HUNT_LEVEL_GAP: u16 = 25;
/// A weak monster flees once, at this share of its life or less...
pub const FLEE_PERCENT: u32 = 15;
/// ...for this long, and only monsters up to this level.
pub const FLEE_TIME: Duration = Duration::from_secs(3);
pub const FLEE_MAX_LEVEL: u16 = 70;
/// Monsters of the same kind this close answer when one is hit.
pub const PACK_RADIUS: u16 = 6;
/// A monster that returns after this long or more is a champion.
pub const CHAMPION_RESPAWN_SECONDS: i32 = 1_800;
/// A champion rages under this share of its life (its blows come twice as
/// fast), and calls its kind under this one, once.
pub const ENRAGE_PERCENT: u32 = 30;
pub const SUMMON_PERCENT: u32 = 50;
pub const SUMMONED_MINIONS: usize = 3;
/// A champion's magic strikes everyone this close, this often.
pub const AREA_RADIUS: u16 = 5;
pub const AREA_EVERY: Duration = Duration::from_secs(10);
/// The magic a champion without its own casts around it (FireMeteor).
pub const CHAMPION_MAGIC: u32 = 1_180;
/// How far from its post a guard walks its round.
pub const PATROL_REACH: i16 = 6;

/// What one monster has done of the special rules in this life.
#[derive(Debug, Clone, Default)]
struct Mind {
    fled: bool,
    flee_until: Option<Instant>,
    alerted: bool,
    enraged: bool,
    summoned: bool,
    next_area: Option<Instant>,
    waypoint: usize,
}

fn minds() -> &'static Mutex<HashMap<u32, Mind>> {
    static MINDS: OnceLock<Mutex<HashMap<u32, Mind>>> = OnceLock::new();
    MINDS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn with<T>(monster_id: u32, f: impl FnOnce(&mut Mind) -> T) -> T {
    let mut minds = minds().lock().unwrap();
    f(minds.entry(monster_id).or_default())
}

/// The monster died or was taken away: its next life starts afresh.
pub fn forget(monster_id: u32) {
    minds().lock().unwrap().remove(&monster_id);
}

/// A world boss, or a monster that takes half an hour or more to return
/// (the field and labyrinth bosses).
pub fn is_champion(monster_id: u32, monster_type: &tq_db::monster::MonsterType) -> bool {
    crate::bosses::is_boss(monster_id) || monster_type.respawn_seconds >= CHAMPION_RESPAWN_SECONDS
}

/// Whether a monster of `monster_level` leaves a hero of `hero_level` alone.
pub fn ignores(monster_level: u16, hero_level: u16) -> bool {
    hero_level >= monster_level.saturating_add(HUNT_LEVEL_GAP)
}

/// How hard a monster hits a hero, in percent, by their levels: three less
/// for each level the hero is more than five above it (down to 30), two
/// more for each the monster is more than five above the hero (up to 150).
pub fn level_damage_percent(monster_level: u16, hero_level: u16) -> u32 {
    let above = u32::from(hero_level.saturating_sub(monster_level));
    let below = u32::from(monster_level.saturating_sub(hero_level));
    if above > 5 {
        100_u32.saturating_sub((above - 5) * 3).max(30)
    } else if below > 5 {
        (100 + (below - 5) * 2).min(150)
    } else {
        100
    }
}

fn percent_of(current: u16, max: u16) -> u32 {
    u32::from(current) * 100 / u32::from(max.max(1))
}

/// Whether the monster is fleeing now.
pub fn fleeing(monster_id: u32, now: Instant) -> bool {
    with(monster_id, |mind| mind.flee_until.is_some_and(|until| until > now))
}

/// A weak monster nearly dead starts to flee, once a life.
pub fn start_flee(monster_id: u32, level: u16, life: (u16, u16), now: Instant) -> bool {
    if level > FLEE_MAX_LEVEL || life.0 == 0 || percent_of(life.0, life.1) > FLEE_PERCENT {
        return false;
    }
    with(monster_id, |mind| {
        if mind.fled {
            return false;
        }
        mind.fled = true;
        mind.flee_until = Some(now + FLEE_TIME);
        true
    })
}

/// A cell `reach` cells from `from`, away from `threat`.
pub fn away_from(from: Location, threat: Location, reach: i16) -> Location {
    let dx = (from.x as i16 - threat.x as i16).signum();
    let dy = (from.y as i16 - threat.y as i16).signum();
    let (dx, dy) = if dx == 0 && dy == 0 { (1, 0) } else { (dx, dy) };
    Location::new(
        (from.x as i16 + dx * reach).max(0) as u16,
        (from.y as i16 + dy * reach).max(0) as u16,
        0,
    )
}

/// The first time a monster is fought in a while: its pack should answer.
/// `false` while it already called them.
pub fn alert(monster_id: u32, provoked: bool) -> bool {
    with(monster_id, |mind| {
        if !provoked {
            mind.alerted = false;
            return false;
        }
        !std::mem::replace(&mut mind.alerted, true)
    })
}

/// A champion falling under [`ENRAGE_PERCENT`]: `true` the moment it rages.
pub fn enrage(monster_id: u32, life: (u16, u16)) -> bool {
    if percent_of(life.0, life.1) >= ENRAGE_PERCENT {
        return false;
    }
    with(monster_id, |mind| !std::mem::replace(&mut mind.enraged, true))
}

pub fn enraged(monster_id: u32) -> bool {
    with(monster_id, |mind| mind.enraged)
}

/// A champion falling under [`SUMMON_PERCENT`]: `true` the moment it calls
/// its kind.
pub fn summon(monster_id: u32, life: (u16, u16)) -> bool {
    if percent_of(life.0, life.1) >= SUMMON_PERCENT {
        return false;
    }
    with(monster_id, |mind| !std::mem::replace(&mut mind.summoned, true))
}

/// Whether a champion's magic strikes around it now; the next in
/// [`AREA_EVERY`].
pub fn area_due(monster_id: u32, now: Instant) -> bool {
    with(monster_id, |mind| match mind.next_area {
        Some(next) if next > now => false,
        _ => {
            mind.next_area = Some(now + AREA_EVERY);
            true
        },
    })
}

/// The corners of a guard's round about its post.
pub fn patrol_round(home: Location) -> [Location; 4] {
    let at = |dx: i16, dy: i16| Location::new((home.x as i16 + dx).max(0) as u16, (home.y as i16 + dy).max(0) as u16, 0);
    [
        at(PATROL_REACH, 0),
        at(0, PATROL_REACH),
        at(-PATROL_REACH, 0),
        at(0, -PATROL_REACH),
    ]
}

/// Where a guard walks next: the corner it heads to, the next one once it
/// is there.
pub fn patrol_target(monster_id: u32, home: Location, at: Location) -> Location {
    let round = patrol_round(home);
    with(monster_id, |mind| {
        let target = round[mind.waypoint % round.len()];
        if at.x.abs_diff(target.x).max(at.y.abs_diff(target.y)) <= 1 {
            mind.waypoint = (mind.waypoint + 1) % round.len();
        }
        round[mind.waypoint % round.len()]
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heroes_far_above_are_left_alone_and_hit_softer() {
        assert!(!ignores(50, 74) && ignores(50, 75));
        assert_eq!(level_damage_percent(50, 55), 100);
        assert_eq!(level_damage_percent(50, 60), 85);
        assert_eq!(level_damage_percent(10, 120), 30);
        assert_eq!(level_damage_percent(70, 60), 110);
        assert_eq!(level_damage_percent(130, 15), 150);
    }

    #[test]
    fn a_weak_monster_flees_once_when_nearly_dead() {
        let now = Instant::now();
        assert!(!start_flee(9_000_001, 30, (50, 100), now), "not hurt enough");
        assert!(!start_flee(9_000_001, 90, (5, 100), now), "too strong to flee");
        assert!(start_flee(9_000_001, 30, (10, 100), now));
        assert!(fleeing(9_000_001, now + Duration::from_secs(1)));
        assert!(!fleeing(9_000_001, now + FLEE_TIME));
        assert!(!start_flee(9_000_001, 30, (5, 100), now), "only once a life");
        forget(9_000_001);
        assert!(start_flee(9_000_001, 30, (5, 100), now), "a new life");
        forget(9_000_001);
    }

    #[test]
    fn packs_answer_once_and_champions_rage_and_call_once() {
        assert!(alert(9_000_002, true));
        assert!(!alert(9_000_002, true));
        assert!(!alert(9_000_002, false));
        assert!(alert(9_000_002, true), "a new fight calls them again");
        assert!(!enrage(9_000_002, (40, 100)));
        assert!(enrage(9_000_002, (29, 100)) && !enrage(9_000_002, (10, 100)));
        assert!(summon(9_000_002, (49, 100)) && !summon(9_000_002, (10, 100)));
        let now = Instant::now();
        assert!(area_due(9_000_002, now) && !area_due(9_000_002, now + Duration::from_secs(5)));
        assert!(area_due(9_000_002, now + AREA_EVERY));
        forget(9_000_002);
    }

    #[test]
    fn guards_walk_their_round() {
        let home = Location::new(100, 100, 0);
        let first = patrol_target(9_000_003, home, home);
        assert_eq!((first.x, first.y), (106, 100));
        let second = patrol_target(9_000_003, home, Location::new(106, 100, 0));
        assert_eq!((second.x, second.y), (100, 106));
        let away = away_from(Location::new(10, 10, 0), Location::new(12, 10, 0), 5);
        assert_eq!((away.x, away.y), (5, 10));
        forget(9_000_003);
    }
}
