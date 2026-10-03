//! The in-game Store: products sold for Conquer Points. A product is an item
//! (and how many) or days of VIP; some have a daily stock shared by the whole
//! server, restocked at midnight (UTC). VIP belongs to the account.

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use parking_lot::Mutex;

use crate::entities::{Character, InventoryItem};
use crate::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Product {
    pub product_id: u32,
    pub category: u32,
    pub name: String,
    pub description: String,
    pub item_type: u32,
    pub amount: u32,
    pub vip_days: u32,
    pub price_cps: u32,
    /// Left today, for a product with a daily stock.
    pub stock_left: Option<u32>,
}

/// Why a purchase was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    NoSuchProduct,
    NotEnoughCps(u32),
    SoldOut,
    BagFull(u32),
}

impl Refusal {
    pub fn message(self) -> String {
        match self {
            Self::NoSuchProduct => "That product is not on sale.".to_owned(),
            Self::NotEnoughCps(price) => format!("You need {price} CPs for that."),
            Self::SoldOut => "Sold out for today; it restocks at midnight.".to_owned(),
            Self::BagFull(slots) => format!("Make room for {slots} item(s) in your bag first."),
        }
    }
}

/// What a purchase gave.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Purchase {
    pub product: Product,
    pub cps_left: u64,
    pub items: Vec<InventoryItem>,
    /// The account's VIP end (seconds since 1970), when VIP was bought.
    pub vip_until: Option<u64>,
}

/// Days since 1970 (UTC), for the daily stock.
fn today() -> i64 {
    crate::bounty::today()
}

pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

/// The products on sale, in the Store's order, with today's stock.
pub async fn catalog(pool: &sqlx::PgPool) -> Result<Vec<Product>, Error> {
    let rows: Vec<(i64, i64, String, String, i64, i64, i64, i64, Option<i64>, Option<i64>)> = sqlx::query_as(
        "SELECT p.product_id, p.category, p.name, p.description, p.item_type, p.amount, p.vip_days, p.price_cps,
                p.daily_stock, s.sold
         FROM store_products p
         LEFT JOIN store_stock s ON s.product_id = p.product_id AND s.day = $1
         WHERE p.enabled
         ORDER BY p.category, p.sort, p.product_id",
    )
    .bind(today())
    .fetch_all(pool)
    .await?;
    let number = |value: i64| u32::try_from(value).unwrap_or_default();
    Ok(rows
        .into_iter()
        .map(
            |(product_id, category, name, description, item_type, amount, vip_days, price_cps, daily_stock, sold)| Product {
                product_id: number(product_id),
                category: number(category),
                name,
                description,
                item_type: number(item_type),
                amount: number(amount),
                vip_days: number(vip_days),
                price_cps: number(price_cps),
                stock_left: daily_stock.map(|stock| number(stock.saturating_sub(sold.unwrap_or(0)))),
            },
        )
        .collect())
}

/// Buys `product_id` for `character`: the CPs, the stock and the record in
/// one transaction, then the items or the VIP days.
pub async fn buy(
    pool: &sqlx::PgPool,
    character: &Character,
    account_id: u32,
    product_id: u32,
) -> Result<Result<Purchase, Refusal>, Error> {
    let Some(product) = catalog(pool)
        .await?
        .into_iter()
        .find(|product| product.product_id == product_id)
    else {
        return Ok(Err(Refusal::NoSuchProduct));
    };
    if product.stock_left == Some(0) {
        return Ok(Err(Refusal::SoldOut));
    }
    let slots_needed = if product.item_type > 0 { product.amount } else { 0 };
    let free_slots = 40_u32.saturating_sub(u32::try_from(character.inventory_items().len()).unwrap_or(40));
    if slots_needed > free_slots {
        return Ok(Err(Refusal::BagFull(slots_needed)));
    }

    let mut transaction = pool.begin().await?;
    let cps_left: Option<i64> = sqlx::query_scalar(
        "UPDATE characters SET cps = cps - $2 WHERE character_id = $1 AND cps >= $2 RETURNING cps",
    )
    .bind(i64::from(character.database_id()))
    .bind(i64::from(product.price_cps))
    .fetch_optional(&mut *transaction)
    .await?;
    let Some(cps_left) = cps_left else {
        return Ok(Err(Refusal::NotEnoughCps(product.price_cps)));
    };
    if product.stock_left.is_some() {
        // Two buyers of the last one: only one update finds room.
        let taken = sqlx::query(
            "INSERT INTO store_stock (product_id, day, sold) VALUES ($1, $2, 1)
             ON CONFLICT (product_id, day) DO UPDATE SET sold = store_stock.sold + 1
             WHERE store_stock.sold < (SELECT daily_stock FROM store_products WHERE product_id = $1)",
        )
        .bind(i64::from(product.product_id))
        .bind(today())
        .execute(&mut *transaction)
        .await?;
        if taken.rows_affected() != 1 {
            return Ok(Err(Refusal::SoldOut));
        }
    }
    sqlx::query("INSERT INTO store_purchases (character_id, product_id, price_cps) VALUES ($1, $2, $3)")
        .bind(i64::from(character.database_id()))
        .bind(i64::from(product.product_id))
        .bind(i64::from(product.price_cps))
        .execute(&mut *transaction)
        .await?;
    let vip_until = if product.vip_days > 0 {
        // VIP adds to what is left of it.
        let until: i64 = sqlx::query_scalar(
            "UPDATE accounts SET vip_until = GREATEST(vip_until, $2) + $3 WHERE account_id = $1 RETURNING vip_until",
        )
        .bind(i64::from(account_id))
        .bind(i64::try_from(now_secs()).unwrap_or(i64::MAX))
        .bind(i64::from(product.vip_days) * 86_400)
        .fetch_one(&mut *transaction)
        .await?;
        Some(u64::try_from(until).unwrap_or_default())
    } else {
        None
    };
    transaction.commit().await?;
    let cps_left = u64::try_from(cps_left).unwrap_or_default();
    character.set_cps(cps_left);

    let mut items = Vec::new();
    for _ in 0..slots_needed {
        match character.add_ground_item(pool, product.item_type, 100, 100).await? {
            Some(item) => {
                crate::audit::item(
                    "store_buy",
                    character.id(),
                    item.uid,
                    item.item_type,
                    format_args!("product_id={} price_cps={}", product.product_id, product.price_cps),
                );
                items.push(item);
            },
            None => break,
        }
    }
    tracing::info!(
        target: "economy",
        character = %character.entity().name(),
        product = %product.name,
        price_cps = product.price_cps,
        "Store purchase"
    );
    Ok(Ok(Purchase {
        product,
        cps_left,
        items,
        vip_until,
    }))
}

