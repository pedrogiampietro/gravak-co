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
        }
    }
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
