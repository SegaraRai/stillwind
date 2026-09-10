//! Immutable styling semantics shared by compilation and editor requests.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use stillwind_source::{Fingerprint, SourceFile};

use super::{PropertyRegistry, ThemeRegistry, UtilityRegistry, VariantRegistry};
use crate::{
    BuiltinDefinitions, CssFileRequest, CssSourceGraph, CssSourceName, DesignSystemLoadError,
    DesignSystemLoadStep, DesignSystemLoader, DesignSystemOptions, DesignSystemOptionsError,
    ThemeIndex, builtin_definitions, builtin_theme_order,
};

#[derive(Clone, Debug, thiserror::Error)]
pub enum DesignSystemError {
    #[error("{0}")]
    Load(#[from] DesignSystemLoadError),
    #[error("host must resolve CSS imports using DesignSystemLoader: {requests:?}")]
    UnresolvedImports { requests: Vec<CssFileRequest> },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct DesignSystemRevision(pub u64);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DefinitionFingerprints {
    pub content: Fingerprint,
    pub config: Fingerprint,
    pub ordering: Fingerprint,
    pub lint_policy: Fingerprint,
    pub theme_tokens: BTreeMap<String, Fingerprint>,
    pub utilities: BTreeMap<String, Fingerprint>,
    pub variants: BTreeMap<String, Fingerprint>,
    pub properties: BTreeMap<String, Fingerprint>,
}

#[derive(Debug)]
struct DesignSystemData {
    revision: DesignSystemRevision,
    theme: ThemeIndex,
    config: DesignSystemOptions,
    fingerprints: DefinitionFingerprints,
    sources: CssSourceGraph,
    utilities: UtilityRegistry,
    variants: VariantRegistry,
    properties: PropertyRegistry,
    tokens: ThemeRegistry,
    builtins: Option<Arc<BuiltinDefinitions>>,
    declaration_formats: BTreeMap<String, String>,
    relationships: BTreeSet<String>,
}

/// A cheaply cloneable, immutable revision. All access is read-only.
#[derive(Clone, Debug)]
pub struct DesignSystemSnapshot {
    inner: Arc<DesignSystemData>,
}

impl DesignSystemSnapshot {
    /// Parses a single supplied buffer without performing I/O.
    ///
    /// # Errors
    /// Returns invalid CSS or invalid resolved configuration.
    pub fn from_source(
        source: &SourceFile,
        options: DesignSystemOptions,
    ) -> Result<Self, DesignSystemError> {
        match DesignSystemLoader::new(options).provide(
            0,
            CssSourceName::from(source.path()),
            source.text().into(),
        ) {
            DesignSystemLoadStep::Complete(result) => result.map_err(DesignSystemError::Load),
            DesignSystemLoadStep::NeedFile(request) => {
                Err(DesignSystemError::UnresolvedImports { requests: request })
            }
        }
    }

    #[allow(clippy::too_many_lines)] // Resolve and fingerprint one immutable set of semantic registries.
    pub(crate) fn from_loaded_theme(
        theme: ThemeIndex,
        mut config: DesignSystemOptions,
        sources: CssSourceGraph,
    ) -> Result<Self, DesignSystemOptionsError> {
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
            "{:?}{:?}",
            semantic_template,
            builtin_theme_order(&theme)
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
            .tokens
            .iter()
            .map(|token| {
                (
                    token.name.value.clone(),
                    Fingerprint::of(format!(
                        "{:?}:{:?}:{}",
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
            "{}|{config:?}|{tokens:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{}",
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
            "{}|{}|{:?}",
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

    pub fn revision(&self) -> DesignSystemRevision {
        self.inner.revision
    }

    pub fn fingerprints(&self) -> &DefinitionFingerprints {
        &self.inner.fingerprints
    }

    pub fn config(&self) -> &DesignSystemOptions {
        &self.inner.config
    }

    pub fn sources(&self) -> &CssSourceGraph {
        &self.inner.sources
    }

    pub fn utilities(&self) -> &UtilityRegistry {
        &self.inner.utilities
    }

    pub fn declaration_formats(&self) -> &BTreeMap<String, String> {
        &self.inner.declaration_formats
    }

    pub fn relationships(&self) -> &BTreeSet<String> {
        &self.inner.relationships
    }

    pub fn lint_policy(&self) -> &BTreeMap<String, String> {
        &self.inner.theme.config.lint_rules
    }

    pub fn variants(&self) -> &VariantRegistry {
        &self.inner.variants
    }

    pub fn properties(&self) -> &PropertyRegistry {
        &self.inner.properties
    }

    pub fn tokens(&self) -> &ThemeRegistry {
        &self.inner.tokens
    }

    pub fn builtins(&self) -> Option<&Arc<BuiltinDefinitions>> {
        self.inner.builtins.as_ref()
    }

    pub fn theme(&self) -> &ThemeIndex {
        &self.inner.theme
    }
}
