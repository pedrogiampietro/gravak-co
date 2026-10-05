//! Writes DXT5 `.dds` files, the format of the 5065 icons, so an icon we
//! import reads in the original client as well as ours.

use image::RgbaImage;

/// `image` as a DXT5 `.dds` without mipmaps. A size that is not a multiple
/// of 4 grows to one with transparent pixels, as DXT readers expect.
pub fn encode_dxt5(image: &RgbaImage) -> Vec<u8> {
    let (width, height) = image.dimensions();
    let (padded_width, padded_height) =
        (width.div_ceil(4).max(1) * 4, height.div_ceil(4).max(1) * 4);
    if (padded_width, padded_height) != (width, height) {
        let mut padded = RgbaImage::new(padded_width, padded_height);
        image::imageops::replace(&mut padded, image, 0, 0);
        return encode_dxt5(&padded);
    }
    let blocks_wide = width.div_ceil(4).max(1);
    let blocks_high = height.div_ceil(4).max(1);
    let mut out = Vec::with_capacity(128 + (blocks_wide * blocks_high * 16) as usize);
    out.extend_from_slice(b"DDS ");
    let mut header = [0u32; 31];
    header[0] = 124; // dwSize
    header[1] = 0x1 | 0x2 | 0x4 | 0x1000 | 0x80000; // CAPS, HEIGHT, WIDTH, PIXELFORMAT, LINEARSIZE
    header[2] = height;
    header[3] = width;
    header[4] = blocks_wide * blocks_high * 16;
    // Pixel format at dword 18: size, flags (FOURCC), fourcc.
    header[18] = 32;
    header[19] = 0x4;
    header[20] = u32::from_le_bytes(*b"DXT5");
    header[26] = 0x1000; // DDSCAPS_TEXTURE
    for value in header {
        out.extend_from_slice(&value.to_le_bytes());
    }
    for block_y in 0..blocks_high {
        for block_x in 0..blocks_wide {
            let mut pixels = [[0u8; 4]; 16];
            for (index, pixel) in pixels.iter_mut().enumerate() {
                let x = (block_x * 4 + index as u32 % 4).min(width.saturating_sub(1));
                let y = (block_y * 4 + index as u32 / 4).min(height.saturating_sub(1));
                *pixel = if width == 0 || height == 0 {
                    [0; 4]
                } else {
                    image.get_pixel(x, y).0
                };
            }
            out.extend_from_slice(&alpha_block(&pixels));
            out.extend_from_slice(&colour_block(&pixels));
        }
    }
    out
}

fn alpha_block(pixels: &[[u8; 4]; 16]) -> [u8; 8] {
    let max = pixels.iter().map(|pixel| pixel[3]).max().unwrap_or(0);
    let min = pixels.iter().map(|pixel| pixel[3]).min().unwrap_or(0);
    let mut block = [0u8; 8];
    block[0] = max;
    block[1] = min;
    if max == min {
        return block;
    }
    // Eight-value mode: a0 > a1, the six between them interpolated.
    let palette: [u32; 8] = std::array::from_fn(|index| match index {
        0 => u32::from(max),
        1 => u32::from(min),
        _ => {
            let step = index as u32 - 1;
            ((7 - step) * u32::from(max) + step * u32::from(min)) / 7
        }
    });
    let mut bits = 0u64;
    for (index, pixel) in pixels.iter().enumerate() {
        let alpha = u32::from(pixel[3]);
        let best = (0..8)
            .min_by_key(|&candidate| palette[candidate].abs_diff(alpha))
            .unwrap_or(0);
        bits |= (best as u64) << (3 * index);
    }
    block[2..].copy_from_slice(&bits.to_le_bytes()[..6]);
    block
}

fn to_565(colour: [u8; 3]) -> u16 {
    (u16::from(colour[0]) >> 3) << 11 | (u16::from(colour[1]) >> 2) << 5 | u16::from(colour[2]) >> 3
}

