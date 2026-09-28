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

async fn send_change(state: &State, change: TeamChange) -> Result<(), Error> {
    match change {
        TeamChange::Invite { leader_id, target_id } => {
            let leader_name = state
                .entity(leader_id)
                .and_then(|entity| {
                    entity
                        .as_character()
                        .map(|character| character.entity().name().to_owned())
                })
                .unwrap_or_else(|| "a player".to_owned());
            if let Some(target) = character_entity(state, target_id) {
                target
                    .owner()
                    .ok_or(Error::CharacterNotFound)?
                    .send(MsgTeam::new(team::REQUEST_INVITE, leader_id))
                    .await?;
            }
            send_notice(state, leader_id, format!("Team invitation sent to {target_id}.")).await?;
            send_notice(state, target_id, format!("{leader_name} invited you to a team.")).await?;
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
            send_notice(
                state,
                leader_id,
                format!("Character {requester_id} requested to join your team."),
            )
            .await?;
            send_notice(state, requester_id, "Team join request sent.").await?;
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
                send_notice(state, added_id, format!("You joined team led by {leader_id}.")).await?;
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
            if action == team::DISMISS {
                for recipient_id in recipients {
                    send_notice(state, recipient_id, "The team was dismissed.").await?;
                }
            }
        },
    }
    Ok(())
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
        if target_required {
            target_entity(state, character, target_id).map_err(|error| Error::Other(error.message().to_owned()))?;
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
