//! The Rifts' tracker (`crate::rifts`): where a team's run stands, sent on
//! every change. Phase 0 hides the tracker.

use serde::{Deserialize, Serialize};
use tq_network::PacketID;

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, PacketID)]
#[packet(id = 9180)]
pub struct MsgRift {
    /// `crate::rifts::Phase`: 0 closed, 1 clearing, 2 the Guardian, 3 won,
    /// 4 lost.
    pub phase: u32,
    /// 0 Lesser, 1 Greater, 2 Abyssal.
    pub tier: u32,
    pub killed: u32,
    pub total: u32,
    /// Until the time runs out (clearing, the Guardian) or the rift closes
    /// (won, lost).
    pub seconds_left: u32,
}
