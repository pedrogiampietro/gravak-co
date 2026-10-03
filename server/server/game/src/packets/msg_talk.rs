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

    pub fn register_invalid_name() -> Self {
        Self::from_system(
            0,
            TalkChannel::Register,
            String::from("Names must be 3-15 letters or digits."),
        )
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
            "INSERT INTO character_chat_history (character_id, channel, sender_name, recipient_name, message) VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(owner_id)
        .bind(i64::from(message.channel))
        .bind(&message.sender_name)
        .bind(&message.recipient_name)
        .bind(&message.message)
        .execute(&mut *transaction)
        .await?;
        sqlx::query(
            "DELETE FROM character_chat_history WHERE character_id = $1 AND id NOT IN (SELECT id FROM character_chat_history WHERE character_id = $2 ORDER BY id DESC LIMIT 40)",
        )
        .bind(owner_id)
        .bind(owner_id)
        .execute(&mut *transaction)
        .await?;
    }
    transaction.commit().await?;
    Ok(())
}

/// Who hears a message, by its channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Route {
    /// The players around (the map region).
    Region,
    Team,
    Guild,
    Friends,
    Spouse,
    /// Everyone online.
    World,
    /// Channels only the server speaks on (system, center, broadcast...).
    Refused,
}

fn route(channel: u16) -> Route {
    match TalkChannel::from(channel) {
        TalkChannel::Talk | TalkChannel::Action | TalkChannel::Ghost | TalkChannel::Yell => Route::Region,
        TalkChannel::Team => Route::Team,
        TalkChannel::Guild => Route::Guild,
        TalkChannel::Friend => Route::Friends,
        TalkChannel::Spouse => Route::Spouse,
        TalkChannel::World => Route::World,
        _ => Route::Refused,
    }
}

/// The online characters a message on `route` reaches, the speaker among
/// them, or why it reaches nobody.
async fn recipients(
    state: &State,
    me: &crate::entities::Character,
    route: Route,
) -> Result<Result<Vec<u32>, &'static str>, crate::Error> {
    let online = |id: u32| state.entity(id).is_some_and(|entity| entity.is_character());
    let ids = match route {
        Route::Team => {
            let members = state.team_manager().members_for(me.id());
            if members.len() < 2 {
                return Ok(Err("You are not in a team."));
            }
            members
        },
        Route::Guild => match state.guild_manager().guild_for(me.id()) {
            Some(guild) => guild.members,
            None => return Ok(Err("You are not in a guild.")),
        },
        Route::Friends => {
            let friends: Vec<i64> = sqlx::query_scalar("SELECT friend_id FROM character_friends WHERE character_id = $1")
                .bind(database_id(me.id()))
                .fetch_all(state.pool())
                .await?;
            std::iter::once(me.id())
                .chain(friends.into_iter().map(|id| id as u32 + CHARACTER_ID_MIN))
                .collect()
        },
        Route::Spouse => {
            let spouse = me.spouse();
            let partner = state
                .entities()
                .into_iter()
                .find(|entity| entity.as_character().is_some_and(|other| other.entity().name() == spouse));
            match partner {
                Some(partner) if spouse != "None" => vec![me.id(), partner.id()],
                _ if spouse == "None" => return Ok(Err("You are not married.")),
                _ => return Ok(Err("Your spouse is not online.")),
            }
        },
        Route::World => state
            .entities()
            .into_iter()
            .filter(|entity| entity.is_character())
            .map(|entity| entity.id())
            .collect(),
        Route::Region | Route::Refused => Vec::new(),
    };
    Ok(Ok(ids.into_iter().filter(|id| online(*id)).collect()))
}

async fn persist_public_chat(state: &State, recipient_ids: &[u32], message: &MsgTalk) -> Result<(), crate::Error> {
    persist_chat_for_owners(state, recipient_ids, message).await
}

