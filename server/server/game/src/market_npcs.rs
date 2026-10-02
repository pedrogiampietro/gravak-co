//! The Market service NPCs, after the reference server scripts
//! (`Npcs/Market/*.cs`): ProficiencyGod, GodlyArtisan, WeaponMaster
//! (Equipment Master), LadyLuck, MillionaireLee and Alex (first rebirth).

use tq_network::Actor;

use crate::entities::Character;
use crate::packets::{ActionType, MsgAction, MsgItemInfo, MsgPlayer, MsgTaskDialog, MsgUserInfo};
use crate::{ActorState, Error, State};

pub const PROFICIENCY_GOD_ID: u32 = 941;
pub const GODLY_ARTISAN_ID: u32 = 10_065;
pub const EQUIPMENT_MASTER_ID: u32 = 7_050;
pub const LADY_LUCK_ID: u32 = 923;
pub const MILLIONAIRE_LEE_ID: u32 = 5_004;
pub const ALEX_ID: u32 = 3_600;
pub const ETHEREAL_ID: u32 = 1_234;
/// Super TortoiseGem, Ethereal's price per blessing point.
const SUPER_TORTOISE_GEM: u32 = 700_073;
const MAX_BLESS: u8 = 7;
/// Alex's answers for a Blessed Rebirth: 170 + profession.
const BLESSED_REBIRTH: u8 = 170;

const DRAGONBALL: u32 = 1_088_000;
const METEOR: u32 = 1_088_001;
const METEOR_SCROLL: u32 = 720_027;
const DRAGONBALL_SCROLL: u32 = 720_028;
const EXP_BALL: u32 = 723_700;
const TOUGH_DRILL: u32 = 1_200_005;
const STAR_DRILL: u32 = 1_200_006;
const CELESTIAL_STONE: u32 = 721_259;
/// Super Phoenix Gem; the other super gems follow every 10.
const BASE_GEM: u32 = 700_003;
/// Chance, in percent, that a ToughDrill opens the second socket.
const TOUGH_DRILL_PERCENT: u32 = 12;

/// ProficiencyGod's weapons: (linkback, label). The weapon type is the
/// linkback plus 400.
const WEAPONS: [(u8, &str); 17] = [
    (10, "Blade "),
    (80, "Club "),
    (20, "Sword "),
    (100, "Bow "),
    (160, "Spear "),
    (161, "Wand "),
    (30, "Hook "),
    (41, "Whip "),
    (130, "PoleAxe "),
    (140, "LongHammer"),
    (110, "Glaive "),
    (180, "Halbert"),
    (21, "Backsword "),
    (81, "Scepter "),
    (90, "Dagger "),
    (60, "Hammer "),
    (50, "Axe"),
];
/// ProficiencyGod's "Yes." answer for `WEAPONS[i]`.
const CONFIRM_BASE: u8 = 200;

/// Equipment slots offered by the forge NPCs: (linkback digit, label).
const GEAR: [(u8, &str); 6] = [
    (1, "Helmet/Earrings/TaoCap "),
    (2, "Necklace/Bag "),
    (6, "Ring/Bracelet "),
    (5, "Shield "),
    (3, "Armor "),
    (8, "Boots "),
];

/// Redux `reborns`: (path, forget, skill). The path is the old profession
/// type times ten plus the new one (Trojan 1, Warrior 2, Water 3, Fire 4,
/// Archer 5).
const REBORN_PATHS: &[(u8, bool, u32)] = &[
    (11, false, 3050), (12, true, 1115), (13, true, 1115), (14, true, 1115), (15, true, 1115),
    (22, false, 3060), (25, true, 1025), (25, true, 1051), (25, true, 1320), (25, true, 1015),
    (24, true, 1025), (24, true, 1051), (24, true, 1320), (24, true, 1015), (23, true, 1015),
    (23, true, 1051), (23, true, 1320), (21, true, 1020), (21, true, 1025), (21, true, 1051),
    (51, false, 5002), (52, false, 5002), (53, false, 5002), (54, false, 5002), (55, false, 5000),
    (51, true, 8001), (51, true, 8000), (51, true, 8003), (51, true, 9000), (51, true, 8002), (51, true, 8030),
    (52, true, 8001), (52, true, 8000), (52, true, 8003), (52, true, 9000), (52, true, 8002), (52, true, 8030),
    (53, true, 8001), (53, true, 8000), (53, true, 8003), (53, true, 9000), (53, true, 8002), (53, true, 8030),
    (54, true, 8001), (54, true, 8000), (54, true, 8003), (54, true, 9000), (54, true, 8002), (54, true, 8030),
    (33, false, 3090),
    (31, true, 1000), (31, true, 1001), (31, true, 1055), (31, true, 1075), (31, true, 1100), (31, true, 1175),
    (31, true, 1170), (31, true, 1010), (31, true, 1050), (31, true, 1125), (31, true, 5001),
    (32, true, 1000), (32, true, 1001), (32, true, 1055), (32, true, 1075), (32, true, 1100), (32, true, 1175),
    (32, true, 1170), (32, true, 1010), (32, true, 1050), (32, true, 1125), (32, true, 5001),
    (34, true, 1001), (34, true, 1085), (34, true, 1090), (34, true, 1095), (34, true, 1100), (34, true, 1170),
    (34, true, 1125), (34, true, 5001),
    (35, true, 1000), (35, true, 1001), (35, true, 1055), (35, true, 1085), (35, true, 1100), (35, true, 1175),
    (35, true, 1170), (35, true, 1010), (35, true, 1050), (35, true, 1125), (35, true, 5001),
    (44, false, 3080),
    (41, true, 1150), (41, true, 1180), (41, true, 1120), (41, true, 1002), (41, true, 1160), (41, true, 1165),
    (41, true, 1010), (41, true, 1125), (41, true, 5001),
    (42, true, 1150), (42, true, 1180), (42, true, 1120), (42, true, 1002), (42, true, 1160), (42, true, 1165),
    (42, true, 1010), (42, true, 1125), (42, true, 5001),
    (43, true, 1001), (43, true, 1002), (43, true, 1150), (43, true, 1180), (43, true, 1160), (43, true, 1165),
    (43, true, 1125), (43, true, 5001),
    (45, true, 1150), (45, true, 1180), (45, true, 1120), (45, true, 1002), (45, true, 1160), (45, true, 1165),
    (45, true, 1010), (45, true, 1125), (45, true, 5001),
];

