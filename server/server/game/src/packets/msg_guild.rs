use crate::packets::{MsgGuildMember, MsgTalk, TalkChannel};
use crate::{ActorState, Error, State};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tq_network::{Actor, PacketID, PacketProcess};
use tq_serde::StringList;

pub const APPLY_JOIN: u32 = 1;
pub const INVITE_JOIN: u32 = 2;
pub const LEAVE_GUILD: u32 = 3;
pub const KICK_MEMBER: u32 = 4;
pub const QUERY_GUILD_NAME: u32 = 6;
pub const SET_GUILD: u32 = 14;
/// Client: give `data` silver to the guild fund (the original's donation).
pub const DONATE_SILVER: u32 = 11;
/// Client: ask the fund; server: the fund, in `data` (capped) and as text.
pub const GUILD_FUND: u32 = 30;
/// Client (leader): take `data` silver from the fund.
pub const WITHDRAW_SILVER: u32 = 31;
/// Silver a donation or withdrawal moves, at most.
const MAX_FUND_MOVE: u32 = 100_000_000;

/// Classic 5065 guild action packet (1107).
#[derive(Debug, Clone, Serialize, Deserialize, PacketID)]
#[packet(id = 1107)]
pub struct MsgGuild {
    pub action: u32,
    pub data: u32,
    pub required_level: i32,
    pub required_rebirth: i32,
    pub required_profession: i32,
    pub strings: StringList,
}

impl MsgGuild {
    pub fn new(action: u32, data: u32) -> Self {
        Self {
            action,
            data,
            required_level: 0,
            required_rebirth: 0,
            required_profession: 0,
            strings: StringList::new(),
        }
    }

    pub fn with_strings(mut self, strings: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.strings = strings.into_iter().map(Into::into).collect();
        self
    }
}

fn character_entity(state: &State, character_id: u32) -> Option<std::sync::Arc<crate::entities::GameEntity>> {
    let entity = state.entity(character_id)?;
    entity.is_character().then_some(entity)
}

async fn send_notice(actor: &Actor<ActorState>, character_id: u32, message: impl Into<String>) -> Result<(), Error> {
    actor
        .send(MsgTalk::from_system(character_id, TalkChannel::Guild, message))
        .await?;
    Ok(())
}

async fn send_to_character(state: &State, character_id: u32, packet: MsgGuild) -> Result<(), Error> {
    if let Some(entity) = character_entity(state, character_id) {
        if let Some(owner) = entity.owner() {
            owner.send(packet).await?;
        }
    }
    Ok(())
}

pub async fn send_guild_members(
    state: &State,
    recipient_id: u32,
    guild: &crate::guild::GuildInfo,
) -> Result<(), Error> {
    let Some(entity) = character_entity(state, recipient_id) else {
        return Ok(());
    };
    let Some(owner) = entity.owner() else {
        return Ok(());
    };
    for (index, member_id) in guild.members.iter().enumerate() {
        let Some(member_entity) = character_entity(state, *member_id) else {
            continue;
        };
        let Some(member) = member_entity.as_character() else {
            continue;
        };
        let rank = if *member_id == guild.leader_id {
            1000
        } else {
            state.guild_manager().role_for(*member_id).map_or(200, |role| u16::try_from(role).unwrap_or(200))
        };
        owner
            .send(MsgGuildMember::from_character(member, rank, index as i32))
            .await?;
    }
    Ok(())
}

async fn broadcast_guild_members(state: &State, guild: &crate::guild::GuildInfo) -> Result<(), Error> {
    for member_id in &guild.members {
        send_guild_members(state, *member_id, guild).await?;
    }
    Ok(())
}

#[async_trait]
impl PacketProcess for MsgGuild {
    type ActorState = ActorState;
    type Error = Error;
    type State = State;

