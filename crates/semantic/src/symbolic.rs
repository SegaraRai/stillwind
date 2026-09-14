//! Sound symbolic values for handle flow between modules.
//!
//! A [`SymbolicValue`] states what a handle expression may denote: exactly one asset
//! kind, and either a finite, duplicate-free set of declaration-site identities or the
//! whole universe of that kind. [`ExportedValue`] is the same idea per asset kind,
//! except that refs are exported as a table of [`RefAlternative`] entries, because a
//! ref and its permitted scopes must stay paired.
//!
//! The algebra is sound, not complete. [`Candidates::Universe`] means "any asset of
//! this kind", so consumers must fall back to the runtime path instead of choosing
//! candidates, and a finite set must never be widened or flattened into a
//! cross-product.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use super::identity::{AssetKind, IdentityId};

/// Failure to build a symbolic value or alternative.
#[derive(Clone, Copy, Debug, Eq, thiserror::Error, PartialEq)]
pub enum SymbolicError {
    /// An identity or scope value of the wrong asset kind was supplied.
    #[error("symbolic handle kind mismatch: expected {expected:?}, got {actual:?}")]
    KindMismatch {
        /// Kind the value was declared with.
        expected: AssetKind,
        /// Kind the supplied candidate carries instead.
        actual: AssetKind,
    },
}

/// What a symbolic value may denote.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum Candidates {
    /// A proven candidate set, deduplicated at construction.
    ///
    /// Iteration order is the derived `Ord` order of [`IdentityId`], not source order.
    /// Two sets built from the same identities in any order compare equal.
    Finite(BTreeSet<IdentityId>),
    /// Any asset of the value's kind. Sound only as an upper bound: consumers must not
    /// pick candidates from it, because nothing proved they can occur.
    Universe,
}

/// A sound abstract value containing identities of exactly one asset kind.
///
/// [`Self::finite`] checks candidate kinds and [`Self::exact`] derives the kind from
/// its identity. Public fields and derived deserialization can bypass those checks;
/// consumers of externally constructed values must validate them.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SymbolicValue {
    /// Asset kind of every identity the value may contain.
    pub kind: AssetKind,
    /// Finite candidate set or universe.
    pub candidates: Candidates,
}

impl SymbolicValue {
    /// Validates candidate kinds after construction or deserialization.
    ///
    /// # Errors
    ///
    /// Returns an error when a finite candidate has another asset kind.
    pub fn validate(&self) -> Result<(), SymbolicError> {
        if let Candidates::Finite(candidates) = &self.candidates {
            for identity in candidates {
                if identity.kind != self.kind {
                    return Err(SymbolicError::KindMismatch {
                        expected: self.kind,
                        actual: identity.kind,
                    });
                }
            }
        }
        Ok(())
    }

