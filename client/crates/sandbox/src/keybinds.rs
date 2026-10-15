//! Shortcuts: every command the keyboard can give, the key each is bound
//! to, and the Shortcuts window that rebinds them (Options > Hotkey). The
//! bindings live in `settings/keybinds.ini` beside the client; a command
//! not written there keeps its default.

use crate::components::{draw_button, draw_card, draw_tab, fill, outline, WidgetState};
use crate::ui::UiRect;
use crate::{labels, ui_theme};
use fontdue::Font;
use image::RgbaImage;
use std::path::PathBuf;
use winit::keyboard::KeyCode;

/// A group of commands, as the window lists them.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum Category {
    #[default]
    All,
    ActionBar,
    Panels,
    Gameplay,
}

impl Category {
    pub const ALL: [Self; 4] = [Self::All, Self::ActionBar, Self::Panels, Self::Gameplay];

    pub fn label(self) -> &'static str {
        match self {
            Self::All => "All commands",
            Self::ActionBar => "Action bar",
            Self::Panels => "Windows",
            Self::Gameplay => "Gameplay",
        }
    }

    fn blurb(self) -> &'static str {
        match self {
            Self::All => "Every shortcut in one list.",
            Self::ActionBar => "The F keys' slots: items, skills and actions.",
            Self::Panels => "Open and close the game's windows.",
            Self::Gameplay => "Moving, sitting and the like.",
        }
    }
}

/// Something a key can do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Command {
    Slot1,
    Slot2,
    Slot3,
    Slot4,
    Slot5,
    Slot6,
    Slot7,
    Slot8,
    Slot9,
    Slot10,
    Inventory,
    Character,
    Skills,
    Guild,
    Friends,
    Team,
    Bounty,
    Store,
    Highscores,
    Nobility,
    Options,
    Emotes,
    Map,
    Shortcuts,
    Helper,
    Stop,
    RunWalk,
    Sit,
    Mine,
    Land,
    Performance,
    HelperToggle,
}

impl Command {
    pub const ALL: [Self; 32] = [
        Self::Slot1,
        Self::Slot2,
        Self::Slot3,
        Self::Slot4,
        Self::Slot5,
        Self::Slot6,
        Self::Slot7,
        Self::Slot8,
        Self::Slot9,
        Self::Slot10,
        Self::Inventory,
        Self::Character,
        Self::Skills,
        Self::Guild,
        Self::Friends,
        Self::Team,
        Self::Bounty,
        Self::Store,
        Self::Highscores,
        Self::Nobility,
        Self::Options,
        Self::Emotes,
        Self::Map,
        Self::Shortcuts,
        Self::Helper,
        Self::Stop,
        Self::RunWalk,
        Self::Sit,
        Self::Mine,
        Self::Land,
        Self::Performance,
        Self::HelperToggle,
    ];

    fn index(self) -> usize {
        Self::ALL.iter().position(|command| *command == self).unwrap_or(0)
    }

    /// The F key slot this command uses (0 for F1), if it is one.
    pub fn slot(self) -> Option<usize> {
        (self.index() < 10).then_some(self.index())
    }

    pub fn category(self) -> Category {
        match self.index() {
            0..10 => Category::ActionBar,
            10..25 => Category::Panels,
            _ => Category::Gameplay,
        }
    }

    /// How the command is written in `keybinds.ini`.
    pub fn id(self) -> String {
        format!("{self:?}").to_ascii_lowercase()
    }

