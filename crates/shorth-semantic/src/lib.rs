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

    /// Returns the least upper bound of two candidate values.
    ///
    /// # Errors
    ///
    /// Returns an error when the values describe different asset kinds.
    pub fn union(&self, other: &Self) -> Result<Self, SymbolicError> {
        if self.kind != other.kind {
            return Err(SymbolicError::KindMismatch {
                expected: self.kind,
                actual: other.kind,
            });
        }
        let candidates = match (&self.candidates, &other.candidates) {
            (Candidates::Universe, _) | (_, Candidates::Universe) => Candidates::Universe,
            (Candidates::Finite(left), Candidates::Finite(right)) => {
                Candidates::Finite(left.union(right).cloned().collect())
            }
        };
        Ok(Self {
            kind: self.kind,
            candidates,
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
    /// # Errors
    ///
    /// Returns an error when alternatives have different asset kinds.
    pub fn union(&self, other: &Self) -> Result<Self, SymbolicError> {
        match (self, other) {
            (Self::Scopes(left), Self::Scopes(right)) => Ok(Self::Scopes(left.union(right)?)),
            (Self::Animations(left), Self::Animations(right)) => {
                Ok(Self::Animations(left.union(right)?))
            }
            (Self::ViewTransitions(left), Self::ViewTransitions(right)) => {
                Ok(Self::ViewTransitions(left.union(right)?))
            }
            (Self::Refs(left), Self::Refs(right)) => {
                let mut alternatives = left.clone();
                for alternative in right {
                    if !alternatives.contains(alternative) {
                        alternatives.push(alternative.clone());
                    }
                }
                alternatives.sort_by(|left, right| left.reference.cmp(&right.reference));
                Ok(Self::Refs(alternatives))
            }
            (left, right) => Err(SymbolicError::KindMismatch {
                expected: left.kind(),
                actual: right.kind(),
            }),
        }
    }

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

/// A resolved graph of source summaries or package manifests.
#[derive(Clone, Debug, Default)]
pub struct ModuleGraph {
    modules: BTreeMap<String, ModuleSummary>,
}

impl ModuleGraph {
    /// # Errors
    ///
    /// Returns an error for unsupported manifest schema versions.
    pub fn new(modules: impl IntoIterator<Item = ModuleSummary>) -> Result<Self, ModuleGraphError> {
        let mut by_path = BTreeMap::new();
        for mut module in modules {
            if module.schema != ModuleSummary::SCHEMA {
                return Err(ModuleGraphError::Schema {
                    module: module.module_path,
                    expected: ModuleSummary::SCHEMA,
                    actual: module.schema,
                });
            }
            module.module_path = normalized_module_path(&module.module_path);
            by_path.insert(module.module_path.clone(), module);
        }
        Ok(Self { modules: by_path })
    }

    pub fn module(&self, module_path: &str) -> Option<&ModuleSummary> {
        self.modules.get(&normalized_module_path(module_path))
    }

    /// Resolves one exported symbolic value through named and star re-exports.
    ///
    /// # Errors
    ///
    /// Returns an error for missing modules/exports, cycles, or incompatible
    /// alternatives merged under one export name.
    pub fn resolve_export(
        &self,
        module_path: &str,
        exported: &str,
    ) -> Result<ExportedValue, ModuleGraphError> {
        self.resolve_export_inner(
            &normalized_module_path(module_path),
            exported,
            &mut BTreeSet::new(),
        )
    }

    /// Resolves all named imports of a module into local symbolic values.
    ///
    /// # Errors
    ///
    /// Returns an error if an edge or requested export cannot be resolved.
    pub fn imported_values(
        &self,
        module_path: &str,
    ) -> Result<BTreeMap<String, ExportedValue>, ModuleGraphError> {
        let module_path = normalized_module_path(module_path);
        let module = self
            .modules
            .get(&module_path)
            .ok_or_else(|| ModuleGraphError::MissingModule(module_path.clone()))?;
        let mut values = BTreeMap::new();
        for import in &module.imports {
            if import.imported == "*" {
                continue;
            }
            let target = self.resolve_specifier(&module_path, &import.source)?;
            values.insert(
                import.local.clone(),
                self.resolve_export(&target, &import.imported)?,
            );
        }
        Ok(values)
    }

    pub fn all_assets(&self) -> BTreeMap<IdentityId, AssetMetadata> {
        self.modules
            .values()
            .flat_map(|module| module.assets.iter())
            .map(|asset| (asset.identity.clone(), asset.metadata.clone()))
            .collect()
    }

    fn resolve_export_inner(
        &self,
        module_path: &str,
        exported: &str,
        visiting: &mut BTreeSet<(String, String)>,
    ) -> Result<ExportedValue, ModuleGraphError> {
        let key = (module_path.to_owned(), exported.to_owned());
        if !visiting.insert(key.clone()) {
            return Err(ModuleGraphError::Cycle {
                module: module_path.to_owned(),
                exported: exported.to_owned(),
            });
        }
        let module = self
            .modules
            .get(module_path)
            .ok_or_else(|| ModuleGraphError::MissingModule(module_path.to_owned()))?;
        let mut value = module.exports.get(exported).cloned();
        for re_export in &module.re_exports {
            let imported = if re_export.exported == exported {
                Some(re_export.imported.as_str())
            } else if re_export.exported == "*" {
                Some(exported)
            } else {
                None
            };
            let Some(imported) = imported else { continue };
            let target = self.resolve_specifier(module_path, &re_export.source)?;
            let alternative = self.resolve_export_inner(&target, imported, visiting)?;
            value = Some(match value {
                Some(value) => value.union(&alternative)?,
                None => alternative,
            });
        }
        visiting.remove(&key);
        value.ok_or_else(|| ModuleGraphError::MissingExport {
            module: module_path.to_owned(),
            exported: exported.to_owned(),
        })
    }

    fn resolve_specifier(&self, from: &str, specifier: &str) -> Result<String, ModuleGraphError> {
        let unresolved = if specifier.starts_with('.') {
            let mut segments = from.split('/').collect::<Vec<_>>();
            segments.pop();
            for segment in specifier.split('/') {
                match segment {
                    "." | "" => {}
                    ".." => {
                        segments.pop();
                    }
                    segment => segments.push(segment),
                }
            }
            segments.join("/")
        } else {
            normalized_module_path(specifier)
        };
        let candidates = [
            unresolved.clone(),
            format!("{unresolved}.ts"),
            format!("{unresolved}.tsx"),
            format!("{unresolved}.js"),
            format!("{unresolved}.jsx"),
            format!("{unresolved}/index.ts"),
            format!("{unresolved}/index.tsx"),
            format!("{unresolved}/index.js"),
            format!("{unresolved}/index.jsx"),
        ];
        candidates
            .into_iter()
            .find(|candidate| self.modules.contains_key(candidate))
            .ok_or(ModuleGraphError::UnresolvedSpecifier {
                from: from.to_owned(),
                specifier: specifier.to_owned(),
            })
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

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ModuleGraphError {
    #[error("unsupported summary schema {actual} for `{module}`; expected {expected}")]
    Schema {
        module: String,
        expected: u32,
        actual: u32,
    },
    #[error("module `{0}` is not present in the Stillwind summary graph")]
    MissingModule(String),
    #[error("cannot resolve `{specifier}` imported by `{from}`")]
    UnresolvedSpecifier { from: String, specifier: String },
    #[error("module `{module}` does not export `{exported}`")]
    MissingExport { module: String, exported: String },
    #[error("cyclic re-export of `{exported}` from `{module}`")]
    Cycle { module: String, exported: String },
    #[error(transparent)]
    Symbolic(#[from] SymbolicError),
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

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

    #[test]
    fn resolves_imports_through_a_re_export_module() {
        let reference = identity(AssetKind::Ref, 8);
        let mut parent = ModuleSummary::new("src/Parent.tsx");
        parent.exports.insert(
            "trigger".to_owned(),
            ExportedValue::Refs(vec![RefAlternative::new(reference.clone(), None).unwrap()]),
        );
        let mut public = ModuleSummary::new("src/public.ts");
        public.re_exports.push(ReExport {
            exported: "menuTrigger".to_owned(),
            source: "./Parent".to_owned(),
            imported: "trigger".to_owned(),
        });
        let mut child = ModuleSummary::new("src/Child.tsx");
        child.imports.push(ImportBinding {
            local: "group".to_owned(),
            source: "./public".to_owned(),
            imported: "menuTrigger".to_owned(),
        });
        let graph = ModuleGraph::new([parent, public, child]).unwrap();

        let imported = graph.imported_values("src/Child.tsx").unwrap();

        assert_eq!(
            imported.get("group"),
            Some(&ExportedValue::Refs(vec![
                RefAlternative::new(reference, None).unwrap()
            ]))
        );
    }

    proptest! {
        #[test]
        fn finite_union_is_commutative_and_idempotent(
            left in prop::collection::btree_set(0_u32..1000, 0..30),
            right in prop::collection::btree_set(0_u32..1000, 0..30),
        ) {
            let left = SymbolicValue::finite(
                AssetKind::Ref,
                left.into_iter().map(|start| identity(AssetKind::Ref, start)),
            ).unwrap();
            let right = SymbolicValue::finite(
                AssetKind::Ref,
                right.into_iter().map(|start| identity(AssetKind::Ref, start)),
            ).unwrap();

            prop_assert_eq!(left.union(&right).unwrap(), right.union(&left).unwrap());
            prop_assert_eq!(left.union(&left).unwrap(), left);
        }
    }
}
