use crate::Error;
use tq_network::ActorHandle;

mod floor_item;
pub use floor_item::{FloorItem, Item};

mod status;
pub use status::{StatusEffect, StatusEffects};

mod basic;
pub use basic::{Entity, Flags};

mod character;
pub use character::{
    ArtifactOutcome, AttributeAllocation, AttributeKind, Character, GemComposeOutcome, InventoryItem,
    ItemUpgradeOutcome, PurchaseOutcome, QualityUpgradeOutcome, RarityBonusOutcome, RarityUpgradeOutcome,
    RefineryOutcome, SaleOutcome, SkillProgress, SocketOutcome, TradeTransfer, GEM_FAMILY_BASES,
};

mod npc;
pub use npc::{Npc, NpcBase, NpcKind, NpcSort};

mod monster;
pub use monster::Monster;

#[derive(Debug)]
pub enum GameEntity {
    Character(Character),
    Npc(Npc),
    Monster(Monster),
}

impl From<Character> for GameEntity {
    fn from(v: Character) -> Self {
        Self::Character(v)
    }
}

impl From<Npc> for GameEntity {
    fn from(v: Npc) -> Self {
        Self::Npc(v)
    }
}

impl From<Monster> for GameEntity {
    fn from(v: Monster) -> Self {
        Self::Monster(v)
    }
}

impl GameEntity {
    /// Returns the ID of the Game Entity.
    pub fn id(&self) -> u32 {
        match self {
            Self::Character(v) => v.id(),
            Self::Npc(v) => v.id(),
            Self::Monster(v) => v.id(),
        }
    }

    /// Returns the Owner of the Game Entity.
    ///
    /// Only [`Character`]s have an owner.
    pub fn owner(&self) -> Option<ActorHandle> {
        match self {
            Self::Character(v) => Some(v.owner()),
            Self::Npc(..) | Self::Monster(..) => None,
        }
    }

    pub fn basic(&self) -> &Entity {
        match self {
            Self::Character(v) => v.entity(),
            Self::Npc(v) => v.entity(),
            Self::Monster(v) => v.entity(),
        }
    }

    /// This method sends the spawn packet to another entity
    pub async fn send_spawn(&self, to: &Self) -> Result<(), Error> {
        match (self, to) {
            (Self::Character(from), Self::Character(to)) => from.send_spawn(&to.owner()).await,
            (Self::Npc(from), Self::Character(to)) => from.send_spawn(&to.owner()).await,
            (Self::Monster(from), Self::Character(to)) => from.send_spawn(&to.owner()).await,
            _ => todo!("send_spawn for non-character entities"),
        }
    }

    /// Returns `true` if the game entity is [`Character`].
    ///
    /// [`Character`]: GameEntity::Character
    #[must_use]
    pub fn is_character(&self) -> bool {
        matches!(self, Self::Character(..))
    }

    pub fn as_character(&self) -> Option<&Character> {
        if let Self::Character(v) = self {
            Some(v)
        } else {
            None
        }
    }

    /// Returns `true` if the game entity is [`Npc`].
    ///
    /// [`Npc`]: GameEntity::Npc
    #[must_use]
    pub fn is_npc(&self) -> bool {
        matches!(self, Self::Npc(..))
    }

    pub fn as_npc(&self) -> Option<&Npc> {
        if let Self::Npc(v) = self {
            Some(v)
        } else {
            None
        }
    }

    pub fn as_monster(&self) -> Option<&Monster> {
        if let Self::Monster(v) = self {
            Some(v)
        } else {
            None
        }
    }

    pub fn apply_status(&self, effect: StatusEffect, duration: std::time::Duration) {
        match self {
            Self::Character(character) => character.apply_status(effect, duration),
            Self::Monster(monster) => monster.apply_status(effect, duration),
            Self::Npc(..) => {},
        }
    }

    pub fn has_status(&self, effect: StatusEffect) -> bool {
        match self {
            Self::Character(character) => character.has_status(effect),
            Self::Monster(monster) => monster.has_status(effect),
            Self::Npc(..) => false,
        }
    }

    pub fn tick_statuses(&self, now: std::time::Instant) -> u32 {
        match self {
            Self::Character(character) => character.tick_statuses(now),
            Self::Monster(monster) => monster.tick_statuses(now),
            Self::Npc(..) => 0,
        }
    }

    pub fn expire_statuses(&self, now: std::time::Instant) -> bool {
        match self {
            Self::Character(character) => character.expire_statuses(now),
            Self::Monster(monster) => monster.expire_statuses(now),
            Self::Npc(..) => false,
        }
    }
}
