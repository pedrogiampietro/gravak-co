use crate::entities::{AttributeAllocation, AttributeKind, ItemUpgradeOutcome};
use crate::packets::{
    MsgGroundItem, MsgGuild, MsgItemInfo, MsgNobility, MsgPlayer, MsgSkill, MsgTalk, MsgUserInfo, TalkChannel,
};
use crate::state::GroundItem;
use crate::world::Maps;
use crate::{ActorState, Error};
use argh::FromArgs;
use std::time::Duration;
use tq_network::Actor;

/// Moves a character to `map_id` at `location`, with its screen refreshed.
/// Where `$go` takes a game master: an online player by name, else an NPC,
/// else a monster's biggest spawn; exact names first, then partial ones.
async fn find_destination(
    state: &crate::State,
    me: &crate::entities::Character,
    wanted: &str,
) -> Result<Option<(String, u32, u16, u16)>, Error> {
    let wanted = wanted.trim();
    if wanted.is_empty() {
        return Ok(None);
    }
    let lower = wanted.to_lowercase();
    let players = state
        .entities()
        .into_iter()
        .filter_map(|entity| {
            let other = entity.as_character()?;
            (other.id() != me.id()).then(|| {
                let location = other.entity().location();
                (other.entity().name().to_owned(), other.entity().map_id(), location.x, location.y)
            })
        })
        .collect::<Vec<_>>();
    if let Some((name, map_id, x, y)) = players.iter().find(|(name, ..)| name.to_lowercase() == lower) {
        return Ok(Some((format!("{name} (player)"), *map_id, *x, *y)));
    }
    let pattern = format!("%{}%", lower.replace('%', "").replace('_', "\\_"));
    for exact in [true, false] {
        let npc: Option<(String, i64, i64, i64)> = sqlx::query_as(if exact {
            "SELECT name, map_id, x, y FROM npcs WHERE lower(name) = $1 ORDER BY id LIMIT 1"
        } else {
            "SELECT name, map_id, x, y FROM npcs WHERE lower(name) LIKE $1 ORDER BY length(name), id LIMIT 1"
        })
        .bind(if exact { &lower } else { &pattern })
        .fetch_optional(state.pool())
        .await?;
        if let Some((name, map_id, x, y)) = npc {
            return Ok(Some((format!("{name} (NPC)"), map_id as u32, x as u16, y as u16)));
        }
        let monster: Option<(String, i64, i64, i64)> = sqlx::query_as(if exact {
            "SELECT t.name, s.map_id, s.x, s.y FROM monster_spawns s JOIN monster_types t ON t.id = s.monster_type_id
             WHERE lower(t.name) = $1 ORDER BY s.amount DESC, s.id LIMIT 1"
        } else {
            "SELECT t.name, s.map_id, s.x, s.y FROM monster_spawns s JOIN monster_types t ON t.id = s.monster_type_id
             WHERE lower(t.name) LIKE $1 ORDER BY length(t.name), s.amount DESC, s.id LIMIT 1"
        })
        .bind(if exact { &lower } else { &pattern })
        .fetch_optional(state.pool())
        .await?;
        if let Some((name, map_id, x, y)) = monster {
            return Ok(Some((format!("{name} (monster)"), map_id as u32, x as u16, y as u16)));
        }
    }
    Ok(players
        .into_iter()
        .find(|(name, ..)| name.to_lowercase().contains(&lower))
        .map(|(name, map_id, x, y)| (format!("{name} (player)"), map_id, x, y)))
}

async fn move_to(
    state: &crate::State,
    entity: &std::sync::Arc<crate::entities::GameEntity>,
    map_id: u32,
    location: (u16, u16),
) -> Result<(), Error> {
    let character = entity.as_character().ok_or(Error::CharacterNotFound)?;
    let old_map_id = character.entity().map_id();
    let old_map = state.try_map(old_map_id)?;
    let map = state.try_map(map_id)?;
    character.teleport(state, map_id, location).await?;
    map.insert_entity(entity.clone()).await?;
    // `Character::teleport` removes the entity from its old map.  Do not
    // remove it a second time when the destination is the same map: in that
    // case `old_map` and `map` are the same map object, and the second
    // removal would erase the freshly reinserted character from the world
    // index.
    if old_map_id != map_id {
        old_map.remove_entity(entity)?;
    }
    // Teleport updates the character location and map membership, but does
    // not itself repopulate the client's surrounding entities. Refresh the
    // screen so nearby NPCs and monsters are spawned.
    character.try_screen()?.load_surroundings(state).await?;
    Ok(())
}

/// Monsters one `$monster` creates, at most.
const MAX_SUMMONED: u32 = 20;

/// Creates `amount` monsters of type `wanted` (an id or a name) around a
/// game master; they fight as any other, drop loot, and are gone once dead.
async fn summon_monsters(
    state: &crate::State,
    me: &crate::entities::Character,
    wanted: &str,
    amount: u32,
) -> Result<String, Error> {
    let id = wanted.parse::<i64>().unwrap_or(-1);
    let monster_type = sqlx::query_as::<_, tq_db::monster::MonsterType>(
        "SELECT id, name, look, level, life, attack_min, attack_max, defense, attack_speed, respawn_seconds, view_range, attack_range, attack_mode, magic_resistance \
         FROM monster_types WHERE id = $1 OR lower(name) = lower($2) ORDER BY (id = $1) DESC, id LIMIT 1",
    )
    .bind(id)
    .bind(wanted)
    .fetch_optional(state.pool())
    .await?;
    let Some(monster_type) = monster_type else {
        return Ok(format!("No monster called {wanted}."));
    };
    let map_id = me.entity().map_id();
    let map = state.try_map(map_id)?;
    let here = me.entity().location();
    let mut created = 0;
    for index in 0..amount {
        // Around the game master, a ring at a time, on walkable cells.
        let ring = 2 + index as i32 / 8;
        let (dx, dy) = [(1, 0), (1, 1), (0, 1), (-1, 1), (-1, 0), (-1, -1), (0, -1), (1, -1)][index as usize % 8];
        let mut origin = here;
        origin.x = (i32::from(here.x) + dx * ring).max(0) as u16;
        origin.y = (i32::from(here.y) + dy * ring).max(0) as u16;
        let Some(spot) = map.landing_position(origin, 4) else {
            continue;
        };
        let spawn = tq_db::monster::MonsterSpawn {
            id: 0,
            monster_type_id: monster_type.id,
            map_id: map_id as i32,
            x: spot.x as i16,
            y: spot.y as i16,
            amount: 1,
            radius: 3,
        };
        let id = crate::world::next_summoned_id();
        let monster = crate::entities::Monster::new(id, monster_type.clone(), spawn);
        let entity = std::sync::Arc::new(crate::entities::GameEntity::from(monster));
        map.add_monster(entity.clone()).await?;
        if let Some(monster) = entity.as_monster() {
            let _ = map.broadcast(MsgPlayer::from_monster(monster)).await;
            let _ = map
                .broadcast(crate::packets::MsgAction::new(id, 0, 0, 0, crate::packets::ActionType::SpawnEffect))
                .await;
        }
        created += 1;
    }
    tracing::info!(
        target: "world",
        character = %me.entity().name(),
        monster = %monster_type.name,
        created,
        map_id,
        "Game master created monsters"
    );
    Ok(format!(
        "Created {created} {} (level {}). $monster-clear removes them.",
        monster_type.name, monster_type.level
    ))
}

