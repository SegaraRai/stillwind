//! Finds shorter canonical spellings and verifies their CSS and property requirements.

use std::collections::BTreeSet;

use lean_string::{LeanStr, LeanString, ToLeanStr};

use stillwind_css::{CssRule, Declaration};
use stillwind_design_system::{UtilityResolutionError, named_domain_values};
use stillwind_diagnostics::CanonicalUtilitiesConfig;
use stillwind_source::ReplaceText;
use stillwind_syntax::{Utility, parse_candidate};

use super::conditions::canonical_conditions;
use super::index::{rank, uses_rem};
use super::preview::{CanonicalPreview, SpacingUnit, dimension, spacing_properties};
use crate::compiler::candidate_allowed;
use crate::query::{DesignSystemQuery, QueryContext, last_top_level};

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
    pub fn canonical_candidate(
        &self,
        raw: &str,
    ) -> Result<Option<LeanStr>, UtilityResolutionError> {
        // Runtime conditions have one canonical spelling, independent of other rules.
        if let Some(canonical) = canonical_conditions(raw, self.context.design_system.theme()) {
            return Ok(Some((canonical).into()));
        }
        // Only arbitrary values, properties, and variants have replacement rules.
        // Arbitrary modifiers alone remain unchanged.
        if !raw.contains('[') {
            return Ok(None);
        }
        if let Some(cached) = self.canonical_cache.borrow().get(raw) {
            return Ok(cached.clone());
        }
        let result = match self.dynamic_named_candidate(raw) {
            Some(named) => Some(named),
            None => self.find_canonical(raw)?,
        }
        .filter(|candidate| {
            candidate_allowed(
                candidate,
                self.context.design_system.theme(),
                &self.lint_settings(),
            )
        });
        self.canonical_cache
            .borrow_mut()
            .insert(raw.to_lean_str(), result.clone());
        Ok(result)
    }

    /// Returns `raw` with its utility name replaced by the named spelling when the name
    /// is the bracket form of a dynamic family that accepts both forms. That family
    /// resolves both spellings to one definition.
    fn dynamic_named_candidate(&self, raw: &str) -> Option<LeanStr> {
        let Utility::Named { name, .. } = parse_candidate(raw)?.utility else {
            return None;
        };
        let named = self
            .context
            .design_system
            .theme()
            .dynamic_named_utility(&name)?;
        let split = last_top_level(raw, ':').map_or(0, |index| index + 1);
        Some(
            format_args!(
                "{}{}",
                &raw[..split],
                raw[split..].replacen(name.as_str(), &named, 1)
            )
            .to_lean_str(),
        )
    }

    fn find_canonical(&self, raw: &str) -> Result<Option<LeanStr>, UtilityResolutionError> {
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
        let scales = self.context.theme_scales();
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
                    name[..index].trim_end_matches('-').to_lean_str(),
                    value.replace_char('_', " ").into(),
                    None,
                )
            }
            Utility::ArbitraryProperty { property, value } => {
                (LeanStr::new(), value.to_lean_str(), Some(property.as_str()))
            }
        };
        let bare_value = value
            .split_once(':')
            .map_or(value.as_str(), |(_, value)| value);
        let roots = self.canonical_roots(&root, property);
        let base_raw = format_args!(
            "{}{}",
            if parsed.negative { "-" } else { "" },
            match &parsed.utility {
                Utility::Named { name, .. } => name.to_lean_str(),
                Utility::ArbitraryProperty { property, value } =>
                    format_args!("[{property}:{value}]").to_lean_str(),
            }
        )
        .to_lean_str();
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
            let replacement = format_args!(
                "{prefix}{}{}{label}{suffix}",
                if parsed.important { "!" } else { "" },
                if parsed.negative { "-" } else { "" }
            )
            .to_lean_str();
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
                && scales
                    .iter()
                    .any(|scale| uses_rem(scale) && scale.referenced_by(&preview.css));
            let mut properties = BTreeSet::new();
            if conversion {
                spacing_properties(&preview.css, &scales, &mut properties, 0);
            }
            let root = self.context.options.root_font_size;
            let signature = original.signature(&scales, &properties, root);
            if signature.is_some() && preview.signature(&scales, &properties, root) == signature {
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
    ) -> Vec<LeanStr> {
        let CanonicalSearch {
            roots,
            value: bare_value,
            property,
            negative,
            modifier,
        } = *search;
        let options = self.canonical_options();
        let scales = self.context.theme_scales();
        let mut labels = BTreeSet::new();
        for root in roots {
            labels.insert(format_args!("{root}-{bare_value}").to_lean_str());
        }
        if let Some((number, unit)) = dimension(bare_value) {
            for root in roots {
                labels.insert(format_args!("{root}-{number}").to_lean_str());
            }
            for (step, scale_unit) in scales.iter().filter_map(|scale| dimension(&scale.value)) {
                let amount = if unit == scale_unit {
                    Some(number)
                } else if unit == SpacingUnit::Px
                    && scale_unit == SpacingUnit::Rem
                    && options.px_to_rem
                {
                    Some(number / self.context.options.root_font_size.get())
                } else {
                    None
                };
                if let Some(amount) = amount.filter(|_| step > 0.0) {
                    for root in roots {
                        labels.insert(format_args!("{root}-{}", amount / step).to_lean_str());
                    }
                }
            }
        }
        let index = self.canonical_index();
        labels.extend(index.value_labels(&self.context, roots, bare_value));
        if let Some(base) = base {
            for record in index.matching(&self.context, base) {
                if let Some(label) =
                    eligible_label(&record, roots, property, negative, &self.context)
                {
                    labels.insert(label.to_lean_str());
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
                    labels.insert(label.to_lean_str());
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
                            .map(LeanStr::from)
                    }),
            );
        }
        let mut labels: Vec<_> = labels.into_iter().collect();
        labels.sort_by(|left, right| rank(left, right));
        labels
    }

    fn canonical_roots(&self, root: &str, property: Option<&str>) -> BTreeSet<LeanStr> {
        if !root.is_empty() {
            return BTreeSet::from([root.to_lean_str()]);
        }
        property
            .into_iter()
            .flat_map(|property| {
                self.context
                    .design_system
                    .utility_search()
                    .names_for_property(property)
            })
            .filter_map(|name| name.strip_suffix("-*").map(LeanStr::from))
            .collect()
    }

    fn canonical_variant(
        &self,
        raw: &str,
        original: &CanonicalPreview,
    ) -> Result<Option<LeanStr>, UtilityResolutionError> {
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
        let Some(signature) =
            original.signature(&[], &properties, self.context.options.root_font_size)
        else {
            return Ok(None);
        };
        for variant in &candidate.variants {
            if !variant.raw.starts_with('[') {
                continue;
            }
            let names = self
                .canonical_index()
                .variants
                .matching(&self.context, raw, &variant.raw);
            for name in names {
                let replacement: LeanString = raw
                    .replacen(
                        format_args!("{}:", variant.raw).to_lean_str().as_str(),
                        &format_args!("{name}:").to_lean_str(),
                        1,
                    )
                    .into();
                if replacement.len() >= raw.len() {
                    continue;
                }
                if let Some(preview) = self.context.canonical_preview(&replacement)?
                    && preview.required_properties == original.required_properties
                    && preview
                        .signature(&[], &properties, self.context.options.root_font_size)
                        .as_ref()
                        == Some(&signature)
                {
                    return Ok(Some((replacement).into()));
                }
            }
        }
        Ok(None)
    }
}

