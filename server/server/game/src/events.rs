//! Player events, after Albetros (`Handlers/Events`): Team Deathmatch,
//! Freeze War, Infection, Capture the Flag and a Last Man Standing in place
//! of its unfinished Skill Tournament. An event opens for a minute of
//! signups (`$join`), splits the players into teams, fights on the event
//! arena (map 7002, a PK field: no PK points, no drops) and sends everyone
//! back where they were, the winners with half an ExpBall.

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use parking_lot::Mutex;

use crate::entities::{Character, StatusEffect};
use crate::packets::{MsgTalk, MsgUserInfo, TalkChannel};
use crate::{Error, State};

/// The event arena: a copy of the arena (1005) where teleports and teams
/// are off.
pub const EVENT_MAP: u32 = 7_002;
/// Team bases on the arena.
const BASES: [(u16, u16); 2] = [(38, 50), (64, 50)];
const SIGNUP: Duration = Duration::from_secs(60);
const RESPAWN: Duration = Duration::from_secs(5);
/// Automatic events, one every so often while players are online.
const AUTOMATIC_EVERY: Duration = Duration::from_secs(3 * 60 * 60);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// First team to 15 kills.
    TeamDeathmatch,
    /// Slain players freeze where they fall; a teammate standing beside them
    /// for three seconds frees them; a team all frozen loses.
    FreezeWar,
    /// One player starts infected; whoever they slay joins them; the last
    /// survivors win, or the infected when no one is left.
    Infection,
    /// Carry the other team's flag to your base; first to three.
    CaptureTheFlag,
    /// Everyone for themselves; the slain are out; the last one standing wins.
    LastManStanding,
}

impl Mode {
    pub fn parse(name: &str) -> Option<Self> {
        Some(match name.to_ascii_lowercase().as_str() {
            "tdm" | "deathmatch" => Self::TeamDeathmatch,
            "freeze" | "freezewar" => Self::FreezeWar,
            "infection" | "zombie" => Self::Infection,
            "ctf" | "flag" => Self::CaptureTheFlag,
            "lms" | "lastman" | "tournament" => Self::LastManStanding,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::TeamDeathmatch => "Team Deathmatch",
            Self::FreezeWar => "Freeze War",
            Self::Infection => "Infection",
            Self::CaptureTheFlag => "Capture the Flag",
            Self::LastManStanding => "Last Man Standing",
        }
    }

    fn length(self) -> Duration {
        Duration::from_secs(match self {
            Self::TeamDeathmatch | Self::CaptureTheFlag => 15 * 60,
            Self::FreezeWar | Self::Infection => 10 * 60,
            Self::LastManStanding => 12 * 60,
        })
    }

    /// Score that wins, for the scoring modes.
    fn score_limit(self) -> u32 {
        match self {
            Self::TeamDeathmatch => 15,
            Self::CaptureTheFlag => 3,
            _ => u32::MAX,
        }
    }

    const ALL: [Self; 5] = [
        Self::TeamDeathmatch,
        Self::FreezeWar,
        Self::Infection,
        Self::CaptureTheFlag,
        Self::LastManStanding,
    ];
}

const TEAM_NAMES: [&str; 2] = ["Red", "Blue"];

#[derive(Debug)]
enum Phase {
    Signup { until: Instant },
    Running { until: Instant },
}

#[derive(Debug)]
struct Event {
    mode: Mode,
    phase: Phase,
    /// Who signed up, then who plays: entity id -> team (0 red, 1 blue; in
    /// Infection 1 is the infected; in Last Man Standing everyone is 0).
    players: HashMap<u32, usize>,
    /// Where each player was before, to send them back.
    origins: HashMap<u32, (u32, u16, u16)>,
    scores: [u32; 2],
    /// Slain players waiting to stand up again, and since when.
    fallen: HashMap<u32, Instant>,
    /// Freeze War: the frozen, and how long a teammate has stood by them.
    frozen: HashMap<u32, Option<Instant>>,
    /// Last Man Standing: who is out.
    out: HashSet<u32>,
    /// Capture the Flag: who carries each team's flag.
    carriers: [Option<u32>; 2],
}

fn current() -> &'static Mutex<Option<Event>> {
    static EVENT: std::sync::OnceLock<Mutex<Option<Event>>> = std::sync::OnceLock::new();
    EVENT.get_or_init(Default::default)
}

