//! Test characters made in the development database with the game's own
//! code, for trying features by hand. Not run with the other tests: they
//! write to the database named by `GRAVAK_SEED_DATABASE_URL`.
//!
//! One hero of level 130 a run, of `GRAVAK_SEED_CLASS`: `water` (a Water
//! Saint with the healer's skills and potions), `trojan`, `warrior` or
//! `archer` (with their skills and the best weapons a level 130 wears):
//!
//! ```text
//! GRAVAK_SEED_DATABASE_URL=postgres://gravak:gravak@localhost:5432/gravak \
//! GRAVAK_SEED_CLASS=trojan GRAVAK_SEED_ACCOUNT=<account> GRAVAK_SEED_PASSWORD=<password> \
//! GRAVAK_SEED_NAME=<character> cargo test -p game seed_hero -- --ignored --nocapture
//! ```
//!
//! The accounts are game masters (for `$monster`, `$go`...) and are locked
//! by `scripts/prepare-launch.ps1` before players come.

use tq_network::Actor;

use crate::entities::Character;
use crate::packets::BaseClass;
use crate::{ActorState, Error, State};

/// A kind of test hero: its class, skills, weapons and bag.
struct Kind {
    base: BaseClass,
    /// The class at level 130 (Water Saint 135, Trojan Master 15...).
    class: u8,
    skills: &'static [u32],
    /// (weapon family, equipment position): the best of each is worn.
    weapons: &'static [(u32, u8)],
    /// Worn as they are: (item type, position).
    worn: &'static [(u32, u8)],
    /// (item type, how many) in the bag.
    bag: &'static [(u32, u32)],
}

const LIFE_POTION: u32 = 1_002_020;
const MANA_POTION: u32 = 1_001_040;

fn kind(name: &str) -> Kind {
    match name {
        "water" => Kind {
            base: BaseClass::Taoist,
            class: 135,
            skills: &[
                1_000, 1_001, 1_005, 1_055, 1_175, 1_170, 1_100, 1_095, 1_090, 1_085, 1_075, 1_195, 1_050,
            ],
            weapons: &[],
            worn: &[],
            bag: &[(LIFE_POTION, 12), (MANA_POTION, 12), (1_000_030, 6)],
        },
        // Hercules sweeps (403), FastBlade and ScentSword strike (401),
        // Phoenix comes off the sword.
        "trojan" => Kind {
            base: BaseClass::Trojan,
            class: 15,
            skills: &[1_115, 1_045, 1_046, 1_025, 1_110, 5_030],
            weapons: &[(410, 4), (420, 5)],
            worn: &[],
            bag: &[(LIFE_POTION, 20)],
        },
        // WideStrike comes off the glaive (403).
        "warrior" => Kind {
            base: BaseClass::Warrior,
            class: 25,
            skills: &[1_250, 1_020, 1_040, 1_025, 1_051, 1_015],
            weapons: &[(510, 4)],
            worn: &[],
            bag: &[(LIFE_POTION, 20)],
        },
        // Scatter and RapidFire shoot with the plain blow.
        "archer" => Kind {
            base: BaseClass::Archer,
            class: 45,
            skills: &[8_001, 8_000, 8_030, 8_002, 8_003, 9_000],
            weapons: &[(500, 4)],
            worn: &[(1_050_002, 5)],
            bag: &[(LIFE_POTION, 20), (1_050_002, 5)],
        },
        other => panic!("GRAVAK_SEED_CLASS {other}: water, trojan, warrior or archer"),
    }
}

/// A skill's highest level (MagicType has it up to 10; the server counts
/// from 1).
fn top_level(skill: u32) -> u16 {
    (1..=10).rev().find(|level| crate::magictype::level(skill, *level).is_some()).unwrap_or(1)
}

#[tokio::test]
#[ignore]
async fn seed_hero() -> Result<(), Error> {
    let env = |key: &str| std::env::var(key).unwrap_or_else(|_| panic!("set {key}"));
    let url = env("GRAVAK_SEED_DATABASE_URL");
    let kind = kind(&env("GRAVAK_SEED_CLASS"));
    let (username, password, name) = (env("GRAVAK_SEED_ACCOUNT"), env("GRAVAK_SEED_PASSWORD"), env("GRAVAK_SEED_NAME"));
    std::env::set_var("DATA_LOCATION", concat!(env!("CARGO_MANIFEST_DIR"), "/../../data"));
    std::env::set_var("SKILLS_DATA", concat!(env!("CARGO_MANIFEST_DIR"), "/data/skills.dat"));
    if std::env::var_os("ITEMTYPE_DATA").is_none() {
        std::env::set_var("ITEMTYPE_DATA", r"R:\5065\ini\itemtype.dat");
    }
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
    sqlx::query("UPDATE accounts SET gm = true WHERE account_id = $1")
        .bind(i64::from(account_id))
        .execute(&pool)
        .await?;

    let state = State::with_pool(pool.clone()).await?;
    let body = if matches!(kind.base, BaseClass::Taoist) {
        crate::packets::BodyType::AgileFemale
    } else {
        crate::packets::BodyType::MuscularMale
    };
    let inner = crate::packets::MsgRegister::build_character_with(name.clone(), body, kind.base, account_id, 1)?;
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

    // The class at 130, life and mana full, every skill at its top.
    me.set_class(&pool, kind.class).await?;
    me.change_level(130);
    me.entity().heal_health(u16::MAX);
    me.entity().restore_mana(u16::MAX);
    for skill in kind.skills {
        me.learn_skill(&pool, *skill, top_level(*skill)).await?;
    }
    me.save(&state).await?;
    let character_id = i64::from(me.database_id());

    // The weapons, in place of the starter's.
    let catalog = state.shop_catalog();
    let mut worn: Vec<(u32, u8)> = kind
        .weapons
        .iter()
        .map(|(family, position)| {
            let item = catalog.best_weapon(*family, 130).unwrap_or_else(|| panic!("no weapon of family {family}"));
            (item, *position)
        })
        .collect();
    worn.extend_from_slice(kind.worn);
    for (item_type, position) in &worn {
        sqlx::query("DELETE FROM equipment_items WHERE character_id = $1 AND position = $2")
            .bind(character_id)
            .bind(i64::from(*position))
            .execute(&pool)
            .await?;
        let (durability, max_durability) = crate::shop::fresh_durability(*item_type);
        sqlx::query(
            "INSERT INTO equipment_items (character_id, item_type, position, durability, max_durability) VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(character_id)
        .bind(i64::from(*item_type))
        .bind(i64::from(*position))
        .bind(i64::from(durability))
        .bind(i64::from(max_durability))
        .execute(&pool)
        .await?;
    }

    // The bag.
    let mut position = 0_i64;
    for (item_type, count) in kind.bag {
        let (durability, max_durability) = crate::shop::fresh_durability(*item_type);
        for _ in 0..*count {
            sqlx::query(
                "INSERT INTO inventory_items (character_id, item_type, position, durability, max_durability) VALUES ($1, $2, $3, $4, $5)",
            )
            .bind(character_id)
            .bind(i64::from(*item_type))
            .bind(position)
            .bind(i64::from(durability.max(1)))
            .bind(i64::from(max_durability.max(1)))
            .execute(&pool)
            .await?;
            position += 1;
        }
    }
    println!(
        "Made {name} (class {}, level {}) on account {username}: skills {:?}, worn {worn:?}, {position} items in the bag",
        kind.class,
        me.entity().level(),
        kind.skills.iter().map(|skill| (*skill, top_level(*skill))).collect::<Vec<_>>(),
    );
    Ok(())
}
