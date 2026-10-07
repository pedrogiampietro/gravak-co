//! Dis City (co.99.com "Quest - Dis City"): the Solar Saint in Ape City
//! sends heroes of level 110 after Ultimate Pluto, through four stages.
//!
//! 1. Hell Gate: five SoulStones from its demons open the way.
//! 2. Hell Hall: 400 Slayer Points (Archers 600, Taoists 300), one a demon.
//! 3. Hell Cloister: the nine Hell Wraiths guarding the Hell Crystals,
//!    slain in order; the last one opens the Battle Formation.
//! 4. Battle Formation: four Syrens make up a Pluto Token, and with it the
//!    Solar Saint summons Ultimate Pluto. Whoever stands in the formation
//!    when Pluto falls claims Pluto's treasure, once a week.
//!
//! Dark Horns (Syrens) and the Star Sword (Pluto) are traded with the Solar
//! Saint in Ape City. The 5065 client has no Mirror of Hell, Pluto Token,
//! Pluto's Key, Yin Jade nor refinery packs: the token is counted, and the
//! treasury is the Saint's prize. Progress lives in memory: a hero who dies
//! or leaves comes back to the stage reached, until the server restarts.

use std::collections::HashMap;

use parking_lot::Mutex;
use tq_network::Actor;

use crate::entities::{Character, Monster};
use crate::packets::{MsgItemInfo, MsgTalk, MsgTaskDialog, TalkChannel};
use crate::{ActorState, Error, State};

pub const APE_SAINT_ID: u32 = 3_215;
const GATE_SAINT_ID: u32 = 3_217;
const HALL_SAINT_ID: u32 = 3_218;
const CLOISTER_SAINT_ID: u32 = 3_219;
const FORMATION_SAINT_ID: u32 = 3_220;

pub const HELL_GATE: u32 = 2_021;
pub const HELL_HALL: u32 = 2_022;
pub const HELL_CLOISTER: u32 = 2_023;
pub const BATTLE_FORMATION: u32 = 2_024;

/// Where each stage starts, beside its Solar Saint.
const ENTRIES: [(u32, u16, u16); 4] = [
    (HELL_GATE, 253, 60),
    (HELL_HALL, 215, 340),
    (HELL_CLOISTER, 341, 380),
    (BATTLE_FORMATION, 147, 30),
];
/// Ape City, where the maps send the fallen (their revive point).
const APE_CITY: (u32, u16, u16) = (1_020, 566, 565);

const MIN_LEVEL: u16 = 110;
const SOUL_STONE: u32 = 723_085;
const SOUL_STONES: usize = 5;
const DARK_HORN: u32 = 790_001;
const STAR_SWORD: u32 = 723_088;

/// The Hell Hall demons: Eidolon, Temptress, Centicore and Naga.
const HALL_DEMONS: [u32; 4] = [5_051, 5_053, 5_054, 5_057];
const HELL_WRAITH: u32 = 5_055;
/// The Hell Wraiths' posts, in the order they must fall (their spawns).
pub const WRAITH_POSTS: [(u16, u16); 9] = [
    (456, 324),
    (288, 268),
    (220, 460),
    (120, 352),
    (236, 204),
    (424, 176),
    (572, 228),
    (496, 416),
    (296, 556),
];
const SYREN: u32 = 5_058;
/// Syrens slain for one Pluto Token (its four scraps).
const SYRENS_FOR_TOKEN: u32 = 4;
const ULTIMATE_PLUTO: u32 = 5_059;
const PLUTO_LAIR: (u16, u16) = (148, 200);

/// The week a hero has claimed Pluto's treasure (`character_timers`).
const WEEK_TIMER: &str = "dis_city";

const EXP_BALL: u32 = crate::leveling::EXP_BALL;
const DRAGONBALL: u32 = 1_088_000;
const METEOR: u32 = 1_088_001;
const PLUS_ONE_STONE: u32 = 730_001;
const PLUS_TWO_STONE: u32 = 730_002;

/// A hero's way through Dis City.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Run {
    /// The furthest stage reached, 1..=4.
    stage: u8,
    slayer_points: u32,
    /// Hell Wraiths slain in order.
    wraiths: u8,
    syrens: u32,
    /// In the formation when Ultimate Pluto fell.
    victor: bool,
}

