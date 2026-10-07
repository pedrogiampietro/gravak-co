//! Login, account registration, character creation and character
//! selection, shown before the game window.
//!
//! The screens are painted by the client (see [`crate::launcher_art`]) into
//! one image that is presented through `softbuffer`; the same layout drives
//! painting and hit-testing.

use crate::login::{CharacterSummary, LoginClient, LoginCommand, LoginEvent};
use crate::network::NetworkHandle;
use crate::settings::{LoginPrefs, SavedAccount};
use crate::worlds::{Status, World};
use image::RgbaImage;
use std::{
    num::NonZeroU32,
    sync::Arc,
    time::{Duration, Instant},
};
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::{ElementState, MouseButton, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{Key, NamedKey},
    platform::run_on_demand::EventLoopExtRunOnDemand,
    window::{Window, WindowId},
};

const FIELD_HEIGHT: i32 = 32;
/// How long a new screen takes to rise out of the dark, in seconds.
const SCREEN_FADE: f32 = 0.35;
const BUTTON_HEIGHT: i32 = 34;
const MAX_INPUT: usize = 15;

const GREY: [u8; 4] = [150, 164, 166, 255];
const ERROR: [u8; 4] = [240, 128, 108, 255];
const OK: [u8; 4] = [136, 220, 146, 255];
/// The client's version, shown on the login screen and compared by the
/// updater.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
/// One row of the world and saved-account lists.
const LIST_ROW: i32 = 30;

pub const BODIES: [(&str, u16); 4] = [
    ("Male", 1003),
    ("Male · Large", 1004),
    ("Female", 2001),
    ("Female · Large", 2002),
];
pub const CLASSES: [(&str, u16); 4] = [
    ("Trojan", 10),
    ("Warrior", 20),
    ("Archer", 40),
    ("Taoist", 100),
];

/// The faces a new character can pick, by sex: the portraits of
/// `data/PlayerFace` (the game server accepts the same ranges).
pub const MALE_FACES: std::ops::RangeInclusive<u16> = 1..=102;
pub const FEMALE_FACES: std::ops::RangeInclusive<u16> = 201..=290;
/// How large the face portrait is drawn on the creation screen.
pub const FACE_SIZE: u32 = 48;
/// The hair styles of 5065 (`ini/armet.ini`, entries
/// `<body>119<colour><style>`), the same for every body.
pub const HAIR_STYLES: [u16; 35] = [
    10, 11, 12, 13, 14, 15, 16, 17, 21, 22, 23, 24, 25, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43,
    44, 45, 46, 47, 48, 49, 50, 51,
];
/// The hair colours: the hundreds of a hair value.
pub const HAIR_COLOURS: [u16; 7] = [3, 4, 5, 6, 7, 8, 9];

/// The faces offered for body `mesh` (1003/1004 male, 2001/2002 female).
pub fn faces_for(mesh: u16) -> std::ops::RangeInclusive<u16> {
    if mesh >= 2000 { FEMALE_FACES } else { MALE_FACES }
}

/// One step through `count` choices from `index`, wrapping around.
fn step(index: usize, count: usize, next: bool) -> usize {
    let count = count.max(1);
    if next { (index + 1) % count } else { (index + count - 1) % count }
}

/// Name of a profession id, base or promoted.
pub fn class_name(class: u8) -> &'static str {
    match class {
        10..=15 => "Trojan",
        20..=25 => "Warrior",
        40..=45 => "Archer",
        100..=101 => "Taoist",
        132..=135 => "Water Taoist",
        142..=145 => "Fire Taoist",
        _ => "Adventurer",
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Screen {
    Login,
    Register,
    CreateCharacter,
    Select(CharacterSummary),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Login,
    OpenRegister,
    Exit,
    Register,
    Back,
    Create,
    Enter,
    Logout,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Field(usize),
    Button(Action),
    Body(usize),
    Class(usize),
    /// A starter coat dye, an index into `showcase::DYES`.
    Dye(usize),
    /// The face: the previous (`false`) or the next (`true`) one.
    Face(bool),
    /// The hair style: the previous or the next one.
    HairStyle(bool),
    /// A hair colour, an index into `HAIR_COLOURS`.
    HairColour(usize),
    /// The world box, which opens the world list.
    WorldBox,
    /// A world in the open list.
    World(usize),
    /// The arrow on the account field, which opens the saved accounts.
    SavedList,
    /// A saved account in the open list, and its forget cross.
    Saved(usize),
    Forget(usize),
    Check(Check),
}

/// The login screen's check boxes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Check {
    RememberAccount,
    SavePassword,
    HideAccount,
}

/// A list dropped down over the login panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum List {
    Worlds,
    Saved,
}

/// `pedro3` as `ped***`: the start of an account, enough to tell one's
/// own apart, without showing it whole.
pub fn mask(account: &str) -> String {
    let count = account.chars().count();
    let shown = if count > 3 { 3 } else { 1.min(count) };
    account.chars().take(shown).chain(std::iter::repeat_n('*', count - shown)).collect()
}

impl Target {
    /// The look controls laid over the chosen hero.
    fn on_hero(self) -> bool {
        matches!(self, Self::Dye(_) | Self::Face(_) | Self::HairStyle(_) | Self::HairColour(_))
    }
}

#[derive(Debug, Clone)]
struct Field {
    label: &'static str,
    value: String,
    secret: bool,
}

impl Field {
    fn new(label: &'static str, secret: bool) -> Self {
        Self {
            label,
            value: String::new(),
            secret,
        }
    }
}

/// What the screens show and what the player has typed.
pub struct LoginUi {
    pub screen: Screen,
    fields: Vec<Field>,
    focus: usize,
    pub body: usize,
    pub class: usize,
    status: String,
    status_colour: [u8; 4],
    /// A request is in flight; buttons wait for its answer.
    pub busy: bool,
    pub hovered: Option<Target>,
    account: String,
    /// A screen change waiting for its first frame, and when its fade began.
    fade_pending: std::cell::Cell<bool>,
    fade_from: std::cell::Cell<Option<f32>>,
    /// The professions' heroes by (body, class), as they finish loading.
    heroes: std::collections::HashMap<(usize, usize), crate::showcase::Hero>,
    /// Each body in the starter coat by (body, dye), and the dyes' swatch
    /// colours by (body, dye).
    coats: std::collections::HashMap<(usize, usize), crate::showcase::Hero>,
    coat_colours: std::collections::HashMap<(usize, usize), [u8; 3]>,
    /// The starter coat's dye, an index into `showcase::DYES`.
    pub dye: usize,
    /// The face, an index into the body's faces (`faces_for`).
    pub face: usize,
    /// The hair, indices into `HAIR_STYLES` and `HAIR_COLOURS`.
    pub hair_style: usize,
    pub hair_colour: usize,
    /// Face portraits by face number.
    faces: std::collections::HashMap<u16, RgbaImage>,
    /// Each body's hair colour swatches, in `HAIR_COLOURS` order.
    hair_swatches: std::collections::HashMap<usize, Vec<[u8; 3]>>,
    /// The new character in other hair than the heroes' by its look.
    looks: std::collections::HashMap<crate::showcase::LookKey, crate::showcase::Hero>,
    /// Asks `showcase` for a look, and the last look asked for.
    look_requests: Option<std::sync::mpsc::Sender<crate::showcase::LookKey>>,
    last_look_request: std::cell::Cell<Option<crate::showcase::LookKey>>,
    /// The look drawn last, kept while the next one loads.
    shown_look: std::cell::Cell<Option<crate::showcase::LookKey>>,
    /// The columns not chosen, painted once per (class, state, place, body,
    /// hero loaded, light tick).
    column_cache: std::cell::RefCell<
        std::collections::HashMap<(usize, crate::launcher_art::ColumnState, [i32; 4], usize, bool, i64), RgbaImage>,
    >,
    /// The chosen hero's latest pose, by (body, class, size, tick, turn).
    chosen_hero: std::cell::RefCell<Option<((usize, usize, [u32; 2], i64, i64), RgbaImage)>>,
    /// How far the player has turned the chosen hero (radians), and the
    /// drag in progress: where it started and the turn then.
    turn: f32,
    turning: Option<(f32, f32)>,
    /// Still renders of the heroes not selected, by (body, class).
    still_heroes: std::cell::RefCell<std::collections::HashMap<(usize, usize), RgbaImage>>,
    /// The worlds to log in to, the one chosen, and how each answers.
    worlds: Vec<World>,
    pub world: usize,
    world_status: Vec<Status>,
    /// Remembered accounts and the check boxes; `prefs_changed` asks the
    /// launcher to save them.
    prefs: LoginPrefs,
    prefs_changed: bool,
    /// The list dropped down, if any.
    list: Option<List>,
    /// The password field holds a saved password, cleared when the account
    /// is edited.
    autofilled: bool,
    /// The account and password of the login in flight, remembered once
    /// the server accepts them.
    pending: Option<(String, String)>,
    /// The updater's line at the foot of the screen, and whether the
    /// buttons wait for it.
    pub update_status: Option<(String, [u8; 4])>,
    pub updating: bool,
}

/// What the screens ask the login session to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Request {
    Send(LoginCommand),
    Exit,
}

impl LoginUi {
    pub fn new(worlds: Vec<World>, mut prefs: LoginPrefs) -> Self {
        assert!(!worlds.is_empty(), "the login screen needs a world");
        let world = prefs
            .world
            .as_ref()
            .and_then(|name| worlds.iter().position(|world| &world.name == name))
            .unwrap_or(0);
        // Accounts saved before worlds belong to the first one.
        for saved in &mut prefs.accounts {
            if saved.world.is_empty() {
                saved.world = worlds[0].name.clone();
            }
        }
        let account = prefs
            .accounts_on(&worlds[world].name)
            .next()
            .map(|saved| saved.account.clone())
            .unwrap_or_default();
        let mut ui = Self {
            screen: Screen::Login,
            fields: Vec::new(),
            focus: 0,
            body: 0,
            class: 0,
            status: String::new(),
            status_colour: GREY,
            busy: false,
            hovered: None,
            account,
            fade_pending: std::cell::Cell::new(false),
            fade_from: std::cell::Cell::new(None),
            heroes: std::collections::HashMap::new(),
            coats: std::collections::HashMap::new(),
            coat_colours: std::collections::HashMap::new(),
            dye: 0,
            face: 0,
            hair_style: 0,
            hair_colour: 0,
            faces: std::collections::HashMap::new(),
            hair_swatches: std::collections::HashMap::new(),
            looks: std::collections::HashMap::new(),
            look_requests: None,
            last_look_request: std::cell::Cell::new(None),
            shown_look: std::cell::Cell::new(None),
            still_heroes: std::cell::RefCell::new(std::collections::HashMap::new()),
            column_cache: std::cell::RefCell::new(std::collections::HashMap::new()),
            chosen_hero: std::cell::RefCell::new(None),
            turn: 0.0,
            turning: None,
            world_status: vec![Status::Checking; worlds.len()],
            worlds,
            world,
            prefs,
            prefs_changed: false,
            list: None,
            autofilled: false,
            pending: None,
            update_status: None,
            updating: false,
        };
        ui.show(Screen::Login);
        ui
    }

    /// Where looks with another hair are asked for (`showcase::start`).
    pub fn set_look_requests(&mut self, sender: std::sync::mpsc::Sender<crate::showcase::LookKey>) {
        self.look_requests = Some(sender);
    }

