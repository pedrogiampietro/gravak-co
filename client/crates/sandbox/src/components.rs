//! Shared reusable UI components for the Gravak 5065 client.
//!
//! Provides consistent styling, hit-box tracking, and visual states
//! (Normal, Hovered, Disabled, Selected) across all game windows:
//! Window frames, buttons, slots, tabs, tooltips, cards, lists, and banners.

use crate::{labels, ui::UiRect, ui_theme};
use fontdue::Font;
use image::{Rgba, RgbaImage};

/// Standard interactive states for client widgets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WidgetState {
    #[default]
    Normal,
    Hovered,
    Disabled,
    Selected,
}

// ============================================================================
// WINDOW & HEADER COMPONENTS
// ============================================================================

// ============================================================================
// BUTTON COMPONENTS
// ============================================================================

/// Renders a standard Conquer bronze text button driven by an explicit `WidgetState`.
pub fn draw_button(
    image: &mut RgbaImage,
    _font: &Font,
    rect: UiRect,
    label: &str,
    state: WidgetState,
) {
    // The game's ornate plate (`ornate.rs`), sized to the rect.
    let look = match state {
        WidgetState::Disabled => crate::ornate::Look::Disabled,
        WidgetState::Hovered => crate::ornate::Look::Hovered,
        WidgetState::Selected => crate::ornate::Look::Pressed,
        WidgetState::Normal => crate::ornate::Look::Normal,
    };
    let size = [rect.width.max(1) as u32, rect.height.max(1) as u32];
    if let Ok(plate) = crate::ornate::plate(None, size, label, crate::ornate::GRAVAK, look, None) {
        image::imageops::overlay(image, &plate, i64::from(rect.x), i64::from(rect.y));
    }
}

/// Renders a wide action button (used in Quick Panels: Trade, Guild, Team, etc.)
pub fn draw_action_button(
    image: &mut RgbaImage,
    font: &Font,
    rect: UiRect,
    label: &str,
    state: WidgetState,
) {
    let (fill_colour, border_colour, text_colour) = match state {
        WidgetState::Disabled => ([22, 26, 32, 230], [60, 70, 80, 220], [110, 115, 125]),
        WidgetState::Hovered => ([80, 57, 29, 255], [255, 226, 123, 255], [255, 240, 190]),
        WidgetState::Selected => {
            ([35, 26, 16, 255], [205, 160, 70, 255], [220, 205, 160])
        }
        WidgetState::Normal => ([16, 34, 46, 255], [111, 80, 36, 255], [225, 215, 180]),
    };

    fill(image, rect, fill_colour);
    outline(image, rect, border_colour);
    outline(
        image,
        UiRect::new(rect.x + 1, rect.y + 1, rect.width - 2, rect.height - 2),
        if state == WidgetState::Hovered {
            [180, 140, 60, 150]
        } else {
            [30, 50, 65, 180]
        },
    );

    let font_size = fit_font(font, label, (rect.width - 12).max(1) as f32, 11.0);
    centered_text(
        image,
        font,
        label,
        rect.x as f32 + rect.width as f32 * 0.5,
        rect.y as f32 + ((rect.height as f32 - font_size) * 0.5).max(1.0),
        font_size,
        text_colour,
    );
}

// ============================================================================
// SLOT & RARITY COMPONENTS
// ============================================================================

/// Renders an item rarity outline with classic Conquer rarity colors:
/// 1=Refined (Blue), 2=Unique (Purple), 3=Elite (Orange), 4=Super (Gold).
#[cfg(test)]
pub fn draw_rarity_border(image: &mut RgbaImage, rect: UiRect, rarity: u8) {
    let colour = match rarity {
        1 => [70, 160, 240, 255], // Refined: Blue
        2 => [180, 80, 240, 255], // Unique: Purple
        3 => [255, 140, 40, 255], // Elite: Orange
        4 => [255, 215, 50, 255], // Super: Gold
        _ => return,
    };
    outline(image, rect, colour);
    outline(
        image,
        UiRect::new(rect.x + 1, rect.y + 1, rect.width - 2, rect.height - 2),
        [colour[0], colour[1], colour[2], 120],
    );
}

