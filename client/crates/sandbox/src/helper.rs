//! The helper: what the hero does by itself for the team. Potions when life
//! or mana runs low, heals on the hero and on hurt teammates, Pray on the
//! fallen, the Taoist's Stigma, Magic Shield and Star of Accuracy on whoever
//! lacks them, following the leader, and the XP skill when the circle fills.
//!
//! It only does what a player could: every potion, cast and step goes to the
//! server as the player's own would, and is checked there the same way. The
//! team's parts wait for a team; the hero's own (potions, heals and buffs on
//! itself, the XP skill) work alone too. The choices live in
//! `settings/helper.ini` beside the client.

use crate::components::{draw_card, draw_tab, fill, outline, WidgetState};
use crate::ui::UiRect;
use crate::{labels, ui_theme};
use fontdue::Font;
use image::RgbaImage;
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
pub const SUPERMAN: u32 = 1_025;
pub const CYCLONE: u32 = 1_110;
pub const ACCURACY: u32 = 1_015;
pub const SHIELD: u32 = 1_020;

/// The heals the helper may cast, weakest first.
pub const HEALS: [u32; 4] = [CURE, HEALING_RAIN, ADVANCED_CURE, NECTAR];
/// The XP skills worth using on a hunt.
pub const XP_SKILLS: [u32; 4] = [SUPERMAN, CYCLONE, ACCURACY, SHIELD];

/// The status flags (`MsgPlayer`) the support buffs show.
pub const STIGMA_FLAG: u64 = 1 << 9;
pub const SHIELD_FLAG: u64 = 1 << 8;
pub const STAR_OF_ACCURACY_FLAG: u64 = 1 << 7;

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
const FOLLOW_GAP: f32 = 0.5;

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
        SUPERMAN => "Superman",
        CYCLONE => "Cyclone",
        ACCURACY => "Accuracy",
        SHIELD => "Shield",
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
        STIGMA | MAGIC_SHIELD => 50,
        STAR_OF_ACCURACY => 200,
        _ => 0,
    }
}

/// Stigma, Magic Shield and Star of Accuracy go on the player aimed at.
pub fn is_ally_buff(skill: u32) -> bool {
    matches!(skill, STIGMA | MAGIC_SHIELD | STAR_OF_ACCURACY)
}

/// The status flag an XP skill shows while it runs.
pub fn xp_flag(skill: u32) -> u64 {
    match skill {
        SUPERMAN => 1 << 18,
        CYCLONE => 1 << 23,
        ACCURACY => STAR_OF_ACCURACY_FLAG,
        SHIELD => SHIELD_FLAG,
        _ => 0,
    }
}

// ---------------------------------------------------------------- settings

/// What the player chose.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Settings {
    pub enabled: bool,
    pub hp_potion: bool,
    pub hp_potion_percent: u8,
    pub mp_potion: bool,
    pub mp_potion_percent: u8,
    pub self_heal: bool,
    pub self_heal_skill: u32,
    pub self_heal_percent: u8,
    pub team_heal: bool,
    pub team_heal_skill: u32,
    pub team_heal_percent: u8,
    pub revive: bool,
    pub stigma: bool,
    pub magic_shield: bool,
    pub accuracy: bool,
    pub buff_self: bool,
    pub follow: bool,
    pub follow_distance: u8,
    pub auto_xp: bool,
    pub xp_skill: u32,
    pub xp_needs_monster: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            enabled: false,
            hp_potion: true,
            hp_potion_percent: 50,
            mp_potion: true,
            mp_potion_percent: 30,
            self_heal: false,
            self_heal_skill: CURE,
            self_heal_percent: 60,
            team_heal: true,
            team_heal_skill: CURE,
            team_heal_percent: 70,
            revive: true,
            stigma: true,
            magic_shield: false,
            accuracy: false,
            buff_self: true,
            follow: false,
            follow_distance: 3,
            auto_xp: false,
            xp_skill: SUPERMAN,
            xp_needs_monster: true,
        }
    }
}

/// A box the player ticks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Toggle {
    HpPotion,
    MpPotion,
    SelfHeal,
    TeamHeal,
    Revive,
    Stigma,
    MagicShield,
    Accuracy,
    BuffSelf,
    Follow,
    AutoXp,
    XpNeedsMonster,
}

/// A number the arrows change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Stepper {
    HpPotion,
    MpPotion,
    SelfHeal,
    TeamHeal,
    FollowDistance,
}

/// A skill the arrows choose.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Picker {
    SelfHeal,
    TeamHeal,
    Xp,
}

impl Picker {
    fn choices(self) -> &'static [u32] {
        match self {
            Self::SelfHeal | Self::TeamHeal => &HEALS,
            Self::Xp => &XP_SKILLS,
        }
    }
}

impl Stepper {
    /// Its step and bounds.
    fn range(self) -> (u8, u8, u8) {
        match self {
            Self::FollowDistance => (1, 1, 10),
            _ => (5, 5, 95),
        }
    }
}

