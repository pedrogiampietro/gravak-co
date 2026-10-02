use gravak_protocol::{
    ActionPacket, AssignAttributesPacket, AuthResponse, FriendPacket, GameEvent, GameSession,
    GroundItemPacket, GuildPacket, GuildWarehouseActionPacket, InteractionPacket, ItemActionPacket,
    ItemRarityPacket, ItemRarityRerollPacket, NpcDialogPacket, NpcPacket, RefineryPacket,
    RegisterCharacter, ServerNpcInfo, ServerPlayer, SocketGemPacket, TalkPacket, TaskDialog,
    TeamActionPacket, TeamMemberPacket, TradePacket, WarehouseActionPacket, authenticate,
};
use std::{
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

fn max_mana_for(spirit: u16, profession: u8) -> u32 {
    let multiplier = match profession {
        132 | 142 => 15,
        133 | 143 => 20,
        134 | 144 => 25,
        135 | 145 => 30,
        _ => 5,
    };
    u32::from(spirit) * multiplier
}

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
    pub body: u32,
    pub hair_style: u16,
    pub map_id: u32,
    pub position: [i32; 2],
    pub silver: u32,
    pub cps: u32,
    pub health: u32,
    pub maximum_health: u32,
    pub mana: u32,
    pub maximum_mana: u32,
    pub level: u8,
    pub experience: u64,
    pub strength: u16,
    pub agility: u16,
    pub vitality: u16,
    pub spirit: u16,
    pub attribute_points: u16,
    pub current_class: u8,
    pub previous_class: u8,
    pub rebirths: u8,
    pub kill_points: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OwnedItem {
    pub uid: u32,
    pub item_type: u32,
    pub position: u8,
    pub durability: u16,
    pub max_durability: u16,
    pub plus: u8,
    /// Custom rarity: 0 common, 1 rare, 2 epic, 3 legendary, 4 mythic.
    pub rarity: u8,
    /// Classic 5065 wire values: 0 closed, 255 open, otherwise gem code.
    pub gem_one: u8,
    pub gem_two: u8,
    /// Server-persisted seed for the item's custom rarity bonus slots.
    pub rarity_roll: u8,
    /// Blessing: 1, 3, 5 or 7 percent less damage taken.
    pub bless: u8,
    /// Life added by a WuxingOven enchantment.
    pub enchant_hp: u8,
    /// Tinter dye (3-9; 0 is the default).
    pub color: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PlayerShopItem {
    pub owner_id: u32,
    pub price: u32,
    pub item: OwnedItem,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TeamMember {
    pub member_id: u32,
    pub name: String,
    pub mesh: u32,
    pub health: u16,
    pub maximum_health: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GuildMember {
    pub name: String,
    pub character_id: u32,
    pub mesh: u32,
    pub level: u32,
    pub rank: u16,
    pub donation: i32,
    pub online: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ChatMessage {
    pub channel: u16,
    pub sender_name: String,
    pub recipient_name: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SocialEntry {
    pub friend_id: u32,
    pub name: String,
    pub online: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NobilityEntry {
    pub position: u32,
    pub character_id: u32,
    pub name: String,
    pub donation: u64,
    pub rank: u32,
}

#[derive(Debug, Default, Clone)]
pub struct TradeView {
    pub active: bool,
    pub peer_id: Option<u32>,
    pub pending_request: Option<u32>,
    /// Name of the player behind `pending_request`, when they are in view.
    pub pending_request_name: Option<String>,
    pub own_items: Vec<OwnedItem>,
    pub peer_items: Vec<OwnedItem>,
    pub own_silver: u32,
    pub peer_silver: u32,
    pub own_confirmed: bool,
    pub peer_confirmed: bool,
}

#[derive(Debug)]
pub enum NetworkEvent {
    Status(String),
    Ready(ReadyState),
    Silver(u32),
    Cps(u32),
    Stamina(u8),
    /// MsgUserAttrib life (0) or maximum life (1) of another entity: the
    /// Guild War pole and gates, whose life does not fit the spawn packet.
    ActorLife {
        id: u32,
        maximum: bool,
        value: u32,
    },
    /// MsgAction 109 (DropMagic): a rebirth took this skill away.
    SkillForgotten(u32),
    Vitals {
        health: u32,
        maximum_health: u32,
        mana: u32,
        maximum_mana: u32,
        level: u8,
        experience: u64,
    },
    Attributes {
        strength: u16,
        agility: u16,
        vitality: u16,
        spirit: u16,
        attribute_points: u16,
        current_class: u8,
        previous_class: u8,
        rebirths: u8,
        kill_points: u16,
        spouse: String,
    },
    Nobility {
        character_id: u32,
        donation: u64,
        rank: u32,
        remaining: u64,
        title_id: u32,
    },
    NobilityRanking(NobilityEntry),
    ExperienceGain {
        experience: u64,
    },
    EquipmentItem(OwnedItem),
    EquipmentItemRemoved {
        uid: u32,
        slot: u8,
    },
    InventoryItem(OwnedItem),
    InventoryItemRemoved(u32),
    WarehouseOpened(u32),
    WarehouseItem(OwnedItem),
    WarehouseItemRemoved(u32),
    WarehouseBalance(u32),
    GuildWarehouseOpened(u32),
    GuildWarehouseItem(OwnedItem),
    GuildWarehouseItemRemoved(u32),
    ChatMessage(ChatMessage),
    Notice(String),
    Team {
        action: u32,
        target_id: u32,
    },
    TeamMember {
        member: TeamMember,
        action: u8,
    },
    Friend {
        friend_id: u32,
        action: u8,
        name: String,
        online: bool,
    },
    Trade {
        target_id: u32,
        subtype: u32,
        data: u32,
    },
    TradeItem(OwnedItem),
    VendorItem {
        owner_id: u32,
        price: u32,
        item: OwnedItem,
    },
    Guild {
        action: u32,
        data: u32,
        name: Option<String>,
    },
    GuildMember(GuildMember),
    WalkConfirmed {
        character_id: u32,
        direction: u8,
    },
    JumpConfirmed {
        character_id: u32,
        position: [i32; 2],
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
    LevelUp {
        level: u8,
    },
    RemoteCombat {
        id: u32,
        target_id: u32,
        action: u32,
        value: u32,
    },
    SkillEffect {
        attacker_id: u32,
        target_id: u32,
        skill_id: u16,
        damage: u32,
    },
    Skill {
        skill_id: u16,
        level: u16,
        experience: u32,
    },
    WeaponSkill {
        weapon_type: u32,
        level: u32,
        experience: u32,
    },
    GroundItem {
        uid: u32,
        item_id: u32,
        position: [f32; 2],
        drop_type: u16,
    },
    NpcDialog(TaskDialog),
    RemoveActor(u32),
    PlayerSpawn(RemoteActor),
    NpcSpawn(RemoteActor),
    Error(String),
}

/// The XP circle: XP skills are ready.
pub const XP_CIRCLE_FLAG: u64 = 1 << 4;
/// Gravak extension bit for double experience.
pub const DOUBLE_EXPERIENCE_FLAG: u64 = 1 << 48;
pub const POISONED_FLAG: u64 = 1 << 1;
pub const STUNNED_FLAG: u64 = 1 << 5;
pub const CYCLONE_FLAG: u64 = 1 << 23;
pub const SUPERMAN_FLAG: u64 = 1 << 18;
pub const STIGMA_FLAG: u64 = 1 << 9;
pub const SHIELD_FLAG: u64 = 1 << 8;
pub const HEAVEN_BLESSING_FLAG: u64 = 1 << 49;

#[derive(Debug, Clone)]
pub struct RemoteActor {
    pub id: u32,
    pub name: String,
    pub position: [f32; 2],
    pub movement_from: [f32; 2],
    pub movement_to: [f32; 2],
    pub movement_started: f32,
    pub movement_until: f32,
    pub direction: u8,
    pub mesh: u32,
    pub body: u32,
    pub armor: u32,
    pub helmet: u32,
    pub hair_style: u16,
    pub right_weapon: u32,
    pub left_weapon: u32,
    pub action: u8,
    pub health: u16,
    pub maximum_health: u16,
    pub status_flags: u64,
    pub npc_kind: u16,
    pub animation_started: f32,
    pub animation_until: f32,
    pub respawn_until: f32,
    pub step_right: bool,
    pub npc: bool,
}

enum NetworkCommand {
    Walk {
        character_id: u32,
        direction: u8,
    },
    Jump {
        character_id: u32,
        target: [i32; 2],
        current: [i32; 2],
    },
    Revive {
        character_id: u32,
    },
    Mine {
        character_id: u32,
    },
    ChangeMap {
        character_id: u32,
        x: u16,
        y: u16,
    },
    Action {
        character_id: u32,
        action: u32,
    },
    Team {
        action: u32,
        target_id: u32,
    },
    Trade {
        target_id: u32,
        subtype: u32,
        data: u32,
    },
    PlayerShopAction {
        packet: ItemActionPacket,
    },
    Friend {
        action: u8,
        target_id: u32,
        target_name: String,
    },
    Guild {
        packet: GuildPacket,
    },
    Attack {
        character_id: u32,
        target_id: u32,
        x: u16,
        y: u16,
    },
    MagicAttack {
        character_id: u32,
        target_id: u32,
        x: u16,
        y: u16,
        skill_id: u32,
    },
    PickupGroundItem {
        packet: GroundItemPacket,
    },
    NpcInteract {
        npc_id: u32,
        kind: u16,
    },
    NpcDialogAnswer {
        task_id: u32,
        option_id: u8,
    },
    NpcDialogText {
        task_id: u32,
        option_id: u8,
        text: String,
    },
    BuyItem {
        character_id: u32,
        item_id: u32,
        inventory_slot: u8,
    },
    SellItem {
        character_id: u32,
        item_uid: u32,
        shop_id: u32,
    },
    DropInventoryItem {
        character_id: u32,
        item_uid: u32,
    },
    UseItem {
        character_id: u32,
        item_uid: u32,
    },
    MoveInventory {
        character_id: u32,
        item_uid: u32,
        inventory_slot: u8,
    },
    RequestItems {
        character_id: u32,
    },
    EquipInventory {
        character_id: u32,
        item_uid: u32,
        equipment_slot: u8,
    },
    Unequip {
        character_id: u32,
        item_uid: u32,
        inventory_slot: u8,
    },
    UpgradeItem {
        character_id: u32,
        item_uid: u32,
    },
    ImproveQuality {
        character_id: u32,
        item_uid: u32,
    },
    SetRarity {
        item_uid: u32,
        rarity: u8,
    },
    RerollRarity {
        item_uid: u32,
    },
    OpenSocket {
        character_id: u32,
        item_uid: u32,
    },
    ApplyRefinery {
        item_uid: u32,
        material_uid: u32,
    },
    ApplyArtifact {
        item_uid: u32,
        material_uid: u32,
    },
    SocketGem {
        item_uid: u32,
        gem_uid: u32,
        socket: u8,
        remove: bool,
    },
    WarehouseAction {
        packet: WarehouseActionPacket,
    },
    GuildWarehouseAction {
        packet: GuildWarehouseActionPacket,
    },
    Talk {
        packet: TalkPacket,
    },
    AssignAttributes {
        packet: AssignAttributesPacket,
    },
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
                | NetworkEvent::JumpConfirmed { .. }
                | NetworkEvent::MovementRejected(_)
                | NetworkEvent::Reposition { .. }
                | NetworkEvent::RemoteWalk { .. }
                | NetworkEvent::RemoteFacing { .. }
                | NetworkEvent::RemoteJump { .. }
                | NetworkEvent::RemoteAction { .. }
                | NetworkEvent::LevelUp { .. }
                | NetworkEvent::SkillForgotten(_)
                | NetworkEvent::RemoteCombat { .. }
                | NetworkEvent::SkillEffect { .. }
                | NetworkEvent::Skill { .. }
                | NetworkEvent::WeaponSkill { .. }
                | NetworkEvent::Stamina(_)
                | NetworkEvent::ActorLife { .. }
                | NetworkEvent::EquipmentItem(_)
                | NetworkEvent::EquipmentItemRemoved { .. }
                | NetworkEvent::GroundItem { .. }
                | NetworkEvent::NpcDialog(_)
                | NetworkEvent::RemoveActor(_)
                | NetworkEvent::PlayerSpawn(_)
                | NetworkEvent::NpcSpawn(_)
                | NetworkEvent::Team { .. }
                | NetworkEvent::TeamMember { .. }
                | NetworkEvent::Trade { .. }
                | NetworkEvent::TradeItem(_)
                | NetworkEvent::VendorItem { .. }
                | NetworkEvent::Guild { .. }
                | NetworkEvent::GuildMember(_) => {}
                NetworkEvent::Silver(_)
                | NetworkEvent::Cps(_)
                | NetworkEvent::Vitals { .. }
                | NetworkEvent::Attributes { .. }
                | NetworkEvent::Nobility { .. }
                | NetworkEvent::NobilityRanking(_)
                | NetworkEvent::ExperienceGain { .. }
                | NetworkEvent::InventoryItem(_)
                | NetworkEvent::InventoryItemRemoved(_)
                | NetworkEvent::WarehouseOpened(_)
                | NetworkEvent::WarehouseItem(_)
                | NetworkEvent::WarehouseItemRemoved(_)
                | NetworkEvent::WarehouseBalance(_)
                | NetworkEvent::GuildWarehouseOpened(_)
                | NetworkEvent::GuildWarehouseItem(_)
                | NetworkEvent::GuildWarehouseItemRemoved(_)
                | NetworkEvent::ChatMessage(_)
                | NetworkEvent::Notice(_)
                | NetworkEvent::Friend { .. } => {}
            }
        }
    }

    pub fn send_walk(&self, character_id: u32, direction: u8) {
        let _ = self.commands.send(NetworkCommand::Walk {
            character_id,
            direction,
        });
    }

    pub fn send_jump(&self, character_id: u32, target: [i32; 2], current: [i32; 2]) {
        let _ = self.commands.send(NetworkCommand::Jump {
            character_id,
            target,
            current,
        });
    }

    pub fn send_action(&self, character_id: u32, action: u32) {
        let _ = self.commands.send(NetworkCommand::Action {
            character_id,
            action,
        });
    }

    pub fn send_team(&self, action: u32, target_id: u32) {
        let _ = self
            .commands
            .send(NetworkCommand::Team { action, target_id });
    }

    pub fn send_trade(&self, target_id: u32, subtype: u32, data: u32) {
        let _ = self.commands.send(NetworkCommand::Trade {
            target_id,
            subtype,
            data,
        });
    }

    pub fn create_player_shop(&self, character_id: u32) {
        let _ = self.commands.send(NetworkCommand::Action {
            character_id,
            action: 111,
        });
    }

    pub fn close_player_shop(&self, character_id: u32) {
        let _ = self.commands.send(NetworkCommand::Action {
            character_id,
            action: 114,
        });
    }

    /// Asks to revive after death (MsgAction 94).
    /// Asks the server to take the hero through the portal at (x, y).
    pub fn change_map(&self, character_id: u32, x: u16, y: u16) {
        let _ = self.commands.send(NetworkCommand::ChangeMap { character_id, x, y });
    }

    pub fn mine(&self, character_id: u32) {
        let _ = self.commands.send(NetworkCommand::Mine { character_id });
    }

    pub fn revive(&self, character_id: u32) {
        let _ = self.commands.send(NetworkCommand::Revive { character_id });
    }

    pub fn player_shop_query(&self, character_id: u32, owner_id: u32) {
        let _ = self.commands.send(NetworkCommand::PlayerShopAction {
            packet: ItemActionPacket::booth_query(character_id, owner_id),
        });
    }

    pub fn player_shop_add(&self, character_id: u32, item_uid: u32, price: u32) {
        let _ = self.commands.send(NetworkCommand::PlayerShopAction {
            packet: ItemActionPacket::booth_add(character_id, item_uid, price),
        });
    }

    pub fn player_shop_remove(&self, character_id: u32, item_uid: u32) {
        let _ = self.commands.send(NetworkCommand::PlayerShopAction {
            packet: ItemActionPacket::booth_remove(character_id, item_uid),
        });
    }

    pub fn player_shop_buy(&self, character_id: u32, item_uid: u32, owner_id: u32) {
        let _ = self.commands.send(NetworkCommand::PlayerShopAction {
            packet: ItemActionPacket::booth_buy(character_id, item_uid, owner_id),
        });
    }

    pub fn send_friend_action(&self, action: u8, target_id: u32, target_name: &str) {
        let _ = self.commands.send(NetworkCommand::Friend {
            action,
            target_id,
            target_name: target_name.to_owned(),
        });
    }

    pub fn send_guild_action(&self, action: u32, data: u32) {
        let _ = self.commands.send(NetworkCommand::Guild {
            packet: GuildPacket::new(action, data),
        });
    }

    pub fn send_attack(&self, character_id: u32, target_id: u32, x: u16, y: u16) {
        let _ = self.commands.send(NetworkCommand::Attack {
            character_id,
            target_id,
            x,
            y,
        });
    }

    pub fn send_magic_attack(
        &self,
        character_id: u32,
        target_id: u32,
        x: u16,
        y: u16,
        skill_id: u32,
    ) {
        let _ = self.commands.send(NetworkCommand::MagicAttack {
            character_id,
            target_id,
            x,
            y,
            skill_id,
        });
    }

    pub fn send_pickup_ground_item(&self, uid: u32, item_id: u32, x: u16, y: u16) {
        let _ = self.commands.send(NetworkCommand::PickupGroundItem {
            packet: GroundItemPacket::pickup(uid, item_id, x, y),
        });
    }

    pub fn send_npc_interact(&self, npc_id: u32, kind: u16) {
        let _ = self
            .commands
            .send(NetworkCommand::NpcInteract { npc_id, kind });
    }

    pub fn send_npc_dialog_text(&self, task_id: u32, option_id: u8, text: String) {
        let _ = self
            .commands
            .send(NetworkCommand::NpcDialogText { task_id, option_id, text });
    }

    pub fn send_npc_dialog_answer(&self, task_id: u32, option_id: u8) {
        let _ = self
            .commands
            .send(NetworkCommand::NpcDialogAnswer { task_id, option_id });
    }

    pub fn buy_item(&self, character_id: u32, item_id: u32, inventory_slot: u8) {
        let _ = self.commands.send(NetworkCommand::BuyItem {
            character_id,
            item_id,
            inventory_slot,
        });
    }

    pub fn sell_item(&self, character_id: u32, item_uid: u32, shop_id: u32) {
        let _ = self.commands.send(NetworkCommand::SellItem {
            character_id,
            item_uid,
            shop_id,
        });
    }

    pub fn drop_inventory_item(&self, character_id: u32, item_uid: u32) {
        let _ = self.commands.send(NetworkCommand::DropInventoryItem {
            character_id,
            item_uid,
        });
    }

    pub fn use_item(&self, character_id: u32, item_uid: u32) {
        let _ = self.commands.send(NetworkCommand::UseItem {
            character_id,
            item_uid,
        });
    }

    pub fn move_inventory_item(&self, character_id: u32, item_uid: u32, inventory_slot: u8) {
        let _ = self.commands.send(NetworkCommand::MoveInventory {
            character_id,
            item_uid,
            inventory_slot,
        });
    }

    pub fn request_items(&self, character_id: u32) {
        let _ = self
            .commands
            .send(NetworkCommand::RequestItems { character_id });
    }

    pub fn equip_inventory_item(&self, character_id: u32, item_uid: u32, equipment_slot: u8) {
        let _ = self.commands.send(NetworkCommand::EquipInventory {
            character_id,
            item_uid,
            equipment_slot,
        });
    }

    pub fn unequip_item(&self, character_id: u32, item_uid: u32, inventory_slot: u8) {
        let _ = self.commands.send(NetworkCommand::Unequip {
            character_id,
            item_uid,
            inventory_slot,
        });
    }

    pub fn upgrade_item(&self, character_id: u32, item_uid: u32) {
        let _ = self.commands.send(NetworkCommand::UpgradeItem {
            character_id,
            item_uid,
        });
    }

    pub fn improve_quality(&self, character_id: u32, item_uid: u32) {
        let _ = self.commands.send(NetworkCommand::ImproveQuality {
            character_id,
            item_uid,
        });
    }

    pub fn set_item_rarity(&self, item_uid: u32, rarity: u8) {
        let _ = self
            .commands
            .send(NetworkCommand::SetRarity { item_uid, rarity });
    }

    pub fn reroll_item_rarity(&self, item_uid: u32) {
        let _ = self
            .commands
            .send(NetworkCommand::RerollRarity { item_uid });
    }

    pub fn open_socket(&self, character_id: u32, item_uid: u32) {
        let _ = self.commands.send(NetworkCommand::OpenSocket {
            character_id,
            item_uid,
        });
    }

    pub fn apply_refinery(&self, item_uid: u32, material_uid: u32) {
        let _ = self.commands.send(NetworkCommand::ApplyRefinery {
            item_uid,
            material_uid,
        });
    }

    pub fn apply_artifact(&self, item_uid: u32, material_uid: u32) {
        let _ = self.commands.send(NetworkCommand::ApplyArtifact {
            item_uid,
            material_uid,
        });
    }

    pub fn insert_gem(&self, item_uid: u32, gem_uid: u32, socket: u8) {
        let _ = self.commands.send(NetworkCommand::SocketGem {
            item_uid,
            gem_uid,
            socket,
            remove: false,
        });
    }

    pub fn remove_gem(&self, item_uid: u32, socket: u8) {
        let _ = self.commands.send(NetworkCommand::SocketGem {
            item_uid,
            gem_uid: 0,
            socket,
            remove: true,
        });
    }

    pub fn warehouse_deposit(&self, character_id: u32, npc_id: u32, item_uid: u32, position: u8) {
        let _ = self.commands.send(NetworkCommand::WarehouseAction {
            packet: WarehouseActionPacket::deposit(character_id, npc_id, item_uid, position),
        });
    }

    pub fn warehouse_withdraw(&self, character_id: u32, npc_id: u32, item_uid: u32, position: u8) {
        let _ = self.commands.send(NetworkCommand::WarehouseAction {
            packet: WarehouseActionPacket::withdraw(character_id, npc_id, item_uid, position),
        });
    }

    pub fn warehouse_silver(&self, character_id: u32, npc_id: u32, amount: u32, deposit: bool) {
        let _ = self.commands.send(NetworkCommand::WarehouseAction {
            packet: WarehouseActionPacket::silver(character_id, npc_id, amount, deposit),
        });
    }

    pub fn guild_warehouse_open(&self, character_id: u32, guild_id: u32) {
        let _ = self.commands.send(NetworkCommand::GuildWarehouseAction {
            packet: GuildWarehouseActionPacket::open(character_id, guild_id),
        });
    }

    pub fn guild_warehouse_deposit(
        &self,
        character_id: u32,
        guild_id: u32,
        item_uid: u32,
        position: u8,
    ) {
        let _ = self.commands.send(NetworkCommand::GuildWarehouseAction {
            packet: GuildWarehouseActionPacket::deposit(character_id, guild_id, item_uid, position),
        });
    }

    pub fn guild_warehouse_withdraw(
        &self,
        character_id: u32,
        guild_id: u32,
        item_uid: u32,
        position: u8,
    ) {
        let _ = self.commands.send(NetworkCommand::GuildWarehouseAction {
            packet: GuildWarehouseActionPacket::withdraw(
                character_id,
                guild_id,
                item_uid,
                position,
            ),
        });
    }

    pub fn send_chat(&self, character_id: u32, sender_name: &str, message: &str) {
        let packet = TalkPacket::public(character_id, sender_name, message);
        let _ = self.commands.send(NetworkCommand::Talk { packet });
    }

    pub fn send_whisper(
        &self,
        character_id: u32,
        sender_name: &str,
        recipient_name: &str,
        message: &str,
    ) {
        let packet = TalkPacket::whisper(character_id, sender_name, recipient_name, message);
        let _ = self.commands.send(NetworkCommand::Talk { packet });
    }

    pub fn assign_attribute(&self, attribute: &str) -> bool {
        let Some(packet) = AssignAttributesPacket::one(attribute) else {
            return false;
        };
        self.commands
            .send(NetworkCommand::AssignAttributes { packet })
            .is_ok()
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

    let (map_id, position, deferred_events) = load_location(&mut session, user.character_id)
        .await
        .map_err(|error| error.to_string())?;
    let local_character_id = user.character_id;
    let silver = user.silver;
    let health = u32::from(user.health_points);
    let maximum_health = u32::try_from(user.reserved0)
        .ok()
        .filter(|value| *value > 0)
        .unwrap_or(health.max(1));
    let mana = u32::from(user.mana_points);
    let maximum_mana = u32::try_from(user.reserved1)
        .ok()
        .filter(|value| *value > 0 || user.spirit == 0)
        .unwrap_or_else(|| max_mana_for(user.spirit, user.current_class));
    events
        .send(NetworkEvent::Ready(ReadyState {
            character_id: local_character_id,
            character_name: user.character_name,
            body: body_from_mesh(user.mesh),
            hair_style: user.hair_style,
            map_id,
            position,
            silver,
            cps: user.cps,
            health,
            maximum_health,
            mana,
            maximum_mana,
            level: user.level,
            experience: user.experience,
            strength: user.strength,
            agility: user.agility,
            vitality: user.vitality,
            spirit: user.spirit,
            attribute_points: user.attribute_points,
            current_class: user.current_class,
            previous_class: user.previous_class,
            rebirths: user.rebirths,
            kill_points: user.kill_points,
        }))
        .map_err(|error| error.to_string())?;
    for event in deferred_events {
        let _ = events.send(event);
    }
    send_status(&events, "connected");

    session
        .send(&ActionPacket::request_items(local_character_id))
        .await
        .map_err(|error| error.to_string())?;

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
                    crate::debug_log::write(format_args!(
                        "network received movement rejection: {}",
                        talk.message
                    ));
                    let _ = events.send(NetworkEvent::MovementRejected(talk.message));
                }
                Some(GameEvent::Talk(talk)) => {
                    crate::debug_log::write(format_args!(
                        "network received talk/notice: {}",
                        talk.message
                    ));
                    let message = ChatMessage {
                        channel: talk.channel,
                        sender_name: talk.sender_name,
                        recipient_name: talk.recipient_name,
                        message: talk.message,
                    };
                    let display = if message.channel == 2001 {
                        format!(
                            "Whisper from {}: {}",
                            message.sender_name, message.message
                        )
                    } else {
                        message.message.clone()
                    };
                    let _ = events.send(NetworkEvent::ChatMessage(message));
                    let _ = events.send(NetworkEvent::Notice(display));
                }
                Some(GameEvent::Team(packet)) => {
                    let _ = events.send(NetworkEvent::Team {
                        action: packet.action,
                        target_id: packet.target_id,
                    });
                }
                Some(GameEvent::TeamMember(packet)) => {
                    let action = packet.action;
                    let _ = events.send(NetworkEvent::TeamMember {
                        member: team_member(packet),
                        action,
                    });
                }
                Some(GameEvent::Trade(packet)) => {
                    let _ = events.send(NetworkEvent::Trade {
                        target_id: packet.target,
                        subtype: packet.subtype,
                        data: packet.data,
                    });
                }
                Some(GameEvent::VendorItem(packet)) => {
                    let _ = events.send(NetworkEvent::VendorItem {
                        owner_id: packet.owner_id,
                        price: packet.price,
                        item: OwnedItem {
                            uid: packet.item_uid,
                            item_type: packet.item_type,
                            position: packet.position,
                            durability: packet.durability,
                            max_durability: packet.max_durability,
                            plus: packet.plus,
                            rarity: 0,
                            gem_one: packet.gem_one,
                            gem_two: packet.gem_two,
                            rarity_roll: 0,
                            bless: 0,
                            enchant_hp: 0,
                            color: 0,
                        },
                    });
                }
                Some(GameEvent::Guild(packet)) => {
                    let gravak_protocol::GuildPacket {
                        action,
                        data,
                        strings,
                        ..
                    } = packet;
                    let name = Vec::<String>::from(strings).into_iter().next();
                    let _ = events.send(NetworkEvent::Guild {
                        action,
                        data,
                        name,
                    });
                }
                Some(GameEvent::GuildMember(packet)) => {
                    let _ = events.send(NetworkEvent::GuildMember(GuildMember {
                        name: packet.name.to_string(),
                        character_id: packet.reserved16,
                        mesh: packet.reserved20,
                        level: packet.level,
                        rank: packet.rank,
                        donation: packet.donation,
                        online: packet.online != 0,
                    }));
                }
                Some(GameEvent::Friend(packet)) => {
                    let _ = events.send(NetworkEvent::Friend {
                        friend_id: packet.friend_id,
                        action: packet.action,
                        name: packet.name.to_string(),
                        online: packet.online_flag != 0,
                    });
                }
                Some(GameEvent::UserInfo(info)) if info.character_id == local_character_id => {
                    let _ = events.send(NetworkEvent::Silver(info.silver));
                    let _ = events.send(NetworkEvent::Cps(info.cps));
                    let _ = events.send(NetworkEvent::Vitals {
                        health: u32::from(info.health_points),
                        maximum_health: u32::try_from(info.reserved0)
                            .ok()
                            .filter(|value| *value > 0)
                            .unwrap_or_else(|| u32::from(info.health_points).max(1)),
                        mana: u32::from(info.mana_points),
                        maximum_mana: u32::try_from(info.reserved1)
                            .ok()
                            .filter(|value| *value > 0 || info.spirit == 0)
                            .unwrap_or_else(|| max_mana_for(info.spirit, info.current_class)),
                        level: info.level,
                        experience: info.experience,
                    });
                    let _ = events.send(NetworkEvent::Attributes {
                        strength: info.strength,
                        agility: info.agility,
                        vitality: info.vitality,
                        spirit: info.spirit,
                        attribute_points: info.attribute_points,
                        current_class: info.current_class,
                        previous_class: info.previous_class,
                        rebirths: info.rebirths,
                        kill_points: info.kill_points,
                        spouse: info.spouse.clone(),
                    });
                }
                Some(GameEvent::Nobility(info)) if info.character_id == local_character_id => {
                    let _ = events.send(NetworkEvent::Nobility {
                        character_id: info.character_id,
                        donation: info.donation,
                        rank: info.rank,
                        remaining: info.remaining,
                        title_id: info.title_id,
                    });
                }
                Some(GameEvent::NobilityRanking(info)) if info.action == 1 => {
                    let _ = events.send(NetworkEvent::NobilityRanking(NobilityEntry {
                        position: info.position,
                        character_id: info.character_id,
                        name: info.character_name,
                        donation: info.donation,
                        rank: info.rank,
                    }));
                }
                Some(GameEvent::ItemInfo(info)) if info.action == 6 && info.position == 80 => {
                    crate::debug_log::write("network received Guild warehouse open");
                    let _ = events.send(NetworkEvent::GuildWarehouseOpened(info.item_uid));
                }
                Some(GameEvent::ItemInfo(info)) if info.position >= 80 && info.action == 1 => {
                    crate::debug_log::write(format_args!(
                        "network received Guild warehouse item: uid={} type={} position={} action={}",
                        info.item_uid, info.item_type, info.position, info.action
                    ));
                    let _ = events.send(NetworkEvent::GuildWarehouseItem(OwnedItem {
                        uid: info.item_uid,
                        item_type: info.item_type,
                        position: info.position - 80,
                        durability: info.durability,
                        max_durability: info.max_durability,
                        plus: info.plus,
                        rarity: info.bless,
                        gem_one: info.gem_one,
                        gem_two: info.gem_two,
                        rarity_roll: info.reserved3 as u8,
                        bless: info.reserved2,
                        enchant_hp: info.enchant,
                        color: info.reserved4 as u8,
                    }));
                }
                Some(GameEvent::ItemInfo(info)) if info.position >= 80 && info.action == 5 => {
                    crate::debug_log::write(format_args!(
                        "network received Guild warehouse removal: uid={} position={} action={}",
                        info.item_uid, info.position, info.action
                    ));
                    let _ = events.send(NetworkEvent::GuildWarehouseItemRemoved(info.item_uid));
                }
                Some(GameEvent::ItemInfo(info)) if info.action == 6 && info.position == 40 => {
                    crate::debug_log::write(format_args!(
                        "network received warehouse open: uid={} position={} action={}",
                        info.item_uid, info.position, info.action
                    ));
                    let _ = events.send(NetworkEvent::WarehouseOpened(info.item_uid));
                }
                Some(GameEvent::ItemInfo(info)) if info.action == 7 && info.position == 40 => {
                    crate::debug_log::write(format_args!(
                        "network received warehouse balance: value={} action={}",
                        info.reserved1, info.action
                    ));
                    let _ = events.send(NetworkEvent::WarehouseBalance(info.reserved1));
                }
                Some(GameEvent::ItemInfo(info)) if info.position >= 40 && info.action == 1 => {
                    crate::debug_log::write(format_args!(
                        "network received warehouse item: uid={} type={} position={} action={}",
                        info.item_uid, info.item_type, info.position, info.action
                    ));
                    let _ = events.send(NetworkEvent::WarehouseItem(OwnedItem {
                        uid: info.item_uid,
                        item_type: info.item_type,
                        position: info.position - 40,
                        durability: info.durability,
                        max_durability: info.max_durability,
                        plus: info.plus,
                        rarity: info.bless,
                        gem_one: info.gem_one,
                        gem_two: info.gem_two,
                        rarity_roll: info.reserved3 as u8,
                        bless: info.reserved2,
                        enchant_hp: info.enchant,
                        color: info.reserved4 as u8,
                    }));
                }
                Some(GameEvent::ItemInfo(info)) if info.position >= 40 && info.action == 5 => {
                    crate::debug_log::write(format_args!(
                        "network received warehouse removal: uid={} position={} action={}",
                        info.item_uid, info.position, info.action
                    ));
                    let _ = events.send(NetworkEvent::WarehouseItemRemoved(info.item_uid));
                }
                Some(GameEvent::ItemInfo(info)) if info.action == 8 => {
                    crate::debug_log::write(format_args!(
                        "network received equipment item: uid={} type={} position={} action={}",
                        info.item_uid, info.item_type, info.position, info.action
                    ));
                    let _ = events.send(NetworkEvent::EquipmentItem(OwnedItem {
                        uid: info.item_uid,
                        item_type: info.item_type,
                        position: info.position,
                        durability: info.durability,
                        max_durability: info.max_durability,
                        plus: info.plus,
                        rarity: info.bless,
                        gem_one: info.gem_one,
                        gem_two: info.gem_two,
                        rarity_roll: info.reserved3 as u8,
                        bless: info.reserved2,
                        enchant_hp: info.enchant,
                        color: info.reserved4 as u8,
                    }));
                }
                Some(GameEvent::ItemInfo(info)) if info.action == 9 => {
                    crate::debug_log::write(format_args!(
                        "network received equipment removal: uid={} slot={} action={}",
                        info.item_uid, info.position, info.action
                    ));
                    let _ = events.send(NetworkEvent::EquipmentItemRemoved {
                        uid: info.item_uid,
                        slot: info.position,
                    });
                }
                Some(GameEvent::ItemInfo(info)) if info.action == 2 => {
                    crate::debug_log::write(format_args!(
                        "network received trade preview: uid={} type={} position={}",
                        info.item_uid, info.item_type, info.position
                    ));
                    let _ = events.send(NetworkEvent::TradeItem(OwnedItem {
                        uid: info.item_uid,
                        item_type: info.item_type,
                        position: info.position,
                        durability: info.durability,
                        max_durability: info.max_durability,
                        plus: info.plus,
                        rarity: info.bless,
                        gem_one: info.gem_one,
                        gem_two: info.gem_two,
                        rarity_roll: info.reserved3 as u8,
                        bless: info.reserved2,
                        enchant_hp: info.enchant,
                        color: info.reserved4 as u8,
                    }));
                }
                Some(GameEvent::ItemInfo(info)) if info.action == 1 => {
                    crate::debug_log::write(format_args!(
                        "network received inventory item: uid={} type={} position={} action={}",
                        info.item_uid, info.item_type, info.position, info.action
                    ));
                    let _ = events.send(NetworkEvent::InventoryItem(OwnedItem {
                        uid: info.item_uid,
                        item_type: info.item_type,
                        position: info.position,
                        durability: info.durability,
                        max_durability: info.max_durability,
                        plus: info.plus,
                        rarity: info.bless,
                        gem_one: info.gem_one,
                        gem_two: info.gem_two,
                        rarity_roll: info.reserved3 as u8,
                        bless: info.reserved2,
                        enchant_hp: info.enchant,
                        color: info.reserved4 as u8,
                    }));
                }
                Some(GameEvent::ItemInfo(info)) if info.action == 5 => {
                    crate::debug_log::write(format_args!(
                        "network received inventory removal: uid={} position={} action={}",
                        info.item_uid, info.position, info.action
                    ));
                    let _ = events.send(NetworkEvent::InventoryItemRemoved(info.item_uid));
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
                    if action.action_type == 109 && action.character_id == local_character_id =>
                {
                    let _ = events.send(NetworkEvent::SkillForgotten(action.data1));
                }
                Some(GameEvent::Action(action))
                    if action.action_type == 92 && action.character_id == local_character_id =>
                {
                    let _ = events.send(NetworkEvent::LevelUp {
                        level: action.details as u8,
                    });
                }
                Some(GameEvent::Action(action)) if action.action_type == 6 => {
                    crate::debug_log::write(format_args!(
                        "action type 6 received: char={}, target={}, data1={}, data2={}, details={}",
                        action.character_id,
                        action.data1,
                        action.data2,
                        action.details,
                        action.action_type
                    ));
                    if action.character_id == local_character_id {
                        let _ = events.send(NetworkEvent::ExperienceGain {
                            experience: action.data1 as u64,
                        });
                    }
                }
                Some(GameEvent::Action(action)) if action.action_type != 86 && action.action_type != 132 && action.action_type != 92 && action.action_type != 79 && action.action_type != 133 && action.action_type != 81 && action.action_type != 131 && action.action_type != 6 => {
                    crate::debug_log::write(format_args!(
                        "unhandled action type: char={}, type={}, data1={}, data2={}, details={}",
                        action.character_id, action.action_type, action.data1, action.data2, action.details
                    ));
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
                    if action.action_type == 133 && action.character_id == local_character_id =>
                {
                    let _ = events.send(NetworkEvent::JumpConfirmed {
                        character_id: action.character_id,
                        position: unpack_position(action.data1),
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
                Some(GameEvent::Action(action))
                    if action.action_type == 131 && action.character_id != local_character_id =>
                {
                    let _ = events.send(NetworkEvent::RemoteAction {
                        id: action.character_id,
                        action: 131,
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
                Some(GameEvent::Interaction(interaction)) => {
                    let _ = events.send(NetworkEvent::RemoteCombat {
                        id: interaction.character_id,
                        target_id: interaction.target_id,
                        action: interaction.action,
                        value: interaction.value,
                    });
                }
                Some(GameEvent::SkillEffect(effect)) => {
                    for (target_id, damage) in effect.targets {
                        let _ = events.send(NetworkEvent::SkillEffect {
                            attacker_id: effect.attacker_id,
                            target_id,
                            skill_id: effect.skill_id,
                            damage,
                        });
                    }
                }
                Some(GameEvent::Skill(skill)) => {
                    let _ = events.send(NetworkEvent::Skill {
                        skill_id: skill.skill_id,
                        level: skill.level,
                        experience: skill.experience,
                    });
                }
                Some(GameEvent::UserAttrib(packet))
                    if packet.character_id == local_character_id
                        && packet.attribute == gravak_protocol::UserAttribPacket::STAMINA =>
                {
                    let _ = events.send(NetworkEvent::Stamina(packet.value.min(255) as u8));
                }
                Some(GameEvent::UserAttrib(packet))
                    if packet.character_id != local_character_id && packet.attribute <= 1 =>
                {
                    let _ = events.send(NetworkEvent::ActorLife {
                        id: packet.character_id,
                        maximum: packet.attribute == 1,
                        value: packet.value.min(u64::from(u32::MAX)) as u32,
                    });
                }
                Some(GameEvent::WeaponSkill(packet)) => {
                    let _ = events.send(NetworkEvent::WeaponSkill {
                        weapon_type: packet.weapon_type,
                        level: packet.level,
                        experience: packet.experience,
                    });
                }
                Some(GameEvent::RefineryInfo(info)) => {
                    crate::debug_log::write(format_args!(
                        "network received refinery info: records={}",
                        info.records.len()
                    ));
                }
                Some(GameEvent::GroundItem(item)) => {
                    let _ = events.send(NetworkEvent::GroundItem {
                        uid: item.uid,
                        item_id: item.item_id,
                        position: [f32::from(item.x), f32::from(item.y)],
                        drop_type: item.drop_type,
                    });
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
                Some(NetworkCommand::Jump {
                    character_id,
                    target,
                    current,
                }) => {
                    session
                        .send(&ActionPacket::jump(character_id, target, current))
                        .await
                        .map_err(|error| error.to_string())?;
                }
                Some(NetworkCommand::ChangeMap { character_id, x, y }) => {
                    session
                        .send(&ActionPacket::change_map(character_id, x, y))
                        .await
                        .map_err(|error| error.to_string())?;
                }
                Some(NetworkCommand::Mine { character_id }) => {
                    session
                        .send(&ActionPacket::mine(character_id))
                        .await
                        .map_err(|error| error.to_string())?;
                }
                Some(NetworkCommand::Revive { character_id }) => {
                    session
                        .send(&ActionPacket::reborn(character_id))
                        .await
                        .map_err(|error| error.to_string())?;
                }
                Some(NetworkCommand::Action { character_id, action }) => {
                    session
                        .send(&ActionPacket::change_action(character_id, action))
                        .await
                        .map_err(|error| error.to_string())?;
                }
                Some(NetworkCommand::Team { action, target_id }) => {
                    session
                        .send(&TeamActionPacket::new(action, target_id))
                        .await
                        .map_err(|error| error.to_string())?;
                }
                Some(NetworkCommand::Trade {
                    target_id,
                    subtype,
                    data,
                }) => {
                    session
                        .send(&TradePacket::new(target_id, subtype, data))
                        .await
                        .map_err(|error| error.to_string())?;
                }
                Some(NetworkCommand::PlayerShopAction { packet }) => {
                    session.send(&packet).await.map_err(|error| error.to_string())?;
                }
                Some(NetworkCommand::Friend {
                    action,
                    target_id,
                    target_name,
                }) => {
                    session
                        .send(&FriendPacket::new(action, target_id, &target_name, true))
                        .await
                        .map_err(|error| error.to_string())?;
                }
                Some(NetworkCommand::Guild { packet }) => {
                    session.send(&packet).await.map_err(|error| error.to_string())?;
                }
                Some(NetworkCommand::PickupGroundItem { packet }) => {
                    session.send(&packet).await.map_err(|error| error.to_string())?;
                }
                Some(NetworkCommand::Attack { character_id, target_id, x, y }) => {
                        session
                        .send(&InteractionPacket::attack(character_id, target_id, x, y))
                        .await
                        .map_err(|error| error.to_string())?;
                    }
                Some(NetworkCommand::MagicAttack { character_id, target_id, x, y, skill_id }) => {
                    session
                        .send(&InteractionPacket::magic_attack(character_id, target_id, x, y, skill_id))
                        .await
                        .map_err(|error| error.to_string())?;
                }
                Some(NetworkCommand::NpcInteract { npc_id, kind }) => {
                    session
                        .send(&NpcPacket::activate(npc_id, kind))
                        .await
                        .map_err(|error| error.to_string())?;
                }
                Some(NetworkCommand::NpcDialogText { task_id, option_id, text }) => {
                    session
                        .send(&NpcDialogPacket::answer_text(task_id, option_id, &text))
                        .await
                        .map_err(|error| error.to_string())?;
                }
                Some(NetworkCommand::NpcDialogAnswer { task_id, option_id }) => {
                    session
                        .send(&NpcDialogPacket::answer(task_id, option_id))
                        .await
                        .map_err(|error| error.to_string())?;
                }
                Some(NetworkCommand::BuyItem { character_id, item_id, inventory_slot }) => {
                    session
                        .send(&ItemActionPacket::buy(character_id, item_id, inventory_slot))
                        .await
                        .map_err(|error| error.to_string())?;
                }
                Some(NetworkCommand::SellItem { character_id, item_uid, shop_id }) => {
                        session
                        .send(&ItemActionPacket::sell(character_id, item_uid, shop_id))
                        .await
                        .map_err(|error| error.to_string())?;
                }
                Some(NetworkCommand::DropInventoryItem { character_id, item_uid }) => {
                    crate::debug_log::write(format_args!(
                        "network send drop: character_id={} uid={}",
                        character_id, item_uid
                    ));
                    session
                        .send(&ItemActionPacket::drop(character_id, item_uid))
                        .await
                        .map_err(|error| error.to_string())?;
                    crate::debug_log::write("network send drop completed");
                }
                Some(NetworkCommand::MoveInventory { character_id, item_uid, inventory_slot }) => {
                    crate::debug_log::write(format_args!(
                        "network send move: character_id={} uid={} target_slot={}",
                        character_id, item_uid, inventory_slot
                    ));
                    session
                        .send(&ItemActionPacket::move_inventory(
                            character_id,
                            item_uid,
                            inventory_slot,
                        ))
                        .await
                        .map_err(|error| error.to_string())?;
                    crate::debug_log::write("network send move completed");
                }
                Some(NetworkCommand::RequestItems { character_id }) => {
                    crate::debug_log::write(format_args!(
                        "network request inventory snapshot: character_id={character_id}"
                    ));
                    session
                        .send(&ActionPacket::request_items(character_id))
                        .await
                        .map_err(|error| error.to_string())?;
                }
                Some(NetworkCommand::EquipInventory { character_id, item_uid, equipment_slot }) => {
                    session
                        .send(&ItemActionPacket::equip(character_id, item_uid, equipment_slot))
                        .await
                        .map_err(|error| error.to_string())?;
                }
                Some(NetworkCommand::UseItem { character_id, item_uid }) => {
                    crate::debug_log::write(format_args!(
                        "network send use item: character_id={} uid={}",
                        character_id, item_uid
                    ));
                    session
                        .send(&ItemActionPacket::use_item(character_id, item_uid))
                        .await
                        .map_err(|error| error.to_string())?;
                }
                Some(NetworkCommand::Unequip { character_id, item_uid, inventory_slot }) => {
                    session
                        .send(&ItemActionPacket::unequip(character_id, item_uid, inventory_slot))
                        .await
                        .map_err(|error| error.to_string())?;
                }
                Some(NetworkCommand::UpgradeItem { character_id, item_uid }) => {
                    session
                        .send(&ItemActionPacket::upgrade(character_id, item_uid))
                        .await
                        .map_err(|error| error.to_string())?;
                }
                Some(NetworkCommand::ImproveQuality { character_id, item_uid }) => {
                    session
                        .send(&ItemActionPacket::improve_quality(character_id, item_uid))
                        .await
                        .map_err(|error| error.to_string())?;
                }
                Some(NetworkCommand::SetRarity { item_uid, rarity }) => {
                    session
                        .send(&ItemRarityPacket::set(item_uid, rarity))
                        .await
                        .map_err(|error| error.to_string())?;
                }
                Some(NetworkCommand::RerollRarity { item_uid }) => {
                    session
                        .send(&ItemRarityRerollPacket::new(item_uid))
                        .await
                        .map_err(|error| error.to_string())?;
                }
                Some(NetworkCommand::OpenSocket { character_id, item_uid }) => {
                    session
                        .send(&ItemActionPacket::open_socket(character_id, item_uid))
                        .await
                        .map_err(|error| error.to_string())?;
                }
                Some(NetworkCommand::ApplyRefinery { item_uid, material_uid }) => {
                    session
                        .send(&RefineryPacket::apply(item_uid, material_uid))
                        .await
                        .map_err(|error| error.to_string())?;
                }
                Some(NetworkCommand::ApplyArtifact { item_uid, material_uid }) => {
                    session
                        .send(&RefineryPacket::apply_artifact(item_uid, material_uid))
                        .await
                        .map_err(|error| error.to_string())?;
                }
                Some(NetworkCommand::SocketGem { item_uid, gem_uid, socket, remove }) => {
                    let packet = if remove {
                        SocketGemPacket::remove(item_uid, socket)
                    } else {
                        SocketGemPacket::insert(item_uid, gem_uid, socket)
                    };
                    session.send(&packet).await.map_err(|error| error.to_string())?;
                }
                Some(NetworkCommand::WarehouseAction { packet }) => {
                    session.send(&packet).await.map_err(|error| error.to_string())?;
                }
                Some(NetworkCommand::GuildWarehouseAction { packet }) => {
                    session.send(&packet).await.map_err(|error| error.to_string())?;
                }
                Some(NetworkCommand::Talk { packet }) => {
                    session.send(&packet).await.map_err(|error| error.to_string())?;
                }
                Some(NetworkCommand::AssignAttributes { packet }) => {
                    session.send(&packet).await.map_err(|error| error.to_string())?;
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
        movement_from: [player.x as f32, player.y as f32],
        movement_to: [player.x as f32, player.y as f32],
        movement_started: 0.0,
        movement_until: 0.0,
        direction: player.direction,
        mesh: player.mesh.max(0) as u32,
        body: body_from_mesh(player.mesh.max(0) as u32),
        // The dyes ride in reserved1: armor, helmet, shield (one byte each).
        armor: crate::character::dyed(player.armor.max(0) as u32, (player.reserved1 & 0xff) as u8),
        helmet: crate::character::dyed(player.helment.max(0) as u32, ((player.reserved1 >> 8) & 0xff) as u8),
        hair_style: player.hair_style.max(0) as u16,
        right_weapon: player.right_hand.max(0) as u32,
        left_weapon: crate::character::dyed(player.left_hand.max(0) as u32, ((player.reserved1 >> 16) & 0xff) as u8),
        action: player.action,
        health: player.health_points,
        maximum_health: player.health_points,
        status_flags: player.status_flags.max(0) as u64,
        npc_kind: 0,
        animation_started: 0.0,
        animation_until: 0.0,
        respawn_until: 0.0,
        step_right: false,
        npc: false,
    }
}

fn team_member(packet: TeamMemberPacket) -> TeamMember {
    TeamMember {
        member_id: packet.member_id,
        name: packet.name.to_string(),
        mesh: packet.mesh,
        health: packet.health,
        maximum_health: packet.max_health,
    }
}

fn npc_actor(npc: ServerNpcInfo) -> RemoteActor {
    RemoteActor {
        id: npc.id,
        name: npc.name.unwrap_or_default(),
        position: [npc.x as f32, npc.y as f32],
        movement_from: [npc.x as f32, npc.y as f32],
        movement_to: [npc.x as f32, npc.y as f32],
        movement_started: 0.0,
        movement_until: 0.0,
        direction: 0,
        mesh: npc.look as u32,
        body: 0,
        armor: 0,
        helmet: 0,
        hair_style: 0,
        right_weapon: 0,
        left_weapon: 0,
        action: 0,
        health: 0,
        maximum_health: 0,
        status_flags: 0,
        npc_kind: npc.kind,
        animation_started: 0.0,
        animation_until: 0.0,
        respawn_until: 0.0,
        step_right: false,
        npc: true,
    }
}

pub fn body_from_mesh(mesh: u32) -> u32 {
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

/// Reads packets until the map arrives. Skill and weapon proficiency
/// packets sent during login are kept and returned, so they are delivered
/// after `Ready` instead of being lost.
async fn load_location(
    session: &mut GameSession,
    _character_id: u32,
) -> Result<(u32, [i32; 2], Vec<NetworkEvent>), gravak_protocol::ProtocolError> {
    let mut location = None;
    let mut deferred = Vec::new();
    let map_id = loop {
        match session.next_event().await? {
            Some(GameEvent::Action(action)) => {
                location = Some(unpack_position(action.data2));
            }
            Some(GameEvent::MapInfo(info)) => break info.map_id,
            Some(GameEvent::Skill(skill)) => deferred.push(NetworkEvent::Skill {
                skill_id: skill.skill_id,
                level: skill.level,
                experience: skill.experience,
            }),
            Some(GameEvent::WeaponSkill(packet)) => deferred.push(NetworkEvent::WeaponSkill {
                weapon_type: packet.weapon_type,
                level: packet.level,
                experience: packet.experience,
            }),
            Some(GameEvent::UserAttrib(packet))
                if packet.attribute == gravak_protocol::UserAttribPacket::STAMINA =>
            {
                deferred.push(NetworkEvent::Stamina(packet.value.min(255) as u8))
            }
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
        deferred,
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
