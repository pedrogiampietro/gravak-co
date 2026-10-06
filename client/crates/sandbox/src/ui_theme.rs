//! Shared visual language for the client-owned UI.
//!
//! The game keeps the Conquer identity through dark blue panels, bronze trim,
//! jewel-like highlights and restrained gold typography, but no longer
//! depends on unrelated atlas crops to position live content.

use crate::ui::UiRect;
use image::{Rgba, RgbaImage};

pub const PANEL_HIGHLIGHT: [u8; 4] = [224, 173, 73, 255];
pub const TEXT: [u8; 3] = [233, 219, 180];
pub const TEXT_MUTED: [u8; 3] = [171, 161, 137];
pub const TEXT_GOLD: [u8; 3] = [255, 226, 143];

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