fn next_automatic() -> &'static Mutex<Instant> {
    static NEXT: std::sync::OnceLock<Mutex<Instant>> = std::sync::OnceLock::new();
    NEXT.get_or_init(|| Mutex::new(Instant::now() + AUTOMATIC_EVERY))
}

/// Splits players (entity id, level), best first, into two balanced teams.
pub fn split(mut players: Vec<(u32, u16)>) -> HashMap<u32, usize> {
    players.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    players
        .into_iter()
        .enumerate()
        // 0, 1, 1, 0, 0, 1, 1, 0...: the strongest pairs balance out.
        .map(|(index, (id, _))| (id, [0, 1, 1, 0][index % 4]))
        .collect()
}

/// Whether two characters are on the same side of the running event.
pub fn allies(left: u32, right: u32) -> bool {
    let event = current().lock();
    let Some(event) = event.as_ref().filter(|event| matches!(event.phase, Phase::Running { .. })) else {
        return false;
    };
    if event.mode == Mode::LastManStanding {
        return false;
    }
    matches!((event.players.get(&left), event.players.get(&right)), (Some(a), Some(b)) if a == b)
}

/// Whether two characters fight each other in the running event (whatever
/// their kill modes).
pub fn enemies(left: u32, right: u32) -> bool {
    let event = current().lock();
    let Some(event) = event.as_ref().filter(|event| matches!(event.phase, Phase::Running { .. })) else {
        return false;
    };
    let (Some(a), Some(b)) = (event.players.get(&left), event.players.get(&right)) else {
        return false;
    };
    event.mode == Mode::LastManStanding || a != b
}

async fn to_everyone(state: &State, channel: TalkChannel, text: &str) {
    let talk = MsgTalk::from_system(0, channel, text);
    for entity in state.entities() {
        if let Some(character) = entity.as_character() {
            let _ = character.owner().send(talk.clone()).await;
        }
    }
}

async fn to_players(state: &State, ids: &[u32], text: &str) {
    for id in ids {
        if let Some(character) = state.entity(*id).as_ref().and_then(|entity| entity.as_character()) {
            let _ = character
                .owner()
                .send(MsgTalk::from_system(character.id(), TalkChannel::Center, text))
                .await;
        }
    }
}

/// Opens `mode` for signups; what to tell whoever asked.
pub async fn open(state: &State, mode: Mode) -> String {
    {
        let mut event = current().lock();
        if event.is_some() {
            return "An event is already under way.".to_owned();
        }
        *event = Some(Event {
            mode,
            phase: Phase::Signup { until: Instant::now() + SIGNUP },
            players: HashMap::new(),
            origins: HashMap::new(),
            scores: [0; 2],
            fallen: HashMap::new(),
            frozen: HashMap::new(),
            out: HashSet::new(),
            carriers: [None; 2],
        });
    }
    *next_automatic().lock() = Instant::now() + AUTOMATIC_EVERY;
    to_everyone(
        state,
        TalkChannel::Center,
        &format!("{} starts in one minute! Type $join to take part.", mode.name()),
    )
    .await;
    format!("{} is open for signups.", mode.name())
}

/// Signs a player up; what to tell them.
pub fn join(character: &Character) -> String {
    let mut event = current().lock();
    let Some(event) = event.as_mut() else {
        return "No event is open. Watch for the next one!".to_owned();
    };
    if !matches!(event.phase, Phase::Signup { .. }) {
        return "This event has already begun.".to_owned();
    }
    if character.entity().is_dead() {
        return "The dead cannot join.".to_owned();
    }
    event.players.insert(character.id(), 0);
    format!("You joined {}. It starts soon.", event.mode.name())
}