    /// The face picked for the chosen body.
    pub fn avatar(&self) -> u16 {
        let faces = faces_for(BODIES[self.body].1);
        let count = usize::from(faces.end() - faces.start()) + 1;
        faces.start() + (self.face % count) as u16
    }

    /// The hair picked: colour * 100 + style.
    pub fn hair(&self) -> u16 {
        HAIR_COLOURS[self.hair_colour] * 100 + HAIR_STYLES[self.hair_style]
    }

    fn show(&mut self, screen: Screen) {
        self.fields = match screen {
            Screen::Login => vec![Field::new("Account", false), Field::new("Password", true)],
            Screen::Register => vec![
                Field::new("Account", false),
                Field::new("Password", true),
                Field::new("Confirm password", true),
            ],
            Screen::CreateCharacter => vec![Field::new("Character name", false)],
            Screen::Select(_) => Vec::new(),
        };
        self.focus = 0;
        self.autofilled = false;
        self.list = None;
        if matches!(screen, Screen::Login | Screen::Register) && !self.account.is_empty() {
            self.fields[0].value = self.account.clone();
            self.focus = 1;
            if screen == Screen::Login
                && let Some(password) = self.saved_password(&self.account)
            {
                self.fields[1].value = password;
                self.autofilled = true;
            }
        }
        self.screen = screen;
        self.fade_pending.set(true);
        self.hovered = None;
    }

    /// Shows the updater's progress; `true` when the client must restart.
    pub fn update_event(&mut self, event: &crate::updater::UpdateEvent) -> bool {
        use crate::updater::UpdateEvent;
        const MB: f64 = 1024.0 * 1024.0;
        let dim = crate::launcher_art::DIM;
        let (text, colour, updating) = match event {
            UpdateEvent::Checking => ("Checking for updates...".to_owned(), dim, true),
            UpdateEvent::Downloading { version, file, done, total } => (
                format!(
                    "Updating to v{version}  ·  {file}  ·  {}%  ({:.1} of {:.1} MB)",
                    done * 100 / (*total).max(1),
                    *done as f64 / MB,
                    *total as f64 / MB
                ),
                crate::launcher_art::GOLD,
                true,
            ),
            UpdateEvent::UpToDate => ("Up to date".to_owned(), dim, false),
            UpdateEvent::Updated(version) => (format!("Updated to v{version}. Restarting..."), OK, true),
            UpdateEvent::Failed(error) => (format!("Could not update: {error}"), ERROR, false),
        };
        self.update_status = Some((text, colour));
        self.updating = updating;
        matches!(event, UpdateEvent::Updated(_))
    }

    pub fn world(&self) -> &World {
        &self.worlds[self.world]
    }

    pub fn set_world_status(&mut self, index: usize, status: Status) {
        if let Some(slot) = self.world_status.get_mut(index) {
            *slot = status;
        }
    }

    /// The remembered accounts and choices, once, after they change.
    pub fn take_prefs(&mut self) -> Option<LoginPrefs> {
        std::mem::take(&mut self.prefs_changed).then(|| self.prefs.clone())
    }

    fn saved(&self) -> Vec<&SavedAccount> {
        self.prefs.accounts_on(&self.worlds[self.world].name).collect()
    }

    fn saved_password(&self, account: &str) -> Option<String> {
        self.saved()
            .into_iter()
            .find(|saved| saved.account.eq_ignore_ascii_case(account))
            .and_then(|saved| saved.password.clone())
    }

    /// An account as the screen shows it.
    fn shown_account(&self, account: &str) -> String {
        if self.prefs.hide_account { mask(account) } else { account.to_owned() }
    }

    fn choose_world(&mut self, index: usize) {
        if index >= self.worlds.len() {
            return;
        }
        self.world = index;
        self.prefs.world = Some(self.worlds[index].name.clone());
        self.prefs_changed = true;
        // The world's own last account, when it has one.
        if let Some(saved) = self.saved().first().map(|saved| saved.account.clone()) {
            self.account = saved;
            let screen = self.screen.clone();
            self.show(screen);
        } else if self.autofilled {
            self.fields[1].value.clear();
            self.autofilled = false;
        }
        self.info(format!("World: {}", self.worlds[index].name));
    }

    fn choose_saved(&mut self, index: usize) {
        let Some(saved) = self.saved().get(index).map(|saved| (*saved).clone()) else {
            return;
        };
        self.fields[0].value = saved.account.clone();
        self.account = saved.account;
        self.fields[1].value = saved.password.clone().unwrap_or_default();
        self.autofilled = saved.password.is_some();
        self.focus = if self.autofilled { 0 } else { 1 };
        self.info("");
    }

    fn forget_saved(&mut self, index: usize) {
        let Some(saved) = self.saved().get(index).map(|saved| (*saved).clone()) else {
            return;
        };
        self.prefs.forget(&saved.world, &saved.account);
        self.prefs_changed = true;
        if self.autofilled && self.fields[0].value.eq_ignore_ascii_case(&saved.account) {
            self.fields[1].value.clear();
            self.autofilled = false;
        }
        self.info(format!("{} forgotten.", self.shown_account(&saved.account)));
    }

    fn toggle(&mut self, check: Check) {
        let prefs = &mut self.prefs;
        match check {
            Check::RememberAccount => {
                prefs.remember_account = !prefs.remember_account;
                // A password is only saved with its account.
                prefs.save_password &= prefs.remember_account;
            }
            Check::SavePassword => {
                prefs.save_password = !prefs.save_password;
                prefs.remember_account |= prefs.save_password;
            }
            Check::HideAccount => prefs.hide_account = !prefs.hide_account,
        }
        self.prefs_changed = true;
    }

    /// The server took the login: remember (or forget) the account.
    fn commit_login(&mut self) {
        let Some((account, password)) = self.pending.take() else {
            return;
        };
        let world = self.worlds[self.world].name.clone();
        if self.prefs.remember_account {
            let password = self.prefs.save_password.then_some(password);
            self.prefs.remember(SavedAccount { world: world.clone(), account, password });
        } else {
            self.prefs.forget(&world, &account);
        }
        self.prefs.world = Some(world);
        self.prefs_changed = true;
    }

    fn set_status(&mut self, text: impl Into<String>, colour: [u8; 4]) {
        self.status = text.into();
        self.status_colour = colour;
    }

    fn info(&mut self, text: impl Into<String>) {
        self.set_status(text, GREY);
    }

    fn error(&mut self, text: impl Into<String>) {
        self.busy = false;
        self.set_status(text, ERROR);
    }

    #[cfg(test)]
    pub fn status(&self) -> &str {
        &self.status
    }

    pub fn field_value(&self, index: usize) -> &str {
        self.fields
            .get(index)
            .map_or("", |field| field.value.as_str())
    }

    /// Updates the screens from the login session.
    pub fn handle_event(&mut self, event: &LoginEvent) {
        match event {
            LoginEvent::Status(text) => self.info(format!("{text}...")),
            LoginEvent::Registered(account) => {
                self.busy = false;
                self.account = account.clone();
                self.show(Screen::Login);
                self.set_status("Account created. You can log in now.", OK);
            }
            LoginEvent::RegisterFailed(message)
            | LoginEvent::LoginFailed(message)
            | LoginEvent::CharacterRejected(message) => self.error(capitalise(message)),
            LoginEvent::NeedsCharacter => {
                self.commit_login();
                self.busy = false;
                self.show(Screen::CreateCharacter);
                self.info("Create your character.");
            }
            LoginEvent::Character(summary) => {
                self.commit_login();
                self.busy = false;
                self.show(Screen::Select(summary.clone()));
                self.info("");
            }
            LoginEvent::Entered(_) => {
                self.busy = true;
                self.info("Entering the world...");
            }
            LoginEvent::Disconnected(message) => {
                self.show(Screen::Login);
                self.error(capitalise(message));
            }
        }
    }

    /// Runs a button's action.
    pub fn activate(&mut self, action: Action) -> Option<Request> {
        if (self.busy || self.updating) && action != Action::Exit {
            return None;
        }
        self.list = None;
        match action {
            Action::Exit => Some(Request::Exit),
            Action::OpenRegister => {
                self.show(Screen::Register);
                self.info("Choose an account name and a password.");
                None
            }
            Action::Back => {
                self.show(Screen::Login);
                self.info("");
                None
            }
            Action::Logout => {
                self.show(Screen::Login);
                self.info("");
                Some(Request::Send(LoginCommand::Logout))
            }
            Action::Login => {
                let username = self.field_value(0).to_owned();
                let password = self.field_value(1).to_owned();
                if username.is_empty() || password.is_empty() {
                    self.error("Enter your account and password.");
                    return None;
                }
                self.account = username.clone();
                self.pending = Some((username.clone(), password.clone()));
                self.busy = true;
                self.info("Logging in...");
                Some(Request::Send(LoginCommand::Login {
                    world: self.world().clone(),
                    username,
                    password,
                }))
            }
            Action::Register => {
                let username = self.field_value(0).to_owned();
                let password = self.field_value(1).to_owned();
                if !valid_account(&username) {
                    self.error("Account names are 4-15 letters, digits or _.");
                    return None;
                }
                if !valid_password(&password) {
                    self.error("Passwords are 4-15 characters, without spaces.");
                    return None;
                }
                if password != self.field_value(2) {
                    self.error("The passwords do not match.");
                    return None;
                }
                self.busy = true;
                self.info("Creating account...");
                Some(Request::Send(LoginCommand::Register {
                    world: self.world().clone(),
                    username,
                    password,
                }))
            }
            Action::Create => {
                let name = self.field_value(0).to_owned();
                if !valid_character_name(&name) {
                    self.error("Names are 3-15 letters or digits.");
                    return None;
                }
                self.busy = true;
                self.info(format!("Creating {name}..."));
                Some(Request::Send(LoginCommand::CreateCharacter {
                    name,
                    mesh: BODIES[self.body].1,
                    class: CLASSES[self.class].1,
                    coat_dye: crate::showcase::DYES[self.dye],
                    avatar: self.avatar(),
                    hair: self.hair(),
                }))
            }
            Action::Enter => {
                self.busy = true;
                self.info("Entering the world...");
                Some(Request::Send(LoginCommand::Enter))
            }
        }
    }

    /// Enter runs the screen's main button, Escape goes back.
    fn primary(&self) -> Action {
        match self.screen {
            Screen::Login => Action::Login,
            Screen::Register => Action::Register,
            Screen::CreateCharacter => Action::Create,
            Screen::Select(_) => Action::Enter,
        }
    }

    fn escape(&self) -> Action {
        match self.screen {
            Screen::Login => Action::Exit,
            Screen::Register => Action::Back,
            Screen::CreateCharacter | Screen::Select(_) => Action::Logout,
        }
    }

