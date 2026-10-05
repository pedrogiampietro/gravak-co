//! `MsgBroadcast` (2050): a player pays to send a message to the server.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tq_network::{Actor, PacketID, PacketProcess};
use tq_serde::StringList;

use super::{MsgTalk, MsgUserInfo, TalkChannel};
use crate::{ActorState, Error, State};

/// Client: broadcast the first string.
pub const SEND: u32 = 3;

#[derive(Debug, Clone, Serialize, Deserialize, PacketID)]
#[packet(id = 2050)]
pub struct MsgBroadcast {
    pub subtype: u32,
    pub unknown: u32,
    pub strings: StringList,
}

#[async_trait]
impl PacketProcess for MsgBroadcast {
    type ActorState = ActorState;
    type Error = Error;
    type State = State;

    async fn process(&self, state: &Self::State, actor: &Actor<Self::ActorState>) -> Result<(), Self::Error> {
        if self.subtype != SEND {
            return Ok(());
        }
        let entity = actor.entity();
        let character = entity.as_character().ok_or(Error::CharacterNotFound)?;
        let message = Vec::<String>::from(self.strings.clone()).into_iter().next().unwrap_or_default();
        let line = crate::broadcast::submit(state, character, &message).await?;
        actor.send(MsgUserInfo::from(character)).await?;
        actor
            .send(MsgTalk::from_system(character.id(), TalkChannel::TopLeft, line))
            .await?;
        Ok(())
    }
}
