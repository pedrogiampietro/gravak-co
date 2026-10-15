//! The helper: what the hero does by itself. Three slots of potions or
//! heals for the hero, each used below its own share of life (or mana, for
//! a mana potion); heals and the Taoist's Stigma, Magic Shield and Star of
//! Accuracy on up to two players picked from the team, each with what they
//! get; Pray on the fallen; following the leader; and hunting: the nearest
//! monster around the spot where the hunt began, with a blow or the skill
//! bar's skill, and what falls picked up; fallen, revived when the server
//! allows it and walked back to the spot. Its merchant sells the junk when
//! the bag fills, repairs worn gear and buys the slots' potions when they
//! run out, at the shops' prices, wherever the hero is (`crate::idle` on
//! the server).
//!
//! It only does what a player could: every potion, cast and step goes to the
//! server as the player's own would, and is checked there the same way. The
//! choices live in `settings/helper.ini` beside the client.

use crate::assets::Assets;
use crate::components::{draw_card, draw_tab, fill, outline, WidgetState};
use crate::ui::UiRect;
use crate::{labels, ui_theme};
use fontdue::Font;
use image::{imageops, RgbaImage};
use std::collections::HashMap;
use std::path::PathBuf;

pub const CURE: u32 = 1_005;
pub const HEALING_RAIN: u32 = 1_055;
pub const ADVANCED_CURE: u32 = 1_175;
pub const NECTAR: u32 = 1_170;
pub const PRAY: u32 = 1_100;
pub const REVIVE: u32 = 1_050;
pub const STIGMA: u32 = 1_095;
pub const MAGIC_SHIELD: u32 = 1_090;
pub const STAR_OF_ACCURACY: u32 = 1_085;
pub const CYCLONE: u32 = 1_110;
pub const SUPERMAN: u32 = 1_025;

/// The XP skills the helper may use when the circle lights, in the order
/// the Hunt tab's arrows choose them.
pub const XP_SKILLS: [u32; 2] = [CYCLONE, SUPERMAN];
/// An XP skill refused goes again after this long.
const XP_AGAIN: f32 = 3.0;

/// The heals the helper may cast, weakest first.
pub const HEALS: [u32; 4] = [CURE, HEALING_RAIN, ADVANCED_CURE, NECTAR];

/// The status flags (`MsgPlayer`) the support buffs show.
pub const STIGMA_FLAG: u64 = 1 << 9;
pub const SHIELD_FLAG: u64 = 1 << 8;
pub const STAR_OF_ACCURACY_FLAG: u64 = 1 << 7;

/// The hero's slots, and how many players may be picked.
pub const SLOTS: usize = 3;
pub const MOST_PICKS: usize = 2;

/// How far heals and buffs reach another player, in cells.
const REACH: f32 = 10.0;
/// How far Pray reaches the fallen.
const REVIVE_REACH: f32 = 12.0;
/// Between two potions, and between two of the helper's casts.
const POTION_GAP: f32 = 1.0;
const CAST_GAP: f32 = 0.4;
/// A teammate's life is told once a second, so a heal waits for the news
/// before it goes on the same teammate again; a buff waits for the status.
const HEAL_AGAIN: f32 = 1.2;
const BUFF_AGAIN: f32 = 5.0;
/// A buff goes on the same player once every this many seconds (Stigma,
/// Magic Shield and Star of Accuracy last 60 to 80 by their level).
const BUFF_EVERY: f32 = 70.0;
const FOLLOW_GAP: f32 = 0.5;
/// Between two of the hunt's choices.
const HUNT_GAP: f32 = 0.4;
/// A target or an item chosen again this soon was not reached: it is left
/// alone for a while.
const UNREACHED_WITHIN: f32 = 1.5;
const LEAVE_ALONE: f32 = 20.0;
/// The hunt goes back to its spot when farther than this from it.
const HOME_SLACK: f32 = 3.0;
/// Below this much mana the hunt strikes instead of casting.
const HUNT_MANA_PERCENT: u32 = 10;
/// The bag's slots.
pub const BAG_SLOTS: usize = 40;
/// The server lets the fallen revive this long after dying (`REVIVE_DELAY`),
/// and the hunt asks a moment later; refused, it asks again after a while.
const REVIVE_AFTER: f32 = 20.5;
const REVIVE_AGAIN: f32 = 5.0;
/// The merchant sells when this few slots are left free.
const SELL_WHEN_FREE: usize = 2;
/// Between two of the merchant's errands of a kind.
const SELL_GAP: f32 = 30.0;
const REPAIR_GAP: f32 = 60.0;
const BUY_GAP: f32 = 30.0;
const DEPOSIT_GAP: f32 = 30.0;

pub fn skill_name(skill: u32) -> &'static str {
    match skill {
        CURE => "Cure",
        HEALING_RAIN => "Healing Rain",
        ADVANCED_CURE => "Advanced Cure",
        NECTAR => "Nectar",
        PRAY => "Pray",
        REVIVE => "Revive",
        STIGMA => "Stigma",
        MAGIC_SHIELD => "Magic Shield",
        STAR_OF_ACCURACY => "Star of Accuracy",
        CYCLONE => "Cyclone",
        SUPERMAN => "Superman",
        _ => "?",
    }
}

/// The least mana a skill takes (MagicType at level 0): the helper does not
/// try one the hero cannot pay for.
fn mana_cost(skill: u32) -> u32 {
    match skill {
        CURE => 10,
        HEALING_RAIN => 150,
        ADVANCED_CURE => 160,
        NECTAR => 600,
        PRAY => 1_000,
        STIGMA | MAGIC_SHIELD => 200,
        STAR_OF_ACCURACY => 200,
        _ => 0,
    }
}

/// Stigma, Magic Shield and Star of Accuracy go on the player aimed at.
pub fn is_ally_buff(skill: u32) -> bool {
    matches!(skill, STIGMA | MAGIC_SHIELD | STAR_OF_ACCURACY)
}

/// Whether an item restores mana (else life) when drunk, or is no potion.
fn restores_mana(item_type: u32) -> Option<bool> {
    if crate::shop::mana_recovery_for_item(item_type).is_some() {
        Some(true)
    } else if crate::shop::health_recovery_for_item(item_type).is_some() {
        Some(false)
    } else {
        None
    }
}

// ---------------------------------------------------------------- settings

/// What a slot holds.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum Use {
    #[default]
    Empty,
    /// A potion, by item type.
    Item(u32),
    /// A heal cast on the hero.
    Skill(u32),
}

/// One of the hero's slots: what it uses, below what share.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Slot {
    pub using: Use,
    pub percent: u8,
}

/// A player picked for heals and buffs, by name, with what they get.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Pick {
    pub name: String,
    pub heal: bool,
    pub stigma: bool,
    pub shield: bool,
    pub accuracy: bool,
}

impl Pick {
    fn new(name: String) -> Self {
        Self { name, heal: true, stigma: true, shield: false, accuracy: false }
    }

    fn flag_mut(&mut self, flag: Gift) -> &mut bool {
        match flag {
            Gift::Heal => &mut self.heal,
            Gift::Stigma => &mut self.stigma,
            Gift::Shield => &mut self.shield,
            Gift::Accuracy => &mut self.accuracy,
        }
    }

    pub fn has(&self, flag: Gift) -> bool {
        match flag {
            Gift::Heal => self.heal,
            Gift::Stigma => self.stigma,
            Gift::Shield => self.shield,
            Gift::Accuracy => self.accuracy,
        }
    }
}

/// What a picked player may be given.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Gift {
    Heal,
    Stigma,
    Shield,
    Accuracy,
}

impl Gift {
    const ALL: [Self; 4] = [Self::Heal, Self::Stigma, Self::Shield, Self::Accuracy];

    fn label(self) -> &'static str {
        match self {
            Self::Heal => "Heal",
            Self::Stigma => "Stigma",
            Self::Shield => "M. Shield",
            Self::Accuracy => "Accuracy",
        }
    }
}

/// What the player chose.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Settings {
    pub enabled: bool,
    pub slots: [Slot; SLOTS],
    pub picks: Vec<Pick>,
    pub team_heal_skill: u32,
    pub team_heal_percent: u8,
    pub revive: bool,
    pub follow: bool,
    pub follow_distance: u8,
    /// Hunting around where it began, within `hunt_radius` cells.
    pub hunt: bool,
    pub hunt_radius: u8,
    /// Picking up what falls.
    pub loot: bool,
    /// Casting the skill bar's skill rather than striking.
    pub hunt_skill: bool,
    /// Monsters at most this many levels above the hero.
    pub hunt_levels_above: u8,
    /// Fallen while hunting, revive and walk back to the spot.
    pub hunt_revive: bool,
    /// The XP skill (`XP_SKILLS[xp_skill]`, or the other when it is not
    /// learned) used as soon as the XP circle lights.
    pub use_xp: bool,
    pub xp_skill: u8,
    /// The merchant: sell the junk when the bag fills (the Sell All list),
    /// repair worn gear, buy `buy_amount` of a slot's potion when none is
    /// left.
    pub auto_sell: bool,
    pub auto_repair: bool,
    pub auto_buy: bool,
    pub buy_amount: u8,
    /// The porter: a valuable (`depositable`) goes to the depot once the
    /// bag holds `deposit_at` of it.
    pub auto_deposit: bool,
    pub deposit_at: u8,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            enabled: false,
            slots: [
                Slot { using: Use::Empty, percent: 50 },
                Slot { using: Use::Empty, percent: 50 },
                Slot { using: Use::Empty, percent: 30 },
            ],
            picks: Vec::new(),
            team_heal_skill: CURE,
            team_heal_percent: 70,
            revive: true,
            follow: false,
            follow_distance: 3,
            hunt: false,
            hunt_radius: 10,
            loot: true,
            hunt_skill: false,
            hunt_levels_above: 3,
            hunt_revive: true,
            use_xp: false,
            xp_skill: 0,
            auto_sell: false,
            auto_repair: true,
            auto_buy: true,
            buy_amount: 10,
            auto_deposit: false,
            deposit_at: 10,
        }
    }
}

/// A box the player ticks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Toggle {
    Revive,
    Follow,
    Hunt,
    Loot,
    HuntSkill,
    HuntRevive,
    UseXp,
    AutoSell,
    AutoRepair,
    AutoBuy,
    AutoDeposit,
}

/// A number the arrows change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Stepper {
    Slot(usize),
    TeamHeal,
    FollowDistance,
    HuntRadius,
    HuntLevels,
    XpSkill,
    BuyAmount,
    DepositAt,
}

impl Stepper {
    /// Its step and bounds.
    fn range(self) -> (u8, u8, u8) {
        match self {
            Self::FollowDistance => (1, 1, 10),
            Self::HuntRadius => (2, 4, 30),
            Self::HuntLevels => (1, 0, 20),
            Self::XpSkill => (1, 0, XP_SKILLS.len() as u8 - 1),
            Self::BuyAmount => (5, 5, 20),
            Self::DepositAt => (1, 1, 40),
            _ => (5, 5, 95),
        }
    }
}

/// What a choice is being made for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Choosing {
    Slot(usize),
    TeamHeal,
}

impl Settings {
    fn flip(&mut self, toggle: Toggle) {
        let value = match toggle {
            Toggle::Revive => &mut self.revive,
            Toggle::Follow => &mut self.follow,
            Toggle::Hunt => &mut self.hunt,
            Toggle::Loot => &mut self.loot,
            Toggle::HuntSkill => &mut self.hunt_skill,
            Toggle::HuntRevive => &mut self.hunt_revive,
            Toggle::UseXp => &mut self.use_xp,
            Toggle::AutoSell => &mut self.auto_sell,
            Toggle::AutoRepair => &mut self.auto_repair,
            Toggle::AutoBuy => &mut self.auto_buy,
            Toggle::AutoDeposit => &mut self.auto_deposit,
        };
        *value = !*value;
    }

