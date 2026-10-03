//! The Task Board of bounty tasks (Gravak packets, not in the 5065 client):
//! `MsgBounty` carries the board's actions and the kill counter, and
//! `MsgBountyState` the whole board.

use crate::bounty::{Board, Bonus, Difficulty, Refusal, Task};
use crate::{ActorState, Error, State};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tq_network::{Actor, PacketID, PacketProcess};

use super::{MsgTalk, TalkChannel};

/// Client: send the board.
pub const OPEN: u32 = 0;
/// Client: a new list of difficulty `param` for a token.
pub const REROLL: u32 = 1;
/// Client: start offer `param` (0..3).
pub const PICK: u32 = 2;
/// Client: take the day's free token.
pub const CLAIM_DAILY: u32 = 3;
/// Client: raise talisman bonus `param` (damage, leech, loot, double kills).
pub const UPGRADE: u32 = 4;
/// Server: the task counted kills; `param` done of `extra`.
pub const PROGRESS: u32 = 10;

#[derive(Debug, Default, Clone, Serialize, Deserialize, PacketID)]
#[packet(id = 9100)]
pub struct MsgBounty {
    pub action: u32,
    pub param: u32,
    pub extra: u32,
}

/// One offer, or the task under way (`monster_type` 0 when there is none).
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BountyTaskWire {
    pub monster_type: u32,
    pub kills: u32,
    pub kills_done: u32,
    pub grade: u32,
    pub points: u32,
    pub experience: u64,
    pub monster_name: String,
}

impl From<&Task> for BountyTaskWire {
    fn from(task: &Task) -> Self {
        Self {
            monster_type: task.monster_type,
            kills: task.kills,
            kills_done: task.kills_done,
            grade: task.grade as u32,
            points: task.points,
            experience: task.experience,
            monster_name: task.monster_name.clone(),
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, PacketID)]
#[packet(id = 9101)]
pub struct MsgBountyState {
    pub points: u32,
    pub tokens: u32,
    pub daily_available: u32,
    pub difficulty: u32,
    pub completed: u32,
    pub damage_level: u32,
    pub leech_level: u32,
    pub loot_level: u32,
    pub double_kill_level: u32,
    pub offer_count: u32,
    pub offer0: BountyTaskWire,
    pub offer1: BountyTaskWire,
    pub offer2: BountyTaskWire,
    pub task: BountyTaskWire,
}

impl From<&Board> for MsgBountyState {
    fn from(board: &Board) -> Self {
        let offer = |slot: usize| board.offers.get(slot).map(BountyTaskWire::from).unwrap_or_default();
        Self {
            points: board.points,
            tokens: board.tokens,
            daily_available: u32::from(board.daily_available),
            difficulty: board.difficulty as u32,
            completed: board.completed,
            damage_level: board.talisman[0],
            leech_level: board.talisman[1],
            loot_level: board.talisman[2],
            double_kill_level: board.talisman[3],
            offer_count: u32::try_from(board.offers.len()).unwrap_or_default(),
            offer0: offer(0),
            offer1: offer(1),
            offer2: offer(2),
            task: board.task.as_ref().map(BountyTaskWire::from).unwrap_or_default(),
        }
    }
}

/// Sends a character its board and keeps its talisman for combat.
pub async fn send_board(state: &State, character: &crate::entities::Character) -> Result<(), Error> {
    let board = crate::bounty::board(state.pool(), character.database_id()).await?;
    state.talismans().set(character.id(), board.talisman());
    character.owner().send(MsgBountyState::from(&board)).await?;
    Ok(())
}

/// Counts a kill of a monster toward a character's bounty, twice when the
/// talisman says so, and pays a finished one; the level reached, if any.
pub async fn count_kill(
    state: &State,
    character: &crate::entities::Character,
    monster_type: u32,
    monster_name: &str,
) -> Result<Option<u8>, Error> {
    let Some(talisman) = state.talismans().get(character.id()) else {
        return Ok(None);
    };
    if !talisman.hunts(monster_name) {
        return Ok(None);
    }
    let kills = if talisman.double_kill(monster_name, crate::combat::next_roll()) { 2 } else { 1 };
    let progress = crate::bounty::record_kills(state.pool(), character.database_id(), monster_type, kills).await?;
    settle(state, character, progress).await
}

/// Counts `kills` toward a character's bounty without a monster (a game
/// master's test); the level reached, if any.
pub async fn add_kills(
    state: &State,
    character: &crate::entities::Character,
    kills: u32,
) -> Result<Result<Option<u8>, &'static str>, Error> {
    let board = crate::bounty::board(state.pool(), character.database_id()).await?;
    let Some(task) = board.task else {
        return Ok(Err("No bounty under way."));
    };
    let progress =
        crate::bounty::record_kills(state.pool(), character.database_id(), task.monster_type, kills).await?;
    Ok(Ok(settle(state, character, progress).await?))
}

