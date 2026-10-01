use crate::combat::{next_roll, resolve_physical_hit};
use crate::entities::GameEntity;
use crate::packets::{ActionType, InteractAction, MsgAction, MsgInteract, MsgPlayer, MsgUserInfo, MsgWalk};
use crate::State;
use primitives::Location;
use std::collections::HashSet;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const VIEW_RANGE: i16 = 18;
const ATTACK_RANGE: i16 = 1;
static NEXT_MOVE: OnceLock<Mutex<std::collections::HashMap<u32, Instant>>> = OnceLock::new();
static NEXT_ATTACK: OnceLock<Mutex<std::collections::HashMap<u32, Instant>>> = OnceLock::new();
static NEXT_RESPAWN: OnceLock<Mutex<std::collections::HashMap<u32, Instant>>> = OnceLock::new();

pub async fn tick(state: &State) {
    let now = Instant::now();
    let next_move = NEXT_MOVE.get_or_init(|| Mutex::new(std::collections::HashMap::new()));
    let next_attack = NEXT_ATTACK.get_or_init(|| Mutex::new(std::collections::HashMap::new()));
    let next_respawn = NEXT_RESPAWN.get_or_init(|| Mutex::new(std::collections::HashMap::new()));
    for map in state.maps().values() {
        let monsters = map.monster_entities();
        if monsters.is_empty() {
            continue;
        }
        let characters = state
            .entities()
            .into_iter()
            .filter(|entity| {
                entity
                    .as_character()
                    .is_some_and(|character| character.entity().map_id() == map.id())
            })
            .collect::<Vec<_>>();
        // Reserve every occupied tile while processing this map. This prevents
        // a pack from selecting the same next tile and visually
        // collapsing into one another during a single AI tick.
        let mut occupied = monsters
            .iter()
            .filter_map(|entity| entity.as_monster().map(|m| m.entity().location()))
            .map(|location| (location.x, location.y))
            .collect::<HashSet<_>>();
        for monster in monsters {
            let Some(monster_ref) = monster.as_monster() else {
                continue;
            };
            if monster_ref.entity().is_dead() {
                let should_respawn = {
                    let mut respawns = next_respawn.lock().unwrap();
                    let due = respawns.entry(monster.id()).or_insert_with(|| {
                        now + Duration::from_secs(monster_ref.monster_type().respawn_seconds.max(3) as u64)
                    });
                    *due <= now
                };
                if should_respawn {
                    let home = monster_ref.home();
                    let location = if map.is_walkable(home.x, home.y) {
                        home
                    } else {
                        map.nearest_walkable_position(home, crate::world::MONSTER_RELOCATION_RADIUS)
                            .unwrap_or(home)
                    };
                    monster_ref.respawn_at(location);
                    next_respawn.lock().unwrap().remove(&monster.id());
                    map.update_region_for(monster.clone());
                    let _ = map.broadcast(MsgPlayer::from_monster(monster_ref)).await;
                    let _ = map
                        .broadcast(MsgAction::new(monster.id(), 0, 0, 0, ActionType::SpawnEffect))
                        .await;
                }
                continue;
            }
            // Nobody can see a monster on a map without players; skip its
            // movement so idle hunting grounds cost nothing per tick.
            if characters.is_empty() || monster_ref.has_status(crate::entities::StatusEffect::Stunned) {
                continue;
            }
            let mloc = monster_ref.entity().location();
            let home = monster_ref.home();
            let wander_radius = monster_ref.wander_radius();
            // A monster that chased too far gives up and walks back home
            // instead of following a player across the whole map.
            if distance_between(mloc, home) > wander_radius.saturating_add(VIEW_RANGE as u16) {
                let can_move = next_move
                    .lock()
                    .unwrap()
                    .get(&monster.id())
                    .is_none_or(|due| *due <= now);
                if can_move {
                    if let Some(next) = next_step(map, mloc, home, &occupied) {
                        occupied.remove(&(mloc.x, mloc.y));
                        occupied.insert((next.0.x, next.0.y));
                        move_to(map, &monster, next.0, next.1).await;
                    }
                    let delay = 650 + (monster.id() % 240) as u64;
                    next_move
                        .lock()
                        .unwrap()
                        .insert(monster.id(), now + Duration::from_millis(delay));
                }
                continue;
            }
            let Some((target, distance)) = nearest_target(&characters, mloc) else {
                let can_move = next_move
                    .lock()
                    .unwrap()
                    .get(&monster.id())
                    .is_none_or(|due| *due <= now);
                if can_move {
                    let seed = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .map(|value| value.as_millis() as u64)
                        .unwrap_or_default();
                    if let Some(next) = wander_step(map, mloc, (home, wander_radius), &occupied, monster.id(), seed) {
                        occupied.remove(&(mloc.x, mloc.y));
                        occupied.insert((next.0.x, next.0.y));
                        move_to(map, &monster, next.0, next.1).await;
                        let delay = 650 + (monster.id() % 240) as u64;
                        next_move
                            .lock()
                            .unwrap()
                            .insert(monster.id(), now + Duration::from_millis(delay));
                    }
                }
                continue;
            };
            let Some(character) = target.as_character() else {
                continue;
            };
            if distance <= ATTACK_RANGE {
                let can_attack = next_attack
                    .lock()
                    .unwrap()
                    .get(&monster.id())
                    .is_none_or(|due| *due <= now);
                if can_attack {
                    attack(state, monster_ref, character).await;
                    let delay = monster_ref.monster_type().attack_speed.max(350) as u64;
                    next_attack
                        .lock()
                        .unwrap()
                        .insert(monster.id(), now + Duration::from_millis(delay));
                }
            } else if distance <= VIEW_RANGE {
                let can_move = next_move
                    .lock()
                    .unwrap()
                    .get(&monster.id())
                    .is_none_or(|due| *due <= now);
                if can_move {
                    if let Some(next) = next_step(map, mloc, character.entity().location(), &occupied) {
                        occupied.remove(&(mloc.x, mloc.y));
                        occupied.insert((next.0.x, next.0.y));
                        move_to(map, &monster, next.0, next.1).await;
                        let delay = 650 + (monster.id() % 240) as u64;
                        next_move
                            .lock()
                            .unwrap()
                            .insert(monster.id(), now + Duration::from_millis(delay));
                    }
                }
            }
        }
    }
}

