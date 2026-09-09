//! Content identity for compiler builds, including unpublished development builds.
use std::{fs, path::Path};

fn hash_tree(path: &Path, hasher: &mut blake3::Hasher) {
    println!("cargo:rerun-if-changed={}", path.display());
    if path.is_dir() {
        let mut entries: Vec<_> = fs::read_dir(path)
            .expect("compiler source directory")
            .map(|entry| entry.expect("compiler source entry").path())
            .collect();
        entries.sort();
        for entry in entries {
            hash_tree(&entry, hasher);
        }
    } else {
        let bytes = fs::read(path).expect("compiler source file");
        hasher.update(&(bytes.len() as u64).to_le_bytes());
        hasher.update(&bytes);
    }
}

fn main() {
    let mut hasher = blake3::Hasher::new();
    for directory in [
        "src",
        "../source/src",
        "../ir/src",
        "../semantic/src",
        "../grammar/src",
        "../css/src",
        "../tailwind/src",
        "../js-oxc/src",
        "../../css",
    ] {
        hash_tree(Path::new(directory), &mut hasher);
    }
    hash_tree(Path::new("../../Cargo.lock"), &mut hasher);
    println!(
        "cargo:rustc-env=STILLWIND_COMPILER_FINGERPRINT={}",
        hasher.finalize().to_hex()
    );
}
