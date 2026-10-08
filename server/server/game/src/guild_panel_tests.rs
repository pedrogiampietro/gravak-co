//! The Guild System window through the requests it sends (9170): founding,
//! a request to join and its acceptance with the entry fee, ranks, the fund,
//! the settings, a war declared, accepted and fought, the deaths kept, an
//! invitation, leaving and disbanding. It checks what the server keeps.

use tq_network::Actor;

use crate::entities::Character;
use crate::guild_panel::*;
use crate::packets::MsgGuildPanel;
use crate::{ActorState, Error, State};

async fn hero(state: &State, account: u32, name: &str) -> Result<Actor<ActorState>, Error> {
    let (tx, mut replies) = tokio::sync::mpsc::channel(4096);
    tokio::spawn(async move { while replies.recv().await.is_some() {} });
    sqlx::query("INSERT INTO accounts (account_id, username, password) VALUES ($1, $2, 'x') ON CONFLICT (account_id) DO NOTHING")
        .bind(i64::from(account))
        .bind(format!("guild-test-{account}"))
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
    for args in [["level", "100"], ["silver", "5000000"]] {
        crate::systems::commands::parse_and_execute(state, &actor, &args).await?;
    }
    Ok(actor)
}

fn request(action: u32, param: u32, amount: u32, texts: [&str; 4]) -> MsgGuildPanel {
    MsgGuildPanel {
        action,
        param,
        amount,
        text1: texts[0].to_owned(),
        text2: texts[1].to_owned(),
        text3: texts[2].to_owned(),
        text4: texts[3].to_owned(),
    }
}

async fn send(state: &State, actor: &Actor<ActorState>, packet: MsgGuildPanel) -> Result<(), Error> {
    let entity = actor.entity();
    let me = entity.as_character().expect("character");
    handle(state, actor, me, &packet).await
}

async fn count(state: &State, sql: &str, guild: u32) -> Result<i64, Error> {
    Ok(sqlx::query_scalar(sql).bind(i64::from(guild)).fetch_one(state.pool()).await?)
}

