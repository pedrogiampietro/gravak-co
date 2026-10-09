//! The city artisans (ArtisanWind in Twin City, ArtisanCloud, ArtisanOu)
//! as the original's: one Meteor raises an item's level and one DragonBall
//! its quality, each at the classic chance of a single catalyst
//! (`artisans::level_chance` and `quality_chance`, Redux
//! `ChanceToUpgradeLevel`/`Quality`). A failure loses the catalyst and
//! halves the item's durability.
//!
//! Talking to them opens the client's forge window (MsgAction OpenDialog,
//! `data1` `WINDOW`), which asks with MsgItem `LEVEL` and `QUALITY` (the
//! item's uid) and hears back the same action, `param1` one of `Result`.

use tq_network::Actor;

use crate::entities::{Character, InventoryItem};
use crate::packets::{ActionType, MsgAction, MsgItem, MsgItemInfo, MsgTalk, MsgUserInfo, TalkChannel};
use crate::{ActorState, Error, State};

/// ArtisanWind (Twin City), ArtisanCloud and ArtisanOu.
pub const NPC_IDS: [u32; 3] = [10_019, 35, 41];
/// The client's forge window, for MsgAction OpenDialog.
pub const WINDOW: u32 = 9_020;
/// MsgItem actions of the window: a level (Meteor) or a quality
/// (DragonBall) upgrade of the item `param0`.
pub const LEVEL: u32 = 9_011;
pub const QUALITY: u32 = 9_012;
/// How far from the artisan the window works.
const REACH: u16 = 18;

/// What came of an upgrade, in the answer's `param1`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Outcome {
    Failed = 0,
    Upgraded = 1,
    Refused = 2,
}

/// Gear the artisans work on: the paper doll's head to boots, not the
/// garment, arrows or weapon accessories.
pub fn upgradable(item_type: u32) -> bool {
    crate::shop::ShopCatalog::equipment_slot_for_item(item_type).is_some_and(|slot| (1..=8).contains(&slot))
        && !crate::shop::ShopCatalog::is_arrow(item_type)
}

/// The next item, the catalyst and the chance (in percent) of `action`
/// (`LEVEL` or `QUALITY`) on `item_type`, or why it cannot be done.
pub fn offer(action: u32, item_type: u32, requirement: u32, exists: impl Fn(u32) -> bool) -> Result<(u32, u32, u32), &'static str> {
    if !upgradable(item_type) {
        return Err("The artisan does not work on this item.");
    }
    if action == LEVEL {
        if requirement >= 120 {
            return Err("Above level 120 only the WeaponMaster raises an item's level.");
        }
        let next = crate::artisans::next_level(item_type, exists).ok_or("This item is at its highest level.")?;
        Ok((next, crate::forge::METEOR, crate::artisans::level_chance(item_type)))
    } else {
        let next = crate::artisans::next_quality(item_type, exists).ok_or("This item's quality cannot be raised.")?;
        Ok((next, crate::forge::DRAGONBALL, crate::artisans::quality_chance(item_type, requirement)))
    }
}

/// Opens the forge window.
pub async fn open(actor: &Actor<ActorState>, character: &Character) -> Result<(), Error> {
    actor
        .send(MsgAction::new(character.id(), WINDOW, 0, 0, ActionType::OpenDialog))
        .await?;
    Ok(())
}

fn near_artisan(state: &State, character: &Character) -> bool {
    let at = character.entity().location();
    let Ok(map) = state.try_map(character.entity().map_id()) else {
        return false;
    };
    NPC_IDS.iter().filter_map(|id| map.npc(*id)).any(|npc| {
        let npc = npc.entity().location();
        at.x.abs_diff(npc.x) <= REACH && at.y.abs_diff(npc.y) <= REACH
    })
}

async fn answer(actor: &Actor<ActorState>, character: &Character, action: u32, item_uid: u32, outcome: Outcome) -> Result<(), Error> {
    actor
        .send(MsgItem::artisan_answer(character.id(), item_uid, action, outcome as u32))
        .await?;
    Ok(())
}

async fn refuse(actor: &Actor<ActorState>, character: &Character, action: u32, item_uid: u32, why: &str) -> Result<(), Error> {
    actor
        .send(MsgTalk::from_system(character.id(), TalkChannel::TopLeft, why))
        .await?;
    answer(actor, character, action, item_uid, Outcome::Refused).await
}

