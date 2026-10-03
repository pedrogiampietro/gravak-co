//! Shared reusable UI components for the Gravak 5065 client.
//!
//! Provides consistent styling, hit-box tracking, and visual states
//! (Normal, Hovered, Pressed, Disabled, Selected) across all game windows:
//! Window frames, buttons, slots, tabs, tooltips, cards, lists, and banners.

use crate::{assets::Result, labels, ui::UiRect, ui_theme};
use fontdue::Font;
use image::{Rgba, RgbaImage};

/// Standard interactive states for client widgets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WidgetState {
    #[default]
    Normal,
    Hovered,
    Pressed,
    Disabled,
    Selected,
}

impl WidgetState {
    pub fn is_interactive(self) -> bool {
        self != Self::Disabled
    }

    pub fn is_hovered(self) -> bool {
        self == Self::Hovered
    }

    pub fn is_active(self) -> bool {
        matches!(self, Self::Pressed | Self::Selected)
    }
}

// ============================================================================
// WINDOW & HEADER COMPONENTS
// ============================================================================

/// Renders a full window container with title bar, gold decorative corners,
/// and close button across the entire image dimensions.
pub fn draw_window(image: &mut RgbaImage, title: &str, close_hovered: bool) -> Result<()> {
    ui_theme::window(image, title, close_hovered)
}

/// Renders a window container within an explicit logical rectangle.
pub fn draw_window_rect(
    image: &mut RgbaImage,
    rect: UiRect,
    title: &str,
    close_state: WidgetState,
) -> Result<()> {
    fill(image, rect, ui_theme::PANEL);
    fill(
        image,
        UiRect::new(
            rect.x + 5,
            rect.y + 5,
            rect.width.saturating_sub(10),
            rect.height.saturating_sub(10),
        ),
        ui_theme::PANEL_INNER,
    );
    outline(image, rect, ui_theme::PANEL_HIGHLIGHT);
    outline(
        image,
        UiRect::new(
            rect.x + 2,
            rect.y + 2,
            rect.width.saturating_sub(4),
            rect.height.saturating_sub(4),
        ),
        ui_theme::PANEL_SHADOW,
    );
    outline(
        image,
        UiRect::new(
            rect.x + 5,
            rect.y + 5,
            rect.width.saturating_sub(10),
            rect.height.saturating_sub(10),
        ),
        [118, 87, 37, 255],
    );

    // Decorative separator under header
    line(
        image,
        [rect.x + 12, rect.y + 39],
        [rect.x + rect.width.saturating_sub(13), rect.y + 39],
        [157, 115, 48, 220],
    );
    line(
        image,
        [rect.x + 12, rect.y + 41],
        [rect.x + rect.width.saturating_sub(13), rect.y + 41],
        [30, 63, 75, 255],
    );

    // Gold diamond corners
    for (dx, dy) in [
        (8, 8),
        (rect.width.saturating_sub(9), 8),
        (8, rect.height.saturating_sub(9)),
        (rect.width.saturating_sub(9), rect.height.saturating_sub(9)),
    ] {
        diamond(
            image,
            rect.x + dx,
            rect.y + dy,
            4,
            4,
            ui_theme::PANEL_HIGHLIGHT,
        );
    }

    let font = labels::decorative_font()?;
    centered_text(
        image,
        &font,
        title,
        rect.x as f32 + rect.width as f32 * 0.5,
        rect.y as f32 + 11.0,
        13.0,
        ui_theme::TEXT_GOLD,
    );

    let close_rect = UiRect::new(rect.x + rect.width.saturating_sub(29), rect.y + 9, 22, 22);
    draw_close_button(image, close_rect, close_state);
    Ok(())
}

