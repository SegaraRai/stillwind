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
//! and as a simple counter for variants, which only ever compare against other
//! variants.
//!
//! Every entry point is usable without a complete theme stylesheet so a host can build
//! a registry once and share it.
//!
//! [`utilities`] and [`variants`] parse their respective definitions. [`contexts`]
//! parses `@stillwind-registry` blocks and applies them to context registries.
//! [`definitions`] stores the selected built-in utility registry. [`order`] records
//! precedence across utilities and arbitrary properties. [`formats`] parses
//! serialization formats, relationship patterns, and theme order. [`utility_lookup`]
//! indexes definitions for candidate lookup. [`markers`] collects the class names that
//! candidates add as written.

mod contexts;
mod definitions;
mod formats;
mod markers;
mod order;
mod utilities;
mod utility_lookup;
mod variants;

pub use contexts::RegistryContext;
pub use definitions::BuiltinDefinitions;
pub use formats::{
    declaration_format, parse_declaration_formats, parse_omit_token_prefixes,
    parse_relationship_patterns, parse_theme_order,
};
pub use markers::{MarkerClasses, MarkerForm, MarkerOrigin};
pub use order::{BuiltinArbitraryPropertyOrder, parse_builtin_arbitrary_property_order};
pub use utilities::{CustomUtility, UtilityRole, parse_builtin_utility_definitions};
pub use variants::{
    BuiltinVariantModifierType, BuiltinVariantSort, BuiltinVariantSortDirection,
    BuiltinVariantValueType, CustomVariant, VariantScope, parse_builtin_variant_definitions,
    parse_variant_definitions,
};

pub(crate) use contexts::{
    ArbitraryPropertyRule, ContextDefinitions, RegistryBlock, accepts_arbitrary_property,
    split_registry_blocks,
};
pub(crate) use order::registration_positions;
pub(crate) use utility_lookup::UtilityLookup;

pub(super) use utilities::parse_utility_definitions;
