//! Our weapon accessories: an item of ours worn in an accessory slot that
//! dresses the weapon of its kind in a look of ours, the model of an
//! original weapon with a texture of ours and an effect always shown on it.
//! `content/accessories/*.toml`:
//!
//! ```toml
//! [[accessory]]
//! id = 370005                        # the item (content/items): 350xxx two-handed,
//!                                    # 360xxx one-handed, 370xxx bow, 380xxx shield
//! model = 500279                     # the original weapon whose model it wears
//! texture = "textures/370005.png"    # optional: the model's own texture without it
//! mesh = "models/370005.obj"         # optional: a model of ours in place of the
//!                                    # model's mesh (see `c3mesh`), its bones kept
//! aura = "Bow130"                    # optional: a 3DEffect.ini effect shown on it
//!                                    # ("none" for none; the model's own without it)
//! aura_tint = "ice"                  # optional: the aura recoloured (see AURA_TINTS)
//! sparkles = 24                      # optional: little stars twinkling over it, in
//!                                    # the aura's tint (the client draws them)
//! ```
//!
//! The look is a weapon id of the model's kind no original uses
//! ([`look_id`]): the client draws, poses and shoots it as that kind of
//! weapon. The build writes, after the originals, its `weapon.ini` section
//! (the model's, with our texture and mesh; the mesh, the model's C3 with
//! ours in it, in `c3/mesh/gravak` and `3dobj.ini`), its `WeaponMotion.ini` poses,
//! `Action3DEffect.ini` flashes and aura, and `3DFlyingObj.ini` arrows (the
//! model's), and `ini/accessory.ini` (`[<id>]` `Look=`), which the server
//! and the client read to know what each accessory looks like.

use crate::Result;
use crate::assets::Installation;
use serde::Deserialize;
use std::collections::HashSet;
use std::path::Path;

pub const WEAPONS: &str = "ini/weapon.ini";
pub const MOTIONS: &str = "ini/WeaponMotion.ini";
pub const EFFECTS: &str = "ini/Action3DEffect.ini";
pub const ARROWS: &str = "ini/3DFlyingObj.ini";
pub const LOOKS: &str = "ini/accessory.ini";
pub const MODELS: &str = "ini/3dobj.ini";
/// The colours an aura may be tinted (`magic::tint` in the client).
pub const AURA_TINTS: [&str; 3] = ["ice", "legendary", "mythic"];

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Accessory {
    pub id: u32,
    pub model: u32,
    /// Relative to the content directory.
    #[serde(default)]
    pub texture: Option<String>,
    /// An OBJ, relative to the content directory.
    #[serde(default)]
    pub mesh: Option<String>,
    #[serde(default)]
    pub aura: Option<String>,
    #[serde(default)]
    pub aura_tint: Option<String>,
    #[serde(default)]
    pub sparkles: u32,
}

#[derive(Debug, Default, Deserialize)]
struct File {
    #[serde(default)]
    accessory: Vec<Accessory>,
}

/// The kind of weapon an accessory family dresses, as the weapon families
/// it fits: 350 two-handed, 360 one-handed, 370 bows, 380 shields.
pub fn fits(accessory: u32, weapon: u32) -> bool {
    let family = weapon / 1_000;
    match accessory / 1_000 {
        350 => (501..=599).contains(&family),
        360 => (410..=499).contains(&family),
        370 => family == 500,
        380 => family == 900,
        _ => false,
    }
}

/// The weapon id accessory `id` is drawn as: in its model's family, past
/// every original (`xxx900`..`xxx999`), by the accessory's last two digits.
pub fn look_id(id: u32, model: u32) -> u32 {
    model / 1_000 * 1_000 + 900 + id % 100
}

/// The `3dtexture.ini` number of accessory `id`'s texture.
pub fn texture_id(id: u32) -> u32 {
    95_000_000 + id % 1_000_000
}

/// Where the build writes that texture.
pub fn texture_path(id: u32) -> String {
    format!("c3/texture/gravak/{id}.dds")
}

