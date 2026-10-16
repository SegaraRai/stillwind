//! Source policies for resolved utility declarations and variant targets.
//!
//! Direct token selections survive inline expansion. Length inspection follows
//! known theme declarations with a cycle and depth bound. Canonical replacements
//! use these same checks before presenting a source edit.

use std::collections::BTreeSet;

use lean_string::{LeanStr, LeanString, ToLeanStr, ToLeanString};

use stillwind_css::{
    SelectorTarget, append_element_condition, arbitrary_value_flags, declaration_is_physical,
    inspect_css_value, instantiate_selector, media_has_size_comparison, policy_glob_matches,
    property_covers, property_value_domains, selector_target,
};
use stillwind_design_system::{
    ThemeIndex, VariantApplication, VariantValueSource, resolve_variant, resolve_variant_with_usage,
};
use stillwind_diagnostics::{
    Label, LabelScope, LintConfig, LintDiagnostic, LintFinding, LintKind, LintSettings,
};
use stillwind_source::{FileId, JoinText, Span};
use stillwind_syntax::{Variant, parse_candidate};

use super::super::emission::collect_declarations;
use super::super::variants::{VariantStep, variant_paths};
use super::resolve::{CompiledUtility, resolve_utility_targets};
use crate::style_analysis::{DeclarationValue, SupplyKind};

pub(in crate::compiler) fn check(
    utility: &CompiledUtility,
    supplies: &[SupplyKind],
    theme: &ThemeIndex,
    settings: &LintSettings,
    span: Span,
) -> Vec<LintFinding> {
    let mut findings = Vec::new();
    if settings.enabled_for_detection(LintKind::PreferDirectStyling)
        && let Some(selector) = variant_other_target(utility, theme)
    {
        findings.push(LintFinding::new(
            LintDiagnostic::IndirectStyling {
                selector: (selector).into(),
            },
            span,
        ));
    }
    let mut reported = BTreeSet::new();
    for supply in supplies {
        let SupplyKind::Declaration {
            property,
            value: DeclarationValue::Css(value),
            important,
            ..
        } = supply
        else {
            continue;
        };
        check_declaration_restrictions(
            property,
            value,
            *important,
            settings,
            span,
            &mut reported,
            &mut findings,
        );
        if settings.enabled_for_detection(LintKind::PreferLogicalProperties)
            && declaration_is_physical(property, value)
            && reported.insert((
                LintKind::PreferLogicalProperties,
                (property.clone()).into(),
                LeanString::new(),
            ))
        {
            findings.push(LintFinding::new(
                LintDiagnostic::PhysicalDeclaration {
                    property: property.clone(),
                    value: value.clone(),
                },
                span,
            ));
        }
        check_length_units(
            property,
            value,
            theme,
            settings,
            span,
            &mut reported,
            &mut findings,
        );
    }
    check_source_policies(utility, theme, settings, span, &mut findings);
    findings
}

fn check_length_units(
    property: &str,
    value: &str,
    theme: &ThemeIndex,
    settings: &LintSettings,
    span: Span,
    reported: &mut BTreeSet<(LintKind, LeanString, LeanString)>,
    findings: &mut Vec<LintFinding>,
) {
    if settings.enabled_for_detection(LintKind::NoRestrictedLengthUnits)
        && let Some(LintConfig::NoRestrictedLengthUnits(config)) =
            settings.get(LintKind::NoRestrictedLengthUnits)
    {
        let domains = property_value_domains(property, value);
        for (domain, value) in &domains {
            if domains
                .iter()
                .any(|(name, _)| name != domain && property_covers(domain, name))
            {
                continue;
            }
            let Some(rule) = config.rules.iter().rev().find(|rule| {
                rule.properties.iter().any(|pattern| {
                    domains.iter().any(|(name, _)| {
                        (name == domain || property_covers(name, domain))
                            && policy_glob_matches(
                                &pattern.to_lean_string().to_ascii_lowercase(),
                                name,
                            )
                    })
                })
            }) else {
                continue;
            };
            let mut units = BTreeSet::new();
            let mut token_spans = Vec::new();
            collect_token_units(
                value,
                theme,
                &mut BTreeSet::new(),
                0,
                &mut units,
                &mut token_spans,
            );
            for unit in units.into_iter().filter(|unit| {
                rule.disallow
                    .iter()
                    .any(|forbidden| forbidden.eq_ignore_ascii_case(unit))
            }) {
                if reported.insert((
                    LintKind::NoRestrictedLengthUnits,
                    domain.clone(),
                    (unit.clone()).into(),
                )) {
                    let mut finding = LintFinding::new(
                        LintDiagnostic::RestrictedLengthUnit {
                            property: (domain.clone()).into(),
                            unit,
                        },
                        span,
                    );
                    for token_span in &token_spans {
                        finding.info.labels.push(Label {
                            scope: LabelScope::Range,
                            span: *token_span,
                            message: Some("referenced theme value".into()),
                        });
                    }
                    findings.push(finding);
                }
            }
        }
    }
}

