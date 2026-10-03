// Source: @digitalm1nd on x.com / _dek on Discord — Preservation Conquer project — https://discord.gg/CvKPXEHYRY
use crate::action::Action;
use crate::assets::{Assets, Result};
use sandbox_data::{
    c3::{C3Document, C3Mesh, C3Motion, Matrix4, Vector3},
    ini::{IdPathCatalog, IniSection, SectionCatalog},
};

#[derive(Clone)]
pub struct Part {
    pub mesh: C3Mesh,
    pub texture: String,
    local: Option<C3Motion>,
    track: usize,
}
#[derive(Clone)]
pub struct Character {
    pub parts: Vec<Part>,
    actions: Vec<C3Document>,
    intervals: [f32; 11],
    pub body_id: u32,
    pub armor_id: u32,
    pub helmet_id: u32,
    pub hair_id: u32,
    pub right_weapon_id: u32,
    pub left_weapon_id: u32,
    pub weapon_action: u32,
    pub name: String,
    pub guild: String,
    pub guild_rank: String,
    /// The guild fund in silver, as the server last said.
    pub guild_fund: u64,
    pub health: u32,
    pub maximum_health: u32,
    pub strength: u16,
    pub agility: u16,
    pub vitality: u16,
    pub spirit: u16,
    pub attribute_points: u16,
    pub current_class: u8,
    pub previous_class: u8,
    pub rebirths: u8,
    /// PK points: red name from 30, black from 100.
    pub kill_points: u16,
    pub spouse: String,
}

#[derive(Clone)]
pub struct NpcModel {
    pub mesh: C3Mesh,
    pub texture: String,
    model_id: u32,
    motion: Option<C3Motion>,
    walk_motion: Option<C3Motion>,
    attack_motion: Option<C3Motion>,
    death_motion: Option<C3Motion>,
}

#[derive(Clone)]
pub struct NpcCatalog {
    appearances: SectionCatalog,
    models: IdPathCatalog,
    motions: IdPathCatalog,
    textures: IdPathCatalog,
}

impl NpcCatalog {
    pub fn load(assets: &mut Assets) -> Result<Self> {
        Ok(Self {
            appearances: SectionCatalog::parse(&assets.read("ini/npc.ini")?),
            models: IdPathCatalog::parse(&assets.read("ini/3dobj.ini")?),
            motions: IdPathCatalog::parse(&assets.read("ini/3dmotion.ini")?),
            textures: IdPathCatalog::parse(&assets.read("ini/3dtexture.ini")?),
        })
    }

    pub fn appearance_sections(&self) -> &[IniSection] {
        self.appearances.sections()
    }

    fn texture_for_model_path(&self, path: &str) -> Option<&str> {
        self.models
            .ids_for_path(path)
            .find_map(|id| self.textures.get(id).map(|texture| texture.as_str()))
    }
}

impl Character {
    pub fn load(assets: &mut Assets, body: u32) -> Result<Self> {
        if !(1..=4).contains(&body) {
            return Err("--body must be 1, 2, 3 or 4".into());
        }
        let profile = SectionCatalog::parse(include_bytes!("../../../character.ini"));
        let profile = profile
            .section("Character")
            .ok_or("Character profile missing")?;
        let number = |key: &str| -> Result<u32> {
            Ok(profile
                .first(key)
                .ok_or_else(|| format!("Missing profile {key}"))?
                .parse()?)
        };
        Self::load_appearance(
            assets,
            body,
            number("Armor")?,
            profile
                .first("Helmet")
                .and_then(|value| value.parse().ok())
                .unwrap_or(0),
            number("Hair")?,
            number("RightWeapon")?,
            number("LeftWeapon")?,
            profile.first("Name").unwrap_or("dek").to_owned(),
            profile.first("Guild").unwrap_or("").to_owned(),
            profile.first("GuildRank").unwrap_or("").to_owned(),
            number("Health")?,
            number("MaximumHealth")?,
        )
    }

    pub fn load_remote(
        assets: &mut Assets,
        body: u32,
        armor: u32,
        helmet: u32,
        hair: u16,
        right_weapon: u32,
        left_weapon: u32,
        name: String,
        health: u16,
    ) -> Result<Self> {
        if !(1..=4).contains(&body) {
            return Err("remote mesh has an unknown body type".into());
        }
        Self::load_appearance(
            assets,
            body,
            // Zero is the protocol value for an empty armor slot.  Keep it
            // intact so the default body model (body * 1_000_000) is loaded
            // after an armor item is unequipped instead of silently putting
            // the sample armor back on the character.
            armor,
            helmet,
            u32::from(hair),
            right_weapon,
            left_weapon,
            name,
            String::new(),
            String::new(),
            u32::from(health),
            u32::from(health).max(1),
        )
    }

