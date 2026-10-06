//! The Gravak pack (`game/gravak.gpk`): every game file in one archive,
//! each one compressed (deflate) and encrypted with ChaCha20-Poly1305, so
//! players can neither read nor change them. A changed byte fails the
//! entry's tag and the client refuses it.
//!
//! Layout:
//!
//! ```text
//! header (64 bytes, plain; authenticated as the index's AAD)
//!   0  magic "GRVKPAK\0"     8  version u32     12 entry count u32
//!   16 index offset u64      24 index length u64 (ciphertext + tag)
//!   32 salt [16]             48 key check [16]
//! entries                    each ciphertext + tag
//! index (encrypted)          names, WDF ids and entry records
//! ```
//!
//! The file key is derived from the client's master key and the pack's
//! random salt. The master key is compiled into the client from
//! `GRAVAK_PACK_KEY` (64 hex digits) by `tools/package-client.ps1`; a
//! build without it uses a development key. Anyone with the executable
//! and enough patience can dig the key out, as with any game client: the
//! pack keeps casual players from opening and editing the files, and the
//! server stays the authority on everything that matters.
//!
//! Names are matched without case, as Windows does (`VirtualPath`
//! lowered to ASCII lowercase); the original 5065 archives' entries keep
//! their WDF id (`wdf::path_id`), and a name always wins over an id, as
//! loose files win over `data.wdf`/`c3.wdf` in the client.

use crate::{VirtualPath, crypto};
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::Path;

pub const FILE_NAME: &str = "gravak.gpk";

const MAGIC: &[u8; 8] = b"GRVKPAK\0";
const VERSION: u32 = 1;
const HEADER_LENGTH: usize = 64;
/// No single game file comes near this; it bounds what a damaged pack
/// can make the client allocate.
pub const MAX_ENTRY_LENGTH: u32 = 256 * 1024 * 1024;
const MAX_INDEX_LENGTH: u64 = 256 * 1024 * 1024;

const METHOD_STORED: u8 = 0;
const METHOD_DEFLATE: u8 = 1;

// ---------------------------------------------------------------- key --

const fn hex_digit(byte: u8) -> u8 {
    match byte {
        b'0'..=b'9' => byte - b'0',
        b'a'..=b'f' => byte - b'a' + 10,
        b'A'..=b'F' => byte - b'A' + 10,
        _ => panic!("GRAVAK_PACK_KEY must be 64 hexadecimal digits"),
    }
}

const fn parse_key(text: &str) -> [u8; 32] {
    let bytes = text.as_bytes();
    assert!(
        bytes.len() == 64,
        "GRAVAK_PACK_KEY must be 64 hexadecimal digits"
    );
    let mut key = [0_u8; 32];
    let mut i = 0;
    while i < 32 {
        key[i] = (hex_digit(bytes[i * 2]) << 4) | hex_digit(bytes[i * 2 + 1]);
        i += 1;
    }
    key
}

const DEVELOPMENT_KEY: &str = "6772766b2d6465762d6b65792d6e6f742d666f722d72656c65617365732d2d21";
const KEY_TEXT: Option<&str> = option_env!("GRAVAK_PACK_KEY");

/// The key is kept in the executable only XORed with this mask, so it
/// does not sit there as one recognisable run of bytes.
const MASK: [u8; 32] = [
    0x3b, 0xd1, 0x7e, 0x09, 0xa4, 0x5c, 0xe2, 0x91, 0x17, 0x6f, 0xc8, 0x2d, 0x84, 0xf3, 0x50, 0xbe,
    0x0a, 0x99, 0x63, 0xd7, 0x2e, 0xc5, 0x78, 0x14, 0xeb, 0x46, 0xaf, 0x31, 0x9d, 0x02, 0xf8, 0x6c,
];

const fn masked(key: [u8; 32]) -> [u8; 32] {
    let mut out = [0_u8; 32];
    let mut i = 0;
    while i < 32 {
        out[i] = key[i] ^ MASK[i];
        i += 1;
    }
    out
}

