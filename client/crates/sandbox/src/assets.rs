// Source: @digitalm1nd on x.com / _dek on Discord — Preservation Conquer project — https://discord.gg/CvKPXEHYRY
use sandbox_data::{
    VirtualPath,
    wdf::{WdfDirectory, WdfHeader, path_id},
};
use std::{
    collections::HashMap,
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
};

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

pub struct Assets {
    root: PathBuf,
    /// Our built content (`GRAVAK_CONTENT`, e.g. `content/build/client`),
    /// read before the installation.
    overlay: Option<PathBuf>,
    archives: Vec<(File, WdfDirectory)>,
    resolved: HashMap<String, PathBuf>,
}

#[cfg(test)]
mod preview {
    #[test]
    #[ignore = "Exports installed status artwork for visual inspection"]
    fn export_status_art() -> super::Result<()> {
        let mut assets = super::Assets::open(std::path::Path::new("R:/5065"))?;
        let out = std::path::Path::new("R:/gravak-co/docs/ui-preview");
        std::fs::create_dir_all(out)?;
        for name in [
            "DialogQuery",
            "DialogQuery_Role",
            "DialogQuery_Role_More",
            "DialogQuery_Weapon",
            "DialogQuery_Mission",
            "DialogNpcEquip",
            "DialogMpcGoods",
            "DialogDepot",
            "CloseBtnS",
            "CloseBtnSclick",
            "ad_MainBG",
            "hotkeysee",
            "hotkeyseeClick",
            "hotkeyseeUNClick",
            "TalkBtnAct",
            "TalkBtnActClick",
            "MpcGoodsMoneyBtn",
            "PublicBtnMoney",
            "PublicBtnMoney01",
            "GeneralBG",
            "GeneralShuxingBG",
            "RoleBtnBase",
            "RoleBtnMore",
            "RoleBtnI",
            "BattleBtn",
        ] {
            match assets.image(&format!("data/main/{name}.dds")) {
                Ok(img) => {
                    println!("{name}: {:?}", img.dimensions());
                    img.save(out.join(format!("{name}.png")))?;
                }
                Err(err) => println!("{name}: {err}"),
            }
        }
        let shell = crate::original_ui::component(
            &mut assets,
            crate::original_ui::CharacterComponent::Shell,
        )?;
        let role = crate::original_ui::component(
            &mut assets,
            crate::original_ui::CharacterComponent::Role,
        )?;
        let weapon = crate::original_ui::component(
            &mut assets,
            crate::original_ui::CharacterComponent::Weapon,
        )?;
        let equipment = crate::original_ui::component(
            &mut assets,
            crate::original_ui::CharacterComponent::Equipment,
        )?;
        let mut role_screen = image::RgbaImage::from_pixel(
            crate::original_ui::CHARACTER_RECT[2],
            crate::original_ui::CHARACTER_RECT[3] + 29,
            image::Rgba([0, 0, 0, 0]),
        );
        image::imageops::overlay(&mut role_screen, &shell, 0, 0);
        // GUI.ini [131-132] places the role component inside the shell.
        image::imageops::overlay(&mut role_screen, &role, 40, 55);
        role_screen.save(out.join("original-character-role.png"))?;

        let mut weapon_screen = role_screen.clone();
        image::imageops::overlay(&mut weapon_screen, &weapon, 40, 55);
        weapon_screen.save(out.join("original-character-weapon.png"))?;

        let mut equipment_screen = image::RgbaImage::from_pixel(
            crate::original_ui::CHARACTER_RECT[2],
            equipment.height(),
            image::Rgba([0, 0, 0, 0]),
        );
        image::imageops::overlay(&mut equipment_screen, &equipment, 0, 0);
        equipment_screen.save(out.join("original-character-equipment.png"))?;
        Ok(())
    }