type Options = Vec<(u8, String)>;

fn roll_index(len: usize) -> usize {
    use rand::Rng;
    rand::thread_rng().gen_range(0..len.max(1))
}

fn roll_percent(percent: u32) -> bool {
    use rand::Rng;
    rand::thread_rng().gen_range(0..100) < percent
}

fn face(npc_id: u32) -> Option<u16> {
    Some(match npc_id {
        PROFICIENCY_GOD_ID | EQUIPMENT_MASTER_ID => 9,
        GODLY_ARTISAN_ID => 54,
        LADY_LUCK_ID => 3,
        MILLIONAIRE_LEE_ID => 102,
        ALEX_ID => 35,
        ETHEREAL_ID => 67,
        _ => return None,
    })
}

fn options(list: &[(u8, &str)]) -> Options {
    list.iter().map(|(id, label)| (*id, (*label).to_owned())).collect()
}

fn gear_menu(group: u8) -> Options {
    let mut list = GEAR
        .iter()
        .map(|(slot, label)| (group * 10 + slot, (*label).to_owned()))
        .collect::<Vec<_>>();
    list.push((255, "I changed my mind. ".to_owned()));
    list
}

/// Trojan 1, Warrior 2, Water Taoist 3, Fire Taoist 4, Archer 5.
fn profession_type(class: u8) -> u8 {
    match class {
        10..=15 => 1,
        20..=25 => 2,
        132..=135 => 3,
        142..=145 => 4,
        40..=45 => 5,
        _ => 0,
    }
}

fn reborn_level(class: u8) -> u16 {
    if profession_type(class) == 3 {
        110
    } else {
        120
    }
}

fn can_reborn(class: u8, rebirths: u8, level: u16) -> bool {
    rebirths == 0 && class % 10 == 5 && profession_type(class) != 0 && level >= reborn_level(class)
}

/// The free attribute points a first rebirth leaves (Redux table).
pub fn reborn_points(level: u16, water_taoist: bool) -> u16 {
    match (level, water_taoist) {
        (110 | 111, _) => 20,
        (112 | 113, _) => 21,
        (114 | 115, _) => 23,
        (116 | 117, _) => 26,
        (118 | 119, _) => 30,
        (120 | 121, true) => 35,
        (120, false) => 20,
        (121, false) => 21,
        (122 | 123, true) => 41,
        (122, false) => 23,
        (123, false) => 26,
        (124 | 125, true) => 48,
        (124, false) => 30,
        (125, false) => 35,
        (126 | 127, true) => 56,
        (126, false) => 41,
        (127, false) => 48,
        (128, true) => 65,
        (128, false) => 56,
        (129, _) => 65,
        (130.., _) => 75,
        _ => 0,
    }
}

/// The skills a rebirth path learns and forgets.
pub fn reborn_skills(old_class: u8, target: u8) -> (Vec<u32>, Vec<u32>) {
    let path = if profession_type(old_class) == 5 {
        50 + target
    } else {
        profession_type(old_class) * 10 + target
    };
    let (forget, learn): (Vec<_>, Vec<_>) = REBORN_PATHS
        .iter()
        .filter(|(row, _, _)| *row == path)
        .partition(|(_, forget, _)| *forget);
    (
        learn.into_iter().map(|(_, _, skill)| skill).collect(),
        forget.into_iter().map(|(_, _, skill)| skill).collect(),
    )
}

