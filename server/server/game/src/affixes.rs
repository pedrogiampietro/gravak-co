//! Rarity affixes: what each rarity slot of an item adds, rolled and kept by
//! the server. A rarity gives as many slots as its tier (Rare one, Mythic
//! four); each holds an affix fitting the kind of item, its value rising
//! with the slot.
//!
//! An affix packs into 16 bits, the kind in the top four and the value in
//! the low twelve, which is also how it travels to the client.

/// What an affix adds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AffixKind {
    /// Percent more physical attack.
    Attack = 1,
    /// Percent more magic attack.
    MagicAttack = 2,
    /// Percent more defense.
    Defense = 3,
    /// Points of life.
    MaxHp = 4,
    /// Points of accuracy (blows and arrows land more).
    Accuracy = 5,
    /// Points of dodge.
    Dodge = 6,
    /// Percent less damage taken.
    DamageResist = 7,
    /// Percent of the target's defense ignored.
    Penetration = 8,
}

impl AffixKind {
    pub const ALL: [Self; 8] = [
        Self::Attack,
        Self::MagicAttack,
        Self::Defense,
        Self::MaxHp,
        Self::Accuracy,
        Self::Dodge,
        Self::DamageResist,
        Self::Penetration,
    ];

    pub fn from_code(code: u16) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| *kind as u16 == code)
    }

    /// The values a first slot rolls between; later slots go higher.
    fn range(self) -> (u16, u16) {
        match self {
            Self::Attack | Self::MagicAttack | Self::Defense => (2, 6),
            Self::MaxHp => (60, 300),
            Self::Accuracy | Self::Dodge => (2, 8),
            Self::DamageResist | Self::Penetration => (1, 3),
        }
    }
}

/// One rolled affix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Affix {
    pub kind: AffixKind,
    pub value: u16,
}

impl Affix {
    pub fn pack(self) -> u16 {
        ((self.kind as u16) << 12) | self.value.min(0x0fff)
    }

    pub fn unpack(packed: u16) -> Option<Self> {
        let kind = AffixKind::from_code(packed >> 12)?;
        Some(Self { kind, value: packed & 0x0fff })
    }
}

/// How many affix slots a rarity tier opens (Rare 1 .. Mythic 4).
pub fn slots(rarity: u8) -> usize {
    usize::from(rarity.min(4))
}

/// The affixes that may roll on `item_type`, by the kind of item: weapons
/// strike, armor and shields hold, rings and necklaces serve either, boots
/// and helmets keep their wearer whole and quick. Taoist backswords and
/// wands roll magic too.
pub fn pool(item_type: u32) -> &'static [AffixKind] {
    use AffixKind::*;
    match item_type / 1_000 {
        421 | 561 => &[Attack, MagicAttack, Penetration, Accuracy],
        410..=599 => &[Attack, Penetration, Accuracy],
        900 => &[Defense, DamageResist, MaxHp],
        110..=119 | 140..=149 => &[Defense, MaxHp, Dodge],
        120..=121 => &[MaxHp, MagicAttack, Defense],
        130..=139 => &[Defense, MaxHp, DamageResist],
        150..=152 => &[Attack, MagicAttack, Accuracy],
        160..=169 => &[Dodge, MaxHp, Defense],
        _ => &[],
    }
}

/// Rolls the affix of slot `slot` (0 first) for `item_type`, preferring a
/// kind the item does not have yet. `roll` gives fresh random numbers.
pub fn roll(item_type: u32, slot: usize, existing: &[Affix], mut roll: impl FnMut() -> u32) -> Option<Affix> {
    let pool = pool(item_type);
    if pool.is_empty() {
        return None;
    }
    let fresh: Vec<AffixKind> = pool
        .iter()
        .copied()
        .filter(|kind| !existing.iter().any(|affix| affix.kind == *kind))
        .collect();
    let choices = if fresh.is_empty() { pool.to_vec() } else { fresh };
    let kind = choices[roll() as usize % choices.len()];
    let (low, high) = kind.range();
    let base = u32::from(low) + roll() % u32::from(high - low + 1);
    // Each later slot rolls a quarter higher (a Mythic's fourth: 1.75x).
    let value = base * (100 + 25 * slot as u32) / 100;
    Some(Affix { kind, value: u16::try_from(value.max(1)).unwrap_or(u16::MAX).min(0x0fff) })
}

/// The affixes packed in an item, in slot order.
pub fn unpack_all(packed: &[u16; 4]) -> Vec<Affix> {
    packed.iter().filter_map(|packed| Affix::unpack(*packed)).collect()
}

/// The total an item's packed affixes give of `kind`.
pub fn total(packed: &[u16; 4], kind: AffixKind) -> u32 {
    unpack_all(packed)
        .into_iter()
        .filter(|affix| affix.kind == kind)
        .map(|affix| u32::from(affix.value))
        .sum()
}

