use crate::nobility::NobilityEntry;
use crate::{ActorState, Error, State};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tq_network::{Actor, PacketID, PacketProcess};

/// The nobility ranking: the client asks for it (`action = 2`), and the
/// server answers with a clear (`action = 3`) and one row (`action = 1`)
/// per seat held, up to the 50 seats.
#[derive(Debug, Default, Clone, Serialize, Deserialize, PacketID)]
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
    pub const REQUEST: u32 = 2;
    pub const CLEAR: u32 = 3;

    pub fn from_entry(position: u32, count: u8, entry: NobilityEntry) -> Self {
        Self {
            action: Self::UPDATE,
            position,
            character_id: entry.character_id,
            donation: entry.donation,
            rank: entry.rank as u32,
            list_count: count,
            character_name: entry.name,
        }
    }
}

#[async_trait]
impl PacketProcess for MsgNobilityRanking {
    type ActorState = ActorState;
    type Error = Error;
    type State = State;

    async fn process(&self, state: &Self::State, actor: &Actor<Self::ActorState>) -> Result<(), Self::Error> {
        if self.action == Self::REQUEST {
            send_nobility_ranking(state, actor).await?;
        }
        Ok(())
    }
}

/// The whole ranking (every seat), from the board.
pub async fn send_nobility_ranking(_state: &State, actor: &Actor<ActorState>) -> Result<(), Error> {
    let entries = crate::nobility::board().top(crate::nobility::SEATS);
    let count = entries.len() as u8;
    actor
        .send(MsgNobilityRanking { action: MsgNobilityRanking::CLEAR, list_count: count, ..Default::default() })
        .await?;
    for (index, entry) in entries.into_iter().enumerate() {
        actor
            .send(MsgNobilityRanking::from_entry(index as u32 + 1, count, entry))
            .await?;
    }
    Ok(())
}
