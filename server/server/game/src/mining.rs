//! Mining, after the reference server (`Player.On_Player_Timer` and the
//! `MineOre`/`MineGem`/`MineMisc` drop rules): with a PickAxe on a mine map,
//! `MsgAction` Mine starts swinging; every three seconds each rule of the
//! map may give an ore (random quality), a gem (sometimes refined or super)
//! or another item. Moving, dying, a full bag or losing the PickAxe stops it.

use std::collections::HashMap;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use parking_lot::Mutex;

use crate::entities::{Character, GameEntity};
use crate::packets::{ActionType, MsgAction, MsgItemInfo, MsgTalk, TalkChannel};
use crate::{Error, State};

const SWING: Duration = Duration::from_secs(3);
const PICKAXE: u32 = 562;
const CHANCE_REFINED_GEM: f64 = 10.0;
const CHANCE_SUPER_GEM: f64 = 1.0;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Rule {
    /// An ore whose last digit is a random quality.
    Ore(u32, f64),
    Gem(u32, f64),
    Misc(u32, f64),
}

/// Redux `drop_rules` of the four mines.
fn rules(map_id: u32) -> &'static [Rule] {
    use Rule::*;
    match map_id {
        1_025 => &[
            Ore(1_072_010, 12.0),
            Ore(1_072_050, 8.0),
            Gem(700_001, 0.4),
            Gem(700_011, 0.4),
            Gem(700_021, 0.4),
            Gem(700_041, 0.4),
        ],
        1_026 => &[
            Ore(1_072_040, 8.0),
            Ore(1_072_050, 8.0),
            Gem(700_061, 0.7),
            Gem(700_051, 0.7),
            Ore(1_072_040, 10.0),
        ],
        1_027 => &[
            Ore(1_072_050, 8.0),
            Gem(700_061, 0.7),
            Gem(700_051, 0.7),
            Ore(1_072_040, 10.0),
            Ore(1_072_040, 8.0),
        ],
        1_028 => &[
            Ore(1_072_010, 12.0),
            Ore(1_072_050, 6.0),
            Misc(1_072_031, 5.0),
            Gem(700_001, 0.35),
            Gem(700_011, 0.35),
            Gem(700_021, 0.35),
            Gem(700_041, 0.35),
        ],
        _ => &[],
    }
}

#[derive(Debug, Clone, Copy)]
struct Miner {
    map_id: u32,
    position: (u16, u16),
    next_swing: Instant,
}

fn miners() -> &'static Mutex<HashMap<u32, Miner>> {
    static MINERS: OnceLock<Mutex<HashMap<u32, Miner>>> = OnceLock::new();
    MINERS.get_or_init(Default::default)
}

fn roll(percent: f64) -> bool {
    use rand::Rng;
    rand::thread_rng().gen_range(0.0..100.0) < percent
}

fn has_pickaxe(character: &Character) -> bool {
    character
        .equipment_item(4)
        .is_some_and(|weapon| weapon.item_type / 1_000 == PICKAXE)
}

/// The mines with drop rules; their maps are mine fields.
fn can_mine_here(state: &State, map_id: u32) -> bool {
    !rules(map_id).is_empty() && state.try_map(map_id).is_ok()
}

/// `MsgAction` Mine from the client.
pub async fn start(state: &State, character: &Character) -> Result<(), Error> {
    let map_id = character.entity().map_id();
    let reason = if !can_mine_here(state, map_id) {
        Some("You cannot mine here.")
    } else if !has_pickaxe(character) {
        Some("You need a PickAxe to mine.")
    } else {
        None
    };
    if let Some(reason) = reason {
        character
            .owner()
            .send(MsgTalk::from_system(character.id(), TalkChannel::TopLeft, reason))
            .await?;
        return Ok(());
    }
    let location = character.entity().location();
    miners().lock().insert(
        character.id(),
        Miner {
            map_id,
            position: (location.x, location.y),
            next_swing: Instant::now() + SWING,
        },
    );
    swing(character).await
}

pub fn stop(character_id: u32) {
    miners().lock().remove(&character_id);
}

async fn swing(character: &Character) -> Result<(), Error> {
    let swing = MsgAction::new(character.id(), 0, 0, 0, ActionType::Mine);
    character.owner().send(swing.clone()).await?;
    if let Ok(screen) = character.try_screen() {
        screen.send_message(swing).await?;
    }
    Ok(())
}

async fn dig(state: &State, character: &Character, map_id: u32) -> Result<(), Error> {
    for rule in rules(map_id) {
        let (item_type, message) = match *rule {
            Rule::Ore(base, chance) if roll(chance) => (base + rand_quality(), "You have just mined an ore!"),
            Rule::Gem(base, chance) if roll(chance) => {
                let tier = if roll(CHANCE_SUPER_GEM) {
                    2
                } else if roll(CHANCE_REFINED_GEM) {
                    1
                } else {
                    0
                };
                (base + tier, "Congratulations! You just mined a gem!")
            },
            Rule::Misc(item, chance) if roll(chance) => (item, "You just mined an item!"),
            _ => continue,
        };
        let Some(item) = character
            .grant_item(state.pool(), state.shop_catalog(), item_type, false)
            .await?
        else {
            stop(character.id());
            return Ok(());
        };
        character.owner().send(MsgItemInfo::inventory_item(item)).await?;
        character
            .owner()
            .send(MsgTalk::from_system(character.id(), TalkChannel::System, message))
            .await?;
    }
    Ok(())
}

fn rand_quality() -> u32 {
    use rand::Rng;
    rand::thread_rng().gen_range(0..10)
}

/// Swings for every miner whose three seconds are up.
pub async fn tick(state: &State) {
    let due = {
        let now = Instant::now();
        let mut miners = miners().lock();
        let due = miners
            .iter_mut()
            .filter(|(_, miner)| miner.next_swing <= now)
            .map(|(id, miner)| {
                miner.next_swing = now + SWING;
                (*id, *miner)
            })
            .collect::<Vec<_>>();
        due
    };
    for (id, miner) in due {
        let Some(entity) = state.entities().into_iter().find(|entity| entity.id() == id) else {
            stop(id);
            continue;
        };
        let GameEntity::Character(character) = entity.as_ref() else {
            continue;
        };
        let location = character.entity().location();
        let still = character.entity().map_id() == miner.map_id && (location.x, location.y) == miner.position;
        if !still || character.entity().is_dead() || !has_pickaxe(character) {
            stop(id);
            continue;
        }
        if character.inventory_items().len() >= 40 {
            let _ = character
                .owner()
                .send(MsgTalk::from_system(
                    id,
                    TalkChannel::System,
                    "Your inventory is full. You can not mine anymore items.",
                ))
                .await;
            stop(id);
            continue;
        }
        let _ = swing(character).await;
        if let Err(error) = dig(state, character, miner.map_id).await {
            tracing::warn!(character = id, ?error, "Mining failed");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_mine_has_rules() {
        for map in [1_025, 1_026, 1_027, 1_028] {
            assert!(!rules(map).is_empty());
        }
        assert!(rules(1_002).is_empty());
    }
}
