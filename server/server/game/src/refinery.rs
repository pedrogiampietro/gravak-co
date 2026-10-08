#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RefineryDefinition {
    pub item_type: u32,
    /// The equipment the material fits (see [`fits`]).
    pub subtype: u32,
    pub refinery_type: u32,
    pub level: u32,
    pub effect: i32,
}

/// The refinery kinds (TQ `refinerytype.type`).
pub const INTENSIFICATION: u32 = 301;
pub const DETOXICATION: u32 = 304;
pub const CRITICAL_STRIKE: u32 = 307;
pub const SKILL_CRITICAL_STRIKE: u32 = 308;
pub const IMMUNITY: u32 = 309;
pub const BREAKTHROUGH: u32 = 310;
pub const COUNTERACTION: u32 = 311;
pub const PENETRATION: u32 = 312;
pub const BLOCK: u32 = 313;
/// Resistances to Metal, Wood, Water, Fire and Earth, in [`Element`] order.
pub const RESIST_METAL: u32 = 314;
pub const RESIST_EARTH: u32 = 318;
pub const MAGIC_DEFENSE: u32 = 319;

/// The refinery materials of the reference server (Albetros `refinerytype`):
/// (item type, equipment subtype, kind, level, effect). Both of its copies
/// of each material (7213xx/7214xx and 7243xx/7244xx...) are kept.
const TABLE: &[(u32, u32, u32, u32, i32)] = &[
    (721350, 120, 304, 3, 11),
    (721351, 120, 304, 4, 13),
    (721352, 120, 304, 5, 15),
    (721358, 900, 313, 1, 6),
    (721359, 900, 313, 2, 8),
    (721360, 900, 313, 3, 10),
    (721361, 900, 313, 4, 12),
    (721362, 900, 313, 5, 15),
    (721373, 121, 304, 1, 8),
    (721374, 121, 304, 2, 9),
    (721375, 121, 304, 3, 11),
    (721376, 121, 304, 4, 13),
    (721377, 121, 304, 5, 15),
    (721380, 110, 312, 1, 4),
    (721381, 110, 312, 2, 6),
    (721382, 110, 312, 3, 8),
    (721383, 110, 312, 4, 10),
    (721384, 110, 312, 5, 12),
    (721390, 152, 312, 1, 2),
    (721391, 152, 312, 2, 3),
    (721392, 152, 312, 3, 4),
    (721393, 152, 312, 4, 5),
    (721394, 152, 312, 5, 6),
    (721400, 500, 307, 1, 4),
    (721401, 500, 307, 2, 6),
    (721402, 500, 307, 3, 8),
    (721403, 500, 307, 4, 10),
    (721404, 500, 307, 5, 12),
    (721405, 444, 307, 1, 2),
    (721406, 444, 307, 2, 3),
    (721407, 444, 307, 3, 4),
    (721408, 444, 307, 4, 5),
    (721409, 444, 307, 5, 6),
    (721410, 555, 307, 1, 4),
    (721411, 555, 307, 2, 6),
    (721412, 555, 307, 3, 8),
    (721413, 555, 307, 4, 10),
    (721414, 555, 307, 5, 12),
    (721420, 421, 308, 1, 4),
    (721421, 421, 308, 2, 6),
    (721422, 421, 308, 3, 8),
    (721423, 421, 308, 4, 10),
    (721424, 421, 308, 5, 12),
    (721445, 444, 310, 1, 5),
    (721450, 444, 310, 2, 6),
    (721451, 444, 310, 3, 7),
    (721452, 444, 310, 4, 8),
    (721453, 444, 310, 5, 9),
    (721454, 555, 310, 1, 10),
    (721455, 555, 310, 2, 12),
    (721456, 555, 310, 3, 14),
    (721457, 555, 310, 4, 16),
    (721458, 555, 310, 5, 18),
    (721459, 500, 310, 1, 10),
    (721460, 500, 310, 2, 12),
    (721461, 500, 310, 3, 14),
    (721462, 500, 310, 4, 16),
    (721463, 500, 310, 5, 18),
    (721483, 110, 304, 1, 8),
    (721484, 110, 304, 2, 9),
    (721485, 110, 304, 3, 11),
    (721486, 110, 304, 4, 13),
    (721487, 110, 304, 5, 15),
    (721488, 139, 304, 1, 8),
    (721489, 139, 304, 2, 9),
    (721490, 139, 304, 3, 11),
    (721491, 139, 304, 4, 13),
    (721492, 139, 304, 5, 15),
    (721493, 160, 304, 1, 8),
    (721494, 160, 304, 2, 9),
    (721495, 160, 304, 3, 11),
    (721496, 160, 304, 4, 13),
    (721497, 160, 304, 5, 15),
    (721498, 120, 304, 1, 8),
    (721499, 120, 304, 2, 9),
    (724350, 120, 304, 3, 11),
    (724351, 120, 304, 4, 13),
    (724352, 120, 304, 5, 15),
    (724353, 110, 313, 1, 5),
    (724354, 110, 313, 2, 6),
    (724355, 110, 313, 3, 7),
    (724356, 110, 313, 4, 8),
    (724357, 110, 313, 5, 10),
    (724358, 900, 313, 1, 6),
    (724359, 900, 313, 2, 8),
    (724360, 900, 313, 3, 10),
    (724361, 900, 313, 4, 12),
    (724362, 900, 313, 5, 15),
    (724363, 152, 310, 1, 4),
    (724364, 152, 310, 2, 6),
    (724365, 152, 310, 3, 8),
    (724366, 152, 310, 4, 10),
    (724367, 152, 310, 5, 12),
    (724368, 121, 311, 1, 4),
    (724369, 121, 311, 2, 6),
    (724370, 121, 311, 3, 8),
    (724371, 121, 311, 4, 10),
    (724372, 121, 311, 5, 12),
    (724373, 121, 304, 1, 8),
    (724374, 121, 304, 2, 9),
    (724375, 121, 304, 3, 11),
    (724376, 121, 304, 4, 13),
    (724377, 121, 304, 5, 15),
    (724380, 110, 312, 1, 4),
    (724381, 110, 312, 2, 6),
    (724382, 110, 312, 3, 8),
    (724383, 110, 312, 4, 10),
    (724384, 110, 312, 5, 12),
    (724385, 121, 312, 1, 2),
    (724386, 121, 312, 2, 3),
    (724387, 121, 312, 3, 4),
    (724388, 121, 312, 4, 5),
    (724389, 121, 312, 5, 6),
    (724390, 152, 312, 1, 2),
    (724391, 152, 312, 2, 3),
    (724392, 152, 312, 3, 4),
    (724393, 152, 312, 4, 5),
    (724394, 152, 312, 5, 6),
    (724400, 500, 307, 1, 4),
    (724401, 500, 307, 2, 6),
    (724402, 500, 307, 3, 8),
    (724403, 500, 307, 4, 10),
    (724404, 500, 307, 5, 12),
    (724405, 444, 307, 1, 2),
    (724406, 444, 307, 2, 3),
    (724407, 444, 307, 3, 4),
    (724408, 444, 307, 4, 5),
    (724409, 444, 307, 5, 6),
    (724410, 555, 307, 1, 4),
    (724411, 555, 307, 2, 6),
    (724412, 555, 307, 3, 8),
    (724413, 555, 307, 4, 10),
    (724414, 555, 307, 5, 12),
    (724415, 150, 307, 1, 2),
    (724416, 150, 307, 2, 3),
    (724417, 150, 307, 3, 4),
    (724418, 150, 307, 4, 5),
    (724419, 150, 307, 5, 6),
    (724420, 421, 308, 1, 4),
    (724421, 421, 308, 2, 6),
    (724422, 421, 308, 3, 8),
    (724423, 421, 308, 4, 10),
    (724424, 421, 308, 5, 12),
    (724425, 152, 308, 1, 2),
    (724426, 152, 308, 2, 3),
    (724427, 152, 308, 3, 4),
    (724428, 152, 308, 4, 5),
    (724429, 152, 308, 5, 6),
    (724430, 139, 309, 1, 3),
    (724431, 139, 309, 2, 5),
    (724432, 139, 309, 3, 7),
    (724433, 139, 309, 4, 9),
    (724434, 139, 309, 5, 10),
    (724435, 160, 309, 1, 1),
    (724436, 160, 309, 2, 2),
    (724437, 160, 309, 3, 3),
    (724438, 160, 309, 4, 4),
    (724439, 160, 309, 5, 5),
    (724440, 110, 301, 1, 200),
    (724441, 110, 301, 2, 500),
    (724442, 110, 301, 3, 1000),
    (724443, 110, 301, 4, 1600),
    (724444, 110, 301, 5, 2500),
    (724445, 444, 310, 1, 5),
    (724450, 444, 310, 2, 6),
    (724451, 444, 310, 3, 7),
    (724452, 444, 310, 4, 8),
    (724453, 444, 310, 5, 9),
    (724454, 555, 310, 1, 10),
    (724455, 555, 310, 2, 12),
    (724456, 555, 310, 3, 14),
    (724457, 555, 310, 4, 16),
    (724458, 555, 310, 5, 18),
    (724459, 500, 310, 1, 10),
    (724460, 500, 310, 2, 12),
    (724461, 500, 310, 3, 14),
    (724462, 500, 310, 4, 16),
    (724463, 500, 310, 5, 18),
    (724464, 150, 310, 1, 4),
    (724465, 150, 310, 2, 6),
    (724470, 150, 310, 3, 8),
    (724471, 150, 310, 4, 10),
    (724472, 150, 310, 5, 12),
    (724473, 139, 311, 1, 10),
    (724474, 139, 311, 2, 12),
    (724475, 139, 311, 3, 14),
    (724476, 139, 311, 4, 16),
    (724477, 139, 311, 5, 18),
    (724478, 120, 311, 1, 4),
    (724479, 120, 311, 2, 6),
    (724480, 120, 311, 3, 8),
    (724481, 120, 311, 4, 10),
    (724482, 120, 311, 5, 12),
    (724483, 110, 304, 1, 8),
    (724484, 110, 304, 2, 9),
    (724485, 110, 304, 3, 11),
    (724486, 110, 304, 4, 13),
    (724487, 110, 304, 5, 15),
    (724488, 139, 304, 1, 8),
    (724489, 139, 304, 2, 9),
    (724490, 139, 304, 3, 11),
    (724491, 139, 304, 4, 13),
    (724492, 139, 304, 5, 15),
    (724493, 160, 304, 1, 8),
    (724494, 160, 304, 2, 9),
    (724495, 160, 304, 3, 11),
    (724496, 160, 304, 4, 13),
    (724497, 160, 304, 5, 15),
    (724498, 120, 304, 1, 8),
    (724499, 120, 304, 2, 9),
    (724500, 0, 314, 1, 3),
    (724501, 0, 314, 2, 5),
    (724502, 0, 314, 3, 7),
    (724503, 0, 314, 4, 10),
    (724504, 0, 314, 5, 15),
    (724505, 0, 315, 1, 3),
    (724506, 0, 315, 2, 5),
    (724507, 0, 315, 3, 7),
    (724508, 0, 315, 4, 10),
    (724509, 0, 315, 5, 15),
    (724510, 0, 316, 1, 3),
    (724511, 0, 316, 2, 5),
    (724512, 0, 316, 3, 7),
    (724513, 0, 316, 4, 10),
    (724514, 0, 316, 5, 15),
    (724515, 0, 317, 1, 3),
    (724516, 0, 317, 2, 5),
    (724517, 0, 317, 3, 7),
    (724518, 0, 317, 4, 10),
    (724519, 0, 317, 5, 15),
    (724520, 0, 318, 1, 3),
    (724521, 0, 318, 2, 5),
    (724522, 0, 318, 3, 7),
    (724523, 0, 318, 4, 10),
    (724524, 0, 318, 5, 15),
    (725172, 120, 319, 1, 4),
    (725173, 120, 319, 2, 6),
    (725174, 120, 319, 3, 8),
    (725175, 120, 319, 4, 10),
    (725176, 120, 319, 5, 12),
    (725177, 121, 319, 1, 4),
    (725178, 121, 319, 2, 6),
    (725179, 121, 319, 3, 8),
    (725180, 121, 319, 4, 10),
    (725181, 121, 319, 5, 12),
    (725182, 152, 319, 1, 4),
    (725183, 152, 319, 2, 6),
    (725184, 152, 319, 3, 8),
    (725185, 152, 319, 4, 10),
    (725186, 152, 319, 5, 12),
    (725187, 151, 319, 1, 4),
    (725188, 151, 319, 2, 6),
    (725189, 151, 319, 3, 8),
    (725190, 151, 319, 4, 10),
    (725191, 151, 319, 5, 12),
    (725192, 120, 319, 1, 4),
    (725193, 120, 319, 2, 6),
    (725194, 120, 319, 3, 8),
    (725195, 120, 319, 4, 10),
    (725196, 120, 319, 5, 12),
    (725197, 121, 319, 1, 4),
    (725198, 121, 319, 2, 6),
    (725199, 121, 319, 3, 8),
    (725200, 121, 319, 4, 10),
    (725201, 121, 319, 5, 12),
    (725202, 152, 319, 1, 4),
    (725203, 152, 319, 2, 6),
    (725204, 152, 319, 3, 8),
    (725205, 152, 319, 4, 10),
    (725206, 152, 319, 5, 12),
    (725207, 150, 319, 1, 4),
    (725208, 150, 319, 2, 6),
    (725209, 150, 319, 3, 8),
    (725210, 150, 319, 4, 10),
    (725211, 150, 319, 5, 12),
];

