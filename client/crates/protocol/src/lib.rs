//! Minimal Conquer Online 5065-compatible wire protocol used by the client.
//!
//! The server currently exposes the CoEmu 5017-era packet IDs. Keeping the
//! transport and the first authentication messages in a small crate gives us
//! a stable place to measure and close that compatibility gap.

use bytes::Bytes;
use serde::{Deserialize, Serialize};
use std::fmt;
use tokio::net::TcpStream;
use tokio_stream::StreamExt;
use tq_codec::{TQDecoder, TQEncoder};
use tq_network::{CQCipher, Cipher, PacketDecode, PacketEncode, PacketID, TQCodec};
use tq_serde::{String16, TQMaskedPassword, TQPassword};

pub const MSG_ACCOUNT: u16 = 1051;
pub const MSG_CONNECT: u16 = 1052;
pub const MSG_CONNECT_EX: u16 = 1055;
pub const MSG_TALK: u16 = 1004;
pub const MSG_REGISTER: u16 = 1001;
pub const MSG_WALK: u16 = 1005;
pub const MSG_USER_INFO: u16 = 1006;
pub const MSG_ITEM_INFO: u16 = 1008;
pub const MSG_ITEM: u16 = 1009;
pub const MSG_ACTION: u16 = 1010;
pub const MSG_WEATHER: u16 = 1016;
pub const MSG_DATA: u16 = 1033;
pub const MSG_MAP_INFO: u16 = 1110;
pub const MSG_PLAYER: u16 = 1014;
pub const MSG_NPC_INFO: u16 = 2030;
pub const MSG_NPC: u16 = 2031;
pub const MSG_TASK_DIALOG: u16 = 2032;

#[derive(Debug, Serialize, PacketID)]
#[packet(id = 1052)]
pub struct GameConnect {
    pub token: u64,
    pub build_version: u16,
    pub language: tq_serde::String10,
    pub file_contents: u32,
}

impl GameConnect {
    pub fn new(token: u64, build_version: u16, language: &str) -> Self {
        Self {
            token,
            build_version,
            language: language.to_owned().into(),
            file_contents: 0,
        }
    }
}

#[derive(Debug, Serialize, PacketID)]
#[packet(id = 1051)]
pub struct AccountLogin {
    pub username: String16,
    pub password: TQPassword,
    pub realm: String16,
}

#[derive(Debug, Serialize, PacketID)]
#[packet(id = 1001)]
pub struct RegisterCharacter {
    pub username: String16,
    pub character_name: String16,
    pub password: TQMaskedPassword,
    pub mesh: u16,
    pub class: u16,
    pub token: u32,
}

impl RegisterCharacter {
    pub fn new(username: &str, character_name: &str, token: u32) -> Self {
        Self {
            username: username.to_owned().into(),
            character_name: character_name.to_owned().into(),
            password: "********".to_owned().into(),
            mesh: 1003,
            class: 10,
            token,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, PacketID)]
#[packet(id = 1010)]
pub struct ActionPacket {
    pub client_timestamp: u32,
    pub character_id: u32,
    pub data1: u32,
    pub data2: u32,
    pub details: u16,
    pub action_type: u16,
}

impl ActionPacket {
    pub fn send_location(character_id: u32) -> Self {
        Self {
            client_timestamp: unix_timestamp(),
            character_id,
            data1: 0,
            data2: 0,
            details: 0,
            action_type: 74,
        }
    }

    pub fn change_action(character_id: u32, action: u32) -> Self {
        Self {
            client_timestamp: unix_timestamp(),
            character_id,
            data1: action,
            data2: 0,
            details: 0,
            action_type: 81,
        }
    }

    pub fn request_items(character_id: u32) -> Self {
        Self {
            client_timestamp: unix_timestamp(),
            character_id,
            data1: 0,
            data2: 0,
            details: 0,
            action_type: 75,
        }
    }
}

#[derive(Debug, Serialize, PacketID)]
#[packet(id = 1009)]
pub struct ItemActionPacket {
    pub character_id: u32,
    pub item_id: u32,
    pub action_type: u32,
    pub client_timestamp: u32,
    pub amount: u32,
    pub data2: u32,
}

impl ItemActionPacket {
    pub fn buy(character_id: u32, item_id: u32, inventory_slot: u8) -> Self {
        Self {
            character_id,
            item_id,
            action_type: 1,
            client_timestamp: unix_timestamp(),
            amount: 1,
            data2: u32::from(inventory_slot),
        }
    }

