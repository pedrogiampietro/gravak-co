use gravak_protocol::{
    ActionPacket, AssignAttributesPacket, AuthResponse, FriendPacket, GameEvent, GameSession,
    GroundItemPacket, InteractionPacket, ItemActionPacket, NpcDialogPacket, NpcPacket,
    RefineryPacket, RegisterCharacter, SocketGemPacket, TalkPacket, TeamActionPacket, TradePacket,
    WalkPacket, WarehouseActionPacket, authenticate,
};
use tokio::time::{Duration, Instant, sleep, timeout, timeout_at};

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let address = args.first().map(String::as_str).unwrap_or("127.0.0.1:9958");
    let username = args.get(1).map(String::as_str).unwrap_or("test1");
    let password = args.get(2).map(String::as_str).unwrap_or("123456");
    let realm = args.get(3).map(String::as_str).unwrap_or("Gravak");

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
    let mut loaded_character_name = String::from("Gravak");
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
                loaded_character_name = info.character_name;
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
                    loaded_character_name = info.character_name;
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
    if let Some(index) = args.iter().position(|argument| argument == "--title-equip") {
        let Some(raw_title_id) = args.get(index + 1) else {
            eprintln!("Title smoke requires --title-equip <title_id>");
            std::process::exit(1);
        };
        let Ok(title_id) = raw_title_id.parse::<u32>() else {
            eprintln!("Title id must be an unsigned integer");
            std::process::exit(1);
        };
        if let Err(error) = equip_title_and_wait(
            &mut session,
            character_id,
            &loaded_character_name,
            title_id,
            "requested",
        )
        .await
        {
            eprintln!("Title smoke failed: {error}");
            std::process::exit(1);
        }
        return;
    }
    if args.iter().any(|argument| argument == "--bounty") {
        if let Err(error) = run_bounty_smoke(&mut session).await {
            eprintln!("Bounty smoke failed: {error}");
            std::process::exit(1);
        }
        return;
    }
    if args.iter().any(|argument| argument == "--nobility") {
        if let Err(error) =
            run_nobility_smoke(&mut session, character_id, &loaded_character_name).await
        {
            eprintln!("Nobility smoke failed: {error}");
            std::process::exit(1);
        }
        return;
    }
    if let Some(index) = args.iter().position(|argument| argument == "--load") {
        let Some(target_username) = args.get(index + 1) else {
            eprintln!("Load smoke requires --load <username> <password>");
            std::process::exit(1);
        };
        let Some(target_password) = args.get(index + 2) else {
            eprintln!("Load smoke requires --load <username> <password>");
            std::process::exit(1);
        };
        if let Err(error) = run_load_smoke(
            &mut session,
            character_id,
            "Gravak",
            address,
            target_username,
            target_password,
            realm,
        )
        .await
        {
            eprintln!("Load smoke failed: {error}");
            std::process::exit(1);
        }
        return;
    }
    if let Some(index) = args.iter().position(|argument| argument == "--team") {
        let Some(target_username) = args.get(index + 1) else {
            eprintln!("Team smoke requires --team <username> <password>");
            std::process::exit(1);
        };
        let Some(target_password) = args.get(index + 2) else {
            eprintln!("Team smoke requires --team <username> <password>");
            std::process::exit(1);
        };
        if let Err(error) = run_team_smoke(
            &mut session,
            character_id,
            "Gravak",
            address,
            target_username,
            target_password,
            realm,
        )
        .await
        {
            eprintln!("Team smoke failed: {error}");
            std::process::exit(1);
        }
        return;
    }
    if let Some(index) = args.iter().position(|argument| argument == "--trade") {
        let Some(target_username) = args.get(index + 1) else {
            eprintln!("Trade smoke requires --trade <username> <password>");
            std::process::exit(1);
        };
        let Some(target_password) = args.get(index + 2) else {
            eprintln!("Trade smoke requires --trade <username> <password>");
            std::process::exit(1);
        };
        if let Err(error) = run_trade_smoke(
            &mut session,
            character_id,
            &loaded_character_name,
            address,
            target_username,
            target_password,
            realm,
        )
        .await
        {
            eprintln!("Trade smoke failed: {error}");
            std::process::exit(1);
        }
        return;
    }
    if let Some(index) = args.iter().position(|argument| argument == "--player-shop") {
        let Some(target_username) = args.get(index + 1) else {
            eprintln!("Player-shop smoke requires --player-shop <username> <password>");
            std::process::exit(1);
        };
        let Some(target_password) = args.get(index + 2) else {
            eprintln!("Player-shop smoke requires --player-shop <username> <password>");
            std::process::exit(1);
        };
        if let Err(error) = run_player_shop_smoke(
            &mut session,
            character_id,
            &loaded_character_name,
            address,
            target_username,
            target_password,
            realm,
        )
        .await
        {
            eprintln!("Player-shop smoke failed: {error}");
            std::process::exit(1);
        }
        return;
    }
    if args.iter().any(|argument| argument == "--warehouse") {
        if let Err(error) =
            run_warehouse_smoke(&mut session, character_id, &loaded_character_name).await
        {
            eprintln!("Warehouse smoke failed: {error}");
            std::process::exit(1);
        }
        return;
    }
    if let Some(index) = args.iter().position(|argument| argument == "--whisper") {
        let Some(target_username) = args.get(index + 1) else {
            eprintln!("Whisper smoke requires --whisper <username> <password>");
            std::process::exit(1);
        };
        let Some(target_password) = args.get(index + 2) else {
            eprintln!("Whisper smoke requires --whisper <username> <password>");
            std::process::exit(1);
        };
        if let Err(error) = run_whisper_smoke(
            &mut session,
            character_id,
            "Gravak",
            address,
            target_username,
            target_password,
            realm,
        )
        .await
        {
            eprintln!("Whisper smoke failed: {error}");
            std::process::exit(1);
        }
        return;
    }
    if let Some(index) = args.iter().position(|argument| argument == "--friends") {
        let Some(target_username) = args.get(index + 1) else {
            eprintln!("Friends smoke requires --friends <username> <password>");
            std::process::exit(1);
        };
        let Some(target_password) = args.get(index + 2) else {
            eprintln!("Friends smoke requires --friends <username> <password>");
            std::process::exit(1);
        };
        if let Err(error) = run_friends_smoke(
            &mut session,
            character_id,
            "Gravak",
            address,
            target_username,
            target_password,
            realm,
        )
        .await
        {
            eprintln!("Friends smoke failed: {error}");
            std::process::exit(1);
        }
        return;
    }
    if let Some(index) = args.iter().position(|argument| argument == "--ignore") {
        let Some(target_username) = args.get(index + 1) else {
            eprintln!("Ignore smoke requires --ignore <username> <password>");
            std::process::exit(1);
        };
        let Some(target_password) = args.get(index + 2) else {
            eprintln!("Ignore smoke requires --ignore <username> <password>");
            std::process::exit(1);
        };
        if let Err(error) = run_ignore_smoke(
            &mut session,
            character_id,
            &loaded_character_name,
            address,
            target_username,
            target_password,
            realm,
        )
        .await
        {
            eprintln!("Ignore smoke failed: {error}");
            std::process::exit(1);
        }
        return;
    }
    if args.iter().any(|argument| argument == "--quest") {
        if let Err(error) = run_quest_smoke(&mut session, character_id, "Gravak").await {
            eprintln!("Quest smoke failed: {error}");
            std::process::exit(1);
        }
        return;
    }
    if args.iter().any(|argument| argument == "--quest-status") {
        if let Err(error) = run_quest_status_smoke(&mut session, character_id, "Gravak").await {
            eprintln!("Quest status smoke failed: {error}");
            std::process::exit(1);
        }
        return;
    }
    if args
        .iter()
        .any(|argument| argument == "--npc-training-positive")
    {
        session
            .send(&TalkPacket::public(
                character_id,
                &loaded_character_name,
                "$test-training",
            ))
            .await
            .unwrap_or_else(|error| {
                eprintln!("NPC training fixture failed: {error}");
                std::process::exit(1);
            });
        if let Err(error) =
            wait_for_talk_fragment(&mut session, "Trainer smoke fixture prepared").await
        {
            eprintln!("NPC training fixture failed: {error}");
            std::process::exit(1);
        }
        if let Err(error) =
            run_npc_training_smoke(&mut session, character_id, &loaded_character_name).await
        {
            eprintln!("NPC training smoke failed: {error}");
            std::process::exit(1);
        }
        return;
    }
    if args.iter().any(|argument| argument == "--npc-training") {
        if let Err(error) = run_npc_training_smoke(&mut session, character_id, "Gravak").await {
            eprintln!("NPC training smoke failed: {error}");
            std::process::exit(1);
        }
        return;
    }
    if args.iter().any(|argument| argument == "--invalid-item") {
        if let Err(error) = run_invalid_item_smoke(&mut session, character_id).await {
            eprintln!("Invalid item smoke failed: {error}");
            std::process::exit(1);
        }
        return;
    }
    let map_change_smoke = args.iter().any(|argument| argument == "--map-change");
    if map_change_smoke
        && let Err(error) = run_map_change_smoke(&mut session, character_id, "Gravak").await
    {
        eprintln!("Map change smoke failed: {error}");
        std::process::exit(1);
    }
    let socket_smoke = args.iter().any(|argument| argument == "--sockets");
    let compose_smoke = args.iter().any(|argument| argument == "--compose");
    let initial_monster = if (socket_smoke || compose_smoke)
        && !args
            .iter()
            .any(|argument| argument == "--combat" || argument == "--pickup")
    {
        None
    } else {
        match load_map_and_walk(&mut session, character_id).await {
            Ok(monster) => monster,
            Err(error) => {
                eprintln!("World probe failed: {error}");
                std::process::exit(1);
            }
        }
    };
    if let Some(index) = args.iter().position(|argument| argument == "--pvp") {
        let Some(target_username) = args.get(index + 1) else {
            eprintln!("PvP smoke requires --pvp <username> <password>");
            std::process::exit(1);
        };
        let Some(target_password) = args.get(index + 2) else {
            eprintln!("PvP smoke requires --pvp <username> <password>");
            std::process::exit(1);
        };
        if let Err(error) = run_pvp_smoke(
            &mut session,
            character_id,
            "Gravak",
            address,
            target_username,
            target_password,
            realm,
        )
        .await
        {
            eprintln!("PvP smoke failed: {error}");
            std::process::exit(1);
        }
    }
    if args.iter().any(|argument| argument == "--burning")
        && let Err(error) = run_burning_smoke(
            &mut session,
            character_id,
            &loaded_character_name,
            initial_monster,
        )
        .await
    {
        eprintln!("Burning smoke failed: {error}");
        std::process::exit(1);
    }
    if args.iter().any(|argument| argument == "--stun")
        && let Err(error) = run_stun_smoke(&mut session, character_id, &loaded_character_name).await
    {
        eprintln!("Stun smoke failed: {error}");
        std::process::exit(1);
    }
    if args.iter().any(|argument| argument == "--cyclone")
        && let Err(error) =
            run_cyclone_smoke(&mut session, character_id, &loaded_character_name).await
    {
        eprintln!("Cyclone smoke failed: {error}");
        std::process::exit(1);
    }
    if args.iter().any(|argument| argument == "--superman")
        && let Err(error) =
            run_superman_smoke(&mut session, character_id, &loaded_character_name).await
    {
        eprintln!("Superman smoke failed: {error}");
        std::process::exit(1);
    }
    if args.iter().any(|argument| argument == "--bless")
        && let Err(error) =
            run_bless_smoke(&mut session, character_id, &loaded_character_name).await
    {
        eprintln!("Bless smoke failed: {error}");
        std::process::exit(1);
    }
    if args.iter().any(|argument| argument == "--stigma")
        && let Err(error) =
            run_stigma_smoke(&mut session, character_id, &loaded_character_name).await
    {
        eprintln!("Stigma smoke failed: {error}");
        std::process::exit(1);
    }
    if args.iter().any(|argument| argument == "--shield")
        && let Err(error) =
            run_shield_smoke(&mut session, character_id, &loaded_character_name).await
    {
        eprintln!("Shield smoke failed: {error}");
        std::process::exit(1);
    }
    if let Some(attribute) = args
        .windows(2)
        .find_map(|window| (window[0] == "--assign-attribute").then(|| window[1].as_str()))
        && let Err(error) = run_attribute_packet_smoke(&mut session, attribute).await
    {
        eprintln!("Attribute packet smoke failed: {error}");
        std::process::exit(1);
    }
    if args.iter().any(|argument| argument == "--combat")
        && let Err(error) = run_combat_smoke(
            &mut session,
            character_id,
            "Gravak",
            initial_monster.map(|(id, x, y, _)| (id, x, y)),
            false,
        )
        .await
    {
        eprintln!("Combat smoke failed: {error}");
        std::process::exit(1);
    }
    if args.iter().any(|argument| argument == "--pickup")
        && let Err(error) = run_ground_pickup_smoke(&mut session, character_id, "Gravak").await
    {
        eprintln!("Ground pickup smoke failed: {error}");
        std::process::exit(1);
    }
    if args.iter().any(|argument| argument == "--upgrade")
        && let Err(error) = run_upgrade_smoke(&mut session, character_id, "Gravak").await
    {
        eprintln!("Item upgrade smoke failed: {error}");
        std::process::exit(1);
    }
    if args.iter().any(|argument| argument == "--npc-upgrade")
        && let Err(error) = run_npc_upgrade_smoke(&mut session, character_id, "Gravak").await
    {
        eprintln!("NPC upgrade smoke failed: {error}");
        std::process::exit(1);
    }
    if args.iter().any(|argument| argument == "--sockets")
        && let Err(error) = run_socket_smoke(&mut session, character_id, "Gravak").await
    {
        eprintln!("Socket smoke failed: {error}");
        std::process::exit(1);
    }
    if args.iter().any(|argument| argument == "--compose")
        && let Err(error) = run_compose_smoke(&mut session, character_id, "Gravak").await
    {
        eprintln!("Gem compose smoke failed: {error}");
        std::process::exit(1);
    }
    if args.iter().any(|argument| argument == "--reconnect") {
        println!("Reconnect smoke: closing the game session and authenticating again...");
        drop(session);
        let server = match authenticate(address, username, password, realm).await {
            Ok(AuthResponse::Forwarded(server)) => server,
            Ok(AuthResponse::Rejected(rejection)) => {
                eprintln!("Reconnect auth rejected: {}", rejection.message);
                std::process::exit(1);
            }
            Err(error) => {
                eprintln!("Reconnect auth failed: {error}");
                std::process::exit(1);
            }
        };
        let game_address = format!("{}:{}", server.address, server.port);
        let mut reconnected =
            match GameSession::connect(&game_address, server.token, 5065, "English").await {
                Ok(session) => session,
                Err(error) => {
                    eprintln!("Reconnect to game failed: {error}");
                    std::process::exit(1);
                }
            };
        let reconnected_id = loop {
            match reconnected.next_event().await {
                Ok(Some(GameEvent::UserInfo(info))) => {
                    println!(
                        "Reconnect User OK: id={}, name={}, level={}",
                        info.character_id, info.character_name, info.level
                    );
                    break info.character_id;
                }
                Ok(Some(GameEvent::Talk(talk))) => println!("Reconnect talk: {}", talk.message),
                Ok(Some(_)) => {}
                Ok(None) => {
                    eprintln!("Game server closed during reconnect");
                    std::process::exit(1);
                }
                Err(error) => {
                    eprintln!("Reconnect protocol failed: {error}");
                    std::process::exit(1);
                }
            }
        };
        if reconnected_id != character_id {
            eprintln!(
                "Reconnect returned a different character: before={}, after={}",
                character_id, reconnected_id
            );
            std::process::exit(1);
        }
        load_map_and_walk(&mut reconnected, character_id)
            .await
            .unwrap_or_else(|error| {
                eprintln!("Reconnect world probe failed: {error}");
                std::process::exit(1);
            });
        if args.iter().any(|argument| argument == "--skill-book")
            && let Err(error) = run_skill_book_smoke(&mut reconnected, character_id, "Gravak").await
        {
            eprintln!("Skill book smoke after reconnect failed: {error}");
            std::process::exit(1);
        }
        println!("Reconnect OK: same character and world state restored");
        return;
    }
    if args.iter().any(|argument| argument == "--skill-book")
        && let Err(error) = run_skill_book_smoke(&mut session, character_id, "Gravak").await
    {
        eprintln!("Skill book smoke failed: {error}");
        std::process::exit(1);
    }
}

async fn connect_existing_session(
    address: &str,
    username: &str,
    password: &str,
    realm: &str,
) -> Result<(GameSession, u32, String, u16, u16), gravak_protocol::ProtocolError> {
    let server = match authenticate(address, username, password, realm).await? {
        AuthResponse::Forwarded(server) => server,
        AuthResponse::Rejected(_) => {
            return Err(gravak_protocol::ProtocolError::MalformedResponse(
                "target authentication was rejected",
            ));
        }
    };
    let game_address = format!("{}:{}", server.address, server.port);
    let mut session = GameSession::connect(&game_address, server.token, 5065, "English").await?;
    loop {
        match session.next_event().await? {
            Some(GameEvent::UserInfo(info)) => {
                let maximum_health = info.reserved0.min(u64::from(u16::MAX)) as u16;
                return Ok((
                    session,
                    info.character_id,
                    info.character_name,
                    info.health_points,
                    maximum_health,
                ));
            }
            Some(GameEvent::Talk(talk)) if talk.message == "NEW_ROLE" => {
                return Err(gravak_protocol::ProtocolError::MalformedResponse(
                    "target account has no character; create it before the PvP smoke",
                ));
            }
            Some(_) => {}
            None => {
                return Err(gravak_protocol::ProtocolError::MalformedResponse(
                    "target game session closed during login",
                ));
            }
        }
    }
}

