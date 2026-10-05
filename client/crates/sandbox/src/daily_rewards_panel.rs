//! The Reward Wall, after Tibia's: opened at the Gravak statue, it shows
//! the seven days of the cycle with their rewards, the days already claimed
//! and today's, whose choices the hero picks one of before claiming. The
//! streak along the top. Drawn inside a quick panel's frame.

use crate::assets::Assets;
use crate::components::{draw_button, draw_card, WidgetState};
use crate::ui::UiRect;
use crate::{labels, ui_theme};
use fontdue::Font;
use gravak_protocol::{DailyRewardChoice, DailyRewardState};
use image::RgbaImage;

/// Days in the cycle.
pub const CYCLE: u32 = 7;

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct DailyRewardsView {
    pub choices: Vec<DailyRewardChoice>,
    pub state: Option<DailyRewardState>,
    /// Which of today's choices is picked.
    pub selected: u32,
}

impl DailyRewardsView {
    /// Adds a choice as it arrives; index 0 starts the cycle over.
    pub fn receive(&mut self, choice: DailyRewardChoice) {
        if choice.index == 0 {
            self.choices.clear();
        }
        self.choices.push(choice);
    }

    /// The choices of one day of the cycle.
    pub fn day(&self, day: u32) -> Vec<&DailyRewardChoice> {
        self.choices.iter().filter(|choice| choice.day == day).collect()
    }

    /// Whether `day` of the current cycle was claimed already.
    pub fn claimed(&self, day: u32) -> bool {
        self.state.as_ref().is_some_and(|state| {
            day < state.day && state.streak > 0 || day == state.day && state.claimed != 0
        })
    }

    pub fn can_claim(&self) -> bool {
        self.state.as_ref().is_some_and(|state| state.claimed == 0)
    }
}

/// What a click on the Reward Wall asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DailyRewardAction {
    /// Picks today's choice.
    Choose(u32),
    /// Claims the picked choice.
    Claim(u32),
}

const DAY_LEFT: i32 = 18;
const DAY_TOP: i32 = 76;
const DAY_WIDTH: i32 = 64;
const DAY_GAP: i32 = 6;
const DAY_HEIGHT: i32 = 150;
const TODAY_TOP: i32 = 244;
const CHOICE_WIDTH: i32 = 150;
const CHOICE_HEIGHT: i32 = 60;
const CLAIM: UiRect = UiRect::new(190, 344, 140, 26);

fn day_card(day: u32) -> UiRect {
    UiRect::new(DAY_LEFT + (day as i32 - 1) * (DAY_WIDTH + DAY_GAP), DAY_TOP, DAY_WIDTH, DAY_HEIGHT)
}

/// Today's choices, centred in a row.
fn choice_card(index: usize, count: usize) -> UiRect {
    let count = count.max(1) as i32;
    let width = count * CHOICE_WIDTH + (count - 1) * 10;
    let left = (520 - width) / 2;
    UiRect::new(left + index as i32 * (CHOICE_WIDTH + 10), TODAY_TOP + 28, CHOICE_WIDTH, CHOICE_HEIGHT)
}

fn width_of(font: &Font, text: &str, size: f32) -> f32 {
    text.chars().map(|c| font.metrics(c, size).advance_width).sum()
}

fn say(image: &mut RgbaImage, font: &Font, text: &str, colour: [u8; 3], x: i32, y: i32, size: f32) {
    labels::text_sized(image, font, &[(text, [10, 18, 22])], [x as f32 + 1.0, y as f32 + 1.0], size);
    labels::text_sized(image, font, &[(text, colour)], [x as f32, y as f32], size);
}

fn centred(image: &mut RgbaImage, font: &Font, text: &str, colour: [u8; 3], centre_x: i32, y: i32, size: f32) {
    let x = centre_x - width_of(font, text, size) as i32 / 2;
    say(image, font, text, colour, x, y, size);
}

/// An item's icon at `size` pixels, its amount in the corner.
fn draw_reward(
    image: &mut RgbaImage,
    assets: &mut Assets,
    ui: &Font,
    choice: &DailyRewardChoice,
    at: [i32; 2],
    size: u32,
    dim: bool,
    amount: bool,
) {
    if let Ok(icon) = crate::shop::item_icon_image(assets, choice.item_type) {
        let mut icon = image::imageops::resize(&icon, size, size, image::imageops::FilterType::Triangle);
        if dim {
            for pixel in icon.pixels_mut() {
                let grey = ((u16::from(pixel[0]) + u16::from(pixel[1]) + u16::from(pixel[2])) / 5) as u8;
                *pixel = image::Rgba([grey, grey, grey, pixel[3]]);
            }
        }
        image::imageops::overlay(image, &icon, i64::from(at[0]), i64::from(at[1]));
    }
    if amount && choice.amount > 1 {
        let amount = format!("x{}", choice.amount);
        let x = at[0] + size as i32 - width_of(ui, &amount, 11.0) as i32;
        say(image, ui, &amount, [255, 255, 255], x, at[1] + size as i32 - 12, 11.0);
    }
}

