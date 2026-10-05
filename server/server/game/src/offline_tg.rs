//! Offline training ground, after the reference server
//! (`Process_OfflineTG`): with Heaven's Blessing, a player banks ten
//! training minutes per minute online (900 at most), enters the offline
//! training ground (map 601) and may log out; back in, the OfflineTGAdmin
//! turns the minutes spent there into experience (Redux `GainExpBall`).
//!
//! The 5065 client reaches it through a Heaven's Blessing window this
//! client does not have, so an NPC in Twin City opens the way.

use std::collections::HashMap;
use std::sync::OnceLock;
use std::time::Instant;

use parking_lot::Mutex;
use tq_network::Actor;

use crate::entities::Character;
use crate::packets::{MsgTaskDialog, MsgUserInfo};
use crate::{ActorState, Error, State};

pub const ADMIN_ID: u32 = 601;
pub const GATE_ID: u32 = 602;
pub const OFFLINE_TG_MAP: u32 = 601;
const TRAINING_SPOT: (u16, u16) = (60, 54);
const TWIN_CITY: (u32, u16, u16) = (1_002, 430, 380);
const MAX_MINUTES: i64 = 900;
const MINUTES_PER_ONLINE_MINUTE: i64 = 10;
const ENTERED: &str = "offline_tg_entered";
const CREDIT: &str = "offline_tg_minutes";

fn logins() -> &'static Mutex<HashMap<u32, Instant>> {
    static LOGINS: OnceLock<Mutex<HashMap<u32, Instant>>> = OnceLock::new();
    LOGINS.get_or_init(Default::default)
}

/// Remembers when the character logged in (the online time earns minutes).
pub fn logged_in(character_id: u32) {
    logins().lock().insert(character_id, Instant::now());
}

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs() as i64)
}

async fn timer(state: &State, character: &Character, kind: &str) -> Result<Option<i64>, Error> {
    Ok(
        sqlx::query_scalar::<_, i64>("SELECT until_unix FROM character_timers WHERE character_id = $1 AND kind = $2")
            .bind(character.database_id())
            .bind(kind)
            .fetch_optional(state.pool())
            .await?,
    )
}

async fn set_timer(state: &State, character: &Character, kind: &str, value: Option<i64>) -> Result<(), Error> {
    match value {
        Some(value) => {
            sqlx::query(
                "INSERT INTO character_timers (character_id, kind, until_unix) VALUES ($1, $2, $3)
                 ON CONFLICT(character_id, kind) DO UPDATE SET until_unix = excluded.until_unix",
            )
            .bind(character.database_id())
            .bind(kind)
            .bind(value)
            .execute(state.pool())
            .await?;
        },
        None => {
            sqlx::query("DELETE FROM character_timers WHERE character_id = $1 AND kind = $2")
                .bind(character.database_id())
                .bind(kind)
                .execute(state.pool())
                .await?;
        },
    }
    Ok(())
}

/// Minutes earned online so far this session (Redux: ten per minute).
fn session_minutes(character_id: u32) -> i64 {
    logins()
        .lock()
        .get(&character_id)
        .map_or(0, |login| login.elapsed().as_secs() as i64 / 60 * MINUTES_PER_ONLINE_MINUTE)
        .min(MAX_MINUTES)
}

/// Minutes spent in the training ground and minutes still banked.
pub fn claimable(entered: i64, banked: i64, now: i64) -> (i64, i64) {
    let used = ((now - entered).max(0) / 60).min(banked);
    (used, banked - used)
}

type Options = Vec<(u8, String)>;

fn options(list: &[(u8, &str)]) -> Options {
    list.iter().map(|(id, label)| (*id, (*label).to_owned())).collect()
}

pub fn opening(npc_id: u32) -> Option<(String, Options, u16)> {
    let (text, options) = match npc_id {
        GATE_ID => (
            "Blessed heroes may train while they rest. With Heaven's Blessing you earn ten minutes of offline training for every minute you play, up to fifteen hours. Shall I send you to the offline training ground?",
            options(&[(1, "Check my training time."), (2, "Send me there."), (255, "Not now.")]),
        ),
        ADMIN_ID => (
            "Welcome back. I can turn the time you spent here into experience and send you home.",
            options(&[(1, "Claim my experience."), (255, "Let me stay.")]),
        ),
        _ => return None,
    };
    Some((text.to_owned(), options, 4))
}

async fn say(actor: &Actor<ActorState>, npc_id: u32, text: impl Into<String>, option: &str) -> Result<(), Error> {
    let dialog = MsgTaskDialog::builder()
        .text(text.into())
        .with_option(255, option)
        .and()
        .with_avatar(4)
        .task_id(npc_id)
        .build();
    actor.send_all(dialog).await?;
    Ok(())
}

pub async fn answer(
    state: &State,
    actor: &Actor<ActorState>,
    character: &Character,
    npc_id: u32,
    option: u8,
) -> Result<bool, Error> {
    match (npc_id, option) {
        (GATE_ID, 1) => {
            let banked = timer(state, character, CREDIT).await?.unwrap_or(0);
            let total = (banked + session_minutes(character.id())).min(MAX_MINUTES);
            say(actor, npc_id, format!("You can train offline for {total} minutes."), "I see.").await?;
        },
        (GATE_ID, 2) => {
            if !character.heaven_blessing_active() {
                say(actor, npc_id, "You may not enter Offline Training Grounds without Heaven's Blessing.", "I see.")
                    .await?;
                return Ok(true);
            }
            let banked = timer(state, character, CREDIT).await?.unwrap_or(0);
            let total = (banked + session_minutes(character.id())).min(MAX_MINUTES);
            set_timer(state, character, CREDIT, Some(total)).await?;
            set_timer(state, character, ENTERED, Some(now_unix())).await?;
            logged_in(character.id());
            character.teleport(state, OFFLINE_TG_MAP, TRAINING_SPOT).await?;
            character.save(state).await?;
            say(
                actor,
                npc_id,
                format!("You have {total} minutes of training. You may log out now; talk to the OfflineTGAdmin when you return."),
                "Thanks.",
            )
            .await?;
        },
        (ADMIN_ID, 1) => {
            let banked = timer(state, character, CREDIT).await?.unwrap_or(0);
            let Some(entered) = timer(state, character, ENTERED).await? else {
                character.teleport(state, TWIN_CITY.0, (TWIN_CITY.1, TWIN_CITY.2)).await?;
                return Ok(true);
            };
            let (used, left) = claimable(entered, banked, now_unix());
            // Redux `GainExpBall(minutes)`: 600 is one ExpBall.
            if used > 0 {
                crate::leveling::use_exp_ball(state, actor, character, used as f64).await?;
            }
            set_timer(state, character, CREDIT, Some(left)).await?;
            set_timer(state, character, ENTERED, None).await?;
            actor.send(MsgUserInfo::from(character)).await?;
            character.teleport(state, TWIN_CITY.0, (TWIN_CITY.1, TWIN_CITY.2)).await?;
        },
        (GATE_ID | ADMIN_ID, _) => {},
        _ => return Ok(false),
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_time_spent_is_capped_by_the_minutes_banked() {
        assert_eq!(claimable(0, 100, 30 * 60), (30, 70));
        assert_eq!(claimable(0, 100, 500 * 60), (100, 0));
        assert_eq!(claimable(100, 50, 50), (0, 50));
    }
}
