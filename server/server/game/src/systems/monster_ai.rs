use super::monster_rules as rules;
use crate::combat::{next_roll, resolve_magic_damage, resolve_physical_hit};
use crate::entities::GameEntity;
use crate::packets::{ActionType, InteractAction, MsgAction, MsgInteract, MsgPlayer, MsgUserInfo, MsgWalk};
use crate::State;
use primitives::Location;
use std::collections::HashSet;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// The chase leash past the spawn area.
const VIEW_RANGE: i16 = 18;
/// TQ `ATKUSER_ACTIVE`: the monster hunts players on sight.
const ACTIVE: i32 = 2;
/// How long a monster remembers who hit it.
const AGGRO_TIME: Duration = Duration::from_secs(30);
static AGGRO: OnceLock<Mutex<std::collections::HashMap<u32, (u32, Instant)>>> = OnceLock::new();

fn aggro() -> &'static Mutex<std::collections::HashMap<u32, (u32, Instant)>> {
    AGGRO.get_or_init(|| Mutex::new(std::collections::HashMap::new()))
}

/// `character_id` hit `monster_id`: the monster turns on them, even a
/// passive one or from beyond its sight.
pub fn provoke(monster_id: u32, character_id: u32) {
    aggro().lock().unwrap().insert(monster_id, (character_id, Instant::now()));
}