/// Takes a player out of the signups.
pub fn leave(character: &Character) -> String {
    let mut event = current().lock();
    match event.as_mut() {
        Some(event) if matches!(event.phase, Phase::Signup { .. }) => {
            event.players.remove(&character.id());
            "You left the signups.".to_owned()
        },
        _ => "You are not waiting for an event.".to_owned(),
    }
}

/// The nearest cell of the arena someone can stand on.
fn standable(state: &State, (x, y): (u16, u16)) -> (u16, u16) {
    state
        .try_map(EVENT_MAP)
        .ok()
        .and_then(|map| map.landing_position(primitives::Location::new(x, y, 0), 8))
        .map_or((x, y), |spot| (spot.x, spot.y))
}

fn base(team: usize) -> (u16, u16) {
    BASES[team.min(1)]
}

/// The event moves on: starts, revives, freezes and frees, carries flags,
/// and ends.
pub async fn tick(state: &State) {
    let online = state.entities().iter().any(|entity| entity.as_character().is_some());
    let automatic_due = online && current().lock().is_none() && Instant::now() >= *next_automatic().lock();
    if automatic_due {
        let mode = Mode::ALL[crate::combat::next_roll() as usize % Mode::ALL.len()];
        open(state, mode).await;
        return;
    }
    let starting = matches!(
        current().lock().as_ref().map(|event| &event.phase),
        Some(Phase::Signup { until }) if Instant::now() >= *until
    );
    if starting {
        if let Err(error) = start(state).await {
            tracing::warn!(%error, "The event could not start");
            *current().lock() = None;
        }
        return;
    }
    if matches!(current().lock().as_ref().map(|event| &event.phase), Some(Phase::Running { .. })) {
        if let Err(error) = run(state).await {
            tracing::warn!(%error, "The event stumbled");
        }
    }
}

fn character_of(state: &State, id: u32) -> Option<std::sync::Arc<crate::entities::GameEntity>> {
    state.entity(id).filter(|entity| entity.as_character().is_some())
}

async fn start(state: &State) -> Result<(), Error> {
    let (mode, signed): (Mode, Vec<u32>) = {
        let event = current().lock();
        let Some(event) = event.as_ref() else {
            return Ok(());
        };
        (event.mode, event.players.keys().copied().collect())
    };
    let present: Vec<(u32, u16)> = signed
        .iter()
        .filter_map(|id| {
            let entity = character_of(state, *id)?;
            let character = entity.as_character()?;
            (!character.entity().is_dead()).then(|| (*id, character.entity().level()))
        })
        .collect();
    if present.len() < 2 {
        *current().lock() = None;
        to_everyone(state, TalkChannel::Center, &format!("{} needs at least two players; maybe next time!", mode.name())).await;
        return Ok(());
    }
    let mut teams = split(present.clone());
    match mode {
        Mode::LastManStanding => teams.values_mut().for_each(|team| *team = 0),
        Mode::Infection => {
            // One starts infected (team 1); everyone else survives (team 0).
            let first = present[crate::combat::next_roll() as usize % present.len()].0;
            teams.values_mut().for_each(|team| *team = 0);
            teams.insert(first, 1);
        },
        _ => {},
    }
    let mut origins = HashMap::new();
    for (id, team) in &teams {
        let Some(entity) = character_of(state, *id) else {
            continue;
        };
        let Some(character) = entity.as_character() else {
            continue;
        };
        let location = character.entity().location();
        origins.insert(*id, (character.entity().map_id(), location.x, location.y));
        let (x, y) = if mode == Mode::LastManStanding {
            // Spread around the arena.
            let spot = (*id % 9) as u16;
            (40 + (spot % 3) * 10, 40 + (spot / 3) * 10)
        } else {
            base(*team)
        };
        crate::world::relocate(state, character, EVENT_MAP, standable(state, (x, y))).await?;
        let side = match mode {
            Mode::LastManStanding => "Everyone for themselves!".to_owned(),
            Mode::Infection if *team == 1 => "You are INFECTED: spread it!".to_owned(),
            Mode::Infection => "You are a survivor: stay alive!".to_owned(),
            _ => format!("You fight for the {} team.", TEAM_NAMES[*team]),
        };
        let _ = character
            .owner()
            .send(MsgTalk::from_system(character.id(), TalkChannel::Center, format!("{}: {side}", mode.name())))
            .await;
    }
    {
        let mut event = current().lock();
        if let Some(event) = event.as_mut() {
            event.players = teams;
            event.origins = origins;
            event.phase = Phase::Running { until: Instant::now() + mode.length() };
        }
    }
    to_everyone(state, TalkChannel::Center, &format!("{} has begun with {} players!", mode.name(), present.len())).await;
    Ok(())
}

