//! Declarative Tailwind v4-compatible utility and variant registries.
//!
//! This crate deliberately knows no JavaScript framework, generated class
//! names, or Stillwind-only relations. It turns a parsed candidate and a
//! CSS-first theme into standard CSS declarations and wrappers.

use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet, HashMap},
    fmt::Write,
    sync::{Arc, OnceLock},
};

use stillwind_css::{
    BuiltinVariantModifierType, BuiltinVariantSortDirection, BuiltinVariantValueType,
    CustomUtility, CustomVariant, ThemeIndex, ThemeNamespace, ValueDomain, ValueType,
    shadow_with_color_fallback, shadow_with_color_override,
};
use stillwind_grammar::{Candidate, Utility, Value, Variant, VariantKind, parse_candidate};

mod types;
pub use types::*;
mod registry;
pub use registry::*;
mod variant_order;
pub use variant_order::*;
mod metadata;
pub use metadata::*;
mod normalization;
use normalization::{canonical_builtin_body, normalize_var_fallbacks};
mod utilities;
pub use utilities::*;
mod formatting;
use formatting::finalize_declaration;
mod variants;
pub use variants::*;
mod variant_substitution;
use variant_substitution::{
    apply_nested_variant, registered_variant_candidates, resolve_variant_css_placeholders,
    substitute_quoted_variant_value, substitute_typed_modifier, substitute_variant_capture,
};
mod variant_body;
use variant_body::resolve_variant_body;
mod utility_matching;
use utility_matching::{resolve_css_utility_branches, resolve_named_utility};
mod utility_body;
use utility_body::resolve_css_utility_body;
mod declarations;
use declarations::resolve_css_declarations;
mod placeholders;
use placeholders::resolve_css_placeholders;
mod colors;
use colors::{resolve_compat_color, resolve_shadow_color};
mod value_macros;
use value_macros::{
    resolve_literal_value_placeholders, resolve_theme_suffix_placeholders,
    resolve_value_map_placeholders,
};
mod nested;
use nested::{
    closing_brace, closing_parenthesis, matching_utilities, split_nested_conditions,
    split_utility_selector,
};
mod value_domains;
use value_domains::{
    resolve_literal_value_domains, resolve_value_domains, split_domains, theme_domain_pattern,
};
mod theme_values;
use theme_values::{
    axis_size_value, fraction_value, theme_arbitrary_types, theme_value, theme_value_literal,
};
mod arbitrary;
use arbitrary::{alpha, decode_arbitrary, important, parse_css_variable};

#[cfg(test)]
mod tests;
