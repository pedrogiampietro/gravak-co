//! `MsgName` (1015): strings tied to an entity. Gravak sends two of its
//! actions: fireworks set off by a player, and a 3D effect played on one.

use serde::{Deserialize, Serialize};
use tq_network::PacketID;
use tq_serde::StringList;

/// Fireworks: the strings hold the firework item's name.
pub const FIREWORKS: u8 = 1;
/// A `3DEffect.ini` effect played on the entity.
pub const ROLE_EFFECT: u8 = 10;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, PacketID)]
#[packet(id = 1015)]
pub struct MsgName {
    pub uid: u32,
    pub action: u8,
    pub strings: StringList,
}

impl MsgName {
    pub fn new(uid: u32, action: u8, text: impl Into<String>) -> Self {
        Self {
            uid,
            action,
            strings: std::iter::once(text.into()).collect(),
        }
    }

    pub fn fireworks(uid: u32, item_name: &str) -> Self {
        Self::new(uid, FIREWORKS, item_name)
    }

    pub fn role_effect(uid: u32, effect: &str) -> Self {
        Self::new(uid, ROLE_EFFECT, effect)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tq_network::PacketEncode;

    #[test]
    fn writes_the_uid_action_and_one_string() {
        let (id, bytes) = MsgName::role_effect(1_000_001, "LevelUp").encode().expect("encodes");
        assert_eq!(id, 1015);
        assert_eq!(&bytes[..4], &1_000_001_u32.to_le_bytes());
        assert_eq!(bytes[4], ROLE_EFFECT);
        assert_eq!(bytes[5], 1, "one string");
        assert_eq!(bytes[6] as usize, "LevelUp".len());
        assert_eq!(&bytes[7..14], b"LevelUp");
    }
}