impl Default for Run {
    fn default() -> Self {
        Self {
            stage: 1,
            slayer_points: 0,
            wraiths: 0,
            syrens: 0,
            victor: false,
        }
    }
}

fn runs() -> &'static Mutex<HashMap<u32, Run>> {
    static RUNS: std::sync::OnceLock<Mutex<HashMap<u32, Run>>> = std::sync::OnceLock::new();
    RUNS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Ultimate Pluto on the formation now: its monster id.
fn pluto() -> &'static Mutex<Option<u32>> {
    static PLUTO: std::sync::OnceLock<Mutex<Option<u32>>> = std::sync::OnceLock::new();
    PLUTO.get_or_init(|| Mutex::new(None))
}

fn run_of(character: &Character) -> Run {
    runs().lock().get(&character.id()).copied().unwrap_or_default()
}

fn update(character: &Character, change: impl FnOnce(&mut Run)) -> Run {
    let mut runs = runs().lock();
    let run = runs.entry(character.id()).or_default();
    change(run);
    *run
}

pub fn is_dis_city(map_id: u32) -> bool {
    (HELL_GATE..=BATTLE_FORMATION).contains(&map_id)
}

/// Where a hero who logs out in Dis City wakes up.
pub fn wake_up_place() -> (u32, u16, u16) {
    APE_CITY
}

/// Slayer Points the Hell Hall asks of a class.
pub fn slayer_points_needed(class: u8) -> u32 {
    match class {
        40..=45 => 600,
        100..=145 => 300,
        _ => 400,
    }
}

/// The post a Hell Wraith guards, by its spawn.
fn wraith_post(x: u16, y: u16) -> Option<usize> {
    WRAITH_POSTS.iter().position(|post| *post == (x, y))
}

/// Pluto's treasure: an ExpBall and one prize, by `roll`.
pub fn treasure(roll: u32) -> [u32; 2] {
    let prize = match roll % 100 {
        0..=9 => DRAGONBALL,
        10..=24 => PLUS_TWO_STONE,
        25..=59 => PLUS_ONE_STONE,
        _ => METEOR,
    };
    [EXP_BALL, prize]
}

/// The Star Sword's dress: a Phoenix, Elegance or Celestial dress of one of
/// the seven colours (181305 WhitePhoenix .. 181925 PurpleCelestial).
pub fn star_sword_dress(roll: u32) -> u32 {
    let colour = roll % 7;
    let kind = (roll / 7) % 3;
    181_305 + colour * 100 + kind * 10
}

/// When the week turns (Monday 00:00 UTC) after `now`, in Unix seconds.
pub fn next_week(now: i64) -> i64 {
    const DAY: i64 = 24 * 60 * 60;
    // 1970-01-01 was a Thursday: Monday is three days on.
    let days = now.div_euclid(DAY);
    let since_monday = (days + 3).rem_euclid(7);
    (days - since_monday + 7) * DAY
}

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs() as i64)
}

type Options = Vec<(u8, String)>;

fn options(list: &[(u8, &str)]) -> Options {
    list.iter().map(|(id, label)| (*id, (*label).to_owned())).collect()
}

const FACE: u16 = 6;

