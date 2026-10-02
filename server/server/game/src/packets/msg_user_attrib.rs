use serde::{Deserialize, Serialize};
use tq_network::PacketID;

/// Attribute kinds of [`MsgUserAttrib`] (Redux `UpdateType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum UserAttribute {
    Life = 0,
    MaxLife = 1,
    Pk = 6,
    Stamina = 9,
}

/// 5065 single-attribute update (1017): one changed value of a character.
#[derive(Debug, Clone, Serialize, Deserialize, PacketID)]
#[packet(id = 1017)]
pub struct MsgUserAttrib {
    pub character_id: u32,
    pub count: u32,
    pub attribute: u32,
    pub value: u64,
    pub reserved: u64,
}

impl MsgUserAttrib {
    pub fn new(character_id: u32, attribute: UserAttribute, value: u64) -> Self {
        Self {
            character_id,
            count: 1,
            attribute: attribute as u32,
            value,
            reserved: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tq_network::PacketEncode;

    #[test]
    fn encodes_the_32_byte_update() {
        let packet = MsgUserAttrib::new(1_000_001, UserAttribute::Stamina, 77);
        let (id, body) = packet.encode().expect("packet encodes");
        assert_eq!(id, 1017);
        assert_eq!(body.len(), 28);
        assert_eq!(u32::from_le_bytes(body[8..12].try_into().unwrap()), 9);
        assert_eq!(u64::from_le_bytes(body[12..20].try_into().unwrap()), 77);
    }
}
