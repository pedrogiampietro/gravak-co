use std::{env, fs};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv()?;
    let db_url = env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite://data/coemu.db?mode=rwc".to_string());

    let pool = sqlx::SqlitePool::connect(&db_url).await?;

    let sql_path = r"R:\gravak-co\server\maps_portals_clean.sql";
    let sql = fs::read_to_string(sql_path)?;

    // Use INSERT OR IGNORE to avoid unique constraint errors
    let sql = sql
        .replace("INSERT INTO maps", "INSERT OR IGNORE INTO maps")
        .replace("INSERT INTO portals", "INSERT OR IGNORE INTO portals");

    for statement in sql.split(';') {
        let stmt = statement.trim();
        if stmt.is_empty() {
            continue;
        }
        sqlx::query(stmt).execute(&pool).await?;
    }

    println!("SQL applied successfully!");
    Ok(())
}
