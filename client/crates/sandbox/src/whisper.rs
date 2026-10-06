//! Private chats: each whisper goes into a conversation with the other
//! player, who shows as a face over the bar until the conversation is
//! opened in its own window, with their level, class and guild.

use crate::assets::{Assets, Result};
use crate::ui::UiRect;
use crate::ui_theme::{fill, outline};
use gravak_protocol::WhisperCard;
use image::{imageops, RgbaImage};

/// The lines a conversation keeps.
const KEPT_LINES: usize = 60;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WhisperLine {
    pub sender: String,
    pub text: String,
    /// A line from the server (someone not found or ignoring you).
    pub system: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct Conversation {
    pub name: String,
    pub card: Option<WhisperCard>,
    pub lines: Vec<WhisperLine>,
    /// Lines came in since it was last open.
    pub unread: bool,
}

#[derive(Debug, Default)]
pub struct Whispers {
    /// Oldest first.
    pub conversations: Vec<Conversation>,
    /// The conversation whose window is open.
    pub open: Option<String>,
    pub input: String,
    pub input_focused: bool,
}

fn same(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

impl Whispers {
    pub fn conversation(&self, name: &str) -> Option<&Conversation> {
        self.conversations.iter().find(|conversation| same(&conversation.name, name))
    }

    fn conversation_mut(&mut self, name: &str) -> &mut Conversation {
        let index = match self.conversations.iter().position(|conversation| same(&conversation.name, name)) {
            Some(index) => index,
            None => {
                self.conversations.push(Conversation { name: name.to_owned(), ..Conversation::default() });
                self.conversations.len() - 1
            }
        };
        &mut self.conversations[index]
    }

    /// A whisper heard or said: it goes into the conversation with the
    /// other end, unread unless that window is open.
    pub fn line(&mut self, own_name: &str, sender: &str, recipient: &str, text: &str) {
        let system = sender.eq_ignore_ascii_case("SYSTEM");
        let peer = if system {
            match self.open.clone() {
                Some(open) => open,
                None => return,
            }
        } else if same(sender, own_name) {
            recipient.to_owned()
        } else {
            sender.to_owned()
        };
        if peer.is_empty() {
            return;
        }
        let open = self.open.as_deref().is_some_and(|open| same(open, &peer));
        let incoming = !system && !same(sender, own_name);
        let conversation = self.conversation_mut(&peer);
        conversation.lines.push(WhisperLine { sender: sender.to_owned(), text: text.to_owned(), system });
        if conversation.lines.len() > KEPT_LINES {
            conversation.lines.remove(0);
        }
        if incoming && !open {
            conversation.unread = true;
        }
    }

    /// Who the other end is, from the server.
    pub fn card(&mut self, card: WhisperCard) {
        let name = card.name.clone();
        self.conversation_mut(&name).card = Some(card);
    }

    /// The faces waiting over the bar: conversations with unread lines,
    /// oldest first.
    pub fn waiting(&self) -> Vec<&Conversation> {
        self.conversations.iter().filter(|conversation| conversation.unread).collect()
    }

    pub fn open(&mut self, name: &str) {
        let conversation = self.conversation_mut(name);
        conversation.unread = false;
        let name = conversation.name.clone();
        self.open = Some(name);
        self.input_focused = true;
    }

    pub fn close(&mut self) {
        self.open = None;
        self.input_focused = false;
    }
}

/// The window, laid out as the original's private chat: the other's name
/// on top, guild and class, the conversation with their face, battle power
/// and level beside it, the line being written with your own face, and
/// Send and Close; in the original's message frame (`DialogLevWord`).
pub const WINDOW_SIZE: [u32; 2] = [448, 318];
const NAME_PLATE: UiRect = UiRect::new(88, 9, 270, 22);
const CLOSE: UiRect = UiRect::new(404, 11, 20, 20);
const GUILD: UiRect = UiRect::new(14, 44, 154, 20);
const CLASS: UiRect = UiRect::new(174, 44, 164, 20);
const HISTORY: UiRect = UiRect::new(14, 72, 324, 114);
const FACE: UiRect = UiRect::new(352, 60, 76, 76);
const POWER: UiRect = UiRect::new(346, 164, 88, 22);
const LEVEL_ROW: i32 = 192;
const INPUT: UiRect = UiRect::new(14, 210, 324, 68);
const OWN_FACE: UiRect = UiRect::new(354, 210, 70, 70);
const SEND: UiRect = UiRect::new(296, 288, 64, 22);
const CLOSE_BUTTON: UiRect = UiRect::new(370, 288, 64, 22);
/// A face waiting over the bar: the arrow and the framed face.
pub const FACE_ICON: [i32; 2] = [62, 48];

const BOX: [u8; 4] = [24, 46, 58, 255];
const BOX_EDGE: [u8; 4] = [104, 128, 136, 255];
const GOLD: [u8; 4] = [255, 226, 143, 255];
const WHITE: [u8; 4] = [235, 235, 235, 255];
const OWN: [u8; 4] = [110, 220, 110, 255];
const OTHER: [u8; 4] = [110, 200, 255, 255];
const SYSTEM: [u8; 4] = [255, 140, 120, 255];

/// Where the window sits: the middle of the screen, a little high.
pub fn window_position(viewport: [f32; 2]) -> [f32; 2] {
    [
        ((viewport[0] - WINDOW_SIZE[0] as f32) * 0.5).max(0.0).round(),
        ((viewport[1] - WINDOW_SIZE[1] as f32) * 0.4).max(0.0).round(),
    ]
}

/// The faces waiting over the bar, right to left from the screen's edge.
pub fn face_rect(viewport: [f32; 2], index: usize) -> UiRect {
    let [_, hud_top] = crate::original_ui_layout::main_hud_position(viewport);
    UiRect::new(
        viewport[0] as i32 - (index as i32 + 1) * (FACE_ICON[0] + 6) - 8,
        hud_top as i32 - FACE_ICON[1] - 70,
        FACE_ICON[0],
        FACE_ICON[1],
    )
}

/// What a click on the window asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WhisperClick {
    Close,
    Send,
    Input,
    /// On the window but nothing in particular.
    Window,
}

pub fn window_click_at(viewport: [f32; 2], cursor: [f32; 2]) -> Option<WhisperClick> {
    let [x, y] = window_position(viewport);
    let point = [cursor[0] - x, cursor[1] - y];
    if !UiRect::new(0, 0, WINDOW_SIZE[0] as i32, WINDOW_SIZE[1] as i32).contains_local(point) {
        return None;
    }
    Some(if CLOSE.contains_local(point) || CLOSE_BUTTON.contains_local(point) {
        WhisperClick::Close
    } else if SEND.contains_local(point) {
        WhisperClick::Send
    } else if INPUT.contains_local(point) {
        WhisperClick::Input
    } else {
        WhisperClick::Window
    })
}

/// The waiting face under the cursor, by its conversation's name.
pub fn face_at(whispers: &Whispers, viewport: [f32; 2], cursor: [f32; 2]) -> Option<String> {
    whispers.waiting().into_iter().enumerate().find_map(|(index, conversation)| {
        face_rect(viewport, index).contains_local(cursor).then(|| conversation.name.clone())
    })
}

fn say(image: &mut RgbaImage, text: &str, at: [i32; 2], colour: [u8; 4], width: Option<i32>) -> Result<()> {
    crate::main_hud::text(image, crate::main_hud::font()?, text, 12.0, at, colour, width);
    Ok(())
}

fn framed(image: &mut RgbaImage, rect: UiRect) {
    fill(image, rect, BOX);
    outline(image, rect, BOX_EDGE);
}

/// The original frame, its four corners kept and cut to `size`.
fn frame(assets: &mut Assets, size: [u32; 2]) -> Result<RgbaImage> {
    let source = crate::original_ui::visible_part(&assets.image("data/main/DialogLevWord.dds")?)?;
    let (width, height) = (source.width(), source.height());
    let (half_w, half_h) = ((size[0] / 2).min(width / 2), (size[1] / 2).min(height / 2));
    let (rest_w, rest_h) = ((size[0] - half_w).min(width), (size[1] - half_h).min(height));
    let mut image = RgbaImage::new(size[0], size[1]);
    for (sx, dx, w) in [(0, 0, half_w), (width - rest_w, half_w, rest_w)] {
        for (sy, dy, h) in [(0, 0, half_h), (height - rest_h, half_h, rest_h)] {
            let part = imageops::crop_imm(&source, sx, sy, w, h).to_image();
            imageops::overlay(&mut image, &part, i64::from(dx), i64::from(dy));
        }
    }
    Ok(image)
}

/// The face of the other end: the card the server sent, or how they look
/// on screen.
fn face(assets: &mut Assets, conversation: &Conversation, seen_mesh: Option<u32>, size: u32) -> Option<RgbaImage> {
    let mesh = conversation.card.as_ref().map(|card| card.mesh).or(seen_mesh)?;
    crate::renderer::player_face(assets, mesh, size)
}

/// `text` in lines no wider than `width`.
fn wrap(font: &fontdue::Font, text: &str, width: i32) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        let candidate = if line.is_empty() { word.to_owned() } else { format!("{line} {word}") };
        if crate::main_hud::text_width(font, &candidate, 12.0) > width && !line.is_empty() {
            lines.push(std::mem::replace(&mut line, word.to_owned()));
        } else {
            line = candidate;
        }
    }
    if !line.is_empty() || lines.is_empty() {
        lines.push(line);
    }
    lines
}

