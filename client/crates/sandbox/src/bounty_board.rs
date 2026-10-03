//! The Task Board of bounty tasks, drawn inside the QUESTS quick panel: the
//! points and tokens, the four difficulties, the three bounties on offer or
//! the one under way, and the talisman's bonuses.

use crate::components::{draw_bar, draw_button, draw_card, draw_tab, WidgetState};
use crate::ui::UiRect;
use crate::{labels, ui_theme};
use fontdue::Font;
use gravak_protocol::{BountyState, BountyTask};
use image::RgbaImage;

pub const DIFFICULTIES: [&str; 4] = ["Beginner", "Adept", "Expert", "Master"];
const BONUSES: [&str; 4] = ["Damage", "Life leech", "Extra loot", "Double kills"];
const MAX_TOKENS: u32 = 10;
const MAX_TALISMAN_LEVEL: u32 = 40;
/// Names a list holds, and what one costs (as the server).
const MAX_LISTED: usize = 3;
const PREFER_COST: u32 = 10;
const AVOID_COST: u32 = 5;
/// Greenish for preferred monsters.
const PREFERRED: [u8; 3] = [150, 220, 140];

/// The board as the server sent it, and the difficulty tab chosen here:
/// browsing the tabs is free, and a reroll brings a list of the chosen one.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct BountyView {
    pub board: BountyState,
    /// The tab chosen on this screen; the board's own difficulty until then.
    pub selected: Option<u32>,
}

impl BountyView {
    pub fn selected(&self) -> u32 {
        self.selected.unwrap_or(self.board.difficulty)
    }
}

/// What a click on the board asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BountyAction {
    ClaimDaily,
    /// Shows a difficulty's tab; nothing is spent.
    Select(u32),
    /// A new list of this difficulty, for a token.
    Reroll(u32),
    Pick(u32),
    /// Drops the bounty under way, for a token.
    Abandon,
    Upgrade(u32),
    /// Puts an offer's monster (by type) on the preferred list.
    Prefer(u32),
    /// Puts an offer's monster (by type) on the avoided list.
    Avoid(u32),
    /// Takes the `index`th name off a list.
    Unlist { avoided: bool, index: u32 },
}

const DAILY: UiRect = UiRect::new(372, 50, 128, 22);
const REROLL: UiRect = UiRect::new(404, 84, 96, 22);
const ABANDON: UiRect = UiRect::new(352, 222, 136, 24);

/// Where the talisman rows start, and their spacing.
const TALISMAN_TOP: i32 = 284;
const TALISMAN_ROW: i32 = 21;
/// The preferred list's row, and the avoided one's under it.
const LISTS_TOP: i32 = 374;
const LISTS_ROW: i32 = 20;

fn difficulty_tab(index: usize) -> UiRect {
    UiRect::new(20 + index as i32 * 96, 84, 92, 22)
}

fn offer_card(slot: usize) -> UiRect {
    UiRect::new(20 + slot as i32 * 162, 116, 156, 140)
}

fn hunt_button(slot: usize) -> UiRect {
    let card = offer_card(slot);
    UiRect::new(card.x + 28, card.y + 108, 100, 24)
}

/// An offer's Prefer (0) or Avoid (1) button.
fn list_button(slot: usize, avoided: bool) -> UiRect {
    let card = offer_card(slot);
    UiRect::new(card.x + 98, card.y + 48 + if avoided { 22 } else { 0 }, 50, 18)
}

/// A name's chip on a list's row.
fn list_chip(avoided: bool, index: usize) -> UiRect {
    UiRect::new(
        96 + index as i32 * 136,
        LISTS_TOP + if avoided { LISTS_ROW } else { 0 },
        130,
        17,
    )
}

/// Whether `name` may still go on a list.
fn can_list(board: &BountyState, name: &str, avoided: bool) -> bool {
    let cost = if avoided { AVOID_COST } else { PREFER_COST };
    !board.listed(false).contains(&name)
        && !board.listed(true).contains(&name)
        && board.listed(avoided).len() < MAX_LISTED
        && board.points >= cost
}