fn check_declaration_restrictions(
    property: &str,
    value: &str,
    important: bool,
    settings: &LintSettings,
    span: Span,
    reported: &mut BTreeSet<(LintKind, LeanString, LeanString)>,
    findings: &mut Vec<LintFinding>,
) {
    if important
        && settings.enabled_for_detection(LintKind::NoImportant)
        && reported.insert((
            LintKind::NoImportant,
            property.to_lean_string(),
            LeanString::new(),
        ))
    {
        findings.push(LintFinding::new(
            LintDiagnostic::ImportantDeclaration {
                property: property.to_lean_str(),
            },
            span,
        ));
    }
    if settings.enabled_for_detection(LintKind::NoRestrictedDeclarations)
        && let Some(LintConfig::NoRestrictedDeclarations(config)) =
            settings.get(LintKind::NoRestrictedDeclarations)
    {
        for (domain, value) in property_value_domains(property, value) {
            for rule in &config.rules {
                if rule.properties.iter().any(|pattern| {
                    policy_glob_matches(&pattern.to_lean_string().to_ascii_lowercase(), &domain)
                }) && rule.values.iter().any(|pattern| {
                    policy_glob_matches(
                        &pattern.to_lean_string().to_ascii_lowercase(),
                        &value.to_lean_string().to_ascii_lowercase(),
                    )
                }) {
                    let explanation = explanation(rule.message.as_deref());
                    if reported.insert((
                        LintKind::NoRestrictedDeclarations,
                        domain.clone(),
                        format_args!("{value}{explanation}").to_lean_string(),
                    )) {
                        findings.push(LintFinding::new(
                            LintDiagnostic::RestrictedDeclaration {
                                property: (domain.clone()).into(),
                                value: (value.clone()).into(),
                                explanation: (explanation).into(),
                            },
                            span,
                        ));
                    }
                }
            }
        }
    }
}

fn check_source_policies(
    utility: &CompiledUtility,
    theme: &ThemeIndex,
    settings: &LintSettings,
    span: Span,
    findings: &mut Vec<LintFinding>,
) {
    let mut reported = BTreeSet::new();
    for usage in &utility.source.arbitrary_uses {
        let (color, spacing) = arbitrary_value_flags(&usage.property, &usage.value, usage.color);
        if color
            && settings.enabled_for_detection(LintKind::NoArbitraryColors)
            && reported.insert((
                LintKind::NoArbitraryColors,
                usage.property.clone(),
                usage.value.clone(),
            ))
        {
            findings.push(LintFinding::new(
                LintDiagnostic::ArbitraryColor {
                    property: usage.property.clone(),
                    value: usage.value.clone(),
                },
                span,
            ));
        }
        if spacing
            && settings.enabled_for_detection(LintKind::NoArbitrarySpacing)
            && let Some(LintConfig::NoArbitrarySpacing(config)) =
                settings.get(LintKind::NoArbitrarySpacing)
            && let Some((domain, _)) = property_value_domains(&usage.property, &usage.value)
                .into_iter()
                .find(|(domain, _)| spacing_property(domain, &config.properties))
            && reported.insert((
                LintKind::NoArbitrarySpacing,
                (domain.clone()).into(),
                usage.value.clone(),
            ))
        {
            findings.push(LintFinding::new(
                LintDiagnostic::ArbitrarySpacing {
                    property: (domain).into(),
                    value: usage.value.clone(),
                },
                span,
            ));
        }
    }
    check_tokens(
        utility
            .source
            .token_uses
            .iter()
            .flat_map(|usage| usage.tokens.iter().map(LeanStr::as_str)),
        theme,
        settings,
        span,
        findings,
    );
    for (index, variant) in utility
        .variants
        .iter()
        .skip(utility.source.wrapper_variant_count)
        .enumerate()
    {
        findings.extend(check_variant(
            variant,
            theme,
            settings,
            utility
                .source
                .variant_spans
                .get(index)
                .copied()
                .unwrap_or(span),
        ));
    }
}

