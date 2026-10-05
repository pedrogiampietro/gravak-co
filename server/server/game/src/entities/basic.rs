use std::sync::atomic::{AtomicU16, AtomicU32, AtomicU64};

use atomic::{Atomic, Ordering};
use primitives::{Gauge, Location};

use crate::constants;

bitflags::bitflags! {
  /// These values can be found in `statuseffect.ini` in the `ini` folder of the client.
  /// These values stacked as bitflags to create a composite status effect on the player.
  #[repr(transparent)]
  #[derive(Copy, Clone)]
  pub struct Flags: u64 {
    const NONE = 0;
    const BLUE_FLASHING_NAME = 1 << 0;
    const POISONED = 1 << 1;
    const REMOVE_MESH = 1 << 2;
    const XP_CIRCLE = 1 << 4;
    /// Gravak runtime flag for the classic Heaven's Blessing timer. It uses
    /// an extension bit so it cannot collide with the 5065 status flags.
    const HEAVEN_BLESSING = 1 << 49;
    /// Gravak runtime flag for double experience, so the XP circle (bit 4)
    /// keeps its classic meaning: XP skills ready.
    const DOUBLE_EXPERIENCE = 1 << 48;
    /// Gravak runtime flag: a VIP account; clients draw its badge.
    const VIP = 1 << 46;
    /// Gravak runtime flag: Intensify is charged for the next shot.
    const INTENSIFY = 1 << 47;
    const RESTRICT_MOVEMENT = 1 << 5;
    const TEAM_LEADER = 1 << 6;
    const STAR_OF_ACCURACY = 1 << 7;
    const SHIELD = 1 << 8;
    const STIGMA = 1 << 9;
    const DEAD = 1 << 10;
    const FADE_OUT = 1 << 11;
    const AZURE_SHIELD = 1 << 12;
    const RED_NAME = 1 << 14;
    const BLACK_NAME = 1 << 15;
    const SUPERMAN = 1 << 18;
    const THIRD_METEMPSYCHOSIS = 1 << 19;
    const FORTH_METEMPSYCHOSIS = 1 << 20;
    const FIFTH_METEMPSYCHOSIS = 1 << 21;
    const INVISIBILITY = 1 << 22;
    const CYCLONE = 1 << 23;
    const SIXTH_METEMPSYCHOSIS = 1 << 24;
    const SEVENTH_METEMPSYCHOSIS = 1 << 25;
    const EIGHTH_METEMPSYCHOSIS = 1 << 26;
    const FLYING = 1 << 27;
    const NINTH_METEMPSYCHOSIS = 1 << 28;
    const TENTH_METEMPSYCHOSIS = 1 << 29;
    const CASTING_PRAY = 1 << 30;
    const PRAYING = 1 << 31;
  }
}

/// A More Advanced Entity Used to be composed with Other Entites Like Player or
/// Monster.
#[derive(Debug)]
pub struct Entity {
    // *** Basic Entity Props ***
    /// Entity Identity in the game world .. Unique over the all game.
    id: u32,
    /// How that entity looks like?
    mesh: AtomicU32,
    /// Could be player name, Monster name .. or anything.
    name: String,
    /// The Current MapID of that entity.
    map_id: AtomicU32,
    /// Current Location (X, Y, Direction)
    location: Atomic<Location>,

    // *** Advanced Entity Props ***
    /// Set of flags shows the current entity status.
    flags: AtomicU64,
    /// Current Entity Level.
    level: AtomicU16,
    /// What Action this entity is doing right now?
    action: AtomicU16,
    /// Old MapID
    prev_map_id: AtomicU32,
    /// The Old Location .. used in calculations with the new location.
    prev_location: Atomic<Location>,
    /// Health Points
    hp: Atomic<Gauge>,
    /// Mana Points
    mp: Atomic<Gauge>,
    base_hp_max: AtomicU16,
    base_mp_max: AtomicU16,
    /// Life and mana the worn equipment adds on top of the base.
    equipment_hp_bonus: AtomicU32,
    equipment_mp_bonus: AtomicU32,
}

pub(crate) fn max_mana_for(spirit: u16, profession: u16) -> u16 {
    let base = u32::from(spirit).saturating_mul(5);
    base.saturating_mul(u32::from(taoist_mana_bonus_percent(profession)))
        .saturating_div(100)
        .min(u32::from(u16::MAX)) as u16
}

