//! The in-game Store (Gravak packets, not in the 5065 client): `MsgStore`
//! carries the client's requests, `MsgStoreProduct` one product of the
//! catalog and `MsgStoreState` the CPs and VIP.

use crate::store::{Product, Refusal};
use crate::{ActorState, Error, State};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tq_network::{Actor, PacketID, PacketProcess};

use super::{MsgItemInfo, MsgTalk, MsgUserInfo, TalkChannel};

/// Client: send the catalog.
pub const OPEN: u32 = 0;
/// Client: buy product `param`.
pub const BUY: u32 = 1;
/// Client (VIP): teleport to city `param` of [`crate::store::CITIES`].
pub const TELEPORT: u32 = 2;
/// Client (VIP): the loot filter becomes `param`.
pub const LOOT_FILTER: u32 = 3;

#[derive(Debug, Default, Clone, Serialize, Deserialize, PacketID)]
#[packet(id = 9110)]
pub struct MsgStore {
    pub action: u32,
    pub param: u32,
}

/// `stock_left` when there is no daily stock.
pub const UNLIMITED: u32 = u32::MAX;

/// One product of the catalog, `index` of `total` (index 0 starts a new one).
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, PacketID)]
#[packet(id = 9111)]
pub struct MsgStoreProduct {
    pub index: u32,
    pub total: u32,
    pub product_id: u32,
    pub category: u32,
    pub item_type: u32,
    pub amount: u32,
    pub vip_days: u32,
    pub price_cps: u32,
    pub stock_left: u32,
    pub name: String,
    pub description: String,
}

impl MsgStoreProduct {
    fn new(index: usize, total: usize, product: &Product) -> Self {
        Self {
            index: u32::try_from(index).unwrap_or_default(),
            total: u32::try_from(total).unwrap_or_default(),
            product_id: product.product_id,
            category: product.category,
            item_type: product.item_type,
            amount: product.amount,
            vip_days: product.vip_days,
            price_cps: product.price_cps,
            stock_left: product.stock_left.unwrap_or(UNLIMITED),
            name: product.name.clone(),
            description: product.description.clone(),
        }
    }
}

/// The buyer's CPs and when their VIP ends (seconds since 1970, 0: none).
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, PacketID)]
#[packet(id = 9112)]
pub struct MsgStoreState {
    pub cps: u64,
    pub vip_until: u64,
    /// Which drops go straight to the bag ([`crate::store::loot`]).
    pub loot_filter: u32,
    /// When the next VIP teleport is allowed (seconds since 1970).
    pub teleport_ready_at: u64,
}

/// One of the buyer's latest purchases, `index` of `total`, newest first;
/// `total` 0 (and nothing else) when there are none.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, PacketID)]
#[packet(id = 9113)]
pub struct MsgStorePurchase {
    pub index: u32,
    pub total: u32,
    pub price_cps: u32,
    pub bought_at: u64,
    pub name: String,
}

/// The account a character belongs to.
fn account_of(character: &crate::entities::Character) -> u32 {
    character.account_id()
}

/// Loads a character's VIP at login.
pub async fn load_vip(state: &State, character: &crate::entities::Character) -> Result<(), Error> {
    let until = crate::store::vip_until(state.pool(), account_of(character)).await?;
    state.vip().set(character.id(), until);
    let filter = crate::store::loot_filter(state.pool(), character.database_id()).await?;
    state.vip().set_loot_filter(character.id(), filter);
    show_vip(character, state.vip().is_vip(character.id())).await
}

/// Sets the VIP badge flag and shows it to the character and those around.
pub async fn show_vip(character: &crate::entities::Character, vip: bool) -> Result<(), Error> {
    use crate::entities::Flags;
    let flags = character.entity().flags();
    if flags.contains(Flags::VIP) == vip {
        return Ok(());
    }
    character
        .entity()
        .set_flags(if vip { flags | Flags::VIP } else { flags & !Flags::VIP });
    let player = super::MsgPlayer::from(character);
    character.owner().send(player.clone()).await?;
    if let Ok(screen) = character.try_screen() {
        screen.send_message(player).await?;
    }
    Ok(())
}

/// Takes the badge off the characters whose VIP ran out.
pub async fn expire_vip(state: &State) {
    for entity in state.entities() {
        if let Some(character) = entity.as_character() {
            if character.entity().flags().contains(crate::entities::Flags::VIP) && !state.vip().is_vip(character.id()) {
                let _ = show_vip(character, false).await;
            }
        }
    }
}