/// A participant was slain by another: scores, freezes, infections.
pub async fn on_kill(state: &State, victim: &Character, killer: Option<&Character>) -> Result<(), Error> {
    let lines: Vec<String> = {
        let mut event = current().lock();
        let Some(event) = event.as_mut().filter(|event| matches!(event.phase, Phase::Running { .. })) else {
            return Ok(());
        };
        let Some(&victim_team) = event.players.get(&victim.id()) else {
            return Ok(());
        };
        let killer_team = killer.and_then(|killer| event.players.get(&killer.id()).copied());
        let mut lines = Vec::new();
        // A carried flag goes home.
        for (flag, carrier) in event.carriers.iter_mut().enumerate() {
            if *carrier == Some(victim.id()) {
                *carrier = None;
                lines.push(format!("The {} flag returns home!", TEAM_NAMES[flag]));
            }
        }
        match event.mode {
            Mode::TeamDeathmatch => {
                if let Some(team) = killer_team.filter(|team| *team != victim_team) {
                    event.scores[team] += 1;
                    lines.push(format!("Red {} - {} Blue", event.scores[0], event.scores[1]));
                }
                event.fallen.insert(victim.id(), Instant::now());
            },
            Mode::FreezeWar => {
                event.frozen.insert(victim.id(), None);
                event.fallen.insert(victim.id(), Instant::now());
                lines.push(format!("{} is frozen!", victim.entity().name()));
            },
            Mode::Infection => {
                if victim_team == 0 && killer_team == Some(1) {
                    event.players.insert(victim.id(), 1);
                    lines.push(format!("{} has been infected!", victim.entity().name()));
                }
                event.fallen.insert(victim.id(), Instant::now());
            },
            Mode::CaptureTheFlag => {
                event.fallen.insert(victim.id(), Instant::now());
            },
            Mode::LastManStanding => {
                event.out.insert(victim.id());
                event.fallen.insert(victim.id(), Instant::now());
                let left = event.players.len() - event.out.len();
                lines.push(format!("{} is out! {left} left standing.", victim.entity().name()));
            },
        }
        lines
    };
    let ids: Vec<u32> = current().lock().as_ref().map(|event| event.players.keys().copied().collect()).unwrap_or_default();
    for line in lines {
        to_players(state, &ids, &line).await;
    }
    Ok(())
}

fn distance(a: (u16, u16), b: (u16, u16)) -> u16 {
    a.0.abs_diff(b.0).max(a.1.abs_diff(b.1))
}