/// `name`, cut to fit a list chip.
fn short(name: &str) -> String {
    if name.chars().count() <= 15 {
        name.to_owned()
    } else {
        format!("{}..", name.chars().take(13).collect::<String>())
    }
}

fn upgrade_button(bonus: usize) -> UiRect {
    UiRect::new(372, TALISMAN_TOP + bonus as i32 * TALISMAN_ROW, 128, 20)
}

/// The talisman bonus at `level`, in hundredths of a percent (as the server).
pub fn bonus_basis_points(bonus: usize, level: u32) -> u32 {
    let level = level.min(MAX_TALISMAN_LEVEL);
    let (step, after) = if bonus == 3 { (100, 50) } else { (50, 25) };
    level.min(20) * step + level.saturating_sub(20) * after
}

/// Bounty points the next level costs.
pub fn upgrade_cost(level: u32) -> u32 {
    5 + 12 * level
}

fn grade(task: &BountyTask) -> (&'static str, [u8; 3]) {
    match task.grade {
        1 => ("Silver", [205, 214, 228]),
        2 => ("Gold", [255, 202, 64]),
        _ => ("Normal", ui_theme::TEXT_MUTED),
    }
}

/// `1234567` as `1.234.567`.
fn thousands(value: u64) -> String {
    let digits = value.to_string();
    let mut out = String::new();
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index) % 3 == 0 {
            out.push('.');
        }
        out.push(digit);
    }
    out
}

fn percent(basis_points: u32) -> String {
    let whole = basis_points / 100;
    match basis_points % 100 {
        0 => format!("{whole}%"),
        part if part % 10 == 0 => format!("{whole},{}%", part / 10),
        part => format!("{whole},{part:02}%"),
    }
}

fn say(image: &mut RgbaImage, font: &Font, text: &str, colour: [u8; 3], x: i32, y: i32) {
    labels::text(image, font, &[(text, colour)], [x as f32, y as f32]);
}

fn button(image: &mut RgbaImage, font: &Font, rect: UiRect, label: &str, enabled: bool) {
    let state = if enabled { WidgetState::Normal } else { WidgetState::Disabled };
    draw_button(image, font, rect, label, state);
}