/// Returns the classic mana multiplier for Water/Fire Taoist promotions.
/// Profession values use the family in the tens digit and the promotion level
/// in the units digit (for example, 132 = Water Taoist level 2).
pub(crate) fn taoist_mana_bonus_percent(profession: u16) -> u16 {
    let family = profession / 10;
    if !matches!(family, 13 | 14) {
        return 100;
    }
    [100, 100, 300, 400, 500, 600]
        .get(usize::from(profession % 10))
        .copied()
        .unwrap_or(100)
}

pub(crate) fn mana_per_spirit(profession: u16) -> u16 {
    max_mana_for(1, profession)
}

/// Returns the classic Trojan life multiplier for a profession value.
///
/// Conquer stores the profession family in the tens digit and its promotion
/// level in the units digit (10 = Trojan level 0, 11 = Trojan level 1, ...).
/// Redux applies this table to the complete attribute-derived life capacity.
pub(crate) fn trojan_life_bonus_percent(profession: u16) -> u16 {
    if profession / 10 != 1 {
        return 100;
    }
    [100, 105, 108, 110, 112, 115]
        .get(usize::from(profession % 10))
        .copied()
        .unwrap_or(100)
}

pub(crate) fn health_from_attributes(strength: u16, agility: u16, vitality: u16, spirit: u16, profession: u16) -> u16 {
    let base = u32::from(strength)
        .saturating_mul(3)
        .saturating_add(u32::from(agility).saturating_mul(3))
        .saturating_add(u32::from(spirit).saturating_mul(3))
        .saturating_add(u32::from(vitality).saturating_mul(24));
    base.saturating_mul(u32::from(trojan_life_bonus_percent(profession)))
        .saturating_div(100)
        .min(u32::from(u16::MAX)) as u16
}

impl Entity {
    pub fn from_monster(
        id: u32,
        monster_type: &tq_db::monster::MonsterType,
        spawn: &tq_db::monster::MonsterSpawn,
    ) -> Self {
        Self {
            id,
            mesh: AtomicU32::new(monster_type.look as u32),
            name: monster_type.name.clone(),
            map_id: AtomicU32::new(spawn.map_id as u32),
            location: Atomic::new(Location::new(spawn.x as u16, spawn.y as u16, 0)),
            flags: AtomicU64::new(Flags::NONE.bits()),
            level: AtomicU16::new(monster_type.level.max(1) as u16),
            action: AtomicU16::new(100),
            prev_map_id: AtomicU32::new(spawn.map_id as u32),
            prev_location: Atomic::new(Location::new(spawn.x as u16, spawn.y as u16, 0)),
            hp: Atomic::new(Gauge {
                current: monster_type.life.max(1).min(u16::MAX as i32) as u16,
                max: monster_type.life.max(1).min(u16::MAX as i32) as u16,
            }),
            mp: Atomic::new(Gauge::default()),
            base_hp_max: AtomicU16::new(monster_type.life.max(1).min(u16::MAX as i32) as u16),
            base_mp_max: AtomicU16::new(0),
            equipment_hp_bonus: AtomicU32::new(0),
            equipment_mp_bonus: AtomicU32::new(0),
        }
    }

    pub fn id(&self) -> u32 {
        self.id
    }

    pub fn is_character(&self) -> bool {
        constants::is_character(self.id)
    }

    pub fn is_npc(&self) -> bool {
        constants::is_npc(self.id)
    }

    pub fn is_monster(&self) -> bool {
        constants::is_monster(self.id)
    }

    pub fn is_pet(&self) -> bool {
        constants::is_pet(self.id)
    }

    pub fn is_call_pet(&self) -> bool {
        constants::is_call_pet(self.id)
    }