fn reborn_class(target: u8) -> Option<u8> {
    Some(match target {
        1 => 11,
        2 => 21,
        3 => 132,
        4 => 142,
        5 => 41,
        _ => return None,
    })
}

/// Redux `DownLevelItem`: the reborn's gear goes back to its first levels.
pub fn down_level(item_type: u32) -> Option<u32> {
    let kind = item_type / 1_000;
    let level = match kind {
        111 | 113 | 114 | 117 | 118 | 120 | 121 | 130 | 131 | 132 | 133 | 134 | 580 | 900 => 0,
        150 | 151 | 152 | 500 | 160 => 1,
        410 | 420 | 421 | 430 | 440 | 450 | 460 | 480 | 490 | 510 | 530 | 540 | 560 | 561 => 2,
        _ => return None,
    };
    Some(kind * 1_000 + level * 10 + item_type % 10)
}

pub fn opening(npc_id: u32, character: &Character) -> Option<(String, Options, u16)> {
    let face = face(npc_id)?;
    let (text, options) = match npc_id {
        PROFICIENCY_GOD_ID => (
            "I can help you master your weapon by channeling the power inside ExpBalls. What weapon would you like to train in? ".to_owned(),
            weapon_page(0),
        ),
        GODLY_ARTISAN_ID => (
            "Hello , did you knew that an socketed equipment is better than one without sockets?It costs DragonBalls or ToughDrills/StarDrills depending on the item.".to_owned(),
            options(&[(1, "Socket my weapon"), (2, "Socket my gears"), (255, "No. I like it this way.")]),
        ),
        EQUIPMENT_MASTER_ID => (
            "Hello, did you noticed that MagicArtisan can only level items up to 120?I can upgrade it above 120 up to 130 ,but it costs 1 DragonBall for each upgrade.".to_owned(),
            options(&[(2, "Yes, I want."), (255, "No. I like it this way.")]),
        ),
        LADY_LUCK_ID => (
            "I always have the worst luck...".to_owned(),
            options(&[(255, "Sorry to hear that.")]),
        ),
        MILLIONAIRE_LEE_ID => (
            "If you have more rare items then you know what to do with, I can help you pack them!".to_owned(),
            options(&[(1, "Pack Meteors"), (2, "Pack DragonBalls"), (255, "I don't have that problem.")]),
        ),
        ETHEREAL_ID => (
            "A good set of equipment can help you a lot in future battles.I can help you by upgrading blessed attribute.Keep in mind that each upgrade costs TortoiseGems witch are rare and expensive.".to_owned(),
            options(&[(1, "Upgrade blessed attribute."), (255, "Nothing right now, thanks.")]),
        ),
        ALEX_ID => {
            let level = character.entity().level();
            if character.rebirths() > 0 {
                (
                    "I have helped you as much as I can. Even heroes have their limits.".to_owned(),
                    options(&[(255, "Thanks")]),
                )
            } else if can_reborn(character.current_class(), 0, level) {
                (
                    "Even bold adventurers eventually grow tired of their journeys. I help these heroes by offering them life anew.".to_owned(),
                    options(&[(1, "I wish to reborn."), (255, "I'm not tired!")]),
                )
            } else {
                (
                    "Hello young one! I help people seek the ultimate goal of rebirth. You are not yet experienced enough to undertake this adventure. Please come back later.".to_owned(),
                    options(&[(255, "I will.")]),
                )
            }
        },
        _ => return None,
    };
    Some((text, options, face))
}

/// ProficiencyGod's weapon pages: 4, 4, 4 and 5 weapons.
fn weapon_page(page: usize) -> Options {
    let (start, end) = match page {
        0 => (0, 4),
        1 => (4, 8),
        2 => (8, 12),
        _ => (12, 17),
    };
    let mut list = WEAPONS[start..end]
        .iter()
        .map(|(id, label)| (*id, (*label).to_owned()))
        .collect::<Vec<_>>();
    if page < 3 {
        list.push((2 + page as u8, "Next Page".to_owned()));
    }
    list.push((
        255,
        if page == 0 { "No. I will do it myself." } else { "I changed my mind. " }.to_owned(),
    ));
    list
}

async fn send(actor: &Actor<ActorState>, npc_id: u32, text: impl Into<String>, options: Options) -> Result<(), Error> {
    let mut builder = MsgTaskDialog::builder().text(text.into());
    for (option, label) in options {
        builder = builder.with_option(option, label);
    }
    let dialog = builder
        .and()
        .with_avatar(face(npc_id).unwrap_or(1))
        .task_id(npc_id)
        .build();
    actor.send_all(dialog).await?;
    Ok(())
}

async fn say(actor: &Actor<ActorState>, npc_id: u32, text: impl Into<String>, option: &str) -> Result<(), Error> {
    send(actor, npc_id, text, vec![(255, option.to_owned())]).await
}

