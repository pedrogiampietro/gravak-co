//! The Guild System window (packets 9170-9172). Without a guild: what the
//! system is, the guilds to ask to join (and the invitations received), and
//! founding one. In a guild: its overview, members, requests and
//! invitations, history, fund, wars, and its members' deaths and frags.
//!
//! Membership itself stays with [`crate::guild::GuildManager`]; what the
//! window adds (title, motto, description, entry fee, requests, history,
//! wars, deaths, level) lives in the tables of migration 81.

use tq_network::Actor;

use crate::constants::CHARACTER_ID_MIN;
use crate::entities::Character;
use crate::guild::{database_id, GuildInfo, DEPUTY_ROLE, LEADER_ROLE, MAX_GUILD_MEMBERS, MEMBER_ROLE};
use crate::packets::{MsgGuild, MsgGuildPanel, MsgGuildPanelInfo, MsgGuildPanelRow, MsgTalk, MsgUserInfo, TalkChannel, SET_GUILD};
use crate::{ActorState, Error, State};

/// Client requests.
pub const OPEN: u32 = 1;
/// A page of a list: `param` the list, `amount` the page, `text1` a filter
/// and `text2` a name searched (the lists say which).
pub const PAGE: u32 = 2;
pub const CREATE: u32 = 3;
pub const REQUEST_JOIN: u32 = 4;
pub const CANCEL_REQUEST: u32 = 5;
pub const ACCEPT_REQUEST: u32 = 6;
pub const REFUSE_REQUEST: u32 = 7;
pub const INVITE: u32 = 8;
pub const CANCEL_INVITE: u32 = 9;
pub const ACCEPT_INVITE: u32 = 10;
pub const REFUSE_INVITE: u32 = 11;
pub const LEAVE: u32 = 12;
pub const KICK: u32 = 13;
pub const PROMOTE: u32 = 14;
pub const DEMOTE: u32 = 15;
pub const TRANSFER: u32 = 16;
pub const DEPOSIT: u32 = 17;
pub const WITHDRAW: u32 = 18;
pub const SETTINGS: u32 = 19;
pub const DECLARE_WAR: u32 = 20;
pub const ANSWER_WAR: u32 = 21;
pub const END_WAR: u32 = 22;
pub const DISBAND: u32 = 23;
/// The leader's new crest in `param` (emblem, field << 8, colour << 16).
pub const EMBLEM: u32 = 24;

/// The lists.
pub const MEMBERS: u32 = 1;
pub const REQUESTS: u32 = 2;
pub const INVITES: u32 = 3;
pub const EVENTS: u32 = 4;
pub const BALANCE: u32 = 5;
pub const WARS: u32 = 6;
pub const WAR_KILLS: u32 = 7;
pub const DEATHS: u32 = 8;
pub const SEARCH: u32 = 9;
pub const MY_INVITES: u32 = 10;
pub const MY_REQUESTS: u32 = 11;

/// War states as the window shows them.
pub const WAR_INCOMING: u32 = 0;
pub const WAR_OUTGOING: u32 = 1;
pub const WAR_ACTIVE: u32 = 2;
pub const WAR_REFUSED: u32 = 3;
pub const WAR_ENDED: u32 = 4;

/// Rows on a page.
pub const PAGE_ROWS: i64 = 8;
/// Requests and invitations last a week; the history 90 days; deaths 30.
pub const EXPIRY_SECS: i64 = 7 * 24 * 60 * 60;
pub const EVENTS_KEPT_SECS: i64 = 90 * 24 * 60 * 60;
pub const DEATHS_KEPT_SECS: i64 = 30 * 24 * 60 * 60;
/// Founding a guild, as at the GuildDirector.
pub const FOUND_LEVEL: u16 = 90;
pub const FOUND_PRICE: u32 = 1_000_000;
/// The most a guild asks of a new member.
pub const MAX_ENTRY_FEE: u32 = 10_000_000;
/// The crest's parts: emblems, field colours and emblem colours the client
/// draws (`guild_emblems.rs`).
pub const EMBLEMS: u32 = 36;
pub const FIELD_COLOURS: u32 = 10;
pub const EMBLEM_COLOURS: u32 = 6;

/// `emblem` with each part brought within what the client draws.
pub fn valid_emblem(emblem: u32) -> u32 {
    let icon = (emblem & 0xff) % EMBLEMS;
    let field = ((emblem >> 8) & 0xff) % FIELD_COLOURS;
    let colour = ((emblem >> 16) & 0xff) % EMBLEM_COLOURS;
    icon | field << 8 | colour << 16
}

/// Text lengths, in characters.
pub const TITLE_CHARS: usize = 30;
pub const MOTTO_CHARS: usize = 60;
pub const DESCRIPTION_CHARS: usize = 250;
pub const MESSAGE_CHARS: usize = 120;
/// A guild earns this share (in hundredths) of the experience its members
/// earn from monsters.
pub const EXPERIENCE_SHARE: u64 = 1;
/// Level `n` needs this many times `n * n` experience.
pub const LEVEL_STEP: u64 = 100_000;

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| value.as_secs() as i64)
        .unwrap_or_default()
}

fn entity_id(database_id: i64) -> u32 {
    (database_id as u32).saturating_add(CHARACTER_ID_MIN)
}

/// `text` trimmed and cut to `chars` characters.
fn clip(text: &str, chars: usize) -> String {
    text.trim().chars().take(chars).collect()
}

/// A guild's level from its experience: (level, into it, its span).
pub fn level_of(experience: u64) -> (u32, u64, u64) {
    let mut level = 0_u64;
    while LEVEL_STEP * (level + 1) * (level + 1) <= experience {
        level += 1;
    }
    let floor = LEVEL_STEP * level * level;
    let next = LEVEL_STEP * (level + 1) * (level + 1);
    (level as u32, experience - floor, next - floor)
}

pub fn class_name(class: u8) -> &'static str {
    match class {
        10..=15 => "Trojan",
        20..=25 => "Warrior",
        40..=45 => "Archer",
        132..=135 => "Water Taoist",
        142..=145 => "Fire Taoist",
        100..=101 => "Taoist",
        _ => "Novice",
    }
}

fn rank_allows(role: u32, least: u32) -> bool {
    role >= least
}