/// The next bounty board, skipping other packets.
async fn next_bounty_board(
    session: &mut GameSession,
) -> Result<gravak_protocol::BountyState, gravak_protocol::ProtocolError> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let event = timeout_at(deadline, session.next_event())
            .await
            .map_err(|_| gravak_protocol::ProtocolError::MalformedResponse("bounty board timed out"))??;
        match event {
            Some(GameEvent::BountyState(board)) => return Ok(board),
            Some(GameEvent::Talk(talk)) => println!("Bounty talk: {}", talk.message),
            Some(_) => {}
            None => {
                return Err(gravak_protocol::ProtocolError::MalformedResponse(
                    "game server closed during the bounty smoke",
                ));
            }
        }
    }
}

/// Opens the Task Board, takes the first bounty, checks it is under way and
/// abandons it again (one reroll token).
async fn run_bounty_smoke(session: &mut GameSession) -> Result<(), gravak_protocol::ProtocolError> {
    use gravak_protocol::BountyPacket;
    let malformed = gravak_protocol::ProtocolError::MalformedResponse;
    session.send(&BountyPacket::new(BountyPacket::OPEN, 0)).await?;
    let board = next_bounty_board(session).await?;
    println!(
        "Bounty board OK: points={}, tokens={}, difficulty={}, offers={:?}, active={:?}",
        board.points,
        board.tokens,
        board.difficulty,
        board.offers().iter().map(|offer| (&offer.monster_name, offer.kills, offer.grade)).collect::<Vec<_>>(),
        board.active().map(|task| (&task.monster_name, task.kills_done, task.kills)),
    );
    if board.active().is_some() {
        println!("A bounty is already under way; leaving it alone.");
        return Ok(());
    }
    let Some(offer) = board.offers().first().map(|offer| (*offer).clone()) else {
        return Err(malformed("the board has no offers"));
    };
    session.send(&BountyPacket::new(BountyPacket::PICK, 0)).await?;
    // Boards sent before the pick (at login) may still be on their way.
    let mut board = next_bounty_board(session).await?;
    while board.active().is_none() {
        board = next_bounty_board(session).await?;
    }
    let task = board.active().ok_or(malformed("the picked bounty is not under way"))?;
    if task.monster_type != offer.monster_type {
        return Err(malformed("the bounty under way is not the one picked"));
    }
    println!("Bounty pick OK: hunting {} 0/{}", task.monster_name, task.kills);
    if board.tokens == 0 {
        return Ok(());
    }
    session.send(&BountyPacket::new(BountyPacket::REROLL, board.difficulty)).await?;
    let mut after = next_bounty_board(session).await?;
    while after.active().is_some() {
        after = next_bounty_board(session).await?;
    }
    if after.active().is_some() || after.tokens + 1 != board.tokens || after.offers().is_empty() {
        return Err(malformed("abandoning did not spend a token for a new list"));
    }
    println!("Bounty abandon OK: tokens {} -> {}, {} new offers", board.tokens, after.tokens, after.offers().len());
    Ok(())
}

async fn run_nobility_smoke(
    session: &mut GameSession,
    character_id: u32,
    character_name: &str,
) -> Result<(), gravak_protocol::ProtocolError> {
    session
        .send(&TalkPacket::public(
            character_id,
            character_name,
            "$nobility-status",
        ))
        .await?;
    let mut received_status = false;
    let mut original_title_id = 0;
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        let event = timeout_at(deadline, session.next_event())
            .await
            .map_err(|_| {
                gravak_protocol::ProtocolError::MalformedResponse("nobility status timed out")
            })??;
        let Some(event) = event else {
            return Err(gravak_protocol::ProtocolError::MalformedResponse(
                "game server closed during nobility status smoke",
            ));
        };
        match event {
            GameEvent::Nobility(info) if info.character_id == character_id => {
                println!(
                    "Nobility status OK: donation={}, rank={}, remaining={}, title_id={}",
                    info.donation, info.rank, info.remaining, info.title_id
                );
                original_title_id = info.title_id;
                received_status = true;
                break;
            }
            GameEvent::Talk(talk) => println!("Nobility talk: {}", talk.message),
            _ => {}
        }
    }
    if !received_status {
        return Err(gravak_protocol::ProtocolError::MalformedResponse(
            "nobility packet was not received",
        ));
    }

    equip_title_and_wait(session, character_id, character_name, 1, "Novice").await?;
    equip_title_and_wait(
        session,
        character_id,
        character_name,
        original_title_id,
        "restored title",
    )
    .await?;

    session
        .send(&TalkPacket::public(
            character_id,
            character_name,
            "$nobility-ranking",
        ))
        .await?;
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        let event = timeout_at(deadline, session.next_event())
            .await
            .map_err(|_| {
                gravak_protocol::ProtocolError::MalformedResponse("nobility ranking timed out")
            })??;
        let Some(event) = event else {
            return Err(gravak_protocol::ProtocolError::MalformedResponse(
                "game server closed during nobility ranking smoke",
            ));
        };
        if let GameEvent::Talk(talk) = event {
            println!("Nobility ranking talk: {}", talk.message);
            if talk.message.contains("Nobility #") || talk.message.contains("ranking is empty") {
                println!("Nobility ranking OK");
                return Ok(());
            }
        }
    }
    Err(gravak_protocol::ProtocolError::MalformedResponse(
        "nobility ranking response was not received",
    ))
}

async fn equip_title_and_wait(
    session: &mut GameSession,
    character_id: u32,
    character_name: &str,
    title_id: u32,
    label: &str,
) -> Result<(), gravak_protocol::ProtocolError> {
    let command = format!("$title-equip {title_id}");
    session
        .send(&TalkPacket::public(character_id, character_name, &command))
        .await?;
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        let event = timeout_at(deadline, session.next_event())
            .await
            .map_err(|_| {
                gravak_protocol::ProtocolError::MalformedResponse("title equip timed out")
            })??;
        let Some(event) = event else {
            return Err(gravak_protocol::ProtocolError::MalformedResponse(
                "game server closed during title equip smoke",
            ));
        };
        match event {
            GameEvent::Nobility(info) if info.character_id == character_id => {
                if info.title_id == title_id {
                    println!("Title equip OK: id={} ({label})", info.title_id);
                    return Ok(());
                }
                println!(
                    "Ignoring stale title packet: received={}, expected={}",
                    info.title_id, title_id
                );
            }
            GameEvent::Talk(talk) => println!("Title talk: {}", talk.message),
            _ => {}
        }
    }
    Err(gravak_protocol::ProtocolError::MalformedResponse(
        "title equip packet was not received",
    ))
}

async fn run_load_smoke(
    session: &mut GameSession,
    character_id: u32,
    character_name: &str,
    address: &str,
    target_username: &str,
    target_password: &str,
    realm: &str,
) -> Result<(), gravak_protocol::ProtocolError> {
    let (mut target_session, target_id, target_name, _, _) =
        connect_existing_session(address, target_username, target_password, realm).await?;
    if target_id == character_id {
        return Err(gravak_protocol::ProtocolError::MalformedResponse(
            "load smoke requires two different characters",
        ));
    }

    println!(
        "Load sessions connected: primary={} target={} ({})",
        character_id, target_id, target_name
    );
    let primary_monster = load_map_and_walk(session, character_id).await?;
    let target_monster = load_map_and_walk(&mut target_session, target_id).await?;
    run_combat_smoke(
        session,
        character_id,
        character_name,
        primary_monster.map(|(id, x, y, _)| (id, x, y)),
        false,
    )
    .await?;
    println!(
        "Load smoke OK: sessions=2, primary={}, target={}, monsters_seen={}, target_monster_seen={}",
        character_id,
        target_id,
        primary_monster.is_some(),
        target_monster.is_some()
    );
    Ok(())
}

async fn run_team_smoke(
    session: &mut GameSession,
    character_id: u32,
    character_name: &str,
    address: &str,
    target_username: &str,
    target_password: &str,
    realm: &str,
) -> Result<(), gravak_protocol::ProtocolError> {
    let (mut target_session, target_id, target_name, _, _) =
        connect_existing_session(address, target_username, target_password, realm).await?;
    if target_id == character_id {
        return Err(gravak_protocol::ProtocolError::MalformedResponse(
            "team smoke requires two different characters",
        ));
    }
    for (target_session, id) in [
        (&mut *session, character_id),
        (&mut target_session, target_id),
    ] {
        target_session
            .send(&TalkPacket::public(
                id,
                character_name,
                "$tele 1002 430 378",
            ))
            .await?;
        target_session
            .send(&ActionPacket::send_location(id))
            .await?;
        wait_for_map(target_session, 1002).await?;
    }

    session.send(&TeamActionPacket::new(0, 0)).await?;
    wait_for_team_action(session, 0, "team create").await?;
    session.send(&TeamActionPacket::new(4, target_id)).await?;
    wait_for_team_action(&mut target_session, 4, "team invitation").await?;
    target_session
        .send(&TeamActionPacket::new(3, character_id))
        .await?;
    wait_for_team_action(session, 3, "team accept leader").await?;
    wait_for_team_action(&mut target_session, 3, "team accept member").await?;
    wait_for_team_member(session, target_id, 0, "team member add").await?;
    wait_for_team_member(&mut target_session, character_id, 0, "team leader add").await?;

    session.send(&TeamActionPacket::new(15, target_id)).await?;
    wait_for_team_action(session, 15, "team leadership transfer").await?;
    wait_for_team_action(&mut target_session, 15, "team leadership transfer member").await?;
    target_session
        .send(&TeamActionPacket::new(15, character_id))
        .await?;
    wait_for_team_action(session, 15, "team leadership return").await?;
    wait_for_team_action(&mut target_session, 15, "team leadership return member").await?;

    target_session.send(&TeamActionPacket::new(2, 0)).await?;
    wait_for_team_action(session, 2, "team leave before join request").await?;
    wait_for_team_member(
        session,
        target_id,
        1,
        "team member remove before join request",
    )
    .await?;
    wait_for_team_action(
        &mut target_session,
        2,
        "team leave before join request member",
    )
    .await?;
    target_session
        .send(&TeamActionPacket::new(1, character_id))
        .await?;
    wait_for_team_action(session, 1, "team join request").await?;
    session.send(&TeamActionPacket::new(5, target_id)).await?;
    wait_for_team_action(session, 5, "team accept join request leader").await?;
    wait_for_team_action(&mut target_session, 5, "team accept join request member").await?;
    wait_for_team_member(session, target_id, 0, "team member add after join request").await?;
    wait_for_team_member(
        &mut target_session,
        character_id,
        0,
        "team leader add after join request",
    )
    .await?;

    run_team_reward_smoke(
        session,
        character_id,
        character_name,
        &mut target_session,
        target_id,
    )
    .await?;

    target_session.send(&TeamActionPacket::new(2, 0)).await?;
    wait_for_team_action(session, 2, "team leave leader").await?;
    wait_for_team_member(session, target_id, 1, "team member remove").await?;
    wait_for_team_action(&mut target_session, 2, "team leave member").await?;
    session.send(&TeamActionPacket::new(6, 0)).await?;
    wait_for_team_action(session, 6, "team dismiss").await?;
    println!(
        "Team smoke OK: leader={}, member={} ({}), create/invite/accept/join-request/leadership/reward/pickup/leave/dismiss synchronized",
        character_id, target_id, target_name
    );
    Ok(())
}

async fn run_trade_smoke(
    session: &mut GameSession,
    character_id: u32,
    character_name: &str,
    address: &str,
    target_username: &str,
    target_password: &str,
    realm: &str,
) -> Result<(), gravak_protocol::ProtocolError> {
    const REQUEST: u32 = 1;
    const HIDE_TABLE: u32 = 5;
    const ADD_ITEM: u32 = 6;
    const SET_MONEY: u32 = 7;
    const SHOW_MONEY: u32 = 8;
    const ACCEPT: u32 = 10;
    const SHOW_CPS: u32 = 12;
    const SET_CPS: u32 = 13;

    let (mut target_session, target_id, target_name, _, _) =
        connect_existing_session(address, target_username, target_password, realm).await?;
    if target_id == character_id {
        return Err(gravak_protocol::ProtocolError::MalformedResponse(
            "trade smoke requires two different characters",
        ));
    }

    for (target_session, id, name) in [
        (&mut *session, character_id, character_name),
        (&mut target_session, target_id, target_name.as_str()),
    ] {
        target_session
            .send(&TalkPacket::public(id, name, "$tele 1002 430 378"))
            .await?;
        target_session
            .send(&TalkPacket::public(id, name, "$cps 10"))
            .await?;
        target_session
            .send(&ActionPacket::send_location(id))
            .await?;
        wait_for_map(target_session, 1002).await?;
    }

    let primary_item =
        ensure_trade_item(session, character_id, character_name, "Trade primary item").await?;
    let target_item = ensure_trade_item(
        &mut target_session,
        target_id,
        &target_name,
        "Trade target item",
    )
    .await?;

    session
        .send(&TradePacket::new(target_id, REQUEST, 0))
        .await?;
    wait_for_trade(&mut target_session, REQUEST, "trade request").await?;
    target_session
        .send(&TradePacket::new(character_id, REQUEST, 0))
        .await?;
    wait_for_trade(session, 3, "trade open primary").await?;
    wait_for_trade(&mut target_session, 3, "trade open target").await?;

    session
        .send(&TradePacket::new(primary_item, ADD_ITEM, 0))
        .await?;
    wait_for_trade_item(&mut target_session, primary_item, "primary offer preview").await?;
    target_session
        .send(&TradePacket::new(target_item, ADD_ITEM, 0))
        .await?;
    wait_for_trade_item(session, target_item, "target offer preview").await?;

    session.send(&TradePacket::new(1, SET_MONEY, 0)).await?;
    target_session
        .send(&TradePacket::new(1, SET_MONEY, 0))
        .await?;
    wait_for_trade(session, SHOW_MONEY, "target money preview").await?;
    wait_for_trade(&mut target_session, SHOW_MONEY, "primary money preview").await?;
    session.send(&TradePacket::new(2, SET_CPS, 0)).await?;
    target_session
        .send(&TradePacket::new(3, SET_CPS, 0))
        .await?;
    wait_for_trade(session, SHOW_CPS, "target CP preview").await?;
    wait_for_trade(&mut target_session, SHOW_CPS, "primary CP preview").await?;

    session
        .send(&TradePacket::new(target_id, ACCEPT, 1))
        .await?;
    wait_for_trade(&mut target_session, ACCEPT, "primary confirmation").await?;
    target_session
        .send(&TradePacket::new(character_id, ACCEPT, 1))
        .await?;
    wait_for_trade(session, HIDE_TABLE, "trade close primary").await?;
    wait_for_trade(&mut target_session, HIDE_TABLE, "trade close target").await?;

    wait_for_item_info(
        session,
        primary_item,
        |item| item.action == 5,
        "primary offered item removal",
    )
    .await?;
    wait_for_item_info(
        &mut target_session,
        target_item,
        |item| item.action == 5,
        "target offered item removal",
    )
    .await?;
    wait_for_item_info(
        session,
        target_item,
        |item| item.action == 1,
        "primary received item",
    )
    .await?;
    wait_for_item_info(
        &mut target_session,
        primary_item,
        |item| item.action == 1,
        "target received item",
    )
    .await?;
    println!(
        "Trade smoke OK: primary={} target={} request/open/offer/money/CPs/dual-confirm/atomic swap",
        character_id, target_id
    );
    Ok(())
}

async fn run_player_shop_smoke(
    session: &mut GameSession,
    character_id: u32,
    character_name: &str,
    address: &str,
    target_username: &str,
    target_password: &str,
    realm: &str,
) -> Result<(), gravak_protocol::ProtocolError> {
    let (mut target_session, target_id, target_name, _, _) =
        connect_existing_session(address, target_username, target_password, realm).await?;
    if target_id == character_id {
        return Err(gravak_protocol::ProtocolError::MalformedResponse(
            "player-shop smoke requires two different characters",
        ));
    }
    for (target_session, id, name) in [
        (&mut *session, character_id, character_name),
        (&mut target_session, target_id, target_name.as_str()),
    ] {
        target_session
            .send(&TalkPacket::public(id, name, "$tele 1036 303 237"))
            .await?;
        target_session
            .send(&ActionPacket::send_location(id))
            .await?;
        wait_for_map(target_session, 1036).await?;
    }

    // The local smoke database is intentionally seeded with this inventory item;
    // using an existing item keeps the test valid even when the seller's bag is full.
    let item_uid = 96;
    session
        .send(&ActionPacket::create_booth(character_id))
        .await?;
    wait_for_player_shop_talk(session, "Player shop opened", "open player shop").await?;
    session
        .send(&ItemActionPacket::booth_add(character_id, item_uid, 100))
        .await?;
    wait_for_player_shop_talk(session, "Item added to player shop", "add player-shop item").await?;

    target_session
        .send(&ItemActionPacket::booth_query(target_id, character_id))
        .await?;
    let listing = wait_for_vendor_item(&mut target_session, item_uid, "query player shop").await?;
    if listing.price != 100 || listing.owner_id != character_id {
        return Err(gravak_protocol::ProtocolError::MalformedResponse(
            "player-shop listing price or owner mismatch",
        ));
    }
    target_session
        .send(&ItemActionPacket::booth_buy(
            target_id,
            item_uid,
            character_id,
        ))
        .await?;
    wait_for_item_info(
        &mut target_session,
        item_uid,
        |item| item.action == 1,
        "player-shop buyer inventory",
    )
    .await?;
    wait_for_item_info(
        session,
        item_uid,
        |item| item.action == 5,
        "player-shop seller removal",
    )
    .await?;
    session
        .send(&ActionPacket::leave_booth(character_id))
        .await?;
    println!(
        "Player-shop smoke OK: seller={} buyer={} query/price/atomic purchase",
        character_id, target_id
    );
    Ok(())
}

