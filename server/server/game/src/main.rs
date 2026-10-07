//! This program encapsulates the game server.
//! The game server is designed to accept authenticated data from the
//! account server, load the player's character data, and control the game
//! world environment. Any game structures involving location and the map
//! are processed on this server. Entity intelligence is processed by this
//! server as well.

use async_trait::async_trait;
use std::env;
use tq_network::{Actor, ActorState as _, PacketHandler, TQCipher};
use tq_server::TQServer;

use game::packets::*;
use game::{ActorState, Error, State};

struct GameServer;

#[async_trait]
impl TQServer for GameServer {
    type ActorState = ActorState;
    type Cipher = TQCipher;
    type PacketHandler = Handler;

    /// Get Called right before ending the connection with that client.
    /// good chance to clean up anything related to that actor.
    #[tracing::instrument(skip(state, actor))]
    async fn on_disconnected(
        state: &<Self::PacketHandler as PacketHandler>::State,
        actor: Actor<Self::ActorState>,
    ) -> Result<(), tq_server::Error> {
        if let Ok(entity) = actor.try_entity() {
            let me = entity.as_character().ok_or(Error::CharacterNotFound)?;
            let mymap_id = me.entity().map_id();
            tracing::info!(target: "login", map = mymap_id, "{} left the world", me.entity().name());
            me.save(state).await?;
            me.try_screen()?.remove_from_observers().await?;
            ActorState::dispose(&actor, actor.handle()).await?;
            state.team_manager().remove_member(me.id());
            state.talismans().set(me.id(), None);
            state.vip().remove(me.id());
            if let Some(game::trade::TradeChange::Closed { owner_id, target_id }) =
                state.trade_manager().remove_member(me.id())
            {
                let peer_id = if owner_id == me.id() { target_id } else { owner_id };
                game::packets::left_mid_trade(state, me, peer_id).await?;
                if let Some(peer_entity) = state.entity(peer_id) {
                    if let Some(peer_owner) = peer_entity.owner() {
                        peer_owner
                            .send(MsgTrade::new(me.id(), game::trade::HIDE_TABLE, 0))
                            .await?;
                    }
                }
            }
            state.player_shop_manager().remove_member(me.id());
            state.guild_manager().remove_member(me.id());
            state.clear_friend_requests_for(me.id());
            state.remove_entity(me.id());
            let mymap = state.try_map(mymap_id)?;
            mymap.remove_entity(&entity)?;
        }
        let _ = actor.shutdown().await;
        Ok(())
    }
}

#[derive(Copy, Clone, PacketHandler)]
#[handle(state = State, actor_state = ActorState)]
pub enum Handler {
    MsgConnect,
    MsgAssignAttributes,
    MsgRegister,
    MsgTalk,
    MsgAction,
    MsgItem,
    MsgWalk,
    MsgTransfer,
    MsgTransferKey,
    MsgNpc,
    MsgTaskDialog,
    MsgWarehouse,
    MsgInteract,
    MsgGroundItem,
    MsgSocketGem,
    MsgRefinery,
    MsgItemRarity,
    MsgItemRarityReroll,
    MsgTeamAction,
    MsgFriend,
    MsgGuild,
    MsgGuildWarehouse,
    MsgTrade,
    MsgBounty,
    MsgStore,
    MsgDailyReward,
    MsgHighscores,
    MsgNobility,
    MsgNobilityRanking,
    MsgBroadcast,
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    dotenvy::dotenv()?;
    let log_verbosity = env::var("LOG_VERBOSITY")
        .map(|s| s.parse::<i32>().unwrap_or(2))
        .unwrap_or(2);
    let started = std::time::Instant::now();
    #[cfg(not(feature = "console"))]
    let _logs = tq_logging::init("game", log_verbosity);
    #[cfg(feature = "console")]
    setup_logger(log_verbosity)?;
    tracing::debug!(target: "server", "Loading the world");

    let static_state = {
        let state = State::init().await?;
        Box::leak(Box::new(state)) as *mut State
    };

