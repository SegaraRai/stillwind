//! Finds the `@stillwind-banner` declarations that apply to the whole stylesheet.
//!
//! A declaration inside `@theme` or a utility definition belongs to that definition,
//! and the definition parsers collect it. This module returns only the other
//! declarations.

use lean_string::ToLeanStr;

use stillwind_css::{CssBanner, banner_blocks, extract_directive_blocks, without_banner_blocks};
use stillwind_diagnostics::CompileDiagnostic;

/// Returns `css` with every banner declaration masked, and the notices declared outside
/// definition blocks in source order.
///
/// The masked CSS keeps the byte offsets and line breaks of `css`.
///
/// # Errors
///
/// Returns [`CompileDiagnostic::CssParse`] for the first malformed declaration.
pub(super) fn extract_stylesheet_banners(
    css: &str,
) -> Result<(String, Vec<CssBanner>), CompileDiagnostic> {
    let masked = without_banner_blocks(css).map_err(|error| CompileDiagnostic::CssParse {
        detail: error.to_lean_str(),
    })?;
    let blocks = banner_blocks(css).map_err(|error| CompileDiagnostic::CssParse {
        detail: error.to_lean_str(),
    })?;
    if blocks.is_empty() {
        return Ok((masked, Vec::new()));
    }
    let scopes: Vec<_> = [
        "@theme",
        "@utility",
        "@stillwind-utility",
        "@stillwind-plugin-theme",
    ]
    .into_iter()
    .flat_map(|name| extract_directive_blocks(css, name))
    .collect();
    let global = blocks
        .into_iter()
        .filter(|block| {
            !scopes
                .iter()
                .any(|parent| parent.body.contains(&block.range.start))
        })
        .map(|block| block.banner)
        .collect();
    Ok((masked, global))
}
