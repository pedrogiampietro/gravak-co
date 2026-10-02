use serde::{Deserialize, Serialize};
use tq_network::PacketID;

use crate::entities::GameEntity;
use crate::packets::{MsgTalk, TalkChannel};
use crate::trade::{self, TradeChange, TradeError};
use crate::{ActorState, Error, State};
use async_trait::async_trait;
use std::sync::Arc;
use tq_network::{Actor, PacketProcess};

#[derive(Debug, Serialize, Deserialize, Clone, PacketID)]
#[packet(id = 1056)]
pub struct MsgTrade {
    pub target: u32,
    pub subtype: u32,
    pub data: u32,
}

impl MsgTrade {
    pub fn new(target: u32, subtype: u32, data: u32) -> Self {
        Self { target, subtype, data }
    }
}

fn character_entity(state: &State, character_id: u32) -> Option<Arc<GameEntity>> {
    let entity = state.entity(character_id)?;
    entity.is_character().then_some(entity)
}

fn target_entity(
    state: &State,
    actor: &crate::entities::Character,
    target_id: u32,
) -> Result<Arc<GameEntity>, TradeError> {
    if target_id == actor.id() {
        return Err(TradeError::RequestMismatch);
    }
    let Some(target) = state.entity(target_id) else {
        return Err(TradeError::RequestMismatch);
    };
    let Some(target_character) = target.as_character() else {
        return Err(TradeError::RequestMismatch);
    };
    if target_character.entity().map_id() != actor.entity().map_id()
        || !tq_math::in_screen(
            actor.entity().location().into(),
            target_character.entity().location().into(),
        )
    {
        return Err(TradeError::RequestMismatch);
    }
    Ok(target)
}

async fn send_notice(state: &State, character_id: u32, message: impl Into<String>) -> Result<(), Error> {
    if let Some(entity) = character_entity(state, character_id) {
        entity
            .owner()
            .ok_or(Error::CharacterNotFound)?
            .send(MsgTalk::from_system(character_id, TalkChannel::Talk, message))
            .await?;
    }
    Ok(())
}

async fn send_trade(state: &State, character_id: u32, packet: MsgTrade) -> Result<(), Error> {
    if let Some(entity) = character_entity(state, character_id) {
        entity.owner().ok_or(Error::CharacterNotFound)?.send(packet).await?;
    }
    Ok(())
}

async fn send_item_info(state: &State, character_id: u32, packet: super::MsgItemInfo) -> Result<(), Error> {
    if let Some(entity) = character_entity(state, character_id) {
        entity.owner().ok_or(Error::CharacterNotFound)?.send(packet).await?;
    }
    Ok(())
}