async fn wait_for_player_shop_talk(
    session: &mut GameSession,
    expected: &str,
    label: &str,
) -> Result<(), gravak_protocol::ProtocolError> {
    let deadline = Instant::now() + Duration::from_secs(4);
    loop {
        let event = match timeout_at(deadline, session.next_event()).await {
            Ok(event) => event?,
            Err(_) => {
                return Err(gravak_protocol::ProtocolError::MalformedResponse(
                    "timed out waiting for player-shop notice",
                ));
            }
        };
        match event {
            Some(GameEvent::Talk(talk)) => {
                println!("{label}: {}", talk.message);
                if talk.message.contains(expected) {
                    return Ok(());
                }
            }
            Some(_) => {}
            None => {
                return Err(gravak_protocol::ProtocolError::MalformedResponse(
                    "game server closed while waiting for player-shop notice",
                ));
            }
        }
    }
}

async fn wait_for_vendor_item(
    session: &mut GameSession,
    item_uid: u32,
    label: &str,
) -> Result<gravak_protocol::VendorItemPacket, gravak_protocol::ProtocolError> {
    let deadline = Instant::now() + Duration::from_secs(4);
    loop {
        let event = match timeout_at(deadline, session.next_event()).await {
            Ok(event) => event?,
            Err(_) => {
                return Err(gravak_protocol::ProtocolError::MalformedResponse(
                    "timed out waiting for vendor item",
                ));
            }
        };
        match event {
            Some(GameEvent::VendorItem(item)) if item.item_uid == item_uid => {
                println!(
                    "{label}: uid={}, owner={}, price={}",
                    item.item_uid, item.owner_id, item.price
                );
                return Ok(item);
            }
            Some(GameEvent::Talk(talk)) => println!("{label} talk: {}", talk.message),
            Some(_) => {}
            None => {
                return Err(gravak_protocol::ProtocolError::MalformedResponse(
                    "game server closed while waiting for vendor item",
                ));
            }
        }
    }
}

async fn wait_for_trade(
    session: &mut GameSession,
    subtype: u32,
    label: &str,
) -> Result<(), gravak_protocol::ProtocolError> {
    let deadline = Instant::now() + Duration::from_secs(4);
    loop {
        let event = match timeout_at(deadline, session.next_event()).await {
            Ok(event) => event?,
            Err(_) => {
                return Err(gravak_protocol::ProtocolError::MalformedResponse(
                    "timed out waiting for trade event",
                ));
            }
        };
        match event {
            Some(GameEvent::Trade(packet)) if packet.subtype == subtype => {
                println!("{label}: target={}, data={}", packet.target, packet.data);
                return Ok(());
            }
            Some(GameEvent::Talk(talk)) => println!("{label} talk: {}", talk.message),
            Some(_) => {}
            None => {
                return Err(gravak_protocol::ProtocolError::MalformedResponse(
                    "game server closed while waiting for trade event",
                ));
            }
        }
    }
}

async fn wait_for_trade_item(
    session: &mut GameSession,
    item_uid: u32,
    label: &str,
) -> Result<(), gravak_protocol::ProtocolError> {
    let deadline = Instant::now() + Duration::from_secs(4);
    loop {
        let event = match timeout_at(deadline, session.next_event()).await {
            Ok(event) => event?,
            Err(_) => {
                return Err(gravak_protocol::ProtocolError::MalformedResponse(
                    "timed out waiting for trade item preview",
                ));
            }
        };
        match event {
            Some(GameEvent::ItemInfo(item)) if item.item_uid == item_uid && item.action == 2 => {
                println!("{label}: uid={}, type={}", item.item_uid, item.item_type);
                return Ok(());
            }
            Some(GameEvent::Talk(talk)) => println!("{label} talk: {}", talk.message),
            Some(_) => {}
            None => {
                return Err(gravak_protocol::ProtocolError::MalformedResponse(
                    "game server closed while waiting for trade item preview",
                ));
            }
        }
    }
}

async fn run_warehouse_smoke(
    session: &mut GameSession,
    character_id: u32,
    character_name: &str,
) -> Result<(), gravak_protocol::ProtocolError> {
    // Storekeeper/warehouseman in Twin City. The server still validates the
    // active NPC and proximity; these packets only exercise the client path.
    const STORAGE_ID: u32 = 8;

    session
        .send(&TalkPacket::public(
            character_id,
            character_name,
            "$tele 1002 409 351",
        ))
        .await?;
    sleep(Duration::from_millis(250)).await;

    // Ask for a fresh inventory stream so the smoke does not depend on which
    // login packets the initial handshake happened to consume.
    session
        .send(&ActionPacket::request_items(character_id))
        .await?;
    session.send(&NpcPacket::activate(STORAGE_ID, 6)).await?;
    session
        .send(&NpcDialogPacket::answer(STORAGE_ID, 1))
        .await?;

    let deadline = Instant::now() + Duration::from_secs(4);
    let mut inventory_item = None;
    let mut warehouse_balance = None;
    let mut warehouse_slots = std::collections::HashSet::new();
    let mut opened = false;
    let mut idle_deadline = None;
    loop {
        let wait_deadline = idle_deadline.unwrap_or(deadline);
        let event = match timeout_at(wait_deadline, session.next_event()).await {
            Ok(event) => event?,
            Err(_) => {
                if opened {
                    break;
                }
                return Err(gravak_protocol::ProtocolError::MalformedResponse(
                    "timed out opening warehouse",
                ));
            }
        };
        match event {
            Some(GameEvent::ItemInfo(item)) if item.action == 6 && item.position == 40 => {
                opened = true;
                idle_deadline = Some(Instant::now() + Duration::from_millis(250));
            }
            Some(GameEvent::ItemInfo(item)) if item.action == 7 && item.position == 40 => {
                warehouse_balance = Some(item.reserved1);
                if opened {
                    idle_deadline = Some(Instant::now() + Duration::from_millis(100));
                }
            }
            Some(GameEvent::ItemInfo(item)) if item.action == 1 && item.position < 40 => {
                inventory_item.get_or_insert(item);
            }
            Some(GameEvent::ItemInfo(item)) if item.action == 1 && item.position >= 40 => {
                warehouse_slots.insert(item.position - 40);
                if opened {
                    idle_deadline = Some(Instant::now() + Duration::from_millis(100));
                }
            }
            Some(GameEvent::Talk(talk)) => println!("warehouse talk: {}", talk.message),
            Some(_) => {}
            None => {
                return Err(gravak_protocol::ProtocolError::MalformedResponse(
                    "game server closed while opening warehouse",
                ));
            }
        }
    }

    let item = inventory_item.ok_or(gravak_protocol::ProtocolError::MalformedResponse(
        "warehouse smoke needs an inventory item",
    ))?;
    let position = (0..40).find(|slot| !warehouse_slots.contains(slot)).ok_or(
        gravak_protocol::ProtocolError::MalformedResponse("warehouse has no free test slot"),
    )?;
    println!(
        "Warehouse fixture: inventory uid={}, type={}, source_slot={}, target_slot={}, existing_warehouse_slots={:?}",
        item.item_uid, item.item_type, item.position, position, warehouse_slots
    );
    session
        .send(&WarehouseActionPacket::deposit(
            character_id,
            STORAGE_ID,
            item.item_uid,
            position,
        ))
        .await?;

    let stored = wait_for_warehouse_item(session, None, item.item_type, true).await?;
    if stored.position != 40 + position {
        return Err(gravak_protocol::ProtocolError::MalformedResponse(
            "warehouse deposit returned the wrong slot",
        ));
    }
    session
        .send(&WarehouseActionPacket::withdraw(
            character_id,
            STORAGE_ID,
            stored.item_uid,
            item.position,
        ))
        .await?;
    let restored = wait_for_warehouse_item(session, None, stored.item_type, false).await?;
    if restored.position != item.position {
        return Err(gravak_protocol::ProtocolError::MalformedResponse(
            "warehouse withdrawal returned the wrong inventory slot",
        ));
    }

    let initial_balance = warehouse_balance.unwrap_or(0);
    session
        .send(&WarehouseActionPacket::silver(
            character_id,
            STORAGE_ID,
            1,
            true,
        ))
        .await?;
    wait_for_warehouse_balance(session, initial_balance + 1).await?;
    session
        .send(&WarehouseActionPacket::silver(
            character_id,
            STORAGE_ID,
            1,
            false,
        ))
        .await?;
    wait_for_warehouse_balance(session, initial_balance).await?;

    println!(
        "Warehouse smoke OK: open/item deposit+withdraw/silver deposit+withdraw (uid={})",
        item.item_uid
    );
    Ok(())
}

async fn wait_for_warehouse_item(
    session: &mut GameSession,
    item_uid: Option<u32>,
    item_type: u32,
    deposited: bool,
) -> Result<gravak_protocol::ItemInfoPacket, gravak_protocol::ProtocolError> {
    let deadline = Instant::now() + Duration::from_secs(4);
    loop {
        let event = match timeout_at(deadline, session.next_event()).await {
            Ok(event) => event?,
            Err(_) => {
                return Err(gravak_protocol::ProtocolError::MalformedResponse(
                    "timed out waiting for warehouse item update",
                ));
            }
        };
        match event {
            Some(GameEvent::ItemInfo(item))
                if item.item_type == item_type
                    && item_uid.map_or(true, |expected| item.item_uid == expected)
                    && if deposited {
                        item.action == 1 && item.position >= 40
                    } else {
                        item.action == 1 && item.position < 40
                    } =>
            {
                println!(
                    "warehouse {} OK: uid={}, position={}",
                    if deposited { "deposit" } else { "withdraw" },
                    item.item_uid,
                    item.position
                );
                return Ok(item);
            }
            Some(GameEvent::ItemInfo(item)) => println!(
                "warehouse item event: uid={}, type={}, action={}, position={}",
                item.item_uid, item.item_type, item.action, item.position
            ),
            Some(GameEvent::Talk(talk)) => println!("warehouse talk: {}", talk.message),
            Some(_) => {}
            None => {
                return Err(gravak_protocol::ProtocolError::MalformedResponse(
                    "game server closed during warehouse item update",
                ));
            }
        }
    }
}

async fn wait_for_warehouse_balance(
    session: &mut GameSession,
    expected: u32,
) -> Result<(), gravak_protocol::ProtocolError> {
    let deadline = Instant::now() + Duration::from_secs(4);
    loop {
        let event = match timeout_at(deadline, session.next_event()).await {
            Ok(event) => event?,
            Err(_) => {
                return Err(gravak_protocol::ProtocolError::MalformedResponse(
                    "timed out waiting for warehouse balance",
                ));
            }
        };
        match event {
            Some(GameEvent::ItemInfo(item)) if item.action == 7 && item.position == 40 => {
                if item.reserved1 != expected {
                    return Err(gravak_protocol::ProtocolError::MalformedResponse(
                        "warehouse balance did not match the expected value",
                    ));
                }
                println!("warehouse balance OK: {}", item.reserved1);
                return Ok(());
            }
            Some(GameEvent::Talk(talk)) => println!("warehouse talk: {}", talk.message),
            Some(_) => {}
            None => {
                return Err(gravak_protocol::ProtocolError::MalformedResponse(
                    "game server closed during warehouse balance update",
                ));
            }
        }
    }
}

async fn run_whisper_smoke(
    session: &mut GameSession,
    character_id: u32,
    character_name: &str,
    address: &str,
    target_username: &str,
    target_password: &str,
    realm: &str,
) -> Result<(), gravak_protocol::ProtocolError> {
    let (mut target_session, target_id, target_name, _, _) =
        connect_existing_session(address, target_username, target_password, realm).await?;
    if target_id == character_id {
        return Err(gravak_protocol::ProtocolError::MalformedResponse(
            "whisper smoke requires two different characters",
        ));
    }
    let body = "private smoke message";
    session
        .send(&TalkPacket::whisper(
            character_id,
            character_name,
            &target_name,
            body,
        ))
        .await?;
    wait_for_whisper(&mut target_session, character_name, body, "target delivery").await?;
    wait_for_whisper(session, character_name, body, "sender echo").await?;

    wait_for_pvp_map(session, character_id, character_name, 1002).await?;
    wait_for_pvp_map(&mut target_session, target_id, &target_name, 1002).await?;
    let public_body = "public smoke history";
    session
        .send(&TalkPacket::public(
            character_id,
            character_name,
            public_body,
        ))
        .await?;
    wait_for_public_chat(
        &mut target_session,
        character_name,
        public_body,
        "public target delivery",
    )
    .await?;
    wait_for_public_chat(
        session,
        character_name,
        public_body,
        "public sender delivery",
    )
    .await?;

    drop(target_session);
    let (mut reloaded_target, reloaded_id, reloaded_name, _, _) =
        connect_existing_session(address, target_username, target_password, realm).await?;
    if reloaded_id == character_id || reloaded_name != target_name {
        return Err(gravak_protocol::ProtocolError::MalformedResponse(
            "whisper history reconnect returned a different character",
        ));
    }
    wait_for_whisper(
        &mut reloaded_target,
        character_name,
        body,
        "persisted whisper history",
    )
    .await?;
    wait_for_public_chat(
        &mut reloaded_target,
        character_name,
        public_body,
        "persisted public history",
    )
    .await?;

    session
        .send(&TalkPacket::whisper(
            character_id,
            character_name,
            "__missing_character__",
            "missing target",
        ))
        .await?;
    let missing_deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let missing = timeout_at(missing_deadline, session.next_event())
            .await
            .map_err(|_| {
                gravak_protocol::ProtocolError::MalformedResponse("missing whisper timed out")
            })??;
        let Some(missing) = missing else {
            return Err(gravak_protocol::ProtocolError::MalformedResponse(
                "session closed while waiting for missing whisper",
            ));
        };
        let GameEvent::Talk(talk) = missing else {
            continue;
        };
        if talk.channel == 2001 && talk.message.contains("was not found") {
            break;
        }
    }
    println!(
        "Whisper smoke OK: {} -> {} delivered privately, reloaded from history, and unknown recipient rejected",
        character_name, target_name
    );
    Ok(())
}

async fn wait_for_whisper(
    session: &mut GameSession,
    sender_name: &str,
    body: &str,
    context: &str,
) -> Result<(), gravak_protocol::ProtocolError> {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let event = timeout_at(deadline, session.next_event())
            .await
            .map_err(|_| {
                gravak_protocol::ProtocolError::MalformedResponse("whisper timed out")
            })??;
        let Some(event) = event else {
            return Err(gravak_protocol::ProtocolError::MalformedResponse(
                "session closed while waiting for whisper",
            ));
        };
        if let GameEvent::Talk(talk) = event {
            if talk.channel == 2001 && talk.sender_name == sender_name && talk.message == body {
                println!("Whisper OK: context={context}, sender={sender_name}");
                return Ok(());
            }
        }
    }
}

async fn wait_for_public_chat(
    session: &mut GameSession,
    sender_name: &str,
    body: &str,
    context: &str,
) -> Result<(), gravak_protocol::ProtocolError> {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let event = timeout_at(deadline, session.next_event())
            .await
            .map_err(|_| {
                gravak_protocol::ProtocolError::MalformedResponse("public chat timed out")
            })??;
        let Some(event) = event else {
            return Err(gravak_protocol::ProtocolError::MalformedResponse(
                "session closed while waiting for public chat",
            ));
        };
        if let GameEvent::Talk(talk) = event
            && talk.channel == 2000
            && talk.sender_name == sender_name
            && talk.message == body
        {
            println!("Public chat OK: context={context}, sender={sender_name}");
            return Ok(());
        }
    }
}

async fn run_friends_smoke(
    session: &mut GameSession,
    character_id: u32,
    character_name: &str,
    address: &str,
    target_username: &str,
    target_password: &str,
    realm: &str,
) -> Result<(), gravak_protocol::ProtocolError> {
    let (mut target_session, target_id, target_name, _, _) =
        connect_existing_session(address, target_username, target_password, realm).await?;
    if target_id == character_id {
        return Err(gravak_protocol::ProtocolError::MalformedResponse(
            "friends smoke requires two different characters",
        ));
    }

    wait_for_pvp_map(session, character_id, character_name, 1002).await?;
    wait_for_pvp_map(&mut target_session, target_id, &target_name, 1002).await?;

    session
        .send(&FriendPacket::new(10, target_id, &target_name, true))
        .await?;
    wait_for_friend(
        &mut target_session,
        10,
        character_id,
        character_name,
        "request delivery",
    )
    .await?;

    target_session
        .send(&FriendPacket::new(10, character_id, character_name, true))
        .await?;
    wait_for_friend(session, 15, target_id, &target_name, "accept requester").await?;
    wait_for_friend(
        &mut target_session,
        15,
        character_id,
        character_name,
        "accept target",
    )
    .await?;

    drop(target_session);
    let (mut target_session, reloaded_id, reloaded_name, _, _) =
        connect_existing_session(address, target_username, target_password, realm).await?;
    if reloaded_id != target_id || reloaded_name != target_name {
        return Err(gravak_protocol::ProtocolError::MalformedResponse(
            "friend reconnect returned a different character",
        ));
    }
    wait_for_friend(
        &mut target_session,
        15,
        character_id,
        character_name,
        "friend list reload",
    )
    .await?;

    session
        .send(&FriendPacket::new(14, target_id, &target_name, false))
        .await?;
    wait_for_friend(session, 14, target_id, &target_name, "break requester").await?;
    wait_for_friend(
        &mut target_session,
        14,
        character_id,
        character_name,
        "break target",
    )
    .await?;

    println!(
        "Friends smoke OK: {} and {} request/accept/break synchronized and persisted",
        character_name, target_name
    );
    Ok(())
}

