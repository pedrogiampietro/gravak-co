//! Models of ours for weapons: an OBJ (what Blender and other tools export)
//! put in an original weapon's C3 in place of its mesh, its bones and
//! motions kept, so it is held, posed and drawn (the bow's string pulled)
//! as that weapon.
//!
//! The OBJ is in the original's rest pose, the space `rest_obj` writes the
//! original in (`gravak-content weapon-obj <weapon> <file.obj>`): each of
//! its vertices follows the bone its group names (`g bone3`, as `rest_obj`
//! names them), else the bone of the original's nearest vertex there.

use crate::Result;
use sandbox_data::c3::{C3Document, Matrix4, Vector3};

/// An OBJ's triangles: each corner's place, texture coordinate (`vt`,
/// with v up as OBJ has it) and the bone its group names.
#[derive(Debug, Default, PartialEq)]
pub struct Obj {
    pub vertices: Vec<([f32; 3], [f32; 2], Option<u32>)>,
    pub triangles: Vec<[u16; 3]>,
}

impl Obj {
    /// Reads `v`, `vt` and `f` (polygons fanned into triangles); the rest
    /// (normals, groups, materials) is left.
    pub fn parse(text: &str) -> Result<Self> {
        let (mut places, mut uvs) = (Vec::new(), Vec::new());
        let mut obj = Obj::default();
        let mut corners = std::collections::HashMap::new();
        let mut bone = None;
        let number = |word: Option<&str>| -> Result<f32> { Ok(word.ok_or("OBJ: a number is missing")?.parse::<f32>()?) };
        for line in text.lines() {
            let mut words = line.split_whitespace();
            match words.next() {
                Some("v") => places.push([number(words.next())?, number(words.next())?, number(words.next())?]),
                Some("vt") => uvs.push([number(words.next())?, number(words.next())?]),
                Some("g" | "o") => bone = words.next().and_then(|name| name.strip_prefix("bone")?.parse::<u32>().ok()),
                Some("f") => {
                    let mut face = Vec::new();
                    for corner in words {
                        let mut parts = corner.split('/');
                        let place: usize = parts.next().ok_or("OBJ: empty corner")?.parse()?;
                        let uv: usize = parts.next().filter(|uv| !uv.is_empty()).ok_or("OBJ: a corner has no vt")?.parse()?;
                        let key = (place, uv, bone);
                        let index = match corners.get(&key) {
                            Some(index) => *index,
                            None => {
                                let place = *places.get(place.wrapping_sub(1)).ok_or("OBJ: a corner's v is missing")?;
                                let uv = *uvs.get(uv.wrapping_sub(1)).ok_or("OBJ: a corner's vt is missing")?;
                                let index = u16::try_from(obj.vertices.len()).map_err(|_| "OBJ: over 65535 vertices")?;
                                obj.vertices.push((place, uv, bone));
                                corners.insert(key, index);
                                index
                            }
                        };
                        face.push(index);
                    }
                    for k in 1..face.len().saturating_sub(1) {
                        obj.triangles.push([face[0], face[k], face[k + 1]]);
                    }
                }
                _ => {}
            }
        }
        if obj.triangles.is_empty() {
            return Err("OBJ: no faces".into());
        }
        Ok(obj)
    }
}

/// Where each bone puts its vertices in the rest pose: the mesh's initial
/// matrix times the bone's first frame.
fn rest_matrices(document: &C3Document) -> Result<Vec<Matrix4>> {
    let mesh = document.meshes.first().ok_or("C3: no mesh")?;
    let motion = document.motions.first().ok_or("C3: no motion")?;
    Ok((0..motion.bone_count).map(|bone| mesh.initial_matrix.multiply(motion.matrix_at(bone, 0.0))).collect())
}

/// The inverse of an affine matrix (rows are the axes, the last the
/// translation, as `Matrix4::transform_point` reads them).
fn inverse(matrix: Matrix4) -> Matrix4 {
    let m = |row: usize, column: usize| matrix.values[row * 4 + column];
    let det = m(0, 0) * (m(1, 1) * m(2, 2) - m(1, 2) * m(2, 1)) - m(0, 1) * (m(1, 0) * m(2, 2) - m(1, 2) * m(2, 0))
        + m(0, 2) * (m(1, 0) * m(2, 1) - m(1, 1) * m(2, 0));
    let d = if det.abs() < 1e-12 { 1.0 } else { 1.0 / det };
    let a = [
        [
            (m(1, 1) * m(2, 2) - m(1, 2) * m(2, 1)) * d,
            (m(0, 2) * m(2, 1) - m(0, 1) * m(2, 2)) * d,
            (m(0, 1) * m(1, 2) - m(0, 2) * m(1, 1)) * d,
        ],
        [
            (m(1, 2) * m(2, 0) - m(1, 0) * m(2, 2)) * d,
            (m(0, 0) * m(2, 2) - m(0, 2) * m(2, 0)) * d,
            (m(0, 2) * m(1, 0) - m(0, 0) * m(1, 2)) * d,
        ],
        [
            (m(1, 0) * m(2, 1) - m(1, 1) * m(2, 0)) * d,
            (m(0, 1) * m(2, 0) - m(0, 0) * m(2, 1)) * d,
            (m(0, 0) * m(1, 1) - m(0, 1) * m(1, 0)) * d,
        ],
    ];
    let t = [m(3, 0), m(3, 1), m(3, 2)];
    let mut values = [0.0; 16];
    for row in 0..3 {
        for column in 0..3 {
            values[row * 4 + column] = a[row][column];
        }
    }
    for column in 0..3 {
        values[12 + column] = -(0..3).map(|k| t[k] * a[k][column]).sum::<f32>();
    }
    values[15] = 1.0;
    Matrix4 { values }
}

