//! City service NPCs, after the reference server scripts (`Npcs/Twin City`,
//! `Npcs/Market` and the city Assistants): travel, barber, arena, jail,
//! mines, CP exchange, body size, attribute reallotment, virtue prizes,
//! the homeless UnknownMan, the CelestialStone and +1..+12 composing.

use tq_network::Actor;

use crate::entities::Character;
use crate::packets::{MsgItemInfo, MsgPlayer, MsgTaskDialog, MsgUserInfo};
use crate::{ActorState, Error, State};

pub const GENERAL_PEACE_ID: u32 = 10_054;
pub const BARBER_ID: u32 = 10_002;
pub const ARENA_GUARD_ID: u32 = 10_021;
pub const CAPTAIN_ID: u32 = 42;
pub const CAPTAIN_LI_ID: u32 = 43;
pub const CP_ADMIN_ID: u32 = 2_071;
pub const SURGEON_ID: u32 = 3_381;
pub const SHOP_BOY_ID: u32 = 10_063;
pub const CELESTIAL_TAO_ID: u32 = 350_050;
pub const UNKNOWN_MAN_ID: u32 = 3_825;
pub const CELESTINE_ID: u32 = 20_005;
pub const SHELBY_ID: u32 = 300_000;
pub const WUXING_OVEN_ID: u32 = 35_016;
pub const STARLIT_ID: u32 = 600_055;
pub const LOVE_STONE_ID: u32 = 390;
pub const TINTER_ID: u32 = 5_030;
/// The dyeing shop's exit.
const TWIN_CITY_SHOP: (u32, u16, u16) = (1_002, 425, 352);
pub const GUILD_DIRECTOR_ID: u32 = 10_003;
const GUILD_LEVEL: u16 = 90;
const GUILD_PRICE: u32 = 1_000_000;
/// The GuildDirector's name box.
const GUILD_NAME_INPUT: u8 = 6;
/// Mine assistants: (npc, required level, destination).
const MINE_ASSISTANTS: [(u32, u16, (u32, u16, u16)); 4] = [
    (4_293, 0, (1_028, 155, 94)),
    (125, 40, (1_025, 29, 71)),
    (126, 70, (1_026, 138, 104)),
    (127, 70, (1_027, 138, 104)),
];

const DRAGONBALL: u32 = 1_088_000;
const METEOR: u32 = 1_088_001;
const DRAGONBALL_SCROLL: u32 = 720_028;
const WINE: u32 = 722_185;
const CLEAN_WATER: u32 = 721_258;
const CELESTIAL_STONE: u32 = 721_259;
/// The seven normal gems a CelestialStone takes.
const NORMAL_GEMS: [u32; 7] = [700_001, 700_011, 700_021, 700_031, 700_041, 700_051, 700_061];

const DESERT_CITY: (u32, u16, u16) = (1_000, 971, 666);
const ARENA: (u32, u16, u16) = (1_005, 51, 71);
const ARENA_FEE: u32 = 50;
const JAIL: (u32, u16, u16) = (6_000, 29, 72);
const JAIL_FEE: u32 = 1_000;
const JAIL_EXIT: (u32, u16, u16) = (1_002, 517, 352);
const DYE_SHOP: (u32, u16, u16) = (1_008, 24, 24);
const BARBER_FEE: u32 = 1_000;
const WINE_PRICE: u32 = 1_000;
/// UnknownMan rests twelve hours between ExpBall lessons.
const UNKNOWN_MAN_REST_SECONDS: i64 = 12 * 3_600;
const UNKNOWN_MAN_TIMER: &str = "unknown_man";

type Options = Vec<(u8, String)>;

fn options(list: &[(u8, &str)]) -> Options {
    list.iter().map(|(id, label)| (*id, (*label).to_owned())).collect()
}

fn face(npc_id: u32) -> u16 {
    match npc_id {
        BARBER_ID => 111,
        GENERAL_PEACE_ID | ARENA_GUARD_ID | CP_ADMIN_ID | SHOP_BOY_ID => 7,
        CAPTAIN_ID | CAPTAIN_LI_ID => 37,
        SURGEON_ID | CELESTIAL_TAO_ID | SHELBY_ID => 54,
        UNKNOWN_MAN_ID | WUXING_OVEN_ID => 67,
        CELESTINE_ID => 9,
        STARLIT_ID => 24,
        LOVE_STONE_ID => 6,
        GUILD_DIRECTOR_ID => 7,
        _ => 9,
    }
}

/// Equipment choices of the forge menus: (slot digit, label).
const GEAR: [(u8, &str); 7] = [
    (1, "Helmet/Earrings/TaoCap "),
    (6, "Ring/Bracelet "),
    (2, "Necklace/Bag "),
    (8, "Boots "),
    (3, "Armor "),
    (5, "Shield "),
    (4, "Weapon "),
];

/// Redux `Process_ComposePacket`: a main item and two minor items of the
/// same +n. Minors above the main item give it their plus; otherwise it
/// gains one. Nothing goes past +9.
pub fn composed_plus(main: u8, minor: u8) -> Option<u8> {
    if main >= 9 || minor == 0 {
        return None;
    }
    Some(if minor > main { minor.min(9) } else { main + 1 })
}

/// The worn slot picked for +1..+9 composing, per character.
fn compose_slots() -> &'static parking_lot::Mutex<std::collections::HashMap<u32, u8>> {
    static SLOTS: std::sync::OnceLock<parking_lot::Mutex<std::collections::HashMap<u32, u8>>> =
        std::sync::OnceLock::new();
    SLOTS.get_or_init(Default::default)
}

/// Pairs of minor items in the bag by plus: (plus, two uids).
fn minor_pairs(character: &Character) -> Vec<(u8, [u32; 2])> {
    let mut by_plus = std::collections::BTreeMap::<u8, Vec<u32>>::new();
    for item in character.inventory_items() {
        if (1..=9).contains(&item.plus) {
            by_plus.entry(item.plus).or_default().push(item.uid);
        }
    }
    by_plus
        .into_iter()
        .filter(|(_, uids)| uids.len() >= 2)
        .map(|(plus, uids)| (plus, [uids[0], uids[1]]))
        .collect()
}

/// The Warden's answer to a black name: how many PK points still keep them
/// here and how long those take to wear off while online.
fn sentence_text(points: u16) -> String {
    let left = points.saturating_sub(crate::death::BLACK_NAME_POINTS) + 1;
    let minutes = u64::from(left) * crate::death::PK_DECAY.as_secs() / 60;
    format!(
        "You must stay and think about what you've done! You have {points} PK points;          I will let you out below {}. That is {left} more, about {minutes} minutes online.",
        crate::death::BLACK_NAME_POINTS
    )
}

fn gear_menu(group: u8) -> Options {
    let mut list = GEAR
        .iter()
        .map(|(slot, label)| (group * 10 + slot, (*label).to_owned()))
        .collect::<Vec<_>>();
    list.push((255, "I changed my mind. ".to_owned()));
    list
}