/// A framed face: a light frame, the portrait inside, or the name's
/// initial when the look is not known.
fn portrait(assets: &mut Assets, image: &mut RgbaImage, rect: UiRect, mesh: Option<u32>, name: &str, gold: bool) -> Result<()> {
    fill(image, rect, if gold { [196, 160, 90, 255] } else { [226, 222, 210, 255] });
    let inner = UiRect::new(rect.x + 3, rect.y + 3, rect.width - 6, rect.height - 6);
    fill(image, inner, [20, 24, 30, 255]);
    match mesh.and_then(|mesh| crate::renderer::player_face(assets, mesh, inner.width as u32)) {
        Some(face) => imageops::overlay(image, &face, i64::from(inner.x), i64::from(inner.y)),
        None => {
            let initial = name.chars().next().map(|first| first.to_uppercase().to_string()).unwrap_or_default();
            say(image, &initial, [inner.x + inner.width / 2 - 4, inner.y + inner.height / 2 - 7], GOLD, None)?;
        }
    }
    Ok(())
}

/// An original button, pressed under the cursor.
fn button(assets: &mut Assets, image: &mut RgbaImage, rect: UiRect, art: &str, pressed: bool) -> Result<()> {
    let path = if pressed { format!("data/main/{art}Click.dds") } else { format!("data/main/{art}.dds") };
    let source = assets.image(&path).or_else(|_| assets.image(&format!("data/main/{art}.dds")))?;
    let source = crate::original_ui::visible_part(&source)?;
    imageops::overlay(image, &source, i64::from(rect.x), i64::from(rect.y));
    Ok(())
}

