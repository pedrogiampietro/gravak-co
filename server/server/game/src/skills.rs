use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

pub const BASIC_FIRE_ID: u32 = 1_000;
pub const FLAME_BURST_ID: u32 = 1_001;
pub const WHIRLWIND_ID: u32 = 1_002;
pub const CYCLONE_ID: u32 = 1_110;
pub const SUPERMAN_ID: u32 = 1_025;
pub const STIGMA_ID: u32 = 1_095;
pub const MAGIC_SHIELD_ID: u32 = 1_090;
pub const WHIRLWIND_BOOK_ITEM: u32 = 723_100;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillStatus {
    Burning,
    Stunned,
    Cyclone,
    Superman,
    Stigma,
    Shield,
    Invisibility,
    Flying,
    Intensify,
    Accuracy,
}

impl SkillStatus {
    pub fn effect(self) -> crate::entities::StatusEffect {
        use crate::entities::StatusEffect;
        match self {
            Self::Burning => StatusEffect::Burning,
            Self::Stunned => StatusEffect::Stunned,
            Self::Cyclone => StatusEffect::Cyclone,
            Self::Superman => StatusEffect::Superman,
            Self::Stigma => StatusEffect::Stigma,
            Self::Shield => StatusEffect::Shield,
            Self::Invisibility => StatusEffect::Invisibility,
            Self::Flying => StatusEffect::Flying,
            Self::Intensify => StatusEffect::Intensify,
            Self::Accuracy => StatusEffect::Accuracy,
        }
    }

