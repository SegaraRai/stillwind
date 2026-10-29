//! The parsed theme: authored CSS, design tokens, shared assets, and registries.
//!
//! Parsing is host-independent. It reads only the buffers it is given and injects
//! nothing: no default theme, no reset, and no package CSS. A stylesheet that declares
//! nothing produces an empty theme, which is what keeps the default definitions an
//! explicit import rather than an implicit baseline.
//!
//! Tokens and assets keep spans into the supplied buffer, and the output template keeps
//! byte offsets, so a caller can attribute generated CSS and diagnostics back to the
//! file a project actually wrote.
//!
//! [`parser`] reads supplied CSS, and [`builder`] collects definitions until origins
//! are mapped. [`index`] owns immutable declarations and evaluation data. [`tokens`]
//! selects effective declarations after resets. [`declarations`] parses value roles,
//! modes, and spans, [`namespace`] reads the declared token families, and
//! [`references`] lowers build-time theme references. [`banners`] separates notices for
//! the stylesheet from notices owned by definitions.

mod banners;
mod builder;
mod declarations;
mod index;
mod namespace;
mod parser;
mod references;
mod tokens;

pub use declarations::{ThemeValue, ThemeValueMode, ThemeValuePrecedence, ThemeValueRole};
pub use index::ThemeIndex;
pub use namespace::{ThemeNamespace, ThemeNamespaces, ThemeScale};
pub use parser::{DynamicFamily, DynamicFamilyKind, UtilityImportance, parse_theme};
pub use references::ThemeReferenceError;

pub(crate) use parser::extract_keyframes_blocks;

pub(super) use parser::parse_theme_builder;

#[cfg(test)]
pub(crate) use builder::ThemeBuilder;
