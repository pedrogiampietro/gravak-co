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
    /// The chat log's x, y, width and height, minimised, letters' size.
    pub chat_rect: Option<[i32; 4]>,
    pub chat_minimized: bool,
    pub chat_font: u8,
}

impl HudSettings {
    pub fn capture(bar: &ActionBar, skill_bar_open: bool, performance_overlay: bool, team_join_enabled: bool) -> Self {
        Self {
            slots: std::array::from_fn(|index| bar.slots[index].action_id.clone()),
            skill_bar_open,
            performance_overlay,
            team_join_enabled,
            chat_rect: None,
            chat_minimized: false,
            chat_font: 1,
        }
    }

    /// The chat log's settings too.
    pub fn with_chat(mut self, chat: &crate::chat_log::ChatLogState) -> Self {
        self.chat_rect = chat.rect;
        self.chat_minimized = chat.minimized;
        self.chat_font = chat.font;
        self
    }

    pub fn apply_chat(&self, chat: &mut crate::chat_log::ChatLogState) {
        chat.rect = self.chat_rect;
        chat.minimized = self.chat_minimized;
        chat.font = self.chat_font;
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
        if let Some([x, y, w, h]) = self.chat_rect {
            text += &format!("chat_rect={x},{y},{w},{h}\n");
        }
        text += &format!("chat_minimized={}\n", u8::from(self.chat_minimized));
        text += &format!("chat_font={}\n", self.chat_font);
        text
    }

    fn parse(text: &str) -> Self {
        let mut settings = Self {
            team_join_enabled: true,
            chat_font: 1,
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
                "chat_minimized" => settings.chat_minimized = value == "1",
                "chat_font" => settings.chat_font = value.parse().unwrap_or(1).min(2),
                "chat_rect" => {
                    let numbers: Vec<i32> = value.split(',').filter_map(|part| part.trim().parse().ok()).collect();
                    settings.chat_rect = <[i32; 4]>::try_from(numbers).ok();
                }
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

/// `gravak.ini` beside the executable, shipped by the installer: where the
/// game data is and which server to log in to. With it the client opens on
/// the login screens without any command-line arguments.
///
/// ```ini
/// [Client]
/// Assets=game
/// Auth=127.0.0.1:9958
/// Realm=Gravak
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClientConfig {
    /// The game data folder, relative paths taken from the executable's.
    pub assets: Option<PathBuf>,
    pub auth: Option<String>,
    pub realm: Option<String>,
}

pub fn load_client_config() -> Option<ClientConfig> {
    let dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
    let text = std::fs::read_to_string(dir.join("gravak.ini")).ok()?;
    Some(parse_client_config(&text, &dir))
}

fn parse_client_config(text: &str, dir: &std::path::Path) -> ClientConfig {
    let mut config = ClientConfig::default();
    for line in text.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.trim();
        if value.is_empty() {
            continue;
        }
        match key.trim().to_ascii_lowercase().as_str() {
            "assets" => config.assets = Some(dir.join(value)),
            "auth" => config.auth = Some(value.to_owned()),
            "realm" => config.realm = Some(value.to_owned()),
            _ => {}
        }
    }
    config
}

#[cfg(test)]
mod client_config_tests {
    use super::*;

    #[test]
    fn reads_the_installers_ini() {
        let dir = std::path::Path::new("C:/Games/Gravak");
        let config = parse_client_config("[Client]\nAssets=game\nAuth = 10.0.0.2:9958\nRealm=Gravak\n; note\n", dir);
        assert_eq!(config.assets, Some(dir.join("game")));
        assert_eq!(config.auth.as_deref(), Some("10.0.0.2:9958"));
        assert_eq!(config.realm.as_deref(), Some("Gravak"));
        // An absolute path stays as written.
        let config = parse_client_config("Assets=R:/5065", dir);
        assert_eq!(config.assets, Some(std::path::PathBuf::from("R:/5065")));
    }
}
