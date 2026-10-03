//! Player death, revival and PK points, after the reference server
//! (Redux `Player.Kill`/`Player.Revive`).

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::entities::{Character, Flags};
use crate::packets::{
    InteractAction, MapFlags, MsgGroundItem, MsgInteract, MsgItemInfo, MsgPlayer, MsgTalk, MsgUserInfo, TalkChannel,
};
use crate::state::GroundItem;
use crate::{Error, State};

/// The client offers Revive after this long.
pub const REVIVE_DELAY: Duration = Duration::from_secs(20);
/// A killer's name flashes blue this long.
pub const PK_FLASH: Duration = Duration::from_secs(180);
/// Attacking a player who is not fair game flashes the name this long.
pub const ATTACK_FLASH: Duration = Duration::from_secs(60);
/// One PK point wears off this often while online.
pub const PK_DECAY: Duration = Duration::from_secs(6 * 60);
pub const RED_NAME_POINTS: u16 = 30;
pub const BLACK_NAME_POINTS: u16 = 100;
/// Black names who die are sent to the prison.
pub const JAIL_MAP: u32 = 6000;
pub const JAIL_POINT: (u16, u16) = (29, 72);
/// The arena (ArenaGuard): no items lost, no PK points, revive in place.
pub const ARENA_MAP: u32 = 1_005;
/// Chance, in percent, that each inventory item falls on death.
const INVENTORY_DROP_PERCENT: u32 = 5;
const DROP_OFFSETS: [(i16, i16); 9] = [(0, 0), (1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (-1, -1), (1, -1), (-1, 1)];

pub fn name_flags(points: u16) -> Flags {
    if points >= BLACK_NAME_POINTS {
        Flags::BLACK_NAME
    } else if points >= RED_NAME_POINTS {
        Flags::RED_NAME
    } else {
        Flags::NONE
    }
}

/// Whether a kill on this map is a crime at all.
pub fn map_counts_pk(flags: MapFlags) -> bool {
    !flags.intersects(
        MapFlags::PK_FIELD | MapFlags::PK_DISABLED | MapFlags::SYNDICATE_MAP | MapFlags::FFA_MAP | MapFlags::PRISON_MAP,
    )
}

/// PK points for killing a player with `victim_flags`: none for flashing
/// or black names (they are fair game), 3 for red names, 10 otherwise.
pub fn pk_penalty(victim_flags: Flags) -> Option<u16> {
    if victim_flags.intersects(Flags::BLUE_FLASHING_NAME | Flags::BLACK_NAME) {
        None
    } else if victim_flags.contains(Flags::RED_NAME) {
        Some(3)
    } else {
        Some(10)
    }
}

/// An attack on a player who is not fair game: where PK counts, the
/// attacker's name starts flashing blue at once, so everyone around sees who
/// started the fight (Redux `CheckCrime`), not only after a kill.
pub async fn check_crime(state: &State, attacker: &Character, victim: &Character) -> Result<(), Error> {
    let map_flags = state
        .try_map(attacker.entity().map_id())
        .map(|map| map.flags())
        .unwrap_or_default();
    if !map_counts_pk(map_flags) || pk_penalty(victim.entity().flags()).is_none() {
        return Ok(());
    }
    let already_flashing = attacker.entity().flags().contains(Flags::BLUE_FLASHING_NAME);
    attacker.flash_name(ATTACK_FLASH);
    if !already_flashing {
        let update = MsgPlayer::from(attacker);
        attacker.owner().send(update.clone()).await?;
        if let Ok(screen) = attacker.try_screen() {
            screen.send_message(update).await?;
        }
    }
    Ok(())
}

/// What a character is told when PK points move their name to another
/// colour; `None` while it keeps the same one.
pub fn name_change_notice(points_before: u16, points_after: u16) -> Option<&'static str> {
    let (before, after) = (name_flags(points_before), name_flags(points_after));
    if before.bits() == after.bits() {
        return None;
    }
    Some(if after.contains(Flags::BLACK_NAME) {
        "Your name has turned black! The guards will hunt you, and if you die you will be sent to jail."
    } else if after.contains(Flags::RED_NAME) && before.contains(Flags::BLACK_NAME) {
        "Your name is red again."
    } else if after.contains(Flags::RED_NAME) {
        "Your name has turned red! If another player kills you, you may lose your equipment."
    } else {
        "Your name is white again."
    })
}

/// Tells `character` that their name changed colour, if it did.
pub async fn notify_name_change(character: &Character, points_before: u16) -> Result<(), Error> {
    if let Some(notice) = name_change_notice(points_before, character.kill_points()) {
        character
            .owner()
            .send(MsgTalk::from_system(character.id(), TalkChannel::System, notice))
            .await?;
    }
    Ok(())
}

