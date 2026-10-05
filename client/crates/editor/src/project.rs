//! What the editor has open: the installation, our content and the items
//! merged from both.

use gravak_content::Result;
use gravak_content::assets::Installation;
use gravak_content::build::{IconSource, Icons, Merged, fit_icon};
use gravak_content::items::ItemDef;
use gravak_content::itemtype::{ItemType, ItemTypeFile};
use image::RgbaImage;
use std::path::{Path, PathBuf};

pub struct Project {
    pub installation: Installation,
    pub content: PathBuf,
    pub merged: Merged,
    icons: Icons,
}

impl Project {
    pub fn open(installation: &Path, content: &Path) -> Result<Self> {
        let mut installation = Installation::open(installation)?;
        std::fs::create_dir_all(content.join("items"))?;
        let merged = Merged::load(&mut installation, content)?;
        let icons = Icons::new(&mut installation);
        Ok(Self {
            installation,
            content: content.to_path_buf(),
            merged,
            icons,
        })
    }

    /// Reads `content/` again over the same installation.
    pub fn reload(&mut self) -> Result<()> {
        self.merged = Merged::load(&mut self.installation, &self.content)?;
        Ok(())
    }

    pub fn original(&self) -> &ItemTypeFile {
        &self.merged.original
    }

    pub fn item(&self, id: u32) -> Option<&ItemType> {
        self.merged.merged.get(id)
    }

    pub fn def(&self, id: u32) -> Option<&ItemDef> {
        self.merged.items.find(id).map(|(_, def)| def)
    }

    /// What the item is before our definition of it: the original, the base
    /// it names, or blank.
    pub fn underlying(&self, id: u32, base: Option<u32>) -> ItemType {
        let mut item = match base {
            Some(base) => self.item(base).cloned(),
            None => self.original().get(id).cloned(),
        }
        .unwrap_or_else(|| ItemType::blank(id));
        item.set_id(id);
        item
    }

    /// The item's icon: `ours` (a path in `content/`, e.g. not saved yet),
    /// else what the build would give it.
    pub fn icon(&mut self, id: u32, ours: Option<&str>) -> Option<RgbaImage> {
        let source = match ours {
            Some(ours) => IconSource::Content(self.content.join(ours)),
            None => self.icons.source(
                &mut self.installation,
                &self.merged.items,
                &self.merged.original,
                &self.content,
                id,
            )?,
        };
        let bytes = match source {
            IconSource::Content(path) => std::fs::read(path).ok()?,
            IconSource::Installation { path, .. } => self.installation.read(&path).ok()?,
        };
        image::load_from_memory(&bytes)
            .ok()
            .map(|image| image.to_rgba8())
    }
}

/// Copies `source` into `content/icons/<id>.png` as a 64x64 icon: the
/// background taken out (when `clear`), cropped to the drawing and fitted.
/// Returns the path relative to `content`.
pub fn import_icon(content: &Path, id: u32, source: &Path, clear: bool) -> Result<String> {
    let mut image = image::open(source)?.to_rgba8();
    if clear {
        clear_background(&mut image);
    }
    let icon = fit_icon(&crop_to_drawing(&image));
    let relative = format!("icons/{id}.png");
    let target = content.join(&relative);
    std::fs::create_dir_all(target.parent().expect("icons folder"))?;
    icon.save(&target)?;
    Ok(relative)
}

/// The smallest rectangle holding every visible pixel.
fn crop_to_drawing(image: &RgbaImage) -> RgbaImage {
    let visible = image
        .enumerate_pixels()
        .filter(|(_, _, pixel)| pixel[3] > 0);
    let bounds = visible.fold(None, |bounds: Option<(u32, u32, u32, u32)>, (x, y, _)| {
        Some(match bounds {
            None => (x, y, x, y),
            Some((left, top, right, bottom)) => {
                (left.min(x), top.min(y), right.max(x), bottom.max(y))
            }
        })
    });
    match bounds {
        Some((left, top, right, bottom)) => {
            image::imageops::crop_imm(image, left, top, right - left + 1, bottom - top + 1)
                .to_image()
        }
        None => image.clone(),
    }
}

