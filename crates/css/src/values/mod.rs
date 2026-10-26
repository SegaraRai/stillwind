//! Value domains: what an arbitrary `[hint:value]` domain accepts.
//!
//! [`ValueType`] is the set of CSS value definitions the utility grammar can name in a
//! hint, [`ValueDomain`] wraps one for the registry, and [`properties`] plus [`webref`]
//! answer property-specific questions from the generated Webref table. [`syntax`] holds
//! the focused helpers (gradients, `content`, transform functions, `var()` detection)
//! used where one typed parser is not enough.
//!
//! Nothing in this module knows about utilities, variants or theme namespaces:
//! hints name CSS value productions, and callers supply the property context.
//!
//! `domain` defines registry domains, and `validation` checks individual values against
//! CSS productions. `policy` inspects properties, units, and media comparisons for
//! caller-defined policies.

mod domain;
mod policy;
mod properties;
mod syntax;
mod validation;
mod webref;

pub use domain::ValueDomain;
pub use policy::{
    arbitrary_value_flags, declaration_is_physical, inspect_css_value, is_length_unit,
    media_has_size_comparison, media_size_comparisons, policy_glob_matches, property_value_domains,
};
pub use properties::CssProperty;
pub use syntax::value_is_well_formed;
pub use validation::ValueType;
pub use webref::{csswg_formal_syntax, properties_interfere, property_covers, repeated_longhands};
