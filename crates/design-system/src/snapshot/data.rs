//! Provides design-system snapshots for compilation and editor requests.
//!
//! [`DesignSystemSnapshot`] keeps one resolved revision through `Arc`. Cloning it
//! does not copy registries. A request can keep a clone while another thread loads
//! a new revision. Existing snapshots stay unchanged.
//!
//! Revision numbers identify successful loads within a process. Content fingerprints
//! are used for caches kept across processes. Snapshot fingerprints exclude source IDs
//! and spans because these can vary with file read order even when the content is
//! identical.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};

use lean_string::{LeanStr, ToLeanStr, ToLeanString};

use stillwind_diagnostics::{CompileDiagnostic, CompileError, SourceFailureContext};
use stillwind_source::{CssSourceName, Fingerprint, SourceFile, SourceRegistryError, Span};

use super::analysis::SnapshotAnalyses;
use super::{
    PropertyRegistry, ThemeRegistry, UtilityRegistry, UtilitySearchIndex, VariantRegistry,
};
use crate::{
    BuiltinDefinitions, CssFileRequest, CssSourceGraph, DesignSystemLoadError,
    DesignSystemLoadStep, DesignSystemLoader, DesignSystemOptions, ThemeIndex, builtin_theme_order,
};

/// Reports why a snapshot could not be created from supplied CSS.
///
/// The loader reports parsing and protocol errors. The host handles failures to resolve
/// or read files separately.
#[derive(Clone, Debug, thiserror::Error)]
pub enum DesignSystemError {
    /// Loading failed; see [`DesignSystemLoadError`].
    #[error("{0}")]
    Load(
        /// Loader failure for the supplied CSS graph.
        #[from]
        DesignSystemLoadError,
    ),
    /// The buffer is valid but imports sources the host must resolve first.
    #[error("host must resolve CSS imports using DesignSystemLoader: {requests:?}")]
    UnresolvedImports {
        /// Imports that must be resolved before the buffer can be loaded.
        requests: Vec<CssFileRequest>,
    },
}

impl DesignSystemError {
    /// Converts a snapshot failure into a compilation error with a source location.
    ///
    /// Existing compilation errors retain their cause, labels, source name, and notes.
    /// Other failures use `span` as their primary label. It must be a valid location
    /// in the caller's source. Existing labels are left unchanged.
    pub fn into_compile_error(self, span: Span) -> CompileError {
        let kind = match self {
            DesignSystemError::UnresolvedImports { requests } => {
                CompileDiagnostic::UnresolvedCssImports {
                    requests: format_args!("{requests:?}").to_lean_str(),
                }
            }
            DesignSystemError::Load(error) => match error {
                DesignSystemLoadError::Compilation(error) => {
                    return error;
                }
                DesignSystemLoadError::Registry(SourceRegistryError::FileIdExhausted) => {
                    CompileDiagnostic::CssSourceIdExhausted
                }
                DesignSystemLoadError::InvalidGraph | DesignSystemLoadError::Registry(_) => {
                    CompileDiagnostic::InvalidCssGraph
                }
                DesignSystemLoadError::Source(error) => {
                    CompileDiagnostic::source_failure(&error, SourceFailureContext::Stylesheet)
                }
                DesignSystemLoadError::RequestIdExhausted => {
                    CompileDiagnostic::CssRequestIdExhausted
                }
                DesignSystemLoadError::AlreadyComplete => CompileDiagnostic::CssLoadAlreadyComplete,
                DesignSystemLoadError::InvalidInitialBatch { count } => {
                    CompileDiagnostic::InvalidCssInitialBatch { count }
                }
                DesignSystemLoadError::UnknownOrDuplicateRequest { id } => {
                    CompileDiagnostic::UnknownCssRequest { id }
                }
                DesignSystemLoadError::ConflictingSource { name } => {
                    CompileDiagnostic::ConflictingCssSource { name }
                }
                DesignSystemLoadError::InvalidOptions(error) => error,
                DesignSystemLoadError::Cycle { chain } => CompileDiagnostic::CssImportCycle {
                    requests: format_args!("{chain:?}").to_lean_str(),
                },
            },
        };
        CompileError::new(kind, span)
    }
}

