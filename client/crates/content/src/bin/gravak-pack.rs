//! gravak-pack: packs the game data a player gets into one encrypted file,
//! `game/gravak.gpk` (see `sandbox_data::pack`).
//!
//!   gravak-pack --assets R:\5065 [--content content\build\client]
//!               --out dist\Gravak\game\gravak.gpk [--level 0..10]
//!               [--threads N] [--no-verify]
//!
//! What goes in, in the order the client looks things up:
//!   1. our built content (`--content`), every file in it;
//!   2. the installation's ani, c3, data, ini, map and sound folders;
//!   3. every entry of data.wdf and then c3.wdf, kept under its WDF id.
//! A name taken earlier wins, as GRAVAK_CONTENT and loose files win in the
//! client. The key is the one compiled into this build (GRAVAK_PACK_KEY),
//! so the pack only opens in a client built with the same key.

use sandbox_data::pack::{self, PackReader, PackWriter, SealedEntry};
use sandbox_data::wdf::{WdfDirectory, WdfHeader};
use std::collections::HashSet;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// The installation folders the client reads; executables and the rest
/// of the installation stay out.
const GAME_FOLDERS: [&str; 6] = ["ani", "c3", "data", "ini", "map", "sound"];
const ARCHIVES: [&str; 2] = ["data.wdf", "c3.wdf"];
/// Entries loaded and sealed together; bounds the memory a batch takes.
const BATCH_BYTES: usize = 96 * 1024 * 1024;
const BATCH_ENTRIES: usize = 2048;

enum Source {
    File(PathBuf),
    Archive { archive: usize, offset: u64, size: u32 },
}

enum Key {
    Name(String),
    Id(u32),
}

struct Job {
    source: Source,
    key: Key,
    size: u64,
}

struct Options {
    assets: PathBuf,
    content: Option<PathBuf>,
    out: PathBuf,
    level: u8,
    threads: usize,
    verify: bool,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("gravak-pack: {error}");
        std::process::exit(1);
    }
}

fn usage() -> &'static str {
    "gravak-pack --assets <5065 installation> [--content <content/build/client>] --out <gravak.gpk> [--level 0..10] [--threads N] [--no-verify]"
}

fn parse_options() -> Result<Options> {
    let mut assets = None;
    let mut content = None;
    let mut out = None;
    let mut level = 9;
    let mut threads = std::thread::available_parallelism().map_or(4, usize::from);
    let mut verify = true;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut value = || args.next().ok_or_else(|| format!("{arg} needs a value; {}", usage()));
        match arg.as_str() {
            "--assets" => assets = Some(PathBuf::from(value()?)),
            "--content" => content = Some(PathBuf::from(value()?)),
            "--out" => out = Some(PathBuf::from(value()?)),
            "--level" => level = value()?.parse::<u8>().map_err(|_| "--level takes 0..10")?.min(10),
            "--threads" => threads = value()?.parse::<usize>().map_err(|_| "--threads takes a number")?.max(1),
            "--no-verify" => verify = false,
            "--help" | "-h" => {
                println!("{}", usage());
                std::process::exit(0);
            }
            _ => return Err(format!("unknown argument {arg}; {}", usage()).into()),
        }
    }
    Ok(Options {
        assets: assets.ok_or_else(|| format!("--assets is required; {}", usage()))?,
        content,
        out: out.ok_or_else(|| format!("--out is required; {}", usage()))?,
        level,
        threads,
        verify,
    })
}