/// Sends the catalog and the buyer's state.
pub async fn send_store(state: &State, character: &crate::entities::Character) -> Result<(), Error> {
    let products = crate::store::catalog(state.pool()).await?;
    let owner = character.owner();
    for (index, product) in products.iter().enumerate() {
        owner.send(MsgStoreProduct::new(index, products.len(), product)).await?;
    }
    let history = crate::store::history(state.pool(), character.database_id()).await?;
    if history.is_empty() {
        owner.send(MsgStorePurchase::default()).await?;
    }
    for (index, purchase) in history.iter().enumerate() {
        owner
            .send(MsgStorePurchase {
                index: u32::try_from(index).unwrap_or_default(),
                total: u32::try_from(history.len()).unwrap_or_default(),
                price_cps: purchase.price_cps,
                bought_at: purchase.bought_at,
                name: purchase.name.clone(),
            })
            .await?;
    }
    owner
        .send(MsgStoreState {
            cps: character.cps(),
            vip_until: state.vip().until(character.id()),
            loot_filter: state.vip().loot_filter(character.id()),
            teleport_ready_at: state.vip().teleport_ready_at(character.id()),
        })
        .await?;
    Ok(())
}

#[async_trait]
impl PacketProcess for MsgStore {
    type ActorState = ActorState;
    type Error = Error;
    type State = State;

    async fn process(&self, state: &Self::State, actor: &Actor<Self::ActorState>) -> Result<(), Self::Error> {
        let entity = actor.entity();
        let character = entity.as_character().ok_or(Error::CharacterNotFound)?;
        if self.action == TELEPORT {
            let line = vip_teleport(state, character, self.param).await?;
            if let Some(line) = line {
                actor
                    .send(MsgTalk::from_system(character.id(), TalkChannel::TopLeft, line))
                    .await?;
            }
        }
        if self.action == LOOT_FILTER {
            let filter = self.param & crate::store::loot::ALL;
            state.vip().set_loot_filter(character.id(), filter);
            crate::store::save_loot_filter(state.pool(), character.database_id(), filter).await?;
        }
        if self.action == BUY {
            let line = match crate::store::buy(state.pool(), character, account_of(character), self.param).await? {
                Ok(purchase) => {
                    for item in &purchase.items {
                        actor.send(MsgItemInfo::inventory_item(*item)).await?;
                    }
                    if let Some(until) = purchase.vip_until {
                        state.vip().set(character.id(), until);
                        show_vip(character, true).await?;
                    }
                    actor.send(MsgUserInfo::from(character)).await?;
                    format!("You bought {} for {} CPs.", purchase.product.name, purchase.product.price_cps)
                },
                Err(refusal) => refusal_line(refusal),
            };
            actor
                .send(MsgTalk::from_system(character.id(), TalkChannel::TopLeft, line))
                .await?;
        }
        send_store(state, character).await
    }
}

/// Takes a VIP to a city; what to tell them, if anything.
async fn vip_teleport(
    state: &State,
    character: &crate::entities::Character,
    city: u32,
) -> Result<Option<String>, Error> {
    let Some(city) = crate::store::CITIES.get(city as usize) else {
        return Ok(None);
    };
    if !state.vip().is_vip(character.id()) {
        return Ok(Some("City teleports are a VIP service.".to_owned()));
    }
    if character.entity().is_dead() {
        return Ok(Some("The dead cannot teleport.".to_owned()));
    }
    let map_id = character.entity().map_id();
    let flags = state.try_map(map_id).map(|map| map.flags()).unwrap_or(super::MapFlags::empty());
    let blocked = super::MapFlags::TELEPORT_DISABLED | super::MapFlags::PRISON_MAP;
    if flags.intersects(blocked) || map_id == crate::guild_war::GUILD_MAP && crate::guild_war::running() {
        return Ok(Some("You cannot teleport from here.".to_owned()));
    }
    let now = crate::store::now_secs();
    if !state.vip().take_teleport(character.id(), now) {
        let left = state.vip().teleport_ready_at(character.id()).saturating_sub(now);
        return Ok(Some(format!("You can teleport again in {left}s.")));
    }
    crate::world::relocate(state, character, city.map_id, city.position).await?;
    tracing::info!(target: "world", character = %character.entity().name(), city = city.name, "VIP teleport");
    Ok(None)
}

fn refusal_line(refusal: Refusal) -> String {
    refusal.message()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tq_network::{PacketDecode, PacketEncode};

    #[test]
    fn products_survive_the_wire() {
        let product = MsgStoreProduct {
            index: 2,
            total: 11,
            product_id: 20,
            category: 2,
            item_type: 1_088_001,
            amount: 1,
            price_cps: 13,
            stock_left: UNLIMITED,
            name: "Meteor".to_owned(),
            description: "Raises the level of an item with a chance of success.".to_owned(),
            ..Default::default()
        };
        let (_, bytes) = product.clone().encode().expect("encodes");
        assert_eq!(MsgStoreProduct::decode(&bytes).expect("decodes"), product);
    }
}
