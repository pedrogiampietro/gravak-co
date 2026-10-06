//! The chat log over the bottom left: channel tabs, each channel in its
//! colour, long lines wrapped, a wheel to read back, and a window of its own
//! to move by its header, size by its corner grip, minimise to the header,
//! and set the letters' size. A name says who it is: a click whispers them,
//! the right button offers more. Drawn as its own texture, so moving it
//! costs nothing.

use crate::assets::Result;
use crate::network::ChatMessage;
use crate::ui::UiRect;
use crate::ui_theme::{fill, outline};
use image::{Rgba, RgbaImage};

/// The tabs and the channel each shows (0: all).
pub const TABS: [(&str, u16); 7] = [
    ("All", 0),
    ("Local", 2000),
    ("World", 2021),
    ("Team", 2003),
    ("Guild", 2004),
    ("Whisper", 2001),
    ("System", 2007),
];
/// The letters' sizes the Aa button goes through.
pub const FONT_SIZES: [f32; 3] = [11.0, 12.0, 14.0];
/// The lines kept.
pub const KEPT: usize = 200;
const HEADER: i32 = 24;
const MIN_SIZE: [i32; 2] = [320, 96];
const DEFAULT_SIZE: [i32; 2] = [590, 232];
const BUTTON: i32 = 18;
const GRIP: i32 = 12;
const PAD: i32 = 6;

/// What the left button is holding.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Grab {
    /// The header, `offset` from the window's corner.
    Move { offset: [f32; 2] },
    /// The top right grip; the bottom left corner stays.
    Resize,
}

/// The right button's menu on a line.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LineMenu {
    /// Empty for the server's lines.
    pub sender: String,
    pub message: String,
    pub at: [i32; 2],
}

pub const MENU_OPTIONS: [&str; 4] = ["Whisper", "Add Friend", "Copy Name", "Copy Message"];
const MENU_ROW: i32 = 20;
const MENU_WIDTH: i32 = 130;

#[derive(Debug, Clone, PartialEq)]
pub struct ChatLogState {
    /// x, y, width, height; `None` where the original keeps it.
    pub rect: Option<[i32; 4]>,
    pub minimized: bool,
    /// `FONT_SIZES` index.
    pub font: u8,
    /// Rows read back from the newest.
    pub scroll: usize,
    pub filter: u16,
    pub grab: Option<Grab>,
    pub menu: Option<LineMenu>,
    /// Lines that came in while read back or minimised.
    pub unread: usize,
}

impl Default for ChatLogState {
    fn default() -> Self {
        Self { rect: None, minimized: false, font: 1, scroll: 0, filter: 0, grab: None, menu: None, unread: 0 }
    }
}

/// Where the log sits at first: the bottom left, over the bar, as the
/// original's.
fn default_rect(viewport: [f32; 2]) -> [i32; 4] {
    let [hud_x, hud_y] = crate::original_ui_layout::main_hud_position(viewport);
    let bottom = if (hud_x as i32) < DEFAULT_SIZE[0] { hud_y as i32 + 8 } else { hud_y as i32 + 18 };
    [0, bottom - DEFAULT_SIZE[1], DEFAULT_SIZE[0], DEFAULT_SIZE[1]]
}

impl ChatLogState {
    /// The whole window, kept on the screen.
    pub fn full_rect(&self, viewport: [f32; 2]) -> [i32; 4] {
        let [x, y, w, h] = self.rect.unwrap_or_else(|| default_rect(viewport));
        let (vw, vh) = (viewport[0] as i32, viewport[1] as i32);
        let w = w.clamp(MIN_SIZE[0], vw.max(MIN_SIZE[0]));
        let h = h.clamp(MIN_SIZE[1], vh.max(MIN_SIZE[1]));
        [x.clamp(0, (vw - w).max(0)), y.clamp(0, (vh - h).max(0)), w, h]
    }

    /// What is shown: the header alone, at the bottom, when minimised.
    pub fn shown_rect(&self, viewport: [f32; 2]) -> [i32; 4] {
        let [x, y, w, h] = self.full_rect(viewport);
        if self.minimized { [x, y + h - HEADER, w, HEADER] } else { [x, y, w, h] }
    }

    pub fn contains(&self, viewport: [f32; 2], cursor: [f32; 2]) -> bool {
        let [x, y, w, h] = self.shown_rect(viewport);
        UiRect::new(x, y, w, h).contains_local(cursor)
    }

