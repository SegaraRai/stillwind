//! The two stylesheet entry points and the errors they can report.
//!
//! Both entry points delegate to the output template; the only difference is whether
//! authored document CSS and Preflight participate. The error set is deliberately about
//! assembly - a missing utilities slot, an unusable `--theme()` reference, or CSS usage
//! parsing failure - rather than about candidate resolution, which reports through
//! diagnostics elsewhere.

use stillwind_css::PropertyUsageError;
use stillwind_design_system::ThemeIndex;

use crate::CompilerOptions;

/// Reasons a stylesheet could not be assembled.
#[derive(Debug, thiserror::Error)]
pub enum StylesheetError {
    /// Authored CSS contains a `--theme()` reference with no fallback and no matching token.
    #[error("authored CSS contains an unresolved --theme() reference without a usable fallback")]
    UnresolvedThemeReference,
    /// Utility rules exist but the CSS entry graph has no `@stillwind-slot utilities;`.
    #[error("generated utilities require an @stillwind-slot utilities; in the CSS entry graph")]
    MissingUtilitiesSlot,
    /// CSS could not be parsed while collecting custom-property usage.
    #[error("{0}")]
    PropertyUsage(#[from] PropertyUsageError),
    /// A node refers to a rule that is not in this output.
    #[error("stylesheet contribution references a missing rule")]
    MissingRule,
}

#[allow(clippy::too_many_lines)]
pub(crate) fn render_stylesheet(
    theme: &ThemeIndex,
    utility_css: &str,
    options: &CompilerOptions,
) -> Result<String, StylesheetError> {
    render(theme, utility_css, options, true)
}

/// A producer's rules and their support closure, without document-wide CSS.
pub(crate) fn render_module_stylesheet(
    theme: &ThemeIndex,
    utility_css: &str,
    options: &CompilerOptions,
) -> Result<String, StylesheetError> {
    render(theme, utility_css, options, false)
}

#[allow(clippy::too_many_lines)]
/// Shared body of both entry points; `document` selects authored CSS and document-only rules.
fn render(
    theme: &ThemeIndex,
    utility_css: &str,
    options: &CompilerOptions,
    document: bool,
) -> Result<String, StylesheetError> {
    super::template::render(
        &theme.output_template,
        theme,
        utility_css,
        options,
        document,
    )
}
