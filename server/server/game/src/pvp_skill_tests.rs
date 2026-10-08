//! Skills on players, on a real map: in Twin City (PK allowed) a Trojan in
//! PK mode strikes another hero with an area skill around him (Hercules), a
//! line (FastBlade) and an aimed skill; each hurts and flashes the
//! attacker's name blue. In Peace mode none of them touches a player.
//!
//! It needs the maps (`server/data`) and the test database, and sets the
//! process's `DATA_LOCATION`, so it runs alone:
//! `cargo test -p game pvp_skill_journey -- --ignored --nocapture`.

use tq_network::{Actor, PacketProcess};

use crate::entities::Character;
use crate::packets::MsgInteract;
use crate::{ActorState, Error, State};

const HERCULES: u32 = 1_115;
const FAST_BLADE: u32 = 1_045;
const CRUEL_SHADE: u32 = 3_050;

async fn hero(state: &State, account: u32) -> Result<Actor<ActorState>, Error> {
    hero_of(state, account, crate::packets::BaseClass::Trojan).await
}

async fn hero_of(state: &State, account: u32, class: crate::packets::BaseClass) -> Result<Actor<ActorState>, Error> {
    let (tx, mut replies) = tokio::sync::mpsc::channel(4096);
    tokio::spawn(async move { while replies.recv().await.is_some() {} });
    sqlx::query("INSERT INTO accounts (account_id, username, password) VALUES ($1, $2, 'x') ON CONFLICT (account_id) DO NOTHING")
        .bind(i64::from(account))
        .bind(format!("pvp-test-{account}"))
        .execute(state.pool())
        .await?;
    let actor = Actor::<ActorState>::new(tx);
    actor.set_id(account as usize);
    let inner = crate::packets::MsgRegister::build_character_with(
        format!("pvp{account}"),
        crate::packets::BodyType::MuscularMale,
        class,
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
    Ok(actor)
}

async fn cast(state: &State, actor: &Actor<ActorState>, skill: u32, target: u32, at: (u16, u16)) -> Result<(), Error> {
    let entity = actor.entity();
    let me = entity.as_character().expect("caster");
    me.entity().heal_health(u16::MAX);
    me.set_stamina(150);
    me.entity().restore_mana(u16::MAX);
    crate::packets::forget_training_cooldown(me.id());
    MsgInteract {
        timestamp: 0,
        character_id: me.id(),
        target_id: target,
        x: at.0,
        y: at.1,
        action: 21,
        value: skill,
        reserved: [0; 12],
    }
    .process(state, actor)
    .await
}

#[tokio::test]
#[ignore]
async fn pvp_skill_journey() -> Result<(), Error> {
    std::env::set_var("DATA_LOCATION", concat!(env!("CARGO_MANIFEST_DIR"), "/../../data"));
    std::env::set_var("SKILLS_DATA", concat!(env!("CARGO_MANIFEST_DIR"), "/data/skills.dat"));
    let state = State::with_pool(tq_db::testing::pool().await).await?;
    let attacker = hero(&state, 500).await?;
    let victim = hero(&state, 501).await?;
    let attacker_entity = attacker.entity();
    let me = attacker_entity.as_character().expect("attacker");
    let victim_entity = victim.entity();
    let them = victim_entity.as_character().expect("victim");
    for skill in [HERCULES, FAST_BLADE, CRUEL_SHADE] {
        me.learn_skill(state.pool(), skill, 1).await?;
    }
    // Side by side in Twin City, where PK is allowed.
    let map = state.try_map(me.entity().map_id())?;
    map.load().await?;
    let here = me.entity().location();
    let mut beside = here;
    beside.x += 2;
    them.entity().set_location(beside);
    let at = (beside.x, beside.y);

    // Peace: no skill touches a player.
    me.set_kill_mode(1);
    for skill in [HERCULES, FAST_BLADE] {
        them.entity().heal_health(u16::MAX);
        let before = them.entity().hp().current;
        cast(&state, &attacker, skill, if skill == HERCULES { me.id() } else { 0 }, at).await?;
        assert_eq!(them.entity().hp().current, before, "skill {skill} spares players in Peace");
    }

    // PK mode: each skill hurts and the attacker flashes blue.
    me.set_kill_mode(0);
    for (skill, target) in [(HERCULES, me.id()), (FAST_BLADE, 0), (CRUEL_SHADE, them.id())] {
        them.entity().heal_health(u16::MAX);
        let before = them.entity().hp().current;
        cast(&state, &attacker, skill, target, at).await?;
        assert!(them.entity().hp().current < before, "skill {skill} hurts a player in PK mode");
        println!("skill {skill}: {} -> {}", before, them.entity().hp().current);
    }
    assert!(
        me.entity().flags().contains(crate::entities::Flags::BLUE_FLASHING_NAME),
        "striking a player flashes the name blue"
    );

    // A city guard: spared outside PK mode, struck (a crime) in it.
    // The blue name from striking a player is put out first.
    me.entity().set_flags(me.entity().flags() - crate::entities::Flags::BLUE_FLASHING_NAME);
    let guard_kind = crate::bosses::monster_type(&state, 900).await?.expect("Guard1");
    let guard = crate::bosses::spawn_monster(&state, me.entity().map_id(), (here.x + 1, here.y), &guard_kind, 0)
        .await?
        .expect("room for the guard");
    let guard_at = map.monster_entity(guard).expect("guard").basic().location();
    let mut beside_guard = guard_at;
    beside_guard.x = guard_at.x.saturating_sub(1);
    me.entity().set_location(beside_guard);
    let swing = |me: &Character| MsgInteract {
        timestamp: 0,
        character_id: me.id(),
        target_id: guard,
        x: guard_at.x,
        y: guard_at.y,
        action: 2,
        value: 0,
        reserved: [0; 12],
    };
    me.set_kill_mode(1);
    let before = map.monster_entity(guard).expect("guard").basic().hp().current;
    crate::packets::forget_attack_cooldown(me.id());
    swing(me).process(&state, &attacker).await?;
    assert_eq!(map.monster_entity(guard).expect("guard").basic().hp().current, before, "Peace spares the guards");
    assert!(!me.entity().flags().contains(crate::entities::Flags::BLUE_FLASHING_NAME), "and no crime");
    me.set_kill_mode(0);
    let mut struck = false;
    for _ in 0..20 {
        crate::packets::forget_attack_cooldown(me.id());
        swing(me).process(&state, &attacker).await?;
        if map.monster_entity(guard).expect("guard").basic().hp().current < before {
            struck = true;
            break;
        }
    }
    assert!(struck, "PK mode strikes the guard");
    assert!(me.entity().flags().contains(crate::entities::Flags::BLUE_FLASHING_NAME), "striking a guard is a crime");
    map.remove_monster(guard);
    Ok(())
}

/// A Taoist's Stigma, Magic Shield and Star of Accuracy go on the hero
/// aimed at (the helper's party buffs), and on the caster when none is.
#[tokio::test]
#[ignore]
async fn support_statuses_go_on_the_player_aimed_at() -> Result<(), Error> {
    use crate::entities::StatusEffect;
    std::env::set_var("DATA_LOCATION", concat!(env!("CARGO_MANIFEST_DIR"), "/../../data"));
    std::env::set_var("SKILLS_DATA", concat!(env!("CARGO_MANIFEST_DIR"), "/data/skills.dat"));
    let state = State::with_pool(tq_db::testing::pool().await).await?;
    let caster = hero_of(&state, 512, crate::packets::BaseClass::Taoist).await?;
    let friend = hero(&state, 511).await?;
    let caster_entity = caster.entity();
    let me = caster_entity.as_character().expect("caster");
    let friend_entity = friend.entity();
    let them = friend_entity.as_character().expect("friend");
    let skills = [
        (crate::skills::STIGMA_ID, StatusEffect::Stigma),
        (crate::skills::MAGIC_SHIELD_ID, StatusEffect::Shield),
        (crate::skills::STAR_OF_ACCURACY_ID, StatusEffect::Accuracy),
    ];
    for (skill, _) in skills {
        me.learn_skill(state.pool(), skill, 1).await?;
    }
    let map = state.try_map(me.entity().map_id())?;
    map.load().await?;
    let mut beside = me.entity().location();
    beside.x += 3;
    them.entity().set_location(beside);

    for (skill, status) in skills {
        cast(&state, &caster, skill, them.id(), (beside.x, beside.y)).await?;
        assert!(them.has_status(status), "skill {skill} goes on the player aimed at");
        assert!(!me.has_status(status), "skill {skill} is not the caster's");
    }
    // Out of reach, or no one aimed at: the caster's own.
    let mut far = beside;
    far.x += 30;
    them.entity().set_location(far);
    cast(&state, &caster, crate::skills::STIGMA_ID, them.id(), (far.x, far.y)).await?;
    assert!(me.has_status(StatusEffect::Stigma), "a player out of reach leaves the Stigma on the caster");
    Ok(())
}