    fn line_height(&self) -> i32 {
        FONT_SIZES[usize::from(self.font).min(FONT_SIZES.len() - 1)] as i32 + 4
    }

    /// Follows the held header or grip.
    pub fn drag(&mut self, viewport: [f32; 2], cursor: [f32; 2]) {
        let Some(grab) = self.grab else {
            return;
        };
        let [x, y, w, h] = self.full_rect(viewport);
        self.rect = Some(match grab {
            Grab::Move { offset } => {
                let shown = self.shown_rect(viewport);
                let top = cursor[1] - offset[1];
                // Minimised, the header is what moves; the body follows.
                let y = if self.minimized { top as i32 - (h - HEADER) } else { top as i32 };
                let _ = shown;
                [(cursor[0] - offset[0]) as i32, y, w, h]
            }
            Grab::Resize => {
                let bottom = y + h;
                let width = (cursor[0] as i32 - x).max(MIN_SIZE[0]);
                let top = (cursor[1] as i32).min(bottom - MIN_SIZE[1]);
                [x, top, width, bottom - top]
            }
        });
    }

    /// Reads back (`up`) or forward a few rows.
    pub fn scroll_by(&mut self, up: bool) {
        self.scroll = if up { self.scroll + 3 } else { self.scroll.saturating_sub(3) };
        if self.scroll == 0 {
            self.unread = 0;
        }
    }

    /// A line came in: read back or minimised, it waits as unread.
    pub fn arrived(&mut self) {
        if self.scroll > 0 || self.minimized {
            self.unread += 1;
            if self.scroll > 0 {
                // Keep the same lines in view.
                self.scroll += 1;
            }
        }
    }
}

/// A channel's tag, its colour and the words' colour.
pub fn channel_style(channel: u16) -> (Option<&'static str>, [u8; 4], [u8; 4]) {
    match channel {
        2001 => (Some("Whisper"), [255, 120, 230, 255], [255, 170, 240, 255]),
        2003 => (Some("Team"), [96, 220, 255, 255], [170, 236, 255, 255]),
        2004 => (Some("Guild"), [110, 240, 110, 255], [180, 255, 170, 255]),
        2009 => (Some("Friend"), [255, 160, 200, 255], [255, 205, 225, 255]),
        2021 => (Some("World"), [255, 170, 70, 255], [255, 214, 150, 255]),
        2005 | 2007 | 2011 | 2012 => (Some("System"), [255, 86, 86, 255], [255, 240, 110, 255]),
        2500 => (Some("Broadcast"), [255, 196, 64, 255], [255, 228, 150, 255]),
        _ => (None, [255, 255, 255, 255], [240, 240, 240, 255]),
    }
}

/// The colour of a speaker's name: the hero's own in gold, others light
/// blue.
const NAME: [u8; 4] = [130, 200, 255, 255];
const OWN_NAME: [u8; 4] = [255, 214, 110, 255];

fn shown_message(message: &ChatMessage) -> bool {
    !message.message.is_empty()
}

fn passes(filter: u16, channel: u16) -> bool {
    match filter {
        0 => true,
        2007 => matches!(channel, 2005 | 2007 | 2011 | 2012 | 2500),
        filter => channel == filter,
    }
}

/// A word of a line in its colour; `name` marks the speaker's name.
#[derive(Debug, Clone)]
struct Piece {
    text: String,
    colour: [u8; 4],
    name: bool,
}

/// One row as drawn: its message (an index into the history), its pieces
/// at their x, and where the speaker's name is.
#[derive(Debug, Clone)]
pub struct Row {
    pub message: usize,
    pieces: Vec<(String, [u8; 4], i32)>,
    pub name: Option<(i32, i32)>,
}

