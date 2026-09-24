use gravak_protocol::{
    ActionPacket, AuthResponse, GameEvent, GameSession, RegisterCharacter, WalkPacket, authenticate,
};

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let address = args.first().map(String::as_str).unwrap_or("127.0.0.1:9958");
    let username = args.get(1).map(String::as_str).unwrap_or("test1");
    let password = args.get(2).map(String::as_str).unwrap_or("123456");
    let realm = args.get(3).map(String::as_str).unwrap_or("CoEmu");

    println!("Connecting to auth {address} as {username} on realm {realm}...");
    let server = match authenticate(address, username, password, realm).await {
        Ok(AuthResponse::Forwarded(server)) => {
            println!(
                "Auth OK: token={}, game_server={}:{}",
                server.token, server.address, server.port
            );
            server
        }
        Ok(AuthResponse::Rejected(rejection)) => {
            println!(
                "Auth rejected: code={}, {}",
                rejection.code, rejection.message
            );
            return;
        }
        Err(error) => {
            eprintln!("Auth probe failed: {error}");
            std::process::exit(1);
        }
    };

    let game_address = format!("{}:{}", server.address, server.port);
    let mut session = match GameSession::connect(&game_address, server.token, 5065, "English").await
    {
        Ok(session) => session,
        Err(error) => {
            eprintln!("Game connect failed: {error}");
            std::process::exit(1);
        }
    };

    let mut needs_reconnect = false;
    let mut character_id = None;
    loop {
        match session.next_event().await {
            Ok(Some(GameEvent::Talk(talk))) => {
                println!(
                    "Game talk: channel={}, sender={}, message={}",
                    talk.channel, talk.sender_name, talk.message
                );
                if talk.message == "NEW_ROLE" {
                    let character_name = new_character_name();
                    println!("Registering test character {character_name}...");
                    if let Err(error) = session
                        .send(&RegisterCharacter::new(
                            username,
                            &character_name,
                            server.token as u32,
                        ))
                        .await
                    {
                        eprintln!("Character registration failed: {error}");
                        std::process::exit(1);
                    }
                    needs_reconnect = true;
                    break;
                }
            }
            Ok(Some(GameEvent::UserInfo(info))) => {
                println!(
                    "User OK: id={}, name={}, level={}, mesh={}",
                    info.character_id, info.character_name, info.level, info.mesh
                );
                character_id = Some(info.character_id);
                break;
            }
            Ok(Some(_)) => {}
            Ok(None) => break,
            Err(error) => {
                eprintln!("Game protocol failed: {error}");
                std::process::exit(1);
            }
        }
    }

    if needs_reconnect {
        match session.next_event().await {
            Ok(Some(GameEvent::Talk(talk))) => println!(
                "Register OK: channel={}, message={}",
                talk.channel, talk.message
            ),
            Ok(Some(event)) => println!("Register response: {event:?}"),
            Ok(None) => {
                eprintln!("Game server closed after registration");
                std::process::exit(1);
            }
            Err(error) => {
                eprintln!("Registration response failed: {error}");
                std::process::exit(1);
            }
        }

        drop(session);
        let server = match authenticate(address, username, password, realm).await {
            Ok(AuthResponse::Forwarded(server)) => server,
            Ok(AuthResponse::Rejected(rejection)) => {
                eprintln!("Re-auth rejected: {}", rejection.message);
                std::process::exit(1);
            }
            Err(error) => {
                eprintln!("Re-auth failed: {error}");
                std::process::exit(1);
            }
        };
        let game_address = format!("{}:{}", server.address, server.port);
        session = match GameSession::connect(&game_address, server.token, 5065, "English").await {
            Ok(session) => session,
            Err(error) => {
                eprintln!("Reconnect to game failed: {error}");
                std::process::exit(1);
            }
        };

        loop {
            match session.next_event().await {
                Ok(Some(GameEvent::Talk(talk))) => println!(
                    "Login talk: channel={}, sender={}, message={}",
                    talk.channel, talk.sender_name, talk.message
                ),
                Ok(Some(GameEvent::UserInfo(info))) => {
                    println!(
                        "User OK: id={}, name={}, level={}, mesh={}",
                        info.character_id, info.character_name, info.level, info.mesh
                    );
                    character_id = Some(info.character_id);
                    break;
                }
                Ok(Some(_)) => {}
                Ok(None) => {
                    eprintln!("Game server closed during login");
                    std::process::exit(1);
                }
                Err(error) => {
                    eprintln!("Login protocol failed: {error}");
                    std::process::exit(1);
                }
            }
        }
    }

    let character_id = character_id.expect("character id is set by UserInfo");
    if let Err(error) = load_map_and_walk(&mut session, character_id).await {
        eprintln!("World probe failed: {error}");
        std::process::exit(1);
    }
}

async fn load_map_and_walk(
    session: &mut GameSession,
    character_id: u32,
) -> Result<(), gravak_protocol::ProtocolError> {
    session
        .send(&ActionPacket::send_location(character_id))
        .await?;
    let map_id = loop {
        match session.next_event().await? {
            Some(GameEvent::MapInfo(info)) => {
                println!(
                    "Map OK: uid={}, map_id={}, flags={}",
                    info.uid, info.map_id, info.flags
                );
                break info.map_id;
            }
            Some(GameEvent::Action(action)) => println!("Location OK: map_id={}", action.data1),
            Some(GameEvent::Talk(talk)) => println!("World talk: {}", talk.message),
            Some(_) => {}
            None => {
                return Err(gravak_protocol::ProtocolError::MalformedResponse(
                    "game server closed before map info",
                ));
            }
        }
    };

    session.send(&WalkPacket::new(character_id, 0)).await?;
    loop {
        match session.next_event().await? {
            Some(GameEvent::Walk(walk)) if walk.character_id == character_id => {
                println!(
                    "Walk OK: character_id={}, direction={}, movement_type={}, map_id={}",
                    walk.character_id, walk.direction, walk.movement_type, map_id
                );
                return Ok(());
            }
            Some(GameEvent::Talk(talk)) => println!("Movement talk: {}", talk.message),
            Some(_) => {}
            None => {
                return Err(gravak_protocol::ProtocolError::MalformedResponse(
                    "game server closed during walk",
                ));
            }
        }
    }
}

fn new_character_name() -> String {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() % 1_000_000_000)
        .unwrap_or_default();
    format!("Gravak{seconds}")
}
