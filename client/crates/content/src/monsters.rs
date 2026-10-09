//! Monsters of our own (`content/monsters.toml`): an original monster's
//! model and motions in a skin we paint, under a look of ours, the way the
//! 5065 makes its own (GreenDevil, 397, is DevilKing's mesh, 197, in green).
//!
//! ```toml
//! [[monster]]
//! look = 951                 # the new look (the server's monster_types.look)
//! base = 211                 # the original whose mesh and motions it takes
//! name = "Esqueleto"         # only for us
//! colour = [120, 150, 200]   # the skin recoloured toward it, keeping its light
//! strength = 0.8             # how far (0 keeps the original colours)
//! light = 1.0                # brighter or darker (not the glow)
//! glow = [90, 220, 255]      # optional: the skin's brightest parts glow so
//! glow_from = 0.7            # from this lightness (0 to 1) up
//! ```
//!
//! The build writes the painted texture (`c3/texture/gravak/monster-<look>.dds`,
//! texture id `<look>000000`), the look's `armor.ini` section (the base's
//! mesh, our texture) and its `3dmotion.ini` lines (the base's motions).

use crate::Result;
use crate::assets::Installation;
use image::{Rgba, RgbaImage};
use serde::Deserialize;
use std::path::Path;

pub const CONFIG: &str = "monsters.toml";
pub const MOTIONS: &str = "ini/3dmotion.ini";

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default)]
    pub monster: Vec<MonsterDef>,
    /// New models (`crate::creatures`).
    #[serde(default)]
    pub creature: Vec<crate::creatures::CreatureDef>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MonsterDef {
    pub look: u32,
    pub base: u32,
    #[serde(default)]
    pub name: String,
    pub colour: [u8; 3],
    #[serde(default = "one")]
    pub strength: f32,
    #[serde(default = "one")]
    pub light: f32,
    #[serde(default)]
    pub glow: Option<[u8; 3]>,
    #[serde(default = "glow_from")]
    pub glow_from: f32,
}

fn one() -> f32 {
    1.0
}

fn glow_from() -> f32 {
    0.7
}

pub fn load(content: &Path) -> Result<Config> {
    let path = content.join(CONFIG);
    if !path.exists() {
        return Ok(Config::default());
    }
    let text = std::fs::read_to_string(&path)?;
    Ok(toml::from_str(&text).map_err(|error| format!("{}: {error}", path.display()))?)
}

/// A look's id in the catalogs (`armor.ini`, `3dmotion.ini`).
fn catalog_id(look: u32) -> u32 {
    look * 1_000_000
}

pub fn texture_path(look: u32) -> String {
    format!("c3/texture/gravak/monster-{look}.dds")
}

/// The `key=value` of `section` in an ini text.
fn section_value(ini: &str, section: &str, key: &str) -> Option<String> {
    let header = format!("[{section}]");
    let mut inside = false;
    for line in ini.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            inside = line.eq_ignore_ascii_case(&header);
            continue;
        }
        if inside && let Some((name, value)) = line.split_once('=') {
            if name.trim().eq_ignore_ascii_case(key) {
                return Some(value.trim().to_owned());
            }
        }
    }
    None
}

/// The value of `id=` in a flat catalog (`3dtexture.ini`).
fn catalog_value(ini: &str, id: u32) -> Option<String> {
    let key = id.to_string();
    ini.lines().find_map(|line| {
        let (name, value) = line.split_once('=')?;
        (name.trim().trim_start_matches('0') == key).then(|| value.trim().replace('\\', "/"))
    })
}

/// Paints `skin` by `def`: each pixel takes `colour` at its own lightness,
/// `strength` of the way; the brightest glow.
pub fn paint(skin: &RgbaImage, def: &MonsterDef) -> RgbaImage {
    let target = def.colour.map(|c| f32::from(c) / 255.0);
    // The target's own lightness, so a pixel keeps its light when recoloured.
    let target_light = (0.3 * target[0] + 0.59 * target[1] + 0.11 * target[2]).max(0.05);
    let glow = def.glow.map(|g| g.map(|c| f32::from(c) / 255.0));
    RgbaImage::from_fn(skin.width(), skin.height(), |x, y| {
        let Rgba([r, g, b, a]) = *skin.get_pixel(x, y);
        let original = [r, g, b].map(|c| f32::from(c) / 255.0);
        let light = 0.3 * original[0] + 0.59 * original[1] + 0.11 * original[2];
        let mut colour: [f32; 3] = std::array::from_fn(|i| {
            let recoloured = target[i] * light / target_light;
            (original[i] * (1.0 - def.strength) + recoloured * def.strength) * def.light
        });
        // The glow keeps its own light: a dark skin can still burn.
        if let Some(glow) = glow {
            let amount = ((light - def.glow_from) / (1.0 - def.glow_from).max(0.01)).clamp(0.0, 1.0);
            colour = std::array::from_fn(|i| colour[i] * (1.0 - amount) + glow[i] * amount);
        }
        let out = colour.map(|c| (c.clamp(0.0, 1.0) * 255.0) as u8);
        Rgba([out[0], out[1], out[2], a])
    })
}

