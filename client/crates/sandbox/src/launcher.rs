//! Login, account registration, character creation and character
//! selection, shown before the game window.
//!
//! The screens are painted by the client (dark teal panels with the gold
//! rules of the game windows) into one image that is presented through
//! `softbuffer`; the same layout drives painting and hit-testing.

use crate::login::{CharacterSummary, LoginClient, LoginCommand, LoginEvent};
use crate::network::NetworkHandle;
use image::{Pixel, Rgba, RgbaImage};
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

const PANEL_WIDTH: i32 = 380;
const FIELD_HEIGHT: i32 = 26;
const BUTTON_HEIGHT: i32 = 28;
const MAX_INPUT: usize = 15;

const BACKGROUND_TOP: [u8; 3] = [10, 24, 32];
const BACKGROUND_BOTTOM: [u8; 3] = [24, 52, 62];
const PANEL: [u8; 4] = [18, 44, 54, 235];
const PANEL_BAND: [u8; 4] = [28, 66, 80, 255];
const GOLD: [u8; 4] = [232, 206, 148, 255];
const GOLD_BRIGHT: [u8; 4] = [255, 232, 170, 255];
const GOLD_RULE: [u8; 4] = [196, 166, 96, 255];
const TEAL_RULE: [u8; 4] = [92, 132, 140, 255];
const WHITE: [u8; 4] = [240, 240, 236, 255];
const GREY: [u8; 4] = [150, 160, 162, 255];
const ERROR: [u8; 4] = [236, 120, 104, 255];
const OK: [u8; 4] = [130, 214, 140, 255];

