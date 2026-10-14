//! The pre-game session: account registration, login, character creation
//! and entering the world, driven by the login screens.
//!
//! Everything runs on one network thread (in the browser, on the page's own
//! loop) so the game connection opened at login is the same one
//! [`crate::network::play`] keeps using in game.

use crate::network::{NetworkEvent, NetworkHandle};
use crate::worlds::World;
use gravak_protocol::{
    AuthResponse, ForwardedGameServer, GameEvent, LoginRejection, RegisterAccountResult,
    RegisterCharacter, UserInfo,
};
use gravak_protocol::{GameSession, authenticate, register_account};
use std::sync::mpsc::{self, Receiver, Sender};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

const REGISTER_CHANNEL: u16 = 2100;
const LOGIN_CHANNEL: u16 = 2101;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoginCommand {
    Register {
        world: World,
        username: String,
        password: String,
    },
    Login {
        world: World,
        username: String,
        password: String,
    },
    CreateCharacter {
        name: String,
        mesh: u16,
        class: u16,
        /// The starter coat's dye (3..=9).
        coat_dye: u8,
        /// The face and hair picked (0 lets the server draw them).
        avatar: u16,
        hair: u16,
    },
    /// Enter the world with the account's character.
    Enter,
    /// Drop the game connection and go back to the login screen.
    Logout,
}

/// The account's character, shown on the selection screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CharacterSummary {
    pub name: String,
    pub level: u8,
    pub class: u8,
    pub mesh: u32,
}

impl CharacterSummary {
    fn from_user(user: &UserInfo) -> Self {
        Self {
            name: user.character_name.clone(),
            level: user.level,
            class: user.current_class,
            mesh: user.mesh,
        }
    }
}

pub enum LoginEvent {
    /// Progress line for the status bar.
    Status(String),
    Registered(String),
    RegisterFailed(String),
    LoginFailed(String),
    /// The account has no character yet.
    NeedsCharacter,
    CharacterRejected(String),
    Character(CharacterSummary),
    /// The world is loading; the handle receives the game events.
    Entered(NetworkHandle),
    Disconnected(String),
}

pub struct LoginClient {
    commands: UnboundedSender<LoginCommand>,
    events: Receiver<LoginEvent>,
}

impl LoginClient {
    pub fn start() -> Self {
        let (events_tx, events) = mpsc::channel();
        let (commands, commands_rx) = tokio::sync::mpsc::unbounded_channel();
        // The browser has no threads: the session is a task on its loop.
        #[cfg(target_arch = "wasm32")]
        wasm_bindgen_futures::spawn_local(serve(events_tx, commands_rx));
        #[cfg(not(target_arch = "wasm32"))]
        std::thread::Builder::new()
            .name("gravak-login".into())
            .spawn(move || {
                let runtime = match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(runtime) => runtime,
                    Err(error) => {
                        let _ = events_tx.send(LoginEvent::Disconnected(error.to_string()));
                        return;
                    }
                };
                runtime.block_on(serve(events_tx, commands_rx));
            })
            .expect("login thread should start");
        Self { commands, events }
    }

    pub fn send(&self, command: LoginCommand) {
        let _ = self.commands.send(command);
    }

    pub fn drain(&self) -> Vec<LoginEvent> {
        self.events.try_iter().collect()
    }
}

enum Phase {
    Idle,
    /// Logged in, waiting for a character to be created.
    Creating {
        session: GameSession,
        username: String,
        password: String,
        token: u32,
    },
    /// Logged in with a character, waiting for Enter.
    Selected {
        session: GameSession,
        user: UserInfo,
    },
}

enum Login {
    NeedsCharacter(GameSession, u32),
    Character(GameSession, UserInfo),
}

