//! A status line in the log every few minutes: who is online, for how long
//! the server has been up, and how the database answers.

use crate::State;
use std::time::{Duration, Instant};

/// How often the status line is written.
const EVERY: Duration = Duration::from_secs(300);

/// Starts writing the status line; it runs until the server stops.
pub fn start(state: &'static State, started: Instant) {
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
}