pub fn opening(npc_id: u32, character: &Character) -> Option<(String, Options, u16)> {
    let run = run_of(character);
    let (text, options) = match npc_id {
        APE_SAINT_ID => (
            "Hundreds of years ago Ultimate Pluto, the leader of the demons, was beaten by the forces of humanity. He escaped, swearing vengeance, and now he has come back. He resides in Dis City, a black mark upon the earth. Will you face him?".to_owned(),
            options(&[
                (1, "I will go to Dis City."),
                (4, "Tell me about Dis City."),
                (2, "I have Dark Horns."),
                (3, "I have a Star Sword."),
                (255, "Not now."),
            ]),
        ),
        GATE_SAINT_ID => (
            format!(
                "The Hell Gate stands shut. Bring me {SOUL_STONES} SoulStones from the demons out here and I shall open the way to the Hell Hall. You have {}.",
                character.count_items(SOUL_STONE)
            ),
            options(&[
                (1, "Here are the SoulStones."),
                (2, "Take me back to Ape City."),
                (255, "I will collect them."),
            ]),
        ),
        HALL_SAINT_ID => (
            format!(
                "The demons of the Hell Hall must pay. Slay them for Slayer Points; at {} the way to the Hell Cloister opens to you. You have {}.",
                slayer_points_needed(character.current_class()),
                run.slayer_points
            ),
            options(&[
                (1, "Open the way to the Hell Cloister."),
                (2, "Take me back to Ape City."),
                (255, "I will fight on."),
            ]),
        ),
        CLOISTER_SAINT_ID => {
            let next = WRAITH_POSTS
                .get(usize::from(run.wraiths))
                .map(|(x, y)| format!(" The next one guards the crystal at ({x},{y})."))
                .unwrap_or_default();
            (
                format!(
                    "Nine Hell Wraiths guard the Hell Crystals of the cloister. Slay them in their proper order and the Battle Formation opens. You have slain {} of them.{next}",
                    run.wraiths
                ),
                options(&[(2, "Take me back to Ape City."), (255, "I will go on.")]),
            )
        },
        FORMATION_SAINT_ID => {
            let mut list = Vec::new();
            if run.victor {
                list.push((3, "Pluto has fallen. I claim his treasure."));
            }
            list.push((1, "Summon Ultimate Pluto."));
            list.push((2, "Take me back to Ape City."));
            list.push((255, "Not yet."));
            (
                format!(
                    "Every {SYRENS_FOR_TOKEN} Syrens you slay leave the scraps of a Pluto Token, with which I can summon Ultimate Pluto. You have slain {} Syrens. Defeat him and his treasure is yours.",
                    run.syrens
                ),
                options(&list),
            )
        },
        _ => return None,
    };
    Some((text, options, FACE))
}

async fn send(actor: &Actor<ActorState>, npc_id: u32, text: impl Into<String>, options: Options) -> Result<(), Error> {
    let mut builder = MsgTaskDialog::builder().text(text.into());
    for (option, label) in options {
        builder = builder.with_option(option, label);
    }
    actor
        .send_all(builder.and().with_avatar(FACE).task_id(npc_id).build())
        .await?;
    Ok(())
}

async fn say(actor: &Actor<ActorState>, npc_id: u32, text: &str, option: &str) -> Result<(), Error> {
    send(actor, npc_id, text, vec![(255, option.to_owned())]).await
}

async fn tell(character: &Character, text: String) -> Result<(), Error> {
    character
        .owner()
        .send(MsgTalk::from_system(character.id(), TalkChannel::TopLeft, text))
        .await?;
    Ok(())
}

async fn take(actor: &Actor<ActorState>, state: &State, character: &Character, item_type: u32, count: usize) -> Result<bool, Error> {
    let Some(items) = character.consume_items(state.pool(), item_type, count).await? else {
        return Ok(false);
    };
    for item in items {
        actor.send(MsgItemInfo::remove_inventory_item(item)).await?;
    }
    Ok(true)
}

async fn give(actor: &Actor<ActorState>, state: &State, character: &Character, item_type: u32) -> Result<(), Error> {
    if let Some(item) = character
        .grant_item(state.pool(), state.shop_catalog(), item_type, false)
        .await?
    {
        actor.send(MsgItemInfo::inventory_item(item)).await?;
    }
    Ok(())
}

fn free_slots(character: &Character) -> usize {
    40usize.saturating_sub(character.inventory_items().len())
}

async fn enter_stage(state: &State, character: &Character, stage: u8) -> Result<(), Error> {
    let (map_id, x, y) = ENTRIES[usize::from(stage.clamp(1, 4) - 1)];
    character.teleport(state, map_id, (x, y)).await
}

async fn leave(state: &State, character: &Character) -> Result<(), Error> {
    let (map_id, x, y) = APE_CITY;
    character.teleport(state, map_id, (x, y)).await
}