pub const BODIES: [(&str, u16); 4] = [
    ("Male", 1003),
    ("Male (large)", 1004),
    ("Female", 2001),
    ("Female (large)", 2002),
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
            Screen::Select(_) => "Character",
        }
    }

    fn buttons(&self) -> Vec<(Action, &'static str)> {
        match self.screen {
            Screen::Login => vec![
                (Action::Login, "Login"),
                (Action::OpenRegister, "Register"),
                (Action::Exit, "Exit"),
            ],
            Screen::Register => vec![(Action::Register, "Create"), (Action::Back, "Back")],
            Screen::CreateCharacter => vec![(Action::Create, "Create"), (Action::Logout, "Back")],
            Screen::Select(_) => vec![(Action::Enter, "Enter"), (Action::Logout, "Back")],
        }
    }

    fn layout(&self, size: [u32; 2]) -> Layout {
        let mut targets = Vec::new();
        let mut y = 52;
        let mut fields = Vec::new();
        for index in 0..self.fields.len() {
            let rect = [24, y + 18, PANEL_WIDTH - 48, FIELD_HEIGHT];
            fields.push(rect);
            targets.push((rect, Target::Field(index)));
            y += 18 + FIELD_HEIGHT + 12;
        }
        let mut choices = Vec::new();
        if self.screen == Screen::CreateCharacter {
            for (row, count, kind) in [(0, BODIES.len(), 0), (1, CLASSES.len(), 1)] {
                y += 18;
                let width = (PANEL_WIDTH - 48 - 6 * (count as i32 - 1)) / count as i32;
                for index in 0..count {
                    let rect = [24 + index as i32 * (width + 6), y, width, 26];
                    choices.push((row, index, rect));
                    targets.push((
                        rect,
                        if kind == 0 {
                            Target::Body(index)
                        } else {
                            Target::Class(index)
                        },
                    ));
                }
                y += 26 + 12;
            }
        }
        if let Screen::Select(_) = self.screen {
            y += 96;
        }
        y += 8;
        let status_y = y;
        y += 26;
        let buttons = self.buttons();
        let count = buttons.len() as i32;
        let width = ((PANEL_WIDTH - 48 - 12 * (count - 1)) / count).min(110);
        let total = width * count + 12 * (count - 1);
        let mut button_rects = Vec::new();
        for (index, (action, label)) in buttons.into_iter().enumerate() {
            let rect = [
                (PANEL_WIDTH - total) / 2 + index as i32 * (width + 12),
                y,
                width,
                BUTTON_HEIGHT,
            ];
            button_rects.push((rect, action, label));
            targets.push((rect, Target::Button(action)));
        }
        let height = y + BUTTON_HEIGHT + 22;
        let origin = [
            ((size[0] as i32 - PANEL_WIDTH) / 2).max(0),
            ((size[1] as i32 - height) / 2 + 40).max(0),
        ];
        let offset = |rect: [i32; 4]| [rect[0] + origin[0], rect[1] + origin[1], rect[2], rect[3]];
        Layout {
            panel: [origin[0], origin[1], PANEL_WIDTH, height],
            fields: fields.into_iter().map(offset).collect(),
            choices: choices
                .into_iter()
                .map(|(row, index, rect)| (row, index, offset(rect)))
                .collect(),
            status_y: origin[1] + status_y,
            buttons: button_rects
                .into_iter()
                .map(|(rect, action, label)| (offset(rect), action, label))
                .collect(),
            targets: targets
                .into_iter()
                .map(|(rect, target)| (offset(rect), target))
                .collect(),
        }
    }

    /// Paints the whole window.
    pub fn image(&self, size: [u32; 2], time: f32) -> RgbaImage {
        let [width, height] = [size[0].max(1), size[1].max(1)];
        let mut image = RgbaImage::new(width, height);
        for y in 0..height {
            let t = y as f32 / height as f32;
            let colour = Rgba([
                lerp(BACKGROUND_TOP[0], BACKGROUND_BOTTOM[0], t),
                lerp(BACKGROUND_TOP[1], BACKGROUND_BOTTOM[1], t),
                lerp(BACKGROUND_TOP[2], BACKGROUND_BOTTOM[2], t),
                255,
            ]);
            for x in 0..width {
                image.put_pixel(x, y, colour);
            }
        }
        let layout = self.layout(size);
        let centre = width as i32 / 2;
        decorative(
            &mut image,
            "Gravak",
            56.0,
            centre,
            layout.panel[1] - 110,
            GOLD_BRIGHT,
        );
        label(
            &mut image,
            "Conquer Online 5065",
            15.0,
            centre,
            layout.panel[1] - 40,
            GOLD,
        );

        let [px, py, pw, ph] = layout.panel;
        fill(&mut image, layout.panel, PANEL);
        fill(&mut image, [px + 3, py + 3, pw - 6, 34], PANEL_BAND);
        outline(
            &mut image,
            [px - 1, py - 1, pw + 2, ph + 2],
            [6, 14, 18, 255],
        );
        outline(&mut image, layout.panel, GOLD_RULE);
        outline(&mut image, [px + 3, py + 3, pw - 6, ph - 6], TEAL_RULE);
        fill(&mut image, [px + 4, py + 37, pw - 8, 1], TEAL_RULE);
        label(&mut image, self.title(), 18.0, px + pw / 2, py + 10, GOLD);

        for (index, (field, rect)) in self.fields.iter().zip(&layout.fields).enumerate() {
            let [x, y, w, h] = *rect;
            body_text(&mut image, field.label, x, y - 18, GOLD, w);
            let focused = index == self.focus;
            let hot = self.hovered == Some(Target::Field(index));
            fill(&mut image, *rect, [8, 20, 26, 255]);
            outline(
                &mut image,
                *rect,
                if focused {
                    GOLD_BRIGHT
                } else if hot {
                    GOLD_RULE
                } else {
                    [70, 100, 108, 255]
                },
            );
            let shown = if field.secret {
                "*".repeat(field.value.len())
            } else {
                field.value.clone()
            };
            let advance = body_text(&mut image, &shown, x + 8, y + 6, WHITE, w - 16);
            if focused && (time * 2.0).fract() < 0.5 {
                fill(&mut image, [x + 9 + advance, y + 6, 1, h - 12], WHITE);
            }
        }

        if self.screen == Screen::CreateCharacter {
            let mut rows = [i32::MAX; 2];
            for (row, index, rect) in &layout.choices {
                rows[*row] = rows[*row].min(rect[1]);
                let (text, selected, target) = if *row == 0 {
                    (BODIES[*index].0, *index == self.body, Target::Body(*index))
                } else {
                    (
                        CLASSES[*index].0,
                        *index == self.class,
                        Target::Class(*index),
                    )
                };
                button(
                    &mut image,
                    *rect,
                    text,
                    selected,
                    self.hovered == Some(target),
                    12.0,
                );
            }
            body_text(&mut image, "Body", px + 24, rows[0] - 18, GOLD, 200);
            body_text(&mut image, "Profession", px + 24, rows[1] - 18, GOLD, 200);
        }

        if let Screen::Select(summary) = &self.screen {
            let card = [px + 24, py + 56, pw - 48, 88];
            fill(&mut image, card, [8, 20, 26, 255]);
            outline(&mut image, card, GOLD_RULE);
            label(
                &mut image,
                &summary.name,
                20.0,
                px + pw / 2,
                card[1] + 12,
                GOLD_BRIGHT,
            );
            label(
                &mut image,
                &format!("Level {}  ·  {}", summary.level, class_name(summary.class)),
                14.0,
                px + pw / 2,
                card[1] + 44,
                GOLD,
            );
            let sex = if summary.mesh % 10_000 >= 2000 {
                "Female"
            } else {
                "Male"
            };
            label(&mut image, sex, 13.0, px + pw / 2, card[1] + 64, GREY);
        }

        if !self.status.is_empty() {
            let width = text_width(&self.status, 12.0);
            body_text(
                &mut image,
                &self.status,
                px + (pw - width).max(0) / 2,
                layout.status_y,
                self.status_colour,
                pw - 24,
            );
        }

        for (rect, action, text) in &layout.buttons {
            let hot = self.hovered == Some(Target::Button(*action)) && !self.busy;
            button(&mut image, *rect, text, false, hot, 15.0);
        }
        image
    }
}