/// Draws the board onto the quick panel's contents.
pub fn draw(image: &mut RgbaImage, font: &Font, view: Option<&BountyView>) {
    let Some(view) = view else {
        say(image, font, "Bounty Tasks", ui_theme::TEXT_GOLD, 24, 56);
        say(image, font, "Waiting for the server...", ui_theme::TEXT_MUTED, 24, 84);
        return;
    };
    let board = &view.board;
    let header = format!(
        "Bounty points: {}    Tokens: {}/{MAX_TOKENS}    Done: {}",
        board.points, board.tokens, board.completed
    );
    say(image, font, &header, ui_theme::TEXT_GOLD, 22, 54);
    button(image, font, DAILY, "Daily token", board.daily_available != 0);

    let selected = view.selected() as usize;
    for (index, name) in DIFFICULTIES.iter().enumerate() {
        draw_tab(image, font, difficulty_tab(index), name, index == selected, false);
    }
    button(image, font, REROLL, "Reroll (1)", board.tokens > 0);
    let note = if board.tokens == 0 {
        "No reroll tokens: take the daily one, or finish a bounty.".to_owned()
    } else if selected != board.difficulty as usize {
        format!("Reroll for a {} list (1 token).", DIFFICULTIES[selected])
    } else {
        String::new()
    };
    if !note.is_empty() {
        labels::text_sized(image, font, &[(note.as_str(), ui_theme::TEXT_MUTED)], [22.0, 69.0], 11.0);
    }

    if let Some(task) = board.active() {
        draw_card(image, UiRect::new(20, 116, 480, 140), WidgetState::Selected);
        let (grade_name, grade_colour) = grade(task);
        say(image, font, &format!("Hunting: {}", task.monster_name), ui_theme::TEXT_GOLD, 36, 126);
        say(image, font, grade_name, grade_colour, 400, 126);
        draw_bar(
            image,
            UiRect::new(36, 156, 448, 16),
            task.kills_done,
            task.kills,
            [176, 130, 48, 255],
            [20, 18, 14, 255],
            [120, 88, 40, 255],
        );
        say(
            image,
            font,
            &format!("{} / {} defeated", task.kills_done, task.kills),
            ui_theme::TEXT,
            36,
            178,
        );
        say(
            image,
            font,
            &format!(
                "Reward: {} exp, {} points and a token",
                thousands(task.experience),
                task.points
            ),
            ui_theme::TEXT_MUTED,
            36,
            200,
        );
        button(image, font, ABANDON, "Abandon (1 token)", board.tokens > 0);
    } else {
        let offers = board.offers();
        if offers.is_empty() {
            say(image, font, "No bounties on offer. Reroll for a new list.", ui_theme::TEXT_MUTED, 24, 130);
        }
        for (slot, task) in offers.into_iter().enumerate() {
            let card = offer_card(slot);
            draw_card(image, card, WidgetState::Normal);
            let (grade_name, grade_colour) = grade(task);
            let x = card.x + 10;
            let name_colour = if board.listed(false).contains(&task.monster_name.as_str()) {
                PREFERRED
            } else {
                ui_theme::TEXT_GOLD
            };
            say(image, font, &task.monster_name, name_colour, x, card.y + 8);
            for avoided in [false, true] {
                let label = if avoided { "Avoid" } else { "Prefer" };
                button(image, font, list_button(slot, avoided), label, can_list(board, &task.monster_name, avoided));
            }
            say(image, font, grade_name, grade_colour, x, card.y + 28);
            say(image, font, &format!("Defeat {}", task.kills), ui_theme::TEXT, x, card.y + 50);
            say(image, font, &format!("{} points", task.points), ui_theme::TEXT_MUTED, x, card.y + 70);
            say(
                image,
                font,
                &format!("{} exp", thousands(task.experience)),
                ui_theme::TEXT_MUTED,
                x,
                card.y + 90,
            );
            button(image, font, hunt_button(slot), "Hunt", true);
        }
    }

    say(image, font, "Talisman - against your bounty's monster", ui_theme::TEXT_GOLD, 22, 264);
    for (index, (name, level)) in BONUSES.iter().zip(board.talisman()).enumerate() {
        let y = TALISMAN_TOP + index as i32 * TALISMAN_ROW;
        let value = percent(bonus_basis_points(index, level));
        let line = match index {
            2 => format!("{name}: {value} chance"),
            3 => format!("{name}: {value} chance"),
            _ => format!("{name}: +{value}"),
        };
        say(image, font, &line, ui_theme::TEXT, 24, y + 2);
        say(image, font, &format!("Lv {level}"), ui_theme::TEXT_MUTED, 300, y + 2);
        if level >= MAX_TALISMAN_LEVEL {
            button(image, font, upgrade_button(index), "Max level", false);
        } else {
            let cost = upgrade_cost(level);
            button(image, font, upgrade_button(index), &format!("Upgrade ({cost})"), board.points >= cost);
        }
    }

    for avoided in [false, true] {
        let y = LISTS_TOP + if avoided { LISTS_ROW } else { 0 };
        let (title, cost) = if avoided { ("Avoided", AVOID_COST) } else { ("Preferred", PREFER_COST) };
        let colour = if avoided { [220, 140, 130] } else { PREFERRED };
        say(image, font, title, colour, 22, y);
        let names = board.listed(avoided);
        for index in 0..MAX_LISTED {
            let chip = list_chip(avoided, index);
            match names.get(index) {
                Some(name) => {
                    draw_card(image, chip, WidgetState::Normal);
                    let line = format!("{}  x", short(name));
                    labels::text_sized(
                        image,
                        font,
                        &[(line.as_str(), ui_theme::TEXT)],
                        [chip.x as f32 + 6.0, chip.y as f32 + 1.0],
                        11.0,
                    );
                }
                None => labels::text_sized(
                    image,
                    font,
                    &[(format!("- free ({cost} pts) -").as_str(), ui_theme::TEXT_MUTED)],
                    [chip.x as f32 + 6.0, chip.y as f32 + 1.0],
                    11.0,
                ),
            }
        }
    }
}