pub fn opening(npc_id: u32, character: &Character) -> Option<(String, Options, u16)> {
    let (text, options): (String, Options) = match npc_id {
        GENERAL_PEACE_ID => (
            "This is the way to the Desert City. Although you are excellent, it is dangerous to go ahead.".into(),
            options(&[(1, "I want to go."), (255, "I think I will stay here")]),
        ),
        BARBER_ID => (
            "I can help you find a new hairstyle just for you! Only 1,000 silver to re-invent yourself.".into(),
            options(&[(1, "Cut my Hair"), (255, "Just passing by.")]),
        ),
        ARENA_GUARD_ID => (
            "The arena is open. Welcome to challenge other people. The admission fee is only 50 silver. If you PK in the arena, you will not gain or lose any experience or items equipped, and will get revived at the place you die. The Kungfu circle is very dangerous, I suggest you PK in area.".into(),
            options(&[(1, "Enter the arena."), (255, "Just passing by.")]),
        ),
        CAPTAIN_ID => {
            if character.kill_points() >= crate::death::BLACK_NAME_POINTS {
                (
                    sentence_text(character.kill_points()),
                    options(&[(255, "I shall..")]),
                )
            } else {
                (
                    "You are no danger to anyone. I can help you leave if you'd like.".into(),
                    options(&[(1, "Yes please"), (255, "Not yet")]),
                )
            }
        },
        CAPTAIN_LI_ID => (
            "What can I do for you?".into(),
            options(&[(1, "Visit the jail."), (255, "Just passing by.")]),
        ),
        id if MINE_ASSISTANTS.iter().any(|(npc, _, _)| *npc == id) => (
            "Hello, I am the assistant of the mine union. If you want to enter the mine cave, I can send you.".into(),
            options(&[(1, "Yes Please"), (255, "Just passing by.")]),
        ),
        CP_ADMIN_ID => (
            "I can help you turn DragonBalls into CP.".into(),
            options(&[(1, "DragonBall (215 CP)"), (2, "DBScroll (2150 CP)"), (255, "No thank you.")]),
        ),
        SURGEON_ID => (
            "Hello, are you satisfield with your stature? I can change your body size for 1 DragonBall. Would you like to change it?".into(),
            options(&[(1, "Yeah."), (255, "No.")]),
        ),
        SHOP_BOY_ID => (
            "Our shop is famous for dyeing. If you want to have your equipment dyed, please wear them before you enter. You have a wide choice of colors. One meteor will be charged before you try the colors. Do you want a try?".into(),
            options(&[(1, "Yes, here is a meteor"), (2, "Do you have any good wine?"), (255, "Never mind")]),
        ),
        CELESTIAL_TAO_ID => (
            "The DragonBall is really a precious item.You can use it to reallot your attribute points after you have been reborned. After reborning at 70 or above you can reallot your attribute points freely. Well, if you have a DragonBall, I can help you do that.".into(),
            options(&[(1, "Please help me."), (255, "Definitely.")]),
        ),
        UNKNOWN_MAN_ID => {
            if character.count_items(WINE) > 0 {
                (
                    "Is that wine for me? I'm so very thirsty!".into(),
                    options(&[(2, "Here you go."), (255, "No! Get away from me")]),
                )
            } else {
                (
                    "Hey, you there! Please take pity on a homeless old man! I am so hungry but can`t even afford a bowl of noodles. Could you help me out here? The gods always smile on kindness, you know!".into(),
                    options(&[(1, "Here are 100 silvers. Take care."), (255, "No way. I don`t do charity.")]),
                )
            }
        },
        CELESTINE_ID => (
            "People are pursuing greather achievement during their lives, but none can make it due to the limit of human constitution.".into(),
            options(&[(1, "What does it mean?"), (255, "I don't belive it.")]),
        ),
        SHELBY_ID => {
            if character.entity().level() >= 70 {
                (
                    "We hope all can help each other. If you power level the newbies, we may reward you with Experience, Meteors or Dragonballs. Are you interested?".into(),
                    options(&[
                        (1, "Tell me more details."),
                        (5, "Check my virtue points."),
                        (6, "Claim prize."),
                        (255, "Just passing by."),
                    ]),
                )
            } else {
                (
                    "You must be level 70 at least or more in order to get virtue points and be able to exchange for am prize.".into(),
                    options(&[(255, "I see.")]),
                )
            }
        },
        WUXING_OVEN_ID => (
            "A good set of equipment can help you a lot in future battles.Is there something I can do for you?".into(),
            options(&[(1, "Enchant HP to my gears."), (2, "Composing Upgrade."), (255, "Nothing right now, thanks.")]),
        ),
        STARLIT_ID => (
            "I can help you to divorce your spouse. You just will have to agree to a divorce. I will also need a MeteorTear. If you have this item and you really want to get divorced, we will do it soon.".into(),
            options(&[(1, "Yes, I want a divorce."), (255, "Just passing by.")]),
        ),
        GUILD_DIRECTOR_ID => (
            "Greetings! I am the guild director, in charge of administrating and managing guilds. What business do you have with me?".into(),
            options(&[
                (1, "Create a guild"),
                (3, "Inquire about my guild."),
                (21, "Manage my guild"),
                (255, "Just passing by."),
            ]),
        ),
        TINTER_ID => (
            "I am the master of all colors. What equipment would you like to dye today?".into(),
            options(&[(1, "Helmet"), (2, "Armor"), (3, "Shield"), (9, "Take me back to Twin City."), (255, "Nevermind")]),
        ),
        LOVE_STONE_ID => (
            "Sometimes, you may feel lonely. Yeah, you do need someone to be with you. I can understand that. Now answer me, have you ever fallen in love with someone?".into(),
            options(&[(1, "Maybe, what should I do?"), (255, "Never.")]),
        ),
        _ => return None,
    };
    Some((text, options, face(npc_id)))
}

async fn send(actor: &Actor<ActorState>, npc_id: u32, text: impl Into<String>, options: Options) -> Result<(), Error> {
    let mut builder = MsgTaskDialog::builder().text(text.into());
    for (option, label) in options {
        builder = builder.with_option(option, label);
    }
    let dialog = builder.and().with_avatar(face(npc_id)).task_id(npc_id).build();
    actor.send_all(dialog).await?;
    Ok(())
}

async fn say(actor: &Actor<ActorState>, npc_id: u32, text: impl Into<String>, option: &str) -> Result<(), Error> {
    send(actor, npc_id, text, vec![(255, option.to_owned())]).await
}

async fn teleport(state: &State, character: &Character, (map_id, x, y): (u32, u16, u16)) -> Result<(), Error> {
    let (x, y) = state
        .try_map(map_id)
        .ok()
        .and_then(|map| {
            let mut origin = character.entity().location();
            origin.x = x;
            origin.y = y;
            map.landing_position(origin, 8)
        })
        .map_or((x, y), |landing| (landing.x, landing.y));
    character.teleport(state, map_id, (x, y)).await
}

async fn take(state: &State, actor: &Actor<ActorState>, character: &Character, item_type: u32, count: usize) -> Result<bool, Error> {
    let Some(items) = character.consume_items(state.pool(), item_type, count).await? else {
        return Ok(false);
    };
    for item in items {
        actor.send(MsgItemInfo::remove_inventory_item(item)).await?;
    }
    Ok(true)
}

async fn give(state: &State, actor: &Actor<ActorState>, character: &Character, item_type: u32) -> Result<bool, Error> {
    let Some(item) = character.grant_item(state.pool(), state.shop_catalog(), item_type, false).await? else {
        return Ok(false);
    };
    actor.send(MsgItemInfo::inventory_item(item)).await?;
    Ok(true)
}

async fn pay(state: &State, actor: &Actor<ActorState>, character: &Character, silver: u32) -> Result<bool, Error> {
    if character.spend_silver(state.pool(), silver).await?.is_none() {
        return Ok(false);
    }
    actor.send(MsgUserInfo::from(character)).await?;
    Ok(true)
}

async fn refresh_look(actor: &Actor<ActorState>, character: &Character) -> Result<(), Error> {
    let update = MsgPlayer::from(character);
    actor.send(update.clone()).await?;
    actor.screen().send_message(update).await?;
    Ok(())
}

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs() as i64)
}

