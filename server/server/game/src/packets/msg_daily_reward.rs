//! The Reward Shrine (Gravak packets, not in the 5065 client):
//! `MsgDailyReward` carries the client's requests, `MsgDailyRewardChoice`
//! one choice of the cycle and `MsgDailyRewardState` where the character
//! stands in it.

use crate::daily_rewards::{self, Standing};
use crate::{ActorState, Error, State};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tq_network::{Actor, PacketID, PacketProcess};

use super::{MsgItemInfo, MsgTalk, TalkChannel};

/// Client: claim choice `param` of today's. Any other action (0, a click
/// on the shrine) opens the Reward Wall.
pub const CLAIM: u32 = 1;

#[derive(Debug, Default, Clone, Serialize, Deserialize, PacketID)]
#[packet(id = 9130)]
pub struct MsgDailyReward {
    pub action: u32,
    pub param: u32,
}

/// One choice of the cycle, `index` of `total` (index 0 starts a new list).
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, PacketID)]
#[packet(id = 9131)]
pub struct MsgDailyRewardChoice {
    pub index: u32,
    pub total: u32,
    pub day: u32,
    pub item_type: u32,
    pub amount: u32,
}

/// Where the character stands: the streak, today's day of the cycle,
/// whether it was claimed, and when the next day starts (seconds since 1970).
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, PacketID)]
#[packet(id = 9132)]
pub struct MsgDailyRewardState {
    pub streak: u32,
    pub day: u32,
    pub claimed: u32,
    pub next_day_at: u64,
}

/// Sends the cycle and the character's standing: the client opens the
/// Reward Wall on it.
pub async fn send_wall(state: &State, character: &crate::entities::Character) -> Result<(), Error> {
    let owner = character.owner();
    let rewards = daily_rewards::rewards();
    for (index, reward) in rewards.iter().enumerate() {
        owner
            .send(MsgDailyRewardChoice {
                index: u32::try_from(index).unwrap_or_default(),
                total: u32::try_from(rewards.len()).unwrap_or_default(),
                day: reward.day,
                item_type: reward.item_type,
                amount: reward.amount,
            })
            .await?;
    }
    let today = daily_rewards::today();
    let (streak, last_day) = daily_rewards::load(state.pool(), character.database_id()).await?;
    let Standing { streak, day, claimed_today } = daily_rewards::standing(streak, last_day, today);
    owner
        .send(MsgDailyRewardState {
            streak,
            day,
            claimed: u32::from(claimed_today),
            next_day_at: daily_rewards::next_day_at(today),
        })
        .await?;
    Ok(())
}

#[async_trait]
impl PacketProcess for MsgDailyReward {
    type ActorState = ActorState;
    type Error = Error;
    type State = State;

    async fn process(&self, state: &Self::State, actor: &Actor<Self::ActorState>) -> Result<(), Self::Error> {
        let entity = actor.entity();
        let character = entity.as_character().ok_or(Error::CharacterNotFound)?;
        let location = character.entity().location();
        if !daily_rewards::near_shrine(character.entity().map_id(), (location.x, location.y)) {
            actor
                .send(MsgTalk::from_system(
                    character.id(),
                    TalkChannel::TopLeft,
                    "Come closer to the Gravak statue to see your daily rewards.",
                ))
                .await?;
            return Ok(());
        }
        if self.action == CLAIM {
            let line = match daily_rewards::claim(state, character, self.param).await? {
                Ok(claim) => {
                    for item in &claim.items {
                        actor.send(MsgItemInfo::inventory_item(*item)).await?;
                    }
                    format!(
                        "You claimed the reward of day {}. {} day(s) in a row!",
                        claim.reward.day, claim.streak
                    )
                },
                Err(refusal) => refusal.message().to_owned(),
            };
            actor
                .send(MsgTalk::from_system(character.id(), TalkChannel::TopLeft, line))
                .await?;
        }
        send_wall(state, character).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tq_network::{PacketDecode, PacketEncode};

    #[test]
    fn the_wall_survives_the_wire() {
        let state = MsgDailyRewardState { streak: 3, day: 4, claimed: 1, next_day_at: 1_791_244_800 };
        let (_, bytes) = state.clone().encode().expect("encodes");
        assert_eq!(MsgDailyRewardState::decode(&bytes).expect("decodes"), state);
        let choice = MsgDailyRewardChoice { index: 1, total: 12, day: 1, item_type: 721_331, amount: 3 };
        let (_, bytes) = choice.clone().encode().expect("encodes");
        assert_eq!(MsgDailyRewardChoice::decode(&bytes).expect("decodes"), choice);
    }
}
