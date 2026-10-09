//! The Rifts (docs/fendas-design.md): crypts for a team. The Rift Keeper in
//! Twin City opens a rift for the leader's team; each team gets a copy of
//! one of its tier's crypts to itself, with one or two creatures in each of
//! its rooms. The team has 25 minutes: when the last creature falls, the Rift
//! Guardian wakes in the crypt's hall; when he falls, everyone is paid and
//! the rift closes a minute later.
//!
//! The crypts are maps of ours that the content build generates
//! (`content/rifts.toml`); it also writes where each room's floor is
//! (`gravak/rifts/<layout>.txt` under `GRAVAK_CONTENT`), where the creatures
//! are put. Each crypt has `INSTANCES` copies (`migrations/97_rifts.sql`),
//! so that many teams can be in it at once; a run lives in memory.

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use tq_network::Actor;

use crate::entities::{Character, Monster};
use crate::packets::{MsgItemInfo, MsgRift, MsgTalk, MsgTaskDialog, TalkChannel};
use crate::{ActorState, Error, State};

/// The Rift Keeper in Twin City, and the way out in every copy.
pub const KEEPER_ID: u32 = 3_330;
pub const EXIT_ID_MIN: u32 = 3_400;
pub const EXIT_ID_MAX: u32 = 3_499;
const TWIN_CITY: (u32, u16, u16) = (1_002, 430, 380);
/// Where a team arrives: the middle of the entrance room.
const ENTRANCE: (u16, u16) = (13, 13);
/// Copies of each crypt; a copy's map id is the crypt's times ten plus its
/// number.
pub const INSTANCES: u32 = 8;

const TIME_LIMIT: Duration = Duration::from_secs(25 * 60);
/// How long the crypt stays open after the Guardian falls, or after a
/// defeat.
const AFTER_VICTORY: Duration = Duration::from_secs(60);
const AFTER_DEFEAT: Duration = Duration::from_secs(10);
const TEAM_MAX: usize = 5;
/// How far the team must stand from its leader to go in.
const GATHER: u16 = 18;
const ENTRIES_PER_DAY: u32 = 2;
const ENTRIES_TIMER: &str = "rift_entries";
const VICTORY_TIMER: &str = "rift_victory";
/// Creatures in each room: one, or two.
const CREATURES_PER_ROOM: [usize; 2] = [1, 2];

const FACE: u16 = 6;

/// A tier of rifts.
pub struct Tier {
    pub name: &'static str,
    pub level: u16,
    /// Its crypts (map ids of `content/rifts.toml`).
    pub layouts: [u32; 3],
    /// Its creatures (monster types of `migrations/97_rifts.sql`).
    pub creatures: [u32; 3],
    pub guardian: u32,
    /// The level of the gear the Guardian leaves.
    pub gear_level: u16,
}

pub const TIERS: [Tier; 3] = [
    Tier {
        name: "Fenda Menor",
        level: 70,
        layouts: [9_101, 9_102, 9_103],
        creatures: [9_150, 9_151, 9_152],
        guardian: 9_159,
        gear_level: 70,
    },
    Tier {
        name: "Fenda Maior",
        level: 100,
        layouts: [9_111, 9_112, 9_113],
        creatures: [9_160, 9_161, 9_162],
        guardian: 9_169,
        gear_level: 100,
    },
    Tier {
        name: "Fenda Abissal",
        level: 120,
        layouts: [9_121, 9_122, 9_123],
        creatures: [9_170, 9_171, 9_172],
        guardian: 9_179,
        gear_level: 120,
    },
];

/// Where a run stands, as the client shows it (`MsgRift::phase`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum Phase {
    /// No rift: the tracker goes.
    Closed = 0,
    Clearing = 1,
    Guardian = 2,
    Won = 3,
    Lost = 4,
}

#[derive(Debug)]
struct Run {
    tier: usize,
    layout: u32,
    members: Vec<u32>,
    started: Instant,
    deadline: Instant,
    total: u32,
    killed: u32,
    creatures: HashSet<u32>,
    guardian: Option<u32>,
    /// Where the Guardian wakes.
    hall: (u16, u16),
    phase: Phase,
    /// When the crypt sends everyone home.
    closes: Option<Instant>,
}