/// The `3dobj.ini` number of accessory `id`'s mesh.
pub fn mesh_id(id: u32) -> u32 {
    95_000_000 + id % 1_000_000
}

/// Where the build writes that mesh.
pub fn mesh_path(id: u32) -> String {
    format!("c3/mesh/gravak/{id}.c3")
}

/// The C3 file of original weapon `weapon`'s mesh (`weapon.ini` `Mesh0`,
/// through `3dobj.ini`).
pub fn model_path(installation: &mut Installation, weapon: u32) -> Result<String> {
    let weapons = String::from_utf8_lossy(&installation.read(WEAPONS)?).into_owned();
    let mesh = section_lines(&weapons, &weapon.to_string())
        .and_then(|lines| lines.iter().find_map(|line| line.trim().strip_prefix("Mesh0=").map(str::to_owned)))
        .ok_or_else(|| format!("weapon {weapon} has no Mesh0 in weapon.ini"))?;
    let models = String::from_utf8_lossy(&installation.read(MODELS)?).into_owned();
    models
        .lines()
        .find_map(|line| {
            let (id, path) = line.split_once('=')?;
            (id.trim() == mesh.trim()).then(|| path.trim().to_owned())
        })
        .ok_or_else(|| format!("mesh {mesh} of weapon {weapon} is not in 3dobj.ini").into())
}

/// The accessories of `content/accessories/*.toml`.
pub fn load(content: &Path) -> Result<Vec<Accessory>> {
    let folder = content.join("accessories");
    let Ok(entries) = std::fs::read_dir(&folder) else {
        return Ok(Vec::new());
    };
    let mut paths: Vec<_> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "toml"))
        .collect();
    paths.sort();
    let mut accessories: Vec<Accessory> = Vec::new();
    let mut looks = HashSet::new();
    for path in paths {
        let text = std::fs::read_to_string(&path)?;
        let file: File = toml::from_str(&text).map_err(|error| format!("{}: {error}", path.display()))?;
        for accessory in file.accessory {
            if !fits(accessory.id, accessory.model) {
                return Err(format!(
                    "{}: accessory {} cannot wear {}: 350xxx dresses two-handed weapons, 360xxx one-handed, 370xxx bows, 380xxx shields",
                    path.display(),
                    accessory.id,
                    accessory.model
                )
                .into());
            }
            if let Some(tint) = accessory.aura_tint.as_deref().filter(|tint| !AURA_TINTS.contains(tint)) {
                return Err(format!("{}: accessory {}: aura_tint {tint} is none of {AURA_TINTS:?}", path.display(), accessory.id).into());
            }
            if !looks.insert(look_id(accessory.id, accessory.model)) {
                return Err(format!(
                    "{}: accessory {} has the same last two digits as another of {}'s kind",
                    path.display(),
                    accessory.id,
                    accessory.model
                )
                .into());
            }
            accessories.push(accessory);
        }
    }
    Ok(accessories)
}

/// The lines of section `name` in an ini's text, without its header.
fn section_lines<'a>(text: &'a str, name: &str) -> Option<Vec<&'a str>> {
    let header = format!("[{name}]");
    let mut lines = text.lines();
    lines.find(|line| line.trim() == header)?;
    Some(lines.take_while(|line| !line.trim_start().starts_with('[')).filter(|line| !line.trim().is_empty()).collect())
}

/// Our `weapon.ini` sections: each look in its model's, with our texture.
pub fn weapon_sections(weapons: &str, accessories: &[Accessory]) -> Result<String> {
    let mut out = String::new();
    for accessory in accessories {
        let lines = section_lines(weapons, &accessory.model.to_string())
            .ok_or_else(|| format!("accessory {}: weapon {} has no section in weapon.ini", accessory.id, accessory.model))?;
        out.push_str(&format!("\r\n[{}]\r\n", look_id(accessory.id, accessory.model)));
        for line in lines {
            let line = if accessory.texture.is_some() && line.trim_start().starts_with("Texture0=") {
                format!("Texture0={}", texture_id(accessory.id))
            } else if accessory.mesh.is_some() && line.trim_start().starts_with("Mesh0=") {
                format!("Mesh0={}", mesh_id(accessory.id))
            } else {
                line.trim_end().to_owned()
            };
            out.push_str(&line);
            out.push_str("\r\n");
        }
    }
    Ok(out)
}