fn run() -> Result<()> {
    let options = parse_options()?;
    if !options.assets.join("ini").is_dir() {
        return Err(format!("{} is not a 5065 installation (no ini folder)", options.assets.display()).into());
    }
    if pack::uses_development_key() {
        eprintln!("gravak-pack: WARNING: built without GRAVAK_PACK_KEY; the pack uses the development key.");
    }

    let mut jobs = Vec::new();
    let mut names = HashSet::new();
    let mut roots = Vec::new();
    if let Some(content) = &options.content {
        if content.is_dir() {
            roots.push((content.clone(), None));
        } else {
            eprintln!("gravak-pack: no built content at {}; packing without it", content.display());
        }
    }
    roots.push((options.assets.clone(), Some(&GAME_FOLDERS[..])));
    for (root, folders) in &roots {
        let mut files = Vec::new();
        match folders {
            Some(folders) => {
                for folder in *folders {
                    if let Some(path) = child(root, folder)? {
                        walk(&path, &mut files)?;
                    }
                }
            }
            None => walk(root, &mut files)?,
        }
        files.sort();
        for path in files {
            let relative = path
                .strip_prefix(root)?
                .to_string_lossy()
                .replace('\\', "/");
            let Ok(name) = pack::normalize(&relative) else {
                eprintln!("gravak-pack: skipping {relative}: not a valid game path");
                continue;
            };
            if !names.insert(name.clone()) {
                continue;
            }
            let size = std::fs::metadata(&path)?.len();
            jobs.push(Job {
                source: Source::File(path),
                key: Key::Name(name),
                size,
            });
        }
    }
    let loose = jobs.len();

    let mut archives = Vec::new();
    let mut ids = HashSet::new();
    for name in ARCHIVES {
        let mut found = None;
        for (root, _) in &roots {
            if let Some(path) = child(root, name)? {
                found = Some(path);
                break;
            }
        }
        let Some(path) = found else {
            eprintln!("gravak-pack: no {name}; packing without it");
            continue;
        };
        let directory = read_wdf(&path)?;
        let mut entries: Vec<_> = directory.entries().collect();
        entries.sort_by_key(|entry| entry.offset);
        for entry in entries {
            if !ids.insert(entry.id) {
                continue;
            }
            jobs.push(Job {
                source: Source::Archive {
                    archive: archives.len(),
                    offset: entry.offset.into(),
                    size: entry.size,
                },
                key: Key::Id(entry.id),
                size: entry.size.into(),
            });
        }
        archives.push(path);
    }
    let total: u64 = jobs.iter().map(|job| job.size).sum();
    println!(
        "Packing {} files and {} archive entries ({:.2} GB) into {}",
        loose,
        jobs.len() - loose,
        total as f64 / 1e9,
        options.out.display()
    );

    if let Some(parent) = options.out.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let master = pack::master_key();
    let mut writer = PackWriter::create(&options.out, &master)?;
    let mut archive_files = archives
        .iter()
        .map(File::open)
        .collect::<std::io::Result<Vec<_>>>()?;
    let mut checksums = Vec::with_capacity(jobs.len());
    let mut done_bytes = 0_u64;
    let mut start = 0;
    let started = std::time::Instant::now();
    while start < jobs.len() {
        let mut end = start;
        let mut batch_bytes = 0;
        while end < jobs.len()
            && end - start < BATCH_ENTRIES
            && (end == start || batch_bytes + jobs[end].size as usize <= BATCH_BYTES)
        {
            batch_bytes += jobs[end].size as usize;
            end += 1;
        }
        let mut plain = Vec::with_capacity(end - start);
        for job in &jobs[start..end] {
            plain.push(load(job, &mut archive_files)?);
        }
        let base = writer.next_ordinal();
        let sealed = seal_all(&writer.sealer(), base, &plain, options.level, options.threads)?;
        for ((job, bytes), entry) in jobs[start..end].iter().zip(&plain).zip(sealed) {
            let ordinal = writer.push(entry)?;
            checksums.push(fnv(bytes));
            match &job.key {
                Key::Name(name) => {
                    writer.add_name(name, ordinal)?;
                }
                Key::Id(id) => {
                    writer.add_id(*id, ordinal)?;
                }
            }
            done_bytes += job.size;
        }
        start = end;
        println!(
            "  {:5.1}%  {} / {} entries",
            done_bytes as f64 * 100.0 / total.max(1) as f64,
            start,
            jobs.len()
        );
    }
    let size = writer.finish()?;
    println!(
        "Pack ready: {:.2} GB of game data in {:.2} GB ({:.0} s)",
        total as f64 / 1e9,
        size as f64 / 1e9,
        started.elapsed().as_secs_f64()
    );

    if options.verify {
        verify(&options.out, &master, &jobs, &checksums)?;
    }
    Ok(())
}

