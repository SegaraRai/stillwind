use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use super::identity::{AssetKind, IdentityId};

#[derive(Clone, Copy, Debug, Eq, thiserror::Error, PartialEq)]
pub enum SymbolicError {
    #[error("symbolic handle kind mismatch: expected {expected:?}, got {actual:?}")]
    KindMismatch {
        expected: AssetKind,
        actual: AssetKind,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum Candidates {
    Finite(BTreeSet<IdentityId>),
    Universe,
}

/// A sound abstract value containing identities of exactly one asset kind.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SymbolicValue {
    pub kind: AssetKind,
    pub candidates: Candidates,
}

impl SymbolicValue {
    pub fn exact(identity: IdentityId) -> Self {
        Self {
            kind: identity.kind,
            candidates: Candidates::Finite(BTreeSet::from([identity])),
        }
    }

    pub const fn universe(kind: AssetKind) -> Self {
        Self {
            kind,
            candidates: Candidates::Universe,
        }
    }

    /// Builds a normalized, duplicate-free finite value.
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

    pub fn finite_candidates(&self) -> Option<&BTreeSet<IdentityId>> {
        match &self.candidates {
            Candidates::Finite(candidates) => Some(candidates),
            Candidates::Universe => None,
        }
    }
}

/// One correlated ref alternative. Keeping the scope value beside its ref
/// prevents an invalid scope/ref Cartesian product during selector factoring.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RefAlternative {
    pub reference: IdentityId,
    /// `None` is the descendant-only, unscoped ref relation.
    pub permitted_scopes: Option<SymbolicValue>,
}

impl RefAlternative {
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
        Ok(Self {
            reference,
            permitted_scopes,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "kebab-case")]
pub enum ExportedValue {
    Styles(SymbolicValue),
    Scopes(SymbolicValue),
    Refs(Vec<RefAlternative>),
    Animations(SymbolicValue),
    ViewTransitions(SymbolicValue),
}

impl ExportedValue {
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
