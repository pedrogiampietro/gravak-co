#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CombatStats {
    pub min_attack: u32,
    pub max_attack: u32,
    pub attack_flat: u32,
    pub defense: u32,
    pub penetration_percent: u16,
    pub max_health: u32,
    pub attack_percent: u16,
    pub damage_reduction_percent: u16,
}

impl CombatStats {
    pub fn normalized(self) -> Self {
        Self {
            min_attack: self.min_attack.min(self.max_attack),
            max_attack: self.max_attack.max(self.min_attack),
            attack_flat: self.attack_flat,
            defense: self.defense,
            penetration_percent: self.penetration_percent.min(100),
            max_health: self.max_health.max(1),
            attack_percent: self.attack_percent.min(100),
            damage_reduction_percent: self.damage_reduction_percent.min(100),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DamageOutcome {
    pub raw_damage: u32,
    pub damage: u32,
    pub remaining_health: u32,
    pub defeated: bool,
}

/// Applies the base power of a skill to the attack range before the normal
/// physical/magical mitigation pipeline.
pub fn scale_skill_power(stats: CombatStats, power_percent: u16) -> CombatStats {
    let scale = u32::from(power_percent.max(1));
    CombatStats {
        min_attack: stats.min_attack.saturating_mul(scale) / 100,
        max_attack: stats.max_attack.saturating_mul(scale) / 100,
        attack_flat: stats.attack_flat.saturating_mul(scale) / 100,
        ..stats
    }
}

pub fn skill_power_percent(base_power: u16, level: u16, proficiency: u32) -> u16 {
    let progression_power = u32::from(level.saturating_sub(1)).saturating_mul(5) + (proficiency.min(100) / 20);
    u16::try_from(u32::from(base_power).saturating_add(progression_power).min(300)).unwrap_or(300)
}

/// Returns the PvE damage multiplier for a character fighting a monster above
/// the normal level band. The classic references begin the advantage after a
/// three-level gap; the cap keeps test commands and extreme level differences
/// from producing unbounded damage.
pub fn level_advantage_percent(attacker_level: u16, defender_level: u16) -> u16 {
    let delta = attacker_level.saturating_sub(defender_level);
    if delta <= 3 {
        return 100;
    }
    let bonus = 50_u32.saturating_add(u32::from(delta.saturating_sub(5)).saturating_mul(14));
    u16::try_from(100_u32.saturating_add(bonus).min(300)).unwrap_or(300)
}

/// Returns the classic PvE physical-damage multiplier for the character
/// family. Archers shoot for two thirds of their attack (Redux
/// `CalculateBowDamage` divides by 150 where melee divides by 100);
/// promoted classes keep their family because their profession remains in
/// the 40..49 range. Other families use the normal melee multiplier.
pub fn pve_physical_class_percent(current_class: u8) -> u16 {
    if (40..50).contains(&current_class) {
        BOW_PERCENT
    } else {
        100
    }
}

/// A bow's share of the attack (`/ 150` against melee's `/ 100`).
pub const BOW_PERCENT: u16 = 67;

/// A bow's share of the attack on another player: a third of that again
/// (Redux `CalculateBowDamage`, `dmg /= 3` on players).
pub const BOW_PVP_PERCENT: u16 = 22;

/// Classic Superman increases physical damage by 10x against monsters and 2x
/// against other characters.
pub fn superman_damage_percent(target_is_monster: bool) -> u16 {
    if target_is_monster {
        1_000
    } else {
        200
    }
}

pub fn resolve_physical_hit(
    attacker: CombatStats,
    defender: CombatStats,
    current_health: u32,
    attack_roll: u32,
) -> DamageOutcome {
    let attacker = attacker.normalized();
    let defender = defender.normalized();
    let attack_range = attacker.max_attack - attacker.min_attack;
    let base_damage = attacker
        .min_attack
        .saturating_add(attacker.attack_flat)
        .saturating_add(attack_roll.min(attack_range));
    let raw_damage = base_damage.saturating_mul(100 + u32::from(attacker.attack_percent)) / 100;
    let effective_defense = defender
        .defense
        .saturating_mul(100 - u32::from(attacker.penetration_percent))
        / 100;
    let mitigated_damage = raw_damage
        .saturating_sub(effective_defense)
        .max(if raw_damage > 0 { 1 } else { 0 });
    let damage = mitigated_damage.saturating_mul(100 - u32::from(defender.damage_reduction_percent)) / 100;
    let damage = damage.max(if raw_damage > 0 { 1 } else { 0 });
    apply_damage(current_health.min(defender.max_health), damage, raw_damage)
}

/// Resolves a magical hit independently from the physical defense pipeline.
/// The values are already assembled by the caller from the skill, equipment,
/// and target state, which keeps physical defense from reducing a spell.
pub fn resolve_magic_damage(
    magic_attack: u32,
    magic_defense: u32,
    magic_resistance_percent: u16,
    damage_reduction_percent: u16,
    current_health: u32,
    max_health: u32,
    fire_resistance_percent: u16,
) -> DamageOutcome {
    let raw_damage = magic_attack;
    let resistance = u32::from(magic_resistance_percent.min(100));
    let resisted_damage = raw_damage.saturating_mul(100 - resistance) / 100;
    let mitigated_damage = resisted_damage
        .saturating_sub(magic_defense)
        .max(if raw_damage > 0 { 1 } else { 0 });
    let reduction = u32::from(damage_reduction_percent.min(100));
    let reduced_damage = mitigated_damage.saturating_mul(100 - reduction) / 100;
    let fire_resistance = u32::from(fire_resistance_percent.min(100));
    let damage = reduced_damage.saturating_mul(100 - fire_resistance) / 100;
    let damage = damage.max(if raw_damage > 0 { 1 } else { 0 });
    apply_damage(current_health.min(max_health.max(1)), damage, raw_damage)
}

fn apply_damage(current_health: u32, damage: u32, raw_damage: u32) -> DamageOutcome {
    let remaining_health = current_health.saturating_sub(damage);
    DamageOutcome {
        raw_damage,
        damage,
        remaining_health,
        defeated: remaining_health == 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hits_land_between_forty_and_a_hundred_percent() {
        assert_eq!(super::hit_chance(0, 0), 90);
        assert_eq!(super::hit_chance(0, 35), 100);
        assert_eq!(super::hit_chance(80, 0), 40);
        // Redux: arrows land 75 - dodge + accuracy, and lose the dodge's share.
        assert_eq!(super::bow_hit_chance(40, 0), 35);
        assert_eq!(super::bow_hit_chance(40, 100), 100);
        assert_eq!(super::after_dodge(100, 40), 60);
        assert_eq!(super::after_dodge(1, 40), 1);
    }

    #[test]
    fn damage_respects_attack_range_and_defense() {
        let outcome = resolve_physical_hit(
            CombatStats {
                min_attack: 20,
                max_attack: 30,
                attack_flat: 0,
                defense: 0,
                penetration_percent: 0,
                max_health: 100,
                attack_percent: 0,
                damage_reduction_percent: 0,
            },
            CombatStats {
                min_attack: 0,
                max_attack: 0,
                attack_flat: 0,
                defense: 8,
                penetration_percent: 0,
                max_health: 100,
                attack_percent: 0,
                damage_reduction_percent: 0,
            },
            100,
            7,
        );

        assert_eq!(outcome.raw_damage, 27);
        assert_eq!(outcome.damage, 19);
        assert_eq!(outcome.remaining_health, 81);
        assert!(!outcome.defeated);
    }

    #[test]
    fn every_positive_hit_deals_at_least_one_damage() {
        let outcome = resolve_physical_hit(
            CombatStats {
                min_attack: 10,
                max_attack: 10,
                attack_flat: 0,
                defense: 0,
                penetration_percent: 0,
                max_health: 3,
                attack_percent: 0,
                damage_reduction_percent: 0,
            },
            CombatStats {
                min_attack: 0,
                max_attack: 0,
                attack_flat: 0,
                defense: 999,
                penetration_percent: 0,
                max_health: 3,
                attack_percent: 0,
                damage_reduction_percent: 0,
            },
            3,
            0,
        );

        assert_eq!(outcome.damage, 1);
        assert_eq!(outcome.remaining_health, 2);
    }

    #[test]
    fn lethal_damage_marks_defeat() {
        let outcome = resolve_physical_hit(
            CombatStats {
                min_attack: 50,
                max_attack: 50,
                attack_flat: 0,
                defense: 0,
                penetration_percent: 0,
                max_health: 100,
                attack_percent: 0,
                damage_reduction_percent: 0,
            },
            CombatStats {
                min_attack: 0,
                max_attack: 0,
                attack_flat: 0,
                defense: 0,
                penetration_percent: 0,
                max_health: 30,
                attack_percent: 0,
                damage_reduction_percent: 0,
            },
            30,
            0,
        );

        assert_eq!(outcome.remaining_health, 0);
        assert!(outcome.defeated);
    }

    #[test]
    fn damage_uses_current_health_for_multi_hit_kills() {
        let stats = CombatStats {
            min_attack: 20,
            max_attack: 20,
            attack_flat: 0,
            defense: 0,
            penetration_percent: 0,
            max_health: 100,
            attack_percent: 0,
            damage_reduction_percent: 0,
        };
        let first = resolve_physical_hit(stats, stats, 33, 0);
        assert_eq!(first.remaining_health, 13);
        assert!(!first.defeated);

        let second = resolve_physical_hit(stats, stats, first.remaining_health, 0);
        assert_eq!(second.remaining_health, 0);
        assert!(second.defeated);
    }

    #[test]
    fn attack_percent_increases_raw_damage() {
        let outcome = resolve_physical_hit(
            CombatStats {
                min_attack: 100,
                max_attack: 100,
                attack_flat: 0,
                defense: 0,
                penetration_percent: 0,
                max_health: 1000,
                attack_percent: 15,
                damage_reduction_percent: 0,
            },
            CombatStats {
                min_attack: 0,
                max_attack: 0,
                attack_flat: 0,
                defense: 0,
                penetration_percent: 0,
                max_health: 1000,
                attack_percent: 0,
                damage_reduction_percent: 0,
            },
            1000,
            0,
        );

        assert_eq!(outcome.raw_damage, 115);
        assert_eq!(outcome.damage, 115);
    }

    #[test]
    fn damage_reduction_percent_reduces_mitigated_damage() {
        let outcome = resolve_physical_hit(
            CombatStats {
                min_attack: 100,
                max_attack: 100,
                attack_flat: 0,
                defense: 0,
                penetration_percent: 0,
                max_health: 1000,
                attack_percent: 0,
                damage_reduction_percent: 0,
            },
            CombatStats {
                min_attack: 0,
                max_attack: 0,
                attack_flat: 0,
                defense: 0,
                penetration_percent: 0,
                max_health: 1000,
                attack_percent: 0,
                damage_reduction_percent: 25,
            },
            1000,
            0,
        );

        assert_eq!(outcome.damage, 75);
    }

    #[test]
    fn damage_reduction_cannot_remove_positive_hit_entirely() {
        let outcome = resolve_physical_hit(
            CombatStats {
                min_attack: 1,
                max_attack: 1,
                attack_flat: 0,
                defense: 0,
                penetration_percent: 0,
                max_health: 100,
                attack_percent: 0,
                damage_reduction_percent: 0,
            },
            CombatStats {
                min_attack: 0,
                max_attack: 0,
                attack_flat: 0,
                defense: 0,
                penetration_percent: 0,
                max_health: 100,
                attack_percent: 0,
                damage_reduction_percent: 100,
            },
            100,
            0,
        );

        assert_eq!(outcome.damage, 1);
    }

    #[test]
    fn refinery_attack_flat_and_penetration_modify_physical_damage() {
        let outcome = resolve_physical_hit(
            CombatStats {
                min_attack: 100,
                max_attack: 100,
                attack_flat: 25,
                defense: 0,
                penetration_percent: 50,
                max_health: 1_000,
                attack_percent: 0,
                damage_reduction_percent: 0,
            },
            CombatStats {
                min_attack: 0,
                max_attack: 0,
                attack_flat: 0,
                defense: 40,
                penetration_percent: 0,
                max_health: 1_000,
                attack_percent: 0,
                damage_reduction_percent: 0,
            },
            1_000,
            0,
        );
        assert_eq!(outcome.raw_damage, 125);
        assert_eq!(outcome.damage, 105);
    }

    #[test]
    fn magic_damage_uses_magic_defense_and_fire_resistance() {
        let resisted = resolve_magic_damage(100, 20, 0, 0, 1_000, 1_000, 25);
        assert_eq!(resisted.raw_damage, 100);
        assert_eq!(resisted.damage, 60);

        let fully_resisted = resolve_magic_damage(100, 0, 0, 0, 1_000, 1_000, 100);
        assert_eq!(fully_resisted.damage, 1);
    }

    #[test]
    fn magic_damage_ignores_physical_defense_and_applies_reduction() {
        let outcome = resolve_magic_damage(100, 15, 0, 25, 1_000, 1_000, 0);
        assert_eq!(outcome.damage, 63);
    }

    #[test]
    fn skill_power_scales_the_attack_range_and_flat_bonus() {
        let stats = CombatStats {
            min_attack: 101,
            max_attack: 199,
            attack_flat: 25,
            defense: 7,
            penetration_percent: 3,
            max_health: 500,
            attack_percent: 4,
            damage_reduction_percent: 5,
        };
        let scaled = scale_skill_power(stats, 125);
        assert_eq!(scaled.min_attack, 126);
        assert_eq!(scaled.max_attack, 248);
        assert_eq!(scaled.attack_flat, 31);
        assert_eq!(scaled.defense, stats.defense);
        assert_eq!(scaled.attack_percent, stats.attack_percent);
    }

    #[test]
    fn skill_progression_increases_power_with_a_safe_cap() {
        assert_eq!(skill_power_percent(110, 1, 0), 110);
        assert_eq!(skill_power_percent(110, 2, 40), 117);
        assert_eq!(skill_power_percent(295, 99, u32::MAX), 300);
    }

    #[test]
    fn level_advantage_is_pve_only_and_has_a_safe_cap() {
        assert_eq!(level_advantage_percent(10, 10), 100);
        assert_eq!(level_advantage_percent(13, 10), 100);
        assert_eq!(level_advantage_percent(14, 10), 150);
        assert_eq!(level_advantage_percent(u16::MAX, 1), 300);
    }

    #[test]
    fn pve_physical_class_modifier_keeps_archer_family_distinct() {
        assert_eq!(pve_physical_class_percent(10), 100);
        assert_eq!(pve_physical_class_percent(20), 100);
        assert_eq!(pve_physical_class_percent(40), 67);
        assert_eq!(pve_physical_class_percent(45), 67);
        assert_eq!(pve_physical_class_percent(100), 100);
    }

    /// A blow takes a fixed number of rolls; whichever one decides a 33%
    /// chance, it comes out at 33%.
    #[test]
    fn every_nth_roll_keeps_the_chance() {
        for stride in 1..=5 {
            let tries = 20_000;
            let hits = (0..tries)
                .map(|_| (0..stride).map(|_| next_roll()).last().expect("a roll"))
                .filter(|roll| roll % 100 < 33)
                .count();
            let percent = hits as f64 * 100.0 / f64::from(tries);
            assert!((percent - 33.0).abs() < 1.5, "every {stride}th roll: {percent:.1}%");
        }
    }

    #[test]
    fn superman_damage_multiplier_matches_classic_paths() {
        assert_eq!(superman_damage_percent(true), 1_000);
        assert_eq!(superman_damage_percent(false), 200);
    }
}
use std::sync::atomic::{AtomicU32, Ordering};

static ROLL_COUNTER: AtomicU32 = AtomicU32::new(0x1357_9bdf);

/// Redux `CalculatePhysicalDamage`: a physical blow lands
/// `max(40, 90 - dodge + accuracy)` percent of the time.
pub fn hit_chance(dodge: u32, accuracy: u32) -> u32 {
    (90 + accuracy).saturating_sub(dodge).clamp(40, 100)
}

/// Redux `CalculateBowDamage`: an arrow lands `75 - dodge + accuracy`
/// percent of the time (never certain to miss).
pub fn bow_hit_chance(dodge: u32, accuracy: u32) -> u32 {
    (75 + accuracy).saturating_sub(dodge).clamp(10, 100)
}

/// An arrow's damage loses the target's dodge share (Redux).
pub fn after_dodge(damage: u32, dodge: u32) -> u32 {
    (damage * 100u32.saturating_sub(dodge.min(90)) / 100).max(u32::from(damage > 0))
}

/// Accuracy and Star of Accuracy raise the hit chance this much.
pub const ACCURACY_STATUS_BONUS: u32 = 20;

/// A random number for combat, drops and forging. The state steps as a
/// linear congruential generator, whose low bits repeat in short cycles
/// (bit k every 2^(k+1) steps), so a `% 100` of it was biased by how many
/// rolls each blow takes: mixed (MurmurHash3's finaliser), every bit is
/// usable. The server seeds it from the clock; tests keep a fixed seed.
pub fn next_roll() -> u32 {
    #[cfg(not(test))]
    {
        static SEEDED: std::sync::Once = std::sync::Once::new();
        SEEDED.call_once(|| {
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |elapsed| elapsed.as_nanos() as u32);
            ROLL_COUNTER.store(nanos ^ 0x1357_9bdf, Ordering::Relaxed);
        });
    }
    let state = ROLL_COUNTER
        .try_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
            Some(value.wrapping_mul(1_664_525).wrapping_add(1_013_904_223))
        })
        .unwrap_or_default();
    mix(state)
}

fn mix(mut value: u32) -> u32 {
    value ^= value >> 16;
    value = value.wrapping_mul(0x85eb_ca6b);
    value ^= value >> 13;
    value = value.wrapping_mul(0xc2b2_ae35);
    value ^ (value >> 16)
}