    fn load_appearance(
        assets: &mut Assets,
        body: u32,
        armor_id: u32,
        helmet_id: u32,
        hair: u32,
        right: u32,
        left: u32,
        name: String,
        guild: String,
        guild_rank: String,
        health: u32,
        maximum_health: u32,
    ) -> Result<Self> {
        // Tinter dyes come folded into the ids (see `dyed`).
        let (armor_id, armor_color) = undyed(armor_id);
        let (helmet_id, helmet_color) = undyed(helmet_id);
        let (left, left_color) = undyed(left);
        let armor = SectionCatalog::parse(&assets.read("ini/armor.ini")?);
        let entry = armor
            .section(&(body * 1_000_000 + armor_id / 10 * 10).to_string())
            .ok_or("Missing armor in armor.ini")?;
        let models = IdPathCatalog::parse(&assets.read("ini/3dobj.ini")?);
        let textures = IdPathCatalog::parse(&assets.read("ini/3dtexture.ini")?);
        let motions = IdPathCatalog::parse(&assets.read("ini/3dmotion.ini")?);
        let model_id = entry.first("Mesh0").ok_or("Missing Mesh0")?.parse()?;
        let texture_id = dyed_texture(
            &textures,
            entry.first("Texture0").ok_or("Missing Texture0")?.parse()?,
            armor_color,
        );
        let doc = C3Document::parse(
            &assets.read(models.get(model_id).ok_or("Body model missing")?.as_str())?,
        )?;
        let index = doc
            .meshes
            .iter()
            .position(|m| m.name == "v_body")
            .ok_or("Body mesh missing")?;
        let mesh = doc.meshes[index].clone();
        let weapon_action = sandbox_data::weapon_action::resolve_weapon_action(
            (right != 0).then_some(right),
            (left != 0).then_some(left),
        );
        let mut motion = |action: Action| -> Result<C3Document> {
            let path = std::iter::once(action)
                .chain(action.paired_action())
                .flat_map(|action| {
                    let action = action.catalog_id();
                    [
                        body * 1_000_000 + weapon_action * 1000 + action,
                        1_000_000 + weapon_action * 1000 + action,
                        body * 1_000_000 + action,
                        1_000_000 + action,
                    ]
                })
                .find_map(|key| motions.get(key))
                .ok_or("Body action missing")?;
            Ok(C3Document::parse(&assets.read(path.as_str())?)?)
        };
        // A body without a death motion falls back to standing.
        let actions = Action::ALL
            .into_iter()
            .map(|action| match action {
                Action::Die | Action::Sit => motion(action).or_else(|_| motion(Action::Idle)),
                _ => motion(action),
            })
            .collect::<Result<Vec<_>>>()?;
        let mut parts = vec![Part {
            mesh,
            texture: textures
                .get(texture_id)
                .ok_or("Body texture missing")?
                .as_str()
                .to_owned(),
            local: None,
            track: index,
        }];

        if hair != 0 {
            if let Ok(part) = load_attachment(
                assets,
                &models,
                &textures,
                &doc,
                "ini/armet.ini",
                body * 1_000_000 + 119_000 + hair,
                "v_armet",
            ) {
                parts.push(part);
            }
        }
        if helmet_id != 0 {
            if let Ok(part) = load_dyed_attachment(
                assets,
                &models,
                &textures,
                &doc,
                "ini/armet.ini",
                body * 1_000_000 + helmet_id / 10 * 10,
                "v_armet",
                helmet_color,
            ) {
                parts.push(part);
            }
        }
        for (weapon, attachment, color) in [(right, "v_r_weapon", 0), (left, "v_l_weapon", left_color)] {
            if weapon == 0 {
                continue;
            }
            if let Ok(part) = load_dyed_attachment(
                assets,
                &models,
                &textures,
                &doc,
                "ini/weapon.ini",
                weapon,
                attachment,
                color,
            ) {
                parts.push(part);
            }
        }
        let timing = assets.read("ini/Action.dat")?;
        let read = |offset: usize| -> Option<u32> {
            Some(u32::from_le_bytes(
                timing.get(offset..offset + 4)?.try_into().ok()?,
            ))
        };
        let count = read(0).ok_or("Action.dat header missing")? as usize;
        if count > 1_000_000 || timing.len() < 4 + count * 20 {
            return Err("Action.dat truncated".into());
        }
        let intervals = Action::ALL.map(|action| {
            let action = action.catalog_id();
            [
                body * 1_000_000 + weapon_action * 1000 + action,
                999_000_000 + weapon_action * 1000 + action,
                body * 1_000_000 + 999_000 + action,
                999_999_000 + action,
            ]
            .into_iter()
            .find_map(|key| {
                (0..count)
                    .find(|i| read(4 + i * 20 + 4) == Some(key))
                    .and_then(|i| read(4 + i * 20 + 12))
            })
            .unwrap_or(33)
            .max(5) as f32
        });
        for action in &actions {
            for part in &parts {
                if part.track >= action.motions.len() {
                    return Err("Missing attachment motion track".into());
                }
            }
        }
        Ok(Self {
            parts,
            actions,
            intervals,
            body_id: body,
            armor_id,
            helmet_id,
            hair_id: hair,
            right_weapon_id: right,
            left_weapon_id: left,
            weapon_action,
            name,
            guild,
            guild_rank,
            guild_fund: 0,
            health,
            maximum_health,
            strength: 0,
            agility: 0,
            vitality: 0,
            spirit: 0,
            attribute_points: 0,
            current_class: 0,
            previous_class: 0,
            rebirths: 0,
            kill_points: 0,
            spouse: String::new(),
        })
    }