    pub fn name(self) -> String {
        match self {
            Self::Inventory => "Inventory".into(),
            Self::Character => "Character".into(),
            Self::Skills => "Skills".into(),
            Self::Guild => "Guild".into(),
            Self::Friends => "Friends".into(),
            Self::Team => "Team".into(),
            Self::Bounty => "Bounty Tasks".into(),
            Self::Store => "Store".into(),
            Self::Highscores => "Highscores".into(),
            Self::Nobility => "Nobility".into(),
            Self::Options => "Options".into(),
            Self::Emotes => "Actions".into(),
            Self::Map => "Corner map".into(),
            Self::Shortcuts => "Shortcuts".into(),
            Self::Helper => "Helper".into(),
            Self::HelperToggle => "Helper on / off".into(),
            Self::Stop => "Stop".into(),
            Self::RunWalk => "Run / Walk".into(),
            Self::Sit => "Sit".into(),
            Self::Mine => "Mine".into(),
            Self::Land => "Land".into(),
            Self::Performance => "Performance".into(),
            slot => format!("Action Bar Slot {}", slot.index() + 1),
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::Inventory => "Opens and closes the bag.",
            Self::Character => "Your attributes, equipment and skills' levels.",
            Self::Skills => "The bar of learned skills above the HUD.",
            Self::Guild => "The Guild System window.",
            Self::Friends => "Friends and the ignored.",
            Self::Team => "Your team.",
            Self::Bounty => "The bounty tasks board.",
            Self::Store => "The Store.",
            Self::Highscores => "The rankings.",
            Self::Nobility => "The nobility ranks.",
            Self::Options => "Sound, window size and the rest.",
            Self::Emotes => "The emotes and actions panel.",
            Self::Map => "Shows or hides the map in the corner.",
            Self::Shortcuts => "This window.",
            Self::Helper => "Potions, heals, buffs and following, by themselves.",
            Self::HelperToggle => "Turns the helper on or off.",
            Self::Stop => "Stops walking and attacking.",
            Self::RunWalk => "Switches between running and walking.",
            Self::Sit => "Sits down or stands up.",
            Self::Mine => "Mines where you stand (with a hoe).",
            Self::Land => "Lands while flying.",
            Self::Performance => "Frame time and draw counts.",
            _ => "Uses the item, skill or action in this slot.",
        }
    }

    /// The key it has until the player changes it: the client's own keys,
    /// and free letters for the windows that had none.
    pub fn default_binding(self) -> Option<Binding> {
        let key = |code| Some(Binding { code, ctrl: false, alt: false, shift: false });
        match self {
            Self::Slot1 => key(KeyCode::F1),
            Self::Slot2 => key(KeyCode::F2),
            Self::Slot3 => key(KeyCode::F3),
            Self::Slot4 => key(KeyCode::F4),
            Self::Slot5 => key(KeyCode::F5),
            Self::Slot6 => key(KeyCode::F6),
            Self::Slot7 => key(KeyCode::F7),
            Self::Slot8 => key(KeyCode::F8),
            Self::Slot9 => key(KeyCode::F9),
            Self::Slot10 => key(KeyCode::F10),
            Self::Inventory => key(KeyCode::KeyI),
            Self::Character => key(KeyCode::KeyU),
            Self::Skills => key(KeyCode::KeyK),
            Self::Guild => key(KeyCode::KeyG),
            Self::Friends => key(KeyCode::KeyF),
            Self::Team => key(KeyCode::KeyT),
            Self::Bounty => key(KeyCode::KeyB),
            Self::Store => key(KeyCode::KeyP),
            Self::Highscores => key(KeyCode::KeyH),
            Self::Nobility => key(KeyCode::KeyN),
            Self::Options => key(KeyCode::KeyO),
            Self::Emotes => key(KeyCode::KeyE),
            Self::Map => key(KeyCode::Tab),
            Self::Shortcuts => key(KeyCode::KeyL),
            Self::Helper => key(KeyCode::KeyJ),
            Self::Stop => key(KeyCode::Space),
            Self::RunWalk => key(KeyCode::Slash),
            Self::Sit => key(KeyCode::KeyX),
            Self::Mine => key(KeyCode::KeyM),
            Self::Land => key(KeyCode::KeyC),
            Self::Performance => key(KeyCode::F11),
            Self::HelperToggle => Some(Binding { code: KeyCode::KeyJ, ctrl: true, alt: false, shift: false }),
        }
    }
}

/// A key and the modifiers held with it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Binding {
    pub code: KeyCode,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
}

