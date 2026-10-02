//! Login, account registration, character creation and character
//! selection, shown before the game window.
//!
//! The screens are painted by the client (see [`crate::launcher_art`]) into
//! one image that is presented through `softbuffer`; the same layout drives
//! painting and hit-testing.

use crate::login::{CharacterSummary, LoginClient, LoginCommand, LoginEvent};
use crate::network::NetworkHandle;
use image::RgbaImage;
use std::{
    num::NonZeroU32,
    rc::Rc,
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
const BUTTON_HEIGHT: i32 = 34;
const MAX_INPUT: usize = 15;

const GREY: [u8; 4] = [150, 164, 166, 255];
const ERROR: [u8; 4] = [240, 128, 108, 255];
const OK: [u8; 4] = [136, 220, 146, 255];

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
    /// The professions' heroes by (body, class), as they finish loading.
    heroes: std::collections::HashMap<(usize, usize), crate::showcase::Hero>,
    /// The columns not chosen, painted once per (class, state, place, body,
    /// hero loaded, light tick).
    column_cache: std::cell::RefCell<
        std::collections::HashMap<(usize, crate::launcher_art::ColumnState, [i32; 4], usize, bool, i64), RgbaImage>,
    >,
    /// The chosen hero's latest pose, by (body, class, size, tick).
    chosen_hero: std::cell::RefCell<Option<((usize, usize, [u32; 2], i64), RgbaImage)>>,
    /// Still renders of the heroes not selected, by (body, class).
    still_heroes: std::cell::RefCell<std::collections::HashMap<(usize, usize), RgbaImage>>,
}

/// What the screens ask the login session to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Request {
    Send(LoginCommand),
    Exit,
}