    pub fn click(&mut self, size: [u32; 2], cursor: [f32; 2]) -> Option<Request> {
        let target = self.target_at(size, cursor);
        // A click anywhere but on the open list closes it.
        let list = self.list.take();
        match target? {
            Target::WorldBox => {
                self.list = (list != Some(List::Worlds) && !self.busy).then_some(List::Worlds);
                None
            }
            Target::World(index) => {
                self.choose_world(index);
                None
            }
            Target::SavedList => {
                self.list = (list != Some(List::Saved) && !self.busy).then_some(List::Saved);
                None
            }
            Target::Saved(index) => {
                self.choose_saved(index);
                None
            }
            Target::Forget(index) => {
                self.forget_saved(index);
                // The list stays open for the others.
                self.list = list.filter(|_| !self.saved().is_empty());
                None
            }
            Target::Check(check) => {
                self.toggle(check);
                None
            }
            Target::Field(index) => {
                self.focus = index;
                None
            }
            Target::Body(index) => {
                self.body = index;
                // The same place in the other sex's faces.
                let faces = faces_for(BODIES[index].1);
                self.face %= usize::from(faces.end() - faces.start()) + 1;
                None
            }
            Target::Class(index) => {
                self.class = index;
                None
            }
            Target::Dye(dye) => {
                self.dye = dye;
                None
            }
            Target::Face(next) => {
                let faces = faces_for(BODIES[self.body].1);
                self.face = step(self.face, usize::from(faces.end() - faces.start()) + 1, next);
                None
            }
            Target::HairStyle(next) => {
                self.hair_style = step(self.hair_style, HAIR_STYLES.len(), next);
                None
            }
            Target::HairColour(colour) => {
                self.hair_colour = colour;
                None
            }
            Target::Button(action) => self.activate(action),
        }
    }

    pub fn key(&mut self, key: &Key, text: Option<&str>, shift: bool) -> Option<Request> {
        if self.list.take().is_some() && matches!(key, Key::Named(NamedKey::Escape)) {
            return None;
        }
        let typing = match key {
            Key::Named(NamedKey::Backspace) => true,
            Key::Named(_) => false,
            _ => text.is_some_and(|text| text.chars().any(|c| c.is_ascii_graphic())),
        };
        if typing && self.screen == Screen::Login && self.autofilled {
            // A saved password goes with its account: changing either
            // starts the password afresh.
            self.autofilled = false;
            self.fields[1].value.clear();
            if self.focus == 1 && matches!(key, Key::Named(NamedKey::Backspace)) {
                return None;
            }
        }
        match key {
            Key::Named(NamedKey::Enter) => return self.activate(self.primary()),
            Key::Named(NamedKey::Escape) => return self.activate(self.escape()),
            Key::Named(NamedKey::Tab) if !self.fields.is_empty() => {
                let count = self.fields.len();
                self.focus = if shift {
                    (self.focus + count - 1) % count
                } else {
                    (self.focus + 1) % count
                };
            }
            Key::Named(NamedKey::Backspace) => {
                if let Some(field) = self.fields.get_mut(self.focus) {
                    field.value.pop();
                }
            }
            _ => {
                if let (Some(text), Some(field)) = (text, self.fields.get_mut(self.focus)) {
                    for c in text.chars().filter(char::is_ascii_graphic) {
                        if field.value.len() < MAX_INPUT {
                            field.value.push(c);
                        }
                    }
                }
            }
        }
        None
    }

    pub fn target_at(&self, size: [u32; 2], cursor: [f32; 2]) -> Option<Target> {
        let point = [cursor[0].floor() as i32, cursor[1].floor() as i32];
        self.layout(size)
            .targets
            .into_iter()
            .find(|(rect, _)| inside(*rect, point))
            .map(|(_, target)| target)
    }

