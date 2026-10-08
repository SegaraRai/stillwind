//! Design-system snapshots for project session integration tests.

use stillwind_design_system::{DesignSystemOptions, DesignSystemSnapshot};
use stillwind_source::{CssSourceName, FileId};
use stillwind_test_support::css_source;

/// Returns a default design-system snapshot from `content` and the standard package.
/// The CSS source has file ID 2 and the name `test.css`.
///
/// # Panics
///
/// Panics if the fixture CSS cannot be loaded into a snapshot.
pub(crate) fn snapshot(content: &str) -> DesignSystemSnapshot {
    DesignSystemSnapshot::from_source(
        &css_source(FileId(2), content).unwrap(),
        CssSourceName::from("test.css"),
        DesignSystemOptions::default(),
    )
    .unwrap()
}

/// Returns a default snapshot with a `--color-brand` token whose CSS value is `color`.
///
/// # Panics
///
/// Panics if the fixture CSS cannot be loaded into a snapshot.
pub(crate) fn brand_snapshot(color: &str) -> DesignSystemSnapshot {
    snapshot(&format!("@theme {{ --color-brand: {color}; }}"))
}
