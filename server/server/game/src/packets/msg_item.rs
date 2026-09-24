use super::{MsgItemInfo, MsgTalk, MsgUserInfo, TalkChannel};
use crate::ActorState;
use crate::entities::{NpcKind, PurchaseOutcome, SaleOutcome};
use crate::state::State;
use async_trait::async_trait;
use num_enum::{FromPrimitive, IntoPrimitive};
use serde::{Deserialize, Serialize};
use tq_network::{Actor, PacketID, PacketProcess};

/// Enumeration type for defining item actions that may be requested by the
/// user, or given to by the server. Allows for action handling as a packet
/// subtype. Enums should be named by the action they provide to a system in the
/// context of the player item.
#[derive(Default, Debug, FromPrimitive, IntoPrimitive, Clone, Copy)]
#[repr(u32)]
enum ItemActionType {
    #[default]
    Unknown,
    Buy = 1,
    Sell = 2,
    Drop = 3,
    Use = 4,
    Equip = 5,
    Unequip = 6,
    SplitItem = 7,
    CombineItem = 8,
    QueryMoneySaved = 9,
    SaveMoney = 10,
    DrawMoney = 11,
    DropMoney = 12,
    SpendMoney = 13,
    Repair = 14,
    RepairAll = 15,
    Ident = 16,
    Durability = 17,
    DropEquipement = 18,
    Improve = 19,
    UpLevel = 20,
    BoothQuery = 21,
    BoothAdd = 22,
    BoothDel = 23,
    BoothBuy = 24,
    SynchroAmount = 25,
    Fireworks = 26,
    Ping = 27,
    Enchant = 28,
    BoothAddCPs = 29,
}

/// Message containing an item action command. Item actions are usually
/// performed to manage player equipment, inventory, money, or item shop
/// purchases and sales. It is serves a second purpose for measuring client
/// ping.
#[derive(Debug, Serialize, Deserialize, Clone, PacketID)]
#[packet(id = 1009)]
pub struct MsgItem {
    character_id: u32,
    param0: u32,
    action_type: u32,
    client_timestamp: u32,
    amount: u32,
    param1: u32,
}

#[async_trait]
impl PacketProcess for MsgItem {
    type ActorState = ActorState;
    type Error = crate::Error;
    type State = State;