    /// Statuses the caster puts on themselves instead of a target.
    pub fn is_self_buff(self) -> bool {
        !matches!(self, Self::Burning | Self::Stunned)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SkillDefinition {
    pub id: u32,
    pub name: String,
    pub mana_cost: u16,
    pub range: f32,
    pub cooldown: Duration,
    pub radius: f32,
    pub power_percent: u16,
    pub status: Option<SkillStatus>,
    pub status_duration: Duration,
    pub starter: bool,
    pub learn_item: Option<u32>,
    pub training_cost: u32,
    pub required_level: u16,
    /// XP skill: castable only while the XP circle is up, and it uses it.
    pub xp: bool,
    /// Stamina used per cast (MagicType `UseStamina`).
    pub stamina_cost: u8,
    /// Magic power added to the caster's magic attack (MagicType `Power`
    /// below 30000), like Fire, Bomb or FireMeteor.
    pub flat_power: u32,
    /// A weapon skill: the physical attack scaled by `power_percent`
    /// (FastBlade, Hercules, RapidFire...).
    pub physical: bool,
    /// Hits everything within `radius` of the caster instead of around a
    /// target (Hercules, Volcano, FireCircle...).
    pub centered: bool,
    /// Life restored to the caster or the ally aimed at (Cure, Nectar...).
    pub heal: u32,
    /// Brings a dead player back where they lie (Revive, Pray).
    pub revive: bool,
    /// Monster types summoned as a pet, one per skill level (Golem...).
    pub summon: Vec<u32>,
    /// A line this many cells long toward the target (FastBlade).
    pub line: u16,
    /// A half circle of this radius toward the target (Scatter).
    pub sector: u16,
    /// Roar (MagicType `DispatchXp`): fills the XP circle of the
    /// teammates around the caster.
    pub dispatch_xp: bool,
    /// Mana restored to the caster (Meditation).
    pub mana_heal: u32,
    /// Needs a shield in the left hand (Dash).
    pub needs_shield: bool,
    /// A weapon skill (Redux `Common.WeaponSkills`): the weapon types
    /// (item type / 1000) whose plain attacks may set it off instead...
    pub weapon: Vec<u32>,
    /// ...one attack in a hundred times this (MagicType `Percent`).
    pub chance: u8,
}

impl SkillDefinition {
    /// The skill as the original has it at the server's `level`
    /// (`magictype.rs`): its mana and stamina, its power, how far it is
    /// cast and what it covers, its status's time and its chance. A skill
    /// the original lacks stays as `skills.dat` has it.
    pub fn at_level(&self, level: u16) -> SkillDefinition {
        let mut skill = self.clone();
        let Some(magic) = crate::magictype::level(self.id, level) else {
            return skill;
        };
        skill.mana_cost = u16::try_from(magic.mana).unwrap_or(u16::MAX);
        skill.stamina_cost = u8::try_from(magic.stamina).unwrap_or(u8::MAX);
        match magic.power {
            // A share of the physical attack (Hercules, Scatter, Phoenix...).
            power if power >= 30_000 => {
                if skill.physical && power % 1_000 > 0 {
                    skill.power_percent = u16::try_from(power % 1_000).unwrap_or(100);
                }
            },
            // Life given (Cure, Nectar...) or magic added (Thunder, Fire...).
            power if power > 0 => {
                let power = power.unsigned_abs();
                if skill.heal > 0 {
                    skill.heal = power;
                } else if !skill.physical && skill.status.is_none_or(|_| skill.flat_power > 0) {
                    skill.flat_power = power;
                }
            },
            _ => {},
        }
        if magic.distance > 0 {
            skill.range = magic.distance as f32;
        }
        if magic.range > 0 {
            if skill.line > 0 {
                skill.line = u16::try_from(magic.range).unwrap_or(skill.line);
            } else if skill.sector > 0 {
                // A fan reaches as far as the skill does (Redux `IsInArc`
                // with `Distance`: Scatter 15 to 20 cells), not its `Range`.
                skill.sector = u16::try_from(magic.distance.max(magic.range)).unwrap_or(skill.sector);
            } else if skill.radius > 0.0 {
                skill.radius = magic.range as f32;
            }
        }
        if skill.status.is_some() && magic.step_secs > 0 {
            skill.status_duration = Duration::from_secs(u64::from(magic.step_secs));
        }
        if !skill.weapon.is_empty() && magic.percent > 0 {
            skill.chance = u8::try_from(magic.percent.min(100)).unwrap_or(100);
        }
        skill
    }
}

/// Skill experience per level, from the client's MagicType (`NeedExp`):
/// MagicType levels start at 0, the server's at 1.
fn level_table() -> &'static HashMap<(u32, u16), u32> {
    static TABLE: std::sync::OnceLock<HashMap<(u32, u16), u32>> = std::sync::OnceLock::new();
    TABLE.get_or_init(|| {
        include_str!("../data/skill_levels.dat")
            .lines()
            .filter(|line| !line.starts_with('#'))
            .filter_map(|line| {
                let mut fields = line.split_whitespace().map(str::parse::<u32>);
                let (Some(Ok(id)), Some(Ok(level)), Some(Ok(need))) = (fields.next(), fields.next(), fields.next())
                else {
                    return None;
                };
                Some(((id, u16::try_from(level).ok()?), need))
            })
            .collect()
    })
}

/// Experience a skill at the server's `level` needs to reach the next:
/// `Some(0)` at its top level, `None` for a skill the original lacks.
pub fn experience_to_level(skill_id: u32, level: u16) -> Option<u32> {
    let table = level_table();
    let magic_level = level.saturating_sub(1);
    match table.get(&(skill_id, magic_level)) {
        Some(need) => Some(*need),
        // Past the last level the original lists: the top.
        None if table.keys().any(|(id, _)| *id == skill_id) => Some(0),
        None => None,
    }
}

#[derive(Debug, Clone)]
pub struct SkillCatalog {
    skills: HashMap<u32, SkillDefinition>,
}

impl SkillCatalog {
    pub fn from_environment() -> Self {
        let mut catalog = Self::default();
        let path = std::env::var_os("SKILLS_DATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("server/game/data/skills.dat"));
        match fs::read_to_string(&path) {
            Ok(contents) => match Self::parse(&contents) {
                Ok(parsed) if !parsed.skills.is_empty() => {
                    catalog.skills = parsed.skills;
                    tracing::info!(skills = catalog.skills.len(), path = ?path, "Loaded skill catalog");
                },
                Ok(_) => tracing::warn!(path = ?path, "Skill catalog is empty; using built-in definitions"),
                Err(error) => {
                    tracing::error!(%error, path = ?path, "Failed to parse skill catalog; using built-in definitions")
                },
            },
            Err(error) => tracing::warn!(%error, path = ?path, "Skill catalog unavailable; using built-in definitions"),
        }
        catalog
    }

    pub fn get(&self, id: u32) -> Option<&SkillDefinition> {
        self.skills.get(&id)
    }

    pub fn len(&self) -> usize {
        self.skills.len()
    }

    pub fn starter_ids(&self) -> impl Iterator<Item = u32> + '_ {
        self.skills.values().filter(|skill| skill.starter).map(|skill| skill.id)
    }

