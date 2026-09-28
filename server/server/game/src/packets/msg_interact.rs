use serde::{Deserialize, Serialize};
use tq_network::{Actor, PacketID, PacketProcess};

use crate::combat::{
    level_advantage_percent, next_roll, pve_physical_class_percent, resolve_magic_damage, resolve_physical_hit,
    scale_skill_power, skill_power_percent, CombatStats,
};
use crate::packets::{
    ActionType, MsgAction, MsgGroundItem, MsgItemInfo, MsgPlayer, MsgSkillEffect, MsgTalk, MsgUserInfo, SkillTarget,
    TalkChannel,
};
use crate::state::GroundItem;
use crate::{ActorState, Error, State};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

static NEXT_PLAYER_ATTACK: OnceLock<Mutex<HashMap<u32, Instant>>> = OnceLock::new();
static NEXT_PLAYER_SKILL: OnceLock<Mutex<HashMap<(u32, u32), Instant>>> = OnceLock::new();

fn skill_cooldown_key(character_id: u32, skill_id: u32) -> (u32, u32) {
    (character_id, skill_id)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, num_enum::FromPrimitive, num_enum::IntoPrimitive)]
#[repr(u32)]
pub enum InteractAction {
    None = 0,
    Steal = 1,
    Attack = 2,
    Heal = 3,
    Poison = 4,
    Assassinate = 5,
    Freeze = 6,
    Unfreeze = 7,
    Court = 8,
    Marry = 9,
    Divorce = 10,
    PresentMoney = 11,
    PresentItem = 12,
    SendFlowers = 13,
    Kill = 14,
    JoinGuild = 15,
    RushAttack = 20,
    MagicAttack = 21,
    #[num_enum(default)]
    Unknown = u32::MAX,
}

#[derive(Debug, Clone, Serialize, Deserialize, PacketID)]
#[packet(id = 1022)]
pub struct MsgInteract {
    pub timestamp: u32,
    pub character_id: u32,
    pub target_id: u32,
    pub x: u16,
    pub y: u16,
    pub action: u32,
    pub value: u32,
    pub reserved: [u8; 12],
}

fn silver_item_id(amount: u32) -> u32 {
    match amount {
        0..=50 => 1_090_000,
        51..=100 => 1_090_010,
        101..=1_000 => 1_090_020,
        1_001..=5_000 => 1_091_000,
        5_001..=10_000 => 1_091_010,
        _ => 1_091_020,
    }
}

fn drop_offset(index: usize) -> (i16, i16) {
    match index {
        0 => (0, 0),
        1 => (1, 0),
        2 => (-1, 0),
        3 => (0, 1),
        _ => (0, -1),
    }
}

fn offset_coordinate(value: u16, offset: i16) -> u16 {
    (i32::from(value) + i32::from(offset)).clamp(0, i32::from(u16::MAX)) as u16
}

fn team_reward_recipients(
    state: &State,
    character: &crate::entities::Character,
) -> Vec<Arc<crate::entities::GameEntity>> {
    let origin = character.entity().location();
    let map_id = character.entity().map_id();
    state
        .team_manager()
        .members_for(character.id())
        .into_iter()
        .filter_map(|member_id| state.entity(member_id))
        .filter(|entity| {
            let Some(member) = entity.as_character() else {
                return false;
            };
            member.entity().map_id() == map_id
                && member.entity().is_alive()
                && tq_math::in_screen(origin.into(), member.entity().location().into())
        })
        .collect()
}

impl MsgInteract {
    pub fn action_kind(&self) -> InteractAction {
        InteractAction::from(self.action)
    }

    async fn degrade_used_weapons(
        &self,
        state: &State,
        actor: &Actor<ActorState>,
        character: &crate::entities::Character,
    ) -> Result<(), Error> {
        for position in [4_u8, 5_u8] {
            if let Some(item) = character.degrade_equipment_item(state.pool(), position, 1).await? {
                actor.send(MsgItemInfo::equipment_item(item)).await?;
                tracing::debug!(
                    character_id = character.id(),
                    item_uid = item.uid,
                    durability = item.durability,
                    max_durability = item.max_durability,
                    "Equipment durability decreased"
                );
            }
        }
        Ok(())
    }

