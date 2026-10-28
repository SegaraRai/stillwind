//! Parsing of the CSS registry language into typed definitions.
//!
//! Two definition syntaxes exist and are deliberately separate. Stillwind definitions
//! use `@stillwind-*` directives (`@stillwind-utility`, `@stillwind-variant`,
//! `@stillwind-format`, …) and carry a `position`. Tailwind-style definitions use
//! `@utility`, `@variant`, and `@custom-variant` and do not. Both may appear in the
//! package CSS and in a stylesheet. The authored order supplies the candidate
//! precedence of authored utility ordering, so a parser here must return definitions in
//! authored order. User variants are grouped by syntax form and do not preserve
//! cross-form order. A `position` is the byte offset of the directive, so utilities and
//! arbitrary properties share one ordering space, and variant order compares with the
//! authored variant positions.
//!
//! Every entry point is usable without a complete theme stylesheet so a host can build
//! a registry once and share it.
//!
//! [`utilities`] and [`variants`] parse their respective definitions. [`contexts`]
//! reads the statements of each registry context and applies them. [`definitions`]
//! stores the Stillwind definitions. [`order`] records precedence across utilities and
//! arbitrary properties. [`formats`] parses serialization formats, relationship
//! patterns, and theme order. [`utility_lookup`] indexes definitions for candidate
//! lookup. [`markers`] collects the class names that candidates add as written.

mod contexts;
mod definitions;
mod formats;
mod markers;
mod order;
mod utilities;
mod utility_lookup;
mod variants;

pub use contexts::{BypassedUndefinition, RegistryContext};
pub use definitions::StillwindDefinitions;
pub use formats::{
    LegacyThemeExposure, LegacyThemeKey, declaration_format, parse_declaration_formats,
    parse_omit_token_prefixes, parse_relationship_patterns, parse_theme_order,
};
pub use markers::{MarkerClasses, MarkerForm, MarkerOrigin};
pub use order::{ArbitraryPropertyOrder, parse_arbitrary_property_order};
pub use utilities::{
    CustomUtility, DefinitionOrigin, UtilityRole, parse_stillwind_utility_definitions,
};
pub use variants::{
    CustomVariant, StillwindVariantModifierType, StillwindVariantSort,
    StillwindVariantSortDirection, StillwindVariantValueType, VariantScope,
    parse_stillwind_variant_definitions, parse_variant_definitions,
};

pub(crate) use contexts::{
    ArbitraryPropertyRule, RegistryStatements, accepts_arbitrary_property, mask_registry_blocks,
};
pub(crate) use order::registration_positions;
pub(crate) use utility_lookup::UtilityLookup;

pub(super) use utilities::parse_utility_definitions;
