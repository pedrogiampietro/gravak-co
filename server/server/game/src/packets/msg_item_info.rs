use num_enum::FromPrimitive;
use serde::Serialize;
use tq_network::PacketID;

#[derive(Debug, FromPrimitive)]
#[repr(u8)]
pub enum ItemInfoAction {
    #[num_enum(default)]
    None = 0,
    AddItem = 1,
    Trade = 2,
    Update = 3,
    OtherPlayerEquipement = 4,
}

/// This packet is sent server>client to add or update the attributes of a
/// specific item.
#[derive(Debug, Serialize, Clone, PacketID, Default)]
#[packet(id = 1008)]
pub struct MsgItemInfo {
    character_id: u32,
    item_id: u32,
    durability: u16,
    max_durability: u16,
    action: u8,
    ident: u8, // always 0
    position: u8,
    /// Unknown
    reserved0: u8,
    reserved1: u32,
    gem_one: u8,
    gem_two: u8,
    reborn_effect: u8,
    magic: u8,
    plus: u8,
    blees: u8,
    enchant: u8,
    reserved2: u8,
    restrain: u32,
    reserved3: u32,
    reserved4: u32,
}

impl MsgItemInfo {
    /// The 5065 wire format uses 0 for a closed socket, 255 for an open empty
    /// socket, and the gem code for an occupied socket.
    fn gem_wire(socket: u8, gem: u8) -> u8 {
        if gem != 0 {
            gem
        } else if socket != 0 {
            255
        } else {
            0
        }
    }

    /// An item another player wears, for the window that views their
    /// equipment (`OtherPlayerEquipement`).
    pub fn worn_by_other(item: crate::entities::InventoryItem) -> Self {
        Self { action: ItemInfoAction::OtherPlayerEquipement as u8, ..Self::inventory_item(item) }
    }

    pub fn inventory_item(item: crate::entities::InventoryItem) -> Self {
        Self {
            character_id: item.uid,
            item_id: item.item_type,
            durability: item.durability,
            max_durability: item.max_durability,
            action: ItemInfoAction::AddItem as u8,
            position: item.position,
            gem_one: Self::gem_wire(item.socket_one, item.gem_one),
            gem_two: Self::gem_wire(item.socket_two, item.gem_two),
            plus: item.plus,
            blees: item.rarity,
            enchant: item.enchant_hp,
            reserved2: item.bless,
            // The custom rarity seed rides in a reserved word so the
            // classic enchant byte carries the HP enchantment.
            reserved3: u32::from(item.rarity_roll),
            reserved4: u32::from(item.color),
            // The weapon's skin (the look it wears), 0 for none.
            reserved1: item.skin,
            ..Self::default()
        }
    }

    pub fn equipment_item(item: crate::entities::InventoryItem) -> Self {
        Self {
            character_id: item.uid,
            item_id: item.item_type,
            durability: item.durability,
            max_durability: item.max_durability,
            // The current client uses action 8 for the explicit equipment
            // stream so inventory positions 1..9 remain unambiguous.
            action: 8,
            position: item.position,
            gem_one: Self::gem_wire(item.socket_one, item.gem_one),
            gem_two: Self::gem_wire(item.socket_two, item.gem_two),
            plus: item.plus,
            blees: item.rarity,
            enchant: item.enchant_hp,
            reserved2: item.bless,
            // The custom rarity seed rides in a reserved word so the
            // classic enchant byte carries the HP enchantment.
            reserved3: u32::from(item.rarity_roll),
            reserved4: u32::from(item.color),
            // The weapon's skin (the look it wears), 0 for none.
            reserved1: item.skin,
            ..Self::default()
        }
    }

    pub fn remove_equipment_item(item: crate::entities::InventoryItem, equipment_slot: u8) -> Self {
        Self {
            character_id: item.uid,
            item_id: item.item_type,
            action: 9,
            position: equipment_slot,
            ..Self::default()
        }
    }