impl Settings {
    pub fn toggle(&self, toggle: Toggle) -> bool {
        match toggle {
            Toggle::HpPotion => self.hp_potion,
            Toggle::MpPotion => self.mp_potion,
            Toggle::SelfHeal => self.self_heal,
            Toggle::TeamHeal => self.team_heal,
            Toggle::Revive => self.revive,
            Toggle::Stigma => self.stigma,
            Toggle::MagicShield => self.magic_shield,
            Toggle::Accuracy => self.accuracy,
            Toggle::BuffSelf => self.buff_self,
            Toggle::Follow => self.follow,
            Toggle::AutoXp => self.auto_xp,
            Toggle::XpNeedsMonster => self.xp_needs_monster,
        }
    }

    fn toggle_mut(&mut self, toggle: Toggle) -> &mut bool {
        match toggle {
            Toggle::HpPotion => &mut self.hp_potion,
            Toggle::MpPotion => &mut self.mp_potion,
            Toggle::SelfHeal => &mut self.self_heal,
            Toggle::TeamHeal => &mut self.team_heal,
            Toggle::Revive => &mut self.revive,
            Toggle::Stigma => &mut self.stigma,
            Toggle::MagicShield => &mut self.magic_shield,
            Toggle::Accuracy => &mut self.accuracy,
            Toggle::BuffSelf => &mut self.buff_self,
            Toggle::Follow => &mut self.follow,
            Toggle::AutoXp => &mut self.auto_xp,
            Toggle::XpNeedsMonster => &mut self.xp_needs_monster,
        }
    }

    pub fn flip(&mut self, toggle: Toggle) {
        let value = self.toggle_mut(toggle);
        *value = !*value;
    }

    pub fn number(&self, stepper: Stepper) -> u8 {
        match stepper {
            Stepper::HpPotion => self.hp_potion_percent,
            Stepper::MpPotion => self.mp_potion_percent,
            Stepper::SelfHeal => self.self_heal_percent,
            Stepper::TeamHeal => self.team_heal_percent,
            Stepper::FollowDistance => self.follow_distance,
        }
    }

    fn number_mut(&mut self, stepper: Stepper) -> &mut u8 {
        match stepper {
            Stepper::HpPotion => &mut self.hp_potion_percent,
            Stepper::MpPotion => &mut self.mp_potion_percent,
            Stepper::SelfHeal => &mut self.self_heal_percent,
            Stepper::TeamHeal => &mut self.team_heal_percent,
            Stepper::FollowDistance => &mut self.follow_distance,
        }
    }

    /// Moves a number one step up (`up`) or down, within its bounds.
    pub fn step(&mut self, stepper: Stepper, up: bool) {
        let (step, least, most) = stepper.range();
        let value = self.number_mut(stepper);
        *value = if up { value.saturating_add(step) } else { value.saturating_sub(step) }.clamp(least, most);
    }

    pub fn skill(&self, picker: Picker) -> u32 {
        match picker {
            Picker::SelfHeal => self.self_heal_skill,
            Picker::TeamHeal => self.team_heal_skill,
            Picker::Xp => self.xp_skill,
        }
    }

    /// The next (`up`) or previous skill of a picker's list, round.
    pub fn pick(&mut self, picker: Picker, up: bool) {
        let choices = picker.choices();
        let at = choices.iter().position(|skill| *skill == self.skill(picker)).unwrap_or(0);
        let next = if up { (at + 1) % choices.len() } else { (at + choices.len() - 1) % choices.len() };
        let skill = choices[next];
        match picker {
            Picker::SelfHeal => self.self_heal_skill = skill,
            Picker::TeamHeal => self.team_heal_skill = skill,
            Picker::Xp => self.xp_skill = skill,
        }
    }

    fn format(&self) -> String {
        let flag = |value: bool| if value { "1" } else { "0" };
        [
            ("enabled", flag(self.enabled).to_owned()),
            ("hp_potion", flag(self.hp_potion).to_owned()),
            ("hp_potion_percent", self.hp_potion_percent.to_string()),
            ("mp_potion", flag(self.mp_potion).to_owned()),
            ("mp_potion_percent", self.mp_potion_percent.to_string()),
            ("self_heal", flag(self.self_heal).to_owned()),
            ("self_heal_skill", self.self_heal_skill.to_string()),
            ("self_heal_percent", self.self_heal_percent.to_string()),
            ("team_heal", flag(self.team_heal).to_owned()),
            ("team_heal_skill", self.team_heal_skill.to_string()),
            ("team_heal_percent", self.team_heal_percent.to_string()),
            ("revive", flag(self.revive).to_owned()),
            ("stigma", flag(self.stigma).to_owned()),
            ("magic_shield", flag(self.magic_shield).to_owned()),
            ("accuracy", flag(self.accuracy).to_owned()),
            ("buff_self", flag(self.buff_self).to_owned()),
            ("follow", flag(self.follow).to_owned()),
            ("follow_distance", self.follow_distance.to_string()),
            ("auto_xp", flag(self.auto_xp).to_owned()),
            ("xp_skill", self.xp_skill.to_string()),
            ("xp_needs_monster", flag(self.xp_needs_monster).to_owned()),
        ]
        .into_iter()
        .map(|(key, value)| format!("{key}={value}\n"))
        .collect()
    }