    pub fn duration(&self, action: Action) -> std::time::Duration {
        let action = action.index();
        std::time::Duration::from_secs_f32(
            self.actions[action].motions[self.parts[0].track]
                .frame_count
                .max(1) as f32
                * self.intervals[action]
                / 1000.0,
        )
    }
    pub fn vertices(
        &self,
        part: &Part,
        seconds: f32,
        action: Action,
        facing: f32,
        progress: Option<f32>,
    ) -> Vec<([f32; 3], [f32; 2])> {
        let action = action.index();
        let track = &self.actions[action].motions[part.track];
        let frame = progress.map_or_else(
            || (seconds * 1000.0 / self.intervals[action]) % track.frame_count.max(1) as f32,
            |p| {
                (p.clamp(0.0, 1.0) * track.frame_count.max(1) as f32)
                    .min(track.frame_count.max(1) as f32 - f32::EPSILON)
            },
        );
        let motion = part.local.as_ref().unwrap_or(track);
        let attachment = if part.local.is_some() {
            track.matrix_at(0, frame)
        } else {
            Matrix4::IDENTITY
        };
        let palette: Vec<Matrix4> = (0..motion.bone_count)
            .map(|b| {
                part.mesh
                    .initial_matrix
                    .multiply(motion.matrix_at(b, frame))
                    .multiply(attachment)
            })
            .collect();
        let (s, c) = facing.sin_cos();
        let (ts, tc) = (-std::f32::consts::FRAC_PI_4).sin_cos();
        part.mesh
            .vertices
            .iter()
            .map(|v| {
                let transform = (0..2)
                    .find(|i| v.bone_weights[*i] > 0.0)
                    .and_then(|i| palette.get(v.bone_indices[i] as usize))
                    .unwrap_or(&part.mesh.initial_matrix);
                let p = transform.transform_point(v.position);
                let rotated = Vector3 {
                    x: p.x * c - p.y * s,
                    y: p.x * s + p.y * c,
                    z: p.z,
                };
                (
                    [
                        rotated.x * 0.75,
                        (rotated.y * ts + rotated.z * tc) * 0.75,
                        (rotated.y * tc - rotated.z * ts) * 0.75,
                    ],
                    [v.texture_coordinate.x, v.texture_coordinate.y],
                )
            })
            .collect()
    }

    pub fn vertices_idle(
        &self,
        part_index: usize,
        facing: f32,
    ) -> Option<Vec<([f32; 3], [f32; 2])>> {
        self.parts
            .get(part_index)
            .map(|part| self.vertices(part, 0.0, Action::Idle, facing, None))
    }
}

impl NpcModel {
    /// Stable key for geometry that does not depend on animation time.
    /// Idle monsters reuse the same model and facing for many frames, so the
    /// renderer can keep their transformed vertices instead of rebuilding
    /// them on every redraw.
    pub fn static_vertices_key(&self, facing: f32) -> (u32, i8) {
        (
            self.model_id,
            (facing / std::f32::consts::FRAC_PI_4).round() as i8,
        )
    }