    /// The weapon skill a weapon of `weapon_type` (item type / 1000) can set
    /// off.
    pub fn weapon_skill(&self, weapon_type: u32) -> Option<&SkillDefinition> {
        self.skills.values().find(|skill| skill.weapon.contains(&weapon_type))
    }

    pub fn skill_for_book(&self, item_type: u32) -> Option<&SkillDefinition> {
        self.skills.values().find(|skill| skill.learn_item == Some(item_type))
    }

    pub fn trainer_skill(&self) -> Option<&SkillDefinition> {
        self.get(WHIRLWIND_ID)
            .or_else(|| self.skills.values().find(|skill| !skill.starter))
    }

    /// Returns the next skill that this trainer can teach at the character's
    /// current level. Ordering is deterministic so a HashMap iteration cannot
    /// change which skill is offered, and already learned skills are skipped.
    pub fn next_trainer_skill<F>(&self, character_level: u16, mut has_skill: F) -> Option<&SkillDefinition>
    where
        F: FnMut(u32) -> bool,
    {
        self.skills
            .values()
            .filter(|skill| {
                !skill.starter
                    && skill.training_cost > 0
                    && skill.required_level <= character_level
                    && !has_skill(skill.id)
            })
            .min_by_key(|skill| (skill.required_level, skill.id))
    }

