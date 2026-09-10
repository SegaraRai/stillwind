mod definitions;
pub(super) mod formats;
mod normalization;
pub(super) mod order;
pub(super) mod utilities;
pub(super) mod variants;

pub use definitions::BuiltinDefinitions;
pub use normalization::canonical_builtin_body;

pub use formats::{
    declaration_format, parse_animation_option_property_mappings, parse_declaration_formats,
    parse_function_formats, parse_omit_token_prefixes, parse_relationship_patterns,
    parse_theme_order,
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