async fn timer_running(state: &State, character: &Character, kind: &str) -> Result<bool, Error> {
    let until = sqlx::query_scalar::<_, i64>("SELECT until_unix FROM character_timers WHERE character_id = $1 AND kind = $2")
        .bind(character.database_id())
        .bind(kind)
        .fetch_optional(state.pool())
        .await?;
    Ok(until.is_some_and(|until| until > now_unix()))
}

async fn start_timer(state: &State, character: &Character, kind: &str, seconds: i64) -> Result<(), Error> {
    sqlx::query(
        "INSERT INTO character_timers (character_id, kind, until_unix) VALUES ($1, $2, $3)
         ON CONFLICT(character_id, kind) DO UPDATE SET until_unix = excluded.until_unix",
    )
    .bind(character.database_id())
    .bind(kind)
    .bind(now_unix() + seconds)
    .execute(state.pool())
    .await?;
    Ok(())
}

/// The other body size of the same look: 1003 <-> 1004, 2001 <-> 2002.
pub fn other_body_size(mesh: u32) -> u32 {
    if mesh % 2 == 0 {
        mesh - 1
    } else {
        mesh + 1
    }
}

/// A new haircut keeps the colour and picks a style from 30 to 50.
pub fn new_hair_style(hair_style: u16, roll: u32) -> u16 {
    hair_style / 100 * 100 + 30 + (roll % 21) as u16
}

/// Handles an answer to one of these NPCs. Returns false for other NPCs.
pub async fn answer(
    state: &State,
    actor: &Actor<ActorState>,
    character: &Character,
    npc_id: u32,
    option: u8,
    typed: &str,
) -> Result<bool, Error> {
    if opening(npc_id, character).is_none() {
        return Ok(false);
    }
    if npc_id == GUILD_DIRECTOR_ID {
        guild_director(state, actor, character, option, typed).await?;
        return Ok(true);
    }
    match (npc_id, option) {
        (GENERAL_PEACE_ID, 1) => teleport(state, character, DESERT_CITY).await?,
        (BARBER_ID, 1) => {
            if pay(state, actor, character, BARBER_FEE).await? {
                character.set_hair_style(new_hair_style(character.hair_style(), crate::combat::next_roll()));
                character.save(state).await?;
                refresh_look(actor, character).await?;
                say(
                    actor,
                    npc_id,
                    "Perfection! I hope you enjoy your new haircut. Come back any time if you wish to change it again.",
                    "Thanks",
                )
                .await?;
            } else {
                say(actor, npc_id, "I'm sorry, you do not have the 1000 silver required for my services.", "Thanks").await?;
            }
        },
        (ARENA_GUARD_ID, 1) => {
            if pay(state, actor, character, ARENA_FEE).await? {
                teleport(state, character, ARENA).await?;
            } else {
                say(actor, npc_id, "Sorry, you do not have enough.", "I see.").await?;
            }
        },
        (CAPTAIN_ID, 1) => {
            if character.kill_points() < crate::death::BLACK_NAME_POINTS {
                teleport(state, character, JAIL_EXIT).await?;
            } else {
                say(actor, npc_id, &sentence_text(character.kill_points()), "I shall..").await?;
            }
        },
        (CAPTAIN_LI_ID, 1) => {
            send(
                actor,
                npc_id,
                "Give me 1000 silver, I will teleport you there",
                options(&[(2, "Here are 1000 silver"), (255, "If so, I will stay here.")]),
            )
            .await?;
        },
        (CAPTAIN_LI_ID, 2) => {
            if pay(state, actor, character, JAIL_FEE).await? {
                teleport(state, character, JAIL).await?;
            } else {
                say(actor, npc_id, "Sorry, you do not have enough.", "I see.").await?;
            }
        },
        (id, 1) if MINE_ASSISTANTS.iter().any(|(npc, _, _)| *npc == id) => {
            let Some((_, level, destination)) = MINE_ASSISTANTS.iter().find(|(npc, _, _)| *npc == id) else {
                return Ok(true);
            };
            if character.entity().level() >= *level {
                teleport(state, character, *destination).await?;
            } else {
                say(
                    actor,
                    npc_id,
                    format!("Sorry, you must be at least level {level} before you can enter this mine."),
                    "Sorry",
                )
                .await?;
            }
        },
        (CP_ADMIN_ID, 1 | 2) => {
            let (item, cps, name) = if option == 1 {
                (DRAGONBALL, 215, "DragonBall")
            } else {
                (DRAGONBALL_SCROLL, 2_150, "DBScroll")
            };
            if take(state, actor, character, item, 1).await? {
                character.grant_cps(state.pool(), cps).await?;
                actor.send(MsgUserInfo::from(character)).await?;
                crate::audit::economic("cp_admin", character.id(), npc_id, item, cps as u32);
                say(actor, npc_id, "It is finished. Enjoy your Conquer Points.", "Thanks.").await?;
            } else {
                say(
                    actor,
                    npc_id,
                    format!("I will not be so easily fooled. You do not have a {name} to trade!"),
                    "Sorry.",
                )
                .await?;
            }
        },
        (SURGEON_ID, 1) => {
            send(
                actor,
                npc_id,
                "Are you sure you want to do this?",
                options(&[(2, "Yes."), (255, "I changed my mind.")]),
            )
            .await?;
        },
        (SURGEON_ID, 2) => {
            if take(state, actor, character, DRAGONBALL, 1).await? {
                character.entity().set_mesh(other_body_size(character.entity().mesh()));
                character.save(state).await?;
                refresh_look(actor, character).await?;
                say(actor, npc_id, "There you go. I hope you are satisfied now!", "Thanks!").await?;
            } else {
                say(actor, npc_id, "You don't have 1 DragonBall.", "Nevermind ").await?;
            }
        },
        (SHOP_BOY_ID, 1) => {
            if take(state, actor, character, METEOR, 1).await? {
                teleport(state, character, DYE_SHOP).await?;
            } else {
                say(actor, npc_id, "You do not have the Meteor I requested.", "Sorry").await?;
            }
        },
        (SHOP_BOY_ID, 2) => {
            send(
                actor,
                npc_id,
                "I can sell you DrunkCelestial wine for 1,000 silver if you would like.",
                options(&[(3, "Yes please"), (255, "No thanks")]),
            )
            .await?;
        },
        (SHOP_BOY_ID, 3) => {
            if character.inventory_items().len() >= 40 {
                say(actor, npc_id, "Please make room in your inventory for the wine.", "Sorry").await?;
            } else if pay(state, actor, character, WINE_PRICE).await? {
                give(state, actor, character, WINE).await?;
                say(actor, npc_id, "Enjoy the wine, it's the finest in all the land!", "I will!").await?;
            } else {
                say(actor, npc_id, "You do not have the 1,000 silver required", "Sorry").await?;
            }
        },
        (CELESTIAL_TAO_ID, 1 | 2) => {
            if character.rebirths() < 1 {
                say(actor, npc_id, "Please come back when you have been reborned at least once in your lifetime?", "I see.").await?;
            } else if character.entity().level() < 70 {
                say(actor, npc_id, "You must be level 70 or above to be able to reallot attribute points!", "Thanks. I see.").await?;
            } else if character.count_items(DRAGONBALL) == 0 {
                say(actor, npc_id, "Sorry, but you must pay one DragonBall come back when you have one!", "I see!").await?;
            } else if option == 1 {
                send(
                    actor,
                    npc_id,
                    "Are you sure you want to reallot your attribute points?",
                    options(&[(2, "Yes."), (255, "No thanks.")]),
                )
                .await?;
            } else if take(state, actor, character, DRAGONBALL, 1).await? {
                character.reallot_attributes();
                character.save(state).await?;
                actor.send(MsgUserInfo::from(character)).await?;
                say(actor, npc_id, "There you go, allot them as you wish!", "I see!").await?;
            }
        },
        (UNKNOWN_MAN_ID, _) => unknown_man(state, actor, character, option).await?,
        (CELESTINE_ID, _) => celestine(state, actor, character, option).await?,
        (SHELBY_ID, _) => shelby(state, actor, character, option).await?,
        (WUXING_OVEN_ID, _) => wuxing_oven(state, actor, character, option).await?,
        (STARLIT_ID, _) => starlit(state, actor, character, option).await?,
        (TINTER_ID, _) => tinter(state, actor, character, option).await?,
        (LOVE_STONE_ID, _) => love_stone(state, actor, character, option).await?,
        _ => {},
    }
    Ok(true)
}

