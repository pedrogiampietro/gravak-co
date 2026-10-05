//! Guild War on the guild map (1038), after the reference server
//! (Redux `Events/GuildWar.cs`, `Objects/SOB.cs` and `Npcs/Guild Map`).
//!
//! The pole and the two gates are the original terrain NPCs of the map
//! (`SynFlag` and `CityGate`). While the war runs, guild members hit the
//! pole for score (a third of the damage) and silver; when the pole falls
//! the best guild wins the round, the pole carries its name and a new round
//! starts ten seconds later. Gates fall open at zero life; the winners'
//! leader can open, close and repair them.

use std::collections::HashMap;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use chrono::{Datelike, Timelike};
use parking_lot::Mutex;
use tq_network::Actor;

use crate::entities::{Character, Npc};
use crate::packets::{
    InteractAction, MsgInteract, MsgNpcInfo, MsgTalk, MsgTaskDialog, MsgUserAttrib, MsgUserInfo, TalkChannel,
    UserAttribute,
};
use crate::{ActorState, Error, State};

pub const GUILD_MAP: u32 = 1_038;
pub const POLE_ID: u32 = 100_810;
pub const LEFT_GATE_ID: u32 = 108_855;
pub const RIGHT_GATE_ID: u32 = 108_856;
pub const POLE_LIFE: u32 = 20_000_000;
pub const GATE_LIFE: u32 = 10_000_000;
const GATE_REPAIR_COST: u32 = 2_000_000;
/// Closed and open looks of the left and right gates.
const LEFT_GATE_LOOKS: (u16, u16) = (241, 251);
const RIGHT_GATE_LOOKS: (u16, u16) = (277, 287);
/// Cells around a closed gate that cannot be walked or jumped through.
const GATE_HALF_WIDTH: u16 = 3;
const NEW_ROUND_DELAY: Duration = Duration::from_secs(10);
const SCORE_PERIOD: Duration = Duration::from_secs(10);
/// Inside the gates, for the winners.
const LEFT_GATE_INSIDE: (u16, u16) = (162, 199);
const RIGHT_GATE_INSIDE: (u16, u16) = (209, 177);
/// The war prison and Twin City.
pub const WAR_PRISON: (u32, u16, u16) = (6_001, 29, 72);
const TWIN_CITY: (u32, u16, u16) = (1_002, 430, 380);
const GUILD_LEADER_ROLE: u32 = 1_000;
/// The weekly war: Saturday from 20:00 to 22:00 server time.
const WAR_WEEKDAY: chrono::Weekday = chrono::Weekday::Sat;
const WAR_HOURS: std::ops::Range<u32> = 20..22;

pub const GATE_KEEPER_ID: u32 = 7_000;
pub const WARDEN_ID: u32 = 140;
/// Guild map conductresses: (npc, destination).
const CONDUCTRESSES: [(u32, (u32, u16, u16)); 4] = [
    (815, (1_015, 744, 630)),
    (816, (1_000, 492, 669)),
    (817, (1_020, 571, 585)),
    // Redux sends this one to map 2054, which the 5065 data does not
    // have; it goes to Phoenix Castle, beside its Conductress.
    (818, (1_011, 230, 258)),
];
const CONDUCTRESS_FEE: u32 = 1_000;

#[derive(Debug, Clone)]
struct Winner {
    guild_id: u32,
    name: String,
}

#[derive(Debug)]
struct War {
    running: bool,
    /// Started by a command: the schedule does not end it.
    manual: bool,
    winner: Option<Winner>,
    scores: HashMap<u32, (String, u64)>,
    life: HashMap<u32, u32>,
    open: HashMap<u32, bool>,
    new_round_at: Option<Instant>,
    last_scores: Instant,
}

impl Default for War {
    fn default() -> Self {
        Self {
            running: false,
            manual: false,
            winner: None,
            scores: HashMap::new(),
            life: HashMap::from([(POLE_ID, POLE_LIFE), (LEFT_GATE_ID, GATE_LIFE), (RIGHT_GATE_ID, GATE_LIFE)]),
            open: HashMap::new(),
            new_round_at: None,
            last_scores: Instant::now(),
        }
    }
}