/// Our `WeaponMotion.ini` lines: the model's poses (`<model>999<code>`) for
/// each look.
pub fn motion_lines(motions: &str, accessories: &[Accessory]) -> String {
    let mut out = String::new();
    for accessory in accessories {
        let prefix = format!("{}999", accessory.model);
        let look = look_id(accessory.id, accessory.model);
        for line in motions.lines() {
            if let Some(rest) = line.trim().strip_prefix(prefix.as_str())
                && rest.len() > 3
                && rest.as_bytes()[3] == b'='
            {
                out.push_str(&format!("{look}999{rest}\r\n"));
            }
        }
    }
    out
}

/// Our `Action3DEffect.ini` lines: the model's effects in every action
/// (`999.<action>.<family>.<item>`) for each look, its aura (action 999)
/// replaced by the accessory's.
pub fn effect_lines(effects: &str, accessories: &[Accessory]) -> String {
    let mut out = String::new();
    for accessory in accessories {
        let model = format!(".{}.{:03}=", accessory.model / 1_000, accessory.model % 1_000);
        let look = look_id(accessory.id, accessory.model);
        let key = |action: &str| format!("999.{action}.{}.{:03}", look / 1_000, look % 1_000);
        for line in effects.lines() {
            let line = line.trim();
            let Some((head, effect)) = line.split_once(model.as_str()) else {
                continue;
            };
            let Some(action) = head.strip_prefix("999.") else {
                continue;
            };
            if action == "999" && accessory.aura.is_some() {
                continue;
            }
            out.push_str(&format!("{}={effect}\r\n", key(action)));
        }
        if let Some(aura) = &accessory.aura {
            out.push_str(&format!("{}={aura}\r\n", key("999")));
        }
    }
    out
}

/// Our `3DFlyingObj.ini` sections: the model's arrows (`[<model>.<arrow>]`)
/// for each look.
pub fn arrow_sections(arrows: &str, accessories: &[Accessory]) -> String {
    let mut out = String::new();
    for accessory in accessories {
        let prefix = format!("[{}.", accessory.model);
        let look = look_id(accessory.id, accessory.model);
        for header in arrows.lines().map(str::trim).filter(|line| line.starts_with(prefix.as_str())) {
            let name = &header[1..header.len() - 1];
            let Some(lines) = section_lines(arrows, name) else {
                continue;
            };
            out.push_str(&format!("\r\n[{look}.{}]\r\n", &name[prefix.len() - 1..]));
            for line in lines {
                out.push_str(line.trim_end());
                out.push_str("\r\n");
            }
        }
    }
    out
}

/// `ini/accessory.ini`: what each accessory is drawn as, the colour of its
/// aura, and where its sparkles twinkle (`Sparkles=x y z;...`, in the
/// weapon's rest pose), from `sparkles`, by accessory.
pub fn look_sections(accessories: &[Accessory], sparkles: &std::collections::HashMap<u32, Vec<[f32; 3]>>) -> String {
    accessories
        .iter()
        .map(|accessory| {
            let tint = accessory.aura_tint.as_ref().map(|tint| format!("AuraTint={tint}\r\n")).unwrap_or_default();
            let points = sparkles
                .get(&accessory.id)
                .filter(|points| !points.is_empty())
                .map(|points| {
                    let points: Vec<String> = points.iter().map(|[x, y, z]| format!("{x:.1} {y:.1} {z:.1}")).collect();
                    format!("Sparkles={}\r\n", points.join(";"))
                })
                .unwrap_or_default();
            format!("[{}]\r\nLook={}\r\n{tint}{points}\r\n", accessory.id, look_id(accessory.id, accessory.model))
        })
        .collect()
}