/// A line in the guild's history.
pub async fn log(
    pool: &sqlx::PgPool,
    guild_id: u32,
    kind: &str,
    actor: &str,
    target: &str,
    amount: i64,
    balance: i64,
) -> Result<(), Error> {
    sqlx::query(
        "INSERT INTO guild_events (guild_id, kind, actor, target, amount, balance) VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(i64::from(guild_id))
    .bind(kind)
    .bind(actor)
    .bind(target)
    .bind(amount)
    .bind(balance)
    .execute(pool)
    .await?;
    Ok(())
}

/// A member's guild earns its share of the experience they won.
pub async fn add_experience(state: &State, character: &Character, experience: u64) -> Result<(), Error> {
    let Some(guild) = state.guild_manager().guild_for(character.id()) else {
        return Ok(());
    };
    let share = experience * EXPERIENCE_SHARE / 100;
    if share == 0 {
        return Ok(());
    }
    sqlx::query("UPDATE guilds SET experience = experience + $1 WHERE guild_id = $2")
        .bind(i64::try_from(share).unwrap_or(i64::MAX))
        .bind(i64::from(guild.id))
        .execute(state.pool())
        .await?;
    Ok(())
}

/// A death for the guilds' records: the victim's guild a death, the
/// killer's a frag, and a kill of the war if theirs are at war.
pub async fn record_death(state: &State, victim: &Character, killer_id: u32, killer: Option<&Character>) -> Result<(), Error> {
    let manager = state.guild_manager();
    let victim_guild = manager.guild_for(victim.id());
    let killer_guild = killer.and_then(|killer| manager.guild_for(killer.id()));
    if victim_guild.is_none() && killer_guild.is_none() {
        return Ok(());
    }
    let killer_name = match killer {
        Some(killer) => killer.entity().name().to_owned(),
        None => state
            .try_map(victim.entity().map_id())
            .ok()
            .and_then(|map| map.monster_entity(killer_id))
            .map_or_else(|| "Unknown".to_owned(), |monster| monster.basic().name().to_owned()),
    };
    let killer_guild_name = killer_guild.as_ref().map_or(String::new(), |guild| guild.name.clone());
    let pool = state.pool();
    let level = i32::from(victim.entity().level());
    for (guild, kind) in [(&victim_guild, "death"), (&killer_guild, "frag")] {
        let Some(guild) = guild else {
            continue;
        };
        sqlx::query(
            "INSERT INTO guild_deaths (guild_id, kind, victim, victim_level, killer, killer_guild, by_player)
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(i64::from(guild.id))
        .bind(kind)
        .bind(victim.entity().name())
        .bind(level)
        .bind(&killer_name)
        .bind(&killer_guild_name)
        .bind(killer.is_some())
        .execute(pool)
        .await?;
        sqlx::query("DELETE FROM guild_deaths WHERE guild_id = $1 AND created_at < $2")
            .bind(i64::from(guild.id))
            .bind(now() - DEATHS_KEPT_SECS)
            .execute(pool)
            .await?;
    }
    if let (Some(ours), Some(theirs)) = (&killer_guild, &victim_guild) {
        let war: Option<i64> = sqlx::query_scalar(
            "SELECT war_id FROM guild_wars WHERE state = 'active'
               AND ((guild_id = $1 AND enemy_id = $2) OR (guild_id = $2 AND enemy_id = $1))
             LIMIT 1",
        )
        .bind(i64::from(ours.id))
        .bind(i64::from(theirs.id))
        .fetch_optional(pool)
        .await?;
        if let Some(war) = war {
            sqlx::query("INSERT INTO guild_war_kills (war_id, killer_guild_id, killer, victim) VALUES ($1, $2, $3, $4)")
                .bind(war)
                .bind(i64::from(ours.id))
                .bind(&killer_name)
                .bind(victim.entity().name())
                .execute(pool)
                .await?;
        }
    }
    Ok(())
}

/// What a guild says of itself, from its row.
struct Profile {
    emblem: i64,
    title: String,
    motto: String,
    description: String,
    entry_fee: i64,
    accepting: bool,
    experience: i64,
    silver: i64,
    created_at: i64,
}

async fn profile(pool: &sqlx::PgPool, guild_id: u32) -> Result<Option<Profile>, Error> {
    let row: Option<(i64, String, String, String, i64, bool, i64, i64, i64)> = sqlx::query_as(
        "SELECT emblem, title, motto, description, entry_fee, accepting, experience, silver, created_at FROM guilds WHERE guild_id = $1",
    )
    .bind(i64::from(guild_id))
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|(emblem, title, motto, description, entry_fee, accepting, experience, silver, created_at)| Profile {
        emblem,
        title,
        motto,
        description,
        entry_fee,
        accepting,
        experience,
        silver,
        created_at,
    }))
}

async fn name_of(pool: &sqlx::PgPool, entity: u32) -> Result<String, Error> {
    Ok(sqlx::query_scalar::<_, String>("SELECT name FROM characters WHERE character_id = $1")
        .bind(database_id(entity))
        .fetch_optional(pool)
        .await?
        .unwrap_or_default())
}

fn online(state: &State, entity: u32) -> bool {
    state.entity(entity).is_some_and(|entity| entity.is_character())
}

fn row(list: u32, index: u32) -> MsgGuildPanelRow {
    MsgGuildPanelRow {
        list,
        index,
        ..Default::default()
    }
}

/// The window's header for `character`, with `note` from the last request.
async fn info(state: &State, character: &Character, note: &str) -> Result<MsgGuildPanelInfo, Error> {
    let pool = state.pool();
    let mut info = MsgGuildPanelInfo {
        silver: character.silver(),
        hero_level: u32::from(character.entity().level()),
        max_members: MAX_GUILD_MEMBERS as u32,
        note: note.to_owned(),
        ..Default::default()
    };
    let (guilds, accepting): (i64, i64) =
        sqlx::query_as("SELECT COUNT(*), COUNT(*) FILTER (WHERE accepting) FROM guilds")
            .fetch_one(pool)
            .await?;
    info.guilds = u32::try_from(guilds).unwrap_or_default();
    info.guilds_accepting = u32::try_from(accepting).unwrap_or_default();
    let Some(guild) = state.guild_manager().guild_for(character.id()) else {
        return Ok(info);
    };
    let Some(profile) = profile(pool, guild.id).await? else {
        return Ok(info);
    };
    let (level, into, span) = level_of(u64::try_from(profile.experience).unwrap_or_default());
    let pending: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM guild_requests WHERE guild_id = $1 AND created_at > $2")
        .bind(i64::from(guild.id))
        .bind(now() - EXPIRY_SECS)
        .fetch_one(pool)
        .await?;
    info.in_guild = 1;
    info.guild_id = guild.id;
    info.role = state.guild_manager().role_for(character.id()).unwrap_or(MEMBER_ROLE);
    info.level = level;
    info.experience = into;
    info.next_experience = span;
    info.members = guild.members.len() as u32;
    info.online = guild.members.iter().filter(|member| online(state, **member)).count() as u32;
    info.fund = u64::try_from(profile.silver).unwrap_or_default();
    info.entry_fee = u32::try_from(profile.entry_fee).unwrap_or_default();
    info.accepting = u32::from(profile.accepting);
    info.created_at = u64::try_from(profile.created_at).unwrap_or_default();
    info.pending_requests = u32::try_from(pending).unwrap_or_default();
    info.emblem = valid_emblem(u32::try_from(profile.emblem).unwrap_or_default());
    info.name = guild.name.clone();
    info.title = profile.title;
    info.motto = profile.motto;
    info.description = profile.description;
    info.leader = name_of(pool, guild.leader_id).await?;
    Ok(info)
}

async fn send_rows(actor: &Actor<ActorState>, list: u32, total: i64, header: MsgGuildPanelRow, rows: Vec<MsgGuildPanelRow>) -> Result<(), Error> {
    let mut head = header;
    head.list = list;
    head.index = 0;
    head.total = u32::try_from(total).unwrap_or_default();
    actor.send(head).await?;
    for (index, mut line) in rows.into_iter().enumerate() {
        line.list = list;
        line.index = index as u32 + 1;
        actor.send(line).await?;
    }
    Ok(())
}

