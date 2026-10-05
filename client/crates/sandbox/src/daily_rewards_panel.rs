//! The Reward Wall, after Tibia's: opened at the Gravak statue, it shows
//! the seven days of the cycle with their rewards, the days already claimed
//! and today's, whose choices the hero picks one of before claiming. The
//! streak along the top. Drawn inside a quick panel's frame.

use crate::assets::Assets;
use crate::components::{draw_button, WidgetState};
use crate::paint::{self, Colour};
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

// The layout, in the panel's 520 x 420 contents.
const DAY_LEFT: i32 = 18;
const DAY_WIDTH: i32 = 64;
const DAY_GAP: i32 = 6;
/// The trail of the seven days, its markers over the cards' centres.
const TRAIL_Y: i32 = 88;
const DAY_TOP: i32 = 104;
const DAY_HEIGHT: i32 = 140;
const TODAY_TOP: i32 = 254;
const CHOICE_WIDTH: i32 = 172;
const CHOICE_HEIGHT: i32 = 58;
const CHOICE_TOP: i32 = TODAY_TOP + 24;
const CLAIM: UiRect = UiRect::new(184, 342, 152, 34);

fn day_card(day: u32) -> UiRect {
    UiRect::new(DAY_LEFT + (day as i32 - 1) * (DAY_WIDTH + DAY_GAP), DAY_TOP, DAY_WIDTH, DAY_HEIGHT)
}

/// Today's choices, centred in a row.
fn choice_card(index: usize, count: usize) -> UiRect {
    let count = count.max(1) as i32;
    let width = count * CHOICE_WIDTH + (count - 1) * 14;
    let left = (520 - width) / 2;
    UiRect::new(left + index as i32 * (CHOICE_WIDTH + 14), CHOICE_TOP, CHOICE_WIDTH, CHOICE_HEIGHT)
}

fn width_of(font: &Font, text: &str, size: f32) -> f32 {
    text.chars().map(|c| font.metrics(c, size).advance_width).sum()
}

fn say(image: &mut RgbaImage, font: &Font, text: &str, colour: [u8; 3], x: i32, y: i32, size: f32) {
    labels::text_sized(image, font, &[(text, [6, 12, 16])], [x as f32 + 1.0, y as f32 + 1.0], size);
    labels::text_sized(image, font, &[(text, colour)], [x as f32, y as f32], size);
}

fn centred(image: &mut RgbaImage, font: &Font, text: &str, colour: [u8; 3], centre_x: i32, y: i32, size: f32) {
    let x = centre_x - width_of(font, text, size).round() as i32 / 2;
    say(image, font, text, colour, x, y, size);
}

// Smooth shapes, painted only inside their box (the panel is redrawn whole
// on every change, and `paint::fill` walks the whole image).

/// Paints `shape` (a signed distance) within `rect` grown by `margin`.
fn paint_in(
    image: &mut RgbaImage,
    rect: UiRect,
    margin: i32,
    shape: impl Fn(f32, f32) -> f32,
    colour: impl Fn(f32, f32) -> Colour,
    alpha: f32,
) {
    for y in rect.y - margin..rect.y + rect.height + margin {
        for x in rect.x - margin..rect.x + rect.width + margin {
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            let cover = paint::coverage(shape(px, py));
            if cover > 0.0 {
                paint::blend(image, x, y, colour(px, py), cover * alpha);
            }
        }
    }
}

/// A soft glow outside `shape`, within `rect` grown by `reach`.
fn glow_in(image: &mut RgbaImage, rect: UiRect, shape: impl Fn(f32, f32) -> f32, reach: f32, colour: Colour, alpha: f32) {
    let margin = reach.ceil() as i32;
    for y in rect.y - margin..rect.y + rect.height + margin {
        for x in rect.x - margin..rect.x + rect.width + margin {
            let distance = shape(x as f32 + 0.5, y as f32 + 0.5);
            if distance > 0.0 && distance < reach {
                let fade = 1.0 - distance / reach;
                paint::blend(image, x, y, colour, alpha * fade * fade);
            }
        }
    }
}

fn rounded(rect: UiRect, radius: f32) -> impl Fn(f32, f32) -> f32 {
    let centre = [rect.x as f32 + rect.width as f32 / 2.0, rect.y as f32 + rect.height as f32 / 2.0];
    let half = [rect.width as f32 / 2.0, rect.height as f32 / 2.0];
    move |x, y| paint::rounded_rect(x, y, centre, half, radius)
}

