//! CSS-defined styling semantics, caller-driven loading, and immutable snapshots.
//!
//! The crate owns everything that the project's CSS can express: theme tokens, the
//! utility, variant, and property registries, import expansion, and candidate
//! evaluation. A host supplies CSS buffers through [`DesignSystemLoader`] and receives
//! a [`DesignSystemSnapshot`]. The loader never reads a file, resolves a specifier, or
//! interprets a source name as a path. The host (the CLI, Vite, or an editor) makes
//! those decisions.
//!
//! Stillwind-owned custom properties use the configurable `--sw-` prefix, and the
//! Tailwind-compatibility namespace uses the separately configurable `--tw-` prefix.
//! The names in the two namespaces do not overlap, so the prefixes may be equal.
//! [`DesignSystemOptions`] validates both prefixes, and
//! [`DesignSystemOptions::rewrite_generated`] substitutes them for the canonical
//! markers. The compiler chooses which generated strings to rewrite; loading does not
//! rewrite authored values.
//!
//! Value and variant resolution share the runtime-conditional syntax through
//! [`RuntimeCondition`] and [`runtime_if_arguments`]. The compiler chooses how to lower
//! it.
//!
//! A snapshot is immutable and cheap to clone, so a compile or editor request keeps one
//! clone for its whole duration and never observes two revisions. Reloading CSS builds
//! a new snapshot.
//!
//! Modules:
//!
//! - [`conditionals`] recognizes runtime-conditional tests.
//! - [`evaluation`] resolves candidates against the loaded definitions.
//! - [`loading`] expands the CSS imports that the host supplies.
//! - [`options`] validates configuration.
//! - [`output`] parses output templates and their slots.
//! - [`registry`] parses utility and variant definitions.
//! - [`requirements`] reads registration requirements from parsed CSS directives.
//! - [`skipped`] records the invalid statements that loading leaves out instead of
//!   failing.
//! - [`snapshot`] owns the immutable revision that compilation and editor requests
//!   share.
//! - [`target`] checks browser versions before generated color fallbacks are removed.
//! - [`theme`] parses tokens, declarations, and shared assets.

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
    NestedUtilityAtRule, NestedUtilityRule, NumericDomain, NumericKind, RedundantVariant,
    ResolvedUtility, ResolvedUtilityTarget, ResolvedVariant, ThemeValueMismatch,
    UnsatisfiableVariants, UntypedVariable, UtilityArbitraryUse, UtilityDeclaration,
    UtilityDiagnosticData, UtilityFailure, UtilityFailureReason, UtilityOrder, UtilityPropertySort,
    UtilityResolutionError, UtilityTokenUse, VariableMismatch, VariableTypeSource,
    VariantApplication, VariantContext, VariantDeclaration, VariantDiagnosticData, VariantFailure,
    VariantFailureReason, VariantMediaValue, VariantObstacle, VariantRedundancy,
    VariantSequenceOrder, VariantValueSource, check_candidate_variants, compare_variant_sequences,
    compound_variant_inner, document_scoped_selector, document_target, dynamic_named_spelling,
    exact_variant_definition_resolves, is_tailwind_relationship_candidate, marker_use,
    named_domain_values, placeholder_namespaces, redundant_variants, repeated_subject_selector,
    resolve_utility, resolve_variant, resolve_variant_with_usage, stillwind_declaration_formats,
    stillwind_definitions, stillwind_omit_token_prefixes, stillwind_property_definitions,
    stillwind_theme_order, stillwind_utility_token_prefixes, suggested_theme_token,
    theme_value_mismatch, unresolvable_value_namespaces, unsatisfiable_variants,
    utility_value_range, variant_alternatives,
};
pub use loading::{
    CssDependencyKind, CssFileInput, CssFileRequest, CssResourceReference, CssSource,
    CssSourceEdge, CssSourceGraph, CssSourceMapping, DesignSystemLoadError, DesignSystemLoadStep,
    DesignSystemLoader, IgnoredTailwindFeature, IgnoredTailwindFeatureKind,
    ignored_tailwind_features,
};
pub use options::{DesignSystemOptions, UtilityOrderMode, rewrite_generated_prefixes};
pub use registry::{
    ArbitraryPropertyOrder, BypassedUndefinition, CustomUtility, CustomVariant, DefinitionOrigin,
    LegacyThemeExposure, LegacyThemeKey, MarkerClasses, MarkerForm, MarkerOrigin, RegistryContext,
    StillwindDefinitions, StillwindVariantModifierType, StillwindVariantSort,
    StillwindVariantSortDirection, StillwindVariantValueType, UtilityRole, VariantScope,
    declaration_format, parse_arbitrary_property_order, parse_declaration_formats,
    parse_omit_token_prefixes, parse_relationship_patterns, parse_stillwind_utility_definitions,
    parse_stillwind_variant_definitions, parse_theme_order, parse_variant_definitions,
};
pub use requirements::{PropertyRequirementError, take_property_requirements};
pub use skipped::{SkipReason, SkippedCss};
pub use snapshot::{
    DefinitionFingerprints, DefinitionRegistry, DesignSystemError, DesignSystemRevision,
    DesignSystemSnapshot, PropertyRegistry, SplitFamily, ThemeRegistry, UtilityRegistry,
    UtilitySearchEntry, UtilitySearchIndex, VariantRegistry, declared_properties,
};
pub use target::{BrowserTargetError, BrowserTargets};
pub use theme::{
    DynamicFamily, DynamicFamilyKind, ThemeIndex, ThemeNamespace, ThemeNamespaces,
    ThemeReferenceError, ThemeScale, ThemeValue, ThemeValueMode, ThemeValuePrecedence,
    ThemeValueRole, UtilityImportance, parse_theme,
};

#[cfg(test)]
mod tests;

#[cfg(test)]
mod test_support;
