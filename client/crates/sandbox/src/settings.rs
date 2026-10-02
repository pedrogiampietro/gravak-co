//! HUD settings kept between sessions: the hotkey layout and the panels
//! left open, one file per character beside the client.

use std::path::PathBuf;

use crate::action_bar::ActionBar;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HudSettings {
    pub slots: [Option<String>; 10],
    pub skill_bar_open: bool,
    pub performance_overlay: bool,
    pub team_join_enabled: bool,
}

impl HudSettings {
    pub fn capture(bar: &ActionBar, skill_bar_open: bool, performance_overlay: bool, team_join_enabled: bool) -> Self {
        Self {
            slots: std::array::from_fn(|index| bar.slots[index].action_id.clone()),
            skill_bar_open,
            performance_overlay,
            team_join_enabled,
        }
    }

    pub fn apply_to(&self, bar: &mut ActionBar) {
        for (slot, action) in bar.slots.iter_mut().zip(&self.slots) {
            slot.action_id = action.clone();
        }
    }

    fn format(&self) -> String {
        let mut text = String::new();
        for (index, action) in self.slots.iter().enumerate() {
            text += &format!("slot{index}={}\n", action.as_deref().unwrap_or(""));
        }
        text += &format!("skill_bar={}\n", u8::from(self.skill_bar_open));
        text += &format!("performance={}\n", u8::from(self.performance_overlay));
        text += &format!("team_join={}\n", u8::from(self.team_join_enabled));
        text
    }

    fn parse(text: &str) -> Self {
        let mut settings = Self {
            team_join_enabled: true,
            ..Self::default()
        };
        for line in text.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let value = value.trim();
            match key.trim() {
                "skill_bar" => settings.skill_bar_open = value == "1",
                "performance" => settings.performance_overlay = value == "1",
                "team_join" => settings.team_join_enabled = value == "1",
                key => {
                    let index = key.strip_prefix("slot").and_then(|index| index.parse::<usize>().ok());
                    if let Some(slot) = index.and_then(|index| settings.slots.get_mut(index)) {
                        *slot = (!value.is_empty()).then(|| value.to_owned());
                    }
                }
            }
        }
        settings
    }
}

fn path(character_id: u32) -> Option<PathBuf> {
    let dir = std::env::current_exe().ok()?.parent()?.join("settings");
    Some(dir.join(format!("hud_{character_id}.ini")))
}

pub fn load(character_id: u32) -> Option<HudSettings> {
    std::fs::read_to_string(path(character_id)?)
        .ok()
        .map(|text| HudSettings::parse(&text))
}

pub fn save(character_id: u32, settings: &HudSettings) {
    let Some(path) = path(character_id) else {
        return;
    };
    let result = path
        .parent()
        .map_or(Ok(()), std::fs::create_dir_all)
        .and_then(|()| std::fs::write(&path, settings.format()));
    if let Err(error) = result {
        eprintln!("Could not save the HUD settings: {error}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_survive_a_round_trip() {
        let mut bar = ActionBar::default();
        bar.assign_skill(1_045);
        bar.slots[0].action_id = None;
        let settings = HudSettings::capture(&bar, true, false, false);
        let loaded = HudSettings::parse(&settings.format());
        assert_eq!(loaded, settings);
        let mut restored = ActionBar::default();
        loaded.apply_to(&mut restored);
        assert_eq!(restored.slots[0].action_id, None);
        assert_eq!(restored.skill_id(2), Some(1_045));
    }
}