const MASKED_KEY: [u8; 32] = masked(parse_key(match KEY_TEXT {
    Some(text) => text,
    None => DEVELOPMENT_KEY,
}));

/// Whether this build carries the development key (no `GRAVAK_PACK_KEY`).
#[must_use]
pub const fn uses_development_key() -> bool {
    KEY_TEXT.is_none()
}

/// The master key compiled into this build.
#[must_use]
pub fn master_key() -> [u8; 32] {
    let masked = std::hint::black_box(MASKED_KEY);
    let mask = std::hint::black_box(MASK);
    let mut key = [0_u8; 32];
    for i in 0..32 {
        key[i] = masked[i] ^ mask[i];
    }
    key
}

fn file_key(master: &[u8; 32], salt: &[u8; 16]) -> [u8; 32] {
    let nonce: [u8; 12] = salt[..12].try_into().expect("12 bytes");
    let counter = u32::from_le_bytes(salt[12..].try_into().expect("4 bytes"));
    let block = crypto::chacha20_block(master, counter, &nonce);
    block[..32].try_into().expect("32 bytes")
}

fn key_check(key: &[u8; 32]) -> [u8; 16] {
    let block = crypto::chacha20_block(key, 0, &[2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    block[..16].try_into().expect("16 bytes")
}

fn entry_nonce(ordinal: u32) -> [u8; 12] {
    let mut nonce = [0_u8; 12];
    nonce[4..].copy_from_slice(&u64::from(ordinal).to_le_bytes());
    nonce
}

const INDEX_NONCE: [u8; 12] = [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];

fn invalid(detail: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, detail.into())
}

/// The name an entry is stored and looked up by.
pub fn normalize(name: &str) -> io::Result<String> {
    let path = VirtualPath::try_from(name).map_err(|error| invalid(error.to_string()))?;
    Ok(path.as_str().to_ascii_lowercase())
}

// ------------------------------------------------------------- writer --

#[derive(Clone, Copy, Debug)]
struct Record {
    offset: u64,
    stored: u32,
    plain: u32,
    method: u8,
}

/// An entry compressed and encrypted, ready for `PackWriter::push`.
pub struct SealedEntry {
    ordinal: u32,
    plain: u32,
    method: u8,
    bytes: Vec<u8>,
}

impl SealedEntry {
    #[must_use]
    pub fn stored_length(&self) -> usize {
        self.bytes.len()
    }
}

/// Seals entries; cheap to clone into worker threads.
#[derive(Clone)]
pub struct Sealer {
    key: [u8; 32],
}

impl Sealer {
    /// Compresses (deflate `level`, 0 to store) and encrypts entry
    /// `ordinal`, which must be the ordinal `PackWriter::push` gives it.
    pub fn seal(&self, ordinal: u32, plain: &[u8], level: u8) -> io::Result<SealedEntry> {
        let plain_length = u32::try_from(plain.len())
            .ok()
            .filter(|length| *length <= MAX_ENTRY_LENGTH)
            .ok_or_else(|| invalid(format!("entry of {} bytes is too large", plain.len())))?;
        let (method, mut bytes) = if level == 0 {
            (METHOD_STORED, plain.to_vec())
        } else {
            let packed = miniz_oxide::deflate::compress_to_vec(plain, level.min(10));
            if packed.len() < plain.len() {
                (METHOD_DEFLATE, packed)
            } else {
                (METHOD_STORED, plain.to_vec())
            }
        };
        crypto::seal(
            &self.key,
            &entry_nonce(ordinal),
            &entry_aad(plain_length, method),
            &mut bytes,
        );
        Ok(SealedEntry {
            ordinal,
            plain: plain_length,
            method,
            bytes,
        })
    }
}

fn entry_aad(plain: u32, method: u8) -> [u8; 5] {
    let mut aad = [0_u8; 5];
    aad[..4].copy_from_slice(&plain.to_le_bytes());
    aad[4] = method;
    aad
}

pub struct PackWriter {
    out: BufWriter<File>,
    key: [u8; 32],
    salt: [u8; 16],
    offset: u64,
    records: Vec<Record>,
    names: Vec<(String, u32)>,
    name_set: HashMap<String, u32>,
    ids: Vec<(u32, u32)>,
    id_set: HashMap<u32, u32>,
}

impl PackWriter {
    pub fn create(path: &Path, master: &[u8; 32]) -> io::Result<Self> {
        let mut out = BufWriter::with_capacity(4 * 1024 * 1024, File::create(path)?);
        out.write_all(&[0; HEADER_LENGTH])?;
        let salt = random_salt();
        Ok(Self {
            out,
            key: file_key(master, &salt),
            salt,
            offset: HEADER_LENGTH as u64,
            records: Vec::new(),
            names: Vec::new(),
            name_set: HashMap::new(),
            ids: Vec::new(),
            id_set: HashMap::new(),
        })
    }

    #[must_use]
    pub fn sealer(&self) -> Sealer {
        Sealer { key: self.key }
    }

    /// The ordinal the next pushed entry gets.
    #[must_use]
    pub fn next_ordinal(&self) -> u32 {
        self.records.len() as u32
    }

    /// Appends a sealed entry; entries must arrive in ordinal order.
    pub fn push(&mut self, entry: SealedEntry) -> io::Result<u32> {
        if entry.ordinal != self.next_ordinal() {
            return Err(invalid(format!(
                "entry {} pushed where {} was expected",
                entry.ordinal,
                self.next_ordinal()
            )));
        }
        self.out.write_all(&entry.bytes)?;
        self.records.push(Record {
            offset: self.offset,
            stored: entry.bytes.len() as u32,
            plain: entry.plain,
            method: entry.method,
        });
        self.offset += entry.bytes.len() as u64;
        Ok(entry.ordinal)
    }

    /// Whether `name` is still free; the first entry given a name keeps it.
    #[must_use]
    pub fn has_name(&self, name: &str) -> bool {
        normalize(name).is_ok_and(|name| self.name_set.contains_key(&name))
    }

    /// Names entry `ordinal`; `false` when the name was already taken.
    pub fn add_name(&mut self, name: &str, ordinal: u32) -> io::Result<bool> {
        let name = normalize(name)?;
        if name.len() > usize::from(u16::MAX) || ordinal >= self.next_ordinal() {
            return Err(invalid(format!("cannot name entry {ordinal} '{name}'")));
        }
        if self.name_set.contains_key(&name) {
            return Ok(false);
        }
        self.name_set.insert(name.clone(), ordinal);
        self.names.push((name, ordinal));
        Ok(true)
    }

    #[must_use]
    pub fn has_id(&self, id: u32) -> bool {
        self.id_set.contains_key(&id)
    }

    /// Gives entry `ordinal` a WDF id; `false` when the id was taken.
    pub fn add_id(&mut self, id: u32, ordinal: u32) -> io::Result<bool> {
        if ordinal >= self.next_ordinal() {
            return Err(invalid(format!("cannot give entry {ordinal} an id")));
        }
        if self.id_set.contains_key(&id) {
            return Ok(false);
        }
        self.id_set.insert(id, ordinal);
        self.ids.push((id, ordinal));
        Ok(true)
    }

    /// Writes the index and the header; returns the pack's size.
    pub fn finish(mut self) -> io::Result<u64> {
        let mut index = Vec::new();
        index.extend_from_slice(&(self.names.len() as u32).to_le_bytes());
        for (name, ordinal) in &self.names {
            index.extend_from_slice(&(name.len() as u16).to_le_bytes());
            index.extend_from_slice(name.as_bytes());
            index.extend_from_slice(&ordinal.to_le_bytes());
        }
        index.extend_from_slice(&(self.ids.len() as u32).to_le_bytes());
        for (id, ordinal) in &self.ids {
            index.extend_from_slice(&id.to_le_bytes());
            index.extend_from_slice(&ordinal.to_le_bytes());
        }
        index.extend_from_slice(&(self.records.len() as u32).to_le_bytes());
        for record in &self.records {
            index.extend_from_slice(&record.offset.to_le_bytes());
            index.extend_from_slice(&record.stored.to_le_bytes());
            index.extend_from_slice(&record.plain.to_le_bytes());
            index.push(record.method);
        }

        let header = Header {
            entries: self.records.len() as u32,
            index_offset: self.offset,
            index_length: (index.len() + crypto::TAG_LENGTH) as u64,
            salt: self.salt,
            check: key_check(&self.key),
        }
        .to_bytes();
        crypto::seal(&self.key, &INDEX_NONCE, &header, &mut index);
        self.out.write_all(&index)?;
        self.out.seek(SeekFrom::Start(0))?;
        self.out.write_all(&header)?;
        self.out.flush()?;
        Ok(self.offset + index.len() as u64)
    }
}

fn random_salt() -> [u8; 16] {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};
    // Unique per pack, not secret: the process's random hasher seeds and
    // the clock are plenty.
    let mut salt = [0_u8; 16];
    for (i, chunk) in salt.chunks_mut(8).enumerate() {
        let mut hasher = RandomState::new().build_hasher();
        hasher.write_usize(i);
        hasher.write_u128(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |time| time.as_nanos()),
        );
        hasher.write_u32(std::process::id());
        chunk.copy_from_slice(&hasher.finish().to_le_bytes());
    }
    salt
}