impl Run {
    fn packet(&self, now: Instant) -> MsgRift {
        let left = match self.phase {
            Phase::Clearing | Phase::Guardian => self.deadline.saturating_duration_since(now),
            _ => self.closes.map_or(Duration::ZERO, |closes| closes.saturating_duration_since(now)),
        };
        MsgRift {
            phase: self.phase as u32,
            tier: self.tier as u32,
            killed: self.killed,
            total: self.total,
            seconds_left: left.as_secs() as u32,
        }
    }
}

/// The runs, by the map of their copy.
fn runs() -> &'static Mutex<HashMap<u32, Run>> {
    static RUNS: std::sync::OnceLock<Mutex<HashMap<u32, Run>>> = std::sync::OnceLock::new();
    RUNS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Whether `map_id` is a copy of a crypt.
pub fn is_rift(map_id: u32) -> bool {
    TIERS.iter().flat_map(|tier| tier.layouts).any(|layout| (layout * 10..layout * 10 + INSTANCES).contains(&map_id))
}

/// Whether `map_id` is a crypt: one of its copies, or the crypt itself (a
/// game master's `$tele`). Nobody is saved in one: the maps change when the
/// content build does, and a saved place could fall outside them.
pub fn is_crypt(map_id: u32) -> bool {
    is_rift(map_id) || TIERS.iter().any(|tier| tier.layouts.contains(&map_id))
}

/// Where a hero who logs out in a rift wakes up.
pub fn wake_up_place() -> (u32, u16, u16) {
    TWIN_CITY
}

/// The rooms of a crypt: each room's kind and walkable cells.
#[derive(Debug, Clone, Default)]
struct Layout {
    rooms: Vec<(String, Vec<(u16, u16)>)>,
}

impl Layout {
    fn parse(text: &str) -> Self {
        let mut layout = Self::default();
        for line in text.lines() {
            let mut fields = line.split_whitespace();
            if fields.next() != Some("room") {
                continue;
            }
            let (_, _, Some(kind)) = (fields.next(), fields.next(), fields.next()) else {
                continue;
            };
            let mut cells = Vec::new();
            for run in fields {
                let Some((row, span)) = run.split_once(':') else { continue };
                let Some((first, last)) = span.split_once('-') else { continue };
                let (Ok(y), Ok(first), Ok(last)) = (row.parse::<u16>(), first.parse::<u16>(), last.parse::<u16>()) else {
                    continue;
                };
                cells.extend((first..=last).map(|x| (x, y)));
            }
            layout.rooms.push((kind.to_owned(), cells));
        }
        layout
    }

    /// The hall's middle cell, where the Guardian wakes.
    fn hall(&self) -> Option<(u16, u16)> {
        let (_, cells) = self.rooms.iter().find(|(kind, _)| kind == "hall")?;
        let (sx, sy) = cells.iter().fold((0u32, 0u32), |(sx, sy), (x, y)| (sx + u32::from(*x), sy + u32::from(*y)));
        let count = cells.len().max(1) as u32;
        Some(((sx / count) as u16, (sy / count) as u16))
    }
}

/// A crypt's rooms, read once from the content build.
fn layout(id: u32) -> Option<Layout> {
    static LAYOUTS: std::sync::OnceLock<Mutex<HashMap<u32, Layout>>> = std::sync::OnceLock::new();
    let layouts = LAYOUTS.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(layout) = layouts.lock().get(&id) {
        return Some(layout.clone());
    }
    let content = std::env::var_os("GRAVAK_CONTENT")?;
    let path = std::path::PathBuf::from(content).join("gravak").join("rifts").join(format!("{id}.txt"));
    let layout = Layout::parse(&std::fs::read_to_string(path).ok()?);
    layouts.lock().insert(id, layout.clone());
    Some(layout)
}

fn today() -> i64 {
    now_unix().div_euclid(24 * 60 * 60)
}

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs() as i64)
}

