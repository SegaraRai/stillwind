//! Expression-level type facts, keyed by source span.
//!
//! A [`TypeFact`] says what one expression in a module denotes after type checking, so
//! the frontend can identify finite asset candidates or a confirmed non-asset without
//! solving TypeScript itself. Facts are keyed by the byte span of
//! the expression in the module's original source; a fact whose span does not slice
//! that source is invalid, and the whole payload is discarded rather than partially
//! applied.

use serde::{Deserialize, Serialize};

use super::symbolic::{ExportedValue, SymbolicError};

/// A checked asset value or confirmation that the expression is not a Stillwind asset.
/// Asset values retain their exported wire shape. `NonAsset` is serialized as `null`.
/// A missing fact means the checker has not established either result.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CheckedValue {
    /// Asset family and its candidate identities, including a nonfinite upper bound.
    Asset(ExportedValue),
    /// The checker confirmed that the expression is not a Stillwind asset.
    NonAsset,
}

impl CheckedValue {
    /// Validates an asset's family and candidate kinds. Confirmed non-assets are valid.
    ///
    /// # Errors
    /// Returns an error when an asset family disagrees with its symbolic kind.
    pub fn validate(&self) -> Result<(), SymbolicError> {
        match self {
            Self::Asset(value) => value.validate(),
            Self::NonAsset => Ok(()),
        }
    }
}

impl From<ExportedValue> for CheckedValue {
    fn from(value: ExportedValue) -> Self {
        Self::Asset(value)
    }
}

/// What one checked expression denotes.
///
/// The fact is a claim by the host checker; consumers must check the enclosing
/// [`super::TypeFacts`] revisions before trusting it, and must fall back to local analysis
/// when it is missing.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeFact {
    /// UTF-8 byte offset of the expression start in the original source.
    pub start: u32,
    /// UTF-8 byte offset of the expression end, exclusive.
    pub end: u32,
    /// Checked asset value, or `null` for a confirmed non-asset.
    pub value: CheckedValue,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn confirmed_non_asset_facts_use_explicit_null_and_require_a_value() {
        let fact = TypeFact {
            start: 1,
            end: 2,
            value: CheckedValue::NonAsset,
        };
        let json = serde_json::json!({"start":1,"end":2,"value":null});
        assert_eq!(serde_json::to_value(&fact).unwrap(), json);
        assert_eq!(serde_json::from_value::<TypeFact>(json).unwrap(), fact);
        assert!(
            serde_json::from_value::<TypeFact>(serde_json::json!({"start":1,"end":2})).is_err()
        );
    }
}
