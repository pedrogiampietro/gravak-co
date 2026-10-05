//! Merges `content/` over the installation into `content/build`.
//!
//! The output mirrors the installation (`client/ini/itemtype.dat`,
//! `client/data/ItemMinIcon/<id>.dds`, ...): the client reads it before the
//! installation (`GRAVAK_CONTENT=content/build/client`) and the server takes
//! its `itemtype.dat` (`ITEMTYPE_DATA`).

use crate::Result;
use crate::assets::Installation;
use crate::items::{ItemSet, apply};
use crate::itemtype::ItemTypeFile;
use image::{RgbaImage, imageops};
use std::path::{Path, PathBuf};

pub const ITEMTYPE: &str = "ini/itemtype.dat";
pub const ICON_SIZE: u32 = 64;
/// Marks a folder the build made, so it can be emptied without fear.
const MARKER: &str = ".gravak-build";

/// The repository's `content` folder, from the working directory or the
/// executable upwards.
pub fn default_content_dir() -> PathBuf {
    let starts = [std::env::current_dir().ok(), std::env::current_exe().ok()];
    for start in starts.into_iter().flatten() {
        for directory in start.ancestors() {
            if directory.join("content").is_dir() && directory.join("client").is_dir() {
                return directory.join("content");
            }
        }
    }
    PathBuf::from("content")
}

/// `GRAVAK_ASSETS`, else `R:\5065`.
pub fn default_installation() -> PathBuf {
    std::env::var_os("GRAVAK_ASSETS").map_or_else(|| PathBuf::from(r"R:\5065"), PathBuf::from)
}

pub fn client_output(content: &Path) -> PathBuf {
    content.join("build").join("client")
}

/// The original items with ours merged in.
pub struct Merged {
    pub original: ItemTypeFile,
    pub merged: ItemTypeFile,
    pub items: ItemSet,
}

impl Merged {
    pub fn load(installation: &mut Installation, content: &Path) -> Result<Self> {
        let original = ItemTypeFile::from_encrypted(&installation.read(ITEMTYPE)?);
        let items = ItemSet::load(content)?;
        let mut merged = original.clone();
        apply(&items.defs().collect::<Vec<_>>(), &mut merged)?;
        Ok(Self {
            original,
            merged,
            items,
        })
    }
}

#[derive(Debug, Default)]
pub struct Report {
    pub added: usize,
    pub changed: usize,
    pub icons: usize,
    /// Maps with objects of ours, and how many objects.
    pub maps: usize,
    pub objects: usize,
    pub output: PathBuf,
}

pub fn build(installation: &mut Installation, content: &Path) -> Result<Report> {
    let Merged {
        original,
        merged,
        items,
    } = Merged::load(installation, content)?;
    let output = client_output(content);
    reset_output(&output)?;

    let itemtype = output.join(ITEMTYPE);
    std::fs::create_dir_all(itemtype.parent().expect("ini folder"))?;
    std::fs::write(&itemtype, merged.to_encrypted(original.item_lines()))?;

    let mut report = Report {
        output,
        ..Report::default()
    };
    let icons = Icons::new(installation);
    for def in items.defs() {
        if original.contains(def.id) {
            report.changed += 1;
        } else {
            report.added += 1;
        }
        let dds = match icons.source(installation, &items, &original, content, def.id) {
            Some(IconSource::Content(source)) => {
                let image = image::open(&source).map_err(|error| {
                    format!("item {}: ícone {}: {error}", def.id, source.display())
                })?;
                crate::dds::encode_dxt5(&fit_icon(&image.to_rgba8()))
            }
            // The installation already shows an original item's own icon.
            Some(IconSource::Installation { id, .. }) if id == def.id => continue,
            Some(IconSource::Installation { path, .. }) => {
                let bytes = installation.read(&path)?;
                if path.to_ascii_lowercase().ends_with(".dds") {
                    bytes
                } else {
                    crate::dds::encode_dxt5(&fit_icon(&image::load_from_memory(&bytes)?.to_rgba8()))
                }
            }
            None => continue,
        };
        for folder in ["data/ItemMinIcon", "data/MapItemIcon"] {
            let path = report.output.join(folder).join(format!("{}.dds", def.id));
            std::fs::create_dir_all(path.parent().expect("icon folder"))?;
            std::fs::write(path, &dds)?;
        }
        report.icons += 1;
    }
    (report.maps, report.objects) = crate::maps::build(content, &report.output)?;
    Ok(report)
}

