//! Both rebirths through Alex, with the answers his dialog sends: a level 120
//! Trojan reborn as a Trojan (CruelShade), back to 120 and reborn again as a
//! Warrior (Hercules forgotten, CruelShade kept, both bonuses), and no third.
//!
//! It needs the item table (`ITEMTYPE_DATA`, as the server's `.env` sets
//! it) and the test database; without the table it is skipped.

use tq_network::Actor;

use crate::entities::Character;
use crate::{ActorState, Error, State};

const CELESTIAL_STONE: u32 = 721_259;
const EXEMPTION_TOKEN: u32 = 723_701;
const HERCULES: u32 = 1_115;
/// Alex's Blessed Rebirth answers: 170 + the new profession.
const TO_TROJAN: u8 = 171;
const TO_WARRIOR: u8 = 172;

async fn first_rebirth_level(state: &State, me: &Character) -> Result<i32, Error> {
    Ok(sqlx::query_scalar("SELECT first_rebirth_level FROM characters WHERE character_id = $1")
        .bind(i64::from(me.database_id()))
        .fetch_one(state.pool())
        .await?)
}

#[tokio::test]
async fn rebirth_journey() -> Result<(), Error> {
    if std::env::var_os("ITEMTYPE_DATA").is_none() {
        let table = std::path::Path::new(r"R:\gravak-co\content\build\client\ini\itemtype.dat");
        if !table.exists() {
            eprintln!("rebirth_journey skipped: no item table");
            return Ok(());
        }
        std::env::set_var("ITEMTYPE_DATA", table);
    }
    let state = State::with_pool(tq_db::testing::pool().await).await?;
    if state.shop_catalog().template(CELESTIAL_STONE).is_none() || state.shop_catalog().template(EXEMPTION_TOKEN).is_none() {
        eprintln!("rebirth_journey skipped: the item table lacks the rebirth items");
        return Ok(());
    }
    let (tx, mut replies) = tokio::sync::mpsc::channel(64);
    tokio::spawn(async move { while replies.recv().await.is_some() {} });
    let actor = Actor::<ActorState>::new(tx);
    actor.set_id(1);
    let inner = crate::packets::MsgRegister::build_character_with(
        "rebirth-test".to_owned(),
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
    let entity = actor.entity();
    let me = entity.as_character().expect("character");
    let gm = |args: &'static [&'static str]| crate::systems::commands::parse_and_execute(&state, &actor, args);

    // A Trojan Master at 120 who knows Hercules.
    me.set_class(state.pool(), 15).await?;
    gm(&["level", "120"]).await?;
    me.learn_skill(state.pool(), HERCULES, 1).await?;

    // Without a CelestialStone nothing happens.
    crate::market_npcs::answer(&state, &actor, me, crate::market_npcs::ALEX_ID, TO_TROJAN).await?;
    assert_eq!(me.rebirths(), 0, "no stone, no rebirth");

    gm(&["item", "721259"]).await?;
    crate::market_npcs::answer(&state, &actor, me, crate::market_npcs::ALEX_ID, TO_TROJAN).await?;
    assert_eq!((me.rebirths(), me.current_class(), me.entity().level()), (1, 11, 15));
    assert_eq!(me.attribute_points(), 20, "a rebirth at 120 leaves 20 points");
    assert!(me.has_skill(crate::skills::CRUEL_SHADE_ID), "a pure Trojan learns CruelShade");
    assert!(me.has_skill(HERCULES), "a Trojan keeps Hercules");
    assert_eq!(me.count_items(CELESTIAL_STONE), 0, "the stone was spent");
    assert_eq!(first_rebirth_level(&state, me).await?, 120);
    println!("first rebirth: Trojan, 20 points, CruelShade");

    // Back to a Trojan Master at 120, then reborn as a Warrior.
    me.set_class(state.pool(), 15).await?;
    gm(&["level", "120"]).await?;
    let points_before = me.attribute_points();
    gm(&["item", "723701"]).await?;
    crate::market_npcs::answer(&state, &actor, me, crate::market_npcs::ALEX_ID, TO_WARRIOR).await?;
    assert_eq!((me.rebirths(), me.current_class(), me.entity().level()), (2, 21, 15));
    assert_eq!(me.attribute_points(), 20 + 20, "both rebirths' bonuses, not the {points_before} left before");
    assert!(!me.has_skill(HERCULES), "Trojan to Warrior forgets Hercules");
    assert!(me.has_skill(crate::skills::CRUEL_SHADE_ID), "what the first rebirth taught stays");
    assert_eq!(me.count_items(EXEMPTION_TOKEN), 0, "the token was spent");
    println!("second rebirth: Warrior, 40 points, Hercules forgotten, CruelShade kept");

    // A third is refused.
    me.set_class(state.pool(), 25).await?;
    gm(&["level", "130"]).await?;
    gm(&["item", "723701"]).await?;
    crate::market_npcs::answer(&state, &actor, me, crate::market_npcs::ALEX_ID, TO_TROJAN).await?;
    assert_eq!(me.rebirths(), 2, "no third rebirth");
    assert_eq!(me.count_items(EXEMPTION_TOKEN), 1, "nothing was spent");
    Ok(())
}
