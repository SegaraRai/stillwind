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
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use stillwind_diagnostics::{CompileDiagnostic, CompileError, LintSettings, SourceFailureContext};
use stillwind_source::{
    CssSourceName, Fingerprint, SourceFile, SourceRegistry, SourceRegistryError, Span,
};

use super::{PropertyRegistry, ThemeRegistry, UtilityRegistry, VariantRegistry};
use crate::{
    BuiltinDefinitions, CssFileRequest, CssSourceGraph, DesignSystemLoadError,
    DesignSystemLoadStep, DesignSystemLoader, DesignSystemOptions, ThemeIndex, builtin_definitions,
    builtin_theme_order,
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
                    requests: format!("{requests:?}"),
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
                    requests: format!("{chain:?}"),
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
/// changes to configuration, ordering, lint settings, or specific definition groups.
/// Loading identical CSS produces the same hashes even when revision numbers differ.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DefinitionFingerprints {
    /// Whole-snapshot content, including the compiler version and the selected built-in
    /// CSS.
    pub content: Fingerprint,
    /// Resolved prefix configuration only.
    pub config: Fingerprint,
    /// Output template, theme order, registration positions, and the utility ordering
    /// mode, which decide emitted ordering.
    pub ordering: Fingerprint,
    /// Authored lint severities.
    pub lint_settings: Fingerprint,
    /// Fingerprint of each effective token after namespace resets and overrides. Its
    /// keys match the active token registry.
    pub theme_tokens: BTreeMap<String, Fingerprint>,
    /// One fingerprint per utility name group.
    pub utilities: BTreeMap<String, Fingerprint>,
    /// One fingerprint per variant name group.
    pub variants: BTreeMap<String, Fingerprint>,
    /// One fingerprint per `@property` definition name.
    pub properties: BTreeMap<String, Fingerprint>,
}