/// Where an item's icon comes from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IconSource {
    /// An image in `content/`.
    Content(PathBuf),
    /// A file of the installation, the icon of item `id`.
    Installation { id: u32, path: String },
}

/// Finds icons the way the client does: `data/ItemMinIcon/<id>.dds`, then
/// `data/MapItemIcon/<id>.dds`, then `ani/ItemMinIcon.Ani`.
pub struct Icons {
    ani: String,
}

impl Icons {
    pub fn new(installation: &mut Installation) -> Self {
        let ani = installation
            .read("ani/ItemMinIcon.Ani")
            .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
            .unwrap_or_default();
        Self { ani }
    }

    /// The icon of item `id` in the installation.
    pub fn installed(&self, installation: &mut Installation, id: u32) -> Option<String> {
        let mut paths = vec![
            format!("data/ItemMinIcon/{id}.dds"),
            format!("data/MapItemIcon/{id}.dds"),
        ];
        paths.extend(ani_frame(&self.ani, &format!("[Item{id}]")));
        paths
            .into_iter()
            .find(|path| installation.read(path).is_ok())
    }

    /// The item's icon: the one its definition names; an original item's
    /// own; else, for a new item, its base's, up the chain.
    pub fn source(
        &self,
        installation: &mut Installation,
        items: &ItemSet,
        original: &ItemTypeFile,
        content: &Path,
        id: u32,
    ) -> Option<IconSource> {
        let mut current = id;
        for _ in 0..32 {
            let def = items.find(current).map(|(_, def)| def);
            if let Some(icon) = def.and_then(|def| def.icon.as_ref()) {
                return Some(IconSource::Content(content.join(icon)));
            }
            if original.contains(current) {
                let path = self.installed(installation, current)?;
                return Some(IconSource::Installation { id: current, path });
            }
            current = def?.base?;
        }
        None
    }
}

/// `Frame0` of an `.Ani` section.
fn ani_frame(ani: &str, section: &str) -> Option<String> {
    let mut inside = false;
    for line in ani.lines().map(str::trim) {
        if line.starts_with('[') {
            if inside {
                return None;
            }
            inside = line.eq_ignore_ascii_case(section);
        } else if inside && let Some(frame) = line.strip_prefix("Frame0=") {
            return Some(frame.trim().to_owned());
        }
    }
    None
}

/// The image scaled into a transparent 64x64 square, centred.
pub fn fit_icon(image: &RgbaImage) -> RgbaImage {
    let (width, height) = image.dimensions();
    if (width, height) == (ICON_SIZE, ICON_SIZE) {
        return image.clone();
    }
    let scale = f64::from(ICON_SIZE) / f64::from(width.max(height).max(1));
    let new_width = ((f64::from(width) * scale).round() as u32).clamp(1, ICON_SIZE);
    let new_height = ((f64::from(height) * scale).round() as u32).clamp(1, ICON_SIZE);
    let scaled = imageops::resize(
        image,
        new_width,
        new_height,
        imageops::FilterType::CatmullRom,
    );
    let mut icon = RgbaImage::new(ICON_SIZE, ICON_SIZE);
    imageops::overlay(
        &mut icon,
        &scaled,
        i64::from((ICON_SIZE - new_width) / 2),
        i64::from((ICON_SIZE - new_height) / 2),
    );
    icon
}