/// The action under `cursor`, in the panel's own coordinates.
pub fn action_at(view: &BountyView, cursor: [f32; 2]) -> Option<BountyAction> {
    let board = &view.board;
    if DAILY.contains_local(cursor) && board.daily_available != 0 {
        return Some(BountyAction::ClaimDaily);
    }
    if let Some(index) = (0..4).find(|index| difficulty_tab(*index).contains_local(cursor)) {
        return Some(BountyAction::Select(index as u32));
    }
    if REROLL.contains_local(cursor) && board.tokens > 0 {
        return Some(BountyAction::Reroll(view.selected()));
    }
    if board.active().is_some() {
        if ABANDON.contains_local(cursor) && board.tokens > 0 {
            return Some(BountyAction::Abandon);
        }
    } else {
        let offers = board.offers();
        if let Some(slot) = (0..offers.len()).find(|slot| hunt_button(*slot).contains_local(cursor)) {
            return Some(BountyAction::Pick(slot as u32));
        }
        for (slot, offer) in offers.iter().enumerate() {
            for avoided in [false, true] {
                if list_button(slot, avoided).contains_local(cursor) && can_list(board, &offer.monster_name, avoided) {
                    return Some(if avoided {
                        BountyAction::Avoid(offer.monster_type)
                    } else {
                        BountyAction::Prefer(offer.monster_type)
                    });
                }
            }
        }
    }
    for avoided in [false, true] {
        let count = board.listed(avoided).len();
        if let Some(index) = (0..count).find(|index| list_chip(avoided, *index).contains_local(cursor)) {
            return Some(BountyAction::Unlist { avoided, index: index as u32 });
        }
    }
    (0..4)
        .find(|bonus| upgrade_button(*bonus).contains_local(cursor))
        .filter(|bonus| {
            let level = board.talisman()[*bonus];
            level < MAX_TALISMAN_LEVEL && board.points >= upgrade_cost(level)
        })
        .map(|bonus| BountyAction::Upgrade(bonus as u32))
}

/// What the tracker under the minimap shows.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Tracker {
    /// The bounty under way: monster, kills done and needed, grade.
    pub hunt: Option<(String, u32, u32, u32)>,
    pub daily_available: bool,
}

/// The tracker for a board: always there, so the board is one click away.
pub fn tracker(board: &BountyState) -> Tracker {
    Tracker {
        hunt: board
            .active()
            .map(|task| (task.monster_name.clone(), task.kills_done, task.kills, task.grade)),
        daily_available: board.daily_available != 0,
    }
}

/// Size of the tracker during a hunt, and of the icon without one.
pub const TRACKER_SIZE: [u32; 2] = [206, 52];
pub const ICON_SIZE: [u32; 2] = [52, 52];

/// Without a hunt, the way to the board is a square icon: the emblem, and a
/// gold badge while the daily token waits.
fn icon_image(assets: &mut crate::assets::Assets, daily: bool, hovered: bool) -> crate::assets::Result<RgbaImage> {
    let [width, height] = ICON_SIZE;
    let gold = [176, 158, 104];
    let frame = if hovered { [226, 208, 154] } else { gold };
    let mut image = crate::hud::hud_style_panel(assets, width, height, frame);
    draw_emblem(&mut image, [width as i32 / 2, height as i32 / 2], gold, hovered);
    if daily {
        // A small badge on the top right corner.
        let (cx, cy) = (width as i32 - 11, 11_i32);
        for dy in -5_i32..=5 {
            for dx in -5_i32..=5 {
                let distance = dx * dx + dy * dy;
                if distance <= 25 {
                    let colour = if distance >= 17 { [90, 52, 8, 255] } else { [255, 202, 64, 255] };
                    image.put_pixel((cx + dx) as u32, (cy + dy) as u32, image::Rgba(colour));
                }
            }
        }
    }
    Ok(image)
}

