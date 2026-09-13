//! Content identity for compiler builds, including unpublished development builds.
//!
//! The fingerprint hashes this crate's own sources together with the crates it lowers into, so a
//! change to any of them yields a new identity and invalidates every artifact keyed by it.
//! Directories are walked in sorted order, and each file's length is hashed alongside its contents,
//! so a file that only changes length is still a change. `Cargo.lock` participates because a
//! dependency update can change emitted CSS without touching a source file in this workspace.

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

/// Hashes file lengths and contents in sorted traversal order and emits Cargo watches.
///
/// Existing hasher state is retained; a read failure can leave a partial contribution.
///
/// # Errors
///
/// Propagates directory enumeration and file-read failures.
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

/// Publishes the compiler content fingerprint as a Cargo build-time environment value.
///
/// # Errors
///
/// Propagates reads of the listed source trees, manifests, or workspace lockfile.
fn main() -> std::io::Result<()> {
    let mut hasher = blake3::Hasher::new();
    for path in [
        "../css/assets",
        "../css/src",
        "../css/Cargo.toml",
        "../design-system/src",
        "../design-system/Cargo.toml",
        "../diagnostics/src",
        "../diagnostics/Cargo.toml",
        "../ir/src",
        "../ir/Cargo.toml",
        "../semantic/src",
        "../semantic/Cargo.toml",
        "../source/src",
        "../source/Cargo.toml",
        "../syntax/src",
        "../syntax/Cargo.toml",
        "src",
        "Cargo.toml",
        "build.rs",
    ] {
        hash_tree(Path::new(path), &mut hasher)?;
    }
    hash_tree(Path::new("../../Cargo.lock"), &mut hasher)?;
    println!(
        "cargo:rustc-env=STILLWIND_COMPILER_FINGERPRINT={}",
        hasher.finalize().to_hex()
    );
    Ok(())
}