fn reset_output(output: &Path) -> Result<()> {
    if output.exists() {
        if !output.join(MARKER).exists() {
            return Err(format!(
                "{} já existe e não foi criada pelo build; apague-a ou escolha outra pasta",
                output.display()
            )
            .into());
        }
        std::fs::remove_dir_all(output)?;
    }
    std::fs::create_dir_all(output)?;
    std::fs::write(
        output.join(MARKER),
        b"Gerado por gravak-content build; nao edite.\n",
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::itemtype::encrypt;

    #[test]
    fn builds_items_and_icons_over_an_installation() {
        let root = std::env::temp_dir().join(format!("gravak-build-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let installation_dir = root.join("5065");
        let content = root.join("content");
        std::fs::create_dir_all(installation_dir.join("ini")).unwrap();
        std::fs::create_dir_all(content.join("items")).unwrap();
        std::fs::create_dir_all(content.join("icons")).unwrap();

        let mut fields = vec!["0".to_owned(); 39];
        fields[0] = "723017".to_owned();
        fields[1] = "ExpBall".to_owned();
        let mut original = format!("1\r\n{}\r\n", fields.join(" ")).into_bytes();
        encrypt(&mut original);
        std::fs::write(installation_dir.join("ini/itemtype.dat"), &original).unwrap();
        RgbaImage::from_pixel(120, 80, image::Rgba([10, 200, 30, 255]))
            .save(content.join("icons/statue.png"))
            .unwrap();
        std::fs::write(
            content.join("items/721000.toml"),
            "[[item]]\nid = 721000\nbase = 723017\nicon = \"icons/statue.png\"\nname = \"Gravak Statue\"\n",
        )
        .unwrap();

        let mut installation = Installation::open(&installation_dir).unwrap();
        let report = build(&mut installation, &content).unwrap();
        assert_eq!((report.added, report.changed, report.icons), (1, 0, 1));
        let built = std::fs::read(report.output.join(ITEMTYPE)).unwrap();
        let built = ItemTypeFile::from_encrypted(&built);
        assert_eq!(built.get(721000).unwrap().name(), "Gravak Statue");
        assert!(built.header().starts_with(b"2"));
        let icon = image::open(report.output.join("data/ItemMinIcon/721000.dds")).unwrap();
        assert_eq!((icon.width(), icon.height()), (64, 64));

        // A copy of the statue takes its icon; a copy of an original item
        // takes the installation's.
        std::fs::create_dir_all(installation_dir.join("data/ItemMinIcon")).unwrap();
        std::fs::write(
            installation_dir.join("data/ItemMinIcon/723017.dds"),
            b"original icon",
        )
        .unwrap();
        std::fs::write(
            content.join("items/more.toml"),
            "[[item]]\nid = 721010\nbase = 721000\n\n[[item]]\nid = 723027\nbase = 723017\n",
        )
        .unwrap();
        let mut installation = Installation::open(&installation_dir).unwrap();
        let report = build(&mut installation, &content).unwrap();
        assert_eq!(report.icons, 3);
        let icons = report.output.join("data/ItemMinIcon");
        assert_eq!(
            std::fs::read(icons.join("721010.dds")).unwrap(),
            std::fs::read(icons.join("721000.dds")).unwrap()
        );
        assert_eq!(
            std::fs::read(icons.join("723027.dds")).unwrap(),
            b"original icon"
        );

        // A statue on map 1002.
        std::fs::create_dir_all(content.join("maps")).unwrap();
        std::fs::write(
            content.join("maps/1002.toml"),
            "[[object]]\nimage = \"icons/statue.png\"\nx = 10\ny = 20\nblock = [[0, 0]]\n",
        )
        .unwrap();
        let report = build(&mut installation, &content).unwrap();
        assert_eq!((report.maps, report.objects), (1, 1));
        let objects = std::fs::read_to_string(report.output.join("gravak/maps/1002.txt")).unwrap();
        let objects = sandbox_data::map::GravakObjects::parse(&objects).unwrap();
        assert_eq!(objects.objects[0].image, "gravak/icons/statue.png");
        assert_eq!(objects.objects[0].offset, [-60, -80]);
        assert_eq!(objects.blocked, [[10, 20]]);
        assert!(report.output.join("gravak/icons/statue.png").is_file());

        // Again over its own output, and never over a folder it did not make.
        build(&mut installation, &content).unwrap();
        let foreign = root.join("foreign");
        std::fs::create_dir_all(foreign.join("build/client")).unwrap();
        assert!(build(&mut installation, &foreign).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn ani_frames_come_from_their_section() {
        let ani = "[Item1]\nFrameAmount=1\nFrame0=data/a.dds\n[Item2]\nFrame0=data/b.dds\n";
        assert_eq!(ani_frame(ani, "[Item2]").as_deref(), Some("data/b.dds"));
        assert_eq!(ani_frame(ani, "[Item3]"), None);
    }

    #[test]
    fn icons_keep_their_shape() {
        let icon = fit_icon(&RgbaImage::from_pixel(
            128,
            64,
            image::Rgba([255, 0, 0, 255]),
        ));
        assert_eq!(icon.dimensions(), (64, 64));
        assert_eq!(icon.get_pixel(32, 2)[3], 0);
        assert_eq!(icon.get_pixel(32, 32)[3], 255);
    }
}