fn pieces(message: &ChatMessage, own_name: &str) -> Vec<Piece> {
    let (tag, tag_colour, words) = channel_style(message.channel);
    let mut pieces = Vec::new();
    if let Some(tag) = tag {
        pieces.push(Piece { text: format!("[{tag}]"), colour: tag_colour, name: false });
    }
    let server = matches!(message.channel, 2005 | 2007 | 2011 | 2012) || message.sender_name.eq_ignore_ascii_case("SYSTEM");
    if !server && !message.sender_name.is_empty() {
        let own = message.sender_name.eq_ignore_ascii_case(own_name);
        pieces.push(Piece {
            text: message.sender_name.clone(),
            colour: if own { OWN_NAME } else { NAME },
            name: true,
        });
        if message.channel == 2001 && own {
            pieces.push(Piece { text: format!("» {}:", message.recipient_name), colour: words, name: false });
        } else {
            let last = pieces.last_mut().expect("the name");
            last.text.push(':');
        }
    }
    for word in message.message.split_whitespace() {
        pieces.push(Piece { text: word.to_owned(), colour: words, name: false });
    }
    pieces
}

/// The rows of the shown lines, oldest first, wrapped to `width`.
pub fn rows(font: &fontdue::Font, history: &[ChatMessage], own_name: &str, filter: u16, width: i32, size: f32) -> Vec<Row> {
    let space = crate::main_hud::text_width(font, " ", size).max(3);
    let mut rows = Vec::new();
    for (index, message) in history.iter().enumerate() {
        if !passes(filter, message.channel) || !shown_message(message) {
            continue;
        }
        let mut row = Row { message: index, pieces: Vec::new(), name: None };
        let mut x = 0;
        for piece in pieces(message, own_name) {
            let piece_width = crate::main_hud::text_width(font, &piece.text, size);
            if x > 0 && x + piece_width > width {
                rows.push(std::mem::replace(&mut row, Row { message: index, pieces: Vec::new(), name: None }));
                x = 14;
            }
            if piece.name {
                let name_width = crate::main_hud::text_width(font, piece.text.trim_end_matches(':'), size);
                row.name = Some((x, x + name_width));
            }
            row.pieces.push((piece.text, piece.colour, x));
            x += piece_width + space;
        }
        rows.push(row);
    }
    rows
}

/// What the cursor is over, for its light.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChatHover {
    Tab(usize),
    Font,
    Minimize,
    Grip,
    Newest,
    /// A speaker's name, by its message.
    Name(usize),
}

/// What a click on the log does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChatClick {
    Tab(u16),
    Font,
    Minimize,
    Grip,
    Header,
    Newest,
    /// A speaker's name: whisper them.
    Name(String),
    /// A line (for the right button's menu): its sender and words.
    Line { sender: String, message: String },
    Body,
}

fn tab_rects(font: &fontdue::Font) -> Vec<UiRect> {
    let mut x = 4;
    TABS.iter()
        .map(|(label, _)| {
            let width = crate::main_hud::text_width(font, label, 12.0) + 16;
            let rect = UiRect::new(x, 3, width, HEADER - 6);
            x += width + 2;
            rect
        })
        .collect()
}

fn font_button(width: i32) -> UiRect {
    UiRect::new(width - GRIP - 2 * (BUTTON + 4), 3, BUTTON + 4, BUTTON)
}

fn minimize_button(width: i32) -> UiRect {
    UiRect::new(width - GRIP - BUTTON - 2, 3, BUTTON, BUTTON)
}

fn grip(width: i32) -> UiRect {
    UiRect::new(width - GRIP, 0, GRIP, GRIP)
}

fn newest_button(width: i32, height: i32) -> UiRect {
    UiRect::new(width - 70, height - 22, 60, 18)
}