/// Distance to the segment from `a` to `b`.
fn segment(x: f32, y: f32, a: [f32; 2], b: [f32; 2]) -> f32 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let t = (((x - a[0]) * dx + (y - a[1]) * dy) / (dx * dx + dy * dy).max(f32::EPSILON)).clamp(0.0, 1.0);
    ((x - a[0] - t * dx).powi(2) + (y - a[1] - t * dy).powi(2)).sqrt()
}

/// A tick in a circle of `radius` around `centre`.
fn tick(image: &mut RgbaImage, centre: [f32; 2], radius: f32, colour: Colour) {
    let [cx, cy] = centre;
    let r = radius;
    let a = [cx - r * 0.45, cy + r * 0.02];
    let b = [cx - r * 0.1, cy + r * 0.38];
    let c = [cx + r * 0.5, cy - r * 0.35];
    let bounds = UiRect::new((cx - r) as i32, (cy - r) as i32, (2.0 * r) as i32 + 1, (2.0 * r) as i32 + 1);
    let width = (r * 0.18).max(1.2);
    paint_in(image, bounds, 1, |x, y| segment(x, y, a, b).min(segment(x, y, b, c)) - width, |_, _| colour, 1.0);
}

const GOLD_LIGHT: Colour = paint::hex(0xfbe7a8);
const GOLD: Colour = paint::hex(0xd8a648);
const GOLD_DARK: Colour = paint::hex(0x7a5218);
const TEAL_TOP: Colour = paint::hex(0x163a52);
const TEAL_BOTTOM: Colour = paint::hex(0x081a26);
const GREEN: Colour = paint::hex(0x5fbf6a);

/// A card: rounded, a teal gradient, a thin border; `lit` gives it a gold
/// border and glow (today, the picked choice), `dim` darkens it.
fn card(image: &mut RgbaImage, rect: UiRect, lit: bool, dim: bool, special: bool) {
    let shape = rounded(rect, 6.0);
    if lit {
        glow_in(image, rect, &shape, 7.0, paint::hex(0xffd36b), 0.45);
    }
    let (top, bottom) = if dim {
        (paint::mix(TEAL_TOP, paint::hex(0x0a1218), 0.55), paint::hex(0x060c10))
    } else if special {
        (paint::mix(TEAL_TOP, paint::hex(0x4a3410), 0.35), TEAL_BOTTOM)
    } else {
        (TEAL_TOP, TEAL_BOTTOM)
    };
    let (y0, height) = (rect.y as f32, rect.height as f32);
    paint_in(image, rect, 1, &shape, |_, y| paint::mix(top, bottom, (y - y0) / height), 0.96);
    let border = if lit {
        GOLD_LIGHT
    } else if special {
        GOLD
    } else if dim {
        paint::hex(0x2a3a40)
    } else {
        paint::hex(0x3d6070)
    };
    paint_in(image, rect, 1, |x, y| shape(x, y).abs() - if lit { 0.9 } else { 0.6 }, |_, _| border, 1.0);
}

/// A slot for an icon: a dark well in a gold-brown frame.
fn slot(image: &mut RgbaImage, rect: UiRect, dim: bool) {
    let shape = rounded(rect, 3.0);
    paint_in(image, rect, 1, &shape, |_, _| paint::hex(0x050c11), 0.95);
    let frame = if dim { paint::hex(0x3a3328) } else { paint::hex(0x9c7434) };
    paint_in(image, rect, 1, |x, y| shape(x, y).abs() - 0.6, |_, _| frame, 1.0);
    let inner = UiRect::new(rect.x + 2, rect.y + 2, rect.width - 4, rect.height - 4);
    let inner_shape = rounded(inner, 2.0);
    paint_in(image, inner, 0, |x, y| inner_shape(x, y).abs() - 0.5, |_, _| paint::hex(0x1d3440), 0.8);
}

