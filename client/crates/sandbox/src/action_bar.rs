/// UI-facing action data. The first two slots expose the starter skills;
/// learned skills are assigned to the next empty slot at runtime.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct ActionSlot {
    pub hotkey: String,
    pub icon: Option<String>,
    pub action_id: Option<String>,
    pub cooldown: Option<u32>,
    /// Cooldown left, in [`COOLDOWN_STEPS`] of the whole (0 when ready).
    pub cooldown_left: u8,
    pub disabled: bool,
}

/// How finely the cooldown sweep animates (each step redraws the HUD).
pub const COOLDOWN_STEPS: u8 = 24;

/// The steps of the cooldown sweep left `remaining` seconds into `total`.
pub fn cooldown_steps(remaining: f32, total: f32) -> u8 {
    if remaining <= 0.0 || total <= 0.0 {
        return 0;
    }
    ((remaining / total).clamp(0.0, 1.0) * f32::from(COOLDOWN_STEPS)).ceil() as u8
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ActionBar {
    pub slots: [ActionSlot; 10],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SkillAction {
    pub id: u32,
    pub name: &'static str,
    pub label: &'static str,
    pub range: f32,
    pub cooldown_seconds: f32,
    pub requires_target: bool,
}

/// Client-side interaction metadata for the skills currently exposed by the
/// server catalog. Damage, mana and effects remain server-authoritative; this
/// only supplies the input contract and local cooldown presentation.
/// Skills aimed at a spot rather than locked on a target: FastBlade and
/// ScentSword strike along a line, Scatter in a fan, toward the clicked
/// cell, once per click (what is not in the way is not hit).
pub fn is_aimed_skill(skill_id: u32) -> bool {
    matches!(skill_id, 1_045 | 1_046 | 8_001)
}

pub fn skill_action(skill_id: u32) -> Option<SkillAction> {
    Some(match skill_id {
        1_000 => SkillAction {
            id: 1_000,
            name: "Basic Fire",
            label: "FIRE",
            range: 9.0,
            cooldown_seconds: 0.8,
            requires_target: true,
        },
        1_001 => SkillAction {
            id: 1_001,
            name: "Flame Burst",
            label: "BURST",
            range: 12.0,
            cooldown_seconds: 0.8,
            requires_target: true,
        },
        1_002 => SkillAction {
            id: 1_002,
            name: "Whirlwind",
            label: "WIND",
            range: 15.0,
            cooldown_seconds: 0.8,
            requires_target: true,
        },
        1_110 => SkillAction {
            id: 1_110,
            name: "Cyclone",
            label: "CYCL",
            range: 0.0,
            cooldown_seconds: 1.0,
            requires_target: false,
        },
        1_025 => SkillAction {
            id: 1_025,
            name: "Superman",
            label: "SUPER",
            range: 0.0,
            cooldown_seconds: 1.0,
            requires_target: false,
        },
        1_095 => SkillAction {
            id: 1_095,
            name: "Stigma",
            label: "STIG",
            range: 0.0,
            cooldown_seconds: 1.0,
            requires_target: false,
        },
        1_090 => SkillAction {
            id: 1_090,
            name: "Magic Shield",
            label: "SHIELD",
            range: 0.0,
            cooldown_seconds: 1.0,
            requires_target: false,
        },
        1_050 => SkillAction {
            id: 1_050,
            name: "Revive",
            label: "REVIVE",
            range: 12.0,
            cooldown_seconds: 1.0,
            requires_target: false,
        },
        1_100 => SkillAction {
            id: 1_100,
            name: "Pray",
            label: "PRAY",
            range: 12.0,
            cooldown_seconds: 1.0,
            requires_target: false,
        },
        1_015 => SkillAction {
            id: 1_015,
            name: "Accuracy",
            label: "ACCUR",
            range: 0.0,
            cooldown_seconds: 1.0,
            requires_target: false,
        },
        1_085 => SkillAction {
            id: 1_085,
            name: "StarofAccuracy",
            label: "STAR",
            range: 0.0,
            cooldown_seconds: 1.0,
            requires_target: false,
        },
        1_075 => SkillAction {
            id: 1_075,
            name: "Invisibility",
            label: "INVIS",
            range: 0.0,
            cooldown_seconds: 1.0,
            requires_target: false,
        },
        8_002 => SkillAction {
            id: 8_002,
            name: "Fly",
            label: "FLY",
            range: 0.0,
            cooldown_seconds: 1.0,
            requires_target: false,
        },
        8_003 => SkillAction {
            id: 8_003,
            name: "AdvancedFly",
            label: "AFLY",
            range: 0.0,
            cooldown_seconds: 1.0,
            requires_target: false,
        },
        9_000 => SkillAction {
            id: 9_000,
            name: "Intensify",
            label: "INTENS",
            range: 0.0,
            cooldown_seconds: 1.0,
            requires_target: false,
        },
        5_002 => SkillAction {
            id: 5_002,
            name: "Poison",
            label: "POISON",
            range: 15.0,
            cooldown_seconds: 0.8,
            requires_target: true,
        },
        1_270 => SkillAction {
            id: 1_270,
            name: "Golem",
            label: "GOLEM",
            range: 0.0,
            cooldown_seconds: 1.0,
            requires_target: false,
        },
        1_280 => SkillAction {
            id: 1_280,
            name: "WaterElf",
            label: "ELF",
            range: 0.0,
            cooldown_seconds: 1.0,
            requires_target: false,
        },
        1_360 => SkillAction {
            id: 1_360,
            name: "NightDevil",
            label: "DEVIL",
            range: 0.0,
            cooldown_seconds: 1.0,
            requires_target: false,
        },
        1_040 => SkillAction {
            id: 1_040,
            name: "Roar",
            label: "ROAR",
            range: 0.0,
            cooldown_seconds: 1.0,
            requires_target: false,
        },
        1_051 => SkillAction {
            id: 1_051,
            name: "Dash",
            label: "DASH",
            range: 2.0,
            cooldown_seconds: 0.8,
            requires_target: true,
        },
        1_195 => SkillAction {
            id: 1_195,
            name: "Meditation",
            label: "MEDIT",
            range: 0.0,
            cooldown_seconds: 1.0,
            requires_target: false,
        },
        // Trainer skills, as in the server catalog (heals and area skills
        // centred on the caster need no target).
        1_005 => SkillAction {
            id: 1_005,
            name: "Cure",
            label: "CURE",
            range: 10.0,
            cooldown_seconds: 0.8,
            requires_target: false,
        },
        1_055 => SkillAction {
            id: 1_055,
            name: "HealingRain",
            label: "RAIN",
            range: 10.0,
            cooldown_seconds: 0.8,
            requires_target: false,
        },
        1_175 => SkillAction {
            id: 1_175,
            name: "AdvancedCure",
            label: "ACURE",
            range: 10.0,
            cooldown_seconds: 0.8,
            requires_target: false,
        },
        1_170 => SkillAction {
            id: 1_170,
            name: "Nectar",
            label: "NECTAR",
            range: 10.0,
            cooldown_seconds: 0.8,
            requires_target: false,
        },
        1_190 => SkillAction {
            id: 1_190,
            name: "SpiritHealing",
            label: "SPIRIT",
            range: 5.0,
            cooldown_seconds: 0.8,
            requires_target: false,
        },
        1_010 => SkillAction {
            id: 1_010,
            name: "Lightning",
            label: "LIGHT",
            range: 9.0,
            cooldown_seconds: 1.0,
            requires_target: false,
        },
        1_125 => SkillAction {
            id: 1_125,
            name: "Volcano",
            label: "VOLC",
            range: 12.0,
            cooldown_seconds: 1.0,
            requires_target: false,
        },
        5_001 => SkillAction {
            id: 5_001,
            name: "SpeedLightning",
            label: "SPEED",
            range: 12.0,
            cooldown_seconds: 1.0,
            requires_target: false,
        },
        1_320 => SkillAction {
            id: 1_320,
            name: "FlyingMoon",
            label: "MOON",
            range: 13.0,
            cooldown_seconds: 1.0,
            requires_target: true,
        },
        1_150 => SkillAction {
            id: 1_150,
            name: "FireRing",
            label: "RING",
            range: 9.0,
            cooldown_seconds: 0.8,
            requires_target: true,
        },
        1_160 => SkillAction {
            id: 1_160,
            name: "Bomb",
            label: "BOMB",
            range: 11.0,
            cooldown_seconds: 0.8,
            requires_target: true,
        },
        1_180 => SkillAction {
            id: 1_180,
            name: "FireMeteor",
            label: "METEOR",
            range: 10.0,
            cooldown_seconds: 0.8,
            requires_target: true,
        },
        1_120 => SkillAction {
            id: 1_120,
            name: "FireCircle",
            label: "CIRCLE",
            range: 8.0,
            cooldown_seconds: 0.8,
            requires_target: false,
        },
        1_165 => SkillAction {
            id: 1_165,
            name: "FireofHell",
            label: "HELL",
            range: 6.0,
            cooldown_seconds: 0.8,
            requires_target: false,
        },
        1_045 => SkillAction {
            id: 1_045,
            name: "FastBlade",
            label: "FBLADE",
            range: 10.0,
            cooldown_seconds: 0.8,
            requires_target: true,
        },
        1_046 => SkillAction {
            id: 1_046,
            name: "ScentSword",
            label: "SCENT",
            range: 10.0,
            cooldown_seconds: 0.8,
            requires_target: true,
        },
        1_115 => SkillAction {
            id: 1_115,
            name: "Hercules",
            label: "HERC",
            range: 4.0,
            cooldown_seconds: 0.8,
            requires_target: false,
        },
        3_090 => SkillAction {
            id: 3_090,
            name: "Pervade",
            label: "PERV",
            range: 7.0,
            cooldown_seconds: 0.8,
            requires_target: false,
        },
        8_000 => SkillAction {
            id: 8_000,
            name: "RapidFire",
            label: "RAPID",
            range: 15.0,
            cooldown_seconds: 0.8,
            requires_target: true,
        },
        8_001 => SkillAction {
            id: 8_001,
            name: "Scatter",
            label: "SCAT",
            range: 15.0,
            cooldown_seconds: 0.8,
            requires_target: true,
        },
        8_030 => SkillAction {
            id: 8_030,
            name: "ArrowRain",
            label: "ARROW",
            range: 9.0,
            cooldown_seconds: 1.0,
            requires_target: false,
        },
        1_020 => SkillAction {
            id: 1_020,
            name: "Shield",
            label: "XSHLD",
            range: 0.0,
            cooldown_seconds: 1.0,
            requires_target: false,
        },
        _ => return None,
    })
}

/// Heals and revives go to the player under the cursor, if there is one.
pub fn is_support_skill(skill_id: u32) -> bool {
    matches!(skill_id, 1_005 | 1_055 | 1_170 | 1_175 | 1_050 | 1_100)
}

/// Skill icon from `ani/Magic.Ani` (`[MagicSkillType{id}]`).
pub fn skill_icon(skill_id: u32) -> String {
    format!("data/main/MagicSkillType{skill_id}.dds")
}

impl ActionSlot {
    /// Explicit icon, or the original skill icon for skill actions.
    pub fn icon_path(&self) -> Option<String> {
        self.icon.clone().or_else(|| {
            self.action_id
                .as_deref()
                .and_then(|action| action.strip_prefix("skill:"))
                .and_then(|id| id.parse().ok())
                .map(skill_icon)
        })
    }
}

impl Default for ActionBar {
    fn default() -> Self {
        Self {
            slots: std::array::from_fn(|index| {
                let mut slot = ActionSlot {
                    hotkey: format!("F{}", index + 1),
                    ..ActionSlot::default()
                };
                if index == 0 {
                    slot.action_id = Some("skill:1000".to_owned());
                } else if index == 1 {
                    slot.action_id = Some("skill:1001".to_owned());
                }
                slot
            }),
        }
    }
}

impl ActionBar {
    pub fn assign_skill(&mut self, skill_id: u32) -> Option<usize> {
        let action_id = format!("skill:{skill_id}");
        if let Some(index) = self
            .slots
            .iter()
            .position(|slot| slot.action_id.as_deref() == Some(action_id.as_str()))
        {
            return Some(index);
        }
        let index = self
            .slots
            .iter()
            .position(|slot| slot.action_id.is_none())?;
        self.slots[index].action_id = Some(action_id);
        Some(index)
    }

    pub fn skill_id(&self, index: usize) -> Option<u32> {
        self.slots
            .get(index)
            .and_then(|slot| slot.action_id.as_deref())
            .and_then(|action| action.strip_prefix("skill:"))
            .and_then(|id| id.parse().ok())
    }
}

#[cfg(test)]
mod tests {
    use super::{ActionBar, skill_action};

    #[test]
    fn first_slots_expose_the_playable_skills() {
        let bar = ActionBar::default();
        assert_eq!(bar.slots[0].action_id.as_deref(), Some("skill:1000"));
        assert_eq!(bar.slots[1].action_id.as_deref(), Some("skill:1001"));
        assert_eq!(bar.slots[2].action_id, None);
    }

    #[test]
    fn newly_learned_skill_uses_first_empty_slot() {
        let mut bar = ActionBar::default();
        assert_eq!(bar.assign_skill(1002), Some(2));
        assert_eq!(bar.skill_id(2), Some(1002));
        assert_eq!(bar.assign_skill(1002), Some(2));
    }

    #[test]
    fn self_buffs_are_targetless_and_have_local_cooldown_metadata() {
        let skill = skill_action(1110).expect("Cyclone metadata");
        assert_eq!(skill.name, "Cyclone");
        assert!(!skill.requires_target);
        assert_eq!(skill.cooldown_seconds, 1.0);
    }

    #[test]
    fn the_cooldown_sweep_shrinks_to_nothing() {
        assert_eq!(super::cooldown_steps(1.0, 1.0), super::COOLDOWN_STEPS);
        assert_eq!(super::cooldown_steps(0.5, 1.0), super::COOLDOWN_STEPS / 2);
        assert_eq!(super::cooldown_steps(0.0, 1.0), 0);
        assert_eq!(super::cooldown_steps(-1.0, 1.0), 0);
    }

    #[test]
    fn unknown_skill_metadata_is_rejected() {
        assert_eq!(skill_action(9999), None);
    }
}
