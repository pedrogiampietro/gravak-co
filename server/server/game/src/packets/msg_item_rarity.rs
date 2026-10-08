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
                let next = character.rarity_chance(state.pool(), item.uid).await?.unwrap_or_default();
                actor
                    .send(reject(&format!(
                        "The rarity did not rise; the catalysts were spent. The next attempt has {next}% chance."
                    )))
                    .await?;
                return Ok(());
            },
            crate::entities::RarityUpgradeOutcome::MaxRarity(_item) => {
                actor.send(reject("This item is already Mythic.")).await?;
                return Ok(());
            },
            crate::entities::RarityUpgradeOutcome::MissingDragonBall => {
                let need = character
                    .item_in_memory(self.item_id)
                    .and_then(|item| crate::affixes::upgrade_cost(item.rarity))
                    .map_or_else(String::new, |(catalyst, count, _)| {
                        let name = if catalyst == crate::affixes::SUPER_DRAGONBALL { "SuperDragonBall" } else { "DragonBall" };
                        format!(" It takes {count} {name}{}.", if count > 1 { "s" } else { "" })
                    });
                actor.send(reject(&format!("You lack the catalysts.{need}"))).await?;
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
        let tier = ["Common", "Rare", "Epic", "Legendary", "Mythic"][usize::from(item.rarity.min(4))];
        actor
            .send(MsgTalk::from_system(
                character.id(),
                TalkChannel::System,
                format!("The item rose to {tier}, and gained an affix."),
            ))
            .await?;
        // A Mythic is news to everyone online.
        if item.rarity >= 4 {
            crate::bosses::announce(state, format!("{} forged a Mythic item!", character.entity().name())).await;
        }
        Ok(())
    }
}