    pub fn vertices_static(&self, facing: f32) -> Vec<([f32; 3], [f32; 2])> {
        self.vertices(0.0, facing)
    }

    pub fn load_monster(assets: &mut Assets, catalog: &NpcCatalog, look: u32) -> Result<Self> {
        // Most looks live in `c3/monster/{look}N`. Others are only named by
        // the catalogs: the city guards (900, 910) use `c3/monster/900`,
        // where `3dmotion.ini` binds each look's stand motion (look * 10^6 +
        // 100) and `3dobj.ini` the model (look * 10^6).
        let catalog_id = look.saturating_mul(1_000_000);
        let cataloged = [
            catalog.motions.get(catalog_id + 100),
            catalog.models.get(catalog_id),
        ]
        .into_iter()
        .flatten()
        .map(|path| path.as_str().to_owned());
        let model_candidates = [
            format!("c3/monster/{look}N/{look:03}000000.c3"),
            format!("c3/monster/{look}n/{look:03}000000.c3"),
        ]
        .into_iter()
        .chain(cataloged);
        let Some((model_path, bytes)) = model_candidates
            .into_iter()
            .find_map(|path| assets.read(&path).ok().map(|bytes| (path, bytes)))
        else {
            return Err(format!("Monster model missing for look {look}").into());
        };
        let document = C3Document::parse(&bytes)?;
        // Monster model files contain the mesh, while their neutral pose is
        // defined by the dedicated 100 motion in 3dmotion.ini. Using the
        // model's first embedded motion leaves Pheasant with its wings spread.
        let mut load_motion = |name: &str| {
            let cataloged = name
                .parse::<u32>()
                .ok()
                .and_then(|action| catalog.motions.get(catalog_id + action))
                .map(|path| path.as_str().to_owned());
            [
                format!("c3/monster/{look}N/{name}.c3"),
                format!("c3/monster/{look}n/{name}.c3"),
            ]
            .into_iter()
            .chain(cataloged)
            .find_map(|path| {
                assets
                    .read(&path)
                    .ok()
                    .and_then(|bytes| C3Document::parse(&bytes).ok())
                    .and_then(|document| document.motions.into_iter().next())
            })
        };
        let idle_motion = load_motion("100");
        // 101 is a legacy stand-by loop for some monsters. Pheasant's
        // movement actions map to 110/120 in 3dmotion.ini.
        let walk_motion = load_motion("110").or_else(|| load_motion("101"));
        let attack_motion = load_motion("401");
        let death_motion = load_motion("101");
        let mesh = document
            .meshes
            .first()
            .ok_or("Monster mesh missing")?
            .clone();
        let model_id = catalog.models.ids_for_path(model_path.as_str()).next();
        // A look's own skin comes first (Guard2 shares Guard1's mesh but
        // not its texture).
        let texture = catalog
            .textures
            .get(catalog_id)
            .or_else(|| {
                mesh.texture_name
                    .trim_end_matches(".dds")
                    .parse::<u32>()
                    .ok()
                    .and_then(|id| catalog.textures.get(id))
            })
            .or_else(|| model_id.and_then(|id| catalog.textures.get(id)))
            .or_else(|| model_id.and_then(|id| catalog.textures.get(id / 10)))
            .map(|path| path.as_str().to_owned())
            .or_else(|| {
                catalog
                    .texture_for_model_path(model_path.as_str())
                    .map(str::to_owned)
            })
            .or_else(|| {
                let path = mesh.texture_name.replace('\\', "/");
                assets.read(&path).ok().map(|_| path)
            })
            .ok_or_else(|| format!("Monster texture missing for look {look} ({model_path})"))?;
        Ok(Self {
            mesh,
            texture,
            model_id: look,
            motion: idle_motion.or_else(|| document.motions.first().cloned()),
            walk_motion,
            attack_motion,
            death_motion,
        })
    }

