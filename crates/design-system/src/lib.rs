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
//! [`RuntimeCondition`] and [`runtime_if_arguments`]; the compiler chooses its
//! lowering.
//!
//! A snapshot is immutable after construction and clones an `Arc`, so a compile or
//! editor request pins one clone for its whole duration instead of observing two
//! revisions. Reloading CSS builds a new snapshot.
//!
//! `loading` expands supplied CSS imports, and `theme` parses tokens and declarations.
//! `registry` parses utility and variant definitions. `evaluation` resolves candidates
//! against those definitions, and `conditionals` recognizes runtime tests. `options`
//! validates configuration, `output` parses output slots, and `snapshot` owns the
//! immutable revision shared by compilation and editor requests. `skipped` records the
//! invalid statements that loading leaves out instead of failing. `target` checks
//! browser versions before removing generated color fallbacks. `requirements` reads
//! registration requirements from parsed CSS directives.

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
mod requirements;
mod skipped;
mod snapshot;
mod target;
mod theme;

pub use conditionals::{RuntimeCondition, runtime_if_arguments};
pub use evaluation::{
    ConditionContext, DocumentTargetFailure, InvalidPropertyValue, InvalidValueReason, MarkerUse,
    NestedUtilityAtRule, NestedUtilityRule, RedundantVariant, ResolvedUtility,
    ResolvedUtilityTarget, ResolvedVariant, ThemeValueMismatch, UnsatisfiableVariants,
    UntypedVariable, UtilityArbitraryUse, UtilityDiagnosticData, UtilityFailure,
    UtilityFailureReason, UtilityOrder, UtilityPropertySort, UtilityResolutionError,
    UtilityTokenUse, VariableMismatch, VariableTypeSource, VariantApplication, VariantContext,
    VariantDeclaration, VariantDiagnosticData, VariantFailure, VariantFailureReason,
    VariantMediaValue, VariantObstacle, VariantRedundancy, VariantSequenceOrder,
    VariantValueSource, check_candidate_variants, compare_variant_sequences,
    compound_variant_inner, document_scoped_selector, document_target, dynamic_named_spelling,
    exact_variant_definition_resolves, is_tailwind_relationship_candidate, marker_use,
    named_domain_values, redundant_variants, repeated_subject_selector, resolve_utility,
    resolve_variant, resolve_variant_with_usage, stillwind_declaration_formats,
    stillwind_definitions, stillwind_omit_token_prefixes, stillwind_property_definitions,
    stillwind_theme_order, stillwind_utility_token_prefixes, suggested_theme_token,
    theme_value_mismatch, unresolvable_value_namespaces, unsatisfiable_variants,
    utility_value_range,
};
pub use loading::{
    CssDependencyKind, CssFileInput, CssFileRequest, CssResourceReference, CssSource,
    CssSourceEdge, CssSourceGraph, CssSourceMapping, DesignSystemLoadError, DesignSystemLoadStep,
    DesignSystemLoader, IgnoredTailwindFeature, IgnoredTailwindFeatureKind,
    ignored_tailwind_features,
};
pub use options::{DesignSystemOptions, UtilityOrderMode};
pub use registry::{
    ArbitraryPropertyOrder, BypassedUndefinition, CustomUtility, CustomVariant, MarkerClasses,
    MarkerForm, MarkerOrigin, RegistryContext, StillwindDefinitions, StillwindVariantModifierType,
    StillwindVariantSort, StillwindVariantSortDirection, StillwindVariantValueType, UtilityRole,
    VariantScope, declaration_format, parse_arbitrary_property_order, parse_declaration_formats,
    parse_omit_token_prefixes, parse_relationship_patterns, parse_stillwind_utility_definitions,
    parse_stillwind_variant_definitions, parse_theme_order, parse_variant_definitions,
};
pub use requirements::{PropertyRequirementError, take_property_requirements};
pub use skipped::{SkipReason, SkippedCss};
pub use snapshot::{
    DefinitionFingerprints, DefinitionRegistry, DesignSystemError, DesignSystemRevision,
    DesignSystemSnapshot, PropertyRegistry, ThemeRegistry, UtilityRegistry, UtilitySearchEntry,
    UtilitySearchIndex, VariantRegistry,
};
pub use target::{BrowserTargetError, BrowserTargets};
pub use theme::{
    DynamicFamily, DynamicFamilyKind, ThemeIndex, ThemeNamespace, ThemeNamespaces,
    ThemeReferenceError, ThemeScale, ThemeToken, ThemeTokenMode, ThemeTokenPrecedence,
    UtilityImportance, parse_theme,
};

#[cfg(test)]
mod tests;

#[cfg(test)]
mod test_support;
