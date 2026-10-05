//! `ini/itemtype.dat`: the item templates of the 5065 client.
//!
//! The file is a text table (a header line, then one item per line with
//! whitespace-separated columns; spaces inside names are written as `~`)
//! scrambled with a fixed key. Lines are kept as their original bytes and only
//! the items we change are written again, so a build reproduces every other
//! line exactly, names in their original encoding included.

use std::collections::HashMap;

/// The scrambling key, the same the client and the server use to read it.
fn key() -> [u8; 128] {
    let mut seed = 9527u32;
    let mut key = [0u8; 128];
    for value in &mut key {
        seed = seed.wrapping_mul(0x343fd).wrapping_add(0x269ec3);
        *value = ((seed >> 16) & 0x7fff) as u8;
    }
    key
}

/// Turns the installed file into its text.
pub fn decrypt(bytes: &mut [u8]) {
    let key = key();
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = (*byte ^ key[index % key.len()]).rotate_right((index % 8) as u32);
    }
}

/// The inverse of [`decrypt`]: the text as the client expects it on disk.
pub fn encrypt(bytes: &mut [u8]) {
    let key = key();
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = byte.rotate_left((index % 8) as u32) ^ key[index % key.len()];
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ColumnKind {
    Number,
    /// Free text; spaces are stored as `~`.
    Text,
}

#[derive(Clone, Copy, Debug)]
pub struct Column {
    /// The key in `content/items/*.toml`.
    pub key: &'static str,
    /// What the editor shows.
    pub label: &'static str,
    pub kind: ColumnKind,
}

const fn number(key: &'static str, label: &'static str) -> Column {
    Column {
        key,
        label,
        kind: ColumnKind::Number,
    }
}

const fn text(key: &'static str, label: &'static str) -> Column {
    Column {
        key,
        label,
        kind: ColumnKind::Text,
    }
}

/// The 39 columns of a 5065 item, in file order. The client and the server
/// read the same positions (`shop.rs` in both).
pub const COLUMNS: [Column; 39] = [
    number("id", "ID"),
    text("name", "Nome"),
    number("req_profession", "Profissão exigida"),
    number("req_weapon_skill", "Proficiência exigida"),
    number("req_level", "Nível exigido"),
    number("req_sex", "Sexo exigido"),
    number("req_strength", "Força exigida"),
    number("req_agility", "Agilidade exigida"),
    number("req_vitality", "Vitalidade exigida"),
    number("req_spirit", "Espírito exigido"),
    number("monopoly", "Monopoly (flags de troca/drop)"),
    number("weight", "Peso"),
    number("price", "Preço (silver)"),
    number("action", "Action ID"),
    number("attack_max", "Ataque máximo"),
    number("attack_min", "Ataque mínimo"),
    number("defense", "Defesa"),
    number("dexterity", "Destreza"),
    number("dodge", "Esquiva"),
    number("health_add", "HP extra"),
    number("mana_add", "MP extra"),
    number("durability", "Durabilidade / quantidade"),
    number("max_durability", "Durabilidade máxima"),
    number("ident", "Ident"),
    number("gem1", "Gema 1"),
    number("gem2", "Gema 2"),
    number("magic1", "Magic 1"),
    number("magic2", "Magic 2"),
    number("magic3", "Magic 3"),
    number("magic_attack", "Ataque mágico"),
    number("magic_defense", "Defesa mágica"),
    number("attack_range", "Alcance"),
    number("attack_speed", "Velocidade de ataque (ms)"),
    number("fray_mode", "Fray mode"),
    number("repair_mode", "Repair mode"),
    number("type_mask", "Type mask"),
    number("cps_price", "Preço (CPs)"),
    text("type_name", "Tipo (texto)"),
    text("description", "Descrição"),
];

pub fn column_index(key: &str) -> Option<usize> {
    COLUMNS.iter().position(|column| column.key == key)
}

/// One item: its columns as the raw bytes of the file.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ItemType {
    fields: Vec<Vec<u8>>,
}

