use crate::refinery::{ArtifactEffect, RefineryEffect};
use serde::Serialize;
use std::time::{SystemTime, UNIX_EPOCH};
use tq_network::PacketID;

/// Classic 5065 packet 2077. It announces the refinery/artifact effects
/// currently attached to an item. One record is sent per effect; `amount` is
/// kept in the wire shape so the client can also accept batched records.
#[derive(Debug, Clone, Serialize, PacketID)]
#[packet(id = 2077)]
pub struct MsgRefineryInfo {
    pub amount: u32,
    pub item_id: u32,
    pub action: u32,
    pub refinery_type: u32,
    pub level: u32,
    pub effect: i32,
    pub time: u32,
    pub unknown: u32,
}

impl MsgRefineryInfo {
    pub fn refinery(item_id: u32, effect: RefineryEffect) -> Self {
        Self::new(
            item_id,
            2,
            effect.refinery_type,
            effect.level,
            effect.effect,
            effect.expires_at,
        )
    }

    pub fn artifact(item_id: u32, effect: ArtifactEffect) -> Self {
        Self::new(
            item_id,
            6,
            effect.artifact_type,
            effect.level,
            effect.effect,
            effect.expires_at,
        )
    }

    fn new(item_id: u32, action: u32, refinery_type: u32, level: u32, effect: i32, expires_at: u64) -> Self {
        Self {
            amount: 1,
            item_id,
            action,
            refinery_type,
            level,
            effect,
            time: remaining_seconds(expires_at),
            unknown: 0,
        }
    }
}

fn remaining_seconds(expires_at: u64) -> u32 {
    if expires_at == 0 {
        return 0;
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs())
        .unwrap_or_default();
    expires_at.saturating_sub(now).min(u64::from(u32::MAX)) as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    use tq_network::PacketEncode;

    #[test]
    fn encodes_one_classic_refinery_info_record() {
        let packet = MsgRefineryInfo::artifact(
            77,
            ArtifactEffect {
                artifact_type: 820_001,
                level: 1,
                effect: 25,
                expires_at: 0,
            },
        )
        .encode()
        .expect("refinery info packet should encode");
        assert_eq!(packet.0, 2077);
        assert_eq!(packet.1.len(), 32);
        assert_eq!(u32::from_le_bytes(packet.1[0..4].try_into().unwrap()), 1);
        assert_eq!(u32::from_le_bytes(packet.1[4..8].try_into().unwrap()), 77);
        assert_eq!(u32::from_le_bytes(packet.1[8..12].try_into().unwrap()), 6);
        assert_eq!(u32::from_le_bytes(packet.1[12..16].try_into().unwrap()), 820_001);
        assert_eq!(i32::from_le_bytes(packet.1[20..24].try_into().unwrap()), 25);
    }
}