fn war() -> &'static Mutex<War> {
    static WAR: OnceLock<Mutex<War>> = OnceLock::new();
    WAR.get_or_init(Default::default)
}

pub fn is_war_structure(npc_id: u32) -> bool {
    matches!(npc_id, POLE_ID | LEFT_GATE_ID | RIGHT_GATE_ID)
}

pub fn running() -> bool {
    war().lock().running
}

fn max_life(npc_id: u32) -> u32 {
    if npc_id == POLE_ID {
        POLE_LIFE
    } else {
        GATE_LIFE
    }
}

pub fn life(npc_id: u32) -> u32 {
    war().lock().life.get(&npc_id).copied().unwrap_or_else(|| max_life(npc_id))
}

fn gate_looks(npc_id: u32) -> (u16, u16) {
    if npc_id == LEFT_GATE_ID {
        LEFT_GATE_LOOKS
    } else {
        RIGHT_GATE_LOOKS
    }
}

/// A gate is open when its leader opened it or it was broken.
fn gate_open(war: &War, npc_id: u32) -> bool {
    war.open.get(&npc_id).copied().unwrap_or(false) || war.life.get(&npc_id).is_some_and(|life| *life <= 1)
}

/// The spawn packet of a war structure: the pole carries the winner's
/// name, the gates show open or closed.
pub fn spawn_packet(npc: &Npc) -> MsgNpcInfo {
    let war = war().lock();
    match npc.id() {
        POLE_ID => MsgNpcInfo::with_look(
            npc,
            npc.entity().mesh() as u16,
            Some(war.winner.as_ref().map_or_else(|| "None".to_owned(), |winner| winner.name.clone())),
        ),
        id => {
            let (closed, open) = gate_looks(id);
            MsgNpcInfo::with_look(npc, if gate_open(&war, id) { open } else { closed }, None)
        },
    }
}

/// Whether a closed gate stands on (x, y) of the guild map.
pub fn blocks(map_id: u32, x: u16, y: u16, gates: &[(u32, u16, u16)]) -> bool {
    if map_id != GUILD_MAP {
        return false;
    }
    let war = war().lock();
    gates.iter().any(|(id, gx, gy)| {
        !gate_open(&war, *id) && x.abs_diff(*gx) <= GATE_HALF_WIDTH && y.abs_diff(*gy) <= GATE_HALF_WIDTH
    })
}

/// Whether a walk or jump from `from` to `to` crosses a closed gate.
pub fn path_blocked(state: &State, map_id: u32, from: (u16, u16), to: (u16, u16)) -> bool {
    if map_id != GUILD_MAP {
        return false;
    }
    let Ok(map) = state.try_map(map_id) else {
        return false;
    };
    let gates = [LEFT_GATE_ID, RIGHT_GATE_ID]
        .into_iter()
        .filter_map(|id| map.npc(id).map(|npc| (id, npc.entity().location().x, npc.entity().location().y)))
        .collect::<Vec<_>>();
    let steps = from.0.abs_diff(to.0).max(from.1.abs_diff(to.1)).max(1);
    (0..=steps).any(|step| {
        let t = f32::from(step) / f32::from(steps);
        let x = (f32::from(from.0) + (f32::from(to.0) - f32::from(from.0)) * t).round() as u16;
        let y = (f32::from(from.1) + (f32::from(to.1) - f32::from(from.1)) * t).round() as u16;
        blocks(map_id, x, y, &gates)
    })
}

async fn broadcast_map(state: &State, map_id: u32, packet: impl Clone + tq_network::PacketEncode + Send + Sync + 'static) {
    for entity in state.entities() {
        if let Some(character) = entity.as_character() {
            if character.entity().map_id() == map_id {
                let _ = character.owner().send(packet.clone()).await;
            }
        }
    }
}

