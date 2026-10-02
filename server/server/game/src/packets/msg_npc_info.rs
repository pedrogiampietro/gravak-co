use crate::entities::Npc;
use serde::Serialize;
use tq_network::PacketID;

/// This packet is used to spawn NPCs to players.
#[derive(Debug, Serialize, Clone, PacketID, Default)]
#[packet(id = 2030)]
pub struct MsgNpcInfo {
    /// UniqueID
    id: u32,
    x: u16,
    y: u16,
    look: u16,
    kind: u16,
    sort: u16,
    /// * 0 if not sending any name
    /// * 1 if sending name
    list_count: u8,
    /// The name of the NPC
    name: Option<String>,
}

impl MsgNpcInfo {
    pub fn new(npc: &Npc) -> Self {
        let loc = npc.entity().location();
        Self {
            id: npc.id(),
            x: loc.x,
            y: loc.y,
            look: npc.entity().mesh() as u16,
            kind: npc.kind() as u16,
            sort: npc.sort() as u16,
            list_count: 0,
            name: None,
        }
    }

    /// An NPC shown with another look and, optionally, a name (the Guild
    /// War pole and gates).
    pub fn with_look(npc: &Npc, look: u16, name: Option<String>) -> Self {
        let mut this = Self::new(npc);
        this.look = look;
        this.list_count = u8::from(name.is_some());
        this.name = name;
        this
    }

    pub fn from_npc_with_name(npc: &Npc) -> Self {
        let mut this = Self::new(npc);
        this.list_count = 1;
        this.name = Some(npc.entity().name().to_string());
        this
    }
}
