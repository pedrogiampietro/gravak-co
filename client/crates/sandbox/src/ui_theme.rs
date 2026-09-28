//! Shared visual language for the client-owned UI.
//!
//! The game keeps the Conquer identity through dark blue panels, bronze trim,
//! jewel-like highlights and restrained gold typography, but no longer
//! depends on unrelated atlas crops to position live content.

use crate::{assets::Result, labels, ui::UiRect};
use fontdue::Font;
use image::{Rgba, RgbaImage};

pub const PANEL: [u8; 4] = [8, 22, 31, 255];
pub const PANEL_INNER: [u8; 4] = [11, 31, 43, 255];
pub const PANEL_HIGHLIGHT: [u8; 4] = [224, 173, 73, 255];
pub const PANEL_SHADOW: [u8; 4] = [72, 45, 22, 255];
pub const TEXT: [u8; 3] = [233, 219, 180];
pub const TEXT_MUTED: [u8; 3] = [171, 161, 137];
pub const TEXT_GOLD: [u8; 3] = [255, 226, 143];

pub fn window(image: &mut RgbaImage, title: &str, close_hovered: bool) -> Result<()> {
    let width = image.width();
    let height = image.height();
    fill(image, UiRect::new(0, 0, width as i32, height as i32), PANEL);
    fill(
        image,
        UiRect::new(
            5,
            5,
            width.saturating_sub(10) as i32,
            height.saturating_sub(10) as i32,
        ),
        PANEL_INNER,
    );
    outline(
        image,
        UiRect::new(0, 0, width as i32, height as i32),
        PANEL_HIGHLIGHT,
    );
    outline(
        image,
        UiRect::new(
            2,
            2,
            width.saturating_sub(4) as i32,
            height.saturating_sub(4) as i32,
        ),
        PANEL_SHADOW,
    );
    outline(
        image,
        UiRect::new(
            5,
            5,
            width.saturating_sub(10) as i32,
            height.saturating_sub(10) as i32,
        ),
        [118, 87, 37, 255],
    );
    line(
        image,
        [12, 39],
        [width.saturating_sub(13) as i32, 39],
        [157, 115, 48, 220],
    );
    line(
        image,
        [12, 41],
        [width.saturating_sub(13) as i32, 41],
        [30, 63, 75, 255],
    );
    for (x, y) in [
        (8, 8),
        (width.saturating_sub(9) as i32, 8),
        (8, height.saturating_sub(9) as i32),
        (
            width.saturating_sub(9) as i32,
            height.saturating_sub(9) as i32,
        ),
    ] {
        diamond(image, x, y, 4, 4, PANEL_HIGHLIGHT);
    }
    let font = labels::decorative_font()?;
    centered_text(
        image,
        &font,
        title,
        width as f32 * 0.5,
        11.0,
        13.0,
        TEXT_GOLD,
    );
    close_button(image, close_hovered, width.saturating_sub(29) as i32, 9);
    Ok(())
}

pub fn tab(image: &mut RgbaImage, font: &Font, rect: UiRect, label: &str, active: bool) {
    let fill_colour = if active {
        [80, 57, 29, 255]
    } else {
        [13, 32, 43, 255]
    };
    let border = if active {
        PANEL_HIGHLIGHT
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
        if active { TEXT_GOLD } else { TEXT },
    );
}

/// Compact icon-only tab used by the Character window. The icons are drawn
/// from primitives so they stay crisp at every scale and cannot inherit text
/// alignment problems from a label rendered inside a narrow tab.
pub fn icon_tab(image: &mut RgbaImage, rect: UiRect, kind: usize, active: bool) {
    let fill_colour = if active {
        [80, 57, 29, 255]
    } else {
        [13, 32, 43, 255]
    };
    let border = if active {
        PANEL_HIGHLIGHT
    } else {
        [115, 84, 38, 255]
    };
    fill(image, rect, fill_colour);
    outline(image, rect, border);
    let cx = rect.x + rect.width / 2;
    let cy = rect.y + rect.height / 2;
    let icon = if active {
        [TEXT_GOLD[0], TEXT_GOLD[1], TEXT_GOLD[2], 255]
    } else {
        [TEXT[0], TEXT[1], TEXT[2], 255]
    };
    match kind {
        0 => {
            line(image, [cx - 9, cy - 3], [cx + 9, cy - 3], icon);
            line(image, [cx - 7, cy - 3], [cx - 5, cy + 8], icon);
            line(image, [cx + 7, cy - 3], [cx + 5, cy + 8], icon);
            line(image, [cx - 5, cy + 8], [cx + 5, cy + 8], icon);
            line(image, [cx - 4, cy - 7], [cx + 4, cy - 7], icon);
        }
        1 => {
            line(image, [cx - 8, cy], [cx + 8, cy], icon);
            line(image, [cx, cy - 8], [cx, cy + 8], icon);
            outline(image, UiRect::new(cx - 10, cy - 10, 20, 20), icon);
        }
        2 => {
            line(image, [cx - 8, cy + 8], [cx + 6, cy - 6], icon);
            line(image, [cx + 6, cy - 6], [cx + 9, cy - 10], icon);
            line(image, [cx - 6, cy + 6], [cx - 10, cy + 9], icon);
            line(image, [cx - 2, cy + 2], [cx + 6, cy + 10], icon);
        }
        _ => {
            line(image, [cx, cy - 10], [cx + 3, cy - 3], icon);
            line(image, [cx + 3, cy - 3], [cx + 10, cy], icon);
            line(image, [cx + 10, cy], [cx + 3, cy + 3], icon);
            line(image, [cx + 3, cy + 3], [cx, cy + 10], icon);
            line(image, [cx, cy + 10], [cx - 3, cy + 3], icon);
            line(image, [cx - 3, cy + 3], [cx - 10, cy], icon);
            line(image, [cx - 10, cy], [cx - 3, cy - 3], icon);
            line(image, [cx - 3, cy - 3], [cx, cy - 10], icon);
        }
    }
}