    async fn process(&self, state: &Self::State, actor: &Actor<Self::ActorState>) -> Result<(), Self::Error> {
        let action = self.action_type.into();
        match action {
            ItemActionType::Ping => {
                // a bit hacky, just testing it out.
                // what if we missed with the client timestamp?
                // does this yield a negative value? let's find out.
                // lets add 30ms from the client timestamp, so when
                // the client receives the packet, it can calculate
                // the round trip time.
                let msg = MsgItem {
                    character_id: self.character_id,
                    param0: self.param0,
                    action_type: self.action_type,
                    client_timestamp: self.client_timestamp + 30,
                    amount: self.amount,
                    param1: self.param1,
                };
                // LMFAO, this is so bad. it actually made the ping appear
                // negative. I'm not sure if this is a bug in the client
                // or if it's a bug in the server. I'm going to remove this
                // later, but I'm going to leave it here for now.
                actor.send(msg).await?;
            },
            ItemActionType::Buy => {
                let entity = actor.entity();
                let character = entity.as_character().ok_or(crate::Error::CharacterNotFound)?;
                let reject = |message: &str| MsgTalk::from_system(character.id(), TalkChannel::TopLeft, message);
                if self.character_id != character.id() {
                    actor
                        .send(reject("Shop purchase rejected: character mismatch."))
                        .await?;
                    return Ok(());
                }
                if self.amount != 1 {
                    actor
                        .send(reject("Shop purchase rejected: buy one item at a time."))
                        .await?;
                    return Ok(());
                }
                let map = state.try_map(character.entity().map_id())?;
                let shop_id = character.active_shop_id();
                let Some(npc) = map.npc(shop_id) else {
                    actor
                        .send(reject("Shop purchase rejected: shop is not nearby."))
                        .await?;
                    return Ok(());
                };
                let player_location = character.entity().location();
                let npc_location = npc.entity().location();
                if npc.kind() != NpcKind::ShopKeeper
                    || player_location.x.abs_diff(npc_location.x) > 12
                    || player_location.y.abs_diff(npc_location.y) > 12
                {
                    actor
                        .send(reject("Shop purchase rejected: shop is not nearby."))
                        .await?;
                    return Ok(());
                }
                let Some(offer) = state.shop_catalog().offer(npc.id()) else {
                    actor
                        .send(reject("Shop purchase rejected: no catalog for this shop."))
                        .await?;
                    return Ok(());
                };
                if !offer.item_ids.contains(&self.param0) {
                    actor
                        .send(reject("Shop purchase rejected: item is not sold here."))
                        .await?;
                    return Ok(());
                }
                let Some(template) = state.shop_catalog().item(self.param0) else {
                    actor
                        .send(reject("Shop purchase rejected: item price is unavailable."))
                        .await?;
                    return Ok(());
                };
                let Ok(inventory_slot) = u8::try_from(self.param1) else {
                    actor
                        .send(reject("Shop purchase rejected: invalid inventory slot."))
                        .await?;
                    return Ok(());
                };
                match character
                    .purchase_item(
                        state.pool(),
                        self.param0,
                        inventory_slot,
                        template.price,
                        template.durability,
                        template.max_durability,
                    )
                    .await?
                {
                    PurchaseOutcome::Bought(item) => {
                        actor.send(MsgItemInfo::inventory_item(item)).await?;
                        actor.send(MsgUserInfo::from(character)).await?;
                        actor
                            .send(MsgTalk::from_system(
                                character.id(),
                                TalkChannel::TopLeft,
                                format!("Purchased item {} for {} silver.", self.param0, template.price),
                            ))
                            .await?;
                    },
                    PurchaseOutcome::InsufficientSilver => {
                        actor.send(reject("Not enough silver to buy this item.")).await?;
                    },
                    PurchaseOutcome::InventoryFull => {
                        actor.send(reject("Inventory is full.")).await?;
                    },
                }
            },
            ItemActionType::Sell => {
                let entity = actor.entity();
                let character = entity.as_character().ok_or(crate::Error::CharacterNotFound)?;
                let reject = |message: &str| MsgTalk::from_system(character.id(), TalkChannel::TopLeft, message);
                if self.character_id != character.id() || self.amount != 1 {
                    actor.send(reject("Shop sale rejected: invalid character or amount.")).await?;
                    return Ok(());
                }
                let shop_id = character.active_shop_id();
                if self.param1 != shop_id {
                    actor.send(reject("Shop sale rejected: open a shop before selling.")).await?;
                    return Ok(());
                }
                let map = state.try_map(character.entity().map_id())?;
                let Some(npc) = map.npc(shop_id) else {
                    actor.send(reject("Shop sale rejected: shop is not nearby.")).await?;
                    return Ok(());
                };
                let player_location = character.entity().location();
                let npc_location = npc.entity().location();
                if npc.kind() != NpcKind::ShopKeeper
                    || player_location.x.abs_diff(npc_location.x) > 12
                    || player_location.y.abs_diff(npc_location.y) > 12
                {
                    actor.send(reject("Shop sale rejected: shop is not nearby.")).await?;
                    return Ok(());
                }
                let Some(item) = character
                    .inventory_items()
                    .into_iter()
                    .find(|item| item.uid == self.param0)
                else {
                    actor.send(reject("Shop sale rejected: item is not in your inventory.")).await?;
                    return Ok(());
                };
                let Some(silver) = state
                    .shop_catalog()
                    .sell_price(item.item_type, item.durability, item.max_durability)
                else {
                    actor.send(reject("This item has no resale value.")).await?;
                    return Ok(());
                };
                match character.sell_item(state.pool(), item.uid, silver).await? {
                    SaleOutcome::Sold { item, silver } => {
                        actor.send(MsgItemInfo::remove_inventory_item(item)).await?;
                        actor.send(MsgUserInfo::from(character)).await?;
                        actor
                            .send(MsgTalk::from_system(
                                character.id(),
                                TalkChannel::TopLeft,
                                format!("Sold item {} for {} silver.", item.item_type, silver),
                            ))
                            .await?;
                    }
                    SaleOutcome::ItemNotFound => {
                        actor.send(reject("Shop sale rejected: item is no longer available.")).await?;
                    }
                    SaleOutcome::NoValue => {
                        actor.send(reject("This item has no resale value.")).await?;
                    }
                    SaleOutcome::SilverLimit => {
                        actor.send(reject("Your silver balance is at the maximum limit.")).await?;
                    }
                }
            }
            _ => {
                actor.send(self.clone()).await?;
                let p = MsgTalk::from_system(
                    self.character_id,
                    TalkChannel::Service,
                    format!("Missing Item Action Type {:?}", action),
                );
                tracing::warn!(
                    ?action,
                    param0 = %self.param0,
                    param1 = %self.param1,
                    action_id = self.action_type,
                    "Missing Item Action Type",
                );
                actor.send(p).await?;
            },
        }
        Ok(())
    }
}
