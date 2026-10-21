//! Records values used by the selected variant definition before CSS expansion.
//!
//! Resolution uses the same definition order as ordinary variant evaluation. Failed
//! definitions contribute no token or media records. Nested variant captures keep
//! the records of the selected child.

use std::collections::BTreeSet;

use lean_string::LeanStr;

use stillwind_css::{inspect_css_value, media_size_comparisons};
use stillwind_syntax::{Variant, VariantKind};

use super::super::context::{ArbitraryValueCheck, EvaluationContext};
use super::super::values::expand_placeholder;
use super::application::VariantApplication;
use super::body::resolve_variant_body;
use super::resolve::{
    parse_nested_variant, relative_selector, resolve_named_variant_with,
    resolve_registered_variant_candidate, resolve_variant, split_typed_capture,
};
use super::substitution::{
    resolve_variant_css_placeholders, resolve_variant_value_with_source,
    substitute_variant_capture, variant_value_capture,
};
use crate::{StillwindVariantValueType, ThemeIndex};

/// One resolved variant and the direct values its selected definition uses.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedVariant {
    /// The application produced by ordinary variant resolution.
    pub application: VariantApplication,
    /// Direct selections and media value origins retained for source policy
    /// diagnostics.
    pub diagnostic_data: VariantDiagnosticData,
}

/// Direct values used to check the source policies of a resolved variant.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct VariantDiagnosticData {
    /// Direct custom-property names, before inline expansion and alias traversal. Names
    /// are sorted and deduplicated across the selected definition and its children.
    pub tokens: Vec<LeanStr>,
    /// Viewport size comparisons and the origins of their supplied values.
    pub media_values: Vec<VariantMediaValue>,
}

/// One viewport size comparison produced by a variant definition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VariantMediaValue {
    /// Expanded comparison, including parentheses.
    pub condition: LeanStr,
    /// Origins of substitutions in this condition. A fixed prelude uses `Definition`.
    pub sources: Vec<VariantValueSource>,
}

/// Where a value supplied to a variant's media condition came from.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VariantValueSource {
    /// A named token selected by the successful value domain.
    ThemeToken(
        /// Complete custom-property name, including its `--` prefix.
        LeanStr,
    ),
    /// A value supplied directly by the source capture or an arbitrary media variant.
    Explicit,
    /// A fixed condition authored in a named variant definition.
    Definition,
}

/// Resolves `variant` against `theme` and records the values its selected definition
/// uses.
///
/// The returned application matches ordinary variant resolution. Diagnostic records
/// include selected child variants and keep direct token names before inline
/// expansion and alias traversal. Invalid or unavailable variants return `None`.
#[must_use]
pub fn resolve_variant_with_usage(
    variant: &Variant,
    theme: &ThemeIndex,
) -> Option<ResolvedVariant> {
    if matches!(variant.kind, VariantKind::Arbitrary(_)) {
        let application = resolve_variant(variant, theme).ok()?;
        let tokens = application_tokens(&application);
        let media_values = media_conditions(&application)
            .into_iter()
            .flat_map(media_size_comparisons)
            .map(|condition| VariantMediaValue {
                condition,
                sources: vec![VariantValueSource::Explicit],
            })
            .collect();
        return Some(ResolvedVariant {
            application,
            diagnostic_data: VariantDiagnosticData {
                tokens: tokens.into_iter().collect(),
                media_values,
            },
        });
    }
    let VariantKind::Named(value) = &variant.kind else {
        return None;
    };
    resolve_named_variant_with(value, theme, |custom, capture, concrete| {
        let application =
            resolve_registered_variant_candidate(custom, capture, concrete, 0, theme).ok()?;
        let mut tokens = BTreeSet::new();
        let mut media_values = Vec::new();
        if custom.value_type == Some(StillwindVariantValueType::Variant) {
            let inner = parse_nested_variant(split_typed_capture(custom, capture?).0);
            // A relative selector resolves only inside its definition and uses no tokens.
            if relative_selector(&inner).is_none() {
                let resolved = resolve_variant_with_usage(&inner, theme)?;
                tokens.extend(resolved.diagnostic_data.tokens);
                media_values.extend(resolved.diagnostic_data.media_values);
            }
        }
        if custom.value_type.is_none() {
            let authored = substitute_variant_capture(&custom.body, capture)?;
            let tracked = EvaluationContext::tracked(theme, ArbitraryValueCheck::Grammar);
            let expanded =
                resolve_variant_css_placeholders(&authored, &custom.name, concrete, &tracked)?;
            if resolve_variant_body(&expanded).is_some() {
                tokens.extend(tracked.take_selections().tokens);
                tokens.extend(inspect_css_value(&authored).1);
                if let Some(template) = resolve_variant_body(&custom.body) {
                    for condition in media_conditions(&template)
                        .into_iter()
                        .flat_map(media_size_comparisons)
                    {
                        let mut sources = Vec::new();
                        let mut expanded = substitute_variant_capture(&condition, capture)?;
                        if expanded != condition {
                            sources.push(VariantValueSource::Explicit);
                        }
                        expand_placeholder(&mut expanded, "--value", |domains| {
                            let (value, source) = resolve_variant_value_with_source(
                                domains,
                                variant_value_capture(&custom.name, concrete),
                                &EvaluationContext::untracked(theme),
                            )?;
                            sources.push(source);
                            Some(value)
                        })?;
                        if sources.is_empty() {
                            sources.push(VariantValueSource::Definition);
                        }
                        media_values.push(VariantMediaValue {
                            condition: (expanded).into(),
                            sources,
                        });
                    }
                }
            }
        } else {
            tokens.extend(inspect_css_value(&custom.body).1);
            if custom.value_type == Some(StillwindVariantValueType::SupportsCondition) {
                tokens.extend(application_tokens(&application));
            }
        }
        Some(ResolvedVariant {
            application,
            diagnostic_data: VariantDiagnosticData {
                tokens: tokens.into_iter().collect(),
                media_values,
            },
        })
    })
}