/// What the build adds to the catalogs.
#[derive(Debug, Default)]
pub struct Built {
    pub monsters: usize,
    /// `armor.ini` sections.
    pub armor: String,
    /// `3dtexture.ini` lines.
    pub textures: String,
    /// `3dmotion.ini` lines.
    pub motions: String,
    /// `3dobj.ini` lines (the creatures' models).
    pub models: String,
}

/// Paints the monsters of `content/monsters.toml` into `output` and returns
/// what the catalogs gain.
pub fn build(installation: &mut Installation, content: &Path, output: &Path) -> Result<Built> {
    let config = load(content)?;
    let mut built = Built::default();
    let creatures = crate::creatures::build(content, &config.creature, output)?;
    built.monsters += creatures.creatures;
    built.armor.push_str(&creatures.armor);
    built.textures.push_str(&creatures.textures);
    built.motions.push_str(&creatures.motions);
    built.models.push_str(&creatures.models);
    if config.monster.is_empty() {
        return Ok(built);
    }
    let armor = String::from_utf8_lossy(&installation.read(crate::garments::ARMOR)?).into_owned();
    let textures = String::from_utf8_lossy(&installation.read(crate::garments::TEXTURES)?).into_owned();
    let motions = String::from_utf8_lossy(&installation.read(MOTIONS)?).into_owned();
    for def in &config.monster {
        let base = catalog_id(def.base).to_string();
        let ours = catalog_id(def.look);
        if section_value(&armor, &ours.to_string(), "Mesh0").is_some() {
            return Err(format!("monstro {}: o look {} já existe no armor.ini", def.name, def.look).into());
        }
        let mesh = section_value(&armor, &base, "Mesh0")
            .ok_or_else(|| format!("monstro {}: o look base {} não está no armor.ini", def.name, def.base))?;
        let texture_id = section_value(&armor, &base, "Texture0")
            .and_then(|id| id.parse::<u32>().ok())
            .ok_or_else(|| format!("monstro {}: o look base {} não tem textura", def.name, def.base))?;
        let skin_path = catalog_value(&textures, texture_id)
            .ok_or_else(|| format!("monstro {}: a textura {texture_id} não está no 3dtexture.ini", def.name))?;
        let skin = image::load_from_memory(&installation.read(&skin_path)?)
            .map_err(|error| format!("monstro {}: {skin_path}: {error}", def.name))?
            .to_rgba8();
        let painted = paint(&skin, def);
        let path = texture_path(def.look);
        let file = output.join(&path);
        std::fs::create_dir_all(file.parent().expect("texture folder"))?;
        std::fs::write(file, crate::dds::encode_dxt5(&painted))?;
        built.textures.push_str(&format!("{ours}={path}\r\n"));
        built.armor.push_str(&format!("[{ours}]\r\nPart=1\r\nMesh0={mesh}\r\nTexture0={ours}\r\nMixTex0=0\r\n\r\n"));
        // The base's motions, under our look: `0LLL000AAA`.
        let prefix = format!("{:04}", def.base);
        let mut found = 0;
        for line in motions.lines() {
            let Some((key, value)) = line.split_once('=') else { continue };
            let key = key.trim();
            if key.len() == 10 && key.starts_with(&prefix) && &key[4..7] == "000" {
                built.motions.push_str(&format!("{:04}{}={}\r\n", def.look, &key[4..], value.trim()));
                found += 1;
            }
        }
        if found == 0 {
            return Err(format!("monstro {}: o look base {} não tem animações no 3dmotion.ini", def.name, def.base).into());
        }
        built.monsters += 1;
    }
    Ok(built)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn def() -> MonsterDef {
        MonsterDef {
            look: 951,
            base: 211,
            name: "teste".into(),
            colour: [0, 0, 255],
            strength: 1.0,
            light: 1.0,
            glow: Some([255, 0, 0]),
            glow_from: 0.9,
        }
    }

    #[test]
    fn a_skin_takes_the_colour_and_keeps_its_light() {
        let skin = RgbaImage::from_fn(3, 1, |x, _| match x {
            0 => Rgba([0, 0, 0, 255]),
            1 => Rgba([100, 100, 100, 0]),
            _ => Rgba([255, 255, 255, 255]),
        });
        let painted = paint(&skin, &def());
        assert_eq!(painted.get_pixel(0, 0).0, [0, 0, 0, 255], "black stays black");
        let grey = painted.get_pixel(1, 0).0;
        assert!(grey[2] > grey[0] && grey[3] == 0, "grey turns blue and keeps its alpha: {grey:?}");
        assert_eq!(painted.get_pixel(2, 0).0, [255, 0, 0, 255], "the brightest glows");
    }

    #[test]
    fn catalogs_are_read_by_section_and_id() {
        let armor = "[211000000]\r\nPart=1\r\nMesh0=211000000\r\nTexture0=211000000\r\n";
        assert_eq!(section_value(armor, "211000000", "Mesh0").as_deref(), Some("211000000"));
        assert_eq!(section_value(armor, "200000000", "Mesh0"), None);
        assert_eq!(catalog_value("211000000=c3\\texture\\211000000.dds\n", 211_000_000).as_deref(), Some("c3/texture/211000000.dds"));
    }
}
