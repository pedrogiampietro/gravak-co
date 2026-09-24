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
use tq_network::{CQCipher, Cipher, PacketDecode, PacketEncode, PacketID, TQCodec};
use tq_serde::{String16, TQPassword};

pub const MSG_ACCOUNT: u16 = 1051;
pub const MSG_CONNECT: u16 = 1052;
pub const MSG_CONNECT_EX: u16 = 1055;
pub const MSG_TALK: u16 = 1004;

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

#[derive(Debug)]
pub enum ProtocolError {
    Io(std::io::Error),
    Encode(tq_network::Error),
    Decode(tq_network::Error),
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
}
