//! Minimal Conquer Online 5065-compatible wire protocol used by the client.
//!
//! The server currently exposes the 5017-era packet IDs of its CoEmu base. Keeping the
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
pub const MSG_NOBILITY: u16 = 2064;
/// Gravak's bounty tasks: board actions and the kill counter.
pub const MSG_BOUNTY: u16 = 9100;
/// Gravak's bounty tasks: the whole Task Board.
pub const MSG_BOUNTY_STATE: u16 = 9101;
/// Gravak's quest log: one quest a packet (or a clear before the whole log).
pub const MSG_QUEST_LOG: u16 = 9102;
/// Gravak's Store: one product of the catalog, and the buyer's CPs and VIP.
pub const MSG_STORE_PRODUCT: u16 = 9111;
pub const MSG_STORE_STATE: u16 = 9112;
/// Gravak's Store: one of the buyer's latest purchases.
pub const MSG_STORE_PURCHASE: u16 = 9113;
/// Gravak: when each of the hero's statuses ends.
pub const MSG_STATUS_TIMERS: u16 = 9120;
/// Gravak's Reward Shrine: one choice of the seven-day cycle, and where the
/// hero stands in it.
pub const MSG_DAILY_REWARD_CHOICE: u16 = 9131;
pub const MSG_DAILY_REWARD_STATE: u16 = 9132;
/// Gravak's Highscores: one ranked character, and the page shown.
pub const MSG_HIGHSCORE_ROW: u16 = 9141;
pub const MSG_WHISPER_CARD: u16 = 9150;
pub const MSG_HIGHSCORE_STATE: u16 = 9142;
/// Strings tied to an entity: fireworks, an effect played on it...
pub const MSG_NAME: u16 = 1015;
pub const MSG_NOBILITY_RANKING: u16 = 2065;
pub const MSG_ITEM_INFO: u16 = 1008;
pub const MSG_ITEM: u16 = 1009;
pub const MSG_ASSIGN_ATTRIBUTES: u16 = 1024;
pub const MSG_SOCKET_GEM: u16 = 1027;
pub const MSG_REFINERY: u16 = 2076;
pub const MSG_REFINERY_INFO: u16 = 2077;
pub const MSG_ITEM_RARITY: u16 = 2078;
pub const MSG_ITEM_RARITY_REROLL: u16 = 2079;
pub const MSG_ACTION: u16 = 1010;
pub const MSG_WEATHER: u16 = 1016;
pub const MSG_DATA: u16 = 1033;
pub const MSG_MAP_INFO: u16 = 1110;
pub const MSG_PLAYER: u16 = 1014;
pub const MSG_INTERACT: u16 = 1022;
pub const MSG_TEAM: u16 = 1023;
pub const MSG_TEAM_MEMBER: u16 = 1026;
pub const MSG_FRIEND: u16 = 1019;
pub const MSG_GROUND_ITEM: u16 = 1101;
pub const MSG_SKILL_EFFECT: u16 = 1105;
pub const MSG_SKILL: u16 = 1103;
pub const MSG_WEAPON_SKILL: u16 = 1025;
pub const MSG_USER_ATTRIB: u16 = 1017;
pub const MSG_NPC_INFO: u16 = 2030;
pub const MSG_NPC: u16 = 2031;
pub const MSG_TASK_DIALOG: u16 = 2032;
pub const MSG_WAREHOUSE_ACTION: u16 = 2033;
pub const MSG_GUILD_WAREHOUSE: u16 = 2034;
pub const MSG_TRADE: u16 = 1056;
pub const MSG_VENDOR_ITEM: u16 = 1108;
pub const MSG_GUILD: u16 = 1107;
pub const MSG_GUILD_MEMBER: u16 = 2102;

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
    /// Gravak: the dye (3..=9) of the starter coat; 0 leaves it plain.
    pub coat_dye: u32,
    /// Gravak: the face picked (1..=102 male, 201..=290 female); 0 lets
    /// the server draw one.
    pub avatar: u16,
    /// Gravak: the hair picked, colour (3..=9) * 100 + style; 0 lets the
    /// server draw one.
    pub hair: u16,
}

impl RegisterCharacter {
    pub fn new(username: &str, character_name: &str, token: u32) -> Self {
        Self::with_look(username, character_name, 1003, 10, token)
    }

    /// `mesh` is the body (1003/1004 male, 2001/2002 female) and `class`
    /// the base profession (10 Trojan, 20 Warrior, 40 Archer, 100 Taoist).
    pub fn with_look(
        username: &str,
        character_name: &str,
        mesh: u16,
        class: u16,
        token: u32,
    ) -> Self {
        Self {
            username: username.to_owned().into(),
            character_name: character_name.to_owned().into(),
            password: "********".to_owned().into(),
            mesh,
            class,
            token,
            coat_dye: 0,
            avatar: 0,
            hair: 0,
        }
    }

    /// The face and hair picked on the creation screen.
    pub fn with_appearance(mut self, avatar: u16, hair: u16) -> Self {
        self.avatar = avatar;
        self.hair = hair;
        self
    }

    /// The starter coat in Tinter dye `dye` (3..=9).
    pub fn with_coat_dye(mut self, dye: u8) -> Self {
        self.coat_dye = u32::from(dye);
        self
    }
}

/// Account registration request (project extension, not part of 5065),
/// answered by [`MSG_REGISTER_ACCOUNT_RESULT`].
#[derive(Debug, Serialize, PacketID)]
#[packet(id = 9001)]
pub struct RegisterAccount {
    pub username: String16,
    pub password: TQPassword,
}

pub const MSG_REGISTER_ACCOUNT_RESULT: u16 = 9002;

/// Outcome of [`register_account`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegisterAccountResult {
    Created,
    UsernameTaken,
    InvalidUsername,
    InvalidPassword,
    TryAgainLater,
    Unknown(u32),
}

impl RegisterAccountResult {
    pub fn from_code(code: u32) -> Self {
        match code {
            0 => Self::Created,
            1 => Self::UsernameTaken,
            2 => Self::InvalidUsername,
            3 => Self::InvalidPassword,
            4 => Self::TryAgainLater,
            other => Self::Unknown(other),
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

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, PacketID)]
#[packet(id = 1022)]
pub struct InteractionPacket {
    pub timestamp: u32,
    pub character_id: u32,
    pub target_id: u32,
    pub x: u16,
    pub y: u16,
    pub action: u32,
    pub value: u32,
    pub reserved: [u8; 12],
}

/// Bounty board action (client) or kill counter (server, action 10).
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, PacketID)]
#[packet(id = 9100)]
pub struct BountyPacket {
    pub action: u32,
    pub param: u32,
    pub extra: u32,
}

impl BountyPacket {
    pub const OPEN: u32 = 0;
    pub const REROLL: u32 = 1;
    pub const PICK: u32 = 2;
    pub const CLAIM_DAILY: u32 = 3;
    pub const UPGRADE: u32 = 4;
    /// Puts monster type `param` on the preferred list.
    pub const PREFER: u32 = 5;
    /// Puts monster type `param` on the avoided list.
    pub const AVOID: u32 = 6;
    /// Takes a name off a list: `param` = list (0 preferred, 1 avoided) * 10
    /// + the name's place.
    pub const UNLIST: u32 = 7;
    pub const PROGRESS: u32 = 10;

    pub fn new(action: u32, param: u32) -> Self {
        Self { action, param, extra: 0 }
    }
}

/// A paid broadcast to the whole server (`MsgBroadcast`, 2050).
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, PacketID)]
#[packet(id = 2050)]
pub struct BroadcastPacket {
    pub subtype: u32,
    pub unknown: u32,
    pub strings: tq_serde::StringList,
}

impl BroadcastPacket {
    pub const SEND: u32 = 3;

    pub fn send(message: &str) -> Self {
        Self {
            subtype: Self::SEND,
            unknown: 0,
            strings: vec![message.to_owned()].into(),
        }
    }
}

/// A Store request: the catalog (0) or buying product `param` (1).
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, PacketID)]
#[packet(id = 9110)]
pub struct StorePacket {
    pub action: u32,
    pub param: u32,
}

impl StorePacket {
    pub const OPEN: u32 = 0;
    pub const BUY: u32 = 1;
    /// VIP: teleport to city `param`.
    pub const TELEPORT: u32 = 2;
    /// VIP: the loot filter becomes `param`.
    pub const LOOT_FILTER: u32 = 3;

    pub fn new(action: u32, param: u32) -> Self {
        Self { action, param }
    }
}