pub fn slot(image: &mut RgbaImage, rect: UiRect, hovered: bool, selected: bool, occupied: bool) {
    let border = if hovered || selected {
        [255, 226, 123, 255]
    } else {
        [170, 124, 51, 255]
    };
    let fill_colour = if occupied {
        [8, 18, 26, 255]
    } else {
        [7, 17, 24, 255]
    };
    fill(image, rect, fill_colour);
    outline(image, rect, border);
    outline(
        image,
        UiRect::new(rect.x + 2, rect.y + 2, rect.width - 4, rect.height - 4),
        if hovered || selected {
            [255, 201, 75, 180]
        } else {
            [43, 72, 81, 255]
        },
    );
    if !occupied {
        let cx = rect.x + rect.width / 2;
        let cy = rect.y + rect.height / 2;
        line(image, [cx - 8, cy], [cx + 8, cy], [107, 129, 133, 140]);
        line(image, [cx, cy - 8], [cx, cy + 8], [107, 129, 133, 140]);
    }
}

pub fn close_button(image: &mut RgbaImage, hovered: bool, x: i32, y: i32) {
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
    fill(image, UiRect::new(x, y, 22, 22), fill_colour);
    outline(image, UiRect::new(x, y, 22, 22), border);
    for offset in 0..8 {
        pixel(image, x + 6 + offset, y + 6 + offset, border);
        pixel(image, x + 15 - offset, y + 6 + offset, border);
        pixel(image, x + 6 + offset, y + 15 - offset, border);
        pixel(image, x + 15 - offset, y + 15 - offset, border);
    }
}

pub fn section(image: &mut RgbaImage, rect: UiRect, title: &str, font: &Font) {
    fill(image, rect, [7, 18, 27, 255]);
    outline(image, rect, [100, 73, 36, 255]);
    let label_rect = UiRect::new(rect.x + 8, rect.y + 5, rect.width - 16, 18);
    let size = fit_font(font, title, label_rect.width as f32, 10.0);
    centered_text(
        image,
        font,
        title,
        label_rect.x as f32 + label_rect.width as f32 * 0.5,
        label_rect.y as f32,
        size,
        TEXT_GOLD,
    );
}

pub fn fill(image: &mut RgbaImage, rect: UiRect, colour: [u8; 4]) {
    for y in rect.y.max(0)..(rect.y + rect.height).min(image.height() as i32) {
        for x in rect.x.max(0)..(rect.x + rect.width).min(image.width() as i32) {
            image.put_pixel(x as u32, y as u32, Rgba(colour));
        }
    }
}

pub fn outline(image: &mut RgbaImage, rect: UiRect, colour: [u8; 4]) {
    if rect.width <= 0 || rect.height <= 0 {
        return;
    }
    line(
        image,
        [rect.x, rect.y],
        [rect.x + rect.width - 1, rect.y],
        colour,
    );
    line(
        image,
        [rect.x, rect.y + rect.height - 1],
        [rect.x + rect.width - 1, rect.y + rect.height - 1],
        colour,
    );
    line(
        image,
        [rect.x, rect.y],
        [rect.x, rect.y + rect.height - 1],
        colour,
    );
    line(
        image,
        [rect.x + rect.width - 1, rect.y],
        [rect.x + rect.width - 1, rect.y + rect.height - 1],
        colour,
    );
}

fn centered_text(
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

fn fit_font(font: &Font, text: &str, max_width: f32, preferred: f32) -> f32 {
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

fn diamond(image: &mut RgbaImage, cx: i32, cy: i32, rx: i32, ry: i32, colour: [u8; 4]) {
    for y in -ry..=ry {
        let width = (rx as f32 * (1.0 - y.abs() as f32 / ry.max(1) as f32)).round() as i32;
        for x in -width..=width {
            pixel(image, cx + x, cy + y, colour);
        }
    }
}

fn line(image: &mut RgbaImage, start: [i32; 2], end: [i32; 2], colour: [u8; 4]) {
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

fn pixel(image: &mut RgbaImage, x: i32, y: i32, colour: [u8; 4]) {
    if x >= 0 && y >= 0 && x < image.width() as i32 && y < image.height() as i32 {
        image.put_pixel(x as u32, y as u32, Rgba(colour));
    }
}
