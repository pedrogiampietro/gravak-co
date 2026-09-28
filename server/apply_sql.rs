use std::env;
use std::fs;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv()?;
    let db_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite://data/coemu.db?mode=rwc".to_string());
    
    let pool = sqlx::SqlitePool::connect(&db_url).await?;
    
    let sql = fs::read_to_string("maps_portals_clean.sql")?;
    
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