use parking_lot::Mutex;
use std::collections::HashMap;

pub const MAX_LISTINGS: usize = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShopListing {
    pub item_uid: u32,
    pub price: u32,
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

    pub fn view(&self, owner_id: u32) -> Option<PlayerShopView> {
        self.book.lock().shops.get(&owner_id).cloned()
    }

    pub fn add_item(&self, owner_id: u32, item_uid: u32, price: u32) -> Result<ShopListing, PlayerShopError> {
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
        let listing = ShopListing { item_uid, price };
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
    fn shop_lifecycle_limits_and_reserves_listings() {
        let manager = PlayerShopManager::default();
        let opened = manager.open(10, 1002).unwrap();
        assert_eq!(opened.owner_id, 10);
        assert_eq!(manager.open(10, 1002), Err(PlayerShopError::AlreadyOpen));
        assert_eq!(manager.add_item(10, 99, 500).unwrap().price, 500);
        assert_eq!(manager.add_item(10, 99, 700), Err(PlayerShopError::ItemAlreadyListed));
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
        assert_eq!(manager.add_item(10, 1, 0), Err(PlayerShopError::InvalidPrice));
        for uid in 1..=MAX_LISTINGS as u32 {
            manager.add_item(10, uid, uid).unwrap();
        }
        assert_eq!(manager.add_item(10, 99, 1), Err(PlayerShopError::TooManyListings));
    }
}
