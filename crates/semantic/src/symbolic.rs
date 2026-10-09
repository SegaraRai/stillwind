//! Sound symbolic values for asset flow between modules.
//!
//! A [`SymbolicValue`] states what an asset expression may denote: exactly one asset
//! kind, and either a finite, duplicate-free set of declaration-site identities or the
//! whole universe of that kind. [`ExportedValue`] tags the symbolic value with its
//! asset family for exports and host facts.
//!
//! The algebra is sound, not complete. [`Candidates::Universe`] means "any asset of
//! this kind", so consumers must fall back to the runtime path instead of choosing
//! candidates, and a finite set must never be widened or flattened into a
//! cross-product.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use super::identity::{AssetKind, IdentityId};

/// Failure to build or validate a symbolic value.
#[derive(Clone, Copy, Debug, Eq, thiserror::Error, PartialEq)]
pub enum SymbolicError {
    /// An identity of the wrong asset kind was supplied.
    #[error("symbolic asset kind mismatch: expected {expected:?}, got {actual:?}")]
    KindMismatch {
        /// Kind the value was declared with.
        expected: AssetKind,
        /// Kind the supplied candidate carries instead.
        actual: AssetKind,
    },
}

/// What a symbolic value may denote.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "camelCase")]
pub enum Candidates {
    /// A proven candidate set, deduplicated at construction.
    ///
    /// Iteration order is the derived `Ord` order of [`IdentityId`], not source order.
    /// Two sets built from the same identities in any order compare equal. An empty set
    /// means analysis proved that no declaration is reachable.
    Finite(BTreeSet<IdentityId>),
    /// Any asset of the value's kind. Sound only as an upper bound: consumers must not
    /// pick candidates from it, because nothing proved they can occur.
    Universe,
}

/// A sound abstract value containing identities of exactly one asset kind.
///
/// Construction and deserialization reject candidates of another asset kind.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", try_from = "SymbolicValueInput")]
pub struct SymbolicValue {
    /// Asset kind of every identity the value may contain.
    kind: AssetKind,
    /// Finite candidate set or universe.
    candidates: Candidates,
}

impl SymbolicValue {
    fn from_candidates(kind: AssetKind, candidates: Candidates) -> Result<Self, SymbolicError> {
        if let Candidates::Finite(identities) = &candidates {
            for identity in identities {
                if identity.kind() != kind {
                    return Err(SymbolicError::KindMismatch {
                        expected: kind,
                        actual: identity.kind(),
                    });
                }
            }
        }
        Ok(Self { kind, candidates })
    }

    /// Returns the asset kind shared by every candidate.
    pub const fn kind(&self) -> AssetKind {
        self.kind
    }

    /// Consumes the value and returns its kind and owned candidates.
    pub fn into_parts(self) -> (AssetKind, Candidates) {
        (self.kind, self.candidates)
    }

    /// The value denoting exactly `identity`, with the kind taken from it.
    pub fn exact(identity: IdentityId) -> Self {
        Self {
            kind: identity.kind(),
            candidates: Candidates::Finite(BTreeSet::from([identity])),
        }
    }

    /// The value denoting any asset of `kind`.
    ///
    /// Use this only as a bound for values the frontend could not resolve; it is not a
    /// licence to guess candidates later.
    pub const fn universe(kind: AssetKind) -> Self {
        Self {
            kind,
            candidates: Candidates::Universe,
        }
    }

    /// Builds a normalized, duplicate-free finite value.
    ///
    /// Duplicates collapse and input order does not matter, so two callers that observe
    /// the same candidates in different orders build equal values.
    ///
    /// # Errors
    ///
    /// Returns an error if any identity belongs to another asset kind.
    pub fn finite(
        kind: AssetKind,
        identities: impl IntoIterator<Item = IdentityId>,
    ) -> Result<Self, SymbolicError> {
        Self::from_candidates(kind, Candidates::Finite(identities.into_iter().collect()))
    }