// ============================================================================
// TAB COMPONENTS
// ============================================================================

/// Renders a labeled tab button.
pub fn draw_tab(
    image: &mut RgbaImage,
    font: &Font,
    rect: UiRect,
    label: &str,
    active: bool,
    hovered: bool,
) {
    let fill_colour = if active {
        [80, 57, 29, 255]
    } else if hovered {
        [24, 48, 62, 255]
    } else {
        [13, 32, 43, 255]
    };
    let border = if active || hovered {
        ui_theme::PANEL_HIGHLIGHT
    } else {
        [115, 84, 38, 255]
    };
    fill(image, rect, fill_colour);
    outline(image, rect, border);
    let size = fit_font(font, label, (rect.width - 8).max(1) as f32, 10.0);
    centered_text(
        image,
        font,
        label,
        rect.x as f32 + rect.width as f32 * 0.5,
        rect.y as f32 + ((rect.height as f32 - size) * 0.5).max(1.0),
        size,
        if active {
            ui_theme::TEXT_GOLD
        } else {
            ui_theme::TEXT
        },
    );
}

// ============================================================================
// TOOLTIP COMPONENTS
// ============================================================================

// ============================================================================
// CARD & LIST COMPONENTS
// ============================================================================

/// Renders a card container for grouping related content (e.g. Trade player side).
pub fn draw_card(image: &mut RgbaImage, rect: UiRect, state: WidgetState) {
    let (fill_colour, border_colour) = match state {
        WidgetState::Hovered => ([14, 36, 48, 255], ui_theme::PANEL_HIGHLIGHT),
        WidgetState::Selected => ([20, 42, 56, 255], [255, 226, 123, 255]),
        _ => ([9, 24, 34, 255], [65, 85, 95, 220]),
    };
    fill(image, rect, fill_colour);
    outline(image, rect, border_colour);
}

/// Renders a list item row with left and optional right text.
#[allow(dead_code)]
pub fn draw_list_item(
    image: &mut RgbaImage,
    font: &Font,
    rect: UiRect,
    left: &str,
    right: Option<&str>,
    state: WidgetState,
) {
    if state == WidgetState::Hovered || state == WidgetState::Selected {
        fill(
            image,
            rect,
            if state == WidgetState::Selected {
                [40, 60, 75, 180]
            } else {
                [20, 40, 55, 140]
            },
        );
        outline(image, rect, [140, 110, 50, 180]);
    }

    let text_colour = match state {
        WidgetState::Disabled => [130, 130, 130],
        WidgetState::Selected => ui_theme::TEXT_GOLD,
        WidgetState::Hovered => [255, 245, 220],
        WidgetState::Normal => ui_theme::TEXT,
    };

    let font_size = 11.0;
    labels::text_sized(
        image,
        font,
        &[(left, text_colour)],
        [rect.x as f32 + 6.0, rect.y as f32 + 3.0],
        font_size,
    );

    if let Some(right_text) = right {
        let right_width = right_text
            .chars()
            .map(|c| font.metrics(c, font_size).advance_width)
            .sum::<f32>();
        labels::text_sized(
            image,
            font,
            &[(right_text, text_colour)],
            [
                (rect.x + rect.width) as f32 - right_width - 6.0,
                rect.y as f32 + 3.0,
            ],
            font_size,
        );
    }
}

// ============================================================================
// BAR & BANNER COMPONENTS
// ============================================================================

/// Renders a resource / health / durability bar.
pub fn draw_bar(
    image: &mut RgbaImage,
    rect: UiRect,
    current: u32,
    max: u32,
    fill_colour: [u8; 4],
    bg_colour: [u8; 4],
    border_colour: [u8; 4],
) {
    fill(image, rect, bg_colour);
    if max > 0 && current > 0 {
        let fraction = (current as f32 / max as f32).clamp(0.0, 1.0);
        let filled_width = ((rect.width - 2) as f32 * fraction).round() as i32;
        if filled_width > 0 {
            fill(
                image,
                UiRect::new(rect.x + 1, rect.y + 1, filled_width, rect.height - 2),
                fill_colour,
            );
        }
    }
    outline(image, rect, border_colour);
}

