use super::{MsgItemInfo, MsgPlayer, MsgRefineryInfo, MsgTalk, MsgUserInfo, TalkChannel};
use crate::entities::{ArtifactOutcome, RefineryOutcome};
use crate::state::State;
use crate::ActorState;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tq_network::{Actor, PacketID, PacketProcess};

/// Classic 5065 packet 2076. Action 0 applies the material in `material_id`
/// to the equipped item in `item_id`.
#[derive(Debug, Clone, Serialize, Deserialize, PacketID)]
#[packet(id = 2076)]
pub struct MsgRefinery {
    pub action: u32,
    pub item_id: u32,
    pub material_id: u32,
}

#[async_trait]
impl PacketProcess for MsgRefinery {
    type ActorState = ActorState;
    type Error = crate::Error;
    type State = State;

    async fn process(&self, state: &Self::State, actor: &Actor<Self::ActorState>) -> Result<(), Self::Error> {
        let entity = actor.entity();
        let character = entity.as_character().ok_or(crate::Error::CharacterNotFound)?;
        let reject = |message: &str| MsgTalk::from_system(character.id(), TalkChannel::System, message);
        if self.action == 1 {
            match character
                .apply_artifact(state.pool(), self.item_id, self.material_id)
                .await?
            {
                ArtifactOutcome::Applied {
                    target,
                    material,
                    catalyst,
                    effect,
                } => {
                    actor.send(MsgItemInfo::remove_inventory_item(material)).await?;
                    actor.send(MsgItemInfo::remove_inventory_item(catalyst)).await?;
                    actor.send(MsgItemInfo::inventory_item(target)).await?;
                    actor.send(MsgRefineryInfo::artifact(target.uid, effect)).await?;
                    actor
                        .send(MsgTalk::from_system(
                            character.id(),
                            TalkChannel::System,
                            format!(
                                "Artifact applied successfully (type {}, level {}, effect {}).",
                                effect.artifact_type, effect.level, effect.effect
                            ),
                        ))
                        .await?;
                },
                ArtifactOutcome::InvalidTarget => {
                    actor
                        .send(reject("Artifact rejected: target must be an inventory equipment item."))
                        .await?
                },
                ArtifactOutcome::InvalidMaterial => {
                    actor
                        .send(reject("Artifact rejected: invalid artifact material."))
                        .await?
                },
                ArtifactOutcome::Incompatible => {
                    actor
                        .send(reject(
                            "Artifact rejected: this material is incompatible with the item.",
                        ))
                        .await?
                },
                ArtifactOutcome::MissingMeteor => actor.send(reject("Artifact rejected: you need a Meteor.")).await?,
                ArtifactOutcome::NotFound => actor.send(reject("Artifact rejected: item was not found.")).await?,
            }
            return Ok(());
        }
        if self.action != 0 {
            actor.send(reject("Refinery rejected: unsupported action.")).await?;
            return Ok(());
        }
        match character
            .apply_refinery(state.pool(), self.item_id, self.material_id)
            .await?
        {
            RefineryOutcome::Applied {
                target,
                material,
                effect,
            } => {
                actor.send(MsgItemInfo::remove_inventory_item(material)).await?;
                actor.send(MsgItemInfo::equipment_item(target)).await?;
                actor.send(MsgRefineryInfo::refinery(target.uid, effect)).await?;
                character.refresh_equipment_resources(state.shop_catalog());
                actor.send(MsgUserInfo::from(character)).await?;
                actor.send(MsgPlayer::from(character)).await?;
                actor.screen().send_message(MsgPlayer::from(character)).await?;
                actor
                    .send(MsgTalk::from_system(
                        character.id(),
                        TalkChannel::System,
                        format!(
                            "Refinery applied successfully (type {}, level {}, effect {}).",
                            effect.refinery_type, effect.level, effect.effect
                        ),
                    ))
                    .await?;
            },
            RefineryOutcome::InvalidTarget => {
                actor
                    .send(reject("Refinery rejected: target must be equipped."))
                    .await?
            },
            RefineryOutcome::InvalidMaterial => {
                actor
                    .send(reject("Refinery rejected: invalid refinery material."))
                    .await?
            },
            RefineryOutcome::Incompatible => {
                actor
                    .send(reject(
                        "Refinery rejected: this material is incompatible with the item.",
                    ))
                    .await?
            },
            RefineryOutcome::NotFound => actor.send(reject("Refinery rejected: item was not found.")).await?,
        }
        Ok(())
    }
}