    fn parse(text: &str) -> Self {
        let mut settings = Self::default();
        for line in text.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let value = value.trim();
            let flag = value == "1";
            let percent = |least: u32, most: u32| value.parse::<u32>().ok().map(|number| number.clamp(least, most) as u8);
            let skill = |list: &[u32]| value.parse::<u32>().ok().filter(|skill| list.contains(skill));
            match key.trim() {
                "enabled" => settings.enabled = flag,
                "hp_potion" => settings.hp_potion = flag,
                "mp_potion" => settings.mp_potion = flag,
                "self_heal" => settings.self_heal = flag,
                "team_heal" => settings.team_heal = flag,
                "revive" => settings.revive = flag,
                "stigma" => settings.stigma = flag,
                "magic_shield" => settings.magic_shield = flag,
                "accuracy" => settings.accuracy = flag,
                "buff_self" => settings.buff_self = flag,
                "follow" => settings.follow = flag,
                "auto_xp" => settings.auto_xp = flag,
                "xp_needs_monster" => settings.xp_needs_monster = flag,
                "hp_potion_percent" => settings.hp_potion_percent = percent(5, 95).unwrap_or(settings.hp_potion_percent),
                "mp_potion_percent" => settings.mp_potion_percent = percent(5, 95).unwrap_or(settings.mp_potion_percent),
                "self_heal_percent" => settings.self_heal_percent = percent(5, 95).unwrap_or(settings.self_heal_percent),
                "team_heal_percent" => settings.team_heal_percent = percent(5, 95).unwrap_or(settings.team_heal_percent),
                "follow_distance" => settings.follow_distance = percent(1, 10).unwrap_or(settings.follow_distance),
                "self_heal_skill" => settings.self_heal_skill = skill(&HEALS).unwrap_or(settings.self_heal_skill),
                "team_heal_skill" => settings.team_heal_skill = skill(&HEALS).unwrap_or(settings.team_heal_skill),
                "xp_skill" => settings.xp_skill = skill(&XP_SKILLS).unwrap_or(settings.xp_skill),
                _ => {}
            }
        }
        settings
    }

    pub fn load() -> Self {
        path().and_then(|path| std::fs::read_to_string(path).ok()).map_or_else(Self::default, |text| Self::parse(&text))
    }

    pub fn save(&self) {
        let Some(path) = path() else {
            return;
        };
        let result = path.parent().map_or(Ok(()), std::fs::create_dir_all).and_then(|()| std::fs::write(&path, self.format()));
        if let Err(error) = result {
            eprintln!("Could not save the helper's settings: {error}");
        }
    }
}

fn path() -> Option<PathBuf> {
    Some(std::env::current_exe().ok()?.parent()?.join("settings").join("helper.ini"))
}

// ---------------------------------------------------------------- deciding

/// A teammate as the client knows it.
#[derive(Debug, Clone, PartialEq)]
pub struct Teammate {
    pub id: u32,
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
    /// The bag's potions: (uid, item type, what it restores).
    pub life_potions: Vec<(u32, u32, u16)>,
    pub mana_potions: Vec<(u32, u32, u16)>,
    /// The other members of the team (empty alone).
    pub team: Vec<Teammate>,
    pub leader: Option<u32>,
    /// Where the leader is, on this map.
    pub leader_at: Option<[f32; 2]>,
    pub monster_near: bool,
    pub xp_ready: bool,
    /// An XP skill is running (Superman, Cyclone...).
    pub in_xp: bool,
    /// Busy with something of the player's own (walking somewhere, an
    /// attack): the helper does not walk then.
    pub busy: bool,
}

/// Something the helper does.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Act {
    Potion { uid: u32 },
    Cast { skill: u32, target: u32 },
    /// Walk toward where the leader is.
    Follow([f32; 2]),
}

/// What the helper remembers between frames.
#[derive(Debug, Clone, Default)]
pub struct Memory {
    next_potion: f32,
    next_cast: f32,
    next_follow: f32,
    /// When a skill last went on a target.
    recent: HashMap<(u32, u32), f32>,
}

fn distance(from: [f32; 2], to: [f32; 2]) -> f32 {
    (from[0] - to[0]).abs().max((from[1] - to[1]).abs())
}

fn percent(value: u32, maximum: u32) -> u32 {
    value.saturating_mul(100) / maximum.max(1)
}

/// The potion for `missing` points: the largest that is not wasted, else
/// the smallest.
fn potion_for(potions: &[(u32, u32, u16)], missing: u32) -> Option<u32> {
    potions
        .iter()
        .filter(|(_, _, restores)| u32::from(*restores) <= missing)
        .max_by_key(|(_, _, restores)| *restores)
        .or_else(|| potions.iter().min_by_key(|(_, _, restores)| *restores))
        .map(|(uid, _, _)| *uid)
}