fn from_565(value: u16) -> [u32; 3] {
    let red = u32::from(value >> 11) & 0x1f;
    let green = u32::from(value >> 5) & 0x3f;
    let blue = u32::from(value) & 0x1f;
    [
        red << 3 | red >> 2,
        green << 2 | green >> 4,
        blue << 3 | blue >> 2,
    ]
}

fn colour_block(pixels: &[[u8; 4]; 16]) -> [u8; 8] {
    // Fully transparent pixels do not pull the endpoints (their colour is
    // never seen), unless the whole block is transparent.
    let visible = pixels
        .iter()
        .filter(|pixel| pixel[3] > 0)
        .collect::<Vec<_>>();
    let sample = if visible.is_empty() {
        pixels.iter().collect()
    } else {
        visible
    };
    let mut low = [255u8; 3];
    let mut high = [0u8; 3];
    for pixel in &sample {
        for channel in 0..3 {
            low[channel] = low[channel].min(pixel[channel]);
            high[channel] = high[channel].max(pixel[channel]);
        }
    }
    let (mut c0, mut c1) = (to_565(high), to_565(low));
    if c0 < c1 {
        std::mem::swap(&mut c0, &mut c1);
    }
    let mut block = [0u8; 8];
    block[..2].copy_from_slice(&c0.to_le_bytes());
    block[2..4].copy_from_slice(&c1.to_le_bytes());
    if c0 == c1 {
        return block;
    }
    let (a, b) = (from_565(c0), from_565(c1));
    let palette = [
        a,
        b,
        std::array::from_fn::<u32, 3, _>(|channel| (2 * a[channel] + b[channel]) / 3),
        std::array::from_fn::<u32, 3, _>(|channel| (a[channel] + 2 * b[channel]) / 3),
    ];
    let mut bits = 0u32;
    for (index, pixel) in pixels.iter().enumerate() {
        let best = (0..4)
            .min_by_key(|&candidate| {
                (0..3)
                    .map(|channel| {
                        let difference =
                            palette[candidate][channel].abs_diff(u32::from(pixel[channel]));
                        difference * difference
                    })
                    .sum::<u32>()
            })
            .unwrap_or(0);
        bits |= (best as u32) << (2 * index);
    }
    block[4..].copy_from_slice(&bits.to_le_bytes());
    block
}

#[cfg(test)]
mod tests {
    use image::{Rgba, RgbaImage};

    #[test]
    fn icons_decode_close_to_the_source() {
        let source = RgbaImage::from_fn(64, 64, |x, y| {
            let alpha = if x < 8 { 0 } else { 255 };
            Rgba([(x * 4) as u8, (y * 4) as u8, 128, alpha])
        });
        let bytes = super::encode_dxt5(&source);
        let decoded = image::load_from_memory_with_format(&bytes, image::ImageFormat::Dds)
            .unwrap()
            .to_rgba8();
        assert_eq!(decoded.dimensions(), (64, 64));
        for (a, b) in source.pixels().zip(decoded.pixels()) {
            assert_eq!(a[3], b[3]);
            if a[3] > 0 {
                for channel in 0..3 {
                    assert!(a[channel].abs_diff(b[channel]) <= 12, "{a:?} vs {b:?}");
                }
            }
        }
    }

    #[test]
    fn odd_sizes_still_decode() {
        let source = RgbaImage::from_pixel(10, 6, Rgba([200, 10, 30, 128]));
        let bytes = super::encode_dxt5(&source);
        let decoded = image::load_from_memory_with_format(&bytes, image::ImageFormat::Dds)
            .unwrap()
            .to_rgba8();
        assert_eq!(decoded.dimensions(), (12, 8));
        assert_eq!(decoded.get_pixel(9, 5)[3], 128);
        assert_eq!(decoded.get_pixel(11, 7)[3], 0);
    }
}