pub async fn parse_and_execute(state: &crate::State, actor: &Actor<ActorState>, args: &[&str]) -> Result<(), Error> {
    let entity = actor.entity();
    let me = entity.as_character().ok_or(Error::CharacterNotFound)?;
    let c = match Command::from_args(&["commands"], args) {
        Ok(cmd) => cmd,
        Err(e) => {
            let lines = e.output.lines().map(|e| e.to_owned()).skip_while(|e| e.is_empty());
            for line in lines {
                actor
                    .send(MsgTalk::from_system(me.id(), TalkChannel::System, line))
                    .await?;
            }
            return Ok(());
        },
    };
    if !c.commands.for_players() && !actor.is_gm() {
        let name = args.first().copied().unwrap_or_default();
        tracing::warn!(target: "auth", character = %me.entity().name(), command = name, "Game master command refused");
        actor
            .send(MsgTalk::from_system(
                me.id(),
                TalkChannel::System,
                format!("${name} is for game masters."),
            ))
            .await?;
        return Ok(());
    }
    match c.commands {
        SubCommands::Dc(_) => {
            actor.shutdown().await?;
            Ok(())
        },
        SubCommands::Teleport(info) => {
            move_to(state, &actor.entity(), info.map_id, (info.x, info.y)).await?;
            if info.all {
                let mut summoned = 0;
                for entity in state.entities() {
                    let Some(other) = entity.as_character() else { continue };
                    if other.id() == me.id() {
                        continue;
                    }
                    move_to(state, &entity, info.map_id, (info.x, info.y)).await?;
                    let _ = other
                        .owner()
                        .send(MsgTalk::from_system(
                            other.id(),
                            TalkChannel::TopLeft,
                            format!("{} summoned everyone here.", me.entity().name()),
                        ))
                        .await;
                    summoned += 1;
                }
                tracing::info!(target: "world", gm = %me.entity().name(), summoned, map = info.map_id, "Summoned everyone online");
                actor
                    .send(MsgTalk::from_system(
                        me.id(),
                        TalkChannel::System,
                        format!("Summoned {summoned} players."),
                    ))
                    .await?;
            }
            Ok(())
        },
        SubCommands::Go(info) => {
            let wanted = info.name.join(" ");
            let Some((what, map_id, x, y)) = find_destination(state, me, &wanted).await? else {
                actor
                    .send(MsgTalk::from_system(
                        me.id(),
                        TalkChannel::System,
                        format!("No player, NPC or monster called {wanted}."),
                    ))
                    .await?;
                return Ok(());
            };
            let map = state.try_map(map_id)?;
            map.load().await?;
            // Beside it rather than on it.
            let origin = primitives::Location::new(x.saturating_add(2), y.saturating_add(2), 0);
            let spot = map.nearest_walkable_position(origin, 12).unwrap_or(origin);
            move_to(state, &actor.entity(), map_id, (spot.x, spot.y)).await?;
            actor
                .send(MsgTalk::from_system(
                    me.id(),
                    TalkChannel::System,
                    format!("Went to {what} (map {map_id}, {x},{y})."),
                ))
                .await?;
            Ok(())
        },
        SubCommands::BountyGo(_) => {
            let board = crate::bounty::board(state.pool(), me.database_id()).await?;
            let Some(task) = board.task else {
                actor
                    .send(MsgTalk::from_system(me.id(), TalkChannel::System, "No bounty under way."))
                    .await?;
                return Ok(());
            };
            // The biggest spawn of any monster type of that name.
            let spawn: Option<(i64, i64, i64)> = sqlx::query_as(
                "SELECT s.map_id, s.x, s.y FROM monster_spawns s
                 JOIN monster_types t ON t.id = s.monster_type_id
                 WHERE t.name = $1 ORDER BY s.amount DESC, s.id LIMIT 1",
            )
            .bind(&task.monster_name)
            .fetch_optional(state.pool())
            .await?;
            let Some((map_id, x, y)) = spawn else {
                actor
                    .send(MsgTalk::from_system(me.id(), TalkChannel::System, "That monster roams nowhere."))
                    .await?;
                return Ok(());
            };
            let map_id = u32::try_from(map_id).unwrap_or_default();
            let map = state.try_map(map_id)?;
            map.load().await?;
            let origin = primitives::Location::new(
                u16::try_from(x).unwrap_or_default(),
                u16::try_from(y).unwrap_or_default(),
                0,
            );
            let spot = map.nearest_walkable_position(origin, 12).unwrap_or(origin);
            move_to(state, &actor.entity(), map_id, (spot.x, spot.y)).await?;
            actor
                .send(MsgTalk::from_system(
                    me.id(),
                    TalkChannel::System,
                    format!("{} roam here.", task.monster_name),
                ))
                .await?;
            Ok(())
        },
        SubCommands::Monster(info) => {
            let amount = info.amount.unwrap_or(1).clamp(1, MAX_SUMMONED);
            let line = summon_monsters(state, me, &info.monster, amount).await?;
            actor.send(MsgTalk::from_system(me.id(), TalkChannel::System, line)).await?;
            Ok(())
        },
        SubCommands::Boss(info) => {
            let line = match crate::bosses::appear(state, info.id).await? {
                Some(name) => format!("{name} appeared."),
                None => format!("No boss {} (or no room for it). Bosses: 8300-8318 and 3120.", info.id),
            };
            actor.send(MsgTalk::from_system(me.id(), TalkChannel::System, line)).await?;
            Ok(())
        },
        SubCommands::Event(info) => {
            let line = match crate::events::Mode::parse(&info.mode) {
                Some(mode) => crate::events::open(state, mode).await,
                None => "Events: tdm, freeze, infection, ctf, lms.".to_owned(),
            };
            actor.send(MsgTalk::from_system(me.id(), TalkChannel::System, line)).await?;
            Ok(())
        },
        SubCommands::Join(_) => {
            let line = crate::events::join(me);
            actor.send(MsgTalk::from_system(me.id(), TalkChannel::TopLeft, line)).await?;
            Ok(())
        },
        SubCommands::Leave(_) => {
            let line = crate::events::leave(me);
            actor.send(MsgTalk::from_system(me.id(), TalkChannel::TopLeft, line)).await?;
            Ok(())
        },
        SubCommands::Xp(_) => {
            me.fill_xp();
            actor
                .send(MsgTalk::from_system(
                    me.id(),
                    TalkChannel::System,
                    "The XP circle fills: in a second the XP skills light up in the skill bar. Pick Cyclone (1110) or Superman (1025, level 40) there and right-click within 20 seconds.",
                ))
                .await?;
            Ok(())
        },
        SubCommands::Stamina(info) => {
            me.set_stamina(info.amount.unwrap_or(0));
            actor
                .send(crate::packets::MsgUserAttrib::new(
                    me.id(),
                    crate::packets::UserAttribute::Stamina,
                    u64::from(me.stamina()),
                ))
                .await?;
            Ok(())
        },
        SubCommands::Lucky(info) => {
            me.add_lucky_time(info.seconds.unwrap_or(600));
            actor
                .send(MsgTalk::from_system(
                    me.id(),
                    TalkChannel::System,
                    format!("Lucky time: {} seconds.", me.lucky_time()),
                ))
                .await?;
            Ok(())
        },
        SubCommands::Rebirths(info) => {
            me.set_rebirths(info.count);
            me.save(state).await?;
            if crate::lucky::grant_bless(state, me).await? {
                if let Some(progress) = me.skills().into_iter().find(|skill| skill.skill_id == crate::lucky::BLESS) {
                    actor.send(MsgSkill::from(progress)).await?;
                }
            }
            actor
                .send(MsgTalk::from_system(me.id(), TalkChannel::System, format!("Rebirths: {}.", me.rebirths())))
                .await?;
            Ok(())
        },
        SubCommands::MonsterClear(_) => {
            let map = state.try_map(me.entity().map_id())?;
            let summoned: Vec<u32> = map
                .monster_entities()
                .into_iter()
                .map(|entity| entity.id())
                .filter(|id| crate::constants::is_summoned(*id))
                .collect();
            for id in &summoned {
                map.remove_monster(*id);
                let _ = map
                    .broadcast(crate::packets::MsgAction::new(*id, 0, 0, 0, crate::packets::ActionType::LeaveMap))
                    .await;
            }
            actor
                .send(MsgTalk::from_system(
                    me.id(),
                    TalkChannel::System,
                    format!("Removed {} created monster(s) from this map.", summoned.len()),
                ))
                .await?;
            Ok(())
        },
        SubCommands::BountyKills(info) => {
            let kills = info.amount.unwrap_or(10).max(1);
            match crate::packets::add_bounty_kills(state, me, kills).await? {
                Ok(leveled) => {
                    if let Some(level) = leveled {
                        crate::leveling::after_level_up(state, me, level).await?;
                    }
                    me.save(state).await?;
                    actor.send(MsgUserInfo::from(me)).await?;
                },
                Err(reason) => {
                    actor.send(MsgTalk::from_system(me.id(), TalkChannel::System, reason)).await?;
                },
            }
            Ok(())
        },
        SubCommands::BountyPoints(info) => {
            let points = info.amount.unwrap_or(100);
            crate::bounty::board(state.pool(), me.database_id()).await?;
            sqlx::query("UPDATE bounty_profiles SET points = points + $2 WHERE character_id = $1")
                .bind(i64::from(me.database_id()))
                .bind(i64::from(points))
                .execute(state.pool())
                .await?;
            crate::packets::send_board(state, me).await?;
            actor
                .send(MsgTalk::from_system(
                    me.id(),
                    TalkChannel::System,
                    format!("Added {points} bounty points."),
                ))
                .await?;
            Ok(())
        },
        SubCommands::Which(which) => {
            if which.map {
                let map_id = me.entity().map_id();
                actor
                    .send(MsgTalk::from_system(
                        me.id(),
                        TalkChannel::System,
                        format!("Current Map: {:?} = {}", Maps::from(map_id), map_id),
                    ))
                    .await?;
            }
            Ok(())
        },
        SubCommands::QuestStatus(_) => {
            let quests = crate::quests::list(state.pool(), me.database_id()).await?;
            if quests.is_empty() {
                actor
                    .send(MsgTalk::from_system(me.id(), TalkChannel::System, "No quests started."))
                    .await?;
            } else {
                for quest in quests {
                    let state = if quest.completed { "completed" } else { "active" };
                    actor
                        .send(MsgTalk::from_system(
                            me.id(),
                            TalkChannel::System,
                            format!(
                                "Quest {} [{}]: {}/{}.",
                                quest.definition.name, state, quest.progress, quest.definition.required_count
                            ),
                        ))
                        .await?;
                }
            }
            Ok(())
        },
        SubCommands::NobilityStatus(_) => {
            let status = me.nobility_status();
            let next = status
                .next_rank
                .map_or_else(|| "MAX".to_owned(), |rank| rank.label().to_owned());
            actor
                .send(MsgTalk::from_system(
                    me.id(),
                    TalkChannel::System,
                    format!(
                        "Nobility: {} ({} donation). Next: {} ({} silver).",
                        status.rank.label(),
                        status.donation,
                        next,
                        status.remaining
                    ),
                ))
                .await?;
            actor.send(MsgNobility::from(me)).await?;
            Ok(())
        },
        SubCommands::NobilityDonate(info) => {
            let outcome = me.donate_nobility(state.pool(), info.amount).await?;
            match outcome {
                crate::nobility::DonateOutcome::Donated(status) => {
                    actor.send(MsgUserInfo::from(me)).await?;
                    actor
                        .send(MsgNobility::status(me.id(), status, me.equipped_title()))
                        .await?;
                    crate::packets::send_nobility_ranking(state, actor).await?;
                    actor
                        .send(MsgTalk::from_system(
                            me.id(),
                            TalkChannel::System,
                            format!(
                                "Donated {} silver. Nobility: {} ({} total).",
                                info.amount,
                                status.rank.label(),
                                status.donation
                            ),
                        ))
                        .await?;
                },
                crate::nobility::DonateOutcome::InvalidLevel => {
                    actor
                        .send(MsgTalk::from_system(
                            me.id(),
                            TalkChannel::System,
                            format!("Nobility requires level {}.", crate::nobility::MIN_LEVEL),
                        ))
                        .await?;
                },
                crate::nobility::DonateOutcome::InvalidAmount => {
                    actor
                        .send(MsgTalk::from_system(
                            me.id(),
                            TalkChannel::System,
                            format!(
                                "Nobility donations require at least {} silver.",
                                crate::nobility::MIN_DONATION
                            ),
                        ))
                        .await?;
                },
                crate::nobility::DonateOutcome::InsufficientSilver => {
                    actor
                        .send(MsgTalk::from_system(
                            me.id(),
                            TalkChannel::System,
                            "Not enough silver for that nobility donation.",
                        ))
                        .await?;
                },
            }
            Ok(())
        },
        SubCommands::TitleStatus(_) => {
            actor
                .send(MsgTalk::from_system(
                    me.id(),
                    TalkChannel::System,
                    format!("Title: {} (id {}).", me.equipped_title_name(), me.equipped_title()),
                ))
                .await?;
            actor.send(MsgNobility::from(me)).await?;
            Ok(())
        },
        SubCommands::TitleEquip(info) => {
            match me.equip_title(state.pool(), info.title_id).await? {
                crate::titles::EquipOutcome::Equipped(title_id) => {
                    actor.send(MsgNobility::from(me)).await?;
                    actor
                        .send(MsgTalk::from_system(
                            me.id(),
                            TalkChannel::System,
                            format!("Equipped title: {} (id {}).", me.equipped_title_name(), title_id),
                        ))
                        .await?;
                },
                crate::titles::EquipOutcome::UnknownTitle => {
                    actor
                        .send(MsgTalk::from_system(
                            me.id(),
                            TalkChannel::System,
                            format!("Unknown title {}.", info.title_id),
                        ))
                        .await?;
                },
                crate::titles::EquipOutcome::Locked => {
                    actor
                        .send(MsgTalk::from_system(
                            me.id(),
                            TalkChannel::System,
                            format!("Title {} is still locked.", info.title_id),
                        ))
                        .await?;
                },
            }
            Ok(())
        },
        SubCommands::NobilityRanking(_) => {
            let entries = crate::nobility::ranking(state.pool(), 10).await?;
            if entries.is_empty() {
                actor
                    .send(MsgTalk::from_system(
                        me.id(),
                        TalkChannel::System,
                        "Nobility ranking is empty.",
                    ))
                    .await?;
            } else {
                for (position, entry) in entries.into_iter().enumerate() {
                    actor
                        .send(MsgTalk::from_system(
                            me.id(),
                            TalkChannel::System,
                            format!(
                                "Nobility #{}: {} - {} ({} donation).",
                                position + 1,
                                entry.name,
                                entry.rank.label(),
                                entry.donation
                            ),
                        ))
                        .await?;
                }
            }
            crate::packets::send_nobility_ranking(state, actor).await?;
            Ok(())
        },
        SubCommands::CreateGuild(info) => {
            let change = state
                .guild_manager()
                .create(me.id(), info.name)
                .map_err(|error| Error::Other(error.message().to_owned()))?;
            let crate::guild::GuildChange::Created { guild } = change else {
                return Err(Error::Other("Guild creation did not produce a guild.".to_owned()));
            };
            crate::guild::GuildManager::persist_created(state.pool(), &guild).await?;
            actor
                .send(MsgGuild::new(crate::packets::SET_GUILD, guild.id).with_strings([guild.name.clone()]))
                .await?;
            crate::packets::send_guild_members(state, me.id(), &guild).await?;
            actor
                .send(MsgTalk::from_system(
                    me.id(),
                    TalkChannel::Guild,
                    format!("Guild {} created.", guild.name),
                ))
                .await?;
            Ok(())
        },
        SubCommands::GuildDeposit(info) => {
            let Some(guild) = state.guild_manager().guild_for(me.id()) else {
                actor
                    .send(MsgTalk::from_system(
                        me.id(),
                        TalkChannel::System,
                        "You are not in a guild.",
                    ))
                    .await?;
                return Ok(());
            };
            let Some((removed, stored)) = me
                .store_guild_warehouse_item(state.pool(), guild.id, info.item_uid, info.position)
                .await?
            else {
                actor
                    .send(MsgTalk::from_system(
                        me.id(),
                        TalkChannel::System,
                        "Could not store that item. Check the UID, position and available guild slot.",
                    ))
                    .await?;
                return Ok(());
            };
            actor.send(MsgItemInfo::remove_inventory_item(removed)).await?;
            actor
                .send(MsgTalk::from_system(
                    me.id(),
                    TalkChannel::Guild,
                    format!(
                        "Stored item {} in guild warehouse slot {} (UID preserved).",
                        stored.item_type, stored.position
                    ),
                ))
                .await?;
            Ok(())
        },
        SubCommands::GuildWithdraw(info) => {
            let Some(guild) = state.guild_manager().guild_for(me.id()) else {
                actor
                    .send(MsgTalk::from_system(
                        me.id(),
                        TalkChannel::System,
                        "You are not in a guild.",
                    ))
                    .await?;
                return Ok(());
            };
            let Some((stored, withdrawn)) = me
                .withdraw_guild_warehouse_item(state.pool(), guild.id, info.item_uid, info.position)
                .await?
            else {
                actor
                    .send(MsgTalk::from_system(
                        me.id(),
                        TalkChannel::System,
                        "Could not withdraw that item. Check the UID and an empty inventory slot.",
                    ))
                    .await?;
                return Ok(());
            };
            actor.send(MsgItemInfo::inventory_item(withdrawn)).await?;
            actor
                .send(MsgTalk::from_system(
                    me.id(),
                    TalkChannel::Guild,
                    format!(
                        "Withdrew item {} from guild warehouse slot {} (UID preserved).",
                        stored.item_type, stored.position
                    ),
                ))
                .await?;
            Ok(())
        },
        SubCommands::JumpBack(_) => {
            me.kick_back().await?;
            Ok(())
        },
        SubCommands::Weather(weather) => {
            let map = state.try_map(me.entity().map_id())?;
            map.change_weather(weather.kind.into()).await?;
            Ok(())
        },
        SubCommands::GiveItem(info) if info.count > 1 => {
            let mut created = 0;
            for _ in 0..info.count.min(40) {
                let Some(item) = me.add_ground_item(state.pool(), info.item_type, 100, 100).await? else {
                    break;
                };
                actor.send(MsgItemInfo::inventory_item(item)).await?;
                created += 1;
            }
            actor
                .send(MsgTalk::from_system(
                    me.id(),
                    TalkChannel::System,
                    format!("Created {created} of item {}.", info.item_type),
                ))
                .await?;
            Ok(())
        },
        SubCommands::GiveItem(info) => {
            let Some(item) = me.add_ground_item(state.pool(), info.item_type, 100, 100).await? else {
                actor
                    .send(MsgTalk::from_system(me.id(), TalkChannel::System, "Inventory is full."))
                    .await?;
                return Ok(());
            };
            let position = item.position;
            let item = if info.rarity > 0 {
                me.set_item_rarity(state.pool(), item.uid, info.rarity)
                    .await?
                    .ok_or_else(|| Error::Other("created item could not receive its rarity".into()))?
            } else {
                item
            };
            actor.send(MsgItemInfo::inventory_item(item)).await?;
            actor
                .send(MsgTalk::from_system(
                    me.id(),
                    TalkChannel::System,
                    format!("Created item {} in inventory slot {}.", info.item_type, position),
                ))
                .await?;
            Ok(())
        },
        SubCommands::TradeFixture(_) => {
            let Some(item) = me.inventory_items().into_iter().find(|item| item.item_type == 410003) else {
                actor
                    .send(MsgTalk::from_system(
                        me.id(),
                        TalkChannel::System,
                        "Trade fixture item 410003 was not found in inventory.",
                    ))
                    .await?;
                return Ok(());
            };
            actor.send(MsgItemInfo::inventory_item(item)).await?;
            actor
                .send(MsgTalk::from_system(
                    me.id(),
                    TalkChannel::System,
                    format!("Trade fixture item {} is ready (UID {}).", item.item_type, item.uid),
                ))
                .await?;
            Ok(())
        },
        SubCommands::TestEquipment(_) => {
            // One item for each usable paper-doll position. Position 0 is a
            // reserved client slot and intentionally has no item family.
            const TEST_ITEMS: [(u32, &str); 9] = [
                (111003, "Head"),
                (120003, "Amulet"),
                (130003, "Armor"),
                (150003, "Ring"),
                (152013, "Bracelet"),
                (160013, "Boots"),
                (181305, "Garment"),
                (410003, "Weapon"),
                (900003, "Shield"),
            ];
            let mut created = Vec::new();
            for (item_type, label) in TEST_ITEMS {
                let Some(item) = me.add_ground_item(state.pool(), item_type, 100, 100).await? else {
                    actor
                        .send(MsgTalk::from_system(
                            me.id(),
                            TalkChannel::System,
                            "Inventory is full; test equipment creation stopped.",
                        ))
                        .await?;
                    break;
                };
                actor.send(MsgItemInfo::inventory_item(item)).await?;
                created.push(format!("{}({})", label, item_type));
            }
            actor
                .send(MsgTalk::from_system(
                    me.id(),
                    TalkChannel::System,
                    if created.is_empty() {
                        "No test equipment was created.".to_owned()
                    } else {
                        format!("Created test equipment: {}.", created.join(", "))
                    },
                ))
                .await?;
            Ok(())
        },
        SubCommands::TestSockets(_) => {
            let samples = [
                (410_003_u32, "weapon"),
                (700_001_u32, "gem"),
                (1_088_000_u32, "DragonBall"),
            ];
            let mut created = Vec::new();
            for (item_type, label) in samples {
                let amount = if item_type == 1_088_000 { 5 } else { 1 };
                for _ in 0..amount {
                    let Some(item) = me.add_ground_item(state.pool(), item_type, 100, 100).await? else {
                        break;
                    };
                    actor.send(MsgItemInfo::inventory_item(item)).await?;
                    created.push(format!("{label}={} (uid {})", item.item_type, item.uid));
                }
            }
            actor
                .send(MsgTalk::from_system(
                    me.id(),
                    TalkChannel::System,
                    format!(
                        "Socket test items created: {}. Open a socket with the DragonBall, then insert the gem.",
                        created.join(", ")
                    ),
                ))
                .await?;
            Ok(())
        },
        SubCommands::TestQuality(_) => {
            let samples = [(410_005_u32, "quality weapon"), (1_088_000_u32, "DragonBall")];
            let mut created = Vec::new();
            for (item_type, label) in samples {
                let Some(item) = me.add_ground_item(state.pool(), item_type, 100, 100).await? else {
                    actor
                        .send(MsgTalk::from_system(me.id(), TalkChannel::System, "Inventory is full."))
                        .await?;
                    break;
                };
                actor.send(MsgItemInfo::inventory_item(item)).await?;
                created.push(format!("{label}={} (uid {})", item.item_type, item.uid));
            }
            actor
                .send(MsgTalk::from_system(
                    me.id(),
                    TalkChannel::System,
                    format!(
                        "Quality test items created: {}. Use QUALITY or ArtisanWind.",
                        created.join(", ")
                    ),
                ))
                .await?;
            Ok(())
        },
        SubCommands::TestRefinery(_) => {
            let samples = [
                (111_003_u32, "refinery helmet"),
                (724_440_u32, "intensification refinery"),
            ];
            let mut created = Vec::new();
            for (item_type, label) in samples {
                let Some(item) = me.add_ground_item(state.pool(), item_type, 100, 100).await? else {
                    actor
                        .send(MsgTalk::from_system(me.id(), TalkChannel::System, "Inventory is full."))
                        .await?;
                    break;
                };
                actor.send(MsgItemInfo::inventory_item(item)).await?;
                created.push(format!("{label}={} (uid {})", item.item_type, item.uid));
            }
            actor
                .send(MsgTalk::from_system(
                    me.id(),
                    TalkChannel::System,
                    format!(
                        "Refinery test items created: {}. Equip the helmet and use REFINERY on it.",
                        created.join(", ")
                    ),
                ))
                .await?;
            Ok(())
        },
        SubCommands::TestArtifact(_) => {
            let samples = [
                (111_003_u32, "artifact target helmet"),
                (820_001_u32, "FlameHat artifact"),
                (1_088_001_u32, "Meteor"),
            ];
            let mut created = Vec::new();
            for (item_type, label) in samples {
                let Some(item) = me.add_ground_item(state.pool(), item_type, 100, 100).await? else {
                    actor
                        .send(MsgTalk::from_system(me.id(), TalkChannel::System, "Inventory is full."))
                        .await?;
                    break;
                };
                actor.send(MsgItemInfo::inventory_item(item)).await?;
                created.push(format!("{label}={} (uid {})", item.item_type, item.uid));
            }
            actor
                .send(MsgTalk::from_system(
                    me.id(),
                    TalkChannel::System,
                    format!(
                        "Artifact test items created: {}. Use ARTIFACT on the helmet before equipping it.",
                        created.join(", ")
                    ),
                ))
                .await?;
            Ok(())
        },
        SubCommands::TestDurability(_) => {
            let item_type = 410003;
            let Some(item) = me.add_ground_item(state.pool(), item_type, 10, 100).await? else {
                actor
                    .send(MsgTalk::from_system(me.id(), TalkChannel::System, "Inventory is full."))
                    .await?;
                return Ok(());
            };
            actor.send(MsgItemInfo::inventory_item(item)).await?;
            actor
                .send(MsgTalk::from_system(
                    me.id(),
                    TalkChannel::System,
                    format!(
                        "Created test weapon {} with durability {}/{}.",
                        item.item_type, item.durability, item.max_durability
                    ),
                ))
                .await?;
            Ok(())
        },
        SubCommands::TestGroundItem(info) => {
            let location = me.entity().location();
            let item_id = info.item_type.unwrap_or(1_088_001);
            let item = GroundItem {
                uid: state.next_ground_item_uid(),
                item_id,
                map_id: me.entity().map_id(),
                x: location.x,
                y: location.y,
                owner_id: me.id(),
                silver: 0,
            };
            state.insert_ground_item(item);
            let packet = MsgGroundItem::new(item.uid, item.item_id, item.x, item.y);
            actor.send(packet.clone()).await?;
            actor.screen().send_message(packet).await?;
            actor
                .send(MsgTalk::from_system(
                    me.id(),
                    TalkChannel::System,
                    format!("Created ground test item {} at ({}, {}).", item.item_id, item.x, item.y),
                ))
                .await?;
            Ok(())
        },
        SubCommands::TestConsumables(_) => {
            const TEST_ITEMS: [(u32, &str); 22] = [
                (1_000_000, "HP Stancher"),
                (1_000_010, "HP Resolutive"),
                (1_000_020, "HP Painkiller"),
                (1_000_030, "HP Amrita"),
                (1_001_000, "MP Agrypnotic"),
                (1_001_010, "MP Tonic"),
                (1_001_020, "MP Pearl Ointment"),
                (1_001_030, "MP Recovery Pill"),
                (720_010, "HP Amrita Pack"),
                (720_011, "HP Panacea Pack"),
                (720_012, "HP Ginseng Pack"),
                (720_013, "HP Vanilla Pack"),
                (721_330, "HP Mil. Ginseng Pack"),
                (720_014, "MP Recovery Pill Pack"),
                (720_015, "MP Soul Pill Pack"),
                (720_016, "MP Refreshing Pill Pack"),
                (720_017, "MP Chant Pill Pack"),
                (721_332, "MP Chant Pill Pack II"),
                (723_017, "Double EXP 60m"),
                (1_200_000, "Heaven's Blessing 3d"),
                (1_200_001, "Heaven's Blessing 7d"),
                (1_200_002, "Heaven's Blessing 30d"),
            ];
            let mut created = Vec::new();
            for (item_type, label) in TEST_ITEMS {
                let Some(item) = me.add_ground_item(state.pool(), item_type, 100, 100).await? else {
                    actor
                        .send(MsgTalk::from_system(
                            me.id(),
                            TalkChannel::System,
                            "Inventory is full; test consumable creation stopped.",
                        ))
                        .await?;
                    break;
                };
                actor.send(MsgItemInfo::inventory_item(item)).await?;
                created.push(format!("{}({})", label, item_type));
            }
            actor
                .send(MsgTalk::from_system(
                    me.id(),
                    TalkChannel::System,
                    if created.is_empty() {
                        "No test consumables were created.".to_owned()
                    } else {
                        format!("Created test consumables: {}.", created.join(", "))
                    },
                ))
                .await?;
            Ok(())
        },
        SubCommands::TestMana(_) => {
            let mana = me.entity().set_mana_for_test(100);
            actor.send(MsgUserInfo::from(me)).await?;
            actor
                .send(MsgTalk::from_system(
                    me.id(),
                    TalkChannel::System,
                    format!("Test MP refilled to {}/{} (not persisted).", mana.current, mana.max),
                ))
                .await?;
            Ok(())
        },
        SubCommands::TestTraining(_) => {
            // This command only prepares a local smoke fixture.  It keeps the
            // starter skills, removes previously trained skills, makes the
            // character eligible for the first trainer skill, and persists
            // the fixture before the real NPC dialog is exercised.
            let starter_ids: Vec<u32> = state.skill_catalog().starter_ids().collect();
            let placeholders = (2..starter_ids.len() + 2)
                .map(|n| format!("${n}"))
                .collect::<Vec<_>>()
                .join(", ");
            let query =
                format!("DELETE FROM character_skills WHERE character_id = $1 AND skill_id NOT IN ({placeholders})");
            let mut statement = sqlx::query(&query).bind(me.database_id());
            for skill_id in &starter_ids {
                statement = statement.bind(i64::from(*skill_id));
            }
            statement.execute(state.pool()).await?;
            me.load_skills(state.pool(), state.skill_catalog()).await?;

            if me.entity().level() < 10 {
                me.entity().set_level(10);
                me.entity().restore_health();
            }
            let silver = me.silver();
            if silver < 500 {
                me.grant_silver(state.pool(), 500 - silver as u32).await?;
            }
            me.save(state).await?;
            actor.send(MsgUserInfo::from(me)).await?;
            actor
                .send(MsgTalk::from_system(
                    me.id(),
                    TalkChannel::System,
                    "Trainer smoke fixture prepared: level 10, 500 silver and starter skills only.",
                ))
                .await?;
            Ok(())
        },
        SubCommands::TestHealth(_) => {
            let health = me.entity().restore_health();
            actor.send(MsgUserInfo::from(me)).await?;
            actor
                .send(MsgTalk::from_system(
                    me.id(),
                    TalkChannel::System,
                    format!("Test HP restored to {}/{} (not persisted).", health.current, health.max),
                ))
                .await?;
            Ok(())
        },
        SubCommands::TestStun(_) => {
            me.apply_status(crate::entities::StatusEffect::Stunned, Duration::from_millis(1_500));
            actor.send(MsgPlayer::from(me)).await?;
            actor
                .send(MsgTalk::from_system(
                    me.id(),
                    TalkChannel::System,
                    "Test stun applied for 1500 ms.",
                ))
                .await?;
            Ok(())
        },
        SubCommands::TestSkillBook(_) => {
            let item_type = crate::skills::WHIRLWIND_BOOK_ITEM;
            let Some(item) = me.add_ground_item(state.pool(), item_type, 100, 100).await? else {
                actor
                    .send(MsgTalk::from_system(me.id(), TalkChannel::System, "Inventory is full."))
                    .await?;
                return Ok(());
            };
            actor.send(MsgItemInfo::inventory_item(item)).await?;
            actor
                .send(MsgTalk::from_system(
                    me.id(),
                    TalkChannel::System,
                    format!("Created skill book {} in inventory slot {}.", item_type, item.position),
                ))
                .await?;
            Ok(())
        },
        SubCommands::Skin(info) => {
            let slot = if info.left { 5 } else { 4 };
            let say = |text: String| MsgTalk::from_system(me.id(), TalkChannel::System, text);
            let Some(weapon) = me.equipment_item(slot) else {
                actor.send(say("Wear a weapon in that hand first.".into())).await?;
                return Ok(());
            };
            if info.look != 0
                && (state.shop_catalog().template(info.look).is_none() || !skin_fits(weapon.item_type, info.look))
            {
                actor
                    .send(say(format!("{} cannot dress {}.", info.look, weapon.item_type)))
                    .await?;
                return Ok(());
            }
            if let Some(item) = me.skin_item(state.pool(), weapon.uid, info.look).await? {
                actor.send(MsgItemInfo::equipment_item(item)).await?;
            }
            let look = crate::packets::MsgPlayer::from(me);
            actor.send(look.clone()).await?;
            actor.screen().send_message(look).await?;
            actor
                .send(say(if info.look == 0 {
                    "The weapon wears its own look again.".into()
                } else {
                    format!("The weapon now looks like {}.", info.look)
                }))
                .await?;
            Ok(())
        },
        SubCommands::Proficiency(info) => {
            let level = info.level.clamp(1, 20);
            me.set_weapon_type_proficiency(state.pool(), info.weapon_type, level).await?;
            for packet in crate::packets::MsgWeaponSkill::for_all(me.weapon_proficiencies()) {
                actor.send(packet).await?;
            }
            actor
                .send(MsgTalk::from_system(
                    me.id(),
                    TalkChannel::System,
                    format!("Weapon proficiency {} set to level {level}.", info.weapon_type),
                ))
                .await?;
            Ok(())
        },
        SubCommands::LearnSkill(info) => {
            let Some(skill) = state.skill_catalog().get(info.skill_id) else {
                actor
                    .send(MsgTalk::from_system(
                        me.id(),
                        TalkChannel::System,
                        format!("Unknown skill {}.", info.skill_id),
                    ))
                    .await?;
                return Ok(());
            };
            let level = info.level.unwrap_or(1).max(1);
            me.learn_skill(state.pool(), skill.id, level).await?;
            let progress = me
                .skills()
                .into_iter()
                .find(|progress| progress.skill_id == skill.id)
                .expect("learn_skill inserts the learned skill");
            actor.send(MsgSkill::from(progress)).await?;
            actor
                .send(MsgTalk::from_system(
                    me.id(),
                    TalkChannel::System,
                    format!("Learned {} at level {}.", skill.name, level),
                ))
                .await?;
            Ok(())
        },
        SubCommands::SetLevel(info) => {
            let level = info.level.clamp(1, 130);
            me.change_level(level);
            me.save(state).await?;
            actor.send(MsgUserInfo::from(me)).await?;
            actor.screen().send_message(crate::packets::MsgPlayer::from(me)).await?;
            actor
                .send(MsgTalk::from_system(me.id(), TalkChannel::System, format!("Level set to {level}.")))
                .await?;
            Ok(())
        },
        SubCommands::GuildWar(info) => {
            match info.action.as_str() {
                "start" => crate::guild_war::start(state, true).await,
                "end" => crate::guild_war::end(state).await,
                _ => {
                    actor
                        .send(MsgTalk::from_system(me.id(), TalkChannel::System, "Use $gw start or $gw end."))
                        .await?;
                },
            }
            Ok(())
        },
        SubCommands::SetVirtue(info) => {
            me.set_virtue(info.points);
            me.save(state).await?;
            actor
                .send(MsgTalk::from_system(
                    me.id(),
                    TalkChannel::System,
                    format!("Virtue set to {}.", info.points),
                ))
                .await?;
            Ok(())
        },
        SubCommands::SetPk(info) => {
            me.set_kill_points(info.points);
            me.save(state).await?;
            actor.send(MsgUserInfo::from(me)).await?;
            let update = crate::packets::MsgPlayer::from(me);
            actor.send(update.clone()).await?;
            actor.screen().send_message(update).await?;
            actor
                .send(MsgTalk::from_system(
                    me.id(),
                    TalkChannel::System,
                    format!("PK points set to {}.", info.points),
                ))
                .await?;
            Ok(())
        },
        SubCommands::GiveSilver(info) => {
            let amount = info.amount.unwrap_or(1_000_000).max(1);
            let total = me.grant_silver(state.pool(), amount).await?;
            crate::audit::economic("command_silver", me.id(), 0, 0, amount);
            actor.send(MsgUserInfo::from(me)).await?;
            actor
                .send(MsgTalk::from_system(
                    me.id(),
                    TalkChannel::System,
                    format!("Added {amount} silver. Total: {total}."),
                ))
                .await?;
            Ok(())
        },
        SubCommands::GiveCps(info) => {
            let amount = info.amount.unwrap_or(100_000).max(1);
            let total = me.grant_cps(state.pool(), u64::from(amount)).await?;
            crate::audit::economic("command_cps", me.id(), 0, 0, amount);
            actor.send(MsgUserInfo::from(me)).await?;
            actor
                .send(MsgTalk::from_system(
                    me.id(),
                    TalkChannel::System,
                    format!("Added {amount} CPs. Total: {total}."),
                ))
                .await?;
            Ok(())
        },
        SubCommands::TradeKit(_) => {
            // A few tradable items plus silver and CPs, so two local
            // characters can exercise the whole trade window.
            const KIT_ITEMS: [u32; 4] = [410003, 130003, 150003, 1_000_030];
            const KIT_SILVER: u32 = 1_000_000;
            const KIT_CPS: u32 = 10_000;
            let mut created = 0;
            for item_type in KIT_ITEMS {
                let Some(item) = me.add_ground_item(state.pool(), item_type, 100, 100).await? else {
                    break;
                };
                actor.send(MsgItemInfo::inventory_item(item)).await?;
                created += 1;
            }
            me.grant_silver(state.pool(), KIT_SILVER).await?;
            me.grant_cps(state.pool(), u64::from(KIT_CPS)).await?;
            crate::audit::economic("command_silver", me.id(), 0, 0, KIT_SILVER);
            crate::audit::economic("command_cps", me.id(), 0, 0, KIT_CPS);
            actor.send(MsgUserInfo::from(me)).await?;
            actor
                .send(MsgTalk::from_system(
                    me.id(),
                    TalkChannel::System,
                    format!("Trade kit: {created} items, {KIT_SILVER} silver and {KIT_CPS} CPs added."),
                ))
                .await?;
            Ok(())
        },
        SubCommands::AddAttribute(info) => {
            let Some(kind) = AttributeKind::parse(&info.attribute) else {
                actor
                    .send(MsgTalk::from_system(
                        me.id(),
                        TalkChannel::System,
                        "Unknown attribute. Use strength, agility, vitality or spirit.",
                    ))
                    .await?;
                return Ok(());
            };
            let amount = info.amount.unwrap_or(1).max(1);
            match me.allocate_attribute(state.pool(), kind, amount).await? {
                AttributeAllocation::Allocated {
                    kind,
                    amount,
                    remaining,
                } => {
                    me.refresh_equipment_resources(state.shop_catalog());
                    actor.send(MsgUserInfo::from(me)).await?;
                    actor
                        .send(MsgTalk::from_system(
                            me.id(),
                            TalkChannel::System,
                            format!("Added {amount} point(s) to {}. Remaining: {remaining}.", kind.label()),
                        ))
                        .await?;
                },
                AttributeAllocation::InsufficientPoints { available } => {
                    actor
                        .send(MsgTalk::from_system(
                            me.id(),
                            TalkChannel::System,
                            format!("Not enough attribute points. Available: {available}."),
                        ))
                        .await?;
                },
            }
            Ok(())
        },
        SubCommands::UpgradeItem(info) => {
            let equipped = me.equipment_items().iter().any(|item| item.uid == info.item_uid);
            match me.upgrade_item(state.pool(), info.item_uid).await? {
                ItemUpgradeOutcome::Upgraded(item) => {
                    if equipped {
                        actor.send(MsgItemInfo::equipment_item(item)).await?;
                    } else {
                        actor.send(MsgItemInfo::inventory_item(item)).await?;
                    }
                    actor
                        .send(MsgTalk::from_system(
                            me.id(),
                            TalkChannel::System,
                            format!("Item {} upgraded to +{}.", item.item_type, item.plus),
                        ))
                        .await?;
                },
                ItemUpgradeOutcome::Failed(item) => {
                    actor
                        .send(MsgTalk::from_system(
                            me.id(),
                            TalkChannel::System,
                            format!("Item {} upgrade failed at +{}.", item.item_type, item.plus),
                        ))
                        .await?;
                },
                ItemUpgradeOutcome::MaxLevel(item) => {
                    actor
                        .send(MsgTalk::from_system(
                            me.id(),
                            TalkChannel::System,
                            format!("Item {} is already at maximum +{}.", item.item_type, item.plus),
                        ))
                        .await?;
                },
                ItemUpgradeOutcome::NotFound => {
                    actor
                        .send(MsgTalk::from_system(
                            me.id(),
                            TalkChannel::System,
                            format!("Owned item {} was not found.", info.item_uid),
                        ))
                        .await?;
                },
                ItemUpgradeOutcome::MissingMeteor => {
                    actor
                        .send(MsgTalk::from_system(
                            me.id(),
                            TalkChannel::System,
                            "Upgrade requires a Meteor.",
                        ))
                        .await?;
                },
            }
            Ok(())
        },
        SubCommands::SetItemRarity(info) => {
            let equipped = me.equipment_items().iter().any(|item| item.uid == info.item_uid);
            let Some(item) = me.set_item_rarity(state.pool(), info.item_uid, info.rarity).await? else {
                actor
                    .send(MsgTalk::from_system(
                        me.id(),
                        TalkChannel::System,
                        format!("Owned item {} was not found.", info.item_uid),
                    ))
                    .await?;
                return Ok(());
            };
            if equipped {
                actor.send(MsgItemInfo::equipment_item(item)).await?;
            } else {
                actor.send(MsgItemInfo::inventory_item(item)).await?;
            }
            actor
                .send(MsgTalk::from_system(
                    me.id(),
                    TalkChannel::System,
                    format!("Item {} custom rarity set to {}.", item.item_type, info.rarity),
                ))
                .await?;
            Ok(())
        },
    }
}

