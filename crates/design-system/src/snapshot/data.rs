//! Immutable styling semantics shared by compilation and editor requests.
//!
//! [`DesignSystemSnapshot`] is an `Arc` handle to one fully resolved revision. Cloning
//! it is cheap and never copies registries, and no method on it mutates shared state,
//! so a request may pin one clone and stay consistent even while another thread
//! publishes a replacement.
//!
//! A revision number identifies one successful load inside the process. Fingerprints
//! identify content, so caches that outlive the process must key on fingerprints; in
//! particular a fingerprint must not include source IDs or spans, which vary with host
//! read order for identical content.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use stillwind_diagnostics::{CompileError, CompileErrorKind, LintSeverity, SourceFailureContext};
use stillwind_source::{
    CssSourceName, Fingerprint, SourceFile, SourceRegistry, SourceRegistryError, Span,
};

use super::{PropertyRegistry, ThemeRegistry, UtilityRegistry, VariantRegistry};
use crate::{
    BuiltinDefinitions, CssFileRequest, CssSourceGraph, DesignSystemLoadError,
    DesignSystemLoadStep, DesignSystemLoader, DesignSystemOptions, ThemeIndex, builtin_definitions,
    builtin_theme_order,
};

/// Why a snapshot could not be produced from an already supplied buffer.
///
/// A host resolution or read failure is not represented here: the loader reports its
/// own protocol errors, and the host keeps resolution failures.
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
    /// Converts a snapshot failure into a source-located compilation error.
    ///
    /// Existing compilation errors retain their cause, labels, source name, and notes.
    /// Other failures receive `span` as their primary label; it must be a valid location
    /// in the caller's source. The fallback does not rebind existing labels.
    pub fn into_compile_error(self, span: Span) -> CompileError {
        let kind = match self {
            DesignSystemError::UnresolvedImports { requests } => {
                CompileErrorKind::UnresolvedCssImports {
                    requests: format!("{requests:?}"),
                }
            }
            DesignSystemError::Load(error) => match error {
                DesignSystemLoadError::Compilation(error) => {
                    return error;
                }
                DesignSystemLoadError::Registry(SourceRegistryError::FileIdExhausted) => {
                    CompileErrorKind::CssSourceIdExhausted
                }
                DesignSystemLoadError::InvalidGraph | DesignSystemLoadError::Registry(_) => {
                    CompileErrorKind::InvalidCssGraph
                }
                DesignSystemLoadError::Source(error) => {
                    CompileErrorKind::source_failure(&error, SourceFailureContext::Stylesheet)
                }
                DesignSystemLoadError::RequestIdExhausted => {
                    CompileErrorKind::CssRequestIdExhausted
                }
                DesignSystemLoadError::AlreadyComplete => CompileErrorKind::CssLoadAlreadyComplete,
                DesignSystemLoadError::InvalidInitialBatch { count } => {
                    CompileErrorKind::InvalidCssInitialBatch { count }
                }
                DesignSystemLoadError::UnknownOrDuplicateRequest { id } => {
                    CompileErrorKind::UnknownCssRequest { id }
                }
                DesignSystemLoadError::ConflictingSource { name } => {
                    CompileErrorKind::ConflictingCssSource { name }
                }
                DesignSystemLoadError::InvalidOptions(error) => error,
                DesignSystemLoadError::Cycle { chain } => CompileErrorKind::CssImportCycle {
                    requests: format!("{chain:?}"),
                },
            },
        };
        CompileError::new(kind, span)
    }
}

/// Process-local identity of one successful load.
///
/// Values increase monotonically within a process and are only meaningful for comparing
/// two live revisions; persist [`DefinitionFingerprints`] instead.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct DesignSystemRevision(
    /// Monotonic process-local revision number; use content fingerprints for persistence.
    pub u64,
);

/// Content fingerprints of independently invalidatable semantic groups.
///
/// `content` covers the whole resolved snapshot and is the fingerprint a cache key
/// should use. The other fields narrow an invalidation to configuration, ordering, lint
/// policy, or one definition group. They are content-derived, so two loads of identical
/// CSS agree even when their revisions differ.
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
    pub lint_policy: Fingerprint,
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