/// Handles an answer to a Solar Saint. Returns false for other NPCs.
pub async fn answer(
    state: &State,
    actor: &Actor<ActorState>,
    character: &Character,
    npc_id: u32,
    option: u8,
) -> Result<bool, Error> {
    match npc_id {
        APE_SAINT_ID => ape_saint(state, actor, character, option).await?,
        GATE_SAINT_ID | HALL_SAINT_ID | CLOISTER_SAINT_ID | FORMATION_SAINT_ID if option == 2 => {
            leave(state, character).await?;
        },
        GATE_SAINT_ID if option == 1 => {
            if take(actor, state, character, SOUL_STONE, SOUL_STONES).await? {
                update(character, |run| run.stage = run.stage.max(2));
                enter_stage(state, character, 2).await?;
            } else {
                say(actor, npc_id, "You do not have enough SoulStones yet.", "I see.").await?;
            }
        },
        HALL_SAINT_ID if option == 1 => {
            let needed = slayer_points_needed(character.current_class());
            if run_of(character).slayer_points >= needed {
                update(character, |run| run.stage = run.stage.max(3));
                enter_stage(state, character, 3).await?;
            } else {
                say(actor, npc_id, &format!("You need {needed} Slayer Points first."), "I see.").await?;
            }
        },
        FORMATION_SAINT_ID if option == 1 => summon_pluto(state, actor, character).await?,
        FORMATION_SAINT_ID if option == 3 => claim_treasure(state, actor, character).await?,
        GATE_SAINT_ID | HALL_SAINT_ID | CLOISTER_SAINT_ID | FORMATION_SAINT_ID => {},
        _ => return Ok(false),
    }
    Ok(true)
}

async fn ape_saint(state: &State, actor: &Actor<ActorState>, character: &Character, option: u8) -> Result<(), Error> {
    const NPC: u32 = APE_SAINT_ID;
    match option {
        1 => {
            if character.entity().level() < MIN_LEVEL {
                return say(actor, NPC, "Only heroes of level 110 or above may enter Dis City.", "I see.").await;
            }
            if crate::city_npcs::timer_running(state, character, WEEK_TIMER).await? {
                return say(
                    actor,
                    NPC,
                    "You have already defeated Ultimate Pluto this week. Rest, and come back next week.",
                    "I will.",
                )
                .await;
            }
            if character.count_items(DARK_HORN) > 0 {
                return say(
                    actor,
                    NPC,
                    "Give me your Dark Horns first: Dis City devours them, and you would lose them.",
                    "I see.",
                )
                .await;
            }
            let stage = run_of(character).stage;
            enter_stage(state, character, stage).await
        },
        2 => {
            let horns = character.count_items(DARK_HORN);
            if horns == 0 {
                return say(actor, NPC, "You have no Dark Horns.", "I see.").await;
            }
            if character.entity().level() >= u16::from(crate::leveling::MAX_LEVEL) {
                return say(actor, NPC, "You can learn nothing more from the Dark Horns.", "I see.").await;
            }
            if take(actor, state, character, DARK_HORN, horns).await? {
                // "300 minutes of EXP": half an ExpBall (600) a horn.
                let seconds = crate::leveling::EXP_BALL_SECONDS / 2.0 * horns as f64;
                crate::leveling::use_exp_ball(state, actor, character, seconds).await?;
                crate::audit::economic("dis_city_dark_horn", character.id(), NPC, DARK_HORN, horns as u32);
                say(actor, NPC, "The Dark Horns' power flows into you.", "Thank you.").await?;
            }
            Ok(())
        },
        3 => {
            if character.count_items(STAR_SWORD) == 0 {
                return say(actor, NPC, "You have no Star Sword.", "I see.").await;
            }
            if take(actor, state, character, STAR_SWORD, 1).await? {
                let dress = star_sword_dress(crate::combat::next_roll());
                give(actor, state, character, dress).await?;
                crate::audit::economic("dis_city_star_sword", character.id(), NPC, dress, 0);
                say(actor, NPC, "Pluto's treasure, reforged into a dress worthy of a hero.", "Thank you.").await?;
            }
            Ok(())
        },
        4 => send(
            actor,
            NPC,
            "At the Hell Gate, collect 5 SoulStones from the demons. In the Hell Hall, earn Slayer Points (400; Archers 600, Taoists 300). In the Hell Cloister, slay the 9 Hell Wraiths in order. In the Battle Formation, Syrens leave the Pluto Token that summons Ultimate Pluto. Once a week.",
            options(&[(1, "I will go to Dis City."), (255, "I see.")]),
        )
        .await,
        _ => Ok(()),
    }
}