/// Marriage: both lovers stand by the LoveStone; one proposes to the
/// player of the other body next to them, who accepts or refuses.
const MARRIAGE_RANGE: u16 = 3;
const METEOR_TEAR: u32 = 1_088_002;
const ACCEPT_PROPOSAL: u8 = 10;
const REFUSE_PROPOSAL: u8 = 11;

/// Records the marriage of two characters, `(database id, name)` each. A
/// marriage row means married (divorce deletes it), so it fails, leaving
/// both unchanged, when either already has one: another proposal may have
/// been accepted at the same moment.
pub(crate) async fn marry(pool: &sqlx::PgPool, one: (i64, &str), other: (i64, &str)) -> Result<bool, Error> {
    let mut transaction = pool.begin().await?;
    for ((me, _), (spouse_id, spouse_name)) in [(one, other), (other, one)] {
        let inserted = sqlx::query(
            "INSERT INTO marriages (character_id, spouse_id, spouse_name) VALUES ($1, $2, $3)
             ON CONFLICT (character_id) DO NOTHING",
        )
        .bind(me)
        .bind(spouse_id)
        .bind(spouse_name)
        .execute(&mut *transaction)
        .await?;
        if inserted.rows_affected() != 1 {
            return Ok(false);
        }
    }
    transaction.commit().await?;
    Ok(true)
}

fn proposals() -> &'static parking_lot::Mutex<std::collections::HashMap<u32, (u32, String)>> {
    static PROPOSALS: std::sync::OnceLock<parking_lot::Mutex<std::collections::HashMap<u32, (u32, String)>>> =
        std::sync::OnceLock::new();
    PROPOSALS.get_or_init(Default::default)
}

/// 1 for male bodies (1003/1004), 2 for female ones (2001/2002).
fn body_sex(mesh: u32) -> u32 {
    mesh % 10_000 / 1_000
}

pub async fn spouse_of(state: &State, character: &Character) -> Result<Option<(u32, String)>, Error> {
    let row = sqlx::query_as::<_, (i64, String)>("SELECT spouse_id, spouse_name FROM marriages WHERE character_id = $1")
        .bind(character.database_id())
        .fetch_optional(state.pool())
        .await?;
    Ok(row.map(|(id, name)| (id.max(0) as u32, name)))
}

async fn love_stone(state: &State, actor: &Actor<ActorState>, character: &Character, option: u8) -> Result<(), Error> {
    const NPC: u32 = LOVE_STONE_ID;
    match option {
        1 => {
            if spouse_of(state, character).await?.is_some() {
                return say(actor, NPC, "It seems you've already found your true love. Be sure to cherish them daily", "I will.").await;
            }
            send(
                actor,
                NPC,
                "Marriage is not a decision to be taken lightly and means spending your life with your lover. Are you positive you've found your mate?",
                options(&[(2, "Positive!"), (255, "Maybe not...")]),
            )
            .await
        },
        2 => {
            if spouse_of(state, character).await?.is_some() {
                return Ok(());
            }
            let here = character.entity().location();
            let lover = state.entities().into_iter().find(|entity| {
                entity.as_character().is_some_and(|other| {
                    let there = other.entity().location();
                    other.id() != character.id()
                        && other.entity().map_id() == character.entity().map_id()
                        && here.x.abs_diff(there.x).max(here.y.abs_diff(there.y)) <= MARRIAGE_RANGE
                        && body_sex(other.entity().mesh()) != body_sex(character.entity().mesh())
                })
            });
            let Some(lover) = lover.as_ref().and_then(|entity| entity.as_character()) else {
                return say(
                    actor,
                    NPC,
                    "Your lover must stand right beside you when you give yourself to them.",
                    "I see.",
                )
                .await;
            };
            if spouse_of(state, lover).await?.is_some() {
                return say(actor, NPC, "Your lover is already married.", "Oh...").await;
            }
            proposals()
                .lock()
                .insert(lover.id(), (character.id(), character.entity().name().to_owned()));
            let dialog = MsgTaskDialog::builder()
                .text(format!(
                    "{} offers you this flower and wants to spend their life with you. Will you marry {}?",
                    character.entity().name(),
                    character.entity().name()
                ))
                .with_option(ACCEPT_PROPOSAL, "Yes, I will!")
                .with_option(REFUSE_PROPOSAL, "Sorry, no.")
                .and()
                .with_avatar(face(NPC))
                .task_id(NPC)
                .build();
            lover.owner().send_all(dialog).await?;
            say(
                actor,
                NPC,
                "By sending this flower to your lover you are giving yourself to them. They may chose to accept or reject your offer.",
                "Thank you.",
            )
            .await
        },
        ACCEPT_PROPOSAL | REFUSE_PROPOSAL => {
            let Some((proposer_id, proposer_name)) = proposals().lock().remove(&character.id()) else {
                return Ok(());
            };
            let proposer = state
                .entities()
                .into_iter()
                .find(|entity| entity.id() == proposer_id);
            let Some(proposer) = proposer.as_ref().and_then(|entity| entity.as_character()) else {
                return say(actor, NPC, "Your lover is gone.", "Oh...").await;
            };
            if option == REFUSE_PROPOSAL {
                proposer
                    .owner()
                    .send(crate::packets::MsgTalk::from_system(
                        proposer.id(),
                        crate::packets::TalkChannel::TopLeft,
                        format!("{} refused your proposal.", character.entity().name()),
                    ))
                    .await?;
                return Ok(());
            }
            if !marry(state.pool(), (i64::from(character.database_id()), character.entity().name()), (i64::from(proposer.database_id()), &proposer_name)).await? {
                return say(actor, NPC, "One of you is already married.", "Oh...").await;
            }
            character.set_spouse(proposer_name.clone());
            proposer.set_spouse(character.entity().name());
            actor.send(MsgUserInfo::from(character)).await?;
            proposer.owner().send(MsgUserInfo::from(proposer)).await?;
            let news = format!(
                "Congratulations! {} and {} are now husband and wife.",
                proposer_name,
                character.entity().name()
            );
            for entity in state.entities() {
                if let Some(player) = entity.as_character() {
                    let _ = player
                        .owner()
                        .send(crate::packets::MsgTalk::from_system(
                            player.id(),
                            crate::packets::TalkChannel::Center,
                            news.clone(),
                        ))
                        .await;
                }
            }
            Ok(())
        },
        _ => Ok(()),
    }
}

