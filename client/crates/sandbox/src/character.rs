// Source: @digitalm1nd on x.com / _dek on Discord — Preservation Conquer project — https://discord.gg/CvKPXEHYRY
use crate::action::Action;
use crate::assets::{Assets, Result};
use sandbox_data::{
    c3::{C3Document, C3Mesh, C3Motion, Matrix4, Vector3},
    ini::{IdPathCatalog, SectionCatalog},
};

pub struct Part {
    pub mesh: C3Mesh,
    pub texture: String,
    local: Option<C3Motion>,
    track: usize,
}
pub struct Character {
    pub parts: Vec<Part>,
    actions: Vec<C3Document>,
    intervals: [f32; 6],
    pub body_id: u32,
    pub weapon_action: u32,
    pub name: String,
    pub guild: String,
    pub guild_rank: String,
    pub health: u32,
    pub maximum_health: u32,
}

pub struct NpcModel {
    pub mesh: C3Mesh,
    pub texture: String,
    motion: Option<C3Motion>,
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
            if armor == 0 { 130109 } else { armor },
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
        hair: u32,
        right: u32,
        left: u32,
        name: String,
        guild: String,
        guild_rank: String,
        health: u32,
        maximum_health: u32,
    ) -> Result<Self> {
        let armor = SectionCatalog::parse(&assets.read("ini/armor.ini")?);
        let entry = armor
            .section(&(body * 1_000_000 + armor_id / 10 * 10).to_string())
            .ok_or("Missing armor in armor.ini")?;
        let models = IdPathCatalog::parse(&assets.read("ini/3dobj.ini")?);
        let textures = IdPathCatalog::parse(&assets.read("ini/3dtexture.ini")?);
        let motions = IdPathCatalog::parse(&assets.read("ini/3dmotion.ini")?);
        let model_id = entry.first("Mesh0").ok_or("Missing Mesh0")?.parse()?;
        let texture_id = entry.first("Texture0").ok_or("Missing Texture0")?.parse()?;
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
        let actions = Action::ALL
            .into_iter()
            .map(&mut motion)
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
        for (weapon, attachment) in [(right, "v_r_weapon"), (left, "v_l_weapon")] {
            if weapon == 0 {
                continue;
            }
            if let Ok(part) = load_attachment(
                assets,
                &models,
                &textures,
                &doc,
                "ini/weapon.ini",
                weapon,
                attachment,
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
            weapon_action,
            name,
            guild,
            guild_rank,
            health,
            maximum_health,
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
}

impl NpcModel {
    pub fn load(assets: &mut Assets, kind: u16, look: u16) -> Result<Self> {
        let npc_catalog = SectionCatalog::parse(&assets.read("ini/npc.ini")?);
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
        let section = npc_catalog
            .section(&format!("NpcType{appearance_type}"))
            .or_else(|| npc_catalog.section(&format!("NpcType{kind}")));
        let simple_id = section
            .and_then(|section| section.first("SimpleObjID"))
            .and_then(|value| value.parse::<u32>().ok())
            .unwrap_or_else(|| u32::from(look / 10));
        let models = IdPathCatalog::parse(&assets.read("ini/3dobj.ini")?);
        let motion_model = section
            .and_then(|section| section.first("StandByMotion"))
            .and_then(|value| value.parse::<u32>().ok())
            // NPC motion IDs append an action/frame suffix to the model ID.
            // For example 999009100 -> model 9990090.
            .and_then(|motion| (motion / 100).checked_sub(1));
        let model_candidates = [
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
            .find_map(|id| models.get(id).map(|path| (id, path)))
            .ok_or("NPC model missing")?;
        let document = C3Document::parse(&assets.read(model_path.as_str())?)?;
        let mesh = document.meshes.first().ok_or("NPC mesh missing")?.clone();
        let textures = IdPathCatalog::parse(&assets.read("ini/3dtexture.ini")?);
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
            .find_map(|id| textures.get(id))
            .ok_or("NPC texture missing")?
            .as_str()
            .to_owned();
        Ok(Self {
            mesh,
            texture,
            motion: document.motions.first().cloned(),
        })
    }

    pub fn vertices(&self, seconds: f32, facing: f32) -> Vec<([f32; 3], [f32; 2])> {
        let frame = self.motion.as_ref().map_or(0.0, |motion| {
            (seconds * 30.0) % motion.frame_count.max(1) as f32
        });
        let (s, c) = facing.sin_cos();
        let (ts, tc) = (-std::f32::consts::FRAC_PI_4).sin_cos();
        self.mesh
            .vertices
            .iter()
            .map(|vertex| {
                let transform = self
                    .motion
                    .as_ref()
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

fn load_attachment(
    assets: &mut Assets,
    models: &IdPathCatalog,
    textures: &IdPathCatalog,
    body: &C3Document,
    catalog: &str,
    key: u32,
    attachment: &str,
) -> Result<Part> {
    let definition = SectionCatalog::parse(&assets.read(catalog)?);
    let entry = definition
        .section(&key.to_string())
        .ok_or_else(|| format!("Missing part {key} in {catalog}"))?;
    let model = entry.first("Mesh0").ok_or("Part Mesh0 missing")?.parse()?;
    let texture = entry
        .first("Texture0")
        .ok_or("Part Texture0 missing")?
        .parse()?;
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
