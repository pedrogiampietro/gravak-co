//! Summoned pets (Golem, WaterElf, NightDevil), after the reference server
//! (`Objects/Pet.cs`, `Managers/PetAI.cs`): the skill's `Power` is the
//! monster type that appears beside the caster. It follows its owner,
//! fights the monsters around them and fades when its time runs out; what
//! it kills rewards the owner.

use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use parking_lot::Mutex;

use crate::combat::{next_roll, resolve_physical_hit};
use crate::entities::{Character, GameEntity, Monster};
use crate::packets::{ActionType, InteractAction, MsgAction, MsgInteract, MsgPlayer, MsgTalk, MsgWalk, TalkChannel};
use crate::{Error, State};

/// Pets use the top of the monster id range, one per owner.
const PET_ID_BASE: u32 = 495_000;
const PET_ID_SPAN: u32 = 4_999;
/// Pets look for monsters this far and attack from this close.
const VIEW_RANGE: u16 = 8;
const ATTACK_RANGE: u16 = 2;
/// Beyond this distance the pet jumps back beside its owner.
const LEASH: u16 = 12;
const FOLLOW_DISTANCE: u16 = 2;
const STEP: Duration = Duration::from_millis(500);

struct Pet {
    monster: Monster,
    owner_id: u32,
    expires: Instant,
    next_attack: Mutex<Instant>,
    next_step: Mutex<Instant>,
}

fn pets() -> &'static Mutex<HashMap<u32, Arc<Pet>>> {
    static PETS: OnceLock<Mutex<HashMap<u32, Arc<Pet>>>> = OnceLock::new();
    PETS.get_or_init(Default::default)
}

pub fn is_pet(id: u32) -> bool {
    (PET_ID_BASE..PET_ID_BASE + PET_ID_SPAN).contains(&id)
}

async fn show<P>(character: &Character, packet: P)
where
    P: tq_network::PacketEncode + tq_network::PacketID + Clone + Send + Sync + 'static,
{
    let _ = character.owner().send(packet.clone()).await;
    if let Ok(screen) = character.try_screen() {
        let _ = screen.send_message(packet).await;
    }
}

fn find_character(state: &State, id: u32) -> Option<Arc<GameEntity>> {
    state
        .entities()
        .into_iter()
        .find(|entity| entity.id() == id && entity.as_character().is_some())
}

/// Summons `monster_type_id` beside `character` for `duration`, replacing
/// their previous pet.
pub async fn summon(state: &State, character: &Character, monster_type_id: u32, duration: Duration) -> Result<(), Error> {
    dismiss(state, character.id()).await;
    let monster_type = sqlx::query_as::<_, tq_db::monster::MonsterType>(
        "SELECT id, name, look, level, life, attack_min, attack_max, defense, attack_speed, respawn_seconds, view_range, attack_range, attack_mode, magic_resistance \
         FROM monster_types WHERE id = ?",
    )
    .bind(i64::from(monster_type_id))
    .fetch_optional(state.pool())
    .await?;
    let Some(monster_type) = monster_type else {
        character
            .owner()
            .send(MsgTalk::from_system(character.id(), TalkChannel::TopLeft, "That creature cannot be summoned."))
            .await?;
        return Ok(());
    };
    let location = character.entity().location();
    let spawn = tq_db::monster::MonsterSpawn {
        id: 0,
        monster_type_id: monster_type.id,
        map_id: character.entity().map_id() as i32,
        x: location.x as i16,
        y: location.y.saturating_add(1) as i16,
        amount: 1,
        radius: 0,
    };
    let id = PET_ID_BASE + character.database_id().max(0) as u32 % PET_ID_SPAN;
    let monster = Monster::new(id, monster_type, spawn);
    show(character, MsgPlayer::from_monster(&monster)).await;
    show(character, MsgAction::new(id, 0, 0, 0, ActionType::SpawnEffect)).await;
    let now = Instant::now();
    pets().lock().insert(
        character.id(),
        Arc::new(Pet {
            monster,
            owner_id: character.id(),
            expires: now + duration,
            next_attack: Mutex::new(now + Duration::from_millis(800)),
            next_step: Mutex::new(now),
        }),
    );
    Ok(())
}

/// Removes the owner's pet from the world.
pub async fn dismiss(state: &State, owner_id: u32) {
    let Some(pet) = pets().lock().remove(&owner_id) else {
        return;
    };
    let leave = MsgAction::new(pet.monster.id(), 0, 0, 0, ActionType::LeaveMap);
    if let Some(owner) = find_character(state, owner_id) {
        if let Some(owner) = owner.as_character() {
            show(owner, leave).await;
        }
    }
}