/// What is under `cursor` on the log; `None` off it.
pub fn click_at(
    state: &ChatLogState,
    history: &[ChatMessage],
    own_name: &str,
    viewport: [f32; 2],
    cursor: [f32; 2],
) -> Option<ChatClick> {
    let font = crate::main_hud::font().ok()?;
    let [x, y, w, h] = state.shown_rect(viewport);
    let point = [cursor[0] - x as f32, cursor[1] - y as f32];
    if !UiRect::new(0, 0, w, h).contains_local(point) {
        return None;
    }
    if point[1] < HEADER as f32 {
        if let Some(index) = tab_rects(font).iter().position(|tab| tab.contains_local(point)) {
            return Some(ChatClick::Tab(TABS[index].1));
        }
        if font_button(w).contains_local(point) {
            return Some(ChatClick::Font);
        }
        if minimize_button(w).contains_local(point) {
            return Some(ChatClick::Minimize);
        }
        if grip(w).contains_local(point) && !state.minimized {
            return Some(ChatClick::Grip);
        }
        return Some(ChatClick::Header);
    }
    if state.scroll > 0 && newest_button(w, h).contains_local(point) {
        return Some(ChatClick::Newest);
    }
    let size = FONT_SIZES[usize::from(state.font).min(FONT_SIZES.len() - 1)];
    let rows = rows(font, history, own_name, state.filter, w - PAD * 2 - 8, size);
    let (first, _) = visible(&rows, state, h);
    let line = state.line_height();
    let top = h - PAD - (rows.len().saturating_sub(state.scroll).saturating_sub(first)) as i32 * line;
    let index = ((point[1] as i32 - top) as f32 / line as f32).floor();
    if index < 0.0 {
        return Some(ChatClick::Body);
    }
    let Some(row) = rows.get(first + index as usize).filter(|_| first + (index as usize) < rows.len() - state.scroll.min(rows.len())) else {
        return Some(ChatClick::Body);
    };
    let message = &history[row.message];
    let local_x = point[0] as i32 - PAD;
    if let Some((from, to)) = row.name
        && local_x >= from
        && local_x <= to
    {
        return Some(ChatClick::Name(message.sender_name.clone()));
    }
    let server = matches!(message.channel, 2005 | 2007 | 2011 | 2012) || message.sender_name.eq_ignore_ascii_case("SYSTEM");
    Some(ChatClick::Line {
        sender: if server { String::new() } else { message.sender_name.clone() },
        message: message.message.clone(),
    })
}

/// The first row shown and how many fit.
fn visible(rows: &[Row], state: &ChatLogState, height: i32) -> (usize, usize) {
    let fit = ((height - HEADER - PAD * 2) / state.line_height()).max(1) as usize;
    let end = rows.len().saturating_sub(state.scroll.min(rows.len().saturating_sub(fit.min(rows.len()))));
    (end.saturating_sub(fit), fit)
}

pub fn hover_at(state: &ChatLogState, history: &[ChatMessage], own_name: &str, viewport: [f32; 2], cursor: [f32; 2]) -> Option<ChatHover> {
    Some(match click_at(state, history, own_name, viewport, cursor)? {
        ChatClick::Tab(channel) => ChatHover::Tab(TABS.iter().position(|(_, tab)| *tab == channel)?),
        ChatClick::Font => ChatHover::Font,
        ChatClick::Minimize => ChatHover::Minimize,
        ChatClick::Grip => ChatHover::Grip,
        ChatClick::Newest => ChatHover::Newest,
        ChatClick::Name(name) => ChatHover::Name(history.iter().rposition(|message| message.sender_name == name)?),
        _ => return None,
    })
}

/// Everything the log's picture depends on.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ChatLogView {
    pub history: Vec<ChatMessage>,
    pub own_name: String,
    pub filter: u16,
    pub size: [i32; 2],
    pub font: u8,
    pub scroll: usize,
    pub minimized: bool,
    pub unread: usize,
    pub hover: Option<ChatHover>,
    pub hovered: bool,
    pub menu: Option<LineMenu>,
}

fn say(image: &mut RgbaImage, font: &fontdue::Font, text: &str, size: f32, at: [i32; 2], colour: [u8; 4], width: Option<i32>) {
    // A dark shadow keeps the words readable over any ground.
    crate::main_hud::text(image, font, text, size, [at[0] + 1, at[1] + 1], [0, 0, 0, 200], width);
    crate::main_hud::text(image, font, text, size, at, colour, width);
}

