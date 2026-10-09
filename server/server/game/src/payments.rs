//! Conquer Points bought with real money. The website takes the payment
//! (Mercado Pago, `web/src/lib/mercadopago.ts`) and marks the order in
//! `payment_orders` approved; this credits its CPs to the character, once,
//! and tells the player when online. When Mercado Pago later refunds the
//! payment or it is charged back, the CPs are taken back; what was spent
//! already becomes the character's `cps_debt`, paid off by its next purchases.

use std::time::Duration;

use crate::packets::{MsgTalk, MsgUserInfo, TalkChannel};
use crate::state::State;
use crate::Error;

/// How often paid orders are looked for.
const EVERY: Duration = Duration::from_secs(10);

/// The statuses of a payment given back to the buyer.
const REVERSED: [&str; 3] = ["refunded", "charged_back", "cancelled"];

/// An order credited.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Credit {
    pub order_id: String,
    pub character_id: i32,
    pub cps: u32,
    /// The part of `cps` that went to paying off the character's debt.
    pub to_debt: u32,
    /// The character's CPs after it.
    pub total: u64,
}

/// An order credited and then refunded or charged back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reversal {
    pub order_id: String,
    pub character_id: i32,
    pub status: String,
    pub cps: u32,
    /// The CPs taken from the character; the rest of `cps` became debt.
    pub taken: u32,
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
            if let Err(error) = reverse_refunded(state).await {
                tracing::warn!(target: "payment", %error, "Refunded orders were not reversed");
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
            to_debt = credit.to_debt,
            total = credit.total,
            "CPs credited"
        );
        crate::audit::economic("payment", u32::try_from(credit.character_id).unwrap_or_default(), 0, 0, credit.cps);
        let message = if credit.to_debt == 0 {
            format!("Your purchase arrived: {} CPs. You have {} CPs now. Thank you!", credit.cps, credit.total)
        } else {
            format!(
                "Your purchase arrived: {} CPs, of which {} paid off the CPs of a refunded purchase. You have {} CPs now.",
                credit.cps, credit.to_debt, credit.total
            )
        };
        tell_online(state, credit.character_id, credit.total, message).await;
        credits.push(credit);
    }
    Ok(credits)
}

/// Takes back the CPs of every credited order refunded or charged back since.
pub async fn reverse_refunded(state: &State) -> Result<Vec<Reversal>, Error> {
    let mut reversals = Vec::new();
    while let Some(reversal) = reverse_next(state.pool()).await? {
        tracing::warn!(
            target: "payment",
            order = %reversal.order_id,
            character_id = reversal.character_id,
            status = %reversal.status,
            cps = reversal.cps,
            taken = reversal.taken,
            debt = reversal.cps - reversal.taken,
            total = reversal.total,
            "CPs taken back"
        );
        crate::audit::economic("payment_reversal", u32::try_from(reversal.character_id).unwrap_or_default(), 0, 0, reversal.taken);
        let why = if reversal.status == "charged_back" { "was charged back" } else { "was refunded" };
        let message = if reversal.taken == reversal.cps {
            format!("A purchase of {} CPs {why}: its CPs were taken back. You have {} CPs now.", reversal.cps, reversal.total)
        } else {
            format!(
                "A purchase of {} CPs {why}: {} CPs were taken back and the other {} will come out of your next purchases.",
                reversal.cps,
                reversal.taken,
                reversal.cps - reversal.taken
            )
        };
        tell_online(state, reversal.character_id, reversal.total, message).await;
        reversals.push(reversal);
    }
    Ok(reversals)
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
    // The character's debt, from refunded purchases, is paid off first.
    let debt: i64 = sqlx::query_scalar("SELECT cps_debt FROM characters WHERE character_id = $1 FOR UPDATE")
        .bind(character_id)
        .fetch_one(&mut *transaction)
        .await?;
    let to_debt = debt.min(i64::from(cps));
    let total: i64 = sqlx::query_scalar(
        "UPDATE characters SET cps = cps + $2, cps_debt = cps_debt - $3 WHERE character_id = $1 RETURNING cps",
    )
    .bind(character_id)
    .bind(i64::from(cps) - to_debt)
    .bind(to_debt)
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
        to_debt: u32::try_from(to_debt).unwrap_or_default(),
        total: u64::try_from(total).unwrap_or_default(),
    }))
}