struct Layout {
    panel: [i32; 4],
    fields: Vec<[i32; 4]>,
    choices: Vec<(usize, usize, [i32; 4])>,
    status_y: i32,
    buttons: Vec<([i32; 4], Action, &'static str)>,
    targets: Vec<([i32; 4], Target)>,
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

fn lerp(a: u8, b: u8, t: f32) -> u8 {
    (f32::from(a) + (f32::from(b) - f32::from(a)) * t).round() as u8
}

fn inside(rect: [i32; 4], point: [i32; 2]) -> bool {
    point[0] >= rect[0]
        && point[0] < rect[0] + rect[2]
        && point[1] >= rect[1]
        && point[1] < rect[1] + rect[3]
}

fn fill(image: &mut RgbaImage, rect: [i32; 4], colour: [u8; 4]) {
    let [x, y, width, height] = rect;
    for py in y.max(0)..(y + height).min(image.height() as i32) {
        for px in x.max(0)..(x + width).min(image.width() as i32) {
            image
                .get_pixel_mut(px as u32, py as u32)
                .blend(&Rgba(colour));
        }
    }
}

fn outline(image: &mut RgbaImage, rect: [i32; 4], colour: [u8; 4]) {
    let [x, y, width, height] = rect;
    fill(image, [x, y, width, 1], colour);
    fill(image, [x, y + height - 1, width, 1], colour);
    fill(image, [x, y, 1, height], colour);
    fill(image, [x + width - 1, y, 1, height], colour);
}

fn button(image: &mut RgbaImage, rect: [i32; 4], text: &str, selected: bool, hot: bool, size: f32) {
    let [x, y, width, height] = rect;
    fill(
        image,
        rect,
        if selected {
            [120, 90, 30, 200]
        } else if hot {
            [60, 70, 50, 200]
        } else {
            [8, 22, 28, 230]
        },
    );
    outline(
        image,
        rect,
        if selected || hot {
            GOLD_BRIGHT
        } else {
            [150, 126, 74, 255]
        },
    );
    outline(
        image,
        [x + 1, y + 1, width - 2, height - 2],
        [40, 78, 90, 255],
    );
    let text_height = size as i32 + 2;
    label(
        image,
        text,
        size,
        x + width / 2,
        y + (height - text_height) / 2,
        if selected || hot { GOLD_BRIGHT } else { GOLD },
    );
}

fn label(image: &mut RgbaImage, text: &str, size: f32, centre_x: i32, y: i32, colour: [u8; 4]) {
    if let Ok(font) = crate::main_hud::label_font() {
        let width = crate::main_hud::text_width(font, text, size);
        crate::main_hud::crisp_text(image, font, text, size, [centre_x - width / 2, y], colour);
    }
}

fn decorative(
    image: &mut RgbaImage,
    text: &str,
    size: f32,
    centre_x: i32,
    y: i32,
    colour: [u8; 4],
) {
    let Ok(font) = crate::labels::decorative_font() else {
        label(image, text, size, centre_x, y, colour);
        return;
    };
    let width = crate::main_hud::text_width(&font, text, size);
    crate::main_hud::text(
        image,
        &font,
        text,
        size,
        [centre_x - width / 2, y],
        colour,
        None,
    );
}

fn text_width(text: &str, size: f32) -> i32 {
    crate::main_hud::font().map_or(0, |font| crate::main_hud::text_width(font, text, size))
}

/// Body text in the UI sans face; returns the advance.
fn body_text(
    image: &mut RgbaImage,
    text: &str,
    x: i32,
    y: i32,
    colour: [u8; 4],
    max_width: i32,
) -> i32 {
    crate::main_hud::font().map_or(0, |font| {
        crate::main_hud::text(image, font, text, 12.0, [x, y], colour, Some(max_width))
    })
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
        // Blink the caret and poll the login session.
        if (self.started.elapsed().as_millis() / 500) % 2 == 0 {
            self.redraw();
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(
            Instant::now() + Duration::from_millis(100),
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