    fn title(&self) -> &'static str {
        match self.screen {
            Screen::Login => "Login",
            Screen::Register => "Create Account",
            Screen::CreateCharacter => "Create Character",
            Screen::Select(_) => "Your Character",
        }
    }

    fn buttons(&self) -> Vec<(Action, &'static str)> {
        match self.screen {
            Screen::Login => vec![
                (Action::Login, "Login"),
                (Action::OpenRegister, "Register"),
                (Action::Exit, "Exit"),
            ],
            Screen::Register => vec![(Action::Register, "Create Account"), (Action::Back, "Back")],
            Screen::CreateCharacter => vec![(Action::Create, "Create"), (Action::Logout, "Back")],
            Screen::Select(_) => vec![(Action::Enter, "Enter the World"), (Action::Logout, "Back")],
        }
    }

    fn layout(&self, size: [u32; 2]) -> Layout {
        if self.screen == Screen::CreateCharacter {
            return self.creation_layout(size);
        }
        let [width, height] = [size[0] as i32, size[1] as i32];
        let mut targets = Vec::new();
        let mut fields = Vec::new();
        let classes = Vec::new();
        let bodies = Vec::new();
        let mut world_box = None;
        let mut saved_arrow = None;
        let mut checks = Vec::new();
        let panel_width;
        let mut y;
        match self.screen {
            Screen::Login | Screen::Register => {
                panel_width = 420;
                y = 66 + 20;
                let rect = [36, y, panel_width - 72, FIELD_HEIGHT];
                world_box = Some(rect);
                targets.push((rect, Target::WorldBox));
                y += FIELD_HEIGHT + 14;
                for index in 0..self.fields.len() {
                    y += 20;
                    let rect = [36, y, panel_width - 72, FIELD_HEIGHT];
                    fields.push(rect);
                    if index == 0 && self.screen == Screen::Login && !self.saved().is_empty() {
                        let arrow = [rect[0] + rect[2] - FIELD_HEIGHT, y, FIELD_HEIGHT, FIELD_HEIGHT];
                        saved_arrow = Some(arrow);
                        targets.push((arrow, Target::SavedList));
                    }
                    targets.push((rect, Target::Field(index)));
                    y += FIELD_HEIGHT + 14;
                }
                if self.screen == Screen::Login {
                    let mut boxes = vec![(Check::RememberAccount, "Remember")];
                    if crate::vault::AVAILABLE {
                        boxes.push((Check::SavePassword, "Save password"));
                    }
                    boxes.push((Check::HideAccount, "Hide account"));
                    let column = (panel_width - 72) / 3;
                    for (index, (check, caption)) in boxes.into_iter().enumerate() {
                        let rect = [36 + index as i32 * column, y - 2, 14, 14];
                        checks.push((rect, check, caption));
                        targets.push(([rect[0] - 2, rect[1] - 4, column - 4, 22], Target::Check(check)));
                    }
                    y += 22;
                }
            }
            Screen::CreateCharacter => unreachable!("laid out by creation_layout"),
            Screen::Select(_) => {
                panel_width = 540;
                y = 196;
            }
        }
        let status_y = y + 2;
        y += 28;
        let buttons = self.buttons();
        let count = buttons.len() as i32;
        let button_width = match self.screen {
            Screen::Login => 108,
            _ => 150,
        };
        let total = button_width * count + 14 * (count - 1);
        let mut button_rects = Vec::new();
        for (index, (action, label)) in buttons.into_iter().enumerate() {
            let rect = [
                (panel_width - total) / 2 + index as i32 * (button_width + 14),
                y,
                button_width,
                BUTTON_HEIGHT,
            ];
            button_rects.push((rect, action, label));
            targets.push((rect, Target::Button(action)));
        }
        let panel_height = y + BUTTON_HEIGHT + 26;
        // Room above the panel for the title; the login screens keep room
        // below for the professions.
        let (top_room, centre_shift) = match self.screen {
            Screen::CreateCharacter => (112, 40),
            Screen::Select(_) => (190, 30),
            Screen::Login | Screen::Register => (190, -40),
        };
        let origin = [
            ((width - panel_width) / 2).max(0),
            ((height - panel_height) / 2 + centre_shift).max(top_room),
        ];
        let offset = |rect: [i32; 4]| [rect[0] + origin[0], rect[1] + origin[1], rect[2], rect[3]];
        let panel = [origin[0], origin[1], panel_width, panel_height];
        // The open list drops below its box, over the rest of the panel,
        // and takes the clicks first.
        let mut list_rows = Vec::new();
        let anchor = match self.list {
            Some(List::Worlds) => world_box.map(|rect| (rect, self.worlds.len())),
            Some(List::Saved) => fields.first().map(|rect| (*rect, self.saved().len())),
            None => None,
        };
        if let Some((anchor, count)) = anchor {
            let [x, y, w, h] = anchor;
            let mut overlay = Vec::new();
            for index in 0..count {
                let row = [x, y + h + 2 + index as i32 * LIST_ROW, w, LIST_ROW];
                list_rows.push((offset(row), index));
                if self.list == Some(List::Saved) {
                    overlay.push(([x + w - 30, row[1], 30, LIST_ROW], Target::Forget(index)));
                    overlay.push((row, Target::Saved(index)));
                } else {
                    overlay.push((row, Target::World(index)));
                }
            }
            overlay.append(&mut targets);
            targets = overlay;
        }
        // The four professions along the bottom of the login screens.
        let mut showcase = Vec::new();
        if matches!(self.screen, Screen::Login | Screen::Register)
            && height - (panel[1] + panel[3]) >= 180
        {
            let spacing = (width / 4).min(210);
            for index in 0..4 {
                let centre = width / 2 + (index * 2 - 3) * spacing / 2;
                showcase.push([centre - 34, height - 168, 68, 68]);
            }
        }
        Layout {
            panel,
            world_box: world_box.map(offset),
            saved_arrow: saved_arrow.map(offset),
            checks: checks.into_iter().map(|(rect, check, caption)| (offset(rect), check, caption)).collect(),
            list_rows,
            fields: fields.into_iter().map(offset).collect(),
            classes: classes.into_iter().map(offset).collect(),
            bodies: bodies.into_iter().map(offset).collect(),
            status_y: origin[1] + status_y,
            buttons: button_rects
                .into_iter()
                .map(|(rect, action, label)| (offset(rect), action, label))
                .collect(),
            targets: targets
                .into_iter()
                .map(|(rect, target)| (offset(rect), target))
                .collect(),
            showcase,
        }
    }

    /// Paints the whole window.
    /// Paints the whole window; a new screen rises out of the dark over
    /// its first moments.
    pub fn image(&self, size: [u32; 2], time: f32) -> RgbaImage {
        let mut image = self.paint(size, time);
        if self.fade_pending.replace(false) {
            self.fade_from.set(Some(time));
        }
        if let Some(from) = self.fade_from.get() {
            let t = ((time - from) / SCREEN_FADE).clamp(0.0, 1.0);
            if t >= 1.0 {
                self.fade_from.set(None);
            } else {
                let light = t * t * (3.0 - 2.0 * t);
                for pixel in image.pixels_mut() {
                    for c in 0..3 {
                        pixel.0[c] = (f32::from(pixel.0[c]) * light) as u8;
                    }
                }
            }
        }
        image
    }

    fn paint(&self, size: [u32; 2], time: f32) -> RgbaImage {
        use crate::launcher_art as art;
        let size = [size[0].max(1), size[1].max(1)];
        let mut image = art::backdrop(size);
        art::scene(&mut image, time);
        art::embers(&mut image, time);
        let layout = self.layout(size);
        if self.screen == Screen::CreateCharacter {
            self.paint_creation(&mut image, &layout, time);
            return image;
        }
        let centre = size[0] as i32 / 2;
        let [px, py, pw, ph] = layout.panel;
        let caret = (time * 2.0).fract() < 0.55;

        // Title.
        let compact = false;
        let (title_size, title_y) = if compact {
            (44.0, py - 108)
        } else {
            (68.0, py - 172)
        };
        art::gold_title(&mut image, "Gravak", title_size, centre, title_y);
        art::title_shine(&mut image, "Gravak", title_size, centre, title_y, time);
        let subtitle_y = if compact { py - 26 } else { py - 50 };
        art::label(
            &mut image,
            "CONQUER ONLINE  ·  5065",
            13.0,
            centre,
            subtitle_y,
            art::GOLD,
        );
        if !compact {
            art::ornament_rule(&mut image, centre, subtitle_y + 22, 140);
        }

        art::panel(&mut image, layout.panel, Some(self.title()));

        match &self.screen {
            Screen::Login | Screen::Register => {
                if let Some(rect) = layout.world_box {
                    self.paint_world_box(&mut image, rect);
                }
                for (index, (field, rect)) in self.fields.iter().zip(&layout.fields).enumerate() {
                    let shown = if field.secret {
                        "•".repeat(field.value.len())
                    } else if index == 0 {
                        self.shown_account(&field.value)
                    } else {
                        field.value.clone()
                    };
                    art::field(
                        &mut image,
                        *rect,
                        field.label,
                        &shown,
                        index == self.focus,
                        self.hovered == Some(Target::Field(index)),
                        caret,
                    );
                }
                if let Some([x, y, w, h]) = layout.saved_arrow {
                    let hot = self.hovered == Some(Target::SavedList) || self.list == Some(List::Saved);
                    art::fill(&mut image, [x, y + 6, 1, h - 12], [168, 128, 52, 160]);
                    let colour = if hot { art::GOLD_BRIGHT } else { art::GOLD };
                    art::chevron(&mut image, [x + w / 2, y + h / 2], self.list == Some(List::Saved), colour);
                }
                for (rect, check, caption) in &layout.checks {
                    let checked = match check {
                        Check::RememberAccount => self.prefs.remember_account,
                        Check::SavePassword => self.prefs.save_password,
                        Check::HideAccount => self.prefs.hide_account,
                    };
                    art::checkbox(&mut image, *rect, caption, checked, self.hovered == Some(Target::Check(*check)));
                }
            }
            Screen::CreateCharacter => {}
            Screen::Select(summary) => {
                let class = class_index(summary.class);
                let emblem = art::emblem(class, 112, true);
                art::radial_glow(&mut image, px + 96, py + 126, 84, [255, 200, 110, 70]);
                image::imageops::overlay(
                    &mut image,
                    &emblem,
                    i64::from(px + 40),
                    i64::from(py + 70),
                );
                let x = px + 180;
                art::heading(
                    &mut image,
                    &summary.name,
                    26.0,
                    x,
                    py + 74,
                    art::GOLD_BRIGHT,
                    false,
                );
                art::label_left(
                    &mut image,
                    &format!(
                        "Level {}   ·   {}",
                        summary.level,
                        class_name(summary.class)
                    ),
                    16.0,
                    x,
                    py + 118,
                    art::GOLD,
                );
                let sex = if summary.mesh % 10_000 >= 2000 {
                    "Female"
                } else {
                    "Male"
                };
                art::label_left(&mut image, sex, 14.0, x, py + 142, art::GREY);
                art::body(
                    &mut image,
                    CLASS_INFO[class].tagline,
                    12.0,
                    x,
                    py + 166,
                    art::DIM,
                    pw - 200,
                );
            }
        }

        if !self.status.is_empty() {
            let width = art::text_width(&self.status, 12.0);
            art::body(
                &mut image,
                &self.status,
                12.0,
                px + (pw - width).max(0) / 2,
                layout.status_y,
                self.status_colour,
                pw - 24,
            );
        }

        for (index, (rect, action, text)) in layout.buttons.iter().enumerate() {
            let hot = self.hovered == Some(Target::Button(*action)) && !self.busy;
            let style = if index == 0 {
                art::ButtonStyle::Primary
            } else {
                art::ButtonStyle::Secondary
            };
            art::button(
                &mut image,
                *rect,
                text,
                style,
                hot,
                (self.busy || self.updating) && *action != Action::Exit,
            );
        }
        self.paint_list(&mut image, &layout);

        // Showcase of the professions.
        let featured = (time / 3.5) as usize % 4;
        for (index, rect) in layout.showcase.iter().enumerate() {
            let lit = index == featured;
            let [x, y, w, _] = *rect;
            if lit {
                art::radial_glow(&mut image, x + w / 2, y + w / 2, w, [255, 196, 100, 70]);
            }
            image::imageops::overlay(
                &mut image,
                &art::emblem(index, w as u32, lit),
                i64::from(x),
                i64::from(y),
            );
            let colour = if lit { art::GOLD_BRIGHT } else { art::GOLD };
            art::heading(
                &mut image,
                CLASSES[index].0,
                15.0,
                x + w / 2,
                y + w + 10,
                colour,
                true,
            );
            let tagline = CLASS_INFO[index].tagline;
            let width = art::text_width(tagline, 11.0);
            art::body(
                &mut image,
                tagline,
                11.0,
                x + w / 2 - width / 2,
                y + w + 34,
                if lit { art::GREY } else { art::DIM },
                200,
            );
        }

        let footer = format!("Gravak-CO  ·  v{VERSION}");
        let width = art::text_width(&footer, 11.0);
        art::body(
            &mut image,
            &footer,
            11.0,
            size[0] as i32 - width - 14,
            size[1] as i32 - 22,
            art::DIM,
            200,
        );
        if let Some((text, colour)) = &self.update_status {
            art::body(&mut image, text, 11.0, 14, size[1] as i32 - 22, *colour, size[0] as i32 / 2);
        }
        let _ = ph;
        image
    }

    /// The world box: the chosen world and whether it answers.
    fn paint_world_box(&self, image: &mut RgbaImage, rect: [i32; 4]) {
        use crate::launcher_art as art;
        let open = self.list == Some(List::Worlds);
        let hot = self.hovered == Some(Target::WorldBox);
        art::field(image, rect, "World", &self.world().name, open, hot, false);
        let [x, y, w, h] = rect;
        art::chevron(image, [x + w - 16, y + h / 2], open, if hot || open { art::GOLD_BRIGHT } else { art::GOLD });
        self.paint_status(image, self.world_status[self.world], x + w - 34, y + h / 2);
    }

    /// A world's lamp and words, ending at `right`.
    fn paint_status(&self, image: &mut RgbaImage, status: Status, right: i32, middle: i32) {
        use crate::launcher_art as art;
        let (text, colour) = match status {
            Status::Checking => ("Checking...".to_owned(), art::DIM),
            Status::Online(ms) => (format!("Online  ·  {ms} ms"), OK),
            Status::Offline => ("Offline".to_owned(), ERROR),
        };
        let width = art::text_width(&text, 11.0);
        art::body(image, &text, 11.0, right - width, middle - 7, colour, 160);
        art::dot(image, [right - width - 10, middle], 3, colour);
    }

    /// The list dropped from the world box or the account field.
    fn paint_list(&self, image: &mut RgbaImage, layout: &Layout) {
        use crate::launcher_art as art;
        let (Some(list), Some(first), Some(last)) = (self.list, layout.list_rows.first(), layout.list_rows.last()) else {
            return;
        };
        let [x, y, w, _] = first.0;
        let height = last.0[1] + last.0[3] - y;
        let frame = [x, y, w, height];
        art::fill(image, [x + 3, y + 4, w, height], [0, 0, 0, 110]);
        art::fill(image, frame, [8, 22, 28, 246]);
        art::outline(image, frame, art::GOLD_DEEP);
        let saved = self.saved();
        for (row, index) in &layout.list_rows {
            let [rx, ry, rw, rh] = *row;
            let middle = ry + rh / 2;
            let (hot, chosen) = match list {
                List::Worlds => (self.hovered == Some(Target::World(*index)), *index == self.world),
                List::Saved => (
                    matches!(self.hovered, Some(Target::Saved(i) | Target::Forget(i)) if i == *index),
                    saved.get(*index).is_some_and(|saved| saved.account.eq_ignore_ascii_case(&self.fields[0].value)),
                ),
            };
            if hot {
                art::fill(image, [rx + 1, ry, rw - 2, rh], [120, 90, 30, 120]);
            }
            let colour = if hot || chosen { art::GOLD_BRIGHT } else { art::WHITE };
            match list {
                List::Worlds => {
                    art::body(image, &self.worlds[*index].name, 13.0, rx + 10, middle - 8, colour, rw / 2);
                    self.paint_status(image, self.world_status[*index], rx + rw - 12, middle);
                }
                List::Saved => {
                    let Some(saved) = saved.get(*index) else { continue };
                    let advance = art::body(image, &self.shown_account(&saved.account), 13.0, rx + 10, middle - 8, colour, rw - 60);
                    if saved.password.is_some() {
                        art::body(image, "·  password saved", 11.0, rx + 18 + advance, middle - 7, art::DIM, 140);
                    }
                    let forget = self.hovered == Some(Target::Forget(*index));
                    art::cross(image, [rx + rw - 15, middle], 4, if forget { ERROR } else { art::DIM });
                }
            }
        }
    }

    /// The creation screen fills the window: a tall column per profession
    /// above a bar with the name, body and buttons.
    fn creation_layout(&self, size: [u32; 2]) -> Layout {
        let [width, height] = [size[0] as i32, size[1] as i32];
        let margin = 20;
        let bar = [margin, height - 132, width - margin * 2, 112];
        let top = 102;
        let column_width = (width - margin * 2) / CLASSES.len() as i32;
        let column_height = (bar[1] - 18 - top).max(120);
        let mut targets = Vec::new();
        let mut classes = Vec::new();
        for index in 0..CLASSES.len() {
            let rect = [
                margin + index as i32 * column_width,
                top,
                column_width,
                column_height,
            ];
            classes.push(rect);
            targets.push((rect, Target::Class(index)));
        }
        // The dye swatches and the face and hair controls sit on the
        // chosen column, so they are hit first.
        if let Some(&chosen) = classes.get(self.class) {
            for (dye, [cx, cy], radius) in self.swatches(chosen).into_iter().rev() {
                let reach = radius + 3;
                targets.insert(0, ([cx - reach, cy - reach, reach * 2, reach * 2], Target::Dye(dye)));
            }
            for (rect, target) in self.look_controls(chosen).targets().into_iter().rev() {
                targets.insert(0, (rect, target));
            }
        }
        let row = bar[1] + 40;
        let name = [bar[0] + 28, row, 240, FIELD_HEIGHT];
        targets.push((name, Target::Field(0)));
        let mut bodies = Vec::new();
        let mut x = name[0] + name[2] + 24;
        for index in 0..BODIES.len() {
            let rect = [x, row, 96, FIELD_HEIGHT];
            bodies.push(rect);
            targets.push((rect, Target::Body(index)));
            x += 96 + 6;
        }
        let mut buttons = Vec::new();
        let mut right = bar[0] + bar[2] - 24;
        for (action, label) in self.buttons().into_iter().rev() {
            let width = if action == Action::Create { 124 } else { 100 };
            right -= width;
            let rect = [right, row - 1, width, BUTTON_HEIGHT];
            buttons.push((rect, action, label));
            targets.push((rect, Target::Button(action)));
            right -= 10;
        }
        buttons.reverse();
        Layout {
            panel: bar,
            fields: vec![name],
            classes,
            bodies,
            status_y: bar[1] + 84,
            buttons,
            targets,
            ..Layout::default()
        }
    }

    fn paint_creation(&self, image: &mut RgbaImage, layout: &Layout, time: f32) {
        use crate::launcher_art as art;
        let [width, _] = [image.width() as i32, image.height() as i32];
        art::gold_title(image, "Choose your Profession", 34.0, width / 2, 14);
        art::label(
            image,
            "Your profession shapes your journey through the Twin City.",
            14.0,
            width / 2,
            74,
            art::GOLD,
        );
        for (index, rect) in layout.classes.iter().enumerate() {
            let state = if index == self.class {
                art::ColumnState::Selected
            } else if self.hovered == Some(Target::Class(index)) {
                art::ColumnState::Hovered
            } else {
                art::ColumnState::Idle
            };
            let info = &CLASS_INFO[index];
            let paint = |target: &mut RgbaImage, rect: [i32; 4], time: f32| {
                let hero = self.hero_image(index, rect, time);
                art::class_column(
                    target,
                    rect,
                    art::ClassColumn {
                        class: index,
                        name: CLASSES[index].0,
                        tagline: info.tagline,
                        weapons: info.weapons,
                        description: info.description,
                        skills: info.skills,
                        stats: info.stats,
                        hero: hero.as_ref(),
                    },
                    state,
                    time,
                );
            };
            // Columns are painted on their own and reused, their light rays
            // stepping four times a second. The chosen one is cached with
            // room for its hero, who is laid over it each frame; the
            // margin keeps its gold glow.
            const MARGIN: i32 = 12;
            let chosen = index == self.class;
            let hero = chosen.then(|| self.hero_image(index, *rect, time)).flatten();
            let tick = (time * 4.0).floor();
            let key = (index, state, *rect, self.body, self.heroes.contains_key(&(self.body, index)), tick as i64);
            let mut columns = self.column_cache.borrow_mut();
            if !columns.contains_key(&key) {
                columns.retain(|cached, _| cached.0 != index);
                let [_, _, w, h] = *rect;
                let mut column = RgbaImage::new((w + MARGIN * 2).max(1) as u32, (h + MARGIN * 2).max(1) as u32);
                let local = [MARGIN, MARGIN, w, h];
                if chosen {
                    // A clear hero of the right size: the glow, the shadow
                    // and the layout around it, without the hero itself.
                    let blank = hero.as_ref().map(|hero| RgbaImage::new(hero.width(), hero.height()));
                    art::class_column(
                        &mut column,
                        local,
                        art::ClassColumn {
                            class: index,
                            name: CLASSES[index].0,
                            tagline: info.tagline,
                            weapons: info.weapons,
                            description: info.description,
                            skills: info.skills,
                            stats: info.stats,
                            hero: blank.as_ref(),
                        },
                        state,
                        tick / 4.0,
                    );
                } else {
                    paint(&mut column, local, tick / 4.0);
                }
                columns.insert(key, column);
            }
            image::imageops::overlay(
                image,
                &columns[&key],
                i64::from(rect[0] - MARGIN),
                i64::from(rect[1] - MARGIN),
            );
            if let Some(hero) = &hero {
                let [hx, hy] = art::column_hero_origin(*rect, hero.dimensions());
                image::imageops::overlay(image, hero, i64::from(hx), i64::from(hy));
                art::column_medallion(image, *rect, index, true);
                // A hint until the hero has been turned once.
                if self.turn == 0.0 && self.turning.is_none() {
                    art::label(image, "‹  drag to turn  ›", 12.0, rect[0] + rect[2] / 2, rect[1] + 22, art::GOLD);
                }
            }
            if chosen {
                let swatches = self.swatches(*rect);
                if let Some(&(_, [cx, cy], _)) = swatches.first() {
                    art::label(image, "Coat", 12.0, cx, cy - 30, art::GOLD);
                }
                for (dye, centre, radius) in swatches {
                    let colour = self.coat_colours[&(self.body, dye)];
                    let hovered = self.hovered == Some(Target::Dye(dye));
                    art::swatch(image, centre, radius, colour, dye == self.dye, hovered);
                }
                self.paint_look_controls(image, *rect);
            }
        }

        let [bx, by, bw, bh] = layout.panel;
        art::panel(image, [bx, by, bw, bh], None);
        let caret = (time * 2.0).fract() < 0.55;
        if let Some(rect) = layout.fields.first() {
            let field = &self.fields[0];
            art::field(
                image,
                *rect,
                field.label,
                &field.value,
                self.focus == 0,
                self.hovered == Some(Target::Field(0)),
                caret,
            );
        }
        if let Some(first) = layout.bodies.first() {
            art::label_left(image, "Body", 13.0, first[0] + 2, first[1] - 19, art::GOLD);
        }
        for (index, rect) in layout.bodies.iter().enumerate() {
            art::button(
                image,
                *rect,
                BODIES[index].0,
                art::ButtonStyle::Choice {
                    selected: index == self.body,
                },
                self.hovered == Some(Target::Body(index)),
                false,
            );
        }
        if !self.status.is_empty() {
            art::body(
                image,
                &self.status,
                12.0,
                bx + 30,
                layout.status_y,
                self.status_colour,
                bw - 60,
            );
        }
        for (rect, action, text) in &layout.buttons {
            let hot = self.hovered == Some(Target::Button(*action)) && !self.busy;
            let style = if *action == Action::Create {
                art::ButtonStyle::Primary
            } else {
                art::ButtonStyle::Secondary
            };
            art::button(image, *rect, text, style, hot, self.busy);
        }
    }

    /// A press on the chosen hero starts turning it; `true` when it did.
    pub fn press(&mut self, size: [u32; 2], cursor: [f32; 2]) -> bool {
        if self.screen != Screen::CreateCharacter || !self.heroes.contains_key(&(self.body, self.class)) {
            return false;
        }
        let Some(&[x, y, w, h]) = self.layout(size).classes.get(self.class) else {
            return false;
        };
        let point = [cursor[0] as i32, cursor[1] as i32];
        // The look controls on the hero are clicks, not turns.
        if !inside([x, y, w, h * 56 / 100], point) || self.target_at(size, cursor).is_some_and(Target::on_hero) {
            return false;
        }
        self.turning = Some((cursor[0], self.turn));
        true
    }

    /// Turns the hero while the button is held: a window's width is about
    /// two turns.
    pub fn drag(&mut self, cursor: [f32; 2]) -> bool {
        let Some((start, turn)) = self.turning else {
            return false;
        };
        self.turn = turn + (cursor[0] - start) * 0.012;
        true
    }

    pub fn release(&mut self) {
        self.turning = None;
    }

    /// A hero or a coat from the loading thread.
    pub fn add_loaded(&mut self, loaded: crate::showcase::Loaded) {
        match loaded {
            crate::showcase::Loaded::Hero { body, class, hero } => {
                self.heroes.insert((body, class), hero);
            }
            crate::showcase::Loaded::Coat { body, dye, hero, colour } => {
                self.coats.insert((body, dye), hero);
                self.coat_colours.insert((body, dye), colour);
            }
            crate::showcase::Loaded::Faces(faces) => self.faces.extend(faces),
            crate::showcase::Loaded::HairColours { body, colours } => {
                self.hair_swatches.insert(body, colours);
            }
            crate::showcase::Loaded::Look { key, hero } => {
                // A few dozen looks at most; the one on screen stays.
                if self.looks.len() >= 32 {
                    let shown = self.shown_look.get();
                    self.looks.retain(|look, _| Some(*look) == shown);
                }
                self.looks.insert(key, hero);
            }
        }
    }

    /// Where the face and hair controls sit on the chosen column `column`:
    /// down its left side, facing the coat swatches on the right.
    fn look_controls(&self, column: [i32; 4]) -> LookControls {
        let [x, y, _, _] = column;
        let left = x + 14;
        let size = FACE_SIZE as i32;
        let colours: Vec<(usize, [i32; 2], i32)> = self
            .hair_swatches
            .get(&self.body)
            .map(|swatches| {
                (0..swatches.len())
                    .map(|colour| {
                        let (col, row) = (colour as i32 % 2, colour as i32 / 2);
                        (colour, [left + 11 + col * 26, y + 172 + row * 24], 8)
                    })
                    .collect()
            })
            .unwrap_or_default();
        LookControls {
            centre: left + size / 2,
            face_label: y + 12,
            face: [left, y + 30, size, size],
            face_previous: [left - 2, y + 30 + size + 4, 24, 18],
            face_next: [left + size - 22, y + 30 + size + 4, 24, 18],
            hair_label: y + 112,
            style_previous: [left - 2, y + 130, 24, 18],
            style_next: [left + size - 22, y + 130, 24, 18],
            colours,
        }
    }

    fn paint_look_controls(&self, image: &mut RgbaImage, column: [i32; 4]) {
        use crate::launcher_art as art;
        let controls = self.look_controls(column);
        let hovered = |target: Target| self.hovered == Some(target);
        art::label(image, "Face", 12.0, controls.centre, controls.face_label, art::GOLD);
        let [fx, fy, fw, fh] = controls.face;
        art::fill(image, controls.face, [0, 0, 0, 150]);
        if let Some(face) = self.faces.get(&self.avatar()) {
            image::imageops::overlay(image, face, i64::from(fx), i64::from(fy));
        }
        if hovered(Target::Face(true)) {
            art::glow(image, controls.face, 4, [255, 200, 110, 90]);
        }
        art::outline(image, [fx - 1, fy - 1, fw + 2, fh + 2], art::GOLD_DEEP);
        art::outline(image, [fx - 2, fy - 2, fw + 4, fh + 4], [0, 0, 0, 140]);
        for (rect, text, target) in [
            (controls.face_previous, "‹", Target::Face(false)),
            (controls.face_next, "›", Target::Face(true)),
            (controls.style_previous, "‹", Target::HairStyle(false)),
            (controls.style_next, "›", Target::HairStyle(true)),
        ] {
            art::button(image, rect, text, art::ButtonStyle::Secondary, hovered(target), false);
        }
        let style = format!("Hair {}/{}", self.hair_style + 1, HAIR_STYLES.len());
        art::label(image, &style, 12.0, controls.centre, controls.hair_label, art::GOLD);
        if let Some(swatches) = self.hair_swatches.get(&self.body) {
            for (colour, centre, radius) in controls.colours {
                let swatch = swatches.get(colour).copied().unwrap_or([128, 128, 128]);
                let lit = hovered(Target::HairColour(colour));
                art::swatch(image, centre, radius, swatch, colour == self.hair_colour, lit);
            }
        }
    }

    /// The new character's hero in look `key`; while that look loads, the
    /// one drawn before it (or the coat's own hair).
    fn look_hero(&self, key: crate::showcase::LookKey) -> Option<&crate::showcase::Hero> {
        let get = |key: crate::showcase::LookKey| {
            if key.2 == crate::showcase::HAIR {
                self.coats.get(&(key.0, key.1))
            } else {
                self.looks.get(&key)
            }
        };
        if let Some(hero) = get(key) {
            self.shown_look.set(Some(key));
            return Some(hero);
        }
        if self.last_look_request.get() != Some(key) {
            if let Some(sender) = &self.look_requests {
                let _ = sender.send(key);
            }
            self.last_look_request.set(Some(key));
        }
        self.shown_look
            .get()
            .filter(|shown| shown.0 == key.0)
            .and_then(get)
            .or_else(|| self.coats.get(&(key.0, key.1)))
    }

    /// The dye swatches on the chosen column, top to bottom, once the
    /// body's coats have loaded: (dye, centre, radius).
    fn swatches(&self, column: [i32; 4]) -> Vec<(usize, [i32; 2], i32)> {
        let [x, y, w, _] = column;
        (0..crate::showcase::DYES.len())
            .filter(|dye| self.coat_colours.contains_key(&(self.body, *dye)))
            .map(|dye| (dye, [x + w - 30, y + 48 + dye as i32 * 28], 10))
            .collect()
    }

    /// The picture of profession `class` for its column: the chosen one
    /// breathes and turns slowly from side to side, the others stand still
    /// facing the camera.
    fn hero_image(&self, class: usize, column: [i32; 4], time: f32) -> Option<RgbaImage> {
        // The chosen profession wears the starter coat in its dye: what
        // the new character will look like. The others show their gear.
        let hero = if class == self.class {
            self.look_hero((self.body, self.dye, self.hair()))
                .or_else(|| self.heroes.get(&(self.body, class)))?
        } else {
            self.heroes.get(&(self.body, class))?
        };
        let [_, _, width, height] = column;
        let size = [(width - 28).max(1) as u32, (height * 56 / 100).max(1) as u32];
        let tall = size[1] as f32 * 0.86;
        if class == self.class {
            // Twenty poses a second are plenty for breathing and turning;
            // the player's own turn adds to the slow sway.
            let tick = (time * 20.0).floor();
            let look = (class * 16 + self.dye) * 10_000 + usize::from(self.hair()) * 10 + self.face % 10;
            let key = (self.body, look, size, tick as i64, (self.turn * 100.0) as i64);
            let mut latest = self.chosen_hero.borrow_mut();
            if let Some((cached, image)) = latest.as_ref()
                && *cached == key
            {
                return Some(image.clone());
            }
            let pose_time = tick / 20.0;
            let image = hero.render(size, tall, pose_time, (pose_time * 0.4).sin() * 0.7 + self.turn);
            *latest = Some((key, image.clone()));
            return Some(image);
        }
        let mut stills = self.still_heroes.borrow_mut();
        Some(
            stills
                .entry((self.body, class))
                .or_insert_with(|| hero.render(size, tall, 0.0, 0.0))
                .clone(),
        )
    }
}

