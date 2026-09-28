//! In-crate integration tests for theme parsing and snapshot construction.
//!
//! The fixtures here wrap a buffer in the standard package CSS through
//! `css_fixture::source`, so the suite exercises resolution against real built-ins
//! rather than a hand-written stub registry.

use stillwind_diagnostics::CompileError;
use stillwind_source::{CssSourceName, FileId, SourceFile};

use crate::*;

mod theme;

/// Parses a fixture buffer after substituting its real source identity.
fn parse_fixture_theme(source: &SourceFile) -> Result<ThemeIndex, CompileError> {
    parse_theme(
        &crate::css_fixture::source(source.id(), source.text()).unwrap(),
        &CssSourceName::from("test.css"),
    )
}