async fn timer(state: &State, character: &Character, kind: &str) -> Result<Option<i64>, Error> {
    Ok(
        sqlx::query_scalar::<_, i64>("SELECT until_unix FROM character_timers WHERE character_id = $1 AND kind = $2")
            .bind(character.database_id())
            .bind(kind)
            .fetch_optional(state.pool())
            .await?,
    )
}

async fn set_timer(state: &State, character: &Character, kind: &str, value: i64) -> Result<(), Error> {
    sqlx::query(
        "INSERT INTO character_timers (character_id, kind, until_unix) VALUES ($1, $2, $3)
         ON CONFLICT(character_id, kind) DO UPDATE SET until_unix = excluded.until_unix",
    )
    .bind(character.database_id())
    .bind(kind)
    .bind(value)
    .execute(state.pool())
    .await?;
    Ok(())
}

/// Entries used today: kept as `day * 10 + entries`.
async fn entries_today(state: &State, character: &Character) -> Result<u32, Error> {
    Ok(match timer(state, character, ENTRIES_TIMER).await? {
        Some(value) if value.div_euclid(10) == today() => value.rem_euclid(10) as u32,
        _ => 0,
    })
}

type Options = Vec<(u8, String)>;

pub fn opening(npc_id: u32, _character: &Character) -> Option<(String, Options, u16)> {
    let options = |list: &[(u8, &str)]| list.iter().map(|(id, label)| (*id, (*label).to_owned())).collect::<Options>();
    match npc_id {
        KEEPER_ID => Some((
            format!(
                "The Gravak relic tore rifts into a broken world. Lead your team through one (up to {TEAM_MAX} heroes, all beside you), clear every creature in its crypt and face the Guardian that wakes at its heart. You have {} minutes; each hero may enter {ENTRIES_PER_DAY} times a day.",
                TIME_LIMIT.as_secs() / 60
            ),
            options(&[
                (1, "Open a Fenda Menor (level 70)."),
                (2, "Open a Fenda Maior (level 100)."),
                (3, "Open a Fenda Abissal (level 120)."),
                (255, "Not now."),
            ]),
            FACE,
        )),
        EXIT_ID_MIN..=EXIT_ID_MAX => Some((
            "This way leads back to Twin City. Whoever leaves cannot come back into this rift.".to_owned(),
            options(&[(1, "Take me back to Twin City."), (255, "I stay.")]),
            FACE,
        )),
        _ => None,
    }
}

async fn say(actor: &Actor<ActorState>, npc_id: u32, text: &str) -> Result<(), Error> {
    let builder = MsgTaskDialog::builder().text(text.to_owned()).with_option(255, "I see.".to_owned());
    actor.send_all(builder.and().with_avatar(FACE).task_id(npc_id).build()).await?;
    Ok(())
}

/// Handles an answer to the Keeper or a way out. False for other NPCs.
pub async fn answer(
    state: &State,
    actor: &Actor<ActorState>,
    character: &Character,
    npc_id: u32,
    option: u8,
) -> Result<bool, Error> {
    match npc_id {
        KEEPER_ID => {
            if let Some(tier) = option.checked_sub(1).map(usize::from).filter(|tier| *tier < TIERS.len()) {
                if let Err(reason) = open(state, character, tier, actor.is_gm()).await? {
                    say(actor, npc_id, &reason).await?;
                }
            }
            Ok(true)
        },
        EXIT_ID_MIN..=EXIT_ID_MAX => {
            if option == 1 {
                leave(state, character).await?;
            }
            Ok(true)
        },
        _ => Ok(false),
    }
}

/// Sends `character` home and hides the tracker.
async fn leave(state: &State, character: &Character) -> Result<(), Error> {
    let (map_id, x, y) = TWIN_CITY;
    character.teleport(state, map_id, (x, y)).await?;
    character.owner().send(closed()).await?;
    Ok(())
}

fn closed() -> MsgRift {
    MsgRift { phase: Phase::Closed as u32, ..MsgRift::default() }
}

