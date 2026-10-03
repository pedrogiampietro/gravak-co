//! Original skill data: `ini/MagicType.dat` (names and effect names per
//! skill level) and the 3D effect tables `ini/3DEffect.ini`,
//! `ini/3DEffectObj.ini` and `ini/3dtexture.ini` that turn an effect name
//! into C3 meshes and textures.

use crate::assets::Assets;
use std::collections::HashMap;
use std::sync::OnceLock;

const RECORD_SIZE: usize = 1584;
const NAME: usize = 8;
const KIND: usize = 212;
const INTONE_EFFECT: usize = 468;
const TARGET_EFFECT: usize = 1120;
const TARGET_SOUND: usize = 1184;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MagicType {
    pub id: u32,
    pub level: u16,
    pub name: String,
    /// "Magic attack", "Physical attack", ... (`~` in the file are spaces).
    pub kind: String,
    /// Effect played on the caster while casting (for example "Intone-1").
    pub intone_effect: Option<String>,
    /// Effect played on the target (for example "Thunder").
    pub target_effect: Option<String>,
    pub sound: Option<String>,
}

fn field(record: &[u8], offset: usize) -> Option<String> {
    let bytes = record.get(offset..)?;
    let end = bytes.iter().position(|byte| *byte == 0).unwrap_or(bytes.len());
    let text = String::from_utf8_lossy(&bytes[..end]).replace('~', " ");
    let text = text.trim();
    (!text.is_empty() && text != "NULL").then(|| text.to_owned())
}

/// Parses `MagicType.dat`: a record count, one `id * 10 + level` key per
/// record, then fixed-size records.
pub fn parse_magic_types(bytes: &[u8]) -> Vec<MagicType> {
    let read_u32 = |offset: usize| {
        bytes
            .get(offset..offset + 4)
            .map(|value| u32::from_le_bytes([value[0], value[1], value[2], value[3]]))
    };
    let Some(count) = read_u32(0).map(|count| count as usize) else {
        return Vec::new();
    };
    let start = 4 + count * 4;
    (0..count)
        .filter_map(|index| {
            let key = read_u32(4 + index * 4)?;
            let record = bytes.get(start + index * RECORD_SIZE..start + (index + 1) * RECORD_SIZE)?;
            Some(MagicType {
                id: read_u32(start + index * RECORD_SIZE)?,
                level: (key % 10) as u16,
                name: field(record, NAME).unwrap_or_default(),
                kind: field(record, KIND).unwrap_or_default(),
                intone_effect: field(record, INTONE_EFFECT),
                target_effect: field(record, TARGET_EFFECT),
                sound: field(record, TARGET_SOUND),
            })
        })
        .collect()
}

fn catalog(assets: &mut Assets) -> &'static HashMap<(u32, u16), MagicType> {
    static CATALOG: OnceLock<HashMap<(u32, u16), MagicType>> = OnceLock::new();
    if let Some(catalog) = CATALOG.get() {
        return catalog;
    }
    let types = assets
        .read("ini/MagicType.dat")
        .map(|bytes| parse_magic_types(&bytes))
        .unwrap_or_default();
    CATALOG.get_or_init(|| {
        types
            .into_iter()
            .map(|magic| ((magic.id, magic.level), magic))
            .collect()
    })
}

/// Skill data for `level`, falling back to the closest lower level and then
/// to any level of the skill.
pub fn magic_type(assets: &mut Assets, skill_id: u32, level: u16) -> Option<&'static MagicType> {
    let catalog = catalog(assets);
    (0..=level)
        .rev()
        .find_map(|level| catalog.get(&(skill_id, level)))
        .or_else(|| (0..10).find_map(|level| catalog.get(&(skill_id, level))))
}

/// Display name of a skill from the original catalog.
pub fn skill_name(assets: &mut Assets, skill_id: u32) -> String {
    magic_type(assets, skill_id, 0)
        .map(|magic| magic.name.clone())
        .filter(|name| !name.is_empty())
        .or_else(|| crate::action_bar::skill_action(skill_id).map(|skill| skill.name.to_owned()))
        .unwrap_or_else(|| format!("Skill {skill_id}"))
}

/// One layer of a 3D effect: a C3 mesh with its texture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectLayer {
    pub mesh: String,
    pub texture: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EffectDefinition {
    pub layers: Vec<EffectLayer>,
    /// Milliseconds per C3 frame (`FrameInterval`).
    pub frame_interval: f32,
}

fn ini_sections(text: &str) -> HashMap<String, HashMap<String, String>> {
    let mut sections = HashMap::new();
    let mut current: Option<String> = None;
    for line in text.lines().map(str::trim) {
        if let Some(name) = line.strip_prefix('[').and_then(|line| line.strip_suffix(']')) {
            current = Some(name.to_ascii_lowercase());
            sections.entry(name.to_ascii_lowercase()).or_insert_with(HashMap::new);
            continue;
        }
        if let (Some(section), Some((key, value))) = (&current, line.split_once('='))
            && let Some(entries) = sections.get_mut(section)
        {
            entries.insert(key.trim().to_ascii_lowercase(), value.trim().to_owned());
        }
    }
    sections
}

fn id_table(text: &str) -> HashMap<u32, String> {
    text.lines()
        .filter_map(|line| {
            let (id, path) = line.split_once('=')?;
            Some((id.trim().parse().ok()?, path.trim().to_owned()))
        })
        .collect()
}