    pub(crate) fn parse(contents: &str) -> Result<Self, String> {
        let mut catalog = Self { skills: HashMap::new() };
        let mut current: Option<SkillDefinition> = None;
        let mut seen_keys = HashSet::new();

        let finish = |catalog: &mut Self, current: &mut Option<SkillDefinition>| {
            if let Some(skill) = current.take() {
                catalog.skills.insert(skill.id, skill);
            }
        };

        for (line_number, raw_line) in contents.lines().enumerate() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
                continue;
            }
            if line.starts_with('[') && line.ends_with(']') {
                if line.eq_ignore_ascii_case("[Skill]") {
                    finish(&mut catalog, &mut current);
                    seen_keys.clear();
                    current = Some(SkillDefinition {
                        id: 0,
                        name: String::new(),
                        mana_cost: 0,
                        range: 0.0,
                        cooldown: Duration::ZERO,
                        radius: 0.0,
                        power_percent: 100,
                        status: None,
                        status_duration: Duration::ZERO,
                        starter: false,
                        learn_item: None,
                        training_cost: 0,
                        required_level: 1,
                        xp: false,
                        stamina_cost: 0,
                        flat_power: 0,
                        physical: false,
                        centered: false,
                        heal: 0,
                        revive: false,
                        summon: Vec::new(),
                        line: 0,
                        sector: 0,
                        dispatch_xp: false,
                        mana_heal: 0,
                        needs_shield: false,
                        weapon: Vec::new(),
                        chance: 0,
                    });
                }
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                return Err(format!("invalid skills.dat line {}", line_number + 1));
            };
            let Some(skill) = current.as_mut() else {
                continue;
            };
            let key = key.trim();
            if !seen_keys.insert(key.to_owned()) {
                return Err(format!("duplicate skill field {key} at line {}", line_number + 1));
            }
            match key {
                "ID" => {
                    skill.id = value
                        .trim()
                        .parse()
                        .map_err(|_| format!("invalid skill ID at line {}", line_number + 1))?
                },
                "Name" => skill.name = value.trim().to_owned(),
                "Mana" => {
                    skill.mana_cost = value
                        .trim()
                        .parse()
                        .map_err(|_| format!("invalid skill mana at line {}", line_number + 1))?
                },
                "Range" => {
                    skill.range = value
                        .trim()
                        .parse()
                        .map_err(|_| format!("invalid skill range at line {}", line_number + 1))?
                },
                "CooldownMs" => {
                    let millis: u64 = value
                        .trim()
                        .parse()
                        .map_err(|_| format!("invalid skill cooldown at line {}", line_number + 1))?;
                    skill.cooldown = Duration::from_millis(millis);
                },
                "Radius" => {
                    skill.radius = value
                        .trim()
                        .parse()
                        .map_err(|_| format!("invalid skill radius at line {}", line_number + 1))?
                },
                "Power" => {
                    skill.power_percent = value
                        .trim()
                        .parse()
                        .map_err(|_| format!("invalid skill power at line {}", line_number + 1))?
                },
                "Status" => {
                    skill.status = match value.trim().to_ascii_lowercase().as_str() {
                        "burning" => Some(SkillStatus::Burning),
                        "stunned" | "freeze" => Some(SkillStatus::Stunned),
                        "cyclone" => Some(SkillStatus::Cyclone),
                        "superman" => Some(SkillStatus::Superman),
                        "stigma" => Some(SkillStatus::Stigma),
                        "shield" | "magicshield" => Some(SkillStatus::Shield),
                        "invisibility" => Some(SkillStatus::Invisibility),
                        "fly" | "flying" => Some(SkillStatus::Flying),
                        "intensify" => Some(SkillStatus::Intensify),
                        "accuracy" => Some(SkillStatus::Accuracy),
                        "" | "none" => None,
                        _ => return Err(format!("invalid skill status at line {}", line_number + 1)),
                    };
                },
                "StatusDurationMs" => {
                    let millis: u64 = value
                        .trim()
                        .parse()
                        .map_err(|_| format!("invalid skill status duration at line {}", line_number + 1))?;
                    skill.status_duration = Duration::from_millis(millis);
                },
                "Starter" => {
                    skill.starter = value
                        .trim()
                        .parse()
                        .map_err(|_| format!("invalid skill starter flag at line {}", line_number + 1))?
                },
                "LearnItem" => {
                    skill.learn_item = Some(
                        value
                            .trim()
                            .parse()
                            .map_err(|_| format!("invalid skill learning item at line {}", line_number + 1))?,
                    );
                },
                "TrainingCost" => {
                    skill.training_cost = value
                        .trim()
                        .parse()
                        .map_err(|_| format!("invalid skill training cost at line {}", line_number + 1))?;
                },
                "FlatPower" => {
                    skill.flat_power = value
                        .trim()
                        .parse()
                        .map_err(|_| format!("invalid skill flat power at line {}", line_number + 1))?;
                },
                "Physical" => {
                    skill.physical = value
                        .trim()
                        .parse()
                        .map_err(|_| format!("invalid skill physical flag at line {}", line_number + 1))?;
                },
                "Centered" => {
                    skill.centered = value
                        .trim()
                        .parse()
                        .map_err(|_| format!("invalid skill centered flag at line {}", line_number + 1))?;
                },
                "DispatchXp" => {
                    skill.dispatch_xp = value
                        .trim()
                        .parse()
                        .map_err(|_| format!("invalid dispatch flag at line {}", line_number + 1))?;
                },
                "ManaHeal" => {
                    skill.mana_heal = value
                        .trim()
                        .parse()
                        .map_err(|_| format!("invalid mana heal at line {}", line_number + 1))?;
                },
                "NeedsShield" => {
                    skill.needs_shield = value
                        .trim()
                        .parse()
                        .map_err(|_| format!("invalid shield flag at line {}", line_number + 1))?;
                },
                "Line" => {
                    skill.line = value
                        .trim()
                        .parse()
                        .map_err(|_| format!("invalid skill line at line {}", line_number + 1))?;
                },
                "Weapon" => {
                    skill.weapon = value
                        .split(',')
                        .map(|id| id.trim().parse())
                        .collect::<Result<_, _>>()
                        .map_err(|_| format!("invalid skill weapon at line {}", line_number + 1))?;
                },
                "Chance" => {
                    skill.chance = value
                        .trim()
                        .parse()
                        .map_err(|_| format!("invalid skill chance at line {}", line_number + 1))?;
                },
                "Sector" => {
                    skill.sector = value
                        .trim()
                        .parse()
                        .map_err(|_| format!("invalid skill sector at line {}", line_number + 1))?;
                },
                "Summon" => {
                    skill.summon = value
                        .split(',')
                        .map(|id| id.trim().parse())
                        .collect::<Result<_, _>>()
                        .map_err(|_| format!("invalid summon list at line {}", line_number + 1))?;
                },
                "Revive" => {
                    skill.revive = value
                        .trim()
                        .parse()
                        .map_err(|_| format!("invalid skill revive flag at line {}", line_number + 1))?;
                },
                "Heal" => {
                    skill.heal = value
                        .trim()
                        .parse()
                        .map_err(|_| format!("invalid skill heal at line {}", line_number + 1))?;
                },
                "Xp" => {
                    skill.xp = value
                        .trim()
                        .parse()
                        .map_err(|_| format!("invalid skill XP flag at line {}", line_number + 1))?;
                },
                "Stamina" => {
                    skill.stamina_cost = value
                        .trim()
                        .parse()
                        .map_err(|_| format!("invalid skill stamina at line {}", line_number + 1))?;
                },
                "RequiredLevel" => {
                    skill.required_level = value
                        .trim()
                        .parse()
                        .map_err(|_| format!("invalid skill required level at line {}", line_number + 1))?;
                },
                _ => {},
            }
        }
        finish(&mut catalog, &mut current);
        if catalog
            .skills
            .values()
            .any(|skill| skill.id == 0 || skill.name.is_empty())
        {
            return Err("skill catalog contains an incomplete skill".to_owned());
        }
        Ok(catalog)
    }
}