async fn serve(
    events: Sender<LoginEvent>,
    mut commands: UnboundedReceiver<LoginCommand>,
) {
    let send = |event| {
        let _ = events.send(event);
    };
    let mut phase = Phase::Idle;
    // The world of the last login: a new character logs in there again.
    let mut world = World::new("", "", "");
    while let Some(command) = commands.recv().await {
        match command {
            LoginCommand::Register { world, username, password } => {
                gravak_protocol::set_login_key(world.key);
                send(LoginEvent::Status("creating account".into()));
                send(match register_account(&world.auth, &username, &password).await {
                    Ok(RegisterAccountResult::Created) => LoginEvent::Registered(username),
                    Ok(result) => LoginEvent::RegisterFailed(register_message(result).into()),
                    Err(error) => {
                        LoginEvent::RegisterFailed(format!("account server unavailable ({error})"))
                    }
                });
            }
            LoginCommand::Login { world: chosen, username, password } => {
                phase = Phase::Idle;
                world = chosen;
                gravak_protocol::set_login_key(world.key);
                send(LoginEvent::Status("authenticating".into()));
                match login(&world.auth, &username, &password, &world.realm, &send).await {
                    Ok(Login::NeedsCharacter(session, token)) => {
                        phase = Phase::Creating {
                            session,
                            username,
                            password,
                            token,
                        };
                        send(LoginEvent::NeedsCharacter);
                    }
                    Ok(Login::Character(session, user)) => {
                        send(LoginEvent::Character(CharacterSummary::from_user(&user)));
                        phase = Phase::Selected { session, user };
                    }
                    Err(error) => send(LoginEvent::LoginFailed(error)),
                }
            }
            LoginCommand::CreateCharacter { name, mesh, class, coat_dye, avatar, hair } => {
                let Phase::Creating {
                    mut session,
                    username,
                    password,
                    token,
                } = std::mem::replace(&mut phase, Phase::Idle)
                else {
                    send(LoginEvent::CharacterRejected("not logged in".into()));
                    continue;
                };
                send(LoginEvent::Status(format!("creating {name}")));
                let look = Look { mesh, class, coat_dye, avatar, hair };
                match create_character(&mut session, &username, &name, look, token).await {
                    Ok(()) => {
                        // The server ends the registration session; log in
                        // again to load the new character.
                        drop(session);
                        match login(&world.auth, &username, &password, &world.realm, &send).await {
                            Ok(Login::Character(session, user)) => {
                                send(LoginEvent::Character(CharacterSummary::from_user(&user)));
                                phase = Phase::Selected { session, user };
                            }
                            Ok(Login::NeedsCharacter(..)) => send(LoginEvent::Disconnected(
                                "the new character was not found".into(),
                            )),
                            Err(error) => send(LoginEvent::Disconnected(error)),
                        }
                    }
                    Err(CreateError::Rejected(message)) => {
                        phase = Phase::Creating {
                            session,
                            username,
                            password,
                            token,
                        };
                        send(LoginEvent::CharacterRejected(message));
                    }
                    Err(CreateError::Closed(message)) => send(LoginEvent::Disconnected(message)),
                }
            }
            LoginCommand::Enter => {
                let Phase::Selected { session, user } = std::mem::replace(&mut phase, Phase::Idle)
                else {
                    continue;
                };
                let (handle, network_events, network_commands) = NetworkHandle::channels();
                send(LoginEvent::Entered(handle));
                if let Err(error) =
                    crate::network::play(session, user, network_events.clone(), network_commands)
                        .await
                {
                    let _ = network_events.send(NetworkEvent::Error(error));
                }
                return;
            }
            LoginCommand::Logout => phase = Phase::Idle,
        }
    }
}

fn register_message(result: RegisterAccountResult) -> &'static str {
    match result {
        RegisterAccountResult::Created => "account created",
        RegisterAccountResult::UsernameTaken => "this account name is already in use",
        RegisterAccountResult::InvalidUsername => "account names are 4-15 letters, digits or _",
        RegisterAccountResult::InvalidPassword => "passwords are 4-15 characters, no spaces",
        RegisterAccountResult::TryAgainLater | RegisterAccountResult::Unknown(_) => {
            "the account server could not create the account, try again"
        }
    }
}

