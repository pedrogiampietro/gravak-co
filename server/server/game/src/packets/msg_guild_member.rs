use crate::entities::Character;
use serde::{Deserialize, Serialize};
use tq_network::PacketID;
use tq_serde::String16;

/// One member record carried by the classic 2102 guild-member list.
/// The server sends one record per packet while the online guild list is
/// still being connected to the persistent repository.
#[derive(Debug, Clone, Serialize, Deserialize, PacketID)]
#[packet(id = 2102)]
pub struct MsgGuildMember {
    pub reserved: u32,
    pub start_index: i32,
    pub amount: i32,
    pub name: String16,
    pub reserved16: u32,
    pub reserved20: u32,
    pub level: u32,
    pub rank: u16,
    pub reserved30: u16,
    pub reserved32: u32,
    pub donation: i32,
    pub online: u8,
    pub reserved41: [u8; 3],
    pub reserved44: u32,
}

impl MsgGuildMember {
    pub fn from_character(character: &Character, rank: u16, start_index: i32) -> Self {
        Self {
            reserved: 0,
            start_index,
            amount: 1,
            name: character.entity().name().to_owned().into(),
            reserved16: character.id(),
            reserved20: character.entity().mesh(),
            level: u32::from(character.entity().level()),
            rank,
            reserved30: 0,
            reserved32: 0,
            donation: 0,
            online: 1,
            reserved41: [0; 3],
            reserved44: 0,
        }
    }
}