/// The original's mesh in its rest pose, as an OBJ: what a model of ours
/// is made to stand in for (a group per bone).
pub fn rest_obj(original: &[u8]) -> Result<String> {
    let document = C3Document::parse(original)?;
    let rest = rest_matrices(&document)?;
    let mesh = &document.meshes[0];
    let mut obj = String::from("# The rest pose of an original weapon: make ours in this space.\n");
    for vertex in &mesh.vertices {
        let p = rest.get(vertex.bone_indices[0] as usize).copied().unwrap_or(Matrix4::IDENTITY).transform_point(vertex.position);
        obj += &format!("v {} {} {}\nvt {} {}\n", p.x, p.y, p.z, vertex.texture_coordinate.x, 1.0 - vertex.texture_coordinate.y);
    }
    for bone in 0..rest.len() {
        obj += &format!("g bone{bone}\n");
        for t in mesh.opaque_indices.chunks_exact(3).chain(mesh.alpha_indices.chunks_exact(3)) {
            if mesh.vertices[t[0] as usize].bone_indices[0] == bone as u32 {
                let [a, b, c] = [t[0] + 1, t[1] + 1, t[2] + 1];
                obj += &format!("f {a}/{a} {b}/{b} {c}/{c}\n");
            }
        }
    }
    Ok(obj)
}

/// `original` (a weapon's C3) with `obj` for its mesh.
pub fn replace_mesh(original: &[u8], obj: &Obj) -> Result<Vec<u8>> {
    let document = C3Document::parse(original)?;
    let rest = rest_matrices(&document)?;
    let mesh = &document.meshes[0];
    // The original's vertices where they stand, and their bones.
    let posed: Vec<([f32; 3], u32)> = mesh
        .vertices
        .iter()
        .map(|vertex| {
            let bone = vertex.bone_indices[0];
            let p = rest.get(bone as usize).copied().unwrap_or(Matrix4::IDENTITY).transform_point(vertex.position);
            ([p.x, p.y, p.z], bone)
        })
        .collect();

    // The first PHY chunk: its place, and where its vertices start and its
    // texture name begins (what follows the indices is kept as it is).
    let header = 16;
    let tag: [u8; 4] = original.get(header..header + 4).ok_or("C3: too short")?.try_into()?;
    if !tag.starts_with(b"PHY") {
        return Err("C3: the first chunk is not a mesh".into());
    }
    let length = u32::from_le_bytes(original[header + 4..header + 8].try_into()?) as usize;
    let chunk = &original[header + 8..header + 8 + length];
    let name_length = u32::from_le_bytes(chunk[0..4].try_into()?) as usize;
    let counts = 4 + name_length;
    let morphs = if &tag == b"PHY " { 4 } else { 1 };
    let extra = match &tag {
        b"PHY3" => 12,
        b"PHY5" => 20,
        _ => 0,
    };
    let vertex_size = morphs * 12 + 28 + extra;
    let vertex_count = (mesh.opaque_vertex_count + mesh.alpha_vertex_count) as usize;
    let triangles_at = counts + 12 + vertex_count * vertex_size;
    let triangle_count = (mesh.opaque_indices.len() + mesh.alpha_indices.len()) / 3;
    let rest_at = triangles_at + 8 + triangle_count * 6;

    let mut ours = Vec::with_capacity(chunk.len());
    ours.extend_from_slice(&chunk[..counts + 4]); // name, blend count
    ours.extend_from_slice(&(obj.vertices.len() as u32).to_le_bytes());
    ours.extend_from_slice(&0u32.to_le_bytes());
    for (place, uv, named) in &obj.vertices {
        let nearest = || {
            posed
                .iter()
                .min_by(|a, b| distance(a.0, *place).total_cmp(&distance(b.0, *place)))
                .map_or(0, |(_, bone)| *bone)
        };
        let bone = named.filter(|bone| (*bone as usize) < rest.len()).unwrap_or_else(nearest);
        let local = inverse(rest.get(bone as usize).copied().unwrap_or(Matrix4::IDENTITY))
            .transform_point(Vector3 { x: place[0], y: place[1], z: place[2] });
        for _ in 0..morphs {
            for c in [local.x, local.y, local.z] {
                ours.extend_from_slice(&c.to_le_bytes());
            }
        }
        ours.extend_from_slice(&uv[0].to_le_bytes());
        ours.extend_from_slice(&(1.0 - uv[1]).to_le_bytes());
        ours.extend_from_slice(&0u32.to_le_bytes());
        ours.extend_from_slice(&bone.to_le_bytes());
        ours.extend_from_slice(&0u32.to_le_bytes());
        ours.extend_from_slice(&1.0f32.to_le_bytes());
        ours.extend_from_slice(&0.0f32.to_le_bytes());
        ours.extend(std::iter::repeat_n(0u8, extra));
    }
    ours.extend_from_slice(&(obj.triangles.len() as u32).to_le_bytes());
    ours.extend_from_slice(&0u32.to_le_bytes());
    for triangle in &obj.triangles {
        for index in triangle {
            ours.extend_from_slice(&index.to_le_bytes());
        }
    }
    ours.extend_from_slice(&chunk[rest_at..]);

    let mut out = Vec::with_capacity(original.len() - length + ours.len());
    out.extend_from_slice(&original[..header + 4]);
    out.extend_from_slice(&(ours.len() as u32).to_le_bytes());
    out.extend_from_slice(&ours);
    out.extend_from_slice(&original[header + 8 + length..]);
    // What the client will read back.
    let check = C3Document::parse(&out)?;
    if check.meshes[0].vertices.len() != obj.vertices.len() || check.meshes[0].texture_name != mesh.texture_name {
        return Err("C3: the mesh of ours did not read back".into());
    }
    Ok(out)
}

