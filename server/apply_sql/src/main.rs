use std::{env, fs};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv()?;
    let db_url = env::var("DATABASE_URL").unwrap_or_else(|_| "postgres://gravak:gravak@localhost:5432/gravak".to_string());

    let pool = sqlx::PgPool::connect(&db_url).await?;

    let sql_path = r"R:\gravak-co\server\maps_portals_clean.sql";
    let sql = fs::read_to_string(sql_path)?;

    for statement in sql.split(';') {
        let stmt = statement.trim();
        if stmt.is_empty() {
            continue;
        }
        // Rows already there are left alone.
        sqlx::query(&format!("{stmt} ON CONFLICT DO NOTHING")).execute(&pool).await?;
    }

    println!("SQL applied successfully!");
    Ok(())
}
