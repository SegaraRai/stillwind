//! Minification of one complete CSS asset and composition of its source map.

use std::collections::HashSet;
use std::sync::{Arc, PoisonError, RwLock};

use lean_string::{LeanStr, LeanString, ToLeanStr};
use lightningcss::stylesheet::{MinifyOptions, ParserOptions, PrinterOptions, StyleSheet};
use lightningcss::visitor::Visit;
use parcel_sourcemap::SourceMap;

use super::optimizer::Optimizer;
use super::options::CssMinifyOptions;
use super::registrations::number_properties;

/// Failure in a final CSS processing stage. No partial output is returned.
#[derive(Debug, thiserror::Error)]
pub enum CssMinifyError {
    /// Unsupported or inconsistent caller options.
    #[error("invalid CSS minify options: {0}")]
    Options(LeanStr),
    /// Lightning CSS could not parse the input.
    #[error("CSS parse failed: {0}")]
    Parse(LeanStr),
    /// Lightning CSS could not optimize the stylesheet.
    #[error("CSS minification failed: {0}")]
    Minify(LeanStr),
    /// Lightning CSS could not serialize the stylesheet.
    #[error("CSS printing failed: {0}")]
    Print(LeanStr),
    /// An input map is invalid or map generation/composition failed.
    #[error("CSS source map failed: {0}")]
    SourceMap(LeanStr),
}

/// A final CSS asset and the optional debugging metadata produced with it.
#[derive(Debug)]
pub struct MinifiedCss {
    /// Minified CSS without an appended source-map URL.
    pub css: String,
    /// Standard v3 source-map JSON when requested.
    pub source_map: Option<String>,
    /// Parser messages including input locations where supplied by Lightning CSS.
    pub warnings: Vec<LeanStr>,
}

/// Minifies one complete CSS asset without reading files or resolving imports.
///
/// `css` must have its Stillwind directives expanded and resource URLs resolved.
/// `options.input_source_map`, when present, must map this exact input to its sources.
/// Returns CSS, an optional v3 map, and parser warnings. Inputs remain unchanged.
/// Source-map precision follows the underlying libraries. CSS transformations follow
/// Lightning CSS, followed by the rewrites that the `minify` module documentation lists.
/// Embedded source-map URLs are ignored, and no source-map comment is appended.
///
/// # Errors
///
/// Returns an option, parse, optimization, serialization, or source-map error.
/// Recovery only applies to parsing and reports skipped input through warnings.
pub fn minify_css(css: &str, options: &CssMinifyOptions) -> Result<MinifiedCss, CssMinifyError> {
    if options.input_source_map.is_some() && !options.source_map {
        return Err(CssMinifyError::Options(
            "input_source_map requires source_map".into(),
        ));
    }
    let targets = options.targets()?;
    let input_map = options
        .input_source_map
        .as_deref()
        .map(parse_source_map)
        .transpose()?;
    let warnings = Arc::new(RwLock::new(Vec::new()));
    let mut sheet = StyleSheet::parse(
        css,
        ParserOptions {
            filename: options.filename.to_string(),
            error_recovery: options.error_recovery,
            warnings: Some(Arc::clone(&warnings)),
            ..ParserOptions::default()
        },
    )
    .map_err(|error| CssMinifyError::Parse(error.to_lean_str()))?;
    sheet.source_map_urls.clear();
    sheet
        .minify(MinifyOptions {
            targets,
            unused_symbols: options
                .unused_symbols
                .iter()
                .map(ToString::to_string)
                .collect::<HashSet<_>>(),
        })
        .map_err(|error| CssMinifyError::Minify(error.to_lean_str()))?;
    let mut optimizer = Optimizer::new(number_properties(&sheet.rules));
    let Ok(()) = sheet.visit(&mut optimizer);
    let mut map = options.source_map.then(|| SourceMap::new(""));
    if let Some(map) = &mut map {
        let source = map.add_source(&options.filename);
        map.set_source_content(source as usize, css)
            .map_err(|error| CssMinifyError::SourceMap(error.to_lean_str()))?;
    }
    let result = sheet
        .to_css(PrinterOptions {
            minify: true,
            targets,
            source_map: map.as_mut(),
            ..PrinterOptions::default()
        })
        .map_err(|error| CssMinifyError::Print(error.to_lean_str()))?;
    let source_map = map
        .map(|mut map| {
            let mut source_root = None;
            if let Some((mut upstream, root)) = input_map {
                map.extends(&mut upstream)
                    .map_err(|error| CssMinifyError::SourceMap(error.to_lean_str()))?;
                source_root = root;
            }
            let json = map
                .to_json(source_root.as_deref())
                .map_err(|error| CssMinifyError::SourceMap(error.to_lean_str()))?;
            if options.include_sources_content {
                Ok(json)
            } else {
                let mut json: serde_json::Value = serde_json::from_str(&json)
                    .map_err(|error| CssMinifyError::SourceMap(error.to_lean_str()))?;
                if let Some(object) = json.as_object_mut() {
                    object.remove("sourcesContent");
                }
                Ok(json.to_string())
            }
        })
        .transpose()?;
    let warnings = warnings
        .read()
        .unwrap_or_else(PoisonError::into_inner)
        .iter()
        .map(ToLeanStr::to_lean_str)
        .collect();
    Ok(MinifiedCss {
        css: result.code,
        source_map,
        warnings,
    })
}

fn parse_source_map(json: &str) -> Result<(SourceMap, Option<LeanString>), CssMinifyError> {
    let value: serde_json::Value = serde_json::from_str(json)
        .map_err(|error| CssMinifyError::SourceMap(error.to_lean_str()))?;
    if value["version"] != 3 || value.get("sections").is_some() {
        return Err(CssMinifyError::SourceMap(
            "expected a non-indexed v3 map".into(),
        ));
    }
    let root = value
        .get("sourceRoot")
        .filter(|root| !root.is_null())
        .map(|root| {
            root.as_str()
                .map(LeanString::from)
                .ok_or_else(|| CssMinifyError::SourceMap("sourceRoot must be a string".into()))
        })
        .transpose()?;
    let map = SourceMap::from_json("", json)
        .map_err(|error| CssMinifyError::SourceMap(error.to_lean_str()))?;
    Ok((map, root))
}
