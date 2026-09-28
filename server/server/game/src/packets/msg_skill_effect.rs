use serde::Serialize;
use tq_network::PacketID;

/// One target entry in the classic 1105 spell-effect packet.
///
/// The legacy client reserves 32 bytes per target even though the current
/// client only consumes the target id and damage fields.
#[derive(Debug, Clone, Serialize)]
pub struct SkillTarget {
    pub target_id: u32,
    pub damage: u32,
    pub hit: u32,
    pub activation_type: u32,
    pub activation_value: u32,
    pub reserved: [u8; 12],
}

impl SkillTarget {
    pub fn hit(target_id: u32, damage: u32) -> Self {
        Self {
            target_id,
            damage,
            hit: 1,
            activation_type: 0,
            activation_value: 0,
            reserved: [0; 12],
        }
    }
}

/// Classic 1105 spell effect. The sequence is deliberately lengthless: the
/// packet carries `target_count` immediately before the target records.
#[derive(Debug, Clone, Serialize, PacketID)]
#[packet(id = 1105)]
pub struct MsgSkillEffect {
    pub attacker_id: u32,
    pub target_x: u16,
    pub target_y: u16,
    pub skill_id: u16,
    pub level: u16,
    pub target_count: u32,
    pub targets: Vec<SkillTarget>,
    pub reserved: [u8; 32],
    pub tail: [u8; 8],
}

impl MsgSkillEffect {
    pub fn single_target(attacker_id: u32, skill_id: u16, target_id: u32, damage: u32) -> Self {
        Self::new(attacker_id, skill_id, vec![SkillTarget::hit(target_id, damage)])
    }

    pub fn new(attacker_id: u32, skill_id: u16, targets: Vec<SkillTarget>) -> Self {
        Self {
            attacker_id,
            target_x: 0,
            target_y: 0,
            skill_id,
            level: 1,
            target_count: targets.len() as u32,
            targets,
            reserved: [0; 32],
            tail: [0; 8],
        }
    }
}