/// Cells of a line from `from` toward `to`, `length` steps long (Redux
/// `GetLinePoints`).
pub fn line_cells(from: (i32, i32), to: (i32, i32), length: u16) -> Vec<(i32, i32)> {
    let (dx, dy) = (to.0 - from.0, to.1 - from.1);
    let steps = dx.abs().max(dy.abs());
    if steps == 0 {
        return Vec::new();
    }
    let (sx, sy) = (dx as f32 / steps as f32, dy as f32 / steps as f32);
    (0..=i32::from(length))
        .map(|k| {
            (
                (from.0 as f32 + sx * k as f32).round() as i32,
                (from.1 as f32 + sy * k as f32).round() as i32,
            )
        })
        .collect()
}

/// Whether `target` lies in the half circle from `from` toward `toward`
/// (Redux `IsInArc`: 90 degrees each side, closer than `range`).
pub fn in_sector(from: (i32, i32), toward: (i32, i32), target: (i32, i32), range: u16) -> bool {
    let distance = (((target.0 - from.0).pow(2) + (target.1 - from.1).pow(2)) as f32).sqrt();
    if distance >= f32::from(range) || target == from {
        return false;
    }
    let center = ((toward.1 - from.1) as f32).atan2((toward.0 - from.0) as f32);
    let angle = ((target.1 - from.1) as f32).atan2((target.0 - from.0) as f32);
    let mut delta = (center - angle).abs();
    if delta > std::f32::consts::PI {
        delta = 2.0 * std::f32::consts::PI - delta;
    }
    delta <= std::f32::consts::FRAC_PI_2
}