async fn removed(actor: &Actor<ActorState>, items: Vec<crate::entities::InventoryItem>) -> Result<(), Error> {
    for item in items {
        actor.send(MsgItemInfo::remove_inventory_item(item)).await?;
    }
    Ok(())
}

/// Handles an answer to one of these NPCs. Returns false for other NPCs.
pub async fn answer(
    state: &State,
    actor: &Actor<ActorState>,
    character: &Character,
    npc_id: u32,
    option: u8,
) -> Result<bool, Error> {
    match npc_id {
        PROFICIENCY_GOD_ID => proficiency_god(state, actor, character, option).await?,
        GODLY_ARTISAN_ID => godly_artisan(state, actor, character, option).await?,
        EQUIPMENT_MASTER_ID => equipment_master(state, actor, character, option).await?,
        MILLIONAIRE_LEE_ID => millionaire_lee(state, actor, character, option).await?,
        ALEX_ID => alex(state, actor, character, option).await?,
        ETHEREAL_ID => ethereal(state, actor, character, option).await?,
        LADY_LUCK_ID => {},
        _ => return Ok(false),
    }
    Ok(true)
}

async fn proficiency_god(state: &State, actor: &Actor<ActorState>, character: &Character, option: u8) -> Result<(), Error> {
    const NPC: u32 = PROFICIENCY_GOD_ID;
    const PICK: &str = "Choose the weapon you want to upgrade its proficiency from the list below. ";
    if (2..=4).contains(&option) {
        return send(actor, NPC, PICK, weapon_page(usize::from(option - 1))).await;
    }
    let (index, confirmed) = if option >= CONFIRM_BASE {
        (usize::from(option - CONFIRM_BASE), true)
    } else if let Some(index) = WEAPONS.iter().position(|(id, _)| *id == option) {
        (index, false)
    } else {
        return Ok(());
    };
    let Some((linkback, _)) = WEAPONS.get(index) else {
        return Ok(());
    };
    let weapon_type = u32::from(*linkback) + 400;
    let Some(progress) = character.weapon_type_proficiency(weapon_type) else {
        return say(actor, NPC, "You must have the proficiency for that weapon at least level 1!", "Nevermind").await;
    };
    if progress.level > 12 {
        return say(actor, NPC, "It seems that your weapon proficiency  is at it's highest level.", "Nevermind").await;
    }
    let cost = usize::from(progress.level) + 1;
    if !confirmed {
        return send(
            actor,
            NPC,
            format!("Are you sure you want to spend  {cost} ExpBalls to improve it?"),
            vec![
                (CONFIRM_BASE + index as u8, "Yes.".to_owned()),
                (255, "No thanks.".to_owned()),
            ],
        )
        .await;
    }
    let Some(balls) = character.consume_items(state.pool(), EXP_BALL, cost).await? else {
        return say(
            actor,
            NPC,
            format!("There must be some mistake. You do not have the required amount of ExpBalls, you must bring +{cost}!"),
            "Nevermind",
        )
        .await;
    };
    removed(actor, balls).await?;
    character.raise_weapon_proficiency(state.pool(), weapon_type).await?;
    if let Some(packet) = crate::packets::MsgWeaponSkill::for_all(character.weapon_proficiencies())
        .into_iter()
        .find(|packet| packet.weapon_type == weapon_type)
    {
        actor.send(packet).await?;
    }
    crate::audit::economic("proficiency_god", character.id(), NPC, weapon_type, cost as u32);
    say(actor, NPC, "It is done! ", "No thanks.").await
}