// ------------------------------------------------------------- header --

struct Header {
    entries: u32,
    index_offset: u64,
    index_length: u64,
    salt: [u8; 16],
    check: [u8; 16],
}

impl Header {
    fn to_bytes(&self) -> [u8; HEADER_LENGTH] {
        let mut bytes = [0_u8; HEADER_LENGTH];
        bytes[..8].copy_from_slice(MAGIC);
        bytes[8..12].copy_from_slice(&VERSION.to_le_bytes());
        bytes[12..16].copy_from_slice(&self.entries.to_le_bytes());
        bytes[16..24].copy_from_slice(&self.index_offset.to_le_bytes());
        bytes[24..32].copy_from_slice(&self.index_length.to_le_bytes());
        bytes[32..48].copy_from_slice(&self.salt);
        bytes[48..64].copy_from_slice(&self.check);
        bytes
    }

    fn parse(bytes: &[u8; HEADER_LENGTH]) -> io::Result<Self> {
        if &bytes[..8] != MAGIC {
            return Err(invalid("not a Gravak pack"));
        }
        let version = u32::from_le_bytes(bytes[8..12].try_into().expect("4 bytes"));
        if version != VERSION {
            return Err(invalid(format!(
                "pack version {version}; this client reads version {VERSION}"
            )));
        }
        Ok(Self {
            entries: u32::from_le_bytes(bytes[12..16].try_into().expect("4 bytes")),
            index_offset: u64::from_le_bytes(bytes[16..24].try_into().expect("8 bytes")),
            index_length: u64::from_le_bytes(bytes[24..32].try_into().expect("8 bytes")),
            salt: bytes[32..48].try_into().expect("16 bytes"),
            check: bytes[48..64].try_into().expect("16 bytes"),
        })
    }
}