/// The log's picture, `view.size` large (the header alone, minimised).
pub fn image(view: &ChatLogView) -> Result<RgbaImage> {
    let font = crate::main_hud::font()?;
    let [w, full_h] = view.size;
    let h = if view.minimized { HEADER } else { full_h };
    let mut image = RgbaImage::new(w as u32, h as u32);
    // The body: dark glass, a little darker under the cursor.
    if !view.minimized {
        fill(&mut image, UiRect::new(0, HEADER, w, h - HEADER), [6, 8, 12, if view.hovered { 120 } else { 82 }]);
    }
    // The header: a darker band with a thin gold rule under it.
    fill(&mut image, UiRect::new(0, 0, w, HEADER), [12, 14, 20, if view.hovered { 210 } else { 170 }]);
    fill(&mut image, UiRect::new(0, HEADER - 1, w, 1), [150, 122, 66, 200]);
    for (index, (tab, (label, channel))) in tab_rects(font).iter().zip(TABS).enumerate() {
        let active = channel == view.filter;
        let lit = view.hover == Some(ChatHover::Tab(index));
        fill(&mut image, *tab, if active { [74, 60, 30, 240] } else if lit { [50, 54, 64, 235] } else { [30, 33, 41, 220] });
        outline(&mut image, *tab, if active { [214, 176, 96, 255] } else { [70, 74, 84, 255] });
        let colour = if active { [255, 226, 143, 255] } else if lit { [255, 255, 255, 255] } else { [196, 200, 208, 255] };
        let x = tab.x + (tab.width - crate::main_hud::text_width(font, label, 12.0)) / 2;
        crate::main_hud::text(&mut image, font, label, 12.0, [x, tab.y + 2], colour, None);
    }
    // Aa: the letters' size; - or +: minimise; the grip.
    let button = |image: &mut RgbaImage, rect: UiRect, lit: bool| {
        fill(image, rect, if lit { [60, 52, 34, 240] } else { [30, 33, 41, 220] });
        outline(image, rect, if lit { [214, 176, 96, 255] } else { [90, 92, 100, 255] });
    };
    let aa = font_button(w);
    button(&mut image, aa, view.hover == Some(ChatHover::Font));
    crate::main_hud::text(&mut image, font, "A", 10.0, [aa.x + 3, aa.y + 4], [230, 230, 230, 255], None);
    crate::main_hud::text(&mut image, font, "A", 14.0, [aa.x + 10, aa.y + 1], [230, 230, 230, 255], None);
    let minimize = minimize_button(w);
    button(&mut image, minimize, view.hover == Some(ChatHover::Minimize));
    let sign = if view.minimized { "+" } else { "-" };
    let sign_width = crate::main_hud::text_width(font, sign, 14.0);
    crate::main_hud::text(&mut image, font, sign, 14.0, [minimize.x + (BUTTON - sign_width) / 2, minimize.y], [230, 230, 230, 255], None);
    if !view.minimized {
        let g = grip(w);
        let colour = if view.hover == Some(ChatHover::Grip) { [255, 226, 143, 255] } else { [150, 150, 160, 220] };
        // Three short diagonals in the corner, like a window's grip.
        for step in [4, 8, 12] {
            for offset in 0..step {
                for thick in 0..2 {
                    let (px, py) = (g.x + GRIP - step + offset + thick, g.y + offset);
                    if px >= 0 && (px as u32) < image.width() && (py as u32) < image.height() {
                        image.put_pixel(px as u32, py as u32, Rgba(colour));
                    }
                }
            }
        }
    }
    if view.minimized {
        if view.unread > 0 {
            let note = format!("{} new", view.unread);
            let x = aa.x - 8 - crate::main_hud::text_width(font, &note, 12.0);
            say(&mut image, font, &note, 12.0, [x, 5], [255, 226, 143, 255], None);
        }
        return Ok(image);
    }
    // The lines, newest at the bottom.
    let size = FONT_SIZES[usize::from(view.font).min(FONT_SIZES.len() - 1)];
    let state = ChatLogState { font: view.font, scroll: view.scroll, ..ChatLogState::default() };
    let rows = rows(font, &view.history, &view.own_name, view.filter, w - PAD * 2 - 8, size);
    let (first, fit) = visible(&rows, &state, h);
    let shown = &rows[first..(first + fit).min(rows.len())];
    let line = state.line_height();
    let top = h - PAD - shown.len() as i32 * line;
    for (index, row) in shown.iter().enumerate() {
        let y = top + index as i32 * line;
        let lit_name = matches!(view.hover, Some(ChatHover::Name(message)) if view.history.get(message).map(|m| &m.sender_name) == view.history.get(row.message).map(|m| &m.sender_name));
        for (text, colour, x) in &row.pieces {
            say(&mut image, font, text, size, [PAD + x, y], *colour, Some(w - PAD - 8 - x));
        }
        // A hovered name is underlined: a click whispers them.
        if let Some((from, to)) = row.name
            && lit_name
        {
            fill(&mut image, UiRect::new(PAD + from, y + size as i32 + 1, to - from, 1), OWN_NAME);
        }
    }
    // Read back: the bar on the right and the way to the newest.
    if rows.len() > fit {
        let track = UiRect::new(w - 6, HEADER + 4, 3, h - HEADER - 8);
        fill(&mut image, track, [255, 255, 255, 30]);
        let thumb_h = ((track.height as f32) * fit as f32 / rows.len() as f32).max(10.0) as i32;
        let thumb_y = track.y + ((track.height - thumb_h) as f32 * first as f32 / (rows.len() - fit).max(1) as f32) as i32;
        fill(&mut image, UiRect::new(track.x, thumb_y, 3, thumb_h), [214, 176, 96, 220]);
    }
    if view.scroll > 0 {
        let newest = newest_button(w, h);
        let lit = view.hover == Some(ChatHover::Newest);
        fill(&mut image, newest, if lit { [90, 72, 36, 240] } else { [50, 42, 24, 230] });
        outline(&mut image, newest, [214, 176, 96, 255]);
        let label = if view.unread > 0 { format!("{} new", view.unread) } else { "Newest".to_owned() };
        let x = newest.x + (newest.width - crate::main_hud::text_width(font, &label, 11.0)) / 2;
        crate::main_hud::text(&mut image, font, &label, 11.0, [x, newest.y + 3], [255, 226, 143, 255], None);
    }
    Ok(image)
}

