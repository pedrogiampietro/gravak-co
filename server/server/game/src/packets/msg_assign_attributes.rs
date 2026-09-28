use super::{MsgTalk, MsgUserInfo, TalkChannel};
use crate::entities::{AttributeKind, Character};
use crate::{ActorState, Error, State};
use serde::{Deserialize, Serialize};
use tq_network::{Actor, PacketID, PacketProcess};

/// Classic 5065 packet 1024. The authenticated actor identifies the character;
/// the four bytes are the requested amounts for Strength, Agility, Vitality and
/// Spirit respectively.
#[derive(Debug, Serialize, Deserialize, Clone, PacketID)]
#[packet(id = 1024)]
pub struct MsgAssignAttributes {
    pub strength: u8,
    pub agility: u8,
    pub vitality: u8,
    pub spirit: u8,
}

impl MsgAssignAttributes {
    async fn allocate_one(
        &self,
        state: &State,
        actor: &Actor<ActorState>,
        character: &Character,
        kind: AttributeKind,
        amount: u8,
    ) -> Result<bool, Error> {
        if amount == 0 {
            return Ok(true);
        }
        match character
            .allocate_attribute(state.pool(), kind, u16::from(amount))
            .await?
        {
            crate::entities::AttributeAllocation::Allocated { .. } => Ok(true),
            crate::entities::AttributeAllocation::InsufficientPoints { available } => {
                actor
                    .send(MsgTalk::from_system(
                        character.id(),
                        TalkChannel::System,
                        format!("Not enough attribute points. Available: {available}."),
                    ))
                    .await?;
                Ok(false)
            },
        }
    }
}

#[async_trait::async_trait]
impl PacketProcess for MsgAssignAttributes {
    type ActorState = ActorState;
    type Error = Error;
    type State = State;

    async fn process(&self, state: &Self::State, actor: &Actor<Self::ActorState>) -> Result<(), Self::Error> {
        let entity = actor.try_entity()?;
        let character = entity.as_character().ok_or(Error::CharacterNotFound)?;
        if !self
            .allocate_one(state, actor, character, AttributeKind::Strength, self.strength)
            .await?
            || !self
                .allocate_one(state, actor, character, AttributeKind::Agility, self.agility)
                .await?
            || !self
                .allocate_one(state, actor, character, AttributeKind::Vitality, self.vitality)
                .await?
            || !self
                .allocate_one(state, actor, character, AttributeKind::Spirit, self.spirit)
                .await?
        {
            return Ok(());
        }
        character.refresh_equipment_resources(state.shop_catalog());
        actor.send(MsgUserInfo::from(character)).await?;
        Ok(())
    }
}