async fn summon_pluto(state: &State, actor: &Actor<ActorState>, character: &Character) -> Result<(), Error> {
    const NPC: u32 = FORMATION_SAINT_ID;
    let alive = (*pluto().lock()).is_some_and(|id| {
        state
            .try_map(BATTLE_FORMATION)
            .is_ok_and(|map| map.monster_entity(id).is_some())
    });
    if alive {
        return say(actor, NPC, "Ultimate Pluto already rages in the formation!", "To arms!").await;
    }
    if run_of(character).syrens < SYRENS_FOR_TOKEN {
        return say(
            actor,
            NPC,
            &format!("You need the scraps of {SYRENS_FOR_TOKEN} Syrens for a Pluto Token."),
            "I see.",
        )
        .await;
    }
    let Some(kind) = crate::bosses::monster_type(state, ULTIMATE_PLUTO).await? else {
        return Ok(());
    };
    let Some(id) = crate::bosses::spawn_monster(state, BATTLE_FORMATION, PLUTO_LAIR, &kind, 6).await? else {
        return say(actor, NPC, "The formation trembles, but Pluto does not answer. Try again.", "I see.").await;
    };
    *pluto().lock() = Some(id);
    update(character, |run| run.syrens -= SYRENS_FOR_TOKEN);
    let talk = MsgTalk::from_system(
        0,
        TalkChannel::Center,
        format!("{} has summoned Ultimate Pluto in the Battle Formation!", character.entity().name()),
    );
    for entity in state.entities() {
        if let Some(other) = entity.as_character().filter(|other| other.entity().map_id() == BATTLE_FORMATION) {
            let _ = other.owner().send(talk.clone()).await;
        }
    }
    Ok(())
}

async fn claim_treasure(state: &State, actor: &Actor<ActorState>, character: &Character) -> Result<(), Error> {
    const NPC: u32 = FORMATION_SAINT_ID;
    if !run_of(character).victor {
        return Ok(());
    }
    let prizes = treasure(crate::combat::next_roll());
    if free_slots(character) < prizes.len() {
        return say(actor, NPC, "Make room for 2 items in your inventory first.", "I see.").await;
    }
    runs().lock().remove(&character.id());
    let week = next_week(now_unix()) - now_unix();
    crate::city_npcs::start_timer(state, character, WEEK_TIMER, week).await?;
    for prize in prizes {
        give(actor, state, character, prize).await?;
        crate::audit::economic("dis_city_treasure", character.id(), NPC, prize, 0);
    }
    leave(state, character).await
}

