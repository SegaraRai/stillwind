//! Cross-module identities, finite candidate values, and module summaries.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use shorth_source::normalized_module_path;
use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AssetKind {
    Scope,
    Ref,
    Animation,
    ViewTransition,
}

/// Full declaration-site identity. The generated 32-bit name is derived later;
/// retaining this key lets the build adapter detect hash collisions.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct IdentityId {
    pub kind: AssetKind,
    pub module_path: String,
    pub start: u32,
    pub end: u32,
    pub binding_name: String,
}

impl IdentityId {
    pub fn new(
        kind: AssetKind,
        module_path: &str,
        start: u32,
        end: u32,
        binding_name: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            module_path: normalized_module_path(module_path),
            start,
            end,
            binding_name: binding_name.into(),
        }
    }
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
    Scopes(SymbolicValue),
    Refs(Vec<RefAlternative>),
    Animations(SymbolicValue),
    ViewTransitions(SymbolicValue),
}

impl ExportedValue {
    pub const fn kind(&self) -> AssetKind {
        match self {
            Self::Scopes(_) => AssetKind::Scope,
            Self::Refs(_) => AssetKind::Ref,
            Self::Animations(_) => AssetKind::Animation,
            Self::ViewTransitions(_) => AssetKind::ViewTransition,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ImportBinding {
    pub local: String,
    pub source: String,
    pub imported: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ReExport {
    pub exported: String,
    pub source: String,
    pub imported: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ModuleSummary {
    pub schema: u32,
    pub module_path: String,
    pub imports: Vec<ImportBinding>,
    pub re_exports: Vec<ReExport>,
    pub assets: Vec<AssetRecord>,
    pub exports: BTreeMap<String, ExportedValue>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AssetMetadata {
    pub label: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AssetRecord {
    pub identity: IdentityId,
    pub metadata: AssetMetadata,
    pub call_start: u32,
    pub call_end: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TypeFact {
    pub start: u32,
    pub end: u32,
    pub value: ExportedValue,
}

impl ModuleSummary {
    pub const SCHEMA: u32 = 1;

    pub fn new(module_path: &str) -> Self {
        Self {
            schema: Self::SCHEMA,
            module_path: normalized_module_path(module_path),
            imports: Vec::new(),
            re_exports: Vec::new(),
            assets: Vec::new(),
            exports: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum SymbolicError {
    #[error("symbolic handle kind mismatch: expected {expected:?}, got {actual:?}")]
    KindMismatch {
        expected: AssetKind,
        actual: AssetKind,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity(kind: AssetKind, start: u32) -> IdentityId {
        IdentityId::new(kind, r"src\Menu.tsx", start, start + 1, "asset")
    }

    #[test]
    fn identities_are_path_stable_across_operating_systems() {
        assert_eq!(
            IdentityId::new(AssetKind::Ref, r"src\Menu.tsx", 4, 8, "menu"),
            IdentityId::new(AssetKind::Ref, "src/Menu.tsx", 4, 8, "menu")
        );
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
