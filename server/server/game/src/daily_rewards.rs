//! Daily rewards, after Tibia's Reward Shrine: at the Gravak statue a
//! character claims one reward a day from a seven-day cycle
//! (`data/daily_rewards.dat`). Days claimed in a row walk the cycle; a day
//! missed starts it over. Days turn at midnight UTC.

use std::sync::OnceLock;

use crate::entities::{Character, InventoryItem};
use crate::Error;

/// Days in the cycle.
pub const CYCLE: u32 = 7;
/// How close to a shrine a character must stand, in cells.
const SHRINE_REACH: u16 = 4;
/// The action of the Gravak objects that are shrines (`content/maps`).
pub const SHRINE_ACTION: &str = "daily_rewards";

/// One choice of one day of the cycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reward {
    pub day: u32,
    pub item_type: u32,
    pub amount: u32,
}

/// The cycle, read once.
pub fn rewards() -> &'static [Reward] {
    static REWARDS: OnceLock<Vec<Reward>> = OnceLock::new();
    REWARDS.get_or_init(|| parse(include_str!("../data/daily_rewards.dat")))
}

/// `day item_type amount` lines; `#` starts a comment.
fn parse(text: &str) -> Vec<Reward> {
    text.lines()
        .filter_map(|line| {
            let line = line.split('#').next()?.trim();
            let mut fields = line.split_whitespace().map(str::parse::<u32>);
            let (Some(Ok(day)), Some(Ok(item_type)), Some(Ok(amount))) = (fields.next(), fields.next(), fields.next())
            else {
                return None;
            };
            ((1..=CYCLE).contains(&day) && amount > 0).then_some(Reward { day, item_type, amount })
        })
        .collect()
}

/// The choices of one day of the cycle.
pub fn choices(day: u32) -> Vec<Reward> {
    rewards().iter().copied().filter(|reward| reward.day == day).collect()
}

/// Days since 1970 (UTC).
pub fn today() -> i64 {
    crate::bounty::today()
}

/// When the next day starts (seconds since 1970).
pub fn next_day_at(today: i64) -> u64 {
    u64::try_from(today + 1).unwrap_or_default() * 86_400
}

/// Where in the cycle a streak of `streak` days stands (1 to 7).
pub fn cycle_day(streak: u32) -> u32 {
    (streak.max(1) - 1) % CYCLE + 1
}

/// What the shrine shows a character today.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Standing {
    /// Days claimed in a row, today's included once claimed.
    pub streak: u32,
    /// Today's day of the cycle.
    pub day: u32,
    pub claimed_today: bool,
}

/// From the saved streak and last day claimed.
pub fn standing(streak: u32, last_day: i64, today: i64) -> Standing {
    if last_day == today {
        Standing { streak, day: cycle_day(streak), claimed_today: true }
    } else if last_day == today - 1 {
        Standing { streak, day: cycle_day(streak + 1), claimed_today: false }
    } else {
        // A day missed (or none ever claimed): the cycle starts over.
        Standing { streak: 0, day: 1, claimed_today: false }
    }
}

/// A character's saved streak and last day.
pub async fn load(pool: &sqlx::PgPool, character_id: i32) -> Result<(u32, i64), Error> {
    let row: Option<(i64, i64)> = sqlx::query_as("SELECT streak, last_day FROM daily_rewards WHERE character_id = $1")
        .bind(i64::from(character_id))
        .fetch_optional(pool)
        .await?;
    Ok(row.map_or((0, -1), |(streak, last_day)| (u32::try_from(streak).unwrap_or_default(), last_day)))
}

/// Records today's claim, once: the new streak, or `None` when today was
/// already claimed (two claims at once cannot both pass).
async fn record(pool: &sqlx::PgPool, character_id: i32, streak: u32, today: i64) -> Result<Option<u32>, Error> {
    let row: Option<(i64,)> = sqlx::query_as(
        "INSERT INTO daily_rewards (character_id, streak, last_day) VALUES ($1, $2, $3)
         ON CONFLICT (character_id) DO UPDATE SET streak = $2, last_day = $3
         WHERE daily_rewards.last_day < $3
         RETURNING streak",
    )
    .bind(i64::from(character_id))
    .bind(i64::from(streak))
    .bind(today)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|(streak,)| u32::try_from(streak).unwrap_or_default()))
}

/// Why a claim was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    AlreadyClaimed,
    NoSuchChoice,
    BagFull,
}

impl Refusal {
    pub fn message(self) -> &'static str {
        match self {
            Self::AlreadyClaimed => "You have already claimed today's reward. Come back tomorrow!",
            Self::NoSuchChoice => "That is not one of today's rewards.",
            Self::BagFull => "Make room in your bag for the reward first.",
        }
    }
}

/// What a claim gave.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Claim {
    pub reward: Reward,
    pub streak: u32,
    pub items: Vec<InventoryItem>,
}

