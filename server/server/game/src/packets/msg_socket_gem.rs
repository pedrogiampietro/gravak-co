use super::{MsgItemInfo, MsgTalk, TalkChannel};
use crate::entities::SocketOutcome;
use crate::state::State;
use crate::ActorState;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tq_network::{Actor, PacketID, PacketProcess};

/// Classic 5065 packet 1027 used by the compose window to insert or remove a
/// gem. `subtype` is 0 for insertion and 1 for removal; `socket` is 1 or 2.
#[derive(Debug, Clone, Serialize, Deserialize, PacketID)]
#[packet(id = 1027)]
pub struct MsgSocketGem {
    pub item_id: u32,
    pub gem_id: u32,
    pub socket: u16,
    pub subtype: u8,
}

#[async_trait]
impl PacketProcess for MsgSocketGem {
    type ActorState = ActorState;
    type Error = crate::Error;
    type State = State;

    async fn process(&self, state: &Self::State, actor: &Actor<Self::ActorState>) -> Result<(), Self::Error> {
        let entity = actor.entity();
        let character = entity.as_character().ok_or(crate::Error::CharacterNotFound)?;
        let socket = u8::try_from(self.socket).unwrap_or_default();
        let remove = self.subtype == 1;
        let outcome = character
            .socket_gem(state.pool(), self.item_id, self.gem_id, socket, remove)
            .await?;
        let reject = |message: &str| MsgTalk::from_system(character.id(), TalkChannel::System, message);
        match outcome {
            SocketOutcome::Inserted { item, gem } => {
                actor.send(MsgItemInfo::remove_inventory_item(gem)).await?;
                Self::send_updated_item(actor, character, item).await?;
                actor
                    .send(MsgTalk::from_system(
                        character.id(),
                        TalkChannel::System,
                        "Gem inserted successfully.",
                    ))
                    .await?;
            },
            SocketOutcome::Removed(item) => {
                Self::send_updated_item(actor, character, item).await?;
                actor
                    .send(MsgTalk::from_system(
                        character.id(),
                        TalkChannel::System,
                        "Gem removed from socket.",
                    ))
                    .await?;
            },
            SocketOutcome::InvalidSlot => actor.send(reject("Socket rejected: invalid socket.")).await?,
            SocketOutcome::SocketClosed => actor.send(reject("Socket rejected: this socket is not open.")).await?,
            SocketOutcome::SocketOccupied => {
                actor
                    .send(reject("Socket rejected: this socket is already occupied."))
                    .await?
            },
            SocketOutcome::InvalidGem => actor.send(reject("Socket rejected: that item is not a gem.")).await?,
            SocketOutcome::NotFound => actor.send(reject("Socket rejected: item was not found.")).await?,
            SocketOutcome::Opened(_) | SocketOutcome::MissingDragonBall | SocketOutcome::DualSockets => {
                actor.send(reject("Socket rejected: invalid gem operation.")).await?
            },
        }
        Ok(())
    }
}

impl MsgSocketGem {
    async fn send_updated_item(
        actor: &Actor<ActorState>,
        character: &crate::entities::Character,
        item: crate::entities::InventoryItem,
    ) -> Result<(), crate::Error> {
        if character.equipment_items().iter().any(|owned| owned.uid == item.uid) {
            actor.send(MsgItemInfo::equipment_item(item)).await?;
        } else {
            actor.send(MsgItemInfo::inventory_item(item)).await?;
        }
        Ok(())
    }
}
