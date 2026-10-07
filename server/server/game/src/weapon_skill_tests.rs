//! Every weapon skill through what a player does: the book that teaches
//! it, the weapon worn, plain attacks on a monster (`MsgInteract` 2) that
//! now and then set it off (`MsgSkillEffect` 1105, at the skill's level),
//! the experience each one earns (`MsgSkill` 1103), and its ten levels:
//! the experience each needs (MagicType `NeedExp`), the chance growing
//! with them (`Percent`), and the hero's level some wait for (`NeedLevel`).
//!
//! It needs the item table (`ITEMTYPE_DATA`), the maps (`server/data`) and
//! the test database; without the table it is skipped. It takes a couple
//! of minutes and sets the process's `DATA_LOCATION`, so it runs alone:
//! `cargo test -p game weapon_skill_journey -- --ignored --nocapture`.

use std::sync::{Arc, Mutex};

use tq_network::{Actor, Message, PacketProcess};

use crate::entities::Character;
use crate::packets::MsgInteract;
use crate::{ActorState, Error, State};

const SKILL_EFFECT: u16 = 1105;
const SKILL_UPDATE: u16 = 1103;
const ARROWS: u32 = 1_050_002;
/// Plain attacks per skill and level tried.
const SWINGS: u32 = 300;

type Sent = Arc<Mutex<Vec<(u16, Vec<u8>)>>>;

fn u16_at(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}

fn u32_at(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().expect("four bytes"))
}

/// A level 130 game master of its own, with what the server tells them.
async fn hero(state: &State, account: u32, name: &str) -> Result<(Actor<ActorState>, Sent), Error> {
    let (tx, mut replies) = tokio::sync::mpsc::channel(256);
    let sent: Sent = Arc::default();
    let log = sent.clone();
    tokio::spawn(async move {
        while let Some(message) = replies.recv().await {
            if let Message::Packet(id, bytes) = message {
                log.lock().expect("log").push((id, bytes.to_vec()));
            }
        }
    });
    sqlx::query(
        "INSERT INTO accounts (account_id, username, password) VALUES ($1, $2, 'x') ON CONFLICT (account_id) DO NOTHING",
    )
    .bind(i64::from(account))
    .bind(format!("ws-test-{account}"))
    .execute(state.pool())
    .await?;
    let actor = Actor::<ActorState>::new(tx);
    actor.set_id(account as usize);
    let inner = crate::packets::MsgRegister::build_character_with(
        name.to_owned(),
        crate::packets::BodyType::MuscularMale,
        crate::packets::BaseClass::Trojan,
        account,
        1,
    )?;
    inner.save(state.pool()).await?;
    let inner = tq_db::character::Character::from_account(state.pool(), account).await?.expect("character");
    let screen = crate::systems::Screen::new(actor.handle());
    actor.update(Character::new(actor.handle(), inner), screen);
    actor.set_gm(true);
    state.insert_entity(actor.entity());
    crate::systems::commands::parse_and_execute(state, &actor, &["level", "130"]).await?;
    Ok((actor, sent))
}

/// The first item of `weapon_type` the table has.
fn weapon_of(state: &State, weapon_type: u32) -> Option<u32> {
    (0..100)
        .flat_map(|level| [5, 3, 4, 6, 7, 8, 9, 0, 1, 2].map(|quality| weapon_type * 1_000 + level * 10 + quality))
        .find(|id| state.shop_catalog().template(*id).is_some())
}

async fn give(state: &State, actor: &Actor<ActorState>, item_type: u32) -> Result<u32, Error> {
    crate::systems::commands::parse_and_execute(state, actor, &["item", &item_type.to_string()]).await?;
    let entity = actor.entity();
    let me = entity.as_character().expect("character");
    Ok(me
        .inventory_items()
        .into_iter()
        .rev()
        .find(|item| item.item_type == item_type)
        .expect("given")
        .uid)
}

/// A sturdy monster beside the hero, who stands still.
async fn target(state: &State, actor: &Actor<ActorState>, kind: &tq_db::monster::MonsterType) -> Result<u32, Error> {
    let entity = actor.entity();
    let me = entity.as_character().expect("character");
    let map_id = me.entity().map_id();
    let here = me.entity().location();
    let id = crate::bosses::spawn_monster(state, map_id, (here.x, here.y), kind, 0).await?.expect("room for it");
    let map = state.try_map(map_id)?;
    let there = map.monster_entity(id).expect("spawned").basic().location();
    let mut beside = there;
    beside.x += 1;
    me.entity().set_location(beside);
    Ok(id)
}

