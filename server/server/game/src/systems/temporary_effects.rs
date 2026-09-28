use crate::entities::GameEntity;
use crate::packets::MsgPlayer;
use crate::State;

/// Expires server-authoritative temporary effects and refreshes the affected
/// character for its owner and nearby observers.
pub async fn tick(state: &State) {
    for entity in state.entities() {
        let GameEntity::Character(character) = entity.as_ref() else {
            continue;
        };
        if !character.expire_temporary_effects() {
            continue;
        }

        let update = MsgPlayer::from(character);
        if let Err(error) = character.owner().send(update.clone()).await {
            tracing::warn!(
                character = character.id(),
                ?error,
                "Failed to send expired effect update to owner"
            );
        }
        if let Ok(screen) = character.try_screen() {
            if let Err(error) = screen.send_message(update).await {
                tracing::warn!(
                    character = character.id(),
                    ?error,
                    "Failed to broadcast expired effect update"
                );
            }
        }
        if let Err(error) = character.save(state).await {
            tracing::error!(
                character = character.id(),
                ?error,
                "Failed to persist expired effect state"
            );
        }
    }
}