/// "5h 12m" until `then` (seconds since 1970).
fn until(then: u64, now: u64) -> String {
    let left = then.saturating_sub(now);
    let (hours, minutes) = (left / 3_600, left / 60 % 60);
    if hours > 0 { format!("{hours}h {minutes:02}m") } else { format!("{minutes}m") }
}

/// Draws the Reward Wall onto the quick panel's contents.
pub fn draw(image: &mut RgbaImage, assets: &mut Assets, view: Option<&DailyRewardsView>) {
    let (Ok(ui), Ok(cinzel)) = (labels::font(), labels::decorative_font()) else {
        return;
    };
    let Some((view, state)) = view.and_then(|view| Some((view, view.state.as_ref()?))) else {
        say(image, &ui, "Opening the Reward Wall...", ui_theme::TEXT_MUTED, 24, 56, 14.0);
        return;
    };
    let now = crate::store_panel::now_secs();

    // The streak and when the next day comes.
    let streak = match state.streak {
        0 => "No streak yet: claim today to start one!".to_owned(),
        1 => "Streak: 1 day".to_owned(),
        days => format!("Streak: {days} days in a row"),
    };
    say(image, &ui, &streak, ui_theme::TEXT_GOLD, 20, 46, 14.0);
    let next = if state.claimed != 0 {
        format!("Next reward in {}", until(state.next_day_at, now))
    } else {
        format!("Today ends in {}", until(state.next_day_at, now))
    };
    say(image, &ui, &next, ui_theme::TEXT_MUTED, 500 - width_of(&ui, &next, 12.0) as i32, 48, 12.0);

    // The seven days: claimed ones dim with a tick, today's in gold.
    for day in 1..=CYCLE {
        let rect = day_card(day);
        let today = day == state.day;
        let claimed = view.claimed(day);
        let look = if today { WidgetState::Selected } else { WidgetState::Normal };
        draw_card(image, rect, look);
        let colour = if today { ui_theme::TEXT_GOLD } else if claimed { ui_theme::TEXT_MUTED } else { ui_theme::TEXT };
        centred(image, &cinzel, &format!("Day {day}"), colour, rect.x + rect.width / 2, rect.y + 6, 12.0);
        let choices = view.day(day);
        // One choice in the middle; two one above the other, "or" between.
        let rows = choices.len().min(2) as i32;
        for (row, choice) in choices.iter().take(2).enumerate() {
            let top = if rows == 1 { rect.y + 46 } else { rect.y + 26 + row as i32 * 48 };
            draw_reward(image, assets, &ui, choice, [rect.x + (rect.width - 34) / 2, top], 34, claimed, true);
        }
        if rows > 1 {
            centred(image, &ui, "or", ui_theme::TEXT_MUTED, rect.x + rect.width / 2, rect.y + 61, 10.0);
        }
        if claimed {
            centred(image, &ui, "Claimed", [130, 200, 120], rect.x + rect.width / 2, rect.y + rect.height - 20, 11.0);
        } else if today {
            centred(image, &ui, "Today", ui_theme::TEXT_GOLD, rect.x + rect.width / 2, rect.y + rect.height - 20, 11.0);
        }
    }

    // Today's choices and the Claim button.
    let today = view.day(state.day);
    let heading = if state.claimed != 0 {
        "Today's reward is yours. Come back tomorrow for the next!"
    } else if today.len() > 1 {
        "Today's reward: choose one"
    } else {
        "Today's reward"
    };
    centred(image, &ui, heading, ui_theme::TEXT, 260, TODAY_TOP, 13.0);
    for (index, choice) in today.iter().enumerate() {
        let rect = choice_card(index, today.len());
        let picked = index as u32 == view.selected && state.claimed == 0;
        draw_card(image, rect, if picked { WidgetState::Selected } else { WidgetState::Normal });
        draw_reward(image, assets, &ui, choice, [rect.x + 8, rect.y + 8], 44, state.claimed != 0, false);
        let name = crate::shop::item_name(assets, choice.item_type);
        say(image, &ui, &name, ui_theme::TEXT, rect.x + 60, rect.y + 14, 12.0);
        say(image, &ui, &format!("x{}", choice.amount), ui_theme::TEXT_GOLD, rect.x + 60, rect.y + 32, 12.0);
    }
    let look = if view.can_claim() { WidgetState::Normal } else { WidgetState::Disabled };
    draw_button(image, &ui, CLAIM, "Claim", look);
    centred(
        image,
        &ui,
        "Claim every day to keep your streak; miss a day and the cycle starts over.",
        ui_theme::TEXT_MUTED,
        260,
        378,
        11.0,
    );
}

