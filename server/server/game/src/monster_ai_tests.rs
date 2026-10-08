//! The special rules of monster AI on a real map, through the AI's own tick:
//! a caster strikes with its magic from beyond its blows' reach, a monster
//! leaves a hero far above it alone until it is hit, its pack answers, a
//! weak monster nearly dead runs, and a champion rages.
//!
//! It needs the maps (`server/data`) and the test database, and sets the
//! process's `DATA_LOCATION`, so it runs alone:
//! `cargo test -p game monster_ai_journey -- --ignored --nocapture`.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use tq_network::{Actor, Message};

use crate::entities::Character;
use crate::systems::{monster_ai, monster_rules};
use crate::{ActorState, Error, State};

const SKILL_EFFECT: u16 = 1105;

type Sent = Arc<Mutex<Vec<(u16, Vec<u8>)>>>;

fn u16_at(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}

fn u32_at(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().expect("four bytes"))
}

async fn hero(state: &State, account: u32, level: &str) -> Result<(Actor<ActorState>, Sent), Error> {
    let (tx, mut replies) = tokio::sync::mpsc::channel(1024);
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
    .bind(format!("ai-test-{account}"))
    .execute(state.pool())
    .await?;
    let actor = Actor::<ActorState>::new(tx);
    actor.set_id(account as usize);
    let inner = crate::packets::MsgRegister::build_character_with(
        format!("ai{account}"),
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
    crate::systems::commands::parse_and_execute(state, &actor, &["level", level]).await?;
    Ok((actor, sent))
}

async fn kind(state: &State, id: u32) -> Result<tq_db::monster::MonsterType, Error> {
    Ok(crate::bosses::monster_type(state, id).await?.expect("monster type"))
}

/// A monster `(dx, dy)` from the hero; its id.
async fn put(state: &State, me: &Character, kind: &tq_db::monster::MonsterType, (dx, dy): (i16, i16)) -> Result<u32, Error> {
    let here = me.entity().location();
    let at = ((here.x as i16 + dx) as u16, (here.y as i16 + dy) as u16);
    Ok(crate::bosses::spawn_monster(state, me.entity().map_id(), at, kind, 0).await?.expect("room for it"))
}

/// Ticks the AI for up to `seconds`, until `done`.
async fn tick_until(state: &State, seconds: u64, mut done: impl FnMut() -> bool) -> bool {
    let until = std::time::Instant::now() + Duration::from_secs(seconds);
    while std::time::Instant::now() < until {
        monster_ai::tick(state).await;
        tokio::time::sleep(Duration::from_millis(50)).await;
        if done() {
            return true;
        }
    }
    false
}

#[tokio::test]
#[ignore]
async fn monster_ai_journey() -> Result<(), Error> {
    std::env::set_var("DATA_LOCATION", concat!(env!("CARGO_MANIFEST_DIR"), "/../../data"));
    std::env::set_var("SKILLS_DATA", concat!(env!("CARGO_MANIFEST_DIR"), "/data/skills.dat"));
    let state = State::with_pool(tq_db::testing::pool().await).await?;

    // 1. A caster (FiendBat, Fire) strikes with its magic from 5 cells.
    let (actor, sent) = hero(&state, 300, "110").await?;
    let entity = actor.entity();
    let me = entity.as_character().expect("character");
    let bat = kind(&state, 3_148).await?;
    assert_eq!(bat.magic_skill, 1_001, "FiendBat casts Fire");
    let caster = put(&state, me, &bat, (5, 0)).await?;
    monster_ai::provoke(caster, me.id());
    let cast = tick_until(&state, 6, || {
        sent.lock().expect("log").iter().any(|(id, body)| {
            *id == SKILL_EFFECT && u32_at(body, 0) == caster && u16_at(body, 8) == 1_001
        })
    })
    .await;
    assert!(cast, "the FiendBat cast Fire at the hero");
    println!("caster: FiendBat cast Fire from afar");
    let map = state.try_map(me.entity().map_id())?;
    map.remove_monster(caster);

    // 2. A Pheasant (level 1) leaves a level 110 hero alone until hit; then
    //    its pack answers.
    let pheasant = kind(&state, 1).await?;
    let first = put(&state, me, &pheasant, (2, 0)).await?;
    let second = put(&state, me, &pheasant, (3, 1)).await?;
    tick_until(&state, 1, || false).await;
    assert_eq!(monster_ai::provoker(first), None, "nobody goes after a hero far above");
    monster_ai::provoke(first, me.id());
    let answered = tick_until(&state, 3, || monster_ai::provoker(second) == Some(me.id())).await;
    assert!(answered, "the pack answered");
    println!("level rule and pack: the Pheasants left the hero alone, then answered together");

    // 3. Hurt nearly to death, the Pheasant runs.
    let before = map.monster_entity(first).expect("alive").basic().location();
    let life = map.monster_entity(first).expect("alive").basic().hp();
    map.monster_entity(first)
        .expect("alive")
        .basic()
        .apply_damage(u32::from(life.current).saturating_sub(1));
    let here = me.entity().location();
    let distance = |at: primitives::Location| at.x.abs_diff(here.x).max(at.y.abs_diff(here.y));
    let ran = tick_until(&state, 3, || {
        map.monster_entity(first)
            .is_some_and(|monster| distance(monster.basic().location()) > distance(before))
    })
    .await;
    assert!(ran, "the dying Pheasant ran from the hero");
    println!("flee: the Pheasant ran");
    map.remove_monster(first);
    map.remove_monster(second);

    // 4. A champion (Satan, a field boss) rages under 30%.
    let satan = kind(&state, 3_644).await?;
    assert!(monster_rules::is_champion(0, &satan));
    let boss = put(&state, me, &satan, (2, 0)).await?;
    let life = map.monster_entity(boss).expect("alive").basic().hp();
    map.monster_entity(boss)
        .expect("alive")
        .basic()
        .apply_damage(u32::from(life.max) * 3 / 4);
    monster_ai::provoke(boss, me.id());
    let raged = tick_until(&state, 3, || monster_rules::enraged(boss)).await;
    assert!(raged, "Satan raged");
    println!("champion: Satan raged under 30% life");
    map.remove_monster(boss);

    // 5. The control: a level 1 hero beside a Pheasant is hunted.
    let (low_actor, low_sent) = hero(&state, 301, "1").await?;
    let low_entity = low_actor.entity();
    let low = low_entity.as_character().expect("character");
    let hunter = put(&state, low, &pheasant, (1, 0)).await?;
    let hunted = tick_until(&state, 4, || {
        low_sent
            .lock()
            .expect("log")
            .iter()
            .any(|(id, body)| *id == 1_022 && u32_at(body, 4) == hunter && u32_at(body, 8) == low.id())
    })
    .await;
    assert!(hunted, "a hero of its own level is hunted");
    println!("control: the Pheasant hunted a level 1 hero");
    map.remove_monster(hunter);
    Ok(())
}
