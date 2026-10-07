// Source: @digitalm1nd on x.com / _dek on Discord — Preservation Conquer project — https://discord.gg/CvKPXEHYRY
mod mesh;
mod motion;

use crate::{ContentError, SliceReader};
pub use mesh::{C3Mesh, C3Vertex, DrawKey, TextureKey, TransparencyKey};
pub use motion::{C3Motion, MotionKey};

const FILE_HEADER_LENGTH: usize = 16;
const CHUNK_HEADER_LENGTH: usize = 8;

#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Vector2 {
    pub x: f32,
    pub y: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Vector3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Matrix4 {
    pub values: [f32; 16],
}

impl Default for Matrix4 {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Matrix4 {
    pub const IDENTITY: Self = Self {
        values: [
            1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
        ],
    };

    #[must_use]
    pub fn multiply(self, right: Self) -> Self {
        let mut values = [0.0; 16];
        for row in 0..4 {
            for column in 0..4 {
                values[row * 4 + column] = (0..4)
                    .map(|index| self.values[row * 4 + index] * right.values[index * 4 + column])
                    .sum();
            }
        }
        Self { values }
    }

    #[must_use]
    pub fn transform_point(self, position: Vector3) -> Vector3 {
        Vector3 {
            x: position.x * self.values[0]
                + position.y * self.values[4]
                + position.z * self.values[8]
                + self.values[12],
            y: position.x * self.values[1]
                + position.y * self.values[5]
                + position.z * self.values[9]
                + self.values[13],
            z: position.x * self.values[2]
                + position.y * self.values[6]
                + position.z * self.values[10]
                + self.values[14],
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct UnknownChunk {
    pub tag: [u8; 4],
    pub length: u32,
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct C3Document {
    pub version: String,
    pub meshes: Vec<C3Mesh>,
    pub motions: Vec<C3Motion>,
    /// Lines that sweep a ribbon behind them (`SHAP`, moved by the `SMOT`
    /// after each).
    pub shapes: Vec<C3Shape>,
    /// Clouds of sprites recorded frame by frame (`PTCL`).
    pub particles: Vec<C3Particles>,
    pub unknown_chunks: Vec<UnknownChunk>,
}

impl C3Document {
    pub fn parse(bytes: &[u8]) -> Result<Self, ContentError> {
        let mut reader = SliceReader::new("C3 document", bytes);
        let raw_header = reader.read_exact(FILE_HEADER_LENGTH)?;
        if !raw_header.starts_with(b"MAXFILE C3 ") {
            return Err(invalid("header does not start with MAXFILE C3"));
        }

        let mut document = Self {
            version: latin1(raw_header).trim_end_matches('\0').trim().to_owned(),
            ..Self::default()
        };

        while reader.remaining() >= CHUNK_HEADER_LENGTH {
            let tag: [u8; 4] = reader
                .read_exact(4)?
                .try_into()
                .expect("chunk tag length was checked");
            let chunk_length = reader.read_u32_le()?;
            let chunk_length_usize = usize::try_from(chunk_length)
                .map_err(|_| invalid(format!("chunk {} is too large", display_tag(tag))))?;
            let chunk = reader.read_exact(chunk_length_usize)?;

            match &tag {
                b"PHY " | b"PHY3" | b"PHY4" | b"PHY5" => {
                    document.meshes.push(C3Mesh::parse(tag, chunk)?)
                }
                b"MOTI" => document.motions.push(C3Motion::parse(chunk)?),
                b"SHAP" => document.shapes.push(C3Shape::parse(chunk)?),
                b"PTCL" => document.particles.push(C3Particles::parse(chunk)?),
                b"SMOT" => {
                    if let Some(shape) = document.shapes.last_mut() {
                        shape.frames = C3Shape::parse_motion(chunk)?;
                    }
                }
                _ => document.unknown_chunks.push(UnknownChunk {
                    tag,
                    length: chunk_length,
                }),
            }
        }

        if reader.remaining() != 0 {
            return Err(invalid(format!(
                "{} trailing bytes cannot form a chunk header",
                reader.remaining()
            )));
        }

        Ok(document)
    }
}

/// A `SHAP`: lines whose last `segments` places, frame after frame, are
/// joined into a ribbon (the original's sword-trail like sweeps).
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct C3Shape {
    pub name: String,
    pub lines: Vec<Vec<Vector3>>,
    pub texture: String,
    /// How many past places the ribbon spans.
    pub segments: u32,
    /// The shape's matrix at each frame (`SMOT`).
    pub frames: Vec<Matrix4>,
}

impl C3Shape {
    fn parse(bytes: &[u8]) -> Result<Self, ContentError> {
        let mut reader = SliceReader::new("C3 shape", bytes);
        let name = read_sized_string(&mut reader)?;
        let line_count = reader.read_u32_le()?;
        let line_count = checked_count(line_count, 4, reader.remaining())?;
        let mut lines = Vec::with_capacity(line_count);
        for _ in 0..line_count {
            let count = reader.read_u32_le()?;
            let count = checked_count(count, 12, reader.remaining())?;
            lines.push((0..count).map(|_| read_vector3(&mut reader)).collect::<Result<Vec<_>, _>>()?);
        }
        let texture = read_sized_string(&mut reader)?;
        let segments = if reader.remaining() >= 4 { reader.read_u32_le()? } else { 0 };
        Ok(Self {
            name,
            lines,
            texture,
            segments,
            frames: Vec::new(),
        })
    }

    fn parse_motion(bytes: &[u8]) -> Result<Vec<Matrix4>, ContentError> {
        let mut reader = SliceReader::new("C3 shape motion", bytes);
        let count = reader.read_u32_le()?;
        let count = checked_count(count, 64, reader.remaining())?;
        (0..count).map(|_| read_matrix(&mut reader)).collect()
    }

    /// Where a line's points are at `frame` (wrapping, between frames
    /// blended).
    pub fn line_at(&self, line: &[Vector3], frame: f32) -> Vec<Vector3> {
        let count = self.frames.len();
        if count == 0 {
            return line.to_vec();
        }
        let frame = frame.rem_euclid(count as f32);
        let first = frame.floor() as usize % count;
        let second = (first + 1) % count;
        let blend = frame.fract();
        line.iter()
            .map(|point| {
                let a = self.frames[first].transform_point(*point);
                let b = self.frames[second].transform_point(*point);
                Vector3 {
                    x: a.x + (b.x - a.x) * blend,
                    y: a.y + (b.y - a.y) * blend,
                    z: a.z + (b.z - a.z) * blend,
                }
            })
            .collect()
    }
}

/// A `PTCL`: a cloud of camera-facing sprites, baked frame by frame. Each
/// sprite has a place, an age (0 to 1 through its life, which picks its
/// cell of the `row` by `row` sheet) and a size in world units.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct C3Particles {
    pub name: String,
    pub texture: String,
    /// The texture is `row` by `row` cells, played left to right, top down.
    pub row: u32,
    pub frames: Vec<ParticleFrame>,
}

/// The sprites alive in one frame of a `PTCL`, placed by its matrix.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct ParticleFrame {
    pub positions: Vec<Vector3>,
    pub ages: Vec<f32>,
    pub sizes: Vec<f32>,
    pub matrix: Matrix4,
}

impl C3Particles {
    fn parse(bytes: &[u8]) -> Result<Self, ContentError> {
        let mut reader = SliceReader::new("C3 particles", bytes);
        let name = read_sized_string(&mut reader)?;
        let texture = read_sized_string(&mut reader)?;
        let row = reader.read_u32_le()?;
        let _most = reader.read_u32_le()?;
        let frame_count = reader.read_u32_le()?;
        // Every frame holds at least its count.
        let frame_count = checked_count(frame_count, 4, reader.remaining())?;
        let mut frames = Vec::with_capacity(frame_count);
        for _ in 0..frame_count {
            let count = reader.read_u32_le()?;
            // An empty frame has no matrix either.
            if count == 0 {
                frames.push(ParticleFrame::default());
                continue;
            }
            let count = checked_count(count, 20, reader.remaining())?;
            let positions = (0..count).map(|_| read_vector3(&mut reader)).collect::<Result<Vec<_>, _>>()?;
            let ages = (0..count).map(|_| reader.read_f32_le()).collect::<Result<Vec<_>, _>>()?;
            let sizes = (0..count).map(|_| reader.read_f32_le()).collect::<Result<Vec<_>, _>>()?;
            let matrix = read_matrix(&mut reader)?;
            frames.push(ParticleFrame {
                positions,
                ages,
                sizes,
                matrix,
            });
        }
        Ok(Self {
            name,
            texture,
            row: row.max(1),
            frames,
        })
    }
}

fn read_vector2(reader: &mut SliceReader<'_>) -> Result<Vector2, ContentError> {
    Ok(Vector2 {
        x: reader.read_f32_le()?,
        y: reader.read_f32_le()?,
    })
}

fn read_vector3(reader: &mut SliceReader<'_>) -> Result<Vector3, ContentError> {
    Ok(Vector3 {
        x: reader.read_f32_le()?,
        y: reader.read_f32_le()?,
        z: reader.read_f32_le()?,
    })
}

fn read_matrix(reader: &mut SliceReader<'_>) -> Result<Matrix4, ContentError> {
    let mut values = [0.0; 16];
    for value in &mut values {
        *value = reader.read_f32_le()?;
    }
    Ok(Matrix4 { values })
}

fn read_sized_string(reader: &mut SliceReader<'_>) -> Result<String, ContentError> {
    let length = usize::try_from(reader.read_u32_le()?)
        .map_err(|_| invalid("string length does not fit this platform"))?;
    if length > 4_096 {
        return Err(invalid(format!(
            "string length {length} exceeds 4096 bytes"
        )));
    }
    Ok(latin1(reader.read_exact(length)?))
}

fn latin1(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| char::from(*byte)).collect()
}

fn display_tag(tag: [u8; 4]) -> String {
    latin1(&tag)
}

fn checked_count(value: u32, element_size: usize, remaining: usize) -> Result<usize, ContentError> {
    let count = usize::try_from(value).map_err(|_| invalid("count does not fit this platform"))?;
    let required = count
        .checked_mul(element_size)
        .ok_or_else(|| invalid("element byte count overflowed"))?;
    if required > remaining {
        return Err(invalid(format!(
            "{count} elements require at least {required} bytes; only {remaining} remain"
        )));
    }
    Ok(count)
}

fn invalid(detail: impl Into<String>) -> ContentError {
    ContentError::InvalidData {
        context: "C3",
        detail: detail.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_particle_clouds() {
        // An empty frame, then one sprite: place, age, size, matrix.
        let mut chunk = Vec::new();
        for text in [&b"PCloud01"[..], &b"1.tga"[..]] {
            chunk.extend((text.len() as u32).to_le_bytes());
            chunk.extend(text);
        }
        for value in [4_u32, 1, 2, 0, 1] {
            chunk.extend(value.to_le_bytes());
        }
        for value in [1.0_f32, 2.0, 3.0, 0.5, 9.0] {
            chunk.extend(value.to_le_bytes());
        }
        for value in Matrix4::IDENTITY.values {
            chunk.extend(value.to_le_bytes());
        }
        let mut bytes = b"MAXFILE C3 00001".to_vec();
        bytes.extend(b"PTCL");
        bytes.extend((chunk.len() as u32).to_le_bytes());
        bytes.extend(chunk);
        let document = C3Document::parse(&bytes).expect("a PTCL parses");
        let cloud = &document.particles[0];
        assert_eq!((cloud.name.as_str(), cloud.row, cloud.frames.len()), ("PCloud01", 4, 2));
        assert!(cloud.frames[0].positions.is_empty());
        assert_eq!(cloud.frames[1].positions[0], Vector3 { x: 1.0, y: 2.0, z: 3.0 });
        assert_eq!((cloud.frames[1].ages[0], cloud.frames[1].sizes[0]), (0.5, 9.0));
    }

    #[test]
    fn rejects_non_c3_payloads() {
        let error = C3Document::parse(&[0; 16]).unwrap_err();
        assert!(error.to_string().contains("MAXFILE C3"));
    }

    #[test]
    fn accepts_an_empty_c3_container() {
        let mut bytes = [0_u8; 16];
        bytes[..11].copy_from_slice(b"MAXFILE C3 ");
        let document = C3Document::parse(&bytes).unwrap();
        assert!(document.meshes.is_empty());
        assert!(document.motions.is_empty());
    }
}
