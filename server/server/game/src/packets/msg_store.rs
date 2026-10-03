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
}

/// The account a character belongs to.
fn account_of(character: &crate::entities::Character) -> u32 {
    character.account_id()
}

/// Loads a character's VIP at login.
pub async fn load_vip(state: &State, character: &crate::entities::Character) -> Result<(), Error> {
    let until = crate::store::vip_until(state.pool(), account_of(character)).await?;
    state.vip().set(character.id(), until);
    Ok(())
}

/// Sends the catalog and the buyer's state.
pub async fn send_store(state: &State, character: &crate::entities::Character) -> Result<(), Error> {
    let products = crate::store::catalog(state.pool()).await?;
    let owner = character.owner();
    for (index, product) in products.iter().enumerate() {
        owner.send(MsgStoreProduct::new(index, products.len(), product)).await?;
    }
    owner
        .send(MsgStoreState {
            cps: character.cps(),
            vip_until: state.vip().until(character.id()),
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
        if self.action == BUY {
            let line = match crate::store::buy(state.pool(), character, account_of(character), self.param).await? {
                Ok(purchase) => {
                    for item in &purchase.items {
                        actor.send(MsgItemInfo::inventory_item(*item)).await?;
                    }
                    if let Some(until) = purchase.vip_until {
                        state.vip().set(character.id(), until);
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