async fn starlit(state: &State, actor: &Actor<ActorState>, character: &Character, option: u8) -> Result<(), Error> {
    const NPC: u32 = STARLIT_ID;
    let Some((spouse_id, spouse_name)) = spouse_of(state, character).await? else {
        return say(actor, NPC, "Sorry, I can not divorce you if you are not married.", "Okay.").await;
    };
    match option {
        1 => {
            send(
                actor,
                NPC,
                "You really sure you want to divorce your spouse now? You will not regret later?",
                options(&[(2, "No I'll not. Let's do this."), (255, "I prefer to remain married.")]),
            )
            .await
        },
        2 => {
            send(
                actor,
                NPC,
                "Are you ready? Be sure you're with MeteorTear, I really need it.",
                options(&[(3, "Yeah. I am ready."), (255, "Let me think it over.")]),
            )
            .await
        },
        3 => {
            if !take(state, actor, character, METEOR_TEAR, 1).await? {
                return say(actor, NPC, "Sorry, I can not divorce you if you don't have a MeteorTear.", "Okay.").await;
            }
            sqlx::query("DELETE FROM marriages WHERE character_id IN ($1, $2)")
                .bind(character.database_id())
                .bind(i64::from(spouse_id))
                .execute(state.pool())
                .await?;
            character.set_spouse("None");
            actor.send(MsgUserInfo::from(character)).await?;
            if let Some(spouse) = state
                .entities()
                .into_iter()
                .find(|entity| entity.as_character().is_some_and(|other| other.database_id() == spouse_id as i32))
            {
                if let Some(spouse) = spouse.as_character() {
                    spouse.set_spouse("None");
                    spouse.owner().send(MsgUserInfo::from(spouse)).await?;
                }
            }
            let news = format!("Unfortunately! {} and {spouse_name} got divorced.", character.entity().name());
            for entity in state.entities() {
                if let Some(player) = entity.as_character() {
                    let _ = player
                        .owner()
                        .send(crate::packets::MsgTalk::from_system(
                            player.id(),
                            crate::packets::TalkChannel::Center,
                            news.clone(),
                        ))
                        .await;
                }
            }
            say(
                actor,
                NPC,
                "Done, you are now divorced. You are free to find another passion around. Be happy my friend.",
                "Thank you.",
            )
            .await
        },
        _ => Ok(()),
    }
}

/// GuildDirector: a level 90 character founds a guild for 1,000,000
/// silver, typing its name in the dialog (Redux [10003]).
async fn guild_director(
    state: &State,
    actor: &Actor<ActorState>,
    character: &Character,
    option: u8,
    typed: &str,
) -> Result<(), Error> {
    const NPC: u32 = GUILD_DIRECTOR_ID;
    // Text boxes of the leader's menu: option -> (prompt, label).
    const INPUTS: [(u8, u8, &str, &str); 6] = [
        (14, 7, "Please enter the name for the guild you wish to enemy", "Enemy Name"),
        (15, 9, "Please enter the name for the guild you wish to ally", "Ally Name"),
        (16, 8, "Choose the enemy you wish to make peace with...", "Make peace with..."),
        (17, 10, "Choose the Ally you wish to remove from your Allies", "Remove..."),
        (18, 11, "What is the name of the player you wish to promote?", "Their name is..."),
        (19, 12, "What is the name of the player you wish to demote?", "Their name is..."),
    ];
    let guild = state.guild_manager().guild_for(character.id());
    let leader = guild.as_ref().is_some_and(|guild| guild.leader_id == character.id());
    if let Some((_, input, prompt, label)) = INPUTS.iter().find(|(menu, ..)| *menu == option) {
        if !leader {
            return say(actor, NPC, "Only the guild leader can do that.", "I see.").await;
        }
        let dialog = MsgTaskDialog::builder()
            .text(*prompt)
            .with_edit(*input, *label)
            .with_option(255, "Never mind.")
            .and()
            .with_avatar(face(NPC))
            .task_id(NPC)
            .build();
        actor.send_all(dialog).await?;
        return Ok(());
    }
    match option {
        1 if guild.is_some() => say(actor, NPC, "You already belong to a guild.", "I see.").await,
        1 if character.entity().level() < GUILD_LEVEL => {
            say(
                actor,
                NPC,
                "Sorry, you cannot create a guild before you reach level 90. Please train harder.",
                "I see.",
            )
            .await
        },
        1 => {
            let dialog = MsgTaskDialog::builder()
                .text("Please enter the name for your guild")
                .with_edit(GUILD_NAME_INPUT, "Name")
                .with_option(255, "Never mind.")
                .and()
                .with_avatar(face(NPC))
                .task_id(NPC)
                .build();
            actor.send_all(dialog).await?;
            Ok(())
        },
        GUILD_NAME_INPUT => create_guild(state, actor, character, typed).await,
        3 => {
            let Some(guild) = guild else {
                return say(actor, NPC, "You are not in a guild.", "I see.").await;
            };
            let list = |kind: &str| {
                let names = state.guild_manager().relations_of(guild.id, kind);
                if names.is_empty() { "none".to_owned() } else { names.join(", ") }
            };
            say(
                actor,
                NPC,
                format!(
                    "{}: {} members. Allies: {}. Enemies: {}.",
                    guild.name,
                    guild.members.len(),
                    list("ally"),
                    list("enemy")
                ),
                "Thanks.",
            )
            .await
        },
        21 if !leader => say(actor, NPC, "Only the guild leader can manage the guild.", "I see.").await,
        21 => {
            send(
                actor,
                NPC,
                "What would you like to do with your guild?",
                options(&[
                    (14, "Add Enemy"),
                    (15, "Add Ally"),
                    (16, "Remove Enemy"),
                    (17, "Remove Ally"),
                    (18, "Promote Member"),
                    (19, "Demote Member"),
                    (20, "Disband Guild"),
                    (255, "Nothing."),
                ]),
            )
            .await
        },
        20 if leader => {
            send(
                actor,
                NPC,
                "Disbanding your guild is a decision not to be taken lightly. When armies fall apart, a period of chaos ensues?",
                options(&[(4, "I decide to disband."), (255, "I changed my mind.")]),
            )
            .await
        },
        4 if leader => {
            let Ok(guild) = state.guild_manager().disband(character.id()) else {
                return Ok(());
            };
            crate::guild::GuildManager::persist_disband(state.pool(), guild.id).await?;
            for member_id in &guild.members {
                if let Some(member) = state.entities().into_iter().find(|entity| entity.id() == *member_id) {
                    if let Some(member) = member.as_character() {
                        member
                            .owner()
                            .send(crate::packets::MsgGuild::new(crate::packets::SET_GUILD, 0))
                            .await?;
                    }
                }
            }
            say(actor, NPC, format!("{} has been disbanded.", guild.name), "So be it.").await
        },
        7 | 8 | 9 | 10 if leader => {
            let Some(guild) = guild else {
                return Ok(());
            };
            let Some(other) = state.guild_manager().guild_by_name(typed).filter(|other| other.id != guild.id) else {
                return say(actor, NPC, "This guild name does not exist.", "Thanks.").await;
            };
            let (kind, reply) = match option {
                7 => (Some("enemy"), "And the war begins..."),
                9 => (Some("ally"), "Our guilds stand together now."),
                8 => (None, "And the land knows peace once again."),
                _ => (None, "And so it is..."),
            };
            if kind.is_none() {
                let expected = if option == 8 { "enemy" } else { "ally" };
                if state.guild_manager().relation(guild.id, other.id).as_deref() != Some(expected) {
                    return say(actor, NPC, format!("You are not {expected} with this guild."), "My mistake.").await;
                }
            }
            state
                .guild_manager()
                .set_relation(state.pool(), guild.id, other.id, kind)
                .await?;
            say(actor, NPC, reply, "Thanks.").await
        },
        11 | 12 if leader => {
            let member_id = sqlx::query_scalar::<_, i64>("SELECT character_id FROM characters WHERE name = $1")
                .bind(typed.trim())
                .fetch_optional(state.pool())
                .await?
                .map(|id| id as u32 + crate::constants::CHARACTER_ID_MIN);
            let Some(member_id) = member_id else {
                return say(actor, NPC, "There is nobody by that name.", "Thanks anyway").await;
            };
            let role = if option == 11 { 990 } else { 200 };
            match state.guild_manager().set_role(character.id(), member_id, role) {
                Ok(guild) => {
                    crate::guild::GuildManager::persist_role(state.pool(), member_id, role).await?;
                    crate::packets::send_guild_members(state, character.id(), &guild).await?;
                    crate::packets::send_guild_members(state, member_id, &guild).await?;
                    let done = if option == 11 { "They have been promoted." } else { "They have been demoted." };
                    say(actor, NPC, done, "Thank you.").await
                },
                Err(_) => say(actor, NPC, "They are not a member of your guild...", "Thanks anyway").await,
            }
        },
        _ => Ok(()),
    }
}

