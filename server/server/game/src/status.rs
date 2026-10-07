//! A status line in the log every few minutes: who is online, for how long
//! the server has been up, and how the database answers.

use crate::State;
use std::time::{Duration, Instant};

/// How often the status line is written.
const EVERY: Duration = Duration::from_secs(300);

/// How often the online count is written for the website.
const PUBLISH_EVERY: Duration = Duration::from_secs(30);

/// A character in the world, as the website keeps it.
struct Hero {
    id: i64,
    level: i32,
    total: i64,
    rebirths: i32,
}

/// Writes who is online now and keeps the record; marks each of them seen
/// and their day's experience (`experience_days`, days in UTC).
async fn publish(pool: &sqlx::PgPool, heroes: &[Hero]) -> Result<(), sqlx::Error> {
    let online = i32::try_from(heroes.len()).unwrap_or(i32::MAX);
    if !heroes.is_empty() {
        let ids: Vec<i64> = heroes.iter().map(|hero| hero.id).collect();
        let levels: Vec<i32> = heroes.iter().map(|hero| hero.level).collect();
        let totals: Vec<i64> = heroes.iter().map(|hero| hero.total).collect();
        let rebirths: Vec<i32> = heroes.iter().map(|hero| hero.rebirths).collect();
        sqlx::query("UPDATE characters SET last_seen = EXTRACT(EPOCH FROM now())::BIGINT WHERE character_id = ANY($1)")
            .bind(&ids)
            .execute(pool)
            .await?;
        sqlx::query(
            "INSERT INTO experience_days (character_id, day, start_total, end_total, start_level, end_level, rebirths)
             SELECT id, (now() AT TIME ZONE 'UTC')::date, total, total, level, level, rebirths
               FROM UNNEST($1::BIGINT[], $2::INT[], $3::BIGINT[], $4::INT[]) AS hero(id, level, total, rebirths)
             ON CONFLICT (character_id, day) DO UPDATE SET
                 end_total = excluded.end_total,
                 end_level = excluded.end_level,
                 rebirths = excluded.rebirths",
        )
        .bind(&ids)
        .bind(&levels)
        .bind(&totals)
        .bind(&rebirths)
        .execute(pool)
        .await?;
    }
    sqlx::query(
        "INSERT INTO server_status (id, online, record, updated_at)
         VALUES (1, $1, $1, EXTRACT(EPOCH FROM now())::BIGINT)
         ON CONFLICT (id) DO UPDATE SET
             online = excluded.online,
             record = GREATEST(server_status.record, excluded.online),
             updated_at = excluded.updated_at",
    )
    .bind(online)
    .execute(pool)
    .await?;
    Ok(())
}

/// Starts writing the status line; it runs until the server stops.
pub fn start(state: &'static State, started: Instant) {
    // VIP badges come off when the VIP runs out.
    tokio::spawn(async move {
        let mut ticks = tokio::time::interval(std::time::Duration::from_secs(60));
        loop {
            ticks.tick().await;
            crate::packets::expire_vip(state).await;
        }
    });
    // The website's online count (`server_status`).
    tokio::spawn(async move {
        let mut ticks = tokio::time::interval(PUBLISH_EVERY);
        loop {
            ticks.tick().await;
            let heroes: Vec<Hero> = state
                .entities()
                .iter()
                .filter_map(|entity| entity.as_character())
                .map(|character| Hero {
                    id: i64::from(character.database_id()),
                    level: i32::from(character.entity().level()),
                    total: i64::try_from(crate::leveling::total_experience(
                        u8::try_from(character.entity().level()).unwrap_or(u8::MAX),
                        character.experience(),
                    ))
                    .unwrap_or(i64::MAX),
                    rebirths: i32::from(character.rebirths()),
                })
                .collect();
            if let Err(error) = publish(state.pool(), &heroes).await {
                tracing::warn!(target: "status", %error, "The online count was not published");
            }
        }
    });
    tokio::spawn(async move {
        let mut ticks = tokio::time::interval(EVERY);
        ticks.tick().await;
        loop {
            ticks.tick().await;
            let online = state.entities().iter().filter(|entity| entity.is_character()).count();
            let pool = state.pool();
            let asked = Instant::now();
            match sqlx::query("SELECT 1").execute(pool).await {
                Ok(_) => tracing::info!(
                    target: "status",
                    online,
                    uptime = %uptime(started.elapsed()),
                    db_ms = asked.elapsed().as_millis() as u64,
                    db_connections = pool.size(),
                    db_idle = pool.num_idle(),
                    "Server status"
                ),
                Err(error) => tracing::error!(
                    target: "status",
                    online,
                    uptime = %uptime(started.elapsed()),
                    %error,
                    "Server status: the database does not answer"
                ),
            }
        }
    });
}

