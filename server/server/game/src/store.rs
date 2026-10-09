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
    /// What it costs the buyer now: with `price_steps`, by how many they
    /// bought today.
    pub price_cps: u32,
    /// Left today, for a product with a daily stock.
    pub stock_left: Option<u32>,
}

/// The price of the purchase after `bought` of today: the steps in order,
/// then 1.8 times the last, rounded to 10 CPs. No steps: `price` always.
pub fn stepped_price(price: u32, steps: &[u32], bought: u32) -> u32 {
    let Some(last) = steps.last() else {
        return price;
    };
    if let Some(step) = steps.get(bought as usize) {
        return *step;
    }
    let mut price = f64::from(*last);
    for _ in steps.len() as u32..=bought {
        price = (price * 1.8 / 10.0).round() * 10.0;
    }
    price.min(f64::from(u32::MAX)) as u32
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

/// The products on sale, in the Store's order, with today's stock and the
/// prices `character_id` pays now.
pub async fn catalog(pool: &sqlx::PgPool, character_id: i32) -> Result<Vec<Product>, Error> {
    #[allow(clippy::type_complexity)]
    let rows: Vec<(i64, i64, String, String, i64, i64, i64, i64, Option<i64>, Option<i64>, Option<String>, i64)> =
        sqlx::query_as(
            "SELECT p.product_id, p.category, p.name, p.description, p.item_type, p.amount, p.vip_days, p.price_cps,
                    p.daily_stock, s.sold, p.price_steps,
                    (SELECT COUNT(*) FROM store_purchases b
                     WHERE b.product_id = p.product_id AND b.character_id = $2 AND b.bought_at >= $1 * 86400)
             FROM store_products p
             LEFT JOIN store_stock s ON s.product_id = p.product_id AND s.day = $1
             WHERE p.enabled
             ORDER BY p.category, p.sort, p.product_id",
        )
        .bind(today())
        .bind(i64::from(character_id))
        .fetch_all(pool)
        .await?;
    let number = |value: i64| u32::try_from(value).unwrap_or_default();
    Ok(rows
        .into_iter()
        .map(
            |(product_id, category, name, description, item_type, amount, vip_days, price_cps, daily_stock, sold, steps, bought)| {
                let steps: Vec<u32> = steps
                    .unwrap_or_default()
                    .split(',')
                    .filter_map(|step| step.trim().parse().ok())
                    .collect();
                Product {
                    product_id: number(product_id),
                    category: number(category),
                    name,
                    description,
                    item_type: number(item_type),
                    amount: number(amount),
                    vip_days: number(vip_days),
                    price_cps: stepped_price(number(price_cps), &steps, number(bought)),
                    stock_left: daily_stock.map(|stock| number(stock.saturating_sub(sold.unwrap_or(0)))),
                }
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
    let Some(product) = catalog(pool, character.database_id())
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
        match character.add_ground_item(pool, product.item_type, crate::shop::fresh_durability(product.item_type).0, crate::shop::fresh_durability(product.item_type).1).await? {
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

/// Purchases shown in the Store's history.
pub const HISTORY_LENGTH: i64 = 10;

/// A past purchase: the product's name, its price and when (seconds since
/// 1970).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PastPurchase {
    pub name: String,
    pub price_cps: u32,
    pub bought_at: u64,
}

/// A character's latest purchases, newest first.
pub async fn history(pool: &sqlx::PgPool, character_id: i32) -> Result<Vec<PastPurchase>, Error> {
    let rows: Vec<(String, i64, i64)> = sqlx::query_as(
        "SELECT p.name, s.price_cps, s.bought_at
         FROM store_purchases s JOIN store_products p ON p.product_id = s.product_id
         WHERE s.character_id = $1
         ORDER BY s.purchase_id DESC
         LIMIT $2",
    )
    .bind(i64::from(character_id))
    .bind(HISTORY_LENGTH)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(name, price_cps, bought_at)| PastPurchase {
            name,
            price_cps: u32::try_from(price_cps).unwrap_or_default(),
            bought_at: u64::try_from(bought_at).unwrap_or_default(),
        })
        .collect())
}

/// The account's VIP end (seconds since 1970); 0 without VIP.
pub async fn vip_until(pool: &sqlx::PgPool, account_id: u32) -> Result<u64, Error> {
    let until: Option<i64> = sqlx::query_scalar("SELECT vip_until FROM accounts WHERE account_id = $1")
        .bind(i64::from(account_id))
        .fetch_optional(pool)
        .await?;
    Ok(until.and_then(|until| u64::try_from(until).ok()).unwrap_or_default())
}

/// The VIP loot filter: the kinds of drops that go straight to the bag.
pub mod loot {
    pub const SILVER: u32 = 1;
    pub const RARES: u32 = 2;
    pub const GEMS: u32 = 4;
    pub const POTIONS: u32 = 8;
    pub const UNIQUE_GEAR: u32 = 16;
    pub const ALL_GEAR: u32 = 32;
    pub const ALL: u32 = 63;
    /// Silver, Meteors and DragonBalls.
    pub const DEFAULT: u32 = SILVER | RARES;

    fn is_gear(item_id: u32) -> bool {
        (110_000..700_000).contains(&item_id) || (900_000..901_000).contains(&item_id)
    }

    /// Whether `filter` sends a drop (an item, or `silver` > 0) to the bag.
    pub fn picks(filter: u32, item_id: u32, silver: u32) -> bool {
        if silver > 0 {
            return filter & SILVER != 0;
        }
        if matches!(item_id, crate::drops::METEOR | crate::drops::DRAGONBALL) {
            return filter & RARES != 0;
        }
        if (700_000..701_000).contains(&item_id) {
            return filter & GEMS != 0;
        }
        if (1_000_000..1_003_000).contains(&item_id) {
            return filter & POTIONS != 0;
        }
        if is_gear(item_id) {
            return filter & ALL_GEAR != 0 || filter & UNIQUE_GEAR != 0 && item_id % 10 >= 7;
        }
        false
    }
}

/// A VIP teleport destination: a city and where in it.
pub struct City {
    pub name: &'static str,
    pub map_id: u32,
    pub position: (u16, u16),
}

/// Where a VIP can teleport, as the city scrolls.
pub const CITIES: [City; 6] = [
    City { name: "Twin City", map_id: 1002, position: (430, 380) },
    City { name: "Phoenix Castle", map_id: 1011, position: (193, 266) },
    City { name: "Ape Mountain", map_id: 1020, position: (566, 565) },
    City { name: "Desert City", map_id: 1000, position: (496, 650) },
    City { name: "Bird Island", map_id: 1015, position: (717, 571) },
    City { name: "Market", map_id: 1036, position: (211, 196) },
];

/// Seconds between two VIP teleports.
pub const TELEPORT_COOLDOWN: u64 = 60;

/// A character's loot filter.
pub async fn loot_filter(pool: &sqlx::PgPool, character_id: i32) -> Result<u32, Error> {
    let filter: Option<i64> = sqlx::query_scalar("SELECT loot_filter FROM characters WHERE character_id = $1")
        .bind(i64::from(character_id))
        .fetch_optional(pool)
        .await?;
    Ok(filter.and_then(|filter| u32::try_from(filter).ok()).unwrap_or(loot::DEFAULT) & loot::ALL)
}

pub async fn save_loot_filter(pool: &sqlx::PgPool, character_id: i32, filter: u32) -> Result<(), Error> {
    sqlx::query("UPDATE characters SET loot_filter = $2 WHERE character_id = $1")
        .bind(i64::from(character_id))
        .bind(i64::from(filter & loot::ALL))
        .execute(pool)
        .await?;
    Ok(())
}

/// The VIP of the characters online, by entity id: when it ends, their loot
/// filter, and when they may teleport again.
#[derive(Debug, Default)]
pub struct VipBook {
    until: Mutex<HashMap<u32, u64>>,
    filters: Mutex<HashMap<u32, u32>>,
    teleport_ready: Mutex<HashMap<u32, u64>>,
}

impl VipBook {
    pub fn loot_filter(&self, character_id: u32) -> u32 {
        self.filters.lock().get(&character_id).copied().unwrap_or(loot::DEFAULT)
    }

    pub fn set_loot_filter(&self, character_id: u32, filter: u32) {
        self.filters.lock().insert(character_id, filter & loot::ALL);
    }

    /// When the character may teleport again (seconds since 1970).
    pub fn teleport_ready_at(&self, character_id: u32) -> u64 {
        self.teleport_ready.lock().get(&character_id).copied().unwrap_or_default()
    }

    /// Starts the cooldown; false while it is still running.
    pub fn take_teleport(&self, character_id: u32, now: u64) -> bool {
        let mut ready = self.teleport_ready.lock();
        if ready.get(&character_id).is_some_and(|at| *at > now) {
            return false;
        }
        ready.insert(character_id, now + TELEPORT_COOLDOWN);
        true
    }

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
        self.filters.lock().remove(&character_id);
        self.teleport_ready.lock().remove(&character_id);
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
    fn the_loot_filter_sorts_drops() {
        use super::loot::*;
        assert!(picks(DEFAULT, 0, 120) && picks(DEFAULT, crate::drops::METEOR, 0));
        assert!(!picks(DEFAULT, 700_001, 0) && picks(GEMS, 700_001, 0));
        assert!(!picks(DEFAULT, 1_000_000, 0) && picks(POTIONS, 1_002_050, 0));
        // A Unique blade (quality 7) and a Normal one (3).
        assert!(picks(UNIQUE_GEAR, 410_017, 0) && !picks(UNIQUE_GEAR, 410_013, 0));
        assert!(picks(ALL_GEAR, 410_013, 0) && picks(ALL_GEAR, 900_005, 0));
        assert!(!picks(ALL, 723_700, 0), "nothing else");
        assert!(!picks(0, 0, 120));
    }

    #[test]
    fn teleports_wait_a_minute() {
        let book = VipBook::default();
        assert!(book.take_teleport(7, 1_000));
        assert!(!book.take_teleport(7, 1_059));
        assert_eq!(book.teleport_ready_at(7), 1_060);
        assert!(book.take_teleport(7, 1_060));
        book.set_loot_filter(7, 0xff);
        assert_eq!(book.loot_filter(7), super::loot::ALL);
        book.remove(7);
        assert_eq!((book.loot_filter(7), book.teleport_ready_at(7)), (super::loot::DEFAULT, 0));
    }

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
        assert_eq!((meteor.cps_left, character.cps(), meteor.items.len()), (990, 990, 1));
        assert_eq!(meteor.items[0].item_type, 1_088_001);

        let vip = buy(&pool, &character, 1, 1).await?.expect("VIP");
        let week = vip.vip_until.expect("VIP until");
        assert!(week >= now_secs() + 7 * 86_400 - 5);
        assert_eq!(vip_until(&pool, 1).await?, week);

        assert_eq!(buy(&pool, &character, 1, 2).await?, Err(Refusal::NotEnoughCps(1_450)));
        assert_eq!(character.cps(), 540, "a refused purchase costs nothing");
        assert_eq!(buy(&pool, &character, 1, 999).await?, Err(Refusal::NoSuchProduct));

        let past = history(&pool, character.database_id()).await?;
        let names: Vec<_> = past.iter().map(|purchase| (purchase.name.as_str(), purchase.price_cps)).collect();
        assert_eq!(names, [("7 Days of VIP", 450), ("Meteor", 10)], "newest first, refusals left out");

        assert_eq!(loot_filter(&pool, character.database_id()).await?, loot::DEFAULT);
        save_loot_filter(&pool, character.database_id(), loot::GEMS | loot::SILVER).await?;
        assert_eq!(loot_filter(&pool, character.database_id()).await?, loot::GEMS | loot::SILVER);
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
        let ball = catalog(&pool, character.database_id()).await?.into_iter().find(|product| product.product_id == 11).expect("Exp Ball");
        assert_eq!(ball.stock_left, Some(0));
        assert_eq!(character.cps(), 100_000 - 600);
        Ok(())
    }

    #[test]
    fn a_stepped_price_climbs_with_the_days_purchases() {
        let steps = [30, 45, 90, 160, 290];
        let prices: Vec<u32> = (0..7).map(|bought| stepped_price(30, &steps, bought)).collect();
        assert_eq!(prices, [30, 45, 90, 160, 290, 520, 940]);
        assert_eq!(stepped_price(60, &[], 5), 60, "no steps: one price");
    }

    #[tokio::test]
    async fn the_exp_potion_costs_more_each_time_today() -> Result<(), Error> {
        let pool = tq_db::testing::pool().await;
        let character = character(&pool, 5_000).await;
        let price = |products: Vec<Product>| products.into_iter().find(|product| product.product_id == 10).map(|product| product.price_cps);
        assert_eq!(price(catalog(&pool, character.database_id()).await?), Some(150));
        for paid in [150, 225, 450] {
            let potion = buy(&pool, &character, 1, 10).await?.expect("an Exp Potion");
            assert_eq!(potion.product.price_cps, paid);
        }
        assert_eq!(price(catalog(&pool, character.database_id()).await?), Some(800), "the fourth");
        assert_eq!(character.cps(), 5_000 - 825);
        Ok(())
    }
}