/// Opens a rift of `tier` for `leader`'s team. Ok(Err) says why not. A game
/// master's team is not held to the entries of the day, nor counted.
async fn open(state: &State, leader: &Character, tier: usize, gm: bool) -> Result<Result<(), String>, Error> {
    let tier_def = &TIERS[tier];
    let teams = state.team_manager();
    if teams.leader_for(leader.id()).is_some_and(|id| id != leader.id()) {
        return Ok(Err("Only the leader of your team may open a rift.".to_owned()));
    }
    let members = teams.members_for(leader.id());
    if members.len() > TEAM_MAX {
        return Ok(Err(format!("A rift takes at most {TEAM_MAX} heroes.")));
    }
    let here = leader.entity().location();
    let mut heroes = Vec::new();
    for id in &members {
        let Some(entity) = state.entity(*id) else {
            return Ok(Err("Everyone in your team must be online.".to_owned()));
        };
        let Some(member) = entity.as_character() else { continue };
        let there = member.entity().location();
        if member.entity().map_id() != leader.entity().map_id()
            || there.x.abs_diff(here.x) > GATHER
            || there.y.abs_diff(here.y) > GATHER
        {
            return Ok(Err(format!("{} must stand here with you.", member.entity().name())));
        }
        if member.entity().level() < tier_def.level {
            return Ok(Err(format!("{} is below level {}.", member.entity().name(), tier_def.level)));
        }
        if !gm && entries_today(state, member).await? >= ENTRIES_PER_DAY {
            return Ok(Err(format!("{} has entered the rifts {ENTRIES_PER_DAY} times today already.", member.entity().name())));
        }
        heroes.push(entity.clone());
    }
    // A free copy of one of the tier's crypts.
    let free: Vec<(u32, u32)> = {
        let runs = runs().lock();
        tier_def
            .layouts
            .iter()
            .flat_map(|layout| (0..INSTANCES).map(move |copy| (*layout, layout * 10 + copy)))
            .filter(|(_, map)| !runs.contains_key(map))
            .collect()
    };
    if free.is_empty() {
        return Ok(Err("Every rift of this kind is taken. Try again in a few minutes.".to_owned()));
    }
    let (layout_id, map_id) = free[crate::combat::next_roll() as usize % free.len()];
    let Some(layout) = layout(layout_id) else {
        tracing::warn!(layout_id, "The rift's layout is missing (GRAVAK_CONTENT)");
        return Ok(Err("The rift will not open. Tell a game master.".to_owned()));
    };
    let Ok(map) = state.try_map(map_id) else {
        tracing::warn!(map_id, "The rift's copy is not in the maps table");
        return Ok(Err("The rift will not open. Tell a game master.".to_owned()));
    };
    map.load().await?;
    let hall = layout.hall().unwrap_or(ENTRANCE);
    let now = Instant::now();
    // Reserved before the creatures come, so no other team takes it.
    runs().lock().insert(
        map_id,
        Run {
            tier,
            layout: layout_id,
            members: members.clone(),
            started: now,
            deadline: now + TIME_LIMIT,
            total: 0,
            killed: 0,
            creatures: HashSet::new(),
            guardian: None,
            hall,
            phase: Phase::Clearing,
            closes: None,
        },
    );
    let mut kinds = Vec::new();
    for id in tier_def.creatures {
        if let Some(kind) = crate::bosses::monster_type(state, id).await? {
            kinds.push(kind);
        }
    }
    let mut creatures = HashSet::new();
    if !kinds.is_empty() {
        for (kind, cells) in &layout.rooms {
            if kind != "room" || cells.is_empty() {
                continue;
            }
            let count = CREATURES_PER_ROOM[crate::combat::next_roll() as usize % CREATURES_PER_ROOM.len()];
            for _ in 0..count {
                let cell = cells[crate::combat::next_roll() as usize % cells.len()];
                let creature = &kinds[crate::combat::next_roll() as usize % kinds.len()];
                if let Some(id) = crate::bosses::spawn_monster(state, map_id, cell, creature, 2).await? {
                    creatures.insert(id);
                }
            }
        }
    }
    if let Some(run) = runs().lock().get_mut(&map_id) {
        run.total = creatures.len() as u32;
        run.creatures = creatures;
    }
    for entity in &heroes {
        let Some(member) = entity.as_character() else { continue };
        if !gm {
            let entries = entries_today(state, member).await? + 1;
            set_timer(state, member, ENTRIES_TIMER, today() * 10 + i64::from(entries)).await?;
        }
        member.teleport(state, map_id, ENTRANCE).await?;
        member
            .owner()
            .send(MsgTalk::from_system(
                member.id(),
                TalkChannel::Center,
                format!("{}: clear every creature, then face the Guardian!", tier_def.name),
            ))
            .await?;
    }
    send_state(state, map_id).await;
    tracing::info!(target: "world", map_id, layout_id, tier = tier_def.name, team = members.len(), "A rift opened");
    Ok(Ok(()))
}