    pub fn sell(character_id: u32, item_uid: u32, shop_id: u32) -> Self {
        Self {
            character_id,
            item_id: item_uid,
            action_type: 2,
            client_timestamp: unix_timestamp(),
            amount: 1,
            data2: shop_id,
        }
    }
}

#[derive(Debug, Deserialize, Clone, PartialEq, Eq)]
pub struct ItemInfoPacket {
    pub item_uid: u32,
    pub item_type: u32,
    pub durability: u16,
    pub max_durability: u16,
    pub action: u8,
    pub ident: u8,
    pub position: u8,
    pub reserved0: u8,
    pub reserved1: u32,
    pub gem_one: u8,
    pub gem_two: u8,
    pub reborn_effect: u8,
    pub magic: u8,
    pub plus: u8,
    pub bless: u8,
    pub enchant: u8,
    pub reserved2: u8,
    pub restrain: u32,
    pub reserved3: u32,
    pub reserved4: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, PacketID)]
#[packet(id = 2031)]
pub struct NpcPacket {
    pub npc_id: u32,
    pub data: u32,
    pub action: u16,
    pub kind: u16,
}

#[derive(Debug, Serialize, Clone, PartialEq, Eq, PacketID)]
#[packet(id = 2032)]
pub struct NpcDialogPacket {
    pub task_id: u32,
    pub avatar: u16,
    pub option_id: u8,
    pub action: u8,
    pub msgs: tq_serde::StringList,
}