/// Swings `SWINGS` times; the weapon skill's casts, the levels they
/// carried and the experience the skill ended with.
async fn swing(
    state: &State,
    actor: &Actor<ActorState>,
    sent: &Sent,
    kind: &tq_db::monster::MonsterType,
    skill_id: u32,
) -> Result<(u32, Vec<u16>, u32), Error> {
    let entity = actor.entity();
    let me = entity.as_character().expect("character");
    let map = state.try_map(me.entity().map_id())?;
    let mut monster = target(state, actor, kind).await?;
    let mut casts = 0;
    let mut levels = Vec::new();
    let mut updates = 0;
    for _ in 0..SWINGS {
        if map.monster_entity(monster).is_none_or(|entity| entity.basic().is_dead()) {
            monster = target(state, actor, kind).await?;
        }
        // The hero never tires or runs dry.
        me.entity().heal_health(u16::MAX);
        crate::packets::forget_attack_cooldown(me.id());
        sent.lock().expect("log").clear();
        let there = map.monster_entity(monster).expect("alive").basic().location();
        let attack = MsgInteract {
            timestamp: 0,
            character_id: me.id(),
            target_id: monster,
            x: there.x,
            y: there.y,
            action: 2,
            value: 0,
            reserved: [0; 12],
        };
        attack.process(state, actor).await?;
        // What the server said reaches the log a moment later.
        tokio::task::yield_now().await;
        tokio::time::sleep(std::time::Duration::from_millis(1)).await;
        let log = sent.lock().expect("log");
        for (_, body) in log.iter().filter(|(id, _)| *id == SKILL_EFFECT) {
            if u32_at(body, 0) == me.id() && u32::from(u16_at(body, 8)) == skill_id {
                casts += 1;
                levels.push(u16_at(body, 10));
            }
        }
        updates += log
            .iter()
            .filter(|(id, body)| *id == SKILL_UPDATE && u32::from(u16_at(body, 4)) == skill_id)
            .count() as u32;
    }
    Ok((casts, levels, updates))
}

/// Whether `hits` of `tries` fit a `percent` chance (four standard
/// deviations either way).
fn fits(hits: u32, tries: u32, percent: u8) -> bool {
    let p = f64::from(percent) / 100.0;
    let mean = f64::from(tries) * p;
    let spread = 4.0 * (f64::from(tries) * p * (1.0 - p)).sqrt();
    (f64::from(hits) - mean).abs() <= spread.max(3.0)
}