/// In Game Commands
#[derive(Debug, Clone, PartialEq, FromArgs)]
struct Command {
    #[argh(subcommand)]
    commands: SubCommands,
}

#[derive(Debug, Clone, PartialEq, FromArgs)]
#[argh(subcommand)]
enum SubCommands {
    Dc(DcCmd),
    Which(WhichCmd),
    QuestStatus(QuestStatusCmd),
    NobilityStatus(NobilityStatusCmd),
    NobilityDonate(NobilityDonateCmd),
    NobilityRanking(NobilityRankingCmd),
    TitleStatus(TitleStatusCmd),
    TitleEquip(TitleEquipCmd),
    CreateGuild(CreateGuildCmd),
    GuildDeposit(GuildDepositCmd),
    GuildWithdraw(GuildWithdrawCmd),
    Teleport(TeleportCmd),
    JumpBack(JumpBackCmd),
    Weather(WeatherCmd),
    GiveItem(GiveItemCmd),
    SetItemRarity(SetItemRarityCmd),
    TradeFixture(TradeFixtureCmd),
    TestEquipment(TestEquipmentCmd),
    TestSockets(TestSocketsCmd),
    TestQuality(TestQualityCmd),
    TestRefinery(TestRefineryCmd),
    TestArtifact(TestArtifactCmd),
    TestDurability(TestDurabilityCmd),
    TestGroundItem(TestGroundItemCmd),
    TestConsumables(TestConsumablesCmd),
    TestMana(TestManaCmd),
    TestTraining(TestTrainingCmd),
    TestHealth(TestHealthCmd),
    TestStun(TestStunCmd),
    Proficiency(ProficiencyCmd),
    Skin(SkinCmd),
    Go(GoCmd),
    LearnSkill(LearnSkillCmd),
    TestSkillBook(TestSkillBookCmd),
    AddAttribute(AddAttributeCmd),
    UpgradeItem(UpgradeItemCmd),
    GiveSilver(GiveSilverCmd),
    GiveCps(GiveCpsCmd),
    TradeKit(TradeKitCmd),
    SetLevel(SetLevelCmd),
    SetPk(SetPkCmd),
    SetVirtue(SetVirtueCmd),
    GuildWar(GuildWarCmd),
    BountyGo(BountyGoCmd),
    BountyKills(BountyKillsCmd),
    BountyPoints(BountyPointsCmd),
    Monster(MonsterCmd),
    MonsterClear(MonsterClearCmd),
    Boss(BossCmd),
    Event(EventCmd),
    Join(JoinCmd),
    Leave(LeaveCmd),
    Xp(XpCmd),
    Stamina(StaminaCmd),
    Lucky(LuckyCmd),
    Rebirths(RebirthsCmd),
}

