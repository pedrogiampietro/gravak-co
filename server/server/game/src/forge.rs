//! The forge rules the NPCs follow, and the item menu that does the same
//! for an item worn or in the bag (Shift + right click: QUALITY, LEVEL,
//! UPGRADE, GEM SOCKET), so both ask the same and give the same:
//!
//! - quality: MagicArtisan, `1 + 100 / chance` DragonBalls, never fails;
//! - level: MagicArtisan below level 120 (Meteors), WeaponMaster above it
//!   (one DragonBall);
//! - +1 to +9: WuxingOven, two items of the same +n from the bag;
//!   +9 to +12: 12 DragonBalls, 25 DragonBalls, 4 DBScrolls;
//! - sockets: GodlyArtisan, a weapon's 1 and 5 DragonBalls, other gear's
//!   12 DragonBalls and then 7 StarDrills (or a ToughDrill's luck).

use tq_network::Actor;

use crate::entities::{Character, InventoryItem};
use crate::packets::{MsgItemInfo, MsgTalk, MsgUserInfo, TalkChannel};
use crate::{ActorState, Error, State};

pub const DRAGONBALL: u32 = 1_088_000;
pub const METEOR: u32 = 1_088_001;
pub const DBSCROLL: u32 = 720_028;
pub const TOUGH_DRILL: u32 = 1_200_005;
pub const STAR_DRILL: u32 = 1_200_006;
/// A ToughDrill opens the second socket this often (GodlyArtisan).
pub const TOUGH_DRILL_PERCENT: u32 = 12;
/// `Constants.SOCKET_RATE`, per mille: a level upgrade may open a socket,
/// a quality upgrade twice as often (MagicArtisan).
pub const SOCKET_PER_MILLE: u32 = 4;

/// Weapons (`4xxxxx` one-handed, `5xxxxx` two-handed and bows) socket at
/// the weapon's price.
pub fn is_weapon(item_type: u32) -> bool {
    matches!(item_type / 100_000, 4 | 5)
}

/// The next quality and its DragonBalls (MagicArtisan).
pub fn quality_step(item_type: u32, requirement: u32, exists: impl Fn(u32) -> bool) -> Option<(u32, u32)> {
    let next = crate::artisans::next_quality(item_type, exists)?;
    Some((next, crate::artisans::quality_cost(item_type, requirement)))
}

/// The next level and what it costs: Meteors at MagicArtisan below level
/// 120, one DragonBall at WeaponMaster from there to 130.
pub fn level_step(item_type: u32, requirement: u32, exists: impl Fn(u32) -> bool) -> Option<(u32, u32, u32)> {
    let next = crate::artisans::next_level(item_type, exists)?;
    if requirement < 120 {
        Some((next, METEOR, crate::artisans::level_cost(item_type)))
    } else {
        Some((next, DRAGONBALL, 1))
    }
}

/// What the plus after `plus` costs from +9 on (WuxingOven).
pub fn plus_price(plus: u8) -> Option<(u32, u32)> {
    match plus {
        9 => Some((DRAGONBALL, 12)),
        10 => Some((DRAGONBALL, 25)),
        11 => Some((DBSCROLL, 4)),
        _ => None,
    }
}

/// Below +9, the pair the item menu melts in: the one that raises the item
/// most, the cheapest of those. `pairs` are the +n of the bag's pairs.
pub fn best_pair(plus: u8, pairs: impl IntoIterator<Item = u8>) -> Option<(u8, u8)> {
    pairs
        .into_iter()
        .filter_map(|minor| Some((minor, crate::city_npcs::composed_plus(plus, minor)?)))
        .max_by_key(|(minor, result)| (*result, std::cmp::Reverse(*minor)))
}

/// What opening the next socket asks (GodlyArtisan): the catalyst, how
/// many, and whether it may fail (a ToughDrill).
pub fn socket_price(item_type: u32, first_open: bool, star_drills: usize) -> (u32, u32, bool) {
    match (is_weapon(item_type), first_open) {
        (true, false) => (DRAGONBALL, 1, false),
        (true, true) => (DRAGONBALL, 5, false),
        (false, false) => (DRAGONBALL, 12, false),
        (false, true) if star_drills >= 7 => (STAR_DRILL, 7, false),
        (false, true) => (TOUGH_DRILL, 1, true),
    }
}

fn name(item_type: u32) -> &'static str {
    match item_type {
        DRAGONBALL => "DragonBalls",
        METEOR => "Meteors",
        DBSCROLL => "DBScrolls",
        STAR_DRILL => "StarDrills",
        TOUGH_DRILL => "ToughDrill",
        _ => "items",
    }
}

