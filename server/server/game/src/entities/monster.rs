use crate::combat::CombatStats;
use crate::entities::{Entity, StatusEffect, StatusEffects};
use crate::packets::MsgPlayer;
use crate::Error;
use tq_network::ActorHandle;

#[derive(Debug)]
pub struct Monster {
    inner: tq_db::monster::MonsterType,
    spawn: tq_db::monster::MonsterSpawn,
    entity: Entity,
    combat_stats: CombatStats,
    statuses: StatusEffects,
}

impl Monster {
    pub fn new(id: u32, monster_type: tq_db::monster::MonsterType, spawn: tq_db::monster::MonsterSpawn) -> Self {
        let entity = Entity::from_monster(id, &monster_type, &spawn);
        let combat_stats = CombatStats {
            min_attack: monster_type.attack_min.max(0) as u32,
            max_attack: monster_type.attack_max.max(0) as u32,
            attack_flat: 0,
            defense: monster_type.defense.max(0) as u32,
            penetration_percent: 0,
            max_health: monster_type.life.max(1) as u32,
            attack_percent: 0,
            damage_reduction_percent: 0,
        };
        Self {
            inner: monster_type,
            spawn,
            entity,
            combat_stats,
            statuses: StatusEffects::default(),
        }
    }

    pub fn entity(&self) -> &Entity {
        &self.entity
    }

    pub fn id(&self) -> u32 {
        self.entity.id()
    }

    pub fn combat_stats(&self) -> CombatStats {
        let defense = if self.has_status(StatusEffect::Burning) {
            self.combat_stats.defense.saturating_mul(90) / 100
        } else {
            self.combat_stats.defense
        };
        let defense = if self.has_status(StatusEffect::Shield) {
            defense.saturating_mul(120) / 100
        } else {
            defense
        };
        CombatStats {
            defense,
            ..self.combat_stats
        }
    }

    pub fn apply_status(&self, effect: StatusEffect, duration: std::time::Duration) {
        self.statuses.apply(effect, duration);
        match effect {
            StatusEffect::Burning => {
                self.entity
                    .set_flags(self.entity.flags() | crate::entities::Flags::POISONED);
            },
            StatusEffect::Stunned => {
                self.entity
                    .set_flags(self.entity.flags() | crate::entities::Flags::RESTRICT_MOVEMENT);
            },
            StatusEffect::Cyclone => {
                self.entity
                    .set_flags(self.entity.flags() | crate::entities::Flags::CYCLONE);
            },
            StatusEffect::Superman => {
                self.entity
                    .set_flags(self.entity.flags() | crate::entities::Flags::SUPERMAN);
            },
            StatusEffect::Stigma => {
                self.entity
                    .set_flags(self.entity.flags() | crate::entities::Flags::STIGMA);
            },
            StatusEffect::Shield => {
                self.entity
                    .set_flags(self.entity.flags() | crate::entities::Flags::SHIELD);
            },
        }
    }

    pub fn has_status(&self, effect: StatusEffect) -> bool {
        self.statuses.contains(effect)
    }

    pub fn tick_statuses(&self, now: std::time::Instant) -> u32 {
        let mut damage: u32 = 0;
        for effect in self.statuses.tick(now) {
            if matches!(effect, StatusEffect::Burning) {
                let max_health = u32::from(self.entity.hp().max).max(1);
                let amount = (max_health / 20).max(1);
                self.entity.apply_damage(amount);
                damage = damage.saturating_add(amount);
            }
        }
        damage
    }

    pub fn expire_statuses(&self, now: std::time::Instant) -> bool {
        let expired = self.statuses.expire(now);
        if expired.iter().any(|effect| matches!(effect, StatusEffect::Burning))
            && !self.statuses.contains(StatusEffect::Burning)
        {
            self.entity
                .set_flags(self.entity.flags() & !crate::entities::Flags::POISONED);
        }
        if expired.iter().any(|effect| matches!(effect, StatusEffect::Stunned))
            && !self.statuses.contains(StatusEffect::Stunned)
        {
            self.entity
                .set_flags(self.entity.flags() & !crate::entities::Flags::RESTRICT_MOVEMENT);
        }
        if expired.iter().any(|effect| matches!(effect, StatusEffect::Cyclone))
            && !self.statuses.contains(StatusEffect::Cyclone)
        {
            self.entity
                .set_flags(self.entity.flags() & !crate::entities::Flags::CYCLONE);
        }
        if expired.iter().any(|effect| matches!(effect, StatusEffect::Superman))
            && !self.statuses.contains(StatusEffect::Superman)
        {
            self.entity
                .set_flags(self.entity.flags() & !crate::entities::Flags::SUPERMAN);
        }
        if expired.iter().any(|effect| matches!(effect, StatusEffect::Stigma))
            && !self.statuses.contains(StatusEffect::Stigma)
        {
            self.entity
                .set_flags(self.entity.flags() & !crate::entities::Flags::STIGMA);
        }
        if expired.iter().any(|effect| matches!(effect, StatusEffect::Shield))
            && !self.statuses.contains(StatusEffect::Shield)
        {
            self.entity
                .set_flags(self.entity.flags() & !crate::entities::Flags::SHIELD);
        }
        !expired.is_empty()
    }

    pub fn spawn(&self) -> &tq_db::monster::MonsterSpawn {
        &self.spawn
    }

    pub fn monster_type(&self) -> &tq_db::monster::MonsterType {
        &self.inner
    }

    /// The cell this instance was spawned around.
    pub fn home(&self) -> primitives::Location {
        primitives::Location::new(self.spawn.x.max(0) as u16, self.spawn.y.max(0) as u16, 0)
    }

    /// How far the instance may wander from [`Self::home`].
    pub fn wander_radius(&self) -> u16 {
        self.spawn.radius.clamp(3, 30) as u16
    }

    /// Brings the monster back to life at `location`, normally its home cell
    /// or the nearest free cell to it.
    pub fn respawn_at(&self, location: primitives::Location) {
        self.entity.set_location(location);
        self.entity.restore_health();
    }

    pub(super) async fn send_spawn(&self, to: &ActorHandle) -> Result<(), Error> {
        to.send(MsgPlayer::from_monster(self)).await?;
        Ok(())
    }
}