/// A level 90 character founds a guild for 1,000,000 silver.
async fn create_guild(state: &State, actor: &Actor<ActorState>, character: &Character, typed: &str) -> Result<(), Error> {
    const NPC: u32 = GUILD_DIRECTOR_ID;
    if character.entity().level() < GUILD_LEVEL || state.guild_manager().guild_for(character.id()).is_some() {
        return Ok(());
    }
    if character.silver() < u64::from(GUILD_PRICE) {
        return say(actor, NPC, "I'm sorry, players must have at least 1 million gold to create a guild", "I'll come back").await;
    }
    let change = match state.guild_manager().create(character.id(), typed.trim()) {
        Ok(change) => change,
        Err(error) => {
            return say(
                actor,
                NPC,
                format!("There was an error setting up your guild: {}", error.message()),
                "I'll try again",
            )
            .await;
        },
    };
    let crate::guild::GuildChange::Created { guild } = change else {
        return Ok(());
    };
    if character.spend_silver(state.pool(), GUILD_PRICE).await?.is_none() {
        state.guild_manager().remove_member(character.id());
        return say(actor, NPC, "I'm sorry, players must have at least 1 million gold to create a guild", "I'll come back").await;
    }
    crate::guild::GuildManager::persist_created(state.pool(), &guild).await?;
    actor.send(MsgUserInfo::from(character)).await?;
    actor
        .send(crate::packets::MsgGuild::new(crate::packets::SET_GUILD, guild.id).with_strings([guild.name.clone()]))
        .await?;
    crate::packets::send_guild_members(state, character.id(), &guild).await?;
    crate::audit::economic("guild_director", character.id(), NPC, guild.id, GUILD_PRICE);
    say(actor, NPC, "I have set up your guild successfully!", "Thanks!").await
}

/// Tinter (Redux [5030]): helmet (1x), armor (3x) and shield (4x) take
/// White 3, Blue 4, Red 5, Purple 6 or Yellow 7.
async fn tinter(state: &State, actor: &Actor<ActorState>, character: &Character, option: u8) -> Result<(), Error> {
    const NPC: u32 = TINTER_ID;
    let slot_of = |group: u8| match group {
        1 => Some((1_u8, "You are not wearing a helmet. I don't do hair!")),
        2 | 3 => Some((3_u8, "You are not wearing armor. I don't do tattoos!")),
        4 => Some((5_u8, "You are not wearing a Shield.")),
        _ => None,
    };
    let worn = |slot: u8| {
        character.equipment_item(slot).filter(|item| match slot {
            1 => (111..=118).contains(&(item.item_type / 1_000)),
            3 => (130..=139).contains(&(item.item_type / 1_000)),
            _ => item.item_type / 1_000 == 900,
        })
    };
    match option {
        9 => teleport(state, character, TWIN_CITY_SHOP).await,
        1..=3 => {
            let group = if option == 1 { 1 } else if option == 2 { 3 } else { 4 };
            let Some((slot, missing)) = slot_of(group) else {
                return Ok(());
            };
            if worn(slot).is_none() {
                return say(actor, NPC, missing, "Sorry").await;
            }
            let list = [(3, "White"), (4, "Blue"), (5, "Red"), (6, "Purple"), (7, "Yellow")]
                .into_iter()
                .map(|(color, label)| (group * 10 + color, label.to_owned()))
                .chain(std::iter::once((255, "Nevermind".to_owned())))
                .collect();
            send(actor, NPC, "What color would you like?", list).await
        },
        13..=17 | 33..=37 | 43..=47 => {
            let Some((slot, missing)) = slot_of(option / 10) else {
                return Ok(());
            };
            let Some(item) = worn(slot) else {
                return say(actor, NPC, missing, "Sorry").await;
            };
            if let Some(item) = character.dye_item(state.pool(), item.uid, option % 10).await? {
                actor.send(MsgItemInfo::equipment_item(item)).await?;
            }
            refresh_look(actor, character).await
        },
        _ => Ok(()),
    }
}

async fn unknown_man(state: &State, actor: &Actor<ActorState>, character: &Character, option: u8) -> Result<(), Error> {
    const NPC: u32 = UNKNOWN_MAN_ID;
    let cost = if character.entity().level() > 99 { 2 } else { 1 };
    match option {
        1 => {
            if pay(state, actor, character, 100).await? {
                say(
                    actor,
                    NPC,
                    "Thank you so much stranger! The only thing that could improve this meal is some wine to wash it down. Can you bring me some DrunkCelestial wine please?",
                    "I'll get it",
                )
                .await?;
            }
        },
        2 => {
            if timer_running(state, character, UNKNOWN_MAN_TIMER).await? {
                say(actor, NPC, "I need some time to recover. Come see me tomorrow", "I shall").await?;
            } else if take(state, actor, character, WINE, 1).await? {
                send(
                    actor,
                    NPC,
                    "Ahhhh... that hit the spot. Your generosity is legendary! Would you like this old man to help you unlock the secret of the DragonBalls?",
                    options(&[(3, "Teach me please"), (255, "No thanks")]),
                )
                .await?;
            }
        },
        3 => {
            send(
                actor,
                NPC,
                format!("I will require {cost} DragonBalls to help you unlock infinite power. This is your last chance to leave"),
                options(&[(4, "Do it"), (255, "Maybe later")]),
            )
            .await?;
        },
        4 => {
            if timer_running(state, character, UNKNOWN_MAN_TIMER).await? {
                say(actor, NPC, "I need some time to recover. Come see me tomorrow", "I shall").await?;
            } else if take(state, actor, character, DRAGONBALL, cost).await? {
                start_timer(state, character, UNKNOWN_MAN_TIMER, UNKNOWN_MAN_REST_SECONDS).await?;
                // Redux `GainExpBall(6000)`: ten ExpBalls.
                crate::leveling::use_exp_ball(state, actor, character, crate::leveling::EXP_BALL_SECONDS * 10.0).await?;
                say(actor, NPC, "It is done! I need some time to recover before you come see me again.", "Thank you").await?;
            } else {
                say(
                    actor,
                    NPC,
                    format!("You do not have the {cost} DragonBalls I require. Come back when you have them"),
                    "Yes sir",
                )
                .await?;
            }
        },
        _ => {},
    }
    Ok(())
}

