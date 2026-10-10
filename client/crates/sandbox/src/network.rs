use gravak_protocol::{
    ActionPacket, AssignAttributesPacket, AuthResponse, FriendPacket, GameEvent, GameSession,
    GroundItemPacket, GuildPacket, GuildWarehouseActionPacket, InteractionPacket, ItemActionPacket,
    ItemRarityPacket, ItemRarityRerollPacket, NpcDialogPacket, NpcPacket, RefineryPacket,
    RegisterCharacter, ServerNpcInfo, ServerPlayer, SocketGemPacket, TalkPacket, TaskDialog,
    TeamActionPacket, TeamMemberPacket, TradePacket, UserInfo, WarehouseActionPacket, authenticate,
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
    /// The hero's look with the avatar, for their face.
    pub mesh: u32,
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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
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
    /// The rarity affixes, packed (kind in the top four bits, value in the
    /// low twelve), slot by slot; 0 where a slot is empty.
    pub affixes: [u16; 4],
    /// Blessing: 1, 3, 5 or 7 percent less damage taken.
    pub bless: u8,
    /// Life added by a WuxingOven enchantment.
    pub enchant_hp: u8,
    /// Tinter dye (3-9; 0 is the default).
    pub color: u8,
    /// A weapon's skin: the item type whose look it wears (0 its own).
    pub skin: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PlayerShopItem {
    pub owner_id: u32,
    pub price: u32,
    /// Priced in CPs rather than silver.
    pub cps: bool,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TradeAmount {
    Silver,
    Cps,
}

#[derive(Debug, Default, Clone)]
pub struct TradeView {
    pub active: bool,
    pub peer_id: Option<u32>,
    pub pending_request: Option<u32>,
    pub own_items: Vec<OwnedItem>,
    pub peer_items: Vec<OwnedItem>,
    pub own_silver: u32,
    pub peer_silver: u32,
    pub own_cps: u32,
    pub peer_cps: u32,
    /// What is typed in the own silver and CP boxes; sent on Enter, the
    /// confirm-amount button, or Agree.
    pub silver_input: String,
    pub cps_input: String,
    /// The amount box holding the keyboard, if any.
    pub editing: Option<TradeAmount>,
    pub own_confirmed: bool,
    pub peer_confirmed: bool,
    /// The partner's (or requester's) name, when the client knows it.
    pub peer_name: String,
}

impl TradeView {
    /// The text an own amount box shows: what is being typed, else the
    /// amount on offer.
    pub fn amount_text(&self, amount: TradeAmount) -> String {
        let (input, offered) = match amount {
            TradeAmount::Silver => (&self.silver_input, self.own_silver),
            TradeAmount::Cps => (&self.cps_input, self.own_cps),
        };
        if self.editing == Some(amount) {
            format!("{input}_")
        } else if input.is_empty() {
            offered.to_string()
        } else {
            input.clone()
        }
    }
}

#[derive(Debug)]
pub enum NetworkEvent {
    Status(String),
    Ready(ReadyState),
    Silver(u32),
    Cps(u32),
    Stamina(u8),
    /// The XP circle's points.
    XpPoints(u8),
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
        position: u32,
        top_flags: u32,
    },
    NobilityRanking(NobilityEntry),
    /// The ranking is about to be sent again, whole.
    NobilityRankingClear,
    /// The bounty Task Board.
    Bounty(gravak_protocol::BountyState),
    /// A line of the quest log, or its clear.
    QuestLog(gravak_protocol::QuestLogPacket),
    /// The Guild System window's header, and a row of one of its lists.
    GuildPanelInfo(gravak_protocol::GuildPanelInfo),
    GuildPanelRow(gravak_protocol::GuildPanelRow),
    /// The guilds at war with or allied to the hero's.
    GuildRelations(gravak_protocol::GuildRelations),
    /// One product of the Store catalog.
    StoreProduct(gravak_protocol::StoreProduct),
    /// The buyer's CPs and VIP.
    StoreState(gravak_protocol::StoreState),
    /// One of the buyer's latest purchases.
    StorePurchase(gravak_protocol::StorePurchase),
    /// One choice of the Reward Shrine's cycle.
    DailyRewardChoice(gravak_protocol::DailyRewardChoice),
    /// Where the hero stands in the cycle: the Reward Wall opens on it.
    DailyRewardState(gravak_protocol::DailyRewardState),
    /// Where the team's Rift run stands.
    Rift(gravak_protocol::RiftState),
    /// One ranked character of the Highscores.
    HighscoreRow(gravak_protocol::HighscoreRow),
    WhisperCard(gravak_protocol::WhisperCard),
    /// The Highscores page shown: the window opens on it.
    HighscoreState(gravak_protocol::HighscoreState),
    /// When each of the hero's statuses ends.
    StatusTimers(gravak_protocol::StatusTimers),
    /// A `3DEffect.ini` effect played once on an actor (fireworks...).
    /// `amount`: what a potion filled, shown rising over the actor.
    RoleEffect { actor_id: u32, effect: String, amount: Option<u32> },
    /// The answer to View Equipment (MsgAction 117): whom, their level and
    /// nobility rank; their items follow.
    ViewedPlayer { id: u32, level: u16, nobility_rank: u32 },
    /// An item the viewed player wears (MsgItemInfo action 4).
    ViewedItem(OwnedItem),
    /// An entity's spouse (MsgName 6).
    Spouse { id: u32, name: String },
    /// Kills counted toward the bounty under way.
    BountyProgress {
        done: u32,
        required: u32,
    },
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
        cps: bool,
        item: OwnedItem,
    },
    /// A booth listing gone (sold, or taken back): MsgItem 23 from the server.
    BoothItemGone(u32),
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
    /// The server confirmed the local hero's kill mode (MsgAction 96).
    KillMode(u8),
    RemoteJump {
        id: u32,
        position: [i32; 2],
        direction: u8,
    },
    /// A ping (MsgItem 27) came back carrying the client clock it left at.
    Pong(u32),
    /// Seconds until the Travel Cube travels again (MsgItem 9001, ours).
    TravelCubeWait(u32),
    /// Where a teammate stands (MsgAction 106); `map_id` is 0 when it is
    /// not on the local hero's map.
    TeamMemberPosition {
        id: u32,
        map_id: u32,
        position: [i32; 2],
    },
    RemoteAction {
        id: u32,
        action: u16,
    },
    LevelUp {
        level: u8,
    },
    /// The server's answer to the exit's Exercise: whether the hero went
    /// to offline training.
    Exercised(bool),
    /// A city artisan opened the forge window (MsgAction OpenDialog 9020).
    OpenForge,
    /// The forge window's upgrade came back: the item, the action, and 0
    /// failed, 1 upgraded, 2 refused.
    ForgeAnswer {
        item_uid: u32,
        outcome: u32,
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
        /// The caster's level of the skill (the server's, from 1).
        level: u16,
        damage: u32,
        /// The cell the cast was aimed at.
        target_cell: (u16, u16),
        /// The first event of a cast, which draws its trail.
        first: bool,
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
/// The Fly skill's status: the hero hovers.
pub const FLY_FLAG: u64 = 1 << 27;
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
    pub action: u16,
    pub health: u16,
    pub maximum_health: u16,
    pub status_flags: u64,
    pub npc_kind: u16,
    pub animation_started: f32,
    pub animation_until: f32,
    pub respawn_until: f32,
    pub step_right: bool,
    pub npc: bool,
    /// Player level from MsgPlayer; 0 for NPCs.
    pub level: u16,
    /// The name over the player's booth while it is open.
    pub booth: Option<String>,
    /// Their guild and rank, written over their name; empty without one.
    pub guild: String,
    pub guild_rank: String,
    pub guild_id: u32,
    /// The war shield beside their name (0 none): see `crate::labels::Shield`.
    pub shield: u8,
    /// KOs of their Cyclone or Superman, shown over their head.
    pub ko_count: u32,
    /// The current movement is a jump: they stay in the air until it
    /// lands, even when they shoot or are hit on the way.
    pub jumping: bool,
    /// How a slain monster dies: 0 falling, 1 flung away, 2 flung a short
    /// way (Conquer.exe 0x4d687a).
    pub flung: u8,
    /// The Top titles a player holds (`top_title::PROFESSIONS` flags).
    pub top_flags: u32,
    /// The rarity of the weapon in each hand (right, left): Legendary and
    /// Mythic glow.
    pub weapon_rarity: [u8; 2],
    /// A player's nobility rank (the server's id; 0 none).
    pub nobility_rank: u32,
}

pub(crate) enum NetworkCommand {
    /// A Store request.
    Broadcast { message: String },
    Store {
        action: u32,
        param: u32,
    },
    /// A Reward Shrine request.
    DailyReward {
        action: u32,
        param: u32,
    },
    /// A Highscores page.
    Highscores {
        category: u32,
        profession: u32,
        page: u32,
    },
    /// The Nobility window's Donate, in silver or in CPs.
    NobilityDonate {
        amount: u64,
        in_cps: bool,
    },
    /// The nobility ranking, asked for.
    NobilityRanking,
    /// The hero's own nobility, asked for.
    NobilityStatus,
    /// A bounty Task Board action.
    Bounty {
        action: u32,
        param: u32,
    },
    /// A request of the Guild System window.
    GuildPanel(gravak_protocol::GuildPanelPacket),
    Walk {
        character_id: u32,
        direction: u8,
        /// Sent as the movement type so other screens play the run cycle.
        running: bool,
    },
    Jump {
        character_id: u32,
        target: [i32; 2],
        current: [i32; 2],
    },
    Face {
        character_id: u32,
        direction: u8,
        current: [i32; 2],
    },
    Revive {
        character_id: u32,
        here: bool,
    },
    Exercise {
        character_id: u32,
    },
    Artisan {
        character_id: u32,
        item_uid: u32,
        action: u32,
    },
    Mine {
        character_id: u32,
    },
    CancelFly {
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
    Booth {
        character_id: u32,
        open: bool,
    },
    QueryTeamMember {
        character_id: u32,
        member_id: u32,
    },
    QueryEquipment {
        character_id: u32,
        target_id: u32,
    },
    Ping {
        character_id: u32,
        timestamp: u32,
    },
    KillMode {
        character_id: u32,
        mode: u8,
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
    RepairItem {
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
        /// What is chosen with it (the Travel Cube's city, 1 and up).
        choice: u32,
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
        slot: u8,
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

    /// A handle plus the network-side ends of its channels, for a session
    /// driven by [`play`] on another thread.
    pub(crate) fn channels() -> (Self, Sender<NetworkEvent>, UnboundedReceiver<NetworkCommand>) {
        let (events_tx, events) = mpsc::channel();
        let (commands, commands_rx) = tokio::sync::mpsc::unbounded_channel();
        (Self { events, commands }, events_tx, commands_rx)
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
                | NetworkEvent::KillMode(_)
                | NetworkEvent::RemoteJump { .. }
                | NetworkEvent::RemoteAction { .. }
                | NetworkEvent::LevelUp { .. }
                | NetworkEvent::Exercised(_)
                | NetworkEvent::OpenForge
                | NetworkEvent::ForgeAnswer { .. }
                | NetworkEvent::SkillForgotten(_)
                | NetworkEvent::RemoteCombat { .. }
                | NetworkEvent::SkillEffect { .. }
                | NetworkEvent::Skill { .. }
                | NetworkEvent::WeaponSkill { .. }
                | NetworkEvent::Stamina(_)
                | NetworkEvent::XpPoints(_)
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
                | NetworkEvent::TeamMemberPosition { .. }
                | NetworkEvent::Pong(_)
                | NetworkEvent::TravelCubeWait(_)
                | NetworkEvent::Trade { .. }
                | NetworkEvent::TradeItem(_)
                | NetworkEvent::VendorItem { .. }
                | NetworkEvent::BoothItemGone(_)
                | NetworkEvent::Guild { .. }
                | NetworkEvent::GuildMember(_) => {}
                NetworkEvent::Silver(_)
                | NetworkEvent::Cps(_)
                | NetworkEvent::Vitals { .. }
                | NetworkEvent::Attributes { .. }
                | NetworkEvent::Nobility { .. }
                | NetworkEvent::NobilityRanking(_)
                | NetworkEvent::NobilityRankingClear
                | NetworkEvent::Bounty(_)
                | NetworkEvent::QuestLog(_)
                | NetworkEvent::GuildPanelInfo(_)
                | NetworkEvent::GuildPanelRow(_)
                | NetworkEvent::GuildRelations(_)
                | NetworkEvent::BountyProgress { .. }
                | NetworkEvent::StoreProduct(_)
                | NetworkEvent::StoreState(_)
                | NetworkEvent::StorePurchase(_)
                | NetworkEvent::DailyRewardChoice(_)
                | NetworkEvent::DailyRewardState(_)
                | NetworkEvent::Rift(_)
                | NetworkEvent::HighscoreRow(_)
                | NetworkEvent::WhisperCard(_)
                | NetworkEvent::HighscoreState(_)
                | NetworkEvent::StatusTimers(_)
                | NetworkEvent::RoleEffect { .. }
                | NetworkEvent::ViewedPlayer { .. }
                | NetworkEvent::ViewedItem(_)
                | NetworkEvent::Spouse { .. }
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

    pub fn send_walk(&self, character_id: u32, direction: u8, running: bool) {
        let _ = self.commands.send(NetworkCommand::Walk {
            character_id,
            direction,
            running,
        });
    }

    pub fn send_jump(&self, character_id: u32, target: [i32; 2], current: [i32; 2]) {
        let _ = self.commands.send(NetworkCommand::Jump {
            character_id,
            target,
            current,
        });
    }

    pub fn send_facing(&self, character_id: u32, direction: u8, current: [i32; 2]) {
        let _ = self.commands.send(NetworkCommand::Face {
            character_id,
            direction,
            current,
        });
    }

    pub fn send_kill_mode(&self, character_id: u32, mode: u8) {
        let _ = self.commands.send(NetworkCommand::KillMode { character_id, mode });
    }

    pub fn send_action(&self, character_id: u32, action: u32) {
        let _ = self.commands.send(NetworkCommand::Action {
            character_id,
            action,
        });
    }

    pub fn ping(&self, character_id: u32, timestamp: u32) {
        let _ = self.commands.send(NetworkCommand::Ping {
            character_id,
            timestamp,
        });
    }

    pub fn query_team_member(&self, character_id: u32, member_id: u32) {
        let _ = self.commands.send(NetworkCommand::QueryTeamMember {
            character_id,
            member_id,
        });
    }

    /// Asks what player `target_id` wears (View Equipment).
    pub fn query_equipment(&self, character_id: u32, target_id: u32) {
        let _ = self.commands.send(NetworkCommand::QueryEquipment { character_id, target_id });
    }

    /// A Store request (`StorePacket` constants).
    pub fn send_store(&self, action: u32, param: u32) {
        let _ = self.commands.send(NetworkCommand::Store { action, param });
    }

    /// A Highscores page.
    pub fn send_highscores(&self, category: u32, profession: u32, page: u32) {
        let _ = self.commands.send(NetworkCommand::Highscores { category, profession, page });
    }

    /// Donates `amount` silver for nobility, paid in CPs when `in_cps`.
    pub fn donate_nobility(&self, amount: u64, in_cps: bool) {
        let _ = self.commands.send(NetworkCommand::NobilityDonate { amount, in_cps });
    }

    /// Asks for the nobility ranking.
    pub fn request_nobility_ranking(&self) {
        let _ = self.commands.send(NetworkCommand::NobilityRanking);
    }

    /// Asks for the hero's own nobility (rank, seat, Top titles).
    pub fn request_nobility_status(&self) {
        let _ = self.commands.send(NetworkCommand::NobilityStatus);
    }

    /// A Reward Shrine request (`DailyRewardPacket` constants).
    pub fn send_daily_reward(&self, action: u32, param: u32) {
        let _ = self.commands.send(NetworkCommand::DailyReward { action, param });
    }

    /// A bounty Task Board action (`gravak_protocol::BountyPacket` constants).
    pub fn send_bounty(&self, action: u32, param: u32) {
        let _ = self.commands.send(NetworkCommand::Bounty { action, param });
    }

    pub fn send_guild_panel(&self, packet: gravak_protocol::GuildPanelPacket) {
        let _ = self.commands.send(NetworkCommand::GuildPanel(packet));
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
        let _ = self.commands.send(NetworkCommand::Booth { character_id, open: true });
    }

    pub fn close_player_shop(&self, character_id: u32) {
        let _ = self.commands.send(NetworkCommand::Booth { character_id, open: false });
    }

    /// Asks to revive after death (MsgAction 94).
    /// Asks the server to take the hero through the portal at (x, y).
    pub fn change_map(&self, character_id: u32, x: u16, y: u16) {
        let _ = self.commands.send(NetworkCommand::ChangeMap { character_id, x, y });
    }

    pub fn cancel_fly(&self, character_id: u32) {
        let _ = self.commands.send(NetworkCommand::CancelFly { character_id });
    }

    pub fn mine(&self, character_id: u32) {
        let _ = self.commands.send(NetworkCommand::Mine { character_id });
    }

    /// Asks to revive in town, or `here` where the body lies (MsgAction 94
    /// with data 1, Heaven's Blessing only).
    pub fn revive(&self, character_id: u32, here: bool) {
        let _ = self.commands.send(NetworkCommand::Revive { character_id, here });
    }

    /// Asks to go to offline training before leaving (the exit's Exercise).
    pub fn exercise(&self, character_id: u32) {
        let _ = self.commands.send(NetworkCommand::Exercise { character_id });
    }

    /// A city artisan's upgrade (`forge_window::LEVEL_ACTION` or
    /// `QUALITY_ACTION`) of item `item_uid`.
    pub fn artisan(&self, character_id: u32, item_uid: u32, action: u32) {
        let _ = self.commands.send(NetworkCommand::Artisan { character_id, item_uid, action });
    }

    pub fn player_shop_query(&self, character_id: u32, owner_id: u32) {
        let _ = self.commands.send(NetworkCommand::PlayerShopAction {
            packet: ItemActionPacket::booth_query(character_id, owner_id),
        });
    }

    /// Lists an item in the own booth, for `price` silver or CPs.
    pub fn player_shop_add(&self, character_id: u32, item_uid: u32, price: u32, cps: bool) {
        let mut packet = ItemActionPacket::booth_add(character_id, item_uid, price);
        if cps {
            // MsgItem 29, `BoothAddCPs`.
            packet.action_type = 29;
        }
        let _ = self.commands.send(NetworkCommand::PlayerShopAction { packet });
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

    pub fn repair_item(&self, character_id: u32, item_uid: u32, shop_id: u32) {
        let _ = self.commands.send(NetworkCommand::RepairItem { character_id, item_uid, shop_id });
    }

    pub fn drop_inventory_item(&self, character_id: u32, item_uid: u32) {
        let _ = self.commands.send(NetworkCommand::DropInventoryItem {
            character_id,
            item_uid,
        });
    }

    pub fn use_item(&self, character_id: u32, item_uid: u32) {
        self.use_item_choice(character_id, item_uid, 0);
    }

    /// Uses an item with a choice made for it (the Travel Cube's city).
    pub fn use_item_choice(&self, character_id: u32, item_uid: u32, choice: u32) {
        let _ = self.commands.send(NetworkCommand::UseItem {
            character_id,
            item_uid,
            choice,
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

    pub fn upgrade_level(&self, character_id: u32, item_uid: u32) {
        let _ = self.commands.send(NetworkCommand::PlayerShopAction {
            packet: ItemActionPacket::upgrade_level(character_id, item_uid),
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

    /// Rerolls every affix (`slot` 255) or the one in `slot`.
    pub fn reroll_item_rarity(&self, item_uid: u32, slot: u8) {
        let _ = self
            .commands
            .send(NetworkCommand::RerollRarity { item_uid, slot });
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

    pub fn send_channel(&self, channel: u16, character_id: u32, sender_name: &str, message: &str) {
        let packet = TalkPacket::on_channel(channel, character_id, sender_name, message);
        let _ = self.commands.send(NetworkCommand::Talk { packet });
    }

    pub fn send_booth_name(&self, character_id: u32, sender_name: &str, name: &str) {
        let packet = TalkPacket::booth(character_id, sender_name, name);
        let _ = self.commands.send(NetworkCommand::Talk { packet });
    }

    /// Pays for a broadcast to the whole server.
    pub fn send_broadcast(&self, message: &str) {
        let _ = self.commands.send(NetworkCommand::Broadcast { message: message.to_owned() });
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
    commands: UnboundedReceiver<NetworkCommand>,
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

    play(session, user, events, commands).await
}

/// The in-game half of a session: `session` has reached `user`'s
/// MsgUserInfo; this loads the map and then serves events and commands
/// until the connection ends.
pub(crate) async fn play(
    mut session: GameSession,
    user: UserInfo,
    events: Sender<NetworkEvent>,
    mut commands: UnboundedReceiver<NetworkCommand>,
) -> Result<(), String> {
    send_status(&events, &format!("loading {}", user.character_name));
    session
        .send(&ActionPacket::send_location(user.character_id))
        .await
        .map_err(|error| error.to_string())?;

    let (map_id, position, deferred_events) = load_location(&mut session, user.character_id)
        .await
        .map_err(|error| error.to_string())?;
    // The board the server sends at login goes by while the map loads; ask
    // for it again so the bounty tracker shows from the start.
    session
        .send(&gravak_protocol::BountyPacket::new(gravak_protocol::BountyPacket::OPEN, 0))
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
            mesh: user.mesh,
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
                        // The view type: 1 silver, 3 CPs.
                        cps: packet.view_type == 3,
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
                            affixes: [0; 4],
                            bless: 0,
                            enchant_hp: 0,
                            color: 0,
                            skin: 0,
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
                        position: info.position,
                        top_flags: info.top_flags,
                    });
                }
                Some(GameEvent::StoreProduct(product)) => {
                    let _ = events.send(NetworkEvent::StoreProduct(product));
                }
                Some(GameEvent::StoreState(store)) => {
                    let _ = events.send(NetworkEvent::StoreState(store));
                }
                Some(GameEvent::DailyRewardChoice(choice)) => {
                    let _ = events.send(NetworkEvent::DailyRewardChoice(choice));
                }
                Some(GameEvent::DailyRewardState(state)) => {
                    let _ = events.send(NetworkEvent::DailyRewardState(state));
                }
                Some(GameEvent::Rift(state)) => {
                    let _ = events.send(NetworkEvent::Rift(state));
                }
                Some(GameEvent::WhisperCard(card)) => {
                    let _ = events.send(NetworkEvent::WhisperCard(card));
                }
                Some(GameEvent::HighscoreRow(row)) => {
                    let _ = events.send(NetworkEvent::HighscoreRow(row));
                }
                Some(GameEvent::HighscoreState(state)) => {
                    let _ = events.send(NetworkEvent::HighscoreState(state));
                }
                Some(GameEvent::StorePurchase(purchase)) => {
                    let _ = events.send(NetworkEvent::StorePurchase(purchase));
                }
                Some(GameEvent::StatusTimers(timers)) => {
                    let _ = events.send(NetworkEvent::StatusTimers(timers));
                }
                Some(GameEvent::Name(name)) => {
                    let text = name.strings.first().cloned().unwrap_or_default();
                    if name.action == gravak_protocol::NamePacket::QUERY_MATE {
                        let _ = events.send(NetworkEvent::Spouse { id: name.uid, name: text.clone() });
                    }
                    let effect = match name.action {
                        gravak_protocol::NamePacket::FIREWORKS => Some(firework_effect(&text).to_owned()),
                        gravak_protocol::NamePacket::ROLE_EFFECT if !text.is_empty() => Some(text),
                        _ => None,
                    };
                    if let Some(effect) = effect {
                        let amount = name.strings.get(1).and_then(|amount| amount.parse().ok());
                        let _ = events.send(NetworkEvent::RoleEffect { actor_id: name.uid, effect, amount });
                    }
                }
                Some(GameEvent::BountyState(board)) => {
                    let _ = events.send(NetworkEvent::Bounty(board));
                }
                Some(GameEvent::QuestLog(line)) => {
                    let _ = events.send(NetworkEvent::QuestLog(line));
                }
                Some(GameEvent::GuildPanelInfo(info)) => {
                    let _ = events.send(NetworkEvent::GuildPanelInfo(info));
                }
                Some(GameEvent::GuildPanelRow(row)) => {
                    let _ = events.send(NetworkEvent::GuildPanelRow(row));
                }
                Some(GameEvent::GuildRelations(relations)) => {
                    let _ = events.send(NetworkEvent::GuildRelations(relations));
                }
                Some(GameEvent::Bounty(packet)) if packet.action == gravak_protocol::BountyPacket::PROGRESS => {
                    let _ = events.send(NetworkEvent::BountyProgress {
                        done: packet.param,
                        required: packet.extra,
                    });
                }
                Some(GameEvent::NobilityRanking(info)) if info.action == 3 => {
                    let _ = events.send(NetworkEvent::NobilityRankingClear);
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
                Some(GameEvent::ItemInfo(info)) if info.action == 4 => {
                    let _ = events.send(NetworkEvent::ViewedItem(OwnedItem {
                        uid: info.item_uid,
                        item_type: info.item_type,
                        position: info.position,
                        durability: info.durability,
                        max_durability: info.max_durability,
                        plus: info.plus,
                        rarity: info.bless,
                        gem_one: info.gem_one,
                        gem_two: info.gem_two,
                        affixes: crate::shop::affix_words(info.reserved3, info.restrain),
                        bless: info.reserved2,
                        enchant_hp: info.enchant,
                        color: info.reserved4 as u8,
                        skin: info.reserved1,
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
                        affixes: crate::shop::affix_words(info.reserved3, info.restrain),
                        bless: info.reserved2,
                        enchant_hp: info.enchant,
                        color: info.reserved4 as u8,
                        skin: info.reserved1,
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
                        affixes: crate::shop::affix_words(info.reserved3, info.restrain),
                        bless: info.reserved2,
                        enchant_hp: info.enchant,
                        color: info.reserved4 as u8,
                        skin: info.reserved1,
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
                        affixes: crate::shop::affix_words(info.reserved3, info.restrain),
                        bless: info.reserved2,
                        enchant_hp: info.enchant,
                        color: info.reserved4 as u8,
                        skin: info.reserved1,
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
                        affixes: crate::shop::affix_words(info.reserved3, info.restrain),
                        bless: info.reserved2,
                        enchant_hp: info.enchant,
                        color: info.reserved4 as u8,
                        skin: info.reserved1,
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
                        affixes: crate::shop::affix_words(info.reserved3, info.restrain),
                        bless: info.reserved2,
                        enchant_hp: info.enchant,
                        color: info.reserved4 as u8,
                        skin: info.reserved1,
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
                Some(GameEvent::Action(action)) if action.action_type == 117 => {
                    let _ = events.send(NetworkEvent::ViewedPlayer {
                        id: action.data1,
                        level: action.data2 as u16,
                        nobility_rank: u32::from(action.details),
                    });
                }
                Some(GameEvent::Action(action)) if action.action_type == 106 => {
                    let _ = events.send(NetworkEvent::TeamMemberPosition {
                        id: action.character_id,
                        map_id: action.data1,
                        position: unpack_position(action.data2),
                    });
                }
                Some(GameEvent::Action(action))
                    if action.action_type == 126 && action.data1 == crate::forge_window::WINDOW =>
                {
                    let _ = events.send(NetworkEvent::OpenForge);
                }
                Some(GameEvent::Action(action))
                    if action.action_type == ActionPacket::EXERCISE && action.character_id == local_character_id =>
                {
                    let _ = events.send(NetworkEvent::Exercised(action.data1 == 1));
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
                Some(GameEvent::Action(action))
                    if action.action_type == 96 && action.character_id == local_character_id =>
                {
                    let _ = events.send(NetworkEvent::KillMode(action.data1 as u8));
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
                    let target_cell = ((effect.data & 0xffff) as u16, (effect.data >> 16) as u16);
                    // A cast that hit nothing still shows on its caster.
                    let targets = if effect.targets.is_empty() { vec![(0, 0)] } else { effect.targets };
                    for (index, (target_id, damage)) in targets.into_iter().enumerate() {
                        let _ = events.send(NetworkEvent::SkillEffect {
                            attacker_id: effect.attacker_id,
                            target_id,
                            skill_id: effect.skill_id,
                            level: effect.level,
                            damage,
                            target_cell,
                            first: index == 0,
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
                    if packet.character_id == local_character_id
                        && packet.attribute == gravak_protocol::UserAttribPacket::XP =>
                {
                    let _ = events.send(NetworkEvent::XpPoints(packet.value.min(100) as u8));
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
                // MsgItem echoes: a ping (27) carries back its send time.
                Some(GameEvent::Other(gravak_protocol::MSG_ITEM, body)) if body.len() >= 16 => {
                    let word = |at: usize| u32::from_le_bytes([body[at], body[at + 1], body[at + 2], body[at + 3]]);
                    if word(8) == 27 {
                        let _ = events.send(NetworkEvent::Pong(word(12)));
                    } else if word(8) == 23 {
                        let _ = events.send(NetworkEvent::BoothItemGone(word(4)));
                    } else if word(8) == 9_001 && body.len() >= 20 {
                        let _ = events.send(NetworkEvent::TravelCubeWait(word(16)));
                    } else if matches!(word(8), crate::forge_window::LEVEL_ACTION | crate::forge_window::QUALITY_ACTION)
                        && body.len() >= 24
                    {
                        let _ = events.send(NetworkEvent::ForgeAnswer { item_uid: word(4), outcome: word(20) });
                    }
                }
                Some(_) => {}
                None => return Ok(()),
            },
            command = commands.recv() => match command {
                Some(NetworkCommand::Walk {
                    character_id,
                    direction,
                    running,
                }) => {
                    let packet = if running {
                        gravak_protocol::WalkPacket::run(character_id, direction)
                    } else {
                        gravak_protocol::WalkPacket::new(character_id, direction)
                    };
                    session
                        .send(&packet)
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
                Some(NetworkCommand::Face {
                    character_id,
                    direction,
                    current,
                }) => {
                    session
                        .send(&ActionPacket::change_facing(character_id, direction, current))
                        .await
                        .map_err(|error| error.to_string())?;
                }
                Some(NetworkCommand::ChangeMap { character_id, x, y }) => {
                    session
                        .send(&ActionPacket::change_map(character_id, x, y))
                        .await
                        .map_err(|error| error.to_string())?;
                }
                Some(NetworkCommand::CancelFly { character_id }) => {
                    session
                        .send(&ActionPacket::cancel_fly(character_id))
                        .await
                        .map_err(|error| error.to_string())?;
                }
                Some(NetworkCommand::Mine { character_id }) => {
                    session
                        .send(&ActionPacket::mine(character_id))
                        .await
                        .map_err(|error| error.to_string())?;
                }
                Some(NetworkCommand::Revive { character_id, here }) => {
                    session
                        .send(&ActionPacket::reborn(character_id, here))
                        .await
                        .map_err(|error| error.to_string())?;
                }
                Some(NetworkCommand::Exercise { character_id }) => {
                    session
                        .send(&ActionPacket::exercise(character_id))
                        .await
                        .map_err(|error| error.to_string())?;
                }
                Some(NetworkCommand::Artisan { character_id, item_uid, action }) => {
                    session
                        .send(&ItemActionPacket::artisan(character_id, item_uid, action))
                        .await
                        .map_err(|error| error.to_string())?;
                }
                Some(NetworkCommand::KillMode { character_id, mode }) => {
                    session
                        .send(&ActionPacket::set_kill_mode(character_id, u32::from(mode)))
                        .await
                        .map_err(|error| error.to_string())?;
                }
                Some(NetworkCommand::Booth { character_id, open }) => {
                    session
                        .send(&ActionPacket::booth(character_id, open))
                        .await
                        .map_err(|error| error.to_string())?;
                }
                Some(NetworkCommand::Action { character_id, action }) => {
                    session
                        .send(&ActionPacket::change_action(character_id, action))
                        .await
                        .map_err(|error| error.to_string())?;
                }
                Some(NetworkCommand::Ping {
                    character_id,
                    timestamp,
                }) => {
                    session
                        .send(&ItemActionPacket::ping(character_id, timestamp))
                        .await
                        .map_err(|error| error.to_string())?;
                }
                Some(NetworkCommand::QueryTeamMember {
                    character_id,
                    member_id,
                }) => {
                    session
                        .send(&ActionPacket::query_team_member(character_id, member_id))
                        .await
                        .map_err(|error| error.to_string())?;
                }
                Some(NetworkCommand::QueryEquipment { character_id, target_id }) => {
                    session
                        .send(&ActionPacket::query_equipment(character_id, target_id))
                        .await
                        .map_err(|error| error.to_string())?;
                }
                Some(NetworkCommand::Broadcast { message }) => {
                    session
                        .send(&gravak_protocol::BroadcastPacket::send(&message))
                        .await
                        .map_err(|error| error.to_string())?;
                }
                Some(NetworkCommand::Store { action, param }) => {
                    session
                        .send(&gravak_protocol::StorePacket::new(action, param))
                        .await
                        .map_err(|error| error.to_string())?;
                }
                Some(NetworkCommand::NobilityDonate { amount, in_cps }) => {
                    session
                        .send(&gravak_protocol::NobilityDonatePacket::new(local_character_id, amount, in_cps))
                        .await
                        .map_err(|error| error.to_string())?;
                }
                Some(NetworkCommand::NobilityStatus) => {
                    session
                        .send(&gravak_protocol::NobilityDonatePacket::query(local_character_id))
                        .await
                        .map_err(|error| error.to_string())?;
                }
                Some(NetworkCommand::NobilityRanking) => {
                    session
                        .send(&gravak_protocol::NobilityRankingRequest::new())
                        .await
                        .map_err(|error| error.to_string())?;
                }
                Some(NetworkCommand::Highscores { category, profession, page }) => {
                    session
                        .send(&gravak_protocol::HighscoresPacket::new(category, profession, page))
                        .await
                        .map_err(|error| error.to_string())?;
                }
                Some(NetworkCommand::DailyReward { action, param }) => {
                    session
                        .send(&gravak_protocol::DailyRewardPacket::new(action, param))
                        .await
                        .map_err(|error| error.to_string())?;
                }
                Some(NetworkCommand::Bounty { action, param }) => {
                    session
                        .send(&gravak_protocol::BountyPacket::new(action, param))
                        .await
                        .map_err(|error| error.to_string())?;
                }
                Some(NetworkCommand::GuildPanel(packet)) => {
                    session.send(&packet).await.map_err(|error| error.to_string())?;
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
                Some(NetworkCommand::RepairItem { character_id, item_uid, shop_id }) => {
                    session
                        .send(&ItemActionPacket::repair(character_id, item_uid, shop_id))
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
                Some(NetworkCommand::UseItem { character_id, item_uid, choice }) => {
                    crate::debug_log::write(format_args!(
                        "network send use item: character_id={} uid={}",
                        character_id, item_uid
                    ));
                    let mut packet = ItemActionPacket::use_item(character_id, item_uid);
                    packet.data2 = choice;
                    session
                        .send(&packet)
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
                Some(NetworkCommand::RerollRarity { item_uid, slot }) => {
                    session
                        .send(&ItemRarityRerollPacket::new(item_uid, slot))
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

/// The effect a firework item sets off (`3DEffect.ini`).
pub fn firework_effect(item_name: &str) -> &'static str {
    match item_name {
        "EndlessLove" => "firework-1love",
        "MyWish" => "firework-2love",
        _ => "firework-like",
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
        // A garment is worn over the armor, as the hero's own (`appearance_armor`).
        armor: if player.germent > 0 {
            player.germent as u32
        } else {
            crate::character::dyed(player.armor.max(0) as u32, (player.reserved1 & 0xff) as u8)
        },
        helmet: crate::character::dyed(player.helment.max(0) as u32, ((player.reserved1 >> 8) & 0xff) as u8),
        hair_style: player.hair_style.max(0) as u16,
        right_weapon: player.right_hand.max(0) as u32,
        left_weapon: crate::character::dyed(player.left_hand.max(0) as u32, ((player.reserved1 >> 16) & 0xff) as u8),
        action: u16::from(player.action),
        health: player.health_points,
        maximum_health: player.health_points,
        status_flags: player.status_flags.max(0) as u64,
        npc_kind: 0,
        animation_started: 0.0,
        animation_until: 0.0,
        respawn_until: 0.0,
        step_right: false,
        npc: false,
        level: player.level.max(0) as u16,
        booth: None,
        guild_rank: if player.guild_name.is_empty() {
            String::new()
        } else {
            match player.syndicate_member_rank {
                100 => "Leader",
                90 => "Deputy Leader",
                _ => "Member",
            }
            .to_owned()
        },
        guild_id: u32::from(player.syndicate_id.max(0) as u16),
        shield: 0,
        guild: player.guild_name,
        ko_count: 0,
        jumping: false,
        flung: 0,
        // The Top titles ride in reserved2.
        top_flags: player.reserved2.max(0) as u32,
        // The weapons' rarity rides in reserved1's top byte.
        weapon_rarity: [((player.reserved1 >> 24) & 7) as u8, ((player.reserved1 >> 27) & 7) as u8],
        nobility_rank: player.nobility_rank.max(0) as u32,
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
        level: 0,
        booth: None,
        guild: String::new(),
        guild_rank: String::new(),
        guild_id: 0,
        shield: 0,
        ko_count: 0,
        jumping: false,
        flung: 0,
        top_flags: 0,
        nobility_rank: 0,
        weapon_rarity: [0, 0],
    }
}

/// The 3D body (armor.ini's `body * 1_000_000` sections) of a look: its
/// last digit. 2001/2002 are the small/large female bodies 1 and 2,
/// 1003/1004 the small/large male bodies 3 and 4.
pub fn body_from_mesh(mesh: u32) -> u32 {
    match mesh % 10_000 {
        2001 => 1,
        2002 => 2,
        1003 => 3,
        1004 => 4,
        _ => 0,
    }
}

#[cfg(test)]
#[test]
fn male_looks_use_the_male_bodies() {
    // Avatar 31, small male: the sample profile's body 3.
    assert_eq!(body_from_mesh(311_003), 3);
    assert_eq!(body_from_mesh(1004), 4);
    assert_eq!(body_from_mesh(2_012_001), 1);
    assert_eq!(body_from_mesh(2002), 2);
    assert_eq!(body_from_mesh(999), 0);
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
