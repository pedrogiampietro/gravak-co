//! Broadcasts (the original's 2050): a player of level 50 or more pays 5 CPs
//! and their message reaches the whole server, one a minute in the order
//! they were paid for (Redux `Process_Broadcast` and `PlayerManager`).

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use parking_lot::Mutex;

use crate::entities::Character;
use crate::packets::{MsgTalk, TalkChannel};
use crate::{Error, State};

pub const MIN_LEVEL: u16 = 50;
pub const PRICE_CPS: u64 = 5;
/// The longest message, in characters.
pub const MAX_LENGTH: usize = 80;
/// Time between two broadcasts.
const INTERVAL: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, PartialEq, Eq)]
struct Broadcast {
    speaker: String,
    message: String,
}

struct Queue {
    waiting: VecDeque<Broadcast>,
    next: Instant,
}

fn queue() -> &'static Mutex<Queue> {
    static QUEUE: std::sync::OnceLock<Mutex<Queue>> = std::sync::OnceLock::new();
    QUEUE.get_or_init(|| {
        Mutex::new(Queue {
            waiting: VecDeque::new(),
            next: Instant::now(),
        })
    })
}

/// Why a broadcast was refused, or its place in the queue.
pub fn check(level: u16, message: &str) -> Result<String, &'static str> {
    if level < MIN_LEVEL {
        return Err("You need level 50 to broadcast.");
    }
    let message: String = message.chars().filter(|character| !character.is_control()).collect();
    let message = message.trim();
    if message.is_empty() {
        return Err("Write something to broadcast.");
    }
    Ok(message.chars().take(MAX_LENGTH).collect())
}

/// Pays for and queues a broadcast; what to tell the player.
pub async fn submit(state: &State, character: &Character, message: &str) -> Result<String, Error> {
    let message = match check(character.entity().level(), message) {
        Ok(message) => message,
        Err(reason) => return Ok(reason.to_owned()),
    };
    let left: Option<i64> = sqlx::query_scalar(
        "UPDATE characters SET cps = cps - $2 WHERE character_id = $1 AND cps >= $2 RETURNING cps",
    )
    .bind(i64::from(character.database_id()))
    .bind(PRICE_CPS as i64)
    .fetch_optional(state.pool())
    .await?;
    let Some(left) = left else {
        return Ok(format!("A broadcast costs {PRICE_CPS} CPs."));
    };
    character.set_cps(u64::try_from(left).unwrap_or_default());
    tracing::info!(target: "economy", character = %character.entity().name(), %message, "Broadcast bought");
    let place = {
        let mut queue = queue().lock();
        queue.waiting.push_back(Broadcast {
            speaker: character.entity().name().to_owned(),
            message,
        });
        queue.waiting.len()
    };
    Ok(if place == 1 {
        "Your broadcast goes out shortly.".to_owned()
    } else {
        format!("Your broadcast is number {place} in line; one goes out a minute.")
    })
}

/// Sends the next broadcast to everyone once a minute has passed.
pub async fn tick(state: &State) {
    let next = {
        let mut queue = queue().lock();
        if queue.waiting.is_empty() || Instant::now() < queue.next {
            return;
        }
        queue.next = Instant::now() + INTERVAL;
        queue.waiting.pop_front()
    };
    let Some(broadcast) = next else {
        return;
    };
    let mut talk = MsgTalk::from_system(0, TalkChannel::Broadcast, broadcast.message);
    talk.sender_name = broadcast.speaker;
    for entity in state.entities() {
        if let Some(character) = entity.as_character() {
            let _ = character.owner().send(talk.clone()).await;
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn broadcasts_need_level_fifty_and_some_words() {
        assert!(super::check(49, "hello").is_err());
        assert!(super::check(50, "   ").is_err());
        assert_eq!(super::check(50, " Selling DBs! ").as_deref(), Ok("Selling DBs!"));
        assert_eq!(super::check(120, &"x".repeat(200)).map(|text| text.len()), Ok(super::MAX_LENGTH));
    }
}