    /// Exports every frame of the `ani/Control.Ani` (or `GRAVAK_ANI_FILE`)
    /// sections whose name starts
    /// with one of the comma-separated prefixes in `GRAVAK_ANI_SECTIONS` to
    /// `GRAVAK_ANI_EXPORT`, e.g. `Dialog4,Button4,Check4,Progress4`.
    #[test]
    #[ignore = "Exports original control artwork for visual inspection"]
    fn export_control_ani_sections() -> super::Result<()> {
        let mut assets = super::Assets::open(std::path::Path::new("R:/5065"))?;
        let out = std::path::PathBuf::from(std::env::var("GRAVAK_ANI_EXPORT")?);
        let prefixes = std::env::var("GRAVAK_ANI_SECTIONS")?;
        let prefixes = prefixes.split(',').map(str::trim).collect::<Vec<_>>();
        std::fs::create_dir_all(&out)?;
        let file = std::env::var("GRAVAK_ANI_FILE").unwrap_or_else(|_| "ani/Control.Ani".into());
        let ani = assets.read(&file)?;
        let ani = String::from_utf8_lossy(&ani);
        let mut section = String::new();
        for line in ani.lines().map(str::trim) {
            if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
                section = name.to_owned();
                continue;
            }
            let Some((key, path)) = line.split_once('=') else {
                continue;
            };
            let Some(frame) = key
                .strip_prefix("Frame")
                .filter(|f| f.parse::<u32>().is_ok())
            else {
                continue;
            };
            if !prefixes.iter().any(|p| section.starts_with(p)) {
                continue;
            }
            let file = format!("{section}_{frame}");
            match assets.image(path.trim()) {
                Ok(image) => {
                    println!("{file}: {path} {}x{}", image.width(), image.height());
                    image.save(out.join(format!("{file}.png")))?;
                }
                Err(error) => println!("{file}: {path} {error}"),
            }
        }
        Ok(())
    }

    #[test]
    #[ignore = "Catalogues the installed 5065 main UI assets"]
    fn catalog_main_ui_assets() -> super::Result<()> {
        let mut assets = super::Assets::open(std::path::Path::new("R:/5065"))?;
        let root = assets.root_path().join("data/main");
        let mut names = std::fs::read_dir(root)?
            .filter_map(|entry| entry.ok())
            .filter_map(|entry| {
                let path = entry.path();
                if path.extension().is_some_and(|extension| {
                    extension.to_string_lossy().eq_ignore_ascii_case("dds")
                }) {
                    path.file_stem()
                        .map(|stem| stem.to_string_lossy().into_owned())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        names.sort_unstable();
        for name in names {
            match assets.image(&format!("data/main/{name}.dds")) {
                Ok(image) => println!("{name}: {}x{}", image.width(), image.height()),
                Err(error) => println!("{name}: {error}"),
            }
        }
        Ok(())
    }
}
impl Assets {
    pub fn open(root: &Path) -> Result<Self> {
        let root = root.canonicalize()?;
        if !root.is_dir() {
            return Err("--assets must name an installation directory".into());
        }
        let overlay = std::env::var_os("GRAVAK_CONTENT")
            .map(PathBuf::from)
            .filter(|path| path.is_dir())
            .and_then(|path| path.canonicalize().ok());
        let mut assets = Self {
            root,
            overlay,
            archives: Vec::new(),
            resolved: HashMap::new(),
        };
        for name in ["data.wdf", "c3.wdf"] {
            if let Some(path) = assets.loose(name)? {
                let mut file = File::open(path)?;
                let length = file.metadata()?.len();
                let mut bytes = [0; 12];
                file.read_exact(&mut bytes)?;
                let header = WdfHeader::parse(&bytes)?;
                let size = header.directory_length()?;
                if size > 64 * 1024 * 1024 {
                    return Err("WDF directory exceeds safety limit".into());
                }
                file.seek(SeekFrom::Start(header.directory_offset as u64))?;
                let mut bytes = vec![0; size];
                file.read_exact(&mut bytes)?;
                assets
                    .archives
                    .push((file, WdfDirectory::parse(header, &bytes, length)?));
            }
        }
        Ok(assets)
    }
    pub fn root_path(&self) -> &Path {
        &self.root
    }
    /// The built content read before the installation, when there is one.
    pub fn overlay_path(&self) -> Option<&Path> {
        self.overlay.as_deref()
    }
    fn loose(&mut self, name: &str) -> Result<Option<PathBuf>> {
        let virtual_path = VirtualPath::try_from(name)?;
        let name = virtual_path.as_str();
        if let Some(path) = self.resolved.get(name) {
            return Ok(Some(path.clone()));
        }
        if let Some(overlay) = &self.overlay
            && let Some(path) = find_under(overlay, name)?
        {
            self.resolved.insert(name.to_owned(), path.clone());
            return Ok(Some(path));
        }
        let Some(path) = find_under(&self.root, name)? else {
            return Ok(None);
        };
        self.resolved.insert(name.to_owned(), path.clone());
        Ok(Some(path))
    }
    pub fn read(&mut self, name: &str) -> Result<Vec<u8>> {
        let virtual_path = VirtualPath::try_from(name)?;
        if let Some(path) = self.loose(name)? {
            let file = File::open(path)?;
            if file.metadata()?.len() > 128 * 1024 * 1024 {
                return Err("asset exceeds 128 MiB safety limit".into());
            }
            let mut bytes = Vec::new();
            file.take(128 * 1024 * 1024).read_to_end(&mut bytes)?;
            return Ok(bytes);
        }
        let id = path_id(&virtual_path)?;
        for (file, directory) in &mut self.archives {
            if let Some(entry) = directory.get(id) {
                if entry.size > 128 * 1024 * 1024 {
                    return Err("archive entry exceeds safety limit".into());
                }
                file.seek(SeekFrom::Start(entry.offset as u64))?;
                let mut bytes = vec![0; entry.size as usize];
                file.read_exact(&mut bytes)?;
                return Ok(bytes);
            }
        }
        Err(format!("Missing installed asset: {name}").into())
    }
    pub fn image(&mut self, name: &str) -> Result<image::RgbaImage> {
        let bytes = self.read(name)?;

        let mut reader =
            image::ImageReader::new(std::io::Cursor::new(bytes)).with_guessed_format()?;
        if reader.format().is_none() {
            reader.set_format(image::ImageFormat::from_path(name)?);
        }
        Ok(reader.decode()?.into_rgba8())
    }
}

/// `name` (a virtual `a/b/c` path) under `root`, each component matched
/// without case as Windows does; a symlink may not lead out of `root`.
fn find_under(root: &Path, name: &str) -> Result<Option<PathBuf>> {
    let mut path = root.to_path_buf();
    for component in name.split('/') {
        let exact = path.join(component);
        path = if exact.exists() {
            exact
        } else {
            let Some(entry) = std::fs::read_dir(&path)?.filter_map(|e| e.ok()).find(|e| {
                e.file_name()
                    .to_string_lossy()
                    .eq_ignore_ascii_case(component)
            }) else {
                return Ok(None);
            };
            entry.path()
        };
        path = path.canonicalize()?;
        if !path.starts_with(root) {
            return Err("asset symlink leaves installation".into());
        }
    }
    Ok(path.is_file().then_some(path))
}
