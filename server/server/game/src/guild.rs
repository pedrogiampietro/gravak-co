use crate::constants::CHARACTER_ID_MIN;
use parking_lot::Mutex;
use sqlx::PgPool;
use std::collections::{HashMap, HashSet};

pub const MAX_GUILD_MEMBERS: usize = 50;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuildInfo {
    pub id: u32,
    pub name: String,
    pub leader_id: u32,
    pub members: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuildError {
    AlreadyInGuild,
    NotInGuild,
    NotLeader,
    GuildFull,
    TargetAlreadyInGuild,
    TargetNotFound,
    NoPendingInvite,
    InvalidName,
    NameAlreadyUsed,
    CannotKickLeader,
}

impl GuildError {
    pub fn message(&self) -> &'static str {
        match self {
            Self::AlreadyInGuild => "You are already in a guild.",
            Self::NotInGuild => "You are not in a guild.",
            Self::NotLeader => "Only the guild leader can do that.",
            Self::GuildFull => "The guild is full.",
            Self::TargetAlreadyInGuild => "The target is already in a guild.",
            Self::TargetNotFound => "The target is not available.",
            Self::NoPendingInvite => "There is no pending guild invitation.",
            Self::InvalidName => "Guild names must contain 3 to 24 characters.",
            Self::NameAlreadyUsed => "That guild name is already used.",
            Self::CannotKickLeader => "The guild leader cannot be kicked.",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuildChange {
    Created {
        guild: GuildInfo,
    },
    Invite {
        guild_id: u32,
        leader_id: u32,
        target_id: u32,
    },
    State {
        guild: GuildInfo,
        added: Option<u32>,
        removed: Option<u32>,
    },
}

#[derive(Debug, Default)]
struct GuildRecord {
    id: u32,
    name: String,
    leader_id: u32,
    members: Vec<u32>,
}

impl GuildRecord {
    fn info(&self) -> GuildInfo {
        GuildInfo {
            id: self.id,
            name: self.name.clone(),
            leader_id: self.leader_id,
            members: self.members.clone(),
        }
    }
}

#[derive(Debug, Default)]
struct GuildBook {
    next_id: u32,
    guilds: HashMap<u32, GuildRecord>,
    membership: HashMap<u32, u32>,
    roles: HashMap<u32, u32>,
    invites: HashMap<u32, u32>,
    /// (guild, other guild) -> "ally" or "enemy".
    relations: HashMap<(u32, u32), String>,
}

/// Server-authoritative guild rules backed by the database. The in-memory book keeps
/// the online membership/index fast while load/persist methods keep guild
/// membership durable across reconnects and restarts.
#[derive(Debug, Default)]
pub struct GuildManager {
    book: Mutex<GuildBook>,
}

impl GuildManager {
    pub async fn load(pool: &PgPool) -> Result<Self, sqlx::Error> {
        let guild_rows =
            sqlx::query_as::<_, (i64, String, i64)>("SELECT guild_id, name, leader_id FROM guilds ORDER BY guild_id")
                .fetch_all(pool)
                .await?;
        let mut book = GuildBook::default();
        for (guild_id, name, leader_id) in guild_rows {
            let member_rows = sqlx::query_as::<_, (i64, i64)>(
                "SELECT character_id, role FROM guild_members WHERE guild_id = $1 ORDER BY joined_at, character_id",
            )
            .bind(guild_id)
            .fetch_all(pool)
            .await?;
            let guild_id = guild_id as u32;
            let leader_id = leader_id as u32 + CHARACTER_ID_MIN;
            let mut members = Vec::with_capacity(member_rows.len());
            for (character_id, role) in member_rows {
                let member_id = character_id as u32 + CHARACTER_ID_MIN;
                members.push(member_id);
                book.membership.insert(member_id, guild_id);
                let normalized_role = if member_id == leader_id {
                    1000
                } else {
                    role.max(200) as u32
                };
                book.roles.insert(member_id, normalized_role);
            }
            book.next_id = book.next_id.max(guild_id);
            book.guilds.insert(
                guild_id,
                GuildRecord {
                    id: guild_id,
                    name,
                    leader_id,
                    members,
                },
            );
        }
        let relation_rows =
            sqlx::query_as::<_, (i64, i64, String)>("SELECT guild_id, other_guild_id, kind FROM guild_relations")
                .fetch_all(pool)
                .await
                .unwrap_or_default();
        for (guild_id, other, kind) in relation_rows {
            book.relations.insert((guild_id as u32, other as u32), kind);
        }
        Ok(Self { book: Mutex::new(book) })
    }

    pub async fn persist_created(pool: &PgPool, guild: &GuildInfo) -> Result<(), sqlx::Error> {
        let mut transaction = pool.begin().await?;
        sqlx::query("INSERT INTO guilds (guild_id, name, leader_id) VALUES ($1, $2, $3)")
            .bind(i64::from(guild.id))
            .bind(&guild.name)
            .bind(database_id(guild.leader_id))
            .execute(&mut *transaction)
            .await?;
        for member_id in &guild.members {
            sqlx::query("INSERT INTO guild_members (guild_id, character_id, role) VALUES ($1, $2, $3)")
                .bind(i64::from(guild.id))
                .bind(database_id(*member_id))
                .bind(if *member_id == guild.leader_id {
                    1000_i64
                } else {
                    200_i64
                })
                .execute(&mut *transaction)
                .await?;
        }
        sqlx::query("INSERT INTO guild_warehouse_silver (guild_id, silver) VALUES ($1, 0) ON CONFLICT DO NOTHING")
            .bind(i64::from(guild.id))
            .execute(&mut *transaction)
            .await?;
        transaction.commit().await
    }

    pub async fn persist_member(pool: &PgPool, guild: &GuildInfo, member_id: u32) -> Result<(), sqlx::Error> {
        sqlx::query("INSERT INTO guild_members (guild_id, character_id, role) VALUES ($1, $2, $3) ON CONFLICT DO NOTHING")
            .bind(i64::from(guild.id))
            .bind(database_id(member_id))
            .bind(if member_id == guild.leader_id {
                1000_i64
            } else {
                200_i64
            })
            .execute(pool)
            .await?;
        sqlx::query("UPDATE guilds SET leader_id = $1 WHERE guild_id = $2")
            .bind(database_id(guild.leader_id))
            .bind(i64::from(guild.id))
            .execute(pool)
            .await?;
        Ok(())
    }

    pub async fn persist_removal(pool: &PgPool, guild: &GuildInfo, removed_id: u32) -> Result<(), sqlx::Error> {
        let mut transaction = pool.begin().await?;
        sqlx::query("DELETE FROM guild_members WHERE guild_id = $1 AND character_id = $2")
            .bind(i64::from(guild.id))
            .bind(database_id(removed_id))
            .execute(&mut *transaction)
            .await?;
        if guild.members.is_empty() {
            sqlx::query("DELETE FROM guilds WHERE guild_id = $1")
                .bind(i64::from(guild.id))
                .execute(&mut *transaction)
                .await?;
        } else {
            sqlx::query("UPDATE guilds SET leader_id = $1 WHERE guild_id = $2")
                .bind(database_id(guild.leader_id))
                .bind(i64::from(guild.id))
                .execute(&mut *transaction)
                .await?;
            sqlx::query(
                "UPDATE guild_members SET role = CASE WHEN character_id = $1 THEN 1000 ELSE 200 END WHERE guild_id = $2",
            )
            .bind(database_id(guild.leader_id))
            .bind(i64::from(guild.id))
            .execute(&mut *transaction)
            .await?;
        }
        transaction.commit().await
    }

    pub fn create(&self, leader_id: u32, name: impl Into<String>) -> Result<GuildChange, GuildError> {
        let name = name.into().trim().to_owned();
        if !(3..=24).contains(&name.chars().count()) {
            return Err(GuildError::InvalidName);
        }
        let mut book = self.book.lock();
        if book.membership.contains_key(&leader_id) {
            return Err(GuildError::AlreadyInGuild);
        }
        if book.guilds.values().any(|guild| guild.name.eq_ignore_ascii_case(&name)) {
            return Err(GuildError::NameAlreadyUsed);
        }
        book.next_id = book.next_id.saturating_add(1).max(1);
        let id = book.next_id;
        let guild = GuildRecord {
            id,
            name,
            leader_id,
            members: vec![leader_id],
        };
        book.membership.insert(leader_id, id);
        book.roles.insert(leader_id, 1000);
        let info = guild.info();
        book.guilds.insert(id, guild);
        Ok(GuildChange::Created { guild: info })
    }

    pub fn invite(&self, leader_id: u32, target_id: u32) -> Result<GuildChange, GuildError> {
        let mut book = self.book.lock();
        let guild_id = book.membership.get(&leader_id).copied().ok_or(GuildError::NotInGuild)?;
        let guild = book.guilds.get(&guild_id).ok_or(GuildError::NotInGuild)?;
        if guild.leader_id != leader_id {
            return Err(GuildError::NotLeader);
        }
        if guild.members.len() >= MAX_GUILD_MEMBERS {
            return Err(GuildError::GuildFull);
        }
        if book.membership.contains_key(&target_id) {
            return Err(GuildError::TargetAlreadyInGuild);
        }
        book.invites.insert(target_id, guild_id);
        Ok(GuildChange::Invite {
            guild_id,
            leader_id,
            target_id,
        })
    }

    pub fn accept_invite(&self, member_id: u32) -> Result<GuildChange, GuildError> {
        let mut book = self.book.lock();
        if book.membership.contains_key(&member_id) {
            return Err(GuildError::AlreadyInGuild);
        }
        let guild_id = book.invites.remove(&member_id).ok_or(GuildError::NoPendingInvite)?;
        let info = {
            let guild = book.guilds.get_mut(&guild_id).ok_or(GuildError::NotInGuild)?;
            if guild.members.len() >= MAX_GUILD_MEMBERS {
                return Err(GuildError::GuildFull);
            }
            guild.members.push(member_id);
            guild.info()
        };
        book.membership.insert(member_id, guild_id);
        book.roles.insert(member_id, 200);
        Ok(GuildChange::State {
            guild: info,
            added: Some(member_id),
            removed: None,
        })
    }

    pub fn leave(&self, member_id: u32) -> Result<GuildChange, GuildError> {
        let mut book = self.book.lock();
        let guild_id = book.membership.remove(&member_id).ok_or(GuildError::NotInGuild)?;
        book.roles.remove(&member_id);
        let (info, new_leader) = {
            let guild = book.guilds.get_mut(&guild_id).ok_or(GuildError::NotInGuild)?;
            guild.members.retain(|id| *id != member_id);
            if guild.members.is_empty() {
                book.guilds.remove(&guild_id);
                return Ok(GuildChange::State {
                    guild: GuildInfo {
                        id: guild_id,
                        name: String::new(),
                        leader_id: 0,
                        members: Vec::new(),
                    },
                    added: None,
                    removed: Some(member_id),
                });
            }
            let new_leader = if guild.leader_id == member_id {
                guild.leader_id = guild.members[0];
                Some(guild.leader_id)
            } else {
                None
            };
            (guild.info(), new_leader)
        };
        if let Some(new_leader) = new_leader {
            book.roles.insert(new_leader, 1000);
        }
        Ok(GuildChange::State {
            guild: info,
            added: None,
            removed: Some(member_id),
        })
    }

    pub fn kick(&self, leader_id: u32, target_id: u32) -> Result<GuildChange, GuildError> {
        let mut book = self.book.lock();
        let guild_id = book.membership.get(&leader_id).copied().ok_or(GuildError::NotInGuild)?;
        let info = {
            let guild = book.guilds.get_mut(&guild_id).ok_or(GuildError::NotInGuild)?;
            if guild.leader_id != leader_id {
                return Err(GuildError::NotLeader);
            }
            if target_id == leader_id {
                return Err(GuildError::CannotKickLeader);
            }
            if !guild.members.contains(&target_id) {
                return Err(GuildError::TargetNotFound);
            }
            guild.members.retain(|id| *id != target_id);
            guild.info()
        };
        book.membership.remove(&target_id);
        book.roles.remove(&target_id);
        Ok(GuildChange::State {
            guild: info,
            added: None,
            removed: Some(target_id),
        })
    }

    pub fn guild_for(&self, character_id: u32) -> Option<GuildInfo> {
        let book = self.book.lock();
        let guild_id = book.membership.get(&character_id)?;
        book.guilds.get(guild_id).map(GuildRecord::info)
    }

    /// Dissolves the leader's guild; its former members are returned.
    pub fn disband(&self, leader_id: u32) -> Result<GuildInfo, GuildError> {
        let mut book = self.book.lock();
        let guild_id = *book.membership.get(&leader_id).ok_or(GuildError::NotInGuild)?;
        if book.guilds.get(&guild_id).is_none_or(|guild| guild.leader_id != leader_id) {
            return Err(GuildError::NotLeader);
        }
        let guild = book.guilds.remove(&guild_id).ok_or(GuildError::NotInGuild)?;
        for member in &guild.members {
            book.membership.remove(member);
            book.roles.remove(member);
        }
        Ok(guild.info())
    }

    /// The leader gives a member another rank (990 deputy, 200 member).
    pub fn set_role(&self, leader_id: u32, member_id: u32, role: u32) -> Result<GuildInfo, GuildError> {
        let mut book = self.book.lock();
        let guild_id = *book.membership.get(&leader_id).ok_or(GuildError::NotInGuild)?;
        let guild = book.guilds.get(&guild_id).ok_or(GuildError::NotInGuild)?;
        if guild.leader_id != leader_id {
            return Err(GuildError::NotLeader);
        }
        if member_id == leader_id {
            return Err(GuildError::CannotKickLeader);
        }
        if book.membership.get(&member_id) != Some(&guild_id) {
            return Err(GuildError::TargetNotFound);
        }
        let info = guild.info();
        book.roles.insert(member_id, role);
        Ok(info)
    }

    /// The guild named `name`, if any.
    pub fn guild_by_name(&self, name: &str) -> Option<GuildInfo> {
        let book = self.book.lock();
        book.guilds
            .values()
            .find(|guild| guild.name.eq_ignore_ascii_case(name.trim()))
            .map(GuildRecord::info)
    }

    pub async fn persist_disband(pool: &PgPool, guild_id: u32) -> Result<(), sqlx::Error> {
        let mut transaction = pool.begin().await?;
        for table in ["guild_members", "guild_relations"] {
            sqlx::query(&format!("DELETE FROM {table} WHERE guild_id = $1"))
                .bind(i64::from(guild_id))
                .execute(&mut *transaction)
                .await?;
        }
        sqlx::query("DELETE FROM guild_relations WHERE other_guild_id = $1")
            .bind(i64::from(guild_id))
            .execute(&mut *transaction)
            .await?;
        sqlx::query("DELETE FROM guilds WHERE guild_id = $1")
            .bind(i64::from(guild_id))
            .execute(&mut *transaction)
            .await?;
        transaction.commit().await
    }

    pub async fn persist_role(pool: &PgPool, member_id: u32, role: u32) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE guild_members SET role = $1 WHERE character_id = $2")
            .bind(i64::from(role))
            .bind(database_id(member_id))
            .execute(pool)
            .await?;
        Ok(())
    }

    /// Adds or removes an ally ("ally") or an enemy ("enemy") guild.
    pub async fn set_relation(
        &self,
        pool: &PgPool,
        guild_id: u32,
        other_guild_id: u32,
        kind: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        {
            let mut book = self.book.lock();
            match kind {
                Some(kind) => {
                    book.relations.insert((guild_id, other_guild_id), kind.to_owned());
                },
                None => {
                    book.relations.remove(&(guild_id, other_guild_id));
                },
            }
        }
        match kind {
            Some(kind) => {
                sqlx::query(
                    "INSERT INTO guild_relations (guild_id, other_guild_id, kind) VALUES ($1, $2, $3)
                     ON CONFLICT(guild_id, other_guild_id) DO UPDATE SET kind = excluded.kind",
                )
                .bind(i64::from(guild_id))
                .bind(i64::from(other_guild_id))
                .bind(kind)
                .execute(pool)
                .await?;
            },
            None => {
                sqlx::query("DELETE FROM guild_relations WHERE guild_id = $1 AND other_guild_id = $2")
                    .bind(i64::from(guild_id))
                    .bind(i64::from(other_guild_id))
                    .execute(pool)
                    .await?;
            },
        }
        Ok(())
    }

    /// The relation `guild_id` declared toward `other_guild_id`.
    pub fn relation(&self, guild_id: u32, other_guild_id: u32) -> Option<String> {
        self.book.lock().relations.get(&(guild_id, other_guild_id)).cloned()
    }

    /// The guilds `guild_id` declared as `kind`.
    pub fn relations_of(&self, guild_id: u32, kind: &str) -> Vec<String> {
        let book = self.book.lock();
        book.relations
            .iter()
            .filter(|((owner, _), relation)| *owner == guild_id && relation.as_str() == kind)
            .filter_map(|((_, other), _)| book.guilds.get(other).map(|guild| guild.name.clone()))
            .collect()
    }

    pub fn role_for(&self, character_id: u32) -> Option<u32> {
        let book = self.book.lock();
        book.roles.get(&character_id).copied()
    }

    pub fn same_guild(&self, left_id: u32, right_id: u32) -> bool {
        if left_id == right_id {
            return true;
        }
        let book = self.book.lock();
        book.membership.get(&left_id).is_some() && book.membership.get(&left_id) == book.membership.get(&right_id)
    }

    pub fn remove_member(&self, character_id: u32) {
        let mut book = self.book.lock();
        let guild_ids: HashSet<u32> = book.guilds.keys().copied().collect();
        book.invites
            .retain(|target_id, guild_id| *target_id != character_id && guild_ids.contains(guild_id));
    }
}

fn database_id(entity_id: u32) -> i64 {
    i64::from(entity_id.saturating_sub(CHARACTER_ID_MIN))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guild_lifecycle_invites_and_transfers_leadership() {
        let manager = GuildManager::default();
        let GuildChange::Created { guild } = manager.create(10, "Knights").unwrap() else {
            panic!("expected guild creation");
        };
        assert_eq!(guild.members, vec![10]);
        assert!(matches!(manager.invite(10, 20), Ok(GuildChange::Invite { .. })));
        let GuildChange::State {
            guild, added: Some(20), ..
        } = manager.accept_invite(20).unwrap()
        else {
            panic!("expected accepted invite");
        };
        assert_eq!(guild.members, vec![10, 20]);
        assert_eq!(manager.role_for(10), Some(1000));
        assert_eq!(manager.role_for(20), Some(200));
        let GuildChange::State {
            guild,
            removed: Some(10),
            ..
        } = manager.leave(10).unwrap()
        else {
            panic!("expected leader leave");
        };
        assert_eq!(guild.leader_id, 20);
        assert_eq!(manager.role_for(20), Some(1000));
        assert!(manager.same_guild(20, 20));
        assert!(!manager.same_guild(20, 30));
    }

    #[test]
    fn guild_rejects_duplicate_names_and_non_leader_actions() {
        let manager = GuildManager::default();
        manager.create(10, "Knights").unwrap();
        assert_eq!(manager.create(20, " knights "), Err(GuildError::NameAlreadyUsed));
        assert_eq!(manager.invite(20, 30), Err(GuildError::NotInGuild));
        manager.invite(10, 20).unwrap();
        assert_eq!(manager.invite(20, 30), Err(GuildError::NotInGuild));
    }

    #[test]
    fn guild_kick_removes_membership_and_disconnect_keeps_persistent_membership() {
        let manager = GuildManager::default();
        manager.create(10, "Knights").unwrap();
        manager.invite(10, 20).unwrap();
        manager.invite(10, 30).unwrap();
        manager.accept_invite(20).unwrap();
        let GuildChange::State {
            guild,
            removed: Some(20),
            ..
        } = manager.kick(10, 20).unwrap()
        else {
            panic!("expected kick");
        };
        assert_eq!(guild.members, vec![10]);
        assert_eq!(manager.role_for(20), None);
        assert_eq!(
            manager.accept_invite(30).unwrap().clone(),
            GuildChange::State {
                guild: GuildInfo {
                    id: 1,
                    name: "Knights".into(),
                    leader_id: 10,
                    members: vec![10, 30],
                },
                added: Some(30),
                removed: None,
            }
        );
        manager.remove_member(30);
        assert!(manager.guild_for(30).is_some());
    }
}
