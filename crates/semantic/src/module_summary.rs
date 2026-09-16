//! Per-module facts published for other modules and for the host.
//!
//! A [`ModuleSummary`] answers what a later module or the host runtime asks about one
//! module: its imports and re-exports, the assets declared in it, what each export
//! denotes symbolically, and which string literals name other modules. It is
//! serializable so the host can keep summaries across runs and shadow a module it does
//! not want to parse again.
//!
//! Spans in a summary belong to the module's original (editable) source, never to a
//! projected shadow, and [`ModuleSummary::module_path`] is stored normalized. The
//! [`ModuleSummary::schema`] field versions this payload for hosts; adding fields is a
//! schema change.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use stillwind_source::normalized_module_path;

use super::assets::AssetRecord;
use super::identity::IdentityId;
use super::symbolic::{Candidates, ExportedValue};

/// One name bound by an `import` declaration.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ImportBinding {
    /// Local name in the importing module.
    pub local: String,
    /// Decoded module specifier value; never resolved here.
    pub source: String,
    /// Name imported from that module: the exported name, `default` for a default
    /// import, or `*` for a namespace import.
    pub imported: String,
}

/// One name re-exported from another module by an `export ... from` declaration.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ReExport {
    /// Name visible to importers of this module.
    pub exported: String,
    /// Decoded module specifier value; never resolved here.
    pub source: String,
    /// Name imported from that module (`default` or `*` as in [`ImportBinding`]).
    pub imported: String,
}

/// Exact string-literal span for host-authoritative shadow module resolution.
///
/// The host, not this crate, resolves [`Self::source`] to a file. The span lets the
/// host find the literal it must rewrite when it substitutes a shadow module, so it
/// covers the literal in the original source rather than any projected copy.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ModuleSpecifier {
    /// Decoded string-literal value, without quotes or source escape spelling.
    pub source: String,
    /// Byte offset of the literal start in the original source.
    pub start: u32,
    /// Byte offset of the literal end, exclusive.
    pub end: u32,
}

/// Everything one module publishes about itself.
///
/// Ordering is field-specific: module specifiers, imports and re-exports retain
/// source order; assets are grouped by kind; exports are keyed by name. Demand import
/// lists are sorted by `(source, imported, local)` by the Oxc frontend.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ModuleSummary {
    /// String literals naming other modules, in source order.
    #[serde(default)]
    pub module_specifiers: Vec<ModuleSpecifier>,
    /// Schema tag; see `SCHEMA`.
    pub schema: u32,
    /// Normalized project-relative path of this module.
    pub module_path: String,
    /// Names bound by `import` declarations.
    pub imports: Vec<ImportBinding>,
    /// Names re-exported from other modules.
    pub re_exports: Vec<ReExport>,
    /// Assets declared in this module.
    pub assets: Vec<AssetRecord>,
    /// What each exported name denotes, keyed by exported name.
    pub exports: BTreeMap<String, ExportedValue>,
    /// Import bindings reachable from type probes through local declarations.
    #[serde(default)]
    pub probe_imports: Vec<ImportBinding>,
    /// Import bindings needed by each explicitly exported local variable.
    #[serde(default)]
    pub export_imports: BTreeMap<String, Vec<ImportBinding>>,
}

impl ModuleSummary {
    /// Schema version hosts must understand before reading a summary.
    pub const SCHEMA: u32 = 1;

    /// Creates an empty summary for [`Self::module_path`], normalizing the path and
    /// stamping the current schema.
    pub fn new(module_path: &str) -> Self {
        Self {
            module_specifiers: Vec::new(),
            schema: Self::SCHEMA,
            module_path: normalized_module_path(module_path),
            imports: Vec::new(),
            re_exports: Vec::new(),
            assets: Vec::new(),
            exports: BTreeMap::new(),
            probe_imports: Vec::new(),
            export_imports: BTreeMap::new(),
        }
    }

    /// Every asset identity the module's exports can reach.
    ///
    /// Finite candidates are exact roots. An exported universe conservatively retains all
    /// owned declarations of that kind, because the exporter cannot prove them unreachable.
    pub fn export_reachable_identities(&self) -> BTreeSet<IdentityId> {
        let mut identities = BTreeSet::new();
        for value in self.exports.values() {
            match value {
                ExportedValue::Refs(alternatives) => {
                    for alternative in alternatives {
                        identities.insert(alternative.reference.clone());
                        if let Some(subtrees) = &alternative.permitted_subtrees
                            && let Candidates::Finite(subtrees) = &subtrees.candidates
                        {
                            identities.extend(subtrees.iter().cloned());
                        }
                    }
                }
                ExportedValue::Styles(value)
                | ExportedValue::Subtrees(value)
                | ExportedValue::Keyframes(value)
                | ExportedValue::ViewTransitions(value)
                | ExportedValue::ViewTransitionKeys(value) => {
                    if let Candidates::Finite(candidates) = &value.candidates {
                        identities.extend(candidates.iter().cloned());
                    } else {
                        identities.extend(
                            self.assets
                                .iter()
                                .filter(|asset| asset.identity.kind == value.kind)
                                .map(|asset| asset.identity.clone()),
                        );
                    }
                }
            }
        }
        identities
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets::{AssetMetadata, AssetRecord};
    use crate::identity::{AssetKind, IdentityId};
    use crate::symbolic::SymbolicValue;

    #[test]
    fn exported_universe_retains_owned_assets_of_its_kind() {
        let mut summary = ModuleSummary::new("x");
        let animation = IdentityId::new(AssetKind::Keyframes, "x", 0, 1, "a");
        summary.assets.push(AssetRecord {
            identity: animation.clone(),
            metadata: AssetMetadata { label: None },
            call_start: 2,
            call_end: 8,
        });
        summary.exports.insert(
            "chosen".into(),
            ExportedValue::Keyframes(SymbolicValue::universe(AssetKind::Keyframes)),
        );
        assert_eq!(
            summary.export_reachable_identities(),
            BTreeSet::from([animation])
        );
    }
}
