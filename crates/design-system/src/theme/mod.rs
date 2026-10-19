//! The parsed theme: the authored CSS, its tokens, and the registries it declares.
//!
//! Parsing is host-independent. It reads only the buffers it is given and injects
//! nothing: no default theme, no reset, and no built-in registry. A stylesheet that
//! declares nothing produces an empty theme, which is what keeps the default
//! definitions an explicit import rather than an implicit baseline.
//!
//! Theme tokens keep spans into the supplied buffer, and the output template keeps byte
//! offsets, so a caller can attribute generated CSS and diagnostics back to the file a
//! project actually wrote.
//!
//! [`parser`] reads supplied CSS, and [`builder`] collects definitions until origins
//! are mapped. [`index`] owns immutable declarations and evaluation data. [`tokens`]
//! selects effective declarations after resets. [`declarations`] parses token modes
//! and spans, [`namespace`] reads the declared token families, and [`references`]
//! lowers build-time theme references.

mod builder;
mod declarations;
mod index;
mod namespace;
mod parser;
mod references;
mod tokens;

pub use declarations::{ThemeToken, ThemeTokenMode, ThemeTokenPrecedence};
pub use index::ThemeIndex;
pub use namespace::{ThemeNamespace, ThemeNamespaces, ThemeScale};
pub use parser::{DynamicFamily, DynamicFamilyKind, UtilityImportance, parse_theme};
pub use references::ThemeReferenceError;

pub(crate) use parser::extract_keyframe_blocks;

pub(super) use parser::parse_theme_builder;

#[cfg(test)]
pub(crate) use builder::ThemeBuilder;
