//! CSS parsing primitives, value validation, math and color handling, custom
//! properties, resource discovery, and output-template data.
//!
//! Everything here is standard CSS or a lexical helper that the design system and the
//! compiler build on. The crate knows nothing about Stillwind's theme, registry, or
//! utility model. Property and value grammar comes from Lightning CSS and the generated
//! Webref table, so value validation uses published CSS data instead of a
//! hand-maintained list.
//!
//! Modules:
//!
//! - [`banner`] retains important comments and masks their compile-time declarations.
//! - [`blocks`] scans raw text for blocks without parsing declarations.
//! - [`colors`] converts concrete colors and rewrites shadow colors.
//! - [`custom_properties`] discovers custom-property references and registrations.
//! - [`declarations`] preserves declaration boundaries and parses `name: value` lists.
//! - [`emission`] preserves rule occurrences and optimizes declaration order.
//! - [`mod@format`] prints complete stylesheets for editor previews.
//! - [`functions`] finds CSS function calls.
//! - [`lists`] splits component value lists.
//! - [`math`] folds compatible numeric expressions.
//! - [`minify`] minifies complete assets and composes their source maps. It requires
//!   the `minify` feature.
//! - [`normalization`] compares fragments after normalizing whitespace and ASCII case.
//! - [`output`] describes where authored CSS goes, without choosing the host's module
//!   or chunk layout.
//! - [`parser`] shares token traversal, block boundaries, and syntax validation.
//! - [`resources`] finds URLs without resolving them.
//! - [`selectors`] expands nesting selector templates.
//! - [`structure`] parses the rule tree of authored stylesheets with byte ranges.
//! - [`values`] validates value grammars and inspects property effects.

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

mod banner;
mod blocks;
mod colors;
mod custom_properties;
mod declarations;
mod emission;
mod format;
mod functions;
mod lists;
mod math;
mod normalization;
mod output;
mod parser;
mod resources;
mod selectors;
mod structure;
mod values;

#[cfg(feature = "minify")]
mod minify;

pub use banner::{CssBanner, CssBannerBlock, CssBannerError, banner_blocks, without_banner_blocks};
pub use blocks::{
    DirectiveBlock, css_comment_ranges, extract_directive_blocks, extract_named_blocks,
    extract_named_statements, extract_order_statements, strip_css_comments, unterminated_block,
};
pub use colors::{
    ColorScheme, HexColor, color_hex, color_rgba, select_color_scheme, shadow_with_color_fallback,
    shadow_with_color_override,
};
pub use custom_properties::{
    CustomPropertyActivation, CustomPropertyDefinition, CustomPropertyUsage, PropertyUsageError,
    custom_property_references, custom_property_usage, extract_authored_property_names,
    extract_authored_property_syntaxes, parse_property_definitions, var_reference,
};
pub use declarations::{
    DeclarationError, ParsedDeclaration, declaration_ranges, declaration_values,
    important_declarations, parse_declaration_list,
};
pub use emission::{CssRule, Declaration, optimize_rules, stylesheet_fragments};
pub use format::{CssFormatError, format_css};
pub use functions::{CssFunctionMatch, find_css_functions};
pub use lists::{split_once_top_level, split_top_level, whitespace_separated_ranges};
pub use math::{MAX_VALUE_NESTING, evaluate_numeric, fold_calc_expressions, format_numeric_zero};
pub use normalization::{CssCase, normalize_css_fragment};
pub use output::{CssOutputNode, CssOutputTemplate, PropertyInheritance};
pub use parser::{
    CssBlockRange, CssSyntaxError, CssSyntaxErrorKind, ScanError, consume_css_block,
    decode_css_escapes, is_css_property_name, literal_closed, matching_css_delimiter,
    parenthesis_nesting_overflow, parse_css_block, validate_css_syntax,
    validate_css_syntax_recovering,
};
pub use resources::{CssUrl, relative_css_urls};
pub use selectors::{
    SelectorTarget, absolutize_nested_selector, append_element_condition, fold_host_conditions,
    fold_nested_host_conditions, implicit_nesting_positions, instantiate_selector,
    selector_cannot_match_host, selector_class_names, selector_is_element_condition,
    selector_is_subject_condition, selector_is_valid, selector_list_branches,
    selector_repeats_subject, selector_selects_host, selector_target, selector_uses_subject,
    subject_as_universal, subject_compound_parts,
};
pub use structure::{CssBlock, CssNode, CssNodeKind, parse_structure};
pub use values::{
    CssProperty, ValueDomain, ValueType, WritingMode, arbitrary_value_flags, csswg_formal_syntax,
    declaration_is_physical, horizontal_logical_counterpart, inspect_css_value, is_length_unit,
    media_has_size_comparison, media_size_comparisons, policy_glob_matches, properties_interfere,
    properties_interfere_in, property_covers, property_value_domains, reads_written_value,
    repeated_longhands, value_is_well_formed, writing_mode_group,
};

#[cfg(feature = "minify")]
pub use minify::{CssMinifyError, CssMinifyOptions, MinifiedCss, minify_css};