/// Identifies one successful load within a process.
///
/// Values increase with each load. Use them only to compare revisions in the same
/// process. Store [`DefinitionFingerprints`] when comparisons must survive a restart.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct DesignSystemRevision(
    /// Monotonic process-local revision number; use content fingerprints for
    /// persistence.
    pub u64,
);

/// Records content hashes for the whole snapshot and its definition groups.
///
/// Use `content` in cache keys to track the whole snapshot. Other fields identify
/// changes to configuration, ordering, or specific definition groups.
/// Loading identical CSS produces the same hashes even when revision numbers differ.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DefinitionFingerprints {
    /// Whole-snapshot content, including the compiler version and the selected built-in
    /// CSS.
    pub content: Fingerprint,
    /// Host-supplied prefixes and utility ordering.
    pub config: Fingerprint,
    /// Output template, theme order, registration positions, and the utility ordering
    /// mode, which decide emitted ordering.
    pub ordering: Fingerprint,
    /// Fingerprint of each effective token after namespace resets and overrides. Its
    /// keys match the active token registry.
    pub theme_tokens: BTreeMap<LeanStr, Fingerprint>,
    /// One fingerprint per utility name group.
    pub utilities: BTreeMap<LeanStr, Fingerprint>,
    /// One fingerprint per variant name group.
    pub variants: BTreeMap<LeanStr, Fingerprint>,
    /// One fingerprint per `@property` definition name.
    pub properties: BTreeMap<LeanStr, Fingerprint>,
}

/// Stores resolved snapshot data. Construction sets it once, and snapshot methods
/// let callers read it without changing it or taking ownership.
#[derive(Debug)]
struct DesignSystemData {
    /// Analyses keyed by settings and shared for this resolved revision.
    analyses: SnapshotAnalyses,
    /// Process-local identity allocated for this successful snapshot.
    revision: DesignSystemRevision,
    /// Resolved theme and authored output template used by evaluators.
    theme: ThemeIndex,
    /// Validated generated-name prefix configuration.
    config: DesignSystemOptions,
    /// Content identities for cache reuse and selective invalidation.
    fingerprints: DefinitionFingerprints,
    /// Contains the names of all effective utility definitions in this snapshot.
    /// The set avoids repeated registry scans. `Arc` lets modules share it
    /// without copying the names.
    utility_names: Arc<BTreeSet<LeanStr>>,
    /// Contains the names of all effective variant definitions in this snapshot.
    /// The set avoids repeated registry scans. `Arc` lets modules share it
    /// without copying the names.
    variant_names: Arc<BTreeSet<LeanStr>>,
    /// Contains the names of all effective `@property` definitions in this snapshot.
    /// The set avoids repeated registry scans. `Arc` lets modules share it
    /// without copying the names.
    property_names: Arc<BTreeSet<LeanStr>>,
    /// Retained CSS source graph for diagnostics, mappings, and resources.
    sources: CssSourceGraph,
    /// Effective utility definitions grouped by name.
    utilities: UtilityRegistry,
    /// Lazily parsed search metadata shared by every clone of this snapshot.
    utility_search: OnceLock<UtilitySearchIndex>,
    /// Effective variant definitions grouped by name.
    variants: VariantRegistry,
    /// Custom-property registrations grouped by name.
    properties: PropertyRegistry,
    /// Effective theme tokens grouped by custom-property name.
    tokens: ThemeRegistry,
    /// Maps property names to templates for serializing authored declarations.
    declaration_formats: BTreeMap<LeanStr, LeanStr>,
    /// Registered relationship names used to validate relationship-aware candidates.
    relationships: BTreeSet<LeanStr>,
}

