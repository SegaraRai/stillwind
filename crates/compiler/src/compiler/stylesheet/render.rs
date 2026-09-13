//! The two stylesheet entry points and the errors they can report.
//!
//! Both entry points delegate to the output template; the only difference is whether
//! authored document CSS and Preflight participate. The error set is deliberately about
//! assembly - a missing utilities slot, an unusable `--theme()` reference, or CSS usage
//! parsing failure - rather than about candidate resolution, which reports through
//! diagnostics elsewhere.

use stillwind_design_system::ThemeIndex;
use stillwind_diagnostics::CompileErrorKind;

use crate::CompilerOptions;

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
) -> Result<String, CompileErrorKind> {
    render(theme, utility_css, options, true)
}

/// Assembles a producer's rules and support closure without document-wide CSS.
///
/// # Errors
///
/// Returns the assembly failures from [`super::template::render`]; no partial CSS is returned.
pub(crate) fn render_module_stylesheet(
    theme: &ThemeIndex,
    utility_css: &str,
    options: &CompilerOptions,
) -> Result<String, CompileErrorKind> {
    render(theme, utility_css, options, false)
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
) -> Result<String, CompileErrorKind> {
    super::template::render(
        &theme.output_template,
        theme,
        utility_css,
        options,
        document,
    )
}
