//! A weapon taken from Normal to a Super +12 with two gems, through the
//! NPCs a player uses (the same answers their dialogs send): MagicArtisan
//! for the quality, GodlyArtisan for the sockets, the gem socket packet,
//! WuxingOven for +1..+9 with +n Stones, +9..+12 with DragonBalls and a
//! DBScroll pack, and the HP enchantment, then Ethereal for the bless.
//!
//! It needs the item table (`ITEMTYPE_DATA`, as the server's `.env` sets
//! it) and the test database; without the table it is skipped. Run with
//! `cargo test -p game forge_journey -- --nocapture` to read each step.

use tq_network::Actor;

use crate::entities::Character;
use crate::packets::MsgSocketGem;
use crate::{ActorState, Error, State};

const BLADE: u32 = 410_205;
const DRAGONBALL: u32 = 1_088_000;
const DBSCROLL: u32 = 720_028;
const PLUS_EIGHT_STONE: u32 = 730_008;
const DRAGON_GEM: u32 = 700_013;
const SUPER_TORTOISE_GEM: u32 = 700_073;
const WEAPON: u8 = 4;

fn character(actor: &Actor<ActorState>) -> std::sync::Arc<crate::entities::GameEntity> {
    actor.entity()
}

/// Items from the GM's `$item` (`--count` above one).
async fn give(state: &State, actor: &Actor<ActorState>, item_type: u32, count: u8) -> Result<(), Error> {
    let item_type = item_type.to_string();
    let count = count.to_string();
    let args: Vec<&str> = if count == "1" { vec!["item", &item_type] } else { vec!["item", &item_type, "--count", &count] };
    crate::systems::commands::parse_and_execute(state, actor, &args).await
}

fn weapon(me: &Character) -> crate::entities::InventoryItem {
    me.equipment_item(WEAPON).expect("the weapon is worn")
}

fn count(me: &Character, item_type: u32) -> usize {
    me.count_items(item_type)
}

