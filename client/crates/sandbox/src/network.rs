use gravak_protocol::{
    ActionPacket, AuthResponse, GameEvent, GameSession, RegisterCharacter, authenticate,
};
use std::{
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Clone)]
pub struct NetworkConfig {
    pub auth_address: String,
    pub username: String,
    pub password: String,
    pub realm: String,
    pub character_name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ReadyState {
    pub character_id: u32,
    pub character_name: String,
    pub map_id: u32,
    pub position: [i32; 2],
}

#[derive(Debug)]
pub enum NetworkEvent {
    Status(String),
    Ready(ReadyState),
    WalkConfirmed { character_id: u32, direction: u8 },
    Error(String),
}

pub struct NetworkHandle {
    events: Receiver<NetworkEvent>,
}

impl NetworkHandle {
    pub fn start(config: NetworkConfig) -> Self {
        let (events_tx, events) = mpsc::channel();
        thread::Builder::new()
            .name("gravak-network".into())
            .spawn(move || {
                let result = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(|error| error.to_string())
                    .and_then(|runtime| runtime.block_on(run(config, events_tx.clone())));
                if let Err(error) = result {
                    let _ = events_tx.send(NetworkEvent::Error(error));
                }
            })
            .expect("network thread should start");
        Self { events }
    }

    pub fn drain(&self) -> Vec<NetworkEvent> {
        self.events.try_iter().collect()
    }

    pub fn wait_ready(&self) -> Result<ReadyState, String> {
        loop {
            match self
                .events
                .recv_timeout(Duration::from_secs(20))
                .map_err(|_| "timed out waiting for the game server".to_owned())?
            {
                NetworkEvent::Status(message) => println!("Network: {message}"),
                NetworkEvent::Ready(ready) => return Ok(ready),
                NetworkEvent::Error(error) => return Err(error),
                NetworkEvent::WalkConfirmed { .. } => {}
            }
        }
    }
}

async fn run(config: NetworkConfig, events: Sender<NetworkEvent>) -> Result<(), String> {
    send_status(&events, "authenticating");
    let server = match authenticate(
        &config.auth_address,
        &config.username,
        &config.password,
        &config.realm,
    )
    .await
    .map_err(|error| error.to_string())?
    {
        AuthResponse::Forwarded(server) => server,
        AuthResponse::Rejected(rejection) => {
            return Err(format!(
                "auth rejected: code={}, {}",
                rejection.code, rejection.message
            ));
        }
    };
    send_status(&events, "connecting to game server");

    let game_address = format!("{}:{}", server.address, server.port);
    let mut session = GameSession::connect(&game_address, server.token, 5065, "English")
        .await
        .map_err(|error| error.to_string())?;
    let user = loop {
        match session
            .next_event()
            .await
            .map_err(|error| error.to_string())?
        {
            Some(GameEvent::Talk(talk)) if talk.message == "NEW_ROLE" => {
                let name = config
                    .character_name
                    .clone()
                    .unwrap_or_else(generated_character_name);
                send_status(&events, &format!("creating character {name}"));
                session
                    .send(&RegisterCharacter::new(
                        &config.username,
                        &name,
                        server.token as u32,
                    ))
                    .await
                    .map_err(|error| error.to_string())?;
                loop {
                    match session
                        .next_event()
                        .await
                        .map_err(|error| error.to_string())?
                    {
                        Some(GameEvent::Talk(talk)) if talk.message == "ANSWER_OK" => break,
                        Some(GameEvent::Talk(talk)) => {
                            return Err(format!("character creation failed: {}", talk.message));
                        }
                        Some(_) => {}
                        None => return Err("game server closed during character creation".into()),
                    }
                }
                drop(session);
                let server = authenticate(
                    &config.auth_address,
                    &config.username,
                    &config.password,
                    &config.realm,
                )
                .await
                .map_err(|error| error.to_string())?;
                let AuthResponse::Forwarded(server) = server else {
                    return Err("auth rejected during reconnect".into());
                };
                let game_address = format!("{}:{}", server.address, server.port);
                session = GameSession::connect(&game_address, server.token, 5065, "English")
                    .await
                    .map_err(|error| error.to_string())?;
            }
            Some(GameEvent::UserInfo(info)) => {
                break info;
            }
            Some(_) => {}
            None => return Err("game server closed during login".into()),
        }
    };

    send_status(&events, &format!("loading {}", user.character_name));
    session
        .send(&ActionPacket::send_location(user.character_id))
        .await
        .map_err(|error| error.to_string())?;

    let (map_id, position) = load_location(&mut session, user.character_id)
        .await
        .map_err(|error| error.to_string())?;
    events
        .send(NetworkEvent::Ready(ReadyState {
            character_id: user.character_id,
            character_name: user.character_name,
            map_id,
            position,
        }))
        .map_err(|error| error.to_string())?;
    send_status(&events, "connected");

    while let Some(event) = session
        .next_event()
        .await
        .map_err(|error| error.to_string())?
    {
        if let GameEvent::Walk(walk) = event {
            let _ = events.send(NetworkEvent::WalkConfirmed {
                character_id: walk.character_id,
                direction: walk.direction,
            });
        }
    }
    Ok(())
}

async fn load_location(
    session: &mut GameSession,
    _character_id: u32,
) -> Result<(u32, [i32; 2]), gravak_protocol::ProtocolError> {
    let mut location = None;
    let map_id = loop {
        match session.next_event().await? {
            Some(GameEvent::Action(action)) => {
                location = Some([(action.data2 & 0xffff) as i32, (action.data2 >> 16) as i32]);
            }
            Some(GameEvent::MapInfo(info)) => break info.map_id,
            Some(_) => {}
            None => {
                return Err(gravak_protocol::ProtocolError::MalformedResponse(
                    "game server closed before map info",
                ));
            }
        }
    };
    Ok((
        map_id,
        location.ok_or(gravak_protocol::ProtocolError::MalformedResponse(
            "game server did not send location",
        ))?,
    ))
}

fn send_status(events: &Sender<NetworkEvent>, message: &str) {
    let _ = events.send(NetworkEvent::Status(message.to_owned()));
}

fn generated_character_name() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() % 1_000_000_000)
        .unwrap_or_default();
    format!("Gravak{seconds}")
}