impl SubCommands {
    /// The commands any player may use; the client sends some of them
    /// itself (titles, nobility). The rest are for game masters.
    fn for_players(&self) -> bool {
        matches!(
            self,
            Self::Dc(_)
                | Self::Which(_)
                | Self::QuestStatus(_)
                | Self::NobilityStatus(_)
                | Self::NobilityDonate(_)
                | Self::NobilityRanking(_)
                | Self::TitleStatus(_)
                | Self::TitleEquip(_)
                | Self::AddAttribute(_)
                | Self::Join(_)
                | Self::Leave(_)
        )
    }
}

/// Start or end the Guild War now (it also runs on Saturdays 20:00-22:00).
#[derive(Debug, Clone, PartialEq, FromArgs)]
#[argh(subcommand, name = "gw")]
struct GuildWarCmd {
    /// start or end
    #[argh(positional)]
    action: String,
}

/// Set the current character's virtue points for local testing.
#[derive(Debug, Clone, PartialEq, FromArgs)]
#[argh(subcommand, name = "virtue")]
struct SetVirtueCmd {
    /// virtue points (Simon asks 2000 for the Labyrinth)
    #[argh(positional)]
    points: u32,
}

/// Set the current character's PK points for local testing.
#[derive(Debug, Clone, PartialEq, FromArgs)]
#[argh(subcommand, name = "pk")]
struct SetPkCmd {
    /// PK points (30 red name, 100 black name)
    #[argh(positional)]
    points: u16,
}