async fn godly_artisan(state: &State, actor: &Actor<ActorState>, character: &Character, option: u8) -> Result<(), Error> {
    const NPC: u32 = GODLY_ARTISAN_ID;
    let open = |item: &crate::entities::InventoryItem| {
        (
            item.socket_one != 0 || item.gem_one != 0,
            item.socket_two != 0 || item.gem_two != 0,
        )
    };
    match option {
        1 | 10 => {
            let Some(weapon) = character.equipment_item(4) else {
                return say(
                    actor,
                    NPC,
                    "There must be some mistake. You must be wearing a weapon before I can help you socket it!",
                    "Nevermind",
                )
                .await;
            };
            let (first, second) = open(&weapon);
            if second {
                return say(
                    actor,
                    NPC,
                    "There must be some mistake. Your weapon already has the maximum number of sockets!",
                    "Nevermind",
                )
                .await;
            }
            let cost = if first { 5 } else { 1 };
            if option == 1 {
                return send(
                    actor,
                    NPC,
                    format!("It will cost {cost} DragonBall(s) to socket this weapon. Are you positive you wish to continue?"),
                    options(&[(10, "I'm Sure!"), (255, "Nevermind")]),
                )
                .await;
            }
            let Some(balls) = character.consume_items(state.pool(), DRAGONBALL, cost).await? else {
                return say(
                    actor,
                    NPC,
                    format!("There must be some mistake. Your do not have the {cost} DragonBall(s) required"),
                    "Nevermind",
                )
                .await;
            };
            removed(actor, balls).await?;
            if let Some(item) = character.open_equipment_socket(state.pool(), 4, first).await? {
                actor.send(MsgItemInfo::equipment_item(item)).await?;
            }
            crate::audit::economic("godly_artisan", character.id(), NPC, weapon.uid, cost as u32);
            say(actor, NPC, "It is done! Please enjoy your new equipment.", "Thanks!").await
        },
        2 => {
            send(
                actor,
                NPC,
                "It costs 12 DragonBalls for the first socket and 7 StarDrills for second.If you consider yourself lucky try ToughDrill! ",
                options(&[(3, "Make the first socket "), (4, "Make the second socket "), (255, "No. I like it this way.")]),
            )
            .await
        },
        3 => {
            send(
                actor,
                NPC,
                "The first socket requires 12 DragonBalls and there is no turning back! What item would you like me to socket?",
                gear_menu(1),
            )
            .await
        },
        4 => {
            send(
                actor,
                NPC,
                "Opening the second socket is a delicate matter. I can guarentee an upgrade by using 7 StarDrills or you can try your luck using a ToughDrill.",
                options(&[(5, "Use ToughDrill"), (6, "Use 7 StarDrills"), (255, "No. I like it this way.")]),
            )
            .await
        },
        5 => {
            send(
                actor,
                NPC,
                "There is no guarentee this upgrade will be a success. Each attempt will require a ToughDrill. What item would you like to try to socket?",
                gear_menu(2),
            )
            .await
        },
        6 => {
            send(
                actor,
                NPC,
                "This upgrade is guarenteed to work but requires 7 StarDrills. What item would you like to try to socket?",
                gear_menu(3),
            )
            .await
        },
        11..=38 if GEAR.iter().any(|(slot, _)| *slot == option % 10) => {
            let position = option % 10;
            let Some(item) = character.equipment_item(position) else {
                return say(
                    actor,
                    NPC,
                    "There must be some mistake., You must be wearing an item before you may socket it!",
                    "Nevermind",
                )
                .await;
            };
            let (first, second) = open(&item);
            let group = option / 10;
            if group == 1 && first {
                return say(actor, NPC, "There must be some mistake. This item already has a socket!", "Nevermind").await;
            }
            if group > 1 && !first {
                return say(
                    actor,
                    NPC,
                    "There must be some mistake. You must open the first socket before you may attempt the second!",
                    "Nevermind",
                )
                .await;
            }
            if group > 1 && second {
                return say(actor, NPC, "There must be some mistake. This item already has two sockets!", "Nevermind").await;
            }
            let (catalyst, count, missing) = match group {
                1 => (DRAGONBALL, 12, "There must be some mistake. You do not have the 12 DragonBalls required to socket this item!"),
                2 => (TOUGH_DRILL, 1, "There must be some mistake. You do not have a ToughDrill!"),
                _ => (STAR_DRILL, 7, "There must be some mistake. You do not have the required 7 StarDrills!"),
            };
            let Some(used) = character.consume_items(state.pool(), catalyst, count).await? else {
                return say(actor, NPC, missing, "Nevermind").await;
            };
            removed(actor, used).await?;
            if group == 2 && !roll_percent(TOUGH_DRILL_PERCENT) {
                if let Some(drill) = character.grant_item(state.pool(), state.shop_catalog(), STAR_DRILL, false).await? {
                    actor.send(MsgItemInfo::inventory_item(drill)).await?;
                }
                return say(
                    actor,
                    NPC,
                    "I was not able to open the second socket for you. I will give you a StarDrill for future use.",
                    "Damn...",
                )
                .await;
            }
            if let Some(item) = character.open_equipment_socket(state.pool(), position, group > 1).await? {
                actor.send(MsgItemInfo::equipment_item(item)).await?;
            }
            crate::audit::economic("godly_artisan", character.id(), NPC, item.uid, count as u32);
            say(actor, NPC, "It is done! Please enjoy your new equipment.", "Thanks!").await
        },
        _ => Ok(()),
    }
}

