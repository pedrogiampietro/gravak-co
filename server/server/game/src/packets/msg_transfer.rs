use crate::{ActorState, Error, State};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::OnceLock;
use tq_network::{Actor, PacketID, PacketProcess};

/// Defines account parameters to be transferred from the account server to the
/// game server. Account information is supplied from the account database, and
/// used on the game server to transfer authentication and authority level.
///
/// It hands out a login token for any account, so only the auth server may
/// send it: one that showed the `TRANSFER_SECRET` key ([`MsgTransferKey`]),
/// or, when no key is set, one on this machine.
#[derive(Clone, Debug, Deserialize, Serialize, PacketID)]
#[packet(id = 4001)]
pub struct MsgTransfer {
    account_id: u32,
    realm_id: u32,
    #[serde(skip_deserializing)]
    token: u64,
}

/// Sent by the auth server before [`MsgTransfer`]: the `TRANSFER_SECRET`
/// both servers share. Never sent by game clients.
#[derive(Clone, Debug, Deserialize, Serialize, PacketID)]
#[packet(id = 4002)]
pub struct MsgTransferKey {
    key: String,
}

/// `TRANSFER_SECRET`, when set and not empty.
fn transfer_secret() -> Option<&'static str> {
    static SECRET: OnceLock<Option<String>> = OnceLock::new();
    SECRET
        .get_or_init(|| std::env::var("TRANSFER_SECRET").ok().filter(|key| !key.trim().is_empty()))
        .as_deref()
}

/// Whether a connection may ask for a login token.
fn may_transfer(secret: Option<&str>, showed_key: bool, peer: Option<SocketAddr>) -> bool {
    match secret {
        Some(_) => showed_key,
        None => peer.is_some_and(|peer| peer.ip().is_loopback()),
    }
}

/// Compares without stopping at the first difference, so the time taken
/// does not tell how much of the key was right.
fn same_key(given: &str, expected: &str) -> bool {
    given.len() == expected.len()
        && given
            .bytes()
            .zip(expected.bytes())
            .fold(0_u8, |difference, (a, b)| difference | (a ^ b))
            == 0
}

#[async_trait]
impl PacketProcess for MsgTransferKey {
    type ActorState = ActorState;
    type Error = Error;
    type State = State;

    async fn process(&self, _state: &Self::State, actor: &Actor<Self::ActorState>) -> Result<(), Self::Error> {
        match transfer_secret() {
            Some(secret) if same_key(&self.key, secret) => actor.mark_auth_server(),
            _ => {
                tracing::warn!(target: "auth", peer = ?actor.peer(), "Wrong transfer key; connection closed");
                actor.shutdown().await?;
            },
        }
        Ok(())
    }
}

#[async_trait]
impl PacketProcess for MsgTransfer {
    type ActorState = ActorState;
    type Error = Error;
    type State = State;

    async fn process(&self, state: &Self::State, actor: &Actor<Self::ActorState>) -> Result<(), Self::Error> {
        if !may_transfer(transfer_secret(), actor.is_auth_server(), actor.peer()) {
            tracing::warn!(
                target: "auth",
                peer = ?actor.peer(),
                account = self.account_id,
                "Login token asked by someone who is not the auth server; connection closed"
            );
            actor.shutdown().await?;
            return Ok(());
        }
        let generated = state.generate_login_token(self.account_id, self.realm_id)?;
        let mut msg = self.clone();
        msg.token = generated.token;
        actor.send(msg).await?;
        actor.shutdown().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_auth_server_gets_login_tokens() {
        let local = Some(SocketAddr::from(([127, 0, 0, 1], 50_000)));
        let remote = Some(SocketAddr::from(([203, 0, 113, 7], 50_000)));
        // Without a key, only this machine.
        assert!(may_transfer(None, false, local));
        assert!(!may_transfer(None, false, remote));
        assert!(!may_transfer(None, false, None));
        // With a key, only who showed it, wherever it is.
        assert!(may_transfer(Some("key"), true, remote));
        assert!(!may_transfer(Some("key"), false, local));
    }

    #[test]
    fn keys_compare_whole() {
        assert!(same_key("s3cret", "s3cret"));
        assert!(!same_key("s3cres", "s3cret"));
        assert!(!same_key("s3cre", "s3cret"));
    }
}
