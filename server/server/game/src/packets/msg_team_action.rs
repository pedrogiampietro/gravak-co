use crate::entities::GameEntity;
use crate::packets::{MsgTalk, MsgTeam, MsgTeamMember, TalkChannel};
use crate::team::{self, TeamChange, TeamError};
use crate::{ActorState, Error, State};
use async_trait::async_trait;
use std::sync::Arc;
use tq_network::{Actor, PacketID, PacketProcess};

#[derive(Debug, serde::Deserialize, Clone, PacketID)]
#[packet(id = 1023)]
pub struct MsgTeamAction {
    action: u32,
    target_id: u32,
    _reserved: u32,
}

impl MsgTeamAction {
    fn target(&self) -> u32 {
        self.target_id
    }
}

fn target_entity(
    state: &State,
    actor: &crate::entities::Character,
    target_id: u32,
) -> Result<Arc<GameEntity>, TeamError> {
    let Some(target) = state.entity(target_id) else {
        return Err(TeamError::TargetNotFound);
    };
    let Some(target_character) = target.as_character() else {
        return Err(TeamError::TargetNotFound);
    };
    if target_character.entity().map_id() != actor.entity().map_id()
        || !tq_math::in_screen(
            actor.entity().location().into(),
            target_character.entity().location().into(),
        )
    {
        return Err(TeamError::TargetNotFound);
    }
    Ok(target)
}

fn character_entity(state: &State, character_id: u32) -> Option<Arc<GameEntity>> {
    let entity = state.entity(character_id)?;
    entity.is_character().then_some(entity)
}

async fn send_notice(state: &State, character_id: u32, message: impl Into<String>) -> Result<(), Error> {
    if let Some(entity) = character_entity(state, character_id) {
        entity
            .owner()
            .ok_or(Error::CharacterNotFound)?
            .send(MsgTalk::from_system(character_id, TalkChannel::Team, message))
            .await?;
    }
    Ok(())
}

/// A character's name, for the team's notices.
fn name_of(state: &State, character_id: u32) -> String {
    state
        .entity(character_id)
        .and_then(|entity| entity.as_character().map(|character| character.entity().name().to_owned()))
        .unwrap_or_else(|| "A player".to_owned())
}