/// What the account server's refusal means, for the player (its codes are
/// the server's `RejectionCode`).
fn rejection_message(rejection: &LoginRejection) -> String {
    match rejection.code {
        1 => "Wrong account or password.".into(),
        12 => "This account is banned.".into(),
        22 => "This account is locked.".into(),
        51 => "Too many wrong passwords. Wait a few minutes and try again.".into(),
        73 => "This client is out of date. Restart it to update.".into(),
        10 | 42 | 70 => "The server is closed right now. Try again later.".into(),
        11 | 20 => "The server is busy. Try again in a moment.".into(),
        _ if !rejection.message.is_empty() => rejection.message.clone(),
        code => format!("The login was refused (code {code})."),
    }
}

async fn login(
    auth: &str,
    username: &str,
    password: &str,
    realm: &str,
    send: &impl Fn(LoginEvent),
) -> Result<Login, String> {
    let server = match authenticate(auth, username, password, realm).await {
        Ok(AuthResponse::Forwarded(server)) => server,
        Ok(AuthResponse::Rejected(rejection)) => return Err(rejection_message(&rejection)),
        Err(error) => return Err(format!("account server unavailable ({error})")),
    };
    send(LoginEvent::Status("connecting to the game server".into()));
    let mut session = connect(&server)
        .await
        .map_err(|error| format!("game server unavailable ({error})"))?;
    loop {
        match session
            .next_event()
            .await
            .map_err(|error| error.to_string())?
        {
            Some(GameEvent::Talk(talk)) if talk.message == "NEW_ROLE" => {
                return Ok(Login::NeedsCharacter(session, server.token as u32));
            }
            Some(GameEvent::Talk(talk))
                if talk.channel == LOGIN_CHANNEL && talk.message != "ANSWER_OK" =>
            {
                return Err(talk.message);
            }
            Some(GameEvent::UserInfo(user)) => return Ok(Login::Character(session, user)),
            Some(_) => {}
            None => return Err("the game server closed the connection".into()),
        }
    }
}

async fn connect(
    server: &ForwardedGameServer,
) -> Result<GameSession, gravak_protocol::ProtocolError> {
    let address = format!("{}:{}", server.address, server.port);
    GameSession::connect(&address, server.token, 5065, "English").await
}

enum CreateError {
    /// The server refused the name or look; the session stays usable.
    Rejected(String),
    Closed(String),
}

/// What the creation screen picked for a new character.
struct Look {
    mesh: u16,
    class: u16,
    coat_dye: u8,
    avatar: u16,
    hair: u16,
}

