//! Stable, process-independent content identity computed with BLAKE3.
//!
//! A fingerprint is the hex digest of the exact bytes passed to [`Fingerprint::of`], so
//! it is reproducible across processes, machines and runs. Caches and generated
//! identities compare fingerprints to decide whether earlier work still describes the
//! same content.
//!
//! This is a content address, not a security primitive: nothing here is keyed or
//! authenticated, and a digest must never be used to prove authorship.

use lean_string::LeanStr;

/// BLAKE3 digest of some content, as lowercase hex.
///
/// A fingerprint says nothing about where the content came from or how it is named;
/// equal bytes give equal fingerprints; collision resistance is not a proof of
/// equality. The ordering is lexicographic over the hex text so fingerprints can key a
/// `BTreeMap` deterministically.

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Fingerprint(pub LeanStr);

impl Fingerprint {
    /// Hashes the bytes of `content`, accepting both `&str` and `&[u8]`.
    pub fn of(content: impl AsRef<[u8]>) -> Self {
        Self(blake3::hash(content.as_ref()).to_hex().as_str().into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprints_are_stable_and_sensitive_to_content() {
        assert_eq!(
            Fingerprint::of("").0,
            "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262"
        );
        assert_eq!(Fingerprint::of("hello"), Fingerprint::of(b"hello"));
        assert_ne!(Fingerprint::of("hello"), Fingerprint::of("Hello"));
    }
}