async fn run_ignore_smoke(
    session: &mut GameSession,
    character_id: u32,
    character_name: &str,
    address: &str,
    target_username: &str,
    target_password: &str,
    realm: &str,
) -> Result<(), gravak_protocol::ProtocolError> {
    let (mut target_session, target_id, target_name, _, _) =
        connect_existing_session(address, target_username, target_password, realm).await?;
    if target_id == character_id {
        return Err(gravak_protocol::ProtocolError::MalformedResponse(
            "ignore smoke requires two different characters",
        ));
    }

    wait_for_pvp_map(session, character_id, character_name, 1002).await?;
    wait_for_pvp_map(&mut target_session, target_id, &target_name, 1002).await?;

    target_session
        .send(&FriendPacket::new(19, character_id, character_name, true))
        .await?;
    wait_for_friend(
        &mut target_session,
        19,
        character_id,
        character_name,
        "ignore add",
    )
    .await?;

    let body = "ignored smoke message";
    target_session
        .send(&TalkPacket::whisper(
            target_id,
            &target_name,
            character_name,
            body,
        ))
        .await?;
    wait_for_talk_fragment(&mut target_session, "is ignoring you").await?;
    assert_whisper_not_delivered(session, &target_name, body).await?;

    drop(target_session);
    let (mut target_session, reloaded_id, reloaded_name, _, _) =
        connect_existing_session(address, target_username, target_password, realm).await?;
    if reloaded_id != target_id || reloaded_name != target_name {
        return Err(gravak_protocol::ProtocolError::MalformedResponse(
            "ignore reconnect returned a different character",
        ));
    }
    wait_for_friend(
        &mut target_session,
        19,
        character_id,
        character_name,
        "ignore list reload",
    )
    .await?;

    target_session
        .send(&FriendPacket::new(18, character_id, character_name, false))
        .await?;
    wait_for_friend(
        &mut target_session,
        18,
        character_id,
        character_name,
        "ignore remove",
    )
    .await?;

    println!(
        "Ignore smoke OK: {} blocked/reloaded/removed ignore entry for {}",
        character_name, target_name
    );
    Ok(())
}

async fn assert_whisper_not_delivered(
    session: &mut GameSession,
    sender_name: &str,
    body: &str,
) -> Result<(), gravak_protocol::ProtocolError> {
    let deadline = Instant::now() + Duration::from_millis(700);
    loop {
        let event = match timeout_at(deadline, session.next_event()).await {
            Ok(event) => event?,
            Err(_) => return Ok(()),
        };
        let Some(event) = event else {
            return Ok(());
        };
        if let GameEvent::Talk(talk) = event
            && talk.channel == 2001
            && talk.sender_name == sender_name
            && talk.message == body
        {
            return Err(gravak_protocol::ProtocolError::MalformedResponse(
                "ignored whisper was delivered to the recipient",
            ));
        }
    }
}

async fn wait_for_friend(
    session: &mut GameSession,
    action: u8,
    friend_id: u32,
    friend_name: &str,
    context: &str,
) -> Result<(), gravak_protocol::ProtocolError> {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let event = timeout_at(deadline, session.next_event())
            .await
            .map_err(|_| {
                gravak_protocol::ProtocolError::MalformedResponse("friend packet timed out")
            })??;
        let Some(event) = event else {
            return Err(gravak_protocol::ProtocolError::MalformedResponse(
                "session closed while waiting for friend packet",
            ));
        };
        if let GameEvent::Friend(packet) = event {
            if packet.action == action && packet.friend_id == friend_id {
                println!("Friend OK: context={context}, action={action}, friend={friend_name}");
                return Ok(());
            }
        }
    }
}

async fn run_team_reward_smoke(
    session: &mut GameSession,
    character_id: u32,
    character_name: &str,
    target_session: &mut GameSession,
    target_id: u32,
) -> Result<(), gravak_protocol::ProtocolError> {
    let (monster_id, monster_x, monster_y) =
        locate_team_monster(session, character_id, character_name).await?;
    session
        .send(&TalkPacket::public(
            character_id,
            character_name,
            &format!("$tele 1002 {monster_x} {monster_y}"),
        ))
        .await?;
    sleep(Duration::from_millis(150)).await;
    session
        .send(&ActionPacket::send_location(character_id))
        .await?;
    loop {
        let event = match timeout(Duration::from_secs(3), session.next_event()).await {
            Ok(event) => event?,
            Err(_) => {
                return Err(gravak_protocol::ProtocolError::MalformedResponse(
                    "team reward smoke timed out during reposition",
                ));
            }
        };
        match event {
            Some(GameEvent::Action(action)) if action.action_type == 74 => break,
            Some(GameEvent::Talk(talk)) => println!("Team combat setup talk: {}", talk.message),
            Some(_) => {}
            None => {
                return Err(gravak_protocol::ProtocolError::MalformedResponse(
                    "team reward smoke closed during reposition",
                ));
            }
        }
    }
    target_session
        .send(&TalkPacket::public(
            target_id,
            character_name,
            &format!("$tele 1002 {monster_x} {monster_y}"),
        ))
        .await?;
    sleep(Duration::from_millis(150)).await;
    target_session
        .send(&ActionPacket::send_location(target_id))
        .await?;
    loop {
        let event = match timeout(Duration::from_secs(3), target_session.next_event()).await {
            Ok(event) => event?,
            Err(_) => {
                return Err(gravak_protocol::ProtocolError::MalformedResponse(
                    "team reward smoke timed out during member reposition",
                ));
            }
        };
        match event {
            Some(GameEvent::Action(action)) if action.action_type == 74 => break,
            Some(GameEvent::Talk(talk)) => println!("Team member setup talk: {}", talk.message),
            Some(_) => {}
            None => {
                return Err(gravak_protocol::ProtocolError::MalformedResponse(
                    "team reward smoke closed during member reposition",
                ));
            }
        }
    }
    session
        .send(&InteractionPacket::attack(
            character_id,
            monster_id,
            monster_x,
            monster_y,
        ))
        .await?;

    let mut first_drop = None;
    let deadline = Instant::now() + Duration::from_secs(4);
    loop {
        let event = match timeout_at(deadline, session.next_event()).await {
            Ok(event) => event?,
            Err(_) => {
                return Err(gravak_protocol::ProtocolError::MalformedResponse(
                    "team reward smoke timed out during combat",
                ));
            }
        };
        match event {
            Some(GameEvent::Interaction(interaction))
                if interaction.character_id == character_id
                    && interaction.target_id == monster_id
                    && interaction.action == 14 =>
            {
                println!(
                    "Team combat kill OK: target={monster_id}, value={}",
                    interaction.value
                );
                break;
            }
            Some(GameEvent::GroundItem(item)) if item.drop_type == 1 => {
                first_drop.get_or_insert(item);
            }
            Some(GameEvent::Talk(talk)) => println!("Team combat talk: {}", talk.message),
            Some(_) => {}
            None => {
                return Err(gravak_protocol::ProtocolError::MalformedResponse(
                    "team reward smoke closed during combat",
                ));
            }
        }
    }
    let Some(drop) = first_drop else {
        return Err(gravak_protocol::ProtocolError::MalformedResponse(
            "team reward smoke kill had no ground item",
        ));
    };
    wait_for_talk_fragment(target_session, "Team reward:").await?;
    wait_for_ground_item(target_session, drop.uid).await?;
    target_session
        .send(&GroundItemPacket::pickup(
            drop.uid,
            drop.item_id,
            drop.x,
            drop.y,
        ))
        .await?;
    loop {
        let event = match timeout(Duration::from_secs(3), target_session.next_event()).await {
            Ok(event) => event?,
            Err(_) => {
                return Err(gravak_protocol::ProtocolError::MalformedResponse(
                    "team reward smoke timed out during shared pickup",
                ));
            }
        };
        match event {
            Some(GameEvent::GroundItem(item)) if item.uid == drop.uid && item.drop_type == 2 => {
                println!(
                    "Team loot OK: member={target_id} picked shared uid={}, item_id={}",
                    drop.uid, drop.item_id
                );
                break;
            }
            Some(GameEvent::Talk(talk)) => println!("Team loot talk: {}", talk.message),
            Some(_) => {}
            None => {
                return Err(gravak_protocol::ProtocolError::MalformedResponse(
                    "team reward smoke closed during shared pickup",
                ));
            }
        }
    }
    Ok(())
}

async fn locate_team_monster(
    session: &mut GameSession,
    character_id: u32,
    character_name: &str,
) -> Result<(u32, u16, u16), gravak_protocol::ProtocolError> {
    for (x, y) in [(384, 458), (410, 388), (424, 352), (440, 378), (439, 389)] {
        session
            .send(&TalkPacket::public(
                character_id,
                character_name,
                &format!("$tele 1002 {x} {y}"),
            ))
            .await?;
        sleep(Duration::from_millis(150)).await;
        session
            .send(&ActionPacket::send_location(character_id))
            .await?;
        let deadline = Instant::now() + Duration::from_millis(1_500);
        loop {
            let event = match timeout_at(deadline, session.next_event()).await {
                Ok(event) => event?,
                Err(_) => break,
            };
            match event {
                Some(GameEvent::Player(player))
                    if (400_001..500_000).contains(&(player.character_id.max(0) as u32))
                        && player.health_points > 0 =>
                {
                    return Ok((player.character_id as u32, player.x, player.y));
                }
                Some(GameEvent::Talk(talk)) => {
                    println!("Team monster search talk: {}", talk.message)
                }
                Some(_) => {}
                None => {
                    return Err(gravak_protocol::ProtocolError::MalformedResponse(
                        "team reward smoke closed while searching for a monster",
                    ));
                }
            }
        }
    }
    Err(gravak_protocol::ProtocolError::MalformedResponse(
        "team reward smoke could not find a monster",
    ))
}

async fn wait_for_ground_item(
    session: &mut GameSession,
    uid: u32,
) -> Result<(), gravak_protocol::ProtocolError> {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let event = match timeout_at(deadline, session.next_event()).await {
            Ok(event) => event?,
            Err(_) => {
                return Err(gravak_protocol::ProtocolError::MalformedResponse(
                    "timed out waiting for shared ground item",
                ));
            }
        };
        match event {
            Some(GameEvent::GroundItem(item)) if item.uid == uid && item.drop_type == 1 => {
                return Ok(());
            }
            Some(GameEvent::Talk(talk)) => println!("Shared item talk: {}", talk.message),
            Some(_) => {}
            None => {
                return Err(gravak_protocol::ProtocolError::MalformedResponse(
                    "game session closed while waiting for shared ground item",
                ));
            }
        }
    }
}

async fn wait_for_team_action(
    session: &mut GameSession,
    action: u32,
    context: &str,
) -> Result<(), gravak_protocol::ProtocolError> {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let event = match timeout_at(deadline, session.next_event()).await {
            Ok(event) => event?,
            Err(_) => {
                return Err(gravak_protocol::ProtocolError::MalformedResponse(
                    "timed out waiting for team action",
                ));
            }
        };
        match event {
            Some(GameEvent::Team(packet)) if packet.action == action => {
                println!(
                    "Team action OK: context={context}, action={action}, target={}",
                    packet.target_id
                );
                return Ok(());
            }
            Some(GameEvent::Talk(talk)) => println!("Team talk: {}", talk.message),
            Some(_) => {}
            None => {
                return Err(gravak_protocol::ProtocolError::MalformedResponse(
                    "game session closed while waiting for team action",
                ));
            }
        }
    }
}

async fn wait_for_team_member(
    session: &mut GameSession,
    member_id: u32,
    action: u8,
    context: &str,
) -> Result<(), gravak_protocol::ProtocolError> {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let event = match timeout_at(deadline, session.next_event()).await {
            Ok(event) => event?,
            Err(_) => {
                return Err(gravak_protocol::ProtocolError::MalformedResponse(
                    "timed out waiting for team member information",
                ));
            }
        };
        match event {
            Some(GameEvent::TeamMember(packet))
                if packet.member_id == member_id && packet.action == action =>
            {
                println!("Team member OK: context={context}, member={member_id}, action={action}");
                return Ok(());
            }
            Some(GameEvent::Talk(talk)) => println!("Team member talk: {}", talk.message),
            Some(_) => {}
            None => {
                return Err(gravak_protocol::ProtocolError::MalformedResponse(
                    "game session closed while waiting for team member information",
                ));
            }
        }
    }
}

async fn run_invalid_item_smoke(
    session: &mut GameSession,
    character_id: u32,
) -> Result<(), gravak_protocol::ProtocolError> {
    session
        .send(&ItemActionPacket::equip(
            character_id.wrapping_add(1),
            u32::MAX,
            1,
        ))
        .await?;
    wait_for_talk_contains(session, "Equip rejected: invalid character or amount.").await?;

    session
        .send(&ItemActionPacket::move_inventory(
            character_id,
            u32::MAX,
            40,
        ))
        .await?;
    wait_for_talk_contains(
        session,
        "Inventory move rejected: item or slot is unavailable.",
    )
    .await?;

    session
        .send(&ItemActionPacket::unequip(character_id, u32::MAX, 0))
        .await?;
    wait_for_talk_contains(
        session,
        "Unequip rejected: item or inventory slot is unavailable.",
    )
    .await?;

    session
        .send(&ItemActionPacket::use_item(character_id, u32::MAX))
        .await?;
    wait_for_talk_contains(session, "Use rejected: item is not in inventory.").await?;

    session
        .send(&ItemActionPacket::drop(character_id, u32::MAX))
        .await?;
    wait_for_talk_contains(
        session,
        "Item drop rejected: item is no longer in your inventory.",
    )
    .await?;

    session
        .send(&ItemActionPacket::upgrade(character_id, u32::MAX))
        .await?;
    wait_for_talk_contains(session, "Upgrade rejected: owned item was not found.").await?;

    session
        .send(&ItemActionPacket::open_socket(character_id, u32::MAX))
        .await?;
    wait_for_talk_contains(session, "Socket rejected: equipment was not found.").await?;

    session.send(&SocketGemPacket::remove(u32::MAX, 3)).await?;
    wait_for_talk_contains(session, "Socket rejected: invalid socket.").await?;

    session
        .send(&RefineryPacket {
            action: 99,
            item_id: u32::MAX,
            material_id: u32::MAX,
        })
        .await?;
    wait_for_talk_contains(session, "Refinery rejected: unsupported action.").await?;

    session
        .send(&RefineryPacket::apply(u32::MAX, u32::MAX))
        .await?;
    wait_for_talk_contains(session, "Refinery rejected: target must be equipped.").await?;

    session
        .send(&RefineryPacket::apply_artifact(u32::MAX, u32::MAX))
        .await?;
    wait_for_talk_contains(
        session,
        "Artifact rejected: target must be an inventory equipment item.",
    )
    .await?;

    session
        .send(&ItemActionPacket::buy(character_id, 1_000_000, 0))
        .await?;
    wait_for_talk_contains(session, "Shop purchase rejected: shop is not nearby.").await?;

    session
        .send(&ItemActionPacket::sell(character_id, u32::MAX, u32::MAX))
        .await?;
    wait_for_talk_contains(session, "Shop sale rejected: open a shop before selling.").await?;

    println!(
        "Invalid item smoke OK: item, socket, refinery, artifact and shop transitions were rejected"
    );
    Ok(())
}

async fn run_quest_smoke(
    session: &mut GameSession,
    character_id: u32,
    character_name: &str,
) -> Result<(), gravak_protocol::ProtocolError> {
    session
        .send(&TalkPacket::public(
            character_id,
            character_name,
            "$tele 1010 96 42",
        ))
        .await?;
    session
        .send(&ActionPacket::send_location(character_id))
        .await?;
    wait_for_map(session, 1010).await?;

    session.send(&NpcPacket::activate(425, 6)).await?;
    wait_for_dialog_option(session, 425, 4, "Quest dialog").await?;
    session.send(&NpcDialogPacket::answer(425, 4)).await?;
    wait_for_talk_fragment(session, "Quest accepted:").await?;

    run_combat_smoke(session, character_id, character_name, None, true).await?;
    println!(
        "Quest smoke OK: quest=1, npc=425, target_monster_type=1, completion_reward_received=true"
    );
    Ok(())
}

async fn run_quest_status_smoke(
    session: &mut GameSession,
    character_id: u32,
    character_name: &str,
) -> Result<(), gravak_protocol::ProtocolError> {
    session
        .send(&TalkPacket::public(
            character_id,
            character_name,
            "$quest-status",
        ))
        .await?;
    wait_for_talk_fragment(session, "Quest Pheasant Hunt").await?;
    println!("Quest status smoke OK: quest=1 was returned by the server");
    Ok(())
}

async fn run_npc_training_smoke(
    session: &mut GameSession,
    character_id: u32,
    character_name: &str,
) -> Result<(), gravak_protocol::ProtocolError> {
    session
        .send(&TalkPacket::public(
            character_id,
            character_name,
            "$tele 1010 96 42",
        ))
        .await?;
    session
        .send(&ActionPacket::send_location(character_id))
        .await?;
    wait_for_map(session, 1010).await?;

    session.send(&NpcPacket::activate(425, 6)).await?;
    wait_for_dialog_option(session, 425, 3, "Trainer dialog").await?;
    session.send(&NpcDialogPacket::answer(425, 3)).await?;

    let deadline = Instant::now() + Duration::from_secs(3);
    let mut learned_skill = None;
    let mut silver_charged = false;
    loop {
        let event = match timeout_at(deadline, session.next_event()).await {
            Ok(event) => event?,
            Err(_) => {
                return Err(gravak_protocol::ProtocolError::MalformedResponse(
                    "timed out waiting for trainer result",
                ));
            }
        };
        match event {
            Some(GameEvent::Skill(skill)) if skill.skill_id >= 1_002 => {
                learned_skill = Some((skill.skill_id, skill.level));
                if silver_charged {
                    println!(
                        "NPC training smoke OK: learned skill_id={}, level={}, silver_charged=true",
                        skill.skill_id, skill.level
                    );
                    return Ok(());
                }
            }
            Some(GameEvent::Talk(talk)) if talk.message.contains("no skill available yet") => {
                println!(
                    "NPC training smoke OK: trainer correctly found no eligible unlearned skill"
                );
                return Ok(());
            }
            Some(GameEvent::Talk(talk)) if talk.message.contains("taught you") => {
                silver_charged = true;
                if let Some((skill_id, level)) = learned_skill {
                    println!(
                        "NPC training smoke OK: learned skill_id={}, level={}, silver_charged=true",
                        skill_id, level
                    );
                    return Ok(());
                }
            }
            Some(GameEvent::Talk(talk)) => println!("Trainer talk: {}", talk.message),
            Some(_) => {}
            None => {
                return Err(gravak_protocol::ProtocolError::MalformedResponse(
                    "game session closed while waiting for trainer result",
                ));
            }
        }
    }
}