async fn celestine(state: &State, actor: &Actor<ActorState>, character: &Character, option: u8) -> Result<(), Error> {
    const NPC: u32 = CELESTINE_ID;
    let has_all = character.count_items(CLEAN_WATER) > 0 && NORMAL_GEMS.iter().all(|gem| character.count_items(*gem) > 0);
    let (text, list): (&str, &[(u8, &str)]) = match option {
        1 => (
            "Mortals are mundane. Only getting rid of it can help them accomplish their aims. If you are high level enough, you can get reborn to learn more and stronger skills.",
            &[(2, "How to get reborn?"), (255, "I am satisfied.")],
        ),
        2 => (
            "It is difficult.First, you should reach an certain level. Second, you need a CelestialStone.",
            &[(3, "How to get CelestialStone?"), (255, "Forget it.")],
        ),
        3 => (
            "CelestialStone is made of 7 gems: VioletGem, KylinGem, RainbowGem, MoonGem, PhoenixGem, FuryGem, DragonGem, and CleanWater.",
            &[(4, "What is CleanWater?"), (255, "It is difficult.")],
        ),
        4 => (
            "It is used to get rid of your mundaneness, and then you won't be affected by the environment. By the way, Clean Water comes from celestial rinsing.",
            &[(5, "What are gems used for?")],
        ),
        5 => (
            "Only seven gems can protect you during the rebirth.",
            &[(6, "I will collect them now."), (255, "I changed my mind.")],
        ),
        6 => ("It is easy to get the gems. But CleanWater...", &[(7, "But what?")]),
        7 => (
            "The Adventure island is the headstream of CleanWater. But it is occupied by WaterEvil and he uses spell to hide the stream.",
            &[(8, "What can I do?"), (255, "I will give up.")],
        ),
        8 => (
            "WaterEvilElder will go to get the water every certain time. If you defeat him, you may get the water. But he is very hard to deal with.",
            &[(255, "I see.Thank you."), (9, "Anything else?")],
        ),
        9 if has_all => (
            "Are you sure you want to refine CelestialStone?",
            &[(10, "Sure."), (255, "I am not sure.")],
        ),
        10 if has_all => {
            take(state, actor, character, CLEAN_WATER, 1).await?;
            for gem in NORMAL_GEMS {
                take(state, actor, character, gem, 1).await?;
            }
            give(state, actor, character, CELESTIAL_STONE).await?;
            ("There you go, check it in your inventory.", &[(255, "Thank you.")])
        },
        9 | 10 => (
            "Come here to refine CelestialStone when you get the 7 Gems and CleanWater.",
            &[(255, "That's ok.")],
        ),
        _ => return Ok(()),
    };
    send(actor, NPC, text, options(list)).await
}

async fn shelby(state: &State, actor: &Actor<ActorState>, character: &Character, option: u8) -> Result<(), Error> {
    const NPC: u32 = SHELBY_ID;
    if character.entity().level() < 70 {
        return Ok(());
    }
    match option {
        1 => {
            send(
                actor,
                NPC,
                "If you are above level 70 and try to power level the newbies (at least 20 levels lower than you), you may gain virtue points.",
                options(&[(2, "What are the virtue points?")]),
            )
            .await
        },
        2 => {
            send(
                actor,
                NPC,
                "The more newbies you power level, the more virtue points you gain. I shall give you a good reward for a certain virtue points.",
                options(&[(3, "How can I gain virtue points?"), (4, "What prize can I expect?")]),
            )
            .await
        },
        3 => say(actor, NPC, "Once the newbies are one level up, the team captain can gain virtue points accordingly.", "I see.").await,
        4 => {
            say(
                actor,
                NPC,
                "I shall reward you exp. of equivalent to an ExpBall for 15,000 virtue points , a meteor for 5,000 virtue points and an Dragonball for 50,000 virtue points.",
                "I see.",
            )
            .await
        },
        5 => {
            say(
                actor,
                NPC,
                format!("Your current virtue points are {}  , please try to gain more.", character.virtue()),
                "I see.",
            )
            .await
        },
        6 => {
            if character.inventory_items().len() >= 40 {
                return say(actor, NPC, "Please make room in your inventory before trying to claim a prize!", "Sorry").await;
            }
            send(
                actor,
                NPC,
                "What prize do you prefer?",
                options(&[(7, "Meteor"), (8, "Experience."), (9, "Dragonball"), (255, "Let me think it over.")]),
            )
            .await
        },
        7 | 9 => {
            let (cost, item) = if option == 7 { (5_000, METEOR) } else { (50_000, DRAGONBALL) };
            if character.spend_virtue(cost) {
                give(state, actor, character, item).await?;
                character.save(state).await?;
                say(actor, NPC, "There you go.Check out in your inventory!", "Thanks!").await
            } else {
                say(actor, NPC, "Sorry, you do not have the required virtue points.", "I see.").await
            }
        },
        8 => {
            if character.entity().level() >= u16::from(crate::leveling::MAX_LEVEL) {
                say(
                    actor,
                    NPC,
                    "Only characters below level 130 can exchange the virtue points for exp. You may find Simon (Twin city 393,235) and spend 2,000 virtue points to hunt the treasure in the Labyrinth.)",
                    "Wow.So cool.",
                )
                .await
            } else if character.spend_virtue(15_000) {
                // Redux `GainExpBall(1200)`: two ExpBalls.
                crate::leveling::use_exp_ball(state, actor, character, crate::leveling::EXP_BALL_SECONDS * 2.0).await?;
                say(actor, NPC, "There you go!", "Thanks!").await
            } else {
                say(actor, NPC, "Sorry, you do not have the required virtue points.", "I see.").await
            }
        },
        _ => Ok(()),
    }
}

/// The HP a gem of `tier` (1 normal, 2 refined, 3 super) enchants.
pub fn enchant_roll(tier: u32, roll: u32) -> u8 {
    let (low, high) = match tier {
        1 => (1, 59),
        2 => (60, 109),
        _ => (110, 255),
    };
    (low + roll % (high - low + 1)) as u8
}

