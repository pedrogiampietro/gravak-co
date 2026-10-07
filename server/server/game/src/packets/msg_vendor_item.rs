use crate::entities::InventoryItem;
use serde::Serialize;
use tq_network::PacketID;

/// A classic 1108 vendor listing sent after a player-shop query.
#[derive(Debug, Serialize, Clone, PacketID)]
#[packet(id = 1108)]
pub struct MsgVendorItem {
    pub item_uid: u32,
    pub owner_id: u32,
    pub price: u32,
    pub item_type: u32,
    pub durability: u16,
    pub max_durability: u16,
    pub view_type: u16,
    pub position: u8,
    pub gem_one: u8,
    pub gem_two: u8,
    pub plus: u8,
    pub reserved: u8,
}

/// How a listing is paid (`view_type`): silver or CPs.
pub const COST_SILVER: u16 = 1;
pub const COST_CPS: u16 = 3;

impl MsgVendorItem {
    pub fn new(owner_id: u32, item: InventoryItem, listing: crate::player_shop::ShopListing) -> Self {
        Self {
            item_uid: item.uid,
            owner_id,
            price: listing.price,
            item_type: item.item_type,
            durability: item.durability,
            max_durability: item.max_durability,
            view_type: if listing.cps { COST_CPS } else { COST_SILVER },
            position: item.position,
            gem_one: item.gem_one,
            gem_two: item.gem_two,
            plus: item.plus,
            reserved: 0,
        }
    }
}