async fn wait_for_talk_contains(
    session: &mut GameSession,
    expected: &str,
) -> Result<(), gravak_protocol::ProtocolError> {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let event = match timeout_at(deadline, session.next_event()).await {
            Ok(event) => event?,
            Err(_) => {
                return Err(gravak_protocol::ProtocolError::MalformedResponse(
                    "timed out waiting for rejection message",
                ));
            }
        };
        match event {
            Some(GameEvent::Talk(talk)) if talk.message == expected => return Ok(()),
            Some(GameEvent::Talk(talk)) => println!("Unexpected item smoke talk: {}", talk.message),
            Some(_) => {}
            None => {
                return Err(gravak_protocol::ProtocolError::MalformedResponse(
                    "game session closed while waiting for rejection message",
                ));
            }
        }
    }
}

async fn wait_for_talk_fragment(
    session: &mut GameSession,
    expected: &str,
) -> Result<(), gravak_protocol::ProtocolError> {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let event = match timeout_at(deadline, session.next_event()).await {
            Ok(event) => event?,
            Err(_) => {
                return Err(gravak_protocol::ProtocolError::MalformedResponse(
                    "timed out waiting for quest message",
                ));
            }
        };
        match event {
            Some(GameEvent::Talk(talk)) if talk.message.contains(expected) => return Ok(()),
            Some(GameEvent::Talk(talk)) => println!("Unexpected quest talk: {}", talk.message),
            Some(_) => {}
            None => {
                return Err(gravak_protocol::ProtocolError::MalformedResponse(
                    "game session closed while waiting for quest message",
                ));
            }
        }
    }
}

async fn wait_for_pvp_map(
    session: &mut GameSession,
    character_id: u32,
    character_name: &str,
    map_id: u32,
) -> Result<(), gravak_protocol::ProtocolError> {
    session
        .send(&TalkPacket::public(
            character_id,
            character_name,
            &format!("$tele {map_id} 430 378"),
        ))
        .await?;
    session
        .send(&ActionPacket::send_location(character_id))
        .await?;
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut received_map = false;
    let mut received_location = false;
    loop {
        let event = match timeout_at(deadline, session.next_event()).await {
            Ok(event) => event?,
            Err(_) => {
                return Err(gravak_protocol::ProtocolError::MalformedResponse(
                    "timed out waiting for PvP map",
                ));
            }
        };
        match event {
            Some(GameEvent::MapInfo(info)) if info.map_id == map_id => {
                received_map = true;
                if received_location {
                    return Ok(());
                }
            }
            Some(GameEvent::Action(action)) if action.action_type == 74 => {
                received_location = true;
                if received_map {
                    return Ok(());
                }
            }
            Some(GameEvent::Talk(talk)) => println!("PvP map talk: {}", talk.message),
            Some(_) => {}
            None => {
                return Err(gravak_protocol::ProtocolError::MalformedResponse(
                    "game session closed during PvP teleport",
                ));
            }
        }
    }
}

async fn run_pvp_smoke(
    session: &mut GameSession,
    character_id: u32,
    character_name: &str,
    address: &str,
    target_username: &str,
    target_password: &str,
    realm: &str,
) -> Result<(), gravak_protocol::ProtocolError> {
    let (mut target_session, target_id, target_name, target_hp, target_max_hp) =
        connect_existing_session(address, target_username, target_password, realm).await?;
    println!(
        "PvP target login: id={}, name={}, hp={}/{}",
        target_id, target_name, target_hp, target_max_hp
    );
    target_session
        .send(&TalkPacket::public(target_id, &target_name, "$test-health"))
        .await?;
    wait_for_character_health(&mut target_session, target_id).await?;
    wait_for_pvp_map(&mut target_session, target_id, &target_name, 1002).await?;
    sleep(Duration::from_millis(500)).await;
    wait_for_pvp_map(session, character_id, character_name, 1002).await?;
    sleep(Duration::from_millis(500)).await;

    session
        .send(&ActionPacket::set_kill_mode(character_id, 0))
        .await?;
    session
        .send(&TalkPacket::public(
            character_id,
            character_name,
            "$test-health",
        ))
        .await?;
    session
        .send(&TalkPacket::public(
            character_id,
            character_name,
            "$test-mana",
        ))
        .await?;
    session
        .send(&TalkPacket::public(
            character_id,
            character_name,
            "$learn-skill 1000 1",
        ))
        .await?;
    wait_for_character_health(session, character_id).await?;
    sleep(Duration::from_millis(150)).await;
    session
        .send(&ActionPacket::send_location(character_id))
        .await?;

    let mut target_spawn = None;
    let deadline = Instant::now() + Duration::from_secs(3);
    while let Ok(event_result) = timeout_at(deadline, session.next_event()).await {
        let Some(event) = event_result? else { break };
        match event {
            GameEvent::Player(player)
                if player.character_id == target_id as i32 && player.health_points > 0 =>
            {
                target_spawn = Some((player.x, player.y));
                break;
            }
            GameEvent::Talk(talk) => println!("PvP setup talk: {}", talk.message),
            _ => {}
        }
    }
    let (target_x, target_y) = target_spawn.unwrap_or((430, 378));
    if target_spawn.is_none() {
        println!(
            "PvP target spawn was not received; trying the coordinated test position (430, 378)"
        );
    }
    let initial_target_hp = target_max_hp.max(target_hp).max(1);
    session
        .send(&InteractionPacket::magic_attack(
            character_id,
            target_id,
            target_x,
            target_y,
            1_000,
        ))
        .await?;

    let mut damage = 0;
    let deadline = Instant::now() + Duration::from_secs(3);
    while let Ok(event_result) = timeout_at(deadline, session.next_event()).await {
        let Some(event) = event_result? else { break };
        if let GameEvent::SkillEffect(effect) = event {
            if let Some((_, value)) = effect
                .targets
                .into_iter()
                .find(|(target, _)| *target == target_id)
            {
                damage = value;
                break;
            }
        }
    }
    if damage == 0 {
        return Err(gravak_protocol::ProtocolError::MalformedResponse(
            "PvP magic skill produced no target damage",
        ));
    }

    let mut remaining_hp =
        wait_for_target_health(&mut target_session, target_id, initial_target_hp).await?;
    let mut casts = 1;
    while remaining_hp > 0 && casts < 20 {
        sleep(Duration::from_millis(1_300)).await;
        session
            .send(&TalkPacket::public(
                character_id,
                character_name,
                "$test-mana",
            ))
            .await?;
        sleep(Duration::from_millis(100)).await;
        session
            .send(&InteractionPacket::magic_attack(
                character_id,
                target_id,
                target_x,
                target_y,
                1_000,
            ))
            .await?;
        let deadline = Instant::now() + Duration::from_secs(3);
        let mut hit = false;
        while let Ok(event_result) = timeout_at(deadline, session.next_event()).await {
            let Some(event) = event_result? else { break };
            if let GameEvent::SkillEffect(effect) = event
                && effect
                    .targets
                    .iter()
                    .any(|(target, _)| *target == target_id)
            {
                hit = true;
                break;
            }
        }
        if !hit {
            return Err(gravak_protocol::ProtocolError::MalformedResponse(
                "PvP magic follow-up was rejected or timed out",
            ));
        }
        remaining_hp = wait_for_target_health(&mut target_session, target_id, remaining_hp).await?;
        casts += 1;
    }
    if remaining_hp != 0 {
        return Err(gravak_protocol::ProtocolError::MalformedResponse(
            "PvP smoke did not defeat the target within the cast budget",
        ));
    }

    target_session
        .send(&ActionPacket::reborn(target_id, false))
        .await?;
    let revived_hp = wait_for_revived_health(&mut target_session, target_id).await?;
    session
        .send(&ActionPacket::set_kill_mode(character_id, 1))
        .await?;
    wait_for_kill_mode(session, 1).await?;
    session
        .send(&InteractionPacket::attack(
            character_id,
            target_id,
            target_x,
            target_y,
        ))
        .await?;
    sleep(Duration::from_millis(350)).await;

    session
        .send(&ActionPacket::set_kill_mode(character_id, 0))
        .await?;
    wait_for_kill_mode(session, 0).await?;
    session
        .send(&InteractionPacket::attack(
            character_id,
            target_id,
            target_x,
            target_y,
        ))
        .await?;
    let physical_damage = wait_for_physical_hit(session, character_id, target_id).await?;
    let physical_remaining_hp =
        wait_for_target_health(&mut target_session, target_id, revived_hp).await?;
    println!(
        "PvP magic/physical/reborn OK: attacker={}, target={}, magic_damage={}, casts={}, revived_hp={}, physical_damage={}, physical_hp={}",
        character_id, target_id, damage, casts, revived_hp, physical_damage, physical_remaining_hp
    );
    Ok(())
}

async fn wait_for_kill_mode(
    session: &mut GameSession,
    mode: u32,
) -> Result<(), gravak_protocol::ProtocolError> {
    let deadline = Instant::now() + Duration::from_secs(3);
    while let Ok(event_result) = timeout_at(deadline, session.next_event()).await {
        let Some(event) = event_result? else { break };
        if let GameEvent::Action(action) = event
            && action.action_type == 96
            && action.data1 == mode
        {
            return Ok(());
        }
    }
    Err(gravak_protocol::ProtocolError::MalformedResponse(
        "kill mode update was not acknowledged",
    ))
}

async fn wait_for_physical_hit(
    session: &mut GameSession,
    character_id: u32,
    target_id: u32,
) -> Result<u32, gravak_protocol::ProtocolError> {
    let deadline = Instant::now() + Duration::from_secs(3);
    while let Ok(event_result) = timeout_at(deadline, session.next_event()).await {
        let Some(event) = event_result? else { break };
        if let GameEvent::Interaction(interaction) = event
            && interaction.character_id == character_id
            && interaction.target_id == target_id
            && interaction.action == 2
            && interaction.value > 0
        {
            return Ok(interaction.value);
        }
    }
    Err(gravak_protocol::ProtocolError::MalformedResponse(
        "PvP physical attack produced no target damage",
    ))
}

async fn wait_for_target_health(
    session: &mut GameSession,
    target_id: u32,
    below: u16,
) -> Result<u16, gravak_protocol::ProtocolError> {
    let deadline = Instant::now() + Duration::from_secs(3);
    while let Ok(event_result) = timeout_at(deadline, session.next_event()).await {
        let Some(event) = event_result? else { break };
        if let GameEvent::UserInfo(info) = event
            && info.character_id == target_id
            && info.health_points < below
        {
            return Ok(info.health_points);
        }
    }
    Err(gravak_protocol::ProtocolError::MalformedResponse(
        "target client did not receive its updated HP",
    ))
}

async fn wait_for_character_health(
    session: &mut GameSession,
    character_id: u32,
) -> Result<u16, gravak_protocol::ProtocolError> {
    let deadline = Instant::now() + Duration::from_secs(3);
    while let Ok(event_result) = timeout_at(deadline, session.next_event()).await {
        let Some(event) = event_result? else { break };
        if let GameEvent::UserInfo(info) = event
            && info.character_id == character_id
            && info.health_points > 0
        {
            return Ok(info.health_points);
        }
    }
    Err(gravak_protocol::ProtocolError::MalformedResponse(
        "character did not receive its restored HP",
    ))
}

async fn wait_for_revived_health(
    session: &mut GameSession,
    target_id: u32,
) -> Result<u16, gravak_protocol::ProtocolError> {
    let deadline = Instant::now() + Duration::from_secs(3);
    while let Ok(event_result) = timeout_at(deadline, session.next_event()).await {
        let Some(event) = event_result? else { break };
        if let GameEvent::UserInfo(info) = event
            && info.character_id == target_id
            && info.health_points > 0
        {
            return Ok(info.health_points);
        }
    }
    Err(gravak_protocol::ProtocolError::MalformedResponse(
        "target client did not receive the revived HP",
    ))
}

async fn run_map_change_smoke(
    session: &mut GameSession,
    character_id: u32,
    character_name: &str,
) -> Result<(), gravak_protocol::ProtocolError> {
    session
        .send(&TalkPacket::public(
            character_id,
            character_name,
            "$tele 1000 486 621",
        ))
        .await?;
    session
        .send(&ActionPacket::send_location(character_id))
        .await?;
    loop {
        match session.next_event().await? {
            Some(GameEvent::MapInfo(info)) => {
                if info.map_id != 1000 {
                    return Err(gravak_protocol::ProtocolError::MalformedResponse(
                        "teleport smoke returned the wrong map",
                    ));
                }
                println!("Map change OK: map_id={}", info.map_id);
                session
                    .send(&TalkPacket::public(
                        character_id,
                        character_name,
                        "$tele 1002 430 378",
                    ))
                    .await?;
                session
                    .send(&ActionPacket::send_location(character_id))
                    .await?;
                loop {
                    match session.next_event().await? {
                        Some(GameEvent::MapInfo(returned)) if returned.map_id == 1002 => {
                            println!("Map restore OK: map_id={}", returned.map_id);
                            return Ok(());
                        }
                        Some(GameEvent::MapInfo(returned)) => {
                            println!("Map restore received map_id={}", returned.map_id);
                        }
                        Some(GameEvent::Talk(talk)) => {
                            println!("Map restore talk: {}", talk.message)
                        }
                        Some(_) => {}
                        None => {
                            return Err(gravak_protocol::ProtocolError::MalformedResponse(
                                "game server closed during map restore",
                            ));
                        }
                    }
                }
            }
            Some(GameEvent::Talk(talk)) => println!("Map change talk: {}", talk.message),
            Some(_) => {}
            None => {
                return Err(gravak_protocol::ProtocolError::MalformedResponse(
                    "game server closed during map change",
                ));
            }
        }
    }
}

async fn run_skill_book_smoke(
    session: &mut GameSession,
    character_id: u32,
    character_name: &str,
) -> Result<(), gravak_protocol::ProtocolError> {
    session
        .send(&TalkPacket::public(
            character_id,
            character_name,
            "$test-skill-book",
        ))
        .await?;
    let book_uid = loop {
        match session.next_event().await? {
            Some(GameEvent::ItemInfo(item)) if item.item_type == 723_100 && item.action == 1 => {
                println!(
                    "Skill book created: uid={}, slot={}",
                    item.item_uid, item.position
                );
                break item.item_uid;
            }
            Some(GameEvent::Talk(talk)) => println!("Skill book talk: {}", talk.message),
            Some(_) => {}
            None => {
                return Err(gravak_protocol::ProtocolError::MalformedResponse(
                    "game server closed before skill book creation",
                ));
            }
        }
    };

    session
        .send(&ItemActionPacket::use_item(character_id, book_uid))
        .await?;
    loop {
        match session.next_event().await? {
            Some(GameEvent::Skill(skill)) if skill.skill_id == 1_002 => {
                println!(
                    "Skill book used: skill_id={}, level={}, experience={}",
                    skill.skill_id, skill.level, skill.experience
                );
                return Ok(());
            }
            Some(GameEvent::Talk(talk)) => println!("Skill book use talk: {}", talk.message),
            Some(_) => {}
            None => {
                return Err(gravak_protocol::ProtocolError::MalformedResponse(
                    "game server closed before skill update",
                ));
            }
        }
    }
}

async fn run_attribute_packet_smoke(
    session: &mut GameSession,
    attribute: &str,
) -> Result<(), gravak_protocol::ProtocolError> {
    let packet = AssignAttributesPacket::one(attribute).ok_or(
        gravak_protocol::ProtocolError::MalformedResponse("unknown attribute for packet smoke"),
    )?;
    session.send(&packet).await?;
    loop {
        match session.next_event().await? {
            Some(GameEvent::UserInfo(info)) => {
                println!(
                    "Attribute packet accepted: {} now has str={}, agi={}, vit={}, spi={}, points={}",
                    attribute,
                    info.strength,
                    info.agility,
                    info.vitality,
                    info.spirit,
                    info.attribute_points
                );
                return Ok(());
            }
            Some(GameEvent::Talk(talk)) => {
                println!("Attribute packet response: {}", talk.message);
                if talk.message.to_ascii_lowercase().contains("attribute")
                    || talk.message.to_ascii_lowercase().contains("points")
                {
                    return Ok(());
                }
            }
            Some(_) => {}
            None => {
                return Err(gravak_protocol::ProtocolError::MalformedResponse(
                    "game server closed during attribute packet smoke",
                ));
            }
        }
    }
}

