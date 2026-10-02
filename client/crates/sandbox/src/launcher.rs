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
const CLASS_COLUMN: i32 = 300;
const CLASS_CARD: i32 = 74;
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
        let [width, height] = [size[0] as i32, size[1] as i32];
        let mut targets = Vec::new();
        let mut fields = Vec::new();
        let mut classes = Vec::new();
        let mut bodies = Vec::new();
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
            Screen::CreateCharacter => {
                panel_width = (width - 40).clamp(640, 880);
                let left = 28;
                for index in 0..CLASSES.len() {
                    let rect = [
                        left,
                        86 + index as i32 * (CLASS_CARD + 10),
                        CLASS_COLUMN,
                        CLASS_CARD,
                    ];
                    classes.push(rect);
                    targets.push((rect, Target::Class(index)));
                }
                y = 86 + 4 * (CLASS_CARD + 10) + 30;
                let name = [left, y, CLASS_COLUMN, FIELD_HEIGHT];
                fields.push(name);
                targets.push((name, Target::Field(0)));
                let right = left + CLASS_COLUMN + 28;
                let column = panel_width - right - 28;
                let gap = 8;
                let choice = (column - gap * 3) / 4;
                for index in 0..BODIES.len() {
                    let rect = [
                        right + index as i32 * (choice + gap),
                        y,
                        choice,
                        FIELD_HEIGHT,
                    ];
                    bodies.push(rect);
                    targets.push((rect, Target::Body(index)));
                }
                y += FIELD_HEIGHT + 14;
            }
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
        art::embers(&mut image, time);
        let layout = self.layout(size);
        let centre = size[0] as i32 / 2;
        let [px, py, pw, ph] = layout.panel;
        let caret = (time * 2.0).fract() < 0.55;

        // Title.
        let compact = self.screen == Screen::CreateCharacter;
        let (title_size, title_y) = if compact {
            (44.0, py - 108)
        } else {
            (68.0, py - 172)
        };
        art::gold_title(&mut image, "Gravak", title_size, centre, title_y);
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
            Screen::CreateCharacter => self.paint_creation(&mut image, &layout, caret),
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

    fn paint_creation(&self, image: &mut RgbaImage, layout: &Layout, caret: bool) {
        use crate::launcher_art as art;
        let [px, py, pw, _] = layout.panel;
        art::label_left(
            image,
            "Choose your profession",
            14.0,
            px + 30,
            py + 60,
            art::GOLD,
        );
        for (index, rect) in layout.classes.iter().enumerate() {
            let selected = index == self.class;
            let hot = self.hovered == Some(Target::Class(index));
            let [x, y, w, h] = *rect;
            if selected {
                art::glow(image, *rect, 6, [255, 200, 110, 110]);
                art::gradient(image, *rect, [88, 66, 28, 255], [40, 30, 14, 255]);
            } else {
                art::gradient(
                    image,
                    *rect,
                    if hot {
                        [34, 70, 82, 255]
                    } else {
                        [16, 38, 46, 255]
                    },
                    if hot {
                        [14, 34, 42, 255]
                    } else {
                        [6, 18, 24, 255]
                    },
                );
            }
            art::outline(
                image,
                *rect,
                if selected {
                    art::GOLD_BRIGHT
                } else if hot {
                    art::GOLD
                } else {
                    [70, 80, 64, 255]
                },
            );
            let emblem = art::emblem(index, (h - 16) as u32, selected);
            image::imageops::overlay(image, &emblem, i64::from(x + 8), i64::from(y + 8));
            let text_x = x + h + 6;
            art::heading(
                image,
                CLASSES[index].0,
                17.0,
                text_x,
                y + 14,
                if selected {
                    art::GOLD_BRIGHT
                } else {
                    art::GOLD
                },
                false,
            );
            art::body(
                image,
                CLASS_INFO[index].tagline,
                12.0,
                text_x,
                y + 42,
                art::GREY,
                w - h - 12,
            );
        }

        // The selected profession in detail.
        let info = &CLASS_INFO[self.class];
        let x = px + 28 + CLASS_COLUMN + 28;
        let column = pw - (x - px) - 28;
        let top = py + 64;
        let emblem = art::emblem(self.class, 104, true);
        art::radial_glow(image, x + 52, top + 58, 80, [255, 200, 110, 60]);
        image::imageops::overlay(image, &emblem, i64::from(x), i64::from(top + 6));
        art::heading(
            image,
            CLASSES[self.class].0,
            30.0,
            x + 124,
            top + 18,
            art::GOLD_BRIGHT,
            false,
        );
        art::label_left(image, info.tagline, 15.0, x + 126, top + 62, art::GOLD);
        art::label_left(image, info.weapons, 13.0, x + 126, top + 84, art::GREY);
        art::ornament_rule(image, x + column / 2, top + 124, column - 40);
        let mut line_y = top + 138;
        for line in art::wrap(info.description, 13.0, column) {
            art::body(image, &line, 13.0, x, line_y, art::WHITE, column);
            line_y += 18;
        }
        line_y += 10;
        for (caption, value) in ["Attack", "Defense", "Range", "Magic"]
            .into_iter()
            .zip(info.stats)
        {
            art::stat_bar(image, x, line_y, caption, value);
            line_y += 20;
        }
        line_y += 6;
        art::label_left(image, "Signature skills", 13.0, x, line_y, art::GOLD);
        art::body(
            image,
            info.skills,
            12.0,
            x + 112,
            line_y + 1,
            art::GREY,
            column - 112,
        );

        // Name and body.
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
            .with_title("Gravak - Login")
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
    auth_address: String,
    realm: String,
) -> Result<Option<NetworkHandle>, String> {
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