    pub fn remove_inventory_item(item: crate::entities::InventoryItem) -> Self {
        Self {
            character_id: item.uid,
            item_id: item.item_type,
            action: 5,
            position: item.position,
            ..Self::default()
        }
    }

    pub fn trade_item(item: crate::entities::InventoryItem) -> Self {
        Self {
            character_id: item.uid,
            item_id: item.item_type,
            durability: item.durability,
            max_durability: item.max_durability,
            action: ItemInfoAction::Trade as u8,
            position: item.position,
            gem_one: Self::gem_wire(item.socket_one, item.gem_one),
            gem_two: Self::gem_wire(item.socket_two, item.gem_two),
            plus: item.plus,
            blees: item.rarity,
            enchant: item.enchant_hp,
            reserved2: item.bless,
            // The custom rarity seed rides in a reserved word so the
            // classic enchant byte carries the HP enchantment.
            reserved3: u32::from(item.rarity_roll),
            reserved4: u32::from(item.color),
            // The weapon's skin (the look it wears), 0 for none.
            reserved1: item.skin,
            ..Self::default()
        }
    }

    pub fn warehouse_item(item: crate::entities::InventoryItem) -> Self {
        Self {
            character_id: item.uid,
            item_id: item.item_type,
            durability: item.durability,
            max_durability: item.max_durability,
            action: ItemInfoAction::AddItem as u8,
            position: 40 + item.position,
            gem_one: Self::gem_wire(item.socket_one, item.gem_one),
            gem_two: Self::gem_wire(item.socket_two, item.gem_two),
            plus: item.plus,
            blees: item.rarity,
            enchant: item.enchant_hp,
            reserved2: item.bless,
            // The custom rarity seed rides in a reserved word so the
            // classic enchant byte carries the HP enchantment.
            reserved3: u32::from(item.rarity_roll),
            reserved4: u32::from(item.color),
            // The weapon's skin (the look it wears), 0 for none.
            reserved1: item.skin,
            ..Self::default()
        }
    }

    pub fn remove_warehouse_item(item: crate::entities::InventoryItem) -> Self {
        Self {
            character_id: item.uid,
            item_id: item.item_type,
            action: 5,
            position: 40 + item.position,
            ..Self::default()
        }
    }

    pub fn guild_warehouse_item(item: crate::entities::InventoryItem) -> Self {
        Self {
            character_id: item.uid,
            item_id: item.item_type,
            durability: item.durability,
            max_durability: item.max_durability,
            action: ItemInfoAction::AddItem as u8,
            position: 80 + item.position,
            gem_one: Self::gem_wire(item.socket_one, item.gem_one),
            gem_two: Self::gem_wire(item.socket_two, item.gem_two),
            plus: item.plus,
            blees: item.rarity,
            enchant: item.enchant_hp,
            reserved2: item.bless,
            // The custom rarity seed rides in a reserved word so the
            // classic enchant byte carries the HP enchantment.
            reserved3: u32::from(item.rarity_roll),
            reserved4: u32::from(item.color),
            // The weapon's skin (the look it wears), 0 for none.
            reserved1: item.skin,
            ..Self::default()
        }
    }

    pub fn remove_guild_warehouse_item(item: crate::entities::InventoryItem) -> Self {
        Self {
            character_id: item.uid,
            item_id: item.item_type,
            action: 5,
            position: 80 + item.position,
            ..Self::default()
        }
    }

    pub fn clear_guild_warehouse(guild_id: u32) -> Self {
        Self {
            character_id: guild_id,
            action: 6,
            position: 80,
            ..Self::default()
        }
    }

    pub fn clear_warehouse(npc_id: u32) -> Self {
        Self {
            character_id: npc_id,
            action: 6,
            position: 40,
            ..Self::default()
        }
    }

    pub fn warehouse_balance(npc_id: u32, silver: u32) -> Self {
        Self {
            character_id: npc_id,
            action: 7,
            position: 40,
            reserved1: silver,
            ..Self::default()
        }
    }
}