fn distance(a: [f32; 3], b: [f32; 3]) -> f32 {
    (0..3).map(|k| (a[k] - b[k]).powi(2)).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_obj_reads_into_shared_corners_and_fanned_triangles() {
        let obj = Obj::parse("v 0 0 0\nv 1 0 0\nv 1 1 0\nv 0 1 0\nvt 0 0\nvt 1 0\nvt 1 1\nvt 0 1\nf 1/1 2/2 3/3 4/4\ng bone2\nf 1/1 2/2 3/3\n").unwrap();
        assert_eq!(obj.vertices.len(), 7);
        assert_eq!((obj.vertices[0].2, obj.vertices[6].2), (None, Some(2)));
        assert_eq!(obj.triangles, vec![[0, 1, 2], [0, 2, 3], [4, 5, 6]]);
        assert!(Obj::parse("v 0 0 0\n").is_err());
    }

    #[test]
    fn an_affine_matrix_inverts() {
        let m = Matrix4 { values: [0.0, 2.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 3.0, 0.0, 5.0, -7.0, 2.0, 1.0] };
        let p = Vector3 { x: 1.5, y: -2.0, z: 4.0 };
        let back = inverse(m).transform_point(m.transform_point(p));
        assert!((back.x - p.x).abs() < 1e-4 && (back.y - p.y).abs() < 1e-4 && (back.z - p.z).abs() < 1e-4);
    }

    /// With the installation: the winged bow made back from its own rest
    /// pose stands where it stood.
    #[test]
    fn the_rest_pose_goes_back_into_the_same_mesh() {
        let Ok(mut installation) = crate::assets::Installation::open(std::path::Path::new(r"R:\5065")) else { return };
        let Ok(original) = installation.read("c3/mesh/500270.c3") else { return };
        let obj = Obj::parse(&rest_obj(&original).unwrap()).unwrap();
        let ours = replace_mesh(&original, &obj).unwrap();
        let (before, after) = (C3Document::parse(&original).unwrap(), C3Document::parse(&ours).unwrap());
        let rest = rest_matrices(&after).unwrap();
        let place = |document: &C3Document, index: usize| {
            let v = &document.meshes[0].vertices[index];
            rest[v.bone_indices[0] as usize].transform_point(v.position)
        };
        // OBJ corners are shared by place and uv, so the counts may differ;
        // every vertex of ours stands on one of the original's.
        for index in 0..after.meshes[0].vertices.len() {
            let p = place(&after, index);
            assert!((0..before.meshes[0].vertices.len()).any(|k| {
                let q = place(&before, k);
                (p.x - q.x).abs() + (p.y - q.y).abs() + (p.z - q.z).abs() < 0.01
            }));
        }
        assert_eq!(after.motions.len(), before.motions.len());
    }
}
