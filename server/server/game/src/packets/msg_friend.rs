use crate::constants::CHARACTER_ID_MIN;
use crate::entities::GameEntity;
use crate::packets::{MsgTalk, TalkChannel};
use crate::{ActorState, Error, State};
use async_trait::async_trait;
use std::sync::Arc;
use tq_network::{Actor, PacketID, PacketProcess};
use tq_serde::String16;

pub const APPLY: u8 = 10;
pub const ACCEPT: u8 = 11;
pub const ONLINE: u8 = 12;
pub const OFFLINE: u8 = 13;
pub const BREAK: u8 = 14;
pub const GET_INFO: u8 = 15;
pub const ENEMY_DELETE: u8 = 18;
pub const ENEMY_ADD: u8 = 19;

fn database_id(character_id: u32) -> i64 {
    character_id.saturating_sub(CHARACTER_ID_MIN) as i64
}

/// Classic 5065 friend request/update packet (1019).
#[derive(Debug, serde::Deserialize, serde::Serialize, Clone, PacketID)]
#[packet(id = 1019)]
pub struct MsgFriend {
    pub friend_id: u32,
    pub action: u8,
    pub online_flag: u8,
    pub reserved: u16,
    pub reserved12: u32,
    pub reserved16: u32,
    pub name: String16,
    pub reserved36: u32,
}

impl MsgFriend {
    pub fn new(action: u8, friend_id: u32, name: impl Into<String>, online: bool) -> Self {
        Self {
            friend_id,
            action,
            online_flag: u8::from(online),
            reserved: 0,
            reserved12: 0,
            reserved16: 0,
            name: name.into().into(),
            reserved36: 0,
        }
    }
}

fn target_entity(state: &State, actor: &crate::entities::Character, target_id: u32) -> Result<Arc<GameEntity>, Error> {
    let target = state.entity(target_id).ok_or(Error::CharacterNotFound)?;
    let target_character = target.as_character().ok_or(Error::CharacterNotFound)?;
    if target_character.entity().map_id() != actor.entity().map_id()
        || !tq_math::in_screen(
            actor.entity().location().into(),
            target_character.entity().location().into(),
        )
    {
        return Err(Error::Other("Friend target is not nearby.".to_owned()));
    }
    Ok(target)
}

async fn is_friend(state: &State, character_id: u32, friend_id: u32) -> Result<bool, Error> {
    Ok(sqlx::query_scalar::<_, i64>(
        "SELECT EXISTS(SELECT 1 FROM character_friends WHERE character_id = ? AND friend_id = ?)",
    )
    .bind(database_id(character_id))
    .bind(database_id(friend_id))
    .fetch_one(state.pool())
    .await?
        != 0)
}

pub async fn is_ignored(state: &State, character_id: u32, ignored_id: u32) -> Result<bool, Error> {
    Ok(sqlx::query_scalar::<_, i64>(
        "SELECT EXISTS(SELECT 1 FROM character_ignores WHERE character_id = ? AND ignored_id = ?)",
    )
    .bind(database_id(character_id))
    .bind(database_id(ignored_id))
    .fetch_one(state.pool())
    .await?
        != 0)
}

async fn set_ignore(state: &State, character_id: u32, ignored_id: u32, ignored_name: &str) -> Result<(), Error> {
    sqlx::query("INSERT OR IGNORE INTO character_ignores (character_id, ignored_id, ignored_name) VALUES (?, ?, ?)")
        .bind(database_id(character_id))
        .bind(database_id(ignored_id))
        .bind(ignored_name)
        .execute(state.pool())
        .await?;
    Ok(())
}

async fn remove_ignore(state: &State, character_id: u32, ignored_id: u32) -> Result<(), Error> {
    sqlx::query("DELETE FROM character_ignores WHERE character_id = ? AND ignored_id = ?")
        .bind(database_id(character_id))
        .bind(database_id(ignored_id))
        .execute(state.pool())
        .await?;
    Ok(())
}

async fn add_mutual_friendship(
    state: &State,
    left_id: u32,
    right_id: u32,
    left_name: &str,
    right_name: &str,
) -> Result<(), Error> {
    let mut transaction = state.pool().begin().await?;
    sqlx::query("INSERT OR IGNORE INTO character_friends (character_id, friend_id, friend_name) VALUES (?, ?, ?)")
        .bind(database_id(left_id))
        .bind(database_id(right_id))
        .bind(right_name)
        .execute(&mut *transaction)
        .await?;
    sqlx::query("INSERT OR IGNORE INTO character_friends (character_id, friend_id, friend_name) VALUES (?, ?, ?)")
        .bind(database_id(right_id))
        .bind(database_id(left_id))
        .bind(left_name)
        .execute(&mut *transaction)
        .await?;
    transaction.commit().await?;
    Ok(())
}

async fn remove_mutual_friendship(state: &State, left_id: u32, right_id: u32) -> Result<(), Error> {
    let mut transaction = state.pool().begin().await?;
    sqlx::query("DELETE FROM character_friends WHERE character_id = ? AND friend_id = ?")
        .bind(database_id(left_id))
        .bind(database_id(right_id))
        .execute(&mut *transaction)
        .await?;
    sqlx::query("DELETE FROM character_friends WHERE character_id = ? AND friend_id = ?")
        .bind(database_id(right_id))
        .bind(database_id(left_id))
        .execute(&mut *transaction)
        .await?;
    transaction.commit().await?;
    Ok(())
}

async fn send_notice(actor: &Actor<ActorState>, character_id: u32, message: impl Into<String>) -> Result<(), Error> {
    actor
        .send(MsgTalk::from_system(character_id, TalkChannel::System, message))
        .await?;
    Ok(())
}

