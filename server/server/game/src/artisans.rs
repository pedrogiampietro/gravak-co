//! Forge NPCs ported from the reference server scripts
//! (`ReduxConquerProject/.../Npcs/Market/[10062] MagicArtisan.cs`) and the
//! item formulas of `Structures/ConquerItem.cs`.
//!
//! The Magic Artisan never fails: quality costs `1 + 100 / chance`
//! DragonBalls and level costs `1 + 100 / chance` Meteors (at most 40),
//! where `chance` is the classic success rate of a single catalyst.

use crate::entities::Character;
use crate::packets::{MsgItemInfo, MsgPlayer, MsgTaskDialog, MsgUserInfo};
use crate::shop::ShopCatalog;
use crate::{ActorState, Error, State};
use tq_network::Actor;

pub const MAGIC_ARTISAN_ID: u32 = 10_062;
const MAGIC_ARTISAN_FACE: u16 = 6;
const DRAGONBALL: u32 = 1_088_000;
const METEOR: u32 = 1_088_001;
/// `Constants.SOCKET_RATE` (in percent): quality upgrades roll twice it.
const SOCKET_RATE_PER_MILLE: u32 = 4;

/// Equipment positions offered for upgrade, in the order the script lists
/// them, with their labels. The option id is `group * 10 + position`.
const SLOTS: [(u8, &str); 7] = [
    (1, "Helmet/Earrings/TaoCap "),
    (2, "Necklace/Bag "),
    (6, "Ring/Bracelet "),
    (4, "Weapon"),
    (5, "Shield "),
    (3, "Armor "),
    (8, "Boots "),
];

fn quality(item_type: u32) -> u32 {
    item_type % 10
}

fn equipment_level(item_type: u32) -> u32 {
    item_type % 1_000 / 10
}

fn equipment_type(item_type: u32) -> u32 {
    item_type / 1_000
}

fn equipment_sort(item_type: u32) -> u32 {
    item_type % 100_000 / 10_000
}

/// Head and body gear are the `1xxxxx` family. Redux checks only the sort
/// digit, which also matches blades (`410xxx`) and would price weapons with
/// the helmet table.
fn is_helmet(item_type: u32) -> bool {
    item_type / 100_000 == 1 && matches!(equipment_sort(item_type), 1 | 4)
}

fn is_armor(item_type: u32) -> bool {
    item_type / 100_000 == 1 && equipment_sort(item_type) == 3
}

fn is_shield(item_type: u32) -> bool {
    equipment_type(item_type) == 900
}

/// `GetNextItemQuality`: the next quality of the same item, or `None` when
/// the item is already Super or below Normal.
pub fn next_quality(item_type: u32, exists: impl Fn(u32) -> bool) -> Option<u32> {
    let quality = quality(item_type);
    if quality < 3 || quality == 9 {
        return None;
    }
    let base = if quality < 5 {
        item_type / 10 * 10 + 5
    } else {
        item_type
    };
    Some(base + 1).filter(|next| exists(*next))
}

/// `ChanceToUpgradeQuality`, in percent.
pub fn quality_chance(item_type: u32, level_requirement: u32) -> u32 {
    let quality = quality(item_type);
    if quality == 9 || quality < 3 {
        return 0;
    }
    let mut chance = match quality.max(5) {
        6 => 50,
        7 => 33,
        8 => 20,
        _ => 100,
    };
    if level_requirement > 70 {
        chance = chance * 100_u32.saturating_sub(level_requirement - 70) / 100;
    }
    chance.max(1)
}

/// `GetNextItemLevel`: the next level of the same item kind and quality.
pub fn next_level(item_type: u32, exists: impl Fn(u32) -> bool) -> Option<u32> {
    if quality(item_type) < 3 {
        return None;
    }
    (1..10)
        .map(|step| item_type + 10 * step)
        .find(|candidate| exists(*candidate))
        .filter(|next| equipment_type(*next) == equipment_type(item_type))
}

/// `ChanceToUpgradeLevel`, in percent.
pub fn level_chance(item_type: u32) -> u32 {
    let level = equipment_level(item_type);
    let mut chance: u32 = if is_shield(item_type) || is_armor(item_type) || is_helmet(item_type) {
        match level {
            5 => 50,
            6 => 40,
            7 => 30,
            8 | 9 => 20,
            _ => 500,
        }
    } else {
        match level {
            11 => 95,
            12 => 90,
            13 => 85,
            14 => 80,
            15 => 75,
            16 => 70,
            17 => 65,
            18 => 60,
            19 => 55,
            20 => 50,
            21 => 45,
            22 => 40,
            _ => 500,
        }
    };
    chance = match quality(item_type) {
        6 => chance * 90 / 100,
        7 => chance * 70 / 100,
        8 => chance * 30 / 100,
        9 => chance * 10 / 100,
        _ => chance,
    };
    chance.clamp(2, 100)
}

