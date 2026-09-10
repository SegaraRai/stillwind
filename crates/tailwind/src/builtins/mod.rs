pub(super) mod metadata;
pub(super) mod registry;

pub use metadata::{
    builtin_declaration_formats, builtin_omit_token_prefixes, builtin_property_definitions,
    builtin_theme_order, builtin_utility_names, builtin_utility_token_prefixes,
    builtin_variant_names,
};
pub use registry::{BuiltinDefinitions, builtin_definitions};