async fn equipment_master(state: &State, actor: &Actor<ActorState>, character: &Character, option: u8) -> Result<(), Error> {
    const NPC: u32 = EQUIPMENT_MASTER_ID;
    match option {
        2 => {
            send(
                actor,
                NPC,
                "It costs 1 DragonBall for any item above level 110.Are you really sure you want to do that?! ",
                options(&[(3, "Yeah, upgrade it! "), (255, "No. I like it this way.")]),
            )
            .await
        },
        3 => {
            send(
                actor,
                NPC,
                "Each upgrade requires 1 DragonBall and there is no turning back! What item would you like me to upgrade?",
                options(&[
                    (11, "Helmet/Earrings/TaoCap "),
                    (12, "Necklace/Bag "),
                    (16, "Ring/Bracelet "),
                    (14, "Right Weapon "),
                    (15, "Shield/Left Weapon "),
                    (13, "Armor "),
                    (18, "Boots "),
                    (255, "I changed my mind. "),
                ]),
            )
            .await
        },
        11..=16 | 18 => {
            let catalog = state.shop_catalog();
            let requirement = |item_type: u32| catalog.template(item_type).map_or(0, |template| template.level);
            let position = option % 10;
            let Some(item) = character.equipment_item(position) else {
                return say(
                    actor,
                    NPC,
                    "There must be some mistake. You must be wearing an item before you may upgrade it!",
                    "Nevermind",
                )
                .await;
            };
            if character.count_items(DRAGONBALL) == 0 {
                return say(
                    actor,
                    NPC,
                    "There must be some mistake. You must pay an DragonBall before you may upgrade it!",
                    "Nevermind",
                )
                .await;
            }
            if requirement(item.item_type) <= 110 {
                return say(actor, NPC, "There must be some mistake. Your item is not above level 110!", "Nevermind").await;
            }
            let Some(next) = crate::artisans::next_level(item.item_type, |id| catalog.template(id).is_some()) else {
                return say(actor, NPC, "There must be some mistake. Your item can't be upgraded anymore !", "Nevermind").await;
            };
            if u32::from(character.entity().level()) < requirement(next) {
                return say(
                    actor,
                    NPC,
                    "There must be some mistake. You are not high level enough to wear the item after upgrade!",
                    "Nevermind",
                )
                .await;
            }
            let Some((upgraded, used)) = character
                .reforge_equipment(state.pool(), position, next, DRAGONBALL, 1, false)
                .await?
            else {
                return Ok(());
            };
            removed(actor, used).await?;
            actor.send(MsgItemInfo::equipment_item(upgraded)).await?;
            character.refresh_equipment_resources(catalog);
            actor.send(MsgUserInfo::from(character)).await?;
            crate::audit::economic("equipment_master", character.id(), NPC, upgraded.uid, 1);
            Ok(())
        },
        _ => Ok(()),
    }
}

async fn millionaire_lee(state: &State, actor: &Actor<ActorState>, character: &Character, option: u8) -> Result<(), Error> {
    const NPC: u32 = MILLIONAIRE_LEE_ID;
    let (input, output, missing, done) = match option {
        1 => (
            METEOR,
            METEOR_SCROLL,
            "Fool! You need 10 Meteors if you want me to help you pack them.",
            "It is done. Enjoy your MeteorScroll.",
        ),
        2 => (
            DRAGONBALL,
            DRAGONBALL_SCROLL,
            "Fool! You need 10 DragonBalls if you want me to help you pack them.",
            "It is done. Enjoy your DBScroll.",
        ),
        _ => return Ok(()),
    };
    let Some(used) = character.consume_items(state.pool(), input, 10).await? else {
        return say(actor, NPC, missing, "I'm sorry!").await;
    };
    removed(actor, used).await?;
    if let Some(scroll) = character.grant_item(state.pool(), state.shop_catalog(), output, false).await? {
        actor.send(MsgItemInfo::inventory_item(scroll)).await?;
    }
    say(actor, NPC, done, "Wow, thanks!").await
}

