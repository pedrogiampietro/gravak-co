//! Our garments: an item of ours wearing the cut (mesh) of an original
//! garment, with a texture of ours for each body. `content/garments/*.toml`:
//!
//! ```toml
//! [[garment]]
//! id = 182995                    # the item (content/items), a garment (18xxxx)
//! look = 181335                  # the original garment whose cut it wears
//! textures = [                   # one per body: 1 and 2 women, 3 and 4 men
//!     "textures/182995-1.png",
//!     "textures/182995-2.png",
//!     "textures/182995-3.png",
//!     "textures/182995-4.png",
//! ]
//! ```
//!
//! The build writes `ini/armor.ini` and `ini/3dtexture.ini` (the originals
//! with our sections and textures after them) and the textures as DDS
//! (`c3/texture/gravak/<id>-<body>.dds`); the client reads them before the
//! installation.

use crate::Result;
use crate::assets::Installation;
use serde::Deserialize;
use std::path::Path;

pub const ARMOR: &str = "ini/armor.ini";
pub const TEXTURES: &str = "ini/3dtexture.ini";
/// The bodies a garment is drawn on: 1 and 2 women, 3 and 4 men.
pub const BODIES: [u32; 4] = [1, 2, 3, 4];

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Garment {
    pub id: u32,
    pub look: u32,
    /// Relative to the content directory, one per body of [`BODIES`].
    pub textures: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
struct File {
    #[serde(default)]
    garment: Vec<Garment>,
}

/// The garments of `content/garments/*.toml`.
pub fn load(content: &Path) -> Result<Vec<Garment>> {
    let folder = content.join("garments");
    let Ok(entries) = std::fs::read_dir(&folder) else {
        return Ok(Vec::new());
    };
    let mut paths: Vec<_> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "toml"))
        .collect();
    paths.sort();
    let mut garments = Vec::new();
    for path in paths {
        let text = std::fs::read_to_string(&path)?;
        let file: File = toml::from_str(&text).map_err(|error| format!("{}: {error}", path.display()))?;
        for garment in file.garment {
            if garment.textures.len() != BODIES.len() {
                return Err(format!("{}: garment {} needs {} textures", path.display(), garment.id, BODIES.len()).into());
            }
            garments.push(garment);
        }
    }
    Ok(garments)
}

/// The `armor.ini` section of `item` on `body`.
pub fn section(body: u32, item: u32) -> u32 {
    body * 1_000_000 + item / 10 * 10
}

/// The `3dtexture.ini` number of garment `item`'s texture on `body`.
pub fn texture_id(body: u32, item: u32) -> u32 {
    90_000_000 + body * 1_000_000 + item % 1_000_000
}

/// Where the build writes that texture.
pub fn texture_path(body: u32, item: u32) -> String {
    format!("c3/texture/gravak/{item}-{body}.dds")
}

/// The lines of section `name` in an ini's text, without its header.
fn section_lines<'a>(text: &'a str, name: &str) -> Option<Vec<&'a str>> {
    let header = format!("[{name}]");
    let mut lines = text.lines();
    lines.find(|line| line.trim() == header)?;
    Some(lines.take_while(|line| !line.trim_start().starts_with('[')).filter(|line| !line.trim().is_empty()).collect())
}

/// Our sections for `garments`, after the cut of each one's look.
pub fn armor_sections(armor: &str, garments: &[Garment]) -> Result<String> {
    let mut out = String::new();
    for garment in garments {
        for body in BODIES {
            let look = section(body, garment.look).to_string();
            let lines = section_lines(armor, &look)
                .ok_or_else(|| format!("garment {}: look {} has no section [{look}]", garment.id, garment.look))?;
            out.push_str(&format!("\r\n[{}]\r\n", section(body, garment.id)));
            for line in lines {
                let line = if line.trim_start().starts_with("Texture0=") {
                    format!("Texture0={}", texture_id(body, garment.id))
                } else {
                    line.trim_end().to_owned()
                };
                out.push_str(&line);
                out.push_str("\r\n");
            }
        }
    }
    Ok(out)
}

/// Writes the merged `armor.ini` and `3dtexture.ini` and the textures under
/// `output`; how many garments.
pub fn build(installation: &mut Installation, content: &Path, output: &Path) -> Result<usize> {
    let garments = load(content)?;
    if garments.is_empty() {
        return Ok(0);
    }
    let armor = installation.read(ARMOR)?;
    let ours = armor_sections(&String::from_utf8_lossy(&armor), &garments)?;
    let mut merged = armor;
    merged.extend_from_slice(ours.as_bytes());
    std::fs::create_dir_all(output.join("ini"))?;
    std::fs::write(output.join(ARMOR), merged)?;

    let mut textures = installation.read(TEXTURES)?;
    if !textures.ends_with(b"\n") {
        textures.extend_from_slice(b"\r\n");
    }
    for garment in &garments {
        for (body, source) in BODIES.into_iter().zip(&garment.textures) {
            let image = image::open(content.join(source))
                .map_err(|error| format!("garment {}: {source}: {error}", garment.id))?
                .to_rgba8();
            let path = texture_path(body, garment.id);
            let file = output.join(&path);
            std::fs::create_dir_all(file.parent().expect("texture folder"))?;
            std::fs::write(file, crate::dds::encode_dxt5(&image))?;
            textures.extend_from_slice(format!("{}={path}\r\n", texture_id(body, garment.id)).as_bytes());
        }
    }
    std::fs::write(output.join(TEXTURES), textures)?;
    Ok(garments.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_garment_takes_the_cut_of_its_look_and_a_texture_of_its_own() {
        let armor = "[1181330]\nPart=1\nMesh0=1137050\nTexture0=1137350\nMixTex0=0\n\n[2181330]\nPart=1\nMesh0=2137050\nTexture0=2137350\n\
                     [3181330]\nMesh0=3137050\nTexture0=3137350\n[4181330]\nMesh0=4137050\nTexture0=4137350\n";
        let garment = Garment { id: 182995, look: 181335, textures: vec![String::new(); 4] };
        let ours = armor_sections(armor, &[garment]).unwrap();
        assert!(ours.contains("[1182990]\r\nPart=1\r\nMesh0=1137050\r\nTexture0=91182995\r\nMixTex0=0\r\n"), "{ours}");
        assert!(ours.contains("[4182990]\r\nMesh0=4137050\r\nTexture0=94182995\r\n"), "{ours}");
        assert_eq!(texture_path(3, 182995), "c3/texture/gravak/182995-3.dds");
        let missing = Garment { id: 182995, look: 181345, textures: vec![String::new(); 4] };
        assert!(armor_sections(armor, &[missing]).is_err());
    }
}