impl ItemType {
    /// An item with every column empty (0, or `~` for text) but its id.
    pub fn blank(id: u32) -> Self {
        let mut item = Self {
            fields: COLUMNS
                .iter()
                .map(|column| match column.kind {
                    ColumnKind::Number => b"0".to_vec(),
                    ColumnKind::Text => b"~".to_vec(),
                })
                .collect(),
        };
        item.set_id(id);
        item
    }

    fn parse(line: &[u8]) -> Option<Self> {
        let fields = line
            .split(u8::is_ascii_whitespace)
            .filter(|field| !field.is_empty())
            .map(<[u8]>::to_vec)
            .collect::<Vec<_>>();
        // The server takes items from 13 columns on; anything shorter is not
        // an item line.
        if fields.len() < 13 {
            return None;
        }
        std::str::from_utf8(&fields[0]).ok()?.parse::<u32>().ok()?;
        Some(Self { fields })
    }

    pub fn id(&self) -> u32 {
        self.number(0) as u32
    }

    pub fn set_id(&mut self, id: u32) {
        self.set_raw(0, id.to_string().into_bytes());
    }

    pub fn len(&self) -> usize {
        self.fields.len()
    }

    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }

    pub fn raw(&self, index: usize) -> &[u8] {
        self.fields.get(index).map_or(&[], Vec::as_slice)
    }

    /// The column as text, `~` shown as spaces.
    pub fn text(&self, index: usize) -> String {
        String::from_utf8_lossy(self.raw(index)).replace('~', " ")
    }

    pub fn number(&self, index: usize) -> i64 {
        std::str::from_utf8(self.raw(index))
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or_default()
    }

    /// The column as an editor value: a number, or the text.
    pub fn value(&self, index: usize) -> String {
        match COLUMNS.get(index).map(|column| column.kind) {
            Some(ColumnKind::Number) => self.number(index).to_string(),
            _ => self.text(index),
        }
    }

    pub fn name(&self) -> String {
        self.text(1)
    }

    pub fn set_raw(&mut self, index: usize, value: Vec<u8>) {
        if self.fields.len() <= index {
            let missing = self.fields.len()..=index;
            self.fields.extend(
                missing.map(|column| match COLUMNS.get(column).map(|c| c.kind) {
                    Some(ColumnKind::Text) => b"~".to_vec(),
                    _ => b"0".to_vec(),
                }),
            );
        }
        self.fields[index] = value;
    }

    pub fn set_number(&mut self, index: usize, value: i64) {
        self.set_raw(index, value.to_string().into_bytes());
    }

    /// Spaces become `~`; an empty text keeps a `~` so the columns after it
    /// stay in place.
    pub fn set_text(&mut self, index: usize, value: &str) {
        let value = value
            .chars()
            .map(|c| if c.is_whitespace() { '~' } else { c })
            .collect::<String>();
        let value = if value.is_empty() {
            "~".to_owned()
        } else {
            value
        };
        self.set_raw(index, value.into_bytes());
    }

    fn to_line(&self) -> Vec<u8> {
        self.fields.join(&b' ')
    }
}

#[derive(Clone, Debug)]
struct Line {
    raw: Vec<u8>,
    item: Option<ItemType>,
}

/// The decrypted file.
#[derive(Clone, Debug)]
pub struct ItemTypeFile {
    header: Vec<u8>,
    lines: Vec<Line>,
    newline: &'static [u8],
    trailing_newline: bool,
    /// Item id to line; the last line wins, as in the client and the server.
    index: HashMap<u32, usize>,
}