/// One of the lists, at `page`, with its filter and search.
async fn send_list(
    state: &State,
    actor: &Actor<ActorState>,
    character: &Character,
    list: u32,
    page: u32,
    filter: &str,
    search: &str,
) -> Result<(), Error> {
    let pool = state.pool();
    let offset = i64::from(page) * PAGE_ROWS;
    let like = format!("%{}%", search.trim().replace(['%', '_'], ""));
    let guild = state.guild_manager().guild_for(character.id());
    let guild_id = guild.as_ref().map_or(0, |guild| i64::from(guild.id));
    match list {
        MEMBERS => {
            let rows: Vec<(i64, String, i64, i64, i64, i64)> = sqlx::query_as(
                "SELECT c.character_id, c.name, c.level, c.current_class, m.role, m.joined_at
                 FROM guild_members m JOIN characters c ON c.character_id = m.character_id
                 WHERE m.guild_id = $1 ORDER BY m.role DESC, c.level DESC, c.name",
            )
            .bind(guild_id)
            .fetch_all(pool)
            .await?;
            let total = rows.len() as i64;
            let leader = guild.as_ref().map_or(0, |guild| guild.leader_id);
            let lines = rows
                .into_iter()
                .map(|(id, name, level, class, role, joined)| {
                    let entity = entity_id(id);
                    let role = if entity == leader { LEADER_ROLE } else { state.guild_manager().role_for(entity).unwrap_or(role as u32) };
                    MsgGuildPanelRow {
                        id: id as u32,
                        level: level as u32,
                        value: role,
                        extra: u32::from(online(state, entity)),
                        at: joined as u64,
                        text1: name,
                        text2: class_name(class as u8).to_owned(),
                        ..row(list, 0)
                    }
                })
                .collect();
            send_rows(actor, list, total, row(list, 0), lines).await
        },
        REQUESTS => {
            let rows: Vec<(i64, String, i64, i64, String, i64)> = sqlx::query_as(
                "SELECT c.character_id, c.name, c.level, c.current_class, r.message, r.created_at
                 FROM guild_requests r JOIN characters c ON c.character_id = r.character_id
                 WHERE r.guild_id = $1 AND r.created_at > $2 ORDER BY r.created_at DESC",
            )
            .bind(guild_id)
            .bind(now() - EXPIRY_SECS)
            .fetch_all(pool)
            .await?;
            let total = rows.len() as i64;
            let lines = rows
                .into_iter()
                .map(|(id, name, level, class, message, at)| MsgGuildPanelRow {
                    id: id as u32,
                    level: level as u32,
                    extra: u32::from(online(state, entity_id(id))),
                    at: at as u64,
                    text1: name,
                    text2: class_name(class as u8).to_owned(),
                    text3: message,
                    ..row(list, 0)
                })
                .collect();
            send_rows(actor, list, total, row(list, 0), lines).await
        },
        INVITES => {
            let rows: Vec<(i64, String, i64, String, i64)> = sqlx::query_as(
                "SELECT c.character_id, c.name, c.level, i.invited_by, i.created_at
                 FROM guild_invites i JOIN characters c ON c.character_id = i.character_id
                 WHERE i.guild_id = $1 AND i.created_at > $2 ORDER BY i.created_at DESC",
            )
            .bind(guild_id)
            .bind(now() - EXPIRY_SECS)
            .fetch_all(pool)
            .await?;
            let total = rows.len() as i64;
            let lines = rows
                .into_iter()
                .map(|(id, name, level, by, at)| MsgGuildPanelRow {
                    id: id as u32,
                    level: level as u32,
                    extra: u32::from(online(state, entity_id(id))),
                    at: at as u64,
                    text1: name,
                    text2: by,
                    ..row(list, 0)
                })
                .collect();
            send_rows(actor, list, total, row(list, 0), lines).await
        },
        EVENTS | BALANCE => {
            // The history's filters: members, fund, war, settings; the
            // fund's own list is its movements.
            let kinds: &[&str] = match (list, filter) {
                (BALANCE, _) | (_, "fund") => &["deposit", "withdraw", "fee"],
                (_, "members") => &["founded", "joined", "left", "kicked", "promoted", "demoted", "leader"],
                (_, "war") => &["war_declared", "war_started", "war_refused", "war_ended"],
                (_, "settings") => &["settings"],
                _ => &[],
            };
            let kinds: Vec<String> = kinds.iter().map(|kind| (*kind).to_owned()).collect();
            let total: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM guild_events
                 WHERE guild_id = $1 AND created_at > $2 AND (cardinality($3::text[]) = 0 OR kind = ANY($3))
                   AND (actor ILIKE $4 OR target ILIKE $4)",
            )
            .bind(guild_id)
            .bind(now() - EVENTS_KEPT_SECS)
            .bind(&kinds)
            .bind(&like)
            .fetch_one(pool)
            .await?;
            let rows: Vec<(i64, String, String, String, i64, i64, i64)> = sqlx::query_as(
                "SELECT event_id, kind, actor, target, amount, balance, created_at FROM guild_events
                 WHERE guild_id = $1 AND created_at > $2 AND (cardinality($3::text[]) = 0 OR kind = ANY($3))
                   AND (actor ILIKE $4 OR target ILIKE $4)
                 ORDER BY created_at DESC, event_id DESC LIMIT $5 OFFSET $6",
            )
            .bind(guild_id)
            .bind(now() - EVENTS_KEPT_SECS)
            .bind(&kinds)
            .bind(&like)
            .bind(PAGE_ROWS)
            .bind(offset)
            .fetch_all(pool)
            .await?;
            let lines = rows
                .into_iter()
                .map(|(id, kind, who, target, amount, balance, at)| MsgGuildPanelRow {
                    id: id as u32,
                    value: u32::try_from(amount).unwrap_or(u32::MAX),
                    extra: u32::try_from(balance).unwrap_or(u32::MAX),
                    at: at as u64,
                    text1: kind,
                    text2: who,
                    text3: target,
                    ..row(list, 0)
                })
                .collect();
            let header = MsgGuildPanelRow {
                level: page,
                ..row(list, 0)
            };
            send_rows(actor, list, total, header, lines).await
        },
        WARS => {
            let rows: Vec<(i64, i64, i64, String, i64, String, String, i64, i64)> = sqlx::query_as(
                "SELECT w.war_id, w.guild_id, w.enemy_id, w.state, w.declared_at, a.name, b.name,
                        (SELECT COUNT(*) FROM guild_war_kills k WHERE k.war_id = w.war_id AND k.killer_guild_id = $1),
                        (SELECT COUNT(*) FROM guild_war_kills k WHERE k.war_id = w.war_id AND k.killer_guild_id <> $1)
                 FROM guild_wars w JOIN guilds a ON a.guild_id = w.guild_id JOIN guilds b ON b.guild_id = w.enemy_id
                 WHERE w.guild_id = $1 OR w.enemy_id = $1
                 ORDER BY (w.state = 'active') DESC, (w.state = 'pending') DESC, w.declared_at DESC LIMIT 30",
            )
            .bind(guild_id)
            .fetch_all(pool)
            .await?;
            let total = rows.len() as i64;
            let lines = rows
                .into_iter()
                .map(|(id, attacker, _, war_state, at, attacker_name, defender_name, ours, theirs)| {
                    let ours_declared = attacker == guild_id;
                    let value = match war_state.as_str() {
                        "pending" if ours_declared => WAR_OUTGOING,
                        "pending" => WAR_INCOMING,
                        "active" => WAR_ACTIVE,
                        "refused" => WAR_REFUSED,
                        _ => WAR_ENDED,
                    };
                    MsgGuildPanelRow {
                        id: id as u32,
                        value,
                        level: ours as u32,
                        extra: theirs as u32,
                        at: at as u64,
                        text1: if ours_declared { defender_name } else { attacker_name },
                        ..row(list, 0)
                    }
                })
                .collect();
            send_rows(actor, list, total, row(list, 0), lines).await
        },
        WAR_KILLS => {
            let war = i64::from(filter.parse::<u32>().unwrap_or_default());
            let rows: Vec<(i64, i64, String, String, i64)> = sqlx::query_as(
                "SELECT k.kill_id, k.killer_guild_id, k.killer, k.victim, k.created_at FROM guild_war_kills k
                 JOIN guild_wars w ON w.war_id = k.war_id
                 WHERE k.war_id = $1 AND (w.guild_id = $2 OR w.enemy_id = $2)
                 ORDER BY k.created_at DESC LIMIT $3 OFFSET $4",
            )
            .bind(war)
            .bind(guild_id)
            .bind(PAGE_ROWS)
            .bind(offset)
            .fetch_all(pool)
            .await?;
            let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM guild_war_kills WHERE war_id = $1")
                .bind(war)
                .fetch_one(pool)
                .await?;
            let lines = rows
                .into_iter()
                .map(|(id, killer_guild, killer, victim, at)| MsgGuildPanelRow {
                    id: id as u32,
                    value: u32::from(killer_guild == guild_id),
                    at: at as u64,
                    text1: killer,
                    text2: victim,
                    ..row(list, 0)
                })
                .collect();
            let header = MsgGuildPanelRow {
                id: war as u32,
                level: page,
                ..row(list, 0)
            };
            send_rows(actor, list, total, header, lines).await
        },
        DEATHS => {
            // `filter`: "deaths", "frags" or all; `search` a name. The
            // summary counts what the filter keeps in the last 12 hours,
            // day, 3 days and week.
            let (kind, cause) = filter.split_once(':').unwrap_or((filter, ""));
            let kind = match kind {
                "deaths" => "death",
                "frags" => "frag",
                _ => "",
            };
            let by_player = match cause {
                "players" => Some(true),
                "monsters" => Some(false),
                _ => None,
            };
            let condition = "guild_id = $1 AND created_at > $2 AND ($3 = '' OR kind = $3)
                 AND ($4::boolean IS NULL OR by_player = $4) AND (victim ILIKE $5 OR killer ILIKE $5)";
            let total: i64 = sqlx::query_scalar(&format!("SELECT COUNT(*) FROM guild_deaths WHERE {condition}"))
                .bind(guild_id)
                .bind(now() - DEATHS_KEPT_SECS)
                .bind(kind)
                .bind(by_player)
                .bind(&like)
                .fetch_one(pool)
                .await?;
            let mut summary = [0_u32; 4];
            for (slot, hours) in [12_i64, 24, 72, 168].into_iter().enumerate() {
                let count: i64 = sqlx::query_scalar(&format!("SELECT COUNT(*) FROM guild_deaths WHERE {condition}"))
                    .bind(guild_id)
                    .bind(now() - hours * 3_600)
                    .bind(kind)
                    .bind(by_player)
                    .bind(&like)
                    .fetch_one(pool)
                    .await?;
                summary[slot] = u32::try_from(count).unwrap_or_default();
            }
            let rows: Vec<(i64, String, String, i32, String, String, bool, i64)> = sqlx::query_as(&format!(
                "SELECT death_id, kind, victim, victim_level, killer, killer_guild, by_player, created_at
                 FROM guild_deaths WHERE {condition} ORDER BY created_at DESC, death_id DESC LIMIT $6 OFFSET $7"
            ))
            .bind(guild_id)
            .bind(now() - DEATHS_KEPT_SECS)
            .bind(kind)
            .bind(by_player)
            .bind(&like)
            .bind(PAGE_ROWS)
            .bind(offset)
            .fetch_all(pool)
            .await?;
            let lines = rows
                .into_iter()
                .map(|(id, kind, victim, level, killer, killer_guild, by_player, at)| MsgGuildPanelRow {
                    id: id as u32,
                    value: u32::from(kind == "frag"),
                    level: level as u32,
                    extra: u32::from(by_player),
                    at: at as u64,
                    text1: victim,
                    text2: killer,
                    text3: killer_guild,
                    ..row(list, 0)
                })
                .collect();
            let header = MsgGuildPanelRow {
                id: summary[3],
                level: summary[0],
                value: summary[1],
                extra: summary[2],
                text1: page.to_string(),
                ..row(list, 0)
            };
            send_rows(actor, list, total, header, lines).await
        },
        SEARCH => {
            let me = database_id(character.id());
            let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM guilds WHERE name ILIKE $1")
                .bind(&like)
                .fetch_one(pool)
                .await?;
            let rows: Vec<(i64, String, String, String, i64, bool, String, i64, bool, i64)> = sqlx::query_as(
                "SELECT g.guild_id, g.name, g.title, g.motto, g.experience, g.accepting, c.name,
                        (SELECT COUNT(*) FROM guild_members m WHERE m.guild_id = g.guild_id),
                        EXISTS (SELECT 1 FROM guild_requests r WHERE r.guild_id = g.guild_id AND r.character_id = $2 AND r.created_at > $3),
                        g.emblem
                 FROM guilds g JOIN characters c ON c.character_id = g.leader_id
                 WHERE g.name ILIKE $1 ORDER BY g.experience DESC, g.name LIMIT $4 OFFSET $5",
            )
            .bind(&like)
            .bind(me)
            .bind(now() - EXPIRY_SECS)
            .bind(PAGE_ROWS)
            .bind(offset)
            .fetch_all(pool)
            .await?;
            let lines = rows
                .into_iter()
                .map(|(id, name, title, motto, experience, accepting, leader, members, requested, emblem)| MsgGuildPanelRow {
                    id: id as u32,
                    at: u64::from(valid_emblem(u32::try_from(emblem).unwrap_or_default())),
                    level: level_of(experience.max(0) as u64).0,
                    value: members as u32,
                    extra: u32::from(accepting) | u32::from(requested) << 1,
                    text1: name,
                    text2: title,
                    text3: leader,
                    text4: motto,
                    ..row(list, 0)
                })
                .collect();
            let header = MsgGuildPanelRow {
                level: page,
                ..row(list, 0)
            };
            send_rows(actor, list, total, header, lines).await
        },
        MY_INVITES | MY_REQUESTS => {
            let table = if list == MY_INVITES { "guild_invites" } else { "guild_requests" };
            let extra = if list == MY_INVITES { "x.invited_by" } else { "x.message" };
            let rows: Vec<(i64, String, String, i64)> = sqlx::query_as(&format!(
                "SELECT g.guild_id, g.name, {extra}, x.created_at FROM {table} x JOIN guilds g ON g.guild_id = x.guild_id
                 WHERE x.character_id = $1 AND x.created_at > $2 ORDER BY x.created_at DESC"
            ))
            .bind(database_id(character.id()))
            .bind(now() - EXPIRY_SECS)
            .fetch_all(pool)
            .await?;
            let total = rows.len() as i64;
            let lines = rows
                .into_iter()
                .map(|(id, name, text, at)| MsgGuildPanelRow {
                    id: id as u32,
                    at: at as u64,
                    text1: name,
                    text2: text,
                    ..row(list, 0)
                })
                .collect();
            send_rows(actor, list, total, row(list, 0), lines).await
        },
        _ => Ok(()),
    }
}

