//! Test characters made in the development database with the game's own
//! code, for trying features by hand. Not run with the other tests: it
//! writes to the database named by `GRAVAK_SEED_DATABASE_URL`.
//!
//! A Water Saint (Taoist 135) at level 130 with the healer's skills and
//! potions, for the helper's heals and buffs:
//!
//! ```text
//! GRAVAK_SEED_DATABASE_URL=postgres://gravak:gravak@localhost:5432/gravak \
//! GRAVAK_SEED_ACCOUNT=<account> GRAVAK_SEED_PASSWORD=<password> GRAVAK_SEED_NAME=<character> \
//! cargo test -p game seed_water_taoist -- --ignored --nocapture
//! ```
//!
//! The account is locked by `scripts/prepare-launch.ps1` before players
//! come.

use tq_network::Actor;

use crate::entities::Character;
use crate::{ActorState, Error, State};

/// Every skill a Water Saint of level 130 has, at its highest level.
const WATER_SKILLS: [(u32, u16); 12] = [
    (1_000, 4), // Thunder
    (1_001, 4), // Fire
    (1_005, 4), // Cure
    (1_055, 4), // Healing Rain
    (1_175, 4), // Advanced Cure
    (1_170, 4), // Nectar
    (1_100, 4), // Pray
    (1_095, 4), // Stigma
    (1_090, 4), // Magic Shield
    (1_085, 4), // Star of Accuracy
    (1_075, 4), // Invisibility
    (1_195, 4), // Meditation
];

/// Potions for the helper's slots: (item type, how many).
const POTIONS: [(u32, u32); 3] = [
    (1_002_020, 12), // life, 2000
    (1_001_040, 12), // mana, 2000
    (1_000_030, 6),  // life, 500
];

#[tokio::test]
#[ignore]
async fn seed_water_taoist() -> Result<(), Error> {
    let env = |key: &str| std::env::var(key).unwrap_or_else(|_| panic!("set {key}"));
    let url = env("GRAVAK_SEED_DATABASE_URL");
    let (username, password, name) = (env("GRAVAK_SEED_ACCOUNT"), env("GRAVAK_SEED_PASSWORD"), env("GRAVAK_SEED_NAME"));
    std::env::set_var("DATA_LOCATION", concat!(env!("CARGO_MANIFEST_DIR"), "/../../data"));
    std::env::set_var("SKILLS_DATA", concat!(env!("CARGO_MANIFEST_DIR"), "/data/skills.dat"));
    let pool = sqlx::PgPool::connect(&url).await?;
    let taken: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM accounts WHERE username = $1)")
        .bind(&username)
        .fetch_one(&pool)
        .await?;
    assert!(!taken, "the account {username} already exists");
    let account = tq_db::account::Account { username: username.clone(), password, ..Default::default() }
        .create(&pool)
        .await?;
    let account_id = u32::try_from(account.account_id).expect("account id");

    let state = State::with_pool(pool.clone()).await?;
    let inner = crate::packets::MsgRegister::build_character_with(
        name.clone(),
        crate::packets::BodyType::AgileFemale,
        crate::packets::BaseClass::Taoist,
        account_id,
        1,
    )?;
    inner.save(&pool).await?;
    let inner = tq_db::character::Character::from_account(&pool, account_id).await?.expect("character");
    let (tx, mut replies) = tokio::sync::mpsc::channel(4096);
    tokio::spawn(async move { while replies.recv().await.is_some() {} });
    let actor = Actor::<ActorState>::new(tx);
    actor.set_id(account_id as usize);
    let screen = crate::systems::Screen::new(actor.handle());
    actor.update(Character::new(actor.handle(), inner), screen);
    let entity = actor.entity();
    let me = entity.as_character().expect("character");

    // Water Saint at 130, life and mana full.
    me.set_class(&pool, 135).await?;
    me.change_level(130);
    me.entity().heal_health(u16::MAX);
    me.entity().restore_mana(u16::MAX);
    for (skill, level) in WATER_SKILLS {
        me.learn_skill(&pool, skill, level).await?;
    }
    me.save(&state).await?;
    let character_id = i64::from(me.database_id());

    // Potions in the bag.
    let mut position = 0_i64;
    for (item_type, count) in POTIONS {
        for _ in 0..count {
            sqlx::query(
                "INSERT INTO inventory_items (character_id, item_type, position, durability, max_durability) VALUES ($1, $2, $3, 1, 1)",
            )
            .bind(character_id)
            .bind(i64::from(item_type))
            .bind(position)
            .execute(&pool)
            .await?;
            position += 1;
        }
    }
    println!(
        "Made {name} (Water Saint, level {}) on account {username}: {} skills, {position} potions",
        me.entity().level(),
        WATER_SKILLS.len()
    );
    Ok(())
}
