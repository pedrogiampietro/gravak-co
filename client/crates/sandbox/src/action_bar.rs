/// UI-facing action data. The first two slots expose the starter skills;
/// learned skills are assigned to the next empty slot at runtime.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct ActionSlot {
    pub hotkey: String,
    pub icon: Option<String>,
    pub action_id: Option<String>,
    pub cooldown: Option<u32>,
    pub disabled: bool,
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
pub fn skill_action(skill_id: u32) -> Option<SkillAction> {
    Some(match skill_id {
        1_000 => SkillAction {
            id: 1_000,
            name: "Basic Fire",
            label: "FIRE",
            range: 6.0,
            cooldown_seconds: 1.2,
            requires_target: true,
        },
        1_001 => SkillAction {
            id: 1_001,
            name: "Flame Burst",
            label: "BURST",
            range: 6.0,
            cooldown_seconds: 2.5,
            requires_target: true,
        },
        1_002 => SkillAction {
            id: 1_002,
            name: "Whirlwind",
            label: "WIND",
            range: 6.0,
            cooldown_seconds: 2.0,
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
        _ => return None,
    })
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
    fn unknown_skill_metadata_is_rejected() {
        assert_eq!(skill_action(9999), None);
    }
}
