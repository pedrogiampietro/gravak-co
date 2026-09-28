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

    fn parse(contents: &str) -> Result<Self, String> {
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

impl Default for SkillCatalog {
    fn default() -> Self {
        let mut skills = HashMap::new();
        skills.insert(
            BASIC_FIRE_ID,
            SkillDefinition {
                id: BASIC_FIRE_ID,
                name: "Basic Fire".to_owned(),
                mana_cost: 20,
                range: 6.0,
                cooldown: Duration::from_millis(1_200),
                radius: 0.0,
                power_percent: 110,
                status: None,
                status_duration: Duration::ZERO,
                starter: true,
                learn_item: None,
                training_cost: 0,
                required_level: 1,
            },
        );
        skills.insert(
            FLAME_BURST_ID,
            SkillDefinition {
                id: FLAME_BURST_ID,
                name: "Flame Burst".to_owned(),
                mana_cost: 35,
                range: 6.0,
                cooldown: Duration::from_millis(2_500),
                radius: 2.5,
                power_percent: 125,
                status: Some(SkillStatus::Burning),
                status_duration: Duration::from_secs(5),
                starter: true,
                learn_item: None,
                training_cost: 0,
                required_level: 1,
            },
        );
        skills.insert(
            WHIRLWIND_ID,
            SkillDefinition {
                id: WHIRLWIND_ID,
                name: "Whirlwind".to_owned(),
                mana_cost: 45,
                range: 6.0,
                cooldown: Duration::from_millis(2_000),
                radius: 2.0,
                power_percent: 145,
                status: Some(SkillStatus::Stunned),
                status_duration: Duration::from_millis(1_500),
                starter: false,
                learn_item: Some(WHIRLWIND_BOOK_ITEM),
                training_cost: 500,
                required_level: 10,
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
    fn built_in_catalog_contains_initial_skills() {
        let catalog = SkillCatalog::default();
        assert_eq!(catalog.len(), 7);
        assert_eq!(catalog.get(BASIC_FIRE_ID).unwrap().mana_cost, 20);
        assert_eq!(catalog.get(BASIC_FIRE_ID).unwrap().power_percent, 110);
        assert_eq!(catalog.get(FLAME_BURST_ID).unwrap().radius, 2.5);
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