/// Stores resolved snapshot data. Construction sets it once, and snapshot methods
/// let callers read it without changing it or taking ownership.
#[derive(Debug)]
struct DesignSystemData {
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
    utility_names: Arc<BTreeSet<String>>,
    /// Contains the names of all effective variant definitions in this snapshot.
    /// The set avoids repeated registry scans. `Arc` lets modules share it
    /// without copying the names.
    variant_names: Arc<BTreeSet<String>>,
    /// Contains the names of all effective `@property` definitions in this snapshot.
    /// The set avoids repeated registry scans. `Arc` lets modules share it
    /// without copying the names.
    property_names: Arc<BTreeSet<String>>,
    /// Retained CSS source graph for diagnostics, mappings, and resources.
    sources: CssSourceGraph,
    /// Effective utility definitions grouped by name.
    utilities: UtilityRegistry,
    /// Effective variant definitions grouped by name.
    variants: VariantRegistry,
    /// Custom-property registrations grouped by name.
    properties: PropertyRegistry,
    /// Effective theme tokens grouped by custom-property name.
    tokens: ThemeRegistry,
    /// Selected built-in definitions, or None when no built-in registry is loaded.
    builtins: Option<Arc<BuiltinDefinitions>>,
    /// Maps property names to templates for serializing authored declarations.
    declaration_formats: BTreeMap<String, String>,
    /// Registered relationship names used to validate relationship-aware candidates.
    relationships: BTreeSet<String>,
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
        match DesignSystemLoader::new(options, SourceRegistry::default()).provide(
            0,
            source_name,
            source.uri().cloned(),
            source.source_map_source().map(Into::into),
            source.text().into(),
        ) {
            DesignSystemLoadStep::Complete(result) => result.map_err(|mut error| {
                // Report errors against the caller's source ID, which can differ from the loader's.
                if let DesignSystemLoadError::Compilation(error) = &mut error {
                    for label in &mut error.labels {
                        label.span.file = source.id();
                    }
                }
                DesignSystemError::Load(error)
            }),
            DesignSystemLoadStep::NeedFile(request) => {
                Err(DesignSystemError::UnresolvedImports { requests: request })
            }
        }
    }

    /// Creates a snapshot from the parsed `theme`, host options in `config`, and `sources`.
    ///
    /// Prefixes in CSS override host defaults before validation. Fingerprints exclude
    /// source IDs and spans so file read order does not change hashes for identical CSS.
    /// The supplied CSS determines built-in definitions, either directly or through imports.
    ///
    /// # Errors
    ///
    /// Returns an error if the final prefix configuration is invalid.
    #[allow(clippy::too_many_lines)] // Resolve and fingerprint one immutable set of semantic registries.
    pub(crate) fn from_loaded_theme(
        theme: ThemeIndex,
        mut config: DesignSystemOptions,
        sources: CssSourceGraph,
    ) -> Result<Self, CompileDiagnostic> {
        static NEXT_REVISION: AtomicU64 = AtomicU64::new(1);
        if let Some(prefix) = &theme.config.prefix {
            config.prefix.clone_from(prefix);
        }
        if let Some(prefix) = &theme.config.tailwind_prefix {
            config.tailwind_prefix.clone_from(prefix);
        }
        config.validate()?;
        // Source IDs are allocated as host reads complete; provenance must not
        // participate in semantic identity or asynchronous load order.
        let mut semantic_template = theme.output_template.clone();
        semantic_template.map_origins(|_| None);
        let config_hash = Fingerprint::of(format!("{config:?}"));
        let ordering = Fingerprint::of(format!(
            "{:?}{:?}{:?}{:?}{:?}",
            semantic_template,
            builtin_theme_order(&theme),
            theme.variant_positions,
            theme.utility_positions,
            theme.config.utility_order,
        ));
        let lint_settings = Fingerprint::of(format!("{:?}", theme.config.lint_settings));
        let builtins = Some(builtin_definitions(&theme));
        let mut declaration_formats = builtins.as_ref().map_or_else(BTreeMap::new, |definitions| {
            definitions.declaration_formats.clone()
        });
        declaration_formats.extend(theme.declaration_formats.clone());
        let mut relationships = builtins.as_ref().map_or_else(BTreeSet::new, |definitions| {
            definitions.relationships.clone()
        });
        relationships.extend(theme.relationship_patterns.clone());
        let utility_registry =
            UtilityRegistry::new(
                builtins
                    .iter()
                    .flat_map(|definitions| {
                        definitions.utilities.iter().map(|definition| {
                            (definition.name.clone(), Arc::new(definition.clone()))
                        })
                    })
                    .chain(
                        theme.utilities.iter().map(|definition| {
                            (definition.name.clone(), Arc::new(definition.clone()))
                        }),
                    ),
            );
        let variant_registry =
            VariantRegistry::new(
                builtins
                    .iter()
                    .flat_map(|definitions| {
                        definitions.variants.iter().map(|definition| {
                            (definition.name.clone(), Arc::new(definition.clone()))
                        })
                    })
                    .chain(
                        theme.variants.iter().map(|definition| {
                            (definition.name.clone(), Arc::new(definition.clone()))
                        }),
                    ),
            );
        let property_registry =
            PropertyRegistry::new(
                builtins
                    .iter()
                    .flat_map(|definitions| {
                        definitions.properties.iter().map(|definition| {
                            (definition.name.clone(), Arc::new(definition.clone()))
                        })
                    })
                    .chain(
                        theme.properties.iter().map(|definition| {
                            (definition.name.clone(), Arc::new(definition.clone()))
                        }),
                    ),
            );
        let token_registry = ThemeRegistry::new(
            theme
                .active_tokens()
                .iter()
                .map(|token| (token.name.value.clone(), Arc::new((*token).clone()))),
        );
        let theme_tokens = theme
            .active_tokens()
            .iter()
            .map(|token| {
                (
                    token.name.value.clone(),
                    Fingerprint::of(format!(
                        "{:?}\0{:?}\0{}\0",
                        token.kind, token.mode, token.value.value
                    )),
                )
            })
            .collect();
        // Include ordered definitions, including duplicate/reset tokens, but never source IDs or spans.
        let tokens: Vec<_> = theme
            .tokens
            .iter()
            .map(|t| (&t.name.value, &t.value.value, &t.kind, t.mode))
            .collect();
        let content = Fingerprint::of(format!(
            "{}\0{config:?}\0{tokens:?}\0{:?}\0{:?}\0{:?}\0{:?}\0{:?}\0{:?}\0{:?}\0{:?}\0{:?}\0{}\0",
            env!("CARGO_PKG_VERSION"),
            theme.utilities,
            theme.variants,
            theme.properties,
            semantic_template,
            theme.config,
            theme.marker_utilities,
            theme.declaration_formats,
            theme.relationship_patterns,
            theme.authored_properties,
            theme.custom_css
        ));
        let content = Fingerprint::of(format!(
            "{}\0{}\0{:?}\0",
            content.0,
            builtins
                .as_ref()
                .map_or("", |definitions| definitions.source.as_str()),
            semantic_template,
        ));
        let utilities = utility_registry.fingerprints();
        let variants = variant_registry.fingerprints();
        let properties = property_registry.fingerprints();
        Ok(Self {
            inner: Arc::new(DesignSystemData {
                revision: DesignSystemRevision(NEXT_REVISION.fetch_add(1, Ordering::Relaxed)),
                theme,
                sources,
                utilities: utility_registry,
                variants: variant_registry,
                properties: property_registry,
                tokens: token_registry,
                builtins,
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
                    lint_settings,
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
    pub fn utility_names(&self) -> &Arc<BTreeSet<String>> {
        &self.inner.utility_names
    }

    /// Returns the shared set of all effective variant definition names.
    /// Callers can clone the `Arc` without copying the names. This snapshot never
    /// changes the set.
    pub fn variant_names(&self) -> &Arc<BTreeSet<String>> {
        &self.inner.variant_names
    }

    /// Returns the shared set of all effective custom-property definition names.
    /// Callers can clone the `Arc` without copying the names. This snapshot never
    /// changes the set.
    pub fn property_names(&self) -> &Arc<BTreeSet<String>> {
        &self.inner.property_names
    }

    /// The resolved prefixes, including CSS-authored overrides of the host defaults.
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

    /// Returns declaration formats after project CSS overrides the selected built-ins.
    pub fn declaration_formats(&self) -> &BTreeMap<String, String> {
        &self.inner.declaration_formats
    }

    /// Returns relationship patterns after project CSS overrides the selected built-ins.
    pub fn relationships(&self) -> &BTreeSet<String> {
        &self.inner.relationships
    }

    /// Authored lint severities, mapped from rule name to `warn`, `deny`, or `off`.
    /// Lint policy never changes generated CSS or generated names.
    pub fn lint_settings(&self) -> &LintSettings {
        &self.inner.theme.config.lint_settings
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

    /// The built-in definition set selected by the loaded CSS. Current construction
    /// returns `Some` even for an empty registry; emptiness is not represented by
    /// `None`.
    pub fn builtins(&self) -> Option<&Arc<BuiltinDefinitions>> {
        self.inner.builtins.as_ref()
    }

    /// The parsed theme, including token spans into the original supplied buffers and
    /// the authored output template.
    pub fn theme(&self) -> &ThemeIndex {
        &self.inner.theme
    }
}