/// `name` directly under `dir`, matched without case as Windows does.
fn child(dir: &Path, name: &str) -> Result<Option<PathBuf>> {
    let exact = dir.join(name);
    if exact.exists() {
        return Ok(Some(exact));
    }
    Ok(std::fs::read_dir(dir)?
        .filter_map(|entry| entry.ok())
        .find(|entry| entry.file_name().to_string_lossy().eq_ignore_ascii_case(name))
        .map(|entry| entry.path()))
}

/// Every file under `dir`, leaving out dot-files (the build marker).
fn walk(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    if dir.is_file() {
        out.push(dir.to_path_buf());
        return Ok(());
    }
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        if entry.file_name().to_string_lossy().starts_with('.') {
            continue;
        }
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            walk(&path, out)?;
        } else if path.is_file() {
            out.push(path);
        }
    }
    Ok(())
}

fn read_wdf(path: &Path) -> Result<WdfDirectory> {
    let mut file = File::open(path)?;
    let length = file.metadata()?.len();
    let mut bytes = [0; 12];
    file.read_exact(&mut bytes)?;
    let header = WdfHeader::parse(&bytes)?;
    let size = header.directory_length()?;
    file.seek(SeekFrom::Start(header.directory_offset.into()))?;
    let mut bytes = vec![0; size];
    file.read_exact(&mut bytes)?;
    Ok(WdfDirectory::parse(header, &bytes, length)?)
}

fn load(job: &Job, archives: &mut [File]) -> Result<Vec<u8>> {
    match &job.source {
        Source::File(path) => Ok(std::fs::read(path)?),
        Source::Archive { archive, offset, size } => {
            let file = &mut archives[*archive];
            file.seek(SeekFrom::Start(*offset))?;
            let mut bytes = vec![0; *size as usize];
            file.read_exact(&mut bytes)?;
            Ok(bytes)
        }
    }
}

fn seal_all(
    sealer: &pack::Sealer,
    base: u32,
    plain: &[Vec<u8>],
    level: u8,
    threads: usize,
) -> Result<Vec<SealedEntry>> {
    let next = AtomicUsize::new(0);
    let results: Mutex<Vec<Option<std::io::Result<SealedEntry>>>> =
        Mutex::new((0..plain.len()).map(|_| None).collect());
    std::thread::scope(|scope| {
        for _ in 0..threads.min(plain.len()).max(1) {
            scope.spawn(|| {
                loop {
                    let index = next.fetch_add(1, Ordering::Relaxed);
                    let Some(bytes) = plain.get(index) else { break };
                    let sealed = sealer.seal(base + index as u32, bytes, level);
                    results.lock().expect("no worker panics")[index] = Some(sealed);
                }
            });
        }
    });
    results
        .into_inner()
        .expect("no worker panics")
        .into_iter()
        .map(|result| Ok(result.expect("every entry sealed")?))
        .collect()
}

/// FNV-1a, to check what the pack gives back against what went in.
fn fnv(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3)
    })
}

/// Opens the pack the way the client does and reads every entry back.
fn verify(path: &Path, master: &[u8; 32], jobs: &[Job], checksums: &[u64]) -> Result<()> {
    println!("Verifying...");
    let mut reader = PackReader::open(path, master)?;
    if reader.len() != jobs.len() {
        return Err(format!("pack holds {} entries, {} were packed", reader.len(), jobs.len()).into());
    }
    for (ordinal, (job, checksum)) in jobs.iter().zip(checksums).enumerate() {
        let found = match &job.key {
            Key::Name(name) => reader.find_name(name),
            Key::Id(id) => reader.find_id(*id),
        };
        if found != Some(ordinal as u32) {
            return Err(format!("entry {ordinal} cannot be found in the pack").into());
        }
        if fnv(&reader.read(ordinal as u32)?) != *checksum {
            return Err(format!("entry {ordinal} reads back different").into());
        }
    }
    println!("Verified: all {} entries read back intact.", jobs.len());
    Ok(())
}
