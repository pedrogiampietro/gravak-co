use gravak_protocol::{
    ActionPacket, AuthResponse, GameEvent, GameSession, NpcPacket, RegisterCharacter,
    ServerNpcInfo, ServerPlayer, TaskDialog, authenticate,
};
use std::{
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

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
    WalkConfirmed {
        character_id: u32,
        direction: u8,
    },
    MovementRejected(String),
    Reposition {
        map_id: u32,
        position: [i32; 2],
        direction: u8,
    },
    RemoteWalk {
        id: u32,
        direction: u8,
        movement_type: u8,
    },
    RemoteFacing {
        id: u32,
        direction: u8,
    },
    RemoteJump {
        id: u32,
        position: [i32; 2],
        direction: u8,
    },
    RemoteAction {
        id: u32,
        action: u16,
    },
    NpcDialog(TaskDialog),
    RemoveActor(u32),
    PlayerSpawn(RemoteActor),
    NpcSpawn(RemoteActor),
    Error(String),
}

#[derive(Debug, Clone)]
pub struct RemoteActor {
    pub id: u32,
    pub name: String,
    pub position: [f32; 2],
    pub direction: u8,
    pub mesh: u32,
    pub body: u32,
    pub armor: u32,
    pub hair_style: u16,
    pub right_weapon: u32,
    pub left_weapon: u32,
    pub action: u8,
    pub health: u16,
    pub npc_kind: u16,
    pub animation_until: f32,
    pub step_right: bool,
    pub npc: bool,
}

enum NetworkCommand {
    Walk { character_id: u32, direction: u8 },
    Action { character_id: u32, action: u32 },
    NpcInteract { npc_id: u32, kind: u16 },
}

pub struct NetworkHandle {
    events: Receiver<NetworkEvent>,
    commands: UnboundedSender<NetworkCommand>,
}

impl NetworkHandle {
    pub fn start(config: NetworkConfig) -> Self {
        let (events_tx, events) = mpsc::channel();
        let (commands, commands_rx) = tokio::sync::mpsc::unbounded_channel();
        thread::Builder::new()
            .name("gravak-network".into())
            .spawn(move || {
                let result = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(|error| error.to_string())
                    .and_then(|runtime| {
                        runtime.block_on(run(config, events_tx.clone(), commands_rx))
                    });
                if let Err(error) = result {
                    let _ = events_tx.send(NetworkEvent::Error(error));
                }
            })
            .expect("network thread should start");
        Self { events, commands }
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
                NetworkEvent::WalkConfirmed { .. }
                | NetworkEvent::MovementRejected(_)
                | NetworkEvent::Reposition { .. }
                | NetworkEvent::RemoteWalk { .. }
                | NetworkEvent::RemoteFacing { .. }
                | NetworkEvent::RemoteJump { .. }
                | NetworkEvent::RemoteAction { .. }
                | NetworkEvent::NpcDialog(_)
                | NetworkEvent::RemoveActor(_)
                | NetworkEvent::PlayerSpawn(_)
                | NetworkEvent::NpcSpawn(_) => {}
            }
        }
    }

    pub fn send_walk(&self, character_id: u32, direction: u8) {
        let _ = self.commands.send(NetworkCommand::Walk {
            character_id,
            direction,
        });
    }

    pub fn send_action(&self, character_id: u32, action: u32) {
        let _ = self.commands.send(NetworkCommand::Action {
            character_id,
            action,
        });
    }

    pub fn send_npc_interact(&self, npc_id: u32, kind: u16) {
        let _ = self
            .commands
            .send(NetworkCommand::NpcInteract { npc_id, kind });
    }
}