/// Set the current character's level for local testing.
#[derive(Debug, Clone, PartialEq, FromArgs)]
#[argh(subcommand, name = "level")]
struct SetLevelCmd {
    /// new level (1-130)
    #[argh(positional)]
    level: u16,
}

/// Create monsters around you: $monster <name or type id> [amount].
#[derive(Debug, Clone, PartialEq, FromArgs)]
#[argh(subcommand, name = "monster")]
struct MonsterCmd {
    /// the monster's name (as in monster_types) or its type id
    #[argh(positional)]
    monster: String,
    /// how many (default 1, at most 20)
    #[argh(positional)]
    amount: Option<u32>,
}

/// Bring a world boss out now: $boss <monster type> (8300-8318, 3120).
#[derive(Debug, Clone, PartialEq, FromArgs)]
#[argh(subcommand, name = "boss")]
struct BossCmd {
    /// the boss's monster type
    #[argh(positional)]
    id: u32,
}

/// Open an event for signups: $event <tdm|freeze|infection|ctf|lms>.
#[derive(Debug, Clone, PartialEq, FromArgs)]
#[argh(subcommand, name = "event")]
struct EventCmd {
    /// tdm, freeze, infection, ctf or lms
    #[argh(positional)]
    mode: String,
}

/// Fill the XP circle, for testing XP skills.
#[derive(Debug, Clone, PartialEq, FromArgs)]
#[argh(subcommand, name = "xp")]
struct XpCmd {}

