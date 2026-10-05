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

    /// Experience for a hit (Redux `GainExperience` after each attack),
    /// with the level-up effect and the leader's virtue.
    async fn grant_hit_experience(
        &self,
        state: &State,
        actor: &Actor<ActorState>,
        character: &crate::entities::Character,
        experience: u64,
    ) -> Result<(), Error> {
        if experience == 0 {
            return Ok(());
        }
        let experience = crate::store::with_vip_bonus(experience, state.vip().is_vip(character.id()));
        let leveled = character.grant_experience(experience);
        actor.send(MsgUserInfo::from(character)).await?;
        if let Some(level) = leveled {
            character.save(state).await?;
            let level_up = MsgAction::new(character.id(), character.entity().map_id(), 0, 0, ActionType::LevelUp);
            actor.send(level_up.clone()).await?;
            actor.screen().send_message(level_up).await?;
            crate::leveling::after_level_up(state, character, level).await?;
        }
        Ok(())
    }

    async fn degrade_used_weapons(
        &self,
        state: &State,
        actor: &Actor<ActorState>,
        character: &crate::entities::Character,
    ) -> Result<(), Error> {
        // Kylin gems: a 50% Kylin bonus spares a third of the wear.
        let kylin = character.gem_bonus(crate::entities::GemFamily::Kylin);
        for position in [4_u8, 5_u8] {
            if kylin > 0 && next_roll() % (100 + kylin) < kylin {
                continue;
            }
            if let Some(item) = character.degrade_equipment_item(state.pool(), position, 1).await? {
                // A quiver that runs out is gone (Redux deletes it).
                if item.durability == 0 && crate::shop::ShopCatalog::is_arrow(item.item_type) {
                    if let Some(spent) = character.destroy_equipment_item(state.pool(), position).await? {
                        actor.send(MsgItemInfo::remove_equipment_item(spent, position)).await?;
                        let update = MsgPlayer::from(character);
                        actor.send(update.clone()).await?;
                        actor.screen().send_message(update).await?;
                    }
                    continue;
                }
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
        _actor: &Actor<ActorState>,
        character: &crate::entities::Character,
        monster: &crate::entities::Monster,
    ) -> Result<(), Error> {
        reward_monster_defeat(state, character, monster, self.character_id).await
    }

    /// A hit on a training ground dummy: the damage is shown, the player
    /// earns experience and weapon or skill proficiency, and nothing is
    /// consumed (no durability, life or mana; the dummy keeps its life).
    async fn train_on_dummy(
        &self,
        state: &State,
        actor: &Actor<ActorState>,
        character: &crate::entities::Character,
        npc: &crate::entities::Npc,
        skill: Option<&crate::skills::SkillDefinition>,
    ) -> Result<(), Error> {
        let player = character.entity();
        let level = player.level();
        if !player.is_alive() {
            return Ok(());
        }
        let war_structure = crate::guild_war::is_war_structure(npc.id());
        if let Some(reason) = war_structure.then(|| crate::guild_war::refuses_hit(npc.id())).flatten() {
            actor
                .send(MsgTalk::from_system(character.id(), TalkChannel::TopLeft, reason))
                .await?;
            return Ok(());
        }
        if !war_structure && !crate::training::can_train(level, npc) {
            // The reference server ignores the hit silently, which looks like
            // a frozen attack; say why instead.
            actor
                .send(MsgTalk::from_system(
                    character.id(),
                    TalkChannel::TopLeft,
                    format!("You must be level {} to train on {}.", npc.level(), npc.entity().name()),
                ))
                .await?;
            return Ok(());
        }
        let location = player.location();
        let target = npc.entity().location();
        let distance = location.x.abs_diff(target.x).max(location.y.abs_diff(target.y));
        let reach = match skill {
            Some(skill) => skill.range,
            None => f32::from(character.attack_range_with_catalog(Some(state.shop_catalog()))),
        };
        if npc.entity().map_id() != player.map_id() || f32::from(distance) > reach.max(1.0) {
            return Ok(());
        }

        // The same cadence as real combat, keyed like real attacks/skills.
        let now = Instant::now();
        let (cooldowns, key, cooldown) = match skill {
            Some(skill) => (
                NEXT_PLAYER_SKILL.get_or_init(|| Mutex::new(HashMap::new())),
                skill_cooldown_key(character.id(), skill.id),
                skill.cooldown,
            ),
            None => (
                NEXT_PLAYER_SKILL.get_or_init(|| Mutex::new(HashMap::new())),
                skill_cooldown_key(character.id(), 0),
                Duration::from_millis(u64::from(
                    character.attack_speed_with_catalog(Some(state.shop_catalog())),
                )),
            ),
        };
        {
            let mut cooldowns = cooldowns
                .lock()
                .map_err(|_| Error::Other("training cooldown lock poisoned".to_owned()))?;
            if cooldowns.get(&key).is_some_and(|next| *next > now) {
                return Ok(());
            }
            cooldowns.insert(key, now + cooldown);
        }

        let attacker = character.combat_stats_with_catalog(Some(state.shop_catalog()));
        let attacker = match skill {
            Some(skill) => {
                let progress = character.skill_progress(skill.id);
                let power = skill_power_percent(
                    skill.power_percent,
                    progress.map_or(1, |progress| progress.level),
                    progress.map_or(0, |progress| progress.proficiency),
                );
                let bonus = character.magic_attack_bonus_with_catalog(Some(state.shop_catalog()));
                let spirit = u32::from(character.spirit());
                scale_skill_power(
                    CombatStats {
                        min_attack: attacker.min_attack.saturating_add(spirit * 3).saturating_add(bonus),
                        max_attack: attacker.max_attack.saturating_add(spirit * 4).saturating_add(bonus),
                        ..attacker
                    },
                    power,
                )
            },
            None => scale_skill_power(attacker, 100 + character.weapon_proficiency_bonus_percent()),
        };
        let damage = resolve_physical_hit(
            attacker,
            crate::training::dummy_stats(npc, skill.is_some()),
            npc.life(),
            next_roll(),
        )
        .damage;

        match skill {
            Some(skill) => {
                let mut effect = MsgSkillEffect::new(
                    character.id(),
                    skill.id as u16,
                    vec![super::SkillTarget::hit(npc.id(), damage)],
                );
                effect.target_x = target.x;
                effect.target_y = target.y;
                actor.send(effect.clone()).await?;
                actor.screen().send_message(effect).await?;
            },
            None => {
                let mut hit = self.clone();
                hit.value = damage;
                actor.send(hit.clone()).await?;
                actor.screen().send_message(hit).await?;
            },
        }
        if war_structure {
            return crate::guild_war::hit(state, actor, character, npc, damage).await;
        }

        let experience = crate::training::experience(level, npc.level(), npc.life(), damage);
        self.grant_hit_experience(state, actor, character, experience).await?;
        let practice = u32::try_from(experience).unwrap_or(u32::MAX).max(1);
        match skill {
            Some(skill) => {
                if let Some(progress) = character.practice_skill(state.pool(), skill.id, practice).await? {
                    actor.send(super::MsgSkill::from(progress)).await?;
                }
            },
            None => {
                if let Some(weapon_type) = character
                    .equipment_item(4)
                    .or_else(|| character.equipment_item(5))
                    .map(|item| item.item_type)
                {
                    let practiced = character
                        .practice_weapon_proficiency(state.pool(), weapon_type, practice)
                        .await?;
                    if practiced.is_some() {
                        let weapon_type = weapon_type / 1_000;
                        if let Some(packet) = super::MsgWeaponSkill::for_all(character.weapon_proficiencies())
                            .into_iter()
                            .find(|packet| packet.weapon_type == weapon_type)
                        {
                            actor.send(packet).await?;
                        }
                    }
                }
            },
        }
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
        if skill.id == crate::lucky::BLESS {
            if let Some(line) = crate::lucky::bless(state, character).await? {
                actor
                    .send(MsgTalk::from_system(character.id(), TalkChannel::TopLeft, line))
                    .await?;
            }
            return Ok(());
        }
        if skill.xp && !character.xp_ready() {
            actor
                .send(MsgTalk::from_system(
                    character.id(),
                    TalkChannel::TopLeft,
                    format!("{} needs a full XP circle.", skill.name),
                ))
                .await?;
            return Ok(());
        }
        if skill.stamina_cost > 0 && character.stamina() < skill.stamina_cost {
            actor
                .send(MsgTalk::from_system(
                    character.id(),
                    TalkChannel::TopLeft,
                    format!("{} needs {} stamina.", skill.name, skill.stamina_cost),
                ))
                .await?;
            return Ok(());
        }

        if skill.status.is_some_and(|status| status.is_self_buff()) {
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
            super::spend_cast_costs(actor, character, &skill).await?;
            let Some(effect) = skill.status.map(|status| status.effect()) else {
                return Ok(());
            };
            if skill.xp && !character.in_xp_skill() {
                character.reset_ko_count();
            }
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

        // Support skills aim at another player in range, or the caster.
        let ally = (self.target_id != character.id())
            .then(|| {
                state
                    .entities()
                    .into_iter()
                    .find(|entity| entity.id() == self.target_id && entity.as_character().is_some())
            })
            .flatten()
            .filter(|entity| {
                let target = entity.basic();
                let here = character.entity().location();
                let there = target.location();
                target.map_id() == character.entity().map_id()
                    && f32::from(here.x.abs_diff(there.x).max(here.y.abs_diff(there.y))) <= skill.range.max(1.0)
            });
        if skill.revive {
            let Some(dead) = ally.as_ref().and_then(|entity| entity.as_character()).filter(|target| target.entity().is_dead())
            else {
                actor
                    .send(MsgTalk::from_system(
                        character.id(),
                        TalkChannel::TopLeft,
                        format!("{} needs a dead player as its target.", skill.name),
                    ))
                    .await?;
                return Ok(());
            };
            if !self.begin_cast(actor, character, &skill).await? {
                return Ok(());
            }
            let location = dead.entity().location();
            let mut effect = MsgSkillEffect::new(character.id(), skill.id as u16, vec![super::SkillTarget::hit(dead.id(), 0)]);
            effect.target_x = location.x;
            effect.target_y = location.y;
            actor.send(effect.clone()).await?;
            actor.screen().send_message(effect).await?;
            crate::death::revive_here(dead).await?;
            return Ok(());
        }
        if skill.needs_shield
            && !character
                .equipment_item(5)
                .is_some_and(|item| item.item_type / 1_000 == 900)
        {
            actor
                .send(MsgTalk::from_system(
                    character.id(),
                    TalkChannel::TopLeft,
                    format!("{} needs a shield.", skill.name),
                ))
                .await?;
            return Ok(());
        }
        if skill.dispatch_xp || skill.mana_heal > 0 {
            if !self.begin_cast(actor, character, &skill).await? {
                return Ok(());
            }
            let mut touched = vec![super::SkillTarget::hit(character.id(), skill.mana_heal)];
            if skill.mana_heal > 0 {
                character
                    .entity()
                    .restore_mana(u16::try_from(skill.mana_heal).unwrap_or(u16::MAX));
                actor.send(MsgUserInfo::from(character)).await?;
            }
            if skill.dispatch_xp {
                let here = character.entity().location();
                for member_id in state.team_manager().members_for(character.id()) {
                    if member_id == character.id() {
                        continue;
                    }
                    let Some(member) = state.entities().into_iter().find(|entity| entity.id() == member_id) else {
                        continue;
                    };
                    let Some(member) = member.as_character() else {
                        continue;
                    };
                    let there = member.entity().location();
                    if member.entity().map_id() == character.entity().map_id()
                        && here.x.abs_diff(there.x).max(here.y.abs_diff(there.y)) <= 18
                    {
                        member.fill_xp();
                        touched.push(super::SkillTarget::hit(member.id(), 0));
                    }
                }
            }
            let location = character.entity().location();
            let mut effect = MsgSkillEffect::new(character.id(), skill.id as u16, touched);
            effect.target_x = location.x;
            effect.target_y = location.y;
            actor.send(effect.clone()).await?;
            actor.screen().send_message(effect).await?;
            if let Some(progress) = character.practice_skill(state.pool(), skill.id, 1).await? {
                actor.send(super::MsgSkill::from(progress)).await?;
            }
            return Ok(());
        }
        if !skill.summon.is_empty() {
            if !self.begin_cast(actor, character, &skill).await? {
                return Ok(());
            }
            let level = character.skill_progress(skill.id).map_or(1, |progress| progress.level);
            let index = usize::from(level.saturating_sub(1)).min(skill.summon.len() - 1);
            let duration = skill.status_duration + Duration::from_secs(5 * u64::from(level.saturating_sub(1)));
            crate::pets::summon(state, character, skill.summon[index], duration).await?;
            if let Some(progress) = character.practice_skill(state.pool(), skill.id, 1).await? {
                actor.send(super::MsgSkill::from(progress)).await?;
            }
            return Ok(());
        }
        if skill.heal > 0 {
            let healed = ally
                .as_ref()
                .and_then(|entity| entity.as_character())
                .filter(|target| target.entity().is_alive())
                .unwrap_or(character);
            if !self.begin_cast(actor, character, &skill).await? {
                return Ok(());
            }
            let health = healed.entity().heal_health(u16::try_from(skill.heal).unwrap_or(u16::MAX));
            let mut effect = MsgSkillEffect::new(
                character.id(),
                skill.id as u16,
                vec![super::SkillTarget::hit(healed.id(), skill.heal)],
            );
            let location = healed.entity().location();
            effect.target_x = location.x;
            effect.target_y = location.y;
            actor.send(effect.clone()).await?;
            actor.screen().send_message(effect).await?;
            actor.send(MsgUserInfo::from(character)).await?;
            if healed.id() != character.id() {
                healed.owner().send(MsgUserInfo::from(healed)).await?;
            }
            if let Some(progress) = character.practice_skill(state.pool(), skill.id, 1).await? {
                actor.send(super::MsgSkill::from(progress)).await?;
            }
            tracing::debug!(character_id = character.id(), skill_id = skill.id, life = health.current, "Healed");
            return Ok(());
        }

        let map = state.try_map(character.entity().map_id())?;
        if !skill.centered {
            if let Some(npc) = map
                .npc(self.target_id)
                .filter(|npc| npc.is_training_goal() || crate::guild_war::is_war_structure(npc.id()))
            {
                return self.train_on_dummy(state, actor, character, npc, Some(&skill)).await;
            }
        }
        let character_location = character.entity().location();
        let targets = if skill.line > 0 || skill.sector > 0 {
            // Toward the aimed monster or the clicked cell.
            let toward = map
                .monster_entity(self.target_id)
                .map(|entity| {
                    let location = entity.basic().location();
                    (i32::from(location.x), i32::from(location.y))
                })
                .unwrap_or((i32::from(self.x), i32::from(self.y)));
            let from = (i32::from(character_location.x), i32::from(character_location.y));
            if toward == from {
                return Ok(());
            }
            let cells = crate::skills::line_cells(from, toward, skill.line);
            let mut hit = map
                .monster_entities()
                .into_iter()
                .filter(|candidate| {
                    let entity = candidate.basic();
                    let location = entity.location();
                    let cell = (i32::from(location.x), i32::from(location.y));
                    entity.map_id() == character.entity().map_id()
                        && !entity.is_dead()
                        && if skill.line > 0 {
                            cells.contains(&cell)
                        } else {
                            crate::skills::in_sector(from, toward, cell, skill.sector)
                        }
                })
                .collect::<Vec<_>>();
            hit.sort_by_key(|target| target.id());
            hit
        } else if skill.centered {
            // Around the caster: every living monster within the radius.
            let mut around = map
                .monster_entities()
                .into_iter()
                .filter(|candidate| {
                    let entity = candidate.basic();
                    let location = entity.location();
                    entity.map_id() == character.entity().map_id()
                        && !entity.is_dead()
                        && f32::from(
                            location
                                .x
                                .abs_diff(character_location.x)
                                .max(location.y.abs_diff(character_location.y)),
                        ) <= skill.radius.max(1.0)
                })
                .collect::<Vec<_>>();
            around.sort_by_key(|target| target.id());
            around
        } else {
            let Some(targets) = self.aimed_targets(state, &map, character, &skill) else {
                return Ok(());
            };
            targets
        };
        if !self.begin_cast(actor, character, &skill).await? {
            return Ok(());
        }
        self.cast_on_targets(state, actor, character, &map, &skill, targets).await
    }

    /// The target the skill was aimed at and, for area skills, the
    /// monsters around it. `None` when the target is invalid.
    fn aimed_targets(
        &self,
        state: &State,
        map: &crate::world::Map,
        character: &crate::entities::Character,
        skill: &crate::skills::SkillDefinition,
    ) -> Option<Vec<Arc<crate::entities::GameEntity>>> {
        let Some(primary) = map.monster_entity(self.target_id).or_else(|| {
            state
                .entities()
                .into_iter()
                .find(|entity| entity.id() == self.target_id)
        }) else {
            tracing::debug!(target_id = self.target_id, "Skill target was not found");
            return None;
        };
        let character_location = character.entity().location();
        let primary_entity = primary.basic();
        let primary_location = primary_entity.location();
        // Like the original server, magic range is measured in cells along
        // the larger axis, so diagonal targets keep the full distance.
        let distance = f32::from(
            character_location
                .x
                .abs_diff(primary_location.x)
                .max(character_location.y.abs_diff(primary_location.y)),
        );
        let target_character = primary.as_character();
        let pvp_allowed = match target_character {
            Some(target) => {
                (character.can_attack_character(target) || crate::events::enemies(character.id(), target.id()))
                    && !team_mode_spares(state, character, target)
                    && !crate::events::allies(character.id(), target.id())
                    && !target.revive_protected()
            },
            None => true,
        };
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
            return None;
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
        Some(targets)
    }

    /// Cooldown, mana, stamina and the XP circle of a cast; `false` when it
    /// cannot be cast now.
    async fn begin_cast(
        &self,
        actor: &Actor<ActorState>,
        character: &crate::entities::Character,
        skill: &crate::skills::SkillDefinition,
    ) -> Result<bool, Error> {
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
            tracing::trace!(character_id = character.id(), "Rejected skill during cooldown");
            return Ok(false);
        }
        if character.entity().spend_mana(skill.mana_cost).is_none() {
            actor
                .send(MsgTalk::from_system(
                    character.id(),
                    TalkChannel::System,
                    format!("{} requires {} MP.", skill.name, skill.mana_cost),
                ))
                .await?;
            return Ok(false);
        }
        if let Ok(mut skills) = next_skill.lock() {
            skills.insert(skill_cooldown_key(character.id(), skill.id), now + skill.cooldown);
        }
        actor.send(MsgUserInfo::from(character)).await?;
        super::spend_cast_costs(actor, character, skill).await?;
        Ok(true)
    }

    async fn cast_on_targets(
        &self,
        state: &State,
        actor: &Actor<ActorState>,
        character: &crate::entities::Character,
        map: &crate::world::Map,
        skill: &crate::skills::SkillDefinition,
        targets: Vec<Arc<crate::entities::GameEntity>>,
    ) -> Result<(), Error> {
        let mana = character.entity().mp();
        let skill = skill.clone();
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
        // Weapon skills: the physical attack, proficiency bonus included.
        let physical_stats = attacker;
        let physical_power = u16::try_from(
            u32::from(skill_power) * u32::from(100 + character.weapon_proficiency_bonus_percent()) / 100,
        )
        .unwrap_or(u16::MAX);
        let mut effects = Vec::with_capacity(targets.len());
        let mut defeated = Vec::new();
        let mut defeated_players = Vec::new();
        let mut hit_experience = 0_u64;
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
            // Phoenix gems strengthen magic (Redux `CalculateSkillDamage`).
            let magic_attack = target_attacker_stats
                .min_attack
                .saturating_add(target_attacker_stats.attack_flat)
                .saturating_add(skill.flat_power)
                .saturating_mul(100 + character.gem_bonus(crate::entities::GemFamily::Phoenix))
                / 100;
            // Superman strengthens every physical blow, skills too (Redux
            // `CalculatePhysicalDamage`): 10x on monsters, 2x on players.
            let superman = if character.has_status(crate::entities::StatusEffect::Superman) {
                crate::combat::superman_damage_percent(target.as_monster().is_some())
            } else {
                100
            };
            let mut outcome = if skill.physical {
                resolve_physical_hit(
                    scale_skill_power(scale_skill_power(physical_stats, physical_power), superman),
                    target_stats,
                    u32::from(health_before.current),
                    next_roll(),
                )
            } else {
                resolve_magic_damage(
                    magic_attack,
                    magic_defense,
                    // Monster `MagicDefence` (Redux takes it as a percent).
                    target.as_monster().map_or(0, |monster| {
                        u16::try_from(monster.monster_type().magic_resistance.clamp(0, 100)).unwrap_or(0)
                    }),
                    target_stats.damage_reduction_percent,
                    u32::from(health_before.current),
                    target_stats.max_health,
                    fire_resistance,
                )
            };
            if let (Some(talisman), Some(monster)) = (state.talismans().get(character.id()), target.as_monster()) {
                crate::bounty::empower(
                    &talisman,
                    monster_name_of(monster),
                    &mut outcome,
                    u32::from(health_before.current),
                );
            }
            crate::lucky::empower(Some(character), target.as_character(), &mut outcome, u32::from(health_before.current));
            let health_after = target_entity.apply_damage(outcome.damage);
            if let Some(victim) = target.as_character() {
                crate::death::check_crime(state, character, victim).await?;
            }
            if let Some(monster) = target.as_monster() {
                crate::systems::monster_ai::provoke(monster.id(), character.id());
                bounty_leech(state, character, monster, outcome.damage).await?;
                hit_experience += crate::leveling::combat_experience(
                    character.entity().level(),
                    u16::try_from(monster.monster_type().level.max(1)).unwrap_or(u16::MAX),
                    u32::from(health_before.current),
                    outcome.damage,
                    false,
                );
            }
            if !outcome.defeated {
                if let Some(status) = skill.status {
                    let effect = status.effect();
                    target.apply_status(effect, skill.status_duration);
                }
            }
            effects.push(SkillTarget::hit(target.id(), outcome.damage));
            if outcome.defeated {
                defeated.push(target.id());
                if target.as_character().is_some() {
                    defeated_players.push(target.clone());
                }
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
        // Attack skills learn as much as the hit earned (Redux
        // `AddSkillExperience(expGain)`), at least one.
        let practice = u32::try_from(hit_experience).unwrap_or(u32::MAX).max(1);
        if let Some(progress) = character.practice_skill(state.pool(), skill.id, practice).await? {
            actor.send(super::MsgSkill::from(progress)).await?;
        }
        for target_id in defeated {
            if let Some(entity) = map.monster_entity(target_id) {
                if let Some(monster) = entity.as_monster() {
                    self.reward_monster_defeat(state, actor, character, monster).await?;
                }
            }
        }
        self.grant_hit_experience(state, actor, character, hit_experience).await?;
        for victim in defeated_players {
            if let Some(victim) = victim.as_character() {
                crate::death::kill_character(state, victim, character.id(), Some(character)).await?;
            }
        }
        Ok(())
    }
}

/// Experience, drops and the death blow when `character` (or their pet,
/// `killer_id`) defeats a monster.
/// Team PK mode does not hit teammates, guildmates or allied guilds.
fn team_mode_spares(state: &State, attacker: &crate::entities::Character, target: &crate::entities::Character) -> bool {
    if attacker.kill_mode() != 2 {
        return false;
    }
    if state.team_manager().same_team(attacker.id(), target.id()) || state.guild_manager().same_guild(attacker.id(), target.id()) {
        return true;
    }
    let (Some(mine), Some(theirs)) = (
        state.guild_manager().guild_for(attacker.id()),
        state.guild_manager().guild_for(target.id()),
    ) else {
        return false;
    };
    state
        .guild_manager()
        .relation(mine.id, theirs.id)
        .is_some_and(|kind| kind == "ally")
}

fn monster_name_of(monster: &crate::entities::Monster) -> &str {
    &monster.monster_type().name
}

/// The bounty talisman's life leech: part of the damage dealt to the task's
/// monster comes back as life.
async fn bounty_leech(
    state: &State,
    character: &crate::entities::Character,
    monster: &crate::entities::Monster,
    damage: u32,
) -> Result<(), Error> {
    let Some(talisman) = state.talismans().get(character.id()) else {
        return Ok(());
    };
    let leeched = talisman.leech(monster_name_of(monster), damage);
    if leeched == 0 || character.entity().hp().current == 0 {
        return Ok(());
    }
    let life = character.entity().heal_health(u16::try_from(leeched).unwrap_or(u16::MAX));
    character
        .owner()
        .send(super::MsgUserAttrib::new(
            character.id(),
            super::UserAttribute::Life,
            u64::from(life.current),
        ))
        .await?;
    Ok(())
}

pub(crate) async fn reward_monster_defeat(
    state: &State,
    character: &crate::entities::Character,
    monster: &crate::entities::Monster,
    killer_id: u32,
) -> Result<(), Error> {
    character.add_xp_point();
    let level = monster.monster_type().level.max(1) as u32;
    let silver_drop = crate::drops::roll_silver(level, next_roll(), next_roll());
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
        let share = crate::store::with_vip_bonus(share, state.vip().is_vip(recipient.id()));
        let leveled = recipient.grant_experience(share.max(1));
        if let Some(level) = leveled {
            crate::leveling::after_level_up(state, recipient, level).await?;
        }
        let quests = crate::quests::record_monster_defeat(state.pool(), recipient, monster_type_id).await?;
        let bounty_leveled =
            super::msg_bounty::count_kill(state, recipient, monster_type_id, &monster.monster_type().name).await?;
        if let Some(level) = bounty_leveled {
            crate::leveling::after_level_up(state, recipient, level).await?;
        }
        let quest_leveled = quests
            .completed
            .iter()
            .filter_map(|quest| quest.leveled_to)
            .chain(bounty_leveled)
            .max();
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
        for quest in quests.completed {
            let item = quest
                .definition
                .reward_item
                .map(|item_type| format!(" and {}", crate::quests::item_name(item_type)))
                .unwrap_or_default();
            if let Some(item) = quest.reward_item {
                owner.send(crate::packets::MsgItemInfo::inventory_item(item)).await?;
            }
            owner
                .send(MsgTalk::from_system(
                    recipient.id(),
                    TalkChannel::System,
                    format!(
                        "Quest complete: {}. Reward: {} silver, {} experience{item}.",
                        quest.definition.name, quest.reward_silver, quest.reward_experience
                    ),
                ))
                .await?;
        }
        for quest in quests.waiting_for_room {
            owner
                .send(MsgTalk::from_system(
                    recipient.id(),
                    TalkChannel::System,
                    format!(
                        "Quest {}: make room in your bag for {}, then defeat one more.",
                        quest.name,
                        crate::quests::item_name(quest.reward_item.unwrap_or_default())
                    ),
                ))
                .await?;
        }
    }

    for entity in recipients.iter().filter(|_| silver_drop.is_some()) {
        let Some(recipient) = entity.as_character() else {
            continue;
        };
        recipient
            .owner()
            .send(MsgTalk::from_system(
                recipient.id(),
                TalkChannel::System,
                format!("Loot: {} silver dropped.", silver_drop.unwrap_or_default()),
            ))
            .await?;
    }
    let location = monster.entity().location();
    let monster_level = monster.monster_type().level.max(0) as u16;
    let mut drops = silver_drop
        .map(|silver| (silver_item_id(silver), silver))
        .into_iter()
        .collect::<Vec<_>>();
    drops.extend(
        state
            .drop_catalog()
            .roll(monster_type_id, monster_level)
            .into_iter()
            .chain(state.drop_catalog().roll_rares())
            .chain(crate::drops::roll_extras(monster_level, next_roll, |item| {
                state.shop_catalog().template(item).is_some()
            }))
            .map(|item_id| (item_id, 0)),
    );
    // The bounty talisman's loot bonus: one more roll of the drop table.
    if state
        .talismans()
        .get(character.id())
        .is_some_and(|talisman| talisman.extra_loot(&monster.monster_type().name, next_roll()))
    {
        drops.extend(
            state
                .drop_catalog()
                .roll(monster_type_id, monster_level)
                .into_iter()
                .map(|item_id| (item_id, 0)),
        );
    }
    // A boss: the server hears of its fall, and it drops more.
    drops.extend(
        crate::bosses::slain(state, character, monster)
            .await?
            .into_iter()
            .map(|item_id| (item_id, 0)),
    );
    // VIP: the drops their loot filter picks go straight into the bag.
    if state.vip().is_vip(character.id()) {
        let filter = state.vip().loot_filter(character.id());
        let mut kept = Vec::with_capacity(drops.len());
        for (item_id, silver) in drops {
            if !crate::store::loot::picks(filter, item_id, silver) {
                kept.push((item_id, silver));
            } else if silver > 0 {
                character.grant_silver(state.pool(), silver).await?;
                character.owner().send(MsgUserInfo::from(character)).await?;
            } else {
                let template = state.shop_catalog().template(item_id);
                let (durability, max_durability) =
                    template.map_or((0, 0), |template| (template.durability, template.max_durability));
                match character.add_ground_item(state.pool(), item_id, durability, max_durability).await? {
                    Some(item) => {
                        crate::audit::item("vip_auto_pickup", character.id(), item.uid, item.item_type, format_args!(""));
                        character.owner().send(super::MsgItemInfo::inventory_item(item)).await?;
                    },
                    // A full bag: it falls to the ground as usual.
                    None => kept.push((item_id, silver)),
                }
            }
        }
        drops = kept;
    }
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
        character.owner().send(item_drop.clone()).await?;
        if let Ok(screen) = character.try_screen() {
            screen.send_message(item_drop).await?;
        }
    }
    let location = monster.entity().location();
    // In Cyclone or Superman each kill counts and lengthens it; the death
    // carries the count as the original's (`65541 * KOCount`, the client
    // reads the high word).
    let ko = (killer_id == character.id()).then(|| character.count_ko()).flatten();
    let kill = MsgInteract {
        timestamp: 0,
        character_id: killer_id,
        target_id: monster.id(),
        x: location.x,
        y: location.y,
        action: InteractAction::Kill as u32,
        value: ko.map_or(silver_drop.unwrap_or_default(), |ko| ko.saturating_mul(65_541)),
        reserved: [0; 12],
    };
    character.owner().send(kill.clone()).await?;
    if let Ok(screen) = character.try_screen() {
        screen.send_message(kill).await?;
    }
    Ok(())
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

        // Ghosts walk, but do not fight.
        if character.entity().is_dead()
            && matches!(self.action_kind(), InteractAction::Attack | InteractAction::MagicAttack)
        {
            return Ok(());
        }
        if matches!(self.action_kind(), InteractAction::Attack | InteractAction::MagicAttack) {
            character.end_revive_protection();
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

        let bow = character
            .equipment_item(4)
            .is_some_and(|weapon| crate::shop::ShopCatalog::is_bow(weapon.item_type));
        let arrows = character.equipment_item(5).filter(|item| {
            crate::shop::ShopCatalog::is_arrow(item.item_type) && item.durability > 0
        });
        if bow && arrows.is_none() {
            actor
                .send(MsgTalk::from_system(
                    character.id(),
                    TalkChannel::TopLeft,
                    "Please equip arrows.",
                ))
                .await?;
            return Ok(());
        }

        let map = state.try_map(character.entity().map_id())?;
        if let Some(npc) = map.npc(self.target_id).filter(|npc| npc.is_training_goal() || crate::guild_war::is_war_structure(npc.id())) {
            return self.train_on_dummy(state, actor, character, npc, None).await;
        }
        let Some(target) = map.monster_entity(self.target_id).or_else(|| {
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
                .is_some_and(|target| {
                    !(character.can_attack_character(target) || crate::events::enemies(character.id(), target.id()))
                        || team_mode_spares(state, character, target)
                        || crate::events::allies(character.id(), target.id())
                        || target.revive_protected()
                })
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
        // A weapon skill the weapon knows may go off instead of the plain hit.
        if let Some(skill_id) = weapon_skill_roll(state, character) {
            let mut cast = self.clone();
            cast.action = InteractAction::MagicAttack as u32;
            cast.value = skill_id;
            let location = target_entity.location();
            cast.x = location.x;
            cast.y = location.y;
            return cast.process_magic_attack(state, actor, character).await;
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
        // Intensify: the charged shot hits twice as hard and is spent.
        let attacker_stats = if character.has_status(crate::entities::StatusEffect::Intensify) {
            character.apply_status(crate::entities::StatusEffect::Intensify, Duration::ZERO);
            scale_skill_power(attacker_stats, 200)
        } else {
            attacker_stats
        };
        // Derive the roll on the authoritative server. The old implementation
        // used the client timestamp directly, which made damage repeatable and
        // often pinned it to the minimum value.
        let life_before = u32::from(target_entity.hp().current());
        let mut outcome = resolve_physical_hit(attacker_stats, target_stats, life_before, next_roll());
        // Fury gems and Accuracy steady the blow; the rest may miss.
        let accuracy = character.gem_bonus(crate::entities::GemFamily::Fury)
            + if character.has_status(crate::entities::StatusEffect::Accuracy) {
                crate::combat::ACCURACY_STATUS_BONUS
            } else {
                0
            };
        if next_roll() % 100 >= crate::combat::hit_chance(0, accuracy) {
            outcome.damage = 0;
            outcome.raw_damage = 0;
            outcome.remaining_health = life_before;
            outcome.defeated = false;
        }
        if let (Some(talisman), Some(monster)) = (state.talismans().get(character.id()), target.as_monster()) {
            crate::bounty::empower(&talisman, monster_name_of(monster), &mut outcome, life_before);
        }
        crate::lucky::empower(Some(character), target.as_character(), &mut outcome, life_before);
        target_entity.apply_damage(outcome.damage);
        if let Some(monster) = target.as_monster() {
            crate::systems::monster_ai::provoke(monster.id(), character.id());
            bounty_leech(state, character, monster, outcome.damage).await?;
        }
        if let Some(victim) = target.as_character() {
            crate::death::check_crime(state, character, victim).await?;
        }
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
            // Redux `AddProficiencyExperience(WeaponType, expGain)`: the
            // weapon learns as much as the hit's experience.
            let practice = target.as_monster().map_or(1, |monster| {
                crate::training::experience(
                    character.entity().level(),
                    u16::try_from(monster.monster_type().level.max(1)).unwrap_or(u16::MAX),
                    life_before,
                    outcome.damage,
                )
            });
            let practiced = character
                .practice_weapon_proficiency(state.pool(), weapon_type, u32::try_from(practice).unwrap_or(u32::MAX).max(1))
                .await?;
            if practiced.is_some() {
                // Send the type's aggregate, which may come from another family.
                let weapon_type = weapon_type / 1_000;
                if let Some(packet) = super::MsgWeaponSkill::for_all(character.weapon_proficiencies())
                    .into_iter()
                    .find(|packet| packet.weapon_type == weapon_type)
                {
                    actor.send(packet).await?;
                }
            }
        }
        if let Some(monster) = target.as_monster() {
            let experience = crate::leveling::combat_experience(
                character.entity().level(),
                u16::try_from(monster.monster_type().level.max(1)).unwrap_or(u16::MAX),
                life_before,
                outcome.damage,
                false,
            );
            self.grant_hit_experience(state, actor, character, experience).await?;
        }
        if outcome.defeated {
            if let Some(victim) = target.as_character() {
                crate::death::kill_character(state, victim, character.id(), Some(character)).await?;
                return Ok(());
            }
            let Some(monster) = target.as_monster() else {
                return Ok(());
            };
            self.reward_monster_defeat(state, actor, character, monster).await?;
            tracing::info!(target_id = self.target_id, "Monster defeated");
        }
        Ok(())
    }
}

/// The weapon skill a plain attack sets off, if any: the right hand's first,
/// then the left's, each going off `chance` times in a hundred (Redux
/// `GetWeaponSkill`).
fn weapon_skill_roll(state: &State, character: &crate::entities::Character) -> Option<u32> {
    let skill = [4_u8, 5].into_iter().find_map(|slot| {
        let weapon = character.equipment_item(slot)?;
        let skill = state.skill_catalog().weapon_skill(weapon.item_type / 1_000)?;
        character.has_skill(skill.id).then_some(skill)
    })?;
    (crate::combat::next_roll() % 100 < u32::from(skill.chance)).then_some(skill.id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skills::BASIC_FIRE_ID;
    use tq_network::PacketEncode;

    #[test]
    fn attack_magics_are_single_target_like_the_original() {
        use crate::skills::{BASIC_FIRE_ID, FLAME_BURST_ID, WHIRLWIND_ID};
        let catalog = crate::skills::SkillCatalog::default();
        let basic = catalog.get(BASIC_FIRE_ID).expect("thunder definition");
        assert_eq!(basic.mana_cost, 20);
        // MagicType.dat: Thunder, Fire and Tornado are Type=1 with Range=0.
        for id in [BASIC_FIRE_ID, FLAME_BURST_ID, WHIRLWIND_ID] {
            assert_eq!(catalog.get(id).expect("skill definition").radius, 0.0);
        }
        assert_eq!(catalog.get(FLAME_BURST_ID).expect("fire").mana_cost, 35);
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