async fn run(
    config: NetworkConfig,
    events: Sender<NetworkEvent>,
    mut commands: UnboundedReceiver<NetworkCommand>,
) -> Result<(), String> {
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

    let local_character_id = user.character_id;
    loop {
        tokio::select! {
            event = session.next_event() => match event.map_err(|error| error.to_string())? {
                Some(GameEvent::Walk(walk)) => {
                    if walk.character_id == local_character_id {
                        let _ = events.send(NetworkEvent::WalkConfirmed {
                            character_id: walk.character_id,
                            direction: walk.direction,
                        });
                    } else {
                        let _ = events.send(NetworkEvent::RemoteWalk {
                            id: walk.character_id,
                            direction: walk.direction,
                            movement_type: walk.movement_type,
                        });
                    }
                }
                Some(GameEvent::Talk(talk)) if talk.message == "Invalid Location" => {
                    let _ = events.send(NetworkEvent::MovementRejected(talk.message));
                }
                Some(GameEvent::Action(action)) if action.action_type == 86 => {
                    let _ = events.send(NetworkEvent::Reposition {
                        map_id: action.data1,
                        position: unpack_position(action.data2),
                        direction: action.details as u8,
                    });
                }
                Some(GameEvent::Action(action)) if action.action_type == 132 => {
                    let _ = events.send(NetworkEvent::RemoveActor(action.character_id));
                }
                Some(GameEvent::Action(action))
                    if action.action_type == 79 && action.character_id != local_character_id =>
                {
                    let _ = events.send(NetworkEvent::RemoteFacing {
                        id: action.character_id,
                        direction: action.details as u8,
                    });
                }
                Some(GameEvent::Action(action))
                    if action.action_type == 133 && action.character_id != local_character_id =>
                {
                    let _ = events.send(NetworkEvent::RemoteJump {
                        id: action.character_id,
                        position: unpack_position(action.data1),
                        direction: action.details as u8,
                    });
                }
                Some(GameEvent::Action(action))
                    if action.action_type == 81 && action.character_id != local_character_id =>
                {
                    let _ = events.send(NetworkEvent::RemoteAction {
                        id: action.character_id,
                        action: action.data1 as u16,
                    });
                }
                Some(GameEvent::TaskDialog(dialog)) => {
                    let _ = events.send(NetworkEvent::NpcDialog(dialog));
                }
                Some(GameEvent::Player(player)) => {
                    let _ = events.send(NetworkEvent::PlayerSpawn(player_actor(player)));
                }
                Some(GameEvent::NpcInfo(npc)) => {
                    let _ = events.send(NetworkEvent::NpcSpawn(npc_actor(npc)));
                }
                Some(_) => {}
                None => return Ok(()),
            },
            command = commands.recv() => match command {
                Some(NetworkCommand::Walk { character_id, direction }) => {
                    session
                        .send(&gravak_protocol::WalkPacket::new(character_id, direction))
                        .await
                        .map_err(|error| error.to_string())?;
                }
                Some(NetworkCommand::Action { character_id, action }) => {
                    session
                        .send(&ActionPacket::change_action(character_id, action))
                        .await
                        .map_err(|error| error.to_string())?;
                }
                Some(NetworkCommand::NpcInteract { npc_id, kind }) => {
                    session
                        .send(&NpcPacket::activate(npc_id, kind))
                        .await
                        .map_err(|error| error.to_string())?;
                }
                None => return Ok(()),
            },
        }
    }
}

fn player_actor(player: ServerPlayer) -> RemoteActor {
    RemoteActor {
        id: player.character_id.max(0) as u32,
        name: player.character_name,
        position: [player.x as f32, player.y as f32],
        direction: player.direction,
        mesh: player.mesh.max(0) as u32,
        body: body_from_mesh(player.mesh.max(0) as u32),
        armor: player.armor.max(0) as u32,
        hair_style: player.hair_style.max(0) as u16,
        right_weapon: player.right_hand.max(0) as u32,
        left_weapon: player.left_hand.max(0) as u32,
        action: player.action,
        health: player.health_points,
        npc_kind: 0,
        animation_until: 0.0,
        step_right: false,
        npc: false,
    }
}

fn npc_actor(npc: ServerNpcInfo) -> RemoteActor {
    RemoteActor {
        id: npc.id,
        name: npc.name.unwrap_or_default(),
        position: [npc.x as f32, npc.y as f32],
        direction: 0,
        mesh: npc.look as u32,
        body: 0,
        armor: 0,
        hair_style: 0,
        right_weapon: 0,
        left_weapon: 0,
        action: 0,
        health: 0,
        npc_kind: npc.kind,
        animation_until: 0.0,
        step_right: false,
        npc: true,
    }
}

fn body_from_mesh(mesh: u32) -> u32 {
    match mesh % 10_000 {
        1003 => 1,
        1004 => 2,
        2001 => 3,
        2002 => 4,
        _ => 0,
    }
}

fn unpack_position(value: u32) -> [i32; 2] {
    [(value & 0xffff) as i32, (value >> 16) as i32]
}

async fn load_location(
    session: &mut GameSession,
    _character_id: u32,
) -> Result<(u32, [i32; 2]), gravak_protocol::ProtocolError> {
    let mut location = None;
    let map_id = loop {
        match session.next_event().await? {
            Some(GameEvent::Action(action)) => {
                location = Some(unpack_position(action.data2));
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