    /// Moves a number one step up (`up`) or down, within its bounds.
    fn step(&mut self, stepper: Stepper, up: bool) {
        let (step, least, most) = stepper.range();
        let value = match stepper {
            Stepper::Slot(index) => &mut self.slots[index.min(SLOTS - 1)].percent,
            Stepper::TeamHeal => &mut self.team_heal_percent,
            Stepper::FollowDistance => &mut self.follow_distance,
            Stepper::HuntRadius => &mut self.hunt_radius,
            Stepper::HuntLevels => &mut self.hunt_levels_above,
            Stepper::XpSkill => &mut self.xp_skill,
            Stepper::BuyAmount => &mut self.buy_amount,
            Stepper::DepositAt => &mut self.deposit_at,
        };
        *value = if up { value.saturating_add(step) } else { value.saturating_sub(step) }.clamp(least, most);
    }

    fn format(&self) -> String {
        let flag = |value: bool| if value { "1" } else { "0" };
        let mut lines = vec![
            format!("enabled={}", flag(self.enabled)),
            format!("team_heal_skill={}", self.team_heal_skill),
            format!("team_heal_percent={}", self.team_heal_percent),
            format!("revive={}", flag(self.revive)),
            format!("follow={}", flag(self.follow)),
            format!("follow_distance={}", self.follow_distance),
            format!("hunt={}", flag(self.hunt)),
            format!("hunt_radius={}", self.hunt_radius),
            format!("loot={}", flag(self.loot)),
            format!("hunt_skill={}", flag(self.hunt_skill)),
            format!("hunt_levels_above={}", self.hunt_levels_above),
            format!("hunt_revive={}", flag(self.hunt_revive)),
            format!("use_xp={}", flag(self.use_xp)),
            format!("xp_skill={}", self.xp_skill),
            format!("auto_sell={}", flag(self.auto_sell)),
            format!("auto_repair={}", flag(self.auto_repair)),
            format!("auto_buy={}", flag(self.auto_buy)),
            format!("buy_amount={}", self.buy_amount),
            format!("auto_deposit={}", flag(self.auto_deposit)),
            format!("deposit_at={}", self.deposit_at),
        ];
        for (index, slot) in self.slots.iter().enumerate() {
            let using = match slot.using {
                Use::Empty => "empty".to_owned(),
                Use::Item(item) => format!("item:{item}"),
                Use::Skill(skill) => format!("skill:{skill}"),
            };
            lines.push(format!("slot{}={using}:{}", index + 1, slot.percent));
        }
        for (index, pick) in self.picks.iter().enumerate() {
            lines.push(format!(
                "pick{}={}:{}{}{}{}",
                index + 1,
                pick.name,
                flag(pick.heal),
                flag(pick.stigma),
                flag(pick.shield),
                flag(pick.accuracy)
            ));
        }
        lines.into_iter().map(|line| line + "\n").collect()
    }

    fn parse(text: &str) -> Self {
        let mut settings = Self::default();
        for line in text.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let value = value.trim();
            let flag = value == "1";
            let number = |least: u32, most: u32| value.parse::<u32>().ok().map(|number| number.clamp(least, most) as u8);
            match key.trim() {
                "enabled" => settings.enabled = flag,
                "revive" => settings.revive = flag,
                "follow" => settings.follow = flag,
                "team_heal_percent" => settings.team_heal_percent = number(5, 95).unwrap_or(settings.team_heal_percent),
                "follow_distance" => settings.follow_distance = number(1, 10).unwrap_or(settings.follow_distance),
                "hunt" => settings.hunt = flag,
                "hunt_radius" => settings.hunt_radius = number(4, 30).unwrap_or(settings.hunt_radius),
                "loot" => settings.loot = flag,
                "hunt_skill" => settings.hunt_skill = flag,
                "hunt_revive" => settings.hunt_revive = flag,
                "use_xp" => settings.use_xp = flag,
                "xp_skill" => settings.xp_skill = number(0, XP_SKILLS.len() as u32 - 1).unwrap_or(settings.xp_skill),
                "auto_sell" => settings.auto_sell = flag,
                "auto_repair" => settings.auto_repair = flag,
                "auto_buy" => settings.auto_buy = flag,
                "buy_amount" => settings.buy_amount = number(5, 20).unwrap_or(settings.buy_amount),
                "auto_deposit" => settings.auto_deposit = flag,
                "deposit_at" => settings.deposit_at = number(1, 40).unwrap_or(settings.deposit_at),
                "hunt_levels_above" => settings.hunt_levels_above = number(0, 20).unwrap_or(settings.hunt_levels_above),
                "team_heal_skill" => {
                    settings.team_heal_skill =
                        value.parse::<u32>().ok().filter(|skill| HEALS.contains(skill)).unwrap_or(settings.team_heal_skill);
                }
                key if key.starts_with("slot") => {
                    let Some(index) = key[4..].parse::<usize>().ok().filter(|index| (1..=SLOTS).contains(index)) else {
                        continue;
                    };
                    let mut parts = value.split(':');
                    let using = match (parts.next(), parts.next().and_then(|id| id.parse::<u32>().ok())) {
                        (Some("item"), Some(item)) if restores_mana(item).is_some() => Use::Item(item),
                        (Some("skill"), Some(skill)) if HEALS.contains(&skill) => Use::Skill(skill),
                        _ => Use::Empty,
                    };
                    let percent = value
                        .rsplit(':')
                        .next()
                        .and_then(|percent| percent.parse::<u32>().ok())
                        .map_or(50, |percent| percent.clamp(5, 95) as u8);
                    settings.slots[index - 1] = Slot { using, percent };
                }
                key if key.starts_with("pick") => {
                    let Some((name, flags)) = value.rsplit_once(':') else {
                        continue;
                    };
                    let bits: Vec<bool> = flags.chars().map(|bit| bit == '1').collect();
                    if name.is_empty() || bits.len() != 4 || settings.picks.len() >= MOST_PICKS {
                        continue;
                    }
                    settings.picks.push(Pick {
                        name: name.to_owned(),
                        heal: bits[0],
                        stigma: bits[1],
                        shield: bits[2],
                        accuracy: bits[3],
                    });
                }
                _ => {}
            }
        }
        settings
    }

    pub fn load() -> Self {
        path().and_then(|path| crate::saved::read(&path)).map_or_else(Self::default, |text| Self::parse(&text))
    }

    pub fn save(&self) {
        let Some(path) = path() else {
            return;
        };
        let result = crate::saved::write(&path, &self.format());
        if let Err(error) = result {
            eprintln!("Could not save the helper's settings: {error}");
        }
    }
}

fn path() -> Option<PathBuf> {
    crate::saved::path("helper.ini")
}

// ---------------------------------------------------------------- deciding

/// A teammate as the client knows it.
#[derive(Debug, Clone, PartialEq)]
pub struct Teammate {
    pub id: u32,
    pub name: String,
    /// Its life in hundredths, when the server has told it.
    pub life_percent: Option<u8>,
    /// Where it is drawn, when it is on screen.
    pub position: Option<[f32; 2]>,
    pub flags: u64,
    pub dead: bool,
}

/// What the helper looks at, taken from the client each frame.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct World {
    pub me: u32,
    pub my_name: String,
    pub position: [f32; 2],
    pub health: u32,
    pub maximum_health: u32,
    pub mana: u32,
    pub maximum_mana: u32,
    pub dead: bool,
    pub flags: u64,
    /// The skills learned.
    pub known: Vec<u32>,
    /// The skills still cooling down.
    pub cooling: Vec<u32>,
    /// The bag's potions: (uid, item type).
    pub potions: Vec<(u32, u32)>,
    /// The other members of the team (empty alone).
    pub team: Vec<Teammate>,
    pub leader: Option<u32>,
    /// Where the leader is, on this map.
    pub leader_at: Option<[f32; 2]>,
    /// The XP circle is full (for Revive).
    pub xp_ready: bool,
    /// Busy with something of the player's own (walking somewhere, an
    /// attack): the helper does not walk then.
    pub busy: bool,
    pub map: u32,
    pub level: u16,
    /// The monsters on screen.
    pub monsters: Vec<Monster>,
    /// What lies on the ground: (uid, cell).
    pub loot: Vec<(u32, [f32; 2])>,
    pub bag_full: bool,
    /// The skill bar's skill, when it strikes a target.
    pub attack_skill: Option<u32>,
    /// The skill being cast at a target: (target, skill).
    pub casting: Option<(u32, u32)>,
    /// How long the hero has lain dead.
    pub dead_for: f32,
    /// Under Heaven's Blessing, which revives where the body lies.
    pub blessed: bool,
    /// The bag's free slots.
    pub free_slots: usize,
    /// Some worn gear is low on durability.
    pub worn_low: bool,
    /// The bag's valuables (`depositable`): item type and how many.
    pub valuables: Vec<(u32, usize)>,
}

/// A monster as the client knows it.
#[derive(Debug, Clone, PartialEq)]
pub struct Monster {
    pub id: u32,
    pub name: String,
    pub level: u16,
    pub position: [f32; 2],
    pub alive: bool,
}

/// Something the helper does.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Act {
    Potion { uid: u32 },
    Cast { skill: u32, target: u32 },
    /// Walk toward where the leader is.
    Follow([f32; 2]),
    /// Strike a monster until it falls.
    Attack(u32),
    /// Cast a skill at a monster until it falls.
    CastAt { skill: u32, target: u32 },
    /// Pick up what lies on the ground.
    Pickup(u32),
    /// Walk back to where the hunt began.
    Home([f32; 2]),
    /// Revive: where the body lies (`here`), or in town.
    Revive { here: bool },
    /// The merchant sells the junk in the bag.
    SellJunk,
    /// The merchant repairs what is worn.
    Repair,
    /// The merchant brings `count` of a potion.
    Buy { item: u32, count: u8 },
    /// The porter carries the bag's `item`s to the depot.
    Deposit { item: u32 },
}

/// What the helper remembers between frames.
#[derive(Debug, Clone, Default)]
pub struct Memory {
    next_potion: f32,
    next_cast: f32,
    next_follow: f32,
    /// Until when a skill does not go on a target again.
    recent: HashMap<(u32, u32), f32>,
    next_hunt: f32,
    /// Where the hunt began, on which map.
    home: Option<(u32, [f32; 2])>,
    /// The last monster or item chosen, and when.
    last_chosen: Option<(u32, f32)>,
    /// Monsters and items left alone until then.
    left_alone: HashMap<u32, f32>,
    next_revive: f32,
    next_sell: f32,
    next_repair: f32,
    next_buy: f32,
    next_deposit: f32,
}

impl Memory {
    /// Where the hunt began, when it is hunting.
    pub fn home(&self) -> Option<[f32; 2]> {
        self.home.map(|(_, at)| at)
    }
}

fn distance(from: [f32; 2], to: [f32; 2]) -> f32 {
    (from[0] - to[0]).abs().max((from[1] - to[1]).abs())
}

fn percent(value: u32, maximum: u32) -> u32 {
    value.saturating_mul(100) / maximum.max(1)
}

/// A picked player as the helper sees them this frame.
struct Picked<'a> {
    pick: &'a Pick,
    id: u32,
    life: u32,
    flags: u64,
}

/// What to do now: at most one potion, one cast and one step a frame.
pub fn decide(settings: &Settings, world: &World, memory: &mut Memory, now: f32) -> Vec<Act> {
    let mut acts = Vec::new();
    // Turned off, the spot is forgotten: turned on again, it hunts there.
    if !settings.enabled {
        memory.home = None;
        return acts;
    }
    // Fallen while hunting: revived once the server allows it; the spot is
    // kept, so the hunt walks back to it.
    if world.dead {
        if settings.hunt && settings.hunt_revive && world.dead_for >= REVIVE_AFTER && now >= memory.next_revive {
            memory.next_revive = now + REVIVE_AGAIN;
            acts.push(Act::Revive { here: world.blessed });
        }
        return acts;
    }
    memory.recent.retain(|_, until| now < *until);
    let life = percent(world.health, world.maximum_health);
    let mana = if world.maximum_mana > 0 { percent(world.mana, world.maximum_mana) } else { 100 };

    // The slots' potions, in their order.
    if now >= memory.next_potion {
        let potion = settings.slots.iter().find_map(|slot| {
            let Use::Item(item) = slot.using else {
                return None;
            };
            let share = if restores_mana(item)? { mana } else { life };
            if share >= u32::from(slot.percent) {
                return None;
            }
            world.potions.iter().find(|(_, kind)| *kind == item).map(|(uid, _)| *uid)
        });
        if let Some(uid) = potion {
            memory.next_potion = now + POTION_GAP;
            acts.push(Act::Potion { uid });
        }
    }

    // One cast.
    if now >= memory.next_cast
        && let Some((skill, target, again)) = next_cast(settings, world, memory, life)
    {
        memory.next_cast = now + CAST_GAP;
        memory.recent.insert((skill, target), now + again);
        acts.push(Act::Cast { skill, target });
    }

    // Following the leader.
    if settings.follow
        && !world.busy
        && now >= memory.next_follow
        && world.leader.is_some_and(|leader| leader != world.me)
        && let Some(at) = world.leader_at
        && distance(world.position, at) > f32::from(settings.follow_distance)
    {
        memory.next_follow = now + FOLLOW_GAP;
        acts.push(Act::Follow(at));
    }

    // The merchant's errands, one a frame.
    if let Some(errand) = merchant(settings, world, memory, now) {
        acts.push(errand);
    }

    // Hunting, when not following somewhere.
    if !settings.hunt {
        memory.home = None;
    } else if !acts.iter().any(|act| matches!(act, Act::Follow(_)))
        && let Some(act) = hunt(settings, world, memory, now, mana)
    {
        acts.push(act);
    }
    acts
}