/// Tells everyone in the run where it stands.
async fn send_state(state: &State, map_id: u32) {
    let (packet, members) = {
        let runs = runs().lock();
        let Some(run) = runs.get(&map_id) else { return };
        (run.packet(Instant::now()), run.members.clone())
    };
    for id in members {
        if let Some(member) = state.entity(id).and_then(|entity| entity.as_character().map(|c| c.owner().clone())) {
            let _ = member.send(packet.clone()).await;
        }
    }
}

async fn tell(state: &State, map_id: u32, text: String) {
    let members = runs().lock().get(&map_id).map(|run| run.members.clone()).unwrap_or_default();
    for id in members {
        if let Some(entity) = state.entity(id) {
            if let Some(member) = entity.as_character() {
                let _ = member.owner().send(MsgTalk::from_system(member.id(), TalkChannel::Center, text.clone())).await;
            }
        }
    }
}

/// A monster fell in a rift: the count goes on, the Guardian wakes when the
/// last creature falls, and the run is won when he does.
pub async fn defeated(state: &State, character: &Character, monster: &Monster) -> Result<(), Error> {
    let map_id = character.entity().map_id();
    enum Next {
        Nothing,
        Counted,
        Wake { kind: u32, hall: (u16, u16) },
        Won,
    }
    let next = {
        let mut runs = runs().lock();
        let Some(run) = runs.get_mut(&map_id) else { return Ok(()) };
        if run.creatures.remove(&monster.id()) {
            run.killed += 1;
            if run.creatures.is_empty() && run.phase == Phase::Clearing {
                run.phase = Phase::Guardian;
                Next::Wake { kind: TIERS[run.tier].guardian, hall: run.hall }
            } else {
                Next::Counted
            }
        } else if run.guardian == Some(monster.id()) && run.phase == Phase::Guardian {
            run.phase = Phase::Won;
            run.closes = Some(Instant::now() + AFTER_VICTORY);
            Next::Won
        } else {
            Next::Nothing
        }
    };
    match next {
        Next::Nothing => {},
        Next::Counted => send_state(state, map_id).await,
        Next::Wake { kind, hall } => {
            if let Some(kind) = crate::bosses::monster_type(state, kind).await? {
                let guardian = crate::bosses::spawn_monster(state, map_id, hall, &kind, 3).await?;
                if let Some(run) = runs().lock().get_mut(&map_id) {
                    run.guardian = guardian;
                }
                tell(state, map_id, format!("The last creature fell. {} wakes in the hall!", kind.name)).await;
            }
            send_state(state, map_id).await;
        },
        Next::Won => {
            reward(state, map_id).await?;
            send_state(state, map_id).await;
        },
    }
    Ok(())
}

