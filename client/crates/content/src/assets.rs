//! Reads files of the 5065 installation the way the client does: a loose file
//! first (names compared without case), then the `data.wdf` and `c3.wdf`
//! archives.

use crate::Result;
use sandbox_data::VirtualPath;
use sandbox_data::wdf::{WdfDirectory, WdfHeader, path_id};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

const LIMIT: u64 = 128 * 1024 * 1024;

pub struct Installation {
    root: PathBuf,
    archives: Vec<(File, WdfDirectory)>,
}

impl Installation {
    pub fn open(root: &Path) -> Result<Self> {
        let root = root
            .canonicalize()
            .map_err(|error| format!("instalação {}: {error}", root.display()))?;
        let mut installation = Self {
            root,
            archives: Vec::new(),
        };
        for name in ["data.wdf", "c3.wdf"] {
            let Some(path) = find_loose(&installation.root, name) else {
                continue;
            };
            let mut file = File::open(path)?;
            let length = file.metadata()?.len();
            let mut bytes = [0; 12];
            file.read_exact(&mut bytes)?;
            let header = WdfHeader::parse(&bytes)?;
            let size = header.directory_length()?;
            if size as u64 > LIMIT {
                return Err("WDF directory exceeds safety limit".into());
            }
            file.seek(SeekFrom::Start(header.directory_offset.into()))?;
            let mut directory = vec![0; size];
            file.read_exact(&mut directory)?;
            let directory = WdfDirectory::parse(header, &directory, length)?;
            installation.archives.push((file, directory));
        }
        Ok(installation)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn read(&mut self, name: &str) -> Result<Vec<u8>> {
        if let Some(path) = find_loose(&self.root, name) {
            if std::fs::metadata(&path)?.len() > LIMIT {
                return Err(format!("{name} exceeds the safety limit").into());
            }
            return Ok(std::fs::read(path)?);
        }
        let id = path_id(&VirtualPath::try_from(name)?)?;
        for (file, directory) in &mut self.archives {
            if let Some(entry) = directory.get(id) {
                if u64::from(entry.size) > LIMIT {
                    return Err(format!("{name} exceeds the safety limit").into());
                }
                file.seek(SeekFrom::Start(entry.offset.into()))?;
                let mut bytes = vec![0; entry.size as usize];
                file.read_exact(&mut bytes)?;
                return Ok(bytes);
            }
        }
        Err(format!("{name} não existe na instalação").into())
    }
}

/// `name` (a `/`-separated path) under `root`, matching each component
/// without case as Windows does.
pub fn find_loose(root: &Path, name: &str) -> Option<PathBuf> {
    let mut path = root.to_path_buf();
    for component in name
        .split(['/', '\\'])
        .filter(|component| !component.is_empty())
    {
        if component == ".." {
            return None;
        }
        let exact = path.join(component);
        path = if exact.exists() {
            exact
        } else {
            std::fs::read_dir(&path)
                .ok()?
                .filter_map(|entry| entry.ok())
                .find(|entry| {
                    entry
                        .file_name()
                        .to_string_lossy()
                        .eq_ignore_ascii_case(component)
                })?
                .path()
        };
    }
    path.is_file().then_some(path)
}

#[cfg(test)]
mod tests {
    #[test]
    fn loose_files_ignore_case() {
        let root = std::env::temp_dir().join(format!("gravak-assets-test-{}", std::process::id()));
        std::fs::create_dir_all(root.join("Ini")).unwrap();
        std::fs::write(root.join("Ini/ItemType.dat"), b"x").unwrap();
        assert!(super::find_loose(&root, "ini/itemtype.dat").is_some());
        assert!(super::find_loose(&root, "ini/missing.dat").is_none());
        assert!(super::find_loose(&root, "../x").is_none());
        let mut installation = super::Installation::open(&root).unwrap();
        assert_eq!(installation.read("INI/itemtype.DAT").unwrap(), b"x");
        std::fs::remove_dir_all(root).unwrap();
    }
}
