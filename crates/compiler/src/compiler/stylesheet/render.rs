use stillwind_css::PropertyUsageError;
use stillwind_design_system::ThemeIndex;

use crate::CompilerOptions;

#[derive(Debug, thiserror::Error)]
pub enum StylesheetError {
    #[error("authored CSS contains an unresolved --theme() reference without a usable fallback")]
    UnresolvedThemeReference,
    #[error("generated utilities require an @stillwind-slot utilities; in the CSS entry graph")]
    MissingUtilitiesSlot,
    #[error("{0}")]
    PropertyUsage(#[from] PropertyUsageError),
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