/// The whole window: its header and the lists it opens on.
async fn send_window(state: &State, actor: &Actor<ActorState>, character: &Character, note: &str) -> Result<(), Error> {
    let header = info(state, character, note).await?;
    let in_guild = header.in_guild == 1;
    actor.send(header).await?;
    let lists: &[u32] = if in_guild {
        &[MEMBERS, REQUESTS, INVITES, EVENTS, BALANCE, WARS, DEATHS]
    } else {
        &[SEARCH, MY_INVITES, MY_REQUESTS]
    };
    for list in lists {
        send_list(state, actor, character, *list, 0, "", "").await?;
    }
    Ok(())
}

/// Refreshes the window of every member online but `except` (who gets theirs
/// with the note), and tells them `line` in the guild channel.
async fn tell_guild(state: &State, guild: &GuildInfo, except: u32, line: Option<String>) -> Result<(), Error> {
    for member in &guild.members {
        if *member == except {
            continue;
        }
        let Some(entity) = state.entity(*member) else {
            continue;
        };
        let Some(other) = entity.as_character() else {
            continue;
        };
        if let Some(line) = &line {
            other
                .owner()
                .send(MsgTalk::from_system(other.id(), TalkChannel::Guild, line.clone()))
                .await?;
        }
    }
    Ok(())
}

