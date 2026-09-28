use parking_lot::Mutex;
use std::collections::{HashMap, HashSet};

pub const MAX_TEAM_MEMBERS: usize = 5;

pub const CREATE: u32 = 0;
pub const REQUEST_JOIN: u32 = 1;
pub const LEAVE_TEAM: u32 = 2;
pub const ACCEPT_INVITE: u32 = 3;
pub const REQUEST_INVITE: u32 = 4;
pub const ACCEPT_JOIN: u32 = 5;
pub const DISMISS: u32 = 6;
pub const KICK: u32 = 7;
pub const FORBID_NEW_MEMBERS: u32 = 8;
pub const ALLOW_NEW_MEMBERS: u32 = 9;
pub const LEADER: u32 = 15;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TeamError {
    AlreadyInTeam,
    NoTeam,
    NotLeader,
    TeamFull,
    TargetAlreadyInTeam,
    TargetNotFound,
    NoPendingInvite,
    NoPendingJoin,
    JoinDisabled,
    InvalidAction,
}

impl TeamError {
    pub fn message(&self) -> &'static str {
        match self {
            Self::AlreadyInTeam => "You are already in a team.",
            Self::NoTeam => "You are not in a team.",
            Self::NotLeader => "Only the team leader can do that.",
            Self::TeamFull => "The team is full.",
            Self::TargetAlreadyInTeam => "The target is already in a team.",
            Self::TargetNotFound => "The target is not available.",
            Self::NoPendingInvite => "There is no pending team invitation.",
            Self::NoPendingJoin => "There is no pending join request for that character.",
            Self::JoinDisabled => "This team is not accepting new members.",
            Self::InvalidAction => "That team action is not supported.",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TeamChange {
    State {
        action: u32,
        leader_id: u32,
        recipients: Vec<u32>,
        members: Vec<u32>,
        added: Option<u32>,
        removed: Option<u32>,
        target_id: u32,
    },
    Invite {
        leader_id: u32,
        target_id: u32,
    },
    JoinRequest {
        leader_id: u32,
        requester_id: u32,
    },
}

#[derive(Debug, Default)]
struct TeamRecord {
    leader_id: u32,
    members: Vec<u32>,
    join_enabled: bool,
}

#[derive(Debug, Default)]
struct TeamBook {
    teams: HashMap<u32, TeamRecord>,
    membership: HashMap<u32, u32>,
    invites: HashMap<u32, u32>,
    join_requests: HashMap<u32, HashSet<u32>>,
}

/// Ephemeral party state. Teams are intentionally not persisted: a reconnect
/// must not resurrect stale online members, while the database remains the
/// authority for character and item state.
#[derive(Debug, Default)]
pub struct TeamManager {
    book: Mutex<TeamBook>,
}

impl TeamManager {
    /// Returns the current members of the actor's team. A solo character is
    /// returned as a one-member party so reward code can use one path for both
    /// solo and team play.
    pub fn members_for(&self, character_id: u32) -> Vec<u32> {
        let book = self.book.lock();
        let Some(team_id) = book.membership.get(&character_id).copied() else {
            return vec![character_id];
        };
        book.teams
            .get(&team_id)
            .map(|team| team.members.clone())
            .unwrap_or_else(|| vec![character_id])
    }

    pub fn same_team(&self, left_id: u32, right_id: u32) -> bool {
        if left_id == right_id {
            return true;
        }
        let book = self.book.lock();
        book.membership.get(&left_id).is_some() && book.membership.get(&left_id) == book.membership.get(&right_id)
    }

    pub fn remove_member(&self, character_id: u32) {
        let _ = self.apply(character_id, 0, LEAVE_TEAM);
    }

    pub fn apply(&self, actor_id: u32, target_id: u32, action: u32) -> Result<TeamChange, TeamError> {
        let mut book = self.book.lock();
        match action {
            CREATE => {
                if book.membership.contains_key(&actor_id) {
                    return Err(TeamError::AlreadyInTeam);
                }
                book.teams.insert(
                    actor_id,
                    TeamRecord {
                        leader_id: actor_id,
                        members: vec![actor_id],
                        join_enabled: true,
                    },
                );
                book.membership.insert(actor_id, actor_id);
                Ok(TeamChange::State {
                    action,
                    leader_id: actor_id,
                    recipients: vec![actor_id],
                    members: vec![actor_id],
                    added: Some(actor_id),
                    removed: None,
                    target_id: actor_id,
                })
            },
            REQUEST_INVITE => {
                let Some(team_id) = book.membership.get(&actor_id).copied() else {
                    return Err(TeamError::NoTeam);
                };
                let team = book.teams.get(&team_id).ok_or(TeamError::NoTeam)?;
                if team.leader_id != actor_id {
                    return Err(TeamError::NotLeader);
                }
                if team.members.len() >= MAX_TEAM_MEMBERS {
                    return Err(TeamError::TeamFull);
                }
                if book.membership.contains_key(&target_id) {
                    return Err(TeamError::TargetAlreadyInTeam);
                }
                book.invites.insert(target_id, actor_id);
                Ok(TeamChange::Invite {
                    leader_id: actor_id,
                    target_id,
                })
            },
            ACCEPT_INVITE => {
                if book.membership.contains_key(&actor_id) {
                    return Err(TeamError::AlreadyInTeam);
                }
                let leader_id = book.invites.remove(&actor_id).ok_or(TeamError::NoPendingInvite)?;
                let team = book.teams.get_mut(&leader_id).ok_or(TeamError::NoTeam)?;
                if !team.join_enabled {
                    return Err(TeamError::JoinDisabled);
                }
                if team.members.len() >= MAX_TEAM_MEMBERS {
                    return Err(TeamError::TeamFull);
                }
                if target_id != 0 && target_id != leader_id {
                    return Err(TeamError::TargetNotFound);
                }
                team.members.push(actor_id);
                let members = team.members.clone();
                book.membership.insert(actor_id, leader_id);
                Ok(TeamChange::State {
                    action,
                    leader_id,
                    recipients: members.clone(),
                    members,
                    added: Some(actor_id),
                    removed: None,
                    target_id: actor_id,
                })
            },
            REQUEST_JOIN => {
                if book.membership.contains_key(&actor_id) {
                    return Err(TeamError::AlreadyInTeam);
                }
                let leader_id = book
                    .membership
                    .get(&target_id)
                    .copied()
                    .ok_or(TeamError::TargetNotFound)?;
                let team = book.teams.get(&leader_id).ok_or(TeamError::NoTeam)?;
                if team.members.len() >= MAX_TEAM_MEMBERS {
                    return Err(TeamError::TeamFull);
                }
                if !team.join_enabled {
                    return Err(TeamError::JoinDisabled);
                }
                book.join_requests.entry(leader_id).or_default().insert(actor_id);
                Ok(TeamChange::JoinRequest {
                    leader_id,
                    requester_id: actor_id,
                })
            },
            ACCEPT_JOIN => {
                let Some(leader_id) = book.membership.get(&actor_id).copied() else {
                    return Err(TeamError::NoTeam);
                };
                {
                    let team = book.teams.get(&leader_id).ok_or(TeamError::NoTeam)?;
                    if team.leader_id != actor_id {
                        return Err(TeamError::NotLeader);
                    }
                    if team.members.len() >= MAX_TEAM_MEMBERS {
                        return Err(TeamError::TeamFull);
                    }
                }
                if book.membership.contains_key(&target_id) {
                    return Err(TeamError::TargetAlreadyInTeam);
                }
                let requests = book.join_requests.get_mut(&leader_id).ok_or(TeamError::NoPendingJoin)?;
                if !requests.remove(&target_id) {
                    return Err(TeamError::NoPendingJoin);
                }
                if requests.is_empty() {
                    book.join_requests.remove(&leader_id);
                }
                let team = book.teams.get_mut(&leader_id).ok_or(TeamError::NoTeam)?;
                team.members.push(target_id);
                let members = team.members.clone();
                book.membership.insert(target_id, leader_id);
                Ok(TeamChange::State {
                    action,
                    leader_id,
                    recipients: members.clone(),
                    members,
                    added: Some(target_id),
                    removed: None,
                    target_id,
                })
            },
            LEAVE_TEAM => {
                let team_id = book.membership.remove(&actor_id).ok_or(TeamError::NoTeam)?;
                let team = book.teams.remove(&team_id).ok_or(TeamError::NoTeam)?;
                if team.leader_id == actor_id {
                    for member_id in &team.members {
                        book.membership.remove(member_id);
                    }
                    return Ok(TeamChange::State {
                        action: DISMISS,
                        leader_id: actor_id,
                        recipients: team.members,
                        members: Vec::new(),
                        added: None,
                        removed: Some(actor_id),
                        target_id: actor_id,
                    });
                }
                let recipients = team.members.clone();
                let mut members = team.members;
                members.retain(|member_id| *member_id != actor_id);
                let leader_id = team.leader_id;
                book.teams.insert(
                    team_id,
                    TeamRecord {
                        leader_id,
                        members: members.clone(),
                        join_enabled: team.join_enabled,
                    },
                );
                Ok(TeamChange::State {
                    action,
                    leader_id,
                    recipients,
                    members,
                    added: None,
                    removed: Some(actor_id),
                    target_id: actor_id,
                })
            },
            KICK => {
                let team_id = book.membership.get(&actor_id).copied().ok_or(TeamError::NoTeam)?;
                let team = book.teams.get_mut(&team_id).ok_or(TeamError::NoTeam)?;
                if team.leader_id != actor_id {
                    return Err(TeamError::NotLeader);
                }
                if target_id == actor_id || !team.members.contains(&target_id) {
                    return Err(TeamError::TargetNotFound);
                }
                let recipients = team.members.clone();
                team.members.retain(|member_id| *member_id != target_id);
                let members = team.members.clone();
                book.membership.remove(&target_id);
                Ok(TeamChange::State {
                    action,
                    leader_id: actor_id,
                    recipients,
                    members,
                    added: None,
                    removed: Some(target_id),
                    target_id,
                })
            },
            DISMISS => {
                let team_id = book.membership.get(&actor_id).copied().ok_or(TeamError::NoTeam)?;
                let team = book.teams.get(&team_id).ok_or(TeamError::NoTeam)?;
                if team.leader_id != actor_id {
                    return Err(TeamError::NotLeader);
                }
                let team = book.teams.remove(&team_id).ok_or(TeamError::NoTeam)?;
                for member_id in &team.members {
                    book.membership.remove(member_id);
                }
                Ok(TeamChange::State {
                    action,
                    leader_id: actor_id,
                    recipients: team.members,
                    members: Vec::new(),
                    added: None,
                    removed: None,
                    target_id: actor_id,
                })
            },
            FORBID_NEW_MEMBERS | ALLOW_NEW_MEMBERS => {
                let team_id = book.membership.get(&actor_id).copied().ok_or(TeamError::NoTeam)?;
                let team = book.teams.get_mut(&team_id).ok_or(TeamError::NoTeam)?;
                if team.leader_id != actor_id {
                    return Err(TeamError::NotLeader);
                }
                team.join_enabled = action == ALLOW_NEW_MEMBERS;
                Ok(TeamChange::State {
                    action,
                    leader_id: actor_id,
                    recipients: team.members.clone(),
                    members: team.members.clone(),
                    added: None,
                    removed: None,
                    target_id: actor_id,
                })
            },
            LEADER => {
                let team_id = book.membership.get(&actor_id).copied().ok_or(TeamError::NoTeam)?;
                let team = book.teams.get(&team_id).ok_or(TeamError::NoTeam)?;
                if team.leader_id != actor_id {
                    return Err(TeamError::NotLeader);
                }
                if target_id == actor_id || !team.members.contains(&target_id) {
                    return Err(TeamError::TargetNotFound);
                }
                let mut team = book.teams.remove(&team_id).ok_or(TeamError::NoTeam)?;
                team.leader_id = target_id;
                let members = team.members.clone();
                for member_id in &members {
                    book.membership.insert(*member_id, target_id);
                }
                book.teams.insert(target_id, team);
                Ok(TeamChange::State {
                    action,
                    leader_id: target_id,
                    recipients: members.clone(),
                    members,
                    added: None,
                    removed: None,
                    target_id,
                })
            },
            _ => Err(TeamError::InvalidAction),
        }
    }

    pub fn snapshot(&self, character_id: u32) -> Option<Vec<u32>> {
        let book = self.book.lock();
        let team_id = book.membership.get(&character_id)?;
        book.teams.get(team_id).map(|team| team.members.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invite_accept_and_leave_update_membership_atomically() {
        let manager = TeamManager::default();
        let TeamChange::State { members, .. } = manager.apply(10, 0, CREATE).unwrap() else {
            panic!("create should change team state");
        };
        assert_eq!(members, vec![10]);
        assert!(matches!(
            manager.apply(10, 20, REQUEST_INVITE),
            Ok(TeamChange::Invite { .. })
        ));
        assert!(matches!(
            manager.apply(20, 10, ACCEPT_INVITE),
            Ok(TeamChange::State { added: Some(20), .. })
        ));
        assert_eq!(manager.snapshot(10).unwrap(), vec![10, 20]);
        assert_eq!(manager.members_for(10), vec![10, 20]);
        assert!(manager.same_team(10, 20));
        assert!(!manager.same_team(10, 30));
        assert!(matches!(
            manager.apply(20, 0, LEAVE_TEAM),
            Ok(TeamChange::State { removed: Some(20), .. })
        ));
        assert!(manager.snapshot(20).is_none());
        assert_eq!(manager.snapshot(10).unwrap(), vec![10]);
    }

    #[test]
    fn team_rejects_non_leader_invites_and_caps_at_five_members() {
        let manager = TeamManager::default();
        manager.apply(1, 0, CREATE).unwrap();
        manager.apply(1, 2, REQUEST_INVITE).unwrap();
        manager.apply(2, 1, ACCEPT_INVITE).unwrap();
        assert_eq!(manager.apply(2, 3, REQUEST_INVITE), Err(TeamError::NotLeader));
        for member_id in 3..=5 {
            manager.apply(1, member_id, REQUEST_INVITE).unwrap();
            manager.apply(member_id, 1, ACCEPT_INVITE).unwrap();
        }
        assert_eq!(manager.apply(1, 6, REQUEST_INVITE), Err(TeamError::TeamFull));
        assert_eq!(manager.members_for(1).len(), MAX_TEAM_MEMBERS);
    }

    #[test]
    fn leadership_transfer_updates_the_team_index_and_permissions() {
        let manager = TeamManager::default();
        manager.apply(10, 0, CREATE).unwrap();
        manager.apply(10, 20, REQUEST_INVITE).unwrap();
        manager.apply(20, 10, ACCEPT_INVITE).unwrap();

        let TeamChange::State {
            leader_id,
            members,
            action,
            ..
        } = manager.apply(10, 20, LEADER).unwrap()
        else {
            panic!("leadership transfer should broadcast a state change");
        };
        assert_eq!(action, LEADER);
        assert_eq!(leader_id, 20);
        assert_eq!(members, vec![10, 20]);
        assert_eq!(manager.snapshot(10).unwrap(), vec![10, 20]);
        assert_eq!(manager.snapshot(20).unwrap(), vec![10, 20]);
        assert_eq!(manager.apply(10, 20, REQUEST_INVITE), Err(TeamError::NotLeader));

        manager.apply(20, 10, LEADER).unwrap();
        assert_eq!(manager.snapshot(10).unwrap(), vec![10, 20]);
    }
}
