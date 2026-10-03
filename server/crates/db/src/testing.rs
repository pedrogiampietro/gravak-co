//! A fresh database for each test, on the Postgres of `docker compose up -d`
//! (or `TEST_DATABASE_URL`).
//!
//! The migrations run once into a template database named after them; each
//! test then gets a copy of it, which takes milliseconds. Copies older than
//! an hour, left by earlier runs, are dropped as new ones are made.

use sqlx::migrate::Migrator;
use sqlx::postgres::{PgConnection, PgPool, PgPoolOptions};
use sqlx::{Connection, Executor};
use std::sync::atomic::{AtomicU32, Ordering};

static MIGRATOR: Migrator = sqlx::migrate!("../../migrations");
/// Serialises making the template between tests and test processes.
const LOCK: i64 = 0x6772_6176_616b;

/// A new database with every migration applied.
pub async fn pool() -> PgPool {
    let url = std::env::var("TEST_DATABASE_URL").unwrap_or_else(|_| crate::DEFAULT_DATABASE_URL.to_owned());
    let mut admin = PgConnection::connect(&url)
        .await
        .unwrap_or_else(|error| panic!("No Postgres at {url} for the tests ({error}); run `docker compose up -d`"));
    admin
        .execute(format!("SELECT pg_advisory_lock({LOCK})").as_str())
        .await
        .expect("test lock");

    let template = format!("gravak_template_{:016x}", migrations_hash());
    let exists: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM pg_database WHERE datname = $1)")
        .bind(&template)
        .fetch_one(&mut admin)
        .await
        .expect("look up the template");
    if !exists {
        admin
            .execute(format!("CREATE DATABASE {template}").as_str())
            .await
            .expect("create the template");
        let template_pool = PgPoolOptions::new()
            .max_connections(1)
            .connect(&with_database(&url, &template))
            .await
            .expect("connect to the template");
        let migrated = MIGRATOR.run(&template_pool).await;
        template_pool.close().await;
        if let Err(error) = migrated {
            // A half-migrated template would be reused by every later run.
            let _ = admin
                .execute(format!("DROP DATABASE IF EXISTS {template} WITH (FORCE)").as_str())
                .await;
            panic!("migrate the template: {error}");
        }
    }

    drop_stale(&mut admin).await;
    admin
        .execute(format!("SELECT pg_advisory_unlock({LOCK})").as_str())
        .await
        .expect("test unlock");

    // Copies of one template can be made side by side.
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let name = format!(
        "gravak_test_{}_{}_{}",
        now_secs(),
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    );
    admin
        .execute(format!("CREATE DATABASE {name} TEMPLATE {template}").as_str())
        .await
        .expect("copy the template");
    let _ = admin.close().await;

    PgPoolOptions::new()
        .max_connections(8)
        .connect(&with_database(&url, &name))
        .await
        .expect("connect to the test database")
}

/// Drops the copies earlier runs left behind.
async fn drop_stale(admin: &mut PgConnection) {
    let names: Vec<String> = sqlx::query_scalar("SELECT datname FROM pg_database WHERE datname LIKE 'gravak_test_%'")
        .fetch_all(&mut *admin)
        .await
        .unwrap_or_default();
    let hour_ago = now_secs().saturating_sub(3600);
    for name in names {
        let made = name
            .trim_start_matches("gravak_test_")
            .split('_')
            .next()
            .and_then(|secs| secs.parse::<u64>().ok());
        if made.is_some_and(|made| made < hour_ago) {
            let _ = admin
                .execute(format!("DROP DATABASE IF EXISTS {name} WITH (FORCE)").as_str())
                .await;
        }
    }
}

fn migrations_hash() -> u64 {
    // FNV-1a over every migration's version and checksum.
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for migration in MIGRATOR.iter() {
        for byte in migration.version.to_le_bytes().iter().chain(migration.checksum.iter()) {
            hash = (hash ^ u64::from(*byte)).wrapping_mul(0x100_0000_01b3);
        }
    }
    hash
}

/// `url` pointing at `database` instead.
fn with_database(url: &str, database: &str) -> String {
    let (base, query) = url.split_once('?').map_or((url, None), |(base, query)| (base, Some(query)));
    let base = base.rsplit_once('/').map_or(base, |(server, _)| server);
    match query {
        Some(query) => format!("{base}/{database}?{query}"),
        None => format!("{base}/{database}"),
    }
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}