/// Renders an in-game status banner for notifications, confirmations, and errors.
pub fn draw_status_banner(
    image: &mut RgbaImage,
    font: &Font,
    rect: UiRect,
    message: &str,
    is_error: bool,
) {
    let (bg, border, text_colour) = if is_error {
        ([50, 18, 18, 240], [210, 60, 60, 255], [255, 200, 200])
    } else {
        ([18, 45, 30, 240], [60, 190, 110, 255], [200, 255, 220])
    };

    fill(image, rect, bg);
    outline(image, rect, border);
    let font_size = fit_font(font, message, (rect.width - 16).max(1) as f32, 11.0);
    centered_text(
        image,
        font,
        message,
        rect.x as f32 + rect.width as f32 * 0.5,
        rect.y as f32 + ((rect.height as f32 - font_size) * 0.5).max(1.0),
        font_size,
        text_colour,
    );
}

// ============================================================================
// INTERNAL DRAWING HELPERS
// ============================================================================

pub fn fill(image: &mut RgbaImage, rect: UiRect, colour: [u8; 4]) {
    ui_theme::fill(image, rect, colour);
}

pub fn outline(image: &mut RgbaImage, rect: UiRect, colour: [u8; 4]) {
    ui_theme::outline(image, rect, colour);
}

pub fn pixel(image: &mut RgbaImage, x: i32, y: i32, colour: [u8; 4]) {
    if x >= 0 && y >= 0 && x < image.width() as i32 && y < image.height() as i32 {
        image.put_pixel(x as u32, y as u32, Rgba(colour));
    }
}

pub fn line(image: &mut RgbaImage, start: [i32; 2], end: [i32; 2], colour: [u8; 4]) {
    let (mut x, mut y) = (start[0], start[1]);
    let dx = (end[0] - x).abs();
    let dy = -(end[1] - y).abs();
    let sx = if x < end[0] { 1 } else { -1 };
    let sy = if y < end[1] { 1 } else { -1 };
    let mut error = dx + dy;
    loop {
        pixel(image, x, y, colour);
        if x == end[0] && y == end[1] {
            break;
        }
        let twice = error * 2;
        if twice >= dy {
            error += dy;
            x += sx;
        }
        if twice <= dx {
            error += dx;
            y += sy;
        }
    }
}

pub fn centered_text(
    image: &mut RgbaImage,
    font: &Font,
    text: &str,
    center_x: f32,
    y: f32,
    size: f32,
    colour: [u8; 3],
) {
    let width = text
        .chars()
        .map(|character| font.metrics(character, size).advance_width)
        .sum::<f32>();
    labels::text_sized(
        image,
        font,
        &[(text, colour)],
        [center_x - width * 0.5, y],
        size,
    );
}

pub fn fit_font(font: &Font, text: &str, max_width: f32, preferred: f32) -> f32 {
    let mut size = preferred;
    while size > 7.0 {
        let width = text
            .chars()
            .map(|character| font.metrics(character, size).advance_width)
            .sum::<f32>();
        if width <= max_width {
            break;
        }
        size -= 0.25;
    }
    size.max(7.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn components_drawing_does_not_panic() {
        let mut image = RgbaImage::new(100, 100);
        let rect = UiRect::new(5, 5, 90, 90);
        fill(&mut image, rect, [10, 20, 30, 255]);
        outline(&mut image, rect, [200, 150, 50, 255]);
        draw_rarity_border(&mut image, UiRect::new(10, 10, 40, 40), 4);
        draw_bar(
            &mut image,
            UiRect::new(10, 60, 80, 12),
            50,
            100,
            [200, 50, 50, 255],
            [30, 30, 30, 255],
            [100, 100, 100, 255],
        );
    }
}
