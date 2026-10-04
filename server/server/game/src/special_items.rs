//! Items whose use does something of its own, after the reference server
//! (`Items/*.cs` of Redux): city scrolls, hair dyes, fireworks, the
//! NinjaAmulet and the BlackTulip.

use tq_network::Actor;

use crate::entities::{Character, InventoryItem};
use crate::packets::{MapFlags, MsgItemInfo, MsgName, MsgPlayer, MsgTalk, TalkChannel};
use crate::{ActorState, Error, State};

/// City scrolls (`Gates/*.cs`): the city and where in it.
const CITY_SCROLLS: [(u32, u32, (u16, u16)); 5] = [
    (1_060_020, 1_002, (430, 380)), // Twin City
    (1_060_021, 1_000, (496, 650)), // Desert City
    (1_060_022, 1_020, (566, 565)), // Ape Mountain
    (1_060_023, 1_011, (193, 266)), // Phoenix Castle
    (1_060_024, 1_015, (717, 571)), // Bird Island
];

/// Hair dyes (`HairDye.cs`): the hair colour each gives.
const HAIR_DYES: [(u32, u16); 7] = [
    (1_060_030, 3), // Black
    (1_060_040, 9), // Violet
    (1_060_050, 8), // Blue
    (1_060_060, 7), // Green
    (1_060_070, 6), // Brown
    (1_060_080, 5), // Red
    (1_060_090, 4), // White
];

/// Fireworks (`Fireworks.cs`) and the effect each sets off.
pub const FIREWORKS: [(u32, &str); 3] = [
    (720_030, "Firework"),
    (720_031, "EndlessLove"),
    (720_032, "MyWish"),
];

const NINJA_AMULET: u32 = 723_583;
const BLACK_TULIP: u32 = 723_584;
/// The armor colour the BlackTulip gives.
const BLACK: u8 = 2;

/// `hair` (colour * 100 + style) with the colour changed.
pub fn dyed_hair(hair: u16, colour: u16) -> u16 {
    colour * 100 + hair % 100
}

/// `mesh` (face * 10000 + body) with the other body of the same sex: the
/// small and the large one swap.
pub fn other_body(mesh: u32) -> u32 {
    let body = mesh % 10_000;
    let other = match body {
        1003 => 1004,
        1004 => 1003,
        2001 => 2002,
        2002 => 2001,
        _ => body,
    };
    mesh - body + other
}

/// Uses `item` when it is one of these; false for any other item.
pub async fn use_item(state: &State, actor: &Actor<ActorState>, character: &Character, item: &InventoryItem) -> Result<bool, Error> {
    let notice = |text: &str| MsgTalk::from_system(character.id(), TalkChannel::TopLeft, text);
    if let Some((_, map_id, position)) = CITY_SCROLLS.iter().find(|(scroll, _, _)| *scroll == item.item_type) {
        let here = character.entity().map_id();
        let flags = state.try_map(here).map(|map| map.flags()).unwrap_or(MapFlags::empty());
        if character.entity().is_dead() || flags.intersects(MapFlags::TELEPORT_DISABLED | MapFlags::PRISON_MAP) {
            actor.send(notice("The scroll does not work here.")).await?;
            return Ok(true);
        }
        if !consume(state, actor, character, item).await? {
            return Ok(true);
        }
        character.teleport(state, *map_id, *position).await?;
        return Ok(true);
    }
    if let Some((_, colour)) = HAIR_DYES.iter().find(|(dye, _)| *dye == item.item_type) {
        if consume(state, actor, character, item).await? {
            character.set_hair_style(dyed_hair(character.hair_style(), *colour));
            refresh(state, actor, character).await?;
        }
        return Ok(true);
    }
    if let Some((_, name)) = FIREWORKS.iter().find(|(firework, _)| *firework == item.item_type) {
        if consume(state, actor, character, item).await? {
            let fireworks = MsgName::fireworks(character.id(), name);
            actor.send(fireworks.clone()).await?;
            actor.screen().send_message(fireworks).await?;
        }
        return Ok(true);
    }
    if item.item_type == NINJA_AMULET {
        if consume(state, actor, character, item).await? {
            character.entity().set_mesh(other_body(character.entity().mesh()));
            refresh(state, actor, character).await?;
        }
        return Ok(true);
    }
    if item.item_type == BLACK_TULIP {
        let armor = character
            .equipment_item(3)
            .filter(|armor| (130..=139).contains(&(armor.item_type / 1_000)));
        let Some(armor) = armor.filter(|armor| armor.color != BLACK) else {
            actor.send(notice("Wear an armor that is not black yet.")).await?;
            return Ok(true);
        };
        if consume(state, actor, character, item).await? {
            if let Some(armor) = character.dye_item(state.pool(), armor.uid, BLACK).await? {
                actor.send(MsgItemInfo::equipment_item(armor)).await?;
            }
            refresh(state, actor, character).await?;
        }
        return Ok(true);
    }
    Ok(false)
}

/// Takes one of the item from the bag; false when it is gone.
async fn consume(state: &State, actor: &Actor<ActorState>, character: &Character, item: &InventoryItem) -> Result<bool, Error> {
    let Some(taken) = character.consume_item_uids(state.pool(), &[item.uid]).await? else {
        return Ok(false);
    };
    for taken in taken {
        actor.send(MsgItemInfo::remove_inventory_item(taken)).await?;
    }
    crate::audit::item("use_special", character.id(), item.uid, item.item_type, format_args!(""));
    Ok(true)
}

/// Saves the new look and shows it around.
async fn refresh(state: &State, actor: &Actor<ActorState>, character: &Character) -> Result<(), Error> {
    character.save(state).await?;
    let update = MsgPlayer::from(character);
    actor.send(update.clone()).await?;
    actor.screen().send_message(update).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dyes_keep_the_style_and_amulets_swap_the_body() {
        assert_eq!(dyed_hair(335, 9), 935);
        assert_eq!(dyed_hair(31, 5), 531);
        assert_eq!(other_body(671_003), 671_004);
        assert_eq!(other_body(2_002), 2_001);
        assert_eq!(other_body(1_234), 1_234, "not a player body");
    }
}