#[tokio::test]
async fn forge_journey() -> Result<(), Error> {
    if std::env::var_os("ITEMTYPE_DATA").is_none() {
        let table = std::path::Path::new(r"R:\gravak-co\content\build\client\ini\itemtype.dat");
        if !table.exists() {
            eprintln!("forge_journey skipped: no item table");
            return Ok(());
        }
        std::env::set_var("ITEMTYPE_DATA", table);
    }
    let state = State::with_pool(tq_db::testing::pool().await).await?;
    if state.shop_catalog().template(BLADE).is_none() {
        eprintln!("forge_journey skipped: the item table has no {BLADE}");
        return Ok(());
    }
    let (tx, mut replies) = tokio::sync::mpsc::channel(64);
    // What the server says goes nowhere, as fast as it comes.
    tokio::spawn(async move { while replies.recv().await.is_some() {} });
    let actor = Actor::<ActorState>::new(tx);
    actor.set_id(1);
    let inner = crate::packets::MsgRegister::build_character_with(
        "forge-test".to_owned(),
        crate::packets::BodyType::MuscularMale,
        crate::packets::BaseClass::Trojan,
        1,
        1,
    )?;
    inner.save(state.pool()).await?;
    let inner = tq_db::character::Character::from_account(state.pool(), 1).await?.expect("character");
    let screen = crate::systems::Screen::new(actor.handle());
    actor.update(Character::new(actor.handle(), inner), screen);
    actor.set_gm(true);
    let entity = character(&actor);
    let me = entity.as_character().expect("character");
    crate::systems::commands::parse_and_execute(&state, &actor, &["level", "130"]).await?;

    // A Normal SunBlade. It asks for level 105, 160 strength and blade
    // proficiency 11: a level 130 Trojan has the first two, ProficiencyGod
    // the third, one level for (level + 1) ExpBalls.
    crate::systems::commands::parse_and_execute(&state, &actor, &["item", &BLADE.to_string()]).await?;
    let blade = me.inventory_items().into_iter().find(|item| item.item_type == BLADE).expect("blade");
    let template = state.shop_catalog().template(BLADE).expect("template");
    // The first proficiency level comes from fighting with a blade: the
    // newbie's, not this one (it counts for every blade).
    me.practice_weapon_proficiency(state.pool(), 410_003, 1).await?;
    assert!(!me.meets_item_requirements(BLADE, &template), "proficiency 1 is not enough");
    let mut spent = 0;
    while me.weapon_type_proficiency(410).map_or(0, |progress| progress.level) < u16::from(template.required_weapon_skill) {
        let level = me.weapon_type_proficiency(410).expect("blade proficiency").level;
        if count(me, crate::leveling::EXP_BALL) <= usize::from(level) {
            give(&state, &actor, crate::leveling::EXP_BALL, 30).await?;
        }
        let before = count(me, crate::leveling::EXP_BALL);
        crate::market_npcs::answer(&state, &actor, me, crate::market_npcs::PROFICIENCY_GOD_ID, 200).await?;
        assert!(me.weapon_type_proficiency(410).expect("blade proficiency").level > level, "ProficiencyGod raised it");
        spent += before - count(me, crate::leveling::EXP_BALL);
    }
    println!("ProficiencyGod: blade proficiency 1 -> {}, {spent} ExpBalls", template.required_weapon_skill);
    while count(me, crate::leveling::EXP_BALL) > 0 {
        me.consume_items(state.pool(), crate::leveling::EXP_BALL, 1).await?;
    }
    assert!(me.meets_item_requirements(BLADE, &template), "the blade can be worn");
    me.equip_inventory_item(state.pool(), blade.uid, WEAPON).await?.expect("equipped");
    println!("start: {} (quality {})", weapon(me).item_type, weapon(me).item_type % 10);

    // 1. MagicArtisan: Normal -> Refined -> Unique -> Elite -> Super, always.
    // 19 for the quality, 6 for the sockets (the bag holds 40).
    give(&state, &actor, DRAGONBALL, 25).await?;
    while weapon(me).item_type % 10 != 9 {
        let before = (weapon(me).item_type, count(me, DRAGONBALL));
        crate::artisans::answer(&state, &actor, me, crate::artisans::MAGIC_ARTISAN_ID, 20 + WEAPON).await?;
        let after = weapon(me).item_type;
        assert_ne!(after, before.0, "MagicArtisan upgraded the quality");
        println!("MagicArtisan: {} -> {after}, {} DragonBalls", before.0, before.1 - count(me, DRAGONBALL));
    }

    // 2. GodlyArtisan: the weapon's first socket (1 DragonBall), then the
    // second (5).
    for socket in 1..=2 {
        let before = count(me, DRAGONBALL);
        crate::market_npcs::answer(&state, &actor, me, crate::market_npcs::GODLY_ARTISAN_ID, 10).await?;
        let item = weapon(me);
        let open = if socket == 1 { item.socket_one != 0 || item.gem_one != 0 } else { item.socket_two != 0 || item.gem_two != 0 };
        assert!(open, "socket {socket} opened");
        println!("GodlyArtisan: socket {socket} open, {} DragonBalls", before - count(me, DRAGONBALL));
    }

    // 3. Two Super DragonGems in them (the item menu's GEM SOCKET).
    give(&state, &actor, DRAGON_GEM, 2).await?;
    for socket in 1..=2_u16 {
        let gem = me.inventory_items().into_iter().find(|item| item.item_type == DRAGON_GEM).expect("gem");
        let packet = MsgSocketGem {
            item_id: weapon(me).uid,
            gem_id: gem.uid,
            socket,
            subtype: 0,
        };
        tq_network::PacketProcess::process(&packet, &state, &actor).await?;
    }
    let item = weapon(me);
    assert_eq!((item.gem_one, item.gem_two), (13, 13), "two Super DragonGems set");
    println!("Gems: {} and {}", item.gem_one, item.gem_two);

    // 4. WuxingOven +1..+9: two +8 Stones make it +8, two more +9.
    give(&state, &actor, PLUS_EIGHT_STONE, 4).await?;
    for expected in [8, 9] {
        crate::city_npcs::answer(&state, &actor, me, crate::city_npcs::WUXING_OVEN_ID, 60 + WEAPON, "").await?;
        crate::city_npcs::answer(&state, &actor, me, crate::city_npcs::WUXING_OVEN_ID, 198, "").await?;
        assert_eq!(weapon(me).plus, expected, "two +8 Stones composed");
        println!("WuxingOven: two +8 Stones -> +{expected}");
    }

    // 5. WuxingOven +9 -> +12: 12 DragonBalls, 25 DragonBalls, 4 DBScrolls.
    give(&state, &actor, DRAGONBALL, 37).await?;
    for (group, cost) in [(1, "12 DragonBalls"), (2, "25 DragonBalls"), (3, "4 DBScrolls")] {
        if group == 3 {
            give(&state, &actor, DBSCROLL, 4).await?;
        }
        let before = weapon(me).plus;
        crate::city_npcs::answer(&state, &actor, me, crate::city_npcs::WUXING_OVEN_ID, group * 10 + WEAPON, "").await?;
        assert_eq!(weapon(me).plus, before + 1, "WuxingOven raised the plus with {cost}");
        println!("WuxingOven: +{before} -> +{}, {cost}", before + 1);
    }

    // 6. WuxingOven: a Super gem melted for HP.
    give(&state, &actor, DRAGON_GEM, 1).await?;
    crate::city_npcs::answer(&state, &actor, me, crate::city_npcs::WUXING_OVEN_ID, 100 + WEAPON * 10 + 3, "").await?;
    println!("WuxingOven: HP enchant +{}", weapon(me).enchant_hp);

    // 7. Ethereal: a blessed item (a rebirth blesses one) gains 2 a step
    // for as many Super TortoiseGems as its bless, up to -7%.
    me.bless_item(state.pool(), weapon(me).uid, 1).await?;
    give(&state, &actor, SUPER_TORTOISE_GEM, 12).await?;
    while weapon(me).bless < 7 {
        let before = (weapon(me).bless, count(me, SUPER_TORTOISE_GEM));
        crate::market_npcs::answer(&state, &actor, me, crate::market_npcs::ETHEREAL_ID, 20 + WEAPON).await?;
        assert!(weapon(me).bless > before.0, "Ethereal raised the bless");
        println!(
            "Ethereal: -{}% -> -{}%, {} Super TortoiseGems",
            before.0,
            weapon(me).bless,
            before.1 - count(me, SUPER_TORTOISE_GEM)
        );
    }

    let item = weapon(me);
    println!(
        "done: {} +{} gems {}/{} HP +{} bless -{}%",
        item.item_type, item.plus, item.gem_one, item.gem_two, item.enchant_hp, item.bless
    );
    assert_eq!((item.item_type % 10, item.plus, item.bless), (9, 12, 7));
    Ok(())
}