#[tokio::test]
async fn guild_window_journey() -> Result<(), Error> {
    let state = State::with_pool(tq_db::testing::pool().await).await?;
    let leader = hero(&state, 400, "GuildLeader").await?;
    let recruit = hero(&state, 401, "GuildRecruit").await?;
    let rival = hero(&state, 402, "RivalLeader").await?;
    let invited = hero(&state, 403, "Invited").await?;
    let leader_entity = leader.entity();
    let leader_me = leader_entity.as_character().expect("leader");
    let recruit_entity = recruit.entity();
    let recruit_me = recruit_entity.as_character().expect("recruit");
    let manager = state.guild_manager();

    // Founding: the name, title, motto and description; a million silver.
    let silver = leader_me.silver();
    send(&state, &leader, request(CREATE, 0, 4 | 1 << 8 | 2 << 16, ["Yellow Wall", "Reis Veteranos", "Reis Vitoriosos", "We hold the wall."])).await?;
    let guild = manager.guild_for(leader_me.id()).expect("founded");
    assert_eq!(leader_me.silver(), silver - u64::from(FOUND_PRICE));
    let (title, motto): (String, String) = sqlx::query_as("SELECT title, motto FROM guilds WHERE guild_id = $1")
        .bind(i64::from(guild.id))
        .fetch_one(state.pool())
        .await?;
    assert_eq!((title.as_str(), motto.as_str()), ("Reis Veteranos", "Reis Vitoriosos"));
    assert_eq!(count(&state, "SELECT COUNT(*) FROM guild_events WHERE guild_id = $1 AND kind = 'founded'", guild.id).await?, 1);
    let emblem: i64 = sqlx::query_scalar("SELECT emblem FROM guilds WHERE guild_id = $1")
        .bind(i64::from(guild.id))
        .fetch_one(state.pool())
        .await?;
    assert_eq!(emblem, 4 | 1 << 8 | 2 << 16, "the crest chosen at founding");
    send(&state, &leader, request(EMBLEM, 7 | 3 << 8, 0, ["", "", "", ""])).await?;
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT emblem FROM guilds WHERE guild_id = $1").bind(i64::from(guild.id)).fetch_one(state.pool()).await?,
        7 | 3 << 8,
        "the leader changes it"
    );
    // Those around see the guild under the leader's name.
    let look = crate::packets::MsgPlayer::from(leader_me);
    assert_eq!(look.guild_name, "Yellow Wall");

    // A fee of 1000 silver, then a request to join, accepted: the fee goes
    // to the fund.
    send(&state, &leader, request(SETTINGS, 1, 1_000, ["Reis Veteranos", "Reis Vitoriosos", "We hold the wall.", ""])).await?;
    send(&state, &recruit, request(REQUEST_JOIN, guild.id, 0, ["envita eu", "", "", ""])).await?;
    assert_eq!(count(&state, "SELECT COUNT(*) FROM guild_requests WHERE guild_id = $1", guild.id).await?, 1);
    let recruit_silver = recruit_me.silver();
    send(&state, &leader, request(ACCEPT_REQUEST, recruit_me.database_id() as u32, 0, ["", "", "", ""])).await?;
    assert!(manager.same_guild(leader_me.id(), recruit_me.id()), "the recruit joined");
    assert_eq!(recruit_me.silver(), recruit_silver - 1_000, "they paid the fee");
    assert_eq!(crate::guild::GuildManager::fund(state.pool(), guild.id).await?, 1_000);
    assert_eq!(count(&state, "SELECT COUNT(*) FROM guild_requests WHERE guild_id = $1", guild.id).await?, 0);

    // Ranks: a deputy, and the deputy survives someone else leaving.
    send(&state, &leader, request(PROMOTE, recruit_me.database_id() as u32, 0, ["", "", "", ""])).await?;
    assert_eq!(manager.role_for(recruit_me.id()), Some(crate::guild::DEPUTY_ROLE));

    // The fund: a deposit, a withdrawal the deputy may not make.
    send(&state, &recruit, request(DEPOSIT, 0, 5_000, ["", "", "", ""])).await?;
    send(&state, &recruit, request(WITHDRAW, 0, 100, ["", "", "", ""])).await?;
    assert_eq!(crate::guild::GuildManager::fund(state.pool(), guild.id).await?, 6_000);
    send(&state, &leader, request(WITHDRAW, 0, 2_000, ["", "", "", ""])).await?;
    assert_eq!(crate::guild::GuildManager::fund(state.pool(), guild.id).await?, 4_000);
    assert_eq!(
        count(&state, "SELECT COUNT(*) FROM guild_events WHERE guild_id = $1 AND kind IN ('deposit', 'withdraw', 'fee')", guild.id).await?,
        3
    );

    // An invitation, accepted from the invited's window.
    send(&state, &leader, request(INVITE, 0, 0, ["Invited", "", "", ""])).await?;
    let invited_entity = invited.entity();
    let invited_me = invited_entity.as_character().expect("invited");
    send(&state, &invited, request(ACCEPT_INVITE, guild.id, 0, ["", "", "", ""])).await?;
    assert!(manager.same_guild(leader_me.id(), invited_me.id()), "the invited joined (paying the fee)");

    // A war: declared, accepted, and a kill counted.
    send(&state, &rival, request(CREATE, 0, 0, ["Red Tide", "", "", ""])).await?;
    let rival_entity = rival.entity();
    let rival_me = rival_entity.as_character().expect("rival");
    let enemy = manager.guild_for(rival_me.id()).expect("the rival's guild");
    send(&state, &leader, request(DECLARE_WAR, 0, 0, ["Red Tide", "", "", ""])).await?;
    let war: i64 = sqlx::query_scalar("SELECT war_id FROM guild_wars WHERE guild_id = $1 AND state = 'pending'")
        .bind(i64::from(guild.id))
        .fetch_one(state.pool())
        .await?;
    send(&state, &rival, request(ANSWER_WAR, war as u32, 1, ["", "", "", ""])).await?;
    assert_eq!(manager.relation(guild.id, enemy.id).as_deref(), Some("enemy"));
    record_death(&state, rival_me, recruit_me.id(), Some(recruit_me)).await?;
    assert_eq!(count(&state, "SELECT COUNT(*) FROM guild_war_kills WHERE killer_guild_id = $1", guild.id).await?, 1);
    assert_eq!(count(&state, "SELECT COUNT(*) FROM guild_deaths WHERE guild_id = $1 AND kind = 'frag'", guild.id).await?, 1);
    assert_eq!(count(&state, "SELECT COUNT(*) FROM guild_deaths WHERE guild_id = $1 AND kind = 'death'", enemy.id).await?, 1);
    send(&state, &rival, request(END_WAR, war as u32, 0, ["", "", "", ""])).await?;
    assert_eq!(manager.relation(guild.id, enemy.id), None, "peace again");

    // The invited leaves; the deputy keeps their rank.
    send(&state, &invited, request(LEAVE, 0, 0, ["", "", "", ""])).await?;
    assert!(manager.guild_for(invited_me.id()).is_none());
    let role: i64 = sqlx::query_scalar("SELECT role FROM guild_members WHERE character_id = $1")
        .bind(i64::from(recruit_me.database_id()))
        .fetch_one(state.pool())
        .await?;
    assert_eq!(role, i64::from(crate::guild::DEPUTY_ROLE), "a removal no longer demotes the deputies");

    // The leader hands the guild over, then the new leader disbands it.
    send(&state, &leader, request(TRANSFER, recruit_me.database_id() as u32, 0, ["", "", "", ""])).await?;
    assert_eq!(manager.guild_for(leader_me.id()).map(|guild| guild.leader_id), Some(recruit_me.id()));
    assert_eq!(count(&state, "SELECT COUNT(*) FROM guild_events WHERE guild_id = $1", guild.id).await? >= 10, true);
    send(&state, &recruit, request(DISBAND, 0, 0, ["", "", "", ""])).await?;
    assert!(manager.guild_for(leader_me.id()).is_none() && manager.guild_for(recruit_me.id()).is_none());
    assert_eq!(crate::packets::MsgPlayer::from(leader_me).guild_name, "", "no guild once disbanded");

    // The lists open without error with and without a guild.
    for list in [SEARCH, MY_INVITES, MY_REQUESTS] {
        send(&state, &leader, request(PAGE, list, 0, ["", "", "", ""])).await?;
    }
    for list in [MEMBERS, REQUESTS, INVITES, EVENTS, BALANCE, WARS, WAR_KILLS, DEATHS] {
        send(&state, &rival, request(PAGE, list, 0, ["", "", "", ""])).await?;
    }
    send(&state, &rival, request(PAGE, DEATHS, 0, ["deaths:players", "Rival", "", ""])).await?;
    send(&state, &rival, request(OPEN, 0, 0, ["", "", "", ""])).await?;
    Ok(())
}
