//! Canonical spelling uses emitted CSS, property support, and the effective typed lint policy.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex, PoisonError};

use cssparser::{ParseError, Parser, Token};

use stillwind_css::{CssMinifyOptions, CssRule, Declaration, fold_calc_expressions, minify_css};
use stillwind_design_system::UtilityResolutionError;
use stillwind_diagnostics::CanonicalUtilitiesConfig;
use stillwind_syntax::{Utility, parse_candidate};

use super::{DesignSystemQuery, QueryContext, last_top_level};
use crate::GeneratedRule;

mod index;
mod values;
mod variants;

pub(super) use index::CanonicalIndex;

/// Spacing transformations that affect equality within one query's fixed lint policy.
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
struct Normalization {
    /// Whether to substitute the owning query's spacing scale.
    resolve_scale: bool,
    /// Property names whose pixel dimensions use the query's root font size.
    properties: BTreeSet<String>,
}

/// Rendered candidate data reused while this query compares canonical spellings.
#[derive(Debug)]
pub(super) struct CanonicalPreview {
    css: String,
    resolved_css: Option<String>,
    required_properties: Vec<String>,
    // Query clones can move to different threads while sharing these previews.
    signatures: Mutex<BTreeMap<Normalization, Option<Arc<str>>>>,
}

impl CanonicalPreview {
    /// Normalizes this preview using its owning query's fixed scale and lint options.
    /// Only `properties` and the presence of `scale` may vary between calls.
    fn signature(
        &self,
        scale: Option<&str>,
        properties: &BTreeSet<String>,
        options: &CanonicalUtilitiesConfig,
    ) -> Option<Arc<str>> {
        let key = Normalization {
            resolve_scale: scale.is_some(),
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
        let result = self.resolved_css.as_ref().and_then(|css| {
            let mut normalized = css.clone();
            if let Some(scale) = scale {
                normalized = normalized.replace("var(--spacing)", scale);
            }
            if !properties.is_empty() {
                normalized = convert_spacing_declarations(
                    &normalized,
                    properties,
                    options.root_font_size.get(),
                    0,
                );
            }
            normalized = fold_calc_expressions(&normalized, None);
            minify_css(&normalized, &CssMinifyOptions::default())
                .ok()
                .map(|output| Arc::from(output.css))
        });
        self.signatures
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(key, result.clone());
        result
    }
}

impl DesignSystemQuery {
    /// Returns effective conversion options after defaults, CSS severity, and project fields.
    pub fn canonical_options(&self) -> CanonicalUtilitiesConfig {
        self.context.canonical_options()
    }

    /// Returns an equivalent named spelling, or `None` when no replacement is verified.
    /// The original variant, modifier, sign, and importance semantics are retained.
    ///
    /// # Errors
    ///
    /// Returns malformed requirements in the design system.
    pub fn canonical_candidate(&self, raw: &str) -> Result<Option<String>, UtilityResolutionError> {
        // Only arbitrary values, properties, and variants have replacement rules.
        // Arbitrary modifiers alone remain unchanged.
        if !raw.contains('[') {
            return Ok(None);
        }
        if let Some(cached) = self.canonical_cache.borrow().get(raw) {
            return Ok(cached.clone());
        }
        let result = self.find_canonical(raw)?;
        self.canonical_cache
            .borrow_mut()
            .insert(raw.to_owned(), result.clone());
        Ok(result)
    }

