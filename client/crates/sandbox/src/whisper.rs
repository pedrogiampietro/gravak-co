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

/// The window: the original's message frame (`DialogLevWord`) cut to size.
pub const WINDOW_SIZE: [u32; 2] = [460, 300];
const NAME_PLATE: UiRect = UiRect::new(130, 9, 200, 22);
const CLOSE: UiRect = UiRect::new(426, 9, 22, 22);
const GUILD: UiRect = UiRect::new(22, 42, 160, 20);
const CLASS: UiRect = UiRect::new(188, 42, 160, 20);
const HISTORY: UiRect = UiRect::new(22, 68, 326, 124);
const FACE: UiRect = UiRect::new(358, 42, 82, 82);
const INPUT: UiRect = UiRect::new(22, 198, 326, 32);
const SEND: UiRect = UiRect::new(290, 240, 70, 22);
const CLOSE_BUTTON: UiRect = UiRect::new(370, 240, 70, 22);
/// A face waiting over the bar: the arrow and the framed face.
pub const FACE_ICON: [i32; 2] = [62, 48];

const BOX: [u8; 4] = [14, 30, 40, 235];
const BOX_EDGE: [u8; 4] = [120, 104, 70, 255];
const GOLD: [u8; 4] = [255, 226, 143, 255];
const WHITE: [u8; 4] = [235, 235, 235, 255];
const OWN: [u8; 4] = [120, 220, 120, 255];
const OTHER: [u8; 4] = [120, 190, 255, 255];
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

/// The private chat window with `conversation`.
pub fn window_image(
    assets: &mut Assets,
    conversation: &Conversation,
    own_name: &str,
    seen_mesh: Option<u32>,
    input: &str,
    input_focused: bool,
    hovered: Option<WhisperClick>,
) -> Result<RgbaImage> {
    let mut image = frame(assets, WINDOW_SIZE)?;
    let font = crate::main_hud::font()?;
    framed(&mut image, NAME_PLATE);
    let width = crate::main_hud::text_width(font, &conversation.name, 12.0);
    say(&mut image, &conversation.name, [NAME_PLATE.x + (NAME_PLATE.width - width) / 2, NAME_PLATE.y + 5], GOLD, None)?;
    crate::ui_theme::close_button(&mut image, hovered == Some(WhisperClick::Close), CLOSE.x, CLOSE.y);
    let card = conversation.card.as_ref();
    framed(&mut image, GUILD);
    let guild = card.map(|card| card.guild.as_str()).filter(|guild| !guild.is_empty()).unwrap_or("-");
    say(&mut image, &format!("Guild: {guild}"), [GUILD.x + 5, GUILD.y + 4], WHITE, Some(GUILD.width - 10))?;
    framed(&mut image, CLASS);
    let class = card.map_or_else(|| "-".to_owned(), |card| crate::shop::class_name(card.class as u8));
    say(&mut image, &format!("Class: {class}"), [CLASS.x + 5, CLASS.y + 4], WHITE, Some(CLASS.width - 10))?;
    // The face in a light frame, the level and rebirths under it.
    fill(&mut image, FACE, [222, 214, 190, 255]);
    let inner = UiRect::new(FACE.x + 3, FACE.y + 3, FACE.width - 6, FACE.height - 6);
    fill(&mut image, inner, [20, 24, 30, 255]);
    if let Some(portrait) = face(assets, conversation, seen_mesh, inner.width as u32) {
        imageops::overlay(&mut image, &portrait, i64::from(inner.x), i64::from(inner.y));
    }
    if let Some(card) = card {
        let level = format!("Level {}", card.level);
        let width = crate::main_hud::text_width(font, &level, 12.0);
        say(&mut image, &level, [FACE.x + (FACE.width - width) / 2, FACE.y + FACE.height + 6], WHITE, None)?;
        if card.rebirths > 0 {
            let reborn = format!("Reborn {}", card.rebirths);
            let width = crate::main_hud::text_width(font, &reborn, 12.0);
            say(&mut image, &reborn, [FACE.x + (FACE.width - width) / 2, FACE.y + FACE.height + 22], GOLD, None)?;
        }
    }
    // The conversation: its last lines that fit.
    framed(&mut image, HISTORY);
    let mut rows: Vec<(String, [u8; 4])> = Vec::new();
    for line in &conversation.lines {
        let colour = if line.system {
            SYSTEM
        } else if line.sender.eq_ignore_ascii_case(own_name) {
            OWN
        } else {
            OTHER
        };
        let text = if line.system { line.text.clone() } else { format!("{}: {}", line.sender, line.text) };
        for (index, row) in wrap(font, &text, HISTORY.width - 12).into_iter().enumerate() {
            rows.push((row, if index == 0 { colour } else { WHITE }));
        }
    }
    let shown = ((HISTORY.height - 8) / 15) as usize;
    for (index, (row, colour)) in rows.iter().skip(rows.len().saturating_sub(shown)).enumerate() {
        say(&mut image, row, [HISTORY.x + 6, HISTORY.y + 5 + index as i32 * 15], *colour, Some(HISTORY.width - 12))?;
    }
    // What is being written.
    framed(&mut image, INPUT);
    if input_focused {
        outline(&mut image, INPUT, GOLD);
    }
    let typed = if input_focused { format!("{input}|") } else { input.to_owned() };
    let rows = wrap(font, &typed, INPUT.width - 12);
    for (index, row) in rows.iter().skip(rows.len().saturating_sub(2)).enumerate() {
        say(&mut image, row, [INPUT.x + 6, INPUT.y + 3 + index as i32 * 14], WHITE, Some(INPUT.width - 12))?;
    }
    crate::ui::draw_text_button(&mut image, font, SEND, "Send", hovered == Some(WhisperClick::Send), input.trim().is_empty());
    crate::ui::draw_text_button(&mut image, font, CLOSE_BUTTON, "Close", hovered == Some(WhisperClick::Close), false);
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
        });
        let conversation = whispers.conversation("tom411").expect("conversation").clone();
        let window = window_image(&mut assets, &conversation, "Ikzordep", None, "ja vou", true, Some(WhisperClick::Send))
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