    pub fn load(assets: &mut Assets, catalog: &NpcCatalog, kind: u16, look: u16) -> Result<Self> {
        // The 2030 packet has two different concepts here:
        //
        // * `kind` is the interaction category (shopkeeper, task NPC,
        //   storage, ...); many visually different NPCs share it.
        // * `look` identifies the concrete NPC appearance.  The 5065 asset
        //   catalog indexes most of those appearances by `look / 10`.
        //
        // Using `kind` as the catalog key made Pharmacists, Pedlars and
        // several quest NPCs look for a non-existent C3 file.  Keep `kind` as
        // a compatibility fallback for custom/terrain NPCs, but prefer the
        // appearance key derived from `look`.
        let appearance_type = u32::from(look) / 10;
        let section = catalog
            .appearances
            .section(&format!("NpcType{appearance_type}"))
            .or_else(|| catalog.appearances.section(&format!("NpcType{kind}")));
        let simple_id = section
            .and_then(|section| section.first("SimpleObjID"))
            .and_then(|value| value.parse::<u32>().ok())
            .unwrap_or_else(|| u32::from(look / 10));
        let standby_motion_id = section
            .and_then(|section| section.first("StandByMotion"))
            .and_then(|value| value.parse::<u32>().ok());
        let model_from_motion_path = standby_motion_id
            .and_then(|id| catalog.motions.get(id))
            .and_then(|path| catalog.models.id_for_path(path.as_str()));
        let motion_model = section
            .and_then(|section| section.first("StandByMotion"))
            .and_then(|value| value.parse::<u32>().ok())
            // The standby ID can also help locate legacy NPC model files.
            // The actual animation is loaded from 3dmotion.ini below.
            .and_then(|motion| (motion / 100).checked_sub(1));
        let model_candidates = [
            model_from_motion_path,
            motion_model,
            Some(9_990_000 + simple_id * 10),
            Some(9_990_000 + u32::from(look)),
            // Some 5065 asset packs omit the individual town-NPC files while
            // still shipping this generic NPC model. Keep the actor visible
            // until the missing art files are restored.
            Some(9_992_090),
        ];
        let (model_id, model_path) = model_candidates
            .into_iter()
            .flatten()
            .find_map(|id| catalog.models.get(id).map(|path| (id, path)))
            .ok_or("NPC model missing")?;
        let document = C3Document::parse(&assets.read(model_path.as_str())?)?;
        let mesh = document.meshes.first().ok_or("NPC mesh missing")?.clone();
        let standby_motion = standby_motion_id
            .and_then(|id| catalog.motions.get(id))
            .and_then(|path| assets.read(path.as_str()).ok())
            .and_then(|bytes| C3Document::parse(&bytes).ok())
            .and_then(|document| document.motions.into_iter().next());
        let texture_candidates = [
            mesh.texture_name
                .trim_end_matches(".dds")
                .parse::<u32>()
                .ok(),
            Some(model_id),
            Some(model_id / 10),
        ];
        let texture = texture_candidates
            .into_iter()
            .flatten()
            .find_map(|id| catalog.textures.get(id))
            .map(|path| path.as_str().to_owned())
            .or_else(|| {
                catalog
                    .texture_for_model_path(model_path.as_str())
                    .map(str::to_owned)
            })
            .or_else(|| {
                let path = mesh.texture_name.replace('\\', "/");
                assets.read(&path).ok().map(|_| path)
            })
            .ok_or_else(|| {
                format!(
                    "NPC texture missing for model {model_id} (mesh texture {:?})",
                    mesh.texture_name
                )
            })?;
        Ok(Self {
            mesh,
            texture,
            model_id,
            motion: standby_motion.or_else(|| document.motions.first().cloned()),
            walk_motion: None,
            attack_motion: None,
            death_motion: None,
        })
    }

    pub fn uses_generic_fallback(&self) -> bool {
        self.model_id == 9_992_090
    }

    pub fn has_motion(&self) -> bool {
        self.motion.is_some()
    }

    pub fn has_animated_motion(&self) -> bool {
        self.motion.as_ref().is_some_and(|motion| {
            motion.frame_count > 1
                && motion
                    .keys
                    .windows(2)
                    .any(|keys| keys[0].bone_matrices != keys[1].bone_matrices)
        })
    }

    pub fn vertices(&self, seconds: f32, facing: f32) -> Vec<([f32; 3], [f32; 2])> {
        self.vertices_with_motion(self.motion.as_ref(), seconds, facing)
    }

    pub fn vertices_walk(&self, seconds: f32, facing: f32) -> Vec<([f32; 3], [f32; 2])> {
        self.vertices_with_motion(
            self.walk_motion.as_ref().or(self.motion.as_ref()),
            seconds,
            facing,
        )
    }

    pub fn vertices_attack(&self, seconds: f32, facing: f32) -> Vec<([f32; 3], [f32; 2])> {
        self.vertices_with_motion(
            self.attack_motion.as_ref().or(self.motion.as_ref()),
            seconds,
            facing,
        )
    }