/// Claims choice `choice` (an index among today's choices).
pub async fn claim(
    state: &crate::State,
    character: &Character,
    choice: u32,
) -> Result<Result<Claim, Refusal>, Error> {
    let pool = state.pool();
    let today = today();
    let (streak, last_day) = load(pool, character.database_id()).await?;
    let standing = standing(streak, last_day, today);
    if standing.claimed_today {
        return Ok(Err(Refusal::AlreadyClaimed));
    }
    let Some(reward) = choices(standing.day).get(choice as usize).copied() else {
        return Ok(Err(Refusal::NoSuchChoice));
    };
    let room = 40_usize.saturating_sub(character.inventory_items().len());
    if room < reward.amount as usize {
        return Ok(Err(Refusal::BagFull));
    }
    let Some(streak) = record(pool, character.database_id(), standing.streak + 1, today).await? else {
        return Ok(Err(Refusal::AlreadyClaimed));
    };
    let mut items = Vec::with_capacity(reward.amount as usize);
    for _ in 0..reward.amount {
        if let Some(item) = character.grant_item(pool, state.shop_catalog(), reward.item_type, false).await? {
            items.push(item);
        }
    }
    sqlx::query(
        "INSERT INTO daily_reward_claims (character_id, day, cycle_day, item_type, amount, claimed_at)
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(i64::from(character.database_id()))
    .bind(today)
    .bind(i64::from(reward.day))
    .bind(i64::from(reward.item_type))
    .bind(i64::from(reward.amount))
    .bind(i64::try_from(crate::store::now_secs()).unwrap_or(i64::MAX))
    .execute(pool)
    .await?;
    tracing::info!(
        target: "economy",
        character = %character.entity().name(),
        day = reward.day,
        item = reward.item_type,
        amount = reward.amount,
        streak,
        "Daily reward claimed"
    );
    Ok(Ok(Claim { reward, streak, items }))
}

/// Whether `(x, y)` of `map_id` is within reach of a Reward Shrine (a
/// Gravak object whose action is `daily_rewards`, in the built content of
/// `GRAVAK_CONTENT`).
pub fn near_shrine(map_id: u32, (x, y): (u16, u16)) -> bool {
    let Some(content) = std::env::var_os("GRAVAK_CONTENT") else {
        return false;
    };
    let path = std::path::Path::new(&content)
        .join("gravak")
        .join("maps")
        .join(format!("{map_id}.txt"));
    let Ok(text) = std::fs::read_to_string(path) else {
        return false;
    };
    shrines(&text)
        .into_iter()
        .any(|(sx, sy)| sx.abs_diff(x) <= SHRINE_REACH && sy.abs_diff(y) <= SHRINE_REACH)
}

/// The cells of the shrines of a Gravak objects file
/// (`object <x> <y> <dx> <dy> <image> daily_rewards`).
fn shrines(text: &str) -> Vec<(u16, u16)> {
    text.lines()
        .filter_map(|line| {
            let fields: Vec<&str> = line.split_whitespace().collect();
            (fields.len() == 7 && fields[0] == "object" && fields[6] == SHRINE_ACTION)
                .then(|| Some((fields[1].parse().ok()?, fields[2].parse().ok()?)))
                .flatten()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cycle_has_a_reward_every_day() {
        for day in 1..=CYCLE {
            assert!(!choices(day).is_empty(), "day {day}");
        }
        assert_eq!(parse("1 1088001 1 # Meteor\n8 1 1\n2 723017 0\nbad\n").len(), 1);
    }

    #[test]
    fn days_in_a_row_walk_the_cycle_and_a_missed_day_starts_over() {
        // Never claimed: day 1.
        assert_eq!(standing(0, -1, 100), Standing { streak: 0, day: 1, claimed_today: false });
        // Claimed yesterday at a streak of 3: today is day 4.
        assert_eq!(standing(3, 99, 100), Standing { streak: 3, day: 4, claimed_today: false });
        // Claimed today: the day just taken.
        assert_eq!(standing(4, 100, 100), Standing { streak: 4, day: 4, claimed_today: true });
        // A day missed: over again.
        assert_eq!(standing(6, 98, 100), Standing { streak: 0, day: 1, claimed_today: false });
        // After day 7, day 1 again, the streak going on.
        assert_eq!(standing(7, 99, 100).day, 1);
        assert_eq!(cycle_day(15), 1);
    }

    #[test]
    fn shrines_are_the_objects_with_the_daily_rewards_action() {
        let text = "# Gravak objects\nobject 440 363 -60 -140 gravak/objects/statue.png daily_rewards\n\
                    object 430 380 0 0 gravak/objects/tree.png\nblock 440 363\n";
        assert_eq!(shrines(text), [(440, 363)]);
    }

    #[tokio::test]
    async fn a_day_is_claimed_once() -> Result<(), Error> {
        let pool = tq_db::testing::pool().await;
        let inner = crate::packets::MsgRegister::build_character_with(
            "daily-test".to_owned(),
            crate::packets::BodyType::MuscularMale,
            crate::packets::BaseClass::Trojan,
            1,
            1,
        )?;
        inner.save(&pool).await?;
        let inner = tq_db::character::Character::from_account(&pool, 1).await?.expect("character");
        let id = inner.character_id;
        assert_eq!(record(&pool, id, 1, 100).await?, Some(1));
        assert_eq!(record(&pool, id, 2, 100).await?, None, "the same day twice");
        assert_eq!(record(&pool, id, 2, 101).await?, Some(2));
        assert_eq!(load(&pool, id).await?, (2, 101));
        Ok(())
    }
}
