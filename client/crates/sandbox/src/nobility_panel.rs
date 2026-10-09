//! The Nobility Rank, as the original client lays it out (`ini/GUI.ini`)
//! over its own art (`data/interface/Style01/checontribute`):
//!
//! * [0-307] the main window: the instructions, the Donate box and the
//!   ranks' buttons that tell what each rank still needs, the ranks' table
//!   (all in the art), Close;
//! * [0-310] beside it, the Donation Rank: the first five, and More;
//! * [0-309] the Contribution Rank More opens: the 50 seats, ten a page.
//!
//! The 50 greatest donors hold seats (King to the 3rd, Prince to the 15th,
//! Duke to the 50th); below them the donation makes an Earl (200 million),
//! a Baron (100 million) or a Knight (30 million). Each donation is at
//! least 3 million silver; without the silver, CPs at 50,000 apiece.

use crate::assets::Assets;
use crate::network::NobilityEntry;
use crate::original_ui::overlay_asset;
use image::RgbaImage;

pub const MIN_DONATION: u64 = 3_000_000;
pub const SILVER_PER_CP: u64 = 10_000;
const MAX_AMOUNT_DIGITS: usize = 10;
const PAGE_ROWS: usize = 10;

const ART: &str = "data/interface/Style01/checontribute";

/// The panel holds [0-307] at its origin and [0-310] where the original
/// puts it against [0-307] (542-100, 242-65); [0-309] alone when shown.
pub const SIZE: [u32; 2] = [442 + 222, 500];
const SIDE: [i32; 2] = [442, 177];

/// A rank's name, for a man or a woman.
pub fn rank_name(rank: u32, female: bool) -> &'static str {
    match (rank, female) {
        (12, false) => "King",
        (12, true) => "Queen",
        (9, false) => "Prince",
        (9, true) => "Princess",
        (7, false) => "Duke",
        (7, true) => "Duchess",
        (5, false) => "Earl",
        (5, true) => "Countess",
        (3, false) => "Baron",
        (3, true) => "Baroness",
        (1, false) => "Knight",
        (1, true) => "Lady",
        _ => "Commoner",
    }
}

/// A rank's art in `data/interface/Style01/nobility` (`nob_<name>32`, its
/// name `nob_<Name>Font32`): 亲王/郡主 King and Queen, 郡公 Prince, 郡侯 Duke,
/// 郡伯 Earl, 郡爵 Baron, 郡守 Knight, 白衣 the commoner.
fn rank_art(rank: u32, female: bool) -> (&'static str, &'static str) {
    match rank {
        12 if female => ("infanta", "Infanta"),
        12 => ("infante", "Infante"),
        9 => ("father", "Father"),
        7 => ("marquis", "Marquis"),
        5 => ("uncle", "Uncle"),
        3 => ("nobility", "Nobility"),
        1 => ("guard", "Guard"),
        _ => ("white", "White"),
    }
}