/// `2d 3h 14m`, `3h 14m` or `14m`.
fn uptime(elapsed: Duration) -> String {
    let minutes = elapsed.as_secs() / 60;
    let (days, hours, minutes) = (minutes / 1440, minutes / 60 % 24, minutes % 60);
    match (days, hours) {
        (0, 0) => format!("{minutes}m"),
        (0, _) => format!("{hours}h {minutes}m"),
        _ => format!("{days}d {hours}h {minutes}m"),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn uptime_reads_naturally() {
        use std::time::Duration;
        assert_eq!(super::uptime(Duration::from_secs(14 * 60)), "14m");
        assert_eq!(super::uptime(Duration::from_secs(3 * 3600 + 60)), "3h 1m");
        assert_eq!(super::uptime(Duration::from_secs(2 * 86400 + 3600)), "2d 1h 0m");
    }

    /// The day keeps where the hero started and follows where they are;
    /// the count and its record follow who is online.
    #[tokio::test]
    async fn publishing_keeps_the_day_and_the_record() -> Result<(), crate::Error> {
        let pool = tq_db::testing::pool().await;
        let account = 4_101_i64;
        sqlx::query("INSERT INTO accounts (account_id, username, password) VALUES ($1, 'status-test', 'x') ON CONFLICT DO NOTHING")
            .bind(account)
            .execute(&pool)
            .await?;
        let inner = crate::packets::MsgRegister::build_character_with(
            "status-test".to_owned(),
            crate::packets::BodyType::MuscularMale,
            crate::packets::BaseClass::Trojan,
            account as u32,
            1,
        )?;
        inner.save(&pool).await?;
        let id: i64 = sqlx::query_scalar("SELECT character_id FROM characters WHERE name = 'status-test'")
            .fetch_one(&pool)
            .await?;
        let hero = |level, total| super::Hero { id, level, total, rebirths: 0 };

        super::publish(&pool, &[hero(10, 1_000)]).await?;
        super::publish(&pool, &[hero(11, 5_000)]).await?;
        let (start, end, start_level, end_level): (i64, i64, i32, i32) = sqlx::query_as(
            "SELECT start_total, end_total, start_level, end_level FROM experience_days WHERE character_id = $1",
        )
        .bind(id)
        .fetch_one(&pool)
        .await?;
        assert_eq!((start, end, start_level, end_level), (1_000, 5_000, 10, 11));
        let seen: i64 = sqlx::query_scalar("SELECT last_seen FROM characters WHERE character_id = $1")
            .bind(id)
            .fetch_one(&pool)
            .await?;
        assert!(seen > 0, "marked seen");

        super::publish(&pool, &[]).await?;
        let (online, record): (i32, i32) = sqlx::query_as("SELECT online, record FROM server_status WHERE id = 1")
            .fetch_one(&pool)
            .await?;
        assert_eq!(online, 0);
        assert!(record >= 1);
        Ok(())
    }

    #[test]
    fn total_experience_counts_the_levels_behind() {
        use crate::leveling::{required, total_experience};
        assert_eq!(total_experience(1, 5), 5);
        assert_eq!(total_experience(3, 7), required(1) + required(2) + 7);
    }
}