/// The Guardian's treasure, for each hero of the team in the rift.
async fn reward(state: &State, map_id: u32) -> Result<(), Error> {
    let Some((tier, members, took)) = runs()
        .lock()
        .get(&map_id)
        .map(|run| (run.tier, run.members.clone(), run.started.elapsed()))
    else {
        return Ok(());
    };
    let minutes = took.as_secs() / 60;
    let seconds = took.as_secs() % 60;
    tracing::info!(target: "world", map_id, tier = TIERS[tier].name, ?took, "A rift was won");
    for id in members {
        let Some(entity) = state.entity(id) else { continue };
        let Some(member) = entity.as_character() else { continue };
        if member.entity().map_id() != map_id {
            continue;
        }
        let first_today = timer(state, member, VICTORY_TIMER).await? != Some(today());
        let items = treasure(tier, first_today, crate::combat::next_roll, |item| state.shop_catalog().template(item).is_some());
        for item_type in &items {
            match member.grant_item(state.pool(), state.shop_catalog(), *item_type, false).await? {
                Some(item) => member.owner().send(MsgItemInfo::inventory_item(item)).await?,
                None => {
                    member
                        .owner()
                        .send(MsgTalk::from_system(member.id(), TalkChannel::System, "Your bag is full: part of the treasure was lost.".to_owned()))
                        .await?;
                    break;
                },
            }
            crate::audit::economic("rift_treasure", member.id(), map_id, *item_type, 1);
        }
        if first_today {
            set_timer(state, member, VICTORY_TIMER, today()).await?;
        }
        member
            .owner()
            .send(MsgTalk::from_system(
                member.id(),
                TalkChannel::Center,
                format!(
                    "The Guardian has fallen in {minutes}:{seconds:02}! {} treasures are yours{}. The rift closes in a minute.",
                    items.len(),
                    if first_today { ", with the day's first victory bonus" } else { "" }
                ),
            ))
            .await?;
    }
    Ok(())
}

/// What the Guardian of `tier` leaves each hero: the tier's sure prizes,
/// its chances, and an ExpBall more for the day's first victory.
pub fn treasure(tier: usize, first_today: bool, mut roll: impl FnMut() -> u32, exists: impl Fn(u32) -> bool) -> Vec<u32> {
    use crate::drops::{DRAGONBALL, METEOR};
    const EXP_BALL: u32 = crate::leveling::EXP_BALL;
    const METEOR_SCROLL: u32 = 720_027;
    let gem = |grade: u32, roll: u32| crate::entities::GEM_FAMILY_BASES[(roll % 7) as usize] + grade;
    let mut items = match tier {
        0 => vec![METEOR, METEOR, EXP_BALL],
        1 => vec![METEOR, METEOR, METEOR, DRAGONBALL, EXP_BALL],
        _ => vec![DRAGONBALL, DRAGONBALL, EXP_BALL, EXP_BALL],
    };
    fn chance(roll: &mut impl FnMut() -> u32, percent: u32) -> bool {
        roll() % 100 < percent
    }
    match tier {
        0 => {
            if chance(&mut roll, 30) {
                items.push(DRAGONBALL);
            }
            if chance(&mut roll, 25) {
                items.push(gem(1, roll()));
            }
        },
        1 => {
            if chance(&mut roll, 30) {
                items.push(gem(2, roll()));
            }
            if chance(&mut roll, 20) {
                items.push(METEOR_SCROLL);
            }
        },
        _ => {
            if chance(&mut roll, 30) {
                items.push(gem(3, roll()));
            }
        },
    }
    // Gear of the tier's level: Elite in a Lesser Rift, Super above.
    let (gear_percent, quality) = match tier {
        0 => (20, 8),
        1 => (25, 9),
        _ => (35, 9),
    };
    if chance(&mut roll, gear_percent) {
        for _ in 0..20 {
            let gear = crate::drops::gear_id(TIERS[tier.min(2)].gear_level, &mut roll) / 10 * 10 + quality;
            if exists(gear) {
                items.push(gear);
                break;
            }
        }
    }
    if first_today {
        items.push(EXP_BALL);
    }
    items.retain(|item| exists(*item));
    items
}