/// The keys that can be bound, and how they are written and shown.
const KEYS: &[(KeyCode, &str)] = &[
    (KeyCode::KeyA, "A"),
    (KeyCode::KeyB, "B"),
    (KeyCode::KeyC, "C"),
    (KeyCode::KeyD, "D"),
    (KeyCode::KeyE, "E"),
    (KeyCode::KeyF, "F"),
    (KeyCode::KeyG, "G"),
    (KeyCode::KeyH, "H"),
    (KeyCode::KeyI, "I"),
    (KeyCode::KeyJ, "J"),
    (KeyCode::KeyK, "K"),
    (KeyCode::KeyL, "L"),
    (KeyCode::KeyM, "M"),
    (KeyCode::KeyN, "N"),
    (KeyCode::KeyO, "O"),
    (KeyCode::KeyP, "P"),
    (KeyCode::KeyQ, "Q"),
    (KeyCode::KeyR, "R"),
    (KeyCode::KeyS, "S"),
    (KeyCode::KeyT, "T"),
    (KeyCode::KeyU, "U"),
    (KeyCode::KeyV, "V"),
    (KeyCode::KeyW, "W"),
    (KeyCode::KeyX, "X"),
    (KeyCode::KeyY, "Y"),
    (KeyCode::KeyZ, "Z"),
    (KeyCode::Digit0, "0"),
    (KeyCode::Digit1, "1"),
    (KeyCode::Digit2, "2"),
    (KeyCode::Digit3, "3"),
    (KeyCode::Digit4, "4"),
    (KeyCode::Digit5, "5"),
    (KeyCode::Digit6, "6"),
    (KeyCode::Digit7, "7"),
    (KeyCode::Digit8, "8"),
    (KeyCode::Digit9, "9"),
    (KeyCode::F1, "F1"),
    (KeyCode::F2, "F2"),
    (KeyCode::F3, "F3"),
    (KeyCode::F4, "F4"),
    (KeyCode::F5, "F5"),
    (KeyCode::F6, "F6"),
    (KeyCode::F7, "F7"),
    (KeyCode::F8, "F8"),
    (KeyCode::F9, "F9"),
    (KeyCode::F10, "F10"),
    (KeyCode::F11, "F11"),
    (KeyCode::F12, "F12"),
    (KeyCode::Space, "Space"),
    (KeyCode::Tab, "Tab"),
    (KeyCode::Backquote, "`"),
    (KeyCode::Minus, "-"),
    (KeyCode::Equal, "="),
    (KeyCode::BracketLeft, "["),
    (KeyCode::BracketRight, "]"),
    (KeyCode::Backslash, "\\"),
    (KeyCode::Semicolon, ";"),
    (KeyCode::Quote, "'"),
    (KeyCode::Comma, ","),
    (KeyCode::Period, "."),
    (KeyCode::Slash, "/"),
    (KeyCode::Insert, "Insert"),
    (KeyCode::Delete, "Delete"),
    (KeyCode::Home, "Home"),
    (KeyCode::End, "End"),
    (KeyCode::PageUp, "PageUp"),
    (KeyCode::PageDown, "PageDown"),
    (KeyCode::ArrowUp, "Up"),
    (KeyCode::ArrowDown, "Down"),
    (KeyCode::ArrowLeft, "Left"),
    (KeyCode::ArrowRight, "Right"),
    (KeyCode::Numpad0, "Num0"),
    (KeyCode::Numpad1, "Num1"),
    (KeyCode::Numpad2, "Num2"),
    (KeyCode::Numpad3, "Num3"),
    (KeyCode::Numpad4, "Num4"),
    (KeyCode::Numpad5, "Num5"),
    (KeyCode::Numpad6, "Num6"),
    (KeyCode::Numpad7, "Num7"),
    (KeyCode::Numpad8, "Num8"),
    (KeyCode::Numpad9, "Num9"),
    (KeyCode::NumpadAdd, "Num+"),
    (KeyCode::NumpadSubtract, "Num-"),
    (KeyCode::NumpadMultiply, "Num*"),
    (KeyCode::NumpadDivide, "Num/"),
];

impl Binding {
    /// Whether `code` can be bound (Escape and Enter stay the game's own).
    pub fn bindable(code: KeyCode) -> bool {
        KEYS.iter().any(|(key, _)| *key == code)
    }

    /// "Ctrl+Shift+B".
    pub fn label(self) -> String {
        let key = KEYS.iter().find(|(key, _)| *key == self.code).map_or("?", |(_, name)| name);
        let mut label = String::new();
        for (held, name) in [(self.ctrl, "Ctrl+"), (self.alt, "Alt+"), (self.shift, "Shift+")] {
            if held {
                label += name;
            }
        }
        label + key
    }

