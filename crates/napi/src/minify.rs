//! Stateless transport for final CSS minification through `stillwind-css`.

use std::collections::HashMap;

use napi_derive::napi;

use stillwind_css::CssMinifyOptions;

/// Final asset processing settings. Omitted fields use the Rust API defaults.
#[napi(object)]
pub struct NativeCssMinifyOptions {
    /// Input identity in diagnostics and source maps. Defaults to `input.css`.
    pub filename: Option<String>,
    /// Browser names mapped to 24-bit `major << 16 | minor << 8 | patch` versions.
    /// Supports android, chrome, edge, firefox, ie, `ios_saf`, opera, safari, samsung.
    pub targets: Option<HashMap<String, u32>>,
    /// Case-sensitive Lightning CSS feature names to transform, e.g. `Nesting`.
    pub include: Option<Vec<String>>,
    /// Feature names to preserve. Must not overlap `include`, including group flags.
    pub exclude: Option<Vec<String>>,
    /// Generate a separate v3 map. Defaults to false.
    pub source_map: Option<bool>,
    /// Non-indexed v3 JSON for this exact input. Requires `sourceMap: true`.
    pub input_source_map: Option<String>,
    /// Include source text in the returned map. Defaults to true.
    pub include_sources_content: Option<bool>,
    /// Names the caller has proven unused, interpreted by Lightning CSS.
    pub unused_symbols: Option<Vec<String>>,
    /// Discard invalid input with warnings instead of failing parsing. Defaults to false.
    pub error_recovery: Option<bool>,
}

impl From<NativeCssMinifyOptions> for CssMinifyOptions {
    fn from(value: NativeCssMinifyOptions) -> Self {
        Self {
            filename: value.filename.unwrap_or_else(|| "input.css".into()),
            targets: value.targets.map(|targets| targets.into_iter().collect()),
            include: value.include.unwrap_or_default(),
            exclude: value.exclude.unwrap_or_default(),
            source_map: value.source_map.unwrap_or(false),
            input_source_map: value.input_source_map,
            include_sources_content: value.include_sources_content.unwrap_or(true),
            unused_symbols: value.unused_symbols.unwrap_or_default(),
            error_recovery: value.error_recovery.unwrap_or(false),
        }
    }
}

/// CSS and optional debugging metadata from final asset processing.
#[napi(object)]
pub struct NativeMinifiedCss {
    /// Minified CSS without an appended source-map URL.
    pub css: String,
    /// Standard v3 source-map JSON when requested.
    pub source_map: Option<String>,
    /// Parser messages with input locations where available.
    pub warnings: Vec<String>,
}

/// Minifies expanded CSS and optionally composes a supplied source map.
/// Resource resolution and map publication belong to the caller. Embedded map URLs
/// are ignored. Returns the CSS, separate map, and warnings without changing inputs.
///
/// # Errors
///
/// Reports invalid options, CSS processing failures, and source-map failures.
#[napi(js_name = "minifyCss", strict)]
#[allow(clippy::needless_pass_by_value)]
pub fn minify_css(
    css: String,
    options: Option<NativeCssMinifyOptions>,
) -> napi::Result<NativeMinifiedCss> {
    let options = options.map(CssMinifyOptions::from).unwrap_or_default();
    let output = stillwind_css::minify_css(&css, &options)
        .map_err(|error| napi::Error::from_reason(error.to_string()))?;
    Ok(NativeMinifiedCss {
        css: output.css,
        source_map: output.source_map,
        warnings: output.warnings,
    })
}