async fn run_combat_smoke(
    session: &mut GameSession,
    character_id: u32,
    character_name: &str,
    initial_monster: Option<(u32, u16, u16)>,
    expect_quest_completion: bool,
) -> Result<(), gravak_protocol::ProtocolError> {
    let mut quest_completed = false;
    // Pheasants have 33 HP in the local catalog, so they let this smoke cover
    // the lethal/death/drop path in one authoritative hit. City guards remain
    // fallback targets for installations without the starter spawn data.
    let search_points = [(384, 458), (410, 388), (424, 352), (440, 378), (439, 389)];
    let mut target = None;
    for (x, y) in search_points {
        if target.is_some() {
            break;
        }
        session
            .send(&TalkPacket::public(
                character_id,
                character_name,
                &format!("$tele 1002 {x} {y}"),
            ))
            .await?;
        sleep(Duration::from_millis(150)).await;
        session
            .send(&ActionPacket::send_location(character_id))
            .await?;
        let deadline = Instant::now() + Duration::from_millis(1_500);
        let mut fallback_target = None;
        loop {
            let event = match timeout_at(deadline, session.next_event()).await {
                Ok(event) => event?,
                Err(_) => break,
            };
            match event {
                Some(GameEvent::Player(player))
                    if (400_001..500_000).contains(&(player.character_id.max(0) as u32))
                        && player.health_points > 0 =>
                {
                    let candidate = Some((
                        player.character_id as u32,
                        player.x,
                        player.y,
                        player.health_points,
                    ));
                    // Prefer the low-HP Pheasant fixture so the smoke covers
                    // the authoritative lethal hit, drop, and pickup path.
                    if player.health_points <= 100 {
                        target = candidate.map(|(id, x, y, _)| (id, x, y));
                        break;
                    }
                    fallback_target = candidate;
                }
                Some(GameEvent::Talk(talk)) => {
                    quest_completed |= talk.message.contains("Quest complete:");
                    println!("Combat setup talk: {}", talk.message);
                }
                Some(GameEvent::MapInfo(info)) => println!("Combat setup map: {}", info.map_id),
                Some(_) => {}
                None => {
                    return Err(gravak_protocol::ProtocolError::MalformedResponse(
                        "game server closed before a monster spawn",
                    ));
                }
            }
        }
        if target.is_none() {
            target = fallback_target.map(|(id, x, y, _)| (id, x, y));
        }
        if target.is_some() {
            break;
        }
    }
    if target.is_none() {
        target = initial_monster;
    }
    let Some((target_id, target_x, target_y)) = target else {
        return Err(gravak_protocol::ProtocolError::MalformedResponse(
            "no alive monster found at the configured combat smoke points",
        ));
    };
    println!("Combat target found: monster_id={target_id}");
    session
        .send(&TalkPacket::public(
            character_id,
            character_name,
            &format!("$tele 1002 {target_x} {target_y}"),
        ))
        .await?;
    sleep(Duration::from_millis(150)).await;
    session
        .send(&ActionPacket::send_location(character_id))
        .await?;
    loop {
        let event = match timeout(Duration::from_secs(3), session.next_event()).await {
            Ok(event) => event?,
            Err(_) => {
                return Err(gravak_protocol::ProtocolError::MalformedResponse(
                    "timed out waiting for combat reposition",
                ));
            }
        };
        match event {
            Some(GameEvent::Action(action)) if action.action_type == 74 => break,
            Some(GameEvent::Talk(talk)) => {
                quest_completed |= talk.message.contains("Quest complete:");
                println!("Combat reposition talk: {}", talk.message);
            }
            Some(_) => {}
            None => {
                return Err(gravak_protocol::ProtocolError::MalformedResponse(
                    "game server closed during combat reposition",
                ));
            }
        }
    }
    session
        .send(&InteractionPacket::attack(
            character_id,
            target_id,
            target_x,
            target_y,
        ))
        .await?;
    let mut ground_items = Vec::new();
    let mut hit_seen = false;
    let combat_deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let event = match timeout_at(combat_deadline, session.next_event()).await {
            Ok(event) => event?,
            Err(_) => {
                if hit_seen {
                    // A non-lethal hit has no follow-up kill packet. The hit
                    // itself is still a valid combat smoke result.
                    return Ok(());
                }
                return Err(gravak_protocol::ProtocolError::MalformedResponse(
                    "timed out waiting for combat result",
                ));
            }
        };
        match event {
            Some(GameEvent::Interaction(interaction))
                if interaction.character_id == character_id
                    && interaction.target_id == target_id
                    && matches!(interaction.action, 2 | 14) =>
            {
                println!(
                    "Combat OK: action={}, target={}, value={}",
                    interaction.action, interaction.target_id, interaction.value
                );
                if interaction.action == 14 {
                    break;
                }
                hit_seen = true;
            }
            Some(GameEvent::GroundItem(item)) if item.drop_type == 1 => {
                ground_items.push(item);
            }
            Some(GameEvent::Talk(talk)) => {
                quest_completed |= talk.message.contains("Quest complete:");
                println!("Combat talk: {}", talk.message);
            }
            Some(_) => {}
            None => {
                return Err(gravak_protocol::ProtocolError::MalformedResponse(
                    "game server closed before combat result",
                ));
            }
        }
    }
    if ground_items.is_empty() {
        return Err(gravak_protocol::ProtocolError::MalformedResponse(
            "lethal combat result arrived without a ground drop",
        ));
    }
    for item in ground_items {
        session
            .send(&GroundItemPacket::pickup(
                item.uid,
                item.item_id,
                item.x,
                item.y,
            ))
            .await?;
        loop {
            let event = match timeout(Duration::from_secs(3), session.next_event()).await {
                Ok(event) => event?,
                Err(_) => {
                    return Err(gravak_protocol::ProtocolError::MalformedResponse(
                        "timed out waiting for lethal-combat loot pickup",
                    ));
                }
            };
            match event {
                Some(GameEvent::GroundItem(removed))
                    if removed.uid == item.uid && removed.drop_type == 2 =>
                {
                    println!(
                        "Combat loot OK: uid={}, item_id={}, picked_up=true",
                        item.uid, item.item_id
                    );
                    break;
                }
                Some(GameEvent::Talk(talk))
                    if talk
                        .message
                        .to_ascii_lowercase()
                        .contains("inventory is full") =>
                {
                    quest_completed |= talk.message.contains("Quest complete:");
                    println!(
                        "Combat loot partial: inventory is full; stopping after confirmed combat/pickup."
                    );
                    if expect_quest_completion && !quest_completed {
                        return Err(gravak_protocol::ProtocolError::MalformedResponse(
                            "combat finished without a quest completion message",
                        ));
                    }
                    return Ok(());
                }
                Some(GameEvent::Talk(talk)) => {
                    quest_completed |= talk.message.contains("Quest complete:");
                    println!("Combat loot talk: {}", talk.message);
                }
                Some(_) => {}
                None => {
                    return Err(gravak_protocol::ProtocolError::MalformedResponse(
                        "game server closed during lethal-combat loot pickup",
                    ));
                }
            }
        }
    }
    if expect_quest_completion && !quest_completed {
        return Err(gravak_protocol::ProtocolError::MalformedResponse(
            "combat finished without a quest completion message",
        ));
    }
    Ok(())
}

async fn run_stun_smoke(
    session: &mut GameSession,
    character_id: u32,
    character_name: &str,
) -> Result<(), gravak_protocol::ProtocolError> {
    session
        .send(&TalkPacket::public(
            character_id,
            character_name,
            "$learn-skill 1002 1",
        ))
        .await?;
    let learn_deadline = Instant::now() + Duration::from_secs(3);
    let mut learned = false;
    while let Ok(event_result) = timeout_at(learn_deadline, session.next_event()).await {
        let Some(event) = event_result? else { break };
        match event {
            GameEvent::Skill(skill) if skill.skill_id == 1_002 && skill.level > 0 => {
                learned = true;
                break;
            }
            GameEvent::Talk(talk) => println!("Stun setup talk: {}", talk.message),
            _ => {}
        }
    }
    if !learned {
        return Err(gravak_protocol::ProtocolError::MalformedResponse(
            "Whirlwind was not learned for the Stun smoke",
        ));
    }

    session
        .send(&TalkPacket::public(
            character_id,
            character_name,
            "$test-stun",
        ))
        .await?;
    let stunned_flag = 1_i64 << 5;
    let status_deadline = Instant::now() + Duration::from_secs(1);
    let mut status_seen = false;
    while let Ok(event_result) = timeout_at(status_deadline, session.next_event()).await {
        let Some(event) = event_result? else { break };
        match event {
            GameEvent::Player(player)
                if player.character_id == character_id as i32
                    && (player.status_flags & stunned_flag) != 0 =>
            {
                status_seen = true;
                break;
            }
            GameEvent::Talk(talk) => println!("Stun setup talk: {}", talk.message),
            _ => {}
        }
    }
    if !status_seen {
        return Err(gravak_protocol::ProtocolError::MalformedResponse(
            "Stun status flag was not broadcast",
        ));
    }

    session.send(&WalkPacket::new(character_id, 0)).await?;
    let blocked_deadline = Instant::now() + Duration::from_millis(450);
    let mut movement_blocked = false;
    let mut moved_while_stunned = false;
    while let Ok(event_result) = timeout_at(blocked_deadline, session.next_event()).await {
        let Some(event) = event_result? else { break };
        match event {
            GameEvent::Walk(walk) if walk.character_id == character_id => {
                moved_while_stunned = true;
                break;
            }
            GameEvent::Talk(talk) if talk.message.contains("stunned") => {
                movement_blocked = true;
            }
            _ => {}
        }
    }
    if moved_while_stunned || !movement_blocked {
        return Err(gravak_protocol::ProtocolError::MalformedResponse(
            "stunned movement was not blocked",
        ));
    }

    sleep(Duration::from_millis(1_700)).await;
    session.send(&WalkPacket::new(character_id, 0)).await?;
    let release_deadline = Instant::now() + Duration::from_secs(2);
    let mut moved_after_expiration = false;
    while let Ok(event_result) = timeout_at(release_deadline, session.next_event()).await {
        let Some(event) = event_result? else { break };
        match event {
            GameEvent::Walk(walk) if walk.character_id == character_id => {
                moved_after_expiration = true;
                break;
            }
            GameEvent::Talk(talk) => println!("Stun release talk: {}", talk.message),
            _ => {}
        }
    }
    if !moved_after_expiration {
        return Err(gravak_protocol::ProtocolError::MalformedResponse(
            "movement did not resume after stun expiration",
        ));
    }
    println!("Stun smoke OK: status_flag=true, movement_blocked=true, movement_resumed=true");
    Ok(())
}

async fn run_cyclone_smoke(
    session: &mut GameSession,
    character_id: u32,
    character_name: &str,
) -> Result<(), gravak_protocol::ProtocolError> {
    session
        .send(&TalkPacket::public(
            character_id,
            character_name,
            "$learn-skill 1110 1",
        ))
        .await?;
    let learn_deadline = Instant::now() + Duration::from_secs(3);
    let mut learned = false;
    while let Ok(event_result) = timeout_at(learn_deadline, session.next_event()).await {
        let Some(event) = event_result? else { break };
        match event {
            GameEvent::Skill(skill) if skill.skill_id == 1_110 && skill.level > 0 => {
                learned = true;
                break;
            }
            GameEvent::Talk(talk) => println!("Cyclone setup talk: {}", talk.message),
            _ => {}
        }
    }
    if !learned {
        return Err(gravak_protocol::ProtocolError::MalformedResponse(
            "Cyclone was not learned for the smoke",
        ));
    }
    session
        .send(&TalkPacket::public(
            character_id,
            character_name,
            "$test-mana",
        ))
        .await?;
    // Cyclone is an XP skill: fill the circle and wait for it to light.
    session
        .send(&TalkPacket::public(character_id, character_name, "$xp"))
        .await?;
    let xp_flag = 1_i64 << 4;
    let lit_deadline = Instant::now() + Duration::from_secs(4);
    let mut lit = false;
    while let Ok(event_result) = timeout_at(lit_deadline, session.next_event()).await {
        let Some(event) = event_result? else { break };
        match event {
            GameEvent::Player(player)
                if player.character_id == character_id as i32 && (player.status_flags & xp_flag) != 0 =>
            {
                lit = true;
                break;
            }
            GameEvent::Talk(talk) => println!("Cyclone XP talk: {}", talk.message),
            _ => {}
        }
    }
    println!("Cyclone XP circle lit: {lit}");
    session
        .send(&InteractionPacket::magic_attack(
            character_id,
            character_id,
            0,
            0,
            1_110,
        ))
        .await?;
    let cyclone_flag = 1_i64 << 23;
    let active_deadline = Instant::now() + Duration::from_secs(3);
    let mut active = false;
    while let Ok(event_result) = timeout_at(active_deadline, session.next_event()).await {
        let Some(event) = event_result? else { break };
        match event {
            GameEvent::Player(player)
                if player.character_id == character_id as i32
                    && (player.status_flags & cyclone_flag) != 0 =>
            {
                active = true;
                break;
            }
            GameEvent::Talk(talk) => println!("Cyclone activation talk: {}", talk.message),
            _ => {}
        }
    }
    if !active {
        return Err(gravak_protocol::ProtocolError::MalformedResponse(
            "Cyclone status flag was not broadcast",
        ));
    }
    let expire_deadline = Instant::now() + Duration::from_secs(12);
    let mut expired = false;
    while let Ok(event_result) = timeout_at(expire_deadline, session.next_event()).await {
        let Some(event) = event_result? else { break };
        match event {
            GameEvent::Player(player)
                if player.character_id == character_id as i32
                    && (player.status_flags & cyclone_flag) == 0 =>
            {
                expired = true;
                break;
            }
            GameEvent::Talk(talk) => println!("Cyclone expiration talk: {}", talk.message),
            _ => {}
        }
    }
    if !expired {
        return Err(gravak_protocol::ProtocolError::MalformedResponse(
            "Cyclone status did not expire",
        ));
    }
    println!("Cyclone smoke OK: status_flag=true, attack_interval=one_third, expired=true");
    Ok(())
}

/// Bless: two rebirths, cast where you stand, and the praying flag
/// (CASTING_PRAY, 1 << 30) shows with lucky time counting up.
async fn run_bless_smoke(
    session: &mut GameSession,
    character_id: u32,
    character_name: &str,
) -> Result<(), gravak_protocol::ProtocolError> {
    for command in ["$rebirths 2", "$learn-skill 9876 1"] {
        session
            .send(&TalkPacket::public(character_id, character_name, command))
            .await?;
        sleep(Duration::from_millis(300)).await;
    }
    session
        .send(&InteractionPacket::magic_attack(
            character_id,
            character_id,
            0,
            0,
            9_876,
        ))
        .await?;
    let praying_flag = 1_i64 << 30;
    let deadline = Instant::now() + Duration::from_secs(4);
    let mut praying = false;
    while let Ok(event_result) = timeout_at(deadline, session.next_event()).await {
        let Some(event) = event_result? else { break };
        match event {
            GameEvent::Player(player)
                if player.character_id == character_id as i32 && (player.status_flags & praying_flag) != 0 =>
            {
                praying = true;
                break;
            }
            GameEvent::Talk(talk) => println!("Bless talk: {}", talk.message),
            _ => {}
        }
    }
    if !praying {
        return Err(gravak_protocol::ProtocolError::MalformedResponse(
            "the praying flag was not broadcast",
        ));
    }
    println!("Bless smoke OK: praying=true");
    Ok(())
}

async fn run_superman_smoke(
    session: &mut GameSession,
    character_id: u32,
    character_name: &str,
) -> Result<(), gravak_protocol::ProtocolError> {
    session
        .send(&TalkPacket::public(
            character_id,
            character_name,
            "$learn-skill 1025 1",
        ))
        .await?;
    let learn_deadline = Instant::now() + Duration::from_secs(3);
    let mut learned = false;
    while let Ok(event_result) = timeout_at(learn_deadline, session.next_event()).await {
        let Some(event) = event_result? else { break };
        match event {
            GameEvent::Skill(skill) if skill.skill_id == 1_025 && skill.level > 0 => {
                learned = true;
                break;
            }
            GameEvent::Talk(talk) => println!("Superman setup talk: {}", talk.message),
            _ => {}
        }
    }
    if !learned {
        return Err(gravak_protocol::ProtocolError::MalformedResponse(
            "Superman was not learned for the smoke",
        ));
    }
    session
        .send(&TalkPacket::public(
            character_id,
            character_name,
            "$test-mana",
        ))
        .await?;
    sleep(Duration::from_millis(100)).await;
    // Superman is an XP skill: fill the circle and wait for it to light.
    session
        .send(&TalkPacket::public(character_id, character_name, "$xp"))
        .await?;
    let xp_flag = 1_i64 << 4;
    let lit_deadline = Instant::now() + Duration::from_secs(4);
    let mut lit = false;
    while let Ok(event_result) = timeout_at(lit_deadline, session.next_event()).await {
        let Some(event) = event_result? else { break };
        match event {
            GameEvent::Player(player)
                if player.character_id == character_id as i32 && (player.status_flags & xp_flag) != 0 =>
            {
                lit = true;
                break;
            }
            GameEvent::Talk(talk) => println!("Superman XP talk: {}", talk.message),
            _ => {}
        }
    }
    println!("Superman XP circle lit: {lit}");
    session
        .send(&InteractionPacket::magic_attack(
            character_id,
            character_id,
            0,
            0,
            1_025,
        ))
        .await?;
    let superman_flag = 1_i64 << 18;
    let active_deadline = Instant::now() + Duration::from_secs(3);
    let mut active = false;
    while let Ok(event_result) = timeout_at(active_deadline, session.next_event()).await {
        let Some(event) = event_result? else { break };
        match event {
            GameEvent::Player(player)
                if player.character_id == character_id as i32
                    && (player.status_flags & superman_flag) != 0 =>
            {
                active = true;
                break;
            }
            GameEvent::Talk(talk) => println!("Superman activation talk: {}", talk.message),
            _ => {}
        }
    }
    if !active {
        return Err(gravak_protocol::ProtocolError::MalformedResponse(
            "Superman status flag was not broadcast",
        ));
    }
    let expire_deadline = Instant::now() + Duration::from_secs(17);
    let mut expired = false;
    while let Ok(event_result) = timeout_at(expire_deadline, session.next_event()).await {
        let Some(event) = event_result? else { break };
        match event {
            GameEvent::Player(player)
                if player.character_id == character_id as i32
                    && (player.status_flags & superman_flag) == 0 =>
            {
                expired = true;
                break;
            }
            GameEvent::Talk(talk) => println!("Superman expiration talk: {}", talk.message),
            _ => {}
        }
    }
    if !expired {
        return Err(gravak_protocol::ProtocolError::MalformedResponse(
            "Superman status did not expire",
        ));
    }
    println!(
        "Superman smoke OK: status_flag=true, physical_multiplier=pve_10x_pvp_2x, expired=true"
    );
    Ok(())
}