async fn broadcast_all(state: &State, text: String) {
    for entity in state.entities() {
        if let Some(character) = entity.as_character() {
            let _ = character
                .owner()
                .send(MsgTalk::from_system(character.id(), TalkChannel::Center, text.clone()))
                .await;
        }
    }
}

/// Re-sends a structure's look, name and life to everyone on the map.
async fn refresh(state: &State, npc_id: u32) {
    let Ok(map) = state.try_map(GUILD_MAP) else {
        return;
    };
    let Some(npc) = map.npc(npc_id) else {
        return;
    };
    broadcast_map(state, GUILD_MAP, spawn_packet(npc)).await;
    broadcast_map(state, GUILD_MAP, MsgUserAttrib::new(npc_id, UserAttribute::MaxLife, u64::from(max_life(npc_id)))).await;
    broadcast_map(state, GUILD_MAP, MsgUserAttrib::new(npc_id, UserAttribute::Life, u64::from(life(npc_id)))).await;
}

/// Sends the life of the war structures to one player entering the map.
pub async fn send_lives(actor: &Actor<ActorState>) -> Result<(), Error> {
    for id in [POLE_ID, LEFT_GATE_ID, RIGHT_GATE_ID] {
        actor.send(MsgUserAttrib::new(id, UserAttribute::MaxLife, u64::from(max_life(id)))).await?;
        actor.send(MsgUserAttrib::new(id, UserAttribute::Life, u64::from(life(id)))).await?;
    }
    Ok(())
}

async fn reset_structures(state: &State) {
    {
        let mut war = war().lock();
        war.life = Default::default();
        war.life.extend([(POLE_ID, POLE_LIFE), (LEFT_GATE_ID, GATE_LIFE), (RIGHT_GATE_ID, GATE_LIFE)]);
        war.open.clear();
        war.scores.clear();
        war.new_round_at = None;
    }
    for id in [POLE_ID, LEFT_GATE_ID, RIGHT_GATE_ID] {
        refresh(state, id).await;
    }
}

pub async fn start(state: &State, manual: bool) {
    {
        let mut war = war().lock();
        if war.running {
            return;
        }
        war.running = true;
        war.manual = manual;
    }
    reset_structures(state).await;
    broadcast_all(state, "The Guild War has begun! Guilds, attack the pole on the guild map!".to_owned()).await;
}

pub async fn end(state: &State) {
    let winner = {
        let mut war = war().lock();
        if !war.running {
            return;
        }
        war.running = false;
        war.manual = false;
        war.winner.clone()
    };
    reset_structures(state).await;
    let name = winner.map_or_else(|| "No guild".to_owned(), |winner| winner.name);
    broadcast_all(state, format!("{name} has won! Congratulations to everyone for their efforts.")).await;
}

/// The guild with the best score wins the round.
fn best_guild(scores: &HashMap<u32, (String, u64)>) -> Option<(u32, String, u64)> {
    scores
        .iter()
        .max_by_key(|(_, (_, score))| *score)
        .map(|(id, (name, score))| (*id, name.clone(), *score))
}

async fn round_end(state: &State) {
    let best = {
        let mut war = war().lock();
        let best = best_guild(&war.scores);
        if let Some((guild_id, name, _)) = &best {
            war.winner = Some(Winner {
                guild_id: *guild_id,
                name: name.clone(),
            });
        }
        war.new_round_at = Some(Instant::now() + NEW_ROUND_DELAY);
        best
    };
    if let Some((guild_id, name, score)) = best {
        persist_winner(state, guild_id, &name).await;
        broadcast_all(
            state,
            format!("{name} has emerged victorious with a score of: {score} Congratulations for their valiant efforts"),
        )
        .await;
    }
    refresh(state, POLE_ID).await;
}