async fn send_change(state: &State, change: TeamChange) -> Result<(), Error> {
    match change {
        TeamChange::Invite { leader_id, target_id } => {
            let leader_name = name_of(state, leader_id);
            if let Some(target) = character_entity(state, target_id) {
                target
                    .owner()
                    .ok_or(Error::CharacterNotFound)?
                    .send(MsgTeam::new(team::REQUEST_INVITE, leader_id))
                    .await?;
            }
            send_notice(state, leader_id, format!("Team invitation sent to {}.", name_of(state, target_id))).await?;
            send_notice(state, target_id, format!("{leader_name} invited you to their team.")).await?;
        },
        TeamChange::JoinRequest {
            leader_id,
            requester_id,
        } => {
            if let Some(leader) = character_entity(state, leader_id) {
                leader
                    .owner()
                    .ok_or(Error::CharacterNotFound)?
                    .send(MsgTeam::new(team::REQUEST_JOIN, requester_id))
                    .await?;
            }
            send_notice(state, leader_id, format!("{} asks to join your team.", name_of(state, requester_id))).await?;
            send_notice(state, requester_id, format!("You asked to join {}'s team.", name_of(state, leader_id))).await?;
        },
        TeamChange::State {
            action,
            leader_id,
            recipients,
            members,
            added,
            removed,
            target_id,
        } => {
            for recipient_id in &recipients {
                let Some(recipient) = character_entity(state, *recipient_id) else {
                    continue;
                };
                recipient
                    .owner()
                    .ok_or(Error::CharacterNotFound)?
                    .send(MsgTeam::new(action, target_id))
                    .await?;
            }
            if let Some(added_id) = added {
                for recipient_id in &recipients {
                    let Some(recipient) = character_entity(state, *recipient_id) else {
                        continue;
                    };
                    let member_ids = if *recipient_id == added_id {
                        members.clone()
                    } else {
                        vec![added_id]
                    };
                    for member_id in member_ids {
                        if let Some(member) = character_entity(state, member_id) {
                            recipient
                                .owner()
                                .ok_or(Error::CharacterNotFound)?
                                .send(MsgTeamMember::from_character(
                                    member.as_character().ok_or(Error::CharacterNotFound)?,
                                    0,
                                ))
                                .await?;
                        }
                    }
                }
                if action == team::CREATE {
                    send_notice(state, added_id, "You made a team: invite players with the Team window's Invite.").await?;
                } else {
                    // The newcomer learns who leads (the others know).
                    if let Some(added) = character_entity(state, added_id) {
                        added
                            .owner()
                            .ok_or(Error::CharacterNotFound)?
                            .send(MsgTeam::new(team::LEADER, leader_id))
                            .await?;
                    }
                    send_notice(state, added_id, format!("You joined {}'s team.", name_of(state, leader_id))).await?;
                    let name = name_of(state, added_id);
                    for recipient_id in recipients.iter().filter(|id| **id != added_id) {
                        send_notice(state, *recipient_id, format!("{name} joined the team.")).await?;
                    }
                }
            }
            if let Some(removed_id) = removed {
                for recipient_id in &recipients {
                    let Some(recipient) = character_entity(state, *recipient_id) else {
                        continue;
                    };
                    if let Some(member) = character_entity(state, removed_id) {
                        recipient
                            .owner()
                            .ok_or(Error::CharacterNotFound)?
                            .send(MsgTeamMember::from_character(
                                member.as_character().ok_or(Error::CharacterNotFound)?,
                                1,
                            ))
                            .await?;
                    }
                }
            }
            if let Some(removed_id) = removed.filter(|_| action != team::DISMISS) {
                let name = name_of(state, removed_id);
                let (yours, theirs) = if action == team::KICK {
                    ("You were removed from the team.".to_owned(), format!("{name} was removed from the team."))
                } else {
                    ("You left the team.".to_owned(), format!("{name} left the team."))
                };
                for recipient_id in &recipients {
                    let line = if *recipient_id == removed_id { yours.clone() } else { theirs.clone() };
                    send_notice(state, *recipient_id, line).await?;
                }
            }
            match action {
                team::DISMISS => {
                    for recipient_id in recipients {
                        send_notice(state, recipient_id, "The team was dismissed.").await?;
                    }
                },
                team::LEADER => {
                    let name = name_of(state, leader_id);
                    for recipient_id in recipients {
                        send_notice(state, recipient_id, format!("{name} now leads the team.")).await?;
                    }
                },
                team::FORBID_NEW_MEMBERS => {
                    send_notice(state, leader_id, "The team takes no new members now.").await?;
                },
                team::ALLOW_NEW_MEMBERS => {
                    send_notice(state, leader_id, "The team takes new members again.").await?;
                },
                _ => {},
            }
        },
    }
    Ok(())
}

/// A member gone offline leaves their team, and the others are told, as
/// if they had left (a leader's going ends the team). The one leaving is
/// not written to: their connection is closing.
pub async fn left_team_on_logout(state: &State, character_id: u32) -> Result<(), Error> {
    let Ok(mut change) = state.team_manager().apply(character_id, 0, team::LEAVE_TEAM) else {
        return Ok(());
    };
    if let TeamChange::State { recipients, .. } = &mut change {
        recipients.retain(|recipient| *recipient != character_id);
    }
    send_change(state, change).await
}

#[async_trait]
impl PacketProcess for MsgTeamAction {
    type ActorState = ActorState;
    type Error = Error;
    type State = State;

    async fn process(&self, state: &Self::State, actor: &Actor<Self::ActorState>) -> Result<(), Self::Error> {
        let entity = actor.entity();
        let character = entity.as_character().ok_or(Error::CharacterNotFound)?;
        let target_id = self.target();
        let target_required = matches!(
            self.action,
            team::REQUEST_JOIN
                | team::REQUEST_INVITE
                | team::ACCEPT_JOIN
                | team::KICK
                | team::ACCEPT_INVITE
                | team::LEADER
        );
        if target_required && target_entity(state, character, target_id).is_err() {
            actor
                .send(MsgTalk::from_system(character.id(), TalkChannel::Team, "That player is not near enough."))
                .await?;
            return Ok(());
        }
        // Inviting without a team makes one first, led by the inviter.
        if self.action == team::REQUEST_INVITE && state.team_manager().snapshot(character.id()).is_none() {
            if let Ok(change) = state.team_manager().apply(character.id(), 0, team::CREATE) {
                send_change(state, change).await?;
            }
        }
        match state.team_manager().apply(character.id(), target_id, self.action) {
            Ok(change) => send_change(state, change).await,
            Err(error) => {
                actor
                    .send(MsgTalk::from_system(character.id(), TalkChannel::Team, error.message()))
                    .await?;
                Ok(())
            },
        }
    }
}
