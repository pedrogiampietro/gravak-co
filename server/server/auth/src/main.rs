//! This program encapsulates the account server.
//! The account server is designed to accept login data from the client and
//! to verify that the username and password combination inputted is
//! correct with the database. If the combination is correct, the client
//! will be transferred to the message server of their choice.

use bytes::Bytes;
use msg_connect::MsgConnect;
use std::env;
use tq_network::{Actor, ActorHandle, PacketDecode, PacketHandler, PacketID, TQCipher};
#[cfg(feature = "server")]
use tq_server::TQServer;
use wasmtime::{Config, Engine, ExternRef, Linker, Module, Store};

use auth::error::Error;
use auth::{Runtime, State};

struct AuthServer;

impl TQServer for AuthServer {
    type ActorState = ();
    type Cipher = TQCipher;
    type PacketHandler = Runtime;
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    dotenvy::dotenv()?;
    let log_verbosity = env::var("LOG_VERBOSITY")
        .map(|s| s.parse::<i32>().unwrap_or(2))
        .unwrap_or(2);
    let started = std::time::Instant::now();
    let _logs = tq_logging::init("auth", log_verbosity);
    let mut config = Config::new();
    config.async_support(true).wasm_reference_types(true);

    let engine = Engine::new(&config)?;
    let mut linker = Linker::new(&engine);
    auth::add_to_linker(&mut linker)?;
    tracing::debug!(target: "auth", "Loading the packet handlers");

    let msg_connect = Module::from_file(
        &engine,
        msg_connect::WASM_BINARY.ok_or(Error::State("msg-connect WASM was not built"))?,
    )?;
    let msg_account = Module::from_file(
        &engine,
        msg_account::WASM_BINARY.ok_or(Error::State("msg-account WASM was not built"))?,
    )?;
    tracing::debug!(target: "auth", "Opening the database");
    let state = State::init().await?;
    let packets = auth::Packets {
        msg_connect,
        msg_account,
    };

    let static_runtime = {
        let runtime = Runtime {
            state,
            linker,
            engine,
            packets,
        };
        Box::leak(Box::new(runtime)) as *mut _
    };
    // SAFETY: We are the only owner of this Box, and we are deref
    // it. This happens only once, so no one else can access.
    let runtime: &'static _ = unsafe { &*static_runtime };

    let auth_port = env::var("AUTH_PORT")?;
    tq_logging::banner(
        "GRAVAK  ·  AUTH SERVER",
        concat!("v", env!("CARGO_PKG_VERSION")),
        &[
            ("Listening", format!("0.0.0.0:{auth_port}")),
            ("Database", tq_db::database_label()),
            ("Log level", log_level_name(log_verbosity).to_owned()),
            ("Log files", "logs/auth.log.<date>".to_owned()),
            ("Started in", format!("{:.2} s", started.elapsed().as_secs_f32())),
        ],
    );
    tracing::info!(target: "auth", port = %auth_port, "Ready for logins");
    AuthServer::run(format!("0.0.0.0:{}", auth_port), runtime).await?;
    unsafe {
        // SAFETY: We are the only owner of this Box, and we are dropping
        // it. This happens at the end of the program, so no one
        // else can access.
        let _ = Box::from_raw(static_runtime);
    };
    tracing::info!(target: "auth", "Shut down");
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
