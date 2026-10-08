//! Renders shared CSS and canonicalizes constant calc expressions.

use napi::Error;
use napi_derive::napi;

use stillwind_compiler::Compiler;
use stillwind_source::{CssSourceName, FileId, SourceFile};

use super::BindingApiError;
use crate::options::{NativeCompilerOptions, compiler_options};

/// Wraps project module CSS fragments in one shared theme/base stylesheet.
///
/// # Errors
///
/// Returns a JavaScript error when the theme or compiler options are invalid.
#[napi(strict)]
#[allow(clippy::needless_pass_by_value)] // Node-API decodes JavaScript strings as owned Rust values.
pub fn render_stylesheet(
    module_css: String,
    theme: String,
    options: Option<NativeCompilerOptions>,
) -> napi::Result<String> {
    let options = compiler_options(options)?;
    let theme = SourceFile::new(FileId(1), None, None, theme).map_err(BindingApiError::from)?;
    Compiler::render_stylesheet(
        &theme,
        &CssSourceName::from("stillwind.css"),
        &module_css,
        &options,
    )
    .map_err(|error| Error::from_reason(error.to_string()))
}

/// Canonicalizes constant `calc()` subexpressions with Stillwind's CSS value parser.
/// Exposed so parity and integration tools compare both compilers after the same
/// semantic normalization. Expressions that do not parse, or that nest values deeper
/// than the calculator accepts, are retained byte-for-byte.
#[napi]
#[allow(clippy::needless_pass_by_value)] // Node-API decodes JavaScript strings as owned Rust values.
pub fn fold_calc_expressions(css: String, zero_unit: Option<String>) -> String {
    stillwind_css::fold_calc_expressions(&css, zero_unit.as_deref())
}
