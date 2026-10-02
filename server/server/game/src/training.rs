//! The training ground (map 1039): Stakes and Scarecrows that players hit
//! for experience and proficiency without spending durability, life or mana.
//!
//! Rules from the reference server (`Redux` `CombatManager.IsValidTarget`
//! and `Entity.CalculateExperienceGain`): a dummy can only be trained on
//! when its level is not above the player's, and each hit grants the damage
//! dealt (capped by the dummy's life), scaled by the level difference and
//! divided by ten on the training ground.

use crate::combat::CombatStats;
use crate::entities::Npc;

/// Whether `player_level` may train on `npc`.
pub fn can_train(player_level: u16, npc: &Npc) -> bool {
    npc.is_training_goal() && npc.level() <= player_level
}

/// `CalculateExperienceGain` for a training dummy.
pub fn experience(player_level: u16, dummy_level: u16, dummy_life: u32, damage: u32) -> u64 {
    let mut experience = f64::from(dummy_life.min(damage));
    let delta = i32::from(player_level) - i32::from(dummy_level);
    experience *= match delta {
        3..=5 => 0.7,
        6..=10 => 0.2,
        11..=20 => 0.1,
        21.. => 0.05,
        ..=-16 => 1.8,
        -15..=-9 => 1.5,
        -8..=-6 => 1.3,
        _ => 1.0,
    };
    (experience / 10.0) as u64
}

/// The dummy as a combat target: its defense and life, nothing else.
pub fn dummy_stats(npc: &Npc, magic: bool) -> CombatStats {
    CombatStats {
        min_attack: 0,
        max_attack: 0,
        attack_flat: 0,
        defense: if magic { npc.magic_defense() } else { npc.defense() },
        penetration_percent: 0,
        max_health: npc.life(),
        attack_percent: 0,
        damage_reduction_percent: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::experience;

    #[test]
    fn experience_follows_the_reference_formula() {
        // Same level: a tenth of the damage.
        assert_eq!(experience(65, 65, 10_000, 500), 50);
        // Damage above the dummy's life is capped.
        assert_eq!(experience(65, 65, 100, 500), 10);
        // Trained levels below give less: delta 4 -> 70%, delta 8 -> 20%.
        assert_eq!(experience(69, 65, 10_000, 1_000), 70);
        assert_eq!(experience(73, 65, 10_000, 1_000), 20);
        assert_eq!(experience(100, 65, 10_000, 1_000), 5);
    }
}
