use serde::{Deserialize, Serialize};
use tq_network::{Actor, PacketID, PacketProcess};

use crate::packets::{MsgItemInfo, MsgTalk, MsgUserInfo, TalkChannel};
use crate::{ActorState, Error, State};

/// Ground item appearance packet (1101), compatible with the classic client.
/// `drop_type` is 1 for a newly dropped item and 2 when synchronising an
/// existing item while entering a screen.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PacketID)]
#[packet(id = 1101)]
pub struct MsgGroundItem {
    pub uid: u32,
    pub item_id: u32,
    pub x: u16,
    pub y: u16,
    pub unknown: u16,
    pub drop_type: u16,
    pub reserved: [u8; 8],
}

impl MsgGroundItem {
    pub fn new(uid: u32, item_id: u32, x: u16, y: u16) -> Self {
        Self {
            uid,
            item_id,
            x,
            y,
            unknown: 3,
            drop_type: 1,
            reserved: [0; 8],
        }
    }

    pub fn delete(uid: u32, item_id: u32, x: u16, y: u16) -> Self {
        Self {
            uid,
            item_id,
            x,
            y,
            unknown: 3,
            drop_type: 2,
            reserved: [0; 8],
        }
    }
}

#[async_trait::async_trait]
impl PacketProcess for MsgGroundItem {
    type ActorState = ActorState;
    type Error = Error;
    type State = State;

    async fn process(&self, state: &State, actor: &Actor<ActorState>) -> Result<(), Error> {
        // GroundItemAction.Pick in the 5065 protocol.
        if self.drop_type != 3 {
            return Ok(());
        }
        let entity = actor.entity();
        let character = entity.as_character().ok_or(Error::CharacterNotFound)?;
        let Some(ground_item) = state.ground_item(self.uid) else {
            return Ok(());
        };
        let location = character.entity().location();
        let dx = f32::from(location.x) - f32::from(ground_item.x);
        let dy = f32::from(location.y) - f32::from(ground_item.y);
        let loot_owner_can_pick = ground_item.owner_id == 0
            || ground_item.owner_id == character.id()
            || state.team_manager().same_team(ground_item.owner_id, character.id());
        if ground_item.map_id != character.entity().map_id()
            || ground_item.item_id != self.item_id
            || ground_item.x != self.x
            || ground_item.y != self.y
            || (dx * dx + dy * dy) > 25.0
            || !loot_owner_can_pick
        {
            return Ok(());
        }

        // Remove atomically before awarding the loot so two clients cannot
        // collect the same UID at the same time.
        let Some(ground_item) = state.take_ground_item(self.uid) else {
            return Ok(());
        };
        let item_template = state.shop_catalog().item(ground_item.item_id);
        let result = if ground_item.silver > 0 {
            let balance = match character.grant_silver(state.pool(), ground_item.silver).await {
                Ok(balance) => balance,
                Err(error) => {
                    state.insert_ground_item(ground_item);
                    return Err(error);
                },
            };
            crate::audit::economic(
                "monster_drop_pickup",
                character.id(),
                ground_item.owner_id,
                ground_item.uid,
                ground_item.silver,
            );
            actor.send(MsgUserInfo::from(character)).await?;
            actor
                .send(MsgTalk::from_system(
                    character.id(),
                    TalkChannel::System,
                    format!("You picked up {} silver (total {}).", ground_item.silver, balance),
                ))
                .await?;
            true
        } else {
            let added = match character
                .add_ground_item(
                    state.pool(),
                    ground_item.item_id,
                    item_template.map_or(0, |template| template.durability),
                    item_template.map_or(0, |template| template.max_durability),
                )
                .await
            {
                Ok(added) => added,
                Err(error) => {
                    state.insert_ground_item(ground_item);
                    return Err(error);
                },
            };
            if let Some(item) = added {
                actor.send(MsgItemInfo::inventory_item(item)).await?;
                actor
                    .send(MsgTalk::from_system(
                        character.id(),
                        TalkChannel::System,
                        "You picked up an item.".to_owned(),
                    ))
                    .await?;
                true
            } else {
                actor
                    .send(MsgTalk::from_system(
                        character.id(),
                        TalkChannel::System,
                        "Your inventory is full.".to_owned(),
                    ))
                    .await?;
                false
            }
        };

        if result {
            let removed = MsgGroundItem::delete(ground_item.uid, ground_item.item_id, ground_item.x, ground_item.y);
            actor.send(removed).await?;
            actor.screen().send_message(removed).await?;
        } else {
            state.insert_ground_item(ground_item);
        }
        Ok(())
    }
}