/// Set stamina, for testing (default 0, to watch it refill).
#[derive(Debug, Clone, PartialEq, FromArgs)]
#[argh(subcommand, name = "stamina")]
struct StaminaCmd {
    /// stamina (0-150)
    #[argh(positional)]
    amount: Option<u8>,
}

/// Add lucky time, for testing (default 600 seconds).
#[derive(Debug, Clone, PartialEq, FromArgs)]
#[argh(subcommand, name = "lucky")]
struct LuckyCmd {
    /// seconds
    #[argh(positional)]
    seconds: Option<u32>,
}

/// Set the number of rebirths (two learn Bless), for testing.
#[derive(Debug, Clone, PartialEq, FromArgs)]
#[argh(subcommand, name = "rebirths")]
struct RebirthsCmd {
    /// rebirths (0-2)
    #[argh(positional)]
    count: u8,
}

/// Take part in the event open for signups.
#[derive(Debug, Clone, PartialEq, FromArgs)]
#[argh(subcommand, name = "join")]
struct JoinCmd {}

/// Leave the event signups.
#[derive(Debug, Clone, PartialEq, FromArgs)]
#[argh(subcommand, name = "leave")]
struct LeaveCmd {}

/// Remove the monsters created with $monster from this map.
#[derive(Debug, Clone, PartialEq, FromArgs)]
#[argh(subcommand, name = "monster-clear")]
struct MonsterClearCmd {}

/// Go to a player, an NPC or a monster by name: `$go WuxingOven`.
#[derive(Debug, Clone, PartialEq, FromArgs)]
#[argh(subcommand, name = "go")]
struct GoCmd {
    /// the name (or part of it) of a player online, an NPC or a monster
    #[argh(positional, greedy)]
    name: Vec<String>,
}

/// Go to where the monster of your bounty roams.
#[derive(Debug, Clone, PartialEq, FromArgs)]
#[argh(subcommand, name = "bounty-go")]
struct BountyGoCmd {}

/// Count kills toward your bounty, for testing.
#[derive(Debug, Clone, PartialEq, FromArgs)]
#[argh(subcommand, name = "bounty-kills")]
struct BountyKillsCmd {
    /// kills to count (default 10)
    #[argh(positional)]
    amount: Option<u32>,
}

/// Add bounty points, for testing the talisman.
#[derive(Debug, Clone, PartialEq, FromArgs)]
#[argh(subcommand, name = "bounty-points")]
struct BountyPointsCmd {
    /// points to add (default 100)
    #[argh(positional)]
    amount: Option<u32>,
}

/// Add silver to the current character for local testing.
#[derive(Debug, Clone, PartialEq, FromArgs)]
#[argh(subcommand, name = "silver")]
struct GiveSilverCmd {
    /// amount of silver to add (default 1000000)
    #[argh(positional)]
    amount: Option<u32>,
}

/// Add Conquer Points to the current character for local testing.
#[derive(Debug, Clone, PartialEq, FromArgs)]
#[argh(subcommand, name = "cps")]
struct GiveCpsCmd {
    /// amount of CPs to add (default 100000)
    #[argh(positional)]
    amount: Option<u32>,
}

/// Create a few tradable items and add silver and CPs, for trade tests.
#[derive(Debug, Clone, PartialEq, FromArgs)]
#[argh(subcommand, name = "trade-kit")]
struct TradeKitCmd {}

/// Disconnect From Server
#[derive(Debug, Clone, PartialEq, FromArgs)]
#[argh(subcommand, name = "dc")]
struct DcCmd {}

/// Jump Back to prev location
#[derive(Debug, Clone, PartialEq, FromArgs)]
#[argh(subcommand, name = "jump-back")]
struct JumpBackCmd {}

/// Ask about things in your environment
#[derive(Debug, Clone, PartialEq, FromArgs)]
#[argh(subcommand, name = "which")]
struct WhichCmd {
    /// get your current map (ID, Name)
    #[argh(switch)]
    map: bool,
}

/// Show active and completed quests for the current character.
#[derive(Debug, Clone, PartialEq, FromArgs)]
#[argh(subcommand, name = "quest-status")]
struct QuestStatusCmd {}

/// Show the current server-authoritative nobility status.
#[derive(Debug, Clone, PartialEq, FromArgs)]
#[argh(subcommand, name = "nobility-status")]
struct NobilityStatusCmd {}

/// Donate silver to the character's persistent nobility score.
#[derive(Debug, Clone, PartialEq, FromArgs)]
#[argh(subcommand, name = "nobility-donate")]
struct NobilityDonateCmd {
    /// amount of silver to donate
    #[argh(positional)]
    amount: u32,
}

/// Show the top ten characters by persistent nobility donation.
#[derive(Debug, Clone, PartialEq, FromArgs)]
#[argh(subcommand, name = "nobility-ranking")]
struct NobilityRankingCmd {}

/// Show the currently equipped title.
#[derive(Debug, Clone, PartialEq, FromArgs)]
#[argh(subcommand, name = "title-status")]
struct TitleStatusCmd {}