/// What the porter carries to the depot (the server's own rule): DragonBalls,
/// Meteors and their tears, gems and ExpBalls.
pub fn depositable(item_type: u32) -> bool {
    matches!(item_type, 1_088_000..=1_088_002 | 723_700) || (700_000..710_000).contains(&item_type)
}

/// The merchant's next errand: the valuables gathered carried to the depot,
/// the junk sold when the bag is nearly full, worn gear repaired, a slot's
/// potion brought when none is left.
fn merchant(settings: &Settings, world: &World, memory: &mut Memory, now: f32) -> Option<Act> {
    if settings.auto_deposit
        && now >= memory.next_deposit
        && let Some((item, _)) = world.valuables.iter().find(|(_, count)| *count >= usize::from(settings.deposit_at))
    {
        memory.next_deposit = now + DEPOSIT_GAP;
        return Some(Act::Deposit { item: *item });
    }
    if settings.auto_sell && world.free_slots <= SELL_WHEN_FREE && now >= memory.next_sell {
        memory.next_sell = now + SELL_GAP;
        return Some(Act::SellJunk);
    }
    if settings.auto_repair && world.worn_low && now >= memory.next_repair {
        memory.next_repair = now + REPAIR_GAP;
        return Some(Act::Repair);
    }
    if settings.auto_buy && world.free_slots > 0 && now >= memory.next_buy {
        let wanted = settings.slots.iter().find_map(|slot| match slot.using {
            Use::Item(item) if !world.potions.iter().any(|(_, kind)| *kind == item) => Some(item),
            _ => None,
        });
        if let Some(item) = wanted {
            memory.next_buy = now + BUY_GAP;
            // Potions stack: one free slot takes them all.
            return Some(Act::Buy { item, count: settings.buy_amount });
        }
    }
    None
}

/// The hunt's next step: out of mana, a strike instead of the cast; what
/// fell nearby picked up; the nearest monster within reach of the spot
/// attacked; else back to the spot.
fn hunt(settings: &Settings, world: &World, memory: &mut Memory, now: f32, mana: u32) -> Option<Act> {
    // The spot: where the hunt began on this map (beside the leader when
    // following one).
    let home = match (settings.follow, world.leader_at, memory.home) {
        (true, Some(at), _) if world.leader.is_some_and(|leader| leader != world.me) => at,
        (_, _, Some((map, at))) if map == world.map => at,
        _ => {
            memory.home = Some((world.map, world.position));
            world.position
        }
    };
    if let Some((target, _)) = world.casting
        && mana < HUNT_MANA_PERCENT
    {
        return Some(Act::Attack(target));
    }
    if world.busy || now < memory.next_hunt {
        return None;
    }
    memory.next_hunt = now + HUNT_GAP;
    memory.left_alone.retain(|_, until| now < *until);
    let radius = f32::from(settings.hunt_radius);
    // Far away (revived in town, say): back to the spot before anything.
    if distance(world.position, home) > radius + HOME_SLACK {
        return Some(Act::Home(home));
    }
    let near_home = |at: [f32; 2]| distance(home, at) <= radius;
    // Chosen again at once: it was not reached.
    let choose = |memory: &mut Memory, id: u32| -> bool {
        if let Some((last, at)) = memory.last_chosen
            && last == id
            && now - at < UNREACHED_WITHIN
        {
            memory.left_alone.insert(id, now + LEAVE_ALONE);
            memory.last_chosen = None;
            return false;
        }
        memory.last_chosen = Some((id, now));
        true
    };

    if settings.loot && !world.bag_full {
        let item = world
            .loot
            .iter()
            .filter(|(uid, at)| near_home(*at) && !memory.left_alone.contains_key(uid))
            .min_by(|a, b| distance(world.position, a.1).total_cmp(&distance(world.position, b.1)))
            .map(|(uid, _)| *uid);
        // Once tried, an item waits: one the server will not give (another
        // player's) is not walked to again and again.
        if let Some(uid) = item {
            memory.left_alone.insert(uid, now + LEAVE_ALONE);
            return Some(Act::Pickup(uid));
        }
    }
    let most_level = world.level.saturating_add(u16::from(settings.hunt_levels_above));
    let monster = world
        .monsters
        .iter()
        .filter(|monster| {
            monster.alive
                && monster.level <= most_level
                && !matches!(monster.name.as_str(), "Guard1" | "Guard2")
                && near_home(monster.position)
                && !memory.left_alone.contains_key(&monster.id)
        })
        .min_by(|a, b| distance(world.position, a.position).total_cmp(&distance(world.position, b.position)))
        .map(|monster| monster.id);
    if let Some(target) = monster {
        if !choose(memory, target) {
            return None;
        }
        return Some(match world.attack_skill {
            Some(skill) if settings.hunt_skill && mana >= HUNT_MANA_PERCENT && !world.cooling.contains(&skill) => {
                Act::CastAt { skill, target }
            }
            _ => Act::Attack(target),
        });
    }
    (distance(world.position, home) > HOME_SLACK).then_some(Act::Home(home))
}

/// The cast that matters most, and how long before it may go on the same
/// target again: the fallen first, then the hero's slots, the most hurt of
/// the picked, a buff a picked player lacks.
fn next_cast(settings: &Settings, world: &World, memory: &Memory, life: u32) -> Option<(u32, u32, f32)> {
    let ready = |skill: u32| {
        world.known.contains(&skill) && !world.cooling.contains(&skill) && world.mana >= mana_cost(skill)
    };
    let fresh = |skill: u32, target: u32| !memory.recent.contains_key(&(skill, target));
    let in_reach = |mate: &Teammate, reach: f32| mate.position.is_some_and(|at| distance(world.position, at) <= reach);

    if settings.revive {
        let skill = if ready(PRAY) {
            Some(PRAY)
        } else {
            (world.xp_ready && world.known.contains(&REVIVE) && !world.cooling.contains(&REVIVE)).then_some(REVIVE)
        };
        if let Some(skill) = skill
            && let Some(mate) = world
                .team
                .iter()
                .find(|mate| mate.dead && in_reach(mate, REVIVE_REACH) && fresh(skill, mate.id))
        {
            return Some((skill, mate.id, BUFF_AGAIN));
        }
    }
    // The XP circle lit: the chosen XP skill, or the other one learned.
    if settings.use_xp && world.xp_ready {
        let chosen = XP_SKILLS[usize::from(settings.xp_skill).min(XP_SKILLS.len() - 1)];
        let skill = std::iter::once(chosen)
            .chain(XP_SKILLS)
            .find(|skill| world.known.contains(skill) && !world.cooling.contains(skill) && fresh(*skill, world.me));
        if let Some(skill) = skill {
            return Some((skill, world.me, XP_AGAIN));
        }
    }
    for slot in &settings.slots {
        if let Use::Skill(skill) = slot.using
            && life < u32::from(slot.percent)
            && ready(skill)
        {
            return Some((skill, world.me, 0.0));
        }
    }
    // The picked: the hero by name, or a living teammate in reach.
    let picked: Vec<Picked> = settings
        .picks
        .iter()
        .filter_map(|pick| {
            if pick.name == world.my_name {
                return Some(Picked { pick, id: world.me, life, flags: world.flags });
            }
            let mate = world.team.iter().find(|mate| mate.name == pick.name)?;
            (!mate.dead && in_reach(mate, REACH)).then(|| Picked {
                pick,
                id: mate.id,
                life: mate.life_percent.map_or(100, u32::from),
                flags: mate.flags,
            })
        })
        .collect();
    if ready(settings.team_heal_skill)
        && let Some(hurt) = picked
            .iter()
            .filter(|one| one.pick.heal && one.life < u32::from(settings.team_heal_percent))
            .filter(|one| fresh(settings.team_heal_skill, one.id))
            .min_by_key(|one| one.life)
    {
        let again = if hurt.id == world.me { 0.0 } else { HEAL_AGAIN };
        return Some((settings.team_heal_skill, hurt.id, again));
    }
    for one in &picked {
        for (gift, skill, flag) in [
            (Gift::Stigma, STIGMA, STIGMA_FLAG),
            (Gift::Shield, MAGIC_SHIELD, SHIELD_FLAG),
            (Gift::Accuracy, STAR_OF_ACCURACY, STAR_OF_ACCURACY_FLAG),
        ] {
            if one.pick.has(gift) && one.flags & flag == 0 && ready(skill) && fresh(skill, one.id) {
                return Some((skill, one.id, BUFF_EVERY));
            }
        }
    }
    None
}

// ---------------------------------------------------------------- window

/// The window's size.
pub const SIZE: [u32; 2] = [720, 440];
const CONTENT_LEFT: i32 = 192;
const CONTENT_TOP: i32 = 56;
const CONTENT_WIDTH: i32 = 512;
const ROW: i32 = 48;

/// The window's tabs.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum Tab {
    #[default]
    Healing,
    Support,
    Follow,
    Hunt,
    Supplies,
}

impl Tab {
    pub const ALL: [Self; 5] = [Self::Healing, Self::Support, Self::Follow, Self::Hunt, Self::Supplies];

    fn label(self) -> &'static str {
        match self {
            Self::Healing => "Healing",
            Self::Support => "Support",
            Self::Follow => "Follow",
            Self::Hunt => "Hunt",
            Self::Supplies => "Supplies",
        }
    }

    fn blurb(self) -> &'static str {
        match self {
            Self::Healing => "Your potions and heals: each slot is used below its share.",
            Self::Support => "Pick up to two players; the helper looks after only them.",
            Self::Follow => "Walking after the team's leader.",
            Self::Hunt => "Fighting by itself around where the hunt begins.",
            Self::Supplies => "Selling, repairing, buying and storing wherever you are.",
        }
    }
}

/// The window as it is drawn.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct HelperView {
    pub settings: Settings,
    pub tab: Tab,
    /// The slot or skill a choice is being made for.
    pub choosing: Option<Choosing>,
    /// The skills learned.
    pub known: Vec<u32>,
    /// The bag's potions: item type and how many.
    pub potions: Vec<(u32, u32)>,
    /// The hero's name, then the teammates'.
    pub party: Vec<String>,
    /// The leader's name, when in a team.
    pub leader: Option<String>,
    /// The key that turns the helper on and off.
    pub toggle_key: String,
    /// What the helper is doing ("Stigma on Pedro").
    pub status: String,
    /// A word for the player ("Two at most").
    pub notice: String,
    pub hover: Option<HelperAction>,
}

/// A click in the window.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum HelperAction {
    Power,
    Tab(Tab),
    Toggle(Toggle),
    Step(Stepper, bool),
    /// Opens the choice for a slot or a skill.
    Choose(Choosing),
    /// The choice made.
    Chosen(Use),
    CancelChoice,
    /// Empties a slot.
    Clear(usize),
    /// Picks or unpicks a party member.
    Member(String),
    /// Gives or takes a gift from a picked player.
    Gift(usize, Gift),
    /// Opens the Sell All list, to choose what is never sold.
    KeepList,
    Close,
}