/// What to do now: at most one potion, one cast and one step a frame.
pub fn decide(settings: &Settings, world: &World, memory: &mut Memory, now: f32) -> Vec<Act> {
    let mut acts = Vec::new();
    if !settings.enabled || world.dead {
        return acts;
    }
    memory.recent.retain(|_, at| now - *at < BUFF_AGAIN);

    // Potions.
    if now >= memory.next_potion {
        let potion = if settings.hp_potion
            && percent(world.health, world.maximum_health) < u32::from(settings.hp_potion_percent)
        {
            potion_for(&world.life_potions, world.maximum_health.saturating_sub(world.health))
        } else {
            None
        }
        .or_else(|| {
            (settings.mp_potion
                && world.maximum_mana > 0
                && percent(world.mana, world.maximum_mana) < u32::from(settings.mp_potion_percent))
            .then(|| potion_for(&world.mana_potions, world.maximum_mana.saturating_sub(world.mana)))
            .flatten()
        });
        if let Some(uid) = potion {
            memory.next_potion = now + POTION_GAP;
            acts.push(Act::Potion { uid });
        }
    }

    // One cast.
    if now >= memory.next_cast
        && let Some((skill, target, again)) = next_cast(settings, world, memory)
    {
        memory.next_cast = now + CAST_GAP;
        memory.recent.insert((skill, target), now - BUFF_AGAIN + again);
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
    acts
}

/// The cast that matters most, and how long before it may go on the same
/// target again: the fallen first, then the hero's life, the most hurt
/// teammate, a missing buff, the XP skill.
fn next_cast(settings: &Settings, world: &World, memory: &Memory) -> Option<(u32, u32, f32)> {
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
    if settings.self_heal
        && ready(settings.self_heal_skill)
        && percent(world.health, world.maximum_health) < u32::from(settings.self_heal_percent)
    {
        return Some((settings.self_heal_skill, world.me, 0.0));
    }
    if settings.team_heal && ready(settings.team_heal_skill) {
        let hurt = world
            .team
            .iter()
            .filter(|mate| !mate.dead && in_reach(mate, REACH) && fresh(settings.team_heal_skill, mate.id))
            .filter_map(|mate| mate.life_percent.map(|life| (life, mate.id)))
            .filter(|(life, _)| *life < settings.team_heal_percent)
            .min();
        if let Some((_, id)) = hurt {
            return Some((settings.team_heal_skill, id, HEAL_AGAIN));
        }
    }
    let buffs = [
        (settings.stigma, STIGMA, STIGMA_FLAG),
        (settings.magic_shield, MAGIC_SHIELD, SHIELD_FLAG),
        (settings.accuracy, STAR_OF_ACCURACY, STAR_OF_ACCURACY_FLAG),
    ];
    for (wanted, skill, flag) in buffs {
        if !wanted || !ready(skill) {
            continue;
        }
        if settings.buff_self && world.flags & flag == 0 && fresh(skill, world.me) {
            return Some((skill, world.me, BUFF_AGAIN));
        }
        if let Some(mate) = world
            .team
            .iter()
            .find(|mate| !mate.dead && in_reach(mate, REACH) && mate.flags & flag == 0 && fresh(skill, mate.id))
        {
            return Some((skill, mate.id, BUFF_AGAIN));
        }
    }
    if settings.auto_xp
        && world.xp_ready
        && !world.in_xp
        && world.known.contains(&settings.xp_skill)
        && !world.cooling.contains(&settings.xp_skill)
        && (world.monster_near || !settings.xp_needs_monster)
    {
        return Some((settings.xp_skill, world.me, 0.0));
    }
    None
}

// ---------------------------------------------------------------- window

/// The window's size.
pub const SIZE: [u32; 2] = [720, 440];
const CONTENT_LEFT: i32 = 192;
const CONTENT_TOP: i32 = 56;
const CONTENT_WIDTH: i32 = 512;
const ROW: i32 = 52;

/// The window's tabs.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum Tab {
    #[default]
    Healing,
    Support,
    Follow,
    Hunting,
}

impl Tab {
    pub const ALL: [Self; 4] = [Self::Healing, Self::Support, Self::Follow, Self::Hunting];