/// The private chat window with `conversation`.
#[allow(clippy::too_many_arguments)]
pub fn window_image(
    assets: &mut Assets,
    conversation: &Conversation,
    own_name: &str,
    own_mesh: u32,
    seen_mesh: Option<u32>,
    input: &str,
    input_focused: bool,
    hovered: Option<WhisperClick>,
) -> Result<RgbaImage> {
    let mut image = frame(assets, WINDOW_SIZE)?;
    // The original's window is solid.
    for pixel in image.pixels_mut() {
        if pixel[3] > 90 {
            pixel[3] = 255;
        }
    }
    let font = crate::main_hud::font()?;
    // The other's name on the plate, as the original: "Name:".
    fill(&mut image, NAME_PLATE, [16, 32, 40, 255]);
    outline(&mut image, NAME_PLATE, [196, 160, 90, 255]);
    let inset = UiRect::new(NAME_PLATE.x + 2, NAME_PLATE.y + 2, NAME_PLATE.width - 4, NAME_PLATE.height - 4);
    outline(&mut image, inset, [92, 74, 44, 255]);
    say(&mut image, &format!("{}:", conversation.name), [NAME_PLATE.x + 10, NAME_PLATE.y + 5], WHITE, None)?;
    // A gold X, lit under the cursor.
    let x_colour = if hovered == Some(WhisperClick::Close) { [255, 244, 190, 255] } else { [214, 176, 96, 255] };
    for step in 0..12 {
        for thick in 0..2 {
            for (x, y) in [(CLOSE.x + 4 + step + thick, CLOSE.y + 4 + step), (CLOSE.x + 15 - step + thick, CLOSE.y + 4 + step)] {
                if x >= 0 && y >= 0 && (x as u32) < image.width() && (y as u32) < image.height() {
                    image.put_pixel(x as u32, y as u32, image::Rgba(x_colour));
                }
            }
        }
    }
    let card = conversation.card.as_ref();
    framed(&mut image, GUILD);
    let guild = card.map(|card| card.guild.as_str()).unwrap_or_default();
    say(&mut image, &format!("Guild: {guild}"), [GUILD.x + 5, GUILD.y + 4], WHITE, Some(GUILD.width - 10))?;
    framed(&mut image, CLASS);
    let class = card.map(|card| crate::shop::class_name(card.class as u8)).unwrap_or_default();
    say(&mut image, &format!("Class: {class}"), [CLASS.x + 5, CLASS.y + 4], WHITE, Some(CLASS.width - 10))?;
    // Their face, battle power and level.
    let their_mesh = card.map(|card| card.mesh).or(seen_mesh);
    portrait(assets, &mut image, FACE, their_mesh, &conversation.name, false)?;
    let label = "Max Battle Power";
    let width = crate::main_hud::text_width(font, label, 11.0);
    crate::main_hud::text(&mut image, font, label, 11.0, [POWER.x + (POWER.width - width) / 2, POWER.y - 16], GOLD, None);
    framed(&mut image, POWER);
    let power = card.map_or_else(|| "-".to_owned(), |card| card.battle_power.to_string());
    let width = crate::main_hud::text_width(font, &power, 12.0);
    say(&mut image, &power, [POWER.x + (POWER.width - width) / 2, POWER.y + 4], WHITE, None)?;
    let level = card.map_or_else(|| "Level -".to_owned(), |card| {
        if card.rebirths > 0 {
            format!("Level {}  (Reborn {})", card.level, card.rebirths)
        } else {
            format!("Level {}", card.level)
        }
    });
    let width = crate::main_hud::text_width(font, &level, 12.0);
    say(&mut image, &level, [HISTORY.x + (HISTORY.width - width) / 2, LEVEL_ROW], WHITE, None)?;
    // The conversation: names in colour, the words in white, the last
    // lines that fit.
    framed(&mut image, HISTORY);
    let mut rows: Vec<(Option<(String, [u8; 4])>, String)> = Vec::new();
    for line in &conversation.lines {
        if line.system {
            for row in wrap(font, &line.text, HISTORY.width - 12) {
                rows.push((Some((row, SYSTEM)), String::new()));
            }
            continue;
        }
        let colour = if line.sender.eq_ignore_ascii_case(own_name) { OWN } else { OTHER };
        let name = format!("{}: ", line.sender);
        let full = format!("{name}{}", line.text);
        for (index, row) in wrap(font, &full, HISTORY.width - 12).into_iter().enumerate() {
            match row.strip_prefix(&name).filter(|_| index == 0) {
                Some(rest) => rows.push((Some((name.clone(), colour)), rest.to_owned())),
                None if index == 0 => rows.push((Some((row, colour)), String::new())),
                None => rows.push((None, row)),
            }
        }
    }
    let shown = ((HISTORY.height - 8) / 15) as usize;
    for (index, (lead, rest)) in rows.iter().skip(rows.len().saturating_sub(shown)).enumerate() {
        let y = HISTORY.y + 5 + index as i32 * 15;
        let mut x = HISTORY.x + 6;
        if let Some((lead, colour)) = lead {
            say(&mut image, lead, [x, y], *colour, Some(HISTORY.width - 12))?;
            x += crate::main_hud::text_width(font, lead, 12.0) + 3;
        }
        if !rest.is_empty() {
            say(&mut image, rest, [x, y], WHITE, Some(HISTORY.x + HISTORY.width - 6 - x))?;
        }
    }
    // What is being written, and your own face beside it.
    framed(&mut image, INPUT);
    if input_focused {
        outline(&mut image, INPUT, [196, 160, 90, 255]);
    }
    let typed = if input_focused { format!("{input}|") } else { input.to_owned() };
    let rows = wrap(font, &typed, INPUT.width - 12);
    for (index, row) in rows.iter().skip(rows.len().saturating_sub(4)).enumerate() {
        say(&mut image, row, [INPUT.x + 6, INPUT.y + 5 + index as i32 * 15], WHITE, Some(INPUT.width - 12))?;
    }
    portrait(assets, &mut image, OWN_FACE, (own_mesh != 0).then_some(own_mesh), own_name, true)?;
    button(assets, &mut image, SEND, "MsgSendSend", hovered == Some(WhisperClick::Send))?;
    button(assets, &mut image, CLOSE_BUTTON, "CloseBtn", hovered == Some(WhisperClick::Close))?;
    Ok(image)
}