struct EffectTables {
    effects: HashMap<String, HashMap<String, String>>,
    meshes: HashMap<u32, String>,
    textures: HashMap<u32, String>,
}

fn tables(assets: &mut Assets) -> &'static EffectTables {
    static TABLES: OnceLock<EffectTables> = OnceLock::new();
    if let Some(tables) = TABLES.get() {
        return tables;
    }
    let mut read = |path: &str| {
        assets
            .read(path)
            .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
            .unwrap_or_default()
    };
    let effects = ini_sections(&read("ini/3DEffect.ini"));
    let meshes = id_table(&read("ini/3DEffectObj.ini"));
    let textures = id_table(&read("ini/3dtexture.ini"));
    TABLES.get_or_init(|| EffectTables {
        effects,
        meshes,
        textures,
    })
}

pub fn effect(assets: &mut Assets, name: &str) -> Option<EffectDefinition> {
    let tables = tables(assets);
    let section = tables.effects.get(&name.to_ascii_lowercase())?;
    let amount = section.get("amount")?.parse::<usize>().ok()?;
    let layers = (0..amount)
        .filter_map(|index| {
            let mesh = section.get(&format!("effectid{index}"))?.parse::<u32>().ok()?;
            let texture = section.get(&format!("textureid{index}"))?.parse::<u32>().ok()?;
            Some(EffectLayer {
                mesh: tables.meshes.get(&mesh)?.clone(),
                texture: tables.textures.get(&texture)?.clone(),
            })
        })
        .collect::<Vec<_>>();
    let frame_interval = section
        .get("frameinterval")
        .and_then(|value| value.parse::<f32>().ok())
        .filter(|value| *value > 0.0)
        .unwrap_or(33.0);
    (!layers.is_empty()).then_some(EffectDefinition {
        layers,
        frame_interval,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_magic_type_records() {
        let mut bytes = Vec::new();
        bytes.extend(1_u32.to_le_bytes());
        bytes.extend(10_002_u32.to_le_bytes());
        let mut record = vec![0_u8; RECORD_SIZE];
        record[0..4].copy_from_slice(&1_000_u32.to_le_bytes());
        record[NAME..NAME + 7].copy_from_slice(b"Thunder");
        record[KIND..KIND + 12].copy_from_slice(b"Magic~attack");
        record[INTONE_EFFECT..INTONE_EFFECT + 8].copy_from_slice(b"Intone-1");
        record[TARGET_EFFECT..TARGET_EFFECT + 4].copy_from_slice(b"NULL");
        bytes.extend(record);
        let types = parse_magic_types(&bytes);
        assert_eq!(types.len(), 1);
        assert_eq!(types[0].id, 1_000);
        assert_eq!(types[0].level, 2);
        assert_eq!(types[0].kind, "Magic attack");
        assert_eq!(types[0].intone_effect.as_deref(), Some("Intone-1"));
        assert_eq!(types[0].target_effect, None);
    }

    #[test]
    fn resolves_effect_layers() {
        let sections = ini_sections("[Thunder]\nAmount=1\nEffectId0=35\nTextureId0=44\nFrameInterval=66\n");
        assert_eq!(sections["thunder"]["effectid0"], "35");
        assert_eq!(id_table("35=C3/Effect/Thunder/1.C3")[&35], "C3/Effect/Thunder/1.C3");
    }

    #[test]
    #[ignore = "Requires a local 5065 installation"]
    fn the_monster_born_effect_loads() {
        let mut assets = Assets::open(std::path::Path::new("R:/5065")).expect("assets");
        let effect = effect(&mut assets, "born").expect("born in 3DEffect.ini");
        assert_eq!(effect.layers.len(), 3);
        for layer in &effect.layers {
            let document = sandbox_data::c3::C3Document::parse(&assets.read(&layer.mesh).expect("c3 present"))
                .expect("c3 parses");
            assert!(!document.meshes.is_empty(), "{}", layer.mesh);
            assert!(assets.image(&layer.texture).is_ok(), "{}", layer.texture);
            println!("born: {} frames={:?} interval={}", layer.mesh, document.motions.iter().map(|m| m.frame_count).collect::<Vec<_>>(), effect.frame_interval);
        }
    }

    #[test]
    #[ignore = "Requires a local 5065 installation"]
    fn starter_skills_resolve_to_original_effects() {
        let mut assets = Assets::open(std::path::Path::new("R:/5065")).expect("assets");
        for skill in [1_000, 1_001, 1_002, 1_045, 1_110] {
            let magic = magic_type(&mut assets, skill, 0).expect("skill in MagicType.dat");
            println!("{skill} {:?}", magic);
            for name in [&magic.intone_effect, &magic.target_effect].into_iter().flatten() {
                let effect = effect(&mut assets, name).expect("effect in 3DEffect.ini");
                for layer in &effect.layers {
                    let document = sandbox_data::c3::C3Document::parse(
                        &assets.read(&layer.mesh).expect("c3 present"),
                    )
                    .expect("c3 parses");
                    assert!(assets.image(&layer.texture).is_ok(), "{}", layer.texture);
                    println!(
                        "  {name}: {} meshes={} frames={:?} interval={}",
                        layer.mesh,
                        document.meshes.len(),
                        document.motions.iter().map(|m| m.frame_count).collect::<Vec<_>>(),
                        effect.frame_interval
                    );
                }
            }
        }
    }

}