/// The words shown beside the icon under the cursor.
pub fn icon_tooltip(tracker: &Tracker) -> &'static str {
    if tracker.daily_available {
        "Bounty Tasks (B) - daily token ready!"
    } else {
        "Bounty Tasks (B)"
    }
}

/// The bounty emblem: a gold ring with sight marks around the gold diamond
/// that marks the bounty's monsters.
fn draw_emblem(image: &mut RgbaImage, centre: [i32; 2], gold: [u8; 3], glow: bool) {
    let mut put = |x: i32, y: i32, colour: [u8; 4]| {
        if x >= 0 && y >= 0 && (x as u32) < image.width() && (y as u32) < image.height() {
            let under = image.get_pixel(x as u32, y as u32).0;
            let alpha = u16::from(colour[3]);
            let mix = |a: u8, b: u8| ((u16::from(a) * alpha + u16::from(b) * (255 - alpha)) / 255) as u8;
            image.put_pixel(x as u32, y as u32, image::Rgba([mix(colour[0], under[0]), mix(colour[1], under[1]), mix(colour[2], under[2]), 255]));
        }
    };
    let [cx, cy] = centre;
    let ring = [gold[0], gold[1], gold[2], 255];
    let dark = [30, 22, 10, 255];
    // A soft disc behind, brighter when hovered.
    for dy in -15_i32..=15 {
        for dx in -15_i32..=15 {
            let distance = ((dx * dx + dy * dy) as f32).sqrt();
            if distance <= 14.0 {
                let alpha = if glow { 90 } else { 55 };
                put(cx + dx, cy + dy, [8, 20, 26, alpha]);
            }
        }
    }
    // The ring, two pixels wide, with a dark edge.
    for step in 0..360 {
        let angle = (step as f32).to_radians();
        for (radius, colour) in [(13.0_f32, dark), (12.0, ring), (11.0, ring)] {
            put(cx + (angle.cos() * radius).round() as i32, cy + (angle.sin() * radius).round() as i32, colour);
        }
    }
    // Sight marks through the ring.
    for offset in 9..=15 {
        for (x, y) in [(cx + offset, cy), (cx - offset, cy), (cx, cy + offset), (cx, cy - offset)] {
            put(x, y, ring);
        }
    }
    // The diamond.
    for dy in -6_i32..=6 {
        for dx in -(6 - dy.abs())..=(6 - dy.abs()) {
            let edge = dx.abs() + dy.abs() >= 5;
            put(cx + dx, cy + dy, if edge { [120, 82, 20, 255] } else { [255, 214, 96, 255] });
        }
    }
}

