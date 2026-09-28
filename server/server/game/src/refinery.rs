#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RefineryDefinition {
    pub item_type: u32,
    pub equipment_slot: u8,
    pub refinery_type: u32,
    pub level: u32,
    pub effect: i32,
}

/// First 5065-compatible refinery materials from the reference data. The
/// table can grow without changing the persisted item effect shape.
pub fn definition(item_type: u32) -> Option<RefineryDefinition> {
    let (equipment_slot, refinery_type, level, effect) = match item_type {
        721_405..=721_409 => (4, 307, item_type - 721_404, 2 + (item_type - 721_405)),
        721_420..=721_424 => (4, 308, item_type - 721_419, 4 + (item_type - 721_420) * 2),
        721_445 => (4, 310, 1, 5),
        721_380..=721_384 => (1, 312, item_type - 721_379, 4 + (item_type - 721_380) * 2),
        724_440..=724_444 => (
            1,
            301,
            item_type - 724_439,
            [200, 500, 1_000, 1_600, 2_500][(item_type - 724_440) as usize],
        ),
        _ => return None,
    };
    Some(RefineryDefinition {
        item_type,
        equipment_slot,
        refinery_type,
        level,
        effect: i32::try_from(effect).ok()?,
    })
}

pub const REFINERY_DURATION_SECS: u64 = 7 * 24 * 60 * 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RefineryEffect {
    pub refinery_type: u32,
    pub level: u32,
    pub effect: i32,
    pub expires_at: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArtifactDefinition {
    pub item_type: u32,
    pub equipment_slot: u8,
    pub level: u32,
    pub effect: i32,
}

/// Initial Dragon Soul artifacts present in the 5065 reference data. Their
/// effect is stored even before the matching elemental-resistance combat
/// formula is added, so applying one never loses state.
pub fn artifact_definition(item_type: u32) -> Option<ArtifactDefinition> {
    let (equipment_slot, level, effect) = match item_type {
        800_401 => (5, 1, 20), // FlameShield: fire resistance
        820_001 => (1, 1, 25), // FlameHat: fire resistance
        822_001 => (3, 1, 30), // FlameArmor: fire resistance
        824_001 => (8, 1, 15), // FlameBoots: fire resistance
        _ => return None,
    };
    Some(ArtifactDefinition {
        item_type,
        equipment_slot,
        level,
        effect,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArtifactEffect {
    pub artifact_type: u32,
    pub level: u32,
    pub effect: i32,
    pub expires_at: u64,
}

#[cfg(test)]
mod tests {
    use super::{artifact_definition, definition};

    #[test]
    fn maps_reference_materials_to_their_equipment_slots() {
        assert_eq!(
            definition(721_405).map(|value| (value.equipment_slot, value.refinery_type, value.effect)),
            Some((4, 307, 2))
        );
        assert_eq!(
            definition(721_424).map(|value| (value.equipment_slot, value.refinery_type, value.effect)),
            Some((4, 308, 12))
        );
        assert_eq!(
            definition(721_380).map(|value| (value.equipment_slot, value.refinery_type, value.effect)),
            Some((1, 312, 4))
        );
        assert_eq!(
            definition(724_444).map(|value| (value.equipment_slot, value.refinery_type, value.effect)),
            Some((1, 301, 2_500))
        );
        assert!(definition(999_999).is_none());
    }

    #[test]
    fn maps_initial_dragon_soul_artifacts() {
        assert_eq!(
            artifact_definition(800_401).map(|value| (value.equipment_slot, value.level, value.effect)),
            Some((5, 1, 20))
        );
        assert_eq!(
            artifact_definition(820_001).map(|value| (value.equipment_slot, value.level, value.effect)),
            Some((1, 1, 25))
        );
        assert!(artifact_definition(999_999).is_none());
    }
}
