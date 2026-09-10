/// Stable, process-independent content identity (BLAKE3).

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Fingerprint(pub String);

impl Fingerprint {
    pub fn of(content: impl AsRef<[u8]>) -> Self {
        Self(blake3::hash(content.as_ref()).to_hex().to_string())
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