impl Default for SkillCatalog {
    fn default() -> Self {
        let mut skills = HashMap::new();
        skills.insert(
            BASIC_FIRE_ID,
            SkillDefinition {
                id: BASIC_FIRE_ID,
                name: "Thunder".to_owned(),
                mana_cost: 20,
                range: 9.0,
                cooldown: Duration::from_millis(800),
                radius: 0.0,
                power_percent: 110,
                status: None,
                status_duration: Duration::ZERO,
                starter: true,
                learn_item: None,
                training_cost: 0,
                required_level: 1,
                xp: false,
                stamina_cost: 0,
                flat_power: 0,
                physical: false,
                centered: false,
                heal: 0,
                revive: false,
                summon: Vec::new(),
                line: 0,
                sector: 0,
                dispatch_xp: false,
                mana_heal: 0,
                needs_shield: false,
                weapon: Vec::new(),
                chance: 0,
            },
        );
        skills.insert(
            FLAME_BURST_ID,
            SkillDefinition {
                id: FLAME_BURST_ID,
                name: "Fire".to_owned(),
                mana_cost: 35,
                range: 12.0,
                cooldown: Duration::from_millis(800),
                radius: 0.0,
                power_percent: 125,
                status: Some(SkillStatus::Burning),
                status_duration: Duration::from_secs(5),
                starter: false,
                learn_item: None,
                training_cost: 0,
                required_level: 1,
                xp: false,
                stamina_cost: 0,
                flat_power: 0,
                physical: false,
                centered: false,
                heal: 0,
                revive: false,
                summon: Vec::new(),
                line: 0,
                sector: 0,
                dispatch_xp: false,
                mana_heal: 0,
                needs_shield: false,
                weapon: Vec::new(),
                chance: 0,
            },
        );
        skills.insert(
            WHIRLWIND_ID,
            SkillDefinition {
                id: WHIRLWIND_ID,
                name: "Tornado".to_owned(),
                mana_cost: 45,
                range: 15.0,
                cooldown: Duration::from_millis(800),
                radius: 0.0,
                power_percent: 145,
                status: Some(SkillStatus::Stunned),
                status_duration: Duration::from_millis(1_500),
                starter: false,
                learn_item: Some(WHIRLWIND_BOOK_ITEM),
                training_cost: 500,
                required_level: 10,
                xp: false,
                stamina_cost: 0,
                flat_power: 0,
                physical: false,
                centered: false,
                heal: 0,
                revive: false,
                summon: Vec::new(),
                line: 0,
                sector: 0,
                dispatch_xp: false,
                mana_heal: 0,
                needs_shield: false,
                weapon: Vec::new(),
                chance: 0,
            },
        );
        skills.insert(
            CYCLONE_ID,
            SkillDefinition {
                id: CYCLONE_ID,
                name: "Cyclone".to_owned(),
                mana_cost: 50,
                range: 0.0,
                cooldown: Duration::from_millis(1_000),
                radius: 0.0,
                power_percent: 100,
                status: Some(SkillStatus::Cyclone),
                status_duration: Duration::from_secs(10),
                starter: false,
                learn_item: None,
                training_cost: 1_000,
                required_level: 15,
                xp: true,
                stamina_cost: 0,
                flat_power: 0,
                physical: false,
                centered: false,
                heal: 0,
                revive: false,
                summon: Vec::new(),
                line: 0,
                sector: 0,
                dispatch_xp: false,
                mana_heal: 0,
                needs_shield: false,
                weapon: Vec::new(),
                chance: 0,
            },
        );
        skills.insert(
            SUPERMAN_ID,
            SkillDefinition {
                id: SUPERMAN_ID,
                name: "Superman".to_owned(),
                mana_cost: 50,
                range: 0.0,
                cooldown: Duration::from_millis(1_000),
                radius: 0.0,
                power_percent: 100,
                status: Some(SkillStatus::Superman),
                status_duration: Duration::from_secs(15),
                starter: false,
                learn_item: None,
                training_cost: 5_000,
                required_level: 40,
                xp: true,
                stamina_cost: 0,
                flat_power: 0,
                physical: false,
                centered: false,
                heal: 0,
                revive: false,
                summon: Vec::new(),
                line: 0,
                sector: 0,
                dispatch_xp: false,
                mana_heal: 0,
                needs_shield: false,
                weapon: Vec::new(),
                chance: 0,
            },
        );
        skills.insert(
            STIGMA_ID,
            SkillDefinition {
                id: STIGMA_ID,
                name: "Stigma".to_owned(),
                mana_cost: 50,
                range: 0.0,
                cooldown: Duration::from_millis(1_000),
                radius: 0.0,
                power_percent: 120,
                status: Some(SkillStatus::Stigma),
                status_duration: Duration::from_secs(70),
                starter: false,
                learn_item: None,
                training_cost: 3_000,
                required_level: 40,
                xp: false,
                stamina_cost: 0,
                flat_power: 0,
                physical: false,
                centered: false,
                heal: 0,
                revive: false,
                summon: Vec::new(),
                line: 0,
                sector: 0,
                dispatch_xp: false,
                mana_heal: 0,
                needs_shield: false,
                weapon: Vec::new(),
                chance: 0,
            },
        );
        skills.insert(
            MAGIC_SHIELD_ID,
            SkillDefinition {
                id: MAGIC_SHIELD_ID,
                name: "Magic Shield".to_owned(),
                mana_cost: 50,
                range: 0.0,
                cooldown: Duration::from_millis(1_000),
                radius: 0.0,
                power_percent: 120,
                status: Some(SkillStatus::Shield),
                status_duration: Duration::from_secs(70),
                starter: false,
                learn_item: None,
                training_cost: 3_000,
                required_level: 40,
                xp: false,
                stamina_cost: 0,
                flat_power: 0,
                physical: false,
                centered: false,
                heal: 0,
                revive: false,
                summon: Vec::new(),
                line: 0,
                sector: 0,
                dispatch_xp: false,
                mana_heal: 0,
                needs_shield: false,
                weapon: Vec::new(),
                chance: 0,
            },
        );
        Self { skills }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        SkillCatalog, SkillStatus, BASIC_FIRE_ID, CYCLONE_ID, FLAME_BURST_ID, MAGIC_SHIELD_ID, STIGMA_ID, SUPERMAN_ID,
        WHIRLWIND_BOOK_ITEM, WHIRLWIND_ID,
    };