    /// The value denoting exactly `identity`, with the kind taken from it.
    pub fn exact(identity: IdentityId) -> Self {
        Self {
            kind: identity.kind,
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
        let identities = identities.into_iter().collect::<BTreeSet<_>>();
        if let Some(identity) = identities.iter().find(|identity| identity.kind != kind) {
            return Err(SymbolicError::KindMismatch {
                expected: kind,
                actual: identity.kind,
            });
        }
        Ok(Self {
            kind,
            candidates: Candidates::Finite(identities),
        })
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

/// One correlated ref alternative. Keeping the scope value beside its ref prevents an
/// invalid scope/ref Cartesian product during selector factoring.
///
/// Each alternative is an independent case. Combining the refs of one alternative with
/// the scopes of another would describe DOM relationships the source never denoted, so
/// alternatives are never merged.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RefAlternative {
    /// The ref identity; always [`AssetKind::Ref`].
    pub reference: IdentityId,
    /// `None` is the descendant-only, unscoped ref relation.
    ///
    /// A finite scope value is a candidate set of this ref's permitted scopes; a
    /// universe value means the frontend could not narrow them.
    pub permitted_scopes: Option<SymbolicValue>,
}

impl RefAlternative {
    /// Builds one correlated alternative, rejecting mismatched kinds.
    ///
    /// Checks the reference, scope value and every finite scope candidate.
    ///
    /// # Errors
    ///
    /// Returns an error for a non-ref identity or non-scope relationship value.
    pub fn new(
        reference: IdentityId,
        permitted_scopes: Option<SymbolicValue>,
    ) -> Result<Self, SymbolicError> {
        if reference.kind != AssetKind::Ref {
            return Err(SymbolicError::KindMismatch {
                expected: AssetKind::Ref,
                actual: reference.kind,
            });
        }
        if let Some(scopes) = &permitted_scopes
            && scopes.kind != AssetKind::Scope
        {
            return Err(SymbolicError::KindMismatch {
                expected: AssetKind::Scope,
                actual: scopes.kind,
            });
        }
        if let Some(scopes) = &permitted_scopes {
            scopes.validate()?;
        }
        Ok(Self {
            reference,
            permitted_scopes,
        })
    }
}

/// What one exported name denotes.
///
/// The serialized shape (`kind` plus `value`, kebab-case tags) is part of the host
/// protocol and must stay stable.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "kebab-case")]
pub enum ExportedValue {
    /// Candidate set of `style()` call sites.
    Styles(SymbolicValue),
    /// Candidate set of scope identities.
    Scopes(SymbolicValue),
    /// Correlated ref alternatives, in the order they were derived.
    ///
    /// This is the one variant that is a table rather than a single value: flattening
    /// it into a ref set plus a scope set would lose the correlation between a ref and
    /// the scopes it may appear under.
    Refs(Vec<RefAlternative>),
    /// Candidate set of animation identities.
    Animations(SymbolicValue),
    /// Candidate set of view-transition identities.
    ViewTransitions(SymbolicValue),
}

impl ExportedValue {
    /// Validates the tagged kind and all nested candidates, including correlated scopes.
    ///
    /// # Errors
    ///
    /// Returns an error when the tag, value or candidate kinds disagree.
    pub fn validate(&self) -> Result<(), SymbolicError> {
        match self {
            Self::Refs(alternatives) => {
                for alternative in alternatives {
                    RefAlternative::new(
                        alternative.reference.clone(),
                        alternative.permitted_scopes.clone(),
                    )?;
                }
                Ok(())
            }
            Self::Styles(value)
            | Self::Scopes(value)
            | Self::Animations(value)
            | Self::ViewTransitions(value) => {
                if value.kind != self.kind() {
                    return Err(SymbolicError::KindMismatch {
                        expected: self.kind(),
                        actual: value.kind,
                    });
                }
                value.validate()
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
            Self::Scopes(_) => AssetKind::Scope,
            Self::Refs(_) => AssetKind::Ref,
            Self::Animations(_) => AssetKind::Animation,
            Self::ViewTransitions(_) => AssetKind::ViewTransition,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity(kind: AssetKind, start: u32) -> IdentityId {
        IdentityId::new(kind, r"src\Menu.tsx", start, start + 1, "asset")
    }

    #[test]
    fn ref_alternatives_retain_scope_correlation() {
        let scope_a = SymbolicValue::exact(identity(AssetKind::Scope, 1));
        let scope_b = SymbolicValue::exact(identity(AssetKind::Scope, 2));
        let alternatives = [
            RefAlternative::new(identity(AssetKind::Ref, 3), Some(scope_a)).unwrap(),
            RefAlternative::new(identity(AssetKind::Ref, 4), Some(scope_b)).unwrap(),
        ];

        assert_ne!(
            alternatives[0].permitted_scopes,
            alternatives[1].permitted_scopes
        );
    }

    #[test]
    fn validates_nested_scope_candidates_and_export_tags() {
        let malformed = SymbolicValue {
            kind: AssetKind::Scope,
            candidates: Candidates::Finite(BTreeSet::from([identity(AssetKind::Animation, 0)])),
        };
        assert!(RefAlternative::new(identity(AssetKind::Ref, 1), Some(malformed.clone())).is_err());
        assert!(ExportedValue::Scopes(malformed).validate().is_err());
        assert!(
            ExportedValue::Animations(SymbolicValue::universe(AssetKind::Scope))
                .validate()
                .is_err()
        );
    }
}

#[cfg(test)]
mod boundary_tests {
    use super::*;

    #[test]
    fn finite_values_deduplicate_and_reject_mixed_asset_kinds() {
        let id = IdentityId::new(AssetKind::Scope, "x", 0, 1, "scope");
        let value = SymbolicValue::finite(AssetKind::Scope, [id.clone(), id]).unwrap();
        assert_eq!(value.finite_candidates().unwrap().len(), 1);
        let wrong = IdentityId::new(AssetKind::Animation, "x", 0, 1, "animation");
        assert!(SymbolicValue::finite(AssetKind::Scope, [wrong.clone()]).is_err());
        assert!(RefAlternative::new(wrong, None).is_err());
        let reference = IdentityId::new(AssetKind::Ref, "x", 0, 1, "ref");
        assert!(
            RefAlternative::new(
                reference,
                Some(SymbolicValue::universe(AssetKind::Animation))
            )
            .is_err()
        );
    }

    proptest::proptest! {
        #[test]
        fn finite_values_ignore_order_and_duplicates(offsets in proptest::collection::vec(0_u32..100, 0..12)) {
            let identities = offsets.iter().map(|&start| IdentityId::new(AssetKind::Ref, "x", start, start + 1, "r")).collect::<Vec<_>>();
            let forward = SymbolicValue::finite(AssetKind::Ref, identities.clone()).unwrap();
            let reverse = SymbolicValue::finite(AssetKind::Ref, identities.iter().rev().chain(&identities).cloned()).unwrap();
            proptest::prop_assert_eq!(forward, reverse);
        }
    }
}