pub(in crate::compiler) fn spacing_property(property: &str, additional: &[LeanStr]) -> bool {
    [
        "margin",
        "margin-*",
        "padding",
        "padding-*",
        "gap",
        "row-gap",
        "column-gap",
    ]
    .into_iter()
    .any(|pattern| policy_glob_matches(pattern, property))
        || additional.iter().any(|pattern| {
            let pattern = pattern.to_lean_string().to_ascii_lowercase();
            policy_glob_matches(&pattern, property) || property_covers(&pattern, property)
        })
}

pub(crate) fn explanation(message: Option<&str>) -> LeanString {
    message.map_or_else(LeanString::new, |message| {
        format_args!(": {message}").to_lean_string()
    })
}

pub(crate) fn check_candidate_name(
    raw: &str,
    settings: &LintSettings,
    span: Span,
) -> Vec<LintFinding> {
    if !settings.enabled_for_detection(LintKind::NoRestrictedUtilities) {
        return Vec::new();
    }
    let Some(LintConfig::NoRestrictedUtilities(config)) =
        settings.get(LintKind::NoRestrictedUtilities)
    else {
        return Vec::new();
    };
    let utility = parse_candidate(raw).map_or(raw, |candidate| {
        let offset = candidate
            .variants
            .iter()
            .map(|variant| variant.raw.len() + 1)
            .sum::<usize>();
        raw[offset..].trim_matches('!')
    });
    config
        .utilities
        .iter()
        .filter(|rule| policy_glob_matches(&rule.pattern, utility))
        .map(|rule| {
            LintFinding::new(
                LintDiagnostic::RestrictedUtility {
                    utility: utility.into(),
                    explanation: (explanation(rule.message.as_deref())).into(),
                },
                span,
            )
        })
        .collect()
}

fn check_tokens<'a>(
    tokens: impl Iterator<Item = &'a str>,
    theme: &ThemeIndex,
    settings: &LintSettings,
    span: Span,
    findings: &mut Vec<LintFinding>,
) {
    let mut reported = BTreeSet::new();
    if settings.enabled_for_detection(LintKind::NoRestrictedThemeTokens)
        && let Some(LintConfig::NoRestrictedThemeTokens(config)) =
            settings.get(LintKind::NoRestrictedThemeTokens)
    {
        for token in tokens {
            if theme.custom_property(token).is_some()
                && config
                    .tokens
                    .iter()
                    .any(|pattern| policy_glob_matches(pattern, token))
                && reported.insert(token.to_lean_string())
            {
                let mut finding = LintFinding::new(
                    LintDiagnostic::RestrictedThemeToken {
                        token: token.to_lean_str(),
                    },
                    span,
                );
                if let Some(definition) = theme.custom_property(token) {
                    finding.info.labels.push(Label {
                        scope: LabelScope::Range,
                        span: definition.name.span,
                        message: Some("restricted theme token definition".into()),
                    });
                }
                findings.push(finding);
            }
        }
    }
}