/// Sends the changed item (worn or in the bag), the catalyst gone and,
/// for worn gear, the hero's figures.
async fn send_changes(
    state: &State,
    actor: &Actor<ActorState>,
    character: &Character,
    item: InventoryItem,
    consumed: Vec<InventoryItem>,
) -> Result<(), Error> {
    for catalyst in consumed {
        actor.send(MsgItemInfo::remove_inventory_item(catalyst)).await?;
    }
    if character.equipment_items().iter().any(|worn| worn.uid == item.uid) {
        actor.send(MsgItemInfo::equipment_item(item)).await?;
        character.refresh_equipment_resources(state.shop_catalog());
        actor.send(MsgUserInfo::from(character)).await?;
    } else {
        actor.send(MsgItemInfo::inventory_item(item)).await?;
    }
    Ok(())
}

/// One upgrade from the window: `action` (`LEVEL` or `QUALITY`) on the
/// item `item_uid`, worn or in the bag.
pub async fn attempt(
    state: &State,
    actor: &Actor<ActorState>,
    character: &Character,
    action: u32,
    item_uid: u32,
) -> Result<(), Error> {
    if !near_artisan(state, character) {
        return refuse(actor, character, action, item_uid, "The artisan is too far away.").await;
    }
    let Some(item) = character
        .equipment_items()
        .into_iter()
        .chain(character.inventory_items())
        .find(|item| item.uid == item_uid)
    else {
        return refuse(actor, character, action, item_uid, "Only equipment you own can be upgraded.").await;
    };
    let catalog = state.shop_catalog();
    let requirement = |item_type: u32| catalog.template(item_type).map_or(0, |template| template.level);
    let (next, catalyst, chance) =
        match offer(action, item.item_type, requirement(item.item_type), |id| catalog.template(id).is_some()) {
            Ok(offer) => offer,
            Err(why) => return refuse(actor, character, action, item_uid, why).await,
        };
    if action == LEVEL && u32::from(character.entity().level()) < requirement(next) {
        return refuse(actor, character, action, item_uid, "You are too low level to wear the upgraded item!").await;
    }
    // Luck shrinks the roll the chance is compared against.
    let roll = crate::combat::next_roll() % 100;
    let roll = if character.lucky_time() > 0 { roll * 9 / 10 } else { roll };
    let upgraded = roll < chance;
    let changed = if upgraded {
        character.reforge_item(state.pool(), item_uid, next, catalyst, 1, false).await?
    } else {
        character.fail_upgrade(state.pool(), item_uid, catalyst).await?
    };
    let what = if catalyst == crate::forge::METEOR { "a Meteor" } else { "a DragonBall" };
    let Some((item, consumed)) = changed else {
        return refuse(actor, character, action, item_uid, &format!("You need {what} for this.")).await;
    };
    send_changes(state, actor, character, item, consumed).await?;
    crate::audit::economic(if action == LEVEL { "artisan_level" } else { "artisan_quality" }, character.id(), 0, item.uid, u32::from(upgraded));
    let line = match (upgraded, action == LEVEL) {
        (true, true) => "The item's level has been raised!",
        (true, false) => "The item's quality has been raised!",
        (false, _) => "The upgrade failed: the catalyst is lost and the item's durability halved.",
    };
    actor
        .send(MsgTalk::from_system(character.id(), TalkChannel::TopLeft, line))
        .await?;
    answer(actor, character, action, item_uid, if upgraded { Outcome::Upgraded } else { Outcome::Failed }).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_artisans_work_on_gear_but_not_garments_arrows_or_accessories() {
        assert!(upgradable(410_339) && upgradable(500_323) && upgradable(130_003) && upgradable(900_003));
        assert!(!upgradable(181_305), "a garment");
        assert!(!upgradable(1_050_002), "arrows");
        assert!(!upgradable(370_001), "a weapon accessory");
    }

    #[test]
    fn a_level_upgrade_takes_a_meteor_and_a_quality_one_a_dragonball() {
        // A level 100 bow, Unique: the next level exists.
        let (next, catalyst, chance) = offer(LEVEL, 500_197, 100, |_| true).unwrap();
        assert_eq!((next, catalyst), (500_207, crate::forge::METEOR));
        assert_eq!(chance, crate::artisans::level_chance(500_197));
        let (next, catalyst, _) = offer(QUALITY, 500_197, 100, |_| true).unwrap();
        assert_eq!((next, catalyst), (500_198, crate::forge::DRAGONBALL));
        assert!(offer(LEVEL, 500_197, 120, |_| true).is_err(), "WeaponMaster's from level 120");
        assert!(offer(QUALITY, 500_199, 100, |_| true).is_err(), "Super is the highest quality");
    }
}