/// The resolved data behind a snapshot. Never mutated after construction, and never
/// handed out by value: callers reach it through accessors on the snapshot.
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
    /// Property-name lookup of authored declaration serialization templates.
    declaration_formats: BTreeMap<String, String>,
    /// Registered relationship names used to validate relationship-aware candidates.
    relationships: BTreeSet<String>,
}

/// A cheaply cloneable, immutable revision. All access is read-only.
///
/// Cloning shares the same resolved registries; creating a new revision is the only way
/// to change semantics, and it never alters a snapshot already in use.
#[derive(Clone, Debug)]
pub struct DesignSystemSnapshot {
    /// Shared immutable data.
    inner: Arc<DesignSystemData>,
}

impl DesignSystemSnapshot {
    /// Parses a single supplied buffer without performing I/O.
    ///
    /// This is the convenience path for a stylesheet with no host-resolved imports. A
    /// buffer that does contain imports is rejected with
    /// [`DesignSystemError::UnresolvedImports`] rather than silently dropping them;
    /// drive [`DesignSystemLoader`] instead when the host must resolve them.
    ///
    /// # Errors
    ///
    /// Returns invalid CSS, unresolved imports, or invalid resolved configuration.
    /// Compilation-error labels use the supplied source identity rather than loader-local
    /// file IDs, and the display source name is retained. Snapshot source-graph IDs remain
    /// loader-local; this rebinding applies only to errors returned from this entry point.
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
                // A single-buffer caller owns the source identity; loader IDs are local.
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

    /// Resolves one parsed theme, its supplied source graph, and the host options into
    /// a snapshot.
    ///
    /// CSS-authored prefixes override the host defaults before validation, so the
    /// stored configuration is the resolved one. Source IDs and spans are deliberately
    /// excluded from every fingerprint: they depend on the order in which host reads
    /// completed, and identical content must fingerprint identically regardless of that
    /// order. Built-ins are parsed from the supplied theme CSS rather than injected;
    /// that CSS may declare a registry directly or receive one through imports.
    ///
    /// # Errors
    ///
    /// Returns an invalid resolved prefix configuration.
    #[allow(clippy::too_many_lines)] // Resolve and fingerprint one immutable set of semantic registries.
    pub(crate) fn from_loaded_theme(
        theme: ThemeIndex,
        mut config: DesignSystemOptions,
        sources: CssSourceGraph,
    ) -> Result<Self, CompileErrorKind> {
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
        let lint_policy = Fingerprint::of(format!("{:?}", theme.config.lint_rules));
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
                fingerprints: DefinitionFingerprints {
                    content,
                    config: config_hash,
                    ordering,
                    lint_policy,
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
    /// This is process-local load identity. Two successful loads of identical CSS still
    /// differ here; compare [`Self::fingerprints`] for content equality.
    pub fn revision(&self) -> DesignSystemRevision {
        self.inner.revision
    }

    /// Stable content hashes of the snapshot and its invalidatable groups.
    ///
    /// Use these to key caches and to compare two revisions; the revision number is
    /// process-local and must not be persisted.
    pub fn fingerprints(&self) -> &DefinitionFingerprints {
        &self.inner.fingerprints
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

    /// Declaration-format metadata after project CSS overrides are merged over the
    /// selected built-ins.
    pub fn declaration_formats(&self) -> &BTreeMap<String, String> {
        &self.inner.declaration_formats
    }

    /// Relationship patterns after project CSS overrides are merged over the selected
    /// built-ins.
    pub fn relationships(&self) -> &BTreeSet<String> {
        &self.inner.relationships
    }

    /// Authored lint severities, mapped from rule name to `warn`, `deny`, or `off`.
    /// Lint policy never changes generated CSS or generated names.
    pub fn lint_policy(&self) -> &BTreeMap<String, LintSeverity> {
        &self.inner.theme.config.lint_rules
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