pub fn quality_cost(item_type: u32, level_requirement: u32) -> u32 {
    1 + 100 / quality_chance(item_type, level_requirement).max(1)
}

pub fn level_cost(item_type: u32) -> u32 {
    (1 + 100 / level_chance(item_type)).min(40)
}

/// Opening dialog: text, options and face.
pub fn opening(npc_id: u32) -> Option<(String, Vec<(u8, String)>, u16)> {
    (npc_id == MAGIC_ARTISAN_ID).then(|| {
        (
            "Hello! I can always succeed in upgrading item quality. But I`ll ask for a few more \
             dragon balls than Artisan Wind. I can also upgrade item levels. How can I help you?"
                .to_owned(),
            vec![
                (1, "Upgrade Quality".to_owned()),
                (2, "Upgrade Level".to_owned()),
                (255, "No thanks".to_owned()),
            ],
            MAGIC_ARTISAN_FACE,
        )
    })
}

/// What the script does for an answer.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Step {
    Say(String, Vec<(u8, String)>),
    UpgradeQuality { position: u8, next: u32, cost: u32 },
    UpgradeLevel { position: u8, next: u32, cost: u32 },
}

fn say(text: impl Into<String>, option: &str) -> Step {
    Step::Say(text.into(), vec![(255, option.to_owned())])
}

fn menu(text: &str, group: u8) -> Step {
    let mut options = SLOTS
        .iter()
        .map(|(position, label)| (group * 10 + position, (*label).to_owned()))
        .collect::<Vec<_>>();
    options.push((255, "I changed my mind. ".to_owned()));
    Step::Say(text.to_owned(), options)
}

/// The script, linkback by linkback, as in `[10062] MagicArtisan.cs`.
fn step(
    option: u8,
    worn: impl Fn(u8) -> Option<u32>,
    catalog: &ShopCatalog,
    player_level: u32,
    has: impl Fn(u32, u32) -> bool,
) -> Option<Step> {
    let exists = |item_type: u32| catalog.template(item_type).is_some();
    let requirement = |item_type: u32| catalog.template(item_type).map_or(0, |template| template.level);
    let position = option % 10;
    if !SLOTS.iter().any(|(slot, _)| *slot == position) && option > 2 {
        return None;
    }
    let missing = || {
        say(
            "There must be some mistake., You must be wearing an item before you may upgrade it!",
            "Nevermind",
        )
    };
    Some(match option {
        1 => menu("What item would you like me to upgrade the quality of?", 1),
        2 => menu("What item would you like me to upgrade the level of?", 3),
        11..=18 | 21..=28 => {
            let Some(item_type) = worn(position) else {
                return Some(missing());
            };
            let Some(next) = next_quality(item_type, exists) else {
                return Some(say(
                    if option < 20 {
                        "There must be some mistake. This item cannot be upgraded further!"
                    } else {
                        "There must be some mistake. This item cannot be upgraded!"
                    },
                    "Nevermind",
                ));
            };
            let cost = quality_cost(item_type, requirement(item_type));
            if option < 20 {
                Step::Say(
                    format!("It will take {cost} DragonBalls to upgrade the quality of this item."),
                    vec![
                        (position + 20, "Upgrade it".to_owned()),
                        (255, "Never mind".to_owned()),
                    ],
                )
            } else if !has(DRAGONBALL, cost) {
                say(
                    format!("You do not have the required {cost} DragonBalls to upgrade quality!"),
                    "Sorry",
                )
            } else {
                Step::UpgradeQuality {
                    position,
                    next,
                    cost,
                }
            }
        },
        31..=38 | 41..=48 => {
            let Some(item_type) = worn(position) else {
                return Some(missing());
            };
            let next = next_level(item_type, exists);
            let Some(next) = next.filter(|_| requirement(item_type) < 120) else {
                return Some(say(
                    "There must be some mistake. I cannot upgrade this item further.",
                    "Nevermind",
                ));
            };
            let cost = level_cost(item_type);
            if option < 40 {
                Step::Say(
                    format!("It will take {cost} Meteors to upgrade the level of this item."),
                    vec![
                        (position + 40, "Upgrade it".to_owned()),
                        (255, "Never mind".to_owned()),
                    ],
                )
            } else if player_level < requirement(next) {
                say("You are too low level to wear the upgraded item!", "Oops")
            } else if !has(METEOR, cost) {
                say(
                    format!("You do not have the required {cost} Meteors to upgrade this item's level!"),
                    "Sorry",
                )
            } else {
                Step::UpgradeLevel {
                    position,
                    next,
                    cost,
                }
            }
        },
        _ => return None,
    })
}

