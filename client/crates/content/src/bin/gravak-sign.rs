//! Signs update manifests, so players' clients take only updates made with
//! the release key (`crates/sandbox/src/updater.rs`).
//!
//! ```text
//! gravak-sign keygen <folder>                 update.key (secret) and update.pub
//! gravak-sign sign <update.key> <manifest.txt> writes manifest.sig beside it
//! ```
//!
//! Keep `update.key` off the repository and off the update host: whoever has
//! it can push files to every player. `update.pub` (both live in `client/keys`)
//! is built into the client
//! (`GRAVAK_UPDATE_KEY`, set by `tools/package-client.ps1`).

use ed25519_dalek::{Signer, SigningKey};
use std::path::{Path, PathBuf};

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn unhex(text: &str) -> Option<[u8; 32]> {
    let text = text.trim();
    if text.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    for (index, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(text.get(index * 2..index * 2 + 2)?, 16).ok()?;
    }
    Some(out)
}

fn keygen(folder: &Path) -> Result<(), String> {
    let secret = folder.join("update.key");
    if secret.exists() {
        return Err(format!("{} already exists; a new key would orphan every installed client", secret.display()));
    }
    let mut seed = [0u8; 32];
    getrandom::getrandom(&mut seed).map_err(|error| error.to_string())?;
    let key = SigningKey::from_bytes(&seed);
    std::fs::create_dir_all(folder).map_err(|error| error.to_string())?;
    std::fs::write(&secret, hex(&seed)).map_err(|error| error.to_string())?;
    std::fs::write(folder.join("update.pub"), hex(key.verifying_key().as_bytes())).map_err(|error| error.to_string())?;
    println!("Release key made: {} (secret) and update.pub", secret.display());
    Ok(())
}

fn sign(secret: &Path, manifest: &Path) -> Result<(), String> {
    let seed = std::fs::read_to_string(secret).map_err(|error| format!("{}: {error}", secret.display()))?;
    let seed = unhex(&seed).ok_or("the key file is not 64 hex digits")?;
    let key = SigningKey::from_bytes(&seed);
    let text = std::fs::read(manifest).map_err(|error| format!("{}: {error}", manifest.display()))?;
    let signature = key.sign(&text);
    let out = manifest.with_file_name("manifest.sig");
    std::fs::write(&out, hex(&signature.to_bytes())).map_err(|error| error.to_string())?;
    println!("Signed: {}", out.display());
    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.iter().map(String::as_str).collect::<Vec<_>>()[..] {
        ["keygen", folder] => keygen(&PathBuf::from(folder)),
        ["sign", key, manifest] => sign(Path::new(key), Path::new(manifest)),
        _ => Err("usage: gravak-sign keygen <folder> | gravak-sign sign <update.key> <manifest.txt>".into()),
    };
    if let Err(error) = result {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