async fn complete_trade(state: &State, session: trade::TradeSession) -> Result<(), Error> {
    let Some(left_entity) = character_entity(state, session.owner_id) else {
        state.trade_manager().finish(session.owner_id, session.target_id);
        return Ok(());
    };
    let Some(right_entity) = character_entity(state, session.target_id) else {
        state.trade_manager().finish(session.owner_id, session.target_id);
        return Ok(());
    };
    let left = left_entity.as_character().ok_or(Error::CharacterNotFound)?;
    let right = right_entity.as_character().ok_or(Error::CharacterNotFound)?;
    let transfer = left
        .complete_trade(
            state.pool(),
            right,
            &session.owner_items,
            &session.target_items,
            session.owner_silver,
            session.target_silver,
            session.owner_cps,
            session.target_cps,
        )
        .await?;
    match transfer {
        Some(transfer) => {
            tracing::info!(
                target: "trade",
                items = transfer.left_removed.len() + transfer.right_removed.len(),
                silver = session.owner_silver + session.target_silver,
                cps = session.owner_cps + session.target_cps,
                "{} and {} traded",
                left.entity().name(),
                right.entity().name()
            );
            for item in &transfer.left_removed {
                crate::audit::item(
                    "trade_out",
                    session.owner_id,
                    item.uid,
                    item.item_type,
                    format_args!("counterparty_id={}", session.target_id),
                );
            }
            for item in &transfer.left_received {
                crate::audit::item(
                    "trade_in",
                    session.owner_id,
                    item.uid,
                    item.item_type,
                    format_args!("counterparty_id={}", session.target_id),
                );
            }
            for item in &transfer.right_removed {
                crate::audit::item(
                    "trade_out",
                    session.target_id,
                    item.uid,
                    item.item_type,
                    format_args!("counterparty_id={}", session.owner_id),
                );
            }
            for item in &transfer.right_received {
                crate::audit::item(
                    "trade_in",
                    session.target_id,
                    item.uid,
                    item.item_type,
                    format_args!("counterparty_id={}", session.owner_id),
                );
            }
            if session.owner_silver > 0 {
                crate::audit::economic(
                    "trade_silver_out",
                    session.owner_id,
                    session.target_id,
                    0,
                    session.owner_silver,
                );
            }
            if session.target_silver > 0 {
                crate::audit::economic(
                    "trade_silver_out",
                    session.target_id,
                    session.owner_id,
                    0,
                    session.target_silver,
                );
            }
            if session.target_silver > 0 {
                crate::audit::economic(
                    "trade_silver_in",
                    session.owner_id,
                    session.target_id,
                    0,
                    session.target_silver,
                );
            }
            if session.owner_silver > 0 {
                crate::audit::economic(
                    "trade_silver_in",
                    session.target_id,
                    session.owner_id,
                    0,
                    session.owner_silver,
                );
            }
            for (from_id, to_id, amount) in [
                (session.owner_id, session.target_id, session.owner_cps),
                (session.target_id, session.owner_id, session.target_cps),
            ] {
                if amount > 0 {
                    crate::audit::economic("trade_cps_out", from_id, to_id, 0, amount);
                    crate::audit::economic("trade_cps_in", to_id, from_id, 0, amount);
                }
            }
            send_trade(
                state,
                session.owner_id,
                MsgTrade::new(session.target_id, trade::HIDE_TABLE, 0),
            )
            .await?;
            send_trade(
                state,
                session.target_id,
                MsgTrade::new(session.owner_id, trade::HIDE_TABLE, 0),
            )
            .await?;
            for item in &transfer.left_removed {
                send_item_info(
                    state,
                    session.owner_id,
                    super::MsgItemInfo::remove_inventory_item(*item),
                )
                .await?;
            }
            for item in &transfer.right_removed {
                send_item_info(
                    state,
                    session.target_id,
                    super::MsgItemInfo::remove_inventory_item(*item),
                )
                .await?;
            }
            for item in &transfer.left_received {
                send_item_info(state, session.owner_id, super::MsgItemInfo::inventory_item(*item)).await?;
            }
            for item in &transfer.right_received {
                send_item_info(state, session.target_id, super::MsgItemInfo::inventory_item(*item)).await?;
            }
            left.owner().send(super::MsgUserInfo::from(left)).await?;
            right.owner().send(super::MsgUserInfo::from(right)).await?;
            send_notice(state, session.owner_id, "Trade completed successfully.").await?;
            send_notice(state, session.target_id, "Trade completed successfully.").await?;
        },
        None => {
            send_trade(
                state,
                session.owner_id,
                MsgTrade::new(session.target_id, trade::HIDE_TABLE, 0),
            )
            .await?;
            send_trade(
                state,
                session.target_id,
                MsgTrade::new(session.owner_id, trade::HIDE_TABLE, 0),
            )
            .await?;
            send_notice(
                state,
                session.owner_id,
                "Trade failed: the offer changed or inventory was full.",
            )
            .await?;
            send_notice(
                state,
                session.target_id,
                "Trade failed: the offer changed or inventory was full.",
            )
            .await?;
        },
    }
    state.trade_manager().finish(session.owner_id, session.target_id);
    Ok(())
}

async fn send_change(state: &State, change: TradeChange) -> Result<(), Error> {
    match change {
        TradeChange::Request { from_id, to_id } => {
            send_trade(state, to_id, MsgTrade::new(from_id, trade::REQUEST, 0)).await?;
            send_notice(state, from_id, "Trade request sent.").await?;
            send_notice(state, to_id, format!("Character {from_id} wants to trade with you.")).await?;
        },
        TradeChange::Open(session) => {
            send_trade(
                state,
                session.owner_id,
                MsgTrade::new(session.target_id, trade::SHOW_TABLE, 0),
            )
            .await?;
            send_trade(
                state,
                session.target_id,
                MsgTrade::new(session.owner_id, trade::SHOW_TABLE, 0),
            )
            .await?;
            send_notice(state, session.owner_id, "Trade window opened.").await?;
            send_notice(state, session.target_id, "Trade window opened.").await?;
        },
        TradeChange::ItemAdded {
            from_id,
            peer_id,
            item_uid,
        } => {
            let item = character_entity(state, from_id)
                .and_then(|entity| {
                    entity.as_character().and_then(|character| {
                        character
                            .inventory_items()
                            .into_iter()
                            .find(|item| item.uid == item_uid)
                    })
                })
                .ok_or(Error::CharacterNotFound)?;
            send_item_info(state, peer_id, super::MsgItemInfo::trade_item(item)).await?;
            send_trade(state, from_id, MsgTrade::new(peer_id, trade::ADD_ITEM, item_uid)).await?;
            send_trade(state, peer_id, MsgTrade::new(from_id, trade::ACCEPT, 0)).await?;
            send_notice(state, from_id, "Item added to trade.").await?;
        },
        TradeChange::ItemRemoved {
            from_id,
            peer_id,
            item_uid,
        } => {
            send_trade(state, peer_id, MsgTrade::new(from_id, trade::REMOVE_ITEM, item_uid)).await?;
            send_trade(state, peer_id, MsgTrade::new(from_id, trade::ACCEPT, 0)).await?;
        },
        TradeChange::MoneySet {
            from_id,
            peer_id,
            amount,
        } => {
            send_trade(state, peer_id, MsgTrade::new(from_id, trade::SHOW_MONEY, amount)).await?;
            send_trade(state, peer_id, MsgTrade::new(from_id, trade::ACCEPT, 0)).await?;
        },
        TradeChange::CpsSet {
            from_id,
            peer_id,
            amount,
        } => {
            send_trade(state, peer_id, MsgTrade::new(from_id, trade::SHOW_CPS, amount)).await?;
            send_trade(state, peer_id, MsgTrade::new(from_id, trade::ACCEPT, 0)).await?;
        },
        TradeChange::Confirmed { from_id, peer_id } => {
            send_trade(state, peer_id, MsgTrade::new(from_id, trade::ACCEPT, 1)).await?;
        },
        TradeChange::ReadyToComplete(session) => complete_trade(state, session).await?,
        TradeChange::Closed { owner_id, target_id } => {
            send_trade(state, owner_id, MsgTrade::new(target_id, trade::HIDE_TABLE, 0)).await?;
            if target_id != owner_id {
                send_trade(state, target_id, MsgTrade::new(owner_id, trade::HIDE_TABLE, 0)).await?;
            }
        },
    }
    Ok(())
}

