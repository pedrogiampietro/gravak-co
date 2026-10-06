//! Who is on the other side of a whisper (a Gravak packet, not in the 5065
//! client): sent with each whisper to both ends, so the private chat window
//! shows the other's face, level, class and guild even off screen.

use crate::{ActorState, Error, State};
use serde::{Deserialize, Serialize};
use tq_network::{Actor, PacketID};

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, PacketID)]
#[packet(id = 9150)]
pub struct MsgWhisperCard {
    /// The look with the avatar (`mesh + avatar * 10000`), for the face.
    pub mesh: u32,
    pub level: u32,
    pub rebirths: u32,
    pub class: u32,
    pub name: String,
    /// Empty without a guild.
    pub guild: String,
    pub battle_power: u32,
}

impl MsgWhisperCard {
    pub fn of(state: &State, character: &crate::entities::Character) -> Self {
        Self {
            mesh: character.entity().mesh() + u32::from(character.avatar()) * 10_000,
            level: u32::from(character.entity().level()),
            rebirths: u32::from(character.rebirths()),
            class: u32::from(character.current_class()),
            name: character.entity().name().to_owned(),
            guild: state.guild_manager().guild_for(character.id()).map(|guild| guild.name).unwrap_or_default(),
            battle_power: battle_power(character),
        }
    }
}

/// The battle power as the client's Battle Power Evaluation counts it:
/// level, five per rebirth, the nobility bonus and, per worn item (the
/// garment aside), quality above Normal, one per gem and one more per super
/// gem, and its plus; a two-hander with nothing in the other hand twice.
fn battle_power(character: &crate::entities::Character) -> u32 {
    use crate::nobility::NobilityRank;
    let nobility = match character.nobility_status().rank {
        NobilityRank::King => 5,
        NobilityRank::Prince => 4,
        NobilityRank::Duke => 3,
        NobilityRank::Earl => 2,
        NobilityRank::Baron => 1,
        NobilityRank::Knight | NobilityRank::Serf => 0,
    };
    let worn = character.equipment_items();
    let off_hand = worn.iter().any(|item| item.position == 5);
    let items: u32 = worn
        .iter()
        .filter(|item| (1..=8).contains(&item.position))
        .map(|item| {
            let gems = [item.gem_one, item.gem_two].into_iter().filter(|gem| *gem != 0 && *gem != 255);
            let value = (item.item_type % 10).saturating_sub(5)
                + gems.clone().count() as u32
                + gems.filter(|gem| gem % 10 == 3).count() as u32
                + u32::from(item.plus);
            let two_handed = item.position == 4 && (500..600).contains(&(item.item_type / 1_000));
            value * if two_handed && !off_hand { 2 } else { 1 }
        })
        .sum();
    u32::from(character.entity().level()) + u32::from(character.rebirths()) * 5 + nobility + items
}

/// Each end of a whisper learns who the other is.
pub async fn exchange(
    state: &State,
    sender: &Actor<ActorState>,
    recipient: &crate::entities::GameEntity,
) -> Result<(), Error> {
    let entity = sender.try_entity()?;
    let (Some(me), Some(them)) = (entity.as_character(), recipient.as_character()) else {
        return Ok(());
    };
    if me.id() == them.id() {
        return Ok(());
    }
    sender.send(MsgWhisperCard::of(state, them)).await?;
    if let Some(owner) = recipient.owner() {
        owner.send(MsgWhisperCard::of(state, me)).await?;
    }
    Ok(())
}