    pub fn is_terrain_npc(&self) -> bool {
        constants::is_terrain_npc(self.id)
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn flags(&self) -> Flags {
        Flags::from_bits(self.flags.load(Ordering::Relaxed)).unwrap_or(Flags::NONE)
    }

    pub fn set_flags(&self, flags: Flags) -> &Self {
        self.flags.store(flags.bits(), Ordering::Relaxed);
        self
    }

    pub fn mesh(&self) -> u32 {
        self.mesh.load(Ordering::Relaxed)
    }

    pub fn set_mesh(&self, value: u32) -> &Self {
        self.mesh.store(value, Ordering::Relaxed);
        self
    }

    pub fn map_id(&self) -> u32 {
        self.map_id.load(Ordering::Relaxed)
    }

    pub fn set_map_id(&self, value: u32) -> &Self {
        let prev_map_id = self.map_id();
        self.prev_map_id.store(prev_map_id, Ordering::Relaxed);
        self.map_id.store(value, Ordering::Relaxed);
        self
    }

    pub fn location(&self) -> Location {
        self.location.load(Ordering::Relaxed)
    }

    pub fn set_location(&self, value: Location) -> &Self {
        let prev_location = self.location();
        self.prev_location.store(prev_location, Ordering::Relaxed);
        self.location.store(value, Ordering::Relaxed);
        self
    }

    pub fn level(&self) -> u16 {
        self.level.load(Ordering::Relaxed)
    }

    pub fn set_level(&self, value: u16) -> &Self {
        self.level.store(value, Ordering::Relaxed);
        self
    }

    pub fn action(&self) -> u16 {
        self.action.load(Ordering::Relaxed)
    }

    pub fn set_action(&self, action: u16) -> &Self {
        self.action.store(action, Ordering::Relaxed);
        self
    }

    pub fn prev_map_id(&self) -> u32 {
        self.prev_map_id.load(Ordering::Relaxed)
    }

    pub fn prev_location(&self) -> Location {
        self.prev_location.load(Ordering::Relaxed)
    }

    pub fn hp(&self) -> Gauge {
        self.hp.load(Ordering::Relaxed)
    }

    pub fn apply_damage(&self, damage: u32) -> Gauge {
        let mut health = self.hp();
        health.current = health.current.saturating_sub(damage.min(u16::MAX as u32) as u16);
        self.hp.store(health, Ordering::Relaxed);
        if health.is_empty() {
            self.set_flags(self.flags() | Flags::DEAD);
        }
        health
    }

    pub fn restore_health(&self) -> Gauge {
        let mut health = self.hp();
        health.current = health.max;
        self.hp.store(health, Ordering::Relaxed);
        self.set_flags(self.flags() & !Flags::DEAD);
        health
    }

    pub fn heal_health(&self, amount: u16) -> Gauge {
        let mut health = self.hp();
        health.current = health.current.saturating_add(amount).min(health.max);
        self.hp.store(health, Ordering::Relaxed);
        health
    }

    pub fn mp(&self) -> Gauge {
        self.mp.load(Ordering::Relaxed)
    }

    pub fn restore_mana(&self, amount: u16) -> Gauge {
        let mut mana = self.mp();
        mana.current = mana.current.saturating_add(amount).min(mana.max);
        self.mp.store(mana, Ordering::Relaxed);
        mana
    }

    pub fn spend_mana(&self, amount: u16) -> Option<Gauge> {
        let mut mana = self.mp();
        if mana.current < amount {
            return None;
        }
        mana.current -= amount;
        self.mp.store(mana, Ordering::Relaxed);
        Some(mana)
    }

    /// Local-only test helper used by the skill smoke-test command.
    pub fn set_mana_for_test(&self, maximum: u16) -> Gauge {
        let mana = Gauge {
            current: maximum,
            max: maximum,
        };
        self.mp.store(mana, Ordering::Relaxed);
        mana
    }

    /// New base life and mana capacities (attributes, level, class); the
    /// equipment bonus stays. With `refill` both gauges become full,
    /// otherwise they keep their value inside the new capacity.
    pub fn set_base_resources(&self, health: u16, mana: u16, refill: bool) {
        self.base_hp_max.store(health, Ordering::Relaxed);
        self.base_mp_max.store(mana, Ordering::Relaxed);
        let health_bonus = self.equipment_hp_bonus.load(Ordering::Relaxed);
        let mana_bonus = self.equipment_mp_bonus.load(Ordering::Relaxed);
        self.set_equipment_resource_bonuses(health_bonus, mana_bonus);
        if refill {
            let mut gauge = self.hp();
            gauge.current = gauge.max;
            self.hp.store(gauge, Ordering::Relaxed);
            let mut gauge = self.mp();
            gauge.current = gauge.max;
            self.mp.store(gauge, Ordering::Relaxed);
            self.set_flags(self.flags() & !Flags::DEAD);
        }
    }

    pub fn set_equipment_resource_bonuses(&self, health_bonus: u32, mana_bonus: u32) {
        self.equipment_hp_bonus.store(health_bonus, Ordering::Relaxed);
        self.equipment_mp_bonus.store(mana_bonus, Ordering::Relaxed);
        let max_resource =
            |base: u16, bonus: u32| u32::from(base).saturating_add(bonus).min(u32::from(u16::MAX)) as u16;

        let mut health = self.hp();
        let was_full = health.current >= health.max;
        health.max = max_resource(self.base_hp_max.load(Ordering::Relaxed), health_bonus);
        health.current = if was_full {
            health.max
        } else {
            health.current.min(health.max)
        };
        self.hp.store(health, Ordering::Relaxed);

        let mut mana = self.mp();
        let was_full = mana.current >= mana.max;
        mana.max = max_resource(self.base_mp_max.load(Ordering::Relaxed), mana_bonus);
        mana.current = if was_full { mana.max } else { mana.current.min(mana.max) };
        self.mp.store(mana, Ordering::Relaxed);
    }

    /// Adjust the base resource capacities after a character changes an
    /// attribute. The current value follows the capacity only when the gauge
    /// was full, so spending points never creates free healing.
    pub fn adjust_base_resource_max(&self, health_delta: u32, mana_delta: u32) {
        if health_delta > 0 {
            let old_base = self.base_hp_max.load(Ordering::Relaxed);
            let new_base = old_base.saturating_add(health_delta.min(u32::from(u16::MAX)) as u16);
            let applied = new_base.saturating_sub(old_base);
            self.base_hp_max.store(new_base, Ordering::Relaxed);
            let mut health = self.hp();
            let was_full = health.current >= health.max;
            health.max = health.max.saturating_add(applied);
            if was_full {
                health.current = health.max;
            }
            self.hp.store(health, Ordering::Relaxed);
        }
        if mana_delta > 0 {
            let old_base = self.base_mp_max.load(Ordering::Relaxed);
            let new_base = old_base.saturating_add(mana_delta.min(u32::from(u16::MAX)) as u16);
            let applied = new_base.saturating_sub(old_base);
            self.base_mp_max.store(new_base, Ordering::Relaxed);
            let mut mana = self.mp();
            let was_full = mana.current >= mana.max;
            mana.max = mana.max.saturating_add(applied);
            if was_full {
                mana.current = mana.max;
            }
            self.mp.store(mana, Ordering::Relaxed);
        }
    }

    pub fn is_alive(&self) -> bool {
        !self.flags().contains(Flags::DEAD)
    }

    pub fn is_dead(&self) -> bool {
        self.flags().contains(Flags::DEAD)
    }
}

impl From<&tq_db::character::Character> for Entity {
    fn from(v: &tq_db::character::Character) -> Self {
        // TODO: handle more flags.
        let flags = {
            let f = Flags::NONE;
            match v.kill_points as u16 {
                30..=99 => f | Flags::RED_NAME,
                100.. => f | Flags::BLACK_NAME,
                _ => f,
            };
            f
        };
        // Older/local characters may have been created with a zero persisted
        // health gauge. Reconstruct the base capacity from their attributes
        // so they do not enter the world as an unusable 0/0 character.
        let stored_health = v.health_points.max(0) as u16;
        let derived_health = health_from_attributes(
            v.strength.max(0) as u16,
            v.agility.max(0) as u16,
            v.vitality.max(0) as u16,
            v.spirit.max(0) as u16,
            v.current_class.max(0) as u16,
        );
        // Redux `Recalculate`: the capacity comes from the attributes; the
        // stored value only rescues a character without any attributes.
        let base_health = if derived_health > 0 { derived_health } else { stored_health };
        let base_mana = max_mana_for(v.spirit.max(0) as u16, v.current_class.max(0) as u16);
        Self {
            id: (v.character_id as u32) + constants::CHARACTER_ID_MIN,
            mesh: AtomicU32::new(v.mesh as _),
            name: v.name.clone(),
            map_id: AtomicU32::new(v.map_id as _),
            location: Atomic::new(Location::new(v.x as _, v.y as _, 0)),
            flags: AtomicU64::new(flags.bits()),
            level: AtomicU16::new(v.level as _),
            action: AtomicU16::new(100),
            prev_map_id: AtomicU32::new(v.map_id as _),
            prev_location: Atomic::new(Location::default()),
            hp: Atomic::new(Gauge {
                current: base_health,
                // TODO: handle max hp.
                max: base_health,
            }),
            mp: Atomic::new(Gauge {
                current: (v.mana_points.max(0) as u16).min(base_mana),
                max: base_mana,
            }),
            base_hp_max: AtomicU16::new(base_health),
            base_mp_max: AtomicU16::new(base_mana),
            equipment_hp_bonus: AtomicU32::new(0),
            equipment_mp_bonus: AtomicU32::new(0),
        }
    }
}

impl From<&tq_db::npc::Npc> for Entity {
    fn from(v: &tq_db::npc::Npc) -> Self {
        Self {
            id: (v.id as u32),
            mesh: AtomicU32::new(v.look as _),
            name: v.name.clone(),
            map_id: AtomicU32::new(v.map_id as _),
            location: Atomic::new(Location::new(v.x as _, v.y as _, (v.look % 10) as _)),
            flags: AtomicU64::new(Flags::NONE.bits()),
            level: AtomicU16::new(v.level as _),
            action: AtomicU16::new(100),
            prev_map_id: AtomicU32::new(v.map_id as _),
            prev_location: Atomic::new(Location::default()),
            hp: Atomic::new(Gauge::default()),
            mp: Atomic::new(Gauge::default()),
            base_hp_max: AtomicU16::new(0),
            base_mp_max: AtomicU16::new(0),
            equipment_hp_bonus: AtomicU32::new(0),
            equipment_mp_bonus: AtomicU32::new(0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{health_from_attributes, max_mana_for, taoist_mana_bonus_percent, trojan_life_bonus_percent, Entity};
    use crate::entities::Flags;
    use atomic::Atomic;
    use primitives::{Gauge, Location};
    use std::sync::atomic::{AtomicU16, AtomicU32, AtomicU64};

    #[test]
    fn mana_capacity_matches_class_family() {
        assert_eq!(max_mana_for(10, 100), 50);
        assert_eq!(taoist_mana_bonus_percent(100), 100);
        assert_eq!(taoist_mana_bonus_percent(131), 100);
        assert_eq!(taoist_mana_bonus_percent(132), 300);
        assert_eq!(taoist_mana_bonus_percent(133), 400);
        assert_eq!(taoist_mana_bonus_percent(134), 500);
        assert_eq!(taoist_mana_bonus_percent(135), 600);
        assert_eq!(taoist_mana_bonus_percent(141), 100);
        assert_eq!(taoist_mana_bonus_percent(142), 300);
        assert_eq!(max_mana_for(10, 132), 150);
        assert_eq!(max_mana_for(10, 135), 300);
        assert_eq!(max_mana_for(0, 135), 0);
    }

    #[test]
    fn trojan_life_bonus_matches_profession_level() {
        assert_eq!(trojan_life_bonus_percent(10), 100);
        assert_eq!(trojan_life_bonus_percent(11), 105);
        assert_eq!(trojan_life_bonus_percent(15), 115);
        assert_eq!(trojan_life_bonus_percent(20), 100);
    }

    #[test]
    fn trojan_life_bonus_scales_attribute_derived_health() {
        assert_eq!(health_from_attributes(4, 6, 12, 0, 10), 318);
        assert_eq!(health_from_attributes(4, 6, 12, 0, 11), 333);
    }

    #[test]
    fn spending_mana_is_atomic_and_rejects_insufficient_cost() {
        let entity = Entity {
            id: 1,
            mesh: AtomicU32::new(0),
            name: "test".to_owned(),
            map_id: AtomicU32::new(1002),
            location: Atomic::new(Location::default()),
            flags: AtomicU64::new(Flags::NONE.bits()),
            level: AtomicU16::new(1),
            action: AtomicU16::new(100),
            prev_map_id: AtomicU32::new(1002),
            prev_location: Atomic::new(Location::default()),
            hp: Atomic::new(Gauge { current: 100, max: 100 }),
            mp: Atomic::new(Gauge { current: 25, max: 50 }),
            base_hp_max: AtomicU16::new(100),
            base_mp_max: AtomicU16::new(50),
            equipment_hp_bonus: AtomicU32::new(0),
            equipment_mp_bonus: AtomicU32::new(0),
        };

        assert_eq!(entity.spend_mana(20).map(|gauge| gauge.current), Some(5));
        assert_eq!(entity.spend_mana(6), None);
        assert_eq!(entity.mp().current, 5);
    }

    #[test]
    fn character_health_defaults_from_attributes_when_persisted_value_is_zero() {
        let character = tq_db::character::Character {
            strength: 4,
            agility: 6,
            vitality: 12,
            spirit: 0,
            health_points: 0,
            ..Default::default()
        };
        let entity = Entity::from(&character);
        assert_eq!(entity.hp().current, 318);
        assert_eq!(entity.hp().max, 318);
    }
}