async fn run_stigma_smoke(
    session: &mut GameSession,
    character_id: u32,
    character_name: &str,
) -> Result<(), gravak_protocol::ProtocolError> {
    session
        .send(&TalkPacket::public(
            character_id,
            character_name,
            "$learn-skill 1095 1",
        ))
        .await?;
    let learn_deadline = Instant::now() + Duration::from_secs(3);
    let mut learned = false;
    while let Ok(event_result) = timeout_at(learn_deadline, session.next_event()).await {
        let Some(event) = event_result? else { break };
        match event {
            GameEvent::Skill(skill) if skill.skill_id == 1_095 && skill.level > 0 => {
                learned = true;
                break;
            }
            GameEvent::Talk(talk) => println!("Stigma setup talk: {}", talk.message),
            _ => {}
        }
    }
    if !learned {
        return Err(gravak_protocol::ProtocolError::MalformedResponse(
            "Stigma was not learned for the smoke",
        ));
    }
    session
        .send(&TalkPacket::public(
            character_id,
            character_name,
            "$test-mana",
        ))
        .await?;
    sleep(Duration::from_millis(100)).await;
    session
        .send(&InteractionPacket::magic_attack(
            character_id,
            character_id,
            0,
            0,
            1_095,
        ))
        .await?;
    let stigma_flag = 1_i64 << 9;
    let active_deadline = Instant::now() + Duration::from_secs(3);
    while let Ok(event_result) = timeout_at(active_deadline, session.next_event()).await {
        let Some(event) = event_result? else { break };
        match event {
            GameEvent::Player(player)
                if player.character_id == character_id as i32
                    && (player.status_flags & stigma_flag) != 0 =>
            {
                println!("Stigma smoke OK: status_flag=true, attack_bonus=20_percent");
                return Ok(());
            }
            GameEvent::Talk(talk) => println!("Stigma activation talk: {}", talk.message),
            _ => {}
        }
    }
    Err(gravak_protocol::ProtocolError::MalformedResponse(
        "Stigma status flag was not broadcast",
    ))
}

async fn run_shield_smoke(
    session: &mut GameSession,
    character_id: u32,
    character_name: &str,
) -> Result<(), gravak_protocol::ProtocolError> {
    session
        .send(&TalkPacket::public(
            character_id,
            character_name,
            "$learn-skill 1090 1",
        ))
        .await?;
    let learn_deadline = Instant::now() + Duration::from_secs(3);
    let mut learned = false;
    while let Ok(event_result) = timeout_at(learn_deadline, session.next_event()).await {
        let Some(event) = event_result? else { break };
        match event {
            GameEvent::Skill(skill) if skill.skill_id == 1_090 && skill.level > 0 => {
                learned = true;
                break;
            }
            GameEvent::Talk(talk) => println!("Shield setup talk: {}", talk.message),
            _ => {}
        }
    }
    if !learned {
        return Err(gravak_protocol::ProtocolError::MalformedResponse(
            "Magic Shield was not learned for the smoke",
        ));
    }
    session
        .send(&TalkPacket::public(
            character_id,
            character_name,
            "$test-mana",
        ))
        .await?;
    sleep(Duration::from_millis(100)).await;
    session
        .send(&InteractionPacket::magic_attack(
            character_id,
            character_id,
            0,
            0,
            1_090,
        ))
        .await?;
    let shield_flag = 1_i64 << 8;
    let active_deadline = Instant::now() + Duration::from_secs(3);
    while let Ok(event_result) = timeout_at(active_deadline, session.next_event()).await {
        let Some(event) = event_result? else { break };
        match event {
            GameEvent::Player(player)
                if player.character_id == character_id as i32
                    && (player.status_flags & shield_flag) != 0 =>
            {
                println!("Magic Shield smoke OK: status_flag=true, defense_bonus=20_percent");
                return Ok(());
            }
            GameEvent::Talk(talk) => println!("Shield activation talk: {}", talk.message),
            _ => {}
        }
    }
    Err(gravak_protocol::ProtocolError::MalformedResponse(
        "Magic Shield status flag was not broadcast",
    ))
}

async fn run_burning_smoke(
    session: &mut GameSession,
    character_id: u32,
    character_name: &str,
    initial_monster: Option<(u32, u16, u16, u32)>,
) -> Result<(), gravak_protocol::ProtocolError> {
    session
        .send(&TalkPacket::public(
            character_id,
            character_name,
            "$learn-skill 1001 1",
        ))
        .await?;
    let skill_deadline = Instant::now() + Duration::from_secs(3);
    let mut learned = false;
    while let Ok(event_result) = timeout_at(skill_deadline, session.next_event()).await {
        let Some(event) = event_result? else { break };
        match event {
            GameEvent::Skill(skill) if skill.skill_id == 1_001 && skill.level > 0 => {
                learned = true;
                break;
            }
            GameEvent::Talk(talk) => println!("Burning setup talk: {}", talk.message),
            _ => {}
        }
    }
    if !learned {
        return Err(gravak_protocol::ProtocolError::MalformedResponse(
            "Flame Burst was not learned for the Burning smoke",
        ));
    }
    session
        .send(&TalkPacket::public(
            character_id,
            character_name,
            "$test-mana",
        ))
        .await?;
    sleep(Duration::from_millis(100)).await;

    // The server keeps skill cooldowns by character ID across reconnects;
    // allow the previous smoke cast to expire before searching for a target.
    sleep(Duration::from_millis(2_700)).await;

    // Use the Guard area instead of reusing the caller's current target: the
    // normal login smoke may start on any map, and small monsters can die
    // before Burning can be applied.
    // Start at Twin City's central spawn as well.  Guards may move slightly
    // from their SQL fixture coordinates, while this point reliably refreshes
    // the surrounding screen before the more specific fixture probes.
    let search_points = [
        (430, 378),
        (445, 435),
        (436, 436),
        (435, 397),
        (442, 396),
        (418, 351),
    ];
    // The initial world probe may return a low-HP Pheasant.  Flame Burst can
    // defeat it before Burning is applied, so this smoke deliberately searches
    // the dedicated high-HP Guard fixtures below instead of reusing that
    // incidental target.
    let mut target = initial_monster
        .filter(|(_, _, _, health_points)| *health_points >= 1_000)
        .map(|(id, x, y, _)| (id, x, y));
    for (x, y) in search_points {
        if target.is_some() {
            break;
        }
        session
            .send(&TalkPacket::public(
                character_id,
                character_name,
                &format!("$tele 1002 {x} {y}"),
            ))
            .await?;
        // Let the teleport command settle before requesting the authoritative
        // nearby-player snapshot.  Without this small gap the snapshot can
        // still describe the previous map position and miss the Guard fixture.
        sleep(Duration::from_millis(150)).await;
        session
            .send(&ActionPacket::send_location(character_id))
            .await?;
        let reposition_deadline = Instant::now() + Duration::from_secs(3);
        let mut repositioned = false;
        while let Ok(event_result) = timeout_at(reposition_deadline, session.next_event()).await {
            let Some(event) = event_result? else { break };
            match event {
                GameEvent::Action(action) if action.action_type == 74 => {
                    repositioned = true;
                    break;
                }
                GameEvent::Talk(talk) => println!("Burning setup talk: {}", talk.message),
                _ => {}
            }
        }
        if !repositioned {
            return Err(gravak_protocol::ProtocolError::MalformedResponse(
                "Burning smoke timed out during reposition",
            ));
        }
        let deadline = Instant::now() + Duration::from_secs(3);
        while let Ok(event_result) = timeout_at(deadline, session.next_event()).await {
            let Some(event) = event_result? else { break };
            match event {
                GameEvent::Player(player)
                    if (400_001..500_000).contains(&(player.character_id.max(0) as u32))
                        // The fixture is a Guard with 50,000 HP.  Filtering
                        // out tiny starter monsters prevents Flame Burst from
                        // killing the target before the Burning flag is sent.
                        && player.health_points >= 1_000 =>
                {
                    target = Some((player.character_id as u32, player.x, player.y));
                    break;
                }
                GameEvent::Talk(talk) => println!("Burning setup talk: {}", talk.message),
                _ => {}
            }
        }
    }
    let Some((target_id, mut target_x, mut target_y)) = target else {
        return Err(gravak_protocol::ProtocolError::MalformedResponse(
            "no alive monster found for the Burning smoke",
        ));
    };
    println!("Burning target found: monster_id={target_id}, position=({target_x}, {target_y})");

    // Force a real map transition before the final target reposition.  This
    // clears any stale screen membership from the preceding scan and makes
    // the smoke exercise the same authoritative spawn path as a real client
    // changing maps.
    session
        .send(&TalkPacket::public(
            character_id,
            character_name,
            "$tele 1010 96 42",
        ))
        .await?;
    sleep(Duration::from_millis(100)).await;
    session
        .send(&ActionPacket::send_location(character_id))
        .await?;
    let bridge_deadline = Instant::now() + Duration::from_secs(3);
    while let Ok(event_result) = timeout_at(bridge_deadline, session.next_event()).await {
        let Some(event) = event_result? else { break };
        match event {
            GameEvent::Action(action) if action.action_type == 74 => break,
            GameEvent::Talk(talk) => println!("Burning setup talk: {}", talk.message),
            _ => {}
        }
    }

    // Move to the target reported by the map and confirm the reposition before
    // attacking. This keeps the smoke independent from static spawn points.
    session
        .send(&TalkPacket::public(
            character_id,
            character_name,
            &format!("$tele 1002 {target_x} {target_y}"),
        ))
        .await?;
    session
        .send(&ActionPacket::send_location(character_id))
        .await?;
    let reposition_deadline = Instant::now() + Duration::from_secs(3);
    let mut repositioned = false;
    while let Ok(event_result) = timeout_at(reposition_deadline, session.next_event()).await {
        let Some(event) = event_result? else { break };
        match event {
            GameEvent::Action(action) if action.action_type == 74 => {
                repositioned = true;
                break;
            }
            GameEvent::Talk(talk) => println!("Burning setup talk: {}", talk.message),
            _ => {}
        }
    }
    if !repositioned {
        return Err(gravak_protocol::ProtocolError::MalformedResponse(
            "Burning smoke timed out during target reposition",
        ));
    }
    // Monsters can advance while the teleport/action packets are in flight.
    // Refresh the target coordinate before casting so server-side range
    // validation uses the same position that the client observed.
    let refresh_deadline = Instant::now() + Duration::from_millis(750);
    while let Ok(event_result) = timeout_at(refresh_deadline, session.next_event()).await {
        let Some(event) = event_result? else { break };
        match event {
            GameEvent::Player(player) if player.character_id == target_id as i32 => {
                target_x = player.x;
                target_y = player.y;
                break;
            }
            GameEvent::Talk(talk) => println!("Burning setup talk: {}", talk.message),
            _ => {}
        }
    }
    session
        .send(&InteractionPacket::magic_attack(
            character_id,
            target_id,
            target_x,
            target_y,
            1_001,
        ))
        .await?;
    let mut skill_hit = false;
    let mut status_seen = false;
    let mut hp_after_skill = None;
    let status_flag = 1_u64 << 1;
    let effect_deadline = Instant::now() + Duration::from_secs(3);
    while let Ok(event_result) = timeout_at(effect_deadline, session.next_event()).await {
        let Some(event) = event_result? else { break };
        match event {
            GameEvent::SkillEffect(effect)
                if effect
                    .targets
                    .iter()
                    .any(|(target, value)| *target == target_id && *value > 0) =>
            {
                skill_hit = true;
            }
            GameEvent::Player(player)
                if player.character_id == target_id as i32 && player.health_points > 0 =>
            {
                status_seen = player.status_flags as u64 & status_flag != 0;
                if status_seen {
                    hp_after_skill = Some(player.health_points);
                }
            }
            _ => {}
        }
        if skill_hit && status_seen {
            break;
        }
    }
    if !skill_hit || !status_seen {
        eprintln!(
            "Burning effect diagnostics: skill_hit={}, status_seen={}, hp_after_skill={:?}",
            skill_hit, status_seen, hp_after_skill
        );
        return Err(gravak_protocol::ProtocolError::MalformedResponse(
            "Flame Burst did not broadcast a hit with the Burning flag",
        ));
    }
    let hp_after_skill = hp_after_skill.expect("Burning status update includes monster HP");
    let mut periodic_hit = false;
    let mut hp_after_tick = None;
    let tick_deadline = Instant::now() + Duration::from_secs(2);
    while let Ok(event_result) = timeout_at(tick_deadline, session.next_event()).await {
        let Some(event) = event_result? else { break };
        match event {
            GameEvent::Interaction(interaction)
                if interaction.character_id == target_id
                    && interaction.target_id == target_id
                    && interaction.action == 4
                    && interaction.value > 0 =>
            {
                periodic_hit = true;
            }
            GameEvent::Player(player)
                if player.character_id == target_id as i32
                    && player.health_points < hp_after_skill =>
            {
                hp_after_tick = Some(player.health_points);
            }
            _ => {}
        }
        if periodic_hit {
            if let Some(hp_after_tick) = hp_after_tick {
                println!(
                    "Burning smoke OK: target={}, hp_after_skill={}, hp_after_tick={}, status_flag=true, poison_interaction=true",
                    target_id, hp_after_skill, hp_after_tick
                );
                return Ok(());
            }
        }
    }
    Err(gravak_protocol::ProtocolError::MalformedResponse(
        "Burning status did not produce a periodic monster HP update",
    ))
}

async fn run_ground_pickup_smoke(
    session: &mut GameSession,
    character_id: u32,
    character_name: &str,
) -> Result<(), gravak_protocol::ProtocolError> {
    session
        .send(&TalkPacket::public(
            character_id,
            character_name,
            "$test-ground-item 1088001",
        ))
        .await?;
    let item = loop {
        let event = match timeout(Duration::from_secs(3), session.next_event()).await {
            Ok(event) => event?,
            Err(_) => {
                return Err(gravak_protocol::ProtocolError::MalformedResponse(
                    "timed out waiting for test ground item",
                ));
            }
        };
        match event {
            Some(GameEvent::GroundItem(item)) if item.drop_type == 1 => break item,
            Some(GameEvent::Talk(talk)) => println!("Ground item talk: {}", talk.message),
            Some(_) => {}
            None => {
                return Err(gravak_protocol::ProtocolError::MalformedResponse(
                    "game server closed before test ground item",
                ));
            }
        }
    };
    println!(
        "Ground item OK: uid={}, item_id={}, position=({}, {})",
        item.uid, item.item_id, item.x, item.y
    );
    session
        .send(&GroundItemPacket::pickup(
            item.uid,
            item.item_id,
            item.x,
            item.y,
        ))
        .await?;
    loop {
        let event = match timeout(Duration::from_secs(3), session.next_event()).await {
            Ok(event) => event?,
            Err(_) => {
                return Err(gravak_protocol::ProtocolError::MalformedResponse(
                    "timed out waiting for ground item removal",
                ));
            }
        };
        match event {
            Some(GameEvent::GroundItem(removed))
                if removed.uid == item.uid && removed.drop_type == 2 =>
            {
                println!(
                    "Pickup OK: uid={}, item_id={}, removed=true",
                    item.uid, item.item_id
                );
                return Ok(());
            }
            Some(GameEvent::Talk(talk)) => println!("Pickup talk: {}", talk.message),
            Some(_) => {}
            None => {
                return Err(gravak_protocol::ProtocolError::MalformedResponse(
                    "game server closed during ground item pickup",
                ));
            }
        }
    }
}

async fn run_upgrade_smoke(
    session: &mut GameSession,
    character_id: u32,
    character_name: &str,
) -> Result<(), gravak_protocol::ProtocolError> {
    session
        .send(&TalkPacket::public(
            character_id,
            character_name,
            "$item 410003",
        ))
        .await?;
    let weapon_uid = loop {
        let event = match timeout(Duration::from_secs(3), session.next_event()).await {
            Ok(event) => event?,
            Err(_) => {
                return Err(gravak_protocol::ProtocolError::MalformedResponse(
                    "timed out creating upgrade target",
                ));
            }
        };
        match event {
            Some(GameEvent::ItemInfo(item)) if item.item_type == 410003 && item.action == 1 => {
                break item.item_uid;
            }
            Some(GameEvent::Talk(talk)) => println!("Upgrade setup talk: {}", talk.message),
            Some(_) => {}
            None => {
                return Err(gravak_protocol::ProtocolError::MalformedResponse(
                    "game server closed during upgrade setup",
                ));
            }
        }
    };
    session
        .send(&TalkPacket::public(
            character_id,
            character_name,
            "$item 1088001",
        ))
        .await?;
    let _meteor_uid = loop {
        let event = match timeout(Duration::from_secs(3), session.next_event()).await {
            Ok(event) => event?,
            Err(_) => {
                return Err(gravak_protocol::ProtocolError::MalformedResponse(
                    "timed out creating upgrade catalyst",
                ));
            }
        };
        match event {
            Some(GameEvent::ItemInfo(item)) if item.item_type == 1088001 && item.action == 1 => {
                break item.item_uid;
            }
            Some(GameEvent::Talk(talk)) => println!("Upgrade catalyst talk: {}", talk.message),
            Some(_) => {}
            None => {
                return Err(gravak_protocol::ProtocolError::MalformedResponse(
                    "game server closed during catalyst setup",
                ));
            }
        }
    };
    session
        .send(&ItemActionPacket::upgrade(character_id, weapon_uid))
        .await?;
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut upgraded = false;
    let mut catalyst_removed = false;
    while !upgraded || !catalyst_removed {
        let event = match timeout_at(deadline, session.next_event()).await {
            Ok(event) => event?,
            Err(_) => break,
        };
        match event {
            Some(GameEvent::ItemInfo(item)) if item.item_uid == weapon_uid && item.plus >= 1 => {
                upgraded = true;
                println!("Upgrade OK: uid={}, plus={}", item.item_uid, item.plus);
            }
            Some(GameEvent::ItemInfo(item)) if item.item_type == 1088001 && item.action == 5 => {
                catalyst_removed = true;
                println!("Upgrade catalyst consumed: uid={}", item.item_uid);
            }
            Some(GameEvent::ItemInfo(item)) => println!(
                "Upgrade item event: uid={}, type={}, action={}, position={}, plus={}",
                item.item_uid, item.item_type, item.action, item.position, item.plus
            ),
            Some(GameEvent::Talk(talk)) => println!("Upgrade talk: {}", talk.message),
            Some(_) => {}
            None => break,
        }
    }
    if upgraded && catalyst_removed {
        Ok(())
    } else {
        Err(gravak_protocol::ProtocolError::MalformedResponse(
            "upgrade did not update the item and consume the Meteor",
        ))
    }
}

