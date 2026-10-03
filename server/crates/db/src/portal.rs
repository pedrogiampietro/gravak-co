#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "sqlx", derive(sqlx::FromRow))]
pub struct Portal {
    #[cfg_attr(feature = "sqlx", sqlx(try_from = "i64"))]
    pub id: i32,
    #[cfg_attr(feature = "sqlx", sqlx(try_from = "i64"))]
    pub from_map_id: i32,
    #[cfg_attr(feature = "sqlx", sqlx(try_from = "i64"))]
    pub from_x: i16,
    #[cfg_attr(feature = "sqlx", sqlx(try_from = "i64"))]
    pub from_y: i16,
    #[cfg_attr(feature = "sqlx", sqlx(try_from = "i64"))]
    pub to_map_id: i32,
    #[cfg_attr(feature = "sqlx", sqlx(try_from = "i64"))]
    pub to_x: i16,
    #[cfg_attr(feature = "sqlx", sqlx(try_from = "i64"))]
    pub to_y: i16,
}

#[cfg(feature = "sqlx")]
impl Portal {
    #[tracing::instrument]
    pub async fn by_map(pool: &sqlx::PgPool, from: i32) -> Result<Vec<Self>, crate::Error> {
        use tokio_stream::StreamExt;

        let mut portals = Vec::new();
        let mut s = sqlx::query_as::<_, Self>("SELECT * FROM portals WHERE from_map_id = $1;")
            .bind(from)
            .fetch(pool);
        while let Some(maybe_portal) = s.next().await {
            match maybe_portal {
                Ok(portal) => portals.push(portal),
                Err(error) => {
                    tracing::error!(
                        %error,
                        from_map_id = %from,
                        "Error while loading a portal"
                    );
                },
            }
        }
        Ok(portals)
    }
}