/// The item menu on a blade in the bag: the NPCs' prices, item for item.
#[tokio::test]
async fn menu_journey() -> Result<(), Error> {
    if std::env::var_os("ITEMTYPE_DATA").is_none() {
        let table = std::path::Path::new(r"R:\gravak-co\content\build\client\ini\itemtype.dat");
        if !table.exists() {
            return Ok(());
        }
        std::env::set_var("ITEMTYPE_DATA", table);
    }
    let state = State::with_pool(tq_db::testing::pool().await).await?;
    if state.shop_catalog().template(BLADE).is_none() {
        return Ok(());
    }
    let (tx, mut replies) = tokio::sync::mpsc::channel(64);
    tokio::spawn(async move { while replies.recv().await.is_some() {} });
    let actor = Actor::<ActorState>::new(tx);
    actor.set_id(2);
    let inner = crate::packets::MsgRegister::build_character_with(
        "menu-test".to_owned(),
        crate::packets::BodyType::MuscularMale,
        crate::packets::BaseClass::Trojan,
        2,
        1,
    )?;
    inner.save(state.pool()).await?;
    let inner = tq_db::character::Character::from_account(state.pool(), 2).await?.expect("character");
    let screen = crate::systems::Screen::new(actor.handle());
    actor.update(Character::new(actor.handle(), inner), screen);
    actor.set_gm(true);
    let entity = character(&actor);
    let me = entity.as_character().expect("character");
    crate::systems::commands::parse_and_execute(&state, &actor, &["level", "130"]).await?;
    give(&state, &actor, BLADE, 1).await?;
    let uid = me.inventory_items().into_iter().find(|item| item.item_type == BLADE).expect("blade").uid;
    let blade = |me: &Character| me.inventory_items().into_iter().find(|item| item.uid == uid).expect("in the bag");

    // QUALITY: 2 + 4 + 5 + 8 DragonBalls, as MagicArtisan.
    give(&state, &actor, DRAGONBALL, 25).await?;
    let mut spent = Vec::new();
    while blade(me).item_type % 10 != 9 {
        let before = count(me, DRAGONBALL);
        crate::forge::quality(&state, &actor, me, uid).await?;
        spent.push(before - count(me, DRAGONBALL));
    }
    assert_eq!(spent, [2, 4, 5, 8], "the menu's quality costs MagicArtisan's");

    // GEM SOCKET: a weapon's 1 and 5 DragonBalls, as GodlyArtisan.
    for cost in [1, 5] {
        let before = count(me, DRAGONBALL);
        crate::forge::socket(&state, &actor, me, uid).await?;
        assert_eq!(before - count(me, DRAGONBALL), cost);
    }
    let item = blade(me);
    assert!(item.socket_one != 0 && item.socket_two != 0, "two sockets");

    // UPGRADE: two +8 Stones twice, then WuxingOven's +9 to +12 prices.
    give(&state, &actor, PLUS_EIGHT_STONE, 4).await?;
    crate::forge::plus(&state, &actor, me, uid).await?;
    crate::forge::plus(&state, &actor, me, uid).await?;
    assert_eq!(blade(me).plus, 9);
    give(&state, &actor, DRAGONBALL, 37).await?;
    for cost in [12, 25] {
        let before = count(me, DRAGONBALL);
        crate::forge::plus(&state, &actor, me, uid).await?;
        assert_eq!(before - count(me, DRAGONBALL), cost);
    }
    give(&state, &actor, DBSCROLL, 4).await?;
    crate::forge::plus(&state, &actor, me, uid).await?;
    assert_eq!((blade(me).plus, count(me, DBSCROLL)), (12, 0));

    // LEVEL: Meteors as MagicArtisan (a Super weapon's level costs 21).
    give(&state, &actor, crate::forge::METEOR, 25).await?;
    let before = (blade(me).item_type, count(me, crate::forge::METEOR));
    crate::forge::level(&state, &actor, me, uid).await?;
    let item = blade(me);
    assert!(item.item_type > before.0 && item.item_type % 10 == 9, "next level, still Super");
    assert_eq!(
        before.1 - count(me, crate::forge::METEOR),
        crate::artisans::level_cost(before.0) as usize,
        "the menu's level costs MagicArtisan's"
    );
    Ok(())
}