pub async fn send_chat_history(
    state: &State,
    actor: &Actor<ActorState>,
    character_id: u32,
) -> Result<(), crate::Error> {
    let rows = sqlx::query_as::<_, (i64, String, String, String, i64)>(
        "SELECT channel, sender_name, recipient_name, message, created_at FROM character_chat_history WHERE character_id = $1 AND channel IN ($2, $3, $4) ORDER BY id ASC LIMIT 40",
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
                channel: u16::try_from(channel).unwrap_or_default(),
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
        if self.channel == u16::from(TalkChannel::Whisper) {
            return deliver_whisper(state, actor, self).await;
        }
        let is_command = self.message.starts_with('$');
        if is_command {
            // Command Message.
            let (_, command) = self.message.split_at(1);
            let args: Vec<_> = command.split_whitespace().collect();
            commands::parse_and_execute(state, actor, &args).await?;
            // Commands are for the server, not for the people around.
            return Ok(());
        }
        let entity = actor.entity();
        let me = entity.as_character().ok_or(crate::Error::CharacterNotFound)?;
        // The client says who speaks and on which channel; only what a player
        // may say goes out, and always under their own name.
        let route = route(self.channel);
        if route == Route::Refused {
            tracing::warn!(
                target: "world",
                character = %me.entity().name(),
                channel = self.channel,
                "Chat on a channel players cannot use; ignored"
            );
            return Ok(());
        }
        let mut message = self.clone();
        message.sender_name = me.entity().name().to_owned();
        if route == Route::Region {
            let map_id = me.entity().map_id();
            let loc = me.entity().location();
            let mymap = state.try_map(map_id)?;
            let myregion = mymap.region(loc.x, loc.y).ok_or(crate::Error::MapRegionNotFound)?;
            let recipient_ids = myregion.with_entities(|entities| {
                entities
                    .values()
                    .filter_map(|entity| entity.upgrade())
                    .filter_map(|entity| entity.as_character().map(|character| character.id()))
                    .collect::<Vec<_>>()
            });
            persist_public_chat(state, &recipient_ids, &message).await?;
            myregion.broadcast(message).await?;
            return Ok(());
        }
        let recipient_ids = match recipients(state, me, route).await? {
            Ok(ids) => ids,
            Err(reason) => {
                actor
                    .send(MsgTalk::from_system(me.id(), TalkChannel::System, reason))
                    .await?;
                return Ok(());
            },
        };
        persist_public_chat(state, &recipient_ids, &message).await?;
        for id in recipient_ids {
            if let Some(character) = state.entity(id).as_ref().and_then(|entity| entity.as_character()) {
                let _ = character.owner().send(message.clone()).await;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    /// A character of account `account` in `state`, and what it receives.
    async fn player(
        state: &crate::State,
        account: u32,
        name: &str,
    ) -> Result<(tq_network::Actor<crate::ActorState>, tokio::sync::mpsc::Receiver<tq_network::Message>), crate::Error> {
        sqlx::query("INSERT INTO accounts (account_id, username, password) VALUES ($1, $2, '') ON CONFLICT DO NOTHING")
            .bind(i64::from(account))
            .bind(name)
            .execute(state.pool())
            .await?;
        let (tx, rx) = tokio::sync::mpsc::channel(64);
        let actor = tq_network::Actor::<crate::ActorState>::new(tx);
        crate::packets::MsgRegister::build_character_with(
            name.to_owned(),
            crate::packets::BodyType::MuscularMale,
            crate::packets::BaseClass::Trojan,
            account,
            1,
        )?
        .save(state.pool())
        .await?;
        let inner = tq_db::character::Character::from_account(state.pool(), account)
            .await?
            .expect("character");
        let character = crate::entities::Character::new(actor.handle(), inner);
        actor.set_id(character.id() as usize);
        let screen = crate::systems::Screen::new(actor.handle());
        actor.update(character, screen);
        state.insert_entity(actor.entity());
        Ok((actor, rx))
    }

    /// The chat lines in `rx` so far.
    fn heard(rx: &mut tokio::sync::mpsc::Receiver<tq_network::Message>) -> Vec<super::MsgTalk> {
        use tq_network::{PacketDecode, PacketID};
        let mut lines = Vec::new();
        while let Ok(message) = rx.try_recv() {
            if let tq_network::Message::Packet(id, bytes) = message {
                if id == super::MsgTalk::PACKET_ID {
                    lines.push(super::MsgTalk::decode(&bytes).expect("chat line"));
                }
            }
        }
        lines
    }

    #[tokio::test]
    async fn world_chat_reaches_everyone_under_the_real_name() -> Result<(), crate::Error> {
        use tq_network::PacketProcess;
        let state = crate::State::with_pool(tq_db::testing::pool().await).await?;
        let (speaker, _) = player(&state, 1, "Speaker").await?;
        let (_listener, mut listener_rx) = player(&state, 2, "Listener").await?;
        let spoofed = super::MsgTalk {
            channel: super::TalkChannel::World as u16,
            sender_name: "GM".to_owned(),
            message: "hello".to_owned(),
            ..Default::default()
        };
        spoofed.process(&state, &speaker).await?;
        let lines = heard(&mut listener_rx);
        assert_eq!(lines.len(), 1, "the listener hears the world line");
        assert_eq!(lines[0].sender_name, "Speaker");
        assert_eq!(lines[0].message, "hello");

        let announcement = super::MsgTalk {
            channel: super::TalkChannel::Broadcast as u16,
            message: "fake announcement".to_owned(),
            ..Default::default()
        };
        announcement.process(&state, &speaker).await?;
        assert!(heard(&mut listener_rx).is_empty(), "nobody hears a forged announcement");
        Ok(())
    }

    #[test]
    fn players_speak_only_on_player_channels() {
        use super::{route, Route, TalkChannel};
        assert_eq!(route(TalkChannel::Talk as u16), Route::Region);
        assert_eq!(route(TalkChannel::Team as u16), Route::Team);
        assert_eq!(route(TalkChannel::Guild as u16), Route::Guild);
        assert_eq!(route(TalkChannel::Friend as u16), Route::Friends);
        assert_eq!(route(TalkChannel::Spouse as u16), Route::Spouse);
        assert_eq!(route(TalkChannel::World as u16), Route::World);
        for server_only in [TalkChannel::System, TalkChannel::Center, TalkChannel::Broadcast, TalkChannel::Announce] {
            assert_eq!(route(server_only as u16), Route::Refused);
        }
        assert_eq!(route(1), Route::Refused);
    }
}
