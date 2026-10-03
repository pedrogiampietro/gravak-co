//! When each status of a character ends (Gravak packet, not in the 5065
//! client), so the HUD can count them down: Superman, Cyclone, Shield...,
//! double experience, Heaven's Blessing and VIP. Sent only when they change.

use serde::{Deserialize, Serialize};
use tq_network::PacketID;

/// `timers` is `flag:end,flag:end...`, a status flag bit and when it ends
/// in seconds since 1970; `now` is the server's clock, for the client's
/// offset.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, PacketID)]
#[packet(id = 9120)]
pub struct MsgStatusTimers {
    pub now: u64,
    pub timers: String,
}

impl MsgStatusTimers {
    pub fn new(now: u64, timers: &[(u64, u64)]) -> Self {
        let timers = timers
            .iter()
            .map(|(flag, end)| format!("{flag}:{end}"))
            .collect::<Vec<_>>()
            .join(",");
        Self { now, timers }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tq_network::{PacketDecode, PacketEncode};

    #[test]
    fn timers_survive_the_wire() {
        let timers = MsgStatusTimers::new(1_000, &[(1 << 18, 1_030), (1 << 49, 2_000_000)]);
        assert_eq!(timers.timers, "262144:1030,562949953421312:2000000");
        let (_, bytes) = timers.clone().encode().expect("encodes");
        assert_eq!(MsgStatusTimers::decode(&bytes).expect("decodes"), timers);
    }
}
