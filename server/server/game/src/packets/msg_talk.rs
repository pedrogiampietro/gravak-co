use crate::constants::{ALL_USERS, CHARACTER_ID_MIN, SYSTEM};
use crate::state::State;
use crate::systems::commands;
use crate::ActorState;
use async_trait::async_trait;
use num_enum::{FromPrimitive, IntoPrimitive};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tq_network::{Actor, PacketID, PacketProcess};

/// Enumeration for defining the channel text is printed to. Can also print to
/// separate states of the client such as character registration, and can be
/// used to change the state of the client or deny a login.
#[derive(Copy, Clone, Debug, FromPrimitive, IntoPrimitive)]
#[repr(u16)]
pub enum TalkChannel {
    Talk = 2000,
    Whisper = 2001,
    Action = 2002,
    Team = 2003,
    Guild = 2004,
    Spouse = 2006,
    System = 2007,
    Yell = 2008,
    Friend = 2009,
    Center = 2011,
    TopLeft = 2012,
    Ghost = 2013,
    Service = 2014,
    Tip = 2015,
    World = 2021,
    Register = 2100,
    Login = 2101,
    Shop = 2102,
    Vendor = 2104,
    Website = 2105,
    Right1 = 2108,
    Right2 = 2109,
    Offline = 2110,
    Announce = 2111,
    TradeBoard = 2201,
    FriendBoard = 2202,
    TeamBoard = 2203,
    GuildBoard = 2204,
    OthersBoard = 2205,
    Broadcast = 2500,
    Monster = 2600,
    #[num_enum(default)]
    Unknown,
}
/// Enumeration type for controlling how text is stylized in the client's chat
/// area. By default, text appears and fades overtime. This can be overridden
/// with multiple styles, hard-coded into the client.
#[derive(Copy, Clone, Debug, FromPrimitive, IntoPrimitive)]
#[repr(u16)]
pub enum TalkStyle {
    Normal = 0,
    Scroll = 1,
    Flash = 2,
    Blast = 3,
    #[num_enum(default)]
    Unknown,
}

/// Message defining a chat message from one player to the other, or from the
/// system to a player. Used for all chat systems in the game, including
/// messages outside of the game world state, such as during character creation
/// or to tell the client to continue logging in after connect.
#[derive(Debug, Default, Deserialize, Serialize, PacketID, Clone)]
#[packet(id = 1004)]
pub struct MsgTalk {
    pub color: u32,
    pub channel: u16,
    pub style: u16,
    pub character_id: u32,
    pub recipient_mesh: u32,
    pub sender_mesh: u32,
    pub list_count: u8,
    pub sender_name: String,
    pub recipient_name: String,
    pub suffix: String,
    pub message: String,
}

impl MsgTalk {
    pub fn from_system(character_id: u32, channel: TalkChannel, message: impl Into<String>) -> Self {
        MsgTalk {
            color: 0x00FF_FFFF,
            channel: channel.into(),
            style: TalkStyle::Normal.into(),
            character_id,
            recipient_mesh: 0,
            sender_mesh: 0,
            list_count: 4,
            sender_name: SYSTEM.to_string(),
            recipient_name: ALL_USERS.to_string(),
            suffix: String::new(),
            message: message.into(),
        }
    }

    pub fn login_invalid() -> Self {
        Self::from_system(0, TalkChannel::Login, "Login Invalid")
    }

    pub fn register_invalid() -> Self {
        Self::from_system(0, TalkChannel::Register, String::from("Register Invalid"))
    }

    pub fn register_ok() -> Self {
        Self::from_system(0, TalkChannel::Register, crate::constants::ANSWER_OK.to_owned())
    }

    pub fn register_name_taken() -> Self {
        Self::from_system(
            0,
            TalkChannel::Register,
            String::from("Character name taken, try another one."),
        )
    }

    pub fn login_ok() -> Self {
        Self::from_system(0, TalkChannel::Login, crate::constants::ANSWER_OK.to_owned())
    }

    pub fn login_new_role() -> Self {
        Self::from_system(0, TalkChannel::Login, crate::constants::NEW_ROLE.to_owned())
    }
}

fn character_named(state: &State, name: &str) -> Option<Arc<crate::entities::GameEntity>> {
    state.entities().into_iter().find(|entity| {
        entity
            .as_character()
            .is_some_and(|character| character.entity().name().eq_ignore_ascii_case(name))
    })
}

