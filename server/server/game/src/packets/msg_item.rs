use super::{MsgGroundItem, MsgItemInfo, MsgPlayer, MsgTalk, MsgUserInfo, TalkChannel};
use crate::entities::{NpcKind, PurchaseOutcome, SaleOutcome};
use crate::shop::TemporaryEffect;
use crate::state::State;
use crate::ActorState;
use async_trait::async_trait;
use num_enum::{FromPrimitive, IntoPrimitive};
use serde::{Deserialize, Serialize};
use std::fmt::Display;
use std::fs::OpenOptions;
use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};
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
    MoveInventory = 30,
    OpenSocket = 43,
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

fn dragdrop_log(message: impl Display) {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default();
    let line = format!("[{timestamp}] [DRAGDROP-SERVER] {message}");
    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open("dragdrop-server-debug.log")
    {
        let _ = writeln!(file, "{line}");
    }
}

fn item_audit_log(action: &str, character_id: u32, item_uid: u32, item_type: u32, details: impl std::fmt::Display) {
    crate::audit::item(action, character_id, item_uid, item_type, details);
}

pub(super) fn economic_audit_log(action: &str, actor_id: u32, counterparty_id: u32, item_uid: u32, price: u32) {
    crate::audit::economic(action, actor_id, counterparty_id, item_uid, price);
}

#[async_trait]
impl PacketProcess for MsgItem {
    type ActorState = ActorState;
    type Error = crate::Error;
    type State = State;