    #[test]
    fn lines_and_sectors_follow_the_reference() {
        let line = super::line_cells((10, 10), (14, 10), 3);
        assert_eq!(line, vec![(10, 10), (11, 10), (12, 10), (13, 10)]);
        assert!(super::line_cells((5, 5), (5, 5), 3).is_empty());
        assert!(super::in_sector((10, 10), (20, 10), (14, 12), 8));
        assert!(!super::in_sector((10, 10), (20, 10), (6, 10), 8));
        assert!(!super::in_sector((10, 10), (20, 10), (19, 10), 8));
    }

    #[test]
    fn a_skill_at_a_level_costs_and_does_what_the_original_says() {
        let catalog = super::SkillCatalog::parse(include_str!("../data/skills.dat")).expect("skills.dat");
        let thunder = catalog.get(1_000).expect("Thunder").at_level(5);
        assert_eq!((thunder.mana_cost, thunder.flat_power, thunder.range), (17, 86, 11.0));
        let fire = catalog.get(1_001).expect("Fire").at_level(1);
        assert_eq!((fire.flat_power, fire.status), (130, None));
        let cure = catalog.get(1_005).expect("Cure").at_level(5);
        assert_eq!((cure.mana_cost, cure.heal), (130, 400));
        let scatter = catalog.get(8_001).expect("Scatter").at_level(6);
        assert_eq!((scatter.power_percent, scatter.sector), (72, 20));
        let fast_blade = catalog.get(1_045).expect("FastBlade").at_level(3);
        assert_eq!((fast_blade.line, fast_blade.stamina_cost), (6, 20));
        let phoenix = catalog.get(5_030).expect("Phoenix").at_level(10);
        assert_eq!((phoenix.chance, phoenix.power_percent), (78, 124));
        let stigma = catalog.get(1_095).expect("Stigma").at_level(5);
        assert_eq!(stigma.status_duration, std::time::Duration::from_secs(80));
    }

    #[test]
    fn skill_levels_follow_the_original_table() {
        assert_eq!(super::experience_to_level(1_045, 1), Some(100_000), "FastBlade's first level");
        assert_eq!(super::experience_to_level(1_045, 2), Some(300_000));
        assert_eq!(super::experience_to_level(1_045, 5), Some(0), "FastBlade tops at the original's level 4");
        assert_eq!(super::experience_to_level(1_045, 9), Some(0));
        assert_eq!(super::experience_to_level(5_010, 1), Some(20_243));
        assert_eq!(super::experience_to_level(4_242_424, 1), None);
    }

    #[test]
    fn the_shipped_catalog_parses() {
        let contents = include_str!("../data/skills.dat");
        let catalog = SkillCatalog::parse(contents).expect("skills.dat parses");
        let fast_blade = catalog.get(1_045).expect("FastBlade");
        assert!(fast_blade.physical && fast_blade.stamina_cost == 20);
        let volcano = catalog.get(1_125).expect("Volcano");
        assert!(volcano.xp && volcano.centered && volcano.flat_power == 300);
        assert_eq!(catalog.get(1_005).expect("Cure").heal, 20);
        // Weapon skills: a club sets off Snow one attack in five.
        let snow = catalog.weapon_skill(561).expect("Snow for clubs");
        assert_eq!((snow.id, snow.chance, snow.learn_item), (5_010, 20, Some(725_026)));
        assert!(snow.physical && snow.radius > 0.0 && snow.mana_cost == 0);
        assert_eq!(catalog.weapon_skill(421).map(|skill| skill.id), Some(5_030), "both blades set off Phoenix");
        assert!(catalog.weapon_skill(410).is_none(), "blades of 410 have none");
        let quake = catalog.weapon_skill(450).expect("Earthquake for axes");
        assert_eq!((quake.id, quake.status.is_some(), quake.power_percent), (7_010, true, 100));
        assert_eq!(catalog.weapon_skill(500).map(|skill| skill.id), Some(5_000), "bows freeze");
    }