/// An item's icon centred in `slot`, its amount in the bottom-right corner.
fn reward_in_slot(image: &mut RgbaImage, assets: &mut Assets, ui: &Font, choice: &DailyRewardChoice, rect: UiRect, dim: bool, amount: bool) {
    slot(image, rect, dim);
    let size = (rect.width - 6) as u32;
    if let Ok(icon) = crate::shop::item_icon_image(assets, choice.item_type) {
        let mut icon = image::imageops::resize(&icon, size, size, image::imageops::FilterType::Triangle);
        if dim {
            for pixel in icon.pixels_mut() {
                let grey = ((u16::from(pixel[0]) + u16::from(pixel[1]) + u16::from(pixel[2])) / 5) as u8;
                *pixel = image::Rgba([grey, grey, grey, pixel[3]]);
            }
        }
        image::imageops::overlay(image, &icon, i64::from(rect.x + 3), i64::from(rect.y + 3));
    }
    if amount && choice.amount > 1 {
        let text = format!("x{}", choice.amount);
        let x = rect.x + rect.width - 3 - width_of(ui, &text, 10.0).round() as i32;
        say(image, ui, &text, [255, 246, 216], x, rect.y + rect.height - 13, 10.0);
    }
}

/// A gold rule with a diamond in its middle, from `left` to `right`.
fn rule(image: &mut RgbaImage, left: i32, right: i32, y: i32) {
    let bounds = UiRect::new(left, y - 4, right - left, 9);
    let (a, b) = ([left as f32, y as f32 + 0.5], [right as f32, y as f32 + 0.5]);
    let middle = (left + right) as f32 / 2.0;
    let span = (right - left) as f32 / 2.0;
    paint_in(
        image,
        bounds,
        0,
        |x, y| segment(x, y, a, b) - 0.5,
        |x, _| paint::mix(GOLD, paint::hex(0x0c1a22), ((x - middle).abs() / span).powf(1.6)),
        0.9,
    );
    let centre = [middle, y as f32 + 0.5];
    paint_in(image, bounds, 0, |x, y| (x - centre[0]).abs() + (y - centre[1]).abs() - 3.5, |_, _| GOLD_LIGHT, 1.0);
}

/// A heading in the decorative font between two gold rules.
fn heading(image: &mut RgbaImage, cinzel: &Font, text: &str, centre_x: i32, y: i32, size: f32, reach: i32) {
    let width = width_of(cinzel, text, size).round() as i32;
    centred(image, cinzel, text, [255, 226, 143], centre_x, y, size);
    let line_y = y + (size * 0.55) as i32;
    rule(image, centre_x - width / 2 - 10 - reach, centre_x - width / 2 - 10, line_y);
    rule(image, centre_x + width / 2 + 10, centre_x + width / 2 + 10 + reach, line_y);
}

/// "5h 12m" until `then` (seconds since 1970).
fn until(then: u64, now: u64) -> String {
    let left = then.saturating_sub(now);
    let (hours, minutes) = (left / 3_600, left / 60 % 60);
    if hours > 0 { format!("{hours}h {minutes:02}m") } else { format!("{minutes}m") }
}

/// The streak medallion: a gold coin with the number of days on it.
fn medallion(image: &mut RgbaImage, cinzel: &Font, centre: [f32; 2], days: u32) {
    let radius = 17.0;
    let bounds = UiRect::new((centre[0] - radius) as i32 - 1, (centre[1] - radius) as i32 - 1, 37, 37);
    glow_in(image, bounds, |x, y| paint::circle(x, y, centre, radius), 6.0, paint::hex(0xffc85a), 0.35);
    paint_in(
        image,
        bounds,
        1,
        |x, y| paint::circle(x, y, centre, radius),
        |_, y| paint::gradient(&[(0.0, GOLD_LIGHT), (0.5, GOLD), (1.0, GOLD_DARK)], (y - centre[1] + radius) / (2.0 * radius)),
        1.0,
    );
    paint_in(image, bounds, 1, |x, y| paint::circle(x, y, centre, radius - 3.5), |_, _| paint::hex(0x123651), 1.0);
    paint_in(image, bounds, 1, |x, y| paint::circle(x, y, centre, radius - 3.5).abs() - 0.6, |_, _| GOLD_DARK, 1.0);
    let text = days.to_string();
    let size = if days > 99 { 12.0 } else { 15.0 };
    centred(image, cinzel, &text, [255, 236, 170], centre[0] as i32, (centre[1] - size * 0.62) as i32, size);
}

/// A pill: a dark rounded label with a gold edge.
fn pill(image: &mut RgbaImage, ui: &Font, text: &str, right: i32, y: i32) {
    let width = width_of(ui, text, 12.0).round() as i32 + 22;
    let rect = UiRect::new(right - width, y, width, 22);
    let shape = rounded(rect, 11.0);
    paint_in(image, rect, 1, &shape, |_, _| paint::hex(0x07131b), 0.9);
    paint_in(image, rect, 1, |x, y| shape(x, y).abs() - 0.6, |_, _| GOLD_DARK, 1.0);
    say(image, ui, text, ui_theme::TEXT, rect.x + 11, y + 4, 12.0);
}

