use parking_lot::Mutex;
use std::collections::HashMap;
use std::time::{Duration, Instant};

pub const REQUEST: u32 = 1;
pub const CLOSE: u32 = 2;
pub const SHOW_TABLE: u32 = 3;
pub const HIDE_TABLE: u32 = 5;
pub const ADD_ITEM: u32 = 6;
pub const SET_MONEY: u32 = 7;
pub const SHOW_MONEY: u32 = 8;
pub const ACCEPT: u32 = 10;
pub const REMOVE_ITEM: u32 = 11;
pub const TIME_OUT: u32 = 17;
pub const TRADE_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TradeSession {
    pub owner_id: u32,
    pub target_id: u32,
    pub open: bool,
    pub owner_items: Vec<u32>,
    pub target_items: Vec<u32>,
    pub owner_silver: u32,
    pub target_silver: u32,
    pub owner_confirmed: bool,
    pub target_confirmed: bool,
    last_activity: Instant,
}

impl TradeSession {
    pub fn new(owner_id: u32, target_id: u32) -> Self {
        Self {
            owner_id,
            target_id,
            open: false,
            owner_items: Vec::new(),
            target_items: Vec::new(),
            owner_silver: 0,
            target_silver: 0,
            owner_confirmed: false,
            target_confirmed: false,
            last_activity: Instant::now(),
        }
    }

    pub fn contains(&self, character_id: u32) -> bool {
        self.owner_id == character_id || self.target_id == character_id
    }

    pub fn peer(&self, character_id: u32) -> Option<u32> {
        if self.owner_id == character_id {
            Some(self.target_id)
        } else if self.target_id == character_id {
            Some(self.owner_id)
        } else {
            None
        }
    }

    pub fn items_for(&self, character_id: u32) -> Option<&[u32]> {
        if self.owner_id == character_id {
            Some(&self.owner_items)
        } else if self.target_id == character_id {
            Some(&self.target_items)
        } else {
            None
        }
    }

    pub fn items_for_mut(&mut self, character_id: u32) -> Option<&mut Vec<u32>> {
        if self.owner_id == character_id {
            Some(&mut self.owner_items)
        } else if self.target_id == character_id {
            Some(&mut self.target_items)
        } else {
            None
        }
    }

    pub fn silver_for(&self, character_id: u32) -> Option<u32> {
        if self.owner_id == character_id {
            Some(self.owner_silver)
        } else if self.target_id == character_id {
            Some(self.target_silver)
        } else {
            None
        }
    }

    pub fn set_silver(&mut self, character_id: u32, amount: u32) -> bool {
        if self.owner_id == character_id {
            self.owner_silver = amount;
            true
        } else if self.target_id == character_id {
            self.target_silver = amount;
            true
        } else {
            false
        }
    }

    pub fn confirmed(&self, character_id: u32) -> Option<bool> {
        if self.owner_id == character_id {
            Some(self.owner_confirmed)
        } else if self.target_id == character_id {
            Some(self.target_confirmed)
        } else {
            None
        }
    }

    pub fn set_confirmed(&mut self, character_id: u32, value: bool) -> bool {
        if self.owner_id == character_id {
            self.owner_confirmed = value;
            true
        } else if self.target_id == character_id {
            self.target_confirmed = value;
            true
        } else {
            false
        }
    }

    pub fn reset_confirmations(&mut self) {
        self.owner_confirmed = false;
        self.target_confirmed = false;
    }

