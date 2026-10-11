//! "While you were away": what the idle hunt brought (`crate::idle` on the
//! server): how long, the monsters felled, the experience and levels, the
//! silver, and what was kept or sold. Shown once on entering the world.

use crate::components::{draw_button, draw_card, fill, WidgetState};
use crate::ui::UiRect;
use crate::{labels, ui_theme};
use fontdue::Font;
use image::RgbaImage;

pub const SIZE: [u32; 2] = [440, 360];
const LEFT: i32 = 22;
const WIDTH: i32 = SIZE[0] as i32 - 2 * LEFT;
/// The lines shown at most.
const MOST_LINES: usize = 7;

const GOLD: [u8; 3] = ui_theme::TEXT_GOLD;
const TEXT: [u8; 3] = ui_theme::TEXT;
const MUTED: [u8; 3] = ui_theme::TEXT_MUTED;
const GREEN: [u8; 3] = [120, 220, 130];

/// One line of the report, ready to show.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Line {
    pub kind: u32,
    pub label: String,
    pub amount: u32,
}

/// The window as drawn.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct IdleReportView {
    pub report: gravak_protocol::IdleReport,
    pub lines: Vec<Line>,
    pub ok_hovered: bool,
}

pub fn ok_rect() -> UiRect {
    UiRect::new((SIZE[0] as i32 - 120) / 2, SIZE[1] as i32 - 48, 120, 30)
}

/// Whether `local` (from the window's corner) is over the window.
pub fn covers(local: [f32; 2]) -> bool {
    local[0] >= 0.0 && local[1] >= 0.0 && local[0] < SIZE[0] as f32 && local[1] < SIZE[1] as f32
}

/// "3h 25m", "12m".
pub fn duration(seconds: u32) -> String {
    let (hours, minutes) = (seconds / 3_600, seconds % 3_600 / 60);
    if hours > 0 { format!("{hours}h {minutes:02}m") } else { format!("{minutes}m") }
}