/// The right button's menu: its options for the line (a server line can
/// only be copied).
pub fn menu_options(menu: &LineMenu) -> Vec<usize> {
    if menu.sender.is_empty() { vec![3] } else { (0..MENU_OPTIONS.len()).collect() }
}

fn menu_rect(menu: &LineMenu, viewport: [f32; 2]) -> UiRect {
    let height = menu_options(menu).len() as i32 * MENU_ROW + 6 + if menu.sender.is_empty() { 0 } else { MENU_ROW };
    let x = menu.at[0].min(viewport[0] as i32 - MENU_WIDTH - 4).max(0);
    let y = (menu.at[1] - height).max(0);
    UiRect::new(x, y, MENU_WIDTH, height)
}

pub fn menu_position(menu: &LineMenu, viewport: [f32; 2]) -> [f32; 2] {
    let rect = menu_rect(menu, viewport);
    [rect.x as f32, rect.y as f32]
}

/// The option under the cursor (`MENU_OPTIONS` index); `Some(None)` on the
/// menu but off its options, `None` off it.
pub fn menu_click_at(menu: &LineMenu, viewport: [f32; 2], cursor: [f32; 2]) -> Option<Option<usize>> {
    let rect = menu_rect(menu, viewport);
    let point = [cursor[0] - rect.x as f32, cursor[1] - rect.y as f32];
    if !UiRect::new(0, 0, rect.width, rect.height).contains_local(point) {
        return None;
    }
    let first = if menu.sender.is_empty() { 3 } else { 3 + MENU_ROW };
    let row = (point[1] as i32 - first).div_euclid(MENU_ROW);
    Some(usize::try_from(row).ok().and_then(|row| menu_options(menu).get(row).copied()))
}

