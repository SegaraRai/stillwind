//! The parsed theme: authored CSS, design tokens, shared assets, and registries.
//!
//! Parsing does not depend on the host. It reads only the buffers that it receives and
//! injects nothing: no default theme, no reset, and no package CSS. A stylesheet that
//! declares nothing produces an empty theme, so the default definitions are always an
//! explicit import.
//!
//! Tokens and assets keep spans into the supplied buffer, and the output template keeps
//! byte offsets, so a caller can attribute generated CSS and diagnostics to the file
//! that the project wrote.
//!
//! Modules:
//!
//! - [`banners`] separates the notices of the stylesheet from the notices that
//!   definitions own.
//! - [`builder`] collects definitions until their origins are mapped.
//! - [`declarations`] parses value roles, modes, and spans.
//! - [`index`] owns immutable declarations and evaluation data.
//! - [`namespace`] reads the declared token families.
//! - [`parser`] reads the supplied CSS.
//! - [`references`] lowers build-time theme references.
//! - [`tokens`] selects the effective declarations after resets.

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