pub fn definition(item_type: u32) -> Option<RefineryDefinition> {
    let index = TABLE.binary_search_by_key(&item_type, |row| row.0).ok()?;
    let (item_type, subtype, refinery_type, level, effect) = TABLE[index];
    Some(RefineryDefinition {
        item_type,
        subtype,
        refinery_type,
        level,
        effect,
    })
}

/// Every refinery material, in item type order.
pub fn materials() -> impl Iterator<Item = RefineryDefinition> {
    TABLE.iter().filter_map(|row| definition(row.0))
}

/// Whether a material for `subtype` fits the item `item_type`: 0 fits all,
/// 110 any headgear, 139 any armor, 444 any one-handed weapon, 555 any
/// two-handed weapon but the bow (500), the rest that exact kind (necklace
/// 120, bag 121, ring 150, heavy ring 151, bracelet 152, boots 160,
/// backsword 421, shield 900).
pub fn fits(subtype: u32, item_type: u32) -> bool {
    let kind = item_type / 1_000;
    match subtype {
        0 => true,
        110 => (111..=119).contains(&kind),
        139 => (130..=139).contains(&kind),
        444 => (400..=499).contains(&kind),
        555 => (501..=599).contains(&kind),
        subtype => kind == subtype,
    }
}

/// The five elements a magic and its resistances belong to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Element {
    Metal,
    Wood,
    Water,
    Fire,
    Earth,
}