async fn tell(actor: &Actor<ActorState>, character: &Character, text: impl Into<String>) -> Result<(), Error> {
    actor
        .send(MsgTalk::from_system(character.id(), TalkChannel::TopLeft, text.into()))
        .await?;
    Ok(())
}

/// Sends what changed: the catalysts gone, the item where it is, and the
/// hero's figures when it is worn.
async fn done(
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

fn owned(character: &Character, item_uid: u32) -> Option<InventoryItem> {
    character
        .equipment_items()
        .into_iter()
        .chain(character.inventory_items())
        .find(|item| item.uid == item_uid)
        .filter(|item| crate::shop::ShopCatalog::equipment_slot_for_item(item.item_type).is_some())
}

/// QUALITY: the item's next quality, as MagicArtisan gives it.
pub async fn quality(state: &State, actor: &Actor<ActorState>, character: &Character, item_uid: u32) -> Result<(), Error> {
    let catalog = state.shop_catalog();
    let Some(item) = owned(character, item_uid) else {
        return tell(actor, character, "Only equipment you own can be upgraded.").await;
    };
    let requirement = catalog.template(item.item_type).map_or(0, |template| template.level);
    let Some((next, cost)) = quality_step(item.item_type, requirement, |id| catalog.template(id).is_some()) else {
        return tell(actor, character, "This item cannot be upgraded further.").await;
    };
    let open_socket = crate::combat::next_roll() % 1_000 < SOCKET_PER_MILLE * 2;
    let Some((item, consumed)) = character
        .reforge_item(state.pool(), item_uid, next, DRAGONBALL, cost, open_socket)
        .await?
    else {
        return tell(actor, character, format!("You need {cost} DragonBalls to upgrade its quality.")).await;
    };
    done(state, actor, character, item, consumed).await?;
    crate::audit::economic("menu_quality", character.id(), 0, item.uid, cost);
    tell(actor, character, format!("The quality has been upgraded for {cost} DragonBalls.")).await
}

/// LEVEL: the item's next level, as MagicArtisan (and WeaponMaster above
/// level 120) gives it.
pub async fn level(state: &State, actor: &Actor<ActorState>, character: &Character, item_uid: u32) -> Result<(), Error> {
    let catalog = state.shop_catalog();
    let requirement = |item_type: u32| catalog.template(item_type).map_or(0, |template| template.level);
    let Some(item) = owned(character, item_uid) else {
        return tell(actor, character, "Only equipment you own can be upgraded.").await;
    };
    let Some((next, catalyst, cost)) =
        level_step(item.item_type, requirement(item.item_type), |id| catalog.template(id).is_some())
    else {
        return tell(actor, character, "This item cannot be upgraded further.").await;
    };
    if u32::from(character.entity().level()) < requirement(next) {
        return tell(actor, character, "You are too low level to wear the upgraded item!").await;
    }
    let open_socket = catalyst == METEOR && crate::combat::next_roll() % 1_000 < SOCKET_PER_MILLE;
    let Some((item, consumed)) = character
        .reforge_item(state.pool(), item_uid, next, catalyst, cost, open_socket)
        .await?
    else {
        return tell(actor, character, format!("You need {cost} {} to upgrade its level.", name(catalyst))).await;
    };
    done(state, actor, character, item, consumed).await?;
    crate::audit::economic("menu_level", character.id(), 0, item.uid, cost);
    tell(actor, character, format!("The level has been upgraded for {cost} {}.", name(catalyst))).await
}

/// UPGRADE: the item's next plus, as WuxingOven gives it.
pub async fn plus(state: &State, actor: &Actor<ActorState>, character: &Character, item_uid: u32) -> Result<(), Error> {
    let Some(item) = owned(character, item_uid) else {
        return tell(actor, character, "Only equipment you own can be upgraded.").await;
    };
    if item.plus >= 12 {
        return tell(actor, character, "This item is already +12.").await;
    }
    let (consumed, result) = if item.plus < 9 {
        // Pairs of the same +n in the bag, the item itself aside.
        let mut by_plus = std::collections::BTreeMap::<u8, Vec<InventoryItem>>::new();
        for owned in character.inventory_items() {
            if (1..=9).contains(&owned.plus) && owned.uid != item_uid {
                by_plus.entry(owned.plus).or_default().push(owned);
            }
        }
        let pairs = by_plus.iter().filter(|(_, items)| items.len() >= 2).map(|(plus, _)| *plus);
        let Some((minor, result)) = best_pair(item.plus, pairs) else {
            return tell(actor, character, "Put two items of the same +n (or +n Stones) in your bag to compose.").await;
        };
        (by_plus[&minor][..2].to_vec(), result)
    } else {
        let (catalyst, count) = plus_price(item.plus).expect("+9 to +11 have a price");
        let Some(paid) = character.bag_catalysts(catalyst, count, item_uid) else {
            return tell(actor, character, format!("You need {count} {} for +{}.", name(catalyst), item.plus + 1)).await;
        };
        (paid, item.plus + 1)
    };
    let Some(item) = character.compose_item_plus(state.pool(), item_uid, &consumed, result).await? else {
        return tell(actor, character, "The composing failed: the items changed.").await;
    };
    crate::audit::economic("menu_plus", character.id(), 0, item.uid, u32::from(result));
    done(state, actor, character, item, consumed).await?;
    tell(actor, character, format!("Your item is now +{result}.")).await
}

/// GEM SOCKET's open: the next socket, as GodlyArtisan opens it.
pub async fn socket(state: &State, actor: &Actor<ActorState>, character: &Character, item_uid: u32) -> Result<(), Error> {
    let Some(item) = owned(character, item_uid) else {
        return tell(actor, character, "Only equipment you own can be socketed.").await;
    };
    let first = item.socket_one != 0 || item.gem_one != 0;
    let second = item.socket_two != 0 || item.gem_two != 0;
    if first && second {
        return tell(actor, character, "This item already has two sockets.").await;
    }
    let (catalyst, count, chancy) = socket_price(item.item_type, first, character.count_items(STAR_DRILL));
    if chancy && crate::combat::next_roll() % 100 >= TOUGH_DRILL_PERCENT {
        let Some(drill) = character.consume_items(state.pool(), TOUGH_DRILL, 1).await? else {
            return tell(actor, character, "You need 7 StarDrills or a ToughDrill for the second socket.").await;
        };
        for used in drill {
            actor.send(MsgItemInfo::remove_inventory_item(used)).await?;
        }
        if let Some(star) = character.grant_item(state.pool(), state.shop_catalog(), STAR_DRILL, false).await? {
            actor.send(MsgItemInfo::inventory_item(star)).await?;
        }
        return tell(actor, character, "The ToughDrill did not open the socket. You got a StarDrill for future use.").await;
    }
    let Some((item, consumed)) = character.open_socket_paying(state.pool(), item_uid, catalyst, count).await? else {
        let need = if chancy { "7 StarDrills or a ToughDrill".to_owned() } else { format!("{count} {}", name(catalyst)) };
        return tell(actor, character, format!("You need {need} to open the socket.")).await;
    };
    done(state, actor, character, item, consumed).await?;
    crate::audit::economic("menu_socket", character.id(), 0, item.uid, count);
    tell(actor, character, "The socket is open.").await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_menu_asks_what_the_npcs_ask() {
        let all = |_| true;
        // MagicArtisan's quality (SunBlade, level 105).
        assert_eq!(quality_step(410_205, 105, all), Some((410_206, 2)));
        assert_eq!(quality_step(410_208, 105, all), Some((410_209, 8)));
        assert_eq!(quality_step(410_209, 105, all), None);
        // MagicArtisan below 120, WeaponMaster above.
        assert_eq!(level_step(410_115, 55, all).map(|(_, catalyst, cost)| (catalyst, cost)), Some((METEOR, 2)));
        assert_eq!(level_step(410_235, 120, all).map(|(_, catalyst, cost)| (catalyst, cost)), Some((DRAGONBALL, 1)));
        // WuxingOven.
        assert_eq!(plus_price(9), Some((DRAGONBALL, 12)));
        assert_eq!(plus_price(10), Some((DRAGONBALL, 25)));
        assert_eq!(plus_price(11), Some((DBSCROLL, 4)));
        assert_eq!(plus_price(8), None);
        // GodlyArtisan.
        assert_eq!(socket_price(410_205, false, 0), (DRAGONBALL, 1, false));
        assert_eq!(socket_price(410_205, true, 0), (DRAGONBALL, 5, false));
        assert_eq!(socket_price(130_037, false, 0), (DRAGONBALL, 12, false));
        assert_eq!(socket_price(130_037, true, 7), (STAR_DRILL, 7, false));
        assert_eq!(socket_price(130_037, true, 3), (TOUGH_DRILL, 1, true));
    }

    #[test]
    fn the_menu_melts_the_pair_that_raises_most() {
        // Two +8 Stones on a +0 item: +8; the cheapest pair when all give +1.
        assert_eq!(best_pair(0, [1, 8]), Some((8, 8)));
        assert_eq!(best_pair(5, [1, 3]), Some((1, 6)));
        assert_eq!(best_pair(8, [8, 9]), Some((8, 9)));
        assert_eq!(best_pair(9, [8]), None);
        assert_eq!(best_pair(2, []), None);
    }
}
