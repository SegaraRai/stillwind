//! Host document identities whose original spelling is retained.

use lean_string::LeanStr;
use serde::{Deserialize, Serialize};

/// An invalid source-document identifier supplied by a host.
#[derive(Clone, Debug, thiserror::Error)]
#[error("invalid source URI: {0}")]
pub struct SourceUriError(pub String);

/// Host-supplied identity of an editable or virtual source document.
///
/// The spelling is preserved because a host or editor may distinguish valid URI
/// spellings that a generic normalizer would consider equivalent.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct SourceUri(LeanStr);

impl SourceUri {
    /// Validates an absolute URI without changing its spelling.
    ///
    /// # Errors
    ///
    /// Returns an error for a missing scheme or malformed URI characters.
    pub fn parse(value: impl Into<LeanStr>) -> Result<Self, SourceUriError> {
        let value = value.into();
        let Some((scheme, rest)) = value.split_once(':') else {
            return Err(SourceUriError(value.to_string()));
        };
        let valid_scheme = !scheme.is_empty()
            && scheme.bytes().enumerate().all(|(index, byte)| {
                if index == 0 {
                    byte.is_ascii_alphabetic()
                } else {
                    byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'-' | b'.')
                }
            });
        let bytes = rest.as_bytes();
        let mut index = 0;
        let mut valid_rest = !rest.is_empty();
        while index < bytes.len() {
            let byte = bytes[index];
            if byte == b'%' {
                valid_rest &= bytes.get(index + 1).is_some_and(u8::is_ascii_hexdigit)
                    && bytes.get(index + 2).is_some_and(u8::is_ascii_hexdigit);
                index += 3;
            } else {
                valid_rest &=
                    byte.is_ascii_alphanumeric() || b"-._~:/?#[]@!$&'()*+,;=".contains(&byte);
                index += 1;
            }
        }
        if !valid_scheme || !valid_rest {
            return Err(SourceUriError(value.to_string()));
        }
        Ok(Self(value))
    }

    /// Returns the exact identifier supplied by the host.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for SourceUri {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        Self::parse(value).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_uris_preserve_host_spelling_and_reject_paths() {
        for value in [
            "file:///F:/Project/a%20b.tsx",
            "untitled:1",
            "https://example.test/app.tsx",
        ] {
            let uri = SourceUri::parse(value).unwrap();
            assert_eq!(uri.as_str(), value);
            let restored: SourceUri =
                serde_json::from_str(&serde_json::to_string(&uri).unwrap()).unwrap();
            assert_eq!(restored, uri);
        }
        for value in [
            r"F:\Project\App.tsx",
            "src/App.tsx",
            ":bad",
            "file:///bad%2",
            "file:///has space",
        ] {
            assert!(SourceUri::parse(value).is_err(), "{value}");
        }
    }
}