/// The face and hair controls on the chosen hero's column.
struct LookControls {
    /// Where the labels are centred, and their tops.
    centre: i32,
    face_label: i32,
    face: [i32; 4],
    face_previous: [i32; 4],
    face_next: [i32; 4],
    hair_label: i32,
    style_previous: [i32; 4],
    style_next: [i32; 4],
    /// (colour, centre, radius), once the body's colours have loaded.
    colours: Vec<(usize, [i32; 2], i32)>,
}

impl LookControls {
    fn targets(&self) -> Vec<([i32; 4], Target)> {
        let mut targets = vec![
            (self.face, Target::Face(true)),
            (self.face_previous, Target::Face(false)),
            (self.face_next, Target::Face(true)),
            (self.style_previous, Target::HairStyle(false)),
            (self.style_next, Target::HairStyle(true)),
        ];
        for &(colour, [cx, cy], radius) in &self.colours {
            let reach = radius + 3;
            targets.push(([cx - reach, cy - reach, reach * 2, reach * 2], Target::HairColour(colour)));
        }
        targets
    }
}

#[derive(Default)]
struct Layout {
    panel: [i32; 4],
    /// The login screens' world box, the account field's saved-accounts
    /// arrow, the check boxes (box, check, caption) and the open list's
    /// rows (row, index).
    world_box: Option<[i32; 4]>,
    saved_arrow: Option<[i32; 4]>,
    checks: Vec<([i32; 4], Check, &'static str)>,
    list_rows: Vec<([i32; 4], usize)>,
    fields: Vec<[i32; 4]>,
    classes: Vec<[i32; 4]>,
    bodies: Vec<[i32; 4]>,
    status_y: i32,
    buttons: Vec<([i32; 4], Action, &'static str)>,
    targets: Vec<([i32; 4], Target)>,
    showcase: Vec<[i32; 4]>,
}

/// What the creation screen tells about each profession.
pub struct ClassInfo {
    pub tagline: &'static str,
    pub weapons: &'static str,
    pub description: &'static str,
    /// Attack, defense, range and magic, out of five.
    pub stats: [u8; 4],
    pub skills: &'static str,
}

pub const CLASS_INFO: [ClassInfo; 4] = [
    ClassInfo {
        tagline: "Master of the twin blades",
        weapons: "Blades · Swords · Clubs",
        description: "Trojans fight in the thick of battle with a weapon in each hand. Their \
            vast life and furious strikes make them the most fearsome close-range fighters \
            of the Twin City.",
        stats: [5, 3, 1, 1],
        skills: "Cyclone · Hercules · Golem",
    },
    ClassInfo {
        tagline: "Unbreakable shield of the realm",
        weapons: "Shields · Spears · Poleaxes",
        description: "Warriors stand behind heavy armour and a shield, holding the line \
            while their allies strike. No profession endures more punishment, and their \
            roar can turn the tide of a guild war.",
        stats: [4, 5, 1, 1],
        skills: "Shield · Roar · Superman",
    },
    ClassInfo {
        tagline: "Death from a distance",
        weapons: "Bows · Arrows",
        description: "Archers strike from afar and take to the skies to escape their foes. \
            Quick and precise, they rain arrows on whole groups of monsters before they \
            come close.",
        stats: [4, 2, 5, 1],
        skills: "Fly · Scatter · Rapid Fire",
    },
    ClassInfo {
        tagline: "Keeper of fire and water",
        weapons: "Backswords · Spells",
        description: "Taoists bend the elements to their will. They call down thunder and \
            fire on their enemies, and as Water Taoists they heal and revive their \
            companions.",
        stats: [2, 2, 4, 5],
        skills: "Thunder · Cure · Meditation",
    },
];

/// Index into [`CLASSES`] of a profession id, base or promoted.
pub fn class_index(class: u8) -> usize {
    match class {
        20..=25 => 1,
        40..=45 => 2,
        100..=101 | 132..=135 | 142..=145 => 3,
        _ => 0,
    }
}

pub fn valid_account(name: &str) -> bool {
    (4..=15).contains(&name.len()) && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

pub fn valid_password(password: &str) -> bool {
    (4..=15).contains(&password.len()) && password.chars().all(|c| c.is_ascii_graphic())
}

pub fn valid_character_name(name: &str) -> bool {
    (3..=15).contains(&name.len()) && name.chars().all(|c| c.is_ascii_alphanumeric())
}

fn capitalise(text: &str) -> String {
    let mut chars = text.chars();
    chars
        .next()
        .map(|first| first.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}

fn inside(rect: [i32; 4], point: [i32; 2]) -> bool {
    point[0] >= rect[0]
        && point[0] < rect[0] + rect[2]
        && point[1] >= rect[1]
        && point[1] < rect[1] + rect[3]
}

/// The screens as a winit application, until the world is entered or the
/// window closed.
struct Launcher {
    ui: LoginUi,
    client: LoginClient,
    window: Option<Arc<Window>>,
    surface: Option<softbuffer::Surface<Arc<Window>, Arc<Window>>>,
    cursor: [f32; 2],
    shift: bool,
    started: Instant,
    entered: Option<NetworkHandle>,
    error: Option<String>,
    /// Views of the game world arriving from `login_scenes`.
    scenes: std::sync::mpsc::Receiver<RgbaImage>,
    /// The professions' heroes arriving from `showcase`.
    heroes: std::sync::mpsc::Receiver<crate::showcase::Loaded>,
    /// The theme and the clicks; none without a sound device.
    audio: Option<crate::audio::LoginAudio>,
    /// How the worlds answer, from `worlds::watch`.
    world_status: std::sync::mpsc::Receiver<(usize, Status)>,
    /// The updater's progress, when the client has an update address.
    updates: Option<std::sync::mpsc::Receiver<crate::updater::UpdateEvent>>,
}

impl Launcher {
    fn size(&self) -> [u32; 2] {
        self.window
            .as_ref()
            .map(|window| {
                let size = window.inner_size();
                [size.width, size.height]
            })
            .unwrap_or([1024, 768])
    }

    /// A page turn when an input or event changed the screen.
    fn sound_screen_change(&self, before: std::mem::Discriminant<Screen>) {
        if std::mem::discriminant(&self.ui.screen) != before
            && let Some(audio) = &self.audio
        {
            audio.play(crate::audio::LoginSound::Page);
        }
    }

    fn run(&mut self, request: Option<Request>, event_loop: &ActiveEventLoop) {
        match request {
            Some(Request::Send(command)) => self.client.send(command),
            Some(Request::Exit) => event_loop.exit(),
            None => {}
        }
        self.redraw();
    }

    fn redraw(&self) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    fn present(&mut self) -> Result<(), String> {
        let size = self.size();
        let (Some(width), Some(height)) = (NonZeroU32::new(size[0]), NonZeroU32::new(size[1]))
        else {
            return Ok(());
        };
        let image = self.ui.image(size, self.started.elapsed().as_secs_f32());
        let surface = self.surface.as_mut().ok_or("no surface")?;
        surface
            .resize(width, height)
            .map_err(|error| error.to_string())?;
        let mut buffer = surface.buffer_mut().map_err(|error| error.to_string())?;
        for (target, pixel) in buffer.iter_mut().zip(image.pixels()) {
            let [r, g, b, _] = pixel.0;
            *target = (u32::from(r) << 16) | (u32::from(g) << 8) | u32::from(b);
        }
        buffer.present().map_err(|error| error.to_string())
    }
}

impl ApplicationHandler for Launcher {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attributes = Window::default_attributes()
            .with_title("Gravak")
            .with_window_icon(crate::window_icon())
            .with_inner_size(LogicalSize::new(1024.0, 768.0));
        let result = event_loop
            .create_window(attributes)
            .map_err(|error| error.to_string())
            .and_then(|window| {
                let window = Arc::new(window);
                let context =
                    softbuffer::Context::new(window.clone()).map_err(|error| error.to_string())?;
                let surface = softbuffer::Surface::new(&context, window.clone())
                    .map_err(|error| error.to_string())?;
                Ok((window, surface))
            });
        match result {
            Ok((window, surface)) => {
                self.window = Some(window);
                self.surface = Some(surface);
                self.redraw();
            }
            Err(error) => {
                self.error = Some(format!("login window unavailable: {error}"));
                event_loop.exit();
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::RedrawRequested => {
                if let Err(error) = self.present() {
                    self.error = Some(error);
                    event_loop.exit();
                }
            }
            WindowEvent::Resized(_) => self.redraw(),
            WindowEvent::ModifiersChanged(modifiers) => self.shift = modifiers.state().shift_key(),
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor = [position.x as f32, position.y as f32];
                if self.ui.drag(self.cursor) {
                    self.redraw();
                }
                let hovered = self.ui.target_at(self.size(), self.cursor);
                if hovered != self.ui.hovered {
                    self.ui.hovered = hovered;
                    self.redraw();
                }
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                if self.ui.press(self.size(), self.cursor) {
                    self.redraw();
                    return;
                }
                let target = self.ui.target_at(self.size(), self.cursor);
                if matches!(
                    target,
                    Some(
                        Target::Button(_)
                            | Target::Body(_)
                            | Target::Class(_)
                            | Target::Dye(_)
                            | Target::WorldBox
                            | Target::World(_)
                            | Target::SavedList
                            | Target::Saved(_)
                            | Target::Forget(_)
                            | Target::Check(_)
                    )
                )
                    && let Some(audio) = &self.audio
                {
                    audio.play(crate::audio::LoginSound::Click);
                }
                let before = std::mem::discriminant(&self.ui.screen);
                let request = self.ui.click(self.size(), self.cursor);
                self.sound_screen_change(before);
                self.run(request, event_loop);
            }
            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: MouseButton::Left,
                ..
            } => self.ui.release(),
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                let before = std::mem::discriminant(&self.ui.screen);
                let request = self
                    .ui
                    .key(&event.logical_key, event.text.as_deref(), self.shift);
                self.sound_screen_change(before);
                self.run(request, event_loop);
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        while let Ok(loaded) = self.heroes.try_recv() {
            self.ui.add_loaded(loaded);
        }
        if let Some(audio) = &mut self.audio {
            audio.update();
        }
        while let Ok(view) = self.scenes.try_recv() {
            crate::launcher_art::add_scene(view, self.started.elapsed().as_secs_f32());
        }
        while let Ok((index, status)) = self.world_status.try_recv() {
            self.ui.set_world_status(index, status);
        }
        let updates: Vec<_> = self.updates.as_ref().map(|updates| updates.try_iter().collect()).unwrap_or_default();
        for event in updates {
            if self.ui.update_event(&event) {
                // New files are in: the client starts again on them.
                match crate::updater::restart() {
                    Ok(()) => {
                        event_loop.exit();
                        return;
                    }
                    Err(error) => self.ui.update_status = Some((format!("Updated; start Gravak again ({error})"), OK)),
                }
            }
        }
        for event in self.client.drain() {
            let before = std::mem::discriminant(&self.ui.screen);
            self.ui.handle_event(&event);
            self.sound_screen_change(before);
            if let LoginEvent::Entered(handle) = event {
                self.entered = Some(handle);
                event_loop.exit();
                return;
            }
            self.redraw();
        }
        if let Some(prefs) = self.ui.take_prefs() {
            crate::settings::save_login_prefs(&prefs);
        }
        // Animate the embers and caret, and poll the login session.
        self.redraw();
        event_loop.set_control_flow(ControlFlow::WaitUntil(
            Instant::now() + Duration::from_millis(33),
        ));
    }
}

/// Shows the screens until a character enters the world. `Ok(None)` when
/// the window is closed or Exit is chosen.
pub fn run(
    event_loop: &mut EventLoop<()>,
    assets: &mut crate::assets::Assets,
    worlds: Vec<World>,
    update_url: Option<String>,
) -> Result<Option<(NetworkHandle, Arc<Window>)>, String> {
    // The panels and columns use the game's window art when it is there.
    match crate::original_panels::general_bg(assets) {
        Ok(source) => crate::launcher_art::set_frame_source(source),
        Err(error) => eprintln!("Login screens without the game's window art: {error}"),
    }
    let (heroes, look_requests) = crate::showcase::start(assets.root_path().to_path_buf());
    let world_status = crate::worlds::watch(&worlds);
    let mut ui = LoginUi::new(worlds, crate::settings::load_login_prefs());
    ui.set_look_requests(look_requests);
    let mut launcher = Launcher {
        ui,
        world_status,
        updates: update_url.and_then(|url| {
            let root = std::env::current_exe().ok()?.parent()?.to_path_buf();
            Some(crate::updater::start(url, root))
        }),
        client: LoginClient::start(),
        window: None,
        surface: None,
        cursor: [0.0; 2],
        shift: false,
        started: Instant::now(),
        entered: None,
        error: None,
        scenes: crate::login_scenes::start(assets.root_path().to_path_buf()),
        heroes,
        audio: {
            let options = crate::settings::load_options();
            crate::audio::LoginAudio::start(assets, options.music_volume, options.sound_volume)
        },
    };
    event_loop
        .run_app_on_demand(&mut launcher)
        .map_err(|error| error.to_string())?;
    if let Some(error) = launcher.error {
        return Err(error);
    }
    // The game goes on in the same window: the screens' surface lets go
    // of it first.
    drop(launcher.surface.take());
    Ok(launcher.entered.zip(launcher.window.take()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIZE: [u32; 2] = [1024, 768];

    fn centre(ui: &LoginUi, target: Target) -> [f32; 2] {
        let rect = ui
            .layout(SIZE)
            .targets
            .into_iter()
            .find(|(_, candidate)| *candidate == target)
            .expect("target on screen")
            .0;
        [
            rect[0] as f32 + rect[2] as f32 / 2.0,
            rect[1] as f32 + rect[3] as f32 / 2.0,
        ]
    }

    fn world() -> World {
        World::new("Gravak", "127.0.0.1:9958", "Gravak")
    }

    fn new_ui() -> LoginUi {
        LoginUi::new(vec![world()], LoginPrefs::default())
    }

    fn type_text(ui: &mut LoginUi, text: &str) {
        ui.key(&Key::Character(text.into()), Some(text), false);
    }

    #[test]
    fn login_sends_typed_credentials() {
        let mut ui = new_ui();
        type_text(&mut ui, "hero");
        ui.key(&Key::Named(NamedKey::Tab), None, false);
        type_text(&mut ui, "s3cret");
        let request = ui.click(SIZE, centre(&ui, Target::Button(Action::Login)));
        assert_eq!(
            request,
            Some(Request::Send(LoginCommand::Login {
                world: world(),
                username: "hero".into(),
                password: "s3cret".into()
            }))
        );
        assert!(ui.busy);
        // Buttons wait for the answer.
        assert_eq!(ui.activate(Action::Login), None);
        // Nothing is remembered until the server takes the login.
        assert_eq!(ui.take_prefs(), None);
    }

    fn two_worlds() -> Vec<World> {
        vec![world(), World::new("Test", "127.0.0.1:9960", "Test")]
    }

    fn character() -> CharacterSummary {
        CharacterSummary { name: "Hero".into(), level: 1, class: 10, mesh: 1003 }
    }

    #[test]
    fn an_accepted_login_is_remembered_on_its_world() {
        let mut ui = LoginUi::new(two_worlds(), LoginPrefs::default());
        ui.click(SIZE, centre(&ui, Target::WorldBox));
        ui.click(SIZE, centre(&ui, Target::World(1)));
        assert_eq!(ui.world().name, "Test");
        ui.take_prefs();
        ui.click(SIZE, centre(&ui, Target::Check(Check::SavePassword)));
        type_text(&mut ui, "hero");
        ui.key(&Key::Named(NamedKey::Tab), None, false);
        type_text(&mut ui, "s3cret");
        let Some(Request::Send(LoginCommand::Login { world, .. })) = ui.activate(Action::Login) else {
            panic!("expected a login");
        };
        assert_eq!(world.name, "Test");
        ui.handle_event(&LoginEvent::Character(character()));
        let prefs = ui.take_prefs().expect("remembered");
        assert_eq!(prefs.world.as_deref(), Some("Test"));
        assert!(prefs.remember_account && prefs.save_password);
        assert_eq!(
            prefs.accounts,
            vec![SavedAccount { world: "Test".into(), account: "hero".into(), password: Some("s3cret".into()) }]
        );

        // Next time the world, account and password are filled in.
        let mut ui = LoginUi::new(two_worlds(), prefs.clone());
        assert_eq!(ui.world().name, "Test");
        assert_eq!((ui.field_value(0), ui.field_value(1)), ("hero", "s3cret"));
        // The other world has its own accounts.
        ui.click(SIZE, centre(&ui, Target::WorldBox));
        ui.click(SIZE, centre(&ui, Target::World(0)));
        assert_eq!(ui.field_value(1), "");
        assert!(ui.target_at(SIZE, centre_or_none(&ui, Target::SavedList)).is_none());
    }

    fn centre_or_none(ui: &LoginUi, target: Target) -> [f32; 2] {
        ui.layout(SIZE)
            .targets
            .into_iter()
            .find(|(_, candidate)| *candidate == target)
            .map_or([-10.0, -10.0], |(rect, _)| [rect[0] as f32 + 1.0, rect[1] as f32 + 1.0])
    }

    #[test]
    fn saved_accounts_fill_in_and_can_be_forgotten() {
        let mut prefs = LoginPrefs { hide_account: true, ..LoginPrefs::default() };
        prefs.remember(SavedAccount { world: "Gravak".into(), account: "second".into(), password: None });
        prefs.remember(SavedAccount { world: "Gravak".into(), account: "pedro3".into(), password: Some("pw".into()) });
        let mut ui = LoginUi::new(vec![world()], prefs);
        assert_eq!((ui.field_value(0), ui.field_value(1)), ("pedro3", "pw"));
        assert_eq!(ui.shown_account(ui.field_value(0)), "ped***");
        // Editing the account drops the saved password.
        ui.click(SIZE, centre(&ui, Target::Field(0)));
        ui.key(&Key::Named(NamedKey::Backspace), None, false);
        assert_eq!(ui.field_value(1), "");
        // The list: pick the other account, then forget it.
        ui.click(SIZE, centre(&ui, Target::SavedList));
        ui.click(SIZE, centre(&ui, Target::Saved(1)));
        assert_eq!((ui.field_value(0), ui.field_value(1)), ("second", ""));
        ui.click(SIZE, centre(&ui, Target::SavedList));
        ui.click(SIZE, centre(&ui, Target::Forget(1)));
        let prefs = ui.take_prefs().expect("forgotten");
        assert_eq!(prefs.accounts.len(), 1);
        // Unticking Remember forgets the account at the next login.
        ui.click(SIZE, centre(&ui, Target::Check(Check::RememberAccount)));
        ui.click(SIZE, centre(&ui, Target::SavedList));
        ui.click(SIZE, centre(&ui, Target::Saved(0)));
        ui.key(&Key::Named(NamedKey::Tab), None, false);
        type_text(&mut ui, "pw");
        ui.activate(Action::Login);
        ui.handle_event(&LoginEvent::NeedsCharacter);
        assert!(ui.take_prefs().unwrap().accounts.is_empty());
    }

    #[test]
    fn masks_keep_the_start() {
        assert_eq!(mask("pedro3"), "ped***");
        assert_eq!(mask("abc"), "a**");
        assert_eq!(mask("a"), "a");
        assert_eq!(mask(""), "");
    }

    #[test]
    fn register_validates_before_sending() {
        let mut ui = new_ui();
        ui.click(SIZE, centre(&ui, Target::Button(Action::OpenRegister)));
        assert_eq!(ui.screen, Screen::Register);
        type_text(&mut ui, "newhero");
        ui.click(SIZE, centre(&ui, Target::Field(1)));
        type_text(&mut ui, "pass1");
        ui.click(SIZE, centre(&ui, Target::Field(2)));
        type_text(&mut ui, "pass2");
        assert_eq!(ui.key(&Key::Named(NamedKey::Enter), None, false), None);
        assert_eq!(ui.status(), "The passwords do not match.");
        ui.key(&Key::Named(NamedKey::Backspace), None, false);
        type_text(&mut ui, "1");
        assert_eq!(
            ui.key(&Key::Named(NamedKey::Enter), None, false),
            Some(Request::Send(LoginCommand::Register {
                world: world(),
                username: "newhero".into(),
                password: "pass1".into()
            }))
        );
        ui.handle_event(&LoginEvent::Registered("newhero".into()));
        assert_eq!(ui.screen, Screen::Login);
        assert_eq!(ui.field_value(0), "newhero");
    }

    #[test]
    fn character_creation_sends_the_chosen_look() {
        let mut ui = new_ui();
        ui.handle_event(&LoginEvent::NeedsCharacter);
        assert_eq!(ui.screen, Screen::CreateCharacter);
        type_text(&mut ui, "Gravak");
        ui.click(SIZE, centre(&ui, Target::Body(2)));
        ui.click(SIZE, centre(&ui, Target::Class(3)));
        assert_eq!(
            ui.click(SIZE, centre(&ui, Target::Button(Action::Create))),
            Some(Request::Send(LoginCommand::CreateCharacter {
                name: "Gravak".into(),
                mesh: 2001,
                class: 100,
                // No coat colour picked: the coat's own (dye 3); the first
                // female face and the heroes' hair.
                coat_dye: 3,
                avatar: 201,
                hair: 310,
            }))
        );
        ui.handle_event(&LoginEvent::CharacterRejected(
            "character name taken".into(),
        ));
        assert!(!ui.busy);
        assert_eq!(ui.screen, Screen::CreateCharacter);
        assert_eq!(ui.status(), "Character name taken");
    }

    #[test]
    fn character_creation_sends_the_picked_face_and_hair() {
        let mut ui = new_ui();
        ui.handle_event(&LoginEvent::NeedsCharacter);
        type_text(&mut ui, "Gravak");
        ui.add_loaded(crate::showcase::Loaded::HairColours { body: 0, colours: vec![[0, 0, 0]; 7] });
        // The first face back wraps to the last male one.
        ui.click(SIZE, centre(&ui, Target::Face(false)));
        assert_eq!(ui.avatar(), 102);
        ui.click(SIZE, centre(&ui, Target::Face(true)));
        ui.click(SIZE, centre(&ui, Target::Face(true)));
        assert_eq!(ui.avatar(), 2);
        ui.click(SIZE, centre(&ui, Target::HairStyle(true)));
        ui.click(SIZE, centre(&ui, Target::HairColour(4)));
        assert_eq!(ui.hair(), 711);
        // The look controls are clicks, not turns of the hero.
        assert!(!ui.press(SIZE, centre(&ui, Target::HairColour(4))));
        assert_eq!(
            ui.click(SIZE, centre(&ui, Target::Button(Action::Create))),
            Some(Request::Send(LoginCommand::CreateCharacter {
                name: "Gravak".into(),
                mesh: 1003,
                class: 10,
                coat_dye: 3,
                avatar: 2,
                hair: 711,
            }))
        );
        // A female body keeps the place among its own faces.
        ui.click(SIZE, centre(&ui, Target::Body(2)));
        assert_eq!(ui.avatar(), 202);
    }

    #[test]
    fn selection_enters_or_logs_out() {
        let mut ui = new_ui();
        ui.handle_event(&LoginEvent::Character(CharacterSummary {
            name: "Gravak".into(),
            level: 3,
            class: 10,
            mesh: 11003,
        }));
        assert!(matches!(ui.screen, Screen::Select(_)));
        assert_eq!(
            ui.key(&Key::Named(NamedKey::Escape), None, false),
            Some(Request::Send(LoginCommand::Logout))
        );
        assert_eq!(ui.screen, Screen::Login);
    }
}

#[cfg(test)]
mod preview {
    //! Paints the login screen in its states to PNGs, to look at.
    use super::*;

    /// `cargo test -p conquer-sandbox login_preview -- --ignored`; the
    /// images land in GRAVAK_PREVIEW_OUT (default `login-preview`).
    #[test]
    #[ignore = "Needs the 5065 installation"]
    fn login_preview() {
        let root = std::path::PathBuf::from(std::env::var("GRAVAK_CLIENT").unwrap_or_else(|_| "R:/5065".into()));
        let out = std::path::PathBuf::from(std::env::var("GRAVAK_PREVIEW_OUT").unwrap_or_else(|_| "login-preview".into()));
        std::fs::create_dir_all(&out).unwrap();
        let mut assets = crate::assets::Assets::open(&root).unwrap();
        crate::launcher_art::set_frame_source(crate::original_panels::general_bg(&mut assets).unwrap());
        let worlds = vec![
            World::new("Gravak", "127.0.0.1:9958", "Gravak"),
            World::new("Gravak PvP", "127.0.0.1:9960", "PvP"),
            World::new("Test Realm", "127.0.0.1:9962", "Test"),
        ];
        let mut prefs = LoginPrefs { save_password: true, hide_account: true, ..LoginPrefs::default() };
        prefs.remember(SavedAccount { world: "Gravak".into(), account: "gravak".into(), password: None });
        prefs.remember(SavedAccount { world: "Gravak".into(), account: "pedro3".into(), password: Some("123456".into()) });
        let size = [1024, 768];
        let shot = |ui: &LoginUi, name: &str| ui.paint(size, 4.0).save(out.join(name)).unwrap();
        let mut ui = LoginUi::new(worlds.clone(), prefs.clone());
        ui.set_world_status(0, Status::Online(12));
        ui.set_world_status(1, Status::Offline);
        shot(&ui, "1-login.png");
        let point = |ui: &LoginUi, target| {
            let rect = ui.layout(size).targets.into_iter().find(|(_, t)| *t == target).unwrap().0;
            [rect[0] as f32 + rect[2] as f32 / 2.0, rect[1] as f32 + rect[3] as f32 / 2.0]
        };
        ui.click(size, point(&ui, Target::WorldBox));
        ui.hovered = Some(Target::World(1));
        shot(&ui, "2-worlds.png");
        ui.click(size, [5.0, 5.0]);
        ui.click(size, point(&ui, Target::SavedList));
        ui.hovered = Some(Target::Forget(1));
        shot(&ui, "3-saved.png");
        ui.click(size, [5.0, 5.0]);
        ui.click(size, point(&ui, Target::Check(Check::HideAccount)));
        ui.update_status = Some(("Downloading update 0.1.1  ·  42%  (12.3 of 29.0 MB)".into(), art_gold()));
        ui.updating = true;
        shot(&ui, "4-updating.png");
        ui.activate(Action::OpenRegister);
        shot(&ui, "5-register.png");
    }

    fn art_gold() -> [u8; 4] {
        crate::launcher_art::GOLD
    }
}

#[cfg(test)]
mod art_probe {
    //! Finds the original client's login and character-creation artwork so
    //! the screens can use it.

    const KEYWORDS: [&str; 16] = [
        "login",
        "logon",
        "regist",
        "role",
        "select",
        "create",
        "newrole",
        "prof",
        "taoist",
        "trojan",
        "archer",
        "warrior",
        "choose",
        "entry",
        "serverlist",
        "sex",
    ];

    fn interesting(text: &str) -> bool {
        let lower = text.to_ascii_lowercase();
        KEYWORDS.iter().any(|keyword| lower.contains(keyword))
    }

    fn walk(dir: &std::path::Path, root: &std::path::Path, out: &mut Vec<String>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, root, out);
            } else if let Ok(relative) = path.strip_prefix(root) {
                out.push(relative.to_string_lossy().replace('\\', "/"));
            }
        }
    }

    /// Lists loose files and ani/ini entries that look like login or
    /// character-creation art, and exports each image it can read as PNG.
    /// GRAVAK_CLIENT (default R:/5065) is the installation and
    /// GRAVAK_PROBE_OUT (default ./login-art-probe) the output folder.
    #[test]
    #[ignore = "Needs the 5065 installation"]
    fn probe_login_art() -> crate::assets::Result<()> {
        let root = std::path::PathBuf::from(
            std::env::var("GRAVAK_CLIENT").unwrap_or_else(|_| "R:/5065".into()),
        );
        let out = std::path::PathBuf::from(
            std::env::var("GRAVAK_PROBE_OUT").unwrap_or_else(|_| "login-art-probe".into()),
        );
        std::fs::create_dir_all(&out)?;
        let mut assets = crate::assets::Assets::open(&root)?;
        let mut report = Vec::new();
        let mut candidates = Vec::new();

        let mut files = Vec::new();
        walk(&root, &root, &mut files);
        files.sort();
        report.push(format!(
            "# {} loose files under {}",
            files.len(),
            root.display()
        ));
        for file in &files {
            let lower = file.to_ascii_lowercase();
            let image = [".dds", ".jpg", ".jpeg", ".bmp", ".tga", ".png"]
                .iter()
                .any(|ext| lower.ends_with(ext));
            if image && interesting(file) {
                report.push(format!("loose image: {file}"));
                candidates.push(file.clone());
            }
        }

        // Config files, loose or in the archives.
        let mut configs: Vec<String> = files
            .iter()
            .filter(|file| {
                let lower = file.to_ascii_lowercase();
                lower.ends_with(".ani") || lower.ends_with(".ini")
            })
            .cloned()
            .collect();
        for known in [
            "ani/Control.Ani",
            "ani/Common.Ani",
            "ani/Login.Ani",
            "ani/Register.Ani",
            "ini/GUI.ini",
            "ini/info.ini",
        ] {
            if !configs.iter().any(|file| file.eq_ignore_ascii_case(known)) {
                configs.push(known.to_owned());
            }
        }
        for config in &configs {
            let Ok(bytes) = assets.read(config) else {
                continue;
            };
            let text = String::from_utf8_lossy(&bytes);
            let mut section = String::new();
            let mut hits = 0;
            for line in text.lines().map(str::trim) {
                if let Some(name) = line
                    .strip_prefix('[')
                    .and_then(|line| line.strip_suffix(']'))
                {
                    section = name.to_owned();
                    continue;
                }
                if !(interesting(&section) || interesting(line)) {
                    continue;
                }
                hits += 1;
                if hits <= 400 {
                    report.push(format!("{config} [{section}] {line}"));
                }
                if let Some((_, value)) = line.split_once('=') {
                    let value = value.trim().replace('\\', "/");
                    let lower = value.to_ascii_lowercase();
                    if [".dds", ".jpg", ".jpeg", ".bmp", ".tga"]
                        .iter()
                        .any(|ext| lower.ends_with(ext))
                    {
                        candidates.push(value);
                    }
                }
            }
            if hits > 0 {
                report.push(format!("# {config}: {hits} matching lines"));
            }
        }

        candidates.sort();
        candidates.dedup();
        for (index, path) in candidates.iter().enumerate() {
            match assets.image(path) {
                Ok(image) => {
                    let name = format!("{index:03}_{}.png", path.replace(['/', '\\', ':'], "_"));
                    report.push(format!(
                        "exported {path} {}x{} -> {name}",
                        image.width(),
                        image.height()
                    ));
                    image.save(out.join(name))?;
                }
                Err(error) => report.push(format!("unreadable {path}: {error}")),
            }
        }
        std::fs::write(out.join("report.txt"), report.join("\n"))?;
        println!(
            "{} candidates; report in {}",
            candidates.len(),
            out.join("report.txt").display()
        );
        Ok(())
    }
}