/// Chance, in percent, that a player killer makes each worn piece fall: 35
/// for black names, 20 for red names, none otherwise.
pub fn equipment_drop_percent(flags: Flags) -> u32 {
    if flags.contains(Flags::BLACK_NAME) {
        35
    } else if flags.contains(Flags::RED_NAME) {
        20
    } else {
        0
    }
}

fn timestamp() -> u32 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_millis() as u32)
        .unwrap_or_default()
}

fn roll_percent(percent: u32) -> bool {
    use rand::Rng;
    rand::thread_rng().gen_range(0..100) < percent
}

/// Runs once when `victim` reaches zero life: the death blow, the dropped
/// items, the killer's PK points and the prison for black names.
pub async fn kill_character(
    state: &State,
    victim: &Character,
    killer_id: u32,
    killer: Option<&Character>,
) -> Result<(), Error> {
    if !victim.mark_dead() {
        return Ok(());
    }
    let location = victim.entity().location();
    let blow = MsgInteract {
        timestamp: timestamp(),
        character_id: killer_id,
        target_id: victim.id(),
        x: location.x,
        y: location.y,
        action: InteractAction::Kill as u32,
        value: 0,
        reserved: [0; 12],
    };
    let owner = victim.owner();
    owner.send(blow.clone()).await?;
    owner.send(MsgUserInfo::from(victim)).await?;
    if let Ok(screen) = victim.try_screen() {
        screen.send_message(blow).await?;
        screen.send_message(MsgPlayer::from(victim)).await?;
    }
    let killer_name = state
        .entity(killer_id)
        .map_or_else(|| format!("#{killer_id}"), |killer| killer.basic().name().to_owned());
    tracing::info!(
        target: "pk",
        map = victim.entity().map_id(),
        "{} was killed by {}",
        victim.entity().name(),
        killer_name
    );

    let map_id = victim.entity().map_id();
    if map_id == ARENA_MAP {
        victim.save(state).await?;
        return Ok(());
    }
    let map_flags = state.try_map(map_id).map(|map| map.flags()).unwrap_or_default();
    let mut dropped = Vec::new();
    for item in victim.inventory_items() {
        if !roll_percent(INVENTORY_DROP_PERCENT) {
            continue;
        }
        if let Some(item) = victim.drop_inventory_item(state.pool(), item.uid).await? {
            owner.send(MsgItemInfo::remove_inventory_item(item)).await?;
            dropped.push(item.item_type);
        }
    }
    // Slain by a player where PK counts, red and black names may lose each
    // piece they wear (Albetros `DropItems`).
    let slain_by_player = killer.is_some_and(|killer| killer.id() != victim.id());
    let equipment_percent = equipment_drop_percent(victim.entity().flags());
    if slain_by_player && map_counts_pk(map_flags) && equipment_percent > 0 {
        for item in (1..=9).filter_map(|slot| victim.equipment_item(slot)) {
            if !roll_percent(equipment_percent) {
                continue;
            }
            if let Some(removed) = drop_equipment(state, victim, item).await? {
                owner.send(MsgItemInfo::remove_equipment_item(item, item.position)).await?;
                dropped.push(removed.item_type);
            }
        }
    }
    for (index, item_type) in dropped.into_iter().enumerate() {
        let (dx, dy) = DROP_OFFSETS[index % DROP_OFFSETS.len()];
        let ground = GroundItem {
            uid: state.next_ground_item_uid(),
            item_id: item_type,
            map_id,
            x: location.x.saturating_add_signed(dx),
            y: location.y.saturating_add_signed(dy),
            owner_id: 0,
            silver: 0,
        };
        state.insert_ground_item(ground);
        let packet = MsgGroundItem::new(ground.uid, ground.item_id, ground.x, ground.y);
        owner.send(packet.clone()).await?;
        if let Ok(screen) = victim.try_screen() {
            screen.send_message(packet).await?;
        }
    }

    let killer = killer.filter(|killer| killer.id() != victim.id() && map_counts_pk(map_flags));
    let penalty = pk_penalty(victim.entity().flags());
    if let (Some(killer), Some(penalty)) = (killer, penalty) {
        let points_before = killer.kill_points();
        killer.set_kill_points(points_before.saturating_add(penalty));
        notify_name_change(killer, points_before).await?;
        killer.flash_name(PK_FLASH);
        killer.save(state).await?;
        killer.owner().send(MsgUserInfo::from(killer)).await?;
        let update = MsgPlayer::from(killer);
        killer.owner().send(update.clone()).await?;
        if let Ok(screen) = killer.try_screen() {
            screen.send_message(update).await?;
        }
    }

    if victim.kill_points() >= BLACK_NAME_POINTS && map_id != JAIL_MAP {
        victim.teleport(state, JAIL_MAP, JAIL_POINT).await?;
    }
    victim.save(state).await?;
    Ok(())
}