    fn find_canonical(&self, raw: &str) -> Result<Option<String>, UtilityResolutionError> {
        let Some(parsed) = parse_candidate(raw) else {
            return Ok(None);
        };
        if matches!(&parsed.utility, Utility::Named { name, .. } if !name.contains('['))
            && !parsed
                .variants
                .iter()
                .any(|variant| variant.raw.starts_with('['))
        {
            return Ok(None);
        }
        let Some(original) = self.context.canonical_preview(raw)? else {
            return Ok(None);
        };
        let options = self.canonical_options();
        let scale = self.spacing_scale();
        let scale_dimension = scale.as_deref().and_then(dimension);
        let split = last_top_level(raw, ':').map_or(0, |index| index + 1);
        let prefix = &raw[..split];
        let utility = raw[split..].trim_matches('!').trim_start_matches('-');
        let suffix = last_top_level(utility, '/').map_or("", |index| &utility[index..]);
        let (root, value, property) = match &parsed.utility {
            Utility::Named { name, .. } => {
                let Some(index) = name.find('[') else {
                    return self.canonical_variant(raw, &original);
                };
                let Some(value) = name[index + 1..].strip_suffix(']') else {
                    return Ok(None);
                };
                (
                    name[..index].trim_end_matches('-').to_owned(),
                    value.replace('_', " "),
                    None,
                )
            }
            Utility::ArbitraryProperty { property, value } => {
                (String::new(), value.clone(), Some(property.as_str()))
            }
        };
        let bare_value = value
            .split_once(':')
            .map_or(value.as_str(), |(_, value)| value);
        let roots = self.canonical_roots(&root, property);
        let base_raw = format!(
            "{}{}",
            if parsed.negative { "-" } else { "" },
            match &parsed.utility {
                Utility::Named { name, .. } => name.clone(),
                Utility::ArbitraryProperty { property, value } => format!("[{property}:{value}]"),
            }
        );
        let base = if base_raw == raw {
            Some(original.clone())
        } else {
            self.context.canonical_preview(&base_raw)?
        };
        let search = CanonicalSearch {
            roots: &roots,
            value: bare_value,
            property,
            negative: parsed.negative,
            modifier: suffix,
        };
        let labels = self.canonical_labels(&search, &original, base.as_deref());
        for label in labels {
            let replacement = format!(
                "{prefix}{}{}{label}{suffix}",
                if parsed.important { "!" } else { "" },
                if parsed.negative { "-" } else { "" }
            );
            if replacement == raw || replacement.len() >= raw.len() {
                continue;
            }
            let Some(preview) = self.context.canonical_preview(&replacement)? else {
                continue;
            };
            if original.required_properties != preview.required_properties {
                continue;
            }
            let conversion = options.px_to_rem
                && scale_dimension.is_some_and(|(_, unit)| unit == "rem")
                && (preview.css.contains("var(--spacing)")
                    || preview.css.contains("var(--spacing-"));
            let mut properties = BTreeSet::new();
            if conversion {
                spacing_properties(&preview.css, &mut properties, 0);
            }
            let signature = original.signature(scale.as_deref(), &properties, &options);
            if signature.is_some()
                && preview.signature(scale.as_deref(), &properties, &options) == signature
            {
                return Ok(Some(replacement));
            }
        }
        self.canonical_variant(raw, &original)
    }

    fn canonical_labels(
        &self,
        search: &CanonicalSearch<'_>,
        original: &CanonicalPreview,
        base: Option<&CanonicalPreview>,
    ) -> Vec<String> {
        let CanonicalSearch {
            roots,
            value: bare_value,
            property,
            negative,
            modifier,
        } = *search;
        let options = self.canonical_options();
        let scale = self.spacing_scale();
        let scale_dimension = scale.as_deref().and_then(dimension);
        let mut labels = BTreeSet::new();
        for root in roots {
            labels.insert(format!("{root}-{bare_value}"));
        }
        if let Some((number, unit)) = dimension(bare_value) {
            for root in roots {
                labels.insert(format!("{root}-{number}"));
            }
            if let Some((step, scale_unit)) = scale_dimension {
                let amount = if unit == scale_unit {
                    Some(number)
                } else if unit == "px" && scale_unit == "rem" && options.px_to_rem {
                    Some(number / options.root_font_size.get())
                } else {
                    None
                };
                if let Some(amount) = amount.filter(|_| step > 0.0) {
                    for root in roots {
                        labels.insert(format!("{root}-{}", amount / step));
                    }
                }
            }
        }
        let index = &self.canonical_index;
        labels.extend(index.value_labels(&self.context, roots, bare_value));
        if let Some(base) = base {
            for record in index.matching(&self.context, base) {
                if let Some(label) =
                    eligible_label(&record, roots, property, negative, &self.context)
                {
                    labels.insert(label.to_owned());
                }
            }
        }
        if !index.errors.is_empty() {
            let properties = declared_properties(&original.css, 0);
            for (label, error) in &index.errors {
                if let Some(label) = eligible_label(label, roots, property, negative, &self.context)
                    && self
                        .context
                        .design_system
                        .utility_search()
                        .definitions_with_prefix(&error.name)
                        .any(|(name, entries)| {
                            name == error.name
                                && entries
                                    .iter()
                                    .any(|entry| !entry.properties.is_disjoint(&properties))
                        })
                {
                    labels.insert(label.to_owned());
                }
            }
        }
        if !modifier.is_empty() {
            // Modifiers can erase value differences or select a branch absent from the base output.
            let preserves_value = index.modifier_preserves_value(roots, modifier);
            labels.extend(
                index
                    .context_labels(roots, negative, preserves_value)
                    .iter()
                    .filter_map(|label| {
                        eligible_label(label, roots, property, negative, &self.context)
                            .map(str::to_owned)
                    }),
            );
        }
        let mut labels: Vec<_> = labels.into_iter().collect();
        labels.sort_by(|left, right| index::rank(left, right));
        labels
    }

