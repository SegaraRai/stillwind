//! CSS parsing primitives, value validation, math and color handling, custom
//! properties, resource discovery, and output-template data.
//!
//! Everything here is either standard CSS or a lexical helper the design
//! system and the compiler build on; the crate deliberately knows nothing
//! about Stillwind's theme, registry or utility model. Property and value
//! grammar is delegated to Lightning CSS and to the generated Webref table, so
//! "is this a valid value for that property" is answered from published CSS
//! data rather than from a hand-maintained list.
//!
//! Boundaries worth knowing: `resources` finds URLs but never resolves them,
//! `blocks` scans raw text without understanding declarations, and `output`
//! describes where authored CSS goes without choosing the host's module or
//! chunk layout. `format_css` prints complete stylesheets for editor previews.
//!
//! With the `minify` feature, `minify_css` processes complete assets and composes
//! source maps independently of Stillwind compilation.
//!
//! `functions` finds CSS calls, and `declarations` preserves declaration boundaries.
//! `selectors` instantiates nesting templates. `colors` converts concrete colors and
//! rewrites shadow colors. `math` folds compatible numeric expressions, and `values`
//! validates grammars and inspects property effects. `custom_properties` discovers
//! references and registrations. `emission` preserves rule occurrences and optimizes
//! declaration order. `format` formats editor previews, while the optional `minify`
//! module processes final assets.

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

mod blocks;
mod colors;
mod custom_properties;
mod declarations;
mod emission;
mod format;
mod functions;
mod math;
mod output;
mod resources;
mod selectors;
mod values;

#[cfg(feature = "minify")]
mod minify;

pub use blocks::{
    PropertyRequirementError, ScanError, extract_named_blocks, extract_named_blocks_with_offsets,
    extract_named_statements, extract_order_statements, matching_brace, strip_blocks,
    strip_css_comments, take_property_requirements, unterminated_block,
};
pub use colors::{
    HexColor, color_hex, color_rgba, color_with_alpha_fallback, css_color_values,
    shadow_with_color_fallback, shadow_with_color_override,
};
pub use custom_properties::{
    CustomPropertyActivation, CustomPropertyDefinition, CustomPropertyUsage, PropertyUsageError,
    custom_property_references, custom_property_usage, extract_authored_property_names,
    extract_property_definitions, parse_property_definitions, referenced_custom_properties,
    used_custom_properties,
};
pub use declarations::{declaration_ranges, important_declarations};
pub use emission::{CssRule, Declaration, optimize_rules, stylesheet_fragments};
pub use format::{CssFormatError, format_css};
pub use functions::{
    CssFunctionMatch, find_css_function, find_css_functions, matching_css_delimiter,
    split_css_function_arguments,
};
pub use math::{
    MAX_VALUE_NESTING, evaluate_numeric, fold_calc_expressions, format_numeric_zero,
    value_nesting_overflow,
};
pub use output::{CssOutputNode, CssOutputTemplate};
pub use resources::{CssUrl, relative_css_urls};
pub use selectors::{
    SelectorTarget, absolutize_nested_selector, append_element_condition, instantiate_selector,
    selector_is_element_condition, selector_is_subject_condition, selector_is_valid,
    selector_repeats_subject, selector_target, selector_uses_subject, subject_as_universal,
};
pub use values::{
    CssProperty, ValueDomain, ValueType, arbitrary_value_flags, csswg_formal_syntax,
    declaration_is_physical, inspect_css_value, is_length_unit, media_has_size_comparison,
    media_size_comparisons, policy_glob_matches, properties_interfere, property_covers,
    property_value_domains,
};

#[cfg(feature = "minify")]
pub use minify::{CssMinifyError, CssMinifyOptions, MinifiedCss, minify_css};