/// What a click asks of the client.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    Nothing,
    /// The settings changed: save them.
    Save,
    /// Turn the helper on or off.
    Power,
    /// Open the Sell All list (its BlackList keeps items from the merchant).
    KeepList,
    Close,
}

/// A click done: the window's state (`view`) and the settings change.
pub fn apply(settings: &mut Settings, view: &mut HelperView, action: HelperAction) -> Effect {
    view.notice.clear();
    match action {
        HelperAction::Close => return Effect::Close,
        HelperAction::KeepList => return Effect::KeepList,
        HelperAction::Power => return Effect::Power,
        HelperAction::Tab(tab) => {
            view.tab = tab;
            view.choosing = None;
            return Effect::Nothing;
        }
        HelperAction::Toggle(toggle) => settings.flip(toggle),
        HelperAction::Step(stepper, up) => settings.step(stepper, up),
        HelperAction::Choose(choosing) => {
            view.choosing = Some(choosing);
            return Effect::Nothing;
        }
        HelperAction::CancelChoice => {
            view.choosing = None;
            return Effect::Nothing;
        }
        HelperAction::Chosen(using) => match (view.choosing.take(), using) {
            (Some(Choosing::Slot(index)), using) => settings.slots[index.min(SLOTS - 1)].using = using,
            (Some(Choosing::TeamHeal), Use::Skill(skill)) => settings.team_heal_skill = skill,
            _ => return Effect::Nothing,
        },
        HelperAction::Clear(index) => settings.slots[index.min(SLOTS - 1)].using = Use::Empty,
        HelperAction::Member(name) => {
            if let Some(at) = settings.picks.iter().position(|pick| pick.name == name) {
                settings.picks.remove(at);
            } else if settings.picks.len() >= MOST_PICKS {
                view.notice = "Two at most: unpick one first.".into();
                return Effect::Nothing;
            } else {
                settings.picks.push(Pick::new(name));
            }
        }
        HelperAction::Gift(index, gift) => {
            let Some(pick) = settings.picks.get_mut(index) else {
                return Effect::Nothing;
            };
            let value = pick.flag_mut(gift);
            *value = !*value;
        }
    }
    Effect::Save
}

fn power_rect() -> UiRect {
    UiRect::new(16, 52, 160, 34)
}

fn tab_rect(index: usize) -> UiRect {
    UiRect::new(16, 104 + index as i32 * 34, 160, 28)
}

fn close_rect() -> UiRect {
    UiRect::new(SIZE[0] as i32 - 31, 6, 27, 30)
}

// The Healing tab: three slot cards.
fn slot_card(index: usize) -> UiRect {
    UiRect::new(CONTENT_LEFT + index as i32 * 172, CONTENT_TOP + 34, 162, 196)
}

fn slot_icon(index: usize) -> UiRect {
    let card = slot_card(index);
    UiRect::new(card.x + (card.width - 60) / 2, card.y + 28, 60, 60)
}

fn slot_clear(index: usize) -> UiRect {
    let card = slot_card(index);
    UiRect::new(card.x + card.width - 22, card.y + 6, 16, 16)
}

fn slot_stepper(index: usize) -> UiRect {
    let card = slot_card(index);
    UiRect::new(card.x + 20, card.y + 152, card.width - 40, 26)
}

// The Support tab: the party on the left, the picked on the right.
fn member_rect(index: usize) -> UiRect {
    UiRect::new(CONTENT_LEFT, CONTENT_TOP + 52 + index as i32 * 34, 168, 30)
}

fn pick_card(index: usize) -> UiRect {
    UiRect::new(CONTENT_LEFT + 180, CONTENT_TOP + 34 + index as i32 * 128, 332, 120)
}

fn gift_rect(pick: usize, gift: usize) -> UiRect {
    let card = pick_card(pick);
    UiRect::new(card.x + 14 + gift as i32 * 80, card.y + 34, 44, 44)
}

fn team_heal_icon() -> UiRect {
    UiRect::new(CONTENT_LEFT + 64, CONTENT_TOP + 300, 36, 36)
}

fn team_heal_stepper() -> UiRect {
    UiRect::new(CONTENT_LEFT + 110, CONTENT_TOP + 305, 112, 26)
}

fn revive_rect() -> UiRect {
    UiRect::new(CONTENT_LEFT + 290, CONTENT_TOP + 300, 222, 36)
}

// The Follow tab: a row.
fn row_rect(index: usize) -> UiRect {
    UiRect::new(CONTENT_LEFT, CONTENT_TOP + 26 + index as i32 * ROW, CONTENT_WIDTH, ROW - 6)
}

fn row_stepper(row: UiRect) -> UiRect {
    UiRect::new(row.x + row.width - 112, row.y + 11, 102, 24)
}

// The choice: a grid over the tab.
const CHOICE_COLUMNS: usize = 7;

fn choice_panel() -> UiRect {
    UiRect::new(CONTENT_LEFT - 4, CONTENT_TOP - 4, CONTENT_WIDTH + 8, 340)
}

fn choice_rect(index: usize) -> UiRect {
    let panel = choice_panel();
    let (column, row) = ((index % CHOICE_COLUMNS) as i32, (index / CHOICE_COLUMNS) as i32);
    UiRect::new(panel.x + 16 + column * 70, panel.y + 44 + row * 82, 52, 52)
}

fn choice_cancel() -> UiRect {
    let panel = choice_panel();
    UiRect::new(panel.x + panel.width - 110, panel.y + panel.height - 38, 96, 26)
}

/// The arrows at either end of a stepper.
fn arrows(rect: UiRect) -> (UiRect, UiRect) {
    (UiRect::new(rect.x, rect.y, 22, rect.height), UiRect::new(rect.x + rect.width - 22, rect.y, 22, rect.height))
}

/// What may go in the slot or skill being chosen, and whether it can.
fn choices(view: &HelperView) -> Vec<(Use, bool)> {
    let heals = HEALS.iter().map(|skill| (Use::Skill(*skill), view.known.contains(skill)));
    match view.choosing {
        Some(Choosing::Slot(_)) => view.potions.iter().map(|(item, _)| (Use::Item(*item), true)).chain(heals).collect(),
        Some(Choosing::TeamHeal) => heals.collect(),
        None => Vec::new(),
    }
}

/// The party list: the hero, then the teammates.
fn party(view: &HelperView) -> &[String] {
    &view.party[..view.party.len().min(6)]
}

fn stepper_at(rect: UiRect, stepper: Stepper, local: [f32; 2]) -> Option<HelperAction> {
    let (less, more) = arrows(rect);
    if less.contains_local(local) {
        Some(HelperAction::Step(stepper, false))
    } else if more.contains_local(local) {
        Some(HelperAction::Step(stepper, true))
    } else {
        None
    }
}

/// What a click at `local` (from the window's corner) does.
pub fn action_at(view: &HelperView, local: [f32; 2]) -> Option<HelperAction> {
    if close_rect().contains_local(local) {
        return Some(HelperAction::Close);
    }
    if power_rect().contains_local(local) {
        return Some(HelperAction::Power);
    }
    for (index, tab) in Tab::ALL.into_iter().enumerate() {
        if tab_rect(index).contains_local(local) {
            return Some(HelperAction::Tab(tab));
        }
    }
    if view.choosing.is_some() {
        if choice_cancel().contains_local(local) {
            return Some(HelperAction::CancelChoice);
        }
        let list = choices(view);
        if matches!(view.choosing, Some(Choosing::Slot(_))) && choice_rect(list.len()).contains_local(local) {
            return Some(HelperAction::Chosen(Use::Empty));
        }
        return list
            .into_iter()
            .enumerate()
            .find(|(index, (_, can))| *can && choice_rect(*index).contains_local(local))
            .map(|(_, (using, _))| HelperAction::Chosen(using));
    }
    match view.tab {
        Tab::Healing => {
            for index in 0..SLOTS {
                if view.settings.slots[index].using != Use::Empty && slot_clear(index).contains_local(local) {
                    return Some(HelperAction::Clear(index));
                }
                if slot_icon(index).contains_local(local) {
                    return Some(HelperAction::Choose(Choosing::Slot(index)));
                }
                if view.settings.slots[index].using != Use::Empty
                    && let Some(action) = stepper_at(slot_stepper(index), Stepper::Slot(index), local)
                {
                    return Some(action);
                }
            }
        }
        Tab::Support => {
            for (index, name) in party(view).iter().enumerate() {
                if member_rect(index).contains_local(local) {
                    return Some(HelperAction::Member(name.clone()));
                }
            }
            for pick in 0..view.settings.picks.len().min(MOST_PICKS) {
                for (column, gift) in Gift::ALL.into_iter().enumerate() {
                    if gift_rect(pick, column).contains_local(local) {
                        return Some(HelperAction::Gift(pick, gift));
                    }
                }
            }
            if team_heal_icon().contains_local(local) {
                return Some(HelperAction::Choose(Choosing::TeamHeal));
            }
            if let Some(action) = stepper_at(team_heal_stepper(), Stepper::TeamHeal, local) {
                return Some(action);
            }
            if revive_rect().contains_local(local) {
                return Some(HelperAction::Toggle(Toggle::Revive));
            }
        }
        Tab::Follow => {
            let row = row_rect(0);
            if let Some(action) = stepper_at(row_stepper(row), Stepper::FollowDistance, local) {
                return Some(action);
            }
            if row.contains_local(local) {
                return Some(HelperAction::Toggle(Toggle::Follow));
            }
        }
        Tab::Hunt | Tab::Supplies => {
            if view.tab == Tab::Supplies && keep_button().contains_local(local) {
                return Some(HelperAction::KeepList);
            }
            let rows: &[(Option<Toggle>, Option<Stepper>)] = if view.tab == Tab::Hunt { &HUNT_ROWS } else { &SUPPLY_ROWS };
            for (index, &(toggle, stepper)) in rows.iter().enumerate() {
                let row = row_rect(index);
                if let Some(stepper) = stepper
                    && let Some(action) = stepper_at(row_stepper(row), stepper, local)
                {
                    return Some(action);
                }
                if row.contains_local(local)
                    && let Some(toggle) = toggle
                {
                    return Some(HelperAction::Toggle(toggle));
                }
            }
        }
    }
    None
}

/// The Supplies tab's rows; the second only holds the BlackList's button.
const SUPPLY_ROWS: [(Option<Toggle>, Option<Stepper>); 5] = [
    (Some(Toggle::AutoSell), None),
    (None, None),
    (Some(Toggle::AutoRepair), None),
    (Some(Toggle::AutoBuy), Some(Stepper::BuyAmount)),
    (Some(Toggle::AutoDeposit), Some(Stepper::DepositAt)),
];

/// The button that opens the Sell All list, on the Supplies tab's second row.
fn keep_button() -> UiRect {
    row_stepper(row_rect(1))
}

/// The Hunt tab's rows: what each ticks, and what its arrows change.
const HUNT_ROWS: [(Option<Toggle>, Option<Stepper>); 6] = [
    (Some(Toggle::Hunt), Some(Stepper::HuntRadius)),
    (Some(Toggle::Loot), None),
    (Some(Toggle::HuntSkill), None),
    (None, Some(Stepper::HuntLevels)),
    (Some(Toggle::HuntRevive), None),
    (Some(Toggle::UseXp), Some(Stepper::XpSkill)),
];

/// Whether `local` is over the window.
pub fn covers(local: [f32; 2]) -> bool {
    local[0] >= 0.0 && local[1] >= 0.0 && local[0] < SIZE[0] as f32 && local[1] < SIZE[1] as f32
}

// ---------------------------------------------------------------- drawing

const GOLD: [u8; 3] = ui_theme::TEXT_GOLD;
const TEXT: [u8; 3] = ui_theme::TEXT;
const MUTED: [u8; 3] = ui_theme::TEXT_MUTED;
const WARN: [u8; 3] = [200, 140, 110];
const GREEN: [u8; 4] = [90, 200, 110, 255];
const TEAL: [u8; 4] = [60, 150, 170, 255];

fn text(image: &mut RgbaImage, font: &Font, words: &str, colour: [u8; 3], x: i32, y: i32, size: f32) {
    labels::text_sized(image, font, &[(words, colour)], [x as f32, y as f32], size);
}

fn width_of(font: &Font, words: &str, size: f32) -> i32 {
    crate::daily_rewards_panel::width_of(font, words, size) as i32
}