    pub fn both_confirmed(&self) -> bool {
        self.owner_confirmed && self.target_confirmed
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TradeError {
    AlreadyTrading,
    NoTrade,
    RequestMismatch,
    TradeNotOpen,
    ItemAlreadyOffered,
    ItemNotOffered,
    InvalidAmount,
    NotParticipant,
}

impl TradeError {
    pub fn message(&self) -> &'static str {
        match self {
            Self::AlreadyTrading => "The player is already trading.",
            Self::NoTrade => "You are not in a trade.",
            Self::RequestMismatch => "That trade request is no longer valid.",
            Self::TradeNotOpen => "The trade window is not open.",
            Self::ItemAlreadyOffered => "That item is already in the trade.",
            Self::ItemNotOffered => "That item is not in the trade.",
            Self::InvalidAmount => "That trade amount is invalid.",
            Self::NotParticipant => "You are not a participant in that trade.",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TradeChange {
    Request { from_id: u32, to_id: u32 },
    Open(TradeSession),
    ItemAdded { from_id: u32, peer_id: u32, item_uid: u32 },
    ItemRemoved { from_id: u32, peer_id: u32, item_uid: u32 },
    MoneySet { from_id: u32, peer_id: u32, amount: u32 },
    Confirmed { from_id: u32, peer_id: u32 },
    ReadyToComplete(TradeSession),
    Closed { owner_id: u32, target_id: u32 },
}

#[derive(Debug, Default)]
struct TradeBook {
    sessions: HashMap<u32, TradeSession>,
}

/// Ephemeral, server-authoritative trade sessions. Items and silver are only
/// moved by the atomic Character::complete_trade operation after both sides
/// confirm the exact same offer state.
#[derive(Debug, Default)]
pub struct TradeManager {
    book: Mutex<TradeBook>,
}

impl TradeManager {
    pub fn session(&self, character_id: u32) -> Option<TradeSession> {
        self.book.lock().sessions.get(&character_id).cloned()
    }

    pub fn remove_member(&self, character_id: u32) -> Option<TradeChange> {
        let mut book = self.book.lock();
        let session = book.sessions.remove(&character_id)?;
        book.sessions.remove(&session.owner_id);
        book.sessions.remove(&session.target_id);
        Some(TradeChange::Closed {
            owner_id: session.owner_id,
            target_id: session.target_id,
        })
    }

    pub fn expire(&self) -> Vec<TradeChange> {
        let mut book = self.book.lock();
        let expired = book
            .sessions
            .values()
            .filter(|session| session.last_activity.elapsed() >= TRADE_TIMEOUT)
            .map(|session| (session.owner_id, session.target_id))
            .collect::<std::collections::HashSet<_>>();
        let mut changes = Vec::with_capacity(expired.len());
        for (owner_id, target_id) in expired {
            book.sessions.remove(&owner_id);
            book.sessions.remove(&target_id);
            changes.push(TradeChange::Closed { owner_id, target_id });
        }
        changes
    }

    pub fn apply(&self, actor_id: u32, target_id: u32, subtype: u32, data: u32) -> Result<TradeChange, TradeError> {
        let mut book = self.book.lock();
        match subtype {
            REQUEST => {
                if let Some(existing) = book.sessions.get(&actor_id).cloned() {
                    if !existing.open && existing.owner_id == target_id && existing.target_id == actor_id {
                        let mut opened = existing;
                        opened.open = true;
                        opened.last_activity = Instant::now();
                        book.sessions.insert(opened.owner_id, opened.clone());
                        book.sessions.insert(opened.target_id, opened.clone());
                        return Ok(TradeChange::Open(opened));
                    }
                    return Err(TradeError::AlreadyTrading);
                }
                if book.sessions.contains_key(&target_id) {
                    return Err(TradeError::AlreadyTrading);
                }
                let session = TradeSession::new(actor_id, target_id);
                book.sessions.insert(actor_id, session.clone());
                book.sessions.insert(target_id, session);
                Ok(TradeChange::Request {
                    from_id: actor_id,
                    to_id: target_id,
                })
            },
            CLOSE | TIME_OUT => {
                let session = book.sessions.remove(&actor_id).ok_or(TradeError::NoTrade)?;
                book.sessions.remove(&session.owner_id);
                book.sessions.remove(&session.target_id);
                Ok(TradeChange::Closed {
                    owner_id: session.owner_id,
                    target_id: session.target_id,
                })
            },
            _ => {
                let mut session = book.sessions.get(&actor_id).cloned().ok_or(TradeError::NoTrade)?;
                if !session.open {
                    return Err(TradeError::TradeNotOpen);
                }
                session.last_activity = Instant::now();
                let peer_id = session.peer(actor_id).ok_or(TradeError::NotParticipant)?;
                match subtype {
                    ADD_ITEM => {
                        if session.owner_items.contains(&data) || session.target_items.contains(&data) {
                            return Err(TradeError::ItemAlreadyOffered);
                        }
                        session
                            .items_for_mut(actor_id)
                            .ok_or(TradeError::NotParticipant)?
                            .push(data);
                        session.reset_confirmations();
                        book.sessions.insert(session.owner_id, session.clone());
                        book.sessions.insert(session.target_id, session);
                        Ok(TradeChange::ItemAdded {
                            from_id: actor_id,
                            peer_id,
                            item_uid: data,
                        })
                    },
                    REMOVE_ITEM => {
                        let offered = session.items_for_mut(actor_id).ok_or(TradeError::NotParticipant)?;
                        let Some(index) = offered.iter().position(|uid| *uid == data) else {
                            return Err(TradeError::ItemNotOffered);
                        };
                        offered.remove(index);
                        session.reset_confirmations();
                        book.sessions.insert(session.owner_id, session.clone());
                        book.sessions.insert(session.target_id, session);
                        Ok(TradeChange::ItemRemoved {
                            from_id: actor_id,
                            peer_id,
                            item_uid: data,
                        })
                    },
                    SET_MONEY => {
                        if !session.set_silver(actor_id, data) {
                            return Err(TradeError::NotParticipant);
                        }
                        session.reset_confirmations();
                        book.sessions.insert(session.owner_id, session.clone());
                        book.sessions.insert(session.target_id, session);
                        Ok(TradeChange::MoneySet {
                            from_id: actor_id,
                            peer_id,
                            amount: data,
                        })
                    },
                    ACCEPT => {
                        if !session.set_confirmed(actor_id, true) {
                            return Err(TradeError::NotParticipant);
                        }
                        if session.both_confirmed() {
                            Ok(TradeChange::ReadyToComplete(session))
                        } else {
                            book.sessions.insert(session.owner_id, session.clone());
                            book.sessions.insert(session.target_id, session);
                            Ok(TradeChange::Confirmed {
                                from_id: actor_id,
                                peer_id,
                            })
                        }
                    },
                    _ => Err(TradeError::RequestMismatch),
                }
            },
        }
    }

    pub fn finish(&self, owner_id: u32, target_id: u32) {
        let mut book = self.book.lock();
        book.sessions.remove(&owner_id);
        book.sessions.remove(&target_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn confirmations_reset_when_the_offer_changes() {
        let manager = TradeManager::default();
        assert_eq!(
            manager.apply(1, 2, REQUEST, 0),
            Ok(TradeChange::Request { from_id: 1, to_id: 2 })
        );
        assert!(matches!(manager.apply(2, 1, REQUEST, 0), Ok(TradeChange::Open(_))));
        assert!(matches!(
            manager.apply(1, 2, ACCEPT, 0),
            Ok(TradeChange::Confirmed { .. })
        ));
        assert!(matches!(
            manager.apply(2, 1, SET_MONEY, 25),
            Ok(TradeChange::MoneySet { amount: 25, .. })
        ));
        assert!(matches!(
            manager.apply(2, 1, ACCEPT, 0),
            Ok(TradeChange::Confirmed { .. })
        ));
        assert!(matches!(
            manager.apply(1, 2, ACCEPT, 0),
            Ok(TradeChange::ReadyToComplete(_))
        ));
    }

    #[test]
    fn duplicate_items_and_cross_participant_removal_are_rejected() {
        let manager = TradeManager::default();
        manager.apply(1, 2, REQUEST, 0).unwrap();
        manager.apply(2, 1, REQUEST, 0).unwrap();
        manager.apply(1, 2, ADD_ITEM, 77).unwrap();
        assert_eq!(manager.apply(2, 1, ADD_ITEM, 77), Err(TradeError::ItemAlreadyOffered));
        assert_eq!(manager.apply(2, 1, REMOVE_ITEM, 77), Err(TradeError::ItemNotOffered));
    }

    #[test]
    fn stale_sessions_are_expired_for_both_participants() {
        let manager = TradeManager::default();
        manager.apply(1, 2, REQUEST, 0).unwrap();
        manager.apply(2, 1, REQUEST, 0).unwrap();
        let stale = Instant::now()
            .checked_sub(TRADE_TIMEOUT + Duration::from_secs(1))
            .expect("test clock should support the timeout interval");
        {
            let mut book = manager.book.lock();
            for session in book.sessions.values_mut() {
                session.last_activity = stale;
            }
        }
        assert_eq!(manager.expire().len(), 1);
        assert!(manager.session(1).is_none());
        assert!(manager.session(2).is_none());
    }
}
