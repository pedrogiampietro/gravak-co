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
    /// A weapon's own motion in some actions, by `Action::index`
    /// (`ini/WeaponMotion.ini`): the bow's string drawn while shooting.
    /// Empty, or `None` in an action, keeps `local`.
    actions_local: Vec<Option<C3Motion>>,
    track: usize,
    /// The weapon this part is (its item type), 0 for the rest.
    pub item: u32,
}
#[derive(Clone)]
pub struct Character {
    pub parts: Vec<Part>,
    actions: Vec<C3Document>,
    intervals: [f32; Action::COUNT],
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
    /// The name over the player's own booth while it is open.
    pub booth: Option<String>,
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
    /// How a monster dies (`3dmotion.ini`): falling (330) and then lying
    /// (331); flung away (340), or a short way (370), before lying.
    death_motion: Option<C3Motion>,
    lie_motion: Option<C3Motion>,
    fly_motion: Option<C3Motion>,
    short_fly_motion: Option<C3Motion>,
}

#[derive(Clone)]
pub struct NpcCatalog {
    appearances: SectionCatalog,
    /// `ini/armor.ini`: a monster look's mesh and skin (`[look * 10^6]`
    /// Mesh0/Texture0), as the original client reads them.
    armors: SectionCatalog,
    models: IdPathCatalog,
    motions: IdPathCatalog,
    textures: IdPathCatalog,
}