fn centred(image: &mut RgbaImage, font: &Font, words: &str, colour: [u8; 3], rect: UiRect, size: f32) {
    let width = width_of(font, words, size);
    text(image, font, words, colour, rect.x + (rect.width - width) / 2, rect.y + (rect.height - size as i32) / 2 - 1, size);
}

/// A tick box.
fn check(image: &mut RgbaImage, rect: UiRect, on: bool) {
    fill(image, rect, [8, 22, 30, 255]);
    outline(image, rect, if on { GREEN } else { [110, 120, 130, 255] });
    if on {
        fill(image, UiRect::new(rect.x + 4, rect.y + 4, rect.width - 8, rect.height - 8), [110, 220, 120, 255]);
    }
}

/// A box with < value > in it.
fn spinner(image: &mut RgbaImage, font: &Font, rect: UiRect, value: &str, hover: (bool, bool)) {
    fill(image, rect, [10, 30, 40, 255]);
    outline(image, rect, TEAL);
    let (less, more) = arrows(rect);
    for (arrow, lit, glyph) in [(less, hover.0, "<"), (more, hover.1, ">")] {
        fill(image, arrow, if lit { [20, 70, 84, 255] } else { [16, 44, 56, 255] });
        outline(image, arrow, TEAL);
        centred(image, font, glyph, if lit { [255, 230, 140] } else { TEXT }, arrow, 11.0);
    }
    centred(image, font, value, TEXT, UiRect::new(less.x + less.width, rect.y, rect.width - 44, rect.height), 10.0);
}

fn spinner_hover(view: &HelperView, stepper: Stepper) -> (bool, bool) {
    (
        view.hover == Some(HelperAction::Step(stepper, false)),
        view.hover == Some(HelperAction::Step(stepper, true)),
    )
}

/// The art of what a slot uses, `size` pixels square.
fn icon_of(assets: &mut Assets, using: Use, size: u32) -> Option<RgbaImage> {
    let art = match using {
        Use::Empty => return None,
        Use::Item(item) => crate::shop::item_icon_image(assets, item).ok()?,
        Use::Skill(skill) => crate::main_hud::skill_icon(assets, skill)?,
    };
    Some(imageops::resize(&art, size, size, imageops::FilterType::Triangle))
}

/// Darkens and greys `image`: something not learned, or not on.
fn dim(image: &mut RgbaImage) {
    for pixel in image.pixels_mut() {
        let [r, g, b, a] = pixel.0;
        let grey = ((u16::from(r) * 3 + u16::from(g) * 6 + u16::from(b)) / 10) as u8;
        let dark = (f32::from(grey) * 0.45) as u8;
        pixel.0 = [dark, dark, dark, a];
    }
}

/// An icon box: the art inside a frame, dimmed when `lit` is false.
fn icon_box(image: &mut RgbaImage, assets: &mut Assets, rect: UiRect, using: Use, lit: bool, hovered: bool) {
    fill(image, rect, [6, 18, 26, 255]);
    outline(image, rect, if hovered { [255, 218, 109, 255] } else if lit { TEAL } else { [70, 90, 100, 255] });
    if let Some(mut art) = icon_of(assets, using, (rect.width - 6) as u32) {
        if !lit {
            dim(&mut art);
        }
        imageops::overlay(image, &art, i64::from(rect.x + 3), i64::from(rect.y + 3));
    }
}

fn use_name(assets: &mut Assets, using: Use) -> String {
    match using {
        Use::Empty => "Empty".into(),
        Use::Item(item) => crate::shop::item_name(assets, item),
        Use::Skill(skill) => skill_name(skill).into(),
    }
}

/// The window's contents (the frame goes around them: `shop::framed`).
pub fn draw(image: &mut RgbaImage, font: &Font, view: &HelperView, assets: &mut Assets) {
    let settings = &view.settings;

    // On and off.
    let power = power_rect();
    let lit = view.hover == Some(HelperAction::Power);
    fill(
        image,
        power,
        match (settings.enabled, lit) {
            (true, false) => [18, 60, 30, 255],
            (true, true) => [26, 80, 40, 255],
            (false, false) => [50, 24, 20, 255],
            (false, true) => [70, 32, 26, 255],
        },
    );
    outline(image, power, if settings.enabled { GREEN } else { [190, 90, 80, 255] });
    let (word, colour) = if settings.enabled { ("Enabled", [120, 230, 130]) } else { ("Disabled", [235, 120, 110]) };
    centred(image, font, &format!("Helper: {word}"), colour, power, 12.0);
    text(image, font, &format!("On / off key: {}", view.toggle_key), MUTED, 18, power.y + power.height + 4, 9.0);

    // The tabs.
    for (index, tab) in Tab::ALL.into_iter().enumerate() {
        let rect = tab_rect(index);
        let hovered = view.hover == Some(HelperAction::Tab(tab));
        draw_tab(image, font, rect, "", view.tab == tab, hovered);
        text(image, font, tab.label(), if view.tab == tab { GOLD } else { TEXT }, rect.x + 10, rect.y + 8, 10.0);
    }
    // What it is doing.
    let foot = SIZE[1] as i32 - 78;
    text(image, font, "NOW", MUTED, 18, foot, 8.0);
    let status = if !settings.enabled {
        "Turned off."
    } else if view.status.is_empty() {
        "Watching."
    } else {
        view.status.as_str()
    };
    for (line, words) in wrap(font, status, 156.0, 9.0).iter().take(3).enumerate() {
        text(image, font, words, TEXT, 18, foot + 12 + line as i32 * 13, 9.0);
    }

    text(image, font, view.tab.label(), GOLD, CONTENT_LEFT, 48, 12.0);
    text(image, font, view.tab.blurb(), MUTED, CONTENT_LEFT, 64, 9.0);
    if view.choosing.is_some() {
        draw_choice(image, font, view, assets);
        return;
    }
    match view.tab {
        Tab::Healing => draw_healing(image, font, view, assets),
        Tab::Support => draw_support(image, font, view, assets),
        Tab::Follow => draw_follow(image, font, view),
        Tab::Hunt => draw_hunt(image, font, view),
        Tab::Supplies => draw_supplies(image, font, view),
    }
}

fn draw_follow(image: &mut RgbaImage, font: &Font, view: &HelperView) {
    let row = row_rect(0);
    let on = view.settings.follow;
    let hovered = view.hover == Some(HelperAction::Toggle(Toggle::Follow));
    draw_card(image, row, if hovered { WidgetState::Hovered } else { WidgetState::Normal });
    fill(image, UiRect::new(row.x, row.y, 3, row.height), if on { GREEN } else { [70, 90, 100, 255] });
    check(image, UiRect::new(row.x + 12, row.y + 14, 18, 18), on);
    text(image, font, "Follow the leader", if on { TEXT } else { MUTED }, row.x + 40, row.y + 8, 11.0);
    text(image, font, "Keeps within this many cells", MUTED, row.x + 40, row.y + 25, 9.0);
    let value = format!("{} cells", view.settings.follow_distance);
    spinner(image, font, row_stepper(row), &value, spinner_hover(view, Stepper::FollowDistance));
    // Whom it follows, or why it cannot.
    let (line, colour) = match (&view.leader, view.party.first()) {
        (None, _) => ("Not in a team: there is no leader to follow.".to_owned(), WARN),
        (Some(leader), Some(me)) if leader == me => ("You lead the team: the others follow you.".to_owned(), WARN),
        (Some(leader), _) => (format!("Follows {leader}."), TEXT),
    };
    text(image, font, &line, colour, CONTENT_LEFT + 4, row.y + row.height + 12, 10.0);
    text(image, font, "Walks only while you are not doing something yourself.", MUTED, CONTENT_LEFT, SIZE[1] as i32 - 50, 9.0);
}

fn draw_supplies(image: &mut RgbaImage, font: &Font, view: &HelperView) {
    let settings = &view.settings;
    let rows: [(&str, &str, bool, Option<String>); 5] = [
        ("Sell the junk when the bag fills", "Your Sell All list; valuables are always kept", settings.auto_sell, None),
        ("Never sell: the BlackList", "What the merchant always keeps", true, None),
        ("Repair worn gear", "Before it breaks", settings.auto_repair, None),
        ("Buy potions when they run out", "The Healing slots' own potions", settings.auto_buy, Some(format!("{} each", settings.buy_amount))),
        (
            "Store valuables in the depot",
            "DragonBalls, Meteors, gems, ExpBalls, of a kind",
            settings.auto_deposit,
            Some(format!("at {}", settings.deposit_at)),
        ),
    ];
    draw_rows(image, font, view, &rows, &SUPPLY_ROWS);
    // The BlackList's button.
    let button = keep_button();
    let lit = view.hover == Some(HelperAction::KeepList);
    fill(image, button, if lit { [40, 70, 80, 255] } else { [28, 48, 56, 255] });
    outline(image, button, TEAL);
    centred(image, font, "Edit list", if lit { GOLD } else { TEXT }, button, 10.0);
    text(image, font, "At the shops' own prices, wherever you are.", MUTED, CONTENT_LEFT, SIZE[1] as i32 - 50, 9.0);
}

/// Rows of ticks and arrows (the Hunt and Supplies tabs).
fn draw_rows(
    image: &mut RgbaImage,
    font: &Font,
    view: &HelperView,
    rows: &[(&str, &str, bool, Option<String>)],
    controls: &[(Option<Toggle>, Option<Stepper>)],
) {
    for (index, ((title, line, on, value), (toggle, stepper))) in rows.iter().zip(controls).enumerate() {
        let row = row_rect(index);
        let hovered = toggle.is_some_and(|toggle| view.hover == Some(HelperAction::Toggle(toggle)));
        draw_card(image, row, if hovered { WidgetState::Hovered } else { WidgetState::Normal });
        fill(image, UiRect::new(row.x, row.y, 3, row.height), if *on { GREEN } else { [70, 90, 100, 255] });
        let left = if toggle.is_some() {
            check(image, UiRect::new(row.x + 12, row.y + 14, 18, 18), *on);
            row.x + 40
        } else {
            row.x + 14
        };
        text(image, font, title, if *on { TEXT } else { MUTED }, left, row.y + 8, 11.0);
        text(image, font, line, MUTED, left, row.y + 25, 9.0);
        if let (Some(stepper), Some(value)) = (stepper, value) {
            spinner(image, font, row_stepper(row), value, spinner_hover(view, *stepper));
        }
    }
}

fn draw_hunt(image: &mut RgbaImage, font: &Font, view: &HelperView) {
    let settings = &view.settings;
    let xp = XP_SKILLS[usize::from(settings.xp_skill).min(XP_SKILLS.len() - 1)];
    let rows: [(&str, &str, bool, Option<String>); 6] = [
        ("Hunt", "Monsters around where it begins", settings.hunt, Some(format!("{} cells", settings.hunt_radius))),
        ("Pick up items", "What falls nearby, while the bag has room", settings.loot, None),
        ("Use the skill bar's skill", "Strikes instead when mana runs low", settings.hunt_skill, None),
        ("Strongest monsters", "Up to this many levels above yours", true, Some(format!("+{} levels", settings.hunt_levels_above))),
        ("Revive and come back", "Fallen, revives when it may and walks back", settings.hunt_revive, None),
        ("Use the XP skill", "As soon as the XP circle lights", settings.use_xp, Some(skill_name(xp).to_owned())),
    ];
    draw_rows(image, font, view, &rows, &HUNT_ROWS);
    let note = if settings.follow && view.leader.is_some() {
        "Following a leader: hunts around them."
    } else {
        "Hunts around where it is turned on; potions and heals keep working."
    };
    text(image, font, note, MUTED, CONTENT_LEFT, SIZE[1] as i32 - 50, 9.0);
}