/// A monster fell to `killer` in Dis City: Slayer Points, Hell Wraiths,
/// Syrens and Ultimate Pluto.
pub async fn defeated(state: &State, killer: &Character, monster: &Monster) -> Result<(), Error> {
    let map_id = monster.entity().map_id();
    if !is_dis_city(map_id) {
        return Ok(());
    }
    let kind = u32::try_from(monster.monster_type().id).unwrap_or_default();
    match (map_id, kind) {
        (HELL_HALL, kind) if HALL_DEMONS.contains(&kind) => {
            let needed = slayer_points_needed(killer.current_class());
            let run = update(killer, |run| {
                run.stage = run.stage.max(2);
                run.slayer_points += 1;
            });
            if run.slayer_points == needed {
                tell(killer, format!("Slayer Points: {needed}. Talk to the Solar Saint to enter the Hell Cloister.")).await?;
            } else if run.slayer_points < needed {
                tell(killer, format!("Slayer Points: {}/{needed}.", run.slayer_points)).await?;
            }
        },
        (HELL_CLOISTER, HELL_WRAITH) => {
            let spawn = monster.spawn();
            let Some(post) = wraith_post(spawn.x as u16, spawn.y as u16) else {
                return Ok(());
            };
            let run = run_of(killer);
            let expected = usize::from(run.wraiths);
            if post < expected {
                return Ok(());
            }
            if post > expected {
                let (x, y) = WRAITH_POSTS[expected];
                return tell(
                    killer,
                    format!("The crystal stays lit: this Hell Wraith is not next. Seek the one at ({x},{y})."),
                )
                .await;
            }
            let run = update(killer, |run| {
                run.stage = run.stage.max(3);
                run.wraiths += 1;
            });
            match WRAITH_POSTS.get(usize::from(run.wraiths)) {
                Some((x, y)) => {
                    tell(killer, format!("Hell Wraith {}/9 slain. The next guards ({x},{y}).", run.wraiths)).await?;
                },
                None => {
                    update(killer, |run| run.stage = 4);
                    tell(killer, "The Hell Crystals go dark: the Battle Formation opens!".to_owned()).await?;
                    enter_stage(state, killer, 4).await?;
                },
            }
        },
        (BATTLE_FORMATION, SYREN) => {
            let run = update(killer, |run| {
                run.stage = 4;
                run.syrens += 1;
            });
            if run.syrens == SYRENS_FOR_TOKEN {
                tell(killer, "The scraps make up a Pluto Token. The Solar Saint can summon Ultimate Pluto.".to_owned()).await?;
            } else if run.syrens < SYRENS_FOR_TOKEN {
                tell(killer, format!("Pluto Token scraps: {}/{SYRENS_FOR_TOKEN}.", run.syrens)).await?;
            }
        },
        (BATTLE_FORMATION, ULTIMATE_PLUTO) => {
            {
                let mut alive = pluto().lock();
                if *alive == Some(monster.id()) {
                    *alive = None;
                }
            }
            let heroes: Vec<_> = state
                .entities()
                .into_iter()
                .filter(|entity| entity.as_character().is_some_and(|hero| hero.entity().map_id() == BATTLE_FORMATION))
                .collect();
            for entity in &heroes {
                if let Some(hero) = entity.as_character() {
                    update(hero, |run| {
                        run.stage = 4;
                        run.victor = true;
                    });
                    tell(hero, "Ultimate Pluto has fallen! Claim his treasure from the Solar Saint.".to_owned()).await?;
                }
            }
            tracing::info!(target: "world", slayer = %killer.entity().name(), heroes = heroes.len(), "Ultimate Pluto was slain");
            crate::bosses::announce(
                state,
                format!("{} and the heroes of Dis City have vanquished Ultimate Pluto!", killer.entity().name()),
            )
            .await;
        },
        _ => {},
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slayer_points_follow_the_class() {
        assert_eq!(slayer_points_needed(15), 400);
        assert_eq!(slayer_points_needed(25), 400);
        assert_eq!(slayer_points_needed(42), 600);
        assert_eq!(slayer_points_needed(101), 300);
        assert_eq!(slayer_points_needed(145), 300);
    }

    #[test]
    fn wraith_posts_are_known_by_their_spawn() {
        assert_eq!(wraith_post(456, 324), Some(0));
        assert_eq!(wraith_post(296, 556), Some(8));
        assert_eq!(wraith_post(457, 324), None);
    }

    #[test]
    fn the_week_turns_on_monday() {
        // 2026-10-07 is a Wednesday; the next Monday is 2026-10-12.
        let wednesday = 1_791_373_258;
        assert_eq!(next_week(wednesday), 1_791_763_200);
        // A Monday at midnight waits a whole week.
        assert_eq!(next_week(1_791_763_200), 1_791_763_200 + 7 * 24 * 60 * 60);
    }

    #[test]
    fn treasure_and_dresses() {
        assert_eq!(treasure(5), [EXP_BALL, DRAGONBALL]);
        assert_eq!(treasure(99), [EXP_BALL, METEOR]);
        assert_eq!(star_sword_dress(0), 181_305);
        assert_eq!(star_sword_dress(6 + 7 * 2), 181_925);
        for roll in 0..100 {
            let dress = star_sword_dress(roll);
            assert!((181_305..=181_925).contains(&dress) && matches!(dress % 100, 5 | 15 | 25));
        }
    }
}