async fn persist_winner(state: &State, guild_id: u32, name: &str) {
    let result = sqlx::query(
        "INSERT INTO guild_war (id, winner_guild_id, winner_name) VALUES (1, $1, $2)
         ON CONFLICT(id) DO UPDATE SET winner_guild_id = excluded.winner_guild_id, winner_name = excluded.winner_name",
    )
    .bind(i64::from(guild_id))
    .bind(name)
    .execute(state.pool())
    .await;
    if let Err(error) = result {
        tracing::warn!(%error, "Failed to save the guild war winner");
    }
}

/// Restores the last winner at start-up.
pub async fn load(state: &State) {
    let row = sqlx::query_as::<_, (i64, String)>("SELECT winner_guild_id, winner_name FROM guild_war WHERE id = 1")
        .fetch_optional(state.pool())
        .await;
    if let Ok(Some((guild_id, name))) = row {
        war().lock().winner = Some(Winner {
            guild_id: guild_id.max(0) as u32,
            name,
        });
    }
}

/// Server time: UTC shifted by `GUILD_WAR_UTC_OFFSET` hours (default -3,
/// Brasília), since chrono is built without the system clock.
fn server_time() -> chrono::NaiveDateTime {
    let offset = std::env::var("GUILD_WAR_UTC_OFFSET")
        .ok()
        .and_then(|value| value.trim().parse::<i64>().ok())
        .unwrap_or(-3);
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs() as i64);
    chrono::DateTime::from_timestamp(seconds + offset * 3_600, 0)
        .map(|time| time.naive_utc())
        .unwrap_or_default()
}

fn in_war_window(now: chrono::NaiveDateTime) -> bool {
    now.weekday() == WAR_WEEKDAY && WAR_HOURS.contains(&now.hour())
}

/// Schedule, new rounds and the score table; runs with the world ticks.
pub async fn tick(state: &State) {
    let window = in_war_window(server_time());
    let (running, manual) = {
        let war = war().lock();
        (war.running, war.manual)
    };
    if window && !running {
        start(state, false).await;
    } else if !window && running && !manual {
        end(state).await;
    }
    let (new_round, scores) = {
        let mut war = war().lock();
        if !war.running {
            return;
        }
        let new_round = war.new_round_at.is_some_and(|at| at <= Instant::now());
        let scores = (war.last_scores.elapsed() >= SCORE_PERIOD).then(|| {
            war.last_scores = Instant::now();
            let mut table = war.scores.values().cloned().collect::<Vec<_>>();
            table.sort_by(|a, b| b.1.cmp(&a.1));
            table
        });
        (new_round, scores)
    };
    if new_round {
        reset_structures(state).await;
    }
    if let Some(table) = scores.filter(|table| !table.is_empty()) {
        for (rank, (name, score)) in table.into_iter().take(5).enumerate() {
            let line = format!("No {}: {name}: {score}", rank + 1);
            for entity in state.entities() {
                if let Some(character) = entity.as_character() {
                    if character.entity().map_id() == GUILD_MAP {
                        let _ = character
                            .owner()
                            .send(MsgTalk::from_system(character.id(), TalkChannel::TopLeft, line.clone()))
                            .await;
                    }
                }
            }
        }
    }
}

/// An attack on the pole or a gate. The war structures only take damage
/// while the war runs.
/// Why a structure cannot be hit now, if it cannot.
pub fn refuses_hit(npc_id: u32) -> Option<&'static str> {
    let war = war().lock();
    if !war.running {
        Some("The Guild War is not running.")
    } else if war.new_round_at.is_some() {
        Some("A new round starts in a moment.")
    } else if npc_id != POLE_ID && gate_open(&war, npc_id) {
        Some("This gate is open.")
    } else {
        None
    }
}