/// Who the monster fights back, while it still remembers.
pub(crate) fn provoker(monster_id: u32) -> Option<u32> {
    let mut aggro = aggro().lock().unwrap();
    match aggro.get(&monster_id) {
        Some((character_id, since)) if since.elapsed() < AGGRO_TIME => Some(*character_id),
        Some(_) => {
            aggro.remove(&monster_id);
            None
        },
        None => None,
    }
}
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
        let pack = monsters.clone();
        for monster in monsters {
            let Some(monster_ref) = monster.as_monster() else {
                continue;
            };
            if monster_ref.entity().is_dead() {
                // A game master's monster lies a moment, then is gone.
                let summoned = crate::constants::is_summoned(monster.id());
                let should_respawn = {
                    let mut respawns = next_respawn.lock().unwrap();
                    let due = respawns.entry(monster.id()).or_insert_with(|| {
                        let seconds = if summoned { 5 } else { monster_ref.monster_type().respawn_seconds.max(3) as u64 };
                        now + Duration::from_secs(seconds)
                    });
                    *due <= now
                };
                if should_respawn {
                    rules::forget(monster.id());
                }
                if should_respawn && summoned {
                    map.remove_monster(monster.id());
                    aggro().lock().unwrap().remove(&monster.id());
                    next_respawn.lock().unwrap().remove(&monster.id());
                    let _ = map
                        .broadcast(MsgAction::new(monster.id(), 0, 0, 0, ActionType::LeaveMap))
                        .await;
                } else if should_respawn {
                    let home = monster_ref.home();
                    let location = if map.is_walkable(home.x, home.y) {
                        home
                    } else {
                        map.nearest_walkable_position(home, crate::world::MONSTER_RELOCATION_RADIUS)
                            .unwrap_or(home)
                    };
                    monster_ref.respawn_at(location);
                    aggro().lock().unwrap().remove(&monster.id());
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
            let monster_type = monster_ref.monster_type();
            let view_range = monster_type.view_range.clamp(1, VIEW_RANGE as i32) as i16;
            let attack_range = monster_type.attack_range.clamp(1, VIEW_RANGE as i32) as i16;
            if distance_between(mloc, home) > wander_radius.saturating_add(VIEW_RANGE as u16) {
                aggro().lock().unwrap().remove(&monster.id());
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
            let guard = is_guard(monster_type.id);
            let champion = rules::is_champion(monster.id(), monster_type);
            let caster = monster_type.magic_skill > 0;
            let monster_level = monster_ref.entity().level();
            let life = monster_ref.entity().hp();
            let life = (life.current, life.max);
            // Whoever hit it first; otherwise, if it hunts, the nearest player
            // (not one far above it, unless it is a guard or a champion).
            let provoked = provoker(monster.id()).and_then(|id| {
                characters
                    .iter()
                    .find(|entity| entity.id() == id)
                    .and_then(|entity| reachable(entity, mloc, false, caster, VIEW_RANGE))
            });
            // Its pack answers the first time it is fought.
            if let Some((target, _)) = &provoked {
                if rules::alert(monster.id(), true) {
                    for other in &pack {
                        let Some(other_monster) = other.as_monster() else {
                            continue;
                        };
                        if other.id() != monster.id()
                            && other_monster.monster_type().id == monster_type.id
                            && !other_monster.entity().is_dead()
                            && distance_between(other_monster.entity().location(), mloc) <= rules::PACK_RADIUS
                            && provoker(other.id()).is_none()
                        {
                            provoke(other.id(), target.id());
                        }
                    }
                }
            } else {
                rules::alert(monster.id(), false);
            }
            let hunts = guard || monster_type.attack_mode & ACTIVE != 0;
            let level_cap = (!guard && !champion).then_some(monster_level);
            let found = provoked.or_else(|| {
                hunts
                    .then(|| nearest_target(&characters, mloc, guard, caster, view_range, level_cap))
                    .flatten()
            });
            // A weak monster nearly dead runs from its foe, once.
            if let Some((target, _)) = &found {
                if !guard && !champion && (rules::fleeing(monster.id(), now) || rules::start_flee(monster.id(), monster_level, life, now)) {
                    let away = rules::away_from(mloc, target.basic().location(), 5);
                    let can_move = next_move.lock().unwrap().get(&monster.id()).is_none_or(|due| *due <= now);
                    if can_move {
                        if let Some(next) = next_step(map, mloc, away, &occupied) {
                            occupied.remove(&(mloc.x, mloc.y));
                            occupied.insert((next.0.x, next.0.y));
                            move_to(map, &monster, next.0, next.1).await;
                        }
                        next_move.lock().unwrap().insert(monster.id(), now + Duration::from_millis(450));
                    }
                    continue;
                }
            }
            // A champion rages, calls its kind, and strikes all around.
            if champion && found.is_some() {
                if rules::enrage(monster.id(), life) {
                    let _ = map
                        .broadcast(crate::packets::MsgTalk::from_system(
                            0,
                            crate::packets::TalkChannel::Center,
                            format!("{} is enraged!", monster_type.name),
                        ))
                        .await;
                }
                if rules::summon(monster.id(), life) {
                    summon_minions(state, map, monster_type.id as u32, mloc).await;
                }
                if rules::area_due(monster.id(), now) {
                    let skill = if caster { monster_type.magic_skill as u32 } else { rules::CHAMPION_MAGIC };
                    for other in &characters {
                        let Some(other) = other.as_character() else {
                            continue;
                        };
                        if !other.entity().is_dead() && distance_between(mloc, other.entity().location()) <= rules::AREA_RADIUS {
                            cast(state, monster_ref, other, skill).await;
                        }
                    }
                }
            }
            let Some((target, distance)) = found else {
                // Guards hold their post until a criminal comes along; one
                // drawn away by a chase walks back to it and stands there.
                if guard {
                    let away = mloc.x != home.x || mloc.y != home.y;
                    let can_move = away && next_move.lock().unwrap().get(&monster.id()).is_none_or(|due| *due <= now);
                    if can_move {
                        if let Some(next) = next_step(map, mloc, home, &occupied) {
                            occupied.remove(&(mloc.x, mloc.y));
                            occupied.insert((next.0.x, next.0.y));
                            move_to(map, &monster, next.0, next.1).await;
                        }
                        next_move.lock().unwrap().insert(monster.id(), now + Duration::from_millis(700));
                    }
                    continue;
                }
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
            // A caster's magic reaches farther than its blows.
            let reach = if caster { attack_range.max(rules::MAGIC_RANGE) } else { attack_range };
            if distance <= reach {
                let can_attack = next_attack
                    .lock()
                    .unwrap()
                    .get(&monster.id())
                    .is_none_or(|due| *due <= now);
                if can_attack {
                    if caster && (distance > attack_range || next_roll() % 2 == 0) {
                        cast(state, monster_ref, character, monster_type.magic_skill as u32).await;
                    } else {
                        attack(state, monster_ref, character).await;
                    }
                    // A boss's blow strikes everyone close to it.
                    if crate::bosses::is_boss(monster.id()) {
                        for other in &characters {
                            let Some(other) = other.as_character() else {
                                continue;
                            };
                            let near = distance_between(mloc, other.entity().location()) <= crate::bosses::AREA_BLOW;
                            if other.id() != character.id() && near && !other.entity().is_dead() {
                                attack(state, monster_ref, other).await;
                            }
                        }
                    }
                    let delay = monster_ref.monster_type().attack_speed.max(350) as u64;
                    // An enraged champion strikes twice as fast.
                    let delay = if rules::enraged(monster.id()) { (delay / 2).max(350) } else { delay };
                    next_attack
                        .lock()
                        .unwrap()
                        .insert(monster.id(), now + Duration::from_millis(delay));
                }
            } else {
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

/// City guards (Guard1, Guard2): they punish crime instead of hunting.
pub(crate) fn is_guard(monster_type_id: i32) -> bool {
    matches!(monster_type_id, 900 | 910 | 911)
}

/// The nearest player the monster may go after. `level_cap`, the monster's
/// level: heroes far above it are left alone.
fn nearest_target(
    characters: &[Arc<GameEntity>],
    location: Location,
    guard: bool,
    caster: bool,
    view_range: i16,
    level_cap: Option<u16>,
) -> Option<(Arc<GameEntity>, i16)> {
    characters
        .iter()
        .filter(|entity| {
            level_cap.is_none_or(|level| {
                entity
                    .as_character()
                    .is_some_and(|character| !rules::ignores(level, character.entity().level()))
            })
        })
        .filter_map(|entity| reachable(entity, location, guard, caster, view_range))
        .min_by_key(|(_, distance)| *distance)
}

/// The player and their distance, if the monster may go after them.
fn reachable(entity: &Arc<GameEntity>, location: Location, guard: bool, caster: bool, view_range: i16) -> Option<(Arc<GameEntity>, i16)> {
    let character = entity.as_character()?;
    // Dead, invisible or flying players are out of reach; guards still
    // shoot fliers down (Redux `Monster`: mesh 900).
    let flags = character.entity().flags();
    if character.entity().is_dead()
        || character.revive_protected()
        || flags.contains(crate::entities::Flags::INVISIBILITY)
        // A caster's magic reaches a flier too.
        || flags.contains(crate::entities::Flags::FLYING) && !guard && !caster
    {
        return None;
    }
    // Guards only go after fresh killers and black names.
    if guard
        && !flags.intersects(crate::entities::Flags::BLUE_FLASHING_NAME | crate::entities::Flags::BLACK_NAME)
    {
        return None;
    }
    let target = character.entity().location();
    let dx = target.x as i16 - location.x as i16;
    let dy = target.y as i16 - location.y as i16;
    let distance = dx.abs().max(dy.abs());
    (distance <= view_range).then(|| (entity.clone(), distance))
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
    if character.revive_protected() {
        return;
    }
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs() as u32)
        .unwrap_or_default();
    let life_before = u32::from(character.entity().hp().current());
    let mut outcome = resolve_physical_hit(
        monster.combat_stats(),
        character.combat_stats_with_catalog(Some(state.shop_catalog())),
        life_before,
        next_roll(),
    );
    // The hero's dodge (worn items) against the monster's accuracy: a blow
    // lands max(40, 90 - dodge + accuracy) percent of the time (Redux).
    let (dodge, _) = character.dodge_and_accuracy(state.shop_catalog());
    if next_roll() % 100 >= crate::combat::hit_chance(dodge, monster.accuracy()) {
        outcome.damage = 0;
        outcome.raw_damage = 0;
        outcome.remaining_health = life_before;
        outcome.defeated = false;
    }
    // Heroes far above the monster take less, far below it more.
    crate::combat::scale_outcome(
        &mut outcome,
        life_before,
        rules::level_damage_percent(monster.entity().level(), character.entity().level()),
    );
    // Refinery Block stops a monster's blow too.
    if crate::refinery::blocked(character.refinery_stats().block, 0, next_roll()) {
        crate::combat::nullify(&mut outcome, life_before);
    }
    crate::lucky::empower(None, Some(character), &mut outcome, life_before);
    // A pure Warrior's Reflect sends the blow back to the monster.
    if outcome.damage > 0 && crate::skills::reflects(character) {
        let reflected = crate::combat::reflected_damage(outcome.damage, u32::from(monster.entity().hp().current()));
        monster.entity().apply_damage(reflected);
        outcome.damage = 0;
        outcome.remaining_health = life_before;
        outcome.defeated = false;
        let location = monster.entity().location();
        let bounce = MsgInteract {
            timestamp,
            character_id: character.id(),
            target_id: monster.id(),
            x: location.x,
            y: location.y,
            action: InteractAction::Attack as u32,
            value: reflected,
            reserved: [0; 12],
        };
        let _ = character.owner().send(bounce.clone()).await;
        if let Ok(screen) = character.try_screen() {
            let _ = screen.send_message(bounce).await;
        }
        let _ = character.owner().send(MsgPlayer::from_monster(monster)).await;
    }
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
    if character.entity().is_dead() {
        if let Err(error) = crate::death::kill_character(state, character, monster.id(), None).await {
            tracing::warn!(character = character.id(), ?error, "Failed to process character death");
        }
    }
}

/// A caster's magic at `character`: its attack, softened like a blow by
/// their levels, against the hero's magic defense and resistance to the
/// magic's element; it cannot be dodged or blocked.
async fn cast(state: &State, monster: &crate::entities::Monster, character: &crate::entities::Character, skill_id: u32) {
    if character.revive_protected() || character.entity().is_dead() {
        return;
    }
    let catalog = state.shop_catalog();
    let stats = monster.combat_stats();
    let spread = stats.max_attack.saturating_sub(stats.min_attack);
    let attack = (stats.min_attack + next_roll() % (spread + 1)) * rules::MAGIC_PERCENT / 100;
    let element = state.skill_catalog().get(skill_id).and_then(|skill| skill.element);
    let refinery = character.refinery_stats();
    let defense = character.combat_stats_with_catalog(Some(catalog));
    let life = character.entity().hp();
    let life_before = u32::from(life.current);
    let mut outcome = resolve_magic_damage(
        attack,
        character.magic_defense_with_catalog(Some(catalog)),
        0,
        defense
            .damage_reduction_percent
            .saturating_add(u16::try_from(refinery.magic_defense).unwrap_or(100)),
        life_before,
        u32::from(life.max),
        u16::try_from(refinery.resistance(element)).unwrap_or(100),
    );
    crate::combat::scale_outcome(
        &mut outcome,
        life_before,
        rules::level_damage_percent(monster.entity().level(), character.entity().level()),
    );
    crate::lucky::empower(None, Some(character), &mut outcome, life_before);
    character.entity().apply_damage(outcome.damage);
    let location = character.entity().location();
    let mut effect = crate::packets::MsgSkillEffect::new(
        monster.id(),
        u16::try_from(skill_id).unwrap_or_default(),
        vec![crate::packets::SkillTarget::hit(character.id(), outcome.damage)],
    );
    effect.target_x = location.x;
    effect.target_y = location.y;
    let _ = character.owner().send(MsgUserInfo::from(character)).await;
    let _ = character.owner().send(effect.clone()).await;
    if let Ok(screen) = character.try_screen() {
        let _ = screen.send_message(effect).await;
    }
    if character.entity().is_dead() {
        if let Err(error) = crate::death::kill_character(state, character, monster.id(), None).await {
            tracing::warn!(character = character.id(), ?error, "Failed to process character death");
        }
    }
}

/// A world boss calls its kind to its side.
async fn summon_minions(state: &State, map: &crate::world::Map, boss_type: u32, at: Location) {
    let Some(kind) = crate::bosses::minion_kind(boss_type) else {
        return;
    };
    let Ok(Some(minion)) = crate::bosses::monster_type(state, kind).await else {
        return;
    };
    for _ in 0..rules::SUMMONED_MINIONS {
        if let Err(error) = crate::bosses::spawn_monster(state, map.id(), (at.x, at.y), &minion, 4).await {
            tracing::warn!(%error, "A boss could not call its kind");
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hit_monster_remembers_its_attacker() {
        assert_eq!(provoker(4_000_001), None);
        provoke(4_000_001, 1_000_042);
        assert_eq!(provoker(4_000_001), Some(1_000_042));
        provoke(4_000_001, 1_000_043);
        assert_eq!(provoker(4_000_001), Some(1_000_043));
    }
}
