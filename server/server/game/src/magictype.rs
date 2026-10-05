//! Skill levels as the original server knows them (`cq_magictype`, from
//! Redux's database): what each level of each skill costs and does.
//! MagicType levels start at 0, the server's at 1.

use std::collections::HashMap;
use std::sync::OnceLock;

/// One level of one skill.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MagicLevel {
    /// What kind of cast it is (`Sort`: 1 a hit, 4 a sector, 5 around the
    /// caster, 6 a status, 14 a line...).
    pub kind: u32,
    pub mana: u32,
    /// Below 30000 the magic it adds (or the life a cure gives); from
    /// 30000 the percent of the physical attack it deals (`% 1000`).
    pub power: i32,
    /// The chance it lands, or a weapon skill goes off, in percent.
    pub percent: u32,
    /// How long its status lasts, in seconds.
    pub step_secs: u32,
    /// The cells it covers: a line's length, a sector's or circle's radius.
    pub range: u32,
    /// How far away it can be cast.
    pub distance: u32,
    /// The character level needed to raise it past this level; 0 at the top.
    pub need_level: u32,
    pub stamina: u32,
}

/// The table, read once.
fn table() -> &'static HashMap<(u32, u32), MagicLevel> {
    static TABLE: OnceLock<HashMap<(u32, u32), MagicLevel>> = OnceLock::new();
    TABLE.get_or_init(|| parse(include_str!("../data/magictype.dat")))
}

/// Columns: ID Level Type UseMP Power Percent StepSecs Range Distance
/// Status NeedLevel UseStamina ...
fn parse(text: &str) -> HashMap<(u32, u32), MagicLevel> {
    text.lines()
        .filter(|line| !line.starts_with('#') && !line.trim().is_empty())
        .filter_map(|line| {
            let fields: Vec<i64> = line.split_whitespace().map(|field| field.parse().unwrap_or(0)).collect();
            let field = |index: usize| fields.get(index).copied().unwrap_or(0);
            let unsigned = |index: usize| u32::try_from(field(index)).unwrap_or(0);
            Some((
                (unsigned(0), unsigned(1)),
                MagicLevel {
                    kind: unsigned(2),
                    mana: unsigned(3),
                    power: i32::try_from(field(4)).unwrap_or(0),
                    percent: unsigned(5),
                    step_secs: unsigned(6),
                    range: unsigned(7),
                    distance: unsigned(8),
                    need_level: unsigned(10),
                    stamina: unsigned(11),
                },
            ))
        })
        .collect()
}

/// `skill_id` at the server's `level` (1 and up).
pub fn level(skill_id: u32, level: u16) -> Option<MagicLevel> {
    table().get(&(skill_id, u32::from(level.max(1) - 1))).copied()
}

/// Whether a character of `character_level` may raise `skill_id` past the
/// server's `level` (the original's `NeedLevel`); skills it lacks always may.
pub fn may_raise(skill_id: u32, level: u16, character_level: u16) -> bool {
    self::level(skill_id, level).is_none_or(|magic| u32::from(character_level) >= magic.need_level)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_come_from_the_original_table() {
        let thunder = level(1_000, 1).expect("Thunder");
        assert_eq!((thunder.mana, thunder.power, thunder.distance), (1, 7, 9));
        let fire = level(1_001, 4).expect("Fire at its top");
        assert_eq!((fire.mana, fire.power), (32, 380));
        let scatter = level(8_001, 6).expect("Scatter");
        assert_eq!((scatter.power % 1000, scatter.range, scatter.distance), (72, 13, 20));
        assert_eq!(level(1_110, 1).map(|cyclone| cyclone.step_secs), Some(20));
        assert!(level(1_000, 6).is_none(), "Thunder has five levels");
    }

    #[test]
    fn a_skill_waits_for_the_character_level_it_needs() {
        // Thunder: 0, 10, 20, 30 to go past its levels 1 to 4.
        assert!(may_raise(1_000, 1, 1));
        assert!(!may_raise(1_000, 2, 9));
        assert!(may_raise(1_000, 2, 10));
        assert!(!may_raise(1_002, 1, 89), "Tornado at 90");
        assert!(may_raise(9_999, 3, 1), "a skill the table lacks");
    }
}
