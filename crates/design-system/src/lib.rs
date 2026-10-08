//! CSS-defined styling semantics, caller-driven loading, and immutable snapshots.
//!
//! The crate owns everything that can be expressed by the project's CSS: theme tokens,
//! the utility, variant and property registries, import expansion, and candidate
//! evaluation. A host supplies the CSS buffers through [`DesignSystemLoader`] and
//! receives a [`DesignSystemSnapshot`]. The loader never reads a file, resolves a
//! specifier, or interprets a source name as a path; those decisions stay in the host
//! (`cli`, Vite, or an editor).
//!
//! Two prefixed custom-property namespaces exist and must stay distinct:
//! Stillwind-owned identifiers use the configurable `--sw-` prefix, and the
//! Tailwind-compatibility namespace uses the separately configurable `--tw-` prefix.
//! [`DesignSystemOptions`] validates that they differ and provides
//! [`DesignSystemOptions::rewrite_generated`] for substituting the canonical markers.
//! The compiler chooses which generated strings to pass through that operation; loading
//! a design system does not rewrite every authored value to the configured prefixes.
//!
//! Runtime-conditional syntax is shared by value and variant resolution through
//! [`is_runtime_condition`] and [`runtime_if_arguments`]; the compiler chooses its lowering.
//!
//! A snapshot is immutable after construction and clones an `Arc`, so a compile or
//! editor request pins one clone for its whole duration instead of observing two
//! revisions. Reloading CSS builds a new snapshot.

#![cfg_attr(
    not(test),
    deny(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::unreachable,
        clippy::todo,
        clippy::unimplemented
    )
)]

mod conditionals;
mod evaluation;
mod loading;
mod options;
mod output;
mod registry;
mod snapshot;
mod theme;

pub use conditionals::{is_runtime_condition, runtime_condition_value, runtime_if_arguments};
pub use evaluation::{
    NestedUtilityAtRule, NestedUtilityRule, ResolvedUtility, ResolvedUtilityTarget,
    ResolvedVariant, UtilityArbitraryUse, UtilityOrder, UtilityPropertySort,
    UtilityResolutionError, UtilityTokenUse, VariantApplication, VariantDeclaration,
    VariantMediaValue, VariantSequenceOrder, VariantValueSource, builtin_declaration_formats,
    builtin_definitions, builtin_omit_token_prefixes, builtin_property_definitions,
    builtin_theme_order, builtin_utility_token_prefixes, compare_variant_sequences,
    compatibility_marker_class, document_scoped_selector, is_condition_variant,
    is_tailwind_relationship_candidate, resolve_animation_option, resolve_utility, resolve_variant,
    resolve_variant_with_usage,
};
pub use loading::{
    CssDependencyKind, CssFileInput, CssFileRequest, CssResourceReference, CssSource,
    CssSourceEdge, CssSourceGraph, CssSourceMapping, DesignSystemLoadError, DesignSystemLoadStep,
    DesignSystemLoader,
};
pub use options::{DesignSystemOptions, UtilityOrderMode};
pub use output::parse_output_template;
pub use registry::{
    BuiltinArbitraryPropertyOrder, BuiltinDefinitions, BuiltinVariantModifierType,
    BuiltinVariantSort, BuiltinVariantSortDirection, BuiltinVariantValueType, CustomUtility,
    CustomVariant, declaration_format, parse_animation_option_property_mappings,
    parse_builtin_animation_option_definitions, parse_builtin_arbitrary_property_order,
    parse_builtin_utility_definitions, parse_builtin_variant_definitions,
    parse_declaration_formats, parse_marker_utilities, parse_omit_token_prefixes,
    parse_relationship_patterns, parse_theme_order, parse_utility_definitions,
    parse_variant_definitions,
};
pub use snapshot::{
    DefinitionFingerprints, DefinitionRegistry, DesignSystemError, DesignSystemRevision,
    DesignSystemSnapshot, PropertyRegistry, ThemeRegistry, UtilityRegistry, UtilitySearchEntry,
    UtilitySearchIndex, VariantRegistry,
};
pub use theme::{
    ThemeIndex, ThemeNamespace, ThemeReferenceError, ThemeToken, ThemeTokenKind, ThemeTokenMode,
    parse_theme, parse_theme_namespace_types,
};

#[cfg(test)]
mod tests;

#[cfg(test)]
mod test_support;