/// Tells an online character that they joined (or left, `guild` None) a
/// guild, so their name and window follow.
async fn set_guild_of(state: &State, entity: u32, guild: Option<&GuildInfo>) -> Result<(), Error> {
    let Some(target) = state.entity(entity) else {
        return Ok(());
    };
    let Some(owner) = target.owner() else {
        return Ok(());
    };
    match guild {
        Some(guild) => {
            owner
                .send(MsgGuild::new(SET_GUILD, guild.id).with_strings([guild.name.clone()]))
                .await?;
        },
        None => owner.send(MsgGuild::new(SET_GUILD, 0)).await?,
    }
    send_relations(state, entity).await?;
    refresh_look(state, entity).await
}

async fn members_changed(state: &State, guild: &GuildInfo) -> Result<(), Error> {
    for member in &guild.members {
        crate::packets::send_guild_members(state, *member, guild).await?;
        refresh_look(state, *member).await?;
    }
    Ok(())
}

/// Tells an online character which guilds are at war with or allied to
/// theirs (none without a guild), for the shields over names.
pub async fn send_relations(state: &State, entity: u32) -> Result<(), Error> {
    let Some(target) = state.entity(entity) else {
        return Ok(());
    };
    let Some(owner) = target.owner() else {
        return Ok(());
    };
    let join = |ids: Vec<u32>| ids.iter().map(u32::to_string).collect::<Vec<_>>().join(",");
    let packet = match state.guild_manager().guild_for(entity) {
        Some(guild) => crate::packets::MsgGuildRelations {
            guild_id: guild.id,
            enemies: join(state.guild_manager().relation_ids(guild.id, "enemy")),
            allies: join(state.guild_manager().relation_ids(guild.id, "ally")),
        },
        None => crate::packets::MsgGuildRelations::default(),
    };
    owner.send(packet).await?;
    Ok(())
}

/// Every online member of `guild` hears its relations anew.
pub async fn relations_changed(state: &State, guild: &GuildInfo) -> Result<(), Error> {
    for member in &guild.members {
        send_relations(state, *member).await?;
    }
    Ok(())
}

/// Shows an online character's guild (or none) to them and those around.
pub async fn refresh_look(state: &State, entity: u32) -> Result<(), Error> {
    let Some(target) = state.entity(entity) else {
        return Ok(());
    };
    let Some(character) = target.as_character() else {
        return Ok(());
    };
    let look = crate::packets::MsgPlayer::from(character);
    character.owner().send(look.clone()).await?;
    if let Ok(screen) = character.try_screen() {
        screen.send_message(look).await?;
    }
    Ok(())
}

/// The requests and invitations a character had, gone once they joined.
async fn clear_pending(pool: &sqlx::PgPool, database: i64) -> Result<(), Error> {
    for table in ["guild_requests", "guild_invites"] {
        sqlx::query(&format!("DELETE FROM {table} WHERE character_id = $1"))
            .bind(database)
            .execute(pool)
            .await?;
    }
    Ok(())
}

/// What a request did: the line for the window (and the chat).
type Outcome = Result<String, Error>;

fn refused(text: &str) -> Outcome {
    Ok(text.to_owned())
}

async fn create(state: &State, actor: &Actor<ActorState>, character: &Character, request: &MsgGuildPanel) -> Outcome {
    if state.guild_manager().guild_for(character.id()).is_some() {
        return refused("You already belong to a guild.");
    }
    if character.entity().level() < FOUND_LEVEL {
        return refused("You must be level 90 to found a guild.");
    }
    if character.silver() < u64::from(FOUND_PRICE) {
        return refused("Founding a guild costs 1,000,000 silver.");
    }
    let change = match state.guild_manager().create(character.id(), request.text1.trim()) {
        Ok(change) => change,
        Err(error) => return refused(error.message()),
    };
    let crate::guild::GuildChange::Created { guild } = change else {
        return refused("The guild could not be founded.");
    };
    if character.spend_silver(state.pool(), FOUND_PRICE).await?.is_none() {
        let _ = state.guild_manager().disband(character.id());
        return refused("Founding a guild costs 1,000,000 silver.");
    }
    crate::guild::GuildManager::persist_created(state.pool(), &guild).await?;
    sqlx::query("UPDATE guilds SET title = $1, motto = $2, description = $3, emblem = $4 WHERE guild_id = $5")
        .bind(clip(&request.text2, TITLE_CHARS))
        .bind(clip(&request.text3, MOTTO_CHARS))
        .bind(clip(&request.text4, DESCRIPTION_CHARS))
        .bind(i64::from(valid_emblem(request.amount)))
        .bind(i64::from(guild.id))
        .execute(state.pool())
        .await?;
    clear_pending(state.pool(), database_id(character.id())).await?;
    log(state.pool(), guild.id, "founded", character.entity().name(), "", 0, 0).await?;
    actor.send(MsgUserInfo::from(character)).await?;
    set_guild_of(state, character.id(), Some(&guild)).await?;
    members_changed(state, &guild).await?;
    crate::audit::economic("guild_panel_found", character.id(), 0, guild.id, FOUND_PRICE);
    Ok(format!("{} was founded. Welcome, leader!", guild.name))
}

