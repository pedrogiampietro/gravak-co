mod map;
pub use map::{next_summoned_id, Map, Maps, MONSTER_RELOCATION_RADIUS};

mod portal;
pub use portal::Portal;

/// Moves a character to `map_id` at `location` the whole way: off the old
/// map, onto the new one, with their screen refreshed (the Conductress way).
pub async fn relocate(
    state: &crate::State,
    character: &crate::entities::Character,
    map_id: u32,
    location: (u16, u16),
) -> Result<(), crate::Error> {
    let Some(entity) = state.entity(character.id()) else {
        return character.teleport(state, map_id, location).await;
    };
    let old_map_id = character.entity().map_id();
    character.teleport(state, map_id, location).await?;
    let map = state.try_map(map_id)?;
    map.insert_entity(entity.clone()).await?;
    // `teleport` already took them off the old map; a second removal on the
    // same map would erase them from the world index.
    if old_map_id != map_id {
        if let Ok(old_map) = state.try_map(old_map_id) {
            old_map.remove_entity(&entity)?;
        }
    }
    character.try_screen()?.load_surroundings(state).await?;
    Ok(())
}