    // SAFETY: We are the only owner of this Box, and we are deref
    // it. This happens only once, so no one else can access.
    let state = unsafe { &*static_state };
    let ai_state = state;
    game::guild_war::load(state).await;
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_millis(500));
        loop {
            interval.tick().await;
            game::systems::temporary_effects::tick(ai_state).await;
            game::systems::status_effects::tick(ai_state).await;
            game::vitals::tick(ai_state).await;
            game::broadcast::tick(ai_state).await;
            game::bosses::tick(ai_state).await;
            game::events::tick(ai_state).await;
            game::guild_war::tick(ai_state).await;
            game::mining::tick(ai_state).await;
            game::pets::tick(ai_state).await;
            game::top_classes::tick(ai_state).await;
            game::systems::monster_ai::tick(ai_state).await;
            if let Err(error) = game::packets::expire_trades(ai_state).await {
                tracing::warn!(%error, "trade timeout cleanup failed");
            }
        }
    });
    let autosave_state = state;
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
        interval.tick().await;
        loop {
            interval.tick().await;
            match autosave_state.autosave_characters().await {
                Ok(saved) if saved > 0 => tracing::debug!(saved, "online characters autosaved"),
                Ok(_) => {},
                Err(error) => tracing::warn!(%error, "online character autosave failed"),
            }
        }
    });
    let realm = tq_db::realm::Realm::by_name(state.pool(), "Gravak")
        .await?
        .ok_or(Error::RealmNotFound)?;
    let game_port = realm.game_port;
    tq_logging::banner(
        "GRAVAK  ·  GAME SERVER",
        concat!("v", env!("CARGO_PKG_VERSION")),
        &[
            ("Listening", format!("0.0.0.0:{game_port}")),
            ("Realm", format!("{} (clients are sent to {}:{})", realm.name, realm.game_ip_address, realm.game_port)),
            ("Database", tq_db::database_label()),
            ("Log level", log_level_name(log_verbosity).to_owned()),
            ("Log files", "logs/game.log.<date>".to_owned()),
            (
                "Login tokens",
                if env::var("TRANSFER_SECRET").is_ok_and(|key| !key.trim().is_empty()) {
                    "auth server with TRANSFER_SECRET".to_owned()
                } else {
                    "this machine only (no TRANSFER_SECRET)".to_owned()
                },
            ),
            ("Started in", format!("{:.2} s", started.elapsed().as_secs_f32())),
        ],
    );
    tracing::info!(target: "server", port = game_port, "Ready for players");
    game::status::start(state, started);

    GameServer::run(format!("0.0.0.0:{}", game_port), state).await?;
    unsafe {
        // SAFETY: We are the only owner of this Box, and we are dropping
        // it. This happens at the end of the program, so no one
        // else can access.
        let state = Box::from_raw(static_state);
        state.clean_up().await?;
        // State dropped here.
    };
    tracing::info!(target: "server", "Shut down");
    Ok(())
}

/// `LOG_VERBOSITY` in words, for the startup box.
fn log_level_name(verbosity: i32) -> &'static str {
    match verbosity {
        i32::MIN..=0 => "errors",
        1 => "warnings",
        2 => "information",
        3 => "debug",
        _ => "trace",
    }
}

/// The tokio-console build keeps the plain subscriber with its layer.
#[cfg(feature = "console")]
fn setup_logger(verbosity: i32) -> Result<(), Error> {
    use tracing::Level;
    use tracing_subscriber::prelude::*;

    let log_level = match verbosity {
        0 => Level::ERROR,
        1 => Level::WARN,
        2 => Level::INFO,
        3 => Level::DEBUG,
        _ => Level::TRACE,
    };

    let logger = tracing_subscriber::fmt::layer().pretty().with_target(true);
    let env_filter = tracing_subscriber::EnvFilter::from_default_env()
        .add_directive(format!("tq_db={}", log_level).parse().unwrap())
        .add_directive(format!("tq_serde={}", log_level).parse().unwrap())
        .add_directive(format!("tq_crypto={}", log_level).parse().unwrap())
        .add_directive(format!("tq_codec={}", log_level).parse().unwrap())
        .add_directive(format!("tq_network={}", log_level).parse().unwrap())
        .add_directive(format!("tq_server={}", log_level).parse().unwrap())
        .add_directive(format!("game={}", log_level).parse().unwrap())
        .add_directive(format!("game_server={}", log_level).parse().unwrap());

    #[cfg(feature = "console")]
    let env_filter = env_filter
        .add_directive("tokio=trace".parse().unwrap())
        .add_directive("runtime=trace".parse().unwrap());

    #[cfg(feature = "console")]
    let console_layer = console_subscriber::ConsoleLayer::builder().with_default_env().spawn();

    let registry = tracing_subscriber::registry().with(env_filter).with(logger);

    #[cfg(feature = "console")]
    let registry = registry.with(console_layer);

    registry.init();
    Ok(())
}
