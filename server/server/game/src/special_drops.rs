//! Special drops: what a kill may leave beyond the shared tables. Refinery
//! materials come only from here: every boss drops one of the strong
//! levels (3 to 5), sometimes a weak one too, and strong monsters now and
//! then drop a weak one (level 1 from level 30, up to 2 from 70, up to 3
//! from 100).

/// One kill in this many of a strong monster drops a refinery material.
pub const MATERIAL_ODDS: u32 = 1_500;
/// The monster level from which refinery materials drop at all.
pub const MATERIAL_MIN_LEVEL: u16 = 30;
/// A boss's second, weaker material, in percent.
pub const BOSS_SECOND_PERCENT: u32 = 25;

/// The highest material level a monster of `level` drops.
fn top_material_level(level: u16) -> u32 {
    match level {
        0..=69 => 1,
        70..=99 => 2,
        _ => 3,
    }
}

/// A refinery material of a level within `levels`, if one exists.
fn material(levels: std::ops::RangeInclusive<u32>, mut roll: impl FnMut() -> u32, exists: &impl Fn(u32) -> bool) -> Option<u32> {
    let choices = crate::refinery::materials()
        .filter(|material| levels.contains(&material.level) && exists(material.item_type))
        .map(|material| material.item_type)
        .collect::<Vec<_>>();
    (!choices.is_empty()).then(|| choices[roll() as usize % choices.len()])
}

/// The special drops of a kill of a monster of `level`.
pub fn roll(level: u16, boss: bool, mut roll: impl FnMut() -> u32, exists: impl Fn(u32) -> bool) -> Vec<u32> {
    let mut drops = Vec::new();
    if boss {
        drops.extend(material(3..=5, &mut roll, &exists));
        if roll() % 100 < BOSS_SECOND_PERCENT {
            drops.extend(material(1..=2, &mut roll, &exists));
        }
    } else if level >= MATERIAL_MIN_LEVEL && roll() % MATERIAL_ODDS == 0 {
        drops.extend(material(1..=top_material_level(level), &mut roll, &exists));
    }
    drops
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bosses_always_drop_a_strong_material() {
        let mut next = 0_u32;
        let drops = roll(80, true, || {
            next += 1;
            next
        }, |_| true);
        let first = crate::refinery::definition(drops[0]).expect("a refinery material");
        assert!((3..=5).contains(&first.level));
    }

    #[test]
    fn weak_monsters_drop_none_and_strong_ones_rarely() {
        assert!(roll(20, false, || 0, |_| true).is_empty());
        let drops = roll(75, false, || 0, |_| true);
        let material = crate::refinery::definition(drops[0]).expect("a refinery material");
        assert!(material.level <= 2);
        assert!(roll(75, false, || 1, |_| true).is_empty());
        assert!(roll(75, true, || 0, |_| false).is_empty(), "only known items drop");
    }
}