async fn alex(state: &State, actor: &Actor<ActorState>, character: &Character, option: u8) -> Result<(), Error> {
    const NPC: u32 = ALEX_ID;
    let class = character.current_class();
    let level = character.entity().level();
    let ready = can_reborn(class, character.rebirths(), level);
    match option {
        1 if ready => {
            send(
                actor,
                NPC,
                "By accepting the gift of rebirth, you will start your life again. You can unlock great strength as well as new skills in the process! Are you sure you're ready to give up your current life? ",
                options(&[(2, "I'm ready."), (255, "Not yet!")]),
            )
            .await
        },
        1 => {
            say(
                actor,
                NPC,
                "Ooh my! Lets not get ahead of ourselves. You are not yet ready to become reborn.",
                "Sorry.",
            )
            .await
        },
        2 if character.count_items(CELESTIAL_STONE) == 0 => {
            say(
                actor,
                NPC,
                "Rebirth requires the unlocking of great power. Please make sure you have a CelestialStone before coming to me.",
                "Sorry.",
            )
            .await
        },
        2 => {
            send(
                actor,
                NPC,
                "You can choose to either receive a gem of great power or bless your equipment for even greater strength. Which would you like?",
                options(&[(3, "Blessed Rebirth"), (5, "Gem Rebirth"), (255, "I'm not sure...")]),
            )
            .await
        },
        3 => {
            let mut list = [(1, "Trojan"), (2, "Warrior"), (3, "Water Taoist"), (4, "Fire Taoist"), (5, "Archer")]
                .into_iter()
                .map(|(job, label)| (BLESSED_REBIRTH + job, label.to_owned()))
                .collect::<Vec<_>>();
            list.push((255, "I'm not sure...".to_owned()));
            send(actor, NPC, "What profession would you like in your new life?", list).await
        },
        5 => {
            send(
                actor,
                NPC,
                "What type of gem would you like?",
                options(&[
                    (20, "Phoenix Gem"),
                    (21, "Dragon Gem"),
                    (22, "Fury Gem"),
                    (23, "Rainbow Gem"),
                    (6, "More Options"),
                    (255, "I'm not sure..."),
                ]),
            )
            .await
        },
        6 => {
            send(
                actor,
                NPC,
                "What type of gem would you like?",
                options(&[
                    (24, "Kylin Gem"),
                    (25, "Violet Gem"),
                    (26, "Moon Gem"),
                    (5, "Previous Options"),
                    (255, "I'm not sure..."),
                ]),
            )
            .await
        },
        20..=26 => {
            // The profession answers carry the gem: 100 + gem * 10 + job.
            let gem = option - 20;
            let mut list = [(1, "Trojan"), (2, "Warrior"), (3, "Water Taoist"), (4, "Fire Taoist"), (5, "Archer")]
                .into_iter()
                .map(|(job, label)| (100 + gem * 10 + job, label.to_owned()))
                .collect::<Vec<_>>();
            list.push((255, "I'm not sure...".to_owned()));
            send(actor, NPC, "What profession would you like in your new life?", list).await
        },
        101..=165 if (1..=5).contains(&(option % 10)) => {
            let gem = u32::from((option - 100) / 10);
            let target = option % 10;
            if !ready {
                return say(
                    actor,
                    NPC,
                    "Lets not get ahead of ourselves! You are not ready to achieve rebirth. Please come back later.",
                    "Sorry.",
                )
                .await;
            }
            let Some(stone) = character.consume_items(state.pool(), CELESTIAL_STONE, 1).await? else {
                return say(
                    actor,
                    NPC,
                    "Rebirth requires the unlocking of great power. Please make sure you have a CelestialStone before coming to me.",
                    "Sorry.",
                )
                .await;
            };
            removed(actor, stone).await?;
            rebirth(state, actor, character, target, Some(BASE_GEM + gem * 10)).await
        },
        171..=175 => {
            if !ready {
                return say(
                    actor,
                    NPC,
                    "Lets not get ahead of ourselves! You are not ready to achieve rebirth. Please come back later.",
                    "Sorry.",
                )
                .await;
            }
            let Some(stone) = character.consume_items(state.pool(), CELESTIAL_STONE, 1).await? else {
                return say(
                    actor,
                    NPC,
                    "Rebirth requires the unlocking of great power. Please make sure you have a CelestialStone before coming to me.",
                    "Sorry.",
                )
                .await;
            };
            removed(actor, stone).await?;
            rebirth(state, actor, character, option - BLESSED_REBIRTH, None).await
        },
        _ => Ok(()),
    }
}

/// A Gem Rebirth gives `gem`; a Blessed Rebirth (`None`) blesses one
/// random worn item that has no blessing yet.
/// Ethereal: +2 blessing for as many Super TortoiseGems as the blessing.
async fn ethereal(state: &State, actor: &Actor<ActorState>, character: &Character, option: u8) -> Result<(), Error> {
    const NPC: u32 = ETHEREAL_ID;
    const SLOTS: [(u8, &str); 7] = [
        (1, "Helmet/Earrings/TaoCap "),
        (2, "Necklace/Bag "),
        (6, "Ring/Bracelet "),
        (4, "Right Weapon "),
        (5, "Shield/Left Weapon "),
        (3, "Armor "),
        (8, "Boots "),
    ];
    if option == 1 {
        let mut list = SLOTS
            .iter()
            .map(|(slot, label)| (10 + slot, (*label).to_owned()))
            .collect::<Vec<_>>();
        list.push((255, "I changed my mind. ".to_owned()));
        return send(
            actor,
            NPC,
            "I see, upgrading an items blessed attribute will help you receive less damage. What item would you like me upgrade?",
            list,
        )
        .await;
    }
    let (position, confirmed) = match option {
        11..=18 => (option - 10, false),
        21..=28 => (option - 20, true),
        _ => return Ok(()),
    };
    let Some(item) = character.equipment_item(position) else {
        return say(
            actor,
            NPC,
            "There must be some mistake. You must be wearing an item before you may upgrade it!",
            "Nevermind",
        )
        .await;
    };
    if item.bless == 0 {
        return say(actor, NPC, "Your item must be blessed in order to upgrade this attribute!", "Nevermind").await;
    }
    if item.bless >= MAX_BLESS {
        return say(actor, NPC, "Your item has maximum bless attribute!", "Nevermind").await;
    }
    let cost = usize::from(item.bless);
    if !confirmed {
        return send(
            actor,
            NPC,
            format!("To upgrade this item you need to pay {cost} Super TortoiseGems! Are you sure?"),
            vec![(20 + position, "Yes.".to_owned()), (255, "No thanks.".to_owned())],
        )
        .await;
    }
    let Some(gems) = character.consume_items(state.pool(), SUPER_TORTOISE_GEM, cost).await? else {
        return say(
            actor,
            NPC,
            format!("To upgrade this item you need to pay {cost} Super TortoiseGems witch you don't have!"),
            "I see.",
        )
        .await;
    };
    removed(actor, gems).await?;
    if let Some(item) = character
        .bless_item(state.pool(), item.uid, (item.bless + 2).min(MAX_BLESS))
        .await?
    {
        actor.send(MsgItemInfo::equipment_item(item)).await?;
    }
    crate::audit::economic("ethereal", character.id(), NPC, item.uid, cost as u32);
    say(actor, NPC, "Congratulations you have upgraded you items blessed attribute!", "Thanks!").await
}