impl NpcCatalog {
    pub fn load(assets: &mut Assets) -> Result<Self> {
        Ok(Self {
            appearances: SectionCatalog::parse(&assets.read("ini/npc.ini")?),
            armors: SectionCatalog::parse(&assets.read("ini/armor.ini")?),
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
        // Each action's motion, and the number its file is named by
        // (`c3/0003/500/401.c3` is 401): `ini/WeaponMotion.ini` keys a
        // weapon's own motions by it.
        let mut motion = |action: Action| -> Result<(C3Document, Option<u32>)> {
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
            let code = std::path::Path::new(path.as_str())
                .file_stem()
                .and_then(|stem| stem.to_str()?.parse().ok());
            Ok((C3Document::parse(&assets.read(path.as_str())?)?, code))
        };
        // A body without a death motion falls back to standing.
        let actions = Action::ALL
            .into_iter()
            .map(|action| match action {
                _ if action == Action::Die || action == Action::Fly || action.emote() => motion(action).or_else(|_| motion(Action::Idle)),
                _ => motion(action),
            })
            .collect::<Result<Vec<_>>>()?;
        let (actions, action_codes): (Vec<C3Document>, Vec<Option<u32>>) = actions.into_iter().unzip();
        let mut parts = vec![Part {
            mesh,
            texture: textures
                .get(texture_id)
                .ok_or("Body texture missing")?
                .as_str()
                .to_owned(),
            local: None,
            actions_local: Vec::new(),
            track: index,
            item: 0,
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
            // Arrows are not drawn on the archer, as in the original: they
            // only make the bow shoot (and choose its stance above).
            if weapon == 0 || is_arrow(weapon) {
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
                let mut part = part;
                part.actions_local = weapon_action_motions(assets, weapon, &action_codes);
                part.item = weapon;
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
            booth: None,
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

    /// Takes over what `previous` knew that a model rebuilt for new gear
    /// does not: the profile from MsgUserInfo (attributes, profession,
    /// PK points, spouse), the guild, the booth and life.
    pub fn keep_profile_from(&mut self, previous: &Character) {
        self.guild = previous.guild.clone();
        self.guild_rank = previous.guild_rank.clone();
        self.guild_fund = previous.guild_fund;
        self.booth = previous.booth.clone();
        self.health = previous.health;
        self.maximum_health = previous.maximum_health.max(self.maximum_health);
        self.strength = previous.strength;
        self.agility = previous.agility;
        self.vitality = previous.vitality;
        self.spirit = previous.spirit;
        self.attribute_points = previous.attribute_points;
        self.current_class = previous.current_class;
        self.previous_class = previous.previous_class;
        self.rebirths = previous.rebirths;
        self.kill_points = previous.kill_points;
        self.spouse = previous.spouse.clone();
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
        let palette = self.palette(part, seconds, action, progress);
        Self::project(part, &palette, facing)
    }

    /// The weapons held and where the hand holds each now (its dummy,
    /// `v_r_weapon` or `v_l_weapon`, posed as `vertices` poses it): the
    /// effects of a weapon hang there, in the dummy's own units.
    pub fn weapon_matrices(&self, seconds: f32, action: Action, progress: Option<f32>) -> Vec<(u32, Matrix4)> {
        self.parts
            .iter()
            .filter(|part| part.item != 0)
            .map(|part| {
                let track = &self.actions[action.index()].motions[part.track];
                (part.item, track.matrix_at(0, self.frame(track, seconds, action, progress)))
            })
            .collect()
    }

    /// The frame of `track` at `seconds` into `action`, or at `progress`
    /// through it.
    fn frame(&self, track: &C3Motion, seconds: f32, action: Action, progress: Option<f32>) -> f32 {
        let frames = track.frame_count.max(1) as f32;
        progress.map_or_else(
            || (seconds * 1000.0 / self.intervals[action.index()]) % frames,
            |p| (p.clamp(0.0, 1.0) * frames).min(frames - f32::EPSILON),
        )
    }

    /// Each bone's transform of `part` in `action`.
    fn palette(&self, part: &Part, seconds: f32, action: Action, progress: Option<f32>) -> Vec<Matrix4> {
        let track = &self.actions[action.index()].motions[part.track];
        let frame = self.frame(track, seconds, action, progress);
        let action = action.index();
        // A weapon's motion for this action runs over the body's: the
        // bow's string is drawn and let go within the shot.
        let action_local = part.actions_local.get(action).and_then(Option::as_ref);
        let motion = action_local.or(part.local.as_ref()).unwrap_or(track);
        let local_frame = action_local.map_or(frame, |local| {
            frame / track.frame_count.max(1) as f32 * local.frame_count.max(1) as f32
        });
        let attachment = if part.local.is_some() {
            track.matrix_at(0, frame)
        } else {
            Matrix4::IDENTITY
        };
        (0..motion.bone_count)
            .map(|b| {
                part.mesh
                    .initial_matrix
                    .multiply(motion.matrix_at(b, local_frame))
                    .multiply(attachment)
            })
            .collect()
    }

    /// `part` posed by `palette`, turned to `facing` and seen from the
    /// game's camera.
    fn project(part: &Part, palette: &[Matrix4], facing: f32) -> Vec<([f32; 3], [f32; 2])> {
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
        // `armor.ini` first: Turtledove (304) is the Pheasant's mesh
        // (104000000) in its own skin, and has no model of its own.
        let armor = catalog.armors.section(&catalog_id.to_string());
        let armor_id = |key: &str| armor.and_then(|section| section.first(key)).and_then(|value| value.parse::<u32>().ok());
        let armor_mesh = armor_id("Mesh0").and_then(|id| catalog.models.get(id)).map(|path| path.as_str().to_owned());
        let armor_texture = armor_id("Texture0").and_then(|id| catalog.textures.get(id)).map(|path| path.as_str().to_owned());
        let cataloged = [
            catalog.motions.get(catalog_id + 100),
            catalog.models.get(catalog_id),
        ]
        .into_iter()
        .flatten()
        .map(|path| path.as_str().to_owned());
        let model_candidates = armor_mesh
            .into_iter()
            .chain([
                format!("c3/monster/{look}N/{look:03}000000.c3"),
                format!("c3/monster/{look}n/{look:03}000000.c3"),
            ])
            .chain(cataloged);
        // A motion file has no mesh; it is not the model.
        let Some((model_path, document)) = model_candidates.into_iter().find_map(|path| {
            let document = C3Document::parse(&assets.read(&path).ok()?).ok()?;
            (!document.meshes.is_empty()).then_some((path, document))
        }) else {
            return Err(format!("Monster model missing for look {look}").into());
        };
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
        let death_motion = load_motion("330");
        let lie_motion = load_motion("331");
        let fly_motion = load_motion("340");
        let short_fly_motion = load_motion("370");
        let mesh = document
            .meshes
            .first()
            .ok_or("Monster mesh missing")?
            .clone();
        let model_id = catalog.models.ids_for_path(model_path.as_str()).next();
        // A look's own skin comes first (Guard2 shares Guard1's mesh but
        // not its texture).
        let texture = armor_texture
            .or_else(|| {
                catalog
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
            })
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
            lie_motion,
            fly_motion,
            short_fly_motion,
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
            lie_motion: None,
            fly_motion: None,
            short_fly_motion: None,
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

    /// `elapsed` seconds into a death (`flung`: 0 falling, 1 flung away, 2
    /// flung a short way): its motion played once, then lying still.
    pub fn vertices_dying(&self, flung: u8, elapsed: f32, facing: f32) -> Vec<([f32; 3], [f32; 2])> {
        let dying = match flung {
            1 => self.fly_motion.as_ref().or(self.death_motion.as_ref()),
            2 => self.short_fly_motion.as_ref().or(self.fly_motion.as_ref()).or(self.death_motion.as_ref()),
            _ => self.death_motion.as_ref(),
        };
        let Some(dying) = dying else {
            return self.vertices_with_motion(self.motion.as_ref(), 0.0, facing);
        };
        let frames = dying.frame_count.max(1) as f32;
        let frame = elapsed.max(0.0) * 30.0;
        if frame < frames {
            return self.vertices_at_frame(Some(dying), frame, facing);
        }
        match &self.lie_motion {
            Some(lying) => self.vertices_at_frame(Some(lying), 0.0, facing),
            None => self.vertices_at_frame(Some(dying), frames - 1.0, facing),
        }
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
        self.vertices_at_frame(motion, frame, facing)
    }

    fn vertices_at_frame(
        &self,
        motion: Option<&C3Motion>,
        frame: f32,
        facing: f32,
    ) -> Vec<([f32; 3], [f32; 2])> {
        let (s, c) = facing.sin_cos();
        let (ts, tc) = (-std::f32::consts::FRAC_PI_4).sin_cos();
        // Each bone's matrix once per pose, not once per vertex: the key
        // search and blend made a pose of a 37-bone Birdman costly enough
        // that only one walking monster a frame could have one.
        let bones: Vec<_> = motion.map_or_else(Vec::new, |motion| {
            (0..motion.bone_count)
                .map(|bone| self.mesh.initial_matrix.multiply(motion.matrix_at(bone, frame)))
                .collect()
        });
        self.mesh
            .vertices
            .iter()
            .map(|vertex| {
                let transform = motion
                    .and_then(|_| {
                        (0..2)
                            .find(|i| vertex.bone_weights[*i] > 0.0)
                            .map(|i| bones.get(vertex.bone_indices[i] as usize).copied().unwrap_or(self.mesh.initial_matrix))
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

/// Whether `item` is a pack of arrows (`1050xxx`).
pub fn is_arrow(item: u32) -> bool {
    item % DYE_UNIT / 1_000 == 1_050
}

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
        actions_local: Vec::new(),
        local: Some(
            (catalog == "ini/weapon.ini")
                .then(|| weapon_motion(assets, key))
                .flatten()
                .map_or_else(|| part.motions.first().cloned().ok_or("Part motion missing"), Ok)?,
        ),
        track,
        item: 0,
    })
}

/// The pose `ini/WeaponMotion.ini` gives weapon `weapon` in every action
/// (`<item>999999`), as the original client uses it: arrows hang down the
/// archer's side from it instead of pointing out of the hand as their own
/// model holds them. Bows keep their own pose there; the per-action entries
/// (the drawn string while shooting) are not used yet.
fn weapon_motion(assets: &mut Assets, weapon: u32) -> Option<C3Motion> {
    weapon_motions(assets, weapon, &[Some(999)]).pop().flatten()
}

/// Weapon `weapon`'s own motion in each action, from its body motion's
/// number `codes[i]` (`<item>999<code>` in `ini/WeaponMotion.ini`): the
/// bow's 401 draws its string. `None` where the file has none.
fn weapon_action_motions(assets: &mut Assets, weapon: u32, codes: &[Option<u32>]) -> Vec<Option<C3Motion>> {
    let motions = weapon_motions(assets, weapon, codes);
    if motions.iter().all(Option::is_none) { Vec::new() } else { motions }
}

fn weapon_motions(assets: &mut Assets, weapon: u32, codes: &[Option<u32>]) -> Vec<Option<C3Motion>> {
    let Ok(text) = assets.read("ini/WeaponMotion.ini") else {
        return vec![None; codes.len()];
    };
    let text = String::from_utf8_lossy(&text);
    let prefix = format!("{weapon}999");
    // This weapon's entries, by motion number.
    let entries: std::collections::HashMap<u32, String> = text
        .lines()
        .filter_map(|line| {
            let (id, path) = line.trim().split_once('=')?;
            let code = id.trim().strip_prefix(prefix.as_str())?;
            (code.len() == 3).then_some(())?;
            Some((code.parse().ok()?, path.trim().to_owned()))
        })
        .collect();
    let mut loaded: std::collections::HashMap<String, Option<C3Motion>> = std::collections::HashMap::new();
    codes
        .iter()
        .map(|code| {
            let path = entries.get(&(*code)?)?;
            loaded
                .entry(path.clone())
                .or_insert_with(|| {
                    C3Document::parse(&assets.read(path).ok()?).ok()?.motions.into_iter().next()
                })
                .clone()
        })
        .collect()
}



#[cfg(test)]
mod monster_probe {
    use super::*;

    /// Loads every monster look the server spawns and names those that fail
    /// or have no walk.
    #[test]
    #[ignore = "Needs the 5065 installation"]
    fn every_server_monster_loads() {
        let mut assets = Assets::open(std::path::Path::new("R:/5065")).unwrap();
        let catalog = NpcCatalog::load(&mut assets).unwrap();
        let looks = [103,104,105,108,117,126,127,129,130,131,132,133,136,145,148,151,152,153,154,155,156,157,160,162,163,164,165,166,168,171,179,188,196,197,200,201,202,203,204,205,206,207,209,211,213,214,217,222,224,225,226,227,230,240,241,242,243,244,245,246,247,248,249,250,252,254,255,257,258,260,262,263,265,266,267,268,273,274,277,303,304,305,306,307,308,309,310,311,312,313,314,317,325,326,327,328,329,332,348,353,354,355,356,357,360,361,362,363,373,374,377,380,381,382,383,397,400,401,402,403,404,405,406,425,426,442,443,463,503,505,517,531,536,548,556,597,601,602,603,604,605,606,900,910,920];
        // The bosses TeratoDragon, SnowBanshee, ThrillingSpook and
        // SwordMaster (950-953) came after 5065: it has no art for them.
        let mut broken = Vec::new();
        for look in looks {
            match NpcModel::load_monster(&mut assets, &catalog, look) {
                Ok(model) if model.walk_motion.is_none() => broken.push(format!("{look}: no walk")),
                Ok(_) => {}
                Err(error) => broken.push(format!("{look}: {error}")),
            }
        }
        println!("{}", broken.join("
"));
        assert!(broken.is_empty(), "{} looks broken", broken.len());
    }
}