fn draw_healing(image: &mut RgbaImage, font: &Font, view: &HelperView, assets: &mut Assets) {
    for (index, slot) in view.settings.slots.iter().enumerate() {
        let card = slot_card(index);
        draw_card(image, card, WidgetState::Normal);
        text(image, font, &format!("Slot {}", index + 1), GOLD, card.x + 10, card.y + 8, 10.0);
        let icon = slot_icon(index);
        let hovered = view.hover == Some(HelperAction::Choose(Choosing::Slot(index)));
        let (usable, caption, count) = match slot.using {
            Use::Empty => (false, "Click to choose".to_owned(), None),
            Use::Item(item) => {
                let count = view.potions.iter().find(|(kind, _)| *kind == item).map_or(0, |(_, count)| *count);
                let meter = if restores_mana(item) == Some(true) { "mana" } else { "life" };
                let caption = if count > 0 { format!("when {meter} is below") } else { "none in the bag".to_owned() };
                (count > 0, caption, Some(count))
            }
            Use::Skill(skill) => {
                let learned = view.known.contains(&skill);
                (learned, if learned { "when life is below".to_owned() } else { "not learned".to_owned() }, None)
            }
        };
        icon_box(image, assets, icon, slot.using, usable, hovered);
        if slot.using == Use::Empty {
            centred(image, font, "+", if hovered { GOLD } else { MUTED }, icon, 22.0);
        }
        if let Some(count) = count {
            let words = count.to_string();
            let width = width_of(font, &words, 10.0);
            fill(image, UiRect::new(icon.x + icon.width - width - 6, icon.y + icon.height - 15, width + 5, 13), [0, 0, 0, 190]);
            let colour = if count > 0 { TEXT } else { [235, 120, 110] };
            text(image, font, &words, colour, icon.x + icon.width - width - 4, icon.y + icon.height - 14, 10.0);
        }
        let name = fit(font, &use_name(assets, slot.using), (card.width - 12) as f32, 10.0);
        centred(image, font, &name, if usable { TEXT } else { MUTED }, UiRect::new(card.x + 4, card.y + 96, card.width - 8, 14), 10.0);
        let caption_colour = if slot.using != Use::Empty && !usable { WARN } else { MUTED };
        centred(image, font, &caption, caption_colour, UiRect::new(card.x + 4, card.y + 116, card.width - 8, 14), 9.0);
        if slot.using != Use::Empty {
            let clear = slot_clear(index);
            let lit = view.hover == Some(HelperAction::Clear(index));
            outline(image, clear, if lit { [230, 130, 120, 255] } else { [90, 100, 110, 255] });
            centred(image, font, "x", if lit { [230, 130, 120] } else { MUTED }, clear, 10.0);
            let value = format!("{}%", slot.percent);
            spinner(image, font, slot_stepper(index), &value, spinner_hover(view, Stepper::Slot(index)));
        }
    }
    let foot = SIZE[1] as i32 - 74;
    text(image, font, "Click a slot to put a potion from your bag or a heal in it.", MUTED, CONTENT_LEFT, foot, 9.0);
    text(image, font, "A mana potion is drunk by your mana; the rest by your life.", MUTED, CONTENT_LEFT, foot + 14, 9.0);
}

fn draw_support(image: &mut RgbaImage, font: &Font, view: &HelperView, assets: &mut Assets) {
    let settings = &view.settings;
    // The party.
    text(image, font, "YOUR PARTY", MUTED, CONTENT_LEFT, CONTENT_TOP + 36, 9.0);
    for (index, name) in party(view).iter().enumerate() {
        let rect = member_rect(index);
        let picked = settings.picks.iter().any(|pick| &pick.name == name);
        let hovered = view.hover == Some(HelperAction::Member(name.clone()));
        let state = if picked {
            WidgetState::Selected
        } else if hovered {
            WidgetState::Hovered
        } else {
            WidgetState::Normal
        };
        draw_card(image, rect, state);
        check(image, UiRect::new(rect.x + 8, rect.y + 7, 16, 16), picked);
        let label = if index == 0 { format!("{name} (you)") } else { name.clone() };
        let label = fit(font, &label, 130.0, 10.0);
        text(image, font, &label, if picked { TEXT } else { MUTED }, rect.x + 32, rect.y + 9, 10.0);
    }
    let below = member_rect(party(view).len()).y + 4;
    let hint = if view.party.len() <= 1 { "Join a team to pick teammates." } else { "Click to pick, two at most." };
    text(image, font, hint, MUTED, CONTENT_LEFT, below, 9.0);

    // The picked, each with what they get.
    for index in 0..MOST_PICKS {
        let card = pick_card(index);
        draw_card(image, card, WidgetState::Normal);
        let Some(pick) = settings.picks.get(index) else {
            centred(image, font, "Pick someone from your party", MUTED, card, 10.0);
            continue;
        };
        let here = view.party.iter().any(|name| name == &pick.name);
        text(image, font, &pick.name, if here { GOLD } else { MUTED }, card.x + 12, card.y + 10, 11.0);
        if !here {
            let note = "not in your party now";
            let width = width_of(font, note, 9.0);
            text(image, font, note, WARN, card.x + card.width - width - 12, card.y + 12, 9.0);
        }
        for (column, gift) in Gift::ALL.into_iter().enumerate() {
            let rect = gift_rect(index, column);
            let skill = match gift {
                Gift::Heal => settings.team_heal_skill,
                Gift::Stigma => STIGMA,
                Gift::Shield => MAGIC_SHIELD,
                Gift::Accuracy => STAR_OF_ACCURACY,
            };
            let on = pick.has(gift);
            let learned = view.known.contains(&skill);
            let hovered = view.hover == Some(HelperAction::Gift(index, gift));
            icon_box(image, assets, rect, Use::Skill(skill), on && learned, hovered);
            if on {
                outline(image, UiRect::new(rect.x - 2, rect.y - 2, rect.width + 4, rect.height + 4), GREEN);
            }
            let label_rect = UiRect::new(rect.x - 14, rect.y + rect.height + 4, rect.width + 28, 12);
            let label = if learned { gift.label() } else { "not learned" };
            let colour = if !learned {
                WARN
            } else if on {
                TEXT
            } else {
                MUTED
            };
            centred(image, font, label, colour, label_rect, 9.0);
        }
    }

    // The heal they get, and the fallen.
    let foot = CONTENT_TOP + 300;
    text(image, font, "Heal with", TEXT, CONTENT_LEFT, foot + 12, 10.0);
    let skill = settings.team_heal_skill;
    let hovered = view.hover == Some(HelperAction::Choose(Choosing::TeamHeal));
    icon_box(image, assets, team_heal_icon(), Use::Skill(skill), view.known.contains(&skill), hovered);
    let value = format!("below {}%", settings.team_heal_percent);
    spinner(image, font, team_heal_stepper(), &value, spinner_hover(view, Stepper::TeamHeal));
    let revive = revive_rect();
    let on = settings.revive;
    let hovered = view.hover == Some(HelperAction::Toggle(Toggle::Revive));
    draw_card(image, revive, if hovered { WidgetState::Hovered } else { WidgetState::Normal });
    check(image, UiRect::new(revive.x + 10, revive.y + 10, 16, 16), on);
    text(image, font, "Revive the fallen", if on { TEXT } else { MUTED }, revive.x + 34, revive.y + 5, 10.0);
    text(image, font, "Pray, or Revive with a full XP", MUTED, revive.x + 34, revive.y + 19, 9.0);
    if !view.notice.is_empty() {
        text(image, font, &view.notice, [255, 200, 120], CONTENT_LEFT, SIZE[1] as i32 - 50, 10.0);
    }
}

fn draw_choice(image: &mut RgbaImage, font: &Font, view: &HelperView, assets: &mut Assets) {
    let panel = choice_panel();
    fill(image, panel, [8, 20, 28, 245]);
    outline(image, panel, [255, 218, 109, 255]);
    let title = match view.choosing {
        Some(Choosing::Slot(index)) => format!("Choose for slot {}", index + 1),
        Some(Choosing::TeamHeal) => "Choose the heal for the picked".into(),
        None => String::new(),
    };
    text(image, font, &title, GOLD, panel.x + 16, panel.y + 12, 12.0);
    let hint = match view.choosing {
        Some(Choosing::Slot(_)) => "Potions in your bag, then heals. Greyed: not learned.",
        _ => "Greyed: not learned.",
    };
    text(image, font, hint, MUTED, panel.x + 16, panel.y + 28, 9.0);
    let list = choices(view);
    for (index, (using, can)) in list.iter().copied().enumerate() {
        let rect = choice_rect(index);
        let hovered = view.hover == Some(HelperAction::Chosen(using));
        icon_box(image, assets, rect, using, can, hovered && can);
        let name = fit(font, &use_name(assets, using), 66.0, 9.0);
        let label = UiRect::new(rect.x - 8, rect.y + rect.height + 3, rect.width + 16, 12);
        centred(image, font, &name, if can { TEXT } else { MUTED }, label, 9.0);
        if let Use::Item(item) = using {
            let count = view.potions.iter().find(|(kind, _)| *kind == item).map_or(0, |(_, count)| *count);
            let words = count.to_string();
            let width = width_of(font, &words, 9.0);
            text(image, font, &words, TEXT, rect.x + rect.width - width - 3, rect.y + rect.height - 13, 9.0);
        }
    }
    if matches!(view.choosing, Some(Choosing::Slot(_))) {
        let rect = choice_rect(list.len());
        let hovered = view.hover == Some(HelperAction::Chosen(Use::Empty));
        fill(image, rect, [6, 18, 26, 255]);
        outline(image, rect, if hovered { [255, 218, 109, 255] } else { [70, 90, 100, 255] });
        centred(image, font, "x", MUTED, rect, 16.0);
        centred(image, font, "Empty", MUTED, UiRect::new(rect.x - 8, rect.y + rect.height + 3, rect.width + 16, 12), 9.0);
    }
    let cancel = choice_cancel();
    let hovered = view.hover == Some(HelperAction::CancelChoice);
    fill(image, cancel, if hovered { [20, 54, 66, 255] } else { [10, 30, 40, 255] });
    outline(image, cancel, TEAL);
    centred(image, font, "Cancel", TEXT, cancel, 10.0);
}

/// `words` cut with an ellipsis to fit `width`.
fn fit(font: &Font, words: &str, width: f32, size: f32) -> String {
    if crate::daily_rewards_panel::width_of(font, words, size) <= width {
        return words.to_owned();
    }
    let mut cut: String = words.to_owned();
    while !cut.is_empty() && crate::daily_rewards_panel::width_of(font, &format!("{cut}..."), size) > width {
        cut.pop();
    }
    format!("{cut}...")
}