#[tokio::test]
#[ignore]
async fn weapon_skill_journey() -> Result<(), Error> {
    if std::env::var_os("ITEMTYPE_DATA").is_none() {
        let table = std::path::Path::new(r"R:\5065\ini\itemtype.dat");
        if !table.exists() {
            eprintln!("weapon_skill_journey skipped: no item table");
            return Ok(());
        }
        std::env::set_var("ITEMTYPE_DATA", table);
    }
    std::env::set_var("DATA_LOCATION", concat!(env!("CARGO_MANIFEST_DIR"), "/../../data"));
    std::env::set_var("SKILLS_DATA", concat!(env!("CARGO_MANIFEST_DIR"), "/data/skills.dat"));
    let state = State::with_pool(tq_db::testing::pool().await).await?;
    // A monster that takes many blows (no guard, no boss of a script).
    let kind = sqlx::query_as::<_, tq_db::monster::MonsterType>(
        "SELECT id, name, look, level, life, attack_min, attack_max, defense, attack_speed, respawn_seconds, view_range, attack_range, attack_mode, magic_resistance \
         FROM monster_types WHERE life BETWEEN 20000 AND 2000000 AND name NOT ILIKE '%guard%' ORDER BY life DESC LIMIT 1",
    )
    .fetch_one(state.pool())
    .await?;
    println!("target: {} (life {}, level {})", kind.name, kind.life, kind.level);

    let skills: Vec<_> = state.skill_catalog().weapon_skills().into_iter().cloned().collect();
    let mut failures = Vec::new();
    println!();
    println!("| skill | id | weapon (item) | taught by | learned at | level 1: casts/swings (chance) | MsgSkill updates | effect levels sent | skill exp, level after | top: casts/swings (chance) | effect level sent at top |");
    println!("|---|---|---|---|---|---|---|---|---|---|---|");
    for (index, skill) in skills.iter().enumerate() {
        let account = 100 + index as u32;
        let (actor, sent) = hero(&state, account, &format!("ws{}", skill.id)).await?;
        let entity = actor.entity();
        let me = entity.as_character().expect("character");
        let weapon_type = skill.weapon[0];
        let Some(weapon) = weapon_of(&state, weapon_type) else {
            failures.push(format!("{}: no item of weapon type {weapon_type}", skill.name));
            continue;
        };
        let uid = give(&state, &actor, weapon).await?;
        me.equip_inventory_item(state.pool(), uid, 4).await?.expect("equipped");
        if weapon_type == 500 {
            let uid = give(&state, &actor, ARROWS).await?;
            me.equip_inventory_item(state.pool(), uid, 5).await?.expect("arrows worn");
        }

        // The book (the bow's comes from the Archer God, `trainers.rs`).
        let (book, progress) = match skill.learn_item {
            Some(book) => {
                let book_uid = give(&state, &actor, book).await?;
                let learned = me.learn_skill_from_item(state.pool(), book_uid, skill.id).await?;
                let Some((_, progress)) = learned else {
                    failures.push(format!("{}: the book {book} did not teach it", skill.name));
                    continue;
                };
                if me.count_items(book) != 0 {
                    failures.push(format!("{}: the book {book} was not used up", skill.name));
                }
                (book.to_string(), progress)
            },
            None => {
                me.learn_skill(state.pool(), skill.id, 1).await?;
                ("Archer God".to_owned(), me.skill_progress(skill.id).expect("learned"))
            },
        };
        if progress.level != 1 {
            failures.push(format!("{}: learned at {}", skill.name, progress.level));
        }

        // Level 1: plain attacks set it off as often as its chance says.
        let chance = skill.at_level(1).chance;
        let (casts, levels, updates) = swing(&state, &actor, &sent, &kind, skill.id).await?;
        let experience = me.skill_progress(skill.id).expect("learned").experience;
        if !fits(casts, SWINGS, chance) {
            failures.push(format!("{}: {casts} casts in {SWINGS} swings at {chance}%", skill.name));
        }
        if casts > 0 && experience == 0 {
            failures.push(format!("{}: {casts} casts and no experience", skill.name));
        }
        if updates < casts {
            failures.push(format!("{}: {casts} casts, {updates} skill updates to the client", skill.name));
        }
        let level_now = me.skill_progress(skill.id).expect("learned").level;
        let mut sent_levels = levels.clone();
        sent_levels.dedup();
        // Each cast carries the level the skill had then: from 1, rising
        // as the casts teach it (the last may raise it after being sent).
        let in_step = levels.first().is_none_or(|first| *first == 1)
            && levels.windows(2).all(|pair| pair[0] <= pair[1])
            && levels.last().is_none_or(|last| *last == level_now || *last + 1 == level_now);
        if !in_step {
            failures.push(format!("{}: the effect carried levels {levels:?}, the skill is at {level_now}", skill.name));
        }

        // Up to the top, each level's experience as MagicType has it.
        let mut level_line = Vec::new();
        loop {
            let progress = me.skill_progress(skill.id).expect("learned");
            let need = crate::skills::experience_to_level(skill.id, progress.level).unwrap_or(0);
            level_line.push(format!("{}:{}%/{}", progress.level, skill.at_level(progress.level).chance, need));
            if need == 0 {
                break;
            }
            let left = need.saturating_sub(progress.proficiency);
            let next = me.practice_skill(state.pool(), skill.id, left).await?.expect("learned");
            if next.level != progress.level + 1 {
                failures.push(format!("{}: {left} experience took level {} to {}", skill.name, progress.level, next.level));
                break;
            }
        }
        let top = me.skill_progress(skill.id).expect("learned").level;
        let top_chance = skill.at_level(top).chance;
        let (top_casts, top_levels, _) = swing(&state, &actor, &sent, &kind, skill.id).await?;
        if !fits(top_casts, SWINGS, top_chance) {
            failures.push(format!("{}: {top_casts} casts in {SWINGS} swings at level {top}, {top_chance}%", skill.name));
        }
        if top_levels.iter().any(|level| *level != top) {
            failures.push(format!("{}: at level {top} the effect carried {:?}", skill.name, top_levels.first()));
        }
        println!(
            "| {} | {} | {} ({weapon}) | {book} | {} | {casts}/{SWINGS} ({chance}%) | {updates} | {sent_levels:?} | {experience}, {level_now} | {top_casts}/{SWINGS} ({top_chance}%) at {top} | {:?} |",
            skill.name,
            skill.id,
            weapon_type,
            progress.level,
            top_levels.first(),
        );
        println!("|   levels (level:chance/exp to next) | {} |", level_line.join(" "));
    }

    // Phoenix's last level waits for a level 110 hero (MagicType 5030
    // level 8 `NeedLevel`).
    let phoenix = 5_030;
    let (actor, _sent) = hero(&state, 99, "ws-gate").await?;
    let entity = actor.entity();
    let me = entity.as_character().expect("character");
    crate::systems::commands::parse_and_execute(&state, &actor, &["level", "100"]).await?;
    me.learn_skill(state.pool(), phoenix, 9).await?;
    let need = crate::skills::experience_to_level(phoenix, 9).expect("level 9 has a next");
    let held = me.practice_skill(state.pool(), phoenix, need).await?.expect("learned");
    println!();
    println!("gate: a level 100 hero with Phoenix 9 earns {need}: level {} ({} / {need})", held.level, held.proficiency);
    if held.level != 9 || held.proficiency != need {
        failures.push(format!("Phoenix went past 9 at level 100: {held:?}"));
    }
    crate::systems::commands::parse_and_execute(&state, &actor, &["level", "110"]).await?;
    let raised = me.practice_skill(state.pool(), phoenix, 1).await?.expect("learned");
    println!("gate: at level 110 one more: level {}", raised.level);
    if raised.level != 10 {
        failures.push(format!("Phoenix stayed at {} at level 110", raised.level));
    }

    // On the training stakes a plain hit sets the weapon skill off too.
    let (actor, sent) = hero(&state, 98, "ws-stake").await?;
    let entity = actor.entity();
    let me = entity.as_character().expect("character");
    let sword = weapon_of(&state, 420).expect("a sword");
    let uid = give(&state, &actor, sword).await?;
    me.equip_inventory_item(state.pool(), uid, 4).await?.expect("equipped");
    me.learn_skill(state.pool(), phoenix, 1).await?;
    // The Training Ground (1039) has stakes of every level.
    me.entity().set_map_id(1039);
    let map = state.try_map(1039)?;
    map.load().await?;
    let stake = map
        .training_goals()
        .into_iter()
        .filter(|npc| npc.level() <= 130)
        .max_by_key(|npc| npc.level())
        .map(|npc| (npc.id(), npc.entity().location(), npc.level()))
        .expect("a stake for level 130");
    let mut beside = stake.1;
    beside.x += 1;
    me.entity().set_location(beside);
    let mut casts = 0;
    let mut plain = 0;
    for _ in 0..SWINGS {
        crate::packets::forget_attack_cooldown(me.id());
        crate::packets::forget_training_cooldown(me.id());
        sent.lock().expect("log").clear();
        MsgInteract {
            timestamp: 0,
            character_id: me.id(),
            target_id: stake.0,
            x: stake.1.x,
            y: stake.1.y,
            action: 2,
            value: 0,
            reserved: [0; 12],
        }
        .process(&state, &actor)
        .await?;
        tokio::task::yield_now().await;
        tokio::time::sleep(std::time::Duration::from_millis(1)).await;
        let log = sent.lock().expect("log");
        casts += log
            .iter()
            .filter(|(id, body)| *id == SKILL_EFFECT && u32::from(u16_at(body, 8)) == phoenix)
            .count() as u32;
        plain += log.iter().filter(|(id, _)| *id == 1022).count() as u32;
    }
    let progress = me.skill_progress(phoenix).expect("learned");
    let chance = state.skill_catalog().get(phoenix).expect("Phoenix").at_level(1).chance;
    println!();
    println!(
        "stake: level {} stake {}: {casts} Phoenix in {SWINGS} plain hits ({chance}%), {plain} plain hits shown, skill experience {}",
        stake.2, stake.0, progress.experience
    );
    if !fits(casts, SWINGS, chance) || progress.experience == 0 {
        failures.push(format!("stake: {casts} Phoenix in {SWINGS}, experience {}", progress.experience));
    }

    println!();
    for failure in &failures {
        println!("FAIL {failure}");
    }
    assert!(failures.is_empty(), "{} failures", failures.len());
    Ok(())
}