    /// Returns the finite candidates, or `None` for [`Candidates::Universe`].
    ///
    /// `None` means "not provable", never "empty": a caller that cannot handle a
    /// universe must abandon that analysis path rather than substitute an empty set.
    pub fn finite_candidates(&self) -> Option<&BTreeSet<IdentityId>> {
        match &self.candidates {
            Candidates::Finite(candidates) => Some(candidates),
            Candidates::Universe => None,
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SymbolicValueInput {
    kind: AssetKind,
    candidates: Candidates,
}

impl TryFrom<SymbolicValueInput> for SymbolicValue {
    type Error = SymbolicError;

    fn try_from(value: SymbolicValueInput) -> Result<Self, Self::Error> {
        Self::from_candidates(value.kind, value.candidates)
    }
}

/// What one exported name denotes.
///
/// The serialized shape (`kind` plus `value`, camelCase tags) is part of the host
/// protocol and must stay stable.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "camelCase")]
pub enum ExportedValue {
    /// Candidate set of `style()` call sites.
    Styles(SymbolicValue),
    /// Candidate set of ref identities.
    Refs(SymbolicValue),
    /// Candidate set of keyframes identities.
    Keyframes(SymbolicValue),
    /// Candidate set of view-transition identities.
    ViewTransitions(SymbolicValue),
}

impl ExportedValue {
    /// Validates that the export tag agrees with the symbolic value's kind.
    ///
    /// # Errors
    ///
    /// Returns an error when the tag and value kinds disagree.
    pub fn validate(&self) -> Result<(), SymbolicError> {
        match self {
            Self::Styles(value)
            | Self::Refs(value)
            | Self::Keyframes(value)
            | Self::ViewTransitions(value) => {
                if value.kind != self.kind() {
                    return Err(SymbolicError::KindMismatch {
                        expected: self.kind(),
                        actual: value.kind,
                    });
                }
                Ok(())
            }
        }
    }

    /// The asset kind this value describes.
    ///
    /// Callers compare it against the kind expected at a use site before reading the
    /// payload, so a value cannot be read as another kind.
    pub const fn kind(&self) -> AssetKind {
        match self {
            Self::Styles(_) => AssetKind::Style,
            Self::Refs(_) => AssetKind::Ref,
            Self::Keyframes(_) => AssetKind::Keyframes,
            Self::ViewTransitions(_) => AssetKind::ViewTransition,
        }
    }
}

#[cfg(test)]
mod tests {
    use stillwind_source::ModuleId;

    use super::*;

    fn identity(kind: AssetKind, start: u32) -> IdentityId {
        IdentityId::new(
            kind,
            ModuleId::from(r"src\Menu.tsx"),
            start,
            start + 1,
            "asset",
        )
    }

    #[test]
    fn validates_ref_candidates_and_export_tags() {
        assert!(
            ExportedValue::Refs(SymbolicValue::universe(AssetKind::Keyframes))
                .validate()
                .is_err()
        );
        assert!(
            ExportedValue::Keyframes(SymbolicValue::universe(AssetKind::Ref))
                .validate()
                .is_err()
        );
        assert!(
            ExportedValue::Refs(SymbolicValue::exact(identity(AssetKind::Ref, 1)))
                .validate()
                .is_ok()
        );
        assert!(
            ExportedValue::Refs(SymbolicValue::universe(AssetKind::Ref))
                .validate()
                .is_ok()
        );
    }

    #[test]
    fn deserialization_validates_candidates_and_preserves_the_wire_shape() {
        let value = SymbolicValue::exact(identity(AssetKind::Ref, 1));
        let mut input = serde_json::to_value(&value).unwrap();
        assert_eq!(input["kind"], "ref");
        assert_eq!(input["candidates"]["kind"], "finite");
        assert_eq!(
            serde_json::from_value::<SymbolicValue>(input.clone()).unwrap(),
            value
        );
        input["kind"] = serde_json::json!("keyframes");
        assert!(serde_json::from_value::<SymbolicValue>(input).is_err());

        let universe = SymbolicValue::universe(AssetKind::Ref);
        assert_eq!(
            serde_json::from_value::<SymbolicValue>(serde_json::to_value(&universe).unwrap())
                .unwrap(),
            universe
        );
    }

    #[test]
    fn finite_values_deduplicate_and_reject_mixed_asset_kinds() {
        let id = IdentityId::new(AssetKind::Ref, ModuleId::from("x"), 0, 1, "boundary");
        let value = SymbolicValue::finite(AssetKind::Ref, [id.clone(), id]).unwrap();
        assert_eq!(value.finite_candidates().unwrap().len(), 1);
        let wrong = IdentityId::new(AssetKind::Keyframes, ModuleId::from("x"), 0, 1, "animation");
        assert!(SymbolicValue::finite(AssetKind::Ref, [wrong.clone()]).is_err());
    }

    proptest::proptest! {
        #[test]
        fn finite_values_ignore_order_and_duplicates(offsets in proptest::collection::vec(0_u32..100, 0..12)) {
            let identities = offsets.iter().map(|&start| IdentityId::new(
                AssetKind::Ref,
                ModuleId::from("x"),
                start,
                start + 1,
                "r"
            )).collect::<Vec<_>>();
            let forward = SymbolicValue::finite(AssetKind::Ref, identities.clone()).unwrap();
            let reverse = SymbolicValue::finite(AssetKind::Ref, identities.iter().rev().chain(&identities).cloned()).unwrap();
            proptest::prop_assert_eq!(forward, reverse);
        }
    }
}