/// Makes the background transparent: starting from the border, every pixel
/// of the background colours (those that fill the outer two-pixel frame, such
/// as white or the two greys of a "transparent" checkerboard in a
/// screenshot). A flood fill, so the same colours inside the drawing stay.
pub fn clear_background(image: &mut RgbaImage) {
    let (width, height) = image.dimensions();
    if width < 3 || height < 3 {
        return;
    }
    let frame = |x: u32, y: u32| x < 2 || y < 2 || x + 2 >= width || y + 2 >= height;
    let mut counts = std::collections::HashMap::<[u8; 3], usize>::new();
    let mut total = 0;
    for (x, y, pixel) in image.enumerate_pixels() {
        if frame(x, y) && pixel[3] > 0 {
            *counts.entry([pixel[0], pixel[1], pixel[2]]).or_default() += 1;
            total += 1;
        }
    }
    let mut keys = counts
        .into_iter()
        .filter(|(_, count)| count * 20 >= total)
        .collect::<Vec<_>>();
    keys.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
    keys.truncate(4);
    let background = |pixel: [u8; 4]| {
        pixel[3] > 0
            && keys
                .iter()
                .any(|(key, _)| (0..3).all(|channel| pixel[channel].abs_diff(key[channel]) <= 6))
    };
    let mut seen = vec![false; (width * height) as usize];
    let mut stack = image
        .enumerate_pixels()
        .filter(|(x, y, _)| frame(*x, *y))
        .map(|(x, y, _)| (x, y))
        .collect::<Vec<_>>();
    while let Some((x, y)) = stack.pop() {
        let index = (y * width + x) as usize;
        if seen[index] || !background(image.get_pixel(x, y).0) {
            continue;
        }
        seen[index] = true;
        image.get_pixel_mut(x, y).0[3] = 0;
        if x > 0 {
            stack.push((x - 1, y));
        }
        if y > 0 {
            stack.push((x, y - 1));
        }
        if x + 1 < width {
            stack.push((x + 1, y));
        }
        if y + 1 < height {
            stack.push((x, y + 1));
        }
    }
}

#[cfg(test)]
mod tests {
    use image::{Rgba, RgbaImage};

    #[test]
    fn backgrounds_clear_from_the_border_only() {
        let mut image = RgbaImage::from_pixel(9, 9, Rgba([255, 255, 255, 255]));
        for x in 2..7 {
            for y in 2..7 {
                image.put_pixel(x, y, Rgba([40, 40, 40, 255]));
            }
        }
        // White inside the drawing, walled off from the border.
        image.put_pixel(4, 4, Rgba([250, 250, 250, 255]));
        super::clear_background(&mut image);
        assert_eq!(image.get_pixel(0, 0)[3], 0);
        assert_eq!(image.get_pixel(3, 3)[3], 255);
        assert_eq!(image.get_pixel(4, 4)[3], 255);
    }

    #[test]
    fn checkerboards_and_frame_lines_clear() {
        let mut image = RgbaImage::from_fn(20, 20, |x, y| {
            if x == 1 {
                Rgba([162, 169, 177, 255])
            } else if (x / 4 + y / 4) % 2 == 0 {
                Rgba([248, 249, 250, 255])
            } else {
                Rgba([230, 231, 232, 255])
            }
        });
        for x in 6..14 {
            for y in 6..14 {
                image.put_pixel(x, y, Rgba([154, 166, 183, 255]));
            }
        }
        super::clear_background(&mut image);
        assert!((0..20).all(|y| image.get_pixel(1, y)[3] == 0));
        assert_eq!(image.get_pixel(17, 17)[3], 0);
        assert_eq!(image.get_pixel(9, 9)[3], 255);
        assert_eq!(image.get_pixel(6, 6)[3], 255);
    }

    #[test]
    fn imported_icons_are_cropped_and_fitted() {
        let root = std::env::temp_dir().join(format!("gravak-editor-icon-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let mut source = RgbaImage::from_pixel(100, 60, Rgba([255, 255, 255, 255]));
        for x in 40..60 {
            for y in 20..40 {
                source.put_pixel(x, y, Rgba([200, 0, 0, 255]));
            }
        }
        source.save(root.join("in.png")).unwrap();
        let relative = super::import_icon(&root, 721000, &root.join("in.png"), true).unwrap();
        assert_eq!(relative, "icons/721000.png");
        let icon = image::open(root.join(&relative)).unwrap().to_rgba8();
        assert_eq!(icon.dimensions(), (64, 64));
        // The red square fills the icon once the white is gone.
        assert_eq!(icon.get_pixel(2, 2)[3], 255);
        assert_eq!(icon.get_pixel(32, 32)[0], 200);
        if let Ok(statue) = std::env::var("GRAVAK_ICON_SAMPLE") {
            let relative =
                super::import_icon(&root, 1, std::path::Path::new(&statue), true).unwrap();
            std::fs::copy(
                root.join(relative),
                std::env::var("GRAVAK_ICON_OUT").unwrap(),
            )
            .unwrap();
        }
        std::fs::remove_dir_all(root).unwrap();
    }
}