/// Equip a title by catalog id. Use id 0 to clear the equipped title.
#[derive(Debug, Clone, PartialEq, FromArgs)]
#[argh(subcommand, name = "title-equip")]
struct TitleEquipCmd {
    /// catalog title id, or 0 to clear
    #[argh(positional)]
    title_id: u32,
}

/// Create and persist a guild for local guild-system testing.
#[derive(Debug, Clone, PartialEq, FromArgs)]
#[argh(subcommand, name = "create-guild")]
struct CreateGuildCmd {
    /// guild name
    #[argh(positional)]
    name: String,
}

/// Move an inventory item into the current guild warehouse for local testing.
#[derive(Debug, Clone, PartialEq, FromArgs)]
#[argh(subcommand, name = "guild-deposit")]
struct GuildDepositCmd {
    /// inventory item UID
    #[argh(positional)]
    item_uid: u32,
    /// guild warehouse slot (0..39)
    #[argh(positional)]
    position: u8,
}

/// Move a guild warehouse item into the current inventory for local testing.
#[derive(Debug, Clone, PartialEq, FromArgs)]
#[argh(subcommand, name = "guild-withdraw")]
struct GuildWithdrawCmd {
    /// guild warehouse item UID
    #[argh(positional)]
    item_uid: u32,
    /// inventory slot (0..39)
    #[argh(positional)]
    position: u8,
}

/// Teleport to other map at specific location
#[derive(Debug, Clone, PartialEq, FromArgs)]
#[argh(subcommand, name = "tele")]
struct TeleportCmd {
    #[argh(positional)]
    map_id: u32,
    #[argh(positional)]
    x: u16,
    #[argh(positional)]
    y: u16,
    /// bring every player online along
    #[argh(switch)]
    all: bool,
}

/// Change the current map's weather
#[derive(Debug, Clone, PartialEq, FromArgs)]
#[argh(subcommand, name = "weather")]
struct WeatherCmd {
    /// weather type
    #[argh(positional)]
    kind: u32,
}

/// Create one item in the current character's inventory.
#[derive(Debug, Clone, PartialEq, FromArgs)]
#[argh(subcommand, name = "item")]
struct GiveItemCmd {
    /// classic item type ID
    #[argh(positional)]
    item_type: u32,
    /// custom rarity: 0 common, 1 rare, 2 epic, 3 legendary, 4 mythic
    #[argh(option, default = "0")]
    rarity: u8,
    /// how many to create (up to 40)
    #[argh(option, default = "1")]
    count: u8,
}

/// Change the custom rarity of an owned inventory, equipment, or warehouse
/// item.
#[derive(Debug, Clone, PartialEq, FromArgs)]
#[argh(subcommand, name = "rarity-item")]
struct SetItemRarityCmd {
    /// owned item UID
    #[argh(positional)]
    item_uid: u32,
    /// custom rarity: 0 common, 1 rare, 2 epic, 3 legendary, 4 mythic
    #[argh(positional)]
    rarity: u8,
}

/// Re-send one existing 410003 item without changing inventory, for trade smoke
/// tests.
#[derive(Debug, Clone, PartialEq, FromArgs)]
#[argh(subcommand, name = "trade-fixture")]
struct TradeFixtureCmd {}

/// Create one sample item for every usable paper-doll slot.
#[derive(Debug, Clone, PartialEq, FromArgs)]
#[argh(subcommand, name = "test-equipment")]
struct TestEquipmentCmd {}

/// Create a weapon, one gem and DragonBalls for socket-flow testing.
#[derive(Debug, Clone, PartialEq, FromArgs)]
#[argh(subcommand, name = "test-sockets")]
struct TestSocketsCmd {}

/// Create a normal-quality weapon and DragonBall for quality-upgrade testing.
#[derive(Debug, Clone, PartialEq, FromArgs)]
#[argh(subcommand, name = "test-quality")]
struct TestQualityCmd {}

/// Create a weapon and a compatible refinery material for local testing.
#[derive(Debug, Clone, PartialEq, FromArgs)]
#[argh(subcommand, name = "test-refinery")]
struct TestRefineryCmd {}

/// Create an equipment item, Dragon Soul artifact and Meteor for local testing.
#[derive(Debug, Clone, PartialEq, FromArgs)]
#[argh(subcommand, name = "test-artifact")]
struct TestArtifactCmd {}

/// Create a weapon with low durability for combat and repair testing.
#[derive(Debug, Clone, PartialEq, FromArgs)]
#[argh(subcommand, name = "test-durability")]
struct TestDurabilityCmd {}

/// Create an owned ground item at the current position for pickup testing.
#[derive(Debug, Clone, PartialEq, FromArgs)]
#[argh(subcommand, name = "test-ground-item")]
struct TestGroundItemCmd {
    /// classic item type ID; defaults to Meteor
    #[argh(positional)]
    item_type: Option<u32>,
}

/// Create one sample HP and MP potion of each supported basic tier.
#[derive(Debug, Clone, PartialEq, FromArgs)]
#[argh(subcommand, name = "test-consumables")]
struct TestConsumablesCmd {}

/// Refill MP for local skill testing without persisting the override.
#[derive(Debug, Clone, PartialEq, FromArgs)]
#[argh(subcommand, name = "test-mana")]
struct TestManaCmd {}

/// Prepare a fresh, eligible character for the positive trainer smoke.
#[derive(Debug, Clone, PartialEq, FromArgs)]
#[argh(subcommand, name = "test-training")]
struct TestTrainingCmd {}

/// Restore HP for local combat testing without persisting the override.
#[derive(Debug, Clone, PartialEq, FromArgs)]
#[argh(subcommand, name = "test-health")]
struct TestHealthCmd {}

/// Apply the temporary stun status for local combat testing.
#[derive(Debug, Clone, PartialEq, FromArgs)]
#[argh(subcommand, name = "test-stun")]
struct TestStunCmd {}

/// Dress the worn weapon in another weapon's look: `$skin 410339` (the
/// right hand; `--left` the left), `$skin 0` takes the skin off.
#[derive(Debug, Clone, PartialEq, FromArgs)]
#[argh(subcommand, name = "skin")]
struct SkinCmd {
    /// the item type whose look the weapon wears; 0 for its own
    #[argh(positional)]
    look: u32,
    /// the weapon in the left hand
    #[argh(switch)]
    left: bool,
}

/// Whether `look` may dress `weapon`: one-handed over one-handed, two-handed
/// over two-handed, bows over bows and shields over shields.
pub(crate) fn skin_fits(weapon: u32, look: u32) -> bool {
    let kind = |item: u32| match item / 1_000 {
        500 => Some(2),
        900 => Some(3),
        410..=499 => Some(0),
        501..=599 => Some(1),
        _ => None,
    };
    kind(weapon).is_some() && kind(weapon) == kind(look)
}

/// Set the proficiency of a kind of weapon: `$proficiency 410 12` (blades;
/// 420 swords, 480 clubs, 500 bows, 421 backswords, 561 wands...).
#[derive(Debug, Clone, PartialEq, FromArgs)]
#[argh(subcommand, name = "proficiency")]
struct ProficiencyCmd {
    /// the weapon kind: the item ID's first three digits (410 blades)
    #[argh(positional)]
    weapon_type: u32,
    /// the level, 1 to 20
    #[argh(positional)]
    level: u16,
}

/// Learn a catalog skill for local progression tests.
#[derive(Debug, Clone, PartialEq, FromArgs)]
#[argh(subcommand, name = "learn-skill")]
struct LearnSkillCmd {
    /// catalog skill ID
    #[argh(positional)]
    skill_id: u32,
    /// initial skill level
    #[argh(positional)]
    level: Option<u16>,
}

/// Create the current skill book for local end-to-end testing.
#[derive(Debug, Clone, PartialEq, FromArgs)]
#[argh(subcommand, name = "test-skill-book")]
struct TestSkillBookCmd {}

/// Spend available base-attribute points.
#[derive(Debug, Clone, PartialEq, FromArgs)]
#[argh(subcommand, name = "add-stat")]
struct AddAttributeCmd {
    /// strength/str, agility/agi, vitality/vit or spirit/spi
    #[argh(positional)]
    attribute: String,
    /// number of points to spend
    #[argh(positional)]
    amount: Option<u16>,
}

/// Increase an owned item by one classic plus level for local progression
/// tests.
#[derive(Debug, Clone, PartialEq, FromArgs)]
#[argh(subcommand, name = "upgrade-item")]
struct UpgradeItemCmd {
    /// inventory or equipment item UID
    #[argh(positional)]
    item_uid: u32,
}

#[cfg(test)]
mod tests {
    use super::{Command, SubCommands};
    use argh::FromArgs;

    #[test]
    fn teleport_takes_everyone_along_with_a_switch() {
        let command = Command::from_args(&["commands"], &["tele", "1002", "430", "380", "--all"]).expect("tele --all");
        assert_eq!(
            command.commands,
            SubCommands::Teleport(super::TeleportCmd { map_id: 1002, x: 430, y: 380, all: true })
        );
        let command = Command::from_args(&["commands"], &["tele", "1002", "430", "380"]).expect("tele");
        assert_eq!(
            command.commands,
            SubCommands::Teleport(super::TeleportCmd { map_id: 1002, x: 430, y: 380, all: false })
        );
    }

    #[tokio::test]
    async fn only_game_masters_run_game_master_commands() -> Result<(), crate::Error> {
        use tq_network::Actor;
        let state = crate::State::with_pool(tq_db::testing::pool().await).await?;
        let (tx, mut replies) = tokio::sync::mpsc::channel(64);
        let actor = Actor::<crate::ActorState>::new(tx);
        actor.set_id(1);
        let inner = crate::packets::MsgRegister::build_character_with(
            "gm-test".to_owned(),
            crate::packets::BodyType::MuscularMale,
            crate::packets::BaseClass::Trojan,
            1,
            1,
        )?;
        inner.save(state.pool()).await?;
        let inner = tq_db::character::Character::from_account(state.pool(), 1)
            .await?
            .expect("character");
        let screen = crate::systems::Screen::new(actor.handle());
        actor.update(crate::entities::Character::new(actor.handle(), inner), screen);
        let silver = || actor.entity().as_character().expect("character").silver();
        let before = silver();

        super::parse_and_execute(&state, &actor, &["silver", "5000"]).await?;
        assert_eq!(silver(), before, "a player gets nothing");
        super::parse_and_execute(&state, &actor, &["quest-status"]).await?;

        actor.set_gm(true);
        super::parse_and_execute(&state, &actor, &["silver", "5000"]).await?;
        assert_eq!(silver(), before + 5000, "a game master gets the silver");
        while replies.try_recv().is_ok() {}
        Ok(())
    }