    async fn reward_monster_defeat(
        &self,
        state: &State,
        actor: &Actor<ActorState>,
        character: &crate::entities::Character,
        monster: &crate::entities::Monster,
    ) -> Result<(), Error> {
        let level = monster.monster_type().level.max(1) as u32;
        let silver_drop = level + next_roll() % (level.saturating_mul(104).saturating_add(1));
        let experience = monster.monster_type().level.max(1) as u64 * 25;
        let recipients = team_reward_recipients(state, character);
        let recipient_count = recipients.len().max(1) as u64;
        let base_share = experience / recipient_count;
        let remainder = experience % recipient_count;
        let monster_type_id = u32::try_from(monster.monster_type().id).unwrap_or_default();
        let shared_reward = recipients.len() > 1;

        for (index, entity) in recipients.iter().enumerate() {
            let Some(recipient) = entity.as_character() else {
                continue;
            };
            let share = base_share + u64::from((index as u64) < remainder);
            let leveled = recipient.grant_experience(share.max(1));
            let quest_completions =
                crate::quests::record_monster_defeat(state.pool(), recipient, monster_type_id).await?;
            let quest_leveled = quest_completions.iter().filter_map(|quest| quest.leveled_to).max();
            recipient.save(state).await?;
            let owner = recipient.owner();
            owner.send(MsgUserInfo::from(recipient)).await?;
            owner
                .send(MsgTalk::from_system(
                    recipient.id(),
                    TalkChannel::System,
                    if shared_reward {
                        format!("Team reward: {share} experience.")
                    } else {
                        format!("Experience gained: {share}.")
                    },
                ))
                .await?;
            if leveled.or(quest_leveled).is_some() {
                owner
                    .send(MsgAction::new(
                        recipient.id(),
                        recipient.entity().map_id(),
                        0,
                        0,
                        ActionType::LevelUp,
                    ))
                    .await?;
            }
            for quest in quest_completions {
                owner
                    .send(MsgTalk::from_system(
                        recipient.id(),
                        TalkChannel::System,
                        format!(
                            "Quest complete: {}. Reward: {} silver and {} experience.",
                            quest.definition.name, quest.reward_silver, quest.reward_experience
                        ),
                    ))
                    .await?;
            }
        }

        for entity in &recipients {
            let Some(recipient) = entity.as_character() else {
                continue;
            };
            recipient
                .owner()
                .send(MsgTalk::from_system(
                    recipient.id(),
                    TalkChannel::System,
                    format!("Loot: {silver_drop} silver dropped."),
                ))
                .await?;
        }
        let location = monster.entity().location();
        let monster_level = monster.monster_type().level.max(0) as u16;
        let mut drops = vec![(silver_item_id(silver_drop), silver_drop)];
        drops.extend(
            state
                .drop_catalog()
                .roll(monster_type_id, monster_level)
                .into_iter()
                .map(|item_id| (item_id, 0)),
        );
        for (index, item_id) in drops.into_iter().enumerate() {
            let (offset_x, offset_y) = drop_offset(index);
            let item_uid = state.next_ground_item_uid();
            let x = offset_coordinate(location.x, offset_x);
            let y = offset_coordinate(location.y, offset_y);
            state.insert_ground_item(GroundItem {
                uid: item_uid,
                item_id: item_id.0,
                map_id: character.entity().map_id(),
                x,
                y,
                owner_id: character.id(),
                silver: item_id.1,
            });
            let item_drop = MsgGroundItem::new(item_uid, item_id.0, x, y);
            actor.send(item_drop.clone()).await?;
            actor.screen().send_message(item_drop).await?;
        }
        let mut kill = self.clone();
        kill.target_id = monster.id();
        kill.action = InteractAction::Kill as u32;
        kill.value = silver_drop;
        actor.send(kill.clone()).await?;
        actor.screen().send_message(kill).await?;
        Ok(())
    }

