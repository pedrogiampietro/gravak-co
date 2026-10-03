use crate::error::Error;
use sqlx::postgres::{PgPool, PgPoolOptions};

#[derive(Debug, Clone)]
pub struct State {
    pool: PgPool,
}

impl State {
    /// Init The State.
    /// Should only get called once.
    pub async fn init() -> Result<Self, Error> {
        let db_url = dotenvy::var("DATABASE_URL").unwrap_or_else(|_| tq_db::DEFAULT_DATABASE_URL.to_owned());
        let pool = PgPoolOptions::new()
            .max_connections(42)
            .min_connections(4)
            .connect(&db_url)
            .await?;
        Ok(Self::with_pool(pool))
    }

    pub fn with_pool(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Get access to the database pool
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}
