//! Gravak's Guild System window: the client's requests (9170), the guild's
//! header (9171) and the rows of its lists (9172). A list is sent as a
//! header row (`index` 0, its `total` for the pages and, for the deaths,
//! its summary) and then its rows from `index` 1. Strings hold 255 bytes.

use serde::{Deserialize, Serialize};
use tq_network::{Actor, PacketID, PacketProcess};

use crate::{ActorState, Error, State};

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, PacketID)]
#[packet(id = 9170)]
pub struct MsgGuildPanel {
    pub action: u32,
    pub param: u32,
    pub amount: u32,
    pub text1: String,
    pub text2: String,
    pub text3: String,
    pub text4: String,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, PacketID)]
#[packet(id = 9171)]
pub struct MsgGuildPanelInfo {
    /// 0 no guild, 1 a member.
    pub in_guild: u32,
    pub guild_id: u32,
    /// 1000 leader, 990 deputy, 200 member.
    pub role: u32,
    pub level: u32,
    /// Experience into the level, and what the level needs in all.
    pub experience: u64,
    pub next_experience: u64,
    pub members: u32,
    pub max_members: u32,
    pub online: u32,
    pub fund: u64,
    pub entry_fee: u32,
    pub accepting: u32,
    pub created_at: u64,
    /// Guilds on the server, and how many take requests.
    pub guilds: u32,
    pub guilds_accepting: u32,
    /// The hero's own silver and level, for what the window offers.
    pub silver: u64,
    pub hero_level: u32,
    /// Join requests waiting (for the tab's count).
    pub pending_requests: u32,
    /// The crest: emblem, field colour << 8, emblem colour << 16.
    pub emblem: u32,
    pub name: String,
    pub title: String,
    pub motto: String,
    pub description: String,
    pub leader: String,
    /// What the last request did, or why not.
    pub note: String,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, PacketID)]
#[packet(id = 9172)]
pub struct MsgGuildPanelRow {
    pub list: u32,
    pub index: u32,
    pub total: u32,
    pub id: u32,
    pub level: u32,
    pub value: u32,
    pub extra: u32,
    pub at: u64,
    pub text1: String,
    pub text2: String,
    pub text3: String,
    pub text4: String,
}

/// The hero's guild and the guilds it is at war with or allied to (ids,
/// joined by `,`), for the shields over the players' names.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, PacketID)]
#[packet(id = 9173)]
pub struct MsgGuildRelations {
    pub guild_id: u32,
    pub enemies: String,
    pub allies: String,
}

#[async_trait::async_trait]
impl PacketProcess for MsgGuildPanel {
    type ActorState = ActorState;
    type Error = Error;
    type State = State;

    async fn process(&self, state: &Self::State, actor: &Actor<Self::ActorState>) -> Result<(), Self::Error> {
        let entity = actor.entity();
        let character = entity.as_character().ok_or(Error::CharacterNotFound)?;
        crate::guild_panel::handle(state, actor, character, self).await
    }
}
