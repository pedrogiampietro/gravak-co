use crate::entities::Character;
use crate::nobility::NobilityStatus;
use serde::{Deserialize, Serialize};
use tq_network::PacketID;

/// Server-authoritative nobility state for the local character.
///
/// `action = 3` matches the status/info action used by the client slice. A
/// separate packet keeps the donation amount out of the legacy UserInfo
/// payload, so older clients can still decode UserInfo safely.
#[derive(Debug, Clone, Serialize, Deserialize, PacketID)]
#[packet(id = 2064)]
pub struct MsgNobility {
    pub action: u32,
    pub character_id: u32,
    pub donation: u64,
    pub rank: u32,
    pub remaining: u64,
    pub title_id: u32,
}

impl MsgNobility {
    pub const STATUS: u32 = 3;

    pub fn status(character_id: u32, status: NobilityStatus, title_id: u32) -> Self {
        Self {
            action: Self::STATUS,
            character_id,
            donation: status.donation,
            rank: status.rank as u32,
            remaining: status.remaining,
            title_id,
        }
    }
}

impl From<&Character> for MsgNobility {
    fn from(character: &Character) -> Self {
        Self::status(character.id(), character.nobility_status(), character.equipped_title())
    }
}
