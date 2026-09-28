use super::{MsgItemInfo, MsgTalk, TalkChannel};
use crate::entities::NpcKind;
use crate::{ActorState, State};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tq_network::{Actor, PacketID, PacketProcess};

#[derive(Debug, Clone, Serialize, Deserialize, PacketID)]
#[packet(id = 2033)]
pub struct MsgWarehouse {
    character_id: u32,
    npc_id: u32,
    action: u8,
    item_uid: u32,
    position: u8,
    amount: u32,
}

#[async_trait]
impl PacketProcess for MsgWarehouse {
    type ActorState = ActorState;
    type Error = crate::Error;
    type State = State;

    async fn process(&self, state: &Self::State, actor: &Actor<Self::ActorState>) -> Result<(), Self::Error> {
        let entity = actor.entity();
        let character = entity.as_character().ok_or(crate::Error::CharacterNotFound)?;
        let reject = |message: &str| MsgTalk::from_system(character.id(), TalkChannel::TopLeft, message);
        if self.character_id != character.id() || self.npc_id != character.active_storage_id() {
            actor
                .send(reject("Warehouse action rejected: open a warehouse first."))
                .await?;
            return Ok(());
        }
        let map = state.try_map(character.entity().map_id())?;
        let Some(npc) = map.npc(self.npc_id) else {
            actor
                .send(reject("Warehouse action rejected: warehouse is not nearby."))
                .await?;
            return Ok(());
        };
        let player = character.entity().location();
        let location = npc.entity().location();
        if npc.kind() != NpcKind::Storage || player.x.abs_diff(location.x) > 12 || player.y.abs_diff(location.y) > 12 {
            actor
                .send(reject("Warehouse action rejected: warehouse is not nearby."))
                .await?;
            return Ok(());
        }

        if self.action == 3 || self.action == 4 {
            match character
                .transfer_warehouse_silver(state.pool(), self.amount, self.action == 3)
                .await?
            {
                Some(saved) => {
                    crate::audit::economic(
                        if self.action == 3 {
                            "warehouse_deposit"
                        } else {
                            "warehouse_withdraw"
                        },
                        character.id(),
                        self.npc_id,
                        0,
                        self.amount,
                    );
                    actor.send(super::MsgUserInfo::from(character)).await?;
                    actor.send(MsgItemInfo::warehouse_balance(self.npc_id, saved)).await?;
                },
                None => {
                    actor
                        .send(reject("Invalid amount or insufficient silver/storage capacity."))
                        .await?
                },
            }
            return Ok(());
        }
        let result = match self.action {
            1 => {
                character
                    .store_inventory_item(state.pool(), self.item_uid, self.position)
                    .await?
            },
            2 => {
                character
                    .withdraw_warehouse_item(state.pool(), self.item_uid, self.position)
                    .await?
            },
            _ => {
                actor.send(reject("Unknown warehouse action.")).await?;
                return Ok(());
            },
        };
        let Some((source, destination)) = result else {
            actor
                .send(reject("That warehouse slot or item is no longer available."))
                .await?;
            return Ok(());
        };
        if self.action == 1 {
            actor.send(MsgItemInfo::remove_inventory_item(source)).await?;
            actor.send(MsgItemInfo::warehouse_item(destination)).await?;
        } else {
            actor.send(MsgItemInfo::remove_warehouse_item(source)).await?;
            actor.send(MsgItemInfo::inventory_item(destination)).await?;
        }
        Ok(())
    }
}