/// Renders a titled section divider inside a panel.
pub fn draw_section(image: &mut RgbaImage, font: &Font, rect: UiRect, title: &str) {
    ui_theme::section(image, rect, title, font);
}

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
        WidgetState::Pressed | WidgetState::Selected => crate::ornate::Look::Pressed,
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
        WidgetState::Pressed | WidgetState::Selected => {
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

/// Renders a window close button with 'X'.
pub fn draw_close_button(image: &mut RgbaImage, rect: UiRect, state: WidgetState) {
    let hovered = state == WidgetState::Hovered || state == WidgetState::Pressed;
    let fill_colour = if hovered {
        [113, 54, 26, 255]
    } else {
        [51, 28, 22, 255]
    };
    let border = if hovered {
        [255, 230, 128, 255]
    } else {
        [224, 173, 73, 255]
    };
    fill(image, rect, fill_colour);
    outline(image, rect, border);
    let (x, y) = (rect.x, rect.y);
    for offset in 0..8 {
        pixel(image, x + 6 + offset, y + 6 + offset, border);
        pixel(image, x + 15 - offset, y + 6 + offset, border);
        pixel(image, x + 6 + offset, y + 15 - offset, border);
        pixel(image, x + 15 - offset, y + 15 - offset, border);
    }
}

/// Renders a scroll button with up or down arrow.
pub fn draw_scroll_button(image: &mut RgbaImage, rect: UiRect, down: bool, state: WidgetState) {
    let hovered = state == WidgetState::Hovered;
    let outer_border = if hovered {
        [250, 214, 128, 255]
    } else {
        [91, 125, 137, 255]
    };
    let inner_fill = if hovered {
        [47, 83, 96, 255]
    } else {
        [16, 42, 56, 255]
    };
    let arrow_colour = if hovered {
        [255, 233, 169, 255]
    } else {
        [191, 215, 221, 255]
    };

    fill(image, rect, inner_fill);
    outline(image, rect, outer_border);

    let cx = rect.x + rect.width / 2;
    let cy = rect.y + rect.height / 2;
    for row in 0..5 {
        let half = if down { 4 - row } else { row };
        let arrow_y = if down { cy - 2 + row } else { cy + 2 - row };
        for x in (cx - half)..=(cx + half) {
            pixel(image, x, arrow_y, arrow_colour);
        }
    }
}

// ============================================================================
// SLOT & RARITY COMPONENTS
// ============================================================================

/// Renders a standardized slot with empty crosshair, border, and inner glow.
pub fn draw_slot(image: &mut RgbaImage, rect: UiRect, state: WidgetState, occupied: bool) {
    let hovered = state == WidgetState::Hovered;
    let selected = state == WidgetState::Selected;
    ui_theme::slot(image, rect, hovered, selected, occupied);
}

/// Renders an item rarity outline with classic Conquer rarity colors:
/// 1=Refined (Blue), 2=Unique (Purple), 3=Elite (Orange), 4=Super (Gold).
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

/// Renders a crisp geometric icon tab for character menus.
pub fn draw_icon_tab(
    image: &mut RgbaImage,
    rect: UiRect,
    kind: usize,
    active: bool,
    hovered: bool,
) {
    let _ = hovered;
    ui_theme::icon_tab(image, rect, kind, active);
}

// ============================================================================
// TOOLTIP COMPONENTS
// ============================================================================

/// Renders a standardized tooltip container frame with accent border.
pub fn draw_tooltip_frame(image: &mut RgbaImage, rect: UiRect, accent: [u8; 3]) {
    fill(image, rect, [8, 16, 24, 250]);
    outline(image, rect, [accent[0], accent[1], accent[2], 255]);
    outline(
        image,
        UiRect::new(rect.x + 1, rect.y + 1, rect.width - 2, rect.height - 2),
        [40, 60, 75, 200],
    );
}

/// Renders a divider rule inside a tooltip.
pub fn draw_tooltip_rule(image: &mut RgbaImage, rect: UiRect, y: i32) {
    if y >= rect.y && y < rect.y + rect.height {
        line(
            image,
            [rect.x + 8, y],
            [rect.x + rect.width - 8, y],
            [83, 110, 125, 200],
        );
    }
}

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
        WidgetState::Pressed => [220, 200, 160],
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

fn diamond(image: &mut RgbaImage, cx: i32, cy: i32, rx: i32, ry: i32, colour: [u8; 4]) {
    for y in -ry..=ry {
        let width = (rx as f32 * (1.0 - y.abs() as f32 / ry.max(1) as f32)).round() as i32;
        for x in -width..=width {
            pixel(image, cx + x, cy + y, colour);
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
    fn widget_state_properties() {
        assert!(WidgetState::Normal.is_interactive());
        assert!(!WidgetState::Disabled.is_interactive());
        assert!(WidgetState::Hovered.is_hovered());
        assert!(WidgetState::Pressed.is_active());
        assert!(WidgetState::Selected.is_active());
    }

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