    fn label(self) -> &'static str {
        match self {
            Self::Healing => "Healing",
            Self::Support => "Support",
            Self::Follow => "Follow",
            Self::Hunting => "Hunting",
        }
    }

    fn blurb(self) -> &'static str {
        match self {
            Self::Healing => "Potions, heals and the fallen.",
            Self::Support => "The Taoist's buffs on the team.",
            Self::Follow => "Walking after the team's leader.",
            Self::Hunting => "The XP skill on a hunt.",
        }
    }

    fn rows(self) -> &'static [Row] {
        const HEALING: [Row; 5] = [
            Row::new(Toggle::HpPotion, "Life potion", "Drinks one when life is below").stepper(Stepper::HpPotion),
            Row::new(Toggle::MpPotion, "Mana potion", "Drinks one when mana is below").stepper(Stepper::MpPotion),
            Row::new(Toggle::SelfHeal, "Heal myself", "Casts on you when life is below")
                .picker(Picker::SelfHeal)
                .stepper(Stepper::SelfHeal),
            Row::new(Toggle::TeamHeal, "Heal the team", "The most hurt teammate in reach")
                .picker(Picker::TeamHeal)
                .stepper(Stepper::TeamHeal),
            Row::new(Toggle::Revive, "Revive the fallen", "Pray, or Revive with a full XP"),
        ];
        const SUPPORT: [Row; 4] = [
            Row::new(Toggle::Stigma, "Stigma", "On teammates in reach without it"),
            Row::new(Toggle::MagicShield, "Magic Shield", "On teammates in reach without it"),
            Row::new(Toggle::Accuracy, "Star of Accuracy", "On teammates in reach without it"),
            Row::new(Toggle::BuffSelf, "Myself too", "The buffs also go on you"),
        ];
        const FOLLOW: [Row; 1] =
            [Row::new(Toggle::Follow, "Follow the leader", "Keeps within this many cells").stepper(Stepper::FollowDistance)];
        const HUNTING: [Row; 2] = [
            Row::new(Toggle::AutoXp, "XP skill", "Used when the XP circle fills").picker(Picker::Xp),
            Row::new(Toggle::XpNeedsMonster, "Only near monsters", "Waits for a monster in 12 cells"),
        ];
        match self {
            Self::Healing => &HEALING,
            Self::Support => &SUPPORT,
            Self::Follow => &FOLLOW,
            Self::Hunting => &HUNTING,
        }
    }
}

/// One line of a tab: a box, and perhaps a skill and a number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Row {
    toggle: Toggle,
    label: &'static str,
    hint: &'static str,
    picker: Option<Picker>,
    stepper: Option<Stepper>,
}

impl Row {
    const fn new(toggle: Toggle, label: &'static str, hint: &'static str) -> Self {
        Self { toggle, label, hint, picker: None, stepper: None }
    }

    const fn picker(mut self, picker: Picker) -> Self {
        self.picker = Some(picker);
        self
    }

    const fn stepper(mut self, stepper: Stepper) -> Self {
        self.stepper = Some(stepper);
        self
    }
}

/// The window as it is drawn.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct HelperView {
    pub settings: Settings,
    pub tab: Tab,
    /// The skills learned, so a picker can say which are not.
    pub known: Vec<u32>,
    /// The key that turns the helper on and off.
    pub toggle_key: String,
    /// What the helper is doing ("Following Pedro").
    pub status: String,
    pub hover: Option<HelperAction>,
}

/// A click in the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HelperAction {
    Power,
    Tab(Tab),
    Toggle(Toggle),
    Step(Stepper, bool),
    Pick(Picker, bool),
    Close,
}

fn power_rect() -> UiRect {
    UiRect::new(16, 52, 160, 34)
}

fn tab_rect(index: usize) -> UiRect {
    UiRect::new(16, 104 + index as i32 * 34, 160, 28)
}

fn row_rect(index: usize) -> UiRect {
    UiRect::new(CONTENT_LEFT, CONTENT_TOP + 26 + index as i32 * ROW, CONTENT_WIDTH, ROW - 6)
}

fn check_rect(row: UiRect) -> UiRect {
    UiRect::new(row.x + 12, row.y + 14, 18, 18)
}

/// The part of a row that ticks its box: all but the arrows.
fn toggle_rect(row: UiRect, spec: &Row) -> UiRect {
    let right = picker_rect(row, spec).map_or(row.x + row.width, |rect| rect.x);
    let right = stepper_rect(row, spec).map_or(right, |rect| rect.x.min(right));
    UiRect::new(row.x, row.y, right - row.x - 6, row.height)
}

fn stepper_rect(row: UiRect, spec: &Row) -> Option<UiRect> {
    spec.stepper.map(|_| UiRect::new(row.x + row.width - 112, row.y + 11, 102, 24))
}

fn picker_rect(row: UiRect, spec: &Row) -> Option<UiRect> {
    spec.picker.map(|_| {
        let right = stepper_rect(row, spec).map_or(row.x + row.width - 10, |rect| rect.x - 8);
        UiRect::new(right - 150, row.y + 11, 150, 24)
    })
}

/// The arrows at either end of a stepper or picker.
fn arrows(rect: UiRect) -> (UiRect, UiRect) {
    (UiRect::new(rect.x, rect.y, 22, rect.height), UiRect::new(rect.x + rect.width - 22, rect.y, 22, rect.height))
}

