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
}

const DAILY: UiRect = UiRect::new(372, 50, 128, 22);
const REROLL: UiRect = UiRect::new(404, 84, 96, 22);
const ABANDON: UiRect = UiRect::new(352, 222, 136, 24);

/// Where the talisman rows start, and their spacing.
const TALISMAN_TOP: i32 = 286;
const TALISMAN_ROW: i32 = 23;

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
            say(image, font, &task.monster_name, ui_theme::TEXT_GOLD, x, card.y + 8);
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
    } else if let Some(slot) = (0..board.offers().len()).find(|slot| hunt_button(*slot).contains_local(cursor)) {
        return Some(BountyAction::Pick(slot as u32));
    }
    (0..4)
        .find(|bonus| upgrade_button(*bonus).contains_local(cursor))
        .filter(|bonus| {
            let level = board.talisman()[*bonus];
            level < MAX_TALISMAN_LEVEL && board.points >= upgrade_cost(level)
        })
        .map(|bonus| BountyAction::Upgrade(bonus as u32))
}

/// The kill counter under the minimap.
pub fn tracker_image(line: &str) -> crate::assets::Result<RgbaImage> {
    let font = labels::font()?;
    let size = 14.0;
    let text_width: f32 = line.chars().map(|character| font.metrics(character, size).advance_width).sum();
    let width = text_width.ceil() as u32 + 20;
    let mut image = RgbaImage::from_pixel(width, 24, image::Rgba([10, 14, 18, 190]));
    crate::components::outline(&mut image, UiRect::new(0, 0, width as i32, 24), [176, 130, 48, 255]);
    labels::text_sized(&mut image, &font, &[(line, ui_theme::TEXT_GOLD)], [10.0, 4.0], size);
    Ok(image)
}

/// The line under the minimap, which opens the board when clicked: the kill
/// counter during a bounty (`Bounty: Robin 37/150`), else its name and key.
pub fn tracker_line(board: &BountyState) -> Option<String> {
    Some(match board.active() {
        Some(task) => format!("Bounty: {} {}/{}", task.monster_name, task.kills_done, task.kills),
        None => "Bounty Tasks (B)".to_owned(),
    })
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
        assert_eq!(tracker_line(&hunting.board).as_deref(), Some("Bounty: Robin 37/150"));
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
            if let Some(line) = super::tracker_line(&board) {
                super::tracker_image(&line).expect("tracker").save(format!("{dir}/bounty-tracker.png")).expect("png");
            }
        }
        crate::hud::revive_image(14).expect("countdown").save(format!("{dir}/revive-14.png")).expect("png");
        crate::hud::revive_image(0).expect("button").save(format!("{dir}/revive-0.png")).expect("png");
        crate::labels::monster_image("Robin", 70, 100, true).expect("label").save(format!("{dir}/bounty-label.png")).expect("png");
    }
}