pub(in crate::compiler) fn check_variant(
    variant: &Variant,
    theme: &ThemeIndex,
    settings: &LintSettings,
    span: Span,
) -> Vec<LintFinding> {
    if ![
        LintKind::NoRestrictedThemeTokens,
        LintKind::NoArbitraryBreakpoints,
    ]
    .into_iter()
    .any(|kind| settings.enabled_for_detection(kind))
    {
        return Vec::new();
    }
    let Some(resolved) = resolve_variant_with_usage(variant, theme) else {
        return Vec::new();
    };
    let mut findings = Vec::new();
    check_tokens(
        resolved.tokens.iter().map(LeanStr::as_str),
        theme,
        settings,
        span,
        &mut findings,
    );
    if settings.enabled_for_detection(LintKind::NoArbitraryBreakpoints) {
        let conditions: BTreeSet<_> = resolved
            .media_values
            .iter()
            .filter(|query| {
                query.sources.contains(&VariantValueSource::Explicit)
                    && media_has_size_comparison(&query.condition)
            })
            .map(|query| query.condition.as_str())
            .collect();
        if !conditions.is_empty() {
            findings.push(LintFinding::new(
                LintDiagnostic::ArbitraryBreakpoint {
                    variant: variant.raw.to_lean_str(),
                    condition: (conditions.into_iter().collect::<Vec<_>>().join_text(", ")).into(),
                },
                span,
            ));
        }
    }
    findings
}

fn collect_token_units(
    value: &str,
    theme: &ThemeIndex,
    seen: &mut BTreeSet<LeanStr>,
    depth: usize,
    units: &mut BTreeSet<LeanStr>,
    spans: &mut Vec<Span>,
) {
    if depth == 32 {
        return;
    }
    let (direct, references) = inspect_css_value(value);
    units.extend(direct);
    for reference in references {
        if seen.insert(reference.clone())
            && let Some(token) = theme.custom_property(&reference)
        {
            spans.push(token.value.span);
            collect_token_units(&token.value.value, theme, seen, depth + 1, units, spans);
        }
    }
}

fn variant_other_target(utility: &CompiledUtility, theme: &ThemeIndex) -> Option<LeanString> {
    let applications = if utility.variant_applications.is_empty() {
        vec![
            utility
                .variants
                .iter()
                .filter_map(|variant| resolve_variant(variant, theme).ok())
                .collect(),
        ]
    } else {
        utility.variant_applications.clone()
    };
    applications
        .iter()
        .flat_map(|applications: &Vec<VariantApplication>| variant_paths(applications))
        .find_map(|path| {
            let mut selector = "&".to_lean_string();
            for step in path {
                match step {
                    VariantStep::SelectorSuffix(suffix) => selector.push_str(suffix),
                    VariantStep::SelectorCondition(condition) => {
                        selector = append_element_condition(&selector, condition);
                    }
                    VariantStep::SelectorTemplate(template)
                    | VariantStep::SelectorRule {
                        selector: template, ..
                    }
                    | VariantStep::SelectorMedia {
                        selector: template, ..
                    }
                    | VariantStep::SelectorMediaRule {
                        selector: template, ..
                    } => selector = instantiate_selector(template, &selector),
                    _ => {}
                }
            }
            (selector_target(&selector) == SelectorTarget::Other).then_some(selector)
        })
}

pub(crate) fn candidate_allowed(raw: &str, theme: &ThemeIndex, settings: &LintSettings) -> bool {
    if !check_candidate_name(raw, settings, Span::new(FileId(0), 0, 0)).is_empty() {
        return false;
    }
    if ![
        LintKind::NoArbitraryColors,
        LintKind::NoArbitrarySpacing,
        LintKind::NoImportant,
        LintKind::NoRestrictedDeclarations,
        LintKind::PreferDirectStyling,
        LintKind::PreferLogicalProperties,
        LintKind::NoRestrictedLengthUnits,
        LintKind::NoRestrictedThemeTokens,
        LintKind::NoArbitraryBreakpoints,
    ]
    .into_iter()
    .any(|kind| settings.enabled_for_detection(kind))
    {
        return true;
    }
    let Ok(Ok(utilities)) = resolve_utility_targets(raw, theme) else {
        return false;
    };
    utilities.iter().all(|utility| {
        let mut supplies = Vec::new();
        collect_declarations(&utility.declaration, "&", &[], false, None, &mut supplies);
        for nested in &utility.nested {
            collect_declarations(&nested.declaration, "&", &[], false, None, &mut supplies);
        }
        check(
            utility,
            &supplies,
            theme,
            settings,
            Span::new(FileId(0), 0, 0),
        )
        .is_empty()
    })
}