/// The guild and the character's rank, if they rank at least `least`.
fn ranked(state: &State, character: &Character, least: u32) -> Result<(GuildInfo, u32), &'static str> {
    let guild = state.guild_manager().guild_for(character.id()).ok_or("You are not in a guild.")?;
    let role = if guild.leader_id == character.id() {
        LEADER_ROLE
    } else {
        state.guild_manager().role_for(character.id()).unwrap_or(MEMBER_ROLE)
    };
    if !rank_allows(role, least) {
        return Err(if least >= LEADER_ROLE { "Only the guild leader can do that." } else { "Only the leader and deputies can do that." });
    }
    Ok((guild, role))
}

/// Takes the character `database` into `guild`: the entry fee if one is
/// asked (they must be online to pay it), the ranks, and everyone told.
async fn admit(state: &State, guild: &GuildInfo, database: i64, by: &str) -> Outcome {
    let pool = state.pool();
    let entity = entity_id(database);
    let name = name_of(pool, entity).await?;
    let fee = profile(pool, guild.id).await?.map_or(0, |profile| profile.entry_fee);
    if state.guild_manager().guild_for(entity).is_some() {
        clear_pending(pool, database).await?;
        return refused("They already joined a guild.");
    }
    if guild.members.len() >= MAX_GUILD_MEMBERS {
        return refused("The guild is full.");
    }
    let member = state.entity(entity);
    let member = member.as_ref().and_then(|entity| entity.as_character());
    let mut balance = 0;
    if fee > 0 {
        let Some(member) = member else {
            return refused("They must be online to pay the entry fee.");
        };
        let fee = u32::try_from(fee).unwrap_or(u32::MAX);
        let Some((_, fund)) = member.transfer_guild_fund(pool, guild.id, fee, true).await? else {
            return refused("They cannot pay the entry fee.");
        };
        balance = i64::try_from(fund).unwrap_or(i64::MAX);
        member.owner().send(MsgUserInfo::from(member)).await?;
    }
    let guild = match state.guild_manager().admit(guild.id, entity) {
        Ok(guild) => guild,
        Err(error) => return refused(error.message()),
    };
    crate::guild::GuildManager::persist_member(pool, &guild, entity).await?;
    clear_pending(pool, database).await?;
    if fee > 0 {
        log(pool, guild.id, "fee", &name, "", fee, balance).await?;
    }
    log(pool, guild.id, "joined", &name, by, 0, 0).await?;
    set_guild_of(state, entity, Some(&guild)).await?;
    members_changed(state, &guild).await?;
    if let Some(member) = member {
        member
            .owner()
            .send(MsgTalk::from_system(member.id(), TalkChannel::Guild, format!("You joined {}.", guild.name)))
            .await?;
    }
    tell_guild(state, &guild, 0, Some(format!("{name} joined the guild."))).await?;
    Ok(format!("{name} joined the guild."))
}

async fn remove(state: &State, guild: GuildInfo, removed: u32, kind: &str, by: &str) -> Result<String, Error> {
    let pool = state.pool();
    let name = name_of(pool, removed).await?;
    crate::guild::GuildManager::persist_removal(pool, &guild, removed).await?;
    set_guild_of(state, removed, None).await?;
    if guild.members.is_empty() {
        return Ok(format!("{name} left; the guild is no more."));
    }
    log(pool, guild.id, kind, &name, by, 0, 0).await?;
    members_changed(state, &guild).await?;
    let line = if kind == "kicked" { format!("{name} was removed from the guild.") } else { format!("{name} left the guild.") };
    tell_guild(state, &guild, 0, Some(line.clone())).await?;
    Ok(line)
}

