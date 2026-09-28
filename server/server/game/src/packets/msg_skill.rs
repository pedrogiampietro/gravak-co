use crate::entities::SkillProgress;
use serde::{Deserialize, Serialize};
use tq_network::PacketID;

/// 5065 skill learned/update packet (1103).
#[derive(Debug, Clone, Serialize, Deserialize, PacketID)]
#[packet(id = 1103)]
pub struct MsgSkill {
    pub experience: u32,
    pub skill_id: u16,
    pub level: u16,
    pub reserved: [u8; 10],
}

impl From<SkillProgress> for MsgSkill {
    fn from(skill: SkillProgress) -> Self {
        Self {
            experience: skill.experience.min(u64::from(u32::MAX)) as u32,
            skill_id: skill.skill_id.min(u32::from(u16::MAX)) as u16,
            level: skill.level,
            reserved: [0; 10],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::MsgSkill;
    use tq_network::PacketEncode;

    #[test]
    fn encodes_5065_skill_update_body() {
        let packet = MsgSkill {
            experience: 123,
            skill_id: 1000,
            level: 2,
            reserved: [0; 10],
        };
        let (id, bytes) = packet.encode().expect("skill packet encodes");
        assert_eq!(id, 1103);
        assert_eq!(bytes.len(), 18);
        assert_eq!(&bytes[0..4], &123_u32.to_le_bytes());
        assert_eq!(&bytes[4..6], &1000_u16.to_le_bytes());
        assert_eq!(&bytes[6..8], &2_u16.to_le_bytes());
    }
}