impl LoginUi {
    pub fn new(last_account: Option<String>) -> Self {
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
            account: last_account.unwrap_or_default(),
            heroes: std::collections::HashMap::new(),
            still_heroes: std::cell::RefCell::new(std::collections::HashMap::new()),
            column_cache: std::cell::RefCell::new(std::collections::HashMap::new()),
            chosen_hero: std::cell::RefCell::new(None),
        };
        ui.show(Screen::Login);
        ui
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
        if matches!(screen, Screen::Login | Screen::Register) && !self.account.is_empty() {
            self.fields[0].value = self.account.clone();
            self.focus = 1;
        }
        self.screen = screen;
        self.hovered = None;
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
                self.busy = false;
                self.show(Screen::CreateCharacter);
                self.info("Create your character.");
            }
            LoginEvent::Character(summary) => {
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
        if self.busy && action != Action::Exit {
            return None;
        }
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
                crate::settings::save_last_account(&username);
                self.busy = true;
                self.info("Logging in...");
                Some(Request::Send(LoginCommand::Login { username, password }))
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
                Some(Request::Send(LoginCommand::Register { username, password }))
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
        match self.target_at(size, cursor)? {
            Target::Field(index) => {
                self.focus = index;
                None
            }
            Target::Body(index) => {
                self.body = index;
                None
            }
            Target::Class(index) => {
                self.class = index;
                None
            }
            Target::Button(action) => self.activate(action),
        }
    }

    pub fn key(&mut self, key: &Key, text: Option<&str>, shift: bool) -> Option<Request> {
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
        let panel_width;
        let mut y;
        match self.screen {
            Screen::Login | Screen::Register => {
                panel_width = 420;
                y = 66;
                for index in 0..self.fields.len() {
                    y += 20;
                    let rect = [36, y, panel_width - 72, FIELD_HEIGHT];
                    fields.push(rect);
                    targets.push((rect, Target::Field(index)));
                    y += FIELD_HEIGHT + 14;
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
    pub fn image(&self, size: [u32; 2], time: f32) -> RgbaImage {
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
                for (index, (field, rect)) in self.fields.iter().zip(&layout.fields).enumerate() {
                    let shown = if field.secret {
                        "•".repeat(field.value.len())
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
                self.busy && *action != Action::Exit,
            );
        }

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

        let footer = "Gravak-CO";
        let width = art::text_width(footer, 11.0);
        art::body(
            &mut image,
            footer,
            11.0,
            size[0] as i32 - width - 14,
            size[1] as i32 - 22,
            art::DIM,
            200,
        );
        let _ = ph;
        image
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
            showcase: Vec::new(),
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

    /// A profession's hero, loaded for one of the `BODIES`.
    pub fn add_hero(&mut self, body: usize, class: usize, hero: crate::showcase::Hero) {
        self.heroes.insert((body, class), hero);
    }

    /// The picture of profession `class` for its column: the chosen one
    /// breathes and turns slowly from side to side, the others stand still
    /// facing the camera.
    fn hero_image(&self, class: usize, column: [i32; 4], time: f32) -> Option<RgbaImage> {
        let hero = self.heroes.get(&(self.body, class))?;
        let [_, _, width, height] = column;
        let size = [(width - 28).max(1) as u32, (height * 56 / 100).max(1) as u32];
        let tall = size[1] as f32 * 0.86;
        if class == self.class {
            // Twenty poses a second are plenty for breathing and turning.
            let tick = (time * 20.0).floor();
            let mut latest = self.chosen_hero.borrow_mut();
            if let Some((key, image)) = latest.as_ref()
                && *key == (self.body, class, size, tick as i64)
            {
                return Some(image.clone());
            }
            let pose_time = tick / 20.0;
            let image = hero.render(size, tall, pose_time, (pose_time * 0.4).sin() * 0.7);
            *latest = Some(((self.body, class, size, tick as i64), image.clone()));
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

struct Layout {
    panel: [i32; 4],
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
    window: Option<Rc<Window>>,
    surface: Option<softbuffer::Surface<Rc<Window>, Rc<Window>>>,
    cursor: [f32; 2],
    shift: bool,
    started: Instant,
    entered: Option<NetworkHandle>,
    error: Option<String>,
    /// Views of the game world arriving from `login_scenes`.
    scenes: std::sync::mpsc::Receiver<RgbaImage>,
    /// The professions' heroes arriving from `showcase`.
    heroes: std::sync::mpsc::Receiver<(usize, usize, crate::showcase::Hero)>,
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
                let window = Rc::new(window);
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
                let request = self.ui.click(self.size(), self.cursor);
                self.run(request, event_loop);
            }
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                let request = self
                    .ui
                    .key(&event.logical_key, event.text.as_deref(), self.shift);
                self.run(request, event_loop);
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        while let Ok((body, class, hero)) = self.heroes.try_recv() {
            self.ui.add_hero(body, class, hero);
        }
        while let Ok(view) = self.scenes.try_recv() {
            crate::launcher_art::add_scene(view, self.started.elapsed().as_secs_f32());
        }
        for event in self.client.drain() {
            self.ui.handle_event(&event);
            if let LoginEvent::Entered(handle) = event {
                self.entered = Some(handle);
                event_loop.exit();
                return;
            }
            self.redraw();
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
    auth_address: String,
    realm: String,
) -> Result<Option<NetworkHandle>, String> {
    // The panels and columns use the game's window art when it is there.
    match crate::original_panels::general_bg(assets) {
        Ok(source) => crate::launcher_art::set_frame_source(source),
        Err(error) => eprintln!("Login screens without the game's window art: {error}"),
    }
    let mut launcher = Launcher {
        ui: LoginUi::new(crate::settings::load_last_account()),
        client: LoginClient::start(auth_address, realm),
        window: None,
        surface: None,
        cursor: [0.0; 2],
        shift: false,
        started: Instant::now(),
        entered: None,
        error: None,
        scenes: crate::login_scenes::start(assets.root_path().to_path_buf()),
        heroes: crate::showcase::start(assets.root_path().to_path_buf()),
    };
    event_loop
        .run_app_on_demand(&mut launcher)
        .map_err(|error| error.to_string())?;
    if let Some(error) = launcher.error {
        return Err(error);
    }
    Ok(launcher.entered)
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

    fn type_text(ui: &mut LoginUi, text: &str) {
        ui.key(&Key::Character(text.into()), Some(text), false);
    }

    #[test]
    fn login_sends_typed_credentials() {
        let mut ui = LoginUi::new(None);
        type_text(&mut ui, "hero");
        ui.key(&Key::Named(NamedKey::Tab), None, false);
        type_text(&mut ui, "s3cret");
        let request = ui.click(SIZE, centre(&ui, Target::Button(Action::Login)));
        assert_eq!(
            request,
            Some(Request::Send(LoginCommand::Login {
                username: "hero".into(),
                password: "s3cret".into()
            }))
        );
        assert!(ui.busy);
        // Buttons wait for the answer.
        assert_eq!(ui.activate(Action::Login), None);
    }

    #[test]
    fn register_validates_before_sending() {
        let mut ui = LoginUi::new(None);
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
        let mut ui = LoginUi::new(Some("hero".into()));
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
                class: 100
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
    fn selection_enters_or_logs_out() {
        let mut ui = LoginUi::new(None);
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
