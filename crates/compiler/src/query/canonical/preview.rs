//! Renders canonical previews and normalizes their CSS under one query's spacing policy.

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;
use std::sync::{Arc, Mutex, PoisonError};

use cssparser::{ParseError, Parser, Token};
use lean_string::{LeanStr, LeanString, ToLeanString};

use stillwind_css::{CssMinifyOptions, CssRule, Declaration, fold_calc_expressions, minify_css};
use stillwind_design_system::UtilityResolutionError;
use stillwind_source::ReplaceText;

use crate::compiler::FLATTEN_COLOR_COMPATIBILITY;
use crate::query::QueryContext;
use crate::query::theme::ScaleReference;
use crate::{GeneratedRule, RootFontSize};

/// Spacing transformations that affect equality within one query's fixed lint policy.
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub(super) struct Normalization {
    /// Whether to substitute the owning query's theme scales.
    pub(super) resolve_scale: bool,
    /// Property names whose pixel dimensions use the query's root font size.
    pub(super) properties: BTreeSet<LeanStr>,
}

/// Rendered candidate data reused while this query compares canonical spellings.
#[derive(Debug)]
pub(in crate::query) struct CanonicalPreview {
    /// Candidate CSS with generated compatibility guards flattened, before known theme
    /// variables are substituted. Authored conditions remain present.
    pub(super) css: LeanStr,
    /// CSS with known theme variables substituted. Unknown variables keep their
    /// spelling. `None` when substitution exceeds the query's expansion limits.
    pub(super) resolved_css: Option<LeanStr>,
    /// Registration dependencies compared alongside CSS before accepting a replacement.
    pub(super) required_properties: Vec<LeanStr>,
    /// Successful and failed normalizations keyed by their spacing transformations.
    /// Query clones share this cache across threads.
    pub(super) signatures: Mutex<BTreeMap<Normalization, Option<LeanStr>>>,
}

impl CanonicalPreview {
    /// Returns cached normalized CSS under the owning query's spacing policy.
    ///
    /// `scales`, when not empty, and `root_font_size` must match the owning query's
    /// values. Only `properties` and whether `scales` is empty may vary between calls.
    /// Caches `None` when theme substitution or CSS normalization fails.
    pub(super) fn signature(
        &self,
        scales: &[ScaleReference],
        properties: &BTreeSet<LeanStr>,
        root_font_size: RootFontSize,
    ) -> Option<LeanStr> {
        let key = Normalization {
            resolve_scale: !scales.is_empty(),
            properties: properties.clone(),
        };
        if let Some(cached) = self
            .signatures
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&key)
        {
            return cached.clone();
        }
        let result = self
            .resolved_css
            .as_deref()
            .and_then(|css| normalized_signature(css, scales, properties, root_font_size));
        self.signatures
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(key, result.clone());
        result
    }
}

/// Normalizes CSS whose known theme variables have already been substituted.
///
/// Replaces the reference to each scale of `scales` with its value, and converts pixel
/// dimensions in `properties` using `root_font_size`. Folds calculations and minifies
/// the result. Returns `None` when the normalized CSS cannot be minified.
pub(super) fn normalized_signature(
    css: &str,
    scales: &[ScaleReference],
    properties: &BTreeSet<LeanStr>,
    root_font_size: RootFontSize,
) -> Option<LeanStr> {
    let mut normalized = css.to_lean_string();
    for scale in scales {
        normalized = normalized.replace_text(&scale.reference, &scale.value);
    }
    if !properties.is_empty() {
        normalized = convert_spacing_declarations(&normalized, properties, root_font_size.get(), 0);
    }
    normalized = fold_calc_expressions(&normalized, None);
    minify_css(&normalized, &CssMinifyOptions::default())
        .ok()
        .map(|output| LeanStr::from(output.css))
}

impl QueryContext {
    /// Renders `raw` with `&` as its subject and caches the preview within this query.
    /// Returns `None` when candidate resolution records a failure. A preview can still
    /// lack `resolved_css` when theme substitution exceeds its expansion limits.
    ///
    /// # Errors
    ///
    /// Returns a malformed property requirement from a matching utility definition.
    pub(in crate::query) fn canonical_preview(
        &self,
        raw: &str,
    ) -> Result<Option<Arc<CanonicalPreview>>, UtilityResolutionError> {
        if let Some(cached) = self.canonical_previews.borrow().get(raw) {
            return Ok(cached.clone());
        }
        let result =
            self.resolve_with_selector::<FLATTEN_COLOR_COMPATIBILITY>(raw, Some("&"), false)?;
        let preview = result.failures.is_empty().then(|| {
            let css: LeanString = result
                .generated_rules
                .iter()
                .map(GeneratedRule::to_css)
                .collect();
            Arc::new(CanonicalPreview {
                resolved_css: (self.resolve_theme_values(&css)),
                css: (css).into(),
                required_properties: result.required_properties,
                signatures: Mutex::new(BTreeMap::new()),
            })
        });
        self.canonical_previews
            .borrow_mut()
            .insert(raw.into(), preview.clone());
        Ok(preview)
    }
}

