//! The two stylesheet entry points and the errors they can report.
//!
//! Both entry points delegate to the output template; the only difference is whether
//! authored document CSS and Preflight participate. The errors describe assembly
//! failures, such as a missing utilities slot, an unusable `--theme()` reference, or a
//! CSS usage parsing failure. Candidate resolution reports through diagnostics instead.

use stillwind_design_system::ThemeIndex;
use stillwind_diagnostics::CompileDiagnostic;

use crate::identifiers::IdentifierContext;
use crate::{CompilerOptions, MappedStylesheet};

/// Assembles document CSS from the theme template and raw utility rules.
///
/// # Errors
///
/// Returns the missing-slot, authored-theme-reference, and property-usage failures
/// from [`super::template::render`], without attaching a reporting location.
#[allow(clippy::too_many_lines)]
pub(crate) fn render_stylesheet(
    theme: &ThemeIndex,
    utility_css: &str,
    options: &CompilerOptions,
    identifiers: &IdentifierContext,
) -> Result<MappedStylesheet, CompileDiagnostic> {
    render(theme, utility_css, options, true, identifiers)
}

/// Assembles a producer's rules and support closure without document-wide CSS.
///
/// # Errors
///
/// Returns the assembly failures from [`super::template::render`]; no partial CSS is
/// returned.
pub(crate) fn render_module_stylesheet(
    theme: &ThemeIndex,
    utility_css: &str,
    options: &CompilerOptions,
    identifiers: &IdentifierContext,
) -> Result<MappedStylesheet, CompileDiagnostic> {
    render(theme, utility_css, options, false, identifiers)
}

/// Renders raw rules through the theme template; `document` includes document-only CSS.
///
/// # Errors
///
/// Propagates template assembly failures without adding a source location.
#[allow(clippy::too_many_lines)]
fn render(
    theme: &ThemeIndex,
    utility_css: &str,
    options: &CompilerOptions,
    document: bool,
    identifiers: &IdentifierContext,
) -> Result<MappedStylesheet, CompileDiagnostic> {
    super::template::render(
        theme.output_template(),
        theme,
        utility_css,
        options,
        document,
        identifiers,
    )
}
