//! Bounties: a player puts silver on another character's head, and whoever
//! slays that character in PvP collects it.
//!
//! The reward lives in the `bounties` table, one row per target, so it
//! survives restarts and further bounties add up. Placing debits the
//! placer's silver first (refunded if the bounty cannot be stored); claiming
//! deletes the row atomically, so a reward is only ever paid once.

use crate::entities::Character;
use crate::Error;

/// The smallest bounty that can be placed.
pub const MIN_BOUNTY: u32 = 10_000;
/// The most a single head can be worth, so a reward always fits the silver
/// a character can hold.
pub const MAX_BOUNTY: u64 = 2_000_000_000;
/// Rows shown in the client's Bounty window.
pub const LIST_LIMIT: u32 = 20;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BountyEntry {
    pub target_id: u32,
    pub name: String,
    pub level: u8,
    pub reward: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlaceOutcome {
    Placed {
        target_id: u32,
        target: String,
        amount: u32,
        total: u64,
    },
    TargetNotFound,
    OwnHead,
    TooSmall,
    TooLarge,
    NotEnoughSilver,
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs() as i64)
}

/// The character named `name` (any case): (character id, name).
async fn find_character(pool: &sqlx::SqlitePool, name: &str) -> Result<Option<(i64, String)>, Error> {
    Ok(sqlx::query_as::<_, (i64, String)>(
        "SELECT character_id, name FROM characters WHERE name = ? COLLATE NOCASE LIMIT 1",
    )
    .bind(name)
    .fetch_optional(pool)
    .await?)
}

/// Adds `amount` to the reward on `target_id`, if the total stays within
/// [`MAX_BOUNTY`]. Returns the new total.
async fn add_reward(pool: &sqlx::SqlitePool, target_id: i64, amount: u32) -> Result<Option<u64>, Error> {
    let now = now();
    let total = sqlx::query_scalar::<_, i64>(
        "INSERT INTO bounties (target_id, reward, placed_at, updated_at) VALUES (?, ?, ?, ?) \
         ON CONFLICT (target_id) DO UPDATE SET reward = reward + excluded.reward, updated_at = excluded.updated_at \
         WHERE reward + excluded.reward <= ? \
         RETURNING reward",
    )
    .bind(target_id)
    .bind(i64::from(amount))
    .bind(now)
    .bind(now)
    .bind(MAX_BOUNTY as i64)
    .fetch_optional(pool)
    .await?;
    Ok(total.map(|total| total.max(0) as u64))
}

/// `placer` puts `amount` silver on `target_name`'s head.
pub async fn place(
    pool: &sqlx::SqlitePool,
    placer: &Character,
    target_name: &str,
    amount: u32,
) -> Result<PlaceOutcome, Error> {
    let Some((target_id, target)) = find_character(pool, target_name.trim()).await? else {
        return Ok(PlaceOutcome::TargetNotFound);
    };
    if target_id == i64::from(placer.database_id()) {
        return Ok(PlaceOutcome::OwnHead);
    }
    if amount < MIN_BOUNTY {
        return Ok(PlaceOutcome::TooSmall);
    }
    if u64::from(amount) > MAX_BOUNTY {
        return Ok(PlaceOutcome::TooLarge);
    }
    if placer.spend_silver(pool, amount).await?.is_none() {
        return Ok(PlaceOutcome::NotEnoughSilver);
    }
    match add_reward(pool, target_id, amount).await {
        Ok(Some(total)) => Ok(PlaceOutcome::Placed {
            target_id: target_id as u32,
            target,
            amount,
            total,
        }),
        Ok(None) => {
            placer.grant_silver(pool, amount).await?;
            Ok(PlaceOutcome::TooLarge)
        },
        Err(error) => {
            placer.grant_silver(pool, amount).await?;
            Err(error)
        },
    }
}

/// The highest bounties, richest first.
pub async fn list(pool: &sqlx::SqlitePool, limit: u32) -> Result<Vec<BountyEntry>, Error> {
    let rows = sqlx::query_as::<_, (i64, String, i64, i64)>(
        "SELECT b.target_id, c.name, c.level, b.reward FROM bounties b \
         JOIN characters c ON c.character_id = b.target_id \
         ORDER BY b.reward DESC, b.placed_at ASC LIMIT ?",
    )
    .bind(i64::from(limit))
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(target_id, name, level, reward)| BountyEntry {
            target_id: target_id as u32,
            name,
            level: level.clamp(0, 255) as u8,
            reward: reward.max(0) as u64,
        })
        .collect())
}

/// The reward on `target_id`, if any.
pub async fn reward_on(pool: &sqlx::SqlitePool, target_id: i32) -> Result<Option<u64>, Error> {
    Ok(
        sqlx::query_scalar::<_, i64>("SELECT reward FROM bounties WHERE target_id = ?")
            .bind(target_id)
            .fetch_optional(pool)
            .await?
            .map(|reward| reward.max(0) as u64),
    )
}