/// The tracker under the minimap, in the HUD's style: the emblem, then the
/// hunt (monster, count and a progress bar) or the way to the board.
pub fn tracker_image(assets: &mut crate::assets::Assets, tracker: &Tracker, hovered: bool) -> crate::assets::Result<RgbaImage> {
    if tracker.hunt.is_none() {
        return icon_image(assets, tracker.daily_available, hovered);
    }
    let [width, height] = TRACKER_SIZE;
    let grade_colour: [u8; 3] = match tracker.hunt.as_ref().map(|hunt| hunt.3) {
        Some(1) => [205, 214, 228],
        Some(2) => [255, 202, 64],
        _ => [176, 158, 104],
    };
    let frame = if hovered {
        [grade_colour[0].saturating_add(50), grade_colour[1].saturating_add(50), grade_colour[2].saturating_add(50)]
    } else {
        grade_colour
    };
    let mut image = crate::hud::hud_style_panel(assets, width, height, frame);
    draw_emblem(&mut image, [26, height as i32 / 2], grade_colour, hovered);
    let cinzel = labels::decorative_font()?;
    let ui = labels::font()?;
    let shadow = [12, 22, 28];
    let teal = [150, 214, 216];
    let write = |image: &mut RgbaImage, font: &Font, text: &str, colour: [u8; 3], x: f32, y: f32, size: f32| {
        labels::text_sized(image, font, &[(text, shadow)], [x + 1.0, y + 1.0], size);
        labels::text_sized(image, font, &[(text, colour)], [x, y], size);
    };
    let width_of = |font: &Font, text: &str, size: f32| text.chars().map(|c| font.metrics(c, size).advance_width).sum::<f32>();
    let left = 48.0;
    let right = width as f32 - 12.0;
    match &tracker.hunt {
        Some((monster, done, kills, grade)) => {
            let count = format!("{done}/{kills}");
            let count_width = width_of(&ui, &count, 12.0);
            let mut size = 14.0;
            while size > 10.0 && width_of(&cinzel, monster, size) > right - left - count_width - 8.0 {
                size -= 0.5;
            }
            let name_colour = if *grade > 0 { grade_colour } else { teal };
            write(&mut image, &cinzel, monster, name_colour, left, 9.0, size);
            write(&mut image, &ui, &count, ui_theme::TEXT, right - count_width, 11.0, 12.0);
            draw_bar(
                &mut image,
                UiRect::new(left as i32, 31, (right - left) as i32, 9),
                *done,
                *kills,
                [196, 150, 58, 255],
                [10, 18, 22, 255],
                [96, 84, 52, 255],
            );
        },
        None => {
            write(&mut image, &cinzel, "BOUNTY TASKS", teal, left, 9.0, 14.0);
            let (hint, colour) = if tracker.daily_available {
                ("Daily token ready!", ui_theme::TEXT_GOLD)
            } else {
                ("Press B to choose a hunt", ui_theme::TEXT_MUTED)
            };
            write(&mut image, &ui, hint, colour, left, 29.0, 11.0);
        },
    }
    Ok(image)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn board() -> BountyView {
        BountyView { selected: None, board: BountyState {
            points: 20,
            tokens: 2,
            daily_available: 1,
            offer_count: 3,
            offer0: BountyTask { monster_type: 1, monster_name: "Pheasant".into(), ..Default::default() },
            offer1: BountyTask { monster_type: 2, monster_name: "Turtledove".into(), ..Default::default() },
            offer2: BountyTask { monster_type: 3, monster_name: "Robin".into(), ..Default::default() },
            ..Default::default()
        } }
    }

    fn centre(rect: UiRect) -> [f32; 2] {
        [rect.x as f32 + rect.width as f32 / 2.0, rect.y as f32 + rect.height as f32 / 2.0]
    }

    #[test]
    fn clicks_find_their_actions() {
        let board = board();
        assert_eq!(action_at(&board, centre(DAILY)), Some(BountyAction::ClaimDaily));
        assert_eq!(action_at(&board, centre(difficulty_tab(3))), Some(BountyAction::Select(3)));
        assert_eq!(action_at(&board, centre(REROLL)), Some(BountyAction::Reroll(0)));
        let mut chosen = board.clone();
        chosen.selected = Some(2);
        assert_eq!(action_at(&chosen, centre(REROLL)), Some(BountyAction::Reroll(2)), "the chosen tab");
        assert_eq!(action_at(&board, centre(hunt_button(2))), Some(BountyAction::Pick(2)));
        assert_eq!(action_at(&board, centre(upgrade_button(1))), Some(BountyAction::Upgrade(1)));
        assert_eq!(action_at(&board, [5.0, 5.0]), None);

        let mut poor = board.clone();
        poor.board.tokens = 0;
        poor.board.points = 4;
        assert_eq!(action_at(&poor, centre(REROLL)), None, "no tokens, no reroll");
        assert_eq!(action_at(&poor, centre(difficulty_tab(1))), Some(BountyAction::Select(1)), "tabs stay free");
        assert_eq!(action_at(&poor, centre(upgrade_button(0))), None, "not enough points");

        let mut hunting = board;
        hunting.board.task = BountyTask { monster_type: 3, kills: 150, kills_done: 37, monster_name: "Robin".into(), ..Default::default() };
        assert_eq!(action_at(&hunting, centre(ABANDON)), Some(BountyAction::Abandon));
        assert_eq!(action_at(&hunting, centre(hunt_button(0))), None, "no offers while hunting");
        assert_eq!(tracker(&hunting.board).hunt, Some(("Robin".to_owned(), 37, 150, 0)));
        assert_eq!(tracker(&super::tests::board().board).hunt, None);
    }

    #[test]
    fn numbers_read_like_the_game() {
        assert_eq!(thousands(1_234_567), "1.234.567");
        assert_eq!(thousands(950), "950");
        assert_eq!(percent(1_000), "10%");
        assert_eq!(percent(1_050), "10,5%");
        assert_eq!(percent(1_025), "10,25%");
        assert_eq!(bonus_basis_points(3, 21), 2_050);
    }
}

