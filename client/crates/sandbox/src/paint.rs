//! Small anti-aliased painting for UI art drawn in code: shapes by signed
//! distance (so their edges are smooth at any size), colour gradients, and
//! text rasterised straight from the font into a coverage mask.

use image::RgbaImage;

pub type Colour = [f32; 3];

/// `#rrggbb` as a colour in 0..1.
pub const fn hex(value: u32) -> Colour {
    [
        ((value >> 16) & 0xff) as f32 / 255.0,
        ((value >> 8) & 0xff) as f32 / 255.0,
        (value & 0xff) as f32 / 255.0,
    ]
}

pub fn mix(a: Colour, b: Colour, t: f32) -> Colour {
    let t = t.clamp(0.0, 1.0);
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

/// A colour along `stops` (position, colour), positions rising from 0 to 1.
pub fn gradient(stops: &[(f32, Colour)], t: f32) -> Colour {
    let t = t.clamp(0.0, 1.0);
    for pair in stops.windows(2) {
        let ((p0, c0), (p1, c1)) = (pair[0], pair[1]);
        if t <= p1 {
            return mix(c0, c1, (t - p0) / (p1 - p0).max(f32::EPSILON));
        }
    }
    stops.last().map_or([0.0; 3], |(_, colour)| *colour)
}

/// Lays `colour` at `alpha` over the pixel, keeping what is beneath.
pub fn blend(image: &mut RgbaImage, x: i32, y: i32, colour: Colour, alpha: f32) {
    if x < 0 || y < 0 || x >= image.width() as i32 || y >= image.height() as i32 {
        return;
    }
    let alpha = alpha.clamp(0.0, 1.0);
    if alpha <= 0.0 {
        return;
    }
    let pixel = image.get_pixel_mut(x as u32, y as u32);
    let below = pixel.0[3] as f32 / 255.0;
    let out = alpha + below * (1.0 - alpha);
    for channel in 0..3 {
        let under = pixel.0[channel] as f32 / 255.0;
        let value = (colour[channel] * alpha + under * below * (1.0 - alpha)) / out.max(f32::EPSILON);
        pixel.0[channel] = (value * 255.0).round().clamp(0.0, 255.0) as u8;
    }
    pixel.0[3] = (out * 255.0).round() as u8;
}

/// Coverage of a pixel at signed distance `distance` (inside negative).
pub fn coverage(distance: f32) -> f32 {
    (0.5 - distance).clamp(0.0, 1.0)
}

/// Signed distance to a rounded rectangle (`centre`, half size, radius).
pub fn rounded_rect(x: f32, y: f32, centre: [f32; 2], half: [f32; 2], radius: f32) -> f32 {
    let qx = (x - centre[0]).abs() - (half[0] - radius);
    let qy = (y - centre[1]).abs() - (half[1] - radius);
    let outside = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt();
    outside + qx.max(qy).min(0.0) - radius
}

/// Signed distance to a circle.
pub fn circle(x: f32, y: f32, centre: [f32; 2], radius: f32) -> f32 {
    ((x - centre[0]).powi(2) + (y - centre[1]).powi(2)).sqrt() - radius
}

/// Paints every pixel inside `shape` (a signed distance) with `colour` at
/// `alpha`, smoothing the edge.
pub fn fill(
    image: &mut RgbaImage,
    shape: impl Fn(f32, f32) -> f32,
    colour: impl Fn(f32, f32) -> Colour,
    alpha: f32,
) {
    for y in 0..image.height() as i32 {
        for x in 0..image.width() as i32 {
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            let cover = coverage(shape(px, py));
            if cover > 0.0 {
                blend(image, x, y, colour(px, py), cover * alpha);
            }
        }
    }
}

/// A line `width` wide along the edge of `shape`.
pub fn stroke(
    image: &mut RgbaImage,
    shape: impl Fn(f32, f32) -> f32,
    width: f32,
    colour: impl Fn(f32, f32) -> Colour,
    alpha: f32,
) {
    fill(image, |x, y| shape(x, y).abs() - width / 2.0, colour, alpha);
}

/// A soft glow around `shape`, fading over `reach` pixels.
pub fn glow(image: &mut RgbaImage, shape: impl Fn(f32, f32) -> f32, reach: f32, colour: Colour, alpha: f32) {
    for y in 0..image.height() as i32 {
        for x in 0..image.width() as i32 {
            let distance = shape(x as f32 + 0.5, y as f32 + 0.5);
            if distance > 0.0 && distance < reach {
                let fade = 1.0 - distance / reach;
                blend(image, x, y, colour, alpha * fade * fade);
            }
        }
    }
}

/// Text as a coverage mask the size of `size_box`, its top left at
/// `origin`, with `spacing` extra pixels between letters.
pub fn text_mask(
    font: &fontdue::Font,
    text: &str,
    size: f32,
    spacing: f32,
    origin: [f32; 2],
    size_box: [u32; 2],
) -> Vec<f32> {
    let [width, height] = size_box;
    let mut mask = vec![0.0; (width * height) as usize];
    let ascent = font.horizontal_line_metrics(size).map_or(size * 0.8, |metrics| metrics.ascent);
    let mut pen = origin[0];
    for character in text.chars() {
        let (metrics, bitmap) = font.rasterize(character, size);
        let left = (pen + metrics.xmin as f32).round() as i32;
        let top = (origin[1] + ascent - metrics.ymin as f32 - metrics.height as f32).round() as i32;
        for row in 0..metrics.height {
            for column in 0..metrics.width {
                let (x, y) = (left + column as i32, top + row as i32);
                if x >= 0 && y >= 0 && (x as u32) < width && (y as u32) < height {
                    let index = (y as u32 * width + x as u32) as usize;
                    mask[index] = (mask[index] + bitmap[row * metrics.width + column] as f32 / 255.0).min(1.0);
                }
            }
        }
        pen += metrics.advance_width + spacing;
    }
    mask
}

/// The width `text` takes with `spacing` between letters.
pub fn text_width(font: &fontdue::Font, text: &str, size: f32, spacing: f32) -> f32 {
    let count = text.chars().count() as f32;
    text.chars().map(|character| font.metrics(character, size).advance_width).sum::<f32>()
        + spacing * (count - 1.0).max(0.0)
}

/// Paints a mask at an offset with a colour per pixel.
pub fn paint_mask(
    image: &mut RgbaImage,
    mask: &[f32],
    offset: [i32; 2],
    colour: impl Fn(f32, f32) -> Colour,
    alpha: f32,
) {
    let width = image.width() as i32;
    for (index, cover) in mask.iter().enumerate() {
        if *cover <= 0.0 {
            continue;
        }
        let (x, y) = (index as i32 % width, index as i32 / width);
        blend(image, x + offset[0], y + offset[1], colour(x as f32, y as f32), cover * alpha);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shapes_have_soft_edges() {
        assert_eq!(coverage(-2.0), 1.0);
        assert_eq!(coverage(2.0), 0.0);
        assert!((coverage(0.0) - 0.5).abs() < 1e-6);
        assert!(circle(10.0, 0.0, [0.0, 0.0], 10.0).abs() < 1e-6);
        assert!(rounded_rect(0.0, 0.0, [0.0, 0.0], [10.0, 5.0], 5.0) < 0.0);
        assert!((rounded_rect(15.0, 0.0, [0.0, 0.0], [10.0, 5.0], 5.0) - 5.0).abs() < 1e-4);
        assert_eq!(gradient(&[(0.0, hex(0x000000)), (1.0, hex(0xffffff))], 0.5), [0.5, 0.5, 0.5]);
    }
}
