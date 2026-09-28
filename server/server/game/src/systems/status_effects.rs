use crate::entities::GameEntity;
use crate::packets::{InteractAction, MsgInteract, MsgPlayer, MsgUserInfo};
use crate::State;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

/// Expires temporary combat statuses and refreshes the affected entity in the
/// owner/observer views. Statuses are runtime-only, so disconnects naturally
/// clear them when the character is recreated.
pub async fn tick(state: &State) {
    let now = Instant::now();
    for entity in state.entities() {
        refresh_entity(state, &entity, now).await;
    }
    for map in state.maps().values() {
        for entity in map.monster_entities() {
            refresh_entity(state, &entity, now).await;
        }
    }
}

async fn refresh_entity(state: &State, entity: &GameEntity, now: Instant) {
    let periodic_damage = entity.tick_statuses(now);
    let expired = entity.expire_statuses(now);
    if periodic_damage == 0 && !expired {
        return;
    }
    let damage_packet = (periodic_damage > 0).then(|| {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|value| value.as_secs() as u32)
            .unwrap_or_default();
        let location = entity.basic().location();
        MsgInteract {
            timestamp,
            character_id: entity.id(),
            target_id: entity.id(),
            x: location.x,
            y: location.y,
            action: InteractAction::Poison as u32,
            value: periodic_damage,
            reserved: [0; 12],
        }
    });
    match entity {
        GameEntity::Character(character) => {
            let update = MsgPlayer::from(character);
            let _ = character.owner().send(update.clone()).await;
            let _ = character.owner().send(MsgUserInfo::from(character)).await;
            if let Some(packet) = damage_packet.clone() {
                let _ = character.owner().send(packet).await;
            }
            if let Ok(screen) = character.try_screen() {
                let _ = screen.send_message(update).await;
                if let Some(packet) = damage_packet {
                    let _ = screen.send_message(packet).await;
                }
            }
        },
        GameEntity::Monster(monster) => {
            if let Ok(map) = state.try_map(monster.entity().map_id()) {
                let _ = map.broadcast(MsgPlayer::from_monster(monster)).await;
                if let Some(packet) = damage_packet {
                    let _ = map.broadcast(packet).await;
                }
            }
        },
        GameEntity::Npc(..) => {},
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::StatusEffect;
    use std::time::Duration;

    #[test]
    fn status_effect_expires_from_entity_state() {
        let now = Instant::now();
        let effects = crate::entities::StatusEffects::default();
        effects.apply(StatusEffect::Burning, Duration::from_millis(1));
        assert!(effects.contains(StatusEffect::Burning));
        std::thread::sleep(Duration::from_millis(3));
        assert_eq!(
            effects.expire(now + Duration::from_secs(1)),
            vec![StatusEffect::Burning]
        );
        assert!(!effects.contains(StatusEffect::Burning));
    }
}
