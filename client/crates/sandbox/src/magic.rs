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
/// Experience to the next level (`NeedExp`; 0 at the top).
const NEED_EXP: usize = 80;
const INTONE_EFFECT: usize = 468;
/// What the caster gives off as the skill leaves it (Hercules' `zf2-e250`,
/// ArrowRain's `zf2-e231`).
const SENDER_EFFECT: usize = 792;
/// What flies from the caster to the target (Bolt's `dianjian`, FireRing's
/// `fireball-fly2`).
const FLY_EFFECT: usize = 1508;
const TARGET_EFFECT: usize = 1120;
const TARGET_SOUND: usize = 1184;
/// A trail of effects along the cast, `ini/tme/<name>` (FastBlade's
/// `windblade4.tme`, ScentSword's `swordrain4.tme`).
const TRAIL: usize = 1444;
/// Size of one TME step: the effect name, then four numbers.
const TRAIL_STEP: usize = 144;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MagicType {
    pub id: u32,
    pub level: u16,
    pub name: String,
    /// "Magic attack", "Physical attack", ... (`~` in the file are spaces).
    pub kind: String,
    /// Effect played on the caster while casting (for example "Intone-1").
    pub intone_effect: Option<String>,
    /// Played on the caster as the skill leaves it.
    pub sender_effect: Option<String>,
    /// Flies from the caster to the target, which the target effect
    /// awaits.
    pub fly_effect: Option<String>,
    /// Effect played on the target (for example "Thunder").
    pub target_effect: Option<String>,
    pub sound: Option<String>,
    /// Experience to the next level; 0 at the top.
    pub need_exp: u32,
    /// The `.tme` trail drawn along the cast, if any.
    pub trail: Option<String>,
}

/// One effect of a trail: played `delay_ms` after the cast, `along` units
/// toward the target and `across` to its side.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrailStep {
    pub effect: String,
    pub delay_ms: u32,
    pub along: i32,
    pub across: i32,
}

/// Parses a `.tme` file: a step count, then per step a 128-byte effect name
/// and four numbers (delay, a second delay, along, across).
pub fn parse_trail(bytes: &[u8]) -> Vec<TrailStep> {
    let number = |offset: usize| {
        bytes
            .get(offset..offset + 4)
            .map(|value| i32::from_le_bytes([value[0], value[1], value[2], value[3]]))
    };
    let count = number(0).unwrap_or(0).clamp(0, 256) as usize;
    (0..count)
        .filter_map(|index| {
            let start = 4 + index * TRAIL_STEP;
            let name = bytes.get(start..start + 128)?;
            let end = name.iter().position(|byte| *byte == 0).unwrap_or(name.len());
            let effect = String::from_utf8_lossy(&name[..end]).trim().to_owned();
            let delay = number(start + 128)?.max(0) + number(start + 132)?.max(0);
            (!effect.is_empty()).then(|| TrailStep {
                effect,
                delay_ms: delay as u32,
                along: number(start + 136).unwrap_or(0),
                across: number(start + 140).unwrap_or(0),
            })
        })
        .collect()
}

/// The steps of the trail `name` (`ini/tme/`), cached.
pub fn trail(assets: &mut Assets, name: &str) -> &'static [TrailStep] {
    static TRAILS: OnceLock<std::sync::Mutex<HashMap<String, &'static [TrailStep]>>> = OnceLock::new();
    let trails = TRAILS.get_or_init(Default::default);
    let key = name.to_ascii_lowercase();
    if let Some(steps) = trails.lock().ok().and_then(|trails| trails.get(&key).copied()) {
        return steps;
    }
    let stem = key.trim_end_matches(".tme");
    let steps = [format!("ini/tme/{stem}.TME"), format!("ini/tme/{stem}.tme"), format!("ini/tme/{name}")]
        .iter()
        .find_map(|path| assets.read(path).ok())
        .map(|bytes| parse_trail(&bytes))
        .unwrap_or_default();
    let steps: &'static [TrailStep] = Box::leak(steps.into_boxed_slice());
    if let Ok(mut trails) = trails.lock() {
        trails.insert(key, steps);
    }
    steps
}