fn close_rect() -> UiRect {
    UiRect::new(SIZE[0] as i32 - 31, 6, 27, 30)
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
    for (index, spec) in view.tab.rows().iter().enumerate() {
        let row = row_rect(index);
        if let (Some(stepper), Some(rect)) = (spec.stepper, stepper_rect(row, spec)) {
            let (less, more) = arrows(rect);
            if less.contains_local(local) {
                return Some(HelperAction::Step(stepper, false));
            }
            if more.contains_local(local) {
                return Some(HelperAction::Step(stepper, true));
            }
        }
        if let (Some(picker), Some(rect)) = (spec.picker, picker_rect(row, spec)) {
            let (less, more) = arrows(rect);
            if less.contains_local(local) {
                return Some(HelperAction::Pick(picker, false));
            }
            if more.contains_local(local) {
                return Some(HelperAction::Pick(picker, true));
            }
        }
        if toggle_rect(row, spec).contains_local(local) {
            return Some(HelperAction::Toggle(spec.toggle));
        }
    }
    None
}

/// Whether `local` is over the window.
pub fn covers(local: [f32; 2]) -> bool {
    local[0] >= 0.0 && local[1] >= 0.0 && local[0] < SIZE[0] as f32 && local[1] < SIZE[1] as f32
}

fn text(image: &mut RgbaImage, font: &Font, words: &str, colour: [u8; 3], x: i32, y: i32, size: f32) {
    labels::text_sized(image, font, &[(words, colour)], [x as f32, y as f32], size);
}

fn centred(image: &mut RgbaImage, font: &Font, words: &str, colour: [u8; 3], rect: UiRect, size: f32) {
    let width = crate::daily_rewards_panel::width_of(font, words, size) as i32;
    text(image, font, words, colour, rect.x + (rect.width - width) / 2, rect.y + (rect.height - size as i32) / 2 - 1, size);
}

/// A box with ◄ value ► in it.
fn draw_spinner(image: &mut RgbaImage, font: &Font, rect: UiRect, value: &str, colour: [u8; 3], hover: (bool, bool)) {
    fill(image, rect, [10, 30, 40, 255]);
    outline(image, rect, [60, 150, 170, 255]);
    let (less, more) = arrows(rect);
    for (arrow, lit, glyph) in [(less, hover.0, "<"), (more, hover.1, ">")] {
        fill(image, arrow, if lit { [20, 70, 84, 255] } else { [16, 44, 56, 255] });
        outline(image, arrow, [60, 150, 170, 255]);
        centred(image, font, glyph, if lit { [255, 230, 140] } else { ui_theme::TEXT }, arrow, 11.0);
    }
    centred(image, font, value, colour, UiRect::new(less.x + less.width, rect.y, rect.width - 44, rect.height), 10.0);
}