// ------------------------------------------------------------- reader --

pub struct PackReader {
    file: File,
    key: [u8; 32],
    records: Vec<Record>,
    names: HashMap<String, u32>,
    ids: HashMap<u32, u32>,
}

struct IndexReader<'a> {
    bytes: &'a [u8],
}

impl IndexReader<'_> {
    fn take(&mut self, length: usize) -> io::Result<&[u8]> {
        if self.bytes.len() < length {
            return Err(invalid("pack index is truncated"));
        }
        let (head, tail) = self.bytes.split_at(length);
        self.bytes = tail;
        Ok(head)
    }
    fn u8(&mut self) -> io::Result<u8> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> io::Result<u16> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().expect("2 bytes")))
    }
    fn u32(&mut self) -> io::Result<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().expect("4 bytes")))
    }
    fn u64(&mut self) -> io::Result<u64> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().expect("8 bytes")))
    }
}

impl PackReader {
    pub fn open(path: &Path, master: &[u8; 32]) -> io::Result<Self> {
        let mut file = File::open(path)?;
        let length = file.metadata()?.len();
        let mut header_bytes = [0_u8; HEADER_LENGTH];
        file.read_exact(&mut header_bytes)
            .map_err(|_| invalid("pack is truncated"))?;
        let header = Header::parse(&header_bytes)?;
        let key = file_key(master, &header.salt);
        if key_check(&key) != header.check {
            return Err(invalid(
                "the pack was made for a different Gravak build; install the game again",
            ));
        }
        let end = header.index_offset.checked_add(header.index_length);
        if header.index_length > MAX_INDEX_LENGTH || end != Some(length) {
            return Err(invalid("pack index is damaged"));
        }
        file.seek(SeekFrom::Start(header.index_offset))?;
        let mut index = vec![0_u8; header.index_length as usize];
        file.read_exact(&mut index)?;
        if !crypto::open(&key, &INDEX_NONCE, &header_bytes, &mut index) {
            return Err(invalid("pack index is damaged or was altered"));
        }

        let mut reader = IndexReader { bytes: &index };
        let name_count = reader.u32()?;
        let mut names = HashMap::with_capacity(name_count.min(1 << 20) as usize);
        for _ in 0..name_count {
            let length = reader.u16()?;
            let name = std::str::from_utf8(reader.take(length.into())?)
                .map_err(|_| invalid("pack index holds a bad name"))?
                .to_owned();
            names.insert(name, reader.u32()?);
        }
        let id_count = reader.u32()?;
        let mut ids = HashMap::with_capacity(id_count.min(1 << 20) as usize);
        for _ in 0..id_count {
            let id = reader.u32()?;
            ids.insert(id, reader.u32()?);
        }
        let record_count = reader.u32()?;
        if record_count != header.entries {
            return Err(invalid("pack index is damaged"));
        }
        let mut records = Vec::with_capacity(record_count.min(1 << 20) as usize);
        for _ in 0..record_count {
            let record = Record {
                offset: reader.u64()?,
                stored: reader.u32()?,
                plain: reader.u32()?,
                method: reader.u8()?,
            };
            let inside = record
                .offset
                .checked_add(record.stored.into())
                .is_some_and(|end| record.offset >= HEADER_LENGTH as u64 && end <= header.index_offset);
            if !inside || record.plain > MAX_ENTRY_LENGTH || record.method > METHOD_DEFLATE {
                return Err(invalid("pack index is damaged"));
            }
            records.push(record);
        }
        if names
            .values()
            .chain(ids.values())
            .any(|ordinal| *ordinal >= record_count)
        {
            return Err(invalid("pack index is damaged"));
        }
        Ok(Self {
            file,
            key,
            records,
            names,
            ids,
        })
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.records.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// The entry stored under `name`, if any.
    #[must_use]
    pub fn find_name(&self, name: &str) -> Option<u32> {
        self.names.get(&normalize(name).ok()?).copied()
    }

    /// The entry stored under WDF id `id`, if any.
    #[must_use]
    pub fn find_id(&self, id: u32) -> Option<u32> {
        self.ids.get(&id).copied()
    }

    /// Every name in the pack (for the packer's verification).
    pub fn names(&self) -> impl Iterator<Item = (&str, u32)> {
        self.names.iter().map(|(name, ordinal)| (name.as_str(), *ordinal))
    }

    pub fn ids(&self) -> impl Iterator<Item = (u32, u32)> + '_ {
        self.ids.iter().map(|(id, ordinal)| (*id, *ordinal))
    }