/// The units accepted when comparing canonical spacing values.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SpacingUnit {
    Rem,
    Px,
}

impl SpacingUnit {
    fn as_str(self) -> &'static str {
        match self {
            Self::Rem => "rem",
            Self::Px => "px",
        }
    }
}

pub(super) fn dimension(value: &str) -> Option<(f64, SpacingUnit)> {
    let value = value.trim();
    let unit = if value.ends_with("rem") {
        SpacingUnit::Rem
    } else if value.ends_with("px") {
        SpacingUnit::Px
    } else {
        return None;
    };
    let number = value.strip_suffix(unit.as_str())?.parse::<f64>().ok()?;
    number.is_finite().then_some((number, unit))
}

pub(super) fn spacing_properties(
    css: &str,
    scales: &[ScaleReference],
    properties: &mut BTreeSet<LeanStr>,
    depth: usize,
) {
    if depth > 64 {
        return;
    }
    if let Some(declarations) = Declaration::parse(css) {
        properties.extend(
            declarations
                .into_iter()
                .filter(|declaration| {
                    scales
                        .iter()
                        .any(|scale| scale.referenced_by(&declaration.css))
                })
                .map(|declaration| declaration.property),
        );
    } else if let Ok(rules) = CssRule::parse(css) {
        for rule in rules {
            if let Some(body) = rule.body {
                spacing_properties(&body, scales, properties, depth + 1);
            }
        }
    }
}

fn convert_spacing_declarations(
    css: &str,
    properties: &BTreeSet<LeanStr>,
    root: f64,
    depth: usize,
) -> LeanString {
    if depth > 64 {
        return css.to_lean_string();
    }
    if let Some(declarations) = Declaration::parse(css) {
        return declarations
            .into_iter()
            .map(|declaration| {
                if properties.contains(&declaration.property)
                    && let Some((name, value)) = declaration.css.split_once(':')
                {
                    format_args!("{name}:{};", convert_pixel_dimensions(value, root))
                        .to_lean_string()
                } else {
                    format_args!("{};", declaration.css).to_lean_string()
                }
            })
            .collect();
    }
    CssRule::parse(css).map_or_else(
        |_| css.to_lean_string(),
        |rules| {
            rules
                .into_iter()
                .map(|mut rule| {
                    if let Some(body) = &rule.body {
                        rule.body = Some(
                            (convert_spacing_declarations(body, properties, root, depth + 1))
                                .into(),
                        );
                    }
                    rule.to_css()
                })
                .collect()
        },
    )
}

pub(super) fn convert_pixel_dimensions(value: &str, root: f64) -> LeanString {
    fn collect(
        parser: &mut Parser<'_>,
        value: &str,
        root: f64,
        edits: &mut Vec<(Range<usize>, LeanString)>,
    ) {
        loop {
            let start = parser.position().byte_index();
            let Ok(token) = parser.next_including_whitespace_and_comments() else {
                break;
            };
            match token {
                Token::Dimension { unit, .. } if unit.eq_ignore_ascii_case("px") => {
                    let end = parser.position().byte_index();
                    if let Some((number, _)) = dimension(&value[start..end]) {
                        edits.push((
                            start..end,
                            format_args!("{}rem", number / root).to_lean_string(),
                        ));
                    }
                }
                Token::Function(name) if !name.eq_ignore_ascii_case("url") => {
                    let _ = parser.parse_nested_block(|nested| {
                        collect(nested, value, root, edits);
                        Ok::<_, ParseError<()>>(())
                    });
                }
                _ => {}
            }
        }
    }
    let mut edits = Vec::new();
    collect(&mut Parser::new(value), value, root, &mut edits);
    let mut result = value.to_lean_string();
    for (range, replacement) in edits.into_iter().rev() {
        result.replace_range(range, &replacement);
    }
    result
}