impl ItemTypeFile {
    /// Reads the decrypted text.
    pub fn parse(text: &[u8]) -> Self {
        let newline: &'static [u8] = if text.windows(2).any(|pair| pair == b"\r\n") {
            b"\r\n"
        } else {
            b"\n"
        };
        let trailing_newline = text.ends_with(b"\n");
        let body = if trailing_newline {
            &text[..text.len() - 1]
        } else {
            text
        };
        let mut raw_lines = body
            .split(|byte| *byte == b'\n')
            .map(|line| line.strip_suffix(b"\r").unwrap_or(line).to_vec());
        let header = raw_lines.next().unwrap_or_default();
        let mut file = Self {
            header,
            lines: Vec::new(),
            newline,
            trailing_newline,
            index: HashMap::new(),
        };
        for raw in raw_lines {
            let item = ItemType::parse(&raw);
            if let Some(item) = &item {
                file.index.insert(item.id(), file.lines.len());
            }
            file.lines.push(Line { raw, item });
        }
        file
    }

    /// Reads the scrambled file as installed.
    pub fn from_encrypted(bytes: &[u8]) -> Self {
        let mut bytes = bytes.to_vec();
        decrypt(&mut bytes);
        Self::parse(&bytes)
    }

    pub fn header(&self) -> &[u8] {
        &self.header
    }

    pub fn get(&self, id: u32) -> Option<&ItemType> {
        self.index
            .get(&id)
            .and_then(|&line| self.lines[line].item.as_ref())
    }

    pub fn contains(&self, id: u32) -> bool {
        self.index.contains_key(&id)
    }

    /// Every item, in file order (an id repeated in the file only once, its
    /// last line).
    pub fn items(&self) -> impl Iterator<Item = &ItemType> {
        self.lines.iter().enumerate().filter_map(|(line, entry)| {
            let item = entry.item.as_ref()?;
            (self.index.get(&item.id()) == Some(&line)).then_some(item)
        })
    }

    pub fn item_count(&self) -> usize {
        self.index.len()
    }

    /// Replaces the item with the same id, or adds it at the end.
    pub fn upsert(&mut self, item: ItemType) {
        let raw = item.to_line();
        match self.index.get(&item.id()) {
            Some(&line) => {
                self.lines[line] = Line {
                    raw,
                    item: Some(item),
                }
            }
            None => {
                self.index.insert(item.id(), self.lines.len());
                self.lines.push(Line {
                    raw,
                    item: Some(item),
                });
            }
        }
    }

    /// The decrypted text. A header that holds the item count (`N` or
    /// `Key=N`) follows the new count.
    pub fn to_text(&self, original_count: usize) -> Vec<u8> {
        let mut out = header_with_count(&self.header, original_count, self.item_lines());
        for line in &self.lines {
            out.extend_from_slice(self.newline);
            out.extend_from_slice(&line.raw);
        }
        if self.trailing_newline {
            out.extend_from_slice(self.newline);
        }
        out
    }

    /// The file as the client installs it.
    pub fn to_encrypted(&self, original_count: usize) -> Vec<u8> {
        let mut bytes = self.to_text(original_count);
        encrypt(&mut bytes);
        bytes
    }

    /// How many lines hold items (the count a header would carry).
    pub fn item_lines(&self) -> usize {
        self.lines.iter().filter(|line| line.item.is_some()).count()
    }

    /// The first free id from `base` on with the same last digit (the
    /// quality) inside the same family (`base / 1000`).
    pub fn suggest_id(&self, base: u32, taken: impl Fn(u32) -> bool) -> Option<u32> {
        let family_end = (base / 1000 + 1) * 1000;
        (base..family_end)
            .step_by(10)
            .find(|id| !self.contains(*id) && !taken(*id))
    }
}