pub fn menu_image(menu: &LineMenu, viewport: [f32; 2], hover: Option<usize>) -> Result<RgbaImage> {
    let font = crate::main_hud::font()?;
    let rect = menu_rect(menu, viewport);
    let mut image = RgbaImage::new(rect.width as u32, rect.height as u32);
    let whole = UiRect::new(0, 0, rect.width, rect.height);
    fill(&mut image, whole, [10, 12, 16, 236]);
    outline(&mut image, whole, [150, 122, 66, 255]);
    let mut y = 3;
    if !menu.sender.is_empty() {
        say(&mut image, font, &menu.sender, 12.0, [8, y + 3], NAME, Some(rect.width - 12));
        fill(&mut image, UiRect::new(4, y + MENU_ROW - 2, rect.width - 8, 1), [90, 74, 44, 255]);
        y += MENU_ROW;
    }
    for option in menu_options(menu) {
        let row = UiRect::new(3, y, rect.width - 6, MENU_ROW);
        let lit = hover == Some(option);
        if lit {
            fill(&mut image, row, [96, 78, 44, 220]);
        }
        crate::main_hud::text(&mut image, font, MENU_OPTIONS[option], 12.0, [row.x + 8, row.y + 3], if lit { [255, 236, 170, 255] } else { [235, 235, 235, 255] }, None);
        y += MENU_ROW;
    }
    Ok(image)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message(channel: u16, sender: &str, text: &str) -> ChatMessage {
        ChatMessage { channel, sender_name: sender.into(), recipient_name: "ALLUSERS".into(), message: text.into() }
    }

    #[test]
    fn the_window_stays_on_screen_and_minimises_to_its_header() {
        let viewport = [1024.0, 768.0];
        let mut state = ChatLogState::default();
        let [x, y, w, h] = state.full_rect(viewport);
        assert_eq!((x, w), (0, DEFAULT_SIZE[0]));
        state.minimized = true;
        assert_eq!(state.shown_rect(viewport), [x, y + h - HEADER, w, HEADER]);
        state.minimized = false;
        state.rect = Some([5000, -40, 10, 10]);
        let [x, y, w, h] = state.full_rect(viewport);
        assert_eq!([w, h], MIN_SIZE);
        assert_eq!([x, y], [1024 - MIN_SIZE[0], 0]);
        state.rect = None;
        state.grab = Some(Grab::Resize);
        let [x, y, w, h] = state.full_rect(viewport);
        state.drag(viewport, [(x + w + 50) as f32, (y - 30) as f32]);
        assert_eq!(state.full_rect(viewport), [x, y - 30, w + 50, h + 30]);
    }

    #[test]
    fn names_are_found_under_the_cursor() {
        let viewport = [1024.0, 768.0];
        let history = vec![message(2000, "Tom411", "eae"), message(2007, "SYSTEM", "A boss appeared")];
        let state = ChatLogState::default();
        let [x, y, _, h] = state.shown_rect(viewport);
        let line = state.line_height() as f32;
        let bottom = (y + h - PAD) as f32;
        let tom = click_at(&state, &history, "Me", viewport, [x as f32 + PAD as f32 + 4.0, bottom - line * 1.5]);
        assert_eq!(tom, Some(ChatClick::Name("Tom411".into())));
        let system = click_at(&state, &history, "Me", viewport, [x as f32 + 200.0, bottom - line * 0.5]);
        assert_eq!(system, Some(ChatClick::Line { sender: String::new(), message: "A boss appeared".into() }));
    }

    /// `BOUNTY_PREVIEW=dir cargo test -p conquer-sandbox chat_log_preview`
    #[test]
    fn chat_log_preview() {
        let Ok(dir) = std::env::var("BOUNTY_PREVIEW") else { return };
        let history = vec![
            message(2000, "Gravak790942391", "eae"),
            message(2000, "Gravak790942391", "oi gente"),
            message(2007, "SYSTEM", "The fearsome WingedSnakeKing has appeared near Phoenix Castle! Brave souls are needed to defeat it."),
            message(2001, "Tom411", "bora caçar?"),
            ChatMessage { channel: 2001, sender_name: "Ikzordep".into(), recipient_name: "Tom411".into(), message: "bora!".into() },
            message(2003, "Tom411", "team aqui"),
            message(2004, "Ikzordep", "guild reunião às 20h"),
            message(2021, "Merchant", "vendo Super Dragon Ball, chama no PM"),
            message(2500, "Ikzordep", "evento de PK hoje!"),
            message(2000, "Ikzordep", "eh"),
        ];
        let view = ChatLogView {
            history,
            own_name: "Ikzordep".into(),
            filter: 0,
            size: [590, 232],
            font: 1,
            scroll: 0,
            minimized: false,
            unread: 0,
            hover: Some(ChatHover::Name(3)),
            hovered: true,
            menu: None,
        };
        let mut sheet = RgbaImage::from_pixel(600, 300, Rgba([128, 132, 138, 255]));
        image::imageops::overlay(&mut sheet, &image(&view).expect("log"), 5, 5);
        let minimized = image(&ChatLogView { minimized: true, unread: 3, hover: Some(ChatHover::Minimize), ..view.clone() }).expect("min");
        image::imageops::overlay(&mut sheet, &minimized, 5, 250);
        sheet.save(format!("{dir}/chat-log.png")).expect("png");
        let menu = LineMenu { sender: "Tom411".into(), message: "bora".into(), at: [100, 200] };
        menu_image(&menu, [1024.0, 768.0], Some(0)).expect("menu").save(format!("{dir}/chat-menu.png")).expect("png");
    }
}
