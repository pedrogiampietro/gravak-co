use parking_lot::Mutex;
use std::collections::HashMap;

pub const MAX_LISTINGS: usize = 10;
/// The longest line a booth hawks (the original's DialogHawk box).
pub const MAX_NAME: usize = 60;

/// A booth's hawk line as typed: trimmed, printable, at most [`MAX_NAME`]
/// letters; `None` when nothing is left.
pub fn clean_name(name: &str) -> Option<String> {
    let name: String = name
        .chars()
        .filter(|character| !character.is_control())
        .take(MAX_NAME)
        .collect();
    let name = name.trim();
    (!name.is_empty()).then(|| name.to_owned())
}

/// The name a character's booth had, if they ever named one.
pub async fn saved_name(pool: &sqlx::PgPool, character_id: i32) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar("SELECT name FROM player_shops WHERE character_id = $1")
        .bind(i64::from(character_id))
        .fetch_optional(pool)
        .await
}

pub async fn save_name(pool: &sqlx::PgPool, character_id: i32, name: &str) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO player_shops (character_id, name) VALUES ($1, $2)
         ON CONFLICT (character_id) DO UPDATE SET name = EXCLUDED.name",
    )
    .bind(i64::from(character_id))
    .bind(name)
    .execute(pool)
    .await?;
    Ok(())
}

/// A character's saved listings.
pub async fn saved_listings(pool: &sqlx::PgPool, character_id: i32) -> Result<Vec<ShopListing>, sqlx::Error> {
    let rows: Vec<(i64, i64, bool)> = sqlx::query_as(
        "SELECT item_uid, price, cps FROM player_shop_listings WHERE character_id = $1 ORDER BY item_uid",
    )
    .bind(i64::from(character_id))
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .filter_map(|(uid, price, cps)| {
            Some(ShopListing {
                item_uid: u32::try_from(uid).ok()?,
                price: u32::try_from(price).ok()?,
                cps,
            })
        })
        .collect())
}