    fn parse(text: &str) -> Option<Self> {
        let mut binding = Self { code: KeyCode::KeyA, ctrl: false, alt: false, shift: false };
        let mut key = None;
        for part in text.split('+').filter(|part| !part.is_empty()) {
            match part {
                "Ctrl" => binding.ctrl = true,
                "Alt" => binding.alt = true,
                "Shift" => binding.shift = true,
                name => key = KEYS.iter().find(|(_, label)| *label == name).map(|(code, _)| *code),
            }
        }
        // "Num+" and "+" split oddly: a trailing plus is the key's own.
        if key.is_none() && text.ends_with("Num+") {
            key = Some(KeyCode::NumpadAdd);
        }
        binding.code = key?;
        Some(binding)
    }
}

/// The key bound to each command.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Keybinds {
    binds: [Option<Binding>; Command::ALL.len()],
}

impl Default for Keybinds {
    fn default() -> Self {
        Self { binds: Command::ALL.map(Command::default_binding) }
    }
}

impl Keybinds {
    pub fn get(&self, command: Command) -> Option<Binding> {
        self.binds[command.index()]
    }

    /// The command `binding` gives.
    pub fn command_for(&self, binding: Binding) -> Option<Command> {
        Command::ALL.into_iter().find(|command| self.get(*command) == Some(binding))
    }

    /// Binds `command` to `binding`; the command that had it loses it, and
    /// is returned.
    pub fn set(&mut self, command: Command, binding: Option<Binding>) -> Option<Command> {
        let previous = binding.and_then(|binding| self.command_for(binding)).filter(|other| *other != command);
        if let Some(previous) = previous {
            self.binds[previous.index()] = None;
        }
        self.binds[command.index()] = binding;
        previous
    }

    pub fn assigned(&self) -> usize {
        self.binds.iter().filter(|binding| binding.is_some()).count()
    }

    fn format(&self) -> String {
        Command::ALL
            .into_iter()
            .map(|command| format!("{}={}\n", command.id(), self.get(command).map(Binding::label).unwrap_or_default()))
            .collect()
    }

    fn parse(text: &str) -> Self {
        let mut binds = Self::default();
        for line in text.lines() {
            let Some((id, value)) = line.split_once('=') else {
                continue;
            };
            let Some(command) = Command::ALL.into_iter().find(|command| command.id() == id.trim()) else {
                continue;
            };
            binds.binds[command.index()] = Binding::parse(value.trim());
        }
        binds
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
            eprintln!("Could not save the shortcuts: {error}");
        }
    }
}

fn path() -> Option<PathBuf> {
    crate::saved::path("keybinds.ini")
}

// ---------------------------------------------------------------- window

/// The window's size.
pub const SIZE: [u32; 2] = [640, 470];
const LIST_LEFT: i32 = 188;
const LIST_TOP: i32 = 90;
const ROW: i32 = 40;
const ROWS: usize = 8;

/// The Shortcuts window as it is drawn.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct ShortcutsView {
    pub binds: Keybinds,
    pub category: Category,
    pub scroll: usize,
    /// The command waiting for a key.
    pub capturing: Option<Command>,
    /// What last happened ("B moved from Bounty Tasks").
    pub message: String,
    pub hover: Option<ShortcutAction>,
}

/// A click in the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShortcutAction {
    Category(Category),
    Bind(Command),
    Clear(Command),
    Restore,
    Close,
}

impl ShortcutsView {
    pub fn listed(&self) -> Vec<Command> {
        Command::ALL
            .into_iter()
            .filter(|command| self.category == Category::All || command.category() == self.category)
            .collect()
    }

    /// Scrolls by `rows` (down positive), within the list.
    pub fn scroll_by(&mut self, rows: i32) {
        let most = self.listed().len().saturating_sub(ROWS);
        self.scroll = (self.scroll as i32 + rows).clamp(0, most as i32) as usize;
    }
}

fn category_rect(index: usize) -> UiRect {
    UiRect::new(16, 84 + index as i32 * 34, 160, 28)
}

fn row_rect(row: usize) -> UiRect {
    UiRect::new(LIST_LEFT, LIST_TOP + row as i32 * ROW, 436, ROW - 4)
}

fn bind_rect(row: usize) -> UiRect {
    let rect = row_rect(row);
    UiRect::new(rect.x + rect.width - 152, rect.y + 5, 118, 26)
}

fn clear_rect(row: usize) -> UiRect {
    let rect = row_rect(row);
    UiRect::new(rect.x + rect.width - 28, rect.y + 9, 18, 18)
}