    fn canonical_roots(&self, root: &str, property: Option<&str>) -> BTreeSet<String> {
        if !root.is_empty() {
            return BTreeSet::from([root.to_owned()]);
        }
        property
            .into_iter()
            .flat_map(|property| {
                self.context
                    .design_system
                    .utility_search()
                    .names_for_property(property)
            })
            .filter_map(|name| name.strip_suffix("-*").map(str::to_owned))
            .collect()
    }

    fn canonical_variant(
        &self,
        raw: &str,
        original: &CanonicalPreview,
    ) -> Result<Option<String>, UtilityResolutionError> {
        let Some(candidate) = parse_candidate(raw) else {
            return Ok(None);
        };
        if !candidate
            .variants
            .iter()
            .any(|variant| variant.raw.starts_with('['))
        {
            return Ok(None);
        }
        let properties = BTreeSet::new();
        let options = self.canonical_options();
        let Some(signature) = original.signature(None, &properties, &options) else {
            return Ok(None);
        };
        for variant in &candidate.variants {
            if !variant.raw.starts_with('[') {
                continue;
            }
            let names = self
                .canonical_index
                .variants
                .matching(&self.context, raw, &variant.raw);
            for name in names {
                let replacement =
                    raw.replacen(&format!("{}:", variant.raw), &format!("{name}:"), 1);
                if replacement.len() >= raw.len() {
                    continue;
                }
                if let Some(preview) = self.context.canonical_preview(&replacement)?
                    && preview.required_properties == original.required_properties
                    && preview.signature(None, &properties, &options).as_ref() == Some(&signature)
                {
                    return Ok(Some(replacement));
                }
            }
        }
        Ok(None)
    }
}

impl QueryContext {
    pub(super) fn canonical_preview(
        &self,
        raw: &str,
    ) -> Result<Option<Arc<CanonicalPreview>>, UtilityResolutionError> {
        if let Some(cached) = self.canonical_previews.borrow().get(raw) {
            return Ok(cached.clone());
        }
        let result = self.resolve_with_selector(raw, Some("&"), false)?;
        let preview = result.diagnostics.is_empty().then(|| {
            let css: String = result
                .generated_rules
                .iter()
                .map(GeneratedRule::to_css)
                .collect();
            Arc::new(CanonicalPreview {
                resolved_css: self.resolve_theme_values(&css),
                css,
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

struct CanonicalSearch<'a> {
    roots: &'a BTreeSet<String>,
    value: &'a str,
    property: Option<&'a str>,
    negative: bool,
    modifier: &'a str,
}

fn eligible_label<'a>(
    label: &'a str,
    roots: &BTreeSet<String>,
    property: Option<&str>,
    negative: bool,
    query: &QueryContext,
) -> Option<&'a str> {
    let registered = label;
    let label = if negative {
        label.strip_prefix('-')?
    } else {
        if property.is_none() && label.starts_with('-') {
            return None;
        }
        label
    };
    let eligible = roots.iter().any(|root| {
        let Some(capture) = label.strip_prefix(&format!("{root}-")) else {
            return false;
        };
        if query.design_system.utilities().get(registered).is_some() {
            return true;
        }
        let family = format!("{}{root}-*", if negative { "-" } else { "" });
        query
            .design_system
            .utility_search()
            .definitions_with_prefix(&family)
            .any(|(name, entries)| {
                name == family
                    && entries.iter().any(|entry| {
                        entry.value_namespaces.iter().any(|namespace| {
                            query
                                .design_system
                                .tokens()
                                .get(&format!("{namespace}{capture}"))
                                .is_some()
                        })
                    })
            })
    }) || property.is_some_and(|property| {
        query
            .design_system
            .utility_search()
            .names_for_property(property)
            .any(|name| name == registered)
    });
    eligible.then_some(label)
}

fn dimension(value: &str) -> Option<(f64, &str)> {
    let value = value.trim();
    let unit = if value.ends_with("rem") {
        "rem"
    } else if value.ends_with("px") {
        "px"
    } else {
        return None;
    };
    let number = value.strip_suffix(unit)?.parse::<f64>().ok()?;
    number.is_finite().then_some((number, unit))
}

fn declared_properties(css: &str, depth: usize) -> BTreeSet<String> {
    if depth > 64 {
        return BTreeSet::new();
    }
    if let Some(declarations) = Declaration::parse(css) {
        return declarations
            .into_iter()
            .map(|declaration| declaration.property)
            .collect();
    }
    CssRule::parse(css).map_or_else(
        |_| BTreeSet::new(),
        |rules| {
            rules
                .into_iter()
                .filter_map(|rule| rule.body)
                .flat_map(|body| declared_properties(&body, depth + 1))
                .collect()
        },
    )
}

fn spacing_properties(css: &str, properties: &mut BTreeSet<String>, depth: usize) {
    if depth > 64 {
        return;
    }
    if let Some(declarations) = Declaration::parse(css) {
        properties.extend(
            declarations
                .into_iter()
                .filter(|declaration| {
                    declaration.css.contains("var(--spacing)")
                        || declaration.css.contains("var(--spacing-")
                })
                .map(|declaration| declaration.property),
        );
    } else if let Ok(rules) = CssRule::parse(css) {
        for rule in rules {
            if let Some(body) = rule.body {
                spacing_properties(&body, properties, depth + 1);
            }
        }
    }
}

fn convert_spacing_declarations(
    css: &str,
    properties: &BTreeSet<String>,
    root: f64,
    depth: usize,
) -> String {
    if depth > 64 {
        return css.to_owned();
    }
    if let Some(declarations) = Declaration::parse(css) {
        return declarations
            .into_iter()
            .map(|declaration| {
                if properties.contains(&declaration.property)
                    && let Some((name, value)) = declaration.css.split_once(':')
                {
                    format!("{name}:{};", convert_pixel_dimensions(value, root))
                } else {
                    format!("{};", declaration.css)
                }
            })
            .collect();
    }
    CssRule::parse(css).map_or_else(
        |_| css.to_owned(),
        |rules| {
            rules
                .into_iter()
                .map(|mut rule| {
                    if let Some(body) = &rule.body {
                        rule.body = Some(convert_spacing_declarations(
                            body,
                            properties,
                            root,
                            depth + 1,
                        ));
                    }
                    rule.to_css()
                })
                .collect()
        },
    )
}

fn convert_pixel_dimensions(value: &str, root: f64) -> String {
    fn collect(
        parser: &mut Parser<'_>,
        value: &str,
        root: f64,
        edits: &mut Vec<(std::ops::Range<usize>, String)>,
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
                        edits.push((start..end, format!("{}rem", number / root)));
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
    let mut result = value.to_owned();
    for (range, replacement) in edits.into_iter().rev() {
        result.replace_range(range, &replacement);
    }
    result
}

#[cfg(test)]
mod tests {
    use stillwind_design_system::{DesignSystemOptions, DesignSystemSnapshot};
    use stillwind_diagnostics::{CanonicalUtilitiesOverride, LintOverrides, RootFontSize};
    use stillwind_source::{CssSourceName, FileId};
    use stillwind_test_support::css;

    use crate::{CompilerOptions, DesignSystemQuery};

    fn make_query(css_text: &str, options: CompilerOptions) -> DesignSystemQuery {
        let source = css::source(FileId(1), css_text).unwrap();
        let snapshot = DesignSystemSnapshot::from_source(
            &source,
            CssSourceName::from("theme.css"),
            DesignSystemOptions::default(),
        )
        .unwrap();
        DesignSystemQuery::new(snapshot, options)
    }

    #[test]
    fn canonical_spacing_respects_units_and_preserves_variants_modifiers_and_importance() {
        let query = make_query(
            "@import 'stillwind'; @theme { --spacing: .25rem; }",
            CompilerOptions::default(),
        );
        for (raw, expected) in [
            ("p-[16px]", "p-4"),
            ("p-[1rem]", "p-4"),
            ("border-[2px]", "border-2"),
            ("hover:!p-[16px]", "hover:!p-4"),
            ("[padding:16px]", "p-4"),
        ] {
            assert_eq!(
                query.canonical_candidate(raw).unwrap(),
                Some(expected.to_owned()),
                "{raw}"
            );
            assert!(query.canonical_candidate(expected).unwrap().is_none());
        }
        let query = make_query(
            "@import 'stillwind'; @theme { --spacing: .25rem; }",
            CompilerOptions {
                lint: LintOverrides {
                    canonical_utilities: Some(CanonicalUtilitiesOverride {
                        px_to_rem: Some(false),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        assert!(query.canonical_candidate("p-[16px]").unwrap().is_none());
        assert_eq!(
            query.canonical_candidate("border-[2px]").unwrap(),
            Some("border-2".into())
        );
    }

    #[test]
    fn canonical_queries_use_project_font_size_and_the_actual_custom_definition() {
        let query = make_query(
            "@import 'stillwind'; @theme { --spacing: .25rem; }",
            CompilerOptions {
                lint: LintOverrides {
                    canonical_utilities: Some(CanonicalUtilitiesOverride {
                        root_font_size: RootFontSize::new(20.5),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        assert_eq!(
            query.canonical_candidate("p-[20.5px]").unwrap(),
            Some("p-4".into())
        );
        let query = make_query(
            "@import 'stillwind'; @theme { --spacing: .25rem; } @utility p-4 { padding: 2rem; }",
            CompilerOptions::default(),
        );
        assert!(query.canonical_candidate("p-[16px]").unwrap().is_none());
    }

    #[test]
    fn spacing_conversion_preserves_selectors_strings_and_nonspacing_declarations() {
        let query = make_query(
            "@theme { --spacing:.25rem; } @utility p-* { padding:--value(--spacing(), [length]); content:'16px'; border-width:2px; } @utility p-4 { padding:calc(var(--spacing) * 4); content:'1rem'; border-width:2px; }",
            CompilerOptions::default(),
        );
        assert!(query.canonical_candidate("p-[16px]").unwrap().is_none());
        assert_eq!(
            super::convert_pixel_dimensions("calc(16px + 2px) '16px' url(16px)", 16.0),
            "calc(1rem + 0.125rem) '16px' url(16px)"
        );
    }

    #[test]
    fn canonical_replacements_resolve_named_spacing_and_color_tokens() {
        let query = make_query(
            "@theme { --spacing:.25rem; --spacing-4:1rem; --color-brand:red; } @utility p-* { padding:--value(--spacing-*, --spacing(), [length]); } @utility bg-* { background-color:--value(--color-*, [color]); }",
            CompilerOptions::default(),
        );
        assert_eq!(
            query.canonical_candidate("p-[16px]").unwrap(),
            Some("p-4".into())
        );
        assert_eq!(
            query.canonical_candidate("bg-[#f00]").unwrap(),
            Some("bg-brand".into())
        );
    }

    #[test]
    fn canonical_overloads_search_tokens_for_the_emitted_properties() {
        let query = make_query(
            "@import 'stillwind'; @theme { --spacing:.25rem; }",
            CompilerOptions::default(),
        );
        assert!(
            query
                .canonical_candidate("border-[line-width:thin]")
                .unwrap()
                .is_none()
        );
        assert!(
            query
                .canonical_candidate("p-[length-percentage:calc(3px_+_50%)]")
                .unwrap()
                .is_none()
        );
        let query = make_query(
            "@theme { --color-brand:red; --spacing-x:1px; } @utility tone-* { border-width:--value(--spacing-*, [length]); } @utility tone-* { border-color:--value(--color-*, [color]); }",
            CompilerOptions::default(),
        );
        assert_eq!(
            query.canonical_candidate("tone-[1px]").unwrap(),
            Some("tone-x".into())
        );
        assert_eq!(
            query.canonical_candidate("tone-[#f00]").unwrap(),
            Some("tone-brand".into())
        );
    }

    #[test]
    fn canonical_filter_keeps_arbitrary_utilities_and_variants() {
        let query = make_query(
            "@theme { --spacing:.25rem; } @utility p-* { padding:--value(--spacing(), [length]); } @variant over { &:hover { @slot; } }",
            CompilerOptions::default(),
        );
        assert_eq!(
            query.canonical_candidate("[&:hover]:p-4").unwrap(),
            Some("over:p-4".into())
        );
        assert_eq!(
            query.canonical_candidate("over:!p-[16px]").unwrap(),
            Some("over:!p-4".into())
        );
        for raw in ["p-4", "over:p-4", "p-4/[2]", "invalid[", "p-(--spacing)"] {
            assert!(query.canonical_candidate(raw).unwrap().is_none(), "{raw}");
        }
    }

    #[test]
    fn canonical_comparisons_keep_spacing_transformations_separate() {
        let query = make_query(
            "@theme { --spacing:.25rem; } @utility p-* { padding:--value(--spacing(), [length]); } @utility p-0 { padding:2rem; } @utility p-1 { padding:calc(var(--spacing) * 4); }",
            CompilerOptions::default(),
        );
        for raw in ["p-[16px]", "p-[1rem]", "[padding:16px]"] {
            assert_eq!(
                query.canonical_candidate(raw).unwrap(),
                Some("p-1".into()),
                "{raw}"
            );
        }
        assert_eq!(
            query.canonical_candidate("p-[15px]").unwrap(),
            Some("p-3.75".into())
        );
    }

    #[test]
    fn canonical_search_preserves_nested_properties_and_overload_domains() {
        let query = make_query(
            "@theme { --color-brand:red; --spacing-x:1px; } @utility tone-* { &:hover { border-width:--value(--spacing-*, [length]); } } @utility tone-* { &:hover { border-color:--value(--color-*, [color]); } } @utility paint { &:hover { --paint:red; } }",
            CompilerOptions::default(),
        );
        assert_eq!(
            query.canonical_candidate("tone-[#f00]").unwrap(),
            Some("tone-brand".into())
        );
        assert_eq!(
            query.canonical_candidate("tone-[1px]").unwrap(),
            Some("tone-x".into())
        );
        assert!(
            query
                .canonical_candidate("[--paint:red]")
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn canonical_variants_require_a_normalizable_value() {
        let query = make_query(
            "@theme { --color-loop:var(--color-loop); } @utility bg-* { background-color:--value(--color-*, [color]); } @variant over { &:hover { @slot; } }",
            CompilerOptions::default(),
        );
        assert!(
            query
                .canonical_candidate("[&:hover]:bg-loop")
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn cloned_queries_can_reuse_previews_on_another_thread() {
        let query = make_query("@theme { --color-brand:red; }", CompilerOptions::default());
        assert_eq!(
            query.canonical_candidate("bg-[#f00]").unwrap(),
            Some("bg-brand".into())
        );
        let cloned = query.clone();
        let result =
            std::thread::spawn(move || cloned.canonical_candidate("bg-[rgb(255_0_0)]").unwrap())
                .join()
                .unwrap();
        assert_eq!(result, Some("bg-brand".into()));
        assert_eq!(
            query.canonical_candidate("bg-[rgb(255_0_0)]").unwrap(),
            result
        );
    }
    #[test]
    fn canonical_templates_keep_static_aliases_quotes_and_requirements() {
        let query = make_query(
            "@theme { --*:initial; --color-a:red; --color-b:blue; --color-alias:var(--color-a); } @property --flag { syntax:'*'; inherits:false; initial-value:0; } @utility tone-* { @stillwind-require-properties --flag; &:hover { color:--value(--color-*, [color]); outline-color:var(--color-alias); content:'var(--color-a)'; } }",
            CompilerOptions::default(),
        );
        for (raw, expected) in [("tone-[#f00]", "tone-a"), ("tone-[#00f]", "tone-b")] {
            assert_eq!(
                query.canonical_candidate(raw).unwrap(),
                Some(expected.to_owned()),
                "{raw}"
            );
        }
    }

    #[test]
    fn canonical_indexes_share_preparation_and_isolate_lint_policies() {
        let source = css::source(FileId(1), "@theme { --*:initial; --spacing:.25rem; } @utility p-* { padding:--value(--spacing(), [length]); }").unwrap();
        let snapshot = DesignSystemSnapshot::from_source(
            &source,
            CssSourceName::from("theme.css"),
            DesignSystemOptions::default(),
        )
        .unwrap();
        crate::prepare_canonical_index(&snapshot, &CompilerOptions::default());
        let prepared =
            crate::query::QueryContext::new(snapshot.clone(), CompilerOptions::default())
                .canonical_index();
        let first = DesignSystemQuery::new(snapshot.clone(), CompilerOptions::default());
        assert!(std::sync::Arc::ptr_eq(&prepared, &first.canonical_index));
        let second = DesignSystemQuery::new(
            snapshot.clone(),
            CompilerOptions {
                lint: LintOverrides {
                    canonical_utilities: Some(CanonicalUtilitiesOverride {
                        severity: Some(stillwind_diagnostics::LintSeverity::Deny),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                class_namespace: "another-project".into(),
                naming_revision: "another-generator".into(),
                hash_length: 12,
                ..Default::default()
            },
        );
        assert!(std::sync::Arc::ptr_eq(
            &first.canonical_index,
            &second.canonical_index
        ));
        let disabled = DesignSystemQuery::new(
            snapshot,
            CompilerOptions {
                lint: LintOverrides {
                    canonical_utilities: Some(CanonicalUtilitiesOverride {
                        px_to_rem: Some(false),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        assert_eq!(
            first.canonical_candidate("p-[16px]").unwrap(),
            Some("p-4".into())
        );
        assert!(disabled.canonical_candidate("p-[16px]").unwrap().is_none());
    }

    #[test]
    fn preparation_defers_requirement_errors_until_a_matching_candidate() {
        let source = css::source(FileId(1),
            "@utility good { color:red; } @utility bad-* { @stillwind-require-properties invalid; color:--value([color]); }").unwrap();
        let snapshot = DesignSystemSnapshot::from_source(
            &source,
            CssSourceName::from("theme.css"),
            DesignSystemOptions::default(),
        )
        .unwrap();
        crate::prepare_canonical_index(&snapshot, &CompilerOptions::default());
        let query = DesignSystemQuery::new(snapshot, CompilerOptions::default());
        assert_eq!(
            query.resolve_candidate("good").unwrap().diagnostics,
            [] as [stillwind_diagnostics::LintFinding; 0]
        );
        assert_eq!(
            query.canonical_candidate("bad-[red]").unwrap_err().name,
            "bad-*"
        );
    }

    #[test]
    fn canonical_modifiers_keep_branches_that_do_not_have_base_output() {
        let query = make_query(
            "@theme { --*:initial; --color-a:red; --color-b:blue; --text-body:1rem; --leading-tight:1.25rem; } @utility text-* { font-size:--value(--text-*, [length]); line-height:--modifier(--leading-*, [length]); } @utility tone-* { color:--value(--color-*, [color]); opacity:--modifier(number); } @utility flat-* { color:red; opacity:calc(--modifier(number) * 0 + 1); --x:calc(--value(--color-*, [color]) * 0); }",
            CompilerOptions::default(),
        );
        assert_eq!(
            query.canonical_candidate("text-[1rem]/tight").unwrap(),
            Some("text-body/tight".into())
        );
        assert_eq!(
            query.canonical_candidate("tone-[#f00]/50").unwrap(),
            Some("tone-a/50".into())
        );
        assert!(
            query
                .canonical_candidate("tone-[#123456]/50")
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn canonical_indexes_keep_shadowing_families_and_exact_overrides() {
        let query = make_query(
            "@theme { --*:initial; --color-special-a:blue; --color-a:red; --color-b:blue; --color-empty:transparent; } @utility tone-* { color:--value(--color-*, [color]); } @utility tone-special-* { color:--value(--color-*, [color]); } @utility tone-a { color:blue; }",
            CompilerOptions::default(),
        );
        assert_eq!(
            query.canonical_candidate("tone-[#00f]").unwrap(),
            Some("tone-a".into())
        );
        assert!(query.canonical_candidate("tone-[#f00]").unwrap().is_none());
    }

    #[test]
    fn canonical_modifiers_keep_overlapping_functional_branches() {
        let query = make_query(
            "@theme { --*:initial; --color-a:red; --color-special-a:blue; } @utility tone-* { color:--value(--color-*, [color]); opacity:--modifier(number); } @utility tone-special-* { color:--value(--color-*, [color]); opacity:--modifier(number); } @utility tone-a { color:blue; opacity:--modifier(number); }",
            CompilerOptions::default(),
        );
        assert_eq!(
            query.canonical_candidate("tone-[rgb(255_0_0)]/50").unwrap(),
            Some("tone-special-a/50".into())
        );
    }

    #[test]
    fn canonical_search_matches_finite_css_reference() {
        let query = make_query(
            "@theme { --*:initial; --spacing:.25rem; --spacing-a:1rem; --color-a:red; --color-b:blue; --color-c:rgb(255 0 0); --color-alias:var(--color-a); --text-body:1rem; --leading-tight:1.25rem; } @utility tone-* { color:--value(--color-*, [color]); opacity:--modifier(number); } @utility plain-* { color:--value(--color-*, [color]); } @utility text-* { font-size:--value(--text-*, [length]); line-height:--modifier(--leading-*, [length]); } @utility nested-* { &:hover { color:--value(--color-*, [color]); outline-color:var(--color-alias); } } @utility padded-* { padding:--value(--spacing-*, [length]); }",
            CompilerOptions::default(),
        );
        let index = &query.canonical_index;
        for (root, value, modifier) in [
            ("tone", "#f00", "/50"),
            ("tone", "#123456", "/50"),
            ("plain", "rgb(255_0_0)", ""),
            ("plain", "#00f", ""),
            ("nested", "#00f", ""),
            ("padded", "1rem", ""),
            ("text", "1rem", "/tight"),
        ] {
            let raw = format!("{root}-[{value}]{modifier}");
            let original = query.context.canonical_preview(&raw).unwrap().unwrap();
            let options = query.canonical_options();
            let scale = query.spacing_scale();
            let mut labels: Vec<_> = index
                .labels
                .iter()
                .filter(|label| label.starts_with(&format!("{root}-")))
                .cloned()
                .collect();
            if value == "1rem" {
                labels.push(format!("{root}-4"));
            }
            labels.sort_by(|left, right| super::index::rank(left, right));
            let expected = labels.into_iter().find_map(|label| {
                let replacement = format!("{label}{modifier}");
                if replacement.len() >= raw.len() {
                    return None;
                }
                let preview = query.context.canonical_preview(&replacement).unwrap()?;
                let normalization = super::index::policy(&preview, &options, scale.as_deref());
                let signature =
                    original.signature(scale.as_deref(), &normalization.properties, &options);
                (signature.is_some()
                    && preview.required_properties == original.required_properties
                    && preview.signature(scale.as_deref(), &normalization.properties, &options)
                        == signature)
                    .then_some(replacement)
            });
            assert_eq!(query.canonical_candidate(&raw).unwrap(), expected, "{raw}");
        }
    }
}