#[cfg(test)]
mod preview {
    /// `BOUNTY_PREVIEW=dir cargo test -p conquer-sandbox bounty_preview` writes
    /// the board as PNGs for a look.
    #[test]
    fn bounty_preview() {
        let Ok(dir) = std::env::var("BOUNTY_PREVIEW") else { return };
        use gravak_protocol::{BountyState, BountyTask};
        let font = crate::labels::font().expect("font");
        let offers = BountyState {
            points: 23,
            tokens: 4,
            daily_available: 1,
            completed: 2,
            damage_level: 3,
            leech_level: 1,
            offer_count: 3,
            offer0: BountyTask { monster_type: 1, kills: 74, grade: 0, points: 3, experience: 4_884, monster_name: "Pheasant".into(), ..Default::default() },
            offer1: BountyTask { monster_type: 3, kills: 158, grade: 1, points: 5, experience: 61_620, monster_name: "Robin".into(), ..Default::default() },
            offer2: BountyTask { monster_type: 4, kills: 196, grade: 2, points: 8, experience: 235_200, monster_name: "Apparition".into(), ..Default::default() },
            ..Default::default()
        };
        let mut hunting = offers.clone();
        hunting.offer_count = 0;
        hunting.task = BountyTask { monster_type: 3, kills: 158, kills_done: 61, grade: 1, points: 5, experience: 61_620, monster_name: "Robin".into() };
        for (name, board) in [("offers", offers), ("hunting", hunting)] {
            let mut content = image::RgbaImage::new(520, 420);
            let view = super::BountyView { selected: Some(3), board: board.clone() };
            super::draw(&mut content, &font, Some(&view));
            // In the game's own frame, like the quick panel.
            let mut assets = crate::assets::Assets::open(std::path::Path::new("R:/5065")).expect("assets");
            let mut image = crate::original_panels::framed_window(&mut assets, 520, 420).expect("frame");
            image::imageops::overlay(&mut image, &content, 0, 0);
            image.save(format!("{dir}/bounty-{name}.png")).expect("png");
            let mut assets = crate::assets::Assets::open(std::path::Path::new("R:/5065")).expect("assets");
            for daily in [false, true] {
                super::icon_image(&mut assets, daily, false)
                    .expect("icon")
                    .save(format!("{dir}/bounty-icon-{daily}.png"))
                    .expect("png");
            }
            for (hover, suffix) in [(false, ""), (true, "-hover")] {
                super::tracker_image(&mut assets, &super::tracker(&board), hover)
                    .expect("tracker")
                    .save(format!("{dir}/bounty-tracker-{name}{suffix}.png"))
                    .expect("png");
            }
        }
        crate::hud::revive_countdown_image(14).expect("countdown").save(format!("{dir}/revive-14.png")).expect("png");
        let mut assets = crate::assets::Assets::open(std::path::Path::new("R:/5065")).expect("assets");
        crate::hud::hud_style_button(&mut assets, "Revive").expect("button").save(format!("{dir}/revive-0.png")).expect("png");
        crate::hud::hud_style_button(&mut assets, "Revive here").expect("button").save(format!("{dir}/revive-here.png")).expect("png");
        crate::labels::monster_image("Robin", 70, 100, true).expect("label").save(format!("{dir}/bounty-label.png")).expect("png");
    }
}
