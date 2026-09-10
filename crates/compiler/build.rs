//! Content identity for compiler builds, including unpublished development builds.

#![cfg_attr(
    not(test),
    deny(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::unreachable,
        clippy::todo,
        clippy::unimplemented
    )
)]

use std::fs;
use std::path::Path;

fn hash_tree(path: &Path, hasher: &mut blake3::Hasher) -> std::io::Result<()> {
    println!("cargo:rerun-if-changed={}", path.display());
    if path.is_dir() {
        let mut entries: Vec<_> = fs::read_dir(path)?
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<Result<_, _>>()?;
        entries.sort();
        for entry in entries {
            hash_tree(&entry, hasher)?;
        }
    } else {
        let bytes = fs::read(path)?;
        hasher.update(&(bytes.len() as u64).to_le_bytes());
        hasher.update(&bytes);
    }
    Ok(())
}

fn main() -> std::io::Result<()> {
    let mut hasher = blake3::Hasher::new();
    for directory in [
        "src",
        "Cargo.toml",
        "build.rs",
        "../source/src",
        "../source/Cargo.toml",
        "../ir/src",
        "../ir/Cargo.toml",
        "../semantic/src",
        "../semantic/Cargo.toml",
        "../syntax/src",
        "../syntax/Cargo.toml",
        "../css/src",
        "../css/assets",
        "../css/Cargo.toml",
        "../design-system/src",
        "../design-system/Cargo.toml",
    ] {
        hash_tree(Path::new(directory), &mut hasher)?;
    }
    hash_tree(Path::new("../../Cargo.lock"), &mut hasher)?;
    println!(
        "cargo:rustc-env=STILLWIND_COMPILER_FINGERPRINT={}",
        hasher.finalize().to_hex()
    );
    Ok(())
}