async fn act(state: &State, actor: &Actor<ActorState>, character: &Character, request: &MsgGuildPanel) -> Outcome {
    let pool = state.pool();
    let me = character.entity().name().to_owned();
    let my_database = database_id(character.id());
    match request.action {
        CREATE => create(state, actor, character, request).await,
        REQUEST_JOIN => {
            if state.guild_manager().guild_for(character.id()).is_some() {
                return refused("You already belong to a guild.");
            }
            let Some(guild) = state.guild_manager().all().into_iter().find(|guild| guild.id == request.param) else {
                return refused("That guild no longer exists.");
            };
            if !profile(pool, guild.id).await?.is_some_and(|profile| profile.accepting) {
                return refused("That guild does not take requests now.");
            }
            sqlx::query(
                "INSERT INTO guild_requests (guild_id, character_id, message) VALUES ($1, $2, $3)
                 ON CONFLICT (guild_id, character_id) DO UPDATE SET message = excluded.message,
                     created_at = EXTRACT(EPOCH FROM now())::BIGINT",
            )
            .bind(i64::from(guild.id))
            .bind(my_database)
            .bind(clip(&request.text1, MESSAGE_CHARS))
            .execute(pool)
            .await?;
            let line = format!("{me} asks to join the guild. See the requests in the Guild System window.");
            for member in &guild.members {
                if state.guild_manager().role_for(*member).is_some_and(|role| role >= DEPUTY_ROLE) || *member == guild.leader_id {
                    if let Some(other) = state.entity(*member).as_ref().and_then(|entity| entity.as_character()) {
                        other.owner().send(MsgTalk::from_system(other.id(), TalkChannel::Guild, line.clone())).await?;
                    }
                }
            }
            Ok(format!("Your request to join {} was sent.", guild.name))
        },
        CANCEL_REQUEST | REFUSE_INVITE => {
            let table = if request.action == CANCEL_REQUEST { "guild_requests" } else { "guild_invites" };
            sqlx::query(&format!("DELETE FROM {table} WHERE guild_id = $1 AND character_id = $2"))
                .bind(i64::from(request.param))
                .bind(my_database)
                .execute(pool)
                .await?;
            Ok(if request.action == CANCEL_REQUEST { "Request withdrawn." } else { "Invitation refused." }.to_owned())
        },
        ACCEPT_INVITE => {
            let invited: Option<i64> = sqlx::query_scalar(
                "SELECT created_at FROM guild_invites WHERE guild_id = $1 AND character_id = $2 AND created_at > $3",
            )
            .bind(i64::from(request.param))
            .bind(my_database)
            .bind(now() - EXPIRY_SECS)
            .fetch_optional(pool)
            .await?;
            let guild = state.guild_manager().all().into_iter().find(|guild| guild.id == request.param);
            let (Some(_), Some(guild)) = (invited, guild) else {
                return refused("That invitation is gone.");
            };
            admit(state, &guild, my_database, "invitation").await
        },
        ACCEPT_REQUEST | REFUSE_REQUEST => {
            let (guild, _) = match ranked(state, character, DEPUTY_ROLE) {
                Ok(ranked) => ranked,
                Err(reason) => return refused(reason),
            };
            let database = i64::from(request.param);
            let asked: Option<i64> = sqlx::query_scalar(
                "SELECT created_at FROM guild_requests WHERE guild_id = $1 AND character_id = $2 AND created_at > $3",
            )
            .bind(i64::from(guild.id))
            .bind(database)
            .bind(now() - EXPIRY_SECS)
            .fetch_optional(pool)
            .await?;
            if asked.is_none() {
                return refused("That request is gone.");
            }
            if request.action == REFUSE_REQUEST {
                sqlx::query("DELETE FROM guild_requests WHERE guild_id = $1 AND character_id = $2")
                    .bind(i64::from(guild.id))
                    .bind(database)
                    .execute(pool)
                    .await?;
                let name = name_of(pool, entity_id(database)).await?;
                if let Some(other) = state.entity(entity_id(database)).as_ref().and_then(|entity| entity.as_character()) {
                    other
                        .owner()
                        .send(MsgTalk::from_system(other.id(), TalkChannel::Guild, format!("{} refused your request.", guild.name)))
                        .await?;
                }
                return Ok(format!("{name}'s request refused."));
            }
            admit(state, &guild, database, &me).await
        },
        INVITE => {
            let (guild, _) = match ranked(state, character, DEPUTY_ROLE) {
                Ok(ranked) => ranked,
                Err(reason) => return refused(reason),
            };
            let target: Option<(i64, String)> = sqlx::query_as("SELECT character_id, name FROM characters WHERE lower(name) = lower($1)")
                .bind(request.text1.trim())
                .fetch_optional(pool)
                .await?;
            let Some((database, name)) = target else {
                return refused("There is nobody by that name.");
            };
            if state.guild_manager().guild_for(entity_id(database)).is_some() {
                return refused("They already belong to a guild.");
            }
            if guild.members.len() >= MAX_GUILD_MEMBERS {
                return refused("The guild is full.");
            }
            sqlx::query(
                "INSERT INTO guild_invites (guild_id, character_id, invited_by) VALUES ($1, $2, $3)
                 ON CONFLICT (guild_id, character_id) DO UPDATE SET invited_by = excluded.invited_by,
                     created_at = EXTRACT(EPOCH FROM now())::BIGINT",
            )
            .bind(i64::from(guild.id))
            .bind(database)
            .bind(&me)
            .execute(pool)
            .await?;
            if let Some(other) = state.entity(entity_id(database)).as_ref().and_then(|entity| entity.as_character()) {
                other
                    .owner()
                    .send(MsgTalk::from_system(
                        other.id(),
                        TalkChannel::Guild,
                        format!("{me} invited you to {}. Answer in the Guild System window.", guild.name),
                    ))
                    .await?;
            }
            Ok(format!("{name} was invited."))
        },
        CANCEL_INVITE => {
            let (guild, _) = match ranked(state, character, DEPUTY_ROLE) {
                Ok(ranked) => ranked,
                Err(reason) => return refused(reason),
            };
            sqlx::query("DELETE FROM guild_invites WHERE guild_id = $1 AND character_id = $2")
                .bind(i64::from(guild.id))
                .bind(i64::from(request.param))
                .execute(pool)
                .await?;
            refused("Invitation withdrawn.")
        },
        LEAVE => {
            let Some(guild) = state.guild_manager().guild_for(character.id()) else {
                return refused("You are not in a guild.");
            };
            if guild.leader_id == character.id() && guild.members.len() > 1 {
                return refused("Hand the guild to another member first, or disband it.");
            }
            match state.guild_manager().leave(character.id()) {
                Ok(crate::guild::GuildChange::State { guild, removed: Some(removed), .. }) => {
                    remove(state, guild, removed, "left", "").await
                },
                Ok(_) => refused("You left the guild."),
                Err(error) => refused(error.message()),
            }
        },
        KICK => {
            let target = entity_id(i64::from(request.param));
            match state.guild_manager().kick(character.id(), target) {
                Ok(crate::guild::GuildChange::State { guild, removed: Some(removed), .. }) => {
                    remove(state, guild, removed, "kicked", &me).await
                },
                Ok(_) => refused("They were removed."),
                Err(error) => refused(error.message()),
            }
        },
        PROMOTE | DEMOTE => {
            let target = entity_id(i64::from(request.param));
            let role = if request.action == PROMOTE { DEPUTY_ROLE } else { MEMBER_ROLE };
            match state.guild_manager().set_role(character.id(), target, role) {
                Ok(guild) => {
                    crate::guild::GuildManager::persist_role(pool, target, role).await?;
                    let name = name_of(pool, target).await?;
                    let kind = if request.action == PROMOTE { "promoted" } else { "demoted" };
                    log(pool, guild.id, kind, &name, &me, 0, 0).await?;
                    members_changed(state, &guild).await?;
                    let line = if request.action == PROMOTE {
                        format!("{name} is now a deputy.")
                    } else {
                        format!("{name} is now a member.")
                    };
                    tell_guild(state, &guild, character.id(), Some(line.clone())).await?;
                    Ok(line)
                },
                Err(error) => refused(error.message()),
            }
        },
        TRANSFER => {
            let target = entity_id(i64::from(request.param));
            match state.guild_manager().transfer(character.id(), target) {
                Ok(guild) => {
                    crate::guild::GuildManager::persist_leader(pool, &guild, character.id()).await?;
                    let name = name_of(pool, target).await?;
                    log(pool, guild.id, "leader", &name, &me, 0, 0).await?;
                    members_changed(state, &guild).await?;
                    let line = format!("{name} now leads the guild.");
                    tell_guild(state, &guild, character.id(), Some(line.clone())).await?;
                    Ok(line)
                },
                Err(error) => refused(error.message()),
            }
        },
        DEPOSIT | WITHDRAW => {
            let Some(guild) = state.guild_manager().guild_for(character.id()) else {
                return refused("You are not in a guild.");
            };
            let donate = request.action == DEPOSIT;
            if !donate && guild.leader_id != character.id() {
                return refused("Only the guild leader can take from the fund.");
            }
            if request.amount == 0 {
                return refused("Type an amount first.");
            }
            match crate::packets::move_guild_fund(state, actor, character, donate, request.amount).await? {
                Some(fund) => Ok(if donate {
                    format!("You gave {} silver to the fund ({fund} now).", request.amount)
                } else {
                    format!("You took {} silver from the fund ({fund} left).", request.amount)
                }),
                None => refused(if donate { "You do not have that much silver." } else { "The fund does not hold that much." }),
            }
        },
        SETTINGS => {
            let (guild, _) = match ranked(state, character, LEADER_ROLE) {
                Ok(ranked) => ranked,
                Err(reason) => return refused(reason),
            };
            let fee = request.amount.min(MAX_ENTRY_FEE);
            sqlx::query(
                "UPDATE guilds SET title = $1, motto = $2, description = $3, entry_fee = $4, accepting = $5 WHERE guild_id = $6",
            )
            .bind(clip(&request.text1, TITLE_CHARS))
            .bind(clip(&request.text2, MOTTO_CHARS))
            .bind(clip(&request.text3, DESCRIPTION_CHARS))
            .bind(i64::from(fee))
            .bind(request.param != 0)
            .bind(i64::from(guild.id))
            .execute(pool)
            .await?;
            log(pool, guild.id, "settings", &me, "", i64::from(fee), 0).await?;
            refused("The guild's settings were saved.")
        },
        DECLARE_WAR => {
            let (guild, _) = match ranked(state, character, LEADER_ROLE) {
                Ok(ranked) => ranked,
                Err(reason) => return refused(reason),
            };
            let Some(enemy) = state.guild_manager().guild_by_name(&request.text1).filter(|enemy| enemy.id != guild.id) else {
                return refused("There is no other guild by that name.");
            };
            let open: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM guild_wars WHERE state IN ('pending', 'active')
                   AND ((guild_id = $1 AND enemy_id = $2) OR (guild_id = $2 AND enemy_id = $1))",
            )
            .bind(i64::from(guild.id))
            .bind(i64::from(enemy.id))
            .fetch_one(pool)
            .await?;
            if open > 0 {
                return refused("There already is a war with that guild.");
            }
            sqlx::query("INSERT INTO guild_wars (guild_id, enemy_id) VALUES ($1, $2)")
                .bind(i64::from(guild.id))
                .bind(i64::from(enemy.id))
                .execute(pool)
                .await?;
            log(pool, guild.id, "war_declared", &me, &enemy.name, 0, 0).await?;
            log(pool, enemy.id, "war_declared", &guild.name, &enemy.name, 0, 0).await?;
            tell_guild(state, &enemy, 0, Some(format!("{} declared war on your guild! The leader may accept or refuse it.", guild.name)))
                .await?;
            Ok(format!("War declared on {}. They must accept it.", enemy.name))
        },
        ANSWER_WAR | END_WAR => {
            let (guild, _) = match ranked(state, character, LEADER_ROLE) {
                Ok(ranked) => ranked,
                Err(reason) => return refused(reason),
            };
            let war: Option<(i64, i64, String)> = sqlx::query_as("SELECT guild_id, enemy_id, state FROM guild_wars WHERE war_id = $1")
                .bind(i64::from(request.param))
                .fetch_optional(pool)
                .await?;
            let Some((attacker, defender, war_state)) = war else {
                return refused("That war does not exist.");
            };
            let ours = i64::from(guild.id);
            let other_id = if attacker == ours { defender } else { attacker };
            let Some(other) = state.guild_manager().all().into_iter().find(|other| i64::from(other.id) == other_id) else {
                return refused("That guild no longer exists.");
            };
            let (next, kind, line) = match (request.action, war_state.as_str()) {
                (ANSWER_WAR, "pending") if defender == ours && request.amount == 1 => ("active", "war_started", format!("The war with {} has begun!", other.name)),
                (ANSWER_WAR, "pending") if defender == ours => ("refused", "war_refused", format!("The war with {} was refused.", other.name)),
                (ANSWER_WAR, "pending") if attacker == ours => ("ended", "war_ended", format!("The declaration on {} was withdrawn.", other.name)),
                (END_WAR, "active") if attacker == ours || defender == ours => ("ended", "war_ended", format!("The war with {} is over.", other.name)),
                _ => return refused("That war cannot change so."),
            };
            let started = if next == "active" { now() } else { 0 };
            let ended = if next == "active" { 0 } else { now() };
            sqlx::query(
                "UPDATE guild_wars SET state = $1, started_at = CASE WHEN $2 > 0 THEN $2 ELSE started_at END, ended_at = $3 WHERE war_id = $4",
            )
            .bind(next)
            .bind(started)
            .bind(ended)
            .bind(i64::from(request.param))
            .execute(pool)
            .await?;
            // At war the two guilds are enemies of each other (PK rules);
            // the war over, no longer.
            let relation = (next == "active").then_some("enemy");
            for (one, two) in [(guild.id, other.id), (other.id, guild.id)] {
                state.guild_manager().set_relation(pool, one, two, relation).await?;
            }
            log(pool, guild.id, kind, &me, &other.name, 0, 0).await?;
            log(pool, other.id, kind, &guild.name, &other.name, 0, 0).await?;
            relations_changed(state, &guild).await?;
            relations_changed(state, &other).await?;
            tell_guild(state, &other, 0, Some(line.replace(&other.name, &guild.name))).await?;
            tell_guild(state, &guild, character.id(), Some(line.clone())).await?;
            Ok(line)
        },
        EMBLEM => {
            let (guild, _) = match ranked(state, character, LEADER_ROLE) {
                Ok(ranked) => ranked,
                Err(reason) => return refused(reason),
            };
            sqlx::query("UPDATE guilds SET emblem = $1 WHERE guild_id = $2")
                .bind(i64::from(valid_emblem(request.param)))
                .bind(i64::from(guild.id))
                .execute(pool)
                .await?;
            log(pool, guild.id, "emblem", &me, "", 0, 0).await?;
            refused("The guild's crest was changed.")
        },
        DISBAND => {
            let Ok(guild) = state.guild_manager().disband(character.id()) else {
                return refused("Only the guild leader can disband it.");
            };
            crate::guild::GuildManager::persist_disband(pool, guild.id).await?;
            for member in &guild.members {
                set_guild_of(state, *member, None).await?;
            }
            tell_guild(state, &guild, character.id(), Some(format!("{} was disbanded.", guild.name))).await?;
            Ok(format!("{} was disbanded.", guild.name))
        },
        _ => Ok(String::new()),
    }
}