    #[test]
    fn built_in_catalog_contains_initial_skills() {
        let catalog = SkillCatalog::default();
        assert_eq!(catalog.len(), 7);
        assert_eq!(catalog.get(BASIC_FIRE_ID).unwrap().mana_cost, 20);
        assert_eq!(catalog.get(BASIC_FIRE_ID).unwrap().power_percent, 110);
        assert_eq!(catalog.get(FLAME_BURST_ID).unwrap().radius, 0.0);
        assert_eq!(catalog.get(FLAME_BURST_ID).unwrap().status, Some(SkillStatus::Burning));
        assert_eq!(catalog.get(WHIRLWIND_ID).unwrap().learn_item, Some(WHIRLWIND_BOOK_ITEM));
        assert_eq!(catalog.get(WHIRLWIND_ID).unwrap().training_cost, 500);
        assert_eq!(catalog.get(WHIRLWIND_ID).unwrap().required_level, 10);
        assert_eq!(catalog.get(WHIRLWIND_ID).unwrap().status, Some(SkillStatus::Stunned));
        assert_eq!(catalog.get(CYCLONE_ID).unwrap().status, Some(SkillStatus::Cyclone));
        assert_eq!(catalog.get(CYCLONE_ID).unwrap().training_cost, 1_000);
        assert_eq!(catalog.get(SUPERMAN_ID).unwrap().status, Some(SkillStatus::Superman));
        assert_eq!(catalog.get(SUPERMAN_ID).unwrap().training_cost, 5_000);
        assert_eq!(catalog.get(STIGMA_ID).unwrap().status, Some(SkillStatus::Stigma));
        assert_eq!(catalog.get(MAGIC_SHIELD_ID).unwrap().status, Some(SkillStatus::Shield));
    }

    #[test]
    fn parses_skill_records_without_hardcoding_handler_rules() {
        let contents = "[Skill]\nID=9000\nName=Test Burst\nMana=44\nRange=7.5\nCooldownMs=3300\nRadius=3.0\nPower=175\nStatus=Burning\nStatusDurationMs=4200\nStarter=false\nLearnItem=723999\nTrainingCost=765\nRequiredLevel=15\n";
        let catalog = SkillCatalog::parse(contents).expect("skill data parses");
        assert_eq!(catalog.len(), 1);
        let skill = catalog.get(9000).expect("test skill exists");
        assert_eq!(skill.name, "Test Burst");
        assert_eq!(skill.mana_cost, 44);
        assert_eq!(skill.cooldown.as_millis(), 3300);
        assert_eq!(skill.radius, 3.0);
        assert_eq!(skill.power_percent, 175);
        assert_eq!(skill.status, Some(SkillStatus::Burning));
        assert_eq!(skill.status_duration.as_millis(), 4200);
        assert!(!skill.starter);
        assert_eq!(skill.learn_item, Some(723999));
        assert_eq!(skill.training_cost, 765);
        assert_eq!(skill.required_level, 15);
    }

    #[test]
    fn rejects_duplicate_skill_fields() {
        let contents = "[Skill]\nID=9001\nName=Duplicate\nMana=10\nMana=20\n";
        let error = SkillCatalog::parse(contents).expect_err("duplicate fields must fail");
        assert!(error.contains("duplicate skill field Mana"));
    }

    #[test]
    fn trainer_progression_is_level_gated_and_skips_learned_skills() {
        let catalog = SkillCatalog::default();
        assert_eq!(catalog.next_trainer_skill(9, |_| false).map(|skill| skill.id), None);
        assert_eq!(
            catalog.next_trainer_skill(10, |_| false).map(|skill| skill.id),
            Some(WHIRLWIND_ID)
        );
        assert_eq!(
            catalog
                .next_trainer_skill(15, |skill_id| skill_id == WHIRLWIND_ID)
                .map(|skill| skill.id),
            Some(CYCLONE_ID)
        );
        assert_eq!(
            catalog
                .next_trainer_skill(40, |skill_id| skill_id == WHIRLWIND_ID || skill_id == CYCLONE_ID)
                .map(|skill| skill.id),
            Some(SUPERMAN_ID)
        );
    }
}
