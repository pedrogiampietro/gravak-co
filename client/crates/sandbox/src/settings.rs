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

/// The Options window's choices, shared by every character.
pub fn load_options() -> crate::hud::HudConfig {
    let mut options = crate::hud::HudConfig::default();
    let Some(text) = options_path().and_then(|path| std::fs::read_to_string(path).ok()) else {
        return options;
    };
    for line in text.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let (key, value) = (key.trim(), value.trim());
        let flag = value == "1";
        let number = value.parse::<u16>().ok();
        match key {
            "music" => options.music_volume = number.map_or(options.music_volume, |n| n.min(100) as u8),
            "sound" => options.sound_volume = number.map_or(options.sound_volume, |n| n.min(100) as u8),
            "names" => options.show_names = flag,
            "counter" => options.kill_counter = flag,
            "shadows" => options.full_shadows = flag,
            "info" => options.show_info = flag,
            "max_fps" => {
                options.max_fps = number
                    .filter(|fps| crate::hud::FPS_CAPS.contains(fps))
                    .unwrap_or(options.max_fps);
            }
            "fullscreen" => options.fullscreen = flag,
            "resolution" => {
                options.resolution = number
                    .filter(|index| usize::from(*index) < crate::hud::RESOLUTIONS.len())
                    .map_or(options.resolution, |index| index as u8);
            }
            _ => {}
        }
    }
    options
}

pub fn save_options(options: &crate::hud::HudConfig) {
    let Some(path) = options_path() else {
        return;
    };
    let text = format!(
        "music={}\nsound={}\nnames={}\ncounter={}\nshadows={}\ninfo={}\nmax_fps={}\nfullscreen={}\nresolution={}\n",
        options.music_volume,
        options.sound_volume,
        u8::from(options.show_names),
        u8::from(options.kill_counter),
        u8::from(options.full_shadows),
        u8::from(options.show_info),
        options.max_fps,
        u8::from(options.fullscreen),
        options.resolution,
    );
    let result = path
        .parent()
        .map_or(Ok(()), std::fs::create_dir_all)
        .and_then(|()| std::fs::write(&path, text));
    if let Err(error) = result {
        eprintln!("Could not save the options: {error}");
    }
}

fn options_path() -> Option<PathBuf> {
    Some(std::env::current_exe().ok()?.parent()?.join("settings").join("options.ini"))
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

/// The account last used on the login screen.
pub fn load_last_account() -> Option<String> {
    let text = std::fs::read_to_string(login_path()?).ok()?;
    text.lines()
        .find_map(|line| line.strip_prefix("account="))
        .map(|account| account.trim().to_owned())
        .filter(|account| !account.is_empty())
}

pub fn save_last_account(account: &str) {
    let Some(path) = login_path() else {
        return;
    };
    let result = path
        .parent()
        .map_or(Ok(()), std::fs::create_dir_all)
        .and_then(|()| std::fs::write(&path, format!("account={account}\n")));
    if let Err(error) = result {
        eprintln!("Could not save the last account: {error}");
    }
}

fn login_path() -> Option<PathBuf> {
    Some(std::env::current_exe().ok()?.parent()?.join("settings").join("login.ini"))
}

/// `settings/hud_icons.ini`: `key=path` lines naming the installation's art
/// for the HUD icons (`bounty`, `mentor`, `lock`, `vip`, `mall`, `bug`) and
/// the Revive plate (`revive`).
pub fn load_hud_icon_paths() -> Vec<(String, String)> {
    let Some(text) = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join("settings").join("hud_icons.ini")))
        .and_then(|path| std::fs::read_to_string(path).ok())
    else {
        return Vec::new();
    };
    text.lines()
        .map(str::trim)
        .filter(|line| !line.starts_with(';') && !line.starts_with('#'))
        .filter_map(|line| line.split_once('='))
        .map(|(key, path)| (key.trim().to_ascii_lowercase(), path.trim().to_owned()))
        .filter(|(key, path)| !key.is_empty() && !path.is_empty())
        .collect()
}