/// Stores one resolved design-system revision that callers can read and share.
///
/// Clones share the registries through `Arc`. Loading new definitions creates a new
/// revision without changing snapshots already in use.
#[derive(Clone, Debug)]
pub struct DesignSystemSnapshot {
    /// Shared immutable data.
    inner: Arc<DesignSystemData>,
}

impl DesignSystemSnapshot {
    /// Parses a single supplied buffer without performing I/O.
    ///
    /// `source` contains the CSS text and its document identity. `source_name` names it
    /// for CSS resolution and error messages. `options` configures the design system.
    /// Use [`DesignSystemLoader`] when the host needs to resolve imports.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid CSS or configuration. Imports return
    /// [`DesignSystemError::UnresolvedImports`]. Compilation errors use `source`'s
    /// file ID and keep the display source name. Sources stored in the snapshot keep
    /// the loader's file IDs.
    pub fn from_source(
        source: &SourceFile,
        source_name: CssSourceName,
        options: DesignSystemOptions,
    ) -> Result<Self, DesignSystemError> {
        match DesignSystemLoader::with_source(options, Arc::new(source.clone())).provide(
            0,
            source_name,
            source.uri().cloned(),
            source.source_map_source().map(Into::into),
            source.text().into(),
        ) {
            DesignSystemLoadStep::Complete(result) => result.map_err(DesignSystemError::Load),
            DesignSystemLoadStep::NeedFile(request) => {
                Err(DesignSystemError::UnresolvedImports { requests: request })
            }
        }
    }