async fn send_dialog(actor: &Actor<ActorState>, npc_id: u32, text: String, options: Vec<(u8, String)>) -> Result<(), Error> {
    let mut builder = MsgTaskDialog::builder().text(text);
    for (option, label) in options {
        builder = builder.with_option(option, label);
    }
    let dialog = builder.and().with_avatar(MAGIC_ARTISAN_FACE).task_id(npc_id).build();
    actor.send_all(dialog).await?;
    Ok(())
}

/// Handles an answer to a forge NPC. Returns false for other NPCs.
pub async fn answer(
    state: &State,
    actor: &Actor<ActorState>,
    character: &Character,
    npc_id: u32,
    option: u8,
) -> Result<bool, Error> {
    if npc_id != MAGIC_ARTISAN_ID {
        return Ok(false);
    }
    let catalog = state.shop_catalog();
    let inventory = character.inventory_items();
    let has = |item_type: u32, count: u32| {
        inventory.iter().filter(|item| item.item_type == item_type).count() >= count as usize
    };
    let worn = |position: u8| character.equipment_item(position).map(|item| item.item_type);
    let Some(step) = step(option, worn, catalog, u32::from(character.entity().level()), has) else {
        return Ok(true);
    };
    let (position, next, catalyst, cost, socket_per_mille, done) = match step {
        Step::Say(text, options) => {
            send_dialog(actor, npc_id, text, options).await?;
            return Ok(true);
        },
        Step::UpgradeQuality { position, next, cost } => (
            position,
            next,
            DRAGONBALL,
            cost,
            SOCKET_RATE_PER_MILLE * 2,
            "The quality has been upgraded! Enjoy your new item.",
        ),
        Step::UpgradeLevel { position, next, cost } => (
            position,
            next,
            METEOR,
            cost,
            SOCKET_RATE_PER_MILLE,
            "The level has been upgraded! Enjoy your new item.",
        ),
    };
    let open_socket = crate::combat::next_roll() % 1_000 < socket_per_mille;
    let Some((item, consumed)) = character
        .reforge_equipment(state.pool(), position, next, catalyst, cost, open_socket)
        .await?
    else {
        send_dialog(
            actor,
            npc_id,
            "There must be some mistake., You must be wearing an item before you may upgrade it!".to_owned(),
            vec![(255, "Nevermind".to_owned())],
        )
        .await?;
        return Ok(true);
    };
    for catalyst in consumed {
        actor.send(MsgItemInfo::remove_inventory_item(catalyst)).await?;
    }
    actor.send(MsgItemInfo::equipment_item(item)).await?;
    character.refresh_equipment_resources(catalog);
    actor.send(MsgUserInfo::from(character)).await?;
    actor.send(MsgPlayer::from(character)).await?;
    actor.screen().send_message(MsgPlayer::from(character)).await?;
    crate::audit::economic("magic_artisan", character.id(), npc_id, item.uid, cost);
    send_dialog(actor, npc_id, done.to_owned(), vec![(255, "Thanks!".to_owned())]).await?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quality_follows_the_reference_formula() {
        let all = |_| true;
        // Normal (5) blade goes to Refined (6), then Unique, Elite, Super.
        assert_eq!(next_quality(410_335, all), Some(410_336));
        // Below Normal jumps to Normal first.
        assert_eq!(next_quality(410_333, all), Some(410_336));
        assert_eq!(next_quality(410_339, all), None);
        assert_eq!(quality_chance(410_336, 30), 50);
        assert_eq!(quality_cost(410_336, 30), 3);
        assert_eq!(quality_cost(410_338, 30), 6);
        // High level items get rarer: level 100 Elite is 20% * 70%.
        assert_eq!(quality_chance(410_998, 100), 14);
    }

    #[test]
    fn level_follows_the_reference_formula() {
        let exists = |id| [410_115, 410_125, 410_135].contains(&id);
        assert_eq!(next_level(410_115, exists), Some(410_125));
        assert_eq!(next_level(410_135, exists), None);
        // A level 11 Normal weapon: 95% -> 2 Meteors.
        assert_eq!(level_chance(410_115), 95);
        assert_eq!(level_cost(410_115), 2);
        // Elite level 20 weapon: 50% * 30% = 15% -> 7 Meteors.
        assert_eq!(level_cost(410_208), 7);
        // Levels outside the table clamp to 100%, which still costs two.
        assert_eq!(level_cost(410_015), 2);
    }

    #[test]
    fn opening_matches_the_reference_dialog() {
        let (text, options, face) = opening(MAGIC_ARTISAN_ID).expect("magic artisan");
        assert!(text.starts_with("Hello! I can always succeed in upgrading item quality."));
        assert_eq!(options[0], (1, "Upgrade Quality".to_owned()));
        assert_eq!(face, 6);
        assert!(opening(1).is_none());
    }
}