/// Tells a character what kills did to its bounty, and pays a finished one.
async fn settle(
    state: &State,
    character: &crate::entities::Character,
    progress: crate::bounty::Progress,
) -> Result<Option<u8>, Error> {
    match progress {
        crate::bounty::Progress::None => Ok(None),
        crate::bounty::Progress::Counted { done, required } => {
            character
                .owner()
                .send(MsgBounty {
                    action: PROGRESS,
                    param: done,
                    extra: required,
                })
                .await?;
            Ok(None)
        },
        crate::bounty::Progress::Completed(reward) => {
            let leveled = character.grant_experience(reward.experience);
            tracing::info!(
                target: "world",
                character = %character.entity().name(),
                monster = %reward.monster_name,
                points = reward.points,
                "Bounty completed"
            );
            character
                .owner()
                .send(MsgTalk::from_system(
                    character.id(),
                    TalkChannel::Center,
                    format!(
                        "Bounty complete: {}! {} experience, {} bounty points and a reroll token.",
                        reward.monster_name, reward.experience, reward.points
                    ),
                ))
                .await?;
            send_board(state, character).await?;
            Ok(leveled)
        },
    }
}

#[async_trait]
impl PacketProcess for MsgBounty {
    type ActorState = ActorState;
    type Error = Error;
    type State = State;

    async fn process(&self, state: &Self::State, actor: &Actor<Self::ActorState>) -> Result<(), Self::Error> {
        let entity = actor.entity();
        let character = entity.as_character().ok_or(Error::CharacterNotFound)?;
        let id = character.database_id();
        let pool = state.pool();
        let outcome: Result<Option<String>, Refusal> = match self.action {
            OPEN => Ok(None),
            REROLL => match Difficulty::from_u32(self.param) {
                Some(difficulty) => crate::bounty::reroll(pool, id, difficulty)
                    .await?
                    .map(|()| Some(format!("A new list of {difficulty:?} bounties."))),
                None => Ok(None),
            },
            PICK => crate::bounty::pick(pool, id, self.param)
                .await?
                .map(|()| Some("Bounty accepted. Good hunting!".to_owned())),
            CLAIM_DAILY => crate::bounty::claim_daily(pool, id)
                .await?
                .map(|()| Some("You took today's reroll token.".to_owned())),
            UPGRADE => match Bonus::from_u32(self.param) {
                Some(bonus) => crate::bounty::upgrade(pool, id, bonus)
                    .await?
                    .map(|level| Some(format!("Talisman {bonus:?} is now level {level}."))),
                None => Ok(None),
            },
            _ => Ok(None),
        };
        let line = match outcome {
            Ok(line) => line,
            Err(refusal) => Some(refusal.message()),
        };
        if let Some(line) = line {
            actor
                .send(MsgTalk::from_system(character.id(), TalkChannel::TopLeft, line))
                .await?;
        }
        send_board(state, character).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tq_network::{PacketDecode, PacketEncode};

    #[test]
    fn the_board_survives_the_wire() {
        let state = MsgBountyState {
            points: 42,
            tokens: 7,
            daily_available: 1,
            difficulty: 2,
            offer_count: 1,
            offer0: BountyTaskWire {
                monster_type: 3,
                kills: 150,
                grade: 1,
                points: 11,
                experience: 90_000,
                monster_name: "Robin".to_owned(),
                ..Default::default()
            },
            task: BountyTaskWire {
                monster_type: 4,
                kills: 200,
                kills_done: 37,
                monster_name: "Apparition".to_owned(),
                ..Default::default()
            },
            ..Default::default()
        };
        let (_, bytes) = state.clone().encode().expect("encodes");
        assert_eq!(MsgBountyState::decode(&bytes).expect("decodes"), state);
    }
}