fn field(record: &[u8], offset: usize) -> Option<String> {
    let bytes = record.get(offset..)?;
    let end = bytes.iter().position(|byte| *byte == 0).unwrap_or(bytes.len());
    let mut bytes = &bytes[..end];
    // Some effects come as "1－bomb4" (a GBK full-width hyphen, A3 AD,
    // after a number): the effect is the name after it.
    if let Some(hyphen) = bytes.windows(2).position(|pair| pair == [0xa3, 0xad]) {
        bytes = &bytes[hyphen + 2..];
    }
    let text = String::from_utf8_lossy(bytes).replace('~', " ");
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
                sender_effect: field(record, SENDER_EFFECT),
                fly_effect: field(record, FLY_EFFECT),
                target_effect: field(record, TARGET_EFFECT),
                sound: field(record, TARGET_SOUND),
                need_exp: record
                    .get(NEED_EXP..NEED_EXP + 4)
                    .map(|value| u32::from_le_bytes([value[0], value[1], value[2], value[3]]))
                    .unwrap_or(0),
                trail: field(record, TRAIL).filter(|name| name.to_ascii_lowercase().ends_with(".tme")),
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
    /// Added onto what is behind (`ADB`=2, D3DBLEND_ONE): the texture's
    /// black is see-through.
    pub additive: bool,
}

/// A texture key the renderer loads glowing: see [`additive_image`].
pub const ADDITIVE_SUFFIX: &str = "#additive";

impl EffectLayer {
    /// The texture as the renderer loads it.
    pub fn texture_key(&self) -> String {
        if self.additive {
            format!("{}{ADDITIVE_SUFFIX}", self.texture)
        } else {
            self.texture.clone()
        }
    }
}

/// An additive texture made to be drawn with alpha blending: what it adds
/// (colour times alpha) becomes its alpha, so black disappears and bright
/// parts glow.
pub fn additive_image(mut image: image::RgbaImage) -> image::RgbaImage {
    for pixel in image.pixels_mut() {
        let [r, g, b, a] = pixel.0;
        let added = [r, g, b].map(|channel| u32::from(channel) * u32::from(a) / 255);
        let alpha = added.into_iter().max().unwrap_or(0);
        pixel.0 = if alpha == 0 {
            [0, 0, 0, 0]
        } else {
            let [r, g, b] = added.map(|channel| (channel * 255 / alpha) as u8);
            [r, g, b, alpha as u8]
        };
    }
    image
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

/// What a blow of a kind of weapon shows and sounds on whom it hits
/// (`ini/WeaponEffect.ini`, by the weapon's `item / 1000`; bare hands are
/// `000`): the effect and the sound.
pub fn weapon_hit(assets: &mut Assets, weapon: u32) -> Option<(String, String)> {
    static HITS: OnceLock<HashMap<u32, (String, String)>> = OnceLock::new();
    let hits = HITS.get_or_init(|| {
        let bytes = assets.read("ini/WeaponEffect.ini").unwrap_or_default();
        sandbox_data::ini::SectionCatalog::parse(&bytes)
            .sections()
            .iter()
            .filter_map(|section| {
                let kind = section.name.trim().parse::<u32>().ok()?;
                let effect = section.first("HitEffect").unwrap_or("").trim().to_owned();
                let sound = section.first("HitSound").unwrap_or("").trim().replace('\\', "/");
                Some((kind, (effect, sound)))
            })
            .collect()
    });
    hits.get(&(weapon / 1_000)).cloned()
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

/// Status effects the original client plays from its own code rather than
/// `statuseffect.ini`: the XP circle (`XpStart`, 0x10) is `CircleUp`
/// (`Hero.cpp`, beside `XpBlock`).
const CLIENT_STATUS_EFFECTS: [(u64, &str); 1] = [(0x10, "CircleUp")];

/// `ini/statuseffect.ini`: a status bit in hex and up to two effect names
/// (`NULL` for none).
pub fn parse_status_effects(text: &str) -> Vec<(u64, String)> {
    text.lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            let bit = u64::from_str_radix(fields.next()?, 16).ok().filter(|bit| *bit != 0)?;
            Some(fields.filter(|name| *name != "NULL").map(move |name| (bit, name.to_owned())))
        })
        .flatten()
        .collect()
}

