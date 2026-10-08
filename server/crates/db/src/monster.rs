#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "sqlx", derive(sqlx::FromRow))]
pub struct MonsterType {
    #[cfg_attr(feature = "sqlx", sqlx(try_from = "i64"))]
    pub id: i32,
    pub name: String,
    #[cfg_attr(feature = "sqlx", sqlx(try_from = "i64"))]
    pub look: i32,
    #[cfg_attr(feature = "sqlx", sqlx(try_from = "i64"))]
    pub level: i32,
    #[cfg_attr(feature = "sqlx", sqlx(try_from = "i64"))]
    pub life: i32,
    #[cfg_attr(feature = "sqlx", sqlx(try_from = "i64"))]
    pub attack_min: i32,
    #[cfg_attr(feature = "sqlx", sqlx(try_from = "i64"))]
    pub attack_max: i32,
    #[cfg_attr(feature = "sqlx", sqlx(try_from = "i64"))]
    pub defense: i32,
    #[cfg_attr(feature = "sqlx", sqlx(try_from = "i64"))]
    pub attack_speed: i32,
    #[cfg_attr(feature = "sqlx", sqlx(try_from = "i64"))]
    pub respawn_seconds: i32,
    /// Tiles a monster sees players from.
    #[cfg_attr(feature = "sqlx", sqlx(try_from = "i64"))]
    pub view_range: i32,
    /// Tiles a monster hits from.
    #[cfg_attr(feature = "sqlx", sqlx(try_from = "i64"))]
    pub attack_range: i32,
    /// TQ attack mode bits; 2 hunts players on sight.
    #[cfg_attr(feature = "sqlx", sqlx(try_from = "i64"))]
    pub attack_mode: i32,
    /// Percent taken off spells.
    #[cfg_attr(feature = "sqlx", sqlx(try_from = "i64"))]
    pub magic_resistance: i32,
    /// Dodge and accuracy (Redux monstertype): how often blows land.
    #[cfg_attr(feature = "sqlx", sqlx(try_from = "i64", default))]
    pub dodge: i32,
    #[cfg_attr(feature = "sqlx", sqlx(try_from = "i64", default))]
    pub accuracy: i32,
    /// The magic it casts (a player magic, for its look and element); 0
    /// when it only strikes.
    #[cfg_attr(feature = "sqlx", sqlx(try_from = "i64", default))]
    pub magic_skill: i32,
}

#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "sqlx", derive(sqlx::FromRow))]
pub struct MonsterSpawn {
    #[cfg_attr(feature = "sqlx", sqlx(try_from = "i64"))]
    pub id: i32,
    #[cfg_attr(feature = "sqlx", sqlx(try_from = "i64"))]
    pub monster_type_id: i32,
    #[cfg_attr(feature = "sqlx", sqlx(try_from = "i64"))]
    pub map_id: i32,
    #[cfg_attr(feature = "sqlx", sqlx(try_from = "i64"))]
    pub x: i16,
    #[cfg_attr(feature = "sqlx", sqlx(try_from = "i64"))]
    pub y: i16,
    #[cfg_attr(feature = "sqlx", sqlx(try_from = "i64"))]
    pub amount: i32,
    #[cfg_attr(feature = "sqlx", sqlx(try_from = "i64"))]
    pub radius: i32,
}

#[cfg(feature = "sqlx")]
impl MonsterSpawn {
    #[tracing::instrument]
    pub async fn by_map(pool: &sqlx::PgPool, map_id: i32) -> Result<Vec<Self>, crate::Error> {
        Ok(sqlx::query_as::<_, Self>(
            "SELECT id, monster_type_id, map_id, x, y, amount, radius \
             FROM monster_spawns WHERE map_id = $1 ORDER BY id",
        )
        .bind(map_id)
        .fetch_all(pool)
        .await?)
    }
}

#[cfg(feature = "sqlx")]
impl MonsterType {
    #[tracing::instrument]
    pub async fn by_ids(pool: &sqlx::PgPool, ids: &[i32]) -> Result<Vec<Self>, crate::Error> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let ids = ids.iter().map(|&id| i64::from(id)).collect::<Vec<_>>();
        let request = sqlx::query_as::<_, Self>(
            "SELECT id, name, look, level, life, attack_min, attack_max, defense, attack_speed, respawn_seconds, view_range, attack_range, attack_mode, magic_resistance, dodge, accuracy, magic_skill FROM monster_types WHERE id = ANY($1) ORDER BY id",
        )
        .bind(ids);
        Ok(request.fetch_all(pool).await?)
    }
}
