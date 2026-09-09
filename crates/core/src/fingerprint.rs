/// Stable, process-independent content identity (BLAKE3).
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Fingerprint(pub String);

impl Fingerprint {
    pub fn of(content: impl AsRef<[u8]>) -> Self {
        Self(blake3::hash(content.as_ref()).to_hex().to_string())
    }
}
