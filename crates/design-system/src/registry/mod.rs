//! Parsing of the CSS registry language into typed definitions.
//!
//! Two definition origins exist and are deliberately separate: compiler-owned
//! directives (`@stillwind-utility`, `@stillwind-variant`, `@stillwind-format`, …)
//! carry a `builtin_order`, while user-authored directives (`@utility`, `@variant`,
//! `@custom-variant`) do not. The authored order supplies the candidate precedence of
//! authored utility ordering, so a parser here must return definitions in authored
//! order. User variants are grouped by syntax form and do not preserve cross-form
//! order. Built-in order is carried as a byte offset where a definition must compare
//! against another kind - utilities and arbitrary properties share one ordering space -
//! and as a simple counter for variants, which only ever compare against other variants.
//!
//! Every entry point is usable without a complete theme stylesheet so a host can build
//! a registry once and share it.

mod definitions;
pub(super) mod formats;
pub(super) mod order;
pub(super) mod utilities;
pub(super) mod variants;

pub use definitions::BuiltinDefinitions;
pub use formats::{
    declaration_format, parse_animation_option_property_mappings, parse_declaration_formats,
    parse_omit_token_prefixes, parse_relationship_patterns, parse_theme_order,
};
pub use order::{BuiltinArbitraryPropertyOrder, parse_builtin_arbitrary_property_order};
pub use utilities::{
    CustomUtility, parse_builtin_animation_option_definitions, parse_builtin_utility_definitions,
    parse_marker_utilities, parse_utility_definitions,
};
pub use variants::{
    BuiltinVariantModifierType, BuiltinVariantSort, BuiltinVariantSortDirection,
    BuiltinVariantValueType, CustomVariant, parse_builtin_variant_definitions,
    parse_variant_definitions,
};