/// 1234567 as "1,234,567".
pub fn grouped(value: u64) -> String {
    let digits = value.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index) % 3 == 0 {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

fn say(image: &mut RgbaImage, font: &Font, words: &str, colour: [u8; 3], x: i32, y: i32, size: f32) {
    labels::text_sized(image, font, &[(words, colour)], [x as f32, y as f32], size);
}

pub fn draw(image: &mut RgbaImage, font: &Font, view: &IdleReportView) {
    let report = &view.report;
    say(image, font, "While you were away", GOLD, LEFT, 46, 13.0);
    say(
        image,
        font,
        &format!("Your helper hunted for {}.", duration(report.seconds)),
        MUTED,
        LEFT,
        66,
        10.0,
    );

    // The four figures, two by two.
    let levels = report.level_after.saturating_sub(report.level_before);
    let figures = [
        ("Monsters felled", grouped(u64::from(report.kills)), String::new()),
        (
            "Experience",
            format!("+{}", grouped(report.experience)),
            if levels > 0 { format!("Level {} -> {}", report.level_before, report.level_after) } else { String::new() },
        ),
        ("Silver", format!("+{}", grouped(u64::from(report.silver))), String::new()),
        ("Items", format!("{} kept", report.kept), if report.sold > 0 { format!("{} sold", report.sold) } else { String::new() }),
    ];
    let card_width = (WIDTH - 10) / 2;
    for (index, (title, value, note)) in figures.iter().enumerate() {
        let rect = UiRect::new(
            LEFT + (index as i32 % 2) * (card_width + 10),
            86 + (index as i32 / 2) * 58,
            card_width,
            50,
        );
        draw_card(image, rect, WidgetState::Normal);
        say(image, font, title, MUTED, rect.x + 10, rect.y + 6, 9.0);
        say(image, font, value, GREEN, rect.x + 10, rect.y + 19, 13.0);
        if !note.is_empty() {
            say(image, font, note, TEXT, rect.x + 10, rect.y + 36, 9.0);
        }
    }

    // What fell and was felled.
    let list = UiRect::new(LEFT, 206, WIDTH, 98);
    fill(image, list, [10, 22, 30, 170]);
    if view.lines.is_empty() {
        say(image, font, "Nothing else to tell.", MUTED, list.x + 10, list.y + 10, 10.0);
    }
    for (row, line) in view.lines.iter().take(MOST_LINES).enumerate() {
        let colour = match line.kind {
            gravak_protocol::IdleLine::KEPT => GREEN,
            gravak_protocol::IdleLine::SOLD => MUTED,
            _ => TEXT,
        };
        let suffix = match line.kind {
            gravak_protocol::IdleLine::KEPT => " (kept)",
            gravak_protocol::IdleLine::SOLD => " (sold)",
            _ => "",
        };
        let column = row as i32 % 2;
        let y = list.y + 8 + (row as i32 / 2) * 22;
        let x = list.x + 10 + column * (WIDTH / 2);
        say(image, font, &format!("{} x{}{suffix}", line.label, grouped(u64::from(line.amount))), colour, x, y, 10.0);
    }
    if view.lines.len() > MOST_LINES {
        say(image, font, &format!("and {} more", view.lines.len() - MOST_LINES), MUTED, list.x + WIDTH / 2 + 10, list.y + 74, 9.0);
    }
    let state = if view.ok_hovered { WidgetState::Hovered } else { WidgetState::Normal };
    draw_button(image, font, ok_rect(), "OK", state);
}

/// The lines as told, one per kind and name: variants of an item (its
/// qualities share a name) added up; the kills, then what was kept, then
/// what was sold, the most of each first.
pub fn grouped_lines(lines: Vec<Line>) -> Vec<Line> {
    let mut merged: Vec<Line> = Vec::new();
    for line in lines {
        match merged.iter_mut().find(|known| known.kind == line.kind && known.label == line.label) {
            Some(known) => known.amount = known.amount.saturating_add(line.amount),
            None => merged.push(line),
        }
    }
    merged.sort_by(|a, b| a.kind.cmp(&b.kind).then(b.amount.cmp(&a.amount)));
    merged
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_items_variants_make_one_line() {
        let line = |kind, label: &str, amount| Line { kind, label: label.into(), amount };
        let lines = grouped_lines(vec![
            line(2, "IronHelmet", 4),
            line(0, "Pheasant", 959),
            line(2, "IronHelmet", 3),
            line(1, "DragonBall", 1),
            line(0, "FlyingRooster", 47),
            line(2, "BadgerHat", 9),
        ]);
        assert_eq!(
            lines,
            vec![
                line(0, "Pheasant", 959),
                line(0, "FlyingRooster", 47),
                line(1, "DragonBall", 1),
                line(2, "BadgerHat", 9),
                line(2, "IronHelmet", 7),
            ]
        );
    }

    #[test]
    fn figures_read_as_people_say_them() {
        assert_eq!(duration(3 * 3_600 + 25 * 60 + 9), "3h 25m");
        assert_eq!(duration(12 * 60), "12m");
        assert_eq!(grouped(1_234_567), "1,234,567");
        assert_eq!(grouped(999), "999");
        assert_eq!(grouped(1_000), "1,000");
    }

    /// `IDLE_PREVIEW=dir cargo test -p conquer-sandbox idle_report_preview`
    #[test]
    fn idle_report_preview() {
        let Ok(dir) = std::env::var("IDLE_PREVIEW") else { return };
        let mut assets = crate::assets::Assets::open(std::path::Path::new("R:/5065")).expect("assets");
        let font = crate::labels::font().expect("font");
        let view = IdleReportView {
            report: gravak_protocol::IdleReport {
                seconds: 3 * 3_600 + 25 * 60,
                kills: 1_482,
                experience: 2_345_678,
                level_before: 54,
                level_after: 57,
                silver: 48_250,
                kept: 6,
                sold: 31,
            },
            lines: vec![
                Line { kind: 0, label: "Robin".into(), amount: 1_102 },
                Line { kind: 0, label: "Turtledove".into(), amount: 380 },
                Line { kind: 1, label: "Meteor".into(), amount: 4 },
                Line { kind: 1, label: "DragonBall".into(), amount: 1 },
                Line { kind: 2, label: "Iron Blade".into(), amount: 22 },
                Line { kind: 2, label: "Leather Coat".into(), amount: 9 },
            ],
            ok_hovered: true,
        };
        let mut content = RgbaImage::new(SIZE[0], SIZE[1]);
        draw(&mut content, &font, &view);
        let image = crate::shop::framed(&mut assets, content, "Helper", false).expect("framed");
        image.save(format!("{dir}/idle-report.png")).expect("png");
    }
}
