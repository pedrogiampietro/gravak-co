//! Our items: `content/items/*.toml`, merged over `itemtype.dat`.
//!
//! ```toml
//! [[item]]
//! id = 721000            # a new id, or an existing one to change it
//! base = 723017          # optional: start from every column of that item
//! icon = "icons/721000.png"
//! name = "Gravak Statue"
//! price = 5000
//! description = "Uma estátua de Gravak."
//! ```
//!
//! Every other key is a column of [`crate::itemtype::COLUMNS`]; columns left
//! out keep the value of the item itself, of `base`, or 0.

use crate::Result;
use crate::itemtype::{COLUMNS, ColumnKind, ItemType, ItemTypeFile, column_index};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use toml::{Table, Value};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ItemDef {
    pub id: u32,
    pub base: Option<u32>,
    /// Relative to the content directory.
    pub icon: Option<String>,
    /// Column index and value, in column order.
    pub values: Vec<(usize, ColumnValue)>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ColumnValue {
    Number(i64),
    Text(String),
}

impl ColumnValue {
    pub fn display(&self) -> String {
        match self {
            Self::Number(value) => value.to_string(),
            Self::Text(value) => value.clone(),
        }
    }
}

impl ItemDef {
    pub fn value(&self, column: usize) -> Option<&ColumnValue> {
        self.values
            .iter()
            .find(|(index, _)| *index == column)
            .map(|(_, value)| value)
    }

    pub fn set(&mut self, column: usize, value: Option<ColumnValue>) {
        self.values.retain(|(index, _)| *index != column);
        if let Some(value) = value {
            self.values.push((column, value));
            self.values.sort_by_key(|(index, _)| *index);
        }
    }

    fn from_table(table: &Table) -> std::result::Result<Self, String> {
        let mut def = Self::default();
        for (key, value) in table {
            match key.as_str() {
                "id" => {
                    def.id = integer(key, value)?
                        .try_into()
                        .map_err(|_| "id fora do intervalo".to_owned())?
                }
                "base" => {
                    def.base = Some(
                        integer(key, value)?
                            .try_into()
                            .map_err(|_| "base fora do intervalo".to_owned())?,
                    )
                }
                "icon" => def.icon = Some(value.as_str().ok_or("icon deve ser texto")?.to_owned()),
                _ => {
                    let column =
                        column_index(key).ok_or_else(|| format!("campo desconhecido `{key}`"))?;
                    let value = match COLUMNS[column].kind {
                        ColumnKind::Number => ColumnValue::Number(integer(key, value)?),
                        ColumnKind::Text => ColumnValue::Text(
                            value
                                .as_str()
                                .ok_or_else(|| format!("`{key}` deve ser texto"))?
                                .to_owned(),
                        ),
                    };
                    def.set(column, Some(value));
                }
            }
        }
        if def.id == 0 {
            return Err("item sem `id`".to_owned());
        }
        Ok(def)
    }

    fn to_table(&self) -> Table {
        let mut table = Table::new();
        table.insert("id".into(), Value::Integer(self.id.into()));
        if let Some(base) = self.base {
            table.insert("base".into(), Value::Integer(base.into()));
        }
        if let Some(icon) = &self.icon {
            table.insert("icon".into(), Value::String(icon.clone()));
        }
        for (column, value) in &self.values {
            let value = match value {
                ColumnValue::Number(value) => Value::Integer(*value),
                ColumnValue::Text(value) => Value::String(value.clone()),
            };
            table.insert(COLUMNS[*column].key.into(), value);
        }
        table
    }
}

fn integer(key: &str, value: &Value) -> std::result::Result<i64, String> {
    value
        .as_integer()
        .ok_or_else(|| format!("`{key}` deve ser um número inteiro"))
}

/// Reads the `[[item]]` entries of one file.
pub fn parse_file(text: &str) -> std::result::Result<Vec<ItemDef>, String> {
    let table = text.parse::<Table>().map_err(|error| error.to_string())?;
    for key in table.keys() {
        if key != "item" {
            return Err(format!("seção desconhecida `{key}` (use [[item]])"));
        }
    }
    let Some(items) = table.get("item") else {
        return Ok(Vec::new());
    };
    let items = items.as_array().ok_or("`item` deve ser [[item]]")?;
    items
        .iter()
        .enumerate()
        .map(|(position, item)| {
            let table = item.as_table().ok_or("`item` deve ser [[item]]")?;
            ItemDef::from_table(table).map_err(|error| format!("item {}: {error}", position + 1))
        })
        .collect()
}

pub fn write_file(defs: &[ItemDef]) -> String {
    let mut out = String::new();
    for (position, def) in defs.iter().enumerate() {
        if position > 0 {
            out.push('\n');
        }
        let mut root = Table::new();
        root.insert(
            "item".into(),
            Value::Array(vec![Value::Table(def.to_table())]),
        );
        out.push_str(&toml::to_string(&root).expect("item tables serialize"));
    }
    out
}

/// Every item file of a content directory.
#[derive(Clone, Debug, Default)]
pub struct ItemSet {
    pub files: Vec<(PathBuf, Vec<ItemDef>)>,
}

impl ItemSet {
    /// `<content>/items/*.toml`, sorted by name. An id defined twice is an
    /// error.
    pub fn load(content: &Path) -> Result<Self> {
        let directory = content.join("items");
        let mut paths = match std::fs::read_dir(&directory) {
            Ok(entries) => entries
                .filter_map(|entry| entry.ok().map(|entry| entry.path()))
                .filter(|path| {
                    path.extension()
                        .is_some_and(|ext| ext.eq_ignore_ascii_case("toml"))
                })
                .collect::<Vec<_>>(),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(error) => return Err(error.into()),
        };
        paths.sort();
        let mut set = Self::default();
        let mut seen = HashMap::new();
        for path in paths {
            let text = std::fs::read_to_string(&path)?;
            let defs = parse_file(&text).map_err(|error| format!("{}: {error}", path.display()))?;
            for def in &defs {
                if let Some(other) = seen.insert(def.id, path.clone()) {
                    return Err(format!(
                        "item {} definido em {} e em {}",
                        def.id,
                        other.display(),
                        path.display()
                    )
                    .into());
                }
            }
            set.files.push((path, defs));
        }
        Ok(set)
    }

    pub fn defs(&self) -> impl Iterator<Item = &ItemDef> {
        self.files.iter().flat_map(|(_, defs)| defs)
    }

    pub fn find(&self, id: u32) -> Option<(&Path, &ItemDef)> {
        self.files.iter().find_map(|(path, defs)| {
            defs.iter()
                .find(|def| def.id == id)
                .map(|def| (path.as_path(), def))
        })
    }

    /// Adds or replaces the definition, saving its file: an existing one where
    /// it is, a new one in `<content>/items/<id>.toml`.
    pub fn save(&mut self, content: &Path, def: ItemDef) -> Result<PathBuf> {
        let position = self
            .files
            .iter()
            .position(|(_, defs)| defs.iter().any(|existing| existing.id == def.id));
        let index = match position {
            Some(index) => {
                let defs = &mut self.files[index].1;
                let slot = defs
                    .iter_mut()
                    .find(|existing| existing.id == def.id)
                    .expect("found above");
                *slot = def;
                index
            }
            None => {
                let path = content.join("items").join(format!("{}.toml", def.id));
                self.files.push((path, vec![def]));
                self.files.len() - 1
            }
        };
        let (path, defs) = &self.files[index];
        std::fs::create_dir_all(path.parent().expect("items directory"))?;
        std::fs::write(path, write_file(defs))?;
        Ok(path.clone())
    }

    /// Drops the definition; its file goes when it has nothing left.
    pub fn remove(&mut self, id: u32) -> Result<()> {
        let Some(index) = self
            .files
            .iter()
            .position(|(_, defs)| defs.iter().any(|def| def.id == id))
        else {
            return Ok(());
        };
        self.files[index].1.retain(|def| def.id != id);
        let (path, defs) = &self.files[index];
        if defs.is_empty() {
            std::fs::remove_file(path)?;
            self.files.remove(index);
        } else {
            std::fs::write(path, write_file(defs))?;
        }
        Ok(())
    }
}

/// The item a definition produces over `file`, which must already hold its
/// base.
pub fn resolve(def: &ItemDef, file: &ItemTypeFile) -> std::result::Result<ItemType, String> {
    let mut item = match (file.get(def.id), def.base) {
        // An existing item is changed in place unless a base is named.
        (Some(item), None) => item.clone(),
        (_, Some(base)) => file
            .get(base)
            .cloned()
            .ok_or_else(|| format!("item {}: base {base} não existe", def.id))?,
        (None, None) => ItemType::blank(def.id),
    };
    item.set_id(def.id);
    for (column, value) in &def.values {
        match value {
            ColumnValue::Number(value) => item.set_number(*column, *value),
            ColumnValue::Text(value) => item.set_text(*column, value),
        }
    }
    Ok(item)
}

/// Merges every definition into `file`. A definition may use another as its
/// base, whatever the order of the files.
pub fn apply(defs: &[&ItemDef], file: &mut ItemTypeFile) -> std::result::Result<(), String> {
    let mut pending = defs.to_vec();
    let mut done = HashSet::new();
    while !pending.is_empty() {
        let before = pending.len();
        let mut waiting = Vec::new();
        for def in pending {
            let base_ready = def.base.is_none_or(|base| {
                done.contains(&base)
                    || (file.contains(base) && !defs.iter().any(|other| other.id == base))
            });
            if base_ready {
                file.upsert(resolve(def, file)?);
                done.insert(def.id);
            } else {
                waiting.push(def);
            }
        }
        if waiting.len() == before {
            let def = waiting[0];
            return Err(format!(
                "item {}: base {} não existe (ou as bases formam um ciclo)",
                def.id,
                def.base.unwrap_or_default()
            ));
        }
        pending = waiting;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file() -> ItemTypeFile {
        let mut fields = vec!["0".to_owned(); 39];
        fields[0] = "723017".to_owned();
        fields[1] = "ExpBall".to_owned();
        fields[12] = "1000".to_owned();
        fields[37] = "Exp".to_owned();
        ItemTypeFile::parse(format!("1\n{}\n", fields.join(" ")).as_bytes())
    }

    #[test]
    fn files_round_trip() {
        let text = r#"
[[item]]
id = 721000
base = 723017
icon = "icons/721000.png"
name = "Gravak Statue"
price = 5000

[[item]]
id = 721010
base = 721000
name = "Gravak Statue II"
"#;
        let defs = parse_file(text).unwrap();
        assert_eq!(defs.len(), 2);
        assert_eq!(defs[0].value(12), Some(&ColumnValue::Number(5000)));
        assert_eq!(parse_file(&write_file(&defs)).unwrap(), defs);
    }

    #[test]
    fn unknown_keys_and_types_are_reported() {
        assert!(
            parse_file("[[item]]\nid = 1\nprise = 3\n")
                .unwrap_err()
                .contains("prise")
        );
        assert!(
            parse_file("[[item]]\nid = 1\nprice = \"3\"\n")
                .unwrap_err()
                .contains("price")
        );
        assert!(
            parse_file("[[item]]\nname = \"x\"\n")
                .unwrap_err()
                .contains("id")
        );
    }

    #[test]
    fn definitions_build_on_each_other_in_any_order() {
        let defs = parse_file(
            "[[item]]\nid = 721010\nbase = 721000\nname = \"Two\"\n\n[[item]]\nid = 721000\nbase = 723017\nname = \"One\"\nprice = 7\n",
        )
        .unwrap();
        let mut file = file();
        apply(&defs.iter().collect::<Vec<_>>(), &mut file).unwrap();
        let two = file.get(721010).unwrap();
        assert_eq!(two.name(), "Two");
        assert_eq!(two.number(12), 7);
        assert_eq!(two.text(37), "Exp");
        assert_eq!(file.get(723017).unwrap().number(12), 1000);
    }

    #[test]
    fn existing_items_change_in_place() {
        let defs = parse_file("[[item]]\nid = 723017\nprice = 1\n").unwrap();
        let mut file = file();
        apply(&defs.iter().collect::<Vec<_>>(), &mut file).unwrap();
        assert_eq!(file.get(723017).unwrap().number(12), 1);
        assert_eq!(file.get(723017).unwrap().name(), "ExpBall");
        assert_eq!(file.item_count(), 1);
    }

    #[test]
    fn missing_bases_and_cycles_fail() {
        let mut file = file();
        let defs = parse_file("[[item]]\nid = 1\nbase = 2\n").unwrap();
        assert!(apply(&defs.iter().collect::<Vec<_>>(), &mut file).is_err());
        let defs = parse_file("[[item]]\nid = 1\nbase = 2\n[[item]]\nid = 2\nbase = 1\n").unwrap();
        assert!(apply(&defs.iter().collect::<Vec<_>>(), &mut file).is_err());
    }

    #[test]
    fn the_set_saves_one_file_per_new_item() {
        let content =
            std::env::temp_dir().join(format!("gravak-content-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&content);
        let mut set = ItemSet::load(&content).unwrap();
        let mut def = ItemDef {
            id: 721000,
            base: Some(723017),
            ..Default::default()
        };
        def.set(1, Some(ColumnValue::Text("Statue".into())));
        let path = set.save(&content, def.clone()).unwrap();
        assert!(path.ends_with("items/721000.toml"));
        let loaded = ItemSet::load(&content).unwrap();
        assert_eq!(loaded.find(721000).unwrap().1, &def);
        set.remove(721000).unwrap();
        assert!(!path.exists());
        std::fs::remove_dir_all(&content).unwrap();
    }
}