/// Takes the whole bounty on `target_id`; `None` if there was none (or it
/// was already claimed).
pub async fn claim(pool: &sqlx::SqlitePool, target_id: i32) -> Result<Option<u64>, Error> {
    Ok(
        sqlx::query_scalar::<_, i64>("DELETE FROM bounties WHERE target_id = ? RETURNING reward")
            .bind(target_id)
            .fetch_optional(pool)
            .await?
            .map(|reward| reward.max(0) as u64),
    )
}

/// Tells every player online, in the centre of the screen.
pub async fn announce(state: &crate::State, text: String) {
    use crate::packets::{MsgTalk, TalkChannel};
    for entity in state.entities() {
        if let Some(character) = entity.as_character() {
            let _ = character
                .owner()
                .send(MsgTalk::from_system(character.id(), TalkChannel::Center, text.clone()))
                .await;
        }
    }
}

/// Pays the bounty on `victim` to `killer`, if there is one and the kill
/// counts.
pub async fn collect(state: &crate::State, killer: &Character, victim: &Character) -> Result<(), Error> {
    use crate::packets::{MsgBounty, MsgTalk, MsgUserInfo, TalkChannel};
    if !can_collect(
        killer.database_id(),
        killer.account_id(),
        victim.database_id(),
        victim.account_id(),
    ) {
        return Ok(());
    }
    let Some(reward) = claim(state.pool(), victim.database_id()).await? else {
        return Ok(());
    };
    let paid = reward.min(u64::from(u32::MAX)) as u32;
    killer.grant_silver(state.pool(), paid).await?;
    crate::audit::economic("bounty_claim", killer.id(), victim.id(), 0, paid);
    let owner = killer.owner();
    owner.send(MsgUserInfo::from(killer)).await?;
    owner
        .send(MsgBounty::claimed(
            victim.database_id() as u32,
            victim.entity().name().to_owned(),
            reward,
        ))
        .await?;
    owner
        .send(MsgTalk::from_system(
            killer.id(),
            TalkChannel::System,
            format!(
                "You collected the bounty of {reward} silver on {}.",
                victim.entity().name()
            ),
        ))
        .await?;
    announce(
        state,
        format!(
            "{} collected the bounty of {reward} silver on {}!",
            killer.entity().name(),
            victim.entity().name()
        ),
    )
    .await;
    Ok(())
}

/// Whether `killer` slaying `victim` collects a bounty: another character
/// of another account.
pub fn can_collect(killer_id: i32, killer_account: i32, victim_id: i32, victim_account: i32) -> bool {
    killer_id != victim_id && killer_account != victim_account
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn pool() -> sqlx::SqlitePool {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::migrate!("../../migrations").run(&pool).await.unwrap();
        for (account, name) in [(1, "Hunter"), (2, "Outlaw")] {
            let character = crate::packets::MsgRegister::build_character_with(
                name.into(),
                crate::packets::BodyType::AgileMale,
                crate::packets::BaseClass::Trojan,
                account,
                1,
            )
            .unwrap();
            character.save(&pool).await.unwrap();
        }
        pool
    }

    async fn id_of(pool: &sqlx::SqlitePool, name: &str) -> i64 {
        find_character(pool, name).await.unwrap().unwrap().0
    }

    #[tokio::test]
    async fn rewards_add_up_and_are_claimed_once() {
        let pool = pool().await;
        let outlaw = id_of(&pool, "outlaw").await;
        assert_eq!(add_reward(&pool, outlaw, 50_000).await.unwrap(), Some(50_000));
        assert_eq!(add_reward(&pool, outlaw, 25_000).await.unwrap(), Some(75_000));
        assert_eq!(reward_on(&pool, outlaw as i32).await.unwrap(), Some(75_000));
        let listed = list(&pool, 10).await.unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].name, "Outlaw");
        assert_eq!(listed[0].reward, 75_000);
        assert_eq!(claim(&pool, outlaw as i32).await.unwrap(), Some(75_000));
        assert_eq!(claim(&pool, outlaw as i32).await.unwrap(), None);
        assert!(list(&pool, 10).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn a_head_is_capped() {
        let pool = pool().await;
        let outlaw = id_of(&pool, "Outlaw").await;
        assert_eq!(
            add_reward(&pool, outlaw, 1_999_990_000).await.unwrap(),
            Some(1_999_990_000)
        );
        assert_eq!(add_reward(&pool, outlaw, 20_000).await.unwrap(), None);
        assert_eq!(reward_on(&pool, outlaw as i32).await.unwrap(), Some(1_999_990_000));
    }

    #[test]
    fn only_other_accounts_collect() {
        assert!(can_collect(1, 10, 2, 20));
        assert!(!can_collect(1, 10, 1, 10));
        assert!(!can_collect(1, 10, 2, 10));
    }
}