    #[test]
    fn parses_learn_skill_command_with_optional_level() {
        let command =
            Command::from_args(&["commands"], &["learn-skill", "1001", "3"]).expect("learn-skill command should parse");
        assert_eq!(
            command.commands,
            SubCommands::LearnSkill(super::LearnSkillCmd {
                skill_id: 1001,
                level: Some(3),
            })
        );
    }

    #[test]
    fn parses_durability_test_command() {
        let command =
            Command::from_args(&["commands"], &["test-durability"]).expect("test-durability command should parse");
        assert_eq!(
            command.commands,
            SubCommands::TestDurability(super::TestDurabilityCmd {})
        );
    }

    #[test]
    fn parses_trade_fixture_command() {
        let command =
            Command::from_args(&["commands"], &["trade-fixture"]).expect("trade fixture command should parse");
        assert_eq!(command.commands, SubCommands::TradeFixture(super::TradeFixtureCmd {}));
    }

    #[test]
    fn parses_trade_kit_and_cps_commands() {
        let command = Command::from_args(&["commands"], &["trade-kit"]).expect("trade-kit command should parse");
        assert_eq!(command.commands, SubCommands::TradeKit(super::TradeKitCmd {}));
        let command = Command::from_args(&["commands"], &["cps", "500"]).expect("cps command should parse");
        assert_eq!(
            command.commands,
            SubCommands::GiveCps(super::GiveCpsCmd { amount: Some(500) })
        );
    }

    #[test]
    fn parses_health_test_command() {
        let command = Command::from_args(&["commands"], &["test-health"]).expect("test-health command should parse");
        assert_eq!(command.commands, SubCommands::TestHealth(super::TestHealthCmd {}));
    }

    #[test]
    fn parses_training_fixture_command() {
        let command =
            Command::from_args(&["commands"], &["test-training"]).expect("test-training command should parse");
        assert_eq!(command.commands, SubCommands::TestTraining(super::TestTrainingCmd {}));
    }

    #[test]
    fn parses_stun_test_command() {
        let command = Command::from_args(&["commands"], &["test-stun"]).expect("test-stun command should parse");
        assert_eq!(command.commands, SubCommands::TestStun(super::TestStunCmd {}));
    }

    #[test]
    fn parses_quest_status_command() {
        let command = Command::from_args(&["commands"], &["quest-status"]).expect("quest-status command should parse");
        assert_eq!(command.commands, SubCommands::QuestStatus(super::QuestStatusCmd {}));
    }

    #[test]
    fn parses_nobility_commands() {
        let status =
            Command::from_args(&["commands"], &["nobility-status"]).expect("nobility status command should parse");
        assert_eq!(
            status.commands,
            SubCommands::NobilityStatus(super::NobilityStatusCmd {})
        );
        let donate = Command::from_args(&["commands"], &["nobility-donate", "3000000"])
            .expect("nobility donation command should parse");
        assert_eq!(
            donate.commands,
            SubCommands::NobilityDonate(super::NobilityDonateCmd { amount: 3_000_000 })
        );
        let ranking =
            Command::from_args(&["commands"], &["nobility-ranking"]).expect("nobility ranking command should parse");
        assert_eq!(
            ranking.commands,
            SubCommands::NobilityRanking(super::NobilityRankingCmd {})
        );
        let title_status =
            Command::from_args(&["commands"], &["title-status"]).expect("title status command should parse");
        assert_eq!(
            title_status.commands,
            SubCommands::TitleStatus(super::TitleStatusCmd {})
        );
        let title_equip =
            Command::from_args(&["commands"], &["title-equip", "2"]).expect("title equip command should parse");
        assert_eq!(
            title_equip.commands,
            SubCommands::TitleEquip(super::TitleEquipCmd { title_id: 2 })
        );
    }

    #[test]
    fn parses_create_guild_command() {
        let command =
            Command::from_args(&["commands"], &["create-guild", "Knights"]).expect("create-guild command should parse");
        assert_eq!(
            command.commands,
            SubCommands::CreateGuild(super::CreateGuildCmd {
                name: "Knights".to_owned(),
            })
        );
    }

    #[test]
    fn parses_guild_warehouse_commands() {
        let deposit = Command::from_args(&["commands"], &["guild-deposit", "1234", "7"])
            .expect("guild-deposit command should parse");
        assert_eq!(
            deposit.commands,
            SubCommands::GuildDeposit(super::GuildDepositCmd {
                item_uid: 1234,
                position: 7,
            })
        );
        let withdraw = Command::from_args(&["commands"], &["guild-withdraw", "1234", "2"])
            .expect("guild-withdraw command should parse");
        assert_eq!(
            withdraw.commands,
            SubCommands::GuildWithdraw(super::GuildWithdrawCmd {
                item_uid: 1234,
                position: 2,
            })
        );
    }

    #[test]
    fn parses_quality_test_command() {
        let command = Command::from_args(&["commands"], &["test-quality"]).expect("test-quality command should parse");
        assert_eq!(command.commands, SubCommands::TestQuality(super::TestQualityCmd {}));
    }

    #[test]
    fn parses_refinery_test_command() {
        let command =
            Command::from_args(&["commands"], &["test-refinery"]).expect("test-refinery command should parse");
        assert_eq!(command.commands, SubCommands::TestRefinery(super::TestRefineryCmd {}));
    }

    #[test]
    fn parses_artifact_test_command() {
        let command =
            Command::from_args(&["commands"], &["test-artifact"]).expect("test-artifact command should parse");
        assert_eq!(command.commands, SubCommands::TestArtifact(super::TestArtifactCmd {}));
    }

    #[test]
    fn parses_level_command() {
        let command = Command::from_args(&["commands"], &["level", "70"]).expect("level command should parse");
        assert_eq!(command.commands, SubCommands::SetLevel(super::SetLevelCmd { level: 70 }));
    }

    #[test]
    fn parses_silver_command_with_optional_amount() {
        let command = Command::from_args(&["commands"], &["silver", "5000"]).expect("silver command should parse");
        assert_eq!(
            command.commands,
            SubCommands::GiveSilver(super::GiveSilverCmd { amount: Some(5_000) })
        );
        let command = Command::from_args(&["commands"], &["silver"]).expect("silver without amount should parse");
        assert_eq!(command.commands, SubCommands::GiveSilver(super::GiveSilverCmd { amount: None }));
    }

    #[test]
    fn skins_dress_weapons_of_their_kind() {
        let command = Command::from_args(&["commands"], &["skin", "410339", "--left"]).expect("skin command should parse");
        assert_eq!(command.commands, SubCommands::Skin(super::SkinCmd { look: 410_339, left: true }));
        assert!(super::skin_fits(410_039, 420_339), "a blade wears a sword");
        assert!(super::skin_fits(560_019, 510_339), "a spear wears a glaive");
        assert!(super::skin_fits(500_009, 500_339), "a bow wears a bow");
        assert!(!super::skin_fits(500_009, 560_339), "a bow is no spear");
        assert!(!super::skin_fits(410_039, 560_339), "one hand is not two");
        assert!(!super::skin_fits(130_009, 130_309), "armor has no skins");
    }

    #[test]
    fn parses_attribute_command_with_optional_amount() {
        let command =
            Command::from_args(&["commands"], &["add-stat", "vitality", "3"]).expect("add-stat command should parse");
        assert_eq!(
            command.commands,
            SubCommands::AddAttribute(super::AddAttributeCmd {
                attribute: "vitality".to_owned(),
                amount: Some(3),
            })
        );
    }

    #[test]
    fn parses_upgrade_item_command() {
        let command =
            Command::from_args(&["commands"], &["upgrade-item", "1234"]).expect("upgrade-item command should parse");
        assert_eq!(
            command.commands,
            SubCommands::UpgradeItem(super::UpgradeItemCmd { item_uid: 1234 })
        );
    }

    #[test]
    fn parses_go_command() {
        let command = Command::from_args(&["commands"], &["go", "Wuxing", "Oven"]).expect("go");
        assert_eq!(
            command.commands,
            SubCommands::Go(super::GoCmd { name: vec!["Wuxing".into(), "Oven".into()] })
        );
    }

    #[test]
    fn parses_proficiency_command() {
        let command = Command::from_args(&["commands"], &["proficiency", "410", "12"]).expect("proficiency");
        assert_eq!(
            command.commands,
            SubCommands::Proficiency(super::ProficiencyCmd { weapon_type: 410, level: 12 })
        );
    }

    #[test]
    fn parses_custom_rarity_command() {
        let command =
            Command::from_args(&["commands"], &["rarity-item", "1234", "3"]).expect("rarity-item command should parse");
        assert_eq!(
            command.commands,
            SubCommands::SetItemRarity(super::SetItemRarityCmd {
                item_uid: 1234,
                rarity: 3,
            })
        );
    }

    #[test]
    fn parses_item_command_with_custom_rarity() {
        let command = Command::from_args(&["commands"], &["item", "410003", "--rarity", "3"])
            .expect("item command with rarity should parse");
        assert_eq!(
            command.commands,
            SubCommands::GiveItem(super::GiveItemCmd {
                item_type: 410003,
                rarity: 3,
                count: 1,
            })
        );
    }

    #[test]
    fn parses_ground_item_command_with_optional_item_type() {
        let command = Command::from_args(&["commands"], &["test-ground-item", "1088001"])
            .expect("test-ground-item command should parse");
        assert_eq!(
            command.commands,
            SubCommands::TestGroundItem(super::TestGroundItemCmd {
                item_type: Some(1_088_001),
            })
        );
    }
}