    /// Reads, authenticates, decrypts and inflates entry `ordinal`.
    pub fn read(&mut self, ordinal: u32) -> io::Result<Vec<u8>> {
        let record = *self
            .records
            .get(ordinal as usize)
            .ok_or_else(|| invalid(format!("no pack entry {ordinal}")))?;
        self.file.seek(SeekFrom::Start(record.offset))?;
        let mut bytes = vec![0_u8; record.stored as usize];
        self.file.read_exact(&mut bytes)?;
        if !crypto::open(
            &self.key,
            &entry_nonce(ordinal),
            &entry_aad(record.plain, record.method),
            &mut bytes,
        ) {
            return Err(invalid(format!(
                "game file {ordinal} is damaged or was altered; install the game again"
            )));
        }
        let plain = if record.method == METHOD_DEFLATE {
            miniz_oxide::inflate::decompress_to_vec_with_limit(&bytes, record.plain as usize)
                .map_err(|_| invalid(format!("game file {ordinal} does not inflate")))?
        } else {
            bytes
        };
        if plain.len() != record.plain as usize {
            return Err(invalid(format!("game file {ordinal} has the wrong size")));
        }
        Ok(plain)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("gravak-pack-{}-{name}.gpk", std::process::id()))
    }

    fn sample(path: &Path, master: &[u8; 32]) {
        let mut writer = PackWriter::create(path, master).unwrap();
        let sealer = writer.sealer();
        let text = b"[Section]\r\nKey=Value\r\n".repeat(200);
        let entry = sealer.seal(writer.next_ordinal(), &text, 9).unwrap();
        assert!(entry.stored_length() < text.len());
        let first = writer.push(entry).unwrap();
        assert!(writer.add_name("ini\\Sound.ini", first).unwrap());
        let noise: Vec<u8> = (0..5000_u32).map(|i| (i.wrapping_mul(2_654_435_761) >> 13) as u8).collect();
        let entry = sealer.seal(writer.next_ordinal(), &noise, 9).unwrap();
        let second = writer.push(entry).unwrap();
        assert!(writer.add_id(0xB8D9_C538, second).unwrap());
        assert!(!writer.add_name("INI/sound.ini", second).unwrap());
        let empty = sealer.seal(writer.next_ordinal(), &[], 9).unwrap();
        let third = writer.push(empty).unwrap();
        assert!(writer.add_name("data/empty.txt", third).unwrap());
        writer.finish().unwrap();
    }

    #[test]
    fn entries_come_back_by_name_and_id() {
        let path = temp("roundtrip");
        let key = [9_u8; 32];
        sample(&path, &key);
        let mut pack = PackReader::open(&path, &key).unwrap();
        assert_eq!(pack.len(), 3);
        let ordinal = pack.find_name("INI/SOUND.INI").unwrap();
        assert_eq!(pack.read(ordinal).unwrap(), b"[Section]\r\nKey=Value\r\n".repeat(200));
        let ordinal = pack.find_id(0xB8D9_C538).unwrap();
        assert_eq!(pack.read(ordinal).unwrap().len(), 5000);
        let ordinal = pack.find_name("data/empty.txt").unwrap();
        assert!(pack.read(ordinal).unwrap().is_empty());
        assert!(pack.find_name("ini/missing.ini").is_none());
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn the_wrong_key_is_named() {
        let path = temp("wrong-key");
        sample(&path, &[9_u8; 32]);
        let error = PackReader::open(&path, &[8_u8; 32]).err().unwrap();
        assert!(error.to_string().contains("different Gravak build"));
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn an_altered_byte_is_refused() {
        let path = temp("tamper");
        let key = [9_u8; 32];
        sample(&path, &key);
        let mut bytes = std::fs::read(&path).unwrap();
        bytes[HEADER_LENGTH + 10] ^= 0x40;
        std::fs::write(&path, &bytes).unwrap();
        let mut pack = PackReader::open(&path, &key).unwrap();
        let ordinal = pack.find_name("ini/sound.ini").unwrap();
        assert!(pack.read(ordinal).is_err());

        // The header is the index's AAD: changing it fails the index.
        bytes[HEADER_LENGTH + 10] ^= 0x40;
        bytes[12] ^= 1;
        std::fs::write(&path, &bytes).unwrap();
        assert!(PackReader::open(&path, &key).is_err());
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn the_key_round_trips_through_its_mask() {
        assert_eq!(master_key(), parse_key(KEY_TEXT.unwrap_or(DEVELOPMENT_KEY)));
    }
}