struct CanonicalSearch<'a> {
    roots: &'a BTreeSet<LeanStr>,
    value: &'a str,
    property: Option<&'a str>,
    negative: bool,
    modifier: &'a str,
}

fn eligible_label<'a>(
    label: &'a str,
    roots: &BTreeSet<LeanStr>,
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
        let Some(capture) = label.strip_prefix(format_args!("{root}-").to_lean_str().as_str())
        else {
            return false;
        };
        if query.design_system.utilities().get(registered).is_some() {
            return true;
        }
        let family = format_args!("{}{root}-*", if negative { "-" } else { "" }).to_lean_str();
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
                                .get(&format_args!("{namespace}{capture}").to_lean_str())
                                .is_some()
                        })
                    })
            })
            || query
                .design_system
                .utilities()
                .get(&family)
                .is_some_and(|definitions| {
                    definitions.iter().any(|definition| {
                        named_domain_values(&definition.body, query.design_system.theme())
                            .iter()
                            .any(|name| name == capture)
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

fn declared_properties(css: &str, depth: usize) -> BTreeSet<LeanStr> {
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use lean_string::ToLeanStr;

    use stillwind_design_system::{DesignSystemOptions, DesignSystemSnapshot};
    use stillwind_diagnostics::{CanonicalUtilitiesOverride, LintOverrides, LintSeverity};
    use stillwind_source::{CssSourceName, FileId};
    use stillwind_test_support::css_source;

    use super::super::index::{policy, rank};
    use super::super::preview::convert_pixel_dimensions;
    use crate::query::QueryContext;
    use crate::{CompilerOptions, DesignSystemQuery, RootFontSize, prepare_canonical_index};

    fn make_query(css_text: &str, options: CompilerOptions) -> DesignSystemQuery {
        let source = css_source(FileId(1), css_text).unwrap();
        let snapshot = DesignSystemSnapshot::from_source(
            &source,
            CssSourceName::from("theme.css"),
            DesignSystemOptions::default(),
        )
        .unwrap();
        DesignSystemQuery::new(snapshot, options)
    }

    #[test]
    fn runtime_conditions_keep_negation_parity_and_drop_empty_comparisons() {
        let query = make_query("@import 'stillwind';", CompilerOptions::default());
        for (raw, expected) in [
            ("if(!!--c):flex", Some("if(--c):flex")),
            ("if(!!!--c):flex", Some("if(!--c):flex")),
            ("if(--c:):flex", Some("if(--c):flex")),
            ("if(!--c):flex", None),
            (
                "[width:--if(!!--c,20px,10px)]",
                Some("[width:--if(--c,20px,10px)]"),
            ),
            (
                "w-[--if(!!!--c,20px,10px)]",
                Some("w-[--if(!--c,20px,10px)]"),
            ),
            (
                "[width:--if(--mode:,1px,2px)]",
                Some("[width:--if(--mode,1px,2px)]"),
            ),
            ("[width:--if(--c,20px,10px)]", None),
        ] {
            assert_eq!(
                query.canonical_candidate(raw).unwrap().as_deref(),
                expected,
                "{raw}"
            );
        }
    }

    #[test]
    fn dynamic_families_with_both_forms_prefer_the_named_spelling() {
        let query = make_query(
            "@import 'stillwind'; @stillwind-dynamic icon-*; @stillwind-dynamic icon-[*]; @stillwind-dynamic i-[*];",
            CompilerOptions::default(),
        );
        for (raw, expected) in [
            ("icon-[mdi--close]", Some("icon-mdi--close")),
            ("hover:!icon-[mdi--close]", Some("hover:!icon-mdi--close")),
            ("icon-mdi--close", None),
            ("i-[mdi--close]", None),
        ] {
            assert_eq!(
                query.canonical_candidate(raw).unwrap().as_deref(),
                expected,
                "{raw}"
            );
        }
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
                Some(expected.to_lean_str()),
                "{raw}"
            );
            assert!(query.canonical_candidate(expected).unwrap().is_none());
        }
        let query = make_query(
            "@import 'stillwind'; @theme { --spacing: .25rem; }",
            CompilerOptions {
                lint: LintOverrides {
                    prefer_canonical_utilities: Some(CanonicalUtilitiesOverride {
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
                root_font_size: RootFontSize::new(20.5).unwrap(),
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
            convert_pixel_dimensions("calc(16px + 2px) '16px' url(16px)", 16.0),
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
    fn canonical_color_modifiers_match_with_the_default_registry() {
        let query = make_query(
            "@import 'stillwind'; @theme { --color-brand:red; --color-b:blue; }",
            CompilerOptions::default(),
        );
        for (raw, expected) in [
            ("bg-[#f00]/50", "bg-brand/50"),
            ("hover:!bg-[#f00]/50", "hover:!bg-brand/50"),
            ("bg-[#f00]/100", "bg-brand/100"),
            ("bg-[#f00]/0", "bg-b/0"),
        ] {
            assert_eq!(
                query.canonical_candidate(raw).unwrap().as_deref(),
                Some(expected),
                "{raw}"
            );
        }
        assert!(
            query
                .canonical_candidate("bg-[#123456]/50")
                .unwrap()
                .is_none()
        );
        assert!(
            query
                .hover_candidate("bg-brand/50")
                .unwrap()
                .unwrap()
                .css
                .contains("@supports")
        );
    }

    #[test]
    fn canonical_color_compatibility_keeps_authored_conditions() {
        let query = make_query(
            "@theme { --*:initial; --color-brand:red; } \
             @utility tone-* { @media print { color:--compat-color(--value(--color-*, [color]), --alpha(--modifier())); } } \
             @utility paint-* { color:--value(--color-*, [color]); } \
             @utility paint-brand { color:red; @supports (color:color-mix(in lab, red, red)) { color:blue; } }",
            CompilerOptions::default(),
        );
        assert_eq!(
            query.canonical_candidate("tone-[#f00]/50").unwrap(),
            Some("tone-brand/50".into())
        );
        assert!(query.canonical_candidate("paint-[#f00]").unwrap().is_none());
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
                Some(expected.to_lean_str()),
                "{raw}"
            );
        }
    }

    #[test]
    fn noncanonical_queries_defer_spelling_index_preparation() {
        let query = make_query(
            "@theme { --*:initial; --color-a:red; } @utility tone-* { color:--value(--color-*, [color]); }",
            CompilerOptions::default(),
        );
        assert_eq!(query.resolve_candidate("tone-a").unwrap().failures, []);
        assert!(query.canonical_candidate("tone-a").unwrap().is_none());
        assert!(
            query
                .canonical_candidate("tone-[length:1px]")
                .unwrap()
                .is_none()
        );
        assert!(query.canonical_index.get().is_none());
        assert_eq!(
            query.canonical_candidate("tone-[#f00]").unwrap(),
            Some("tone-a".into())
        );
        assert!(query.canonical_index.get().is_some());
    }

    #[test]
    fn canonical_indexes_share_preparation_and_isolate_lint_policies() {
        let source = css_source(FileId(1), "@theme { --*:initial; --spacing:.25rem; } @utility p-* { padding:--value(--spacing(), [length]); }").unwrap();
        let snapshot = DesignSystemSnapshot::from_source(
            &source,
            CssSourceName::from("theme.css"),
            DesignSystemOptions::default(),
        )
        .unwrap();
        prepare_canonical_index(&snapshot, &CompilerOptions::default());
        let prepared =
            QueryContext::new(snapshot.clone(), CompilerOptions::default()).canonical_index();
        let first = DesignSystemQuery::new(snapshot.clone(), CompilerOptions::default());
        assert!(Arc::ptr_eq(&prepared, first.canonical_index()));
        let second = DesignSystemQuery::new(
            snapshot.clone(),
            CompilerOptions {
                lint: LintOverrides {
                    prefer_canonical_utilities: Some(CanonicalUtilitiesOverride {
                        severity: Some(LintSeverity::Deny),
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
        assert!(Arc::ptr_eq(
            first.canonical_index(),
            second.canonical_index()
        ));
        let disabled = DesignSystemQuery::new(
            snapshot,
            CompilerOptions {
                lint: LintOverrides {
                    prefer_canonical_utilities: Some(CanonicalUtilitiesOverride {
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
        let source = css_source(FileId(1),
            "@utility good { color:red; } @utility bad-* { @stillwind-require-properties invalid; color:--value([color]); }").unwrap();
        let snapshot = DesignSystemSnapshot::from_source(
            &source,
            CssSourceName::from("theme.css"),
            DesignSystemOptions::default(),
        )
        .unwrap();
        prepare_canonical_index(&snapshot, &CompilerOptions::default());
        let query = DesignSystemQuery::new(snapshot, CompilerOptions::default());
        assert_eq!(query.resolve_candidate("good").unwrap().failures, []);
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
    fn canonical_color_modifiers_search_all_values_when_alpha_erases_the_color() {
        let query = make_query(
            "@theme { --*:initial; --color-a:red; --color-b:blue; } @theme inline { --opacity-50:0; } \
             @utility fixed-* { color:--compat-color(--value(--color-*, [color]), 0%); opacity:--modifier(number); } \
             @utility token-* { color:--compat-color(--value(--color-*, [color]), --alpha(--modifier(--opacity-*))); } \
             @utility extra-* { color:--compat-color(--value(--color-*, [color]), --alpha(--modifier()), 0%); }",
            CompilerOptions::default(),
        );
        for root in ["fixed", "token", "extra"] {
            let raw = format!("{root}-[#00f]/50");
            assert_eq!(
                query.canonical_candidate(&raw).unwrap().as_deref(),
                Some(format!("{root}-a/50").as_str()),
                "{root}"
            );
        }
    }

    #[test]
    fn canonical_search_matches_finite_css_reference() {
        let query = make_query(
            "@theme { --*:initial; --spacing:.25rem; --spacing-a:1rem; --color-a:red; --color-b:blue; --color-c:rgb(255 0 0); --color-alias:var(--color-a); --text-body:1rem; --leading-tight:1.25rem; } @utility tone-* { color:--value(--color-*, [color]); opacity:--modifier(number); } @utility plain-* { color:--value(--color-*, [color]); } @utility text-* { font-size:--value(--text-*, [length]); line-height:--modifier(--leading-*, [length]); } @utility nested-* { &:hover { color:--value(--color-*, [color]); outline-color:var(--color-alias); } } @utility padded-* { padding:--value(--spacing-*, [length]); }",
            CompilerOptions::default(),
        );
        let index = query.canonical_index();
        for (root, value, modifier) in [
            ("tone", "#f00", "/50"),
            ("tone", "#123456", "/50"),
            ("plain", "rgb(255_0_0)", ""),
            ("plain", "#00f", ""),
            ("nested", "#00f", ""),
            ("padded", "1rem", ""),
            ("text", "1rem", "/tight"),
        ] {
            let raw = format_args!("{root}-[{value}]{modifier}").to_lean_str();
            let original = query.context.canonical_preview(&raw).unwrap().unwrap();
            let options = query.canonical_options();
            let scales = query.context.theme_scales();
            let mut labels: Vec<_> = index
                .labels
                .iter()
                .filter(|label| label.starts_with(format_args!("{root}-").to_lean_str().as_str()))
                .cloned()
                .collect();
            if value == "1rem" {
                labels.push(format_args!("{root}-4").to_lean_str());
            }
            labels.sort_by(|left, right| rank(left, right));
            let expected = labels.into_iter().find_map(|label| {
                let replacement = format_args!("{label}{modifier}").to_lean_str();
                if replacement.len() >= raw.len() {
                    return None;
                }
                let preview = query.context.canonical_preview(&replacement).unwrap()?;
                let normalization = policy(&preview.css, &options, &scales);
                let signature = original.signature(
                    &scales,
                    &normalization.properties,
                    query.context.options.root_font_size,
                );
                (signature.is_some()
                    && preview.required_properties == original.required_properties
                    && preview.signature(
                        &scales,
                        &normalization.properties,
                        query.context.options.root_font_size,
                    ) == signature)
                    .then_some(replacement)
            });
            assert_eq!(query.canonical_candidate(&raw).unwrap(), expected, "{raw}");
        }
    }
}