/// The cost and odds of the next rarity tier from `rarity`: the catalyst
/// (DragonBall or Super DragonBall), how many, and the chance in percent
/// before the failures' protection.
pub fn upgrade_cost(rarity: u8) -> Option<(u32, u8, u32)> {
    Some(match rarity {
        0 => (crate::drops::DRAGONBALL, 1, 75),
        1 => (crate::drops::DRAGONBALL, 3, 50),
        2 => (SUPER_DRAGONBALL, 1, 35),
        3 => (SUPER_DRAGONBALL, 3, 20),
        _ => return None,
    })
}

/// Every failed attempt on an item adds this much chance to its next one,
/// until it rises.
pub const FAILURE_BONUS: u32 = 5;

/// The Super DragonBall: the catalyst of Legendary and Mythic, and of
/// rerolling a single affix. Made from ten DragonBalls, dropped by bosses
/// and, very rarely, by any monster.
pub const SUPER_DRAGONBALL: u32 = 1_088_003;

/// How many DragonBalls compose a Super DragonBall.
pub const SUPER_DRAGONBALL_RECIPE: u8 = 10;

/// The rarity a piece of equipment falls with: `roll` in 0..10000. Bosses'
/// gear is Epic one time in five and Legendary one in thirty; a monster's is
/// Rare 3% of the time and Epic 0.5%.
pub fn drop_rarity(boss: bool, roll: u32) -> u8 {
    let roll = roll % 10_000;
    if boss {
        match roll {
            0..333 => 3,
            333..2_333 => 2,
            _ => 1,
        }
    } else {
        match roll {
            0..50 => 2,
            50..350 => 1,
            _ => 0,
        }
    }
}

/// A Super DragonBall falls from a monster one time in 20000 (a boss
/// always drops one).
pub fn rolls_super_dragonball(roll: u32) -> bool {
    roll % 20_000 == 0
}

/// Whether `item_type` is equipment that can carry a rarity.
pub fn can_carry_rarity(item_type: u32) -> bool {
    !pool(item_type).is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sequence(values: &[u32]) -> impl FnMut() -> u32 + '_ {
        let mut index = 0;
        move || {
            let value = values[index % values.len()];
            index += 1;
            value
        }
    }

    #[test]
    fn affixes_pack_into_sixteen_bits() {
        let affix = Affix { kind: AffixKind::MaxHp, value: 525 };
        assert_eq!(Affix::unpack(affix.pack()), Some(affix));
        assert_eq!(Affix::unpack(0), None, "an empty slot");
    }

    #[test]
    fn weapons_roll_offense_and_later_slots_roll_higher() {
        let first = roll(410_339, 0, &[], sequence(&[0, 4])).unwrap();
        assert_eq!(first, Affix { kind: AffixKind::Attack, value: 6 });
        let fourth = roll(410_339, 3, &[first], sequence(&[0, 4])).unwrap();
        assert_eq!(fourth.kind, AffixKind::Penetration, "a kind it lacks first");
        assert_eq!(fourth.value, 3, "2 at 1.75x");
        assert!(roll(1_088_000, 0, &[], sequence(&[0])).is_none(), "no affix on a DragonBall");
    }

    #[test]
    fn an_item_sums_its_affixes() {
        let packed = [
            Affix { kind: AffixKind::Attack, value: 4 }.pack(),
            Affix { kind: AffixKind::Attack, value: 3 }.pack(),
            Affix { kind: AffixKind::Accuracy, value: 5 }.pack(),
            0,
        ];
        assert_eq!(total(&packed, AffixKind::Attack), 7);
        assert_eq!(total(&packed, AffixKind::Accuracy), 5);
        assert_eq!(total(&packed, AffixKind::Dodge), 0);
    }

    #[test]
    fn the_ladder_turns_to_super_dragonballs() {
        assert_eq!(upgrade_cost(0), Some((crate::drops::DRAGONBALL, 1, 75)));
        assert_eq!(upgrade_cost(2), Some((SUPER_DRAGONBALL, 1, 35)));
        assert_eq!(upgrade_cost(3), Some((SUPER_DRAGONBALL, 3, 20)));
        assert_eq!(upgrade_cost(4), None);
    }

    #[test]
    fn drops_are_rarely_rare() {
        assert_eq!(drop_rarity(false, 9_999), 0);
        assert_eq!(drop_rarity(false, 100), 1);
        assert_eq!(drop_rarity(false, 10), 2);
        assert_eq!(drop_rarity(true, 9_999), 1, "a boss's gear is at least Rare");
        assert_eq!(drop_rarity(true, 100), 3);
    }
}