fn media_conditions(application: &VariantApplication) -> Vec<&str> {
    match application {
        VariantApplication::Compound(children) | VariantApplication::Sequence(children) => {
            children.iter().flat_map(media_conditions).collect()
        }
        VariantApplication::Media(condition)
        | VariantApplication::SelectorMedia { condition, .. }
        | VariantApplication::SelectorMediaRule { condition, .. } => vec![condition],
        _ => Vec::new(),
    }
}

fn application_tokens(application: &VariantApplication) -> BTreeSet<LeanStr> {
    let mut tokens = BTreeSet::new();
    match application {
        VariantApplication::Compound(children) | VariantApplication::Sequence(children) => {
            for child in children {
                tokens.extend(application_tokens(child));
            }
        }
        VariantApplication::SelectorRule { declarations, .. } => {
            for declaration in declarations {
                tokens.extend(inspect_css_value(&declaration.value).1);
            }
        }
        VariantApplication::SelectorMedia { condition, .. }
        | VariantApplication::Media(condition)
        | VariantApplication::Container(condition)
        | VariantApplication::Supports(condition) => {
            tokens.extend(inspect_css_value(condition).1);
        }
        VariantApplication::SelectorMediaRule {
            condition,
            declarations,
            ..
        } => {
            tokens.extend(inspect_css_value(condition).1);
            for declaration in declarations {
                tokens.extend(inspect_css_value(&declaration.value).1);
            }
        }
        _ => {}
    }
    tokens
}

#[cfg(test)]
mod tests {
    use lean_string::ToLeanString;

    use stillwind_source::{FileId, SourceFile};

    use super::*;
    use crate::test_support::parse_fixture_theme;

    #[test]
    fn spacing_domains_distinguish_named_tokens_from_explicit_values_and_reject_variables() {
        let source = SourceFile::new(
            FileId(1),
            None,
            None,
            "@theme inline {
            --spacing:0.25rem; --spacing-ref-small:1rem; --spacing-gutter:2rem;
        } @variant scaled-* { @media (width>=--value(--spacing(), [length])) {@slot;} }",
        )
        .unwrap();
        let theme = parse_fixture_theme(&source).unwrap();
        for (capture, tokens, origin) in [
            (
                "gutter",
                vec!["--spacing-gutter"],
                VariantValueSource::ThemeToken("--spacing-gutter".into()),
            ),
            ("4", vec!["--spacing"], VariantValueSource::Explicit),
            ("[3rem]", vec![], VariantValueSource::Explicit),
        ] {
            let resolved = resolve_variant_with_usage(
                &parse_nested_variant(&format_args!("scaled-{capture}").to_lean_string()),
                &theme,
            )
            .unwrap();
            assert_eq!(resolved.diagnostic_data.tokens, tokens, "{capture}");
            assert_eq!(
                resolved.diagnostic_data.media_values[0].sources,
                [origin],
                "{capture}"
            );
        }
        // A media query cannot evaluate `var()`.
        for capture in ["[var(--spacing-ref-small)]", "(--spacing-ref-small)"] {
            let variant = parse_nested_variant(&format_args!("scaled-{capture}").to_lean_string());
            assert!(
                resolve_variant_with_usage(&variant, &theme).is_none(),
                "{capture}"
            );
        }
        let source = SourceFile::new(
            FileId(2),
            None,
            None,
            source.text().replace("@theme inline", "@theme"),
        )
        .unwrap();
        let theme = parse_fixture_theme(&source).unwrap();
        assert!(resolve_variant_with_usage(&parse_nested_variant("scaled-4"), &theme).is_none());
    }

    #[test]
    fn records_only_selected_definitions_and_preserves_direct_alias_names() {
        let source = SourceFile::new(FileId(1), None, None, "@theme inline {
            --breakpoint-md:48rem; --spacing-md:1rem; --spacing-ref-small:1rem;
            --spacing-gutter:var(--spacing-ref-small);
        }
        @variant pick-* { @media (width>=--value(--breakpoint-*)) { @slot; } }
        @variant pick-* { @media (width>=--value(--spacing-*)){ @supports (display:--value([color])) { @slot; } } }
        @variant gap-* { & {padding:--value(--spacing-*); @slot;} }").unwrap();
        let theme = crate::test_support::parse_fixture_theme(&source).unwrap();
        let pick = resolve_variant_with_usage(&parse_nested_variant("pick-md"), &theme).unwrap();
        assert_eq!(pick.diagnostic_data.tokens, ["--breakpoint-md"]);
        assert_eq!(
            pick.diagnostic_data.media_values,
            [VariantMediaValue {
                condition: "(width>=48rem)".into(),
                sources: vec![VariantValueSource::ThemeToken("--breakpoint-md".into())]
            }]
        );
        let gap = resolve_variant_with_usage(&parse_nested_variant("gap-gutter"), &theme).unwrap();
        assert_eq!(gap.diagnostic_data.tokens, ["--spacing-gutter"]);
        assert_eq!(
            gap.diagnostic_data.media_values,
            [] as [VariantMediaValue; 0]
        );
        assert_eq!(
            resolve_variant(&parse_nested_variant("pick-md"), &theme),
            Ok(pick.application)
        );
    }
}