    /// Creates a snapshot from the parsed `theme`, host options in `config`, and `sources`.
    ///
    /// Host options are validated before construction. Fingerprints exclude
    /// source IDs and spans so file read order does not change hashes for identical CSS.
    /// The supplied CSS determines built-in definitions, either directly or through imports.
    ///
    /// # Errors
    ///
    /// Returns an error if the final prefix configuration is invalid.
    #[allow(clippy::too_many_lines)] // Resolve and fingerprint one immutable set of semantic registries.
    pub(crate) fn from_loaded_theme(
        theme: ThemeIndex,
        config: DesignSystemOptions,
        sources: CssSourceGraph,
    ) -> Result<Self, CompileDiagnostic> {
        static NEXT_REVISION: AtomicU64 = AtomicU64::new(1);
        config.validate()?;
        // Source IDs are allocated as host reads complete; provenance must not
        // participate in semantic identity or asynchronous load order.
        let mut semantic_template = theme.output_template().clone();
        semantic_template.map_origins(|_| None);
        let config_hash = Fingerprint::of(format_args!("{config:?}").to_lean_string());
        let ordering = Fingerprint::of(
            format_args!(
                "{:?}{:?}{:?}{:?}{:?}",
                semantic_template,
                builtin_theme_order(&theme),
                theme.variant_positions(),
                theme.utility_positions(),
                theme.utility_order(),
            )
            .to_lean_string(),
        );
        let builtins = theme.registry();
        let mut declaration_formats = builtins.declaration_formats().clone();
        declaration_formats.extend(theme.declaration_formats().clone());
        let mut relationships = builtins.relationships().clone();
        relationships.extend(theme.relationship_patterns().clone());
        let utility_registry = UtilityRegistry::new(
            builtins
                .utilities()
                .iter()
                .chain(theme.utilities())
                .map(|definition| (definition.name.clone(), Arc::clone(definition))),
        );
        let variant_registry = VariantRegistry::new(
            builtins
                .variants()
                .iter()
                .chain(theme.variants())
                .map(|definition| (definition.name.clone(), Arc::clone(definition))),
        );
        let property_registry = PropertyRegistry::new(
            builtins
                .properties()
                .iter()
                .chain(theme.properties())
                .map(|definition| (definition.name.as_str().into(), Arc::clone(definition))),
        );
        let token_registry = ThemeRegistry::new(
            theme
                .active_token_definitions()
                .map(|token| (token.name.value.as_str().into(), Arc::clone(token))),
        );
        let theme_tokens = theme
            .active_tokens()
            .iter()
            .map(|token| {
                (
                    token.name.value.clone(),
                    Fingerprint::of(
                        format_args!(
                            "{:?}\0{:?}\0{:?}\0{}\0",
                            token.kind, token.mode, token.precedence, token.value.value
                        )
                        .to_lean_string(),
                    ),
                )
            })
            .collect();
        // Include ordered definitions, including duplicate/reset tokens, but never source IDs or spans.
        let tokens: Vec<_> = theme
            .tokens()
            .iter()
            .map(|t| (&t.name.value, &t.value.value, &t.kind, t.mode, t.precedence))
            .collect();
        let content = Fingerprint::of(format_args!(
            "{}\0{config:?}\0{tokens:?}\0{:?}\0{:?}\0{:?}\0{:?}\0{:?}\0{:?}\0{:?}\0{:?}\0{:?}\0{}\0",
            env!("CARGO_PKG_VERSION"),
            theme.utilities(),
            theme.variants(),
            theme.properties(),
            semantic_template,
            theme.utility_order(),
            theme.marker_utilities(),
            theme.declaration_formats(),
            theme.relationship_patterns(),
            theme.authored_properties(),
            theme.custom_css()
        ).to_lean_string());
        let mut css_templates = theme.css_templates().clone();
        for template in css_templates.values_mut() {
            template.map_origins(|_| None);
        }
        let content = Fingerprint::of(
            format_args!(
                "{}\0{}\0{:?}\0{:?}\0{:?}\0",
                content.0,
                builtins.source(),
                semantic_template,
                css_templates,
                theme.dynamic_utilities(),
            )
            .to_lean_string(),
        );
        let utilities = utility_registry.fingerprints();
        let variants = variant_registry.fingerprints();
        let properties = property_registry.fingerprints();
        Ok(Self {
            inner: Arc::new(DesignSystemData {
                analyses: SnapshotAnalyses::default(),
                revision: DesignSystemRevision(NEXT_REVISION.fetch_add(1, Ordering::Relaxed)),
                theme,
                sources,
                utilities: utility_registry,
                utility_search: OnceLock::new(),
                variants: variant_registry,
                properties: property_registry,
                tokens: token_registry,
                declaration_formats,
                relationships,
                config,
                utility_names: Arc::new(utilities.keys().cloned().collect()),
                variant_names: Arc::new(variants.keys().cloned().collect()),
                property_names: Arc::new(properties.keys().cloned().collect()),
                fingerprints: DefinitionFingerprints {
                    content,
                    config: config_hash,
                    ordering,
                    theme_tokens,
                    utilities,
                    variants,
                    properties,
                },
            }),
        })
    }

    /// Revision of the snapshot's load, for cache identity within one process.
    ///
    /// Two successful loads have different revision numbers even if their CSS matches.
    /// Compare [`Self::fingerprints`] to check content equality.
    pub fn revision(&self) -> DesignSystemRevision {
        self.inner.revision
    }

    /// Returns stable content hashes for the snapshot and its definition groups.
    ///
    /// Use these hashes for cache keys and to compare revisions. Revision numbers are
    /// valid only within one process and must not be stored for later comparisons.
    pub fn fingerprints(&self) -> &DefinitionFingerprints {
        &self.inner.fingerprints
    }

    /// Returns the shared set of all effective utility definition names.
    /// Callers can clone the `Arc` without copying the names. This snapshot never
    /// changes the set.
    pub fn utility_names(&self) -> &Arc<BTreeSet<LeanStr>> {
        &self.inner.utility_names
    }

    /// Returns the shared set of all effective variant definition names.
    /// Callers can clone the `Arc` without copying the names. This snapshot never
    /// changes the set.
    pub fn variant_names(&self) -> &Arc<BTreeSet<LeanStr>> {
        &self.inner.variant_names
    }