    async fn process(&self, state: &Self::State, actor: &Actor<Self::ActorState>) -> Result<(), Self::Error> {
        dragdrop_log(format_args!(
            "item action received character_id={} item_uid={} action_type={} amount={} param1={}",
            self.character_id, self.param0, self.action_type, self.amount, self.param1
        ));
        tracing::info!(
            character_id = self.character_id,
            item_uid = self.param0,
            action_type = self.action_type,
            amount = self.amount,
            param1 = self.param1,
            "item action received"
        );
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
                        economic_audit_log("npc_buy", character.id(), npc.id(), item.uid, template.price);
                        item_audit_log(
                            "buy",
                            character.id(),
                            item.uid,
                            item.item_type,
                            format_args!("price={} slot={}", template.price, item.position),
                        );
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
            ItemActionType::BoothAdd | ItemActionType::BoothAddCPs => {
                let entity = actor.entity();
                let character = entity.as_character().ok_or(crate::Error::CharacterNotFound)?;
                if self.character_id != character.id() {
                    actor
                        .send(MsgTalk::from_system(
                            character.id(),
                            TalkChannel::System,
                            "Player shop rejected: character mismatch.",
                        ))
                        .await?;
                    return Ok(());
                }
                let Some(item) = character
                    .inventory_items()
                    .into_iter()
                    .find(|item| item.uid == self.param0)
                else {
                    actor
                        .send(MsgTalk::from_system(
                            character.id(),
                            TalkChannel::System,
                            "Player shop rejected: item is not in your inventory.",
                        ))
                        .await?;
                    return Ok(());
                };
                match state
                    .player_shop_manager()
                    .add_item(character.id(), item.uid, self.amount)
                {
                    Ok(_) => {
                        economic_audit_log(
                            "player_shop_list",
                            character.id(),
                            character.id(),
                            item.uid,
                            self.amount,
                        );
                        actor.send(self.clone()).await?;
                        actor
                            .send(super::MsgVendorItem::new(character.id(), item, self.amount))
                            .await?;
                        actor
                            .send(MsgTalk::from_system(
                                character.id(),
                                TalkChannel::System,
                                "Item added to player shop.",
                            ))
                            .await?;
                    },
                    Err(error) => {
                        actor
                            .send(MsgTalk::from_system(
                                character.id(),
                                TalkChannel::System,
                                error.message(),
                            ))
                            .await?;
                    },
                }
            },
            ItemActionType::BoothDel => {
                let entity = actor.entity();
                let character = entity.as_character().ok_or(crate::Error::CharacterNotFound)?;
                if self.character_id != character.id() {
                    return Ok(());
                }
                match state.player_shop_manager().remove_item(character.id(), self.param0) {
                    Ok(listing) => {
                        economic_audit_log(
                            "player_shop_remove",
                            character.id(),
                            character.id(),
                            listing.item_uid,
                            listing.price,
                        );
                        actor.send(self.clone()).await?;
                    },
                    Err(error) => {
                        actor
                            .send(MsgTalk::from_system(
                                character.id(),
                                TalkChannel::System,
                                error.message(),
                            ))
                            .await?;
                    },
                }
            },
            ItemActionType::BoothQuery => {
                let entity = actor.entity();
                let buyer = entity.as_character().ok_or(crate::Error::CharacterNotFound)?;
                let owner_id = if self.param1 != 0 { self.param1 } else { self.param0 };
                let Some(owner_entity) = state.entity(owner_id) else {
                    actor
                        .send(MsgTalk::from_system(
                            buyer.id(),
                            TalkChannel::System,
                            "Player shop rejected: seller is not online.",
                        ))
                        .await?;
                    return Ok(());
                };
                let seller = owner_entity.as_character().ok_or(crate::Error::CharacterNotFound)?;
                if seller.id() == buyer.id()
                    || seller.entity().map_id() != buyer.entity().map_id()
                    || !tq_math::in_screen(buyer.entity().location().into(), seller.entity().location().into())
                {
                    actor
                        .send(MsgTalk::from_system(
                            buyer.id(),
                            TalkChannel::System,
                            "Player shop rejected: seller is too far away.",
                        ))
                        .await?;
                    return Ok(());
                }
                let Some(shop) = state.player_shop_manager().view(owner_id) else {
                    actor
                        .send(MsgTalk::from_system(
                            buyer.id(),
                            TalkChannel::System,
                            "Player shop is not open.",
                        ))
                        .await?;
                    return Ok(());
                };
                for listing in shop.listings {
                    if let Some(item) = seller
                        .inventory_items()
                        .into_iter()
                        .find(|item| item.uid == listing.item_uid)
                    {
                        actor
                            .send(super::MsgVendorItem::new(owner_id, item, listing.price))
                            .await?;
                    }
                }
            },
            ItemActionType::BoothBuy => {
                let entity = actor.entity();
                let buyer = entity.as_character().ok_or(crate::Error::CharacterNotFound)?;
                let owner_id = self.param1;
                let Some(owner_entity) = state.entity(owner_id) else {
                    actor
                        .send(MsgTalk::from_system(
                            buyer.id(),
                            TalkChannel::System,
                            "Player shop rejected: seller is not online.",
                        ))
                        .await?;
                    return Ok(());
                };
                let seller = owner_entity.as_character().ok_or(crate::Error::CharacterNotFound)?;
                if seller.id() == buyer.id()
                    || seller.entity().map_id() != buyer.entity().map_id()
                    || !tq_math::in_screen(buyer.entity().location().into(), seller.entity().location().into())
                {
                    actor
                        .send(MsgTalk::from_system(
                            buyer.id(),
                            TalkChannel::System,
                            "Player shop rejected: seller is too far away.",
                        ))
                        .await?;
                    return Ok(());
                }
                let listing = match state.player_shop_manager().take_item(owner_id, self.param0) {
                    Ok(listing) => listing,
                    Err(error) => {
                        actor
                            .send(MsgTalk::from_system(buyer.id(), TalkChannel::System, error.message()))
                            .await?;
                        return Ok(());
                    },
                };
                let Some(item) = seller
                    .inventory_items()
                    .into_iter()
                    .find(|item| item.uid == listing.item_uid)
                else {
                    let _ = state.player_shop_manager().restore_item(owner_id, listing);
                    actor
                        .send(MsgTalk::from_system(
                            buyer.id(),
                            TalkChannel::System,
                            "Player shop rejected: listed item is no longer available.",
                        ))
                        .await?;
                    return Ok(());
                };
                match seller
                    .complete_trade(state.pool(), buyer, &[item.uid], &[], 0, listing.price)
                    .await?
                {
                    Some(transfer) => {
                        economic_audit_log("player_shop_buy", buyer.id(), seller.id(), item.uid, listing.price);
                        for removed in transfer.left_removed {
                            seller.owner().send(MsgItemInfo::remove_inventory_item(removed)).await?;
                        }
                        for received in transfer.right_received {
                            actor.send(MsgItemInfo::inventory_item(received)).await?;
                        }
                        seller.owner().send(MsgUserInfo::from(seller)).await?;
                        actor.send(MsgUserInfo::from(buyer)).await?;
                        seller
                            .owner()
                            .send(MsgTalk::from_system(
                                seller.id(),
                                TalkChannel::System,
                                format!("Your player shop sold item {}.", item.item_type),
                            ))
                            .await?;
                        actor
                            .send(MsgTalk::from_system(
                                buyer.id(),
                                TalkChannel::System,
                                format!("Purchased item {} for {} silver.", item.item_type, listing.price),
                            ))
                            .await?;
                    },
                    None => {
                        let _ = state.player_shop_manager().restore_item(owner_id, listing);
                        actor
                            .send(MsgTalk::from_system(
                                buyer.id(),
                                TalkChannel::System,
                                "Player shop purchase rejected: inventory or silver changed.",
                            ))
                            .await?;
                    },
                }
            },
            ItemActionType::Sell => {
                let entity = actor.entity();
                let character = entity.as_character().ok_or(crate::Error::CharacterNotFound)?;
                let reject = |message: &str| MsgTalk::from_system(character.id(), TalkChannel::TopLeft, message);
                if self.character_id != character.id() || self.amount != 1 {
                    dragdrop_log(format_args!(
                        "equip rejected: character_id={} item_uid={} amount={} expected_character_id={}",
                        self.character_id,
                        self.param0,
                        self.amount,
                        character.id()
                    ));
                    actor
                        .send(reject("Shop sale rejected: invalid character or amount."))
                        .await?;
                    return Ok(());
                }
                let shop_id = character.active_shop_id();
                if self.param1 != shop_id {
                    actor
                        .send(reject("Shop sale rejected: open a shop before selling."))
                        .await?;
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
                    actor
                        .send(reject("Shop sale rejected: item is not in your inventory."))
                        .await?;
                    return Ok(());
                };
                let Some(silver) =
                    state
                        .shop_catalog()
                        .sell_price(item.item_type, item.durability, item.max_durability)
                else {
                    actor.send(reject("This item has no resale value.")).await?;
                    return Ok(());
                };
                match character.sell_item(state.pool(), item.uid, silver).await? {
                    SaleOutcome::Sold { item, silver } => {
                        economic_audit_log("npc_sell", character.id(), npc.id(), item.uid, silver);
                        item_audit_log(
                            "sell",
                            character.id(),
                            item.uid,
                            item.item_type,
                            format_args!("silver={}", silver),
                        );
                        actor.send(MsgItemInfo::remove_inventory_item(item)).await?;
                        actor.send(MsgUserInfo::from(character)).await?;
                        actor
                            .send(MsgTalk::from_system(
                                character.id(),
                                TalkChannel::TopLeft,
                                format!("Sold item {} for {} silver.", item.item_type, silver),
                            ))
                            .await?;
                    },
                    SaleOutcome::ItemNotFound => {
                        actor
                            .send(reject("Shop sale rejected: item is no longer available."))
                            .await?;
                    },
                    SaleOutcome::NoValue => {
                        actor.send(reject("This item has no resale value.")).await?;
                    },
                    SaleOutcome::SilverLimit => {
                        actor
                            .send(reject("Your silver balance is at the maximum limit."))
                            .await?;
                    },
                }
            },
            ItemActionType::Use => {
                let entity = actor.entity();
                let character = entity.as_character().ok_or(crate::Error::CharacterNotFound)?;
                let reject = |message: &str| MsgTalk::from_system(character.id(), TalkChannel::TopLeft, message);
                if self.character_id != character.id() || self.amount != 1 {
                    actor.send(reject("Use rejected: invalid character or amount.")).await?;
                    return Ok(());
                }
                let Some(item) = character
                    .inventory_items()
                    .into_iter()
                    .find(|item| item.uid == self.param0)
                else {
                    actor.send(reject("Use rejected: item is not in inventory.")).await?;
                    return Ok(());
                };
                if let Some(skill) = state.skill_catalog().skill_for_book(item.item_type) {
                    if character.has_skill(skill.id) {
                        actor.send(reject("This skill is already learned.")).await?;
                        return Ok(());
                    }
                    let Some((consumed, progress)) = character
                        .learn_skill_from_item(state.pool(), item.uid, skill.id)
                        .await?
                    else {
                        actor.send(reject("Skill book is no longer in inventory.")).await?;
                        return Ok(());
                    };
                    item_audit_log(
                        "learn_skill",
                        character.id(),
                        consumed.uid,
                        consumed.item_type,
                        format_args!("skill_id={} level={}", skill.id, progress.level),
                    );
                    actor.send(MsgItemInfo::remove_inventory_item(consumed)).await?;
                    actor.send(super::MsgSkill::from(progress)).await?;
                    actor
                        .send(MsgTalk::from_system(
                            character.id(),
                            TalkChannel::System,
                            format!("Learned {}.", skill.name),
                        ))
                        .await?;
                    return Ok(());
                }
                if let Some((contents_type, amount)) =
                    crate::shop::ShopCatalog::consumable_bundle_for_item(item.item_type)
                {
                    let Some((consumed, created)) = character
                        .open_consumable_bundle(state.pool(), item.uid, item.item_type, contents_type, amount)
                        .await?
                    else {
                        actor
                            .send(reject("Pack opening requires three free inventory slots."))
                            .await?;
                        return Ok(());
                    };
                    item_audit_log(
                        "open_bundle",
                        character.id(),
                        consumed.uid,
                        consumed.item_type,
                        format_args!("contents_type={} amount={}", contents_type, created.len()),
                    );
                    actor.send(MsgItemInfo::remove_inventory_item(consumed)).await?;
                    for created_item in created {
                        actor.send(MsgItemInfo::inventory_item(created_item)).await?;
                    }
                    actor
                        .send(MsgTalk::from_system(
                            character.id(),
                            TalkChannel::System,
                            format!("Opened item {} into {} items.", item.item_type, amount),
                        ))
                        .await?;
                    return Ok(());
                }
                if let Some(effect) = crate::shop::ShopCatalog::temporary_effect_for_item(item.item_type) {
                    let Some(consumed) = character.drop_inventory_item(state.pool(), item.uid).await? else {
                        actor
                            .send(reject("Use rejected: item disappeared from inventory."))
                            .await?;
                        return Ok(());
                    };
                    let message = match effect {
                        TemporaryEffect::DoubleExperience(duration) => {
                            character.activate_double_experience(duration);
                            format!("Double experience activated for {} minutes.", duration.as_secs() / 60)
                        },
                        TemporaryEffect::HeavenBlessing(duration) => {
                            character.activate_heaven_blessing(duration);
                            format!("Heaven's Blessing activated for {} days.", duration.as_secs() / 86_400)
                        },
                    };
                    character.save(state).await?;
                    item_audit_log(
                        "use_effect",
                        character.id(),
                        consumed.uid,
                        consumed.item_type,
                        format_args!("effect={effect:?}"),
                    );
                    actor.send(MsgItemInfo::remove_inventory_item(consumed)).await?;
                    actor.send(MsgPlayer::from(character)).await?;
                    actor.screen().send_message(MsgPlayer::from(character)).await?;
                    actor
                        .send(MsgTalk::from_system(character.id(), TalkChannel::System, message))
                        .await?;
                    return Ok(());
                }
                let recovery = if let Some(amount) = crate::shop::ShopCatalog::health_recovery_for_item(item.item_type)
                {
                    Some((false, amount))
                } else {
                    crate::shop::ShopCatalog::mana_recovery_for_item(item.item_type).map(|amount| (true, amount))
                };
                let Some((mana, recovery)) = recovery else {
                    actor.send(reject("This item cannot be used yet.")).await?;
                    return Ok(());
                };
                if character.entity().is_dead() {
                    actor.send(reject("A dead character cannot use a potion.")).await?;
                    return Ok(());
                }
                let resource_before = if mana {
                    character.entity().mp()
                } else {
                    character.entity().hp()
                };
                if resource_before.current >= resource_before.max {
                    actor
                        .send(reject(if mana {
                            "MP is already full."
                        } else {
                            "HP is already full."
                        }))
                        .await?;
                    return Ok(());
                }
                let Some(consumed) = character.drop_inventory_item(state.pool(), item.uid).await? else {
                    actor
                        .send(reject("Use rejected: item disappeared from inventory."))
                        .await?;
                    return Ok(());
                };
                let resource_after = if mana {
                    character.entity().restore_mana(recovery)
                } else {
                    character.entity().heal_health(recovery)
                };
                item_audit_log(
                    "use",
                    character.id(),
                    consumed.uid,
                    consumed.item_type,
                    format_args!(
                        "resource={} recovered={}",
                        if mana { "mp" } else { "hp" },
                        resource_after.current.saturating_sub(resource_before.current)
                    ),
                );
                character.save(state).await?;
                actor.send(MsgItemInfo::remove_inventory_item(consumed)).await?;
                actor.send(MsgUserInfo::from(character)).await?;
                actor
                    .send(MsgTalk::from_system(
                        character.id(),
                        TalkChannel::System,
                        format!(
                            "Used item {} and recovered {} {}.",
                            item.item_type,
                            resource_after.current.saturating_sub(resource_before.current),
                            if mana { "MP" } else { "HP" }
                        ),
                    ))
                    .await?;
            },
            ItemActionType::Drop => {
                dragdrop_log(format_args!("processing ground drop item_uid={}", self.param0));
                tracing::info!(item_uid = self.param0, "processing inventory ground drop");
                let entity = actor.entity();
                let character = entity.as_character().ok_or(crate::Error::CharacterNotFound)?;
                let reject = |message: &str| MsgTalk::from_system(character.id(), TalkChannel::TopLeft, message);
                if self.character_id != character.id() || self.amount != 1 {
                    actor
                        .send(reject("Item drop rejected: invalid character or amount."))
                        .await?;
                    return Ok(());
                }
                let Some(item) = character.drop_inventory_item(state.pool(), self.param0).await? else {
                    dragdrop_log(format_args!(
                        "ground drop rejected item_uid={} reason=item_not_found",
                        self.param0
                    ));
                    tracing::warn!(item_uid = self.param0, "ground drop rejected: item not found");
                    actor
                        .send(reject("Item drop rejected: item is no longer in your inventory."))
                        .await?;
                    return Ok(());
                };
                let location = character.entity().location();
                let ground = crate::state::GroundItem {
                    uid: state.next_ground_item_uid(),
                    item_id: item.item_type,
                    map_id: character.entity().map_id(),
                    x: location.x,
                    y: location.y,
                    owner_id: character.id(),
                    silver: 0,
                };
                state.insert_ground_item(ground);
                dragdrop_log(format_args!(
                    "ground drop completed item_uid={} ground_uid={} x={} y={}",
                    item.uid, ground.uid, ground.x, ground.y
                ));
                tracing::info!(
                    item_uid = item.uid,
                    ground_uid = ground.uid,
                    x = ground.x,
                    y = ground.y,
                    "inventory item dropped on ground"
                );
                item_audit_log(
                    "drop",
                    character.id(),
                    item.uid,
                    item.item_type,
                    format_args!("ground_uid={} x={} y={}", ground.uid, ground.x, ground.y),
                );
                actor.send(MsgItemInfo::remove_inventory_item(item)).await?;
                let dropped = MsgGroundItem::new(ground.uid, ground.item_id, ground.x, ground.y);
                actor.send(dropped.clone()).await?;
                actor.screen().send_message(dropped).await?;
            },
            ItemActionType::MoveInventory => {
                dragdrop_log(format_args!(
                    "processing inventory move item_uid={} target_slot={}",
                    self.param0, self.param1
                ));
                tracing::info!(
                    item_uid = self.param0,
                    target_slot = self.param1,
                    "processing inventory move"
                );
                let entity = actor.entity();
                let character = entity.as_character().ok_or(crate::Error::CharacterNotFound)?;
                let reject = |message: &str| MsgTalk::from_system(character.id(), TalkChannel::TopLeft, message);
                if self.character_id != character.id() || self.amount != 1 {
                    actor
                        .send(reject("Inventory move rejected: invalid character or amount."))
                        .await?;
                    return Ok(());
                }
                let Ok(position) = u8::try_from(self.param1) else {
                    actor
                        .send(reject("Inventory move rejected: invalid inventory slot."))
                        .await?;
                    return Ok(());
                };
                let Some((moved, swapped)) = character
                    .move_inventory_item(state.pool(), self.param0, position)
                    .await?
                else {
                    dragdrop_log(format_args!(
                        "inventory move rejected item_uid={} target_slot={} reason=item_or_slot_unavailable",
                        self.param0, self.param1
                    ));
                    tracing::warn!(
                        item_uid = self.param0,
                        target_slot = self.param1,
                        "inventory move rejected: item or slot unavailable"
                    );
                    actor
                        .send(reject("Inventory move rejected: item or slot is unavailable."))
                        .await?;
                    return Ok(());
                };
                let swapped_uid = swapped.as_ref().map(|item| item.uid);
                actor.send(MsgItemInfo::inventory_item(moved)).await?;
                if let Some(swapped) = swapped {
                    actor.send(MsgItemInfo::inventory_item(swapped)).await?;
                }
                dragdrop_log(format_args!(
                    "inventory move completed item_uid={} target_slot={} swapped_uid={:?}",
                    moved.uid, moved.position, swapped_uid
                ));
                tracing::info!(
                    item_uid = moved.uid,
                    target_slot = moved.position,
                    swapped_uid,
                    "inventory move completed"
                );
            },
            ItemActionType::Equip => {
                let entity = actor.entity();
                let character = entity.as_character().ok_or(crate::Error::CharacterNotFound)?;
                let reject = |message: &str| MsgTalk::from_system(character.id(), TalkChannel::TopLeft, message);
                if self.character_id != character.id() || self.amount != 1 {
                    actor
                        .send(reject("Equip rejected: invalid character or amount."))
                        .await?;
                    return Ok(());
                }
                let Ok(position) = u8::try_from(self.param1) else {
                    dragdrop_log(format_args!(
                        "equip rejected: item_uid={} invalid_slot={}",
                        self.param0, self.param1
                    ));
                    actor.send(reject("Equip rejected: invalid equipment slot.")).await?;
                    return Ok(());
                };
                let Some(source) = character
                    .inventory_items()
                    .into_iter()
                    .find(|item| item.uid == self.param0)
                else {
                    dragdrop_log(format_args!(
                        "equip rejected: item_uid={} is not in inventory",
                        self.param0
                    ));
                    actor.send(reject("Equip rejected: item is not in inventory.")).await?;
                    return Ok(());
                };
                let Some(expected_slot) = crate::shop::ShopCatalog::equipment_slot_for_item(source.item_type) else {
                    dragdrop_log(format_args!(
                        "equip rejected: item_uid={} unknown_item_type={}",
                        source.uid, source.item_type
                    ));
                    actor
                        .send(reject("Equip rejected: item type cannot be equipped."))
                        .await?;
                    return Ok(());
                };
                if expected_slot != position && !(expected_slot == 4 && position == 5) {
                    dragdrop_log(format_args!(
                        "equip rejected: item_uid={} item_type={} expected_slot={} requested_slot={}",
                        source.uid, source.item_type, expected_slot, position
                    ));
                    actor
                        .send(reject("Equip rejected: item does not fit this slot."))
                        .await?;
                    return Ok(());
                }
                if let Some(template) = state.shop_catalog().template(source.item_type) {
                    if !character.meets_item_requirements(source.item_type, &template) {
                        dragdrop_log(format_args!(
                            "equip rejected: item_uid={} item_type={} requirements not met",
                            source.uid, source.item_type
                        ));
                        actor
                            .send(reject("Equip rejected: character requirements not met."))
                            .await?;
                        return Ok(());
                    }
                }
                let Some((equipped, swapped)) = character
                    .equip_inventory_item(state.pool(), self.param0, position)
                    .await?
                else {
                    dragdrop_log(format_args!(
                        "equip rejected: item_uid={} slot={} unavailable",
                        self.param0, position
                    ));
                    actor
                        .send(reject("Equip rejected: item or slot is unavailable."))
                        .await?;
                    return Ok(());
                };
                actor.send(MsgItemInfo::equipment_item(equipped)).await?;
                actor.send(MsgItemInfo::remove_inventory_item(equipped)).await?;
                if let Some(swapped) = swapped {
                    actor.send(MsgItemInfo::inventory_item(swapped)).await?;
                }
                character.refresh_equipment_resources(state.shop_catalog());
                actor.send(MsgUserInfo::from(character)).await?;
                actor.send(MsgPlayer::from(character)).await?;
                actor.screen().send_message(MsgPlayer::from(character)).await?;
                dragdrop_log(format_args!(
                    "equip completed: item_uid={} slot={} swapped={}",
                    equipped.uid,
                    position,
                    swapped.is_some()
                ));
                item_audit_log(
                    "equip",
                    character.id(),
                    equipped.uid,
                    equipped.item_type,
                    format_args!("slot={} swapped={}", position, swapped.is_some()),
                );
            },
            ItemActionType::Unequip => {
                let entity = actor.entity();
                let character = entity.as_character().ok_or(crate::Error::CharacterNotFound)?;
                let reject = |message: &str| MsgTalk::from_system(character.id(), TalkChannel::TopLeft, message);
                if self.character_id != character.id() || self.amount != 1 {
                    dragdrop_log(format_args!(
                        "unequip rejected: character_id={} item_uid={} amount={} expected_character_id={}",
                        self.character_id,
                        self.param0,
                        self.amount,
                        character.id()
                    ));
                    actor
                        .send(reject("Unequip rejected: invalid character or amount."))
                        .await?;
                    return Ok(());
                }
                let Ok(position) = u8::try_from(self.param1) else {
                    dragdrop_log(format_args!(
                        "unequip rejected: item_uid={} invalid_inventory_slot={}",
                        self.param0, self.param1
                    ));
                    actor.send(reject("Unequip rejected: invalid inventory slot.")).await?;
                    return Ok(());
                };
                let equipment_slot = character
                    .equipment_items()
                    .into_iter()
                    .find(|equipped| equipped.uid == self.param0)
                    .map(|equipped| equipped.position);
                let Some(item) = character.unequip_item(state.pool(), self.param0, position).await? else {
                    dragdrop_log(format_args!(
                        "unequip rejected: item_uid={} inventory_slot={} unavailable",
                        self.param0, position
                    ));
                    actor
                        .send(reject("Unequip rejected: item or inventory slot is unavailable."))
                        .await?;
                    return Ok(());
                };
                actor
                    .send(MsgItemInfo::remove_equipment_item(
                        item,
                        equipment_slot.unwrap_or_default(),
                    ))
                    .await?;
                actor.send(MsgItemInfo::inventory_item(item)).await?;
                character.refresh_equipment_resources(state.shop_catalog());
                actor.send(MsgUserInfo::from(character)).await?;
                actor.send(MsgPlayer::from(character)).await?;
                actor.screen().send_message(MsgPlayer::from(character)).await?;
                dragdrop_log(format_args!(
                    "unequip completed: item_uid={} inventory_slot={}",
                    item.uid, position
                ));
                item_audit_log(
                    "unequip",
                    character.id(),
                    item.uid,
                    item.item_type,
                    format_args!("inventory_slot={}", position),
                );
            },
            ItemActionType::OpenSocket => {
                let entity = actor.entity();
                let character = entity.as_character().ok_or(crate::Error::CharacterNotFound)?;
                let reject = |message: &str| MsgTalk::from_system(character.id(), TalkChannel::System, message);
                if self.character_id != character.id() || self.amount != 1 {
                    actor
                        .send(reject("Socket rejected: invalid character or amount."))
                        .await?;
                    return Ok(());
                }
                let equipped = character.equipment_items().iter().any(|item| item.uid == self.param0);
                let (outcome, catalysts) = character.open_item_socket(state.pool(), self.param0).await?;
                match outcome {
                    crate::entities::SocketOutcome::Opened(item) => {
                        for catalyst in catalysts {
                            actor.send(MsgItemInfo::remove_inventory_item(catalyst)).await?;
                        }
                        if equipped {
                            actor.send(MsgItemInfo::equipment_item(item)).await?;
                        } else {
                            actor.send(MsgItemInfo::inventory_item(item)).await?;
                        }
                        actor
                            .send(MsgTalk::from_system(
                                character.id(),
                                TalkChannel::System,
                                "Socket opened successfully.",
                            ))
                            .await?;
                    },
                    crate::entities::SocketOutcome::MissingDragonBall => {
                        actor.send(reject("Socket rejected: you need a DragonBall.")).await?
                    },
                    crate::entities::SocketOutcome::DualSockets => {
                        actor
                            .send(reject("Socket rejected: this item already has dual sockets."))
                            .await?
                    },
                    crate::entities::SocketOutcome::NotFound => {
                        actor.send(reject("Socket rejected: equipment was not found.")).await?
                    },
                    _ => {
                        actor
                            .send(reject("Socket rejected: this item cannot receive a socket."))
                            .await?
                    },
                }
            },
            ItemActionType::Improve => {
                let entity = actor.entity();
                let character = entity.as_character().ok_or(crate::Error::CharacterNotFound)?;
                let reject = |message: &str| MsgTalk::from_system(character.id(), TalkChannel::TopLeft, message);
                if self.character_id != character.id() || self.amount != 1 {
                    actor
                        .send(reject("Quality upgrade rejected: invalid character or amount."))
                        .await?;
                    return Ok(());
                }
                let Some(target) = character
                    .inventory_items()
                    .into_iter()
                    .chain(character.equipment_items())
                    .find(|item| item.uid == self.param0)
                else {
                    actor
                        .send(reject("Quality upgrade rejected: owned item was not found."))
                        .await?;
                    return Ok(());
                };
                if crate::shop::ShopCatalog::equipment_slot_for_item(target.item_type).is_none() {
                    actor
                        .send(reject("Quality upgrade rejected: this item is not equipment."))
                        .await?;
                    return Ok(());
                }
                let equipped = character.equipment_items().iter().any(|item| item.uid == self.param0);
                let (outcome, catalyst) = character
                    .upgrade_quality_with_dragonball(state.pool(), self.param0)
                    .await?;
                match outcome {
                    crate::entities::QualityUpgradeOutcome::Upgraded(item)
                    | crate::entities::QualityUpgradeOutcome::Failed(item) => {
                        if let Some(catalyst) = catalyst {
                            actor.send(MsgItemInfo::remove_inventory_item(catalyst)).await?;
                        }
                        if equipped {
                            actor.send(MsgItemInfo::equipment_item(item)).await?;
                            character.refresh_equipment_resources(state.shop_catalog());
                            actor.send(MsgUserInfo::from(character)).await?;
                            actor.send(MsgPlayer::from(character)).await?;
                            actor.screen().send_message(MsgPlayer::from(character)).await?;
                        } else {
                            actor.send(MsgItemInfo::inventory_item(item)).await?;
                        }
                        let success = matches!(outcome, crate::entities::QualityUpgradeOutcome::Upgraded(_));
                        item_audit_log(
                            if success {
                                "quality-upgrade"
                            } else {
                                "quality-upgrade-failed"
                            },
                            character.id(),
                            item.uid,
                            item.item_type,
                            format_args!(
                                "equipped={equipped} durability={}/{}",
                                item.durability, item.max_durability
                            ),
                        );
                        let message = if success {
                            format!("Item {} quality upgraded successfully.", item.item_type)
                        } else {
                            format!(
                                "Item quality upgrade failed; durability is now {}/{}.",
                                item.durability, item.max_durability
                            )
                        };
                        actor
                            .send(MsgTalk::from_system(character.id(), TalkChannel::System, message))
                            .await?;
                    },
                    crate::entities::QualityUpgradeOutcome::MaxQuality(item) => {
                        actor
                            .send(reject(&format!(
                                "Item {} is already at maximum quality.",
                                item.item_type
                            )))
                            .await?;
                    },
                    crate::entities::QualityUpgradeOutcome::InvalidQuality(item) => {
                        actor
                            .send(reject(&format!(
                                "Item {} cannot be refined through quality upgrade.",
                                item.item_type
                            )))
                            .await?;
                    },
                    crate::entities::QualityUpgradeOutcome::MissingDragonBall => {
                        actor
                            .send(reject("Quality upgrade rejected: you need a DragonBall."))
                            .await?;
                    },
                    crate::entities::QualityUpgradeOutcome::NotFound => {
                        actor
                            .send(reject("Quality upgrade rejected: owned item was not found."))
                            .await?;
                    },
                }
            },
            ItemActionType::UpLevel => {
                let entity = actor.entity();
                let character = entity.as_character().ok_or(crate::Error::CharacterNotFound)?;
                let reject = |message: &str| MsgTalk::from_system(character.id(), TalkChannel::TopLeft, message);
                if self.character_id != character.id() || self.amount != 1 {
                    actor
                        .send(reject("Upgrade rejected: invalid character or amount."))
                        .await?;
                    return Ok(());
                }
                let Some(target) = character
                    .inventory_items()
                    .into_iter()
                    .chain(character.equipment_items())
                    .find(|item| item.uid == self.param0)
                else {
                    actor
                        .send(reject("Upgrade rejected: owned item was not found."))
                        .await?;
                    return Ok(());
                };
                if crate::shop::ShopCatalog::equipment_slot_for_item(target.item_type).is_none() {
                    actor
                        .send(reject("Upgrade rejected: this item is not equipment."))
                        .await?;
                    return Ok(());
                }
                let equipped = character.equipment_items().iter().any(|item| item.uid == self.param0);
                let (outcome, catalysts) = character.upgrade_item_with_meteor(state.pool(), self.param0).await?;
                match outcome {
                    crate::entities::ItemUpgradeOutcome::Upgraded(item) => {
                        for catalyst in catalysts {
                            actor.send(MsgItemInfo::remove_inventory_item(catalyst)).await?;
                        }
                        if equipped {
                            actor.send(MsgItemInfo::equipment_item(item)).await?;
                            character.refresh_equipment_resources(state.shop_catalog());
                            actor.send(MsgUserInfo::from(character)).await?;
                            actor.send(MsgPlayer::from(character)).await?;
                            actor.screen().send_message(MsgPlayer::from(character)).await?;
                        } else {
                            actor.send(MsgItemInfo::inventory_item(item)).await?;
                        }
                        actor
                            .send(MsgTalk::from_system(
                                character.id(),
                                TalkChannel::System,
                                format!("Item {} upgraded to +{}.", item.item_type, item.plus),
                            ))
                            .await?;
                    },
                    crate::entities::ItemUpgradeOutcome::Failed(item) => {
                        for catalyst in catalysts {
                            actor.send(MsgItemInfo::remove_inventory_item(catalyst)).await?;
                        }
                        if equipped {
                            actor.send(MsgItemInfo::equipment_item(item)).await?;
                        } else {
                            actor.send(MsgItemInfo::inventory_item(item)).await?;
                        }
                        actor.send(reject("Upgrade failed; the Meteors were consumed.")).await?;
                    },
                    crate::entities::ItemUpgradeOutcome::MaxLevel(item) => {
                        actor
                            .send(reject(&format!(
                                "Item {} is already at maximum +{}.",
                                item.item_type, item.plus
                            )))
                            .await?;
                    },
                    crate::entities::ItemUpgradeOutcome::MissingMeteor => {
                        actor.send(reject("Upgrade rejected: you need a Meteor.")).await?;
                    },
                    crate::entities::ItemUpgradeOutcome::NotFound => {
                        actor
                            .send(reject("Upgrade rejected: owned item was not found."))
                            .await?;
                    },
                }
            },
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
