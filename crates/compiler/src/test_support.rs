//! Theme construction for unit tests that need a theme index without a snapshot.

use stillwind_design_system::{DesignSystemOptions, DesignSystemSnapshot, ThemeIndex};
use stillwind_source::{CssSourceName, SourceFile};

/// Loads `source` as `stillwind.css` with the default options and returns its theme.
///
/// # Panics
///
/// Panics if the CSS cannot be loaded, including unresolved imports.
pub(crate) fn read_theme(source: &SourceFile) -> ThemeIndex {
    DesignSystemSnapshot::from_source(
        source,
        CssSourceName::from("stillwind.css"),
        DesignSystemOptions::default(),
    )
    .unwrap()
    .theme()
    .clone()
}
