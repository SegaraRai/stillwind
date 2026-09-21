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

pub(super) mod config;
pub(super) mod declarations;
pub(super) mod index;
pub(super) mod namespace;
pub(super) mod parser;
pub(super) mod references;

pub use config::{LintSeverity, StillwindConfig, UtilityOrderMode};
pub use declarations::{ThemeToken, ThemeTokenKind, ThemeTokenMode};
pub use index::ThemeIndex;
pub use namespace::{ThemeNamespace, parse_theme_namespace_types};
pub use parser::parse_theme;
