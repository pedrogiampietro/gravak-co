use crate::entities::WeaponProficiency;
use serde::{Deserialize, Serialize};
use tq_network::PacketID;

/// 5065 weapon proficiency packet (1025): weapon type (`item_type / 1000`,
/// for example 410 for blades), level and experience inside that level.
#[derive(Debug, Clone, Serialize, Deserialize, PacketID)]
#[packet(id = 1025)]
pub struct MsgWeaponSkill {
    pub weapon_type: u32,
    pub level: u32,
    pub experience: u32,
}

impl MsgWeaponSkill {
    /// One packet per weapon type. Proficiency is stored per item family
    /// (`item_type / 10`), so several families can share a type; the most
    /// advanced one is the type's proficiency.
    pub fn for_all(proficiencies: impl IntoIterator<Item = WeaponProficiency>) -> Vec<Self> {
        let mut by_type = std::collections::BTreeMap::<u32, WeaponProficiency>::new();
        for progress in proficiencies {
            let weapon_type = progress.weapon_family / 100;
            let entry = by_type.entry(weapon_type).or_insert(progress);
            if (progress.level, progress.proficiency) > (entry.level, entry.proficiency) {
                *entry = progress;
            }
        }
        by_type
            .into_iter()
            .map(|(weapon_type, progress)| Self::new(weapon_type, progress))
            .collect()
    }

    pub fn new(weapon_type: u32, progress: WeaponProficiency) -> Self {
        Self {
            weapon_type,
            level: u32::from(progress.level),
            experience: progress.proficiency,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tq_network::PacketEncode;

    #[test]
    fn encodes_5065_weapon_skill_body() {
        let packet = MsgWeaponSkill {
            weapon_type: 410,
            level: 20,
            experience: 1_200,
        };
        let (id, bytes) = packet.encode().expect("weapon skill packet encodes");
        assert_eq!(id, 1025);
        assert_eq!(bytes.len(), 12);
        assert_eq!(&bytes[0..4], &410_u32.to_le_bytes());
        assert_eq!(&bytes[4..8], &20_u32.to_le_bytes());
        assert_eq!(&bytes[8..12], &1_200_u32.to_le_bytes());
    }

    #[test]
    fn keeps_the_most_advanced_family_per_type() {
        let progress = |weapon_family, level, proficiency| WeaponProficiency {
            weapon_family,
            level,
            experience: 0,
            proficiency,
        };
        let packets = MsgWeaponSkill::for_all([
            progress(41000, 3, 10),
            progress(41001, 5, 0),
            progress(48000, 1, 7),
        ]);
        assert_eq!(packets.len(), 2);
        assert_eq!((packets[0].weapon_type, packets[0].level), (410, 5));
        assert_eq!((packets[1].weapon_type, packets[1].level, packets[1].experience), (480, 1, 7));
    }
}
