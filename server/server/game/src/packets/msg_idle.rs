//! The idle hunt and the helper's merchant (Gravak packets, not in the 5065
//! client): `MsgIdle` carries the helper's requests, `MsgIdleReport` and
//! `MsgIdleLine` what the time away brought (`crate::idle`).

use crate::{ActorState, Error, State};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tq_network::{Actor, PacketID, PacketProcess};

/// A helper's request: what it does (`action`) and with what.
#[derive(Debug, Default, Clone, Serialize, Deserialize, PacketID)]
#[packet(id = 9190)]
pub struct MsgIdle {
    pub action: u32,
    pub param0: u32,
    pub param1: u32,
    pub param2: u32,
    pub param3: u32,
}

impl MsgIdle {
    /// Hunting (`param0` 1) at x `param1`, y `param2`, within reach and
    /// levels above (`param3`: reach in the low half, levels in the high
    /// one); `param0` 0, stopped.
    pub const HUNT: u32 = 1;
    /// The client is in the world: what the time away brought, if anything.
    pub const BACK: u32 = 2;
    /// Sell bag item `param0`.
    pub const SELL: u32 = 3;
    /// Repair what is worn.
    pub const REPAIR: u32 = 4;
    /// Buy `param1` of potion `param0`.
    pub const BUY: u32 = 5;
}

/// What the time away brought: how long, the kills, the experience and
/// levels, the silver, and how many items were kept and sold. Its lines
/// (`MsgIdleLine`) come first.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, PacketID)]
#[packet(id = 9191)]
pub struct MsgIdleReport {
    pub seconds: u32,
    pub kills: u32,
    pub experience: u64,
    pub level_before: u32,
    pub level_after: u32,
    pub silver: u32,
    pub kept: u32,
    pub sold: u32,
}

/// One line of the report, `index` of `total`: the monsters felled of a
/// kind (`name`), or an item kept or sold (`id`), and how many.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, PacketID)]
#[packet(id = 9192)]
pub struct MsgIdleLine {
    pub index: u32,
    pub total: u32,
    pub kind: u32,
    pub id: u32,
    pub amount: u32,
    pub name: String,
}

impl MsgIdleLine {
    pub const KILLS: u32 = 0;
    pub const KEPT: u32 = 1;
    pub const SOLD: u32 = 2;
}

#[async_trait]
impl PacketProcess for MsgIdle {
    type ActorState = ActorState;
    type Error = Error;
    type State = State;

    async fn process(&self, state: &Self::State, actor: &Actor<Self::ActorState>) -> Result<(), Self::Error> {
        let entity = actor.entity();
        let character = entity.as_character().ok_or(Error::CharacterNotFound)?;
        match self.action {
            Self::HUNT => {
                let spot = (self.param0 == 1).then(|| crate::idle::Spot {
                    map_id: character.entity().map_id(),
                    x: self.param1.min(u32::from(u16::MAX)) as u16,
                    y: self.param2.min(u32::from(u16::MAX)) as u16,
                    radius: (self.param3 & 0xffff).clamp(1, 30) as u16,
                    levels_above: (self.param3 >> 16).min(20) as u16,
                });
                crate::idle::set_spot(character.id(), spot);
            },
            Self::BACK => crate::idle::welcome_back(state, character).await?,
            Self::SELL => {
                crate::idle::sell(state, character, self.param0).await?;
            },
            Self::REPAIR => {
                crate::idle::repair(state, character).await?;
            },
            Self::BUY => {
                crate::idle::buy(state, character, self.param0, self.param1).await?;
            },
            _ => {},
        }
        Ok(())
    }
}