fn nearest_target(characters: &[Arc<GameEntity>], location: Location) -> Option<(Arc<GameEntity>, i16)> {
    characters
        .iter()
        .filter_map(|entity| {
            let character = entity.as_character()?;
            if character.entity().is_dead() {
                return None;
            }
            let target = character.entity().location();
            let dx = target.x as i16 - location.x as i16;
            let dy = target.y as i16 - location.y as i16;
            let distance = dx.abs().max(dy.abs());
            (distance <= VIEW_RANGE).then(|| (entity.clone(), distance))
        })
        .min_by_key(|(_, distance)| *distance)
}

fn next_step(
    map: &crate::world::Map,
    from: Location,
    to: Location,
    occupied: &HashSet<(u16, u16)>,
) -> Option<(Location, u8)> {
    let dx = (to.x as i16 - from.x as i16).signum();
    let dy = (to.y as i16 - from.y as i16).signum();
    let candidates = [(dx, dy), (dx, 0), (0, dy), (-dy, dx), (dy, -dx)];
    let (dx, dy) = candidates.into_iter().find(|(cx, cy)| {
        if *cx == 0 && *cy == 0 {
            return false;
        }
        let x = (from.x as i16 + *cx).max(0) as u16;
        let y = (from.y as i16 + *cy).max(0) as u16;
        if (x, y) == (to.x, to.y) || occupied.contains(&(x, y)) {
            return false;
        }
        map.tile(x, y)
            .is_some_and(|tile| tile.access > crate::systems::TileType::Npc)
    })?;
    let x = (from.x as i16 + dx).max(0) as u16;
    let y = (from.y as i16 + dy).max(0) as u16;
    Some((Location::new(x, y, direction(dx, dy)), direction(dx, dy)))
}

fn distance_between(a: Location, b: Location) -> u16 {
    a.x.abs_diff(b.x).max(a.y.abs_diff(b.y))
}

fn wander_step(
    map: &crate::world::Map,
    from: Location,
    (home, radius): (Location, u16),
    occupied: &HashSet<(u16, u16)>,
    id: u32,
    tick: u64,
) -> Option<(Location, u8)> {
    // A stable per-monster rotation gives an organic spread without requiring a
    // random-number generator or making the simulation non-reproducible.
    let directions = [(0, 1), (-1, 1), (-1, 0), (-1, -1), (0, -1), (1, -1), (1, 0), (1, 1)];
    let start = ((id as u64 + tick) as usize) % directions.len();
    (0..directions.len()).find_map(|offset| {
        let (dx, dy) = directions[(start + offset) % directions.len()];
        let x = (from.x as i16 + dx).max(0) as u16;
        let y = (from.y as i16 + dy).max(0) as u16;
        if occupied.contains(&(x, y)) {
            return None;
        }
        // Idle monsters stay inside their spawn area; one that is outside
        // (after a chase) may only step closer to home.
        let next = Location::new(x, y, 0);
        if distance_between(next, home) > radius.max(distance_between(from, home).saturating_sub(1)) {
            return None;
        }
        map.tile(x, y)
            .filter(|tile| tile.access > crate::systems::TileType::Npc)
            .map(|_| (Location::new(x, y, direction(dx, dy)), direction(dx, dy)))
    })
}

async fn move_to(map: &crate::world::Map, monster: &Arc<GameEntity>, location: Location, direction: u8) {
    monster.basic().set_location(location);
    map.update_region_for(monster.clone());
    let _ = map.broadcast(MsgWalk::new(monster.id(), direction)).await;
}

fn direction(dx: i16, dy: i16) -> u8 {
    match (dx, dy) {
        (0, 1) => 0,
        (-1, 1) => 1,
        (-1, 0) => 2,
        (-1, -1) => 3,
        (0, -1) => 4,
        (1, -1) => 5,
        (1, 0) => 6,
        _ => 7,
    }
}

async fn attack(state: &State, monster: &crate::entities::Monster, character: &crate::entities::Character) {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs() as u32)
        .unwrap_or_default();
    let outcome = resolve_physical_hit(
        monster.combat_stats(),
        character.combat_stats_with_catalog(Some(state.shop_catalog())),
        u32::from(character.entity().hp().current()),
        next_roll(),
    );
    character.entity().apply_damage(outcome.damage);
    // MsgInteract is an effect packet; the local HUD receives its actual life
    // through MsgUserInfo. Without this update the server lost HP internally,
    // but the client bar never changed.
    let _ = character.owner().send(MsgUserInfo::from(character)).await;
    let location = character.entity().location();
    let packet = MsgInteract {
        timestamp,
        character_id: monster.id(),
        target_id: character.id(),
        x: location.x,
        y: location.y,
        action: InteractAction::Attack as u32,
        value: outcome.damage,
        reserved: [0; 12],
    };
    let _ = character.owner().send(packet.clone()).await;
    if let Ok(screen) = character.try_screen() {
        let _ = screen.send_message(packet).await;
    }
}
