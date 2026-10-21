//! Theme construction for tests that evaluate the standard package's definitions, and
//! definition parsing for tests that do not need offsets.

use stillwind_diagnostics::CompileError;
use stillwind_source::{CssSourceName, SourceFile};
use stillwind_test_support::{css_source, fixture_css};

use crate::registry::parse_utility_definitions;
use crate::{CustomUtility, CustomVariant, ThemeIndex, parse_theme, parse_variant_definitions};

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
        &css_source(source.id(), fixture_css(source.text())).unwrap(),
        &CssSourceName::from("test.css"),
    )
}

/// Parses the `@utility` definitions of `css` without their offsets.
pub(crate) fn utility_definitions(css: &str) -> Vec<CustomUtility> {
    parse_utility_definitions(css)
        .into_iter()
        .map(|(_, utility)| utility)
        .collect()
}

/// Parses the variant definitions of `css` without their offsets.
pub(crate) fn variant_definitions(css: &str) -> Vec<CustomVariant> {
    parse_variant_definitions(css)
        .into_iter()
        .map(|(_, variant)| variant)
        .collect()
}
