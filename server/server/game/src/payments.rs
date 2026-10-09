//! Conquer Points bought with real money. The website takes the payment
//! (Mercado Pago, `web/src/lib/mercadopago.ts`) and marks the order in
//! `payment_orders` approved; this credits its CPs to the character, once,
//! and tells the player when online.

use std::time::Duration;

use crate::packets::{MsgTalk, MsgUserInfo, TalkChannel};
use crate::state::State;
use crate::Error;

/// How often paid orders are looked for.
const EVERY: Duration = Duration::from_secs(10);

/// An order credited.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Credit {
    pub order_id: String,
    pub character_id: i32,
    pub cps: u32,
    /// The character's CPs after it.
    pub total: u64,
}

/// Starts crediting paid orders; it runs until the server stops.
pub fn start(state: &'static State) {
    tokio::spawn(async move {
        let mut ticks = tokio::time::interval(EVERY);
        loop {
            ticks.tick().await;
            if let Err(error) = credit_paid(state).await {
                tracing::warn!(target: "payment", %error, "Paid orders were not credited");
            }
        }
    });
}

/// Credits every approved order not credited yet.
pub async fn credit_paid(state: &State) -> Result<Vec<Credit>, Error> {
    let mut credits = Vec::new();
    while let Some(credit) = credit_next(state.pool()).await? {
        tracing::info!(
            target: "payment",
            order = %credit.order_id,
            character_id = credit.character_id,
            cps = credit.cps,
            total = credit.total,
            "CPs credited"
        );
        crate::audit::economic("payment", u32::try_from(credit.character_id).unwrap_or_default(), 0, 0, credit.cps);
        tell_online(state, &credit).await;
        credits.push(credit);
    }
    Ok(credits)
}

/// Credits the oldest approved order, the character's CPs and the order's
/// `credited_at` in one transaction: an order is never credited twice.
/// None when there is nothing to credit.
async fn credit_next(pool: &sqlx::PgPool) -> Result<Option<Credit>, Error> {
    let mut transaction = pool.begin().await?;
    let order: Option<(String, i64, i32)> = sqlx::query_as(
        "SELECT order_id::text, character_id, cps FROM payment_orders
          WHERE status = 'approved' AND credited_at IS NULL AND character_id IS NOT NULL
          ORDER BY paid_at NULLS FIRST LIMIT 1
          FOR UPDATE SKIP LOCKED",
    )
    .fetch_optional(&mut *transaction)
    .await?;
    let Some((order_id, character_id, cps)) = order else {
        return Ok(None);
    };
    let total: i64 = sqlx::query_scalar("UPDATE characters SET cps = cps + $2 WHERE character_id = $1 RETURNING cps")
        .bind(character_id)
        .bind(i64::from(cps))
        .fetch_one(&mut *transaction)
        .await?;
    sqlx::query("UPDATE payment_orders SET credited_at = now() WHERE order_id = $1::uuid")
        .bind(&order_id)
        .execute(&mut *transaction)
        .await?;
    transaction.commit().await?;
    Ok(Some(Credit {
        order_id,
        character_id: i32::try_from(character_id).unwrap_or_default(),
        cps: u32::try_from(cps).unwrap_or_default(),
        total: u64::try_from(total).unwrap_or_default(),
    }))
}

/// Mirrors the new CPs on the character, when online, and says so.
async fn tell_online(state: &State, credit: &Credit) {
    for entity in state.entities() {
        let Some(character) = entity.as_character() else {
            continue;
        };
        if character.database_id() != credit.character_id {
            continue;
        }
        character.set_cps(credit.total);
        let owner = character.owner();
        let _ = owner.send(MsgUserInfo::from(character)).await;
        let _ = owner
            .send(MsgTalk::from_system(
                character.id(),
                TalkChannel::System,
                format!("Your purchase arrived: {} CPs. You have {} CPs now. Thank you!", credit.cps, credit.total),
            ))
            .await;
        return;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn order(pool: &sqlx::PgPool, character_id: i64, status: &str) -> String {
        sqlx::query_scalar(
            "INSERT INTO payment_orders (account_id, character_id, character_name, package_id, price_cents, cps, channel, status, paid_at)
             SELECT account_id, character_id, name, 2, 2500, 55, 'client', $2, now() FROM characters WHERE character_id = $1
             RETURNING order_id::text",
        )
        .bind(character_id)
        .bind(status)
        .fetch_one(pool)
        .await
        .expect("order")
    }

    #[tokio::test]
    async fn approved_orders_are_credited_once() -> Result<(), Error> {
        let pool = tq_db::testing::pool().await;
        let id = crate::packets::MsgRegister::build_character_with(
            "payer".to_owned(),
            crate::packets::BodyType::MuscularMale,
            crate::packets::BaseClass::Trojan,
            1,
            1,
        )?
        .save(&pool)
        .await?;
        let character_id = i64::from(id);
        sqlx::query("UPDATE characters SET cps = 10 WHERE character_id = $1")
            .bind(character_id)
            .execute(&pool)
            .await?;
        let approved = order(&pool, character_id, "approved").await;
        order(&pool, character_id, "pending").await;

        let credit = credit_next(&pool).await?.expect("the approved order");
        assert_eq!((credit.order_id.as_str(), credit.cps, credit.total), (approved.as_str(), 55, 65));
        assert_eq!(credit_next(&pool).await?, None, "credited once; a pending order waits");

        let cps: i64 = sqlx::query_scalar("SELECT cps FROM characters WHERE character_id = $1")
            .bind(character_id)
            .fetch_one(&pool)
            .await?;
        assert_eq!(cps, 65);
        Ok(())
    }
}