/// The window's contents (the frame goes around them: `shop::framed`).
pub fn draw(image: &mut RgbaImage, font: &Font, view: &HelperView) {
    const GOLD: [u8; 3] = ui_theme::TEXT_GOLD;
    const TEXT: [u8; 3] = ui_theme::TEXT;
    const MUTED: [u8; 3] = ui_theme::TEXT_MUTED;
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
    outline(image, power, if settings.enabled { [90, 200, 110, 255] } else { [190, 90, 80, 255] });
    let (word, colour) = if settings.enabled { ("Enabled", [120, 230, 130]) } else { ("Disabled", [235, 120, 110]) };
    let label = format!("Helper: {word}");
    centred(image, font, &label, colour, power, 12.0);
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

    // The tab's rows.
    text(image, font, view.tab.label(), GOLD, CONTENT_LEFT, 48, 12.0);
    text(image, font, view.tab.blurb(), MUTED, CONTENT_LEFT, 64, 9.0);
    for (index, spec) in view.tab.rows().iter().enumerate() {
        let row = row_rect(index);
        let on = settings.toggle(spec.toggle);
        let hovered = view.hover == Some(HelperAction::Toggle(spec.toggle));
        draw_card(image, row, if hovered { WidgetState::Hovered } else { WidgetState::Normal });
        fill(image, UiRect::new(row.x, row.y, 3, row.height), if on { [90, 200, 110, 255] } else { [70, 90, 100, 255] });
        let check = check_rect(row);
        fill(image, check, [8, 22, 30, 255]);
        outline(image, check, if on { [90, 200, 110, 255] } else { [110, 120, 130, 255] });
        if on {
            fill(image, UiRect::new(check.x + 4, check.y + 4, check.width - 8, check.height - 8), [110, 220, 120, 255]);
        }
        text(image, font, spec.label, if on { TEXT } else { MUTED }, row.x + 40, row.y + 8, 11.0);
        text(image, font, spec.hint, MUTED, row.x + 40, row.y + 25, 9.0);
        if let (Some(picker), Some(rect)) = (spec.picker, picker_rect(row, spec)) {
            let skill = settings.skill(picker);
            let learned = view.known.contains(&skill);
            let name = if learned { skill_name(skill).to_owned() } else { format!("{} (not learned)", skill_name(skill)) };
            let hover = (
                view.hover == Some(HelperAction::Pick(picker, false)),
                view.hover == Some(HelperAction::Pick(picker, true)),
            );
            draw_spinner(image, font, rect, &name, if learned { TEXT } else { [200, 140, 110] }, hover);
        }
        if let (Some(stepper), Some(rect)) = (spec.stepper, stepper_rect(row, spec)) {
            let number = settings.number(stepper);
            let value = if stepper == Stepper::FollowDistance { format!("{number} cells") } else { format!("{number}%") };
            let hover = (
                view.hover == Some(HelperAction::Step(stepper, false)),
                view.hover == Some(HelperAction::Step(stepper, true)),
            );
            draw_spinner(image, font, rect, &value, TEXT, hover);
        }
    }
    let note = match view.tab {
        Tab::Healing | Tab::Support => "The team's parts need a team; yours work alone too.",
        Tab::Follow => "Walks only while you are not doing something yourself.",
        Tab::Hunting => "Superman, Cyclone, Accuracy or Shield, when learned.",
    };
    text(image, font, note, MUTED, CONTENT_LEFT, SIZE[1] as i32 - 50, 9.0);
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

    fn world() -> World {
        World {
            me: 1,
            position: [100.0, 100.0],
            health: 1_000,
            maximum_health: 1_000,
            mana: 2_000,
            maximum_mana: 2_000,
            known: vec![CURE, ADVANCED_CURE, PRAY, STIGMA, MAGIC_SHIELD, SUPERMAN],
            // Stigma on already, so only what a test sets up happens.
            flags: STIGMA_FLAG,
            ..Default::default()
        }
    }

    fn mate(id: u32, life: u8, position: [f32; 2]) -> Teammate {
        Teammate { id, life_percent: Some(life), position: Some(position), flags: STIGMA_FLAG, dead: false }
    }

    fn on() -> Settings {
        Settings { enabled: true, ..Settings::default() }
    }

    #[test]
    fn settings_write_and_read_back() {
        let mut settings = on();
        settings.flip(Toggle::Follow);
        settings.step(Stepper::HpPotion, true);
        settings.step(Stepper::FollowDistance, false);
        settings.pick(Picker::TeamHeal, true);
        settings.pick(Picker::Xp, false);
        assert_eq!(settings.hp_potion_percent, 55);
        assert_eq!(settings.follow_distance, 2);
        assert_eq!(settings.team_heal_skill, HEALING_RAIN);
        assert_eq!(settings.xp_skill, SHIELD, "the list goes round");
        assert_eq!(Settings::parse(&settings.format()), settings);
        // Nonsense keeps the defaults.
        let read = Settings::parse("hp_potion_percent=900\nxp_skill=1000\nwhat=1");
        assert_eq!(read.hp_potion_percent, 95);
        assert_eq!(read.xp_skill, SUPERMAN);
    }

    #[test]
    fn nothing_when_off_or_dead() {
        let mut low = world();
        low.health = 100;
        low.life_potions = vec![(9, 1_000_000, 70)];
        assert!(decide(&Settings::default(), &low, &mut Memory::default(), 10.0).is_empty());
        low.dead = true;
        assert!(decide(&on(), &low, &mut Memory::default(), 10.0).is_empty());
    }

    #[test]
    fn a_potion_that_fits_and_then_a_pause() {
        let mut low = world();
        low.health = 300;
        low.life_potions = vec![(1, 1_000_000, 70), (2, 1_000_030, 500), (3, 1_002_020, 2_000)];
        let mut memory = Memory::default();
        let acts = decide(&on(), &low, &mut memory, 10.0);
        assert_eq!(acts, vec![Act::Potion { uid: 2 }], "500 of the 700 missing, none wasted");
        assert!(decide(&on(), &low, &mut memory, 10.5).is_empty(), "one a second");
        assert_eq!(decide(&on(), &low, &mut memory, 11.1), vec![Act::Potion { uid: 2 }]);
        // Mana when life is fine.
        let mut thirsty = world();
        thirsty.mana = 100;
        thirsty.mana_potions = vec![(7, 1_001_000, 70)];
        assert_eq!(decide(&on(), &thirsty, &mut Memory::default(), 0.0), vec![Act::Potion { uid: 7 }]);
    }

    #[test]
    fn the_most_hurt_teammate_in_reach_is_healed() {
        let mut team = world();
        team.team = vec![mate(2, 60, [103.0, 100.0]), mate(3, 30, [104.0, 101.0]), mate(4, 5, [140.0, 100.0])];
        let mut memory = Memory::default();
        let acts = decide(&on(), &team, &mut memory, 0.0);
        assert_eq!(acts, vec![Act::Cast { skill: CURE, target: 3 }], "4 is out of reach");
        // The same teammate waits for its life to be told again.
        let acts = decide(&on(), &team, &mut memory, 0.5);
        assert_eq!(acts, vec![Act::Cast { skill: CURE, target: 2 }]);
        // Alone, no team heal.
        assert!(decide(&on(), &world(), &mut Memory::default(), 0.0).is_empty());
    }

    #[test]
    fn the_fallen_come_first_then_buffs() {
        let mut team = world();
        let mut dead = mate(5, 0, [102.0, 100.0]);
        dead.dead = true;
        let mut bare = mate(6, 100, [101.0, 101.0]);
        bare.flags = 0;
        team.team = vec![bare.clone(), dead];
        team.flags = STIGMA_FLAG;
        let mut memory = Memory::default();
        assert_eq!(decide(&on(), &team, &mut memory, 0.0), vec![Act::Cast { skill: PRAY, target: 5 }]);
        team.team.retain(|mate| !mate.dead);
        assert_eq!(decide(&on(), &team, &mut memory, 1.0), vec![Act::Cast { skill: STIGMA, target: 6 }]);
        // Not again while the server has not shown it.
        assert!(decide(&on(), &team, &mut memory, 2.0).is_empty());
        // Too little mana for Stigma.
        team.mana = 20;
        assert!(decide(&on(), &team, &mut Memory::default(), 0.0).is_empty());
    }

    #[test]
    fn the_hero_buffs_itself_and_uses_the_xp_skill() {
        let mut alone = world();
        alone.flags = 0;
        let mut settings = on();
        assert_eq!(decide(&settings, &alone, &mut Memory::default(), 0.0), vec![Act::Cast { skill: STIGMA, target: 1 }]);
        alone.flags = STIGMA_FLAG;
        settings.auto_xp = true;
        alone.xp_ready = true;
        assert!(decide(&settings, &alone, &mut Memory::default(), 0.0).is_empty(), "no monster near");
        alone.monster_near = true;
        assert_eq!(decide(&settings, &alone, &mut Memory::default(), 0.0), vec![Act::Cast { skill: SUPERMAN, target: 1 }]);
        alone.in_xp = true;
        assert!(decide(&settings, &alone, &mut Memory::default(), 0.0).is_empty(), "not while it runs");
    }

    #[test]
    fn the_leader_is_followed_from_afar() {
        let mut settings = on();
        settings.follow = true;
        let mut team = world();
        team.flags = STIGMA_FLAG;
        team.team = vec![mate(2, 100, [120.0, 100.0])];
        team.leader = Some(2);
        team.leader_at = Some([120.0, 100.0]);
        assert_eq!(decide(&settings, &team, &mut Memory::default(), 0.0), vec![Act::Follow([120.0, 100.0])]);
        team.leader_at = Some([102.0, 101.0]);
        assert!(decide(&settings, &team, &mut Memory::default(), 0.0).is_empty(), "near enough");
        team.leader_at = Some([120.0, 100.0]);
        team.busy = true;
        assert!(decide(&settings, &team, &mut Memory::default(), 0.0).is_empty(), "not while busy");
        team.busy = false;
        team.leader = Some(1);
        assert!(decide(&settings, &team, &mut Memory::default(), 0.0).is_empty(), "the leader follows no one");
    }

    /// `HELPER_PREVIEW=dir cargo test -p conquer-sandbox helper_preview`
    #[test]
    fn helper_preview() {
        let Ok(dir) = std::env::var("HELPER_PREVIEW") else { return };
        let mut assets = crate::assets::Assets::open(std::path::Path::new("R:/5065")).expect("assets");
        let font = crate::labels::font().expect("font");
        for tab in Tab::ALL {
            let view = HelperView {
                settings: on(),
                tab,
                known: vec![CURE, STIGMA, SUPERMAN],
                toggle_key: "Ctrl+J".into(),
                status: "Stigma on Pedro.".into(),
                hover: Some(HelperAction::Step(Stepper::HpPotion, true)),
            };
            let mut content = RgbaImage::new(SIZE[0], SIZE[1]);
            draw(&mut content, &font, &view);
            let framed = crate::shop::framed(&mut assets, content, "Helper", false).expect("frame");
            framed.save(format!("{dir}/helper-{tab:?}.png")).expect("png");
        }
    }

    #[test]
    fn clicks_land_on_their_controls() {
        let view = HelperView { settings: on(), ..Default::default() };
        let centre = |rect: UiRect| [rect.x as f32 + rect.width as f32 / 2.0, rect.y as f32 + rect.height as f32 / 2.0];
        assert_eq!(action_at(&view, centre(power_rect())), Some(HelperAction::Power));
        assert_eq!(action_at(&view, centre(tab_rect(2))), Some(HelperAction::Tab(Tab::Follow)));
        let row = row_rect(2);
        let spec = Tab::Healing.rows()[2];
        assert_eq!(action_at(&view, centre(check_rect(row))), Some(HelperAction::Toggle(Toggle::SelfHeal)));
        let (less, more) = arrows(stepper_rect(row, &spec).unwrap());
        assert_eq!(action_at(&view, centre(less)), Some(HelperAction::Step(Stepper::SelfHeal, false)));
        assert_eq!(action_at(&view, centre(more)), Some(HelperAction::Step(Stepper::SelfHeal, true)));
        let (_, more) = arrows(picker_rect(row, &spec).unwrap());
        assert_eq!(action_at(&view, centre(more)), Some(HelperAction::Pick(Picker::SelfHeal, true)));
        // Every row fits above the foot.
        for tab in Tab::ALL {
            let last = row_rect(tab.rows().len() - 1);
            assert!(last.y + last.height < SIZE[1] as i32 - 40, "{tab:?} fits");
        }
    }
}