/// One product of the Store, `index` of `total` (index 0 starts a catalog).
#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq, Eq, Hash, PacketID)]
#[packet(id = 9111)]
pub struct StoreProduct {
    pub index: u32,
    pub total: u32,
    pub product_id: u32,
    /// 0 VIP, 1 Consumables, 2 Upgrades.
    pub category: u32,
    /// The item sold, or 0 for days of VIP.
    pub item_type: u32,
    pub amount: u32,
    pub vip_days: u32,
    pub price_cps: u32,
    /// Left today; `u32::MAX` without a daily stock.
    pub stock_left: u32,
    pub name: String,
    pub description: String,
}

impl StoreProduct {
    pub fn limited(&self) -> bool {
        self.stock_left != u32::MAX
    }
}

/// The buyer's CPs and when their VIP ends (seconds since 1970, 0: none).
#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq, Eq, Hash, PacketID)]
#[packet(id = 9112)]
pub struct StoreState {
    pub cps: u64,
    pub vip_until: u64,
    /// Which drops go straight to a VIP's bag (a bit per kind).
    pub loot_filter: u32,
    /// When the next VIP teleport is allowed (seconds since 1970).
    pub teleport_ready_at: u64,
}

/// One of the buyer's latest purchases, `index` of `total`, newest first;
/// `total` 0 when there are none.
#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq, Eq, Hash, PacketID)]
#[packet(id = 9113)]
pub struct StorePurchase {
    pub index: u32,
    pub total: u32,
    pub price_cps: u32,
    /// Seconds since 1970.
    pub bought_at: u64,
    pub name: String,
}

/// A Reward Shrine request: open the Reward Wall (0) or claim today's
/// choice `param` (1).
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, PacketID)]
#[packet(id = 9130)]
pub struct DailyRewardPacket {
    pub action: u32,
    pub param: u32,
}

impl DailyRewardPacket {
    pub const OPEN: u32 = 0;
    pub const CLAIM: u32 = 1;

    pub fn new(action: u32, param: u32) -> Self {
        Self { action, param }
    }
}

/// One choice of the cycle's `day`, `index` of `total` (index 0 starts the
/// cycle afresh).
#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq, Eq, Hash, PacketID)]
#[packet(id = 9131)]
pub struct DailyRewardChoice {
    pub index: u32,
    pub total: u32,
    pub day: u32,
    pub item_type: u32,
    pub amount: u32,
}

/// Where the hero stands: days claimed in a row, today's day of the cycle
/// (1 to 7), whether it was claimed, and when the next day starts
/// (seconds since 1970).
#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq, Eq, Hash, PacketID)]
#[packet(id = 9132)]
pub struct DailyRewardState {
    pub streak: u32,
    pub day: u32,
    pub claimed: u32,
    pub next_day_at: u64,
}

/// A Highscores request: a page of `category` narrowed to `profession`
/// ([`HighscoresPacket::OWN_PAGE`]: the hero's own page).
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, PacketID)]
#[packet(id = 9140)]
pub struct HighscoresPacket {
    pub category: u32,
    pub profession: u32,
    pub page: u32,
}

impl HighscoresPacket {
    pub const OWN_PAGE: u32 = u32::MAX;

    pub fn new(category: u32, profession: u32, page: u32) -> Self {
        Self { category, profession, page }
    }
}

/// Who is on the other side of a whisper (Gravak): their look with the
/// avatar, level, rebirths, class, name and guild (empty without one).
#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq, Eq, Hash, PacketID)]
#[packet(id = 9150)]
pub struct WhisperCard {
    pub mesh: u32,
    pub level: u32,
    pub rebirths: u32,
    pub class: u32,
    pub name: String,
    pub guild: String,
    pub battle_power: u32,
}

/// One ranked character, `index` of `total` (index 0 starts the rows over);
/// `kind` 0 a row of the page, 1 one of the podium's three.
#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq, Eq, Hash, PacketID)]
#[packet(id = 9141)]
pub struct HighscoreRow {
    pub index: u32,
    pub total: u32,
    pub kind: u32,
    pub rank: u32,
    pub value: u64,
    pub level: u32,
    pub rebirths: u32,
    pub class: u32,
    pub mesh: u32,
    pub name: String,
}

/// The page shown, how many there are, the hero's rank (0: not ranked) and
/// when the ranking was worked out (seconds since 1970).
#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq, Eq, Hash, PacketID)]
#[packet(id = 9142)]
pub struct HighscoreState {
    pub category: u32,
    pub profession: u32,
    pub page: u32,
    pub pages: u32,
    pub own_rank: u32,
    pub own_value: u64,
    pub updated_at: u64,
    /// The hero's look, for their portrait.
    pub own_mesh: u32,
}

/// When each of the hero's statuses ends: `timers` is `flag:end,...` (a
/// status flag bit, seconds since 1970); `now` is the server's clock.
#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq, Eq, Hash, PacketID)]
#[packet(id = 9120)]
pub struct StatusTimers {
    pub now: u64,
    pub timers: String,
}

impl StatusTimers {
    /// The (flag, end) pairs.
    pub fn entries(&self) -> Vec<(u64, u64)> {
        self.timers
            .split(',')
            .filter_map(|entry| {
                let (flag, end) = entry.split_once(':')?;
                Some((flag.parse().ok()?, end.parse().ok()?))
            })
            .collect()
    }
}

/// `MsgName` (1015): an entity, an action and its strings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamePacket {
    pub uid: u32,
    pub action: u8,
    pub strings: Vec<String>,
}

impl NamePacket {
    /// Fireworks: the string is the firework item's name.
    pub const FIREWORKS: u8 = 1;
    /// The entity's spouse ("None" when single).
    pub const QUERY_MATE: u8 = 6;
    /// An effect from `3DEffect.ini` played on the entity.
    pub const ROLE_EFFECT: u8 = 10;

    pub fn parse(body: &[u8]) -> Option<Self> {
        let uid = u32::from_le_bytes(body.get(0..4)?.try_into().ok()?);
        let action = *body.get(4)?;
        let count = usize::from(*body.get(5).unwrap_or(&0));
        let mut strings = Vec::with_capacity(count);
        let mut offset = 6;
        for _ in 0..count {
            let length = usize::from(*body.get(offset)?);
            let text = body.get(offset + 1..offset + 1 + length)?;
            strings.push(String::from_utf8_lossy(text).into_owned());
            offset += 1 + length;
        }
        Some(Self { uid, action, strings })
    }
}

/// One bounty on offer, or the one under way (`monster_type` 0: none).
#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq, Eq, Hash)]
pub struct BountyTask {
    pub monster_type: u32,
    pub kills: u32,
    pub kills_done: u32,
    /// 0 Normal, 1 Silver, 2 Gold.
    pub grade: u32,
    pub points: u32,
    pub experience: u64,
    pub monster_name: String,
}

/// One line of the quest log, or (`action` 0) the clear before a whole log.
#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq, Eq, Hash, PacketID)]
#[packet(id = 9102)]
pub struct QuestLogPacket {
    pub action: u32,
    pub quest_id: u32,
    /// 0 the giver's next, 1 under way, 2 done.
    pub state: u32,
    /// 0 hunt, 1 delivery, 2 collection, 3 message.
    pub kind: u32,
    pub progress: u32,
    pub required: u32,
    pub min_level: u32,
    pub name: String,
    pub giver: String,
    pub description: String,
}

impl QuestLogPacket {
    pub const CLEAR: u32 = 0;
    pub const ENTRY: u32 = 1;
    pub const AVAILABLE: u32 = 0;
    pub const ACTIVE: u32 = 1;
    pub const DONE: u32 = 2;
}

/// The Task Board of bounty tasks.
#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq, Eq, Hash, PacketID)]
#[packet(id = 9101)]
pub struct BountyState {
    pub points: u32,
    pub tokens: u32,
    pub daily_available: u32,
    /// 0 Beginner, 1 Adept, 2 Expert, 3 Master.
    pub difficulty: u32,
    pub completed: u32,
    pub damage_level: u32,
    pub leech_level: u32,
    pub loot_level: u32,
    pub double_kill_level: u32,
    pub offer_count: u32,
    pub offer0: BountyTask,
    pub offer1: BountyTask,
    pub offer2: BountyTask,
    pub task: BountyTask,
    /// Preferred monsters' names, joined by `|`.
    pub preferred: String,
    /// Avoided monsters' names, joined by `|`.
    pub avoided: String,
}

impl BountyState {
    /// The preferred monsters (`avoided` false) or the avoided ones.
    pub fn listed(&self, avoided: bool) -> Vec<&str> {
        let joined = if avoided { &self.avoided } else { &self.preferred };
        joined.split('|').filter(|name| !name.is_empty()).collect()
    }

    /// The bounties on offer.
    pub fn offers(&self) -> Vec<&BountyTask> {
        [&self.offer0, &self.offer1, &self.offer2]
            .into_iter()
            .take(self.offer_count as usize)
            .collect()
    }