/// One step of the running event.
async fn run(state: &State) -> Result<(), Error> {
    let now = Instant::now();
    // What to do, decided under the lock; done after it.
    let mut revive: Vec<(u32, (u16, u16))> = Vec::new();
    let mut lines: Vec<String> = Vec::new();
    let mut winner: Option<Option<String>> = None;
    let ids: Vec<u32>;
    {
        let mut guard = current().lock();
        let Some(event) = guard.as_mut() else {
            return Ok(());
        };
        let Phase::Running { until } = event.phase else {
            return Ok(());
        };
        // Who is still here.
        event
            .players
            .retain(|id, _| state.entity(*id).is_some_and(|entity| entity.as_character().is_some_and(|c| c.entity().map_id() == EVENT_MAP)));
        ids = event.players.keys().copied().collect();
        let position = |id: u32| {
            state
                .entity(id)
                .map(|entity| entity.basic().location())
                .map(|location| (location.x, location.y))
        };
        let alive = |id: u32| state.entity(id).is_some_and(|entity| !entity.basic().is_dead());
        // The slain stand up again at their base.
        let due: Vec<u32> = event
            .fallen
            .iter()
            .filter(|(_, since)| now.duration_since(**since) >= RESPAWN)
            .map(|(id, _)| *id)
            .collect();
        for id in due {
            event.fallen.remove(&id);
            if event.out.contains(&id) {
                continue;
            }
            let team = event.players.get(&id).copied().unwrap_or(0);
            if event.mode == Mode::FreezeWar {
                // Frozen where they fell.
                if let Some(here) = position(id) {
                    revive.push((id, here));
                }
            } else {
                revive.push((id, base(team)));
            }
        }
        match event.mode {
            Mode::FreezeWar => {
                // A teammate beside a frozen player for three seconds frees them.
                let frozen: Vec<u32> = event.frozen.keys().copied().collect();
                for id in frozen {
                    let team = event.players.get(&id).copied();
                    let here = position(id);
                    let helped = event.players.iter().any(|(other, other_team)| {
                        *other != id
                            && Some(*other_team) == team
                            && !event.frozen.contains_key(other)
                            && alive(*other)
                            && matches!((here, position(*other)), (Some(a), Some(b)) if distance(a, b) <= 1)
                    });
                    let since = event.frozen.get_mut(&id).expect("frozen");
                    match (helped, *since) {
                        (true, None) => *since = Some(now),
                        (true, Some(started)) if now.duration_since(started) >= Duration::from_secs(3) => {
                            event.frozen.remove(&id);
                            lines.push("A teammate has been freed!".to_owned());
                        },
                        (false, _) => *since = None,
                        _ => {},
                    }
                }
                for team in 0..2 {
                    let members: Vec<u32> = event.players.iter().filter(|(_, t)| **t == team).map(|(id, _)| *id).collect();
                    if !members.is_empty() && members.iter().all(|id| event.frozen.contains_key(id)) {
                        winner = Some(Some(format!("the {} team", TEAM_NAMES[1 - team])));
                    }
                }
            },
            Mode::Infection => {
                let survivors = event.players.values().filter(|team| **team == 0).count();
                if survivors == 0 {
                    winner = Some(Some("the infected".to_owned()));
                } else if !event.players.values().any(|team| *team == 1) {
                    winner = Some(Some("the survivors".to_owned()));
                }
            },
            Mode::CaptureTheFlag => {
                for (id, team) in event.players.clone() {
                    let Some(here) = position(id).filter(|_| alive(id)) else {
                        continue;
                    };
                    let enemy = 1 - team;
                    // Pick up the enemy flag at their base.
                    if event.carriers[enemy].is_none() && distance(here, base(enemy)) <= 1 {
                        event.carriers[enemy] = Some(id);
                        lines.push(format!("The {} flag has been taken!", TEAM_NAMES[enemy]));
                    }
                    // Bring it home, with your own flag at home, to score.
                    if event.carriers[enemy] == Some(id) && event.carriers[team].is_none() && distance(here, base(team)) <= 1 {
                        event.carriers[enemy] = None;
                        event.scores[team] += 1;
                        lines.push(format!(
                            "The {} team captures! Red {} - {} Blue",
                            TEAM_NAMES[team], event.scores[0], event.scores[1]
                        ));
                    }
                }
            },
            Mode::LastManStanding => {
                let standing: Vec<u32> = event.players.keys().filter(|id| !event.out.contains(id)).copied().collect();
                if standing.len() <= 1 {
                    let name = standing
                        .first()
                        .and_then(|id| state.entity(*id))
                        .map(|entity| entity.basic().name().to_owned());
                    winner = Some(name);
                }
            },
            Mode::TeamDeathmatch => {},
        }
        let limit = event.mode.score_limit();
        if let Some(team) = (0..2).find(|team| event.scores[*team] >= limit) {
            winner = Some(Some(format!("the {} team", TEAM_NAMES[team])));
        }
        if winner.is_none() && (now >= until || event.players.len() < 2) {
            winner = Some(match event.scores[0].cmp(&event.scores[1]) {
                std::cmp::Ordering::Greater => Some("the Red team".to_owned()),
                std::cmp::Ordering::Less => Some("the Blue team".to_owned()),
                std::cmp::Ordering::Equal => None,
            });
        }
    }
    for (id, (x, y)) in revive {
        if let Some(entity) = character_of(state, id) {
            if let Some(character) = entity.as_character() {
                if character.entity().is_dead() {
                    crate::world::relocate(state, character, EVENT_MAP, standable(state, (x, y))).await?;
                    crate::death::revive_here(character).await?;
                }
                let frozen = current().lock().as_ref().is_some_and(|event| event.frozen.contains_key(&id));
                if frozen {
                    character.apply_status(StatusEffect::Stunned, Duration::from_secs(600));
                }
            }
        }
    }
    // Freed players move again.
    let frozen_now: HashSet<u32> = current().lock().as_ref().map(|event| event.frozen.keys().copied().collect()).unwrap_or_default();
    for id in &ids {
        if let Some(entity) = character_of(state, *id) {
            if let Some(character) = entity.as_character() {
                if !frozen_now.contains(id) && character.has_status(StatusEffect::Stunned) {
                    let mode = current().lock().as_ref().map(|event| event.mode);
                    if mode == Some(Mode::FreezeWar) {
                        character.apply_status(StatusEffect::Stunned, Duration::ZERO);
                    }
                }
            }
        }
    }
    for line in lines {
        to_players(state, &ids, &line).await;
    }
    if let Some(winner) = winner {
        finish(state, winner).await?;
    }
    Ok(())
}