impl Element {
    pub fn index(self) -> usize {
        self as usize
    }

    pub fn parse(value: &str) -> Option<Self> {
        Some(match value.trim().to_ascii_lowercase().as_str() {
            "metal" => Self::Metal,
            "wood" => Self::Wood,
            "water" => Self::Water,
            "fire" => Self::Fire,
            "earth" => Self::Earth,
            _ => return None,
        })
    }
}

/// What a character's refined equipment adds up to. Percentages are capped
/// where they could make a hero untouchable.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RefineryStats {
    /// Extra life.
    pub intensification: u32,
    /// Less poison damage, in percent.
    pub detoxication: u32,
    /// Chance in percent that a blow is critical (x1.5).
    pub critical: u32,
    /// Chance in percent that a magic is critical (x1.5).
    pub skill_critical: u32,
    /// Chance in percent that a critical against the hero is not.
    pub immunity: u32,
    /// Chance in percent taken off the target's Block.
    pub breakthrough: u32,
    /// Chance in percent taken off the attacker's criticals.
    pub counteraction: u32,
    /// Defense ignored, in percent.
    pub penetration: u32,
    /// Chance in percent that a physical blow is blocked.
    pub block: u32,
    /// Resistance in percent to each element.
    pub resist: [u32; 5],
    /// Magic damage taken off, in percent.
    pub magic_defense: u32,
}