/// The action under `cursor`, in the panel's own coordinates.
pub fn action_at(view: &DailyRewardsView, cursor: [f32; 2]) -> Option<DailyRewardAction> {
    let state = view.state.as_ref()?;
    if state.claimed != 0 {
        return None;
    }
    let today = view.day(state.day);
    if let Some(index) = (0..today.len()).find(|index| choice_card(*index, today.len()).contains_local(cursor)) {
        return Some(DailyRewardAction::Choose(index as u32));
    }
    CLAIM
        .contains_local(cursor)
        .then_some(DailyRewardAction::Claim(view.selected.min(today.len().saturating_sub(1) as u32)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view(streak: u32, day: u32, claimed: u32) -> DailyRewardsView {
        let mut view = DailyRewardsView::default();
        for (index, (day, item_type, amount)) in [(1, 1_088_001, 1), (1, 721_331, 3), (2, 723_017, 1)].into_iter().enumerate() {
            view.receive(DailyRewardChoice { index: index as u32, total: 3, day, item_type, amount });
        }
        view.state = Some(DailyRewardState { streak, day, claimed, next_day_at: 0 });
        view
    }

    #[test]
    fn days_before_today_are_claimed_while_the_streak_lives() {
        let going = view(1, 2, 0);
        assert!(going.claimed(1));
        assert!(!going.claimed(2));
        assert!(going.can_claim());
        let taken = view(2, 2, 1);
        assert!(taken.claimed(2));
        assert!(!taken.can_claim());
        let fresh = view(0, 1, 0);
        assert!(!fresh.claimed(1));
        assert_eq!(fresh.day(1).len(), 2);
    }

    #[test]
    fn a_choice_is_picked_then_claimed() {
        let fresh = view(0, 1, 0);
        let second = choice_card(1, 2);
        let point = [second.x as f32 + 5.0, second.y as f32 + 5.0];
        assert_eq!(action_at(&fresh, point), Some(DailyRewardAction::Choose(1)));
        let claim = [CLAIM.x as f32 + 5.0, CLAIM.y as f32 + 5.0];
        assert_eq!(action_at(&fresh, claim), Some(DailyRewardAction::Claim(0)));
        assert_eq!(action_at(&view(2, 2, 1), claim), None, "already claimed");
    }

    /// `BOUNTY_PREVIEW=dir cargo test -p conquer-sandbox daily_rewards_preview`
    #[test]
    fn daily_rewards_preview() {
        let Ok(dir) = std::env::var("BOUNTY_PREVIEW") else { return };
        let mut assets = Assets::open(std::path::Path::new("R:/5065")).expect("assets");
        let cycle = [
            (1, 1_088_001, 1), (1, 721_331, 3), (2, 723_017, 1), (2, 721_333, 3), (3, 1_088_001, 2), (3, 1_060_020, 3),
            (4, 723_700, 1), (5, 1_088_001, 3), (5, 723_017, 2), (6, 723_700, 1), (6, 720_027, 1), (7, 1_088_000, 1),
        ];
        let mut view = DailyRewardsView::default();
        for (index, (day, item_type, amount)) in cycle.into_iter().enumerate() {
            view.receive(DailyRewardChoice { index: index as u32, total: cycle.len() as u32, day, item_type, amount });
        }
        let next_day_at = crate::store_panel::now_secs() + 5 * 3_600 + 12 * 60;
        for (name, streak, day, claimed, selected) in [("fresh", 0, 1, 0, 0), ("day3", 2, 3, 0, 1), ("claimed", 5, 5, 1, 0)] {
            view.state = Some(DailyRewardState { streak, day, claimed, next_day_at });
            view.selected = selected;
            let mut content = RgbaImage::new(520, 420);
            draw(&mut content, &mut assets, Some(&view));
            let mut image = crate::original_panels::framed_window(&mut assets, 520, 420).expect("frame");
            image::imageops::overlay(&mut image, &content, 0, 0);
            image.save(format!("{dir}/daily-{name}.png")).expect("png");
        }
    }
}