/// A rank's effects in the original (`Conquer.exe` 0x4b8150, 0x4c98e3,
/// `3DEffect.ini` 爵位光效): the one that stays on the noble, and the ones
/// played once when the noble comes into sight or rises to it (the fiery
/// name, `letter7` "King", and for some a crown, `coronet`).
pub fn rank_effects(rank: u32, female: bool) -> Option<(&'static str, &'static [&'static str])> {
    Some(match rank {
        12 if female => ("SynQueens", &["letter6", "coronet3"]),
        12 => ("SynKings", &["letter7", "coronet4"]),
        9 => ("InsigniaNoble06s", &["letter4"]),
        7 => ("InsigniaNoble05s", &["letter5"]),
        5 => ("InsigniaNoble04s", &["letter1"]),
        3 => ("InsigniaNoble03s", &["letter3", "coronet7"]),
        1 => ("InsigniaNoble02s", &["letter2", "coronet6"]),
        _ => return None,
    })
}

/// A rank's medal and name art at `size` (16, 32 or 64).
pub fn rank_art_paths(rank: u32, female: bool, size: u32) -> [String; 2] {
    let (art, font) = rank_art(rank, female);
    [
        format!("data/interface/Style01/nobility/nob_{art}{size}.dds"),
        format!("data/interface/Style01/nobility/nob_{font}Font{size}.dds"),
    ]
}

/// The ranks' buttons of [0-307] in their order there ([307-1604] .. [307-1573]):
/// the rank, its control and its art.
const RANK_BUTTONS: [(u32, [i32; 4], &str); 6] = [
    (12, [193, 182, 33, 20], "infante"),
    (9, [225, 182, 33, 20], "shirefather"),
    (5, [257, 182, 33, 20], "shireuncle"),
    (7, [290, 182, 33, 20], "shiremarquis"),
    (3, [323, 182, 33, 20], "shirechevalier"),
    (1, [355, 182, 33, 20], "shireguard"),
];

// [0-307]'s controls.
const AMOUNT: [i32; 4] = [101, 154, 105, 19];
const DONATE: [i32; 4] = [208, 146, 70, 25];
const RESULT: [i32; 2] = [55, 205];
const CLOSE: [i32; 4] = [188, 430, 80, 28];
const CLOSE_X: [i32; 4] = [412, 24, 26, 26];
// [0-310]'s More, and [0-309]'s page arrows, page, Close and X.
const MORE: [i32; 4] = [65, 165, 80, 28];
const PAGE_BACK: [i32; 4] = [180, 350, 25, 23];
const PAGE_ON: [i32; 4] = [240, 350, 25, 23];
const PAGE: [i32; 2] = [210, 350];
const FULL_CLOSE: [i32; 4] = [185, 447, 70, 28];
const FULL_CLOSE_X: [i32; 4] = [414, 24, 28, 28];

/// What the window shows.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct NobilityView {
    pub donation: u64,
    pub rank: u32,
    /// The hero's seat (0: none).
    pub position: u32,
    pub remaining: u64,
    pub own_name: String,
    pub female: bool,
    pub silver: u64,
    pub cps: u64,
    /// The 50 seats, in order.
    pub ranking: Vec<NobilityEntry>,
    /// The Donate box: its digits, and whether keys go to it.
    pub amount: String,
    pub amount_focused: bool,
    /// The Contribution Rank is shown, at this page.
    pub full_list: bool,
    pub page: usize,
    /// The line under the ranks' buttons (a rank's minimum, a donation's
    /// trouble).
    pub note: String,
    /// The control under the cursor, lit.
    pub hover: Option<NobilityAction>,
}

impl NobilityView {
    pub fn amount(&self) -> u64 {
        self.amount.parse().unwrap_or(0)
    }

    pub fn pages(&self) -> usize {
        self.ranking.len().div_ceil(PAGE_ROWS).max(1)
    }

    /// Silver still to give for `rank` (at least one donation's least).
    pub fn needed_for(&self, rank: u32) -> u64 {
        let last_seat = match rank {
            12 => Some(3),
            9 => Some(15),
            7 => Some(50),
            _ => None,
        };
        let target = match last_seat {
            Some(last) => self
                .ranking
                .iter()
                .filter(|entry| entry.name != self.own_name)
                .nth(last - 1)
                .map_or(0, |entry| entry.donation + 1),
            None => match rank {
                5 => 200_000_000,
                3 => 100_000_000,
                _ => 30_000_000,
            },
        };
        target.saturating_sub(self.donation).max(MIN_DONATION)
    }