pub async fn save_listing(pool: &sqlx::PgPool, character_id: i32, listing: ShopListing) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO player_shop_listings (item_uid, character_id, price, cps) VALUES ($1, $2, $3, $4)
         ON CONFLICT (item_uid) DO UPDATE SET character_id = EXCLUDED.character_id, price = EXCLUDED.price, cps = EXCLUDED.cps",
    )
    .bind(i64::from(listing.item_uid))
    .bind(i64::from(character_id))
    .bind(i64::from(listing.price))
    .bind(listing.cps)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn forget_listing(pool: &sqlx::PgPool, item_uid: u32) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM player_shop_listings WHERE item_uid = $1")
        .bind(i64::from(item_uid))
        .execute(pool)
        .await?;
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShopListing {
    pub item_uid: u32,
    pub price: u32,
    /// Priced in CPs rather than silver.
    pub cps: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerShopView {
    pub owner_id: u32,
    pub map_id: u32,
    pub listings: Vec<ShopListing>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerShopError {
    AlreadyOpen,
    NotOpen,
    InvalidPrice,
    TooManyListings,
    ItemAlreadyListed,
    ItemNotListed,
}

impl PlayerShopError {
    pub fn message(self) -> &'static str {
        match self {
            Self::AlreadyOpen => "Player shop is already open.",
            Self::NotOpen => "Open a player shop first.",
            Self::InvalidPrice => "Player shop prices must be greater than zero.",
            Self::TooManyListings => "Player shops can list at most ten items.",
            Self::ItemAlreadyListed => "That item is already listed in the shop.",
            Self::ItemNotListed => "That item is not listed in the shop.",
        }
    }
}

#[derive(Debug, Default)]
struct ShopBook {
    shops: HashMap<u32, PlayerShopView>,
}

/// Runtime player-vendor state. Listings are ephemeral: the item remains in
/// the seller's inventory until a purchase transaction moves it atomically.
#[derive(Debug, Default)]
pub struct PlayerShopManager {
    book: Mutex<ShopBook>,
}

impl PlayerShopManager {
    pub fn open(&self, owner_id: u32, map_id: u32) -> Result<PlayerShopView, PlayerShopError> {
        let mut book = self.book.lock();
        if book.shops.contains_key(&owner_id) {
            return Err(PlayerShopError::AlreadyOpen);
        }
        let shop = PlayerShopView {
            owner_id,
            map_id,
            listings: Vec::new(),
        };
        book.shops.insert(owner_id, shop.clone());
        Ok(shop)
    }

    pub fn close(&self, owner_id: u32) -> Option<PlayerShopView> {
        self.book.lock().shops.remove(&owner_id)
    }

    /// Whose booths are open.
    pub fn owners(&self) -> Vec<u32> {
        self.book.lock().shops.keys().copied().collect()
    }

    pub fn view(&self, owner_id: u32) -> Option<PlayerShopView> {
        self.book.lock().shops.get(&owner_id).cloned()
    }

    pub fn add_item(&self, owner_id: u32, item_uid: u32, price: u32, cps: bool) -> Result<ShopListing, PlayerShopError> {
        if price == 0 {
            return Err(PlayerShopError::InvalidPrice);
        }
        let mut book = self.book.lock();
        let shop = book.shops.get_mut(&owner_id).ok_or(PlayerShopError::NotOpen)?;
        if shop.listings.len() >= MAX_LISTINGS {
            return Err(PlayerShopError::TooManyListings);
        }
        if shop.listings.iter().any(|listing| listing.item_uid == item_uid) {
            return Err(PlayerShopError::ItemAlreadyListed);
        }
        let listing = ShopListing { item_uid, price, cps };
        shop.listings.push(listing);
        Ok(listing)
    }

    pub fn remove_item(&self, owner_id: u32, item_uid: u32) -> Result<ShopListing, PlayerShopError> {
        let mut book = self.book.lock();
        let shop = book.shops.get_mut(&owner_id).ok_or(PlayerShopError::NotOpen)?;
        let index = shop
            .listings
            .iter()
            .position(|listing| listing.item_uid == item_uid)
            .ok_or(PlayerShopError::ItemNotListed)?;
        Ok(shop.listings.remove(index))
    }

    /// Reserve one listing before the asynchronous purchase transaction. A
    /// caller must restore it if the transaction cannot complete.
    pub fn take_item(&self, owner_id: u32, item_uid: u32) -> Result<ShopListing, PlayerShopError> {
        self.remove_item(owner_id, item_uid)
    }

    pub fn restore_item(&self, owner_id: u32, listing: ShopListing) -> Result<(), PlayerShopError> {
        let mut book = self.book.lock();
        let shop = book.shops.get_mut(&owner_id).ok_or(PlayerShopError::NotOpen)?;
        if shop.listings.len() >= MAX_LISTINGS
            || shop
                .listings
                .iter()
                .any(|existing| existing.item_uid == listing.item_uid)
        {
            return Err(PlayerShopError::ItemAlreadyListed);
        }
        shop.listings.push(listing);
        Ok(())
    }

    pub fn remove_member(&self, owner_id: u32) {
        self.book.lock().shops.remove(&owner_id);
    }
}

#[cfg(test)]
mod tests {
    use super::{PlayerShopError, PlayerShopManager, MAX_LISTINGS};

    #[test]
    fn booth_names_are_cleaned() {
        assert_eq!(super::clean_name("  Pedro's Gems  ").as_deref(), Some("Pedro's Gems"));
        assert_eq!(super::clean_name("   "), None);
        assert_eq!(super::clean_name("a\u{7}b").as_deref(), Some("ab"));
        assert_eq!(super::clean_name(&"x".repeat(80)).map(|name| name.len()), Some(super::MAX_NAME));
    }

    #[tokio::test]
    async fn names_and_listings_are_kept() -> Result<(), sqlx::Error> {
        let pool = tq_db::testing::pool().await;
        let id = crate::packets::MsgRegister::build_character_with(
            "booth-test".to_owned(),
            crate::packets::BodyType::MuscularMale,
            crate::packets::BaseClass::Trojan,
            1,
            1,
        )
        .expect("character")
        .save(&pool)
        .await
        .expect("saved");
        assert_eq!(super::saved_name(&pool, id).await?, None);
        super::save_name(&pool, id, "Gems").await?;
        super::save_name(&pool, id, "Cheap Gems").await?;
        assert_eq!(super::saved_name(&pool, id).await?.as_deref(), Some("Cheap Gems"));
        let listing = super::ShopListing { item_uid: 77, price: 900, cps: false };
        super::save_listing(&pool, id, listing).await?;
        super::save_listing(&pool, id, super::ShopListing { price: 800, cps: true, ..listing }).await?;
        assert_eq!(
            super::saved_listings(&pool, id).await?,
            [super::ShopListing { item_uid: 77, price: 800, cps: true }]
        );
        super::forget_listing(&pool, 77).await?;
        assert!(super::saved_listings(&pool, id).await?.is_empty());
        Ok(())
    }

    #[test]
    fn shop_lifecycle_limits_and_reserves_listings() {
        let manager = PlayerShopManager::default();
        let opened = manager.open(10, 1002).unwrap();
        assert_eq!(opened.owner_id, 10);
        assert_eq!(manager.open(10, 1002), Err(PlayerShopError::AlreadyOpen));
        assert_eq!(manager.add_item(10, 99, 500, false).unwrap().price, 500);
        assert_eq!(manager.add_item(10, 99, 700, true), Err(PlayerShopError::ItemAlreadyListed));
        let reserved = manager.take_item(10, 99).unwrap();
        assert_eq!(manager.view(10).unwrap().listings.len(), 0);
        manager.restore_item(10, reserved).unwrap();
        assert_eq!(manager.view(10).unwrap().listings.len(), 1);
        manager.close(10);
        assert!(manager.view(10).is_none());
    }

    #[test]
    fn shop_rejects_zero_price_and_more_than_ten_items() {
        let manager = PlayerShopManager::default();
        manager.open(10, 1002).unwrap();
        assert_eq!(manager.add_item(10, 1, 0, false), Err(PlayerShopError::InvalidPrice));
        for uid in 1..=MAX_LISTINGS as u32 {
            manager.add_item(10, uid, uid, uid % 2 == 0).unwrap();
        }
        assert_eq!(manager.add_item(10, 99, 1, false), Err(PlayerShopError::TooManyListings));
    }
}
