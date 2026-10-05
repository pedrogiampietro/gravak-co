use std::collections::HashMap;
use std::sync::Arc;

use crate::Error;
use parking_lot::RwLock;
use sqlx::postgres::{PgPool, PgPoolOptions};

type Shared<T> = Arc<RwLock<T>>;

#[derive(Debug, Clone)]
pub struct State {
    pool: PgPool,
    /// Maps Account ID to Token value
    tokens: Shared<HashMap<i32, u64>>,
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
        let state = Self {
            pool,
            tokens: Default::default(),
        };
        Ok(state)
    }

    /// Get access to the database pool
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub fn tokens(&self) -> &Shared<HashMap<i32, u64>> {
        &self.tokens
    }
}
