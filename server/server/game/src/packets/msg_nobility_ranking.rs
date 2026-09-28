use crate::nobility::NobilityEntry;
use crate::{ActorState, Error, State};
use serde::{Deserialize, Serialize};
use tq_network::{Actor, PacketID};

/// One row of the global nobility ranking.
///
/// The packet is intentionally separate from the local status packet so the
/// client can update the list incrementally without changing the 2064 wire
/// shape.
#[derive(Debug, Clone, Serialize, Deserialize, PacketID)]
#[packet(id = 2065)]
pub struct MsgNobilityRanking {
    pub action: u32,
    pub position: u32,
    pub character_id: u32,
    pub donation: u64,
    pub rank: u32,
    pub list_count: u8,
    pub character_name: String,
}

impl MsgNobilityRanking {
    pub const UPDATE: u32 = 1;

    pub fn from_entry(position: u32, entry: NobilityEntry) -> Self {
        Self {
            action: Self::UPDATE,
            position,
            character_id: entry.character_id,
            donation: entry.donation,
            rank: entry.rank as u32,
            list_count: 1,
            character_name: entry.name,
        }
    }
}

pub async fn send_nobility_ranking(state: &State, actor: &Actor<ActorState>) -> Result<(), Error> {
    for (position, entry) in crate::nobility::ranking(state.pool(), 10)
        .await?
        .into_iter()
        .enumerate()
    {
        actor
            .send(MsgNobilityRanking::from_entry(position as u32 + 1, entry))
            .await?;
    }
    Ok(())
}
