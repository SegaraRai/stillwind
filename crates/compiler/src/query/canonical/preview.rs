//! Renders canonical previews and normalizes their CSS under one query's spacing policy.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;
use std::ops::Range;
use std::sync::{Arc, Mutex, PoisonError};

use cssparser::{ParseError, Parser, Token};
use lean_string::{LeanStr, LeanString, ToLeanString};

use stillwind_css::{
    CssMinifyOptions, CssRule, Declaration, fold_calc_expressions, inspect_css_value, minify_css,
};
use stillwind_design_system::UtilityResolutionError;
use stillwind_source::{Fingerprint, ReplaceText};
use stillwind_syntax::{decode_arbitrary_value, print_arbitrary_value};

use crate::compiler::FLATTEN_COLOR_COMPATIBILITY;
use crate::options::PACKAGE_TAILWIND_PREFIX;
use crate::query::QueryContext;
use crate::query::theme::ScaleReference;
use crate::{GeneratedRule, RootFontSize};

/// The changes that a query applies to CSS before it compares two candidates. Each
/// query uses fixed values of these changes.
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub(super) struct Normalization {
    /// Whether to substitute the owning query's theme scales.
    pub(super) resolve_scale: bool,
    /// Property names whose pixel dimensions use the query's root font size.
    pub(super) properties: BTreeSet<LeanStr>,
    /// Whether a selector must keep its specificity during normalization.
    pub(super) preserve_specificity: bool,
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
    /// Whether selectors must keep their specificity. The value comes from the query
    /// that created the preview.
    preserve_specificity: bool,
    /// The internal custom properties that comparison removes. The value comes from the
    /// query that created the preview.
    composition: Arc<Composition>,
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
            preserve_specificity: self.preserve_specificity,
        };
        if let Some(cached) = self
            .signatures
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&key)
        {
            return cached.clone();
        }
        let result = self.resolved_css.as_deref().and_then(|css| {
            normalized_signature(
                css,
                scales,
                properties,
                root_font_size,
                self.preserve_specificity,
                &self.composition,
            )
        });
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
    preserve_specificity: bool,
    composition: &Composition,
) -> Option<LeanStr> {
    let mut normalized = css.to_lean_string();
    for scale in scales {
        normalized = normalized.replace_text(&scale.reference, &scale.value);
    }
    if !properties.is_empty() {
        normalized = convert_spacing_declarations(&normalized, properties, root_font_size.get(), 0);
    }
    normalized = fold_calc_expressions(&normalized, None);
    normalized = composition_declarations(&normalized, composition, 0).into();
    if normalized.contains(":is(")
        || normalized.contains(":not(")
        || normalized.contains('[')
        || (!preserve_specificity && normalized.contains(":where("))
    {
        normalized =
            super::selectors::selector_signatures(&normalized, preserve_specificity, 0).into();
    }
    minify_css(&normalized, &CssMinifyOptions::default())
        .ok()
        .map(|output| LeanStr::from(output.css))
}

/// Internal custom properties that only repeat the value of a standard property.
///
/// A utility definition can set an internal custom property and a standard property to
/// the same value, as `leading-*` sets `--tw-leading` and `line-height`. Comparison
/// removes such a custom property when the standard property has the same value,
/// because the custom property adds no CSS of its own. The pairs come from the utility
/// definitions of the snapshot.
#[derive(Debug, Default)]
pub(in crate::query) struct Composition {
    /// The prefix of internal custom properties in the generated CSS, such as `--tw-`.
    pub(super) prefix: LeanStr,
    /// Internal custom properties that utility definitions declare or the design system
    /// registers, with `prefix`. A configured prefix can also name Stillwind's own
    /// properties, so it does not identify these properties by itself.
    pub(super) internal: BTreeSet<LeanStr>,
    /// Maps each internal custom property, with `prefix`, to the standard property that
    /// receives the same value in some utility definition.
    pub(super) shadows: BTreeMap<LeanStr, LeanStr>,
}

impl Composition {
    /// Finds the pairs in the utility definitions of `query`.
    fn prepare(query: &QueryContext) -> Self {
        let mut composition = Self {
            prefix: query.options.tailwind_prefix.clone(),
            internal: query
                .design_system
                .theme()
                .properties()
                .iter()
                .filter(|property| property.name.starts_with(PACKAGE_TAILWIND_PREFIX))
                .map(|property| query.options.rewrite_generated(&property.name))
                .collect(),
            shadows: BTreeMap::new(),
        };
        for definition in query
            .design_system
            .utilities()
            .iter()
            .flat_map(|(_, definitions)| definitions)
        {
            each_declaration_list(&definition.body, 0, &mut |declarations| {
                for custom in declarations
                    .iter()
                    .filter(|declaration| declaration.property.starts_with(PACKAGE_TAILWIND_PREFIX))
                {
                    composition
                        .internal
                        .insert(query.options.rewrite_generated(&custom.property));
                    let value = declaration_value(custom);
                    if let Some(standard) = declarations.iter().find(|other| {
                        !other.property.starts_with("--") && declaration_value(other) == value
                    }) {
                        composition.shadows.insert(
                            query.options.rewrite_generated(&custom.property),
                            standard.property.clone(),
                        );
                    }
                }
            });
        }
        composition
    }