async fn rebirth(state: &State, actor: &Actor<ActorState>, character: &Character, target: u8, gem: Option<u32>) -> Result<(), Error> {
    let Some(new_class) = reborn_class(target) else {
        return Ok(());
    };
    let old_class = character.current_class();
    let points = reborn_points(character.entity().level(), profession_type(old_class) == 3);
    let (learn, forget) = reborn_skills(old_class, target);
    match gem {
        Some(gem) => {
            if let Some(gem) = character.grant_item(state.pool(), state.shop_catalog(), gem, false).await? {
                actor.send(MsgItemInfo::inventory_item(gem)).await?;
            }
        },
        None => {
            let unblessed = character
                .equipment_items()
                .into_iter()
                .filter(|item| item.bless == 0)
                .collect::<Vec<_>>();
            if !unblessed.is_empty() {
                let pick = unblessed[roll_index(unblessed.len())];
                if let Some(item) = character.bless_item(state.pool(), pick.uid, 1).await? {
                    actor.send(MsgItemInfo::equipment_item(item)).await?;
                }
            }
        },
    }
    character.rebirth(state.pool(), new_class, points, &learn, &forget).await?;
    let catalog = state.shop_catalog();
    for position in 1..=8 {
        let Some(item) = character.equipment_item(position) else {
            continue;
        };
        let Some(lower) = down_level(item.item_type).filter(|lower| *lower != item.item_type && catalog.template(*lower).is_some())
        else {
            continue;
        };
        if let Some((item, _)) = character.reforge_equipment(state.pool(), position, lower, 0, 0, false).await? {
            actor.send(MsgItemInfo::equipment_item(item)).await?;
        }
    }
    character.refresh_equipment_resources(catalog);
    character.save(state).await?;
    for skill_id in &forget {
        actor
            .send(MsgAction::new(character.id(), *skill_id, 0, 0, ActionType::DropMagic))
            .await?;
    }
    for skill in character.skills() {
        actor.send(crate::packets::MsgSkill::from(skill)).await?;
    }
    actor.send(MsgUserInfo::from(character)).await?;
    let update = MsgPlayer::from(character);
    actor.send(update.clone()).await?;
    actor.screen().send_message(update).await?;
    actor
        .send(crate::packets::MsgTalk::from_system(
            character.id(),
            crate::packets::TalkChannel::System,
            format!("{} has been reborned successfully. Congratulations!", character.entity().name()),
        ))
        .await?;
    crate::audit::economic("first_rebirth", character.id(), ALEX_ID, u32::from(new_class), 1);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reborn_paths_follow_the_reference_table() {
        // Trojan to Trojan learns Cruel Shade; to Warrior forgets Hercules.
        assert_eq!(reborn_skills(15, 1), (vec![3050], vec![]));
        assert_eq!(reborn_skills(15, 2), (vec![], vec![1115]));
        // Archer paths are 50 + target.
        assert_eq!(reborn_skills(45, 5), (vec![5000], vec![]));
        assert_eq!(reborn_class(3), Some(132));
        assert!(can_reborn(135, 0, 110));
        assert!(!can_reborn(15, 0, 119));
        assert!(!can_reborn(14, 0, 130));
        assert_eq!(reborn_points(120, false), 20);
        assert_eq!(reborn_points(120, true), 35);
        assert_eq!(reborn_points(130, false), 75);
    }

    #[test]
    fn rebirth_lowers_gear_but_keeps_quality() {
        assert_eq!(down_level(410_339), Some(410_029));
        assert_eq!(down_level(130_109), Some(130_009));
        assert_eq!(down_level(150_255), Some(150_015));
        assert_eq!(down_level(1_088_000), None);
    }

    #[test]
    fn proficiency_pages_list_every_weapon() {
        let listed = (0..4)
            .flat_map(weapon_page)
            .filter(|(id, _)| WEAPONS.iter().any(|(weapon, _)| weapon == id))
            .count();
        assert_eq!(listed, WEAPONS.len());
        assert!(usize::from(CONFIRM_BASE) + WEAPONS.len() < 255);
    }
}