    /// Types `text` into the Donate box (digits only).
    pub fn type_text(&mut self, text: &str) {
        for digit in text.chars().filter(char::is_ascii_digit) {
            if self.amount.len() < MAX_AMOUNT_DIGITS && !(self.amount.is_empty() && digit == '0') {
                self.amount.push(digit);
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NobilityAction {
    FocusAmount,
    Donate,
    /// What `rank` still needs, told and put in the box.
    Query(u32),
    /// The Contribution Rank, and back.
    More,
    Back,
    Page(usize),
    Close,
}

fn inside([x, y, width, height]: [i32; 4], point: [f32; 2]) -> bool {
    point[0] >= x as f32 && point[1] >= y as f32 && point[0] < (x + width) as f32 && point[1] < (y + height) as f32
}

fn shifted([x, y, width, height]: [i32; 4], [dx, dy]: [i32; 2]) -> [i32; 4] {
    [x + dx, y + dy, width, height]
}

pub fn action_at(view: &NobilityView, cursor: [f32; 2]) -> Option<NobilityAction> {
    if view.full_list {
        if inside(FULL_CLOSE, cursor) || inside(FULL_CLOSE_X, cursor) {
            return Some(NobilityAction::Back);
        }
        if inside(PAGE_BACK, cursor) {
            return view.page.checked_sub(1).map(NobilityAction::Page);
        }
        if inside(PAGE_ON, cursor) {
            return Some(view.page + 1).filter(|page| *page < view.pages()).map(NobilityAction::Page);
        }
        return None;
    }
    if inside(CLOSE, cursor) || inside(CLOSE_X, cursor) {
        return Some(NobilityAction::Close);
    }
    if inside(AMOUNT, cursor) {
        return Some(NobilityAction::FocusAmount);
    }
    if inside(DONATE, cursor) {
        return Some(NobilityAction::Donate);
    }
    if let Some((rank, _, _)) = RANK_BUTTONS.iter().find(|(_, rect, _)| inside(*rect, cursor)) {
        return Some(NobilityAction::Query(*rank));
    }
    if inside(shifted(MORE, SIDE), cursor) {
        return Some(NobilityAction::More);
    }
    None
}

/// A button's art: lit (`Emboss`) under the cursor.
fn button(image: &mut RgbaImage, assets: &mut Assets, name: &str, [x, y, ..]: [i32; 4], lit: bool) {
    let face = if lit { "Emboss" } else { "Normal" };
    let _ = overlay_asset(image, assets, &format!("{ART}/{name}{face}.dds"), i64::from(x), i64::from(y));
}

/// Text in the client's face, light over a dark edge as the original
/// writes on its parchment.
fn text(image: &mut RgbaImage, value: &str, [x, y]: [i32; 2], colour: [u8; 4], width: i32) {
    if let Ok(font) = crate::main_hud::font() {
        for [dx, dy] in [[-1, 0], [1, 0], [0, -1], [0, 1], [1, 1]] {
            crate::main_hud::text(image, font, value, 12.0, [x + dx, y + dy], EDGE, Some(width));
        }
        crate::main_hud::text(image, font, value, 12.0, [x, y], colour, Some(width));
    }
}

fn text_centred(image: &mut RgbaImage, value: &str, [x, y, width, _]: [i32; 4], colour: [u8; 4]) {
    if let Ok(font) = crate::main_hud::font() {
        let shown = crate::main_hud::text_width(font, value, 12.0);
        text(image, value, [x + (width - shown) / 2, y], colour, width);
    }
}

/// A rank's icon and name art, `icon` and `name` pixels square.
fn rank_marks(image: &mut RgbaImage, assets: &mut Assets, rank: u32, icon: [i32; 3], name: [i32; 3]) {
    let (art, font) = rank_art(rank, false);
    for (path, [x, y, size]) in [
        (format!("data/interface/Style01/nobility/nob_{art}32.dds"), icon),
        (format!("data/interface/Style01/nobility/nob_{font}Font32.dds"), name),
    ] {
        if let Ok(mark) = assets.image(&path) {
            let mark = if mark.width() == size as u32 {
                mark
            } else {
                image::imageops::resize(&mark, size as u32, size as u32, image::imageops::FilterType::Triangle)
            };
            image::imageops::overlay(image, &mark, i64::from(x), i64::from(y));
        }
    }
}

/// The names and numbers, and the edge around them.
const INK: [u8; 4] = [255, 248, 228, 255];
const PALE: [u8; 4] = [255, 236, 190, 255];
const EDGE: [u8; 4] = [58, 26, 8, 255];
/// Where the Donate box sits in the art (a little under [307-1565]).
const AMOUNT_TEXT: [i32; 2] = [106, 157];

/// Draws the window.
pub fn draw(image: &mut RgbaImage, assets: &mut Assets, view: Option<&NobilityView>) {
    let Some(view) = view else {
        return;
    };
    let lit = |action: NobilityAction| view.hover == Some(action);
    let money = |value: u64| crate::store_panel::thousands(value);
    if view.full_list {
        draw_full_list(image, assets, view);
        return;
    }

    // [0-307].
    let _ = overlay_asset(image, assets, &format!("{ART}/checontributeBG.dds"), 0, 0);
    let _ = overlay_asset(image, assets, &format!("{ART}/{}.dds", if lit(NobilityAction::Close) { "closeEmboss" } else { "closeNormal" }), 412, 22);
    let _ = overlay_asset(image, assets, &format!("{ART}/helpNormal.dds"), 384, 22);
    let shown = if view.amount.is_empty() { String::new() } else { money(view.amount()) };
    text(image, &shown, AMOUNT_TEXT, PALE, 96);
    if view.amount_focused
        && let Ok(font) = crate::main_hud::font()
    {
        let x = AMOUNT_TEXT[0] + 1 + crate::main_hud::text_width(font, &shown, 12.0);
        for y in AMOUNT_TEXT[1]..AMOUNT_TEXT[1] + 13 {
            if let Some(pixel) = image.get_pixel_mut_checked(x as u32, y as u32) {
                *pixel = image::Rgba(PALE);
            }
        }
    }
    button(image, assets, "contribute", DONATE, lit(NobilityAction::Donate));
    text(image, "Minimum Donation", [RESULT[0] - 8, 184], INK, 136);
    for (rank, rect, art) in RANK_BUTTONS {
        let art = if rank == 12 && view.female { "infanta" } else { art };
        button(image, assets, art, rect, lit(NobilityAction::Query(rank)));
    }
    // The band under the buttons: a query's answer, or the hero's standing.
    let note = if view.note.is_empty() {
        let place = if view.position > 0 { format!("   Ranking: {}", view.position) } else { String::new() };
        format!("Donation: {}{place}   Noble Rank: {}", money(view.donation), rank_name(view.rank, view.female))
    } else {
        view.note.clone()
    };
    text(image, &note, RESULT, PALE, 330);
    button(image, assets, "closeBtn", CLOSE, lit(NobilityAction::Close));

    // [0-310], beside it.
    let [sx, sy] = SIDE;
    let _ = overlay_asset(image, assets, &format!("{ART}/checontributeBG2.dds"), i64::from(sx), i64::from(sy));
    for (row, entry) in view.ranking.iter().take(5).enumerate() {
        let row = row as i32;
        let place_y = sy + [55, 76, 95, 115, 134][row as usize];
        text(image, &entry.position.to_string(), [sx + 22, place_y], INK, 18);
        rank_marks(image, assets, entry.rank, [sx + 42, sy + 55 + row * 20, 16], [sx + 64, sy + 48 + row * 20, 32]);
        text(image, &entry.name, [sx + 110, sy + 53 + row * 20 - row / 3], INK, 100);
    }
    button(image, assets, "more", shifted(MORE, SIDE), lit(NobilityAction::More));
}

/// [0-309]: the 50 seats, ten a page.
fn draw_full_list(image: &mut RgbaImage, assets: &mut Assets, view: &NobilityView) {
    let lit = |action: NobilityAction| view.hover == Some(action);
    let _ = overlay_asset(image, assets, &format!("{ART}/exploitbeadrollBG.dds"), 0, 0);
    let _ = overlay_asset(image, assets, &format!("{ART}/{}.dds", if lit(NobilityAction::Back) { "closeEmboss" } else { "closeNormal" }), 414, 22);
    let page = view.page.min(view.pages() - 1);
    for (row, entry) in view.ranking.iter().skip(page * PAGE_ROWS).take(PAGE_ROWS).enumerate() {
        let y = 80 + row as i32 * 24;
        let own = entry.name == view.own_name;
        let colour = if own { [176, 40, 20, 255] } else { INK };
        text_centred(image, &entry.position.to_string(), [70, y, 30, 16], colour);
        rank_marks(image, assets, entry.rank, [120, y - 3, 24], [145, y - 8, 32]);
        text(image, &entry.name, [193, y], colour, 120);
        text(image, &crate::store_panel::thousands(entry.donation), [318, y], colour, 100);
    }
    let _ = overlay_asset(image, assets, &format!("{ART}/leftarrowheadNormal.dds"), i64::from(PAGE_BACK[0]), i64::from(PAGE_BACK[1]));
    let _ = overlay_asset(image, assets, &format!("{ART}/rightarrowheadNormal.dds"), i64::from(PAGE_ON[0]), i64::from(PAGE_ON[1]));
    text_centred(image, &format!("{}/{}", page + 1, view.pages()), [PAGE[0], PAGE[1] + 3, 36, 16], INK);
    button(image, assets, "closeBtn", FULL_CLOSE, lit(NobilityAction::Back));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(position: u32, name: &str, donation: u64, rank: u32) -> NobilityEntry {
        NobilityEntry { position, character_id: position, name: name.into(), donation, rank }
    }

    fn view() -> NobilityView {
        let mut view = NobilityView {
            own_name: "Hero".into(),
            donation: 40_000_000,
            rank: 9,
            position: 5,
            silver: 900_000_000,
            ..Default::default()
        };
        let names = ["[GM]dek", "BrowserTest", "TheTester", "Isabele", "Hero", "Archetype"];
        for (index, name) in names.iter().enumerate() {
            let donation = [900, 500, 300, 100, 40, 35][index] * 1_000_000;
            view.ranking.push(entry(index as u32 + 1, name, donation, [12, 12, 12, 9, 9, 9][index]));
        }
        view
    }

    #[test]
    fn the_rank_buttons_tell_what_each_rank_needs() {
        let view = view();
        // The third King (300 million) is to be outgiven by one.
        assert_eq!(view.needed_for(12), 300_000_001 - 40_000_000);
        // Fewer than 15 others: a free seat, one donation.
        assert_eq!(view.needed_for(9), MIN_DONATION);
        assert_eq!(view.needed_for(5), 160_000_000);
        assert_eq!(view.needed_for(1), MIN_DONATION);
    }

    #[test]
    fn the_box_takes_digits_only() {
        let mut view = NobilityView::default();
        view.type_text("0a3x000000");
        assert_eq!(view.amount(), 3_000_000);
        view.type_text("99999999999");
        assert_eq!(view.amount.len(), MAX_AMOUNT_DIGITS);
    }

    #[test]
    fn clicks_find_the_originals_controls() {
        let mut view = view();
        let at = |[x, y, width, height]: [i32; 4]| [(x + width / 2) as f32, (y + height / 2) as f32];
        assert_eq!(action_at(&view, at(AMOUNT)), Some(NobilityAction::FocusAmount));
        assert_eq!(action_at(&view, at(DONATE)), Some(NobilityAction::Donate));
        assert_eq!(action_at(&view, at(RANK_BUTTONS[0].1)), Some(NobilityAction::Query(12)));
        assert_eq!(action_at(&view, at(RANK_BUTTONS[5].1)), Some(NobilityAction::Query(1)));
        assert_eq!(action_at(&view, at(CLOSE)), Some(NobilityAction::Close));
        assert_eq!(action_at(&view, at(shifted(MORE, SIDE))), Some(NobilityAction::More));
        view.full_list = true;
        // One page only: neither arrow turns it.
        assert_eq!(action_at(&view, at(PAGE_ON)), None);
        assert_eq!(action_at(&view, at(FULL_CLOSE)), Some(NobilityAction::Back));
    }

    /// `BOUNTY_PREVIEW=dir cargo test -p conquer-sandbox nobility_preview`
    #[test]
    fn nobility_preview() {
        let Ok(dir) = std::env::var("BOUNTY_PREVIEW") else { return };
        let mut assets = Assets::open(std::path::Path::new("R:/5065")).expect("assets");
        let mut view = view();
        view.amount = "260000001".into();
        view.amount_focused = true;
        view.hover = Some(NobilityAction::Donate);
        let mut image = RgbaImage::new(SIZE[0], SIZE[1]);
        draw(&mut image, &mut assets, Some(&view));
        image.save(format!("{dir}/nobility.png")).expect("png");
        view.full_list = true;
        let mut image = RgbaImage::new(SIZE[0], SIZE[1]);
        draw(&mut image, &mut assets, Some(&view));
        image.save(format!("{dir}/nobility-list.png")).expect("png");
    }
}
