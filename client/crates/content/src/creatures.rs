//! Creatures of our own (`[[creature]]` in `content/monsters.toml`): new
//! models, rigged and animated elsewhere (Meshy's humanoid rig, the
//! Mixamo skeleton), brought in from GLB as the client's C3.
//!
//! ```toml
//! [[creature]]
//! look = 981                                  # the new look
//! folder = "art/monstros/sacerdote-de-sangue" # walk.glb, and optionally
//!                                             # run, idle, attack, death
//! name = "SacerdoteSangue"                    # only for us
//! height = 200                                # standing height (C3 units)
//! ```
//!
//! `walk.glb` gives the mesh, its skin and texture, and the walk. Each other
//! motion comes from its own GLB when there is one, else it is made from the
//! skeleton: standing (the walk's average pose, breathing), a blow (the
//! right arm raised forward and brought down) and a fall backward.
//!
//! A vertex keeps its two strongest bones. A bone's matrix is its pose in
//! the client's space: `A * global * inverse bind`, where `A` takes the
//! model's space (Y up, facing +Z, metres) to the client's (Z down, facing
//! -Y, the height asked for, feet at zero). Matrices are written for row vectors,
//! as C3 keeps them.
//!
//! The build writes `c3/monster/gravak-<look>/model.c3` (the mesh and its
//! standing motion), one `.c3` per motion, the texture
//! (`c3/texture/gravak/monster-<look>.dds`, 512 pixels), and the look's
//! lines in `3dobj.ini`, `armor.ini`, `3dtexture.ini` and `3dmotion.ini`.

use crate::Result;
use image::RgbaImage;
use serde::Deserialize;
use serde_json::Value;
use std::path::Path;

const FPS: f32 = 30.0;
const TEXTURE_SIZE: u32 = 512;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreatureDef {
    pub look: u32,
    pub folder: String,
    #[serde(default)]
    pub name: String,
    #[serde(default = "default_height")]
    pub height: f32,
}

fn default_height() -> f32 {
    200.0
}

/// A 4x4 matrix for column vectors: `m[row][column]`.
type Mat = [[f32; 4]; 4];

const IDENTITY: Mat = [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]];

fn mul(a: &Mat, b: &Mat) -> Mat {
    std::array::from_fn(|r| std::array::from_fn(|c| (0..4).map(|k| a[r][k] * b[k][c]).sum()))
}

fn apply(m: &Mat, p: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|r| m[r][0] * p[0] + m[r][1] * p[1] + m[r][2] * p[2] + m[r][3])
}

fn translation(t: [f32; 3]) -> Mat {
    let mut m = IDENTITY;
    m[0][3] = t[0];
    m[1][3] = t[1];
    m[2][3] = t[2];
    m
}

/// A rotation of `angle` radians about the unit `axis`.
fn rotation(axis: [f32; 3], angle: f32) -> Mat {
    let half = angle / 2.0;
    let s = half.sin();
    quaternion([axis[0] * s, axis[1] * s, axis[2] * s, half.cos()])
}

