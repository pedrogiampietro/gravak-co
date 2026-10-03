use crate::bounty::BountyEntry;
use crate::{ActorState, Error, State};
use serde::{Deserialize, Serialize};
use tq_network::{Actor, PacketID};

/// The Bounty window's list (project packet, not part of 5065): a `CLEAR`
/// row, one `ENTRY` per wanted character, richest first, then `END`.
/// `CLAIMED` tells a player their kill collected a bounty.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PacketID)]
#[packet(id = 9010)]
pub struct MsgBounty {
    pub action: u32,
    pub target_id: u32,
    pub reward: u64,
    pub level: u32,
    pub online: u32,
    pub list_count: u8,
    pub name: String,
}

impl MsgBounty {
    pub const CLAIMED: u32 = 3;
    pub const CLEAR: u32 = 0;
    pub const END: u32 = 2;
    pub const ENTRY: u32 = 1;

    fn row(action: u32) -> Self {
        Self {
            action,
            list_count: 1,
            ..Default::default()
        }
    }

    pub fn entry(entry: BountyEntry, online: bool) -> Self {
        Self {
            action: Self::ENTRY,
            target_id: entry.target_id,
            reward: entry.reward,
            level: u32::from(entry.level),
            online: u32::from(online),
            list_count: 1,
            name: entry.name,
        }
    }

    pub fn claimed(target_id: u32, name: String, reward: u64) -> Self {
        Self {
            action: Self::CLAIMED,
            target_id,
            reward,
            list_count: 1,
            name,
            ..Default::default()
        }
    }
}

/// Sends the current list of bounties to `actor`.
pub async fn send_bounty_list(state: &State, actor: &Actor<ActorState>) -> Result<(), Error> {
    actor.send(MsgBounty::row(MsgBounty::CLEAR)).await?;
    let online: std::collections::HashSet<i32> = state
        .entities()
        .into_iter()
        .filter_map(|entity| entity.as_character().map(|character| character.database_id()))
        .collect();
    for entry in crate::bounty::list(state.pool(), crate::bounty::LIST_LIMIT).await? {
        let is_online = online.contains(&(entry.target_id as i32));
        actor.send(MsgBounty::entry(entry, is_online)).await?;
    }
    actor.send(MsgBounty::row(MsgBounty::END)).await?;
    Ok(())
}