fn restore_rect() -> UiRect {
    UiRect::new(SIZE[0] as i32 - 176, SIZE[1] as i32 - 46, 160, 28)
}

fn close_rect() -> UiRect {
    UiRect::new(SIZE[0] as i32 - 31, 6, 27, 30)
}

/// What a click at `local` (from the window's corner) does.
pub fn action_at(view: &ShortcutsView, local: [f32; 2]) -> Option<ShortcutAction> {
    if close_rect().contains_local(local) {
        return Some(ShortcutAction::Close);
    }
    if restore_rect().contains_local(local) {
        return Some(ShortcutAction::Restore);
    }
    for (index, category) in Category::ALL.into_iter().enumerate() {
        if category_rect(index).contains_local(local) {
            return Some(ShortcutAction::Category(category));
        }
    }
    for (row, command) in view.listed().into_iter().skip(view.scroll).take(ROWS).enumerate() {
        if view.binds.get(command).is_some() && clear_rect(row).contains_local(local) {
            return Some(ShortcutAction::Clear(command));
        }
        if bind_rect(row).contains_local(local) {
            return Some(ShortcutAction::Bind(command));
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

/// The window's contents (the frame goes around them: `shop::framed`).
pub fn draw(image: &mut RgbaImage, font: &Font, view: &ShortcutsView) {
    const GOLD: [u8; 3] = ui_theme::TEXT_GOLD;
    const TEXT: [u8; 3] = ui_theme::TEXT;
    const MUTED: [u8; 3] = ui_theme::TEXT_MUTED;
    // The categories.
    text(image, font, "BROWSE COMMANDS", GOLD, 18, 52, 11.0);
    text(image, font, "Choose a category", MUTED, 18, 66, 9.0);
    for (index, category) in Category::ALL.into_iter().enumerate() {
        let rect = category_rect(index);
        let count = Command::ALL
            .iter()
            .filter(|command| category == Category::All || command.category() == category)
            .count();
        let hovered = view.hover == Some(ShortcutAction::Category(category));
        draw_tab(image, font, rect, "", view.category == category, hovered);
        text(image, font, category.label(), if view.category == category { GOLD } else { TEXT }, rect.x + 10, rect.y + 8, 10.0);
        let count = count.to_string();
        let width = crate::daily_rewards_panel::width_of(font, &count, 9.0) as i32;
        text(image, font, &count, MUTED, rect.x + rect.width - 10 - width, rect.y + 9, 9.0);
    }
    // The list's head.
    text(image, font, view.category.label(), GOLD, LIST_LEFT, 52, 12.0);
    text(image, font, view.category.blurb(), MUTED, LIST_LEFT, 68, 9.0);
    let assigned = format!("{} / {} ASSIGNED", view.binds.assigned(), Command::ALL.len());
    let width = crate::daily_rewards_panel::width_of(font, &assigned, 9.0) as i32;
    let pill = UiRect::new(LIST_LEFT + 436 - width - 20, 50, width + 16, 20);
    fill(image, pill, [12, 40, 52, 255]);
    outline(image, pill, [60, 150, 170, 255]);
    text(image, font, &assigned, [120, 210, 225], pill.x + 8, pill.y + 5, 9.0);
    // The commands.
    let listed = view.listed();
    for (row, command) in listed.iter().copied().skip(view.scroll).take(ROWS).enumerate() {
        let rect = row_rect(row);
        let capturing = view.capturing == Some(command);
        draw_card(image, rect, if capturing { WidgetState::Selected } else { WidgetState::Normal });
        fill(image, UiRect::new(rect.x, rect.y, 3, rect.height), [60, 150, 170, 255]);
        text(image, font, &command.name(), TEXT, rect.x + 12, rect.y + 5, 11.0);
        text(image, font, command.description(), MUTED, rect.x + 12, rect.y + 21, 9.0);
        let binding = view.binds.get(command);
        let label = if capturing {
            "Press a key...".to_owned()
        } else {
            binding.map_or_else(|| "+ Assign".to_owned(), Binding::label)
        };
        let bind = bind_rect(row);
        let hovered = view.hover == Some(ShortcutAction::Bind(command));
        fill(image, bind, if capturing { [70, 52, 20, 255] } else if hovered { [20, 54, 66, 255] } else { [10, 30, 40, 255] });
        outline(
            image,
            bind,
            if capturing {
                [255, 218, 109, 255]
            } else if binding.is_some() {
                [60, 150, 170, 255]
            } else {
                [70, 90, 100, 255]
            },
        );
        let colour = if capturing { [255, 230, 140] } else if binding.is_some() { TEXT } else { MUTED };
        let width = crate::daily_rewards_panel::width_of(font, &label, 10.0) as i32;
        text(image, font, &label, colour, bind.x + (bind.width - width) / 2, bind.y + 7, 10.0);
        if binding.is_some() {
            let clear = clear_rect(row);
            let hovered = view.hover == Some(ShortcutAction::Clear(command));
            outline(image, clear, if hovered { [230, 130, 120, 255] } else { [90, 100, 110, 255] });
            text(image, font, "x", if hovered { [230, 130, 120] } else { MUTED }, clear.x + 6, clear.y + 3, 10.0);
        }
    }
    // A scrollbar when the list runs past the window.
    if listed.len() > ROWS {
        let track = UiRect::new(LIST_LEFT + 440, LIST_TOP, 4, ROWS as i32 * ROW - 4);
        fill(image, track, [20, 34, 42, 255]);
        let thumb_height = (track.height * ROWS as i32 / listed.len() as i32).max(16);
        let most = listed.len() - ROWS;
        let thumb_y = track.y + (track.height - thumb_height) * view.scroll as i32 / most.max(1) as i32;
        fill(image, UiRect::new(track.x, thumb_y, 4, thumb_height), [60, 150, 170, 255]);
    }
    // The foot: what happened, and the defaults.
    let foot = SIZE[1] as i32 - 46;
    text(image, font, "LATEST CHANGE", MUTED, 18, foot + 2, 8.0);
    let message = if view.capturing.is_some() {
        "Press the new key (with Ctrl, Alt or Shift if you like). Esc cancels."
    } else if view.message.is_empty() {
        "Choose a command and click its key to change it."
    } else {
        view.message.as_str()
    };
    text(image, font, message, TEXT, 18, foot + 14, 10.0);
    draw_button(
        image,
        font,
        restore_rect(),
        "Restore defaults",
        if view.hover == Some(ShortcutAction::Restore) { WidgetState::Hovered } else { WidgetState::Normal },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bindings_write_and_read_back() {
        let mut binds = Keybinds::default();
        let moved = binds.set(
            Command::Inventory,
            Some(Binding { code: KeyCode::KeyB, ctrl: false, alt: false, shift: false }),
        );
        assert_eq!(moved, Some(Command::Bounty), "B leaves the Bounty Tasks");
        assert_eq!(binds.get(Command::Bounty), None);
        binds.set(Command::Skills, Some(Binding { code: KeyCode::KeyS, ctrl: true, alt: false, shift: true }));
        binds.set(Command::Map, Some(Binding { code: KeyCode::NumpadAdd, ctrl: false, alt: false, shift: false }));
        let read = Keybinds::parse(&binds.format());
        assert_eq!(read, binds);
        assert_eq!(read.get(Command::Skills).unwrap().label(), "Ctrl+Shift+S");
        assert_eq!(
            read.command_for(Binding { code: KeyCode::KeyB, ctrl: false, alt: false, shift: false }),
            Some(Command::Inventory)
        );
    }

    #[test]
    fn the_defaults_keep_the_clients_own_keys() {
        let binds = Keybinds::default();
        assert_eq!(binds.get(Command::Slot1).unwrap().label(), "F1");
        assert_eq!(binds.get(Command::Stop).unwrap().label(), "Space");
        assert_eq!(binds.get(Command::Bounty).unwrap().label(), "B");
        assert_eq!(binds.assigned(), Command::ALL.len());
        let mut seen = std::collections::HashSet::new();
        assert!(Command::ALL.iter().all(|command| seen.insert(binds.get(*command))), "no key twice");
    }

    #[test]
    fn the_list_scrolls_within_its_commands() {
        let mut view = ShortcutsView::default();
        view.scroll_by(100);
        assert_eq!(view.scroll, Command::ALL.len() - ROWS);
        view.category = Category::Gameplay;
        view.scroll = 0;
        view.scroll_by(5);
        assert_eq!(view.scroll, 0, "the gameplay commands fit");
    }
}