async fn create_character(
    session: &mut GameSession,
    username: &str,
    name: &str,
    look: Look,
    token: u32,
) -> Result<(), CreateError> {
    session
        .send(
            &RegisterCharacter::with_look(username, name, look.mesh, look.class, token)
                .with_coat_dye(look.coat_dye)
                .with_appearance(look.avatar, look.hair),
        )
        .await
        .map_err(|error| CreateError::Closed(error.to_string()))?;
    loop {
        match session.next_event().await {
            Ok(Some(GameEvent::Talk(talk))) if talk.message == "ANSWER_OK" => return Ok(()),
            Ok(Some(GameEvent::Talk(talk))) if talk.channel == REGISTER_CHANNEL => {
                return Err(CreateError::Rejected(talk.message));
            }
            Ok(Some(_)) => {}
            Ok(None) => {
                return Err(CreateError::Closed(
                    "the game server closed the connection".into(),
                ));
            }
            Err(error) => return Err(CreateError::Closed(error.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refusals_say_what_happened() {
        let refusal = |code| rejection_message(&LoginRejection { code, message: String::new() });
        assert_eq!(refusal(1), "Wrong account or password.");
        assert_eq!(refusal(51), "Too many wrong passwords. Wait a few minutes and try again.");
        assert_eq!(refusal(99), "The login was refused (code 99).");
        assert_eq!(rejection_message(&LoginRejection { code: 99, message: "Closed".into() }), "Closed");
    }
}

#[cfg(test)]
mod smoke {
    use super::*;
    use web_time::{Duration, Instant, SystemTime, UNIX_EPOCH};

    fn next(client: &LoginClient) -> LoginEvent {
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            if let Some(event) = client.drain().into_iter().find(|event| !matches!(event, LoginEvent::Status(_))) {
                return event;
            }
            assert!(Instant::now() < deadline, "timed out waiting for the login session");
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    /// Five wrong passwords lock this address out of logging in, the right
    /// one included, against a running auth server (GRAVAK_AUTH) reached
    /// over the network: this machine's own address is never locked out.
    #[test]
    #[ignore = "Needs the auth server running, and locks this address out for minutes"]
    fn wrong_passwords_lock_the_address_out() {
        let auth = std::env::var("GRAVAK_AUTH").unwrap_or_else(|_| "127.0.0.1:9958".into());
        let suffix = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() % 1_000_000;
        let account = format!("lock{suffix}");
        let world = World::new("Gravak", auth, "Gravak");
        let client = LoginClient::start();
        client.send(LoginCommand::Register { world: world.clone(), username: account.clone(), password: "pass1234".into() });
        assert!(matches!(next(&client), LoginEvent::Registered(_)));

        for _ in 0..5 {
            client.send(LoginCommand::Login { world: world.clone(), username: account.clone(), password: "wrong".into() });
            assert!(matches!(next(&client), LoginEvent::LoginFailed(message) if message == "Wrong account or password."));
        }
        client.send(LoginCommand::Login { world: world.clone(), username: account.clone(), password: "pass1234".into() });
        match next(&client) {
            LoginEvent::LoginFailed(message) => assert!(message.starts_with("Too many wrong passwords"), "{message}"),
            _ => panic!("expected the address locked out"),
        }
    }

    /// Registers a fresh account, creates its character and enters the
    /// world against running servers (GRAVAK_AUTH, default 127.0.0.1:9958).
    #[test]
    #[ignore = "Needs the auth and game servers running"]
    fn register_create_and_enter() {
        let auth = std::env::var("GRAVAK_AUTH").unwrap_or_else(|_| "127.0.0.1:9958".into());
        let suffix = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() % 1_000_000;
        let account = format!("smoke{suffix}");
        let world = World::new("Gravak", auth, "Gravak");
        let client = LoginClient::start();

        client.send(LoginCommand::Register { world: world.clone(), username: account.clone(), password: "pass1234".into() });
        assert!(matches!(next(&client), LoginEvent::Registered(name) if name == account));
        client.send(LoginCommand::Register { world: world.clone(), username: account.clone(), password: "pass1234".into() });
        assert!(matches!(next(&client), LoginEvent::RegisterFailed(_)));

        client.send(LoginCommand::Login { world: world.clone(), username: account.clone(), password: "wrong".into() });
        assert!(matches!(next(&client), LoginEvent::LoginFailed(_)));
        client.send(LoginCommand::Login { world: world.clone(), username: account.clone(), password: "pass1234".into() });
        assert!(matches!(next(&client), LoginEvent::NeedsCharacter));

        client.send(LoginCommand::CreateCharacter { name: "x".into(), mesh: 2001, class: 40, coat_dye: 3, avatar: 205, hair: 742 });
        assert!(matches!(next(&client), LoginEvent::CharacterRejected(_)));
        let name = format!("Smoke{suffix}");
        client.send(LoginCommand::CreateCharacter { name: name.clone(), mesh: 2001, class: 40, coat_dye: 3, avatar: 205, hair: 742 });
        match next(&client) {
            LoginEvent::Character(summary) => {
                assert_eq!(summary.name, name);
                assert_eq!(summary.class, 40);
            }
            _ => panic!("expected the new character"),
        }

        client.send(LoginCommand::Enter);
        let LoginEvent::Entered(handle) = next(&client) else {
            panic!("expected to enter the world");
        };
        let ready = handle.wait_ready().expect("world loads");
        assert_eq!(ready.character_name, name);
    }
}