/// Ends the event: the winners are named and rewarded, everyone goes back.
async fn finish(state: &State, winner: Option<String>) -> Result<(), Error> {
    let Some(event) = current().lock().take() else {
        return Ok(());
    };
    let winning_team = match event.mode {
        Mode::LastManStanding => None,
        Mode::Infection => winner.as_deref().map(|name| usize::from(name == "the infected")),
        _ => winner
            .as_deref()
            .and_then(|name| TEAM_NAMES.iter().position(|team| name.contains(team))),
    };
    for (id, team) in &event.players {
        let Some(entity) = character_of(state, *id) else {
            continue;
        };
        let Some(character) = entity.as_character() else {
            continue;
        };
        character.apply_status(StatusEffect::Stunned, Duration::ZERO);
        if character.entity().is_dead() {
            crate::death::revive_here(character).await?;
        }
        let won = match event.mode {
            Mode::LastManStanding => winner.as_deref() == Some(character.entity().name()),
            _ => winning_team == Some(*team),
        };
        if won {
            if let Some(level) = character.grant_exp_ball(crate::leveling::EXP_BALL_SECONDS / 2.0) {
                crate::leveling::after_level_up(state, character, level).await?;
            }
            let _ = character.owner().send(MsgUserInfo::from(character)).await;
        }
        if let Some((map, x, y)) = event.origins.get(id) {
            crate::world::relocate(state, character, *map, (*x, *y)).await?;
        }
        character.save(state).await?;
    }
    let line = match winner {
        Some(name) => format!("{} is over: {name} won! Well fought, everyone.", event.mode.name()),
        None => format!("{} is over with no winner.", event.mode.name()),
    };
    to_everyone(state, TalkChannel::Center, &line).await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn teams_balance_the_strongest() {
        let teams = split(vec![(1, 100), (2, 90), (3, 80), (4, 70), (5, 60)]);
        assert_eq!(teams[&1], 0);
        assert_eq!(teams[&2], 1);
        assert_eq!(teams[&3], 1);
        assert_eq!(teams[&4], 0);
        assert_eq!(teams[&5], 0);
    }

    #[test]
    fn modes_have_short_names() {
        assert_eq!(Mode::parse("TDM"), Some(Mode::TeamDeathmatch));
        assert_eq!(Mode::parse("ctf"), Some(Mode::CaptureTheFlag));
        assert_eq!(Mode::parse("tournament"), Some(Mode::LastManStanding));
        assert_eq!(Mode::parse("poker"), None);
    }
}