    /// The bounty under way.
    pub fn active(&self) -> Option<&BountyTask> {
        (self.task.monster_type != 0).then_some(&self.task)
    }

    /// Talisman levels: damage, life leech, loot, double kills.
    pub fn talisman(&self) -> [u32; 4] {
        [self.damage_level, self.leech_level, self.loot_level, self.double_kill_level]
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, PacketID)]
#[packet(id = 1023)]
pub struct TeamActionPacket {
    pub action: u32,
    pub target_id: u32,
    pub reserved: u32,
}

impl TeamActionPacket {
    pub fn new(action: u32, target_id: u32) -> Self {
        Self {
            action,
            target_id,
            reserved: 0,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, PacketID)]
#[packet(id = 1026)]
pub struct TeamMemberPacket {
    pub reserved: u8,
    pub reserved_header: [u8; 3],
    pub name: String16,
    pub member_id: u32,
    pub mesh: u32,
    pub max_health: u16,
    pub health: u16,
    pub action: u8,
    pub reserved_tail: [u8; 7],
}

/// Classic 5065 trade packet. `target` carries the peer or item UID and
/// `data` carries an amount for money/confirmation extensions.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, PacketID)]
#[packet(id = 1056)]
pub struct TradePacket {
    pub target: u32,
    pub subtype: u32,
    pub data: u32,
}

impl TradePacket {
    pub fn new(target: u32, subtype: u32, data: u32) -> Self {
        Self {
            target,
            subtype,
            data,
        }
    }
}

/// Classic 5065 friend request/update packet.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, PacketID)]
#[packet(id = 1019)]
pub struct FriendPacket {
    pub friend_id: u32,
    pub action: u8,
    pub online_flag: u8,
    pub reserved: u16,
    pub reserved12: u32,
    pub reserved16: u32,
    pub name: String16,
    pub reserved36: u32,
}

impl FriendPacket {
    pub fn new(action: u8, friend_id: u32, name: &str, online: bool) -> Self {
        Self {
            friend_id,
            action,
            online_flag: u8::from(online),
            reserved: 0,
            reserved12: 0,
            reserved16: 0,
            name: name.into(),
            reserved36: 0,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, PacketID)]
#[packet(id = 1101)]
pub struct GroundItemPacket {
    pub uid: u32,
    pub item_id: u32,
    pub x: u16,
    pub y: u16,
    pub unknown: u16,
    pub drop_type: u16,
    pub reserved: [u8; 8],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillEffect {
    pub attacker_id: u32,
    pub data: u32,
    pub skill_id: u16,
    pub level: u16,
    pub targets: Vec<(u32, u32)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefineryInfoRecord {
    pub item_id: u32,
    pub action: u32,
    pub refinery_type: u32,
    pub level: u32,
    pub effect: i32,
    pub time: u32,
    pub unknown: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefineryInfoPacket {
    pub records: Vec<RefineryInfoRecord>,
}

/// Weapon proficiency (1025): weapon type (`item_type / 1000`), level and
/// experience inside the level.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, PacketID)]
#[packet(id = 1025)]
pub struct WeaponSkillPacket {
    pub weapon_type: u32,
    pub level: u32,
    pub experience: u32,
}

/// One changed attribute of a character (1017); `attribute` 9 is stamina.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, PacketID)]
#[packet(id = 1017)]
pub struct UserAttribPacket {
    pub character_id: u32,
    pub count: u32,
    pub attribute: u32,
    pub value: u64,
    pub reserved: u64,
}

impl UserAttribPacket {
    pub const STAMINA: u32 = 9;
    /// The XP circle's points (0-100).
    pub const XP: u32 = 28;
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, PacketID)]
#[packet(id = 1103)]
pub struct SkillPacket {
    pub experience: u32,
    pub skill_id: u16,
    pub level: u16,
    pub reserved: [u8; 10],
}

impl GroundItemPacket {
    pub fn pickup(uid: u32, item_id: u32, x: u16, y: u16) -> Self {
        Self {
            uid,
            item_id,
            x,
            y,
            // The classic client leaves this field zero in the pickup
            // request. Servers fill it when broadcasting the result.
            unknown: 0,
            drop_type: 3,
            reserved: [0; 8],
        }
    }
}

impl InteractionPacket {
    pub fn attack(character_id: u32, target_id: u32, x: u16, y: u16) -> Self {
        Self {
            timestamp: unix_timestamp(),
            character_id,
            target_id,
            x,
            y,
            action: 2,
            value: 0,
            reserved: [0; 12],
        }
    }

    pub fn magic_attack(character_id: u32, target_id: u32, x: u16, y: u16, skill_id: u32) -> Self {
        Self {
            timestamp: unix_timestamp(),
            character_id,
            target_id,
            x,
            y,
            action: 21,
            value: skill_id,
            reserved: [0; 12],
        }
    }
}

#[derive(Debug, Serialize, PacketID, Clone)]
#[packet(id = 1004)]
pub struct TalkPacket {
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

impl TalkPacket {
    /// Names the sender's booth (the original's Vendor channel, 2104).
    pub fn booth(character_id: u32, sender_name: &str, name: &str) -> Self {
        Self {
            channel: 2104,
            ..Self::public(character_id, sender_name, name)
        }
    }

    pub fn public(character_id: u32, sender_name: &str, message: &str) -> Self {
        Self {
            color: 0x00FF_FFFF,
            channel: 2000,
            style: 0,
            character_id,
            recipient_mesh: 0,
            sender_mesh: 0,
            list_count: 4,
            sender_name: sender_name.to_owned(),
            recipient_name: "ALLUSERS".to_owned(),
            suffix: String::new(),
            message: message.to_owned(),
        }
    }

    /// A line on `channel` (Team 2003, Guild 2004, Friend 2009, World
    /// 2021...).
    pub fn on_channel(channel: u16, character_id: u32, sender_name: &str, message: &str) -> Self {
        Self {
            channel,
            ..Self::public(character_id, sender_name, message)
        }
    }

    pub fn whisper(
        character_id: u32,
        sender_name: &str,
        recipient_name: &str,
        message: &str,
    ) -> Self {
        Self {
            color: 0x00FF_FFFF,
            channel: 2001,
            style: 0,
            character_id,
            recipient_mesh: 0,
            sender_mesh: 0,
            list_count: 4,
            sender_name: sender_name.to_owned(),
            recipient_name: recipient_name.to_owned(),
            suffix: String::new(),
            message: message.to_owned(),
        }
    }
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

    /// MsgAction 106: asks where teammate `member_id` is.
    pub fn query_team_member(character_id: u32, member_id: u32) -> Self {
        Self {
            client_timestamp: unix_timestamp(),
            character_id,
            data1: member_id,
            data2: 0,
            details: 0,
            action_type: 106,
        }
    }

    /// MsgAction 117: asks what player `target_id` wears (the main bar's
    /// View Equipment).
    pub fn query_equipment(character_id: u32, target_id: u32) -> Self {
        Self {
            client_timestamp: unix_timestamp(),
            character_id,
            data1: target_id,
            data2: 0,
            details: 0,
            action_type: 117,
        }
    }

    /// Sets up the booth (MsgAction 111, `CreateBooth`) or takes it down
    /// (114, `LeaveBooth`).
    pub fn booth(character_id: u32, open: bool) -> Self {
        Self {
            client_timestamp: unix_timestamp(),
            character_id,
            data1: 0,
            data2: 0,
            details: 0,
            action_type: if open { 111 } else { 114 },
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

    pub fn jump(character_id: u32, target: [i32; 2], current: [i32; 2]) -> Self {
        Self {
            client_timestamp: unix_timestamp(),
            character_id,
            data1: pack_position(target),
            data2: pack_position(current),
            details: 0,
            action_type: 133,
        }
    }

    /// MsgAction 79: the hero turned in place at `current` to `direction`
    /// (MsgWalk convention).
    pub fn change_facing(character_id: u32, direction: u8, current: [i32; 2]) -> Self {
        Self {
            client_timestamp: unix_timestamp(),
            character_id,
            data1: 0,
            data2: pack_position(current),
            details: u16::from(direction % 8),
            action_type: 79,
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

    /// MsgAction 85: the hero stepped on a map portal at (x, y).
    pub fn change_map(character_id: u32, x: u16, y: u16) -> Self {
        Self {
            client_timestamp: unix_timestamp(),
            character_id,
            data1: u32::from(x) | (u32::from(y) << 16),
            data2: 0,
            details: 0,
            action_type: 85,
        }
    }

    /// MsgAction 99: start mining with a PickAxe.
    /// The flier lands (MsgAction 120, Redux `CancelFly`).
    pub fn cancel_fly(character_id: u32) -> Self {
        Self {
            action_type: 120,
            ..Self::mine(character_id)
        }
    }

    pub fn mine(character_id: u32) -> Self {
        Self {
            client_timestamp: unix_timestamp(),
            character_id,
            data1: 0,
            data2: 0,
            details: 0,
            action_type: 99,
        }
    }

    pub fn set_kill_mode(character_id: u32, mode: u32) -> Self {
        Self {
            client_timestamp: unix_timestamp(),
            character_id,
            data1: mode,
            data2: 0,
            details: 0,
            action_type: 96,
        }
    }

    /// Revive (94): in town, or `here` where the body lies (data 1).
    pub fn reborn(character_id: u32, here: bool) -> Self {
        Self {
            client_timestamp: unix_timestamp(),
            character_id,
            data1: u32::from(here),
            data2: 0,
            details: 0,
            action_type: 94,
        }
    }

    pub fn create_booth(character_id: u32) -> Self {
        Self {
            client_timestamp: unix_timestamp(),
            character_id,
            data1: 0,
            data2: 0,
            details: 0,
            action_type: 111,
        }
    }

    pub fn leave_booth(character_id: u32) -> Self {
        Self {
            client_timestamp: unix_timestamp(),
            character_id,
            data1: 0,
            data2: 0,
            details: 0,
            action_type: 114,
        }
    }
}

fn pack_position(position: [i32; 2]) -> u32 {
    (position[0].clamp(0, i32::from(u16::MAX)) as u32)
        | ((position[1].clamp(0, i32::from(u16::MAX)) as u32) << 16)
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

/// Classic 5065 attribute-allocation request. The actor identity comes from
/// the authenticated game session; each field contains the amount to spend.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, PacketID)]
#[packet(id = 1024)]
pub struct AssignAttributesPacket {
    pub strength: u8,
    pub agility: u8,
    pub vitality: u8,
    pub spirit: u8,
}

impl AssignAttributesPacket {
    pub fn one(attribute: &str) -> Option<Self> {
        Some(match attribute {
            "strength" => Self {
                strength: 1,
                agility: 0,
                vitality: 0,
                spirit: 0,
            },
            "agility" => Self {
                strength: 0,
                agility: 1,
                vitality: 0,
                spirit: 0,
            },
            "vitality" => Self {
                strength: 0,
                agility: 0,
                vitality: 1,
                spirit: 0,
            },
            "spirit" => Self {
                strength: 0,
                agility: 0,
                vitality: 0,
                spirit: 1,
            },
            _ => return None,
        })
    }
}

impl ItemActionPacket {
    /// MsgItem 27: the server sends it straight back, so the time it took is
    /// the round trip. `timestamp` is the client's own millisecond clock.
    pub fn ping(character_id: u32, timestamp: u32) -> Self {
        Self {
            character_id,
            item_id: 0,
            action_type: 27,
            client_timestamp: timestamp,
            amount: 0,
            data2: 0,
        }
    }

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

    /// The shop's Repair of one item (MsgItem 14), at shop `shop_id`.
    pub fn repair(character_id: u32, item_uid: u32, shop_id: u32) -> Self {
        Self {
            action_type: 14,
            ..Self::sell(character_id, item_uid, shop_id)
        }
    }

    pub fn drop(character_id: u32, item_uid: u32) -> Self {
        Self {
            character_id,
            item_id: item_uid,
            action_type: 3,
            client_timestamp: unix_timestamp(),
            amount: 1,
            data2: 0,
        }
    }

    pub fn use_item(character_id: u32, item_uid: u32) -> Self {
        Self {
            character_id,
            item_id: item_uid,
            action_type: 4,
            client_timestamp: unix_timestamp(),
            amount: 1,
            data2: 0,
        }
    }

    pub fn move_inventory(character_id: u32, item_uid: u32, inventory_slot: u8) -> Self {
        Self {
            character_id,
            item_id: item_uid,
            action_type: 30,
            client_timestamp: unix_timestamp(),
            amount: 1,
            data2: u32::from(inventory_slot),
        }
    }

    pub fn equip(character_id: u32, item_uid: u32, equipment_slot: u8) -> Self {
        Self {
            character_id,
            item_id: item_uid,
            action_type: 5,
            client_timestamp: unix_timestamp(),
            amount: 1,
            data2: u32::from(equipment_slot),
        }
    }

    pub fn unequip(character_id: u32, item_uid: u32, inventory_slot: u8) -> Self {
        Self {
            character_id,
            item_id: item_uid,
            action_type: 6,
            client_timestamp: unix_timestamp(),
            amount: 1,
            data2: u32::from(inventory_slot),
        }
    }

    pub fn upgrade(character_id: u32, item_uid: u32) -> Self {
        Self {
            character_id,
            item_id: item_uid,
            action_type: 20,
            client_timestamp: unix_timestamp(),
            amount: 1,
            data2: 0,
        }
    }

    /// The item menu's LEVEL (MsgItem 44): the next level of the item, as
    /// MagicArtisan or WeaponMaster gives it.
    pub fn upgrade_level(character_id: u32, item_uid: u32) -> Self {
        Self {
            character_id,
            item_id: item_uid,
            action_type: 44,
            client_timestamp: unix_timestamp(),
            amount: 1,
            data2: 0,
        }
    }

    pub fn improve_quality(character_id: u32, item_uid: u32) -> Self {
        Self {
            character_id,
            item_id: item_uid,
            action_type: 19,
            client_timestamp: unix_timestamp(),
            amount: 1,
            data2: 0,
        }
    }

    pub fn open_socket(character_id: u32, item_uid: u32) -> Self {
        Self {
            character_id,
            item_id: item_uid,
            action_type: 43,
            client_timestamp: unix_timestamp(),
            amount: 1,
            data2: 0,
        }
    }

    pub fn booth_add(character_id: u32, item_uid: u32, price: u32) -> Self {
        Self {
            character_id,
            item_id: item_uid,
            action_type: 22,
            client_timestamp: unix_timestamp(),
            amount: price,
            data2: 0,
        }
    }

    pub fn booth_remove(character_id: u32, item_uid: u32) -> Self {
        Self {
            character_id,
            item_id: item_uid,
            action_type: 23,
            client_timestamp: unix_timestamp(),
            amount: 1,
            data2: 0,
        }
    }

    pub fn booth_query(character_id: u32, owner_id: u32) -> Self {
        Self {
            character_id,
            item_id: owner_id,
            action_type: 21,
            client_timestamp: unix_timestamp(),
            amount: 1,
            data2: owner_id,
        }
    }

    pub fn booth_buy(character_id: u32, item_uid: u32, owner_id: u32) -> Self {
        Self {
            character_id,
            item_id: item_uid,
            action_type: 24,
            client_timestamp: unix_timestamp(),
            amount: 1,
            data2: owner_id,
        }
    }
}

/// Classic 5065 packet 1027 for inserting/removing a gem from an open socket.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, PacketID)]
#[packet(id = 1027)]
pub struct SocketGemPacket {
    pub item_id: u32,
    pub gem_id: u32,
    pub socket: u16,
    pub subtype: u8,
}

impl SocketGemPacket {
    pub fn insert(item_id: u32, gem_id: u32, socket: u8) -> Self {
        Self {
            item_id,
            gem_id,
            socket: u16::from(socket),
            subtype: 0,
        }
    }

    pub fn remove(item_id: u32, socket: u8) -> Self {
        Self {
            item_id,
            gem_id: 0,
            socket: u16::from(socket),
            subtype: 1,
        }
    }
}

/// Classic 5065 packet 2076. Action 0 applies an inventory refinery material
/// to an equipped item; the server remains authoritative for compatibility.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, PacketID)]
#[packet(id = 2076)]
pub struct RefineryPacket {
    pub action: u32,
    pub item_id: u32,
    pub material_id: u32,
}

impl RefineryPacket {
    pub fn apply(item_id: u32, material_id: u32) -> Self {
        Self {
            action: 0,
            item_id,
            material_id,
        }
    }

    pub fn apply_artifact(item_id: u32, material_id: u32) -> Self {
        Self {
            action: 1,
            item_id,
            material_id,
        }
    }
}

/// Custom item-rarity request used by the sandbox UI. The classic item
/// quality suffix and the +level remain independent from this field.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, PacketID)]
#[packet(id = 2078)]
pub struct ItemRarityPacket {
    pub item_id: u32,
    pub rarity: u8,
}

impl ItemRarityPacket {
    pub fn set(item_id: u32, rarity: u8) -> Self {
        Self { item_id, rarity }
    }
}

/// Rerolls an item's rarity affixes: all of them for a DragonBall (`slot`
/// 255), or the one in `slot` for a SuperDragonBall.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, PacketID)]
#[packet(id = 2079)]
pub struct ItemRarityRerollPacket {
    pub item_id: u32,
    pub slot: u8,
}

impl ItemRarityRerollPacket {
    pub fn new(item_id: u32, slot: u8) -> Self {
        Self { item_id, slot }
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

#[derive(Debug, Deserialize, Clone, PartialEq, Eq)]
pub struct VendorItemPacket {
    pub item_uid: u32,
    pub owner_id: u32,
    pub price: u32,
    pub item_type: u32,
    pub durability: u16,
    pub max_durability: u16,
    pub view_type: u16,
    pub position: u8,
    pub gem_one: u8,
    pub gem_two: u8,
    pub plus: u8,
    pub reserved: u8,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, PacketID)]
#[packet(id = 1107)]
pub struct GuildPacket {
    pub action: u32,
    pub data: u32,
    pub required_level: i32,
    pub required_rebirth: i32,
    pub required_profession: i32,
    pub strings: tq_serde::StringList,
}

impl GuildPacket {
    pub fn new(action: u32, data: u32) -> Self {
        Self {
            action,
            data,
            required_level: 0,
            required_rebirth: 0,
            required_profession: 0,
            strings: Vec::<String>::new().into(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, PacketID)]
#[packet(id = 2102)]
pub struct GuildMemberPacket {
    pub reserved: u32,
    pub start_index: i32,
    pub amount: i32,
    pub name: String16,
    pub reserved16: u32,
    pub reserved20: u32,
    pub level: u32,
    pub rank: u16,
    pub reserved30: u16,
    pub reserved32: u32,
    pub donation: i32,
    pub online: u8,
    pub reserved41: [u8; 3],
    pub reserved44: u32,
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
    /// The answer to a dialog text box, carrying what was typed.
    pub fn answer_text(task_id: u32, option_id: u8, text: &str) -> Self {
        Self {
            task_id,
            avatar: 0,
            option_id,
            action: 101,
            msgs: vec![text.to_owned()].into(),
        }
    }

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

/// Custom client-to-server request for moving one item across the warehouse boundary.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, PacketID)]
#[packet(id = 2033)]
pub struct WarehouseActionPacket {
    pub character_id: u32,
    pub npc_id: u32,
    pub action: u8,
    pub item_uid: u32,
    pub position: u8,
    pub amount: u32,
}

impl WarehouseActionPacket {
    pub fn deposit(character_id: u32, npc_id: u32, item_uid: u32, position: u8) -> Self {
        Self {
            character_id,
            npc_id,
            action: 1,
            item_uid,
            position,
            amount: 0,
        }
    }

    pub fn withdraw(character_id: u32, npc_id: u32, item_uid: u32, position: u8) -> Self {
        Self {
            character_id,
            npc_id,
            action: 2,
            item_uid,
            position,
            amount: 0,
        }
    }

    pub fn silver(character_id: u32, npc_id: u32, amount: u32, deposit: bool) -> Self {
        Self {
            character_id,
            npc_id,
            action: if deposit { 3 } else { 4 },
            item_uid: 0,
            position: 0,
            amount,
        }
    }
}

/// Client-to-server request for the Guild item warehouse.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, PacketID)]
#[packet(id = 2034)]
pub struct GuildWarehouseActionPacket {
    pub character_id: u32,
    pub guild_id: u32,
    pub action: u8,
    pub item_uid: u32,
    pub position: u8,
}

impl GuildWarehouseActionPacket {
    pub fn open(character_id: u32, guild_id: u32) -> Self {
        Self {
            character_id,
            guild_id,
            action: 0,
            item_uid: 0,
            position: 0,
        }
    }

    pub fn deposit(character_id: u32, guild_id: u32, item_uid: u32, position: u8) -> Self {
        Self {
            character_id,
            guild_id,
            action: 1,
            item_uid,
            position,
        }
    }

    pub fn withdraw(character_id: u32, guild_id: u32, item_uid: u32, position: u8) -> Self {
        Self {
            character_id,
            guild_id,
            action: 2,
            item_uid,
            position,
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

    /// A step that observers animate as running (movement type 1).
    pub fn run(character_id: u32, direction: u8) -> Self {
        Self {
            movement_type: 1,
            ..Self::new(character_id, direction)
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
pub struct NobilityInfo {
    pub action: u32,
    pub character_id: u32,
    pub donation: u64,
    pub rank: u32,
    pub remaining: u64,
    pub title_id: u32,
    /// The place among the donors (0: none).
    pub position: u32,
    /// The Top titles the hero holds.
    pub top_flags: u32,
}

/// The Nobility window's Donate: `donation` silver (MsgNobility 2064,
/// action 1).
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, PacketID)]
#[packet(id = 2064)]
pub struct NobilityDonatePacket {
    pub action: u32,
    pub character_id: u32,
    pub donation: u64,
    pub rank: u32,
    pub remaining: u64,
    pub title_id: u32,
    pub position: u32,
    pub top_flags: u32,
}

impl NobilityDonatePacket {
    /// Asks for the hero's status (action 4); the server answers with it.
    pub fn query(character_id: u32) -> Self {
        Self { action: 4, ..Self::new(character_id, 0, false) }
    }

    /// Action 1 pays in silver, 2 in CPs (one per 50,000 silver).
    pub fn new(character_id: u32, amount: u64, in_cps: bool) -> Self {
        let action = if in_cps { 2 } else { 1 };
        Self { action, character_id, donation: amount, rank: 0, remaining: 0, title_id: 0, position: 0, top_flags: 0 }
    }
}

/// Asks for the nobility ranking (MsgNobilityRanking 2065, action 2); the
/// server answers with a clear (action 3) and a row per seat (action 1).
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, PacketID)]
#[packet(id = 2065)]
pub struct NobilityRankingRequest {
    pub action: u32,
    pub position: u32,
    pub character_id: u32,
    pub donation: u64,
    pub rank: u32,
    pub list_count: u8,
    pub character_name: String,
}

impl NobilityRankingRequest {
    pub fn new() -> Self {
        Self { action: 2, position: 0, character_id: 0, donation: 0, rank: 0, list_count: 0, character_name: String::new() }
    }
}

impl Default for NobilityRankingRequest {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Deserialize, Clone, PartialEq, Eq)]
pub struct NobilityRankingInfo {
    pub action: u32,
    pub position: u32,
    pub character_id: u32,
    pub donation: u64,
    pub rank: u32,
    pub list_count: u8,
    pub character_name: String,
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
    Nobility(NobilityInfo),
    NobilityRanking(NobilityRankingInfo),
    Bounty(BountyPacket),
    BountyState(BountyState),
    QuestLog(QuestLogPacket),
    StoreProduct(StoreProduct),
    StoreState(StoreState),
    DailyRewardChoice(DailyRewardChoice),
    DailyRewardState(DailyRewardState),
    HighscoreRow(HighscoreRow),
    WhisperCard(WhisperCard),
    HighscoreState(HighscoreState),
    StorePurchase(StorePurchase),
    StatusTimers(StatusTimers),
    Name(NamePacket),
    ItemInfo(ItemInfoPacket),
    Data(ServerData),
    MapInfo(MapInfo),
    Action(ActionPacket),
    Walk(WalkPacket),
    Player(ServerPlayer),
    Interaction(InteractionPacket),
    Team(TeamActionPacket),
    TeamMember(TeamMemberPacket),
    Trade(TradePacket),
    VendorItem(VendorItemPacket),
    Guild(GuildPacket),
    GuildMember(GuildMemberPacket),
    Friend(FriendPacket),
    GroundItem(GroundItemPacket),
    SkillEffect(SkillEffect),
    Skill(SkillPacket),
    WeaponSkill(WeaponSkillPacket),
    UserAttrib(UserAttribPacket),
    RefineryInfo(RefineryInfoPacket),
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
        MSG_NOBILITY => Ok(GameEvent::Nobility(
            NobilityInfo::decode(body).map_err(ProtocolError::Decode)?,
        )),
        MSG_NOBILITY_RANKING => Ok(GameEvent::NobilityRanking(
            NobilityRankingInfo::decode(body).map_err(ProtocolError::Decode)?,
        )),
        MSG_BOUNTY => Ok(GameEvent::Bounty(
            BountyPacket::decode(body).map_err(ProtocolError::Decode)?,
        )),
        MSG_BOUNTY_STATE => Ok(GameEvent::BountyState(
            BountyState::decode(body).map_err(ProtocolError::Decode)?,
        )),
        MSG_QUEST_LOG => Ok(GameEvent::QuestLog(
            QuestLogPacket::decode(body).map_err(ProtocolError::Decode)?,
        )),
        MSG_STORE_PRODUCT => Ok(GameEvent::StoreProduct(
            StoreProduct::decode(body).map_err(ProtocolError::Decode)?,
        )),
        MSG_STORE_STATE => Ok(GameEvent::StoreState(
            StoreState::decode(body).map_err(ProtocolError::Decode)?,
        )),
        MSG_STORE_PURCHASE => Ok(GameEvent::StorePurchase(
            StorePurchase::decode(body).map_err(ProtocolError::Decode)?,
        )),
        MSG_DAILY_REWARD_CHOICE => Ok(GameEvent::DailyRewardChoice(
            DailyRewardChoice::decode(body).map_err(ProtocolError::Decode)?,
        )),
        MSG_DAILY_REWARD_STATE => Ok(GameEvent::DailyRewardState(
            DailyRewardState::decode(body).map_err(ProtocolError::Decode)?,
        )),
        MSG_WHISPER_CARD => Ok(GameEvent::WhisperCard(
            WhisperCard::decode(body).map_err(ProtocolError::Decode)?,
        )),
        MSG_HIGHSCORE_ROW => Ok(GameEvent::HighscoreRow(
            HighscoreRow::decode(body).map_err(ProtocolError::Decode)?,
        )),
        MSG_HIGHSCORE_STATE => Ok(GameEvent::HighscoreState(
            HighscoreState::decode(body).map_err(ProtocolError::Decode)?,
        )),
        MSG_STATUS_TIMERS => Ok(GameEvent::StatusTimers(
            StatusTimers::decode(body).map_err(ProtocolError::Decode)?,
        )),
        MSG_NAME => NamePacket::parse(body)
            .map(GameEvent::Name)
            .ok_or(ProtocolError::MalformedResponse("MsgName is truncated")),
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
        MSG_INTERACT => Ok(GameEvent::Interaction(
            InteractionPacket::decode(body).map_err(ProtocolError::Decode)?,
        )),
        MSG_TEAM => Ok(GameEvent::Team(
            TeamActionPacket::decode(body).map_err(ProtocolError::Decode)?,
        )),
        MSG_TEAM_MEMBER => Ok(GameEvent::TeamMember(
            TeamMemberPacket::decode(body).map_err(ProtocolError::Decode)?,
        )),
        MSG_TRADE => Ok(GameEvent::Trade(
            TradePacket::decode(body).map_err(ProtocolError::Decode)?,
        )),
        MSG_VENDOR_ITEM => Ok(GameEvent::VendorItem(
            VendorItemPacket::decode(body).map_err(ProtocolError::Decode)?,
        )),
        MSG_GUILD => Ok(GameEvent::Guild(
            GuildPacket::decode(body).map_err(ProtocolError::Decode)?,
        )),
        MSG_GUILD_MEMBER => Ok(GameEvent::GuildMember(
            GuildMemberPacket::decode(body).map_err(ProtocolError::Decode)?,
        )),
        MSG_FRIEND => Ok(GameEvent::Friend(
            FriendPacket::decode(body).map_err(ProtocolError::Decode)?,
        )),
        MSG_GROUND_ITEM => Ok(GameEvent::GroundItem(
            GroundItemPacket::decode(body).map_err(ProtocolError::Decode)?,
        )),
        MSG_SKILL_EFFECT => Ok(GameEvent::SkillEffect(decode_skill_effect(body)?)),
        MSG_WEAPON_SKILL => Ok(GameEvent::WeaponSkill(
            WeaponSkillPacket::decode(body).map_err(ProtocolError::Decode)?,
        )),
        MSG_USER_ATTRIB => Ok(GameEvent::UserAttrib(
            UserAttribPacket::decode(body).map_err(ProtocolError::Decode)?,
        )),
        MSG_SKILL => Ok(GameEvent::Skill(
            SkillPacket::decode(body).map_err(ProtocolError::Decode)?,
        )),
        MSG_REFINERY_INFO => Ok(GameEvent::RefineryInfo(decode_refinery_info(body)?)),
        MSG_NPC_INFO => Ok(GameEvent::NpcInfo(decode_npc_info(body)?)),
        MSG_TASK_DIALOG => Ok(GameEvent::TaskDialog(decode_task_dialog(body)?)),
        _ => Ok(GameEvent::Other(id, body.clone())),
    }
}

fn decode_skill_effect(body: &Bytes) -> Result<SkillEffect, ProtocolError> {
    if body.len() < 16 {
        return Err(ProtocolError::MalformedResponse(
            "skill effect header is missing",
        ));
    }
    let attacker_id = u32::from_le_bytes(body[0..4].try_into().unwrap());
    let target_x = u16::from_le_bytes(body[4..6].try_into().unwrap());
    let target_y = u16::from_le_bytes(body[6..8].try_into().unwrap());
    let skill_id = u16::from_le_bytes(body[8..10].try_into().unwrap());
    let level = u16::from_le_bytes(body[10..12].try_into().unwrap());
    let count = u32::from_le_bytes(body[12..16].try_into().unwrap()) as usize;
    let required = 16usize.saturating_add(count.saturating_mul(32));
    if body.len() < required {
        return Err(ProtocolError::MalformedResponse(
            "skill effect targets are truncated",
        ));
    }
    let mut targets = Vec::with_capacity(count);
    for index in 0..count {
        let offset = 16 + index * 32;
        let target_id = u32::from_le_bytes(body[offset..offset + 4].try_into().unwrap());
        let damage = u32::from_le_bytes(body[offset + 4..offset + 8].try_into().unwrap());
        targets.push((target_id, damage));
    }
    Ok(SkillEffect {
        attacker_id,
        data: u32::from(target_x) | (u32::from(target_y) << 16),
        skill_id,
        level,
        targets,
    })
}

fn decode_refinery_info(body: &Bytes) -> Result<RefineryInfoPacket, ProtocolError> {
    if body.len() < 4 {
        return Err(ProtocolError::MalformedResponse(
            "refinery info header is missing",
        ));
    }
    let count = u32::from_le_bytes(body[0..4].try_into().unwrap()) as usize;
    let required = 4usize.saturating_add(count.saturating_mul(28));
    if body.len() < required {
        return Err(ProtocolError::MalformedResponse(
            "refinery info records are truncated",
        ));
    }
    let mut records = Vec::with_capacity(count);
    for index in 0..count {
        let offset = 4 + index * 28;
        records.push(RefineryInfoRecord {
            item_id: u32::from_le_bytes(body[offset..offset + 4].try_into().unwrap()),
            action: u32::from_le_bytes(body[offset + 4..offset + 8].try_into().unwrap()),
            refinery_type: u32::from_le_bytes(body[offset + 8..offset + 12].try_into().unwrap()),
            level: u32::from_le_bytes(body[offset + 12..offset + 16].try_into().unwrap()),
            effect: i32::from_le_bytes(body[offset + 16..offset + 20].try_into().unwrap()),
            time: u32::from_le_bytes(body[offset + 20..offset + 24].try_into().unwrap()),
            unknown: u32::from_le_bytes(body[offset + 24..offset + 28].try_into().unwrap()),
        });
    }
    Ok(RefineryInfoPacket { records })
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

/// Creates an account on the account server (`9001`/`9002`).
pub async fn register_account(
    address: &str,
    username: &str,
    password: &str,
) -> Result<RegisterAccountResult, ProtocolError> {
    let stream = TcpStream::connect(address).await?;
    let (mut encoder, mut decoder) = TQCodec::new(stream, CQCipher::new()).split();
    let packet = RegisterAccount {
        username: username.to_owned().into(),
        password: password.to_owned().into(),
    }
    .encode()
    .map_err(ProtocolError::Encode)?;
    encoder.send(packet).await?;
    while let Some(packet) = decoder.next().await {
        let (id, body) = packet?;
        if id != MSG_REGISTER_ACCOUNT_RESULT {
            return Err(ProtocolError::UnexpectedPacket(id));
        }
        if body.len() < 4 {
            return Err(ProtocolError::MalformedResponse("missing registration result"));
        }
        let code = u32::from_le_bytes(body[..4].try_into().expect("four bytes"));
        return Ok(RegisterAccountResult::from_code(code));
    }
    Err(ProtocolError::Io(std::io::Error::new(
        std::io::ErrorKind::UnexpectedEof,
        "account server closed the connection without a registration result",
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
    use serde::Serialize;
    use tq_network::TQCipher;
    use tq_network::{PacketEncode, PacketID};

    #[test]
    fn name_packets_read_their_strings() {
        let mut body = 1_000_001_u32.to_le_bytes().to_vec();
        body.extend([NamePacket::ROLE_EFFECT, 1, 7]);
        body.extend(b"LevelUp");
        let name = NamePacket::parse(&body).expect("parses");
        assert_eq!((name.uid, name.action, name.strings), (1_000_001, 10, vec!["LevelUp".to_owned()]));
        assert!(NamePacket::parse(&body[..3]).is_none());
    }

    #[test]
    fn account_login_matches_server_wire_shape() {
        let packet = AccountLogin::new("test1", "123456", "Gravak")
            .encode()
            .expect("account packet should encode");
        assert_eq!(packet.0, MSG_ACCOUNT);
        assert_eq!(packet.1.len(), 48);
        assert_eq!(&packet.1[..6], b"test1\0");
        assert_eq!(&packet.1[32..38], b"Gravak");
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
        let packet = AccountLogin::new("test1", "123456", "Gravak")
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
        assert_eq!(register.1.len(), 64);
        assert_eq!(&register.1[52..56], &42_u32.to_le_bytes());
        // Gravak's starter coat dye follows the classic fields.
        let dyed = RegisterCharacter::new("test1", "Gravak", 42)
            .with_coat_dye(5)
            .encode()
            .expect("register packet should encode");
        assert_eq!(&dyed.1[56..60], &5_u32.to_le_bytes());
        // Then the face and the hair picked.
        let look = RegisterCharacter::new("test1", "Gravak", 42)
            .with_appearance(205, 742)
            .encode()
            .expect("register packet should encode");
        assert_eq!(&look.1[60..62], &205_u16.to_le_bytes());
        assert_eq!(&look.1[62..64], &742_u16.to_le_bytes());

        let action = ActionPacket::send_location(1000001)
            .encode()
            .expect("action packet should encode");
        assert_eq!(action.0, MSG_ACTION);
        assert_eq!(action.1.len(), 20);

        let jump = ActionPacket::jump(1000001, [321, 654], [320, 653])
            .encode()
            .expect("jump packet should encode");
        assert_eq!(jump.0, MSG_ACTION);
        assert_eq!(
            u32::from_le_bytes(jump.1[8..12].try_into().unwrap()),
            654 << 16 | 321
        );
        assert_eq!(
            u32::from_le_bytes(jump.1[12..16].try_into().unwrap()),
            653 << 16 | 320
        );
        assert_eq!(u16::from_le_bytes(jump.1[18..20].try_into().unwrap()), 133);

        let walk = WalkPacket::new(1000001, 0)
            .encode()
            .expect("walk packet should encode");
        assert_eq!(walk.0, MSG_WALK);
        assert_eq!(walk.1.len(), 6);
    }

    #[test]
    fn core_game_request_fixtures_keep_packet_ids_and_lengths_stable() {
        let talk = TalkPacket::public(1_000_001, "Tester", "hello")
            .encode()
            .expect("talk packet should encode");
        assert_eq!(talk.0, MSG_TALK);
        assert!(talk.1.len() >= 32);

        let whisper = TalkPacket::whisper(1_000_001, "Tester", "Target", "hello")
            .encode()
            .expect("whisper packet should encode");
        assert_eq!(whisper.0, MSG_TALK);
        let whisper_packet = TalkPacket::whisper(1_000_001, "Tester", "Target", "hello");
        assert_eq!(whisper_packet.channel, 2001);
        assert_eq!(whisper_packet.recipient_name, "Target");

        let use_item = ItemActionPacket::use_item(1_000_001, 77)
            .encode()
            .expect("item use packet should encode");
        assert_eq!(use_item.0, MSG_ITEM);
        assert_eq!(use_item.1.len(), 24);
        assert_eq!(u32::from_le_bytes(use_item.1[8..12].try_into().unwrap()), 4);

        let upgrade = ItemActionPacket::upgrade(1_000_001, 77)
            .encode()
            .expect("item upgrade packet should encode");
        assert_eq!(upgrade.0, MSG_ITEM);
        assert_eq!(u32::from_le_bytes(upgrade.1[8..12].try_into().unwrap()), 20);

        let quality = ItemActionPacket::improve_quality(1_000_001, 77)
            .encode()
            .expect("quality upgrade packet should encode");
        assert_eq!(quality.0, MSG_ITEM);
        assert_eq!(u32::from_le_bytes(quality.1[8..12].try_into().unwrap()), 19);

        let socket = SocketGemPacket::insert(77, 88, 1)
            .encode()
            .expect("socket packet should encode");
        assert_eq!(socket.0, MSG_SOCKET_GEM);
        assert_eq!(socket.1.len(), 11);
        assert_eq!(u32::from_le_bytes(socket.1[0..4].try_into().unwrap()), 77);
        assert_eq!(socket.1[10], 0);

        let refinery = RefineryPacket::apply(77, 88)
            .encode()
            .expect("refinery packet should encode");
        assert_eq!(refinery.0, MSG_REFINERY);
        assert_eq!(refinery.1.len(), 12);
        assert_eq!(u32::from_le_bytes(refinery.1[0..4].try_into().unwrap()), 0);
        assert_eq!(u32::from_le_bytes(refinery.1[4..8].try_into().unwrap()), 77);
        assert_eq!(
            u32::from_le_bytes(refinery.1[8..12].try_into().unwrap()),
            88
        );

        let artifact = RefineryPacket::apply_artifact(99, 100)
            .encode()
            .expect("artifact packet should encode");
        assert_eq!(artifact.0, MSG_REFINERY);
        assert_eq!(artifact.1.len(), 12);
        assert_eq!(u32::from_le_bytes(artifact.1[0..4].try_into().unwrap()), 1);
        assert_eq!(u32::from_le_bytes(artifact.1[4..8].try_into().unwrap()), 99);
        assert_eq!(
            u32::from_le_bytes(artifact.1[8..12].try_into().unwrap()),
            100
        );

        let action = ActionPacket::change_action(1_000_001, 140)
            .encode()
            .expect("action packet should encode");
        assert_eq!(action.0, MSG_ACTION);
        assert_eq!(action.1.len(), 20);

        let kill_mode = ActionPacket::set_kill_mode(1_000_001, 0)
            .encode()
            .expect("kill mode packet should encode");
        assert_eq!(kill_mode.0, MSG_ACTION);
        assert_eq!(
            u32::from_le_bytes(kill_mode.1[8..12].try_into().unwrap()),
            0
        );
        assert_eq!(
            u16::from_le_bytes(kill_mode.1[18..20].try_into().unwrap()),
            96
        );

        let reborn = ActionPacket::reborn(1_000_001, false)
            .encode()
            .expect("reborn packet should encode");
        assert_eq!(reborn.0, MSG_ACTION);
        assert_eq!(u16::from_le_bytes(reborn.1[18..20].try_into().unwrap()), 94);

        let interaction = InteractionPacket::attack(1_000_001, 400_001, 61, 110)
            .encode()
            .expect("interaction packet should encode");
        assert_eq!(interaction.0, MSG_INTERACT);
        assert_eq!(interaction.1.len(), 36);

        let magic = InteractionPacket::magic_attack(1_000_001, 400_001, 61, 110, 1_000)
            .encode()
            .expect("magic interaction packet should encode");
        assert_eq!(magic.0, MSG_INTERACT);
        assert_eq!(magic.1.len(), 36);
        assert_eq!(u32::from_le_bytes(magic.1[16..20].try_into().unwrap()), 21);
        assert_eq!(
            u32::from_le_bytes(magic.1[20..24].try_into().unwrap()),
            1_000
        );
        assert_eq!(
            u32::from_le_bytes(interaction.1[16..20].try_into().unwrap()),
            2
        );

        let npc = NpcPacket::activate(123, 10)
            .encode()
            .expect("npc packet should encode");
        assert_eq!(npc.0, MSG_NPC);
        assert_eq!(npc.1.len(), 12);
    }

    #[test]
    fn team_packets_match_classic_1023_and_1026_shapes() {
        let action = TeamActionPacket::new(4, 1_000_002)
            .encode()
            .expect("team action should encode");
        assert_eq!(action.0, MSG_TEAM);
        assert_eq!(action.1.len(), 12);
        assert_eq!(
            TeamActionPacket::decode(&action.1).unwrap().target_id,
            1_000_002
        );

        let member = TeamMemberPacket {
            reserved: 0,
            reserved_header: [0; 3],
            name: "Gravak".into(),
            member_id: 1_000_002,
            mesh: 311_003,
            max_health: 318,
            health: 318,
            action: 0,
            reserved_tail: [0; 7],
        }
        .encode()
        .expect("team member should encode");
        assert_eq!(member.0, MSG_TEAM_MEMBER);
        assert_eq!(member.1.len(), 40);
        assert_eq!(
            TeamMemberPacket::decode(&member.1).unwrap().member_id,
            1_000_002
        );
    }

    #[test]
    fn guild_packet_matches_classic_1107_shape() {
        let mut packet = GuildPacket::new(2, 1_000_002);
        packet.strings = vec!["Knights".to_owned()].into();
        let encoded = packet.encode().expect("guild packet should encode");
        assert_eq!(encoded.0, MSG_GUILD);
        assert!(encoded.1.len() >= 24);
        let decoded = GuildPacket::decode(&encoded.1).expect("guild packet should decode");
        assert_eq!(decoded.action, 2);
        assert_eq!(decoded.data, 1_000_002);
        assert_eq!(Vec::<String>::from(decoded.strings), vec!["Knights"]);
    }

    #[test]
    fn guild_member_packet_matches_classic_2102_record_shape() {
        let packet = GuildMemberPacket {
            reserved: 0,
            start_index: 0,
            amount: 1,
            name: "Leader".into(),
            reserved16: 1_000_001,
            reserved20: 1003,
            level: 13,
            rank: 1000,
            reserved30: 0,
            reserved32: 0,
            donation: 0,
            online: 1,
            reserved41: [0; 3],
            reserved44: 0,
        };
        let encoded = packet.encode().expect("guild member packet should encode");
        assert_eq!(encoded.0, MSG_GUILD_MEMBER);
        assert_eq!(encoded.1.len(), 60);
        let decoded =
            GuildMemberPacket::decode(&encoded.1).expect("guild member packet should decode");
        assert_eq!(decoded.name, "Leader".into());
        assert_eq!(decoded.rank, 1000);
        assert_eq!(decoded.online, 1);
    }

    #[test]
    fn decodes_nobility_ranking_record_with_name() {
        #[derive(Debug, Serialize, PacketID)]
        #[packet(id = 2065)]
        struct RankingWire {
            action: u32,
            position: u32,
            character_id: u32,
            donation: u64,
            rank: u32,
            list_count: u8,
            character_name: String,
        }

        let encoded = RankingWire {
            action: 1,
            position: 1,
            character_id: 1_000_001,
            donation: 30_000_000,
            rank: 1,
            list_count: 1,
            character_name: "Gravak".to_owned(),
        }
        .encode()
        .expect("nobility ranking record should encode");
        assert_eq!(encoded.0, MSG_NOBILITY_RANKING);
        let decoded =
            NobilityRankingInfo::decode(&encoded.1).expect("nobility ranking record should decode");
        assert_eq!(decoded.position, 1);
        assert_eq!(decoded.character_id, 1_000_001);
        assert_eq!(decoded.character_name, "Gravak");
    }

    #[test]
    fn guild_warehouse_packet_round_trip_uses_dedicated_2034_shape() {
        let packet = GuildWarehouseActionPacket::deposit(1_000_001, 7, 42, 12)
            .encode()
            .expect("guild warehouse packet should encode");
        assert_eq!(packet.0, MSG_GUILD_WAREHOUSE);
        let decoded = GuildWarehouseActionPacket::decode(&packet.1)
            .expect("guild warehouse packet should decode");
        assert_eq!(decoded.character_id, 1_000_001);
        assert_eq!(decoded.guild_id, 7);
        assert_eq!(decoded.action, 1);
        assert_eq!(decoded.item_uid, 42);
        assert_eq!(decoded.position, 12);
    }

    #[test]
    fn trade_packet_matches_classic_1056_shape() {
        let packet = TradePacket::new(1_000_002, 6, 42)
            .encode()
            .expect("trade packet should encode");
        assert_eq!(packet.0, MSG_TRADE);
        assert_eq!(packet.1.len(), 12);
        let decoded = TradePacket::decode(&packet.1).expect("trade packet should decode");
        assert_eq!(decoded.target, 1_000_002);
        assert_eq!(decoded.subtype, 6);
        assert_eq!(decoded.data, 42);
    }

    #[test]
    fn friend_packet_matches_classic_1019_shape() {
        let packet = FriendPacket::new(15, 1_000_002, "Target", true)
            .encode()
            .expect("friend packet should encode");
        assert_eq!(packet.0, MSG_FRIEND);
        assert_eq!(packet.1.len(), 36);
        assert_eq!(
            FriendPacket::decode(&packet.1).unwrap().friend_id,
            1_000_002
        );
        assert_eq!(FriendPacket::decode(&packet.1).unwrap().action, 15);
    }

    #[test]
    fn assign_attributes_matches_classic_1024_shape() {
        let packet = AssignAttributesPacket::one("vitality")
            .expect("vitality should be a supported attribute")
            .encode()
            .expect("attribute packet should encode");
        assert_eq!(packet.0, MSG_ASSIGN_ATTRIBUTES);
        assert_eq!(packet.1, vec![0, 0, 1, 0]);
        assert!(AssignAttributesPacket::one("unknown").is_none());
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
    fn decodes_single_target_skill_effect() {
        let mut body = vec![0_u8; 88];
        body[0..4].copy_from_slice(&1_000_001_u32.to_le_bytes());
        body[4..6].copy_from_slice(&430_u16.to_le_bytes());
        body[6..8].copy_from_slice(&378_u16.to_le_bytes());
        body[8..10].copy_from_slice(&1_000_u16.to_le_bytes());
        body[10..12].copy_from_slice(&1_u16.to_le_bytes());
        body[12..16].copy_from_slice(&1_u32.to_le_bytes());
        body[16..20].copy_from_slice(&400_001_u32.to_le_bytes());
        body[20..24].copy_from_slice(&37_u32.to_le_bytes());

        let event = decode_game_event(MSG_SKILL_EFFECT, &Bytes::from(body))
            .expect("skill effect should decode");
        assert_eq!(
            event,
            GameEvent::SkillEffect(SkillEffect {
                attacker_id: 1_000_001,
                data: 378_u32 << 16 | 430,
                skill_id: 1_000,
                level: 1,
                targets: vec![(400_001, 37)],
            })
        );
    }

    #[test]
    fn decodes_refinery_info_record() {
        let mut body = vec![0_u8; 4 + 28];
        body[0..4].copy_from_slice(&1_u32.to_le_bytes());
        body[4..8].copy_from_slice(&77_u32.to_le_bytes());
        body[8..12].copy_from_slice(&6_u32.to_le_bytes());
        body[12..16].copy_from_slice(&820_001_u32.to_le_bytes());
        body[16..20].copy_from_slice(&1_u32.to_le_bytes());
        body[20..24].copy_from_slice(&25_i32.to_le_bytes());
        body[24..28].copy_from_slice(&604_800_u32.to_le_bytes());

        let event = decode_game_event(MSG_REFINERY_INFO, &Bytes::from(body))
            .expect("refinery info should decode");
        assert_eq!(
            event,
            GameEvent::RefineryInfo(RefineryInfoPacket {
                records: vec![RefineryInfoRecord {
                    item_id: 77,
                    action: 6,
                    refinery_type: 820_001,
                    level: 1,
                    effect: 25,
                    time: 604_800,
                    unknown: 0,
                }],
            })
        );
    }

    #[test]
    fn decodes_multiple_skill_effect_targets() {
        let mut body = vec![0_u8; 16 + 32 * 2];
        body[0..4].copy_from_slice(&7_u32.to_le_bytes());
        body[12..16].copy_from_slice(&2_u32.to_le_bytes());
        body[16..20].copy_from_slice(&101_u32.to_le_bytes());
        body[20..24].copy_from_slice(&11_u32.to_le_bytes());
        body[48..52].copy_from_slice(&202_u32.to_le_bytes());
        body[52..56].copy_from_slice(&22_u32.to_le_bytes());

        let GameEvent::SkillEffect(effect) =
            decode_game_event(MSG_SKILL_EFFECT, &Bytes::from(body)).unwrap()
        else {
            panic!("expected skill effect");
        };
        assert_eq!(effect.targets, vec![(101, 11), (202, 22)]);
    }

    #[test]
    fn decodes_learned_skill_update() {
        let mut body = vec![0_u8; 18];
        body[0..4].copy_from_slice(&123_u32.to_le_bytes());
        body[4..6].copy_from_slice(&1_000_u16.to_le_bytes());
        body[6..8].copy_from_slice(&2_u16.to_le_bytes());
        assert_eq!(
            decode_game_event(MSG_SKILL, &Bytes::from(body)).unwrap(),
            GameEvent::Skill(SkillPacket {
                experience: 123,
                skill_id: 1_000,
                level: 2,
                reserved: [0; 10],
            })
        );
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

    #[test]
    fn rejects_truncated_server_fixtures() {
        assert!(ActionPacket::decode(&Bytes::from(vec![0_u8; 19])).is_err());
        assert!(InteractionPacket::decode(&Bytes::from(vec![0_u8; 35])).is_err());
        assert!(MapInfo::decode(&Bytes::from(vec![0_u8; 8])).is_err());
        assert!(decode_game_event(MSG_SKILL_EFFECT, &Bytes::from(vec![0_u8; 15])).is_err());
        assert!(
            decode_game_event(
                MSG_REFINERY_INFO,
                &Bytes::from(1_u32.to_le_bytes().to_vec())
            )
            .is_err()
        );
        assert!(decode_game_event(MSG_TASK_DIALOG, &Bytes::from(vec![0_u8; 7])).is_err());
    }
}
