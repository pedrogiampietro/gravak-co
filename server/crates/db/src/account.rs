/// Account information for a registered player. The account server uses this
/// information to authenticate the player on login. Passwords are hashed using
/// bcrypt
#[derive(Default, Debug)]
#[cfg_attr(feature = "sqlx", derive(sqlx::FromRow))]
pub struct Account {
    #[cfg_attr(feature = "sqlx", sqlx(try_from = "i64"))]
    pub account_id: i32,
    pub username: String,
    pub password: String,
    pub name: Option<String>,
    pub email: Option<String>,
}

#[cfg(feature = "sqlx")]
impl Account {
    pub async fn auth(pool: &sqlx::PgPool, username: &str, password: &str) -> Result<Account, crate::Error> {
        let maybe_account = sqlx::query_as::<_, Self>("SELECT * FROM accounts WHERE username = $1;")
            .bind(username)
            .fetch_optional(pool)
            .await?;
        match maybe_account {
            Some(account) => {
                let matched = bcrypt::verify(password, &account.password)?;
                if matched {
                    Ok(account)
                } else {
                    Err(crate::Error::InvalidPassword)
                }
            },
            None => Err(crate::Error::AccountNotFound),
        }
    }

    /// Returns all accounts in the database.
    ///
    /// Useful for testing purposes.
    pub async fn all(
        pool: &sqlx::PgPool,
        limit: Option<i64>,
        offset: Option<i64>,
    ) -> Result<Vec<Self>, crate::Error> {
        use futures::TryFutureExt;
        sqlx::query_as::<_, Self>("SELECT * FROM accounts LIMIT $1 OFFSET $2;")
            .bind(limit.unwrap_or(100))
            .bind(offset.unwrap_or(0))
            .fetch_all(pool)
            .map_err(Into::into)
            .await
    }

    // === Methods ===

    /// The development accounts of the migrations that still have their
    /// published password (`123456`).
    pub async fn with_published_password(pool: &sqlx::PgPool) -> Result<Vec<String>, crate::Error> {
        let accounts: Vec<(String, String)> =
            sqlx::query_as("SELECT username, password FROM accounts WHERE username IN ('shekohex', 'test1') ORDER BY username")
                .fetch_all(pool)
                .await?;
        Ok(accounts
            .into_iter()
            .filter(|(_, hash)| bcrypt::verify("123456", hash).unwrap_or(false))
            .map(|(username, _)| username)
            .collect())
    }

    /// Creates a new account in the database.
    pub async fn create(mut self, pool: &sqlx::PgPool) -> Result<Self, crate::Error> {
        let password = bcrypt::hash(&self.password, bcrypt::DEFAULT_COST)?;
        let id: Option<i64> = sqlx::query_scalar(
            "INSERT INTO accounts (username, password, name, email) VALUES ($1, $2, $3, $4) RETURNING account_id",
        )
        .bind(&self.username)
        .bind(&password)
        .bind(&self.name)
        .bind(&self.email)
        .fetch_optional(pool)
        .await?;
        let id = id.ok_or(crate::Error::CreateAccountFailed)?;
        self.account_id = i32::try_from(id).map_err(|_| crate::Error::CreateAccountFailed)?;
        Ok(self)
    }
}