    /// Checks whether `declaration` only repeats a standard property among
    /// `declarations`, so that comparison can remove it.
    fn redundant(&self, declaration: &Declaration, declarations: &[Declaration]) -> bool {
        let Some(standard) = self.shadows.get(&declaration.property) else {
            return false;
        };
        let value = declaration_value(declaration);
        declarations.iter().any(|other| {
            other.property == *standard && {
                let other = declaration_value(other);
                other == value || (zero(value) && zero(other))
            }
        })
    }
}

impl QueryContext {
    /// Returns the internal custom properties of this snapshot that only repeat a
    /// standard property. Queries for the same snapshot and prefix share the result.
    pub(in crate::query) fn composition(&self) -> Arc<Composition> {
        self.design_system.analysis(
            Fingerprint::of(format!(
                "canonical-composition/v1\0{}",
                self.options.tailwind_prefix
            )),
            || Composition::prepare(self),
        )
    }
}

/// Calls `visit` with the declarations of each rule in `css` and in its nested rules.
/// `depth` is the nesting level of `css`. CSS that cannot be parsed, and rules nested
/// deeper than 64 levels, are skipped.
fn each_declaration_list(css: &str, depth: usize, visit: &mut impl FnMut(&[Declaration])) {
    if depth > 64 {
        return;
    }
    if let Some(declarations) = Declaration::parse(css) {
        visit(&declarations);
        return;
    }
    if let Ok(rules) = CssRule::parse(css) {
        for body in rules.iter().filter_map(|rule| rule.body.as_deref()) {
            each_declaration_list(body, depth + 1, visit);
        }
    }
}

/// Returns the value of `declaration`, without `;` and the surrounding spaces.
fn declaration_value(declaration: &Declaration) -> &str {
    declaration
        .css
        .split_once(':')
        .map_or("", |(_, value)| value.trim().trim_end_matches(';').trim())
}

/// Checks whether `value` is zero, with or without a unit, such as `0` or `0px`. An
/// `!important` suffix is ignored.
fn zero(value: &str) -> bool {
    let value = value.trim_end_matches("!important").trim();
    let number = value.trim_end_matches(|character: char| character.is_ascii_alphabetic());
    !number.is_empty() && number.parse::<f64>().is_ok_and(|number| number == 0.0)
}

/// Returns `css` without the internal custom properties that `composition` marks as
/// redundant, and with each value written with only the required spaces. For example,
/// `--tw-leading: 1.5` is removed next to `line-height: 1.5`.
///
/// `depth` is the nesting level of `css`. CSS that cannot be parsed, and rules nested
/// deeper than 64 levels, stay as written.
fn composition_declarations(css: &str, composition: &Composition, depth: usize) -> String {
    if depth > 64
        || (!css.contains(composition.prefix.as_str())
            && !css.contains([',', '+', '~', '>', '*', '/'])
            && !css.contains(" - "))
    {
        return css.into();
    }
    if let Some(declarations) = Declaration::parse(css) {
        return declarations
            .iter()
            .filter(|declaration| !composition.redundant(declaration, &declarations))
            .fold(String::new(), |mut output, declaration| {
                let value =
                    decode_arbitrary_value(&print_arbitrary_value(declaration_value(declaration)));
                let _ = write!(output, "{}:{value};", declaration.property);
                output
            });
    }
    CssRule::parse(css).map_or_else(
        |_| css.into(),
        |rules| {
            rules
                .into_iter()
                .map(|rule| {
                    rule.body.as_deref().map_or_else(
                        || rule.prelude.to_string(),
                        |body| {
                            format!(
                                "{}{{{}}}",
                                rule.prelude,
                                composition_declarations(body, composition, depth + 1)
                            )
                        },
                    )
                })
                .collect()
        },
    )
}

/// Returns `required` without the registered internal properties whose declarations
/// `composition_declarations` removes from `css`. `required` must contain the
/// registration requirements of `css`, written with the package prefix or with the
/// prefix of `composition`.
pub(super) fn canonical_requirements(
    css: &str,
    required: Vec<LeanStr>,
    composition: &Composition,
) -> Vec<LeanStr> {
    let prefix = composition.prefix.as_str();
    let internal = |name: &str| {
        name.starts_with(PACKAGE_TAILWIND_PREFIX) || composition.internal.contains(name)
    };
    if !required.iter().any(|name| internal(name)) {
        return required;
    }
    let canonical = composition_declarations(css, composition, 0);
    required
        .into_iter()
        .filter(|name| {
            let rendered = name
                .strip_prefix(PACKAGE_TAILWIND_PREFIX)
                .map_or_else(|| name.to_string(), |suffix| format!("{prefix}{suffix}"));
            !internal(name)
                || canonical.contains(&rendered)
                || !css.contains(&format!("{rendered}:"))
        })
        .collect()
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
            let resolved_css = self.resolve_theme_values(&css);
            let composition = self.composition();
            let required_properties = canonical_requirements(
                resolved_css.as_deref().unwrap_or(&css),
                result.required_properties,
                &composition,
            );
            Arc::new(CanonicalPreview {
                resolved_css,
                css: (css).into(),
                required_properties,
                preserve_specificity: self.canonical_options().preserve_specificity,
                composition,
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
                        || inspect_css_value(&declaration.css)
                            .0
                            .iter()
                            .any(|unit| matches!(unit.as_str(), "px" | "rem"))
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