pub async fn expire_trades(state: &State) -> Result<(), Error> {
    for change in state.trade_manager().expire() {
        send_change(state, change).await?;
    }
    Ok(())
}

#[async_trait]
impl PacketProcess for MsgTrade {
    type ActorState = ActorState;
    type Error = Error;
    type State = State;

    async fn process(&self, state: &Self::State, actor: &Actor<Self::ActorState>) -> Result<(), Self::Error> {
        let entity = actor.entity();
        let character = entity.as_character().ok_or(Error::CharacterNotFound)?;
        if self.subtype == trade::REQUEST {
            target_entity(state, character, self.target).map_err(|error| Error::Other(error.message().to_owned()))?;
        }
        if self.subtype == trade::ADD_ITEM {
            let Some(item) = character
                .inventory_items()
                .into_iter()
                .find(|item| item.uid == self.target)
            else {
                actor
                    .send(MsgTalk::from_system(
                        character.id(),
                        TalkChannel::Talk,
                        "Only inventory items can be traded.",
                    ))
                    .await?;
                return Ok(());
            };
            let Some(session) = state.trade_manager().session(character.id()) else {
                actor
                    .send(MsgTalk::from_system(
                        character.id(),
                        TalkChannel::Talk,
                        TradeError::NoTrade.message(),
                    ))
                    .await?;
                return Ok(());
            };
            let peer_id = session.peer(character.id()).ok_or(Error::CharacterNotFound)?;
            let Some(peer_entity) = character_entity(state, peer_id) else {
                actor
                    .send(MsgTalk::from_system(
                        character.id(),
                        TalkChannel::Talk,
                        TradeError::RequestMismatch.message(),
                    ))
                    .await?;
                return Ok(());
            };
            let peer = peer_entity.as_character().ok_or(Error::CharacterNotFound)?;
            let peer_outgoing = session.items_for(peer_id).unwrap_or_default().len();
            let actor_offered = session.items_for(character.id()).unwrap_or_default().len();
            if peer.inventory_items().len().saturating_sub(peer_outgoing) + actor_offered + 1 > 40 {
                actor
                    .send(MsgTalk::from_system(
                        character.id(),
                        TalkChannel::Talk,
                        "The other inventory cannot hold this trade.",
                    ))
                    .await?;
                return Ok(());
            }
            let _ = item;
        }
        if self.subtype == trade::SET_MONEY && u64::from(self.target) > character.silver() {
            actor
                .send(MsgTalk::from_system(
                    character.id(),
                    TalkChannel::Talk,
                    "You do not have enough silver for this trade.",
                ))
                .await?;
            return Ok(());
        }
        if self.subtype == trade::SET_CPS && u64::from(self.target) > character.cps() {
            actor
                .send(MsgTalk::from_system(
                    character.id(),
                    TalkChannel::Talk,
                    "You do not have enough CPs for this trade.",
                ))
                .await?;
            return Ok(());
        }
        // Items and amounts travel in the `target` field, as in the classic
        // client.
        let data = if matches!(
            self.subtype,
            trade::ADD_ITEM | trade::REMOVE_ITEM | trade::SET_MONEY | trade::SET_CPS
        ) {
            self.target
        } else {
            self.data
        };
        match state
            .trade_manager()
            .apply(character.id(), self.target, self.subtype, data)
        {
            Ok(change) => send_change(state, change).await,
            Err(error) => {
                actor
                    .send(MsgTalk::from_system(character.id(), TalkChannel::Talk, error.message()))
                    .await?;
                Ok(())
            },
        }
    }
}