fn direction(from: (u16, u16), to: (u16, u16)) -> u8 {
    let dx = (i32::from(to.0) - i32::from(from.0)).signum();
    let dy = (i32::from(to.1) - i32::from(from.1)).signum();
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

fn distance(a: (u16, u16), b: (u16, u16)) -> u16 {
    a.0.abs_diff(b.0).max(a.1.abs_diff(b.1))
}

/// One step of the pet toward `goal`, if the cell is walkable.
async fn step_toward(state: &State, owner: &Character, pet: &Pet, goal: (u16, u16)) {
    let here = pet.monster.entity().location();
    let from = (here.x, here.y);
    let dir = direction(from, goal);
    let dx = (i32::from(goal.0) - i32::from(from.0)).signum();
    let dy = (i32::from(goal.1) - i32::from(from.1)).signum();
    let next = ((i32::from(from.0) + dx).max(0) as u16, (i32::from(from.1) + dy).max(0) as u16);
    let walkable = state
        .try_map(owner.entity().map_id())
        .is_ok_and(|map| map.is_walkable(next.0, next.1));
    if !walkable {
        return;
    }
    let mut location = here;
    location.x = next.0;
    location.y = next.1;
    location.direction = dir;
    pet.monster.entity().set_location(location);
    show(owner, MsgWalk::new(pet.monster.id(), dir)).await;
}

/// Pets follow, fight and fade.
pub async fn tick(state: &State) {
    let now = Instant::now();
    let all = pets().lock().values().cloned().collect::<Vec<_>>();
    for pet in all {
        let owner_entity = find_character(state, pet.owner_id);
        let Some(owner) = owner_entity.as_ref().and_then(|entity| entity.as_character()) else {
            pets().lock().remove(&pet.owner_id);
            continue;
        };
        let owner_location = owner.entity().location();
        let pet_location = pet.monster.entity().location();
        let away = distance((owner_location.x, owner_location.y), (pet_location.x, pet_location.y));
        if now >= pet.expires
            || owner.entity().is_dead()
            || pet.monster.entity().map_id() != owner.entity().map_id()
        {
            dismiss(state, pet.owner_id).await;
            continue;
        }
        if away > LEASH {
            // Jump back beside the owner.
            let mut location = pet_location;
            location.x = owner_location.x;
            location.y = owner_location.y.saturating_add(1);
            pet.monster.entity().set_location(location);
            show(owner, MsgPlayer::from_monster(&pet.monster)).await;
            continue;
        }
        let Ok(map) = state.try_map(owner.entity().map_id()) else {
            continue;
        };
        let here = (pet_location.x, pet_location.y);
        let target = map
            .monster_entities()
            .into_iter()
            .filter(|entity| {
                entity.as_monster().is_some_and(|monster| {
                    let location = monster.entity().location();
                    !monster.entity().is_dead()
                        && !matches!(monster.monster_type().id, 900 | 910 | 911)
                        && distance(here, (location.x, location.y)) <= VIEW_RANGE
                })
            })
            .min_by_key(|entity| {
                let location = entity.basic().location();
                distance(here, (location.x, location.y))
            });
        let Some(target) = target else {
            if away > FOLLOW_DISTANCE && *pet.next_step.lock() <= now {
                *pet.next_step.lock() = now + STEP;
                step_toward(state, owner, &pet, (owner_location.x, owner_location.y)).await;
            }
            continue;
        };
        let Some(monster) = target.as_monster() else {
            continue;
        };
        let target_location = monster.entity().location();
        let target_cell = (target_location.x, target_location.y);
        if distance(here, target_cell) > ATTACK_RANGE {
            if *pet.next_step.lock() <= now {
                *pet.next_step.lock() = now + STEP;
                step_toward(state, owner, &pet, target_cell).await;
            }
            continue;
        }
        if *pet.next_attack.lock() > now {
            continue;
        }
        let speed = pet.monster.monster_type().attack_speed.max(500) as u64 + 500;
        *pet.next_attack.lock() = now + Duration::from_millis(speed);
        let life_before = u32::from(monster.entity().hp().current());
        let outcome = resolve_physical_hit(pet.monster.combat_stats(), monster.combat_stats(), life_before, next_roll());
        monster.entity().apply_damage(outcome.damage);
        show(
            owner,
            MsgInteract {
                timestamp: 0,
                character_id: pet.monster.id(),
                target_id: monster.id(),
                x: target_location.x,
                y: target_location.y,
                action: InteractAction::Attack as u32,
                value: outcome.damage,
                reserved: [0; 12],
            },
        )
        .await;
        show(owner, MsgPlayer::from_monster(monster)).await;
        if outcome.defeated {
            if let Err(error) =
                crate::packets::reward_monster_defeat(state, owner, monster, pet.monster.id()).await
            {
                tracing::warn!(?error, "Pet kill reward failed");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pet_ids_stay_in_the_monster_range() {
        assert!(is_pet(PET_ID_BASE));
        assert!(PET_ID_BASE + PET_ID_SPAN <= crate::constants::MONSTER_ID_MAX);
        assert!(!is_pet(400_001));
        assert_eq!(direction((10, 10), (11, 10)), 6);
        assert_eq!(distance((1, 1), (4, 2)), 3);
    }
}