    async fn process_magic_attack(
        &self,
        state: &State,
        actor: &Actor<ActorState>,
        character: &crate::entities::Character,
    ) -> Result<(), Error> {
        let Some(skill) = state.skill_catalog().get(self.value).cloned() else {
            tracing::debug!(skill_id = self.value, "Rejected unknown or invalid skill");
            return Ok(());
        };
        if !character.entity().is_alive() {
            tracing::debug!(skill_id = skill.id, "Rejected skill from dead character");
            return Ok(());
        }
        if !character.has_skill(skill.id) {
            actor
                .send(MsgTalk::from_system(
                    character.id(),
                    TalkChannel::System,
                    format!("{} is not learned yet.", skill.name),
                ))
                .await?;
            tracing::debug!(
                character_id = character.id(),
                skill_id = skill.id,
                "Rejected unlearned skill"
            );
            return Ok(());
        }

        if matches!(
            skill.status,
            Some(
                crate::skills::SkillStatus::Cyclone
                    | crate::skills::SkillStatus::Superman
                    | crate::skills::SkillStatus::Stigma
                    | crate::skills::SkillStatus::Shield,
            )
        ) {
            let now = Instant::now();
            let next_skill = NEXT_PLAYER_SKILL.get_or_init(|| Mutex::new(HashMap::new()));
            let can_cast = {
                let skills = next_skill
                    .lock()
                    .map_err(|_| Error::Other("player skill cooldown lock poisoned".to_owned()))?;
                skills
                    .get(&skill_cooldown_key(character.id(), skill.id))
                    .is_none_or(|next| *next <= now)
            };
            if !can_cast {
                return Ok(());
            }
            let Some(mana) = character.entity().spend_mana(skill.mana_cost) else {
                actor
                    .send(MsgTalk::from_system(
                        character.id(),
                        TalkChannel::System,
                        format!("{} requires {} MP.", skill.name, skill.mana_cost),
                    ))
                    .await?;
                return Ok(());
            };
            if let Ok(mut skills) = next_skill.lock() {
                skills.insert(skill_cooldown_key(character.id(), skill.id), now + skill.cooldown);
            }
            let effect = match skill.status {
                Some(crate::skills::SkillStatus::Cyclone) => crate::entities::StatusEffect::Cyclone,
                Some(crate::skills::SkillStatus::Superman) => crate::entities::StatusEffect::Superman,
                Some(crate::skills::SkillStatus::Stigma) => crate::entities::StatusEffect::Stigma,
                Some(crate::skills::SkillStatus::Shield) => crate::entities::StatusEffect::Shield,
                _ => unreachable!("self buff status was validated above"),
            };
            character.apply_status(effect, skill.status_duration);
            actor.send(MsgUserInfo::from(character)).await?;
            actor.send(MsgPlayer::from(character)).await?;
            actor
                .send(MsgTalk::from_system(
                    character.id(),
                    TalkChannel::System,
                    format!(
                        "{} activated for {} seconds.",
                        skill.name,
                        skill.status_duration.as_secs()
                    ),
                ))
                .await?;
            if let Some(progress) = character.practice_skill(state.pool(), skill.id, 1).await? {
                actor.send(super::MsgSkill::from(progress)).await?;
            }
            tracing::debug!(
                character_id = character.id(),
                skill_id = skill.id,
                mana_remaining = mana.current,
                "Applied self-buff skill"
            );
            return Ok(());
        }

        let map = state.try_map(character.entity().map_id())?;
        let Some(primary) = map.monster_entity(self.target_id).cloned().or_else(|| {
            state
                .entities()
                .into_iter()
                .find(|entity| entity.id() == self.target_id)
        }) else {
            tracing::debug!(target_id = self.target_id, "Skill target was not found");
            return Ok(());
        };
        let character_location = character.entity().location();
        let primary_entity = primary.basic();
        let primary_location = primary_entity.location();
        let distance = tq_math::get_distance(character_location.into(), primary_location.into());
        let target_character = primary.as_character();
        let pvp_allowed = target_character.is_none_or(|target| character.can_attack_character(target));
        let valid_target = primary_entity.map_id() == character.entity().map_id()
            && tq_math::in_screen(character_location.into(), primary_location.into())
            && distance <= skill.range
            && !primary_entity.is_dead()
            && !target_character.is_some_and(|target| target.id() == character.id())
            && pvp_allowed;
        if !valid_target {
            tracing::debug!(
                target_id = self.target_id,
                skill_id = skill.id,
                attacker_map_id = character.entity().map_id(),
                target_map_id = primary_entity.map_id(),
                attacker_x = character_location.x,
                attacker_y = character_location.y,
                target_x = primary_location.x,
                target_y = primary_location.y,
                distance,
                range = skill.range,
                target_alive = !primary_entity.is_dead(),
                pvp_allowed,
                "Rejected invalid skill target"
            );
            return Ok(());
        }

        let primary_is_monster = primary.as_monster().is_some();
        let mut targets = vec![primary.clone()];
        if skill.radius > 0.0 && primary_is_monster {
            let mut area_targets = map
                .monster_entities()
                .into_iter()
                .filter(|candidate| candidate.id() != primary.id())
                .filter(|candidate| {
                    let entity = candidate.basic();
                    entity.map_id() == character.entity().map_id()
                        && !entity.is_dead()
                        && tq_math::in_screen(character_location.into(), entity.location().into())
                        && tq_math::get_distance(primary_location.into(), entity.location().into()) <= skill.radius
                })
                .collect::<Vec<_>>();
            area_targets.sort_by_key(|target| target.id());
            targets.extend(area_targets);
        }

        let now = Instant::now();
        let next_skill = NEXT_PLAYER_SKILL.get_or_init(|| Mutex::new(HashMap::new()));
        let can_cast = {
            let skills = next_skill
                .lock()
                .map_err(|_| Error::Other("player skill cooldown lock poisoned".to_owned()))?;
            let can_cast = skills
                .get(&skill_cooldown_key(character.id(), skill.id))
                .is_none_or(|next| *next <= now);
            can_cast
        };
        if !can_cast {
            tracing::trace!(character_id = character.id(), "Rejected skill during cooldown");
            return Ok(());
        }

        let Some(mana) = character.entity().spend_mana(skill.mana_cost) else {
            actor
                .send(MsgTalk::from_system(
                    character.id(),
                    TalkChannel::System,
                    format!("{} requires {} MP.", skill.name, skill.mana_cost),
                ))
                .await?;
            return Ok(());
        };
        if let Ok(mut skills) = next_skill.lock() {
            skills.insert(skill_cooldown_key(character.id(), skill.id), now + skill.cooldown);
        }
        actor.send(MsgUserInfo::from(character)).await?;

        let attacker = character.combat_stats_with_catalog(Some(state.shop_catalog()));
        let progress = character
            .skill_progress(skill.id)
            .unwrap_or(crate::entities::SkillProgress {
                skill_id: skill.id,
                level: 1,
                experience: 0,
                proficiency: 0,
            });
        let skill_power = skill_power_percent(skill.power_percent, progress.level, progress.proficiency);
        let magic_attack_bonus = character.magic_attack_bonus_with_catalog(Some(state.shop_catalog()));
        let attacker_stats = CombatStats {
            min_attack: attacker
                .min_attack
                .saturating_add(u32::from(character.spirit()).saturating_mul(3))
                .saturating_add(magic_attack_bonus),
            max_attack: attacker
                .max_attack
                .saturating_add(u32::from(character.spirit()).saturating_mul(4))
                .saturating_add(magic_attack_bonus),
            ..attacker
        };
        let attacker_stats = scale_skill_power(attacker_stats, skill_power);
        let mut effects = Vec::with_capacity(targets.len());
        let mut defeated = Vec::new();
        let mut player_owners = Vec::new();
        for target in targets {
            let target_entity = target.basic();
            let (target_stats, magic_defense, fire_resistance, target_owner) =
                if let Some(monster) = target.as_monster() {
                    (monster.combat_stats(), 0, 0, None)
                } else if let Some(target_character) = target.as_character() {
                    let stats = target_character.combat_stats_with_catalog(Some(state.shop_catalog()));
                    (
                        stats,
                        target_character.magic_defense_with_catalog(Some(state.shop_catalog())),
                        target_character.fire_resistance_percent(),
                        Some(target_character.owner()),
                    )
                } else {
                    continue;
                };
            let health_before = target_entity.hp();
            let target_attacker_stats = if target.as_monster().is_some() {
                scale_skill_power(
                    attacker_stats,
                    level_advantage_percent(character.entity().level(), target_entity.level()),
                )
            } else {
                attacker_stats
            };
            let magic_attack = target_attacker_stats
                .min_attack
                .saturating_add(target_attacker_stats.attack_flat);
            let outcome = resolve_magic_damage(
                magic_attack,
                magic_defense,
                0,
                target_stats.damage_reduction_percent,
                u32::from(health_before.current),
                target_stats.max_health,
                fire_resistance,
            );
            let health_after = target_entity.apply_damage(outcome.damage);
            if !outcome.defeated {
                if let Some(status) = skill.status {
                    let effect = match status {
                        crate::skills::SkillStatus::Burning => crate::entities::StatusEffect::Burning,
                        crate::skills::SkillStatus::Stunned => crate::entities::StatusEffect::Stunned,
                        crate::skills::SkillStatus::Cyclone => crate::entities::StatusEffect::Cyclone,
                        crate::skills::SkillStatus::Superman => crate::entities::StatusEffect::Superman,
                        crate::skills::SkillStatus::Stigma => crate::entities::StatusEffect::Stigma,
                        crate::skills::SkillStatus::Shield => crate::entities::StatusEffect::Shield,
                    };
                    target.apply_status(effect, skill.status_duration);
                }
            }
            effects.push(SkillTarget::hit(target.id(), outcome.damage));
            if outcome.defeated {
                defeated.push(target.id());
            }
            let player_update = if let Some(monster) = target.as_monster() {
                MsgPlayer::from_monster(monster)
            } else if let Some(target_character) = target.as_character() {
                MsgPlayer::from(target_character)
            } else {
                continue;
            };
            actor.send(player_update.clone()).await?;
            actor.screen().send_message(player_update.clone()).await?;
            if let Some(owner) = target_owner {
                player_owners.push(owner.clone());
                owner.send(player_update).await?;
                if let Some(target_character) = target.as_character() {
                    owner.send(MsgUserInfo::from(target_character)).await?;
                }
            }
            tracing::debug!(
                character_id = character.id(),
                target_id = target.id(),
                skill_id = skill.id,
                mana_remaining = mana.current,
                damage = outcome.damage,
                fire_resistance,
                defeated = outcome.defeated,
                hp_before = health_before.current,
                hp_max = health_before.max,
                calculated_remaining_hp = outcome.remaining_health,
                hp_after = health_after.current,
                dead_flag = target_entity.is_dead(),
                "Magic attack resolved"
            );
        }

        let mut effect = MsgSkillEffect::new(character.id(), skill.id as u16, effects);
        effect.target_x = self.x;
        effect.target_y = self.y;
        actor.send(effect.clone()).await?;
        actor.screen().send_message(effect.clone()).await?;
        for owner in player_owners {
            owner.send(effect.clone()).await?;
        }
        self.degrade_used_weapons(state, actor, character).await?;
        if let Some(progress) = character.practice_skill(state.pool(), skill.id, 1).await? {
            actor.send(super::MsgSkill::from(progress)).await?;
        }
        for target_id in defeated {
            if let Some(monster) = map.monster_entity(target_id).and_then(|entity| entity.as_monster()) {
                self.reward_monster_defeat(state, actor, character, monster).await?;
            }
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl PacketProcess for MsgInteract {
    type ActorState = ActorState;
    type Error = Error;
    type State = State;

    async fn process(&self, state: &Self::State, actor: &Actor<Self::ActorState>) -> Result<(), Self::Error> {
        let entity = actor.entity();
        let character = entity.as_character().ok_or(Error::CharacterNotFound)?;
        if self.character_id != character.id() {
            tracing::warn!(
                packet_character_id = self.character_id,
                actor_character_id = character.id(),
                "Rejected interaction from another character"
            );
            return Ok(());
        }

        if character.has_status(crate::entities::StatusEffect::Stunned) {
            actor
                .send(MsgTalk::from_system(
                    character.id(),
                    TalkChannel::System,
                    "You are stunned and cannot attack or use skills.",
                ))
                .await?;
            return Ok(());
        }

        if self.action_kind() == InteractAction::MagicAttack {
            return self.process_magic_attack(state, actor, character).await;
        }

        if self.action_kind() != InteractAction::Attack {
            actor.send(self.clone()).await?;
            actor.screen().send_message(self.clone()).await?;
            return Ok(());
        }

        let map = state.try_map(character.entity().map_id())?;
        let Some(target) = map.monster_entity(self.target_id).cloned().or_else(|| {
            state
                .entities()
                .into_iter()
                .find(|entity| entity.id() == self.target_id)
        }) else {
            tracing::debug!(target_id = self.target_id, "Attack target was not found");
            return Ok(());
        };
        let target_entity = target.basic();
        if target_entity.map_id() != character.entity().map_id()
            || !tq_math::in_screen(character.entity().location().into(), target_entity.location().into())
            || tq_math::get_distance(character.entity().location().into(), target_entity.location().into())
                > f32::from(character.attack_range_with_catalog(Some(state.shop_catalog())))
            || target_entity.is_dead()
            || target
                .as_character()
                .is_some_and(|target| target.id() == character.id())
            || target
                .as_character()
                .is_some_and(|target| !character.can_attack_character(target))
        {
            tracing::debug!(target_id = self.target_id, "Rejected invalid player attack target");
            return Ok(());
        }

        let now = Instant::now();
        let next_attack = NEXT_PLAYER_ATTACK.get_or_init(|| Mutex::new(HashMap::new()));
        let attack_cooldown = Duration::from_millis(u64::from(
            character.attack_speed_with_catalog(Some(state.shop_catalog())),
        ));
        let can_attack = {
            let mut attacks = next_attack
                .lock()
                .map_err(|_| Error::Other("player attack cooldown lock poisoned".to_owned()))?;
            let can_attack = attacks.get(&character.id()).is_none_or(|next| *next <= now);
            if can_attack {
                attacks.insert(character.id(), now + attack_cooldown);
            }
            can_attack
        };
        if !can_attack {
            tracing::trace!(character_id = character.id(), "Rejected attack during cooldown");
            return Ok(());
        }

        let attacker_stats = character.combat_stats_with_catalog(Some(state.shop_catalog()));
        let weapon_bonus = character.weapon_proficiency_bonus_percent();
        let attacker_stats = scale_skill_power(attacker_stats, 100 + weapon_bonus);
        let target_stats = if let Some(monster) = target.as_monster() {
            monster.combat_stats()
        } else if let Some(target_character) = target.as_character() {
            target_character.combat_stats_with_catalog(Some(state.shop_catalog()))
        } else {
            return Ok(());
        };
        let attacker_stats = if target.as_monster().is_some() {
            let level_bonus = level_advantage_percent(character.entity().level(), target_entity.level());
            let class_bonus = pve_physical_class_percent(character.current_class());
            let superman_bonus = if character.has_status(crate::entities::StatusEffect::Superman) {
                crate::combat::superman_damage_percent(true)
            } else {
                100
            };
            scale_skill_power(
                scale_skill_power(scale_skill_power(attacker_stats, level_bonus), class_bonus),
                superman_bonus,
            )
        } else if character.has_status(crate::entities::StatusEffect::Superman) {
            scale_skill_power(attacker_stats, crate::combat::superman_damage_percent(false))
        } else {
            attacker_stats
        };
        // Derive the roll on the authoritative server. The old implementation
        // used the client timestamp directly, which made damage repeatable and
        // often pinned it to the minimum value.
        let outcome = resolve_physical_hit(
            attacker_stats,
            target_stats,
            u32::from(target_entity.hp().current()),
            next_roll(),
        );
        target_entity.apply_damage(outcome.damage);
        tracing::debug!(
            character_id = character.id(),
            target_id = self.target_id,
            damage = outcome.damage,
            remaining_health = outcome.remaining_health,
            defeated = outcome.defeated,
            "Physical attack resolved"
        );
        let mut hit = self.clone();
        hit.value = outcome.damage;
        actor.send(hit.clone()).await?;
        actor.screen().send_message(hit).await?;
        let (target_update, target_owner) = if let Some(monster) = target.as_monster() {
            (MsgPlayer::from_monster(monster), None)
        } else if let Some(target_character) = target.as_character() {
            (MsgPlayer::from(target_character), Some(target_character.owner()))
        } else {
            return Ok(());
        };
        actor.send(target_update.clone()).await?;
        actor.screen().send_message(target_update.clone()).await?;
        if let Some(owner) = target_owner {
            owner.send(target_update).await?;
            if let Some(target_character) = target.as_character() {
                owner.send(MsgUserInfo::from(target_character)).await?;
            }
        }
        self.degrade_used_weapons(state, actor, character).await?;
        if let Some(weapon_type) = character
            .equipment_item(4)
            .or_else(|| character.equipment_item(5))
            .map(|item| item.item_type)
        {
            character
                .practice_weapon_proficiency(state.pool(), weapon_type, 1)
                .await?;
        }
        if outcome.defeated {
            let Some(monster) = target.as_monster() else {
                return Ok(());
            };
            self.reward_monster_defeat(state, actor, character, monster).await?;
            tracing::info!(target_id = self.target_id, "Monster defeated");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skills::BASIC_FIRE_ID;
    use tq_network::PacketEncode;

    #[test]
    fn skill_catalog_exposes_single_and_area_skills() {
        use crate::skills::{BASIC_FIRE_ID, FLAME_BURST_ID};
        let catalog = crate::skills::SkillCatalog::default();
        let basic = catalog.get(BASIC_FIRE_ID).expect("basic fire definition");
        assert_eq!(basic.mana_cost, 20);
        assert_eq!(basic.radius, 0.0);

        let burst = catalog.get(FLAME_BURST_ID).expect("flame burst definition");
        assert_eq!(burst.mana_cost, 35);
        assert!(burst.radius > 0.0);
        assert!(catalog.get(99_999).is_none());
    }

    #[test]
    fn skill_cooldowns_are_scoped_by_character_and_skill() {
        assert_ne!(skill_cooldown_key(7, 1_000), skill_cooldown_key(7, 1_001));
        assert_ne!(skill_cooldown_key(7, 1_000), skill_cooldown_key(8, 1_000));
        assert_eq!(skill_cooldown_key(7, 1_000), (7, 1_000));
    }

    #[test]
    fn encodes_the_5065_interaction_body() {
        let packet = MsgInteract {
            timestamp: 123,
            character_id: 42,
            target_id: 9001,
            x: 430,
            y: 378,
            action: InteractAction::Attack as u32,
            value: 0,
            reserved: [0; 12],
        };
        let (packet_id, bytes) = packet.encode().expect("interaction packet encodes");
        assert_eq!(packet_id, 1022);
        assert_eq!(bytes.len(), 36);
    }

    #[test]
    fn encodes_a_single_target_skill_effect_body() {
        use tq_network::PacketEncode;
        let effect = MsgSkillEffect::single_target(1_000_001, BASIC_FIRE_ID as u16, 400_001, 37);
        let (packet_id, bytes) = effect.encode().expect("skill effect encodes");
        assert_eq!(packet_id, 1105);
        assert_eq!(bytes.len(), 88);
        assert_eq!(&bytes[0..4], &1_000_001_u32.to_le_bytes());
        assert_eq!(&bytes[8..10], &1_000_u16.to_le_bytes());
        assert_eq!(&bytes[12..16], &1_u32.to_le_bytes());
        assert_eq!(&bytes[16..20], &400_001_u32.to_le_bytes());
        assert_eq!(&bytes[20..24], &37_u32.to_le_bytes());
    }

    #[test]
    fn encodes_multiple_skill_effect_targets() {
        use crate::packets::SkillTarget;
        use tq_network::PacketEncode;
        let effect = MsgSkillEffect::new(
            1_000_001,
            BASIC_FIRE_ID as u16,
            vec![SkillTarget::hit(400_001, 37), SkillTarget::hit(400_002, 41)],
        );
        let (packet_id, bytes) = effect.encode().expect("skill effect encodes");
        assert_eq!(packet_id, 1105);
        assert_eq!(bytes.len(), 120);
        assert_eq!(&bytes[12..16], &2_u32.to_le_bytes());
        assert_eq!(&bytes[48..52], &400_002_u32.to_le_bytes());
        assert_eq!(&bytes[52..56], &41_u32.to_le_bytes());
    }
}