/// The status bits and the effects that play around whoever has them, as
/// the original lists them; names missing from `3DEffect.ini` are left out.
pub fn status_effects(assets: &mut Assets) -> &'static [(u64, String)] {
    static STATUS: OnceLock<Vec<(u64, String)>> = OnceLock::new();
    if let Some(status) = STATUS.get() {
        return status;
    }
    let text = assets
        .read("ini/statuseffect.ini")
        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
        .unwrap_or_default();
    let tables = tables(assets);
    let status = parse_status_effects(&text)
        .into_iter()
        .chain(CLIENT_STATUS_EFFECTS.iter().map(|(bit, name)| (*bit, (*name).to_owned())))
        .filter(|(_, name)| tables.effects.contains_key(&name.to_ascii_lowercase()))
        .collect();
    STATUS.get_or_init(|| status)
}

/// What flies when a bow shoots (`ini/3DFlyingObj.ini`): the arrow's model
/// and texture (through `3DSimpleObj.ini`, `3dobj.ini`, `3dtexture.ini`)
/// and the effect some bows give it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlyingObject {
    pub mesh: String,
    pub texture: String,
    pub effect: Option<String>,
    /// What it sounds as it leaves the bow (`FlyingSound`, `bow`).
    pub sound: Option<String>,
}

/// The arrow `bow` shoots with arrows `arrow`: their own entry
/// (`[<bow>.<arrow>]`, the super bows with an effect), else any bow's
/// (`[999999.<arrow>]`), else the plain arrow's.
pub fn flying_object(assets: &mut Assets, bow: u32, arrow: u32) -> Option<FlyingObject> {
    static CACHE: OnceLock<std::sync::Mutex<HashMap<(u32, u32), Option<FlyingObject>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    if let Some(found) = cache.lock().ok()?.get(&(bow, arrow)) {
        return found.clone();
    }
    let found = (|| {
        let flying = sandbox_data::ini::SectionCatalog::parse(&assets.read("ini/3DFlyingObj.ini").ok()?);
        let section = [format!("{bow}.{arrow}"), format!("999999.{arrow}"), "999999.1050000".to_owned()]
            .into_iter()
            .find_map(|name| flying.section(&name))?;
        let object = section.first("SimpleObjID")?.trim().to_owned();
        let effect = section
            .first("EffectIndex")
            .map(str::trim)
            .filter(|name| !name.is_empty() && !name.eq_ignore_ascii_case("NULL"))
            .map(str::to_owned);
        let simple = sandbox_data::ini::SectionCatalog::parse(&assets.read("ini/3DSimpleObj.ini").ok()?);
        let simple = simple.section(&format!("ObjIDType{object}"))?;
        let mesh = simple.first("Part0")?.trim().parse::<u32>().ok()?;
        let texture = simple.first("Texture0")?.trim().parse::<u32>().ok()?;
        let models = sandbox_data::ini::IdPathCatalog::parse(&assets.read("ini/3dobj.ini").ok()?);
        let textures = sandbox_data::ini::IdPathCatalog::parse(&assets.read("ini/3dtexture.ini").ok()?);
        let sound = section
            .first("FlyingSound")
            .map(str::trim)
            .filter(|name| !name.is_empty() && !name.eq_ignore_ascii_case("NULL"))
            .map(str::to_owned);
        Some(FlyingObject {
            sound,
            mesh: models.get(mesh)?.as_str().to_owned(),
            texture: textures.get(texture)?.as_str().to_owned(),
            effect,
        })
    })();
    if let Ok(mut cache) = cache.lock() {
        cache.insert((bow, arrow), found.clone());
    }
    found
}

/// A sound named by the tables as a file: `sound\bow1.wav`, `bow` or
/// `bomb4` (in `sound/`, a `.wav` unless it says otherwise).
pub fn sound_path(name: &str) -> String {
    let name = name.trim().replace('\\', "/");
    let name = if name.contains('/') { name } else { format!("sound/{name}") };
    if name.rsplit('/').next().is_some_and(|file| file.contains('.')) { name } else { format!("{name}.wav") }
}