/// The account's VIP end (seconds since 1970); 0 without VIP.
pub async fn vip_until(pool: &sqlx::PgPool, account_id: u32) -> Result<u64, Error> {
    let until: Option<i64> = sqlx::query_scalar("SELECT vip_until FROM accounts WHERE account_id = $1")
        .bind(i64::from(account_id))
        .fetch_optional(pool)
        .await?;
    Ok(until.and_then(|until| u64::try_from(until).ok()).unwrap_or_default())
}

/// The VIP of the characters online, by entity id: when it ends.
#[derive(Debug, Default)]
pub struct VipBook {
    until: Mutex<HashMap<u32, u64>>,
}

impl VipBook {
    pub fn set(&self, character_id: u32, until: u64) {
        let mut book = self.until.lock();
        if until > now_secs() {
            book.insert(character_id, until);
        } else {
            book.remove(&character_id);
        }
    }

    pub fn remove(&self, character_id: u32) {
        self.until.lock().remove(&character_id);
    }

    /// Whether the character is VIP now.
    pub fn is_vip(&self, character_id: u32) -> bool {
        self.until.lock().get(&character_id).is_some_and(|until| *until > now_secs())
    }

    pub fn until(&self, character_id: u32) -> u64 {
        self.until.lock().get(&character_id).copied().unwrap_or_default()
    }
}

/// Experience with the VIP bonus (+10%).
pub fn with_vip_bonus(experience: u64, vip: bool) -> u64 {
    if vip { experience + experience / 10 } else { experience }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vip_adds_a_tenth() {
        assert_eq!(with_vip_bonus(1_000, true), 1_100);
        assert_eq!(with_vip_bonus(1_000, false), 1_000);
    }

    async fn character(pool: &sqlx::PgPool, cps: i64) -> Character {
        let id = crate::packets::MsgRegister::build_character_with(
            "store-test".to_owned(),
            crate::packets::BodyType::MuscularMale,
            crate::packets::BaseClass::Trojan,
            1,
            1,
        )
        .expect("character")
        .save(pool)
        .await
        .expect("saved");
        sqlx::query("UPDATE characters SET cps = $2 WHERE character_id = $1")
            .bind(i64::from(id))
            .bind(cps)
            .execute(pool)
            .await
            .expect("cps");
        let inner = tq_db::character::Character::from_account(pool, 1)
            .await
            .expect("load")
            .expect("character");
        let (tx, _rx) = tokio::sync::mpsc::channel(8);
        let actor = tq_network::Actor::<crate::ActorState>::new(tx);
        actor.set_id(1);
        Character::new(actor.handle(), inner)
    }

    #[tokio::test]
    async fn buying_spends_cps_gives_items_and_vip() -> Result<(), Error> {
        let pool = tq_db::testing::pool().await;
        let character = character(&pool, 1_000).await;

        let meteor = buy(&pool, &character, 1, 20).await?.expect("a Meteor");
        assert_eq!((meteor.cps_left, character.cps(), meteor.items.len()), (987, 987, 1));
        assert_eq!(meteor.items[0].item_type, 1_088_001);

        let vip = buy(&pool, &character, 1, 1).await?.expect("VIP");
        let week = vip.vip_until.expect("VIP until");
        assert!(week >= now_secs() + 7 * 86_400 - 5);
        assert_eq!(vip_until(&pool, 1).await?, week);

        assert_eq!(buy(&pool, &character, 1, 2).await?, Err(Refusal::NotEnoughCps(1_800)));
        assert_eq!(character.cps(), 487, "a refused purchase costs nothing");
        assert_eq!(buy(&pool, &character, 1, 999).await?, Err(Refusal::NoSuchProduct));
        Ok(())
    }

    #[tokio::test]
    async fn daily_stock_runs_out() -> Result<(), Error> {
        let pool = tq_db::testing::pool().await;
        let character = character(&pool, 100_000).await;
        sqlx::query("UPDATE store_products SET daily_stock = 2 WHERE product_id = 11")
            .execute(&pool)
            .await?;
        assert!(buy(&pool, &character, 1, 11).await?.is_ok());
        assert!(buy(&pool, &character, 1, 11).await?.is_ok());
        assert_eq!(buy(&pool, &character, 1, 11).await?, Err(Refusal::SoldOut));
        let ball = catalog(&pool).await?.into_iter().find(|product| product.product_id == 11).expect("Exp Ball");
        assert_eq!(ball.stock_left, Some(0));
        assert_eq!(character.cps(), 100_000 - 400);
        Ok(())
    }
}
