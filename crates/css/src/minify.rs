//! Final stylesheet minification and source-map composition through Lightning CSS.
//!
//! [`minify_css`] accepts expanded CSS and an optional map for that exact input.
//! Hosts own resource resolution, asset boundaries, and source-map publication.

use std::collections::{BTreeMap, HashSet};
use std::convert::Infallible;
use std::sync::{Arc, PoisonError, RwLock};

use lean_string::{LeanStr, LeanString, ToLeanStr};
use lightningcss::properties::custom::{Function, Token, TokenList, TokenOrValue};
use lightningcss::stylesheet::{MinifyOptions, ParserOptions, PrinterOptions, StyleSheet};
use lightningcss::targets::{Browsers, Features, Targets};
use lightningcss::visit_types;
use lightningcss::visitor::{Visit, VisitTypes, Visitor};
use parcel_sourcemap::SourceMap;

use crate::values::MATH_FUNCTIONS;

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
/// Source-map precision and CSS transformations follow the underlying libraries, except
/// that unparsed values also lose whitespace next to separators.
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
                .map(std::string::ToString::to_string)
                .collect::<HashSet<_>>(),
        })
        .map_err(|error| CssMinifyError::Minify(error.to_lean_str()))?;
    let Ok(()) = sheet.visit(&mut TokenWhitespace);
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

/// Controls final asset optimization independently of compiler generation options.
#[derive(Clone, Debug)]
pub struct CssMinifyOptions {
    /// Input identity used in errors and generated source maps. Defaults to `input.css`.
    pub filename: LeanStr,
    /// Browser names (`android`, `chrome`, `edge`, `firefox`, `ie`, `ios_saf`,
    /// `opera`, `safari`, `samsung`) mapped to `major << 16 | minor << 8 | patch`.
    /// Each component occupies one byte. `None` leaves browser targets unspecified.
    pub targets: Option<BTreeMap<LeanStr, u32>>,
    /// Lightning CSS feature names to always transform, such as `Nesting` or
    /// `LogicalProperties`. Unknown names and overlap with `exclude` are errors.
    pub include: Vec<LeanStr>,
    /// Lightning CSS feature names to preserve even when targets lack support.
    pub exclude: Vec<LeanStr>,
    /// Generate a separate v3 JSON map. Defaults to false.
    pub source_map: bool,
    /// Ordinary v3 JSON mapping the input CSS to original sources. Requires
    /// `source_map`; indexed maps are unsupported. Inline map comments are ignored.
    pub input_source_map: Option<LeanStr>,
    /// Include source text in the output map. Defaults to true.
    pub include_sources_content: bool,
    /// Names the caller has proven unused, interpreted by Lightning CSS.
    pub unused_symbols: Vec<LeanStr>,
    /// Allow the parser to discard invalid rules/declarations and return warnings.
    /// Defaults to false.
    pub error_recovery: bool,
}

impl Default for CssMinifyOptions {
    fn default() -> Self {
        Self {
            filename: "input.css".into(),
            targets: None,
            include: Vec::new(),
            exclude: Vec::new(),
            source_map: false,
            input_source_map: None,
            include_sources_content: true,
            unused_symbols: Vec::new(),
            error_recovery: false,
        }
    }
}

impl CssMinifyOptions {
    fn targets(&self) -> Result<Targets, CssMinifyError> {
        let browsers = self
            .targets
            .as_ref()
            .map(|entries| {
                let mut browsers = Browsers::default();
                for (name, version) in entries {
                    if *version > 0x00ff_ffff {
                        return Err(CssMinifyError::Options(
                            format_args!("browser version exceeds 24 bits: {name}").to_lean_str(),
                        ));
                    }
                    let slot = match name.as_str() {
                        "android" => &mut browsers.android,
                        "chrome" => &mut browsers.chrome,
                        "edge" => &mut browsers.edge,
                        "firefox" => &mut browsers.firefox,
                        "ie" => &mut browsers.ie,
                        "ios_saf" => &mut browsers.ios_saf,
                        "opera" => &mut browsers.opera,
                        "safari" => &mut browsers.safari,
                        "samsung" => &mut browsers.samsung,
                        _ => {
                            return Err(CssMinifyError::Options(
                                format_args!("unknown browser: {name}").to_lean_str(),
                            ));
                        }
                    };
                    *slot = Some(*version);
                }
                Ok(browsers)
            })
            .transpose()?;
        let include = features(&self.include)?;
        let exclude = features(&self.exclude)?;
        if include.intersects(exclude) {
            return Err(CssMinifyError::Options(
                "include and exclude overlap".into(),
            ));
        }
        Ok(Targets {
            browsers,
            include,
            exclude,
        })
    }
}

fn features(names: &[LeanStr]) -> Result<Features, CssMinifyError> {
    names.iter().try_fold(Features::empty(), |flags, name| {
        Features::from_name(name)
            .map(|feature| flags | feature)
            .ok_or_else(|| {
                CssMinifyError::Options(format_args!("unknown CSS feature: {name}").to_lean_str())
            })
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

/// Removes whitespace tokens from unparsed values when the printed tokens keep their
/// meaning without them.
///
/// Every token list loses whitespace next to commas. The arguments of an unparsed math
/// function such as `calc(var(--a) * 2)` also lose whitespace next to `*` and `/`.
/// Outside math functions, whitespace around `*` can be a descendant combinator, as in
/// `.a * .b` inside the block of an unknown at-rule.
struct TokenWhitespace;

impl<'i> Visitor<'i> for TokenWhitespace {
    type Error = Infallible;

    fn visit_types(&self) -> VisitTypes {
        visit_types!(TOKENS | FUNCTIONS)
    }

    fn visit_token_list(&mut self, tokens: &mut TokenList<'i>) -> Result<(), Self::Error> {
        remove_separator_whitespace(&mut tokens.0, false);
        tokens.visit_children(self)
    }

    fn visit_function(&mut self, function: &mut Function<'i>) -> Result<(), Self::Error> {
        if MATH_FUNCTIONS
            .iter()
            .any(|name| function.name.0.eq_ignore_ascii_case(name))
        {
            remove_separator_whitespace(&mut function.arguments.0, true);
        }
        function.visit_children(self)
    }
}

/// Drops each whitespace token next to a comma, or next to `*` or `/` when `math` is
/// true.
///
/// The token printer writes adjacent tokens without a separator. Whitespace next to
/// `*` or `/` stays when the other neighbor is a delimiter or a comment, because
/// `/ *` would print as the comment opener `/*` and `* =` as the `*=` token.
fn remove_separator_whitespace(tokens: &mut Vec<TokenOrValue<'_>>, math: bool) {
    let mut input = std::mem::take(tokens).into_iter().peekable();
    while let Some(token) = input.next() {
        let removable = token.is_whitespace()
            && (separates(tokens.last(), input.peek(), math)
                || separates(input.peek(), tokens.last(), math));
        if !removable {
            tokens.push(token);
        }
    }
}

/// Whether `separator` is a token that whitespace between it and `other` may be
/// dropped next to.
fn separates(
    separator: Option<&TokenOrValue<'_>>,
    other: Option<&TokenOrValue<'_>>,
    math: bool,
) -> bool {
    match separator {
        Some(TokenOrValue::Token(Token::Comma)) => true,
        Some(TokenOrValue::Token(Token::Delim('*' | '/'))) if math => !matches!(
            other,
            Some(TokenOrValue::Token(Token::Delim(_) | Token::Comment(_)))
        ),
        _ => false,
    }
}
