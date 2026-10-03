//! Theme construction for tests that evaluate the standard package's definitions.

use stillwind_diagnostics::CompileError;
use stillwind_source::{CssSourceName, SourceFile};
use stillwind_test_support::{css, tailwind};

use crate::{ThemeIndex, parse_theme};

/// Parses `source` with the standard package CSS, retaining its file ID.
/// Converts upstream Tailwind fixture imports before package expansion. The returned
/// theme uses `test.css` as its CSS source name.
///
/// # Errors
///
/// Returns a compilation error if the expanded CSS cannot be parsed as a theme.
///
/// # Panics
///
/// Panics if package expansion fails or the expanded source exceeds the source limit.
pub(crate) fn parse_fixture_theme(source: &SourceFile) -> Result<ThemeIndex, CompileError> {
    parse_theme(
        &css::source(source.id(), tailwind::fixture_css(source.text())).unwrap(),
        &CssSourceName::from("test.css"),
    )
}