pub fn effect(assets: &mut Assets, name: &str) -> Option<EffectDefinition> {
    let tables = tables(assets);
    let section = tables.effects.get(&name.to_ascii_lowercase())?;
    let amount = section.get("amount")?.parse::<usize>().ok()?;
    let layers = (0..amount)
        .filter_map(|index| {
            let mesh = section.get(&format!("effectid{index}"))?.parse::<u32>().ok()?;
            let texture = section.get(&format!("textureid{index}"))?.parse::<u32>().ok()?;
            // The destination kept whole: ONE (2), or DESTALPHA (7), which
            // on the alpha-less back buffer is one too. Both add light.
            let additive = section
                .get(&format!("adb{index}"))
                .is_some_and(|value| matches!(value.trim(), "2" | "7"));
            Some(EffectLayer {
                mesh: tables.meshes.get(&mesh)?.clone(),
                texture: tables.textures.get(&texture)?.clone(),
                additive,
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
    fn sounds_named_by_the_tables_are_files() {
        assert_eq!(sound_path("sound\\bow1.wav"), "sound/bow1.wav");
        assert_eq!(sound_path("bow"), "sound/bow.wav");
        assert_eq!(sound_path("sound/blunt_h01.wav"), "sound/blunt_h01.wav");
    }

    #[test]
    #[ignore = "Requires a local 5065 installation"]
    fn scatter_and_bolt_leave_their_casters() {
        let mut assets = Assets::open(std::path::Path::new("R:/5065")).expect("assets");
        let scatter = magic_type(&mut assets, 8_001, 0).expect("Scatter");
        assert_eq!(scatter.target_effect.as_deref(), Some("fire-bomb"));
        let bolt = magic_type(&mut assets, 1_002, 0).or_else(|| magic_type(&mut assets, 10_020, 0));
        if let Some(bolt) = bolt {
            assert!(bolt.fly_effect.is_some() || bolt.sender_effect.is_some() || bolt.target_effect.is_some());
        }
        let blade = weapon_hit(&mut assets, 410_339).expect("blades hit");
        assert_eq!(blade, ("m-b02".to_owned(), "sound/blunt_h01.wav".to_owned()));
    }

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
        record[TARGET_SOUND..TARGET_SOUND + 8].copy_from_slice(b"1\xa3\xadbomb4");
        bytes.extend(record);
        let types = parse_magic_types(&bytes);
        assert_eq!(types.len(), 1);
        assert_eq!(types[0].id, 1_000);
        assert_eq!(types[0].level, 2);
        assert_eq!(types[0].kind, "Magic attack");
        assert_eq!(types[0].intone_effect.as_deref(), Some("Intone-1"));
        assert_eq!(types[0].target_effect, None);
        assert_eq!(types[0].sound.as_deref(), Some("bomb4"), "the name after the full-width hyphen");
    }

    #[test]
    fn reads_a_trail() {
        let mut bytes = 2_i32.to_le_bytes().to_vec();
        for (name, delay, along, across) in [("swordgas-m", 50, 60, 25), ("swordgas-b", 100, 120, 15)] {
            let mut step = vec![0_u8; TRAIL_STEP];
            step[..name.len()].copy_from_slice(name.as_bytes());
            step[128..132].copy_from_slice(&(delay as i32).to_le_bytes());
            step[136..140].copy_from_slice(&(along as i32).to_le_bytes());
            step[140..144].copy_from_slice(&(across as i32).to_le_bytes());
            bytes.extend(step);
        }
        let steps = parse_trail(&bytes);
        assert_eq!(steps.len(), 2);
        assert_eq!(
            steps[1],
            TrailStep { effect: "swordgas-b".to_owned(), delay_ms: 100, along: 120, across: 15 }
        );
    }

    #[test]
    #[ignore = "Requires a local 5065 installation"]
    fn the_blades_have_their_trails() {
        let mut assets = Assets::open(std::path::Path::new("R:/5065")).expect("assets");
        let fast_blade = magic_type(&mut assets, 1_045, 0).expect("FastBlade");
        let name = fast_blade.trail.clone().expect("FastBlade has a trail");
        let steps = trail(&mut assets, &name);
        assert_eq!(steps.len(), 6, "{name}");
        assert!(steps.iter().all(|step| effect(&mut assets, &step.effect).is_some()), "{steps:?}");
        let scent_sword = magic_type(&mut assets, 1_046, 0).expect("ScentSword");
        assert!(!trail(&mut assets, scent_sword.trail.as_deref().expect("trail")).is_empty());
    }

    #[test]
    fn reads_the_status_effect_table() {
        let effects = parse_status_effects("00000000 NULL NULL
00000002 poisontime NULL
00000040 NULL TeamLeader
80000000 LuckAbsorb NULL
");
        assert_eq!(
            effects,
            [(0x2, "poisontime".to_owned()), (0x40, "TeamLeader".to_owned()), (0x8000_0000, "LuckAbsorb".to_owned())]
        );
    }

    #[test]
    fn resolves_effect_layers() {
        let sections = ini_sections("[Thunder]\nAmount=1\nEffectId0=35\nTextureId0=44\nFrameInterval=66\n");
        assert_eq!(sections["thunder"]["effectid0"], "35");
        assert_eq!(id_table("35=C3/Effect/Thunder/1.C3")[&35], "C3/Effect/Thunder/1.C3");
    }

    #[test]
    fn additive_black_is_see_through_and_light_glows() {
        let image = image::RgbaImage::from_raw(3, 1, vec![0, 0, 0, 255, 255, 128, 0, 255, 100, 50, 0, 128]).unwrap();
        let image = additive_image(image);
        assert_eq!(image.get_pixel(0, 0).0, [0, 0, 0, 0]);
        assert_eq!(image.get_pixel(1, 0).0, [255, 128, 0, 255]);
        assert_eq!(image.get_pixel(2, 0).0[3], 50);
    }

    #[test]
    #[ignore = "Requires a local 5065 installation"]
    fn the_praying_effects_load() {
        let mut assets = Assets::open(std::path::Path::new("R:/5065")).expect("assets");
        let status = status_effects(&mut assets);
        for (bit, name) in [(1_u64 << 30, "LuckDiffuse"), (1 << 31, "LuckAbsorb")] {
            assert!(status.iter().any(|(b, n)| *b == bit && n == name), "{name} is listed");
            let effect = effect(&mut assets, name).expect("in 3DEffect.ini");
            for layer in &effect.layers {
                assert!(assets.read(&layer.mesh).is_ok(), "{}", layer.mesh);
                assert!(assets.image(&layer.texture).is_ok(), "{}", layer.texture);
            }
            assert!(!effect.layers.is_empty(), "{name} has layers");
        }
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

    /// Every skill's casting and target effects, as the client would play
    /// them: `cargo test -p conquer-sandbox every_skill_effect -- --ignored --nocapture`.
    #[test]
    #[ignore = "Requires a local 5065 installation"]
    fn every_skill_effect_loads() {
        let mut assets = Assets::open(std::path::Path::new("R:/5065")).expect("assets");
        let bytes = assets.read("ini/MagicType.dat").expect("MagicType.dat");
        let mut names = std::collections::BTreeMap::<String, Vec<(u32, String)>>::new();
        for magic in parse_magic_types(&bytes) {
            for name in [&magic.intone_effect, &magic.target_effect].into_iter().flatten() {
                names.entry(name.clone()).or_default().push((magic.id, magic.name.clone()));
            }
        }
        let mut broken = Vec::new();
        for (name, users) in &names {
            let problem = match effect(&mut assets, name) {
                None => Some("not in 3DEffect.ini".to_owned()),
                Some(definition) => definition.layers.iter().find_map(|layer| {
                    match assets.read(&layer.mesh) {
                        Err(_) => Some(format!("missing mesh {}", layer.mesh)),
                        Ok(bytes) if sandbox_data::c3::C3Document::parse(&bytes).is_err() => {
                            Some(format!("unreadable mesh {}", layer.mesh))
                        },
                        Ok(_) if assets.image(&layer.texture).is_err() => Some(format!("missing texture {}", layer.texture)),
                        Ok(_) => None,
                    }
                }),
            };
            if let Some(problem) = problem {
                let mut skills: Vec<String> = users.iter().map(|(id, name)| format!("{id} {name}")).collect();
                skills.dedup();
                broken.push(format!("{name}: {problem} (used by {})", skills.join(", ")));
            }
        }
        println!("{} effects, {} broken", names.len(), broken.len());
        for line in &broken {
            println!("  {line}");
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

#[cfg(test)]
mod shape_tests {
    #[test]
    #[ignore = "Requires a local 5065 installation"]
    fn the_tornado_ribbons_parse() {
        let mut assets = crate::assets::Assets::open(std::path::Path::new("R:/5065")).expect("assets");
        let effect = super::effect(&mut assets, "Tornado").expect("Tornado");
        let document = sandbox_data::c3::C3Document::parse(&assets.read(&effect.layers[0].mesh).unwrap()).unwrap();
        assert_eq!(document.shapes.len(), 4);
        for shape in &document.shapes {
            assert_eq!(shape.segments, 20);
            assert_eq!(shape.frames.len(), 31);
            assert_eq!(shape.lines.len(), 1);
        }
    }
}




