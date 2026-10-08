use super::{MsgItemInfo, MsgTalk, TalkChannel};
use crate::state::State;
use crate::ActorState;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tq_network::{Actor, PacketID, PacketProcess};

/// Custom sandbox packet 2079 for rerolling an item's rarity affixes: all
/// of them for a DragonBall (`slot` 255), or the one in `slot` for a Super
/// DragonBall.
#[derive(Debug, Clone, Serialize, Deserialize, PacketID)]
#[packet(id = 2079)]
pub struct MsgItemRarityReroll {
    pub item_id: u32,
    pub slot: u8,
}

#[async_trait]
impl PacketProcess for MsgItemRarityReroll {
    type ActorState = ActorState;
    type Error = crate::Error;
    type State = State;

    async fn process(&self, state: &Self::State, actor: &Actor<Self::ActorState>) -> Result<(), Self::Error> {
        let entity = actor.entity();
        let character = entity.as_character().ok_or(crate::Error::CharacterNotFound)?;
        let reject = |message: &str| MsgTalk::from_system(character.id(), TalkChannel::System, message);
        let (outcome, catalyst) = character
            .reroll_item_rarity_bonus_with_dragonball(
                state.pool(),
                self.item_id,
                (self.slot != u8::MAX).then_some(usize::from(self.slot)),
            )
            .await?;
        if let Some(catalyst) = catalyst {
            actor.send(MsgItemInfo::remove_inventory_item(catalyst)).await?;
        }
        let item = match outcome {
            crate::entities::RarityBonusOutcome::Rerolled(item) => item,
            crate::entities::RarityBonusOutcome::NoRarity => {
                actor.send(reject("Reroll requires at least Rare rarity.")).await?;
                return Ok(());
            },
            crate::entities::RarityBonusOutcome::MissingDragonBall => {
                actor
                    .send(reject(if self.slot == u8::MAX {
                        "Rerolling every affix takes a DragonBall."
                    } else {
                        "Rerolling one affix takes a SuperDragonBall."
                    }))
                    .await?;
                return Ok(());
            },
            crate::entities::RarityBonusOutcome::NotFound => {
                actor.send(reject("Reroll rejected: item was not found.")).await?;
                return Ok(());
            },
        };
        if character.equipment_items().iter().any(|owned| owned.uid == item.uid) {
            actor.send(MsgItemInfo::equipment_item(item)).await?;
            character.refresh_equipment_resources(state.shop_catalog());
            actor.send(super::MsgUserInfo::from(character)).await?;
        } else {
            actor.send(MsgItemInfo::inventory_item(item)).await?;
        }
        actor
            .send(MsgTalk::from_system(
                character.id(),
                TalkChannel::System,
                if self.slot == u8::MAX { "The item's affixes were rerolled." } else { "The affix was rerolled." },
            ))
            .await?;
        Ok(())
    }
}