/// Takes back the CPs of the oldest credited order refunded or charged back,
/// in one transaction with the order's `reversed_at`: never twice. What the
/// character no longer has goes to its `cps_debt`. None when there is nothing
/// to take back.
async fn reverse_next(pool: &sqlx::PgPool) -> Result<Option<Reversal>, Error> {
    let mut transaction = pool.begin().await?;
    let order: Option<(String, i64, i32, String)> = sqlx::query_as(
        "SELECT order_id::text, character_id, cps, status FROM payment_orders
          WHERE status = ANY($1) AND credited_at IS NOT NULL AND reversed_at IS NULL AND character_id IS NOT NULL
          ORDER BY credited_at LIMIT 1
          FOR UPDATE SKIP LOCKED",
    )
    .bind(&REVERSED[..])
    .fetch_optional(&mut *transaction)
    .await?;
    let Some((order_id, character_id, cps, status)) = order else {
        return Ok(None);
    };
    let had: i64 = sqlx::query_scalar("SELECT cps FROM characters WHERE character_id = $1 FOR UPDATE")
        .bind(character_id)
        .fetch_one(&mut *transaction)
        .await?;
    let taken = had.min(i64::from(cps));
    let total: i64 = sqlx::query_scalar(
        "UPDATE characters SET cps = cps - $2, cps_debt = cps_debt + $3 WHERE character_id = $1 RETURNING cps",
    )
    .bind(character_id)
    .bind(taken)
    .bind(i64::from(cps) - taken)
    .fetch_one(&mut *transaction)
    .await?;
    sqlx::query("UPDATE payment_orders SET reversed_at = now(), reversed_cps = $2 WHERE order_id = $1::uuid")
        .bind(&order_id)
        .bind(i32::try_from(taken).unwrap_or_default())
        .execute(&mut *transaction)
        .await?;
    transaction.commit().await?;
    Ok(Some(Reversal {
        order_id,
        character_id: i32::try_from(character_id).unwrap_or_default(),
        status,
        cps: u32::try_from(cps).unwrap_or_default(),
        taken: u32::try_from(taken).unwrap_or_default(),
        total: u64::try_from(total).unwrap_or_default(),
    }))
}

/// Mirrors the new CPs on the character, when online, and tells it why.
async fn tell_online(state: &State, character_id: i32, total: u64, message: String) {
    for entity in state.entities() {
        let Some(character) = entity.as_character() else {
            continue;
        };
        if character.database_id() != character_id {
            continue;
        }
        character.set_cps(total);
        let owner = character.owner();
        let _ = owner.send(MsgUserInfo::from(character)).await;
        let _ = owner.send(MsgTalk::from_system(character.id(), TalkChannel::System, message)).await;
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
        assert_eq!((credit.order_id.as_str(), credit.cps, credit.to_debt, credit.total), (approved.as_str(), 55, 0, 65));
        assert_eq!(credit_next(&pool).await?, None, "credited once; a pending order waits");

        let cps: i64 = sqlx::query_scalar("SELECT cps FROM characters WHERE character_id = $1")
            .bind(character_id)
            .fetch_one(&pool)
            .await?;
        assert_eq!(cps, 65);
        Ok(())
    }

    async fn cps(pool: &sqlx::PgPool, character_id: i64) -> (i64, i64) {
        sqlx::query_as("SELECT cps, cps_debt FROM characters WHERE character_id = $1")
            .bind(character_id)
            .fetch_one(pool)
            .await
            .expect("character")
    }

    #[tokio::test]
    async fn refunds_take_the_cps_back_once_and_spent_ones_become_debt() -> Result<(), Error> {
        let pool = tq_db::testing::pool().await;
        let id = crate::packets::MsgRegister::build_character_with(
            "refunder".to_owned(),
            crate::packets::BodyType::MuscularMale,
            crate::packets::BaseClass::Trojan,
            1,
            1,
        )?
        .save(&pool)
        .await?;
        let character_id = i64::from(id);
        let first = order(&pool, character_id, "approved").await;
        credit_next(&pool).await?.expect("credited");
        assert_eq!(reverse_next(&pool).await?, None, "nothing refunded yet");

        // 40 of the 55 CPs spent, then the payment refunded.
        sqlx::query("UPDATE characters SET cps = 15 WHERE character_id = $1").bind(character_id).execute(&pool).await?;
        sqlx::query("UPDATE payment_orders SET status = 'refunded' WHERE order_id = $1::uuid").bind(&first).execute(&pool).await?;
        // Refunded before it was credited: nothing to take back.
        order(&pool, character_id, "refunded").await;

        let reversal = reverse_next(&pool).await?.expect("the refunded order");
        assert_eq!((reversal.order_id.as_str(), reversal.cps, reversal.taken, reversal.total), (first.as_str(), 55, 15, 0));
        assert_eq!(cps(&pool, character_id).await, (0, 40));
        assert_eq!(reverse_next(&pool).await?, None, "taken back once");

        // The next purchase pays the debt off first.
        order(&pool, character_id, "approved").await;
        let credit = credit_next(&pool).await?.expect("credited");
        assert_eq!((credit.cps, credit.to_debt, credit.total), (55, 40, 15));
        assert_eq!(cps(&pool, character_id).await, (15, 0));
        Ok(())
    }
}
