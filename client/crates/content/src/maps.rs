//! Our objects on the maps: `content/maps/<map id>.toml`.
//!
//! ```toml
//! [[object]]
//! name = "Estátua de Gravak"       # only for the editor
//! image = "objects/gravak-statue.png"
//! x = 430                           # the cell its base stands on
//! y = 380
//! offset = [0, 0]                   # pixels, from the base at the cell
//! block = [[0, 0], [1, 0]]          # cells, relative to x/y, closed to walking
//! action = "daily_rewards"          # optional: what a click on it does
//! ```
//!
//! The image's bottom centre stands on the cell; `offset` moves it. The build
//! copies the images and writes `gravak/maps/<id>.txt`, which the client draws
//! and the client and the server use to close the blocked cells.

use crate::Result;
use sandbox_data::map::{GravakObject, GravakObjects};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MapObject {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    /// Relative to the content directory.
    pub image: String,
    pub x: i32,
    pub y: i32,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub offset: [i32; 2],
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub block: Vec<[i32; 2]>,
    /// What a click on it does in the game (`daily_rewards`: the Reward
    /// Shrine). One word.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
}

fn is_zero(offset: &[i32; 2]) -> bool {
    *offset == [0, 0]
}

impl MapObject {
    /// The cells it closes, on the map.
    pub fn blocked_cells(&self) -> impl Iterator<Item = [i32; 2]> + '_ {
        self.block.iter().map(|[dx, dy]| [self.x + dx, self.y + dy])
    }

    /// From the cell's point to the image's top-left corner, for an image of
    /// `size` pixels.
    pub fn draw_offset(&self, size: [u32; 2]) -> [i32; 2] {
        [
            -(size[0] as i32 / 2) + self.offset[0],
            -(size[1] as i32) + self.offset[1],
        ]
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MapObjects {
    #[serde(default, rename = "object", skip_serializing_if = "Vec::is_empty")]
    pub objects: Vec<MapObject>,
}

impl MapObjects {
    pub fn parse(text: &str) -> std::result::Result<Self, String> {
        toml::from_str(text).map_err(|error| error.to_string())
    }

    pub fn write(&self) -> String {
        toml::to_string(self).expect("map objects serialize")
    }
}

pub fn map_file(content: &Path, map_id: u32) -> PathBuf {
    content.join("maps").join(format!("{map_id}.toml"))
}

/// The objects of one map; none when it has no file.
pub fn load(content: &Path, map_id: u32) -> Result<MapObjects> {
    let path = map_file(content, map_id);
    match std::fs::read_to_string(&path) {
        Ok(text) => {
            Ok(MapObjects::parse(&text).map_err(|error| format!("{}: {error}", path.display()))?)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(MapObjects::default()),
        Err(error) => Err(error.into()),
    }
}

/// Saves the objects; an empty map loses its file.
pub fn save(content: &Path, map_id: u32, objects: &MapObjects) -> Result<()> {
    let path = map_file(content, map_id);
    if objects.objects.is_empty() {
        if path.exists() {
            std::fs::remove_file(path)?;
        }
        return Ok(());
    }
    std::fs::create_dir_all(path.parent().expect("maps folder"))?;
    std::fs::write(path, objects.write())?;
    Ok(())
}

/// Every map with objects: `<content>/maps/<id>.toml`.
pub fn all(content: &Path) -> Result<Vec<(u32, MapObjects)>> {
    let directory = content.join("maps");
    let entries = match std::fs::read_dir(&directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error.into()),
    };
    let mut maps = Vec::new();
    for entry in entries {
        let path = entry?.path();
        if !path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("toml"))
        {
            continue;
        }
        let id = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .and_then(|stem| stem.parse::<u32>().ok())
            .ok_or_else(|| {
                format!(
                    "{}: o nome deve ser o ID do mapa (ex.: 1002.toml)",
                    path.display()
                )
            })?;
        maps.push((id, load(content, id)?));
    }
    maps.sort_by_key(|(id, _)| *id);
    Ok(maps)
}

/// Where an object image goes in the build: `gravak/` and its path in
/// `content/`, with nothing that would break the one-line format.
pub fn built_image_path(image: &str) -> String {
    let clean = image
        .replace('\\', "/")
        .split('/')
        .filter(|part| !part.is_empty() && *part != "..")
        .collect::<Vec<_>>()
        .join("/")
        .chars()
        .map(|c| if c.is_whitespace() { '_' } else { c })
        .collect::<String>();
    format!("gravak/{clean}")
}

/// Writes the images and `gravak/maps/<id>.txt` of every map into `output`.
/// Returns how many maps and objects.
pub fn build(content: &Path, output: &Path) -> Result<(usize, usize)> {
    let maps = all(content)?;
    let mut objects_built = 0;
    for (map_id, objects) in &maps {
        let mut built = GravakObjects::default();
        for object in &objects.objects {
            let source = content.join(&object.image);
            let image = image::open(&source)
                .map_err(|error| format!("mapa {map_id}: imagem {}: {error}", source.display()))?;
            let target_name = built_image_path(&object.image);
            let target = output.join(&target_name).with_extension("png");
            std::fs::create_dir_all(target.parent().expect("objects folder"))?;
            image.to_rgba8().save(&target)?;
            let target_name = Path::new(&target_name)
                .with_extension("png")
                .to_string_lossy()
                .replace('\\', "/");
            built.objects.push(GravakObject {
                x: object.x,
                y: object.y,
                offset: object.draw_offset([image.width(), image.height()]),
                image: target_name,
                action: object
                    .action
                    .as_deref()
                    .map(str::trim)
                    .filter(|action| !action.is_empty() && !action.contains(char::is_whitespace))
                    .map(str::to_owned),
            });
            built.blocked.extend(object.blocked_cells());
            objects_built += 1;
        }
        let path = output.join(sandbox_data::map::gravak_objects_path(*map_id));
        std::fs::create_dir_all(path.parent().expect("maps folder"))?;
        std::fs::write(path, built.write())?;
    }
    Ok((maps.len(), objects_built))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn files_round_trip() {
        let text = "[[object]]\nname = \"Estátua\"\nimage = \"objects/statue.png\"\nx = 430\ny = 380\nblock = [[0, 0], [1, 0]]\n";
        let objects = MapObjects::parse(text).unwrap();
        assert_eq!(objects.objects[0].offset, [0, 0]);
        assert_eq!(
            objects.objects[0].blocked_cells().collect::<Vec<_>>(),
            [[430, 380], [431, 380]]
        );
        assert_eq!(MapObjects::parse(&objects.write()).unwrap(), objects);
        assert!(
            MapObjects::parse("[[object]]\nimage = \"a.png\"\nx = 1\ny = 1\nblok = []\n").is_err()
        );
    }

    #[test]
    fn the_base_stands_on_the_cell() {
        let object = MapObject {
            offset: [3, -2],
            ..MapObject::default()
        };
        assert_eq!(object.draw_offset([100, 150]), [-47, -152]);
    }

    #[test]
    fn built_paths_stay_inside_and_on_one_line() {
        assert_eq!(
            built_image_path("objects/my statue.png"),
            "gravak/objects/my_statue.png"
        );
        assert_eq!(built_image_path("../../x.png"), "gravak/x.png");
    }
}