/// Takes a worn item off and out of the game; it then lies on the ground.
async fn drop_equipment(
    state: &State,
    victim: &Character,
    item: crate::entities::InventoryItem,
) -> Result<Option<crate::entities::InventoryItem>, Error> {
    let occupied = victim.inventory_items();
    let Some(free) = (0..40u8).find(|slot| !occupied.iter().any(|owned| owned.position == *slot)) else {
        return Ok(None);
    };
    if victim.unequip_item(state.pool(), item.uid, free).await?.is_none() {
        return Ok(None);
    }
    victim.drop_inventory_item(state.pool(), item.uid).await
}

/// Revive and Pray: the dead player gets up where they lie.
pub async fn revive_here(character: &Character) -> Result<(), Error> {
    character.entity().restore_health();
    character.clear_death();
    character.refresh_after_revive();
    let reborn = crate::packets::MsgAction::from_character(character, 0, crate::packets::ActionType::Reborn);
    let player = MsgPlayer::from(character);
    let owner = character.owner();
    owner.send(MsgUserInfo::from(character)).await?;
    owner.send(reborn.clone()).await?;
    owner.send(player.clone()).await?;
    if let Ok(screen) = character.try_screen() {
        screen.send_message(reborn).await?;
        screen.send_message(player).await?;
    }
    Ok(())
}

/// Revives a dead character at the revive point of the map's reborn map.
/// `false` while the revive delay is still running.
pub async fn revive(state: &State, character: &Character) -> Result<bool, Error> {
    if !character.can_revive() {
        return Ok(false);
    }
    character.entity().restore_health();
    character.clear_death();
    character.refresh_after_revive();
    let map_id = character.entity().map_id();
    if map_id == ARENA_MAP {
        // Arena fighters get up where they fell.
        revive_here(character).await?;
        return Ok(true);
    }
    if map_id == crate::guild_war::GUILD_MAP && crate::guild_war::running() {
        let (prison, x, y) = crate::guild_war::WAR_PRISON;
        character.teleport(state, prison, (x, y)).await?;
        return Ok(true);
    }
    let reborn_map = state.try_map(map_id).map(|map| map.reborn_map()).unwrap_or(map_id);
    if let Ok(map) = state.try_map(reborn_map) {
        let point = map.revive_point();
        let mut origin = character.entity().location();
        origin.x = point.x as u16;
        origin.y = point.y as u16;
        let landing = map.landing_position(origin, 8).unwrap_or(origin);
        character.teleport(state, reborn_map, (landing.x, landing.y)).await?;
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worn_pieces_fall_by_name_colour() {
        assert_eq!(equipment_drop_percent(Flags::NONE), 0);
        assert_eq!(equipment_drop_percent(Flags::BLUE_FLASHING_NAME), 0);
        assert_eq!(equipment_drop_percent(Flags::RED_NAME), 20);
        assert_eq!(equipment_drop_percent(Flags::BLACK_NAME), 35);
    }

    #[test]
    fn name_colour_changes_are_announced_once() {
        assert_eq!(name_change_notice(20, 29), None);
        assert!(name_change_notice(20, 30).is_some_and(|text| text.contains("red")));
        assert!(name_change_notice(95, 105).is_some_and(|text| text.contains("black")));
        assert_eq!(name_change_notice(100, 99), Some("Your name is red again."));
        assert_eq!(name_change_notice(30, 29), Some("Your name is white again."));
        assert_eq!(name_change_notice(105, 101), None);
    }

    #[test]
    fn names_follow_pk_points() {
        assert!(name_flags(29).is_empty());
        assert!(name_flags(30).contains(Flags::RED_NAME));
        assert!(name_flags(100).contains(Flags::BLACK_NAME));
    }

    #[test]
    fn penalties_follow_the_reference_server() {
        assert_eq!(pk_penalty(Flags::NONE), Some(10));
        assert_eq!(pk_penalty(Flags::RED_NAME), Some(3));
        assert_eq!(pk_penalty(Flags::BLACK_NAME), None);
        assert_eq!(pk_penalty(Flags::BLUE_FLASHING_NAME), None);
        assert!(map_counts_pk(MapFlags::NONE));
        assert!(!map_counts_pk(MapFlags::PK_FIELD));
        assert!(!map_counts_pk(MapFlags::PRISON_MAP));
    }
}