    async fn process(&self, state: &State, actor: &Actor<ActorState>) -> Result<(), Error> {
        let entity = actor.entity();
        let character = entity.as_character().ok_or(Error::CharacterNotFound)?;
        let character_id = character.id();
        let result = match self.action {
            INVITE_JOIN => match state.guild_manager().accept_invite(character_id) {
                Ok(change) => {
                    let guild = match change {
                        crate::guild::GuildChange::State { guild, .. } => guild,
                        _ => unreachable!("accepting a guild invite must return state"),
                    };
                    let guild_name = guild.name.clone();
                    actor
                        .send(MsgGuild::new(SET_GUILD, guild.id).with_strings([guild_name]))
                        .await?;
                    crate::guild::GuildManager::persist_member(state.pool(), &guild, character_id).await?;
                    broadcast_guild_members(state, &guild).await?;
                    send_notice(actor, character_id, "You joined the guild.").await?;
                    Ok(())
                },
                Err(crate::guild::GuildError::NoPendingInvite) => {
                    if self.data == 0 || self.data == character_id {
                        return send_notice(actor, character_id, "Invalid guild invitation target.").await;
                    }
                    state
                        .guild_manager()
                        .invite(character_id, self.data)
                        .map_err(|error| Error::Other(error.message().to_owned()))?;
                    send_to_character(state, self.data, MsgGuild::new(INVITE_JOIN, character_id)).await?;
                    send_notice(actor, character_id, "Guild invitation sent.").await?;
                    send_notice_to_character(
                        state,
                        self.data,
                        format!("Character {character_id} invited you to a guild."),
                    )
                    .await?;
                    Ok(())
                },
                Err(error) => Err(Error::Other(error.message().to_owned())),
            },
            APPLY_JOIN => Err(Error::Other("Guild applications are not wired yet.".to_owned())),
            LEAVE_GUILD => {
                let change = state
                    .guild_manager()
                    .leave(character_id)
                    .map_err(|error| Error::Other(error.message().to_owned()))?;
                if let crate::guild::GuildChange::State {
                    guild,
                    removed: Some(removed),
                    ..
                } = change
                {
                    crate::guild::GuildManager::persist_removal(state.pool(), &guild, removed).await?;
                    actor.send(MsgGuild::new(SET_GUILD, 0)).await?;
                    if !guild.members.is_empty() {
                        broadcast_guild_members(state, &guild).await?;
                    }
                }
                send_notice(actor, character_id, "You left the guild.").await
            },
            KICK_MEMBER => {
                let change = state
                    .guild_manager()
                    .kick(character_id, self.data)
                    .map_err(|error| Error::Other(error.message().to_owned()))?;
                send_to_character(state, self.data, MsgGuild::new(SET_GUILD, 0)).await?;
                if let crate::guild::GuildChange::State {
                    guild,
                    removed: Some(removed),
                    ..
                } = change
                {
                    crate::guild::GuildManager::persist_removal(state.pool(), &guild, removed).await?;
                    broadcast_guild_members(state, &guild).await?;
                }
                send_notice(actor, character_id, "Guild member removed.").await
            },
            QUERY_GUILD_NAME => {
                let Some(guild) = state.guild_manager().guild_for(self.data) else {
                    return send_notice(actor, character_id, "That character is not in a guild.").await;
                };
                actor
                    .send(MsgGuild::new(QUERY_GUILD_NAME, guild.id).with_strings([guild.name]))
                    .await?;
                Ok(())
            },
            SET_GUILD => Ok(()),
            GUILD_FUND => {
                if let Some(guild) = state.guild_manager().guild_for(character_id) {
                    let fund = crate::guild::GuildManager::fund(state.pool(), guild.id).await?;
                    actor.send(fund_packet(fund)).await?;
                }
                Ok(())
            },
            DONATE_SILVER | WITHDRAW_SILVER => move_fund(state, actor, character, self.action == DONATE_SILVER, self.data).await,
            _ => Err(Error::Other(format!("Unsupported guild action {}.", self.action))),
        };
        if let Err(error) = result {
            send_notice(actor, character_id, error.to_string()).await?;
        }
        Ok(())
    }
}

fn fund_packet(fund: u64) -> MsgGuild {
    MsgGuild::new(GUILD_FUND, u32::try_from(fund).unwrap_or(u32::MAX)).with_strings([fund.to_string()])
}

/// A donation to the guild fund, or the leader taking from it; every member
/// online hears of it and sees the new fund.
async fn move_fund(
    state: &State,
    actor: &Actor<ActorState>,
    character: &crate::entities::Character,
    donate: bool,
    amount: u32,
) -> Result<(), Error> {
    let character_id = character.id();
    let Some(guild) = state.guild_manager().guild_for(character_id) else {
        return send_notice(actor, character_id, "You are not in a guild.").await;
    };
    if !donate && guild.leader_id != character_id {
        return send_notice(actor, character_id, "Only the guild leader can take from the fund.").await;
    }
    if amount == 0 || amount > MAX_FUND_MOVE {
        return Ok(());
    }
    let Some((_, fund)) = character.transfer_guild_fund(state.pool(), guild.id, amount, donate).await? else {
        let reason = if donate { "You do not have that much silver." } else { "The guild fund does not hold that much." };
        return send_notice(actor, character_id, reason).await;
    };
    actor.send(super::MsgUserInfo::from(character)).await?;
    let name = character.entity().name();
    let line = if donate {
        format!("{name} gave {amount} silver to the guild fund ({fund} now).")
    } else {
        format!("{name} took {amount} silver from the guild fund ({fund} left).")
    };
    tracing::info!(target: "economy", character = %name, guild = %guild.name, amount, donate, fund, "Guild fund");
    for member_id in &guild.members {
        send_to_character(state, *member_id, fund_packet(fund)).await?;
        send_notice_to_character(state, *member_id, line.clone()).await?;
    }
    Ok(())
}

async fn send_notice_to_character(state: &State, character_id: u32, message: impl Into<String>) -> Result<(), Error> {
    if let Some(entity) = character_entity(state, character_id) {
        if let Some(owner) = entity.owner() {
            owner
                .send(MsgTalk::from_system(character_id, TalkChannel::Guild, message))
                .await?;
        }
    }
    Ok(())
}
