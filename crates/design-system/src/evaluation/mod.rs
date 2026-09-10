mod builtins;
mod syntax;
mod utilities;
mod values;
mod variants;

pub use builtins::{
    builtin_declaration_formats, builtin_definitions, builtin_omit_token_prefixes,
    builtin_property_definitions, builtin_theme_order, builtin_utility_names,
    builtin_utility_token_prefixes, builtin_variant_names,
};
pub use utilities::{
    NestedUtilityAtRule, NestedUtilityRule, ResolvedUtility, ResolvedUtilityTarget,
    resolve_animation_option, resolve_utility,
};
pub use variants::{
    VariantApplication, VariantDeclaration, compare_variant_sequences, compatibility_marker_class,
    is_tailwind_relationship_candidate, resolve_variant,
};

#[cfg(test)]
mod tests;