/// The most any chance or resistance from refinery reaches.
pub const CAP_PERCENT: u32 = 60;

impl RefineryStats {
    pub fn add(&mut self, refinery_type: u32, effect: i32) {
        let effect = effect.max(0) as u32;
        let slot = match refinery_type {
            INTENSIFICATION => &mut self.intensification,
            DETOXICATION => &mut self.detoxication,
            CRITICAL_STRIKE => &mut self.critical,
            SKILL_CRITICAL_STRIKE => &mut self.skill_critical,
            IMMUNITY => &mut self.immunity,
            BREAKTHROUGH => &mut self.breakthrough,
            COUNTERACTION => &mut self.counteraction,
            PENETRATION => &mut self.penetration,
            BLOCK => &mut self.block,
            MAGIC_DEFENSE => &mut self.magic_defense,
            kind @ RESIST_METAL..=RESIST_EARTH => &mut self.resist[(kind - RESIST_METAL) as usize],
            _ => return,
        };
        *slot = slot.saturating_add(effect);
    }

    /// Every percentage held under [`CAP_PERCENT`].
    pub fn capped(mut self) -> Self {
        for value in [
            &mut self.detoxication,
            &mut self.critical,
            &mut self.skill_critical,
            &mut self.immunity,
            &mut self.breakthrough,
            &mut self.counteraction,
            &mut self.penetration,
            &mut self.block,
            &mut self.magic_defense,
        ] {
            *value = (*value).min(CAP_PERCENT);
        }
        for value in &mut self.resist {
            *value = (*value).min(CAP_PERCENT);
        }
        self
    }

