//! Copies every row of the old SQLite database into the Postgres one.
//!
//! ```text
//! cargo run -p sqlite-to-postgres -- [data/coemu.db] [--overwrite]
//! ```
//!
//! The Postgres database (`DATABASE_URL`, else the one of
//! `docker compose up -d`) is migrated first, then emptied and filled with
//! the SQLite rows, so it ends up holding exactly what the old file held.
//! It refuses to empty a database that already has characters unless
//! `--overwrite` is given.

use sqlx::sqlite::{SqliteConnectOptions, SqlitePool};
use sqlx::{Column, Connection, Executor, Row, TypeInfo, ValueRef};
use std::str::FromStr;

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("../../migrations");

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = dotenvy::dotenv();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let overwrite = args.iter().any(|arg| arg == "--overwrite");
    let source = args
        .iter()
        .find(|arg| !arg.starts_with("--"))
        .cloned()
        .unwrap_or_else(|| "data/coemu.db".to_owned());
    let target = std::env::var("DATABASE_URL").unwrap_or_else(|_| tq_db::DEFAULT_DATABASE_URL.to_owned());

    let sqlite = SqlitePool::connect_with(SqliteConnectOptions::from_str(&format!("sqlite://{source}"))?.read_only(true)).await?;
    let mut postgres = sqlx::PgConnection::connect(&target).await?;
    println!("From  {source}\nTo    {}", tq_db::database_label());

    MIGRATOR.run(&mut postgres).await?;
    let characters: i64 = sqlx::query_scalar("SELECT count(*) FROM characters").fetch_one(&mut postgres).await?;
    if characters > 0 && !overwrite {
        return Err(format!(
            "the Postgres database already has {characters} characters; run again with --overwrite to replace everything in it"
        )
        .into());
    }

    let tables: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' AND name <> '_sqlx_migrations' ORDER BY name",
    )
    .fetch_all(&sqlite)
    .await?;

    let mut transaction = postgres.begin().await?;
    // Rows arrive table by table, not in the order their references need.
    transaction.execute("SET LOCAL session_replication_role = replica").await?;
    let target_tables: Vec<String> = sqlx::query_scalar(
        "SELECT table_name::text FROM information_schema.tables
         WHERE table_schema = current_schema() AND table_type = 'BASE TABLE' AND table_name <> '_sqlx_migrations'",
    )
    .fetch_all(&mut *transaction)
    .await?;
    let quoted = target_tables.iter().map(|name| format!("\"{name}\"")).collect::<Vec<_>>().join(", ");
    transaction.execute(format!("TRUNCATE {quoted}").as_str()).await?;

    let mut copied = 0;
    for table in &tables {
        if !target_tables.contains(table) {
            println!("  {table:<32} skipped (no such table in Postgres)");
            continue;
        }
        let columns: Vec<(String, String)> = sqlx::query_as(
            "SELECT column_name::text, format_type(a.atttypid, a.atttypmod)
             FROM information_schema.columns c
             JOIN pg_attribute a ON a.attrelid = c.table_name::regclass AND a.attname = c.column_name
             WHERE c.table_schema = current_schema() AND c.table_name = $1",
        )
        .bind(table)
        .fetch_all(&mut *transaction)
        .await?;
        let rows = sqlx::query(&format!("SELECT * FROM \"{table}\"")).fetch_all(&sqlite).await?;
        let Some(first) = rows.first() else {
            println!("  {table:<32} 0 rows");
            continue;
        };
        // The columns both sides have, each cast from text to its Postgres type.
        let shared: Vec<(usize, &str, &str)> = first
            .columns()
            .iter()
            .filter_map(|column| {
                columns
                    .iter()
                    .find(|(name, _)| name == column.name())
                    .map(|(name, kind)| (column.ordinal(), name.as_str(), kind.as_str()))
            })
            .collect();
        let insert = format!(
            "INSERT INTO \"{table}\" ({}) VALUES ({})",
            shared.iter().map(|(_, name, _)| format!("\"{name}\"")).collect::<Vec<_>>().join(", "),
            shared
                .iter()
                .enumerate()
                .map(|(n, (_, _, kind))| format!("${}::text::{kind}", n + 1))
                .collect::<Vec<_>>()
                .join(", ")
        );
        for row in &rows {
            let mut query = sqlx::query(&insert);
            for (ordinal, _, _) in &shared {
                query = query.bind(text_of(row, *ordinal)?);
            }
            query.execute(&mut *transaction).await?;
        }
        copied += rows.len();
        println!("  {table:<32} {} rows", rows.len());
    }
    sqlx::query("SELECT sync_identity_sequences()").execute(&mut *transaction).await?;
    transaction.commit().await?;
    println!("Copied {copied} rows from {} tables.", tables.len());
    Ok(())
}

/// A SQLite value as the text Postgres casts back to the column's type.
fn text_of(row: &sqlx::sqlite::SqliteRow, ordinal: usize) -> Result<Option<String>, sqlx::Error> {
    let raw = row.try_get_raw(ordinal)?;
    if raw.is_null() {
        return Ok(None);
    }
    let kind = raw.type_info().name().to_owned();
    Ok(Some(match kind.as_str() {
        "INTEGER" => row.try_get::<i64, _>(ordinal)?.to_string(),
        "REAL" => row.try_get::<f64, _>(ordinal)?.to_string(),
        "BLOB" => String::from_utf8_lossy(&row.try_get::<Vec<u8>, _>(ordinal)?).into_owned(),
        _ => row.try_get::<String, _>(ordinal)?,
    }))
}