async fn send_to_character(state: &State, character_id: u32, packet: MsgFriend) -> Result<(), Error> {
    if let Some(entity) = state.entity(character_id) {
        entity.owner().ok_or(Error::CharacterNotFound)?.send(packet).await?;
    }
    Ok(())
}

pub async fn send_friend_list(state: &State, actor: &Actor<ActorState>, character_id: u32) -> Result<(), Error> {
    let rows = sqlx::query_as::<_, (i64, String)>(
        "SELECT friend_id, friend_name FROM character_friends WHERE character_id = ? ORDER BY friend_name",
    )
    .bind(database_id(character_id))
    .fetch_all(state.pool())
    .await?;
    for (friend_id, friend_name) in rows {
        let friend_id = friend_id as u32 + CHARACTER_ID_MIN;
        let online = state.entity(friend_id).is_some();
        actor
            .send(MsgFriend::new(GET_INFO, friend_id as u32, friend_name, online))
            .await?;
    }
    Ok(())
}

pub async fn send_ignore_list(state: &State, actor: &Actor<ActorState>, character_id: u32) -> Result<(), Error> {
    let rows = sqlx::query_as::<_, (i64, String)>(
        "SELECT ignored_id, ignored_name FROM character_ignores WHERE character_id = ? ORDER BY ignored_name",
    )
    .bind(database_id(character_id))
    .fetch_all(state.pool())
    .await?;
    for (ignored_id, ignored_name) in rows {
        let ignored_id = ignored_id as u32 + CHARACTER_ID_MIN;
        let online = state.entity(ignored_id).is_some();
        actor
            .send(MsgFriend::new(ENEMY_ADD, ignored_id, ignored_name, online))
            .await?;
    }
    Ok(())
}

#[async_trait]
impl PacketProcess for MsgFriend {
    type ActorState = ActorState;
    type Error = Error;
    type State = State;

    async fn process(&self, state: &Self::State, actor: &Actor<Self::ActorState>) -> Result<(), Self::Error> {
        let entity = actor.entity();
        let character = entity.as_character().ok_or(Error::CharacterNotFound)?;
        let character_id = character.id();
        let character_name = character.entity().name().to_owned();

        match self.action {
            APPLY => {
                let target = target_entity(state, character, self.friend_id)?;
                let target_character = target.as_character().ok_or(Error::CharacterNotFound)?;
                if target_character.id() == character_id {
                    return Ok(());
                }
                if is_friend(state, character_id, target_character.id()).await? {
                    send_notice(actor, character_id, "That character is already your friend.").await?;
                    return Ok(());
                }
                if state.take_friend_request(character_id) == Some(target_character.id()) {
                    add_mutual_friendship(
                        state,
                        character_id,
                        target_character.id(),
                        &character_name,
                        target_character.entity().name(),
                    )
                    .await?;
                    actor
                        .send(MsgFriend::new(
                            GET_INFO,
                            target_character.id(),
                            target_character.entity().name(),
                            true,
                        ))
                        .await?;
                    send_to_character(
                        state,
                        target_character.id(),
                        MsgFriend::new(GET_INFO, character_id, &character_name, true),
                    )
                    .await?;
                    send_notice(actor, character_id, "Friendship accepted.").await?;
                    if let Some(owner) = target.owner() {
                        owner
                            .send(MsgTalk::from_system(
                                target_character.id(),
                                TalkChannel::System,
                                "Friendship accepted.",
                            ))
                            .await?;
                    }
                } else {
                    state.set_friend_request(target_character.id(), character_id);
                    send_to_character(
                        state,
                        target_character.id(),
                        MsgFriend::new(APPLY, character_id, &character_name, true),
                    )
                    .await?;
                    send_notice(actor, character_id, "Friend request sent.").await?;
                    if let Some(owner) = target.owner() {
                        owner
                            .send(MsgTalk::from_system(
                                target_character.id(),
                                TalkChannel::System,
                                format!("{character_name} wants to be your friend."),
                            ))
                            .await?;
                    }
                }
            },
            BREAK => {
                if !is_friend(state, character_id, self.friend_id).await? {
                    send_notice(actor, character_id, "That character is not your friend.").await?;
                    return Ok(());
                }
                remove_mutual_friendship(state, character_id, self.friend_id).await?;
                actor
                    .send(MsgFriend::new(BREAK, self.friend_id, self.name.to_string(), false))
                    .await?;
                send_to_character(
                    state,
                    self.friend_id,
                    MsgFriend::new(BREAK, character_id, &character_name, false),
                )
                .await?;
                send_notice(actor, character_id, "Friend removed.").await?;
            },
            ENEMY_ADD => {
                let target = state.entity(self.friend_id).ok_or(Error::CharacterNotFound)?;
                let target_character = target.as_character().ok_or(Error::CharacterNotFound)?;
                if target_character.id() == character_id {
                    return Ok(());
                }
                set_ignore(
                    state,
                    character_id,
                    target_character.id(),
                    target_character.entity().name(),
                )
                .await?;
                actor
                    .send(MsgFriend::new(
                        ENEMY_ADD,
                        target_character.id(),
                        target_character.entity().name(),
                        true,
                    ))
                    .await?;
                send_notice(actor, character_id, "Character added to ignore list.").await?;
            },
            ENEMY_DELETE => {
                remove_ignore(state, character_id, self.friend_id).await?;
                actor
                    .send(MsgFriend::new(
                        ENEMY_DELETE,
                        self.friend_id,
                        self.name.to_string(),
                        false,
                    ))
                    .await?;
                send_notice(actor, character_id, "Character removed from ignore list.").await?;
            },
            _ => {},
        }
        Ok(())
    }
}