    pub fn resistance(&self, element: Option<Element>) -> u32 {
        element.map_or(0, |element| self.resist[element.index()])
    }
}

/// Whether a blow (or magic) is critical: the attacker's chance, less the
/// target's Counteraction, then the target's Immunity may still cancel it.
pub fn critical(attacker_chance: u32, counteraction: u32, immunity: u32, roll: u32, second_roll: u32) -> bool {
    let chance = attacker_chance.saturating_sub(counteraction);
    roll % 100 < chance && second_roll % 100 >= immunity
}

/// Whether a physical blow is blocked: the target's Block, less the
/// attacker's Breakthrough.
pub fn blocked(block: u32, breakthrough: u32, roll: u32) -> bool {
    roll % 100 < block.saturating_sub(breakthrough)
}

/// What a refinery kind is called.
pub fn kind_name(refinery_type: u32) -> &'static str {
    match refinery_type {
        INTENSIFICATION => "Intensification",
        DETOXICATION => "Detoxication",
        CRITICAL_STRIKE => "Critical Strike",
        SKILL_CRITICAL_STRIKE => "Skill Critical Strike",
        IMMUNITY => "Immunity",
        BREAKTHROUGH => "Breakthrough",
        COUNTERACTION => "Counteraction",
        PENETRATION => "Penetration",
        BLOCK => "Block",
        314 => "Metal Resistance",
        315 => "Wood Resistance",
        316 => "Water Resistance",
        317 => "Fire Resistance",
        RESIST_EARTH => "Earth Resistance",
        MAGIC_DEFENSE => "Magic Defense",
        _ => "Refinery",
    }
}

/// The effect as players read it: life for Intensification, a percentage
/// for the rest.
pub fn effect_text(refinery_type: u32, effect: i32) -> String {
    if refinery_type == INTENSIFICATION {
        format!("+{effect} HP")
    } else {
        format!("+{effect}%")
    }
}

/// A critical blow or magic does half again as much.
pub const CRITICAL_PERCENT: u32 = 150;

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
    use super::*;

    #[test]
    fn maps_reference_materials_to_what_they_fit() {
        let crit = definition(721_405).unwrap();
        assert_eq!((crit.subtype, crit.refinery_type, crit.level, crit.effect), (444, 307, 1, 2));
        assert_eq!(definition(724_444).map(|value| (value.refinery_type, value.effect)), Some((301, 2_500)));
        assert_eq!(
            definition(721_362).map(|value| (value.subtype, value.refinery_type, value.effect)),
            Some((900, 313, 15))
        );
        assert!(definition(999_999).is_none());
        assert_eq!(materials().count(), 275);
        assert!(fits(444, 410_001) && fits(444, 421_001) && !fits(444, 500_001));
        assert!(fits(555, 510_001) && !fits(555, 500_001) && fits(500, 500_001));
        assert!(fits(110, 117_001) && fits(139, 131_001) && fits(900, 900_001) && !fits(900, 131_001));
        assert!(fits(0, 160_001));
    }

    #[test]
    fn refinery_effects_add_up_and_cap() {
        let mut stats = RefineryStats::default();
        stats.add(BLOCK, 15);
        stats.add(BLOCK, 60);
        stats.add(317, 10);
        stats.add(INTENSIFICATION, 2_500);
        let stats = stats.capped();
        assert_eq!(stats.block, CAP_PERCENT);
        assert_eq!(stats.resistance(Some(Element::Fire)), 10);
        assert_eq!(stats.resistance(Some(Element::Water)), 0);
        assert_eq!(stats.intensification, 2_500);
        assert!(blocked(30, 10, 19) && !blocked(30, 10, 20));
        assert!(critical(10, 0, 0, 9, 0) && !critical(10, 5, 0, 5, 0) && !critical(10, 0, 50, 0, 10));
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
