//! Lucky time (the original's LuckDiffuse and LuckAbsorb): a character with
//! two rebirths casts Bless and prays, gaining three seconds of lucky time a
//! second; whoever stands within three cells absorbs one a second. Moving
//! stops it, and lucky time otherwise wears off a second at a time.
//!
//! While it lasts (as `StatusTips.ini` describes it): now and then a hit
//! lands twice as hard, a hit taken does no harm, and upgrades succeed more.

use crate::entities::{Character, Flags, GameEntity};
use crate::packets::MsgPlayer;
use crate::State;

/// The Bless skill (MagicType 9876, "Pray for lucky time").
pub const BLESS: u32 = 9_876;
/// Lucky time a character can hold, in seconds.
pub const MAX_LUCKY_TIME: u32 = 7_200;
/// Seconds gained a second by whoever prays, and by whoever absorbs.
const DIFFUSE_GAIN: u32 = 3;
const ABSORB_GAIN: u32 = 1;
/// How far from someone praying luck is absorbed, in cells.
const ABSORB_RANGE: u16 = 3;
/// Chances, in percent, while lucky: a hit dealt twice as hard, a hit taken
/// for nothing.
const LUCKY_STRIKE_PERCENT: u32 = 10;
const LUCKY_DODGE_PERCENT: u32 = 5;
/// A status bit the client's status bar shows lucky time under (not a
/// MsgPlayer flag).
pub const LUCKY_TIMER_BIT: u64 = 1 << 50;

/// Rebirths needed to learn Bless.
const BLESS_REBIRTHS: u8 = 2;

/// A character with two rebirths knows Bless; the skill, if just learned.
pub async fn grant_bless(state: &State, character: &Character) -> Result<bool, crate::Error> {
    if character.rebirths() < BLESS_REBIRTHS || character.has_skill(BLESS) {
        return Ok(false);
    }
    character.learn_skill(state.pool(), BLESS, 0).await?;
    Ok(true)
}

/// Bless: the character starts praying where they stand.
pub async fn bless(state: &State, character: &Character) -> Result<Option<&'static str>, crate::Error> {
    if character.rebirths() < BLESS_REBIRTHS {
        return Ok(Some("Only those reborn twice can pray for luck."));
    }
    if character.entity().is_dead() {
        return Ok(None);
    }
    character.start_praying(true);
    show(character).await;
    let _ = state;
    Ok(Some("You pray for luck. Stay still to store lucky time."))
}

async fn show(character: &Character) {
    let update = MsgPlayer::from(character);
    let _ = character.owner().send(update.clone()).await;
    if let Ok(screen) = character.try_screen() {
        let _ = screen.send_message(update).await;
    }
}

fn close(a: &Character, b: &Character) -> bool {
    let (left, right) = (a.entity().location(), b.entity().location());
    a.entity().map_id() == b.entity().map_id()
        && left.x.abs_diff(right.x) <= ABSORB_RANGE
        && left.y.abs_diff(right.y) <= ABSORB_RANGE
}

/// One second of lucky time for every character: praying, absorbing or
/// wearing off.
pub async fn tick(state: &State) {
    let entities: Vec<std::sync::Arc<GameEntity>> = state.entities();
    let all: Vec<&Character> = entities.iter().filter_map(|entity| entity.as_character()).collect();
    for &me in &all {
        let flags = me.entity().flags();
        let moved = me.prayer_moved();
        if flags.contains(Flags::CASTING_PRAY) {
            if moved || me.entity().is_dead() {
                me.stop_praying();
                show(me).await;
                continue;
            }
            me.add_lucky_time(DIFFUSE_GAIN);
            // Those standing around absorb the luck.
            for &other in &all {
                let other_flags = other.entity().flags();
                if other.id() != me.id()
                    && !other_flags.intersects(Flags::CASTING_PRAY | Flags::PRAYING)
                    && !other.entity().is_dead()
                    && close(me, other)
                {
                    other.start_praying(false);
                    show(other).await;
                }
            }
        } else if flags.contains(Flags::PRAYING) {
            let diffuser_near = all
                .iter()
                .any(|other| other.entity().flags().contains(Flags::CASTING_PRAY) && close(me, other));
            if moved || !diffuser_near || me.entity().is_dead() {
                me.stop_praying();
                show(me).await;
                continue;
            }
            me.add_lucky_time(ABSORB_GAIN);
        } else {
            me.wear_lucky_time();
        }
    }
}

/// Whether a lucky character's hit lands twice as hard.
pub fn lucky_strike(character: &Character) -> bool {
    character.lucky_time() > 0 && crate::combat::next_roll() % 100 < LUCKY_STRIKE_PERCENT
}

/// Whether a lucky character shrugs off a hit.
pub fn lucky_dodge(character: &Character) -> bool {
    character.lucky_time() > 0 && crate::combat::next_roll() % 100 < LUCKY_DODGE_PERCENT
}

/// A hit as luck changes it: twice as hard for a lucky attacker, one point
/// for a lucky target; never past the life the target had.
pub fn empower(
    attacker: Option<&Character>,
    target: Option<&Character>,
    outcome: &mut crate::combat::DamageOutcome,
    life_before: u32,
) {
    let mut damage = outcome.damage;
    if damage > 0 && attacker.is_some_and(lucky_strike) {
        damage = damage.saturating_mul(2);
    }
    if damage > 1 && target.is_some_and(lucky_dodge) {
        damage = 1;
    }
    if damage != outcome.damage {
        let damage = damage.min(life_before);
        outcome.damage = damage;
        outcome.remaining_health = life_before - damage;
        outcome.defeated = outcome.remaining_health == 0;
    }
}

/// An upgrade chance (percent) with luck: a tenth of what is missing more.
pub fn lucky_chance(chance: u32, lucky: bool) -> u32 {
    if lucky {
        (chance + (100_u32.saturating_sub(chance)) / 10).min(100)
    } else {
        chance
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn luck_raises_upgrade_chances_by_a_tenth_of_what_is_missing() {
        assert_eq!(lucky_chance(50, true), 55);
        assert_eq!(lucky_chance(100, true), 100);
        assert_eq!(lucky_chance(10, true), 19);
        assert_eq!(lucky_chance(50, false), 50);
    }
}
