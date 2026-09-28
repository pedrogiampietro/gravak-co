use crate::labels;
use fontdue::Font;
use image::{Rgba, RgbaImage};

/// Small layout primitive shared by drawing and hit-testing.
///
/// Coordinates are local to the surface that owns the component. Callers can
/// translate the same rectangle by their panel origin instead of maintaining
/// a second set of click-only constants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UiRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl UiRect {
    pub const fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn contains_local(self, point: [f32; 2]) -> bool {
        point[0] >= self.x as f32
            && point[0] < (self.x + self.width) as f32
            && point[1] >= self.y as f32
            && point[1] < (self.y + self.height) as f32
    }
}

/// Draw the bronze text-button used by Character actions and title selection.
/// The visual state is driven by the same `UiRect` used by its hit zone.
pub fn draw_text_button(
    image: &mut RgbaImage,
    font: &Font,
    rect: UiRect,
    label: &str,
    hovered: bool,
    disabled: bool,
) {
    let fill = if disabled {
        [30, 29, 25, 235]
    } else if hovered {
        [111, 82, 37, 255]
    } else {
        [49, 40, 29, 255]
    };
    let text = if disabled {
        [133, 121, 96]
    } else {
        [235, 225, 193]
    };
    for y in rect.y..rect.y + rect.height {
        for x in rect.x..rect.x + rect.width {
            pixel(image, x, y, [fill[0], fill[1], fill[2], fill[3]]);
        }
    }
    line(
        image,
        [rect.x, rect.y],
        [rect.x + rect.width - 1, rect.y],
        [205, 160, 70, 230],
    );
    line(
        image,
        [rect.x, rect.y + rect.height - 1],
        [rect.x + rect.width - 1, rect.y + rect.height - 1],
        [111, 77, 33, 210],
    );

    let text_width = label
        .chars()
        .map(|character| font.metrics(character, 10.0).advance_width)
        .sum::<f32>();
    labels::text_sized(
        image,
        font,
        &[(label, text)],
        [
            rect.x as f32 + ((rect.width as f32 - text_width) * 0.5).max(4.0),
            rect.y as f32 + ((rect.height as f32 - 10.0) * 0.5).max(2.0),
        ],
        10.0,
    );
}

fn pixel(image: &mut RgbaImage, x: i32, y: i32, color: [u8; 4]) {
    if x >= 0 && y >= 0 && x < image.width() as i32 && y < image.height() as i32 {
        image.put_pixel(x as u32, y as u32, Rgba(color));
    }
}

fn line(image: &mut RgbaImage, a: [i32; 2], b: [i32; 2], color: [u8; 4]) {
    let (mut x, mut y) = (a[0], a[1]);
    let (dx, dy) = ((b[0] - x).abs(), -(b[1] - y).abs());
    let (sx, sy) = (if x < b[0] { 1 } else { -1 }, if y < b[1] { 1 } else { -1 });
    let mut error = dx + dy;
    loop {
        pixel(image, x, y, color);
        if x == b[0] && y == b[1] {
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

#[cfg(test)]
mod tests {
    use super::UiRect;

    #[test]
    fn rectangle_uses_same_half_open_edges_as_hit_zones() {
        let rect = UiRect::new(10, 20, 40, 24);
        assert!(rect.contains_local([10.0, 20.0]));
        assert!(rect.contains_local([49.9, 43.9]));
        assert!(!rect.contains_local([50.0, 43.9]));
        assert!(!rect.contains_local([49.9, 44.0]));
    }
}