/// `words` broken into lines no wider than `width`.
fn wrap(font: &Font, words: &str, width: f32, size: f32) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for word in words.split_whitespace() {
        match lines.last_mut() {
            Some(line) if crate::daily_rewards_panel::width_of(font, &format!("{line} {word}"), size) <= width => {
                line.push(' ');
                line.push_str(word);
            }
            _ => lines.push(word.to_owned()),
        }
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIFE_POTION: u32 = 1_000_030;
    const MANA_POTION: u32 = 1_001_000;

    fn world() -> World {
        World {
            me: 1,
            my_name: "Pedro".into(),
            position: [100.0, 100.0],
            health: 1_000,
            maximum_health: 1_000,
            mana: 2_000,
            maximum_mana: 2_000,
            known: vec![CURE, ADVANCED_CURE, PRAY, STIGMA, MAGIC_SHIELD],
            ..Default::default()
        }
    }

    fn mate(id: u32, name: &str, life: u8, position: [f32; 2]) -> Teammate {
        Teammate {
            id,
            name: name.into(),
            life_percent: Some(life),
            position: Some(position),
            flags: STIGMA_FLAG,
            dead: false,
        }
    }

    fn on() -> Settings {
        Settings { enabled: true, ..Settings::default() }
    }

    fn hunting() -> Settings {
        Settings { enabled: true, hunt: true, ..Settings::default() }
    }

    fn monster(id: u32, name: &str, level: u16, position: [f32; 2]) -> Monster {
        Monster { id, name: name.into(), level, position, alive: true }
    }

    fn hunter() -> World {
        World { level: 20, map: 1015, ..world() }
    }

    #[test]
    fn the_hunt_strikes_the_nearest_monster_it_may_take() {
        let mut world = hunter();
        world.monsters = vec![
            monster(400_001, "Pheasant", 1, [106.0, 100.0]),
            monster(400_002, "Guard1", 1, [101.0, 100.0]),
            monster(400_003, "Bandit", 30, [102.0, 100.0]),
            monster(400_004, "Turtledove", 22, [104.0, 104.0]),
            Monster { alive: false, ..monster(400_005, "Robin", 5, [100.0, 101.0]) },
            monster(400_006, "Hawk", 5, [140.0, 100.0]),
        ];
        // The guard, the one far above, the fallen and the one out of reach
        // are left; of the rest, the nearest.
        assert_eq!(decide(&hunting(), &world, &mut Memory::default(), 0.0), vec![Act::Attack(400_004)]);
    }

    #[test]
    fn what_fell_is_picked_up_first_while_the_bag_has_room() {
        let mut world = hunter();
        world.monsters = vec![monster(400_001, "Pheasant", 1, [101.0, 100.0])];
        world.loot = vec![(7, [103.0, 100.0]), (8, [130.0, 100.0])];
        assert_eq!(decide(&hunting(), &world, &mut Memory::default(), 0.0), vec![Act::Pickup(7)]);
        world.bag_full = true;
        assert_eq!(decide(&hunting(), &world, &mut Memory::default(), 0.0), vec![Act::Attack(400_001)]);
        // Tried once, an item waits before it is tried again.
        world.bag_full = false;
        let mut memory = Memory::default();
        decide(&hunting(), &world, &mut memory, 0.0);
        assert_eq!(decide(&hunting(), &world, &mut memory, 1.0), vec![Act::Attack(400_001)]);
    }

    #[test]
    fn the_skill_is_cast_while_mana_lasts() {
        let mut world = hunter();
        world.monsters = vec![monster(400_001, "Pheasant", 1, [103.0, 100.0])];
        world.attack_skill = Some(1_000);
        let settings = Settings { hunt_skill: true, ..hunting() };
        assert_eq!(decide(&settings, &world, &mut Memory::default(), 0.0), vec![Act::CastAt { skill: 1_000, target: 400_001 }]);
        // Out of mana in the middle of a cast: a strike on the same target.
        world.mana = 100;
        world.casting = Some((400_001, 1_000));
        world.busy = true;
        assert_eq!(decide(&settings, &world, &mut Memory::default(), 0.0), vec![Act::Attack(400_001)]);
    }

    #[test]
    fn with_nothing_to_hunt_the_hero_goes_back_to_the_spot() {
        let mut world = hunter();
        let mut memory = Memory::default();
        assert!(decide(&hunting(), &world, &mut memory, 0.0).is_empty(), "the spot is here");
        world.position = [110.0, 100.0];
        assert_eq!(decide(&hunting(), &world, &mut memory, 1.0), vec![Act::Home([100.0, 100.0])]);
        // Another map: a new spot.
        world.map = 1002;
        assert!(decide(&hunting(), &world, &mut memory, 2.0).is_empty());
        // Turned off and on elsewhere: a new spot there too.
        decide(&Settings { enabled: false, ..hunting() }, &world, &mut memory, 3.0);
        world.position = [130.0, 100.0];
        assert!(decide(&hunting(), &world, &mut memory, 4.0).is_empty());
    }

    #[test]
    fn a_monster_not_reached_is_left_alone() {
        let mut world = hunter();
        world.monsters = vec![monster(400_001, "Pheasant", 1, [103.0, 100.0]), monster(400_002, "Robin", 1, [105.0, 100.0])];
        let mut memory = Memory::default();
        assert_eq!(decide(&hunting(), &world, &mut memory, 0.0), vec![Act::Attack(400_001)]);
        // Chosen again at once (the way to it is shut): the next one.
        assert!(decide(&hunting(), &world, &mut memory, 0.5).is_empty());
        assert_eq!(decide(&hunting(), &world, &mut memory, 1.0), vec![Act::Attack(400_002)]);
    }

    /// A whole hunt played out frame by frame against a small world: each
    /// act done as the client would (a blow fells a monster after a while,
    /// a pickup takes the item, steps walk home), until nothing is left.
    #[test]
    fn a_hunt_clears_the_spot_and_comes_home() {
        let settings = hunting();
        let mut world = hunter();
        world.monsters = vec![
            monster(400_001, "Pheasant", 1, [104.0, 100.0]),
            monster(400_002, "Pheasant", 1, [96.0, 103.0]),
            monster(400_003, "Robin", 2, [100.0, 108.0]),
            monster(400_004, "Bandit", 90, [101.0, 101.0]),
        ];
        world.loot = vec![(7, [98.0, 98.0])];
        let mut memory = Memory::default();
        // What the hero is busy with: a target and how long it still takes.
        let mut fighting: Option<(u32, u32)> = None;
        let mut picked = Vec::new();
        for frame in 0..400 {
            let now = frame as f32 * 0.1;
            if let Some((target, left)) = fighting {
                if left == 0 {
                    // Felled: it dies and drops something.
                    let fallen = world.monsters.iter_mut().find(|monster| monster.id == target).unwrap();
                    fallen.alive = false;
                    world.loot.push((target, fallen.position));
                    fighting = None;
                } else {
                    fighting = Some((target, left - 1));
                }
            }
            world.busy = fighting.is_some();
            for act in decide(&settings, &world, &mut memory, now) {
                match act {
                    Act::Attack(target) => {
                        world.position = world.monsters.iter().find(|monster| monster.id == target).unwrap().position;
                        fighting = Some((target, 5));
                    }
                    Act::Pickup(uid) => {
                        let at = world.loot.iter().position(|(item, _)| *item == uid).unwrap();
                        world.position = world.loot.remove(at).1;
                        picked.push(uid);
                    }
                    Act::Home(at) => world.position = at,
                    other => panic!("unexpected {other:?}"),
                }
            }
        }
        let standing: Vec<u32> = world.monsters.iter().filter(|monster| monster.alive).map(|monster| monster.id).collect();
        assert_eq!(standing, vec![400_004], "all but the one far above are felled");
        assert!(world.loot.is_empty(), "everything picked up: {:?}", world.loot);
        assert_eq!(picked.len(), 4, "the item there and what the three dropped");
        assert_eq!(world.position, [100.0, 100.0], "back at the spot");
    }

    #[test]
    fn the_fallen_hunter_revives_and_walks_back() {
        let settings = hunting();
        let mut world = hunter();
        let mut memory = Memory::default();
        decide(&settings, &world, &mut memory, 0.0);
        // Dead: nothing until the server allows reviving, then once.
        world.dead = true;
        world.dead_for = 10.0;
        assert!(decide(&settings, &world, &mut memory, 10.0).is_empty());
        world.dead_for = 21.0;
        assert_eq!(decide(&settings, &world, &mut memory, 21.0), vec![Act::Revive { here: false }]);
        assert!(decide(&settings, &world, &mut memory, 22.0).is_empty(), "not asked again at once");
        world.blessed = true;
        assert_eq!(decide(&settings, &world, &mut memory, 26.5), vec![Act::Revive { here: true }]);
        // Revived in town on the same map: back to the spot.
        world.dead = false;
        world.position = [430.0, 380.0];
        assert_eq!(decide(&settings, &world, &mut memory, 30.0), vec![Act::Home([100.0, 100.0])]);
        // Without it, the fallen stay down.
        let mut stay = Memory::default();
        world.dead = true;
        assert!(decide(&Settings { hunt_revive: false, ..settings }, &world, &mut stay, 40.0).is_empty());
    }

    #[test]
    fn far_from_the_spot_it_walks_back_first() {
        let mut world = hunter();
        let mut memory = Memory::default();
        decide(&hunting(), &world, &mut memory, 0.0);
        // Revived in town: loot and monsters there wait.
        world.position = [400.0, 400.0];
        world.monsters = vec![monster(400_001, "Pheasant", 1, [401.0, 400.0])];
        world.loot = vec![(7, [400.0, 401.0])];
        assert_eq!(decide(&hunting(), &world, &mut memory, 1.0), vec![Act::Home([100.0, 100.0])]);
    }

    #[test]
    fn the_xp_skill_goes_off_when_the_circle_lights() {
        let mut settings = Settings { use_xp: true, ..on() };
        let mut world = world();
        world.known.push(SUPERMAN);
        let mut memory = Memory::default();
        // No circle, nothing.
        assert!(decide(&settings, &world, &mut memory, 0.0).is_empty());
        // Lit: Cyclone is chosen but not learned, so Superman.
        world.xp_ready = true;
        assert_eq!(decide(&settings, &world, &mut memory, 1.0), vec![Act::Cast { skill: SUPERMAN, target: 1 }]);
        // Both learned: the chosen one.
        world.known.push(CYCLONE);
        assert_eq!(decide(&settings, &world, &mut Memory::default(), 2.0), vec![Act::Cast { skill: CYCLONE, target: 1 }]);
        settings.step(Stepper::XpSkill, true);
        assert_eq!(decide(&settings, &world, &mut Memory::default(), 2.0), vec![Act::Cast { skill: SUPERMAN, target: 1 }]);
        // Turned off, the circle waits for the player.
        settings.use_xp = false;
        assert!(decide(&settings, &world, &mut Memory::default(), 3.0).is_empty());
    }

    #[test]
    fn the_merchant_sells_repairs_and_brings_potions() {
        let mut settings = Settings { enabled: true, auto_sell: true, ..Settings::default() };
        settings.slots[0] = Slot { using: Use::Item(LIFE_POTION), percent: 50 };
        let mut world = world();
        world.free_slots = 10;
        world.potions = vec![(9, LIFE_POTION)];
        let mut memory = Memory::default();
        // Nothing wanted.
        assert!(decide(&settings, &world, &mut memory, 0.0).is_empty());
        // The bag nearly full: the junk goes, once in a while.
        world.free_slots = 2;
        assert_eq!(decide(&settings, &world, &mut memory, 1.0), vec![Act::SellJunk]);
        assert!(decide(&settings, &world, &mut memory, 2.0).is_empty());
        // Worn gear.
        world.worn_low = true;
        assert_eq!(decide(&settings, &world, &mut memory, 3.0), vec![Act::Repair]);
        // The slot's potion run out: brought, as many as the bag takes.
        world.potions.clear();
        assert_eq!(decide(&settings, &world, &mut memory, 4.0), vec![Act::Buy { item: LIFE_POTION, count: 10 }]);
        // Ten Meteors gathered: carried to the depot; nine wait.
        settings.auto_deposit = true;
        world.valuables = vec![(1_088_000, 9)];
        assert!(decide(&settings, &world, &mut memory, 5.0).is_empty());
        world.valuables = vec![(1_088_000, 9), (1_088_001, 10)];
        assert_eq!(decide(&settings, &world, &mut memory, 6.0), vec![Act::Deposit { item: 1_088_001 }]);
        assert!(depositable(1_088_001) && depositable(700_003) && !depositable(410_301) && !depositable(LIFE_POTION));
        // Turned off, none of it.
        let off = Settings { auto_sell: false, auto_repair: false, auto_buy: false, auto_deposit: false, ..settings };
        assert!(decide(&off, &world, &mut Memory::default(), 7.0).is_empty());
    }

    #[test]
    fn settings_write_and_read_back() {
        let mut settings = on();
        settings.slots[0] = Slot { using: Use::Item(LIFE_POTION), percent: 55 };
        settings.slots[2] = Slot { using: Use::Skill(CURE), percent: 40 };
        settings.picks = vec![Pick::new("Robert".into()), Pick { accuracy: true, ..Pick::new("Pedro".into()) }];
        settings.step(Stepper::FollowDistance, false);
        settings.team_heal_skill = NECTAR;
        settings.auto_deposit = true;
        settings.step(Stepper::DepositAt, true);
        settings.use_xp = true;
        settings.step(Stepper::XpSkill, true);
        assert_eq!(Settings::parse(&settings.format()), settings);
        // Nonsense keeps the defaults.
        let read = Settings::parse("slot1=item:42:50\nslot9=skill:1005:50\nteam_heal_skill=1000\npick1=:1111\nwhat=1");
        assert_eq!(read, Settings::default());
    }

    #[test]
    fn nothing_when_off_or_dead() {
        let mut low = world();
        low.health = 100;
        low.potions = vec![(9, LIFE_POTION)];
        let mut settings = Settings::default();
        settings.slots[0].using = Use::Item(LIFE_POTION);
        assert!(decide(&settings, &low, &mut Memory::default(), 10.0).is_empty());
        settings.enabled = true;
        low.dead = true;
        assert!(decide(&settings, &low, &mut Memory::default(), 10.0).is_empty());
    }

    #[test]
    fn each_slot_by_its_own_share() {
        let mut settings = on();
        settings.slots = [
            Slot { using: Use::Item(LIFE_POTION), percent: 30 },
            Slot { using: Use::Item(MANA_POTION), percent: 40 },
            Slot { using: Use::Skill(CURE), percent: 60 },
        ];
        let mut hero = world();
        hero.potions = vec![(5, LIFE_POTION), (6, MANA_POTION)];
        // Life at 50%: only the heal's share is crossed.
        hero.health = 500;
        assert_eq!(decide(&settings, &hero, &mut Memory::default(), 0.0), vec![Act::Cast { skill: CURE, target: 1 }]);
        // Life at 20%: the potion too.
        hero.health = 200;
        assert!(decide(&settings, &hero, &mut Memory::default(), 0.0).contains(&Act::Potion { uid: 5 }));
        // Mana low and life fine: the mana potion, once a second.
        hero.health = 1_000;
        hero.mana = 400;
        let mut memory = Memory::default();
        assert_eq!(decide(&settings, &hero, &mut memory, 0.0), vec![Act::Potion { uid: 6 }]);
        assert!(decide(&settings, &hero, &mut memory, 0.5).is_empty());
        // None left in the bag: nothing.
        hero.potions.clear();
        assert!(decide(&settings, &hero, &mut Memory::default(), 0.0).is_empty());
    }

    #[test]
    fn only_the_picked_are_looked_after() {
        let mut settings = on();
        settings.picks = vec![Pick::new("Robert".into())];
        let mut team = world();
        team.team = vec![mate(2, "Robert", 60, [103.0, 100.0]), mate(3, "Ana", 10, [104.0, 101.0])];
        let mut memory = Memory::default();
        assert_eq!(decide(&settings, &team, &mut memory, 0.0), vec![Act::Cast { skill: CURE, target: 2 }], "not Ana");
        // Robert waits for his life to be told again.
        assert!(decide(&settings, &team, &mut memory, 0.5).is_empty());
        // His Stigma when it runs out.
        team.team[0].life_percent = Some(100);
        team.team[0].flags = 0;
        assert_eq!(decide(&settings, &team, &mut Memory::default(), 0.0), vec![Act::Cast { skill: STIGMA, target: 2 }]);
        // Not again on Robert before 70 seconds, then again.
        let mut memory = Memory::default();
        assert_eq!(decide(&settings, &team, &mut memory, 0.0), vec![Act::Cast { skill: STIGMA, target: 2 }]);
        assert!(decide(&settings, &team, &mut memory, 69.0).is_empty());
        assert_eq!(decide(&settings, &team, &mut memory, 70.5), vec![Act::Cast { skill: STIGMA, target: 2 }]);
        // Without the Stigma ticked, nothing.
        settings.picks[0].stigma = false;
        assert!(decide(&settings, &team, &mut Memory::default(), 0.0).is_empty());
        // Too far.
        settings.picks[0].stigma = true;
        team.team[0].position = Some([140.0, 100.0]);
        assert!(decide(&settings, &team, &mut Memory::default(), 0.0).is_empty());
    }

    #[test]
    fn the_hero_may_pick_themself() {
        let mut settings = on();
        settings.picks = vec![Pick { shield: true, ..Pick::new("Pedro".into()) }];
        let mut alone = world();
        alone.flags = STIGMA_FLAG;
        assert_eq!(decide(&settings, &alone, &mut Memory::default(), 0.0), vec![Act::Cast { skill: MAGIC_SHIELD, target: 1 }]);
        alone.flags = STIGMA_FLAG | SHIELD_FLAG;
        alone.health = 500;
        assert_eq!(decide(&settings, &alone, &mut Memory::default(), 0.0), vec![Act::Cast { skill: CURE, target: 1 }]);
    }

    #[test]
    fn the_fallen_come_first() {
        let mut settings = on();
        settings.picks = vec![Pick::new("Robert".into())];
        let mut team = world();
        let mut dead = mate(5, "Ana", 0, [102.0, 100.0]);
        dead.dead = true;
        team.team = vec![mate(2, "Robert", 20, [101.0, 101.0]), dead];
        assert_eq!(decide(&settings, &team, &mut Memory::default(), 0.0), vec![Act::Cast { skill: PRAY, target: 5 }]);
        settings.revive = false;
        assert_eq!(decide(&settings, &team, &mut Memory::default(), 0.0), vec![Act::Cast { skill: CURE, target: 2 }]);
    }

    #[test]
    fn the_leader_is_followed_from_afar() {
        let mut settings = on();
        settings.follow = true;
        let mut team = world();
        team.team = vec![mate(2, "Robert", 100, [120.0, 100.0])];
        team.leader = Some(2);
        team.leader_at = Some([120.0, 100.0]);
        assert_eq!(decide(&settings, &team, &mut Memory::default(), 0.0), vec![Act::Follow([120.0, 100.0])]);
        team.leader_at = Some([102.0, 101.0]);
        assert!(decide(&settings, &team, &mut Memory::default(), 0.0).is_empty(), "near enough");
        team.leader_at = Some([120.0, 100.0]);
        team.busy = true;
        assert!(decide(&settings, &team, &mut Memory::default(), 0.0).is_empty(), "not while busy");
    }

    #[test]
    fn two_picks_at_most_and_choices_land() {
        let mut settings = on();
        let mut view = HelperView { tab: Tab::Support, ..Default::default() };
        for name in ["Pedro", "Robert"] {
            assert_eq!(apply(&mut settings, &mut view, HelperAction::Member(name.into())), Effect::Save);
        }
        assert_eq!(apply(&mut settings, &mut view, HelperAction::Member("Ana".into())), Effect::Nothing);
        assert!(!view.notice.is_empty());
        assert_eq!(apply(&mut settings, &mut view, HelperAction::Member("Pedro".into())), Effect::Save, "unpicked");
        assert_eq!(settings.picks.len(), 1);
        apply(&mut settings, &mut view, HelperAction::Gift(0, Gift::Shield));
        assert!(settings.picks[0].shield);
        // A slot's choice.
        apply(&mut settings, &mut view, HelperAction::Choose(Choosing::Slot(1)));
        assert_eq!(view.choosing, Some(Choosing::Slot(1)));
        apply(&mut settings, &mut view, HelperAction::Chosen(Use::Item(LIFE_POTION)));
        assert_eq!(settings.slots[1].using, Use::Item(LIFE_POTION));
        assert_eq!(view.choosing, None);
        apply(&mut settings, &mut view, HelperAction::Clear(1));
        assert_eq!(settings.slots[1].using, Use::Empty);
    }

    #[test]
    fn clicks_land_on_their_controls() {
        let centre = |rect: UiRect| [rect.x as f32 + rect.width as f32 / 2.0, rect.y as f32 + rect.height as f32 / 2.0];
        let mut view = HelperView { settings: on(), ..Default::default() };
        view.settings.slots[1].using = Use::Item(LIFE_POTION);
        assert_eq!(action_at(&view, centre(power_rect())), Some(HelperAction::Power));
        assert_eq!(action_at(&view, centre(slot_icon(0))), Some(HelperAction::Choose(Choosing::Slot(0))));
        assert_eq!(action_at(&view, centre(slot_clear(1))), Some(HelperAction::Clear(1)));
        let (_, more) = arrows(slot_stepper(1));
        assert_eq!(action_at(&view, centre(more)), Some(HelperAction::Step(Stepper::Slot(1), true)));
        view.tab = Tab::Support;
        view.party = vec!["Pedro".into(), "Robert".into()];
        view.settings.picks = vec![Pick::new("Robert".into())];
        assert_eq!(action_at(&view, centre(member_rect(1))), Some(HelperAction::Member("Robert".into())));
        assert_eq!(action_at(&view, centre(gift_rect(0, 1))), Some(HelperAction::Gift(0, Gift::Stigma)));
        assert_eq!(action_at(&view, centre(gift_rect(1, 1))), None, "no second pick");
        assert_eq!(action_at(&view, centre(team_heal_icon())), Some(HelperAction::Choose(Choosing::TeamHeal)));
        // Choosing: only learned skills are taken.
        view.choosing = Some(Choosing::TeamHeal);
        view.known = vec![ADVANCED_CURE];
        assert_eq!(action_at(&view, centre(choice_rect(0))), None, "Cure not learned");
        assert_eq!(action_at(&view, centre(choice_rect(2))), Some(HelperAction::Chosen(Use::Skill(ADVANCED_CURE))));
        // Supplies: the BlackList's button, the depot's tick and arrows.
        view.choosing = None;
        view.tab = Tab::Supplies;
        assert_eq!(action_at(&view, centre(keep_button())), Some(HelperAction::KeepList));
        assert_eq!(action_at(&view, centre(row_rect(1))), None, "the row itself is no tick");
        assert_eq!(action_at(&view, [row_rect(4).x as f32 + 20.0, centre(row_rect(4))[1]]), Some(HelperAction::Toggle(Toggle::AutoDeposit)));
        let (_, more) = arrows(row_stepper(row_rect(4)));
        assert_eq!(action_at(&view, centre(more)), Some(HelperAction::Step(Stepper::DepositAt, true)));
        assert!(row_rect(5).y + row_rect(5).height < SIZE[1] as i32 - 52, "the Hunt tab's last row clears the note");
        // Everything fits in the window.
        assert!(slot_card(2).x + slot_card(2).width <= SIZE[0] as i32 - 8);
        assert!(pick_card(1).y + pick_card(1).height < team_heal_icon().y);
        assert!(choice_panel().y + choice_panel().height < SIZE[1] as i32 - 30);
    }

    /// `HELPER_PREVIEW=dir cargo test -p conquer-sandbox helper_preview`
    #[test]
    fn helper_preview() {
        let Ok(dir) = std::env::var("HELPER_PREVIEW") else { return };
        let mut assets = Assets::open(std::path::Path::new("R:/5065")).expect("assets");
        let font = crate::labels::font().expect("font");
        let mut settings = on();
        settings.slots = [
            Slot { using: Use::Item(LIFE_POTION), percent: 40 },
            Slot { using: Use::Item(MANA_POTION), percent: 30 },
            Slot { using: Use::Skill(ADVANCED_CURE), percent: 60 },
        ];
        settings.picks = vec![Pick::new("Pedro".into()), Pick { shield: true, ..Pick::new("Robert".into()) }];
        let base = HelperView {
            settings,
            known: vec![CURE, STIGMA, MAGIC_SHIELD],
            potions: vec![(LIFE_POTION, 12), (MANA_POTION, 0), (1_002_000, 4)],
            party: vec!["Pedro".into(), "Robert".into(), "Ana".into()],
            leader: Some("Robert".into()),
            toggle_key: "Ctrl+J".into(),
            status: "Stigma on Robert.".into(),
            ..Default::default()
        };
        let views = [
            ("healing", HelperView { tab: Tab::Healing, ..base.clone() }),
            ("support", HelperView { tab: Tab::Support, ..base.clone() }),
            ("choice", HelperView { tab: Tab::Healing, choosing: Some(Choosing::Slot(0)), ..base.clone() }),
            ("follow", HelperView { tab: Tab::Follow, ..base.clone() }),
            (
                "supplies",
                HelperView {
                    tab: Tab::Supplies,
                    settings: Settings { auto_sell: true, auto_deposit: true, ..base.settings.clone() },
                    hover: Some(HelperAction::KeepList),
                    ..base.clone()
                },
            ),
            (
                "hunt",
                HelperView {
                    tab: Tab::Hunt,
                    settings: Settings { hunt: true, hunt_skill: true, use_xp: true, ..base.settings.clone() },
                    status: "Hunting Pheasant.".into(),
                    hover: Some(HelperAction::Toggle(Toggle::Loot)),
                    ..base.clone()
                },
            ),
        ];
        for (name, view) in views {
            let mut content = RgbaImage::new(SIZE[0], SIZE[1]);
            draw(&mut content, &font, &view, &mut assets);
            let framed = crate::shop::framed(&mut assets, content, "Helper", false).expect("frame");
            framed.save(format!("{dir}/helper-{name}.png")).expect("png");
        }
    }
}