/// `[x, y, z, w]`.
fn quaternion(q: [f32; 4]) -> Mat {
    let [x, y, z, w] = q;
    [
        [1.0 - 2.0 * (y * y + z * z), 2.0 * (x * y - z * w), 2.0 * (x * z + y * w), 0.0],
        [2.0 * (x * y + z * w), 1.0 - 2.0 * (x * x + z * z), 2.0 * (y * z - x * w), 0.0],
        [2.0 * (x * z - y * w), 2.0 * (y * z + x * w), 1.0 - 2.0 * (x * x + y * y), 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

fn compose(t: [f32; 3], r: [f32; 4], s: [f32; 3]) -> Mat {
    let mut m = quaternion(r);
    for row in 0..3 {
        m[row][0] *= s[0];
        m[row][1] *= s[1];
        m[row][2] *= s[2];
    }
    m[0][3] = t[0];
    m[1][3] = t[1];
    m[2][3] = t[2];
    m
}

fn slerp(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    let mut dot: f32 = (0..4).map(|i| a[i] * b[i]).sum();
    let b = if dot < 0.0 {
        dot = -dot;
        b.map(|v| -v)
    } else {
        b
    };
    let (wa, wb) = if dot > 0.9995 {
        (1.0 - t, t)
    } else {
        let theta = dot.clamp(-1.0, 1.0).acos();
        let sin = theta.sin();
        (((1.0 - t) * theta).sin() / sin, (t * theta).sin() / sin)
    };
    let q: [f32; 4] = std::array::from_fn(|i| a[i] * wa + b[i] * wb);
    let length = q.iter().map(|v| v * v).sum::<f32>().sqrt().max(1e-9);
    q.map(|v| v / length)
}

/// A node's local transform.
#[derive(Clone, Copy, Debug)]
struct Trs {
    t: [f32; 3],
    r: [f32; 4],
    s: [f32; 3],
}

impl Trs {
    fn matrix(&self) -> Mat {
        compose(self.t, self.r, self.s)
    }
}

/// What a GLB holds that a creature needs.
struct Gltf {
    json: Value,
    bin: Vec<u8>,
}

impl Gltf {
    fn parse(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < 20 || &bytes[0..4] != b"glTF" {
            return Err("not a GLB file".into());
        }
        let json_length = u32::from_le_bytes(bytes[12..16].try_into()?) as usize;
        let json: Value = serde_json::from_slice(&bytes[20..20 + json_length])?;
        let bin_start = 20 + json_length;
        let bin = if bytes.len() >= bin_start + 8 {
            let length = u32::from_le_bytes(bytes[bin_start..bin_start + 4].try_into()?) as usize;
            bytes[bin_start + 8..bin_start + 8 + length].to_vec()
        } else {
            Vec::new()
        };
        Ok(Self { json, bin })
    }

    fn get(&self, kind: &str, index: usize) -> Result<&Value> {
        self.json[kind].get(index).ok_or_else(|| format!("glTF {kind} {index} missing").into())
    }

    fn view(&self, index: usize) -> Result<(&[u8], usize)> {
        let view = self.get("bufferViews", index)?;
        let offset = view["byteOffset"].as_u64().unwrap_or(0) as usize;
        let length = view["byteLength"].as_u64().unwrap_or(0) as usize;
        let stride = view["byteStride"].as_u64().unwrap_or(0) as usize;
        let bytes = self.bin.get(offset..offset + length).ok_or("glTF buffer view out of range")?;
        Ok((bytes, stride))
    }

    /// An accessor's values as floats, `components` to an element.
    fn floats(&self, index: usize) -> Result<(Vec<f32>, usize)> {
        let accessor = self.get("accessors", index)?;
        let count = accessor["count"].as_u64().unwrap_or(0) as usize;
        let components = match accessor["type"].as_str().unwrap_or("SCALAR") {
            "SCALAR" => 1,
            "VEC2" => 2,
            "VEC3" => 3,
            "VEC4" => 4,
            "MAT4" => 16,
            other => return Err(format!("glTF accessor type {other}").into()),
        };
        let kind = accessor["componentType"].as_u64().unwrap_or(5126);
        let normalized = accessor["normalized"].as_bool().unwrap_or(false);
        let size = match kind {
            5120 | 5121 => 1,
            5122 | 5123 => 2,
            _ => 4,
        };
        let view = accessor["bufferView"].as_u64().ok_or("glTF accessor without a view")? as usize;
        let (bytes, stride) = self.view(view)?;
        let offset = accessor["byteOffset"].as_u64().unwrap_or(0) as usize;
        let stride = if stride == 0 { size * components } else { stride };
        let mut values = Vec::with_capacity(count * components);
        for element in 0..count {
            for component in 0..components {
                let at = offset + element * stride + component * size;
                let raw = bytes.get(at..at + size).ok_or("glTF accessor out of range")?;
                let value = match kind {
                    5121 => f32::from(raw[0]) / if normalized { 255.0 } else { 1.0 },
                    5120 => f32::from(raw[0] as i8) / if normalized { 127.0 } else { 1.0 },
                    5123 => f32::from(u16::from_le_bytes([raw[0], raw[1]])) / if normalized { 65535.0 } else { 1.0 },
                    5122 => f32::from(i16::from_le_bytes([raw[0], raw[1]])) / if normalized { 32767.0 } else { 1.0 },
                    5125 => u32::from_le_bytes(raw.try_into()?) as f32,
                    _ => f32::from_le_bytes(raw.try_into()?),
                };
                values.push(value);
            }
        }
        Ok((values, components))
    }

    fn nodes(&self) -> Vec<Trs> {
        self.json["nodes"]
            .as_array()
            .map(|nodes| {
                nodes
                    .iter()
                    .map(|node| {
                        let read = |key: &str, default: &[f32]| -> Vec<f32> {
                            node[key]
                                .as_array()
                                .map(|values| values.iter().map(|v| v.as_f64().unwrap_or(0.0) as f32).collect())
                                .unwrap_or_else(|| default.to_vec())
                        };
                        let (t, r, s) = (read("translation", &[0.0; 3]), read("rotation", &[0.0, 0.0, 0.0, 1.0]), read("scale", &[1.0; 3]));
                        Trs { t: [t[0], t[1], t[2]], r: [r[0], r[1], r[2], r[3]], s: [s[0], s[1], s[2]] }
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    fn names(&self) -> Vec<String> {
        self.json["nodes"]
            .as_array()
            .map(|nodes| nodes.iter().map(|node| node["name"].as_str().unwrap_or("").to_owned()).collect())
            .unwrap_or_default()
    }

    fn parents(&self) -> Vec<Option<usize>> {
        let count = self.json["nodes"].as_array().map_or(0, Vec::len);
        let mut parents = vec![None; count];
        for (index, node) in self.json["nodes"].as_array().into_iter().flatten().enumerate() {
            for child in node["children"].as_array().into_iter().flatten() {
                if let Some(child) = child.as_u64() {
                    parents[child as usize] = Some(index);
                }
            }
        }
        parents
    }
}

/// An animation's channels by node name: (path, times, values).
struct Clip {
    channels: Vec<(String, String, Vec<f32>, Vec<f32>)>,
    duration: f32,
}

impl Clip {
    fn read(gltf: &Gltf) -> Result<Option<Self>> {
        let Some(animation) = gltf.json["animations"].get(0) else { return Ok(None) };
        let names = gltf.names();
        let mut channels = Vec::new();
        let mut duration: f32 = 0.0;
        for channel in animation["channels"].as_array().into_iter().flatten() {
            let node = channel["target"]["node"].as_u64().unwrap_or(0) as usize;
            let path = channel["target"]["path"].as_str().unwrap_or("").to_owned();
            let sampler = &animation["samplers"][channel["sampler"].as_u64().unwrap_or(0) as usize];
            let (times, _) = gltf.floats(sampler["input"].as_u64().unwrap_or(0) as usize)?;
            let (values, _) = gltf.floats(sampler["output"].as_u64().unwrap_or(0) as usize)?;
            duration = duration.max(times.iter().copied().fold(0.0, f32::max));
            channels.push((names.get(node).cloned().unwrap_or_default(), path, times, values));
        }
        Ok(Some(Self { channels, duration }))
    }

    /// `rest` posed at `time`: the clip's channels over the named nodes.
    fn pose(&self, rest: &[Trs], names: &[String], time: f32) -> Vec<Trs> {
        let mut pose = rest.to_vec();
        for (name, path, times, values) in &self.channels {
            let Some(node) = names.iter().position(|n| n == name) else { continue };
            let width = if path == "rotation" { 4 } else { 3 };
            if times.is_empty() || values.len() < times.len() * width {
                continue;
            }
            let next = times.iter().position(|t| *t > time).unwrap_or(times.len() - 1);
            let previous = next.saturating_sub(1);
            let span = times[next] - times[previous];
            let amount = if span > 0.0 { ((time - times[previous]) / span).clamp(0.0, 1.0) } else { 0.0 };
            let at = |key: usize| &values[key * width..key * width + width];
            match path.as_str() {
                "rotation" => {
                    let (a, b) = (at(previous), at(next));
                    pose[node].r = slerp([a[0], a[1], a[2], a[3]], [b[0], b[1], b[2], b[3]], amount);
                },
                "translation" | "scale" => {
                    let (a, b) = (at(previous), at(next));
                    let v = std::array::from_fn(|i| a[i] + (b[i] - a[i]) * amount);
                    if path == "translation" {
                        pose[node].t = v;
                    } else {
                        pose[node].s = v;
                    }
                },
                _ => {},
            }
        }
        pose
    }
}

fn globals(locals: &[Trs], parents: &[Option<usize>]) -> Vec<Mat> {
    let mut out: Vec<Option<Mat>> = vec![None; locals.len()];
    fn resolve(i: usize, locals: &[Trs], parents: &[Option<usize>], out: &mut Vec<Option<Mat>>) -> Mat {
        if let Some(m) = out[i] {
            return m;
        }
        let local = locals[i].matrix();
        let m = match parents[i] {
            Some(p) => mul(&resolve(p, locals, parents, out), &local),
            None => local,
        };
        out[i] = Some(m);
        m
    }
    (0..locals.len()).map(|i| resolve(i, locals, parents, &mut out)).collect()
}

/// A creature read from its GLBs.
pub struct Creature {
    pub def: CreatureDef,
    /// Mesh space positions, texture coordinates, two bones and weights.
    vertices: Vec<([f32; 3], [f32; 2], [u32; 2], [f32; 2])>,
    indices: Vec<u16>,
    texture: RgbaImage,
    /// Each motion: its action ids and its frames' bone matrices (C3, row
    /// vectors).
    pub motions: Vec<(Vec<u32>, Vec<Vec<[f32; 16]>>)>,
    bounds: ([f32; 3], [f32; 3]),
}

/// The motions of a monster by the client's action ids.
const STAND: [u32; 2] = [100, 105];
const WALK: [u32; 3] = [101, 110, 120];
const ATTACK: [u32; 2] = [401, 402];
const DEATH: [u32; 3] = [330, 340, 370];
const LIE: [u32; 1] = [331];

impl Creature {
    pub fn load(content: &Path, def: &CreatureDef) -> Result<Self> {
        let folder = content.join(&def.folder);
        let read = |name: &str| -> Result<Option<Gltf>> {
            let path = folder.join(format!("{name}.glb"));
            if !path.exists() {
                return Ok(None);
            }
            Ok(Some(Gltf::parse(&std::fs::read(&path)?).map_err(|error| format!("{}: {error}", path.display()))?))
        };
        let walk = read("walk")?.ok_or_else(|| format!("criatura {}: falta {}/walk.glb", def.name, def.folder))?;

        // The mesh: its first primitive, skinned.
        let primitive = &walk.json["meshes"][0]["primitives"][0];
        let attribute = |name: &str| primitive["attributes"][name].as_u64().map(|v| v as usize);
        let (positions, _) = walk.floats(attribute("POSITION").ok_or("sem POSITION")?)?;
        let (uvs, _) = walk.floats(attribute("TEXCOORD_0").ok_or("sem TEXCOORD_0")?)?;
        let (joints, _) = walk.floats(attribute("JOINTS_0").ok_or("sem JOINTS_0 (o modelo precisa do rig)")?)?;
        let (weights, _) = walk.floats(attribute("WEIGHTS_0").ok_or("sem WEIGHTS_0")?)?;
        let (indices, _) = walk.floats(primitive["indices"].as_u64().ok_or("sem índices")? as usize)?;
        let count = positions.len() / 3;
        if count > usize::from(u16::MAX) {
            return Err(format!("criatura {}: {count} vértices, o máximo é 65535", def.name).into());
        }
        let vertices = (0..count)
            .map(|i| {
                let mut influences: Vec<(u32, f32)> = (0..4).map(|k| (joints[i * 4 + k] as u32, weights[i * 4 + k])).collect();
                influences.sort_by(|a, b| b.1.total_cmp(&a.1));
                let total = (influences[0].1 + influences[1].1).max(1e-6);
                (
                    [positions[i * 3], positions[i * 3 + 1], positions[i * 3 + 2]],
                    [uvs[i * 2], uvs[i * 2 + 1]],
                    [influences[0].0, influences[1].0],
                    [influences[0].1 / total, influences[1].1 / total],
                )
            })
            .collect::<Vec<_>>();
        let indices = indices.iter().map(|i| *i as u16).collect::<Vec<_>>();

        // The texture: the material's base colour, at 512 pixels.
        let material = primitive["material"].as_u64().unwrap_or(0) as usize;
        let texture_index = walk.json["materials"][material]["pbrMetallicRoughness"]["baseColorTexture"]["index"].as_u64().unwrap_or(0) as usize;
        let image_index = walk.json["textures"][texture_index]["source"].as_u64().unwrap_or(0) as usize;
        let view = walk.json["images"][image_index]["bufferView"].as_u64().ok_or("textura fora do GLB")? as usize;
        let (image_bytes, _) = walk.view(view)?;
        let texture = image::load_from_memory(image_bytes)?.to_rgba8();
        let texture = image::imageops::resize(&texture, TEXTURE_SIZE, TEXTURE_SIZE, image::imageops::FilterType::Lanczos3);

        // The skeleton.
        let skin = &walk.json["skins"][0];
        let joint_nodes: Vec<usize> = skin["joints"].as_array().ok_or("sem esqueleto")?.iter().map(|v| v.as_u64().unwrap_or(0) as usize).collect();
        let (inverse, _) = walk.floats(skin["inverseBindMatrices"].as_u64().ok_or("sem inverseBindMatrices")? as usize)?;
        // glTF keeps matrices by columns.
        let inverse_bind: Vec<Mat> = (0..joint_nodes.len())
            .map(|j| std::array::from_fn(|r| std::array::from_fn(|c| inverse[j * 16 + c * 4 + r])))
            .collect();
        let rest = walk.nodes();
        let names = walk.names();
        let parents = walk.parents();
        let joint = |name: &str| joint_nodes.iter().position(|n| names[*n].ends_with(name));
        let hips = joint("Hips").ok_or("o esqueleto não tem Hips")?;

        let walk_clip = Clip::read(&walk)?.ok_or("walk.glb sem animação")?;
        let sample = |clip: &Clip| -> Vec<Vec<Trs>> {
            let frames = ((clip.duration * FPS).round() as usize).max(1);
            (0..frames).map(|f| clip.pose(&rest, &names, f as f32 / FPS)).collect()
        };
        let walking = sample(&walk_clip);
        // Standing: each bone's average turn over the walk.
        let average: Vec<Trs> = (0..rest.len())
            .map(|node| {
                let mut q = [0.0f32; 4];
                let first = walking[0][node].r;
                for frame in &walking {
                    let r = frame[node].r;
                    let sign = if (0..4).map(|i| r[i] * first[i]).sum::<f32>() < 0.0 { -1.0 } else { 1.0 };
                    for i in 0..4 {
                        q[i] += r[i] * sign;
                    }
                }
                let length = q.iter().map(|v| v * v).sum::<f32>().sqrt().max(1e-9);
                let t = walking.iter().fold([0.0f32; 3], |acc, frame| std::array::from_fn(|i| acc[i] + frame[node].t[i] / walking.len() as f32));
                Trs { t, r: q.map(|v| v / length), s: rest[node].s }
            })
            .collect();

        // Space: Y up, facing +Z → the client's Z down, facing -Y (the
        // originals' front), its height, feet at zero, the hips over the cell
        // (the walk in place).
        let standing_globals = globals(&average, &parents);
        let skinned = |globals: &[Mat]| -> Vec<[f32; 3]> {
            vertices
                .iter()
                .map(|(p, _, bones, weights)| {
                    let mut out = [0.0f32; 3];
                    for k in 0..2 {
                        let m = mul(&globals[joint_nodes[bones[k] as usize]], &inverse_bind[bones[k] as usize]);
                        let q = apply(&m, *p);
                        for i in 0..3 {
                            out[i] += q[i] * weights[k];
                        }
                    }
                    out
                })
                .collect()
        };
        let standing_points = skinned(&standing_globals);
        let (low, high) = standing_points.iter().fold(([f32::MAX; 3], [f32::MIN; 3]), |(lo, hi), p| {
            (std::array::from_fn(|i| lo[i].min(p[i])), std::array::from_fn(|i| hi[i].max(p[i])))
        });
        let scale = def.height / (high[1] - low[1]).max(1e-3);
        let hips_rest = apply(&standing_globals[joint_nodes[hips]], [0.0; 3]);
        let to_client: Mat = mul(
            &[[-scale, 0.0, 0.0, 0.0], [0.0, 0.0, -scale, 0.0], [0.0, -scale, 0.0, 0.0], [0.0, 0.0, 0.0, 1.0]],
            &translation([-hips_rest[0], -low[1], -hips_rest[2]]),
        );

        let hips_node = joint_nodes[hips];
        // A frame's bone matrices, the hips kept over the cell.
        let frame = |locals: &[Trs], world: Option<&dyn Fn(usize, &Mat) -> Mat>| -> Vec<[f32; 16]> {
            let mut locals = locals.to_vec();
            locals[hips_node].t[0] = average[hips_node].t[0];
            locals[hips_node].t[2] = average[hips_node].t[2];
            let mut g = globals(&locals, &parents);
            if let Some(world) = world {
                for (node, m) in g.iter_mut().enumerate() {
                    *m = world(node, m);
                }
            }
            joint_nodes
                .iter()
                .enumerate()
                .map(|(j, node)| {
                    let m = mul(&to_client, &mul(&g[*node], &inverse_bind[j]));
                    // Row vectors: the transpose, row by row.
                    std::array::from_fn(|i| m[i % 4][i / 4])
                })
                .collect()
        };

        let mut motions = Vec::new();
        // Standing: breathing over two seconds.
        let spine = joint("Spine1").or_else(|| joint("Spine"));
        let standing = match read("idle")?.as_ref().map(Clip::read).transpose()?.flatten() {
            Some(clip) => sample(&clip).iter().map(|locals| frame(locals, None)).collect(),
            None => (0..60)
                .map(|f| {
                    let mut locals = average.clone();
                    if let Some(spine) = spine {
                        let breath = (f as f32 / 60.0 * std::f32::consts::TAU).sin() * 0.025;
                        let turn = mul(&quaternion(locals[joint_nodes[spine]].r), &rotation([1.0, 0.0, 0.0], breath));
                        locals[joint_nodes[spine]].r = matrix_quaternion(&turn);
                    }
                    frame(&locals, None)
                })
                .collect(),
        };
        motions.push((STAND.to_vec(), standing));
        motions.push((WALK.to_vec(), walking.iter().map(|locals| frame(locals, None)).collect()));
        if let Some(clip) = read("run")?.as_ref().map(Clip::read).transpose()?.flatten() {
            motions.push((vec![140], sample(&clip).iter().map(|locals| frame(locals, None)).collect()));
        }
        // The blow: the right arm swung up forward, then down, over 0.8 s.
        let attack = match read("attack")?.as_ref().map(Clip::read).transpose()?.flatten() {
            Some(clip) => sample(&clip).iter().map(|locals| frame(locals, None)).collect(),
            None => {
                let arm = joint("RightArm").map(|j| joint_nodes[j]);
                let descendants = |root: usize| -> Vec<bool> {
                    (0..rest.len())
                        .map(|mut node| loop {
                            if node == root {
                                break true;
                            }
                            match parents[node] {
                                Some(p) => node = p,
                                None => break false,
                            }
                        })
                        .collect()
                };
                let in_arm = arm.map(descendants).unwrap_or_default();
                (0..24)
                    .map(|f| {
                        let t = f as f32 / 23.0;
                        // Up to 150 degrees by 60%, then down past the start.
                        let angle = if t < 0.6 { t / 0.6 * 2.6 } else { 2.6 - (t - 0.6) / 0.4 * 3.0 };
                        let shoulder = arm.map(|arm| apply(&standing_globals[arm], [0.0; 3])).unwrap_or([0.0; 3]);
                        let swing = mul(&translation(shoulder), &mul(&rotation([1.0, 0.0, 0.0], -angle.max(0.0)), &translation(shoulder.map(|v| -v))));
                        let world = |node: usize, m: &Mat| if in_arm.get(node).copied().unwrap_or(false) { mul(&swing, m) } else { *m };
                        frame(&average, Some(&world))
                    })
                    .collect()
            },
        };
        motions.push((ATTACK.to_vec(), attack));
        // The fall: backward over a second, about the feet.
        let dying: Vec<Vec<[f32; 16]>> = match read("death")?.as_ref().map(Clip::read).transpose()?.flatten() {
            Some(clip) => sample(&clip).iter().map(|locals| frame(locals, None)).collect(),
            None => (0..30)
                .map(|f| {
                    let t = (f as f32 / 29.0).powf(1.6);
                    let pivot = [hips_rest[0], low[1], hips_rest[2]];
                    let fall = mul(&translation(pivot), &mul(&rotation([1.0, 0.0, 0.0], -t * 1.5), &translation(pivot.map(|v| -v))));
                    let world = |_: usize, m: &Mat| mul(&fall, m);
                    frame(&average, Some(&world))
                })
                .collect(),
        };
        let lying = vec![dying.last().cloned().unwrap_or_default()];
        motions.push((DEATH.to_vec(), dying));
        motions.push((LIE.to_vec(), lying));

        // Bounds of the standing pose, in the client's space.
        let client_points: Vec<[f32; 3]> = standing_points.iter().map(|p| apply(&to_client, *p)).collect();
        let bounds = client_points.iter().fold(([f32::MAX; 3], [f32::MIN; 3]), |(lo, hi), p| {
            (std::array::from_fn(|i| lo[i].min(p[i])), std::array::from_fn(|i| hi[i].max(p[i])))
        });
        Ok(Self { def: def.clone(), vertices, indices, texture, motions, bounds })
    }

    pub fn folder(&self) -> String {
        format!("c3/monster/gravak-{}", self.def.look)
    }

    pub fn texture_path(&self) -> String {
        format!("c3/texture/gravak/monster-{}.dds", self.def.look)
    }

    pub fn texture_dds(&self) -> Vec<u8> {
        crate::dds::encode_dxt5(&self.texture)
    }

    /// `model.c3`: the mesh and its standing motion.
    pub fn model_bytes(&self) -> Vec<u8> {
        let mut file = header();
        let mut chunk = Vec::new();
        sized(&mut chunk, &format!("gravak-{}", self.def.look));
        chunk.extend(2u32.to_le_bytes());
        chunk.extend((self.vertices.len() as u32).to_le_bytes());
        chunk.extend(0u32.to_le_bytes());
        for (p, uv, bones, weights) in &self.vertices {
            for v in p.iter().chain(uv.iter()) {
                chunk.extend(v.to_le_bytes());
            }
            chunk.extend(0xFFFF_FFFFu32.to_le_bytes());
            for b in bones {
                chunk.extend(b.to_le_bytes());
            }
            for w in weights {
                chunk.extend(w.to_le_bytes());
            }
        }
        chunk.extend(((self.indices.len() / 3) as u32).to_le_bytes());
        chunk.extend(0u32.to_le_bytes());
        for i in &self.indices {
            chunk.extend(i.to_le_bytes());
        }
        sized(&mut chunk, &format!("monster-{}.dds", self.def.look));
        for v in self.bounds.0.iter().chain(self.bounds.1.iter()) {
            chunk.extend(v.to_le_bytes());
        }
        let identity: [f32; 16] = [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0];
        for v in identity {
            chunk.extend(v.to_le_bytes());
        }
        // One row of texture; no transparency, draw or texture keys.
        for v in [1u32, 0, 0, 0] {
            chunk.extend(v.to_le_bytes());
        }
        push_chunk(&mut file, b"PHY4", &chunk);
        push_chunk(&mut file, b"MOTI", &motion_chunk(&self.motions[0].1));
        file
    }

    /// A motion's own `.c3`.
    pub fn motion_bytes(frames: &[Vec<[f32; 16]>]) -> Vec<u8> {
        let mut file = header();
        push_chunk(&mut file, b"MOTI", &motion_chunk(frames));
        file
    }
}

/// A rotation matrix's quaternion `[x, y, z, w]`.
fn matrix_quaternion(m: &Mat) -> [f32; 4] {
    let trace = m[0][0] + m[1][1] + m[2][2];
    let q = if trace > 0.0 {
        let s = (trace + 1.0).sqrt() * 2.0;
        [(m[2][1] - m[1][2]) / s, (m[0][2] - m[2][0]) / s, (m[1][0] - m[0][1]) / s, 0.25 * s]
    } else if m[0][0] > m[1][1] && m[0][0] > m[2][2] {
        let s = (1.0 + m[0][0] - m[1][1] - m[2][2]).sqrt() * 2.0;
        [0.25 * s, (m[0][1] + m[1][0]) / s, (m[0][2] + m[2][0]) / s, (m[2][1] - m[1][2]) / s]
    } else if m[1][1] > m[2][2] {
        let s = (1.0 + m[1][1] - m[0][0] - m[2][2]).sqrt() * 2.0;
        [(m[0][1] + m[1][0]) / s, 0.25 * s, (m[1][2] + m[2][1]) / s, (m[0][2] - m[2][0]) / s]
    } else {
        let s = (1.0 + m[2][2] - m[0][0] - m[1][1]).sqrt() * 2.0;
        [(m[0][2] + m[2][0]) / s, (m[1][2] + m[2][1]) / s, 0.25 * s, (m[1][0] - m[0][1]) / s]
    };
    let length = q.iter().map(|v| v * v).sum::<f32>().sqrt().max(1e-9);
    q.map(|v| v / length)
}

fn header() -> Vec<u8> {
    let mut bytes = b"MAXFILE C3 00001".to_vec();
    bytes.truncate(16);
    bytes
}

fn sized(out: &mut Vec<u8>, text: &str) {
    out.extend((text.len() as u32).to_le_bytes());
    out.extend(text.as_bytes());
}

fn push_chunk(file: &mut Vec<u8>, tag: &[u8; 4], chunk: &[u8]) {
    file.extend(tag);
    file.extend((chunk.len() as u32).to_le_bytes());
    file.extend(chunk);
}

/// A `MOTI` chunk: matrix keys, one a frame.
fn motion_chunk(frames: &[Vec<[f32; 16]>]) -> Vec<u8> {
    let bones = frames.first().map_or(0, Vec::len) as u32;
    let mut chunk = Vec::new();
    chunk.extend(bones.to_le_bytes());
    chunk.extend((frames.len() as u32).to_le_bytes());
    chunk.extend(b"KKEY");
    chunk.extend((frames.len() as u32).to_le_bytes());
    for (index, matrices) in frames.iter().enumerate() {
        chunk.extend((index as u32).to_le_bytes());
        for matrix in matrices {
            for v in matrix {
                chunk.extend(v.to_le_bytes());
            }
        }
    }
    chunk.extend(0u32.to_le_bytes());
    chunk
}

/// What the catalogs gain from the creatures.
#[derive(Debug, Default)]
pub struct Built {
    pub creatures: usize,
    pub models: String,
    pub armor: String,
    pub textures: String,
    pub motions: String,
}

pub fn build(content: &Path, defs: &[CreatureDef], output: &Path) -> Result<Built> {
    let mut built = Built::default();
    for def in defs {
        // The GLBs stay outside git: without them the creature is left out
        // (its monsters show nothing), and the rest of the build goes on.
        if !content.join(&def.folder).join("walk.glb").exists() {
            eprintln!("aviso: criatura {} sem {}/walk.glb; ficou de fora", def.name, def.folder);
            continue;
        }
        let creature = Creature::load(content, def).map_err(|error| format!("criatura {} ({}): {error}", def.name, def.folder))?;
        let id = def.look * 1_000_000;
        let folder = creature.folder();
        let write = |path: &str, bytes: &[u8]| -> Result<()> {
            let file = output.join(path);
            std::fs::create_dir_all(file.parent().expect("a folder"))?;
            std::fs::write(file, bytes)?;
            Ok(())
        };
        let model = format!("{folder}/model.c3");
        write(&model, &creature.model_bytes())?;
        write(&creature.texture_path(), &creature.texture_dds())?;
        built.models.push_str(&format!("{id}={model}\r\n"));
        built.textures.push_str(&format!("{id}={}\r\n", creature.texture_path()));
        built.armor.push_str(&format!("[{id}]\r\nPart=1\r\nMesh0={id}\r\nTexture0={id}\r\nMixTex0=0\r\n\r\n"));
        for (actions, frames) in &creature.motions {
            let path = format!("{folder}/{}.c3", actions[0]);
            write(&path, &Creature::motion_bytes(frames))?;
            for action in actions {
                built.motions.push_str(&format!("{:04}000{action:03}={path}\r\n", def.look));
            }
        }
        built.creatures += 1;
    }
    Ok(built)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sandbox_data::c3::C3Document;

    #[test]
    fn rotations_and_their_quaternions_agree() {
        let m = rotation([0.0, 1.0, 0.0], 0.7);
        let q = matrix_quaternion(&m);
        let back = quaternion(q);
        for r in 0..3 {
            for c in 0..3 {
                assert!((m[r][c] - back[r][c]).abs() < 1e-5);
            }
        }
    }

    #[test]
    fn creatures_read_back_as_the_client_reads_them() {
        let content = crate::build::default_content_dir();
        let def = CreatureDef { look: 981, folder: "art/monstros/sacerdote-de-sangue".into(), name: "teste".into(), height: 200.0 };
        if !content.join(&def.folder).join("walk.glb").exists() {
            return;
        }
        let creature = Creature::load(&content, &def).unwrap();
        let document = C3Document::parse(&creature.model_bytes()).unwrap();
        assert_eq!(document.meshes.len(), 1);
        assert_eq!(document.meshes[0].vertices.len(), creature.vertices.len());
        assert_eq!(document.motions[0].bone_count as usize, creature.motions[0].1[0].len());
        // Standing height near the one asked for (Z is down).
        let (low, high) = creature.bounds;
        assert!((high[2] - low[2] - 200.0).abs() < 2.0, "{low:?} {high:?}");
        assert!(high[2].abs() < 1.0, "the feet stand at zero: {low:?} {high:?}");
        for (_, frames) in &creature.motions {
            C3Document::parse(&Creature::motion_bytes(frames)).unwrap();
        }
    }
}
