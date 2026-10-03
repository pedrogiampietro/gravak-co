use parking_lot::Mutex;
use std::collections::HashMap;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StatusEffect {
    Burning,
    Stunned,
    Cyclone,
    Superman,
    Stigma,
    Shield,
    /// Monsters do not see the player.
    Invisibility,
    /// Melee monsters cannot reach the player.
    Flying,
    /// The next physical hit does double damage.
    Intensify,
    /// Accuracy and Star of Accuracy.
    Accuracy,
}

impl StatusEffect {
    /// The status flag the client shows for this effect.
    pub fn flag(self) -> crate::entities::Flags {
        use crate::entities::Flags;
        match self {
            Self::Burning => Flags::POISONED,
            Self::Stunned => Flags::RESTRICT_MOVEMENT,
            Self::Cyclone => Flags::CYCLONE,
            Self::Superman => Flags::SUPERMAN,
            Self::Stigma => Flags::STIGMA,
            Self::Shield => Flags::SHIELD,
            Self::Invisibility => Flags::INVISIBILITY,
            Self::Flying => Flags::FLYING,
            Self::Intensify => Flags::INTENSIFY,
            Self::Accuracy => Flags::STAR_OF_ACCURACY,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct StatusInstance {
    expires_at: Instant,
    next_tick_at: Instant,
}

#[derive(Debug, Default)]
pub struct StatusEffects {
    active: Mutex<HashMap<StatusEffect, StatusInstance>>,
}

impl StatusEffects {
    pub fn apply(&self, effect: StatusEffect, duration: Duration) {
        self.apply_at(effect, duration, Instant::now());
    }

    fn apply_at(&self, effect: StatusEffect, duration: Duration, now: Instant) {
        if duration.is_zero() {
            self.active.lock().remove(&effect);
            return;
        }
        self.active.lock().insert(
            effect,
            StatusInstance {
                expires_at: now + duration,
                next_tick_at: now + Duration::from_secs(1),
            },
        );
    }

    /// Returns effects whose periodic tick is due. Each effect advances by one
    /// interval, so a delayed server tick never applies several damage ticks
    /// at once.
    pub fn tick(&self, now: Instant) -> Vec<StatusEffect> {
        let mut active = self.active.lock();
        let mut due = Vec::new();
        for (effect, status) in active.iter_mut() {
            if status.next_tick_at <= now && now < status.expires_at {
                due.push(*effect);
                status.next_tick_at = now + Duration::from_secs(1);
            }
        }
        due
    }

    /// The effects under way and when each ends.
    pub fn active(&self) -> Vec<(StatusEffect, Instant)> {
        let now = Instant::now();
        self.active
            .lock()
            .iter()
            .filter(|(_, status)| status.expires_at > now)
            .map(|(effect, status)| (*effect, status.expires_at))
            .collect()
    }

    pub fn contains(&self, effect: StatusEffect) -> bool {
        self.active
            .lock()
            .get(&effect)
            .is_some_and(|status| status.expires_at > Instant::now())
    }

    pub fn expire(&self, now: Instant) -> Vec<StatusEffect> {
        let mut active = self.active.lock();
        let expired = active
            .iter()
            .filter_map(|(effect, status)| (status.expires_at <= now).then_some(*effect))
            .collect::<Vec<_>>();
        for effect in &expired {
            active.remove(effect);
        }
        expired
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn burning_ticks_once_per_second_until_expiration() {
        let effects = StatusEffects::default();
        let applied_at = Instant::now();
        effects.apply_at(StatusEffect::Burning, Duration::from_secs(3), applied_at);

        assert!(effects.tick(applied_at + Duration::from_millis(999)).is_empty());
        assert_eq!(
            effects.tick(applied_at + Duration::from_secs(1)),
            vec![StatusEffect::Burning]
        );
        assert!(effects.tick(applied_at + Duration::from_secs(1)).is_empty());
        assert_eq!(
            effects.tick(applied_at + Duration::from_secs(2)),
            vec![StatusEffect::Burning]
        );
        assert!(effects.tick(applied_at + Duration::from_secs(3)).is_empty());
    }
}