impl NpcDialogPacket {
    pub fn answer(task_id: u32, option_id: u8) -> Self {
        Self {
            task_id,
            avatar: 0,
            option_id,
            action: 101,
            msgs: Vec::<String>::new().into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskDialog {
    pub task_id: u32,
    pub avatar: u16,
    pub option_id: u8,
    pub action: u8,
    pub messages: Vec<String>,
}

impl NpcPacket {
    pub fn activate(npc_id: u32, kind: u16) -> Self {
        Self {
            npc_id,
            data: 0,
            action: 0,
            kind,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, PacketID)]
#[packet(id = 1005)]
pub struct WalkPacket {
    pub character_id: u32,
    pub direction: u8,
    pub movement_type: u8,
}

impl WalkPacket {
    pub fn new(character_id: u32, direction: u8) -> Self {
        Self {
            character_id,
            direction,
            movement_type: 0,
        }
    }
}

impl AccountLogin {
    pub fn new(username: &str, password: &str, realm: &str) -> Self {
        Self {
            username: username.to_owned().into(),
            password: password.to_owned().into(),
            realm: realm.to_owned().into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForwardedGameServer {
    pub token: u64,
    pub address: String,
    pub port: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoginRejection {
    pub code: u32,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthResponse {
    Forwarded(ForwardedGameServer),
    Rejected(LoginRejection),
}

#[derive(Debug, Deserialize, Clone, PartialEq, Eq)]
pub struct ServerTalk {
    pub color: u32,
    pub channel: u16,
    pub style: u16,
    pub character_id: u32,
    pub recipient_mesh: u32,
    pub sender_mesh: u32,
    pub list_count: u8,
    pub sender_name: String,
    pub recipient_name: String,
    pub suffix: String,
    pub message: String,
}

#[derive(Debug, Deserialize, Clone, PartialEq, Eq)]
pub struct UserInfo {
    pub character_id: u32,
    pub mesh: u32,
    pub hair_style: u16,
    pub silver: u32,
    pub cps: u32,
    pub experience: u64,
    pub reserved0: u64,
    pub reserved1: u64,
    pub strength: u16,
    pub agility: u16,
    pub vitality: u16,
    pub spirit: u16,
    pub attribute_points: u16,
    pub health_points: u16,
    pub mana_points: u16,
    pub kill_points: u16,
    pub level: u8,
    pub current_class: u8,
    pub previous_class: u8,
    pub rebirths: u8,
    pub show_name: bool,
    pub list_count: u8,
    pub character_name: String,
    pub spouse: String,
}

#[derive(Debug, Deserialize, Clone, PartialEq, Eq)]
pub struct ServerData {
    pub action: u32,
    pub year: i32,
    pub month: i32,
    pub day: i32,
    pub hour: i32,
    pub minute: i32,
    pub second: i32,
}

#[derive(Debug, Deserialize, Clone, PartialEq, Eq)]
pub struct MapInfo {
    pub uid: u32,
    pub map_id: u32,
    pub flags: u8,
}

#[derive(Debug, Deserialize, Clone, PartialEq, Eq)]
pub struct ServerPlayer {
    pub character_id: i32,
    pub mesh: i32,
    pub status_flags: i64,
    pub syndicate_id: i16,
    pub reserved0: u8,
    pub syndicate_member_rank: u8,
    pub germent: i32,
    pub helment: i32,
    pub armor: i32,
    pub right_hand: i32,
    pub left_hand: i32,
    pub reserved1: i32,
    pub health_points: u16,
    pub level: i16,
    pub x: u16,
    pub y: u16,
    pub hair_style: i16,
    pub direction: u8,
    pub action: u8,
    pub metempsychosis: i16,
    pub level2: i16,
    pub reserved2: i32,
    pub nobility_rank: i32,
    pub character_id2: i32,
    pub nobility_position: i32,
    pub list_count: u8,
    pub character_name: String,
}

#[derive(Debug, Deserialize, Clone, PartialEq, Eq)]
struct ServerNpcInfoWire {
    pub id: u32,
    pub x: u16,
    pub y: u16,
    pub look: u16,
    pub kind: u16,
    pub sort: u16,
    pub list_count: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerNpcInfo {
    pub id: u32,
    pub x: u16,
    pub y: u16,
    pub look: u16,
    pub kind: u16,
    pub sort: u16,
    pub name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GameEvent {
    Talk(ServerTalk),
    UserInfo(UserInfo),
    ItemInfo(ItemInfoPacket),
    Data(ServerData),
    MapInfo(MapInfo),
    Action(ActionPacket),
    Walk(WalkPacket),
    Player(ServerPlayer),
    NpcInfo(ServerNpcInfo),
    TaskDialog(TaskDialog),
    Other(u16, Bytes),
}

pub struct GameSession {
    encoder: TQEncoder<TcpStream, CQCipher>,
    decoder: TQDecoder<TcpStream, CQCipher>,
}

impl GameSession {
    pub async fn connect(
        address: &str,
        token: u64,
        build_version: u16,
        language: &str,
    ) -> Result<Self, ProtocolError> {
        let stream = TcpStream::connect(address).await?;
        let cipher = CQCipher::new();
        let (mut encoder, decoder) = TQCodec::new(stream, cipher.clone()).split();
        let packet = GameConnect::new(token, build_version, language)
            .encode()
            .map_err(ProtocolError::Encode)?;
        encoder.send(packet).await?;
        cipher.generate_keys(token);
        Ok(Self { encoder, decoder })
    }

    pub async fn send<P>(&mut self, packet: &P) -> Result<(), ProtocolError>
    where
        P: PacketEncode,
        P::Error: Into<tq_network::Error>,
    {
        let encoded = packet
            .encode()
            .map_err(|error| ProtocolError::Encode(error.into()))?;
        self.encoder.send(encoded).await?;
        Ok(())
    }

    pub async fn next_packet(&mut self) -> Result<Option<(u16, Bytes)>, ProtocolError> {
        match self.decoder.next().await {
            Some(packet) => Ok(Some(packet?)),
            None => Ok(None),
        }
    }

    pub async fn next_event(&mut self) -> Result<Option<GameEvent>, ProtocolError> {
        let Some((id, body)) = self.next_packet().await? else {
            return Ok(None);
        };
        decode_game_event(id, &body)
            .map(Some)
            .map_err(|error| match error {
                ProtocolError::Decode(reason) => ProtocolError::MalformedPacket {
                    id,
                    reason: format!("body_len={} {reason}", body.len()),
                },
                other => other,
            })
    }
}

fn decode_game_event(id: u16, body: &Bytes) -> Result<GameEvent, ProtocolError> {
    match id {
        MSG_TALK => Ok(GameEvent::Talk(
            ServerTalk::decode(body).map_err(ProtocolError::Decode)?,
        )),
        MSG_USER_INFO => Ok(GameEvent::UserInfo(
            UserInfo::decode(body).map_err(ProtocolError::Decode)?,
        )),
        MSG_ITEM_INFO => Ok(GameEvent::ItemInfo(
            ItemInfoPacket::decode(body).map_err(ProtocolError::Decode)?,
        )),
        MSG_DATA => Ok(GameEvent::Data(
            ServerData::decode(body).map_err(ProtocolError::Decode)?,
        )),
        MSG_MAP_INFO => Ok(GameEvent::MapInfo(
            MapInfo::decode(body).map_err(ProtocolError::Decode)?,
        )),
        MSG_ACTION => Ok(GameEvent::Action(
            ActionPacket::decode(body).map_err(ProtocolError::Decode)?,
        )),
        MSG_WALK => Ok(GameEvent::Walk(
            WalkPacket::decode(body).map_err(ProtocolError::Decode)?,
        )),
        MSG_PLAYER => Ok(GameEvent::Player(
            ServerPlayer::decode(body).map_err(ProtocolError::Decode)?,
        )),
        MSG_NPC_INFO => Ok(GameEvent::NpcInfo(decode_npc_info(body)?)),
        MSG_TASK_DIALOG => Ok(GameEvent::TaskDialog(decode_task_dialog(body)?)),
        _ => Ok(GameEvent::Other(id, body.clone())),
    }
}

fn decode_task_dialog(body: &Bytes) -> Result<TaskDialog, ProtocolError> {
    if body.len() < 8 {
        return Err(ProtocolError::MalformedResponse(
            "task dialog header is missing",
        ));
    }
    let task_id = u32::from_le_bytes(body[0..4].try_into().unwrap());
    let avatar = u16::from_le_bytes(body[4..6].try_into().unwrap());
    let option_id = body[6];
    let action = body[7];
    let mut offset = 8;
    let mut messages = Vec::new();
    if offset < body.len() {
        let count = body[offset] as usize;
        offset += 1;
        for _ in 0..count {
            if offset >= body.len() {
                return Err(ProtocolError::MalformedResponse(
                    "task dialog message length is missing",
                ));
            }
            let length = body[offset] as usize;
            offset += 1;
            if offset + length > body.len() {
                return Err(ProtocolError::MalformedResponse(
                    "task dialog message is truncated",
                ));
            }
            messages.push(String::from_utf8_lossy(&body[offset..offset + length]).into_owned());
            offset += length;
        }
    }
    Ok(TaskDialog {
        task_id,
        avatar,
        option_id,
        action,
        messages,
    })
}

fn decode_npc_info(body: &Bytes) -> Result<ServerNpcInfo, ProtocolError> {
    let wire = ServerNpcInfoWire::decode(body).map_err(ProtocolError::Decode)?;
    let name = if wire.list_count > 0 {
        if body.len() <= 15 {
            return Err(ProtocolError::MalformedResponse("npc info name is missing"));
        }
        Some(String::decode(&body.slice(15..)).map_err(ProtocolError::Decode)?)
    } else {
        None
    };
    Ok(ServerNpcInfo {
        id: wire.id,
        x: wire.x,
        y: wire.y,
        look: wire.look,
        kind: wire.kind,
        sort: wire.sort,
        name,
    })
}

#[derive(Debug)]
pub enum ProtocolError {
    Io(std::io::Error),
    Encode(tq_network::Error),
    Decode(tq_network::Error),
    MalformedPacket { id: u16, reason: String },
    UnexpectedPacket(u16),
    MalformedResponse(&'static str),
    InvalidAddress,
}

impl fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "network error: {error}"),
            Self::Encode(error) => write!(f, "packet encode error: {error}"),
            Self::Decode(error) => write!(f, "packet decode error: {error}"),
            Self::MalformedPacket { id, reason } => {
                write!(f, "packet {id} could not be decoded: {reason}")
            }
            Self::UnexpectedPacket(id) => write!(f, "unexpected packet id {id}"),
            Self::MalformedResponse(reason) => write!(f, "malformed auth response: {reason}"),
            Self::InvalidAddress => write!(f, "auth response contained an invalid address"),
        }
    }
}

impl std::error::Error for ProtocolError {}

impl From<std::io::Error> for ProtocolError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

/// Performs the first leg of the login flow against an auth server.
pub async fn authenticate(
    address: &str,
    username: &str,
    password: &str,
    realm: &str,
) -> Result<AuthResponse, ProtocolError> {
    let stream = TcpStream::connect(address).await?;
    let (mut encoder, mut decoder) = TQCodec::new(stream, CQCipher::new()).split();
    let packet = AccountLogin::new(username, password, realm)
        .encode()
        .map_err(ProtocolError::Encode)?;
    encoder.send(packet).await?;

    while let Some(packet) = decoder.next().await {
        let (id, body) = packet?;
        if id != MSG_CONNECT_EX {
            return Err(ProtocolError::UnexpectedPacket(id));
        }
        return parse_auth_response(&body);
    }

    Err(ProtocolError::Io(std::io::Error::new(
        std::io::ErrorKind::UnexpectedEof,
        "auth server closed the connection without a response",
    )))
}

/// Performs the second leg of the login flow against a game server.
pub async fn connect_game(
    address: &str,
    token: u64,
    build_version: u16,
    language: &str,
) -> Result<ServerTalk, ProtocolError> {
    let stream = TcpStream::connect(address).await?;
    let cipher = CQCipher::new();
    let (mut encoder, mut decoder) = TQCodec::new(stream, cipher.clone()).split();
    let packet = GameConnect::new(token, build_version, language)
        .encode()
        .map_err(ProtocolError::Encode)?;
    encoder.send(packet).await?;
    cipher.generate_keys(token);

    while let Some(packet) = decoder.next().await {
        let (id, body) = packet?;
        if id != MSG_TALK {
            continue;
        }
        return ServerTalk::decode(&body).map_err(ProtocolError::Decode);
    }

    Err(ProtocolError::Io(std::io::Error::new(
        std::io::ErrorKind::UnexpectedEof,
        "game server closed the connection without a response",
    )))
}

fn parse_auth_response(body: &Bytes) -> Result<AuthResponse, ProtocolError> {
    if body.len() < 8 {
        return Err(ProtocolError::MalformedResponse("missing response header"));
    }

    let first = u32::from_le_bytes(body[0..4].try_into().unwrap());
    let second = u32::from_le_bytes(body[4..8].try_into().unwrap());

    if body.len() >= 28 {
        let token = u64::from_le_bytes(body[0..8].try_into().unwrap());
        let address = fixed_string(&body[8..24])?;
        let port = u32::from_le_bytes(body[24..28].try_into().unwrap());
        if address.parse::<std::net::IpAddr>().is_err() && address != "localhost" {
            return Err(ProtocolError::InvalidAddress);
        }
        return Ok(AuthResponse::Forwarded(ForwardedGameServer {
            token,
            address,
            port,
        }));
    }

    let message = if body.len() >= 24 {
        fixed_string(&body[8..24])?
    } else {
        String::new()
    };
    Ok(AuthResponse::Rejected(LoginRejection {
        code: second,
        message: format!("{message} (reserved={first})"),
    }))
}

fn fixed_string(bytes: &[u8]) -> Result<String, ProtocolError> {
    let end = bytes
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(bytes.len());
    String::from_utf8(bytes[..end].to_vec())
        .map_err(|_| ProtocolError::MalformedResponse("invalid text"))
}

fn unix_timestamp() -> u32 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as u32)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tq_network::TQCipher;

    #[test]
    fn account_login_matches_server_wire_shape() {
        let packet = AccountLogin::new("test1", "123456", "CoEmu")
            .encode()
            .expect("account packet should encode");
        assert_eq!(packet.0, MSG_ACCOUNT);
        assert_eq!(packet.1.len(), 48);
        assert_eq!(&packet.1[..6], b"test1\0");
        assert_eq!(&packet.1[32..37], b"CoEmu");
    }

    #[test]
    fn parses_forwarded_server() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&42_u64.to_le_bytes());
        bytes.extend_from_slice(b"127.0.0.1\0\0\0\0\0\0\0");
        bytes.extend_from_slice(&5816_u32.to_le_bytes());
        let response = parse_auth_response(&Bytes::from(bytes)).unwrap();
        assert_eq!(
            response,
            AuthResponse::Forwarded(ForwardedGameServer {
                token: 42,
                address: "127.0.0.1".into(),
                port: 5816,
            })
        );
    }

    #[tokio::test]
    async fn codec_round_trips_an_encrypted_frame() {
        let (client_stream, server_stream) = tokio::io::duplex(1024);
        let (mut client_encoder, _client_decoder) =
            TQCodec::new(client_stream, CQCipher::new()).split();
        let (_server_encoder, mut server_decoder) =
            TQCodec::new(server_stream, TQCipher::new()).split();
        let packet = AccountLogin::new("test1", "123456", "CoEmu")
            .encode()
            .expect("account packet should encode");

        client_encoder.send(packet).await.unwrap();
        let received = server_decoder.next().await.unwrap().unwrap();
        assert_eq!(received.0, MSG_ACCOUNT);
        assert_eq!(received.1.len(), 48);
    }

    #[test]
    fn game_connect_matches_server_wire_shape() {
        let packet = GameConnect::new(42, 5065, "English")
            .encode()
            .expect("game connect packet should encode");
        assert_eq!(packet.0, MSG_CONNECT);
        assert_eq!(packet.1.len(), 24);
        assert_eq!(&packet.1[..8], &42_u64.to_le_bytes());
        assert_eq!(&packet.1[10..17], b"English");
    }

    #[test]
    fn world_packets_match_server_wire_shapes() {
        let register = RegisterCharacter::new("test1", "Gravak", 42)
            .encode()
            .expect("register packet should encode");
        assert_eq!(register.0, MSG_REGISTER);
        assert_eq!(register.1.len(), 56);
        assert_eq!(&register.1[52..56], &42_u32.to_le_bytes());

        let action = ActionPacket::send_location(1000001)
            .encode()
            .expect("action packet should encode");
        assert_eq!(action.0, MSG_ACTION);
        assert_eq!(action.1.len(), 20);

        let walk = WalkPacket::new(1000001, 0)
            .encode()
            .expect("walk packet should encode");
        assert_eq!(walk.0, MSG_WALK);
        assert_eq!(walk.1.len(), 6);
    }

    #[test]
    fn decodes_server_map_info_with_one_byte_flags() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&1010_u32.to_le_bytes());
        bytes.extend_from_slice(&1010_u32.to_le_bytes());
        bytes.push(0);
        let map = MapInfo::decode(&Bytes::from(bytes)).expect("map info should decode");
        assert_eq!(map.uid, 1010);
        assert_eq!(map.map_id, 1010);
        assert_eq!(map.flags, 0);
    }

    #[test]
    fn decodes_player_and_npc_screen_spawns() {
        let mut player = Vec::new();
        player.extend_from_slice(&1000002_i32.to_le_bytes());
        player.extend_from_slice(&241003_i32.to_le_bytes());
        player.extend_from_slice(&0_i64.to_le_bytes());
        player.extend_from_slice(&0_i16.to_le_bytes());
        player.extend_from_slice(&[0, 0]);
        player.extend_from_slice(&[0; 24]);
        player.extend_from_slice(&100_u16.to_le_bytes());
        player.extend_from_slice(&1_i16.to_le_bytes());
        player.extend_from_slice(&61_u16.to_le_bytes());
        player.extend_from_slice(&110_u16.to_le_bytes());
        player.extend_from_slice(&311_i16.to_le_bytes());
        player.extend_from_slice(&[0, 1]);
        player.extend_from_slice(&[0; 2 + 2 + 4 + 4 + 4 + 4]);
        player.push(1);
        player.push(5);
        player.extend_from_slice(b"Other");
        let GameEvent::Player(player) =
            decode_game_event(MSG_PLAYER, &Bytes::from(player)).unwrap()
        else {
            panic!("expected player spawn");
        };
        assert_eq!(player.character_id, 1000002);
        assert_eq!(player.character_name, "Other");
        assert_eq!((player.x, player.y), (61, 110));

        let mut npc = Vec::new();
        npc.extend_from_slice(&77_u32.to_le_bytes());
        npc.extend_from_slice(&62_u16.to_le_bytes());
        npc.extend_from_slice(&111_u16.to_le_bytes());
        npc.extend_from_slice(&123_u16.to_le_bytes());
        npc.extend_from_slice(&1_u16.to_le_bytes());
        npc.extend_from_slice(&2_u16.to_le_bytes());
        npc.push(1);
        npc.push(3);
        npc.extend_from_slice(b"Bob");
        let GameEvent::NpcInfo(npc) = decode_game_event(MSG_NPC_INFO, &Bytes::from(npc)).unwrap()
        else {
            panic!("expected npc spawn");
        };
        assert_eq!(npc.id, 77);
        assert_eq!((npc.x, npc.y), (62, 111));
        assert_eq!(npc.name.as_deref(), Some("Bob"));
    }

    #[test]
    fn decodes_task_dialog_messages() {
        let mut dialog = Vec::new();
        dialog.extend_from_slice(&42_u32.to_le_bytes());
        dialog.extend_from_slice(&47_u16.to_le_bytes());
        dialog.extend_from_slice(&255_u8.to_le_bytes());
        dialog.extend_from_slice(&0_u8.to_le_bytes());
        dialog.push(2);
        dialog.push(5);
        dialog.extend_from_slice(b"Hello");
        dialog.push(3);
        dialog.extend_from_slice(b"NPC");
        let GameEvent::TaskDialog(dialog) =
            decode_game_event(MSG_TASK_DIALOG, &Bytes::from(dialog)).unwrap()
        else {
            panic!("expected task dialog");
        };
        assert_eq!(dialog.task_id, 42);
        assert_eq!(dialog.avatar, 47);
        assert_eq!(dialog.messages, ["Hello", "NPC"]);
    }
}