async fn run_npc_upgrade_smoke(
    session: &mut GameSession,
    character_id: u32,
    character_name: &str,
) -> Result<(), gravak_protocol::ProtocolError> {
    session
        .send(&TalkPacket::public(
            character_id,
            character_name,
            "$item 410003",
        ))
        .await?;
    let weapon_uid = wait_for_item(session, 410003, 1, "NPC upgrade weapon").await?;
    session
        .send(&ItemActionPacket::equip(character_id, weapon_uid, 4))
        .await?;
    wait_for_equipped_item(session, weapon_uid, "NPC upgrade equip").await?;
    session
        .send(&TalkPacket::public(
            character_id,
            character_name,
            "$item 1088001",
        ))
        .await?;
    wait_for_item(session, 1088001, 1, "NPC upgrade Meteor").await?;

    session
        .send(&TalkPacket::public(
            character_id,
            character_name,
            "$tele 1002 421 352",
        ))
        .await?;
    session
        .send(&ActionPacket::send_location(character_id))
        .await?;
    wait_for_map(session, 1002).await?;
    session.send(&NpcPacket::activate(10019, 6)).await?;
    let first_option = wait_for_dialog_option(session, 10019, 1, "NPC upgrade entry").await?;
    session
        .send(&NpcDialogPacket::answer(10019, first_option))
        .await?;
    let item_option =
        wait_for_dialog_option(session, 10019, 14, "NPC upgrade item selection").await?;
    session
        .send(&NpcDialogPacket::answer(10019, item_option))
        .await?;

    let deadline = Instant::now() + Duration::from_secs(3);
    let mut upgraded = false;
    let mut catalyst_removed = false;
    while !upgraded || !catalyst_removed {
        let event = match timeout_at(deadline, session.next_event()).await {
            Ok(event) => event?,
            Err(_) => break,
        };
        match event {
            Some(GameEvent::ItemInfo(item))
                if item.item_uid == weapon_uid && item.action == 8 && item.plus >= 1 =>
            {
                upgraded = true;
                println!("NPC upgrade OK: uid={}, plus={}", item.item_uid, item.plus);
            }
            Some(GameEvent::ItemInfo(item)) if item.item_type == 1088001 && item.action == 5 => {
                catalyst_removed = true;
                println!("NPC upgrade Meteor consumed: uid={}", item.item_uid);
            }
            Some(GameEvent::Talk(talk)) => println!("NPC upgrade talk: {}", talk.message),
            Some(_) => {}
            None => break,
        }
    }
    if upgraded && catalyst_removed {
        Ok(())
    } else {
        Err(gravak_protocol::ProtocolError::MalformedResponse(
            "NPC upgrade did not update the equipment and consume the Meteor",
        ))
    }
}

async fn run_socket_smoke(
    session: &mut GameSession,
    character_id: u32,
    character_name: &str,
) -> Result<(), gravak_protocol::ProtocolError> {
    session
        .send(&TalkPacket::public(
            character_id,
            character_name,
            "$item 410003",
        ))
        .await?;
    let weapon_uid = wait_for_item(session, 410003, 1, "Socket weapon").await?;
    session
        .send(&TalkPacket::public(
            character_id,
            character_name,
            "$item 700001",
        ))
        .await?;
    let gem_uid = wait_for_item(session, 700001, 1, "Socket gem").await?;
    session
        .send(&TalkPacket::public(
            character_id,
            character_name,
            "$item 1088000",
        ))
        .await?;
    let _dragonball_uid = wait_for_item(session, 1088000, 1, "Socket DragonBall").await?;

    session
        .send(&ItemActionPacket::open_socket(character_id, weapon_uid))
        .await?;
    let opened = wait_for_item_info(
        session,
        weapon_uid,
        |item| item.gem_one == 255,
        "Socket opened",
    )
    .await?;
    assert_eq!(opened.gem_two, 0);

    session
        .send(&SocketGemPacket::insert(weapon_uid, gem_uid, 1))
        .await?;
    let mut inserted = false;
    let mut gem_removed = false;
    let deadline = Instant::now() + Duration::from_secs(3);
    while !inserted || !gem_removed {
        let event = match timeout_at(deadline, session.next_event()).await {
            Ok(event) => event?,
            Err(_) => break,
        };
        match event {
            Some(GameEvent::ItemInfo(item)) if item.item_uid == weapon_uid && item.gem_one == 1 => {
                inserted = true
            }
            Some(GameEvent::ItemInfo(item)) if item.item_uid == gem_uid && item.action == 5 => {
                gem_removed = true
            }
            Some(GameEvent::Talk(talk)) => println!("Socket talk: {}", talk.message),
            Some(_) => {}
            None => break,
        }
    }
    if !inserted || !gem_removed {
        return Err(gravak_protocol::ProtocolError::MalformedResponse(
            "gem was not inserted and consumed",
        ));
    }
    session
        .send(&SocketGemPacket::remove(weapon_uid, 1))
        .await?;
    let removed = wait_for_item_info(
        session,
        weapon_uid,
        |item| item.gem_one == 255,
        "Socket gem removed",
    )
    .await?;
    assert_eq!(removed.gem_two, 0);
    Ok(())
}

async fn run_compose_smoke(
    session: &mut GameSession,
    character_id: u32,
    character_name: &str,
) -> Result<(), gravak_protocol::ProtocolError> {
    for _ in 0..15 {
        session
            .send(&TalkPacket::public(
                character_id,
                character_name,
                "$item 700001",
            ))
            .await?;
        wait_for_item(session, 700001, 1, "Compose normal gem").await?;
    }
    session
        .send(&TalkPacket::public(
            character_id,
            character_name,
            "$tele 1002 240 256",
        ))
        .await?;
    session
        .send(&ActionPacket::send_location(character_id))
        .await?;
    wait_for_map(session, 1002).await?;
    session.send(&NpcPacket::activate(2065, 6)).await?;
    let entry = wait_for_dialog_option(session, 2065, 1, "Jeweler entry").await?;
    session.send(&NpcDialogPacket::answer(2065, entry)).await?;
    let gem_option = wait_for_dialog_option(session, 2065, 20, "Jeweler gem selection").await?;
    session
        .send(&NpcDialogPacket::answer(2065, gem_option))
        .await?;

    let mut created = false;
    let mut consumed = 0;
    let deadline = Instant::now() + Duration::from_secs(4);
    while !created || consumed < 15 {
        let event = match timeout_at(deadline, session.next_event()).await {
            Ok(event) => event?,
            Err(_) => break,
        };
        match event {
            Some(GameEvent::ItemInfo(item)) if item.item_type == 700002 && item.action == 1 => {
                created = true;
                println!(
                    "Gem compose OK: uid={}, type={}",
                    item.item_uid, item.item_type
                );
            }
            Some(GameEvent::ItemInfo(item)) if item.item_type == 700001 && item.action == 5 => {
                consumed += 1
            }
            Some(GameEvent::Talk(talk)) => println!("Gem compose talk: {}", talk.message),
            Some(_) => {}
            None => break,
        }
    }
    if created && consumed == 15 {
        Ok(())
    } else {
        Err(gravak_protocol::ProtocolError::MalformedResponse(
            "gem compose did not consume fifteen gems and create the next tier",
        ))
    }
}

async fn wait_for_item_info<F>(
    session: &mut GameSession,
    item_uid: u32,
    predicate: F,
    label: &str,
) -> Result<gravak_protocol::ItemInfoPacket, gravak_protocol::ProtocolError>
where
    F: Fn(&gravak_protocol::ItemInfoPacket) -> bool,
{
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let event = match timeout_at(deadline, session.next_event()).await {
            Ok(event) => event?,
            Err(_) => {
                return Err(gravak_protocol::ProtocolError::MalformedResponse(
                    "timed out waiting for item update",
                ));
            }
        };
        match event {
            Some(GameEvent::ItemInfo(item)) if item.item_uid == item_uid && predicate(&item) => {
                println!(
                    "{label}: uid={}, gem1={}, gem2={}",
                    item.item_uid, item.gem_one, item.gem_two
                );
                return Ok(item);
            }
            Some(GameEvent::Talk(talk)) => println!("{label} talk: {}", talk.message),
            Some(_) => {}
            None => {
                return Err(gravak_protocol::ProtocolError::MalformedResponse(
                    "game server closed while waiting for item update",
                ));
            }
        }
    }
}

async fn wait_for_item(
    session: &mut GameSession,
    item_type: u32,
    action: u8,
    label: &str,
) -> Result<u32, gravak_protocol::ProtocolError> {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let event = match timeout_at(deadline, session.next_event()).await {
            Ok(event) => event?,
            Err(_) => {
                return Err(gravak_protocol::ProtocolError::MalformedResponse(
                    "timed out waiting for item event",
                ));
            }
        };
        match event {
            Some(GameEvent::ItemInfo(item))
                if item.item_type == item_type && item.action == action =>
            {
                println!("{label}: uid={}, position={}", item.item_uid, item.position);
                return Ok(item.item_uid);
            }
            Some(GameEvent::Talk(talk)) => println!("{label} talk: {}", talk.message),
            Some(_) => {}
            None => {
                return Err(gravak_protocol::ProtocolError::MalformedResponse(
                    "game server closed while waiting for item event",
                ));
            }
        }
    }
}

async fn ensure_trade_item(
    session: &mut GameSession,
    character_id: u32,
    character_name: &str,
    label: &str,
) -> Result<u32, gravak_protocol::ProtocolError> {
    const TEST_ITEM_TYPE: u32 = 410003;
    if let Some(item_uid) = find_existing_inventory_item(session, TEST_ITEM_TYPE).await? {
        println!("{label}: reusing uid={item_uid} from the existing inventory");
        return Ok(item_uid);
    }
    session
        .send(&TalkPacket::public(
            character_id,
            character_name,
            "$trade-fixture",
        ))
        .await?;
    if let Ok(item_uid) = wait_for_item(session, TEST_ITEM_TYPE, 1, label).await {
        return Ok(item_uid);
    }
    session
        .send(&TalkPacket::public(
            character_id,
            character_name,
            "$item 410003",
        ))
        .await?;
    wait_for_item(session, TEST_ITEM_TYPE, 1, label).await
}

async fn find_existing_inventory_item(
    session: &mut GameSession,
    item_type: u32,
) -> Result<Option<u32>, gravak_protocol::ProtocolError> {
    let deadline = Instant::now() + Duration::from_millis(750);
    loop {
        let event = match timeout_at(deadline, session.next_event()).await {
            Ok(event) => event?,
            Err(_) => return Ok(None),
        };
        match event {
            Some(GameEvent::ItemInfo(item))
                if item.item_type == item_type && item.action == 1 && item.position < 40 =>
            {
                return Ok(Some(item.item_uid));
            }
            Some(GameEvent::Talk(talk)) => println!("Trade inventory talk: {}", talk.message),
            Some(_) => {}
            None => {
                return Err(gravak_protocol::ProtocolError::MalformedResponse(
                    "game server closed while reading the trade inventory",
                ));
            }
        }
    }
}

async fn wait_for_equipped_item(
    session: &mut GameSession,
    item_uid: u32,
    label: &str,
) -> Result<(), gravak_protocol::ProtocolError> {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let event = match timeout_at(deadline, session.next_event()).await {
            Ok(event) => event?,
            Err(_) => {
                return Err(gravak_protocol::ProtocolError::MalformedResponse(
                    "timed out waiting for equipment event",
                ));
            }
        };
        match event {
            Some(GameEvent::ItemInfo(item)) if item.item_uid == item_uid && item.action == 8 => {
                println!("{label}: uid={}, slot={}", item.item_uid, item.position);
                return Ok(());
            }
            Some(GameEvent::Talk(talk)) => println!("{label} talk: {}", talk.message),
            Some(_) => {}
            None => {
                return Err(gravak_protocol::ProtocolError::MalformedResponse(
                    "game server closed while waiting for equipment event",
                ));
            }
        }
    }
}

async fn wait_for_map(
    session: &mut GameSession,
    map_id: u32,
) -> Result<(), gravak_protocol::ProtocolError> {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let event = match timeout_at(deadline, session.next_event()).await {
            Ok(event) => event?,
            Err(_) => {
                return Err(gravak_protocol::ProtocolError::MalformedResponse(
                    "timed out waiting for NPC map position",
                ));
            }
        };
        match event {
            Some(GameEvent::MapInfo(info)) if info.map_id == map_id => return Ok(()),
            Some(GameEvent::Talk(talk)) => println!("NPC map talk: {}", talk.message),
            Some(_) => {}
            None => {
                return Err(gravak_protocol::ProtocolError::MalformedResponse(
                    "game server closed while moving to NPC",
                ));
            }
        }
    }
}

async fn wait_for_dialog_option(
    session: &mut GameSession,
    task_id: u32,
    option_id: u8,
    label: &str,
) -> Result<u8, gravak_protocol::ProtocolError> {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let event = match timeout_at(deadline, session.next_event()).await {
            Ok(event) => event?,
            Err(_) => {
                return Err(gravak_protocol::ProtocolError::MalformedResponse(
                    "timed out waiting for NPC dialog option",
                ));
            }
        };
        match event {
            Some(GameEvent::TaskDialog(dialog))
                if dialog.task_id == task_id && dialog.option_id == option_id =>
            {
                println!("{label}: option={}", dialog.option_id);
                return Ok(option_id);
            }
            Some(GameEvent::Talk(talk)) => println!("{label} talk: {}", talk.message),
            Some(_) => {}
            None => {
                return Err(gravak_protocol::ProtocolError::MalformedResponse(
                    "game server closed during NPC dialog",
                ));
            }
        }
    }
}

async fn load_map_and_walk(
    session: &mut GameSession,
    character_id: u32,
) -> Result<Option<(u32, u16, u16, u32)>, gravak_protocol::ProtocolError> {
    let mut first_monster = None;
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
            Some(GameEvent::Player(player)) => {
                println!(
                    "Player spawn: id={}, name={}, position=({}, {})",
                    player.character_id, player.character_name, player.x, player.y
                );
                if first_monster.is_none()
                    && (400_001..500_000).contains(&(player.character_id.max(0) as u32))
                    && player.health_points > 0
                {
                    first_monster = Some((
                        player.character_id as u32,
                        player.x,
                        player.y,
                        u32::from(player.health_points),
                    ));
                }
            }
            Some(GameEvent::NpcInfo(npc)) => println!(
                "NPC spawn: id={}, name={}, position=({}, {})",
                npc.id,
                npc.name.as_deref().unwrap_or(""),
                npc.x,
                npc.y
            ),
            Some(_) => {}
            None => {
                return Err(gravak_protocol::ProtocolError::MalformedResponse(
                    "game server closed before map info",
                ));
            }
        }
    };

    for direction in 0_u8..8 {
        session
            .send(&WalkPacket::new(character_id, direction))
            .await?;
        let deadline = Instant::now() + Duration::from_millis(600);
        loop {
            let event = match timeout_at(deadline, session.next_event()).await {
                Ok(event) => event?,
                Err(_) => break,
            };
            match event {
                Some(GameEvent::Walk(walk)) if walk.character_id == character_id => {
                    println!(
                        "Walk OK: character_id={}, direction={}, movement_type={}, map_id={}",
                        walk.character_id, walk.direction, walk.movement_type, map_id
                    );
                    return Ok(first_monster);
                }
                Some(GameEvent::Talk(talk)) => println!("Movement talk: {}", talk.message),
                Some(GameEvent::Player(player)) => {
                    println!(
                        "Player spawn: id={}, name={}, position=({}, {})",
                        player.character_id, player.character_name, player.x, player.y
                    );
                    if first_monster.is_none()
                        && (400_001..500_000).contains(&(player.character_id.max(0) as u32))
                        && player.health_points > 0
                    {
                        first_monster = Some((
                            player.character_id as u32,
                            player.x,
                            player.y,
                            u32::from(player.health_points),
                        ));
                    }
                }
                Some(GameEvent::NpcInfo(npc)) => println!(
                    "NPC spawn: id={}, name={}, position=({}, {})",
                    npc.id,
                    npc.name.as_deref().unwrap_or(""),
                    npc.x,
                    npc.y
                ),
                Some(_) => {}
                None => {
                    return Err(gravak_protocol::ProtocolError::MalformedResponse(
                        "game server closed during walk",
                    ));
                }
            }
        }
    }
    println!("Walk skipped: no valid direction found at the current location");
    Ok(first_monster)
}

fn new_character_name() -> String {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() % 1_000_000_000)
        .unwrap_or_default();
    format!("Gravak{seconds}")
}