    pub fn vertices_death(&self, seconds: f32, facing: f32) -> Vec<([f32; 3], [f32; 2])> {
        self.vertices_with_motion(
            self.death_motion.as_ref().or(self.motion.as_ref()),
            seconds,
            facing,
        )
    }

    fn vertices_with_motion(
        &self,
        motion: Option<&C3Motion>,
        seconds: f32,
        facing: f32,
    ) -> Vec<([f32; 3], [f32; 2])> {
        let frame = motion.map_or(0.0, |motion| {
            (seconds * 30.0) % motion.frame_count.max(1) as f32
        });
        let (s, c) = facing.sin_cos();
        let (ts, tc) = (-std::f32::consts::FRAC_PI_4).sin_cos();
        self.mesh
            .vertices
            .iter()
            .map(|vertex| {
                let transform = motion
                    .and_then(|motion| {
                        (0..2)
                            .find(|i| vertex.bone_weights[*i] > 0.0)
                            .and_then(|i| {
                                Some(
                                    self.mesh
                                        .initial_matrix
                                        .multiply(motion.matrix_at(vertex.bone_indices[i], frame)),
                                )
                            })
                    })
                    .unwrap_or(self.mesh.initial_matrix);
                let p = transform.transform_point(vertex.position);
                let rotated = Vector3 {
                    x: p.x * c - p.y * s,
                    y: p.x * s + p.y * c,
                    z: p.z,
                };
                (
                    [
                        rotated.x * 0.75,
                        (rotated.y * ts + rotated.z * tc) * 0.75,
                        (rotated.y * tc - rotated.z * ts) * 0.75,
                    ],
                    [vertex.texture_coordinate.x, vertex.texture_coordinate.y],
                )
            })
            .collect()
    }
}

/// Folds a Tinter dye into an item id for the model loader.
pub fn dyed(item_id: u32, color: u8) -> u32 {
    if item_id == 0 || !(3..=9).contains(&color) {
        item_id
    } else {
        item_id + u32::from(color) * DYE_UNIT
    }
}

const DYE_UNIT: u32 = 10_000_000;

/// The item id and dye folded by [`dyed`].
fn undyed(item_id: u32) -> (u32, u8) {
    (item_id % DYE_UNIT, (item_id / DYE_UNIT) as u8)
}

/// The 5065 dye textures replace the hundreds digit of the texture id
/// (1130330 is colour 3, 1130430 colour 4...); missing ones keep the
/// default.
fn dyed_texture(textures: &IdPathCatalog, texture: u32, color: u8) -> u32 {
    if !(3..=9).contains(&color) {
        return texture;
    }
    let candidate = texture - (texture / 100 % 10) * 100 + u32::from(color) * 100;
    if textures.get(candidate).is_some() { candidate } else { texture }
}

fn load_attachment(
    assets: &mut Assets,
    models: &IdPathCatalog,
    textures: &IdPathCatalog,
    body: &C3Document,
    catalog: &str,
    key: u32,
    attachment: &str,
) -> Result<Part> {
    load_dyed_attachment(assets, models, textures, body, catalog, key, attachment, 0)
}

#[allow(clippy::too_many_arguments)]
fn load_dyed_attachment(
    assets: &mut Assets,
    models: &IdPathCatalog,
    textures: &IdPathCatalog,
    body: &C3Document,
    catalog: &str,
    key: u32,
    attachment: &str,
    color: u8,
) -> Result<Part> {
    let definition = SectionCatalog::parse(&assets.read(catalog)?);
    let entry = definition
        .section(&key.to_string())
        .ok_or_else(|| format!("Missing part {key} in {catalog}"))?;
    let model = entry.first("Mesh0").ok_or("Part Mesh0 missing")?.parse()?;
    let texture: u32 = entry
        .first("Texture0")
        .ok_or("Part Texture0 missing")?
        .parse()?;
    let texture = dyed_texture(textures, texture, color);
    let part =
        C3Document::parse(&assets.read(models.get(model).ok_or("Part model missing")?.as_str())?)?;
    let track = body
        .meshes
        .iter()
        .position(|m| m.name == attachment)
        .ok_or("Attachment missing")?;
    Ok(Part {
        mesh: part.meshes.first().ok_or("Empty part")?.clone(),
        texture: textures
            .get(texture)
            .ok_or("Part texture missing")?
            .as_str()
            .to_owned(),
        local: Some(part.motions.first().ok_or("Part motion missing")?.clone()),
        track,
    })
}
