//! Canonical spelling uses emitted CSS, property support, and the effective typed lint policy.

use std::collections::BTreeSet;

use cssparser::{ParseError, Parser, Token};

use stillwind_css::{CssMinifyOptions, CssRule, Declaration, fold_calc_expressions, minify_css};
use stillwind_design_system::UtilityResolutionError;
use stillwind_diagnostics::{CanonicalUtilitiesConfig, LintConfig, LintKind};
use stillwind_syntax::{Utility, parse_candidate};

use super::{DesignSystemQuery, last_top_level};

impl DesignSystemQuery {
    /// Returns effective conversion options after defaults, CSS severity, and project fields.
    pub fn canonical_options(&self) -> CanonicalUtilitiesConfig {
        match self.lint_settings().get(LintKind::CanonicalUtilities) {
            Some(LintConfig::CanonicalUtilities(config)) => config.clone(),
            _ => CanonicalUtilitiesConfig::default(),
        }
    }

    /// Returns an equivalent named spelling, or `None` when no replacement is verified.
    /// The original variant, modifier, sign, and importance semantics are retained.
    ///
    /// # Errors
    ///
    /// Returns malformed requirements in the design system.
    pub fn canonical_candidate(&self, raw: &str) -> Result<Option<String>, UtilityResolutionError> {
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
        if !raw.contains('[') {
            return Ok(None);
        }
        let Some(parsed) = parse_candidate(raw) else {
            return Ok(None);
        };
        let Some(original) = self.hover_candidate(raw)? else {
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
                    return self.canonical_variant(raw, &original.css);
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
        let properties = declared_properties(&original.css, 0);
        let labels = self.canonical_labels(
            &root,
            bare_value,
            property,
            &properties,
            &options,
            scale_dimension,
        );
        for label in labels {
            let replacement = format!(
                "{prefix}{}{}{label}{suffix}",
                if parsed.important { "!" } else { "" },
                if parsed.negative { "-" } else { "" }
            );
            if replacement == raw || replacement.len() >= raw.len() {
                continue;
            }
            let Some(preview) = self.hover_candidate(&replacement)? else {
                continue;
            };
            let conversion = options.px_to_rem
                && scale_dimension.is_some_and(|(_, unit)| unit == "rem")
                && (preview.css.contains("var(--spacing)")
                    || preview.css.contains("var(--spacing-"));
            let mut properties = BTreeSet::new();
            if conversion {
                spacing_properties(&preview.css, &mut properties, 0);
            }
            let signature = self.canonical_signature(
                raw,
                &original.css,
                scale.as_deref(),
                &properties,
                &options,
            )?;
            if signature.is_some()
                && self.canonical_signature(
                    &replacement,
                    &preview.css,
                    scale.as_deref(),
                    &properties,
                    &options,
                )? == signature
            {
                return Ok(Some(replacement));
            }
        }
        self.canonical_variant(raw, &original.css)
    }

    fn canonical_labels(
        &self,
        root: &str,
        bare_value: &str,
        property: Option<&str>,
        properties: &BTreeSet<String>,
        options: &CanonicalUtilitiesConfig,
        scale_dimension: Option<(f64, &str)>,
    ) -> Vec<String> {
        let mut labels = BTreeSet::new();
        let mut roots = BTreeSet::new();
        if !root.is_empty() {
            roots.insert(root.to_owned());
            labels.insert(format!("{root}-{bare_value}"));
        } else if let Some(property) = property {
            for (name, definitions) in self.design_system.utilities().iter() {
                if definitions
                    .iter()
                    .any(|definition| defines_property(&definition.body, property, 0))
                {
                    if let Some(root) = name.strip_suffix("-*") {
                        roots.insert(root.to_owned());
                    } else if !name.contains('*') {
                        labels.insert(name.to_owned());
                    }
                }
            }
        }
        for root in &roots {
            for (name, definitions) in self.design_system.utilities().iter() {
                if !name.starts_with(&format!("{root}-")) {
                    continue;
                }
                for definition in definitions.iter().filter(|definition| {
                    properties
                        .iter()
                        .any(|property| defines_property(&definition.body, property, 0))
                }) {
                    if name == format!("{root}-*") {
                        for namespace in
                            super::completion::value_namespaces(&definition.body, "--value")
                        {
                            for token in self.design_system.theme().active_tokens() {
                                if let Some(value) = token.name.value.strip_prefix(&namespace) {
                                    labels.insert(format!("{root}-{value}"));
                                }
                            }
                        }
                    } else if !name.contains('*') {
                        labels.insert(name.to_owned());
                    }
                }
            }
        }
        if let Some((number, unit)) = dimension(bare_value) {
            for root in &roots {
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
                    for root in &roots {
                        labels.insert(format!("{root}-{}", amount / step));
                    }
                }
            }
        }
        let mut labels: Vec<_> = labels.into_iter().collect();
        labels.sort_by(|a, b| a.len().cmp(&b.len()).then_with(|| a.cmp(b)));
        labels
    }

    fn canonical_variant(
        &self,
        raw: &str,
        css: &str,
    ) -> Result<Option<String>, UtilityResolutionError> {
        let Some(candidate) = parse_candidate(raw) else {
            return Ok(None);
        };
        for variant in &candidate.variants {
            if !variant.raw.starts_with('[') {
                continue;
            }
            for name in self
                .design_system
                .variants()
                .names()
                .filter(|name| !name.contains('*'))
            {
                let replacement =
                    raw.replacen(&format!("{}:", variant.raw), &format!("{name}:"), 1);
                if replacement.len() >= raw.len() {
                    continue;
                }
                if let Some(preview) = self.hover_candidate(&replacement)?
                    && self.canonical_signature(
                        raw,
                        css,
                        None,
                        &BTreeSet::new(),
                        &self.canonical_options(),
                    )? == self.canonical_signature(
                        &replacement,
                        &preview.css,
                        None,
                        &BTreeSet::new(),
                        &self.canonical_options(),
                    )?
                {
                    return Ok(Some(replacement));
                }
            }
        }
        Ok(None)
    }

    fn canonical_signature(
        &self,
        raw: &str,
        css: &str,
        scale: Option<&str>,
        properties: &BTreeSet<String>,
        options: &CanonicalUtilitiesConfig,
    ) -> Result<Option<(String, Vec<String>)>, UtilityResolutionError> {
        let Some(mut normalized) = self.resolve_theme_values(css) else {
            return Ok(None);
        };
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
        let Ok(output) = minify_css(&normalized, &CssMinifyOptions::default()) else {
            return Ok(None);
        };
        let result = self.resolve_candidate(raw)?;
        Ok(Some((output.css, result.required_properties)))
    }
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

fn defines_property(body: &str, property: &str, depth: usize) -> bool {
    if depth > 64 || !body.contains(property) {
        return false;
    }
    if let Some(declarations) = Declaration::parse(body) {
        return declarations
            .iter()
            .any(|declaration| declaration.property == property);
    }
    CssRule::parse(body).is_ok_and(|rules| {
        rules.iter().any(|rule| {
            rule.body
                .as_deref()
                .is_some_and(|body| defines_property(body, property, depth + 1))
        })
    })
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
}