    /// Returns the shared set of all effective custom-property definition names.
    /// Callers can clone the `Arc` without copying the names. This snapshot never
    /// changes the set.
    pub fn property_names(&self) -> &Arc<BTreeSet<LeanStr>> {
        &self.inner.property_names
    }

    /// The host-supplied prefixes and utility ordering captured by this snapshot.
    pub fn config(&self) -> &DesignSystemOptions {
        &self.inner.config
    }

    /// Every source the host supplied, with its resolved edges and the expansion
    /// mapping. Hosts use this to watch files and to attribute generated CSS.
    pub fn sources(&self) -> &CssSourceGraph {
        &self.inner.sources
    }

    /// Utilities from the selected built-ins and from the project CSS, grouped by name.
    pub fn utilities(&self) -> &UtilityRegistry {
        &self.inner.utilities
    }

    /// Returns shared metadata for searching utilities by property or name prefix.
    /// The first call builds the index. Later calls and snapshot clones reuse it.
    pub fn utility_search(&self) -> &UtilitySearchIndex {
        self.inner
            .utility_search
            .get_or_init(|| UtilitySearchIndex::new(&self.inner.utilities))
    }

    /// Returns declaration formats after project CSS overrides the selected built-ins.
    pub fn declaration_formats(&self) -> &BTreeMap<LeanStr, LeanStr> {
        &self.inner.declaration_formats
    }

    /// Returns relationship patterns after project CSS overrides the selected built-ins.
    pub fn relationships(&self) -> &BTreeSet<LeanStr> {
        &self.inner.relationships
    }

    /// Variants from the selected built-ins and from the project CSS, grouped by name.
    pub fn variants(&self) -> &VariantRegistry {
        &self.inner.variants
    }

    /// `@stillwind-property` definitions from the selected built-ins and the project
    /// CSS.
    pub fn properties(&self) -> &PropertyRegistry {
        &self.inner.properties
    }

    /// The effective theme tokens after namespace resets, keyed by custom property
    /// name.
    pub fn tokens(&self) -> &ThemeRegistry {
        &self.inner.tokens
    }

    /// The built-in definitions selected by the loaded CSS; empty when it imports none.
    pub fn builtins(&self) -> &Arc<BuiltinDefinitions> {
        self.inner.theme.registry()
    }

    /// Prepares and shares an analysis for this revision and `key`.
    /// The key must include every setting that affects `init`. The initializer runs
    /// once across snapshot clones. Dependencies between analyses must not form cycles.
    /// The result must not retain this snapshot, which would form an ownership cycle.
    pub fn analysis<T: std::any::Any + Send + Sync>(
        &self,
        key: Fingerprint,
        init: impl FnOnce() -> T,
    ) -> Arc<T> {
        self.inner.analyses.get_or_init(key, init)
    }

    /// The parsed theme, including token spans into the original supplied buffers and
    /// the authored output template.
    pub fn theme(&self) -> &ThemeIndex {
        &self.inner.theme
    }
}

#[cfg(test)]
mod tests {
    use stillwind_source::FileId;

    use super::*;

    #[test]
    fn name_groups_share_authored_definitions_and_effective_tokens() {
        let source = SourceFile::new(
            FileId(1),
            None,
            None,
            "@theme { --color-brand:red; } @utility brand { color:red; }",
        )
        .unwrap();
        let snapshot = DesignSystemSnapshot::from_source(
            &source,
            CssSourceName::from("theme.css"),
            DesignSystemOptions::default(),
        )
        .unwrap();
        let utility = &snapshot.utilities().get("brand").unwrap()[0];
        assert!(Arc::ptr_eq(utility, &snapshot.theme().utilities()[0]));
        let token = &snapshot.tokens().get("--color-brand").unwrap()[0];
        assert!(Arc::ptr_eq(token, &snapshot.theme().tokens()[0]));
    }
}
