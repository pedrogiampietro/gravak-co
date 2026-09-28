use super::{MsgItemInfo, MsgTalk, TalkChannel};
use crate::{ActorState, State};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tq_network::{Actor, PacketID, PacketProcess};

pub const OPEN: u8 = 0;
pub const DEPOSIT: u8 = 1;
pub const WITHDRAW: u8 = 2;

/// Client request for the Guild item warehouse. This is intentionally separate
/// from the NPC warehouse packet: Guild storage has no NPC proximity or active
/// storage session and uses a Guild ID instead of an NPC ID.
#[derive(Debug, Clone, Serialize, Deserialize, PacketID)]
#[packet(id = 2034)]
pub struct MsgGuildWarehouse {
    character_id: u32,
    guild_id: u32,
    action: u8,
    item_uid: u32,
    position: u8,
}

impl MsgGuildWarehouse {
    fn notice(character_id: u32, message: impl Into<String>) -> MsgTalk {
        MsgTalk::from_system(character_id, TalkChannel::Guild, message)
    }
}

#[async_trait]
impl PacketProcess for MsgGuildWarehouse {
    type ActorState = ActorState;
    type Error = crate::Error;
    type State = State;

    async fn process(&self, state: &State, actor: &Actor<ActorState>) -> Result<(), crate::Error> {
        let entity = actor.entity();
        let character = entity.as_character().ok_or(crate::Error::CharacterNotFound)?;
        let Some(guild) = state.guild_manager().guild_for(character.id()) else {
            actor
                .send(Self::notice(
                    character.id(),
                    "Guild warehouse rejected: you are not in a guild.",
                ))
                .await?;
            return Ok(());
        };
        if guild.id != self.guild_id || self.character_id != character.id() {
            actor
                .send(Self::notice(
                    character.id(),
                    "Guild warehouse rejected: invalid Guild or character.",
                ))
                .await?;
            return Ok(());
        }
        let role = state.guild_manager().role_for(character.id()).unwrap_or_default();
        if self.action == WITHDRAW && role < 1000 {
            actor
                .send(Self::notice(
                    character.id(),
                    "Guild warehouse rejected: only the Guild leader can withdraw items.",
                ))
                .await?;
            return Ok(());
        }

        match self.action {
            OPEN => {
                actor.send(MsgItemInfo::clear_guild_warehouse(guild.id)).await?;
                for item in character.guild_warehouse_items(state.pool(), guild.id).await? {
                    actor.send(MsgItemInfo::guild_warehouse_item(item)).await?;
                }
            },
            DEPOSIT => {
                let Some((source, destination)) = character
                    .store_guild_warehouse_item(state.pool(), guild.id, self.item_uid, self.position)
                    .await?
                else {
                    actor
                        .send(Self::notice(
                            character.id(),
                            "Guild warehouse rejected: item or slot is unavailable.",
                        ))
                        .await?;
                    return Ok(());
                };
                actor.send(MsgItemInfo::remove_inventory_item(source)).await?;
                actor.send(MsgItemInfo::guild_warehouse_item(destination)).await?;
            },
            WITHDRAW => {
                let Some((source, destination)) = character
                    .withdraw_guild_warehouse_item(state.pool(), guild.id, self.item_uid, self.position)
                    .await?
                else {
                    actor
                        .send(Self::notice(
                            character.id(),
                            "Guild warehouse rejected: item or inventory slot is unavailable.",
                        ))
                        .await?;
                    return Ok(());
                };
                actor.send(MsgItemInfo::remove_guild_warehouse_item(source)).await?;
                actor.send(MsgItemInfo::inventory_item(destination)).await?;
            },
            _ => {
                actor
                    .send(Self::notice(
                        character.id(),
                        "Guild warehouse rejected: unknown action.",
                    ))
                    .await?;
            },
        }
        Ok(())
    }
}
