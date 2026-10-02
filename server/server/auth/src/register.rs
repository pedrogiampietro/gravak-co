//! Account registration from the client's login screen.
//!
//! The 5065 protocol has no registration message, so this is a project
//! extension (`9001` request, `9002` reply) handled natively by the account
//! server rather than by a packet WASM module.

use serde::{Deserialize, Serialize};
use tq_network::{Actor, PacketID};
use tq_serde::{String16, TQPassword};

/// Request to create an account, sent by the client before logging in.
#[derive(Default, Debug, Serialize, Deserialize, PacketID)]
#[packet(id = 9001)]
pub struct MsgRegisterAccount {
    pub username: String16,
    pub password: TQPassword,
}

/// Outcome of [`MsgRegisterAccount`].
#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, PacketID)]
#[packet(id = 9002)]
pub struct MsgRegisterAccountResult {
    pub result: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum RegisterResult {
    Created = 0,
    UsernameTaken = 1,
    InvalidUsername = 2,
    InvalidPassword = 3,
    TryAgainLater = 4,
}

impl RegisterResult {
    pub fn packet(self) -> MsgRegisterAccountResult {
        MsgRegisterAccountResult { result: self as u32 }
    }
}

pub const USERNAME_LENGTH: std::ops::RangeInclusive<usize> = 4..=15;
pub const PASSWORD_LENGTH: std::ops::RangeInclusive<usize> = 4..=15;

/// Usernames are 4-15 ASCII letters, digits or underscores.
pub fn valid_username(username: &str) -> bool {
    USERNAME_LENGTH.contains(&username.len()) && username.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Passwords are 4-15 printable ASCII characters (they must fit the
/// 16-byte login field).
pub fn valid_password(password: &str) -> bool {
    PASSWORD_LENGTH.contains(&password.len()) && password.chars().all(|c| c.is_ascii_graphic())
}

pub async fn register(pool: &sqlx::SqlitePool, username: &str, password: &str) -> RegisterResult {
    if !valid_username(username) {
        return RegisterResult::InvalidUsername;
    }
    if !valid_password(password) {
        return RegisterResult::InvalidPassword;
    }
    let taken = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM accounts WHERE username = ? COLLATE NOCASE;")
        .bind(username)
        .fetch_one(pool)
        .await;
    match taken {
        Ok(0) => {},
        Ok(_) => return RegisterResult::UsernameTaken,
        Err(error) => {
            tracing::error!(%error, "Failed to check username");
            return RegisterResult::TryAgainLater;
        },
    }
    let account = tq_db::account::Account {
        username: username.to_owned(),
        password: password.to_owned(),
        ..Default::default()
    };
    match account.create(pool).await {
        Ok(account) => {
            tracing::info!(account_id = account.account_id, %username, "Account registered");
            RegisterResult::Created
        },
        // Lost a race with another registration of the same name.
        Err(tq_db::Error::Db(sqlx::Error::Database(error))) if error.is_unique_violation() => {
            RegisterResult::UsernameTaken
        },
        Err(error) => {
            tracing::error!(%error, "Failed to create account");
            RegisterResult::TryAgainLater
        },
    }
}

pub async fn process(
    msg: MsgRegisterAccount,
    pool: &sqlx::SqlitePool,
    actor: &Actor<()>,
) -> Result<(), crate::error::Error> {
    let result = register(pool, &msg.username, &msg.password).await;
    actor.send(result.packet()).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn pool() -> sqlx::SqlitePool {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::migrate!("../../migrations").run(&pool).await.unwrap();
        pool
    }

    #[test]
    fn validates_username_and_password() {
        assert!(valid_username("Gravak_01"));
        assert!(!valid_username("abc"));
        assert!(!valid_username("bad name"));
        assert!(!valid_username("a234567890123456"));
        assert!(valid_password("s3cr3t!"));
        assert!(!valid_password("abc"));
        assert!(!valid_password("with space"));
    }

    #[tokio::test]
    async fn registers_once_and_logs_in() {
        let pool = pool().await;
        assert_eq!(register(&pool, "newhero", "hunter2").await, RegisterResult::Created);
        assert_eq!(
            register(&pool, "NewHero", "other1").await,
            RegisterResult::UsernameTaken
        );
        assert_eq!(register(&pool, "x", "hunter2").await, RegisterResult::InvalidUsername);
        assert_eq!(register(&pool, "another", "no").await, RegisterResult::InvalidPassword);
        let account = tq_db::account::Account::auth(&pool, "newhero", "hunter2")
            .await
            .unwrap();
        assert_eq!(account.username, "newhero");
    }
}