/// WuxingOven: HP enchantments, and +9 to +12 with DragonBalls and DBScrolls.
async fn wuxing_oven(state: &State, actor: &Actor<ActorState>, character: &Character, option: u8) -> Result<(), Error> {
    const NPC: u32 = WUXING_OVEN_ID;
    // Enchanting: the gear (51..58), then the gem: 100 + slot * 10 + tier.
    if option == 1 {
        return send(
            actor,
            NPC,
            "A gem melted into your gear adds to your life. A Normal gem gives up to 59, a Refined one up to 109 and a Super one up to 255. Which item?",
            gear_menu(5),
        )
        .await;
    }
    if (51..=58).contains(&option) {
        let slot = option - 50;
        return send(
            actor,
            NPC,
            "Which gem will you melt? Any kind of gem works; its quality decides the strength.",
            vec![
                (100 + slot * 10 + 1, "A Normal gem".to_owned()),
                (100 + slot * 10 + 2, "A Refined gem".to_owned()),
                (100 + slot * 10 + 3, "A Super gem".to_owned()),
                (255, "Never mind.".to_owned()),
            ],
        )
        .await;
    }
    if (111..=183).contains(&option) && (1..=3).contains(&(option % 10)) {
        let slot = (option - 100) / 10;
        let tier = u32::from(option % 10);
        let Some(item) = character.equipment_item(slot) else {
            return say(actor, NPC, "There must be some mistake.... You must be wearing the item!", "Nevermind").await;
        };
        let Some(gem) = character
            .inventory_items()
            .into_iter()
            .find(|owned| owned.item_type / 1_000 == 700 && owned.item_type % 10 == tier)
        else {
            return say(actor, NPC, "You do not have a gem of that quality.", "I see.").await;
        };
        if !take(state, actor, character, gem.item_type, 1).await? {
            return Ok(());
        }
        let hp = enchant_roll(tier, crate::combat::next_roll());
        if hp <= item.enchant_hp {
            return say(
                actor,
                NPC,
                format!("The gem gave +{hp} HP, no better than the +{} your item has.", item.enchant_hp),
                "Too bad.",
            )
            .await;
        }
        if let Some(item) = character.enchant_item(state.pool(), item.uid, hp).await? {
            actor.send(MsgItemInfo::equipment_item(item)).await?;
        }
        character.refresh_equipment_resources(state.shop_catalog());
        actor.send(MsgUserInfo::from(character)).await?;
        return say(actor, NPC, format!("Your item now adds +{hp} HP."), "Thanks!").await;
    }
    // +1..+9: the worn item (61..68), then the pair of minors (191..199).
    if option == 3 {
        return send(
            actor,
            NPC,
            "Put two items of the same +n into your bag. If they are above your gear, it takes their plus; otherwise it gains one, up to +9. Which item?",
            gear_menu(6),
        )
        .await;
    }
    if (61..=68).contains(&option) {
        let slot = option - 60;
        let Some(item) = character.equipment_item(slot) else {
            return say(actor, NPC, "There must be some mistake.... You must be wearing the item!", "Nevermind").await;
        };
        if item.plus >= 9 {
            return say(actor, NPC, "This item is already +9. Only DragonBalls can raise it further.", "I see.").await;
        }
        let mut list = minor_pairs(character)
            .into_iter()
            .filter_map(|(plus, _)| {
                let result = composed_plus(item.plus, plus)?;
                Some((190 + plus, format!("Two +{plus} items (your item becomes +{result}).")))
            })
            .collect::<Vec<_>>();
        if list.is_empty() {
            return say(actor, NPC, "You need two items of the same +n in your bag.", "I see.").await;
        }
        compose_slots().lock().insert(character.id(), slot);
        list.push((255, "Never mind.".to_owned()));
        return send(actor, NPC, "Which items will you melt into it?", list).await;
    }
    if (191..=199).contains(&option) {
        let plus = option - 190;
        let Some(slot) = compose_slots().lock().remove(&character.id()) else {
            return Ok(());
        };
        let Some(item) = character.equipment_item(slot) else {
            return say(actor, NPC, "There must be some mistake.... You must be wearing the item!", "Nevermind").await;
        };
        let Some(result) = composed_plus(item.plus, plus) else {
            return Ok(());
        };
        let Some((_, uids)) = minor_pairs(character).into_iter().find(|(minor, _)| *minor == plus) else {
            return say(actor, NPC, "You need two items of the same +n in your bag.", "I see.").await;
        };
        let Some(minors) = character.consume_item_uids(state.pool(), &uids).await? else {
            return Ok(());
        };
        for minor in minors {
            actor.send(MsgItemInfo::remove_inventory_item(minor)).await?;
        }
        if let Some(item) = character.set_equipment_plus(state.pool(), slot, result).await? {
            actor.send(MsgItemInfo::equipment_item(item)).await?;
        }
        character.refresh_equipment_resources(state.shop_catalog());
        actor.send(MsgUserInfo::from(character)).await?;
        crate::audit::economic("wuxing_compose", character.id(), NPC, item.uid, u32::from(result));
        return say(actor, NPC, format!("Your item is now +{result}."), "Thanks!").await;
    }
    match option {
        2 => {
            send(
                actor,
                NPC,
                "There are two ways of composing from +1 to +9 with +n stones or +n items and from +9 to +12 with DragonBalls. Whitch one would you want?",
                options(&[
                    (3, "Compose +1 to +9."),
                    (4, "Compose +9 to +12."),
                    (5, "How does composing  +9 to +12 works?"),
                    (255, "Thanks."),
                ]),
            )
            .await
        },
        4 => {
            send(
                actor,
                NPC,
                "What a glorious day! I belive it's the best weather to refine equipment in. Do you need my help with anything?",
                options(&[
                    (6, "Compose +9 Equipment to +10."),
                    (7, "Compose +10 Equipment to +11."),
                    (8, "Compose +11 Equipment to +12."),
                    (255, "Nothing right now, thanks."),
                ]),
            )
            .await
        },
        5 => {
            send(
                actor,
                NPC,
                "If you have any +9 items, you may use DragonBalls to refine it and upgrade its bonus level up to +12.",
                options(&[(9, "Are there any limits to equipment refining?"), (17, "How many DragonBalls would I need?")]),
            )
            .await
        },
        9 => {
            say(
                actor,
                NPC,
                "When your level reaches 130, I can help you upgrade the bonus level of your equiment and weapons. However, there are limitations.Only right-Handed weapons can be refined. Whereas, there are no limits to equipment refining.",
                "Thanks. I never knew.",
            )
            .await
        },
        17 => {
            say(
                actor,
                NPC,
                "12 DragonBalls are required for refining +9 items to +10, 25 DragonBalls are required for refining +10 items to +11, 4 DBScrolls are required for refining +11 items to +12,",
                "Got it, thanks.",
            )
            .await
        },
        6..=8 => {
            let plus = option + 3;
            send(
                actor,
                NPC,
                format!("Choose the equipment you want to upgrade from +{plus} to +{} from the list below. ", plus + 1),
                gear_menu(option - 5),
            )
            .await
        },
        11..=38 if GEAR.iter().any(|(slot, _)| *slot == option % 10) => {
            let group = option / 10;
            let position = option % 10;
            let from = 8 + group;
            let (catalyst, count, name) = match group {
                1 => (DRAGONBALL, 12, "12 DragonBalls"),
                2 => (DRAGONBALL, 25, "25 DragonBalls"),
                _ => (DRAGONBALL_SCROLL, 4, "4 DBScrolls"),
            };
            let Some(item) = character.equipment_item(position) else {
                return say(
                    actor,
                    NPC,
                    "There must be some mistake.... You must be wearing the items in order to refine it!",
                    "Nevermind",
                )
                .await;
            };
            if item.plus != from {
                return say(
                    actor,
                    NPC,
                    format!("It seems that your items is not +{from}, in this case I can not help you."),
                    "Nevermind",
                )
                .await;
            }
            if !take(state, actor, character, catalyst, count).await? {
                return say(
                    actor,
                    NPC,
                    format!("There must be some mistake. You do not have the {name} required to refine this item!"),
                    "Nevermind",
                )
                .await;
            }
            if let Some(item) = character.set_equipment_plus(state.pool(), position, from + 1).await? {
                actor.send(MsgItemInfo::equipment_item(item)).await?;
                crate::audit::economic("wuxing_oven", character.id(), NPC, item.uid, count as u32);
            }
            say(actor, NPC, "It is done! Please enjoy your new equipment.", "Nevermind").await
        },
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn one_character_cannot_marry_twice_at_the_same_moment() -> Result<(), crate::Error> {
        let pool = tq_db::testing::pool().await;
        for round in 0..20_i64 {
            let (a, b, c) = (3 * round + 1, 3 * round + 2, 3 * round + 3);
            let (left, right) = tokio::join!(
                super::marry(&pool, (a, "a"), (b, "b")),
                super::marry(&pool, (b, "b"), (c, "c")),
            );
            assert_eq!(usize::from(left?) + usize::from(right?), 1, "round {round}: one marriage wins");
            let rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM marriages WHERE character_id IN ($1, $2, $3)")
                .bind(a)
                .bind(b)
                .bind(c)
                .fetch_one(&pool)
                .await?;
            assert_eq!(rows, 2, "round {round}: exactly one couple is recorded");
        }
        Ok(())
    }

    #[test]
    fn composing_follows_the_minor_items() {
        // Three +1 make a +2; two +5 lift a +2 to +5; two +1 still add one.
        assert_eq!(composed_plus(1, 1), Some(2));
        assert_eq!(composed_plus(2, 5), Some(5));
        assert_eq!(composed_plus(7, 1), Some(8));
        assert_eq!(composed_plus(9, 9), None);
        assert_eq!(composed_plus(3, 0), None);
    }

    #[test]
    fn surgeon_swaps_the_body_size() {
        assert_eq!(other_body_size(1_003), 1_004);
        assert_eq!(other_body_size(1_004), 1_003);
        assert_eq!(other_body_size(2_001), 2_002);
    }

    #[test]
    fn haircuts_keep_the_colour() {
        assert_eq!(new_hair_style(615, 0), 630);
        assert_eq!(new_hair_style(615, 20), 650);
        assert_eq!(new_hair_style(615, 21), 630);
    }
}