/// A waiting face: a gold arrow and the other end's face in a light frame
/// (the original's whisper notice).
pub fn face_icon(assets: &mut Assets, conversation: &Conversation, seen_mesh: Option<u32>, hovered: bool) -> Result<RgbaImage> {
    let [width, height] = FACE_ICON;
    let mut image = RgbaImage::new(width as u32, height as u32);
    let arrow = if hovered { [255, 236, 150, 255] } else { [224, 173, 73, 255] };
    for row in 0..20 {
        let reach = if row < 10 { row } else { 19 - row };
        for column in 0..=reach {
            image.put_pixel(column as u32, (height / 2 - 10 + row) as u32, image::Rgba(arrow));
        }
    }
    let frame = UiRect::new(width - height, 0, height, height);
    fill(&mut image, frame, if hovered { [255, 244, 200, 255] } else { [232, 226, 210, 255] });
    let inner = UiRect::new(frame.x + 3, frame.y + 3, frame.width - 6, frame.height - 6);
    fill(&mut image, inner, [20, 24, 30, 255]);
    match face(assets, conversation, seen_mesh, inner.width as u32) {
        Some(portrait) => imageops::overlay(&mut image, &portrait, i64::from(inner.x), i64::from(inner.y)),
        None => {
            let initial = conversation.name.chars().next().map(|first| first.to_uppercase().to_string()).unwrap_or_default();
            say(&mut image, &initial, [inner.x + inner.width / 2 - 4, inner.y + inner.height / 2 - 7], GOLD, None)?;
        }
    }
    Ok(image)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `BOUNTY_PREVIEW=dir cargo test -p conquer-sandbox whisper_preview`
    #[test]
    fn whisper_preview() {
        let Ok(dir) = std::env::var("BOUNTY_PREVIEW") else { return };
        let mut assets = Assets::open(std::path::Path::new("R:/5065")).expect("assets");
        let mut whispers = Whispers::default();
        whispers.line("Ikzordep", "Tom411", "Ikzordep", "eae, bora caçar no Bird Island? Tem um boss nascendo daqui a pouco");
        whispers.line("Ikzordep", "Ikzordep", "Tom411", "bora!");
        whispers.card(WhisperCard {
            mesh: 1_003 + 67 * 10_000,
            level: 110,
            rebirths: 1,
            class: 15,
            name: "Tom411".into(),
            guild: "Elite".into(),
            battle_power: 143,
        });
        let conversation = whispers.conversation("tom411").expect("conversation").clone();
        let window = window_image(&mut assets, &conversation, "Ikzordep", 1_004 + 12 * 10_000, None, "ja vou", true, Some(WhisperClick::Send))
            .expect("window");
        let icon = face_icon(&mut assets, &conversation, None, false).expect("icon");
        let mut sheet = RgbaImage::from_pixel(WINDOW_SIZE[0] + 80, WINDOW_SIZE[1], image::Rgba([60, 70, 60, 255]));
        imageops::overlay(&mut sheet, &window, 0, 0);
        imageops::overlay(&mut sheet, &icon, i64::from(WINDOW_SIZE[0]) + 10, 10);
        sheet.save(format!("{dir}/whisper.png")).expect("png");
    }

    #[test]
    fn whispers_gather_by_the_other_end() {
        let mut whispers = Whispers::default();
        whispers.line("Me", "Tom411", "Me", "eae");
        whispers.line("Me", "Me", "tom411", "opa");
        assert_eq!(whispers.conversations.len(), 1);
        assert_eq!(whispers.waiting().len(), 1);
        whispers.open("TOM411");
        assert!(whispers.waiting().is_empty());
        assert_eq!(whispers.open.as_deref(), Some("Tom411"));
        whispers.line("Me", "SYSTEM", "Me", "Character x was not found.");
        whispers.line("Me", "Tom411", "Me", "again");
        let conversation = whispers.conversation("tom411").expect("conversation");
        assert_eq!(conversation.lines.len(), 4);
        assert!(conversation.lines[2].system);
        assert!(!conversation.unread);
    }
}

