//! Stamina and the XP circle, regenerated once a second like the reference
//! server's `Player.On_Player_Timer`.

use std::time::Duration;

use crate::entities::GameEntity;
use crate::packets::{MsgPlayer, MsgUserAttrib, UserAttribute};
use crate::State;

pub const MAX_STAMINA: u8 = 100;
/// Heaven's Blessing raises the stamina cap.
pub const BLESSED_MAX_STAMINA: u8 = 150;
pub const MAX_XP: u8 = 100;
pub const REGENERATION_PERIOD: Duration = Duration::from_secs(1);
/// One XP point every 3 seconds.
pub const XP_PERIOD: Duration = Duration::from_secs(3);
/// How long a lit XP circle waits for an XP skill.
pub const XP_CIRCLE_TIME: Duration = Duration::from_secs(20);
/// `MsgAction` actions that rest faster.
const SIT: u16 = 250;
const LIE: u16 = 14;

/// What one regeneration step changed.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Regeneration {
    pub stamina: bool,
    /// The XP circle lit up or went out.
    pub flags: bool,
}

/// Stamina per second: 3, or 11 while sitting or lying.
pub fn stamina_gain(action: u16) -> u8 {
    if action == SIT || action == LIE {
        11
    } else {
        3
    }
}

pub async fn tick(state: &State) {
    crate::lucky::tick(state).await;
    for entity in state.entities() {
        let GameEntity::Character(character) = entity.as_ref() else {
            continue;
        };
        let change = character.regenerate(character.has_xp_skill(state.skill_catalog()));
        if change.stamina {
            let update = MsgUserAttrib::new(character.id(), UserAttribute::Stamina, u64::from(character.stamina()));
            if let Err(error) = character.owner().send(update).await {
                tracing::debug!(character = character.id(), ?error, "Failed to send stamina");
            }
        }
        // The yellow ring around the orb fills with the XP points.
        if let Some(xp) = character.xp_to_send() {
            let update = MsgUserAttrib::new(character.id(), UserAttribute::Xp, u64::from(xp));
            let _ = character.owner().send(update).await;
        }
        // The HUD counts statuses down; it hears when they change.
        let mut timers = character.status_timers();
        if character.lucky_time() > 0 {
            timers.push((crate::lucky::LUCKY_TIMER_BIT, crate::store::now_secs() + u64::from(character.lucky_time())));
        }
        let vip_until = state.vip().until(character.id());
        if vip_until > crate::store::now_secs() {
            timers.push((crate::entities::Flags::VIP.bits(), vip_until));
        }
        if character.timers_changed(&timers) {
            if let Some((_, end)) = timers.iter().find(|(bit, _)| *bit == crate::lucky::LUCKY_TIMER_BIT) {
                tracing::info!(
                    target: "status",
                    character = %character.entity().name(),
                    now = crate::store::now_secs(),
                    end,
                    "Lucky timer sent"
                );
            }
            let packet = crate::packets::MsgStatusTimers::new(crate::store::now_secs(), &timers);
            let _ = character.owner().send(packet).await;
        }
        if change.flags {
            let update = MsgPlayer::from(character);
            let _ = character.owner().send(update.clone()).await;
            if let Ok(screen) = character.try_screen() {
                let _ = screen.send_message(update).await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sitting_rests_faster() {
        assert_eq!(stamina_gain(100), 3);
        assert_eq!(stamina_gain(SIT), 11);
        assert_eq!(stamina_gain(LIE), 11);
    }
}