pub async fn hit(state: &State, actor: &Actor<ActorState>, character: &Character, npc: &Npc, damage: u32) -> Result<(), Error> {
    let guild = state.guild_manager().guild_for(character.id());
    let npc_id = npc.id();
    let (remaining, pole_fell, gate_fell) = {
        let mut war = war().lock();
        if !war.running || war.new_round_at.is_some() || (npc_id != POLE_ID && gate_open(&war, npc_id)) {
            return Ok(());
        }
        let life = war.life.entry(npc_id).or_insert_with(|| max_life(npc_id));
        let floor = if npc_id == POLE_ID { 0 } else { 1 };
        *life = life.saturating_sub(damage).max(floor);
        let remaining = *life;
        if npc_id == POLE_ID {
            if let Some(guild) = &guild {
                let entry = war.scores.entry(guild.id).or_insert_with(|| (guild.name.clone(), 0));
                entry.1 = entry.1.saturating_add(u64::from(damage / 3));
            }
        }
        (remaining, npc_id == POLE_ID && remaining == 0, npc_id != POLE_ID && remaining <= 1)
    };
    broadcast_map(state, GUILD_MAP, MsgUserAttrib::new(npc_id, UserAttribute::Life, u64::from(remaining))).await;
    if npc_id == POLE_ID && guild.is_some() {
        character.grant_silver(state.pool(), damage / 250).await?;
        actor.send(MsgUserInfo::from(character)).await?;
    }
    if pole_fell || gate_fell {
        let location = npc.entity().location();
        let kill = MsgInteract {
            timestamp: 0,
            character_id: character.id(),
            target_id: npc_id,
            x: location.x,
            y: location.y,
            action: InteractAction::Kill as u32,
            value: 0,
            reserved: [0; 12],
        };
        broadcast_map(state, GUILD_MAP, kill).await;
    }
    if pole_fell {
        round_end(state).await;
    } else if gate_fell {
        refresh(state, npc_id).await;
    }
    Ok(())
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

fn winner_member(state: &State, character: &Character) -> Option<(u32, bool)> {
    let guild = state.guild_manager().guild_for(character.id())?;
    let war = war().lock();
    let winner = war.winner.as_ref()?;
    (winner.guild_id == guild.id).then(|| {
        (
            guild.id,
            state.guild_manager().role_for(character.id()) == Some(GUILD_LEADER_ROLE) || guild.leader_id == character.id(),
        )
    })
}

type Options = Vec<(u8, String)>;

fn options(list: &[(u8, &str)]) -> Options {
    list.iter().map(|(id, label)| (*id, (*label).to_owned())).collect()
}

pub fn opening(state: &State, npc_id: u32, character: &Character) -> Option<(String, Options, u16)> {
    let (text, options) = match npc_id {
        LEFT_GATE_ID | RIGHT_GATE_ID => {
            winner_member(state, character)?;
            if life(npc_id) > 1 {
                (
                    "Your guild is currently Victorious. I can let you inside if you wish!",
                    options(&[(1, "Let me in"), (2, "Open/Close the Gate"), (255, "No Thanks.")]),
                )
            } else {
                (
                    "Your guild is currently Victorious. I can repair the gate if you wish.",
                    options(&[(3, "Repair"), (255, "No Thanks.")]),
                )
            }
        },
        GATE_KEEPER_ID => (
            "Are you going to leave here?",
            options(&[(1, "Yes."), (255, "No. Wait a moment")]),
        ),
        WARDEN_ID => {
            if !running() || pardon_time(server_time().minute()) {
                (
                    "It is time for Pardon! I can let you out if ou wish",
                    options(&[(1, "Let me out!"), (255, "No Thanks.")]),
                )
            } else {
                (
                    "It is not time for Pardon! There is a pardon every 30 minutes while Guild War is running.",
                    options(&[(255, "I will wait")]),
                )
            }
        },
        id if CONDUCTRESSES.iter().any(|(npc, _)| *npc == id) => (
            "Are you heading for the next teleporter? It is free for our members, and 1,000 silver for others.",
            options(&[(1, "Please teleport me there."), (255, "Just passing by.")]),
        ),
        _ => return None,
    };
    Some((text.to_owned(), options, 1))
}

/// The Warden pardons at minutes 0 and 30 while the war runs.
fn pardon_time(minute: u32) -> bool {
    minute == 0 || minute == 30
}

async fn say(actor: &Actor<ActorState>, npc_id: u32, text: &str, option: &str) -> Result<(), Error> {
    let dialog = MsgTaskDialog::builder()
        .text(text)
        .with_option(255, option)
        .and()
        .with_avatar(1)
        .task_id(npc_id)
        .build();
    actor.send_all(dialog).await?;
    Ok(())
}

pub async fn answer(
    state: &State,
    actor: &Actor<ActorState>,
    character: &Character,
    npc_id: u32,
    option: u8,
) -> Result<bool, Error> {
    match npc_id {
        LEFT_GATE_ID | RIGHT_GATE_ID => {
            let Some((_, leader)) = winner_member(state, character) else {
                return Ok(true);
            };
            match option {
                1 => {
                    let (x, y) = if npc_id == LEFT_GATE_ID { LEFT_GATE_INSIDE } else { RIGHT_GATE_INSIDE };
                    teleport(state, character, (GUILD_MAP, x, y)).await?;
                },
                2 | 3 if !leader => say(actor, npc_id, "Sorry, you are not able to perform this action.", "Thanks").await?,
                2 => {
                    {
                        let mut war = war().lock();
                        let open = war.open.entry(npc_id).or_insert(false);
                        *open = !*open;
                    }
                    refresh(state, npc_id).await;
                },
                3 => {
                    if life(npc_id) > 1 {
                        return Ok(true);
                    }
                    if character.spend_silver(state.pool(), GATE_REPAIR_COST).await?.is_none() {
                        say(actor, npc_id, "The repair costs 2,000,000 silver.", "I see.").await?;
                        return Ok(true);
                    }
                    actor.send(MsgUserInfo::from(character)).await?;
                    {
                        let mut war = war().lock();
                        war.life.insert(npc_id, GATE_LIFE);
                        war.open.insert(npc_id, false);
                    }
                    refresh(state, npc_id).await;
                },
                _ => {},
            }
        },
        GATE_KEEPER_ID => {
            if option == 1 {
                teleport(state, character, TWIN_CITY).await?;
            }
        },
        WARDEN_ID => {
            if option == 1 && (!running() || pardon_time(server_time().minute())) {
                teleport(state, character, TWIN_CITY).await?;
            }
        },
        id => {
            let Some((_, (map_id, x, y))) = CONDUCTRESSES.iter().find(|(npc, _)| *npc == id) else {
                return Ok(false);
            };
            if option != 1 {
                return Ok(true);
            }
            let member = winner_member(state, character).is_some();
            if !member && character.spend_silver(state.pool(), CONDUCTRESS_FEE).await?.is_none() {
                say(actor, id, "Sorry, you do not have enough.", "I see.").await?;
                return Ok(true);
            }
            actor.send(MsgUserInfo::from(character)).await?;
            teleport(state, character, (*map_id, *x, *y)).await?;
        },
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(day: u32, hour: u32) -> chrono::NaiveDateTime {
        chrono::NaiveDate::from_ymd_opt(2026, 10, day)
            .and_then(|date| date.and_hms_opt(hour, 30, 0))
            .expect("valid date")
    }

    #[test]
    fn the_war_runs_on_saturday_evening() {
        assert!(in_war_window(at(3, 20)));
        assert!(!in_war_window(at(3, 22)));
        assert!(!in_war_window(at(4, 20)));
    }

    #[test]
    fn the_best_score_wins_the_round() {
        let scores = HashMap::from([(1, ("Alpha".to_owned(), 10)), (2, ("Beta".to_owned(), 30))]);
        assert_eq!(best_guild(&scores), Some((2, "Beta".to_owned(), 30)));
        assert_eq!(best_guild(&HashMap::new()), None);
    }

    #[test]
    fn closed_gates_block_their_cells() {
        let gates = [(LEFT_GATE_ID, 163, 210)];
        assert!(blocks(GUILD_MAP, 163, 211, &gates));
        assert!(!blocks(GUILD_MAP, 170, 211, &gates));
        assert!(!blocks(1_002, 163, 211, &gates));
        assert!(pardon_time(30) && !pardon_time(15));
    }
}
