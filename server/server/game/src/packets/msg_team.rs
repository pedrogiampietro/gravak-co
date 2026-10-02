use crate::entities::Character;
use serde::{Deserialize, Serialize};
use tq_network::PacketID;
use tq_serde::String16;

/// Classic 1023 party action. The action and target are authoritative; the
/// server never trusts a client-provided character object or team membership.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, PacketID)]
#[packet(id = 1023)]
pub struct MsgTeam {
    pub action: u32,
    pub target_id: u32,
    pub reserved: u32,
}

impl MsgTeam {
    pub fn new(action: u32, target_id: u32) -> Self {
        Self {
            action,
            target_id,
            reserved: 0,
        }
    }
}

/// Team member information used by the classic team window.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, PacketID)]
#[packet(id = 1026)]
pub struct MsgTeamMember {
    pub reserved: u8,
    pub reserved_header: [u8; 3],
    pub name: String16,
    pub member_id: u32,
    pub mesh: u32,
    pub max_health: u16,
    pub health: u16,
    pub action: u8,
    pub reserved_tail: [u8; 7],
}

impl MsgTeamMember {
    pub fn from_character(character: &Character, action: u8) -> Self {
        Self {
            reserved: 0,
            reserved_header: [0; 3],
            name: character.entity().name().to_owned().into(),
            member_id: character.id(),
            // The look with the face, as in MsgPlayer: the team window draws
            // the member's portrait from it.
            mesh: character.entity().mesh() + u32::from(character.avatar()) * 10_000,
            max_health: character.maximum_health_points(),
            health: character.entity().hp().current(),
            action,
            reserved_tail: [0; 7],
        }
    }
}
