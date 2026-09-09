//! Immutable styling semantics shared by compilation and editor requests.

use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use stillwind_css::{ThemeError, ThemeIndex};
use stillwind_source::SourceFile;

use crate::CompilerOptions;

/// Stable, process-independent content identity (BLAKE3).
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Fingerprint(pub String);

impl Fingerprint {
    pub fn of(content: impl AsRef<[u8]>) -> Self {
        Self(blake3::hash(content.as_ref()).to_hex().to_string())
    }
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

/// A cheaply cloneable, immutable revision. All access is read-only.
#[derive(Clone, Debug)]
pub struct DesignSystemSnapshot {
    inner: Arc<DesignSystemData>,
}

#[derive(Debug)]
struct DesignSystemData {
    revision: DesignSystemRevision,
    theme: ThemeIndex,
    config: CompilerOptions,
    fingerprints: DefinitionFingerprints,
    sources: crate::CssSourceGraph,
}

impl DesignSystemSnapshot {
    /// Parses a single supplied buffer without performing I/O.
    ///
    /// # Errors
    /// Returns invalid CSS or invalid resolved configuration.
    pub fn from_source(source: &SourceFile, options: CompilerOptions) -> Result<Self, ThemeError> {
        match crate::DesignSystemLoader::new(options).provide(
            crate::CssSourceName::from(source.path()),
            source.text().into(),
        ) {
            crate::DesignSystemLoadStep::Complete(result) => {
                result.map_err(|error| ThemeError::Parse(error.to_string()))
            }
            crate::DesignSystemLoadStep::NeedFile(request) => Err(ThemeError::Parse(format!(
                "host must resolve {} imported by {} using DesignSystemLoader",
                request.specifier, request.importer.0
            ))),
        }
    }

    pub(crate) fn from_loaded_theme(
        theme: ThemeIndex,
        mut config: CompilerOptions,
        sources: crate::CssSourceGraph,
    ) -> Result<Self, ThemeError> {
        static NEXT_REVISION: AtomicU64 = AtomicU64::new(1);
        if let Some(prefix) = &theme.config.prefix {
            config.prefix.clone_from(prefix);
        }
        if let Some(prefix) = &theme.config.tailwind_prefix {
            config.tailwind_prefix.clone_from(prefix);
        }
        config.validate().map_err(ThemeError::Parse)?;
        let config_hash = Fingerprint::of(format!("{config:?}"));
        let ordering = Fingerprint::of(format!(
            "{:?}{:?}",
            theme.imports,
            stillwind_tailwind::builtin_theme_order()
        ));
        let lint_policy = Fingerprint::of(format!("{:?}", theme.config.lint_rules));
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
        let utilities = theme
            .utilities
            .iter()
            .map(|definition| {
                (
                    definition.name.clone(),
                    Fingerprint::of(format!("{definition:?}")),
                )
            })
            .collect();
        let variants = theme
            .variants
            .iter()
            .map(|definition| {
                (
                    definition.name.clone(),
                    Fingerprint::of(format!("{definition:?}")),
                )
            })
            .collect();
        let properties = theme
            .properties
            .iter()
            .map(|definition| {
                (
                    definition.name.clone(),
                    Fingerprint::of(format!("{definition:?}")),
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
            theme.imports,
            theme.config,
            theme.marker_utilities,
            theme.declaration_formats,
            theme.relationship_patterns,
            theme.authored_properties,
            theme.custom_css
        ));
        Ok(Self {
            inner: Arc::new(DesignSystemData {
                revision: DesignSystemRevision(NEXT_REVISION.fetch_add(1, Ordering::Relaxed)),
                theme,
                sources,
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
    pub fn config(&self) -> &CompilerOptions {
        &self.inner.config
    }
    pub fn sources(&self) -> &crate::CssSourceGraph {
        &self.inner.sources
    }
    pub(crate) fn theme(&self) -> &ThemeIndex {
        &self.inner.theme
    }

    pub fn generate_react_types(&self) -> String {
        crate::render_react_types(self.theme())
    }
    pub fn render_stylesheet(&self, module_css: &str) -> String {
        crate::render_stylesheet(self.theme(), module_css, self.config())
    }
}