/// Keeps the runs: who is still in, the time, the defeat when it runs out
/// or the whole team is down, and the end.
pub async fn tick(state: &State) {
    let now = Instant::now();
    let mut ended = Vec::new();
    let mut lost = Vec::new();
    let mut emptied = Vec::new();
    {
        let mut runs = runs().lock();
        for (map_id, run) in runs.iter_mut() {
            // Who left (logged out, went home) is out of the run.
            run.members.retain(|id| {
                state
                    .entity(*id)
                    .and_then(|entity| entity.as_character().map(|member| member.entity().map_id() == *map_id))
                    .unwrap_or(false)
            });
            if run.members.is_empty() {
                emptied.push(*map_id);
                continue;
            }
            match run.phase {
                Phase::Clearing | Phase::Guardian => {
                    let all_down = run.members.iter().all(|id| {
                        state
                            .entity(*id)
                            .and_then(|entity| entity.as_character().map(|member| member.entity().is_dead()))
                            .unwrap_or(true)
                    });
                    if now >= run.deadline || all_down {
                        run.phase = Phase::Lost;
                        run.closes = Some(now + AFTER_DEFEAT);
                        lost.push((*map_id, all_down));
                    }
                },
                Phase::Won | Phase::Lost => {
                    if run.closes.is_some_and(|closes| now >= closes) {
                        ended.push(*map_id);
                    }
                },
                Phase::Closed => ended.push(*map_id),
            }
        }
    }
    for (map_id, all_down) in lost {
        let reason = if all_down { "Your whole team has fallen." } else { "Time is up." };
        tell(state, map_id, format!("{reason} The rift is closing!")).await;
        send_state(state, map_id).await;
    }
    for map_id in ended {
        let members = runs().lock().get(&map_id).map(|run| run.members.clone()).unwrap_or_default();
        for id in members {
            if let Some(entity) = state.entity(id) {
                if let Some(member) = entity.as_character() {
                    if member.entity().is_dead() {
                        let _ = crate::death::revive(state, member, false).await;
                    }
                    if let Err(error) = leave(state, member).await {
                        tracing::warn!(%error, map_id, "A hero could not leave the rift");
                    }
                }
            }
        }
        emptied.push(map_id);
    }
    for map_id in emptied {
        close(state, map_id);
    }
}

/// Clears a copy for the next team: its creatures and Guardian go.
fn close(state: &State, map_id: u32) {
    let Some(run) = runs().lock().remove(&map_id) else { return };
    if let Ok(map) = state.try_map(map_id) {
        for entity in map.monster_entities() {
            map.remove_monster(entity.id());
        }
    }
    tracing::info!(target: "world", map_id, layout = run.layout, killed = run.killed, total = run.total, "A rift closed");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copies_belong_to_their_crypt() {
        assert!(is_rift(91_010));
        assert!(is_rift(91_017));
        assert!(!is_rift(91_018));
        assert!(is_rift(91_230));
        assert!(!is_rift(9_101));
        assert!(!is_rift(1_002));
        assert!(is_crypt(9_101) && is_crypt(91_010) && !is_crypt(1_002));
    }

    #[test]
    fn layouts_read_their_rooms() {
        let layout = Layout::parse("# x\nsize 88\nroom 0 0 entrance 13:11-15\nroom 3 3 hall 40:38-42 41:38-42\n");
        assert_eq!(layout.rooms.len(), 2);
        assert_eq!(layout.rooms[0].1, vec![(11, 13), (12, 13), (13, 13), (14, 13), (15, 13)]);
        assert_eq!(layout.hall(), Some((40, 40)));
    }

    #[test]
    fn treasures_grow_with_the_tier() {
        let mut counter = 0u32;
        let roll = || {
            counter = counter.wrapping_add(7919);
            counter
        };
        let lesser = treasure(0, false, roll, |_| true);
        assert!(lesser.iter().filter(|item| **item == crate::drops::METEOR).count() >= 2);
        let abyssal = treasure(2, true, || 0, |_| true);
        // Every chance taken: two DragonBalls, ExpBalls, a super gem, gear
        // and the first victory's ExpBall.
        assert!(abyssal.iter().filter(|item| **item == crate::drops::DRAGONBALL).count() == 2);
        assert!(abyssal.iter().filter(|item| **item == crate::leveling::EXP_BALL).count() == 3);
        assert!(abyssal.iter().any(|item| (700_003..700_080).contains(item) && item % 10 == 3));
        assert!(treasure(1, false, || 0, |_| false).is_empty(), "items that do not exist are left out");
    }
}