async fn deliver_whisper(state: &State, actor: &Actor<ActorState>, message: &MsgTalk) -> Result<(), crate::Error> {
    let recipient_name = message.recipient_name.trim();
    let Some(recipient) = character_named(state, recipient_name) else {
        actor
            .send(MsgTalk::from_system(
                actor.entity().id(),
                TalkChannel::Whisper,
                format!("Character {recipient_name} was not found."),
            ))
            .await?;
        return Ok(());
    };
    if crate::packets::msg_friend::is_ignored(state, recipient.id(), actor.entity().id()).await? {
        actor
            .send(MsgTalk::from_system(
                actor.entity().id(),
                TalkChannel::Whisper,
                format!("Character {recipient_name} is ignoring you."),
            ))
            .await?;
        return Ok(());
    }
    let mut outgoing = message.clone();
    outgoing.channel = TalkChannel::Whisper.into();
    outgoing.recipient_name = recipient_name.to_owned();
    recipient
        .owner()
        .ok_or(crate::Error::CharacterNotFound)?
        .send(outgoing.clone())
        .await?;
    if recipient.id() != actor.entity().id() {
        actor.send(outgoing.clone()).await?;
    }
    persist_chat_for_owners(state, &[actor.entity().id(), recipient.id()], &outgoing).await?;
    Ok(())
}

fn database_id(character_id: u32) -> i64 {
    character_id.saturating_sub(CHARACTER_ID_MIN) as i64
}

fn persists_channel(channel: u16) -> bool {
    channel == TalkChannel::Talk as u16 || channel == TalkChannel::World as u16
}

async fn persist_chat_for_owners(state: &State, owner_ids: &[u32], message: &MsgTalk) -> Result<(), crate::Error> {
    if !persists_channel(message.channel) && message.channel != TalkChannel::Whisper as u16 {
        return Ok(());
    }
    let mut transaction = state.pool().begin().await?;
    let mut owners = owner_ids.iter().copied().map(database_id).collect::<Vec<_>>();
    owners.sort_unstable();
    owners.dedup();
    for owner_id in owners {
        sqlx::query(
            "INSERT INTO character_chat_history (character_id, channel, sender_name, recipient_name, message) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(owner_id)
        .bind(i64::from(message.channel))
        .bind(&message.sender_name)
        .bind(&message.recipient_name)
        .bind(&message.message)
        .execute(&mut *transaction)
        .await?;
        sqlx::query(
            "DELETE FROM character_chat_history WHERE character_id = ? AND id NOT IN (SELECT id FROM character_chat_history WHERE character_id = ? ORDER BY id DESC LIMIT 40)",
        )
        .bind(owner_id)
        .bind(owner_id)
        .execute(&mut *transaction)
        .await?;
    }
    transaction.commit().await?;
    Ok(())
}

async fn persist_public_chat(state: &State, recipient_ids: &[u32], message: &MsgTalk) -> Result<(), crate::Error> {
    persist_chat_for_owners(state, recipient_ids, message).await
}

pub async fn send_chat_history(
    state: &State,
    actor: &Actor<ActorState>,
    character_id: u32,
) -> Result<(), crate::Error> {
    let rows = sqlx::query_as::<_, (u16, String, String, String, i64)>(
        "SELECT channel, sender_name, recipient_name, message, created_at FROM character_chat_history WHERE character_id = ? AND channel IN (?, ?, ?) ORDER BY id ASC LIMIT 40",
    )
    .bind(database_id(character_id))
    .bind(i64::from(TalkChannel::Talk as u16))
    .bind(i64::from(TalkChannel::Whisper as u16))
    .bind(i64::from(TalkChannel::World as u16))
    .fetch_all(state.pool())
    .await?;
    for (channel, sender_name, recipient_name, message, _) in rows {
        actor
            .send(MsgTalk {
                color: 0x00FF_FFFF,
                channel,
                style: TalkStyle::Normal.into(),
                character_id,
                recipient_mesh: 0,
                sender_mesh: 0,
                list_count: 4,
                sender_name,
                recipient_name,
                suffix: String::new(),
                message,
            })
            .await?;
    }
    Ok(())
}

#[async_trait]
impl PacketProcess for MsgTalk {
    type ActorState = ActorState;
    type Error = crate::Error;
    type State = State;

    async fn process(&self, state: &Self::State, actor: &Actor<Self::ActorState>) -> Result<(), Self::Error> {
        if self.channel == TalkChannel::Whisper.into() {
            return deliver_whisper(state, actor, self).await;
        }
        let is_command = self.message.starts_with('$');
        if is_command {
            // Command Message.
            let (_, command) = self.message.split_at(1);
            let args: Vec<_> = command.split_whitespace().collect();
            commands::parse_and_execute(state, actor, &args).await?;
        }
        // For now, we just broadcast the message to all players in our region.
        // TODO: Implement this properly.
        let map_id = actor.entity().basic().map_id();
        let loc = actor.entity().basic().location();
        let mymap = state.try_map(map_id)?;
        let myregion = mymap.region(loc.x, loc.y).ok_or(crate::Error::MapRegionNotFound)?;
        let recipient_ids = myregion.with_entities(|entities| {
            entities
                .values()
                .filter_map(|entity| entity.upgrade())
                .filter_map(|entity| entity.as_character().map(|character| character.id()))
                .collect::<Vec<_>>()
        });
        if !is_command {
            persist_public_chat(state, &recipient_ids, self).await?;
        }
        myregion.broadcast(self.clone()).await?;
        Ok(())
    }
}
