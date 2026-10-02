#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "sqlx", derive(sqlx::FromRow))]
pub struct MonsterType {
    pub id: i32,
    pub name: String,
    pub look: i32,
    pub level: i32,
    pub life: i32,
    pub attack_min: i32,
    pub attack_max: i32,
    pub defense: i32,
    pub attack_speed: i32,
    pub respawn_seconds: i32,
    /// Tiles a monster sees players from.
    pub view_range: i32,
    /// Tiles a monster hits from.
    pub attack_range: i32,
    /// TQ attack mode bits; 2 hunts players on sight.
    pub attack_mode: i32,
    /// Percent taken off spells.
    pub magic_resistance: i32,
}

#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "sqlx", derive(sqlx::FromRow))]
pub struct MonsterSpawn {
    pub id: i32,
    pub monster_type_id: i32,
    pub map_id: i32,
    pub x: i16,
    pub y: i16,
    pub amount: i32,
    pub radius: i32,
}

#[cfg(feature = "sqlx")]
impl MonsterSpawn {
    #[tracing::instrument]
    pub async fn by_map(pool: &sqlx::SqlitePool, map_id: i32) -> Result<Vec<Self>, crate::Error> {
        Ok(sqlx::query_as::<_, Self>(
            "SELECT id, monster_type_id, map_id, x, y, amount, radius \
             FROM monster_spawns WHERE map_id = ? ORDER BY id",
        )
        .bind(map_id)
        .fetch_all(pool)
        .await?)
    }
}

#[cfg(feature = "sqlx")]
impl MonsterType {
    #[tracing::instrument]
    pub async fn by_ids(pool: &sqlx::SqlitePool, ids: &[i32]) -> Result<Vec<Self>, crate::Error> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let placeholders = std::iter::repeat("?").take(ids.len()).collect::<Vec<_>>().join(",");
        let query = format!(
            "SELECT id, name, look, level, life, attack_min, attack_max, defense, attack_speed, respawn_seconds, view_range, attack_range, attack_mode, magic_resistance \
             FROM monster_types WHERE id IN ({placeholders}) ORDER BY id"
        );
        let mut request = sqlx::query_as::<_, Self>(&query);
        for id in ids {
            request = request.bind(id);
        }
        Ok(request.fetch_all(pool).await?)
    }
}