/// `count` places among `places`, spread over them: the same ones each
/// build (a fixed walk from `seed`).
pub fn sparkle_points(places: &[[f32; 3]], count: u32, seed: u32) -> Vec<[f32; 3]> {
    if places.is_empty() {
        return Vec::new();
    }
    let mut state = seed.wrapping_mul(2_654_435_761).wrapping_add(1);
    (0..count)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            places[state as usize % places.len()]
        })
        .collect()
}

/// An original ini with `ours` after it.
fn merged(installation: &mut Installation, name: &str, ours: &str) -> Result<Vec<u8>> {
    let mut bytes = installation.read(name)?;
    if !bytes.ends_with(b"\n") {
        bytes.extend_from_slice(b"\r\n");
    }
    bytes.extend_from_slice(ours.as_bytes());
    Ok(bytes)
}

/// Writes the merged inis, `ini/accessory.ini` and the textures under
/// `output`; how many accessories, and the `3dtexture.ini` lines of their
/// textures.
pub fn build(installation: &mut Installation, content: &Path, output: &Path) -> Result<(usize, String)> {
    let accessories = load(content)?;
    if accessories.is_empty() {
        return Ok((0, String::new()));
    }
    let text = |installation: &mut Installation, name: &str| -> Result<String> {
        Ok(String::from_utf8_lossy(&installation.read(name)?).into_owned())
    };
    let weapons = weapon_sections(&text(installation, WEAPONS)?, &accessories)?;
    let motions = motion_lines(&text(installation, MOTIONS)?, &accessories);
    let effects = effect_lines(&text(installation, EFFECTS)?, &accessories);
    let arrows = arrow_sections(&text(installation, ARROWS)?, &accessories);
    std::fs::create_dir_all(output.join("ini"))?;
    for (name, ours) in [(WEAPONS, weapons), (MOTIONS, motions), (EFFECTS, effects), (ARROWS, arrows)] {
        std::fs::write(output.join(name), merged(installation, name, &ours)?)?;
    }

    let mut meshes = String::new();
    let mut sparkles = std::collections::HashMap::new();
    for accessory in &accessories {
        let original = model_path(installation, accessory.model)?;
        let Some(source) = &accessory.mesh else {
            if accessory.sparkles > 0 {
                let rest = crate::c3mesh::Obj::parse(&crate::c3mesh::rest_obj(&installation.read(&original)?)?)?;
                let places: Vec<_> = rest.vertices.iter().map(|vertex| vertex.0).collect();
                sparkles.insert(accessory.id, sparkle_points(&places, accessory.sparkles, accessory.id));
            }
            continue;
        };
        let obj = std::fs::read_to_string(content.join(source))
            .map_err(|error| format!("accessory {}: {source}: {error}", accessory.id))?;
        let obj = crate::c3mesh::Obj::parse(&obj).map_err(|error| format!("accessory {}: {source}: {error}", accessory.id))?;
        let places: Vec<_> = obj.vertices.iter().map(|vertex| vertex.0).collect();
        sparkles.insert(accessory.id, sparkle_points(&places, accessory.sparkles, accessory.id));
        let ours = crate::c3mesh::replace_mesh(&installation.read(&original)?, &obj)
            .map_err(|error| format!("accessory {}: {source} in {original}: {error}", accessory.id))?;
        let path = mesh_path(accessory.id);
        let file = output.join(&path);
        std::fs::create_dir_all(file.parent().expect("mesh folder"))?;
        std::fs::write(file, ours)?;
        meshes.push_str(&format!("{}={path}\r\n", mesh_id(accessory.id)));
    }
    if !meshes.is_empty() {
        std::fs::write(output.join(MODELS), merged(installation, MODELS, &meshes)?)?;
    }
    std::fs::write(output.join(LOOKS), look_sections(&accessories, &sparkles))?;

    let mut textures = String::new();
    for accessory in &accessories {
        let Some(source) = &accessory.texture else {
            continue;
        };
        let image = image::open(content.join(source))
            .map_err(|error| format!("accessory {}: {source}: {error}", accessory.id))?
            .to_rgba8();
        let path = texture_path(accessory.id);
        let file = output.join(&path);
        std::fs::create_dir_all(file.parent().expect("texture folder"))?;
        std::fs::write(file, crate::dds::encode_dxt5(&image))?;
        textures.push_str(&format!("{}={path}\r\n", texture_id(accessory.id)));
    }
    Ok((accessories.len(), textures))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bow() -> Accessory {
        Accessory { id: 370005, model: 500279, texture: Some(String::new()), mesh: None, aura: Some("angelwing3".into()), aura_tint: None, sparkles: 0 }
    }

    #[test]
    fn an_accessory_is_drawn_as_a_free_weapon_of_its_models_kind() {
        assert_eq!(look_id(370005, 500279), 500905);
        assert!(fits(370005, 500279) && fits(360005, 410339) && fits(350005, 561229) && fits(380005, 900309));
        assert!(!fits(370005, 410339) && !fits(350005, 500279) && !fits(360005, 900309));
    }

    #[test]
    fn its_look_takes_the_models_section_poses_flashes_and_arrows() {
        let weapons = "[500279]\nPart=1\nMesh0=500270\nTexture0=500275\nMixTex0=0\n\n[500289]\nMesh0=1\n";
        let ours = weapon_sections(weapons, &[bow()]).unwrap();
        assert_eq!(ours, "\r\n[500905]\r\nPart=1\r\nMesh0=500270\r\nTexture0=95370005\r\nMixTex0=0\r\n");
        let own = Accessory { texture: None, ..bow() };
        assert!(weapon_sections(weapons, &[own]).unwrap().contains("Texture0=500275"));
        let modelled = Accessory { mesh: Some(String::new()), ..bow() };
        assert!(weapon_sections(weapons, &[modelled]).unwrap().contains("Mesh0=95370005\r\nTexture0=95370005"));

        let motions = "500279999401=c3/mesh/500270401.c3\n500279999999=c3/mesh/500270.c3\n500289999401=x\n";
        assert_eq!(
            motion_lines(motions, &[bow()]),
            "500905999401=c3/mesh/500270401.c3\r\n500905999999=c3/mesh/500270.c3\r\n"
        );

        let effects = "999.401.500.279=Flash500\n999.999.500.279=Bow125\n999.999.500.289=Bow130\n";
        assert_eq!(effect_lines(effects, &[bow()]), "999.401.500.905=Flash500\r\n999.999.500.905=angelwing3\r\n");
        let own = Accessory { aura: None, ..bow() };
        assert_eq!(effect_lines(effects, &[own]), "999.401.500.905=Flash500\r\n999.999.500.905=Bow125\r\n");

        let arrows = "[500279.1050000]\nSimpleObjID=1\nEffectIndex=Bow125\n[500289.1050000]\nSimpleObjID=2\n";
        assert_eq!(arrow_sections(arrows, &[bow()]), "\r\n[500905.1050000]\r\nSimpleObjID=1\r\nEffectIndex=Bow125\r\n");
        assert_eq!(look_sections(&[bow()], &Default::default()), "[370005]\r\nLook=500905\r\n\r\n");
        let ice = Accessory { aura_tint: Some("ice".into()), ..bow() };
        assert_eq!(look_sections(&[ice], &Default::default()), "[370005]\r\nLook=500905\r\nAuraTint=ice\r\n\r\n");
        let points = sparkle_points(&[[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]], 3, 370005);
        // Three of the places, the same each time.
        assert_eq!(points.len(), 3);
        assert_eq!(points, sparkle_points(&[[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]], 3, 370005));
        let sparkling: std::collections::HashMap<_, _> = [(370005, vec![[1.0, -2.25, 3.0], [4.0, 5.0, 6.0]])].into();
        assert_eq!(look_sections(&[bow()], &sparkling), "[370005]\r\nLook=500905\r\nSparkles=1.0 -2.2 3.0;4.0 5.0 6.0\r\n\r\n");
    }
}
