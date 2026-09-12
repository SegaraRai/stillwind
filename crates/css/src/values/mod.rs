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

pub(super) mod domain;
pub(super) mod properties;
pub(super) mod syntax;
pub(super) mod validation;
pub(super) mod webref;

pub use domain::ValueDomain;
pub use properties::CssProperty;
pub use validation::ValueType;
pub use webref::csswg_formal_syntax;