fn header_with_count(header: &[u8], original_count: usize, count: usize) -> Vec<u8> {
    let Ok(text) = std::str::from_utf8(header) else {
        return header.to_vec();
    };
    let (prefix, number) = match text.rsplit_once('=') {
        Some((prefix, number)) => (format!("{prefix}="), number),
        None => (String::new(), text),
    };
    let trimmed = number.trim();
    if trimmed.parse::<usize>().ok() == Some(original_count) && original_count != count {
        let end = number.trim_end();
        let suffix = &number[end.len()..];
        let start = &number[..number.len() - number.trim_start().len()];
        return format!("{prefix}{start}{count}{suffix}").into_bytes();
    }
    header.to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(id: u32, name: &str) -> String {
        let mut fields = vec!["0".to_owned(); 39];
        fields[0] = id.to_string();
        fields[1] = name.to_owned();
        fields[12] = "100".to_owned();
        fields[37] = "Pill".to_owned();
        fields[38] = "A~pill.".to_owned();
        fields.join(" ")
    }

    fn sample() -> Vec<u8> {
        format!(
            "2\r\n{}\r\n{}\r\n",
            line(1000000, "Stancher"),
            line(1000010, "Medicine")
        )
        .into_bytes()
    }

    #[test]
    fn encrypt_undoes_decrypt() {
        let plain = (0..=255u8).cycle().take(1000).collect::<Vec<_>>();
        let mut bytes = plain.clone();
        encrypt(&mut bytes);
        assert_ne!(bytes, plain);
        decrypt(&mut bytes);
        assert_eq!(bytes, plain);
    }

    #[test]
    fn untouched_files_come_back_byte_for_byte() {
        let text = sample();
        let file = ItemTypeFile::parse(&text);
        assert_eq!(file.item_count(), 2);
        assert_eq!(file.to_text(2), text);
        let mut scrambled = text.clone();
        encrypt(&mut scrambled);
        let file = ItemTypeFile::from_encrypted(&scrambled);
        assert_eq!(file.to_encrypted(2), scrambled);
    }

    #[test]
    fn odd_lines_and_spacing_survive() {
        let text = b"header\n; comment\n1000000   Name\t0 0 0 0 0 0 0 0 0 0 5 \xb0\xa1".to_vec();
        let file = ItemTypeFile::parse(&text);
        assert_eq!(file.item_count(), 1);
        assert_eq!(file.to_text(1), text);
    }

    #[test]
    fn new_items_go_to_the_end_and_the_count_follows() {
        let mut file = ItemTypeFile::parse(&sample());
        let mut item = file.get(1000000).unwrap().clone();
        item.set_id(1000020);
        item.set_text(1, "Gravak Pill");
        item.set_number(12, 999);
        file.upsert(item);
        let text = String::from_utf8(file.to_text(2)).unwrap();
        assert!(text.starts_with("3\r\n"));
        let last = text.lines().last().unwrap();
        assert!(last.starts_with("1000020 Gravak~Pill "));
        let again = ItemTypeFile::parse(text.as_bytes());
        assert_eq!(again.get(1000020).unwrap().number(12), 999);
        assert_eq!(again.get(1000020).unwrap().name(), "Gravak Pill");
    }

    #[test]
    fn replacing_keeps_the_place() {
        let mut file = ItemTypeFile::parse(&sample());
        let mut item = file.get(1000000).unwrap().clone();
        item.set_number(12, 5);
        file.upsert(item);
        let text = String::from_utf8(file.to_text(2)).unwrap();
        let lines = text.lines().collect::<Vec<_>>();
        assert_eq!(lines[0], "2");
        assert!(lines[1].starts_with("1000000 Stancher"));
        assert_eq!(
            ItemTypeFile::parse(text.as_bytes())
                .get(1000000)
                .unwrap()
                .number(12),
            5
        );
    }

    #[test]
    fn headers_with_keys_follow_the_count() {
        assert_eq!(header_with_count(b"Amount=2", 2, 3), b"Amount=3");
        assert_eq!(header_with_count(b"ItemType", 2, 3), b"ItemType");
        assert_eq!(header_with_count(b"7", 2, 3), b"7");
    }

    #[test]
    fn empty_text_keeps_the_columns() {
        let mut item = ItemType::blank(721000);
        item.set_text(38, "");
        assert_eq!(item.len(), 39);
        assert_eq!(ItemType::parse(&item.to_line()).unwrap().len(), 39);
    }

    #[test]
    fn suggestions_stay_in_the_family_and_quality() {
        let file = ItemTypeFile::parse(&sample());
        assert_eq!(file.suggest_id(1000000, |_| false), Some(1000020));
        assert_eq!(file.suggest_id(1000000, |id| id == 1000020), Some(1000030));
        assert_eq!(file.suggest_id(410_003, |_| false), Some(410_003));
    }
}