/// A request from the window.
pub async fn handle(state: &State, actor: &Actor<ActorState>, character: &Character, request: &MsgGuildPanel) -> Result<(), Error> {
    match request.action {
        OPEN => send_window(state, actor, character, "").await,
        PAGE => send_list(state, actor, character, request.param, request.amount, &request.text1, &request.text2).await,
        _ => {
            let note = act(state, actor, character, request).await?;
            if !note.is_empty() {
                actor
                    .send(MsgTalk::from_system(character.id(), TalkChannel::TopLeft, note.clone()))
                    .await?;
            }
            send_window(state, actor, character, &note).await
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guild_levels_grow_by_squares() {
        assert_eq!(level_of(0), (0, 0, 100_000));
        assert_eq!(level_of(99_999), (0, 99_999, 100_000));
        assert_eq!(level_of(100_000), (1, 0, 300_000));
        assert_eq!(level_of(400_000), (2, 0, 500_000));
    }

    #[test]
    fn crests_stay_within_what_the_client_draws() {
        assert_eq!(valid_emblem(0), 0);
        assert_eq!(valid_emblem(5 | 3 << 8 | 2 << 16), 5 | 3 << 8 | 2 << 16);
        assert_eq!(valid_emblem(200 | 99 << 8 | 7 << 16), (200 % EMBLEMS) | (99 % FIELD_COLOURS) << 8 | (7 % EMBLEM_COLOURS) << 16);
    }

    #[test]
    fn texts_are_trimmed_and_cut() {
        assert_eq!(clip("  Reis Veteranos  ", TITLE_CHARS), "Reis Veteranos");
        assert_eq!(clip(&"á".repeat(400), DESCRIPTION_CHARS).chars().count(), DESCRIPTION_CHARS);
        assert_eq!(class_name(15), "Trojan");
        assert_eq!(class_name(145), "Fire Taoist");
    }
}
