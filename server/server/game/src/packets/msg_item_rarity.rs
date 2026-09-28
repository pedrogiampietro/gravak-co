use super::{MsgItemInfo, MsgPlayer, MsgTalk, MsgUserInfo, TalkChannel};
use crate::state::State;
use crate::ActorState;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tq_network::{Actor, PacketID, PacketProcess};

/// Custom sandbox packet 2078 for attempting the next independent item rarity.
#[derive(Debug, Clone, Serialize, Deserialize, PacketID)]
#[packet(id = 2078)]
pub struct MsgItemRarity {
    pub item_id: u32,
    pub rarity: u8,
}

#[async_trait]
impl PacketProcess for MsgItemRarity {
    type ActorState = ActorState;
    type Error = crate::Error;
    type State = State;

    async fn process(&self, state: &Self::State, actor: &Actor<Self::ActorState>) -> Result<(), Self::Error> {
        let entity = actor.entity();
        let character = entity.as_character().ok_or(crate::Error::CharacterNotFound)?;
        let reject = |message: &str| MsgTalk::from_system(character.id(), TalkChannel::System, message);
        if self.rarity > 4 {
            actor.send(reject("Rarity rejected: invalid rarity tier.")).await?;
            return Ok(());
        }
        if let Some(item) = character.item_in_memory(self.item_id) {
            if self.rarity != item.rarity.saturating_add(1) {
                actor
                    .send(reject("Rarity rejected: only the next tier can be attempted."))
                    .await?;
                return Ok(());
            }
        }
        let (outcome, catalysts) = character
            .upgrade_item_rarity_with_dragonballs(state.pool(), self.item_id)
            .await?;
        for catalyst in catalysts {
            actor.send(MsgItemInfo::remove_inventory_item(catalyst)).await?;
        }
        let item = match outcome {
            crate::entities::RarityUpgradeOutcome::Upgraded(item) => item,
            crate::entities::RarityUpgradeOutcome::Failed(item) => {
                let equipped = character.equipment_items().iter().any(|owned| owned.uid == item.uid);
                if equipped {
                    actor.send(MsgItemInfo::equipment_item(item)).await?;
                } else {
                    actor.send(MsgItemInfo::inventory_item(item)).await?;
                }
                actor
                    .send(reject("Rarity upgrade failed; DragonBalls were consumed."))
                    .await?;
                return Ok(());
            },
            crate::entities::RarityUpgradeOutcome::MaxRarity(_item) => {
                actor.send(reject("This item is already Mythic.")).await?;
                return Ok(());
            },
            crate::entities::RarityUpgradeOutcome::MissingDragonBall => {
                actor
                    .send(reject("Rarity upgrade rejected: you need more DragonBalls."))
                    .await?;
                return Ok(());
            },
            crate::entities::RarityUpgradeOutcome::NotFound => {
                actor
                    .send(reject("Rarity upgrade rejected: item was not found."))
                    .await?;
                return Ok(());
            },
        };

        let equipped = character.equipment_items().iter().any(|owned| owned.uid == item.uid);
        if equipped {
            actor.send(MsgItemInfo::equipment_item(item)).await?;
            character.refresh_equipment_resources(state.shop_catalog());
            actor.send(MsgUserInfo::from(character)).await?;
            actor.send(MsgPlayer::from(character)).await?;
            actor.screen().send_message(MsgPlayer::from(character)).await?;
        } else {
            actor.send(MsgItemInfo::inventory_item(item)).await?;
        }
        actor
            .send(MsgTalk::from_system(
                character.id(),
                TalkChannel::System,
                format!("Item rarity upgraded to tier {}.", item.rarity),
            ))
            .await?;
        Ok(())
    }
}