/// The seven days as a trail: a line through markers, ticked where claimed,
/// glowing today.
fn trail(image: &mut RgbaImage, ui: &Font, view: &DailyRewardsView, today: u32) {
    let centre_x = |day: u32| (day_card(day).x + DAY_WIDTH / 2) as f32;
    let y = TRAIL_Y as f32;
    let bounds = UiRect::new(DAY_LEFT, TRAIL_Y - 12, 7 * DAY_WIDTH + 6 * DAY_GAP, 24);
    let (first, last) = ([centre_x(1), y], [centre_x(CYCLE), y]);
    paint_in(image, bounds, 0, |x, y| segment(x, y, first, last) - 1.5, |_, _| paint::hex(0x24404c), 1.0);
    // Gold up to today.
    if today > 1 {
        let reached = [centre_x(today), y];
        paint_in(image, bounds, 0, |x, y| segment(x, y, first, reached) - 1.5, |_, _| GOLD, 1.0);
    }
    for day in 1..=CYCLE {
        let centre = [centre_x(day), y];
        let claimed = view.claimed(day);
        let is_today = day == today;
        let radius = if is_today { 9.0 } else { 7.5 };
        if is_today {
            glow_in(image, bounds, |x, y| paint::circle(x, y, centre, radius), 7.0, paint::hex(0xffd36b), 0.6);
        }
        let fill = if claimed {
            GOLD
        } else if is_today {
            paint::hex(0x123651)
        } else {
            paint::hex(0x0b1a22)
        };
        paint_in(image, bounds, 2, |x, y| paint::circle(x, y, centre, radius), |_, _| fill, 1.0);
        let ring = if claimed || is_today { GOLD_LIGHT } else { paint::hex(0x46626e) };
        paint_in(image, bounds, 2, |x, y| paint::circle(x, y, centre, radius).abs() - 0.7, |_, _| ring, 1.0);
        if claimed {
            tick(image, centre, radius - 1.0, paint::hex(0x1b2a10));
        } else {
            let colour = if is_today { [255, 226, 143] } else { [120, 140, 146] };
            centred(image, ui, &day.to_string(), colour, centre[0] as i32, (y - 6.0) as i32, 10.0);
        }
    }
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

    // The streak on its medallion, and when the next day comes.
    medallion(image, &cinzel, [38.0, 60.0], state.streak);
    say(image, &ui, "Daily streak", ui_theme::TEXT_MUTED, 64, 44, 11.0);
    let streak = match state.streak {
        0 => "Claim today to start one!".to_owned(),
        1 => "1 day in a row".to_owned(),
        days => format!("{days} days in a row"),
    };
    say(image, &cinzel, &streak, ui_theme::TEXT_GOLD, 64, 58, 14.0);
    let next = if state.claimed != 0 {
        format!("Next reward in {}", until(state.next_day_at, now))
    } else {
        format!("Today ends in {}", until(state.next_day_at, now))
    };
    pill(image, &ui, &next, 502, 48);

    trail(image, &ui, view, state.day);

    // The seven days.
    for day in 1..=CYCLE {
        let rect = day_card(day);
        let is_today = day == state.day;
        let claimed = view.claimed(day);
        let grand = day == CYCLE;
        card(image, rect, is_today, claimed, grand);
        // Its title band.
        let band = UiRect::new(rect.x + 1, rect.y + 1, rect.width - 2, 20);
        let band_shape = rounded(band, 5.0);
        let band_colour = if is_today || grand { paint::hex(0x3a2a0e) } else { paint::hex(0x0b1d28) };
        paint_in(image, band, 0, |x, y| band_shape(x, y).max(-(y - band.y as f32 - 4.0)).min(band_shape(x, y)), |_, _| band_colour, 0.85);
        let title_colour = if is_today || grand {
            [255, 226, 143]
        } else if claimed {
            [120, 130, 128]
        } else {
            [220, 210, 180]
        };
        centred(image, &cinzel, &format!("Day {day}"), title_colour, rect.x + rect.width / 2, rect.y + 4, 11.0);

        let choices = view.day(day);
        let slot_size = 38;
        let slot_x = rect.x + (rect.width - slot_size) / 2;
        match choices.as_slice() {
            [] => {},
            [only] => {
                let slot_rect = UiRect::new(slot_x, rect.y + 48, slot_size, slot_size);
                reward_in_slot(image, assets, &ui, only, slot_rect, claimed, true);
            },
            [first, second, ..] => {
                reward_in_slot(image, assets, &ui, first, UiRect::new(slot_x, rect.y + 26, slot_size, slot_size), claimed, true);
                let or_y = rect.y + 66;
                rule(image, rect.x + 8, rect.x + 22, or_y + 6);
                rule(image, rect.x + rect.width - 22, rect.x + rect.width - 8, or_y + 6);
                centred(image, &ui, "or", [150, 160, 156], rect.x + rect.width / 2, or_y, 10.0);
                reward_in_slot(image, assets, &ui, second, UiRect::new(slot_x, rect.y + 80, slot_size, slot_size), claimed, true);
            },
        }
        // What it is now: a green tick, today, or the grand prize.
        let foot = rect.y + rect.height - 14;
        if claimed {
            // A green tick and "Claimed", centred together.
            let label_width = width_of(&ui, "Claimed", 10.0).round() as i32;
            let left = rect.x + (rect.width - (14 + 3 + label_width)) / 2;
            let centre = [(left + 7) as f32, (foot + 4) as f32];
            let badge = UiRect::new(left - 1, foot - 4, 16, 16);
            paint_in(image, badge, 1, |x, y| paint::circle(x, y, centre, 6.5), |_, _| GREEN, 1.0);
            tick(image, centre, 5.5, paint::hex(0xf4fff0));
            say(image, &ui, "Claimed", [150, 214, 150], left + 17, foot - 2, 10.0);
        } else if is_today {
            centred(image, &ui, "Today", [255, 226, 143], rect.x + rect.width / 2, foot - 2, 10.0);
        } else if grand {
            centred(image, &ui, "Grand prize", [236, 196, 108], rect.x + rect.width / 2, foot - 2, 10.0);
        }
    }

    // Today's choices and the Claim button.
    let today = view.day(state.day);
    let title = if state.claimed != 0 {
        "Today's Reward Claimed"
    } else if today.len() > 1 {
        "Choose Today's Reward"
    } else {
        "Today's Reward"
    };
    heading(image, &cinzel, title, 260, TODAY_TOP, 14.0, 90);
    for (index, choice) in today.iter().enumerate() {
        let rect = choice_card(index, today.len());
        let picked = index as u32 == view.selected && state.claimed == 0;
        card(image, rect, picked, state.claimed != 0, false);
        let slot_rect = UiRect::new(rect.x + 8, rect.y + 8, 42, 42);
        reward_in_slot(image, assets, &ui, choice, slot_rect, state.claimed != 0, false);
        let name = crate::shop::item_name(assets, choice.item_type);
        let name_colour = if state.claimed != 0 { [150, 150, 140] } else { [255, 226, 143] };
        say(image, &ui, &name, name_colour, rect.x + 60, rect.y + 13, 13.0);
        say(image, &ui, &format!("Amount: {}", choice.amount), ui_theme::TEXT_MUTED, rect.x + 60, rect.y + 31, 11.0);
        // A radio mark on the right while there is a choice to make.
        if state.claimed == 0 && today.len() > 1 {
            let centre = [(rect.x + rect.width - 16) as f32, (rect.y + rect.height / 2) as f32];
            let bounds = UiRect::new(rect.x + rect.width - 26, rect.y + rect.height / 2 - 10, 20, 20);
            paint_in(image, bounds, 1, |x, y| paint::circle(x, y, centre, 6.5).abs() - 0.7, |_, _| GOLD, 1.0);
            if picked {
                paint_in(image, bounds, 1, |x, y| paint::circle(x, y, centre, 3.8), |_, _| GOLD_LIGHT, 1.0);
            }
        }
    }
    let look = if view.can_claim() { WidgetState::Normal } else { WidgetState::Disabled };
    let label = if view.can_claim() { "Claim" } else { "Claimed" };
    draw_button(image, &ui, CLAIM, label, look);
    centred(
        image,
        &ui,
        "Claim every day to keep your streak; miss a day and the cycle starts over.",
        ui_theme::TEXT_MUTED,
        260,
        380,
        10.0,
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
