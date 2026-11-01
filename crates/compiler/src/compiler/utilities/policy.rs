//! Source policies for resolved utility declarations and variant targets.
//!
//! Direct token selections survive inline expansion. Length inspection follows
//! known theme declarations with a cycle and depth bound. Canonical replacements
//! use these same checks before presenting a source edit.

use std::collections::BTreeSet;

use lean_string::{LeanStr, LeanString, ToLeanStr, ToLeanString};

use stillwind_css::{
    SelectorTarget, arbitrary_value_flags, declaration_is_physical, inspect_css_value,
    media_has_size_comparison, policy_glob_matches, property_covers, property_value_domains,
    selector_target,
};
use stillwind_design_system::{
    ThemeIndex, UtilityDeclaration, VariantApplication, VariantContext, VariantValueSource,
    resolve_variant, resolve_variant_with_usage,
};
use stillwind_diagnostics::{
    Label, LabelScope, LintConfig, LintDiagnostic, LintFinding, LintKind, LintSettings,
};
use stillwind_source::{FileId, JoinText, Span};
use stillwind_syntax::{Variant, parse_candidate};

use super::super::emission::collect_declarations;
use super::super::variants::{VariantStep, path_selector, variant_paths};
use super::declaration_order::prepend_missing_declarations;
use super::resolve::{CompiledUtility, resolve_utility_targets};
use crate::style_analysis::{DeclarationValue, SupplyKind};

/// Returns enabled policy findings for one use of `utility` at `span`.
///
/// `supplies` must describe the resolved CSS of `utility`, including candidate
/// importance. `theme` must be the design system that resolved it. `settings` selects
/// the rules and whether each rule includes fixed implementation declarations.
/// Variant findings use the recorded variant ranges when available. The inputs are
/// unchanged.
pub(in crate::compiler) fn check(
    utility: &CompiledUtility,
    supplies: &[SupplyKind],
    theme: &ThemeIndex,
    settings: &LintSettings,
    span: Span,
) -> Vec<LintFinding> {
    let mut ordinary_supplies = Vec::new();
    let filter_importance = matches!(settings.get(LintKind::NoImportant), Some(LintConfig::NoImportant(config)) if !config.check_implementation);
    let filter_restrictions = matches!(settings.get(LintKind::NoRestrictedDeclarations), Some(LintConfig::NoRestrictedDeclarations(config)) if !config.check_implementation);
    let declarations = if filter_importance
        || filter_restrictions
        || settings.enabled_for_detection(LintKind::PreferLogicalProperties)
        || settings.enabled_for_detection(LintKind::NoRestrictedLengthUnits)
    {
        declaration_policies(utility, theme)
    } else {
        Vec::new()
    };
    if filter_importance || filter_restrictions {
        for declaration in declarations
            .iter()
            .filter(|declaration| !declaration.implementation)
        {
            collect_declarations(
                &format_args!("{}:{};", declaration.property, declaration.value).to_lean_string(),
                "&",
                &[],
                false,
                None,
                &mut ordinary_supplies,
            );
        }
    }
    let mut findings = check_importance(
        utility,
        supplies,
        if filter_importance {
            &ordinary_supplies
        } else {
            supplies
        },
        settings,
        span,
    );
    if settings.enabled_for_detection(LintKind::PreferDirectStyling)
        && let Some(selector) = variant_other_target(utility, theme)
    {
        findings.push(LintFinding::new(
            LintDiagnostic::IndirectStyling {
                selector: selector.into(),
            },
            span,
        ));
    }
    let mut reported = BTreeSet::new();
    for supply in if filter_restrictions {
        &ordinary_supplies
    } else {
        supplies
    } {
        if let SupplyKind::Declaration {
            property,
            value: DeclarationValue::Css(value),
            ..
        } = supply
        {
            check_declaration_restrictions(
                property,
                value,
                settings,
                span,
                &mut reported,
                &mut findings,
            );
        }
    }
    check_value_policies(
        &declarations,
        theme,
        settings,
        span,
        &mut reported,
        &mut findings,
    );
    check_source_policies(utility, theme, settings, span, &mut findings);
    findings
}

/// Appends logical-property and length-unit findings at `span` to `findings`.
///
/// `declarations` contains values and their implementation classifications. `theme`
/// resolves referenced length tokens, and `settings` selects the checks and their
/// implementation policy. `reported` records findings already emitted for this use
/// and receives the keys of new findings.
fn check_value_policies(
    declarations: &[UtilityDeclaration],
    theme: &ThemeIndex,
    settings: &LintSettings,
    span: Span,
    reported: &mut BTreeSet<(LintKind, LeanString, LeanString)>,
    findings: &mut Vec<LintFinding>,
) {
    if settings.enabled_for_detection(LintKind::PreferLogicalProperties)
        || settings.enabled_for_detection(LintKind::NoRestrictedLengthUnits)
    {
        for declaration in declarations {
            let property = &declaration.property;
            let value = &declaration.value;
            if settings.enabled_for_detection(LintKind::PreferLogicalProperties)
                && (!declaration.implementation
                    || matches!(settings.get(LintKind::PreferLogicalProperties), Some(LintConfig::PreferLogicalProperties(config)) if config.check_implementation))
                && declaration_is_physical(property, value)
                && reported.insert((
                    LintKind::PreferLogicalProperties,
                    property.to_lean_string(),
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
            if !declaration.implementation
                || matches!(settings.get(LintKind::NoRestrictedLengthUnits), Some(LintConfig::NoRestrictedLengthUnits(config)) if config.check_implementation)
            {
                check_length_units(property, value, theme, settings, span, reported, findings);
            }
        }
    }
}

/// Collects declaration values and their classifications for checks of `utility`.
///
/// `theme` must be the design system that resolved `utility`. Recorded values precede
/// runtime guards. A group without records is parsed from its current CSS and receives
/// ordinary classifications. Variant declarations are also ordinary and are included
/// only when rendering would add them. The result can contain repeated declarations.
fn declaration_policies(utility: &CompiledUtility, theme: &ThemeIndex) -> Vec<UtilityDeclaration> {
    let resolved;
    let applications: Vec<&[VariantApplication]> = if utility.variant_applications.is_empty() {
        resolved = utility
            .variants
            .iter()
            .filter_map(|variant| resolve_variant(variant, theme).ok())
            .collect::<Vec<_>>();
        vec![&resolved]
    } else {
        utility
            .variant_applications
            .iter()
            .map(Vec::as_slice)
            .collect()
    };
    let mut declarations = Vec::new();
    for (css, classifications) in
        std::iter::once((&utility.declaration, &utility.diagnostic_data.declarations)).chain(
            utility
                .nested
                .iter()
                .map(|rule| (&rule.declaration, &rule.policy_declarations)),
        )
    {
        if classifications.is_empty() {
            declarations.extend(ordinary_declarations(css));
        } else {
            declarations.extend(classifications.iter().cloned());
        }
        for applications in &applications {
            for path in variant_paths(applications) {
                let mut expanded = css.to_lean_string();
                for step in path {
                    if let VariantStep::SelectorRule { declarations, .. }
                    | VariantStep::SelectorMediaRule { declarations, .. } = step
                    {
                        prepend_missing_declarations(&mut expanded, declarations);
                    }
                }
                declarations.extend(ordinary_declarations(
                    &expanded[..expanded.len() - css.len()],
                ));
            }
        }
    }
    declarations
}

/// Parses declarations from `css`, including nested rules, for policy checks.
///
/// Every recovered declaration has `implementation: false` and `base: true` because
/// the result does not retain selector groups. Non-declaration supplies are omitted.
fn ordinary_declarations(css: &str) -> Vec<UtilityDeclaration> {
    let mut supplies = Vec::new();
    collect_declarations(css, "&", &[], false, None, &mut supplies);
    supplies
        .into_iter()
        .filter_map(|supply| match supply {
            SupplyKind::Declaration {
                property,
                value: DeclarationValue::Css(value),
                ..
            } => Some(UtilityDeclaration {
                property,
                value,
                implementation: false,
                base: true,
            }),
            _ => None,
        })
        .collect()
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
    for property in value_properties(property, theme) {
        check_property_length_units(property, value, theme, settings, span, reported, findings);
    }
}

/// Returns the properties used by value policies for a declaration of `property`.
///
/// The last registration of a custom property supplies its `value-for` list. Project
/// registrations override Stillwind registrations, including when they omit the list.
/// An ordinary property or a registration without a list retains its own name.
/// The result borrows the inputs and does not change declaration effects or emission.
fn value_properties<'a>(property: &'a str, theme: &'a ThemeIndex) -> impl Iterator<Item = &'a str> {
    let targets = if property.starts_with("--") {
        theme
            .properties()
            .iter()
            .rev()
            .chain(theme.registry().properties().iter().rev())
            .find(|definition| definition.name == property)
            .map_or(&[][..], |definition| definition.value_for.as_slice())
    } else {
        &[][..]
    };
    targets
        .iter()
        .map(LeanStr::as_str)
        .chain(targets.is_empty().then_some(property))
}

/// Appends findings for the length units of `value` under one effective `property`.
/// `theme` resolves referenced tokens, and `reported` receives keys of new findings.
fn check_property_length_units(
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

/// Returns enabled importance findings at the utility use identified by `span`.
///
/// `utility` supplies the authored candidate. `supplies` must include its complete
/// resolved CSS effects with candidate importance applied. `definition_supplies`
/// contains the declarations retained by the rule's implementation policy. A source
/// `!` modifier reports once when `supplies` contains an important CSS declaration.
/// Otherwise, each important property in `definition_supplies` reports once.
fn check_importance(
    utility: &CompiledUtility,
    supplies: &[SupplyKind],
    definition_supplies: &[SupplyKind],
    settings: &LintSettings,
    span: Span,
) -> Vec<LintFinding> {
    if !settings.enabled_for_detection(LintKind::NoImportant) {
        return Vec::new();
    }
    let modifier = parse_candidate(&utility.candidate).is_some_and(|candidate| candidate.important);
    if modifier
        && supplies.iter().any(|supply| {
            matches!(
                supply,
                SupplyKind::Declaration {
                    value: DeclarationValue::Css(_),
                    important: true,
                    ..
                }
            )
        })
    {
        return vec![LintFinding::new(LintDiagnostic::ImportantUtility, span)];
    }
    let mut findings = Vec::new();
    let mut reported = BTreeSet::new();
    for supply in definition_supplies {
        let SupplyKind::Declaration {
            property,
            value: DeclarationValue::Css(_),
            important: true,
            ..
        } = supply
        else {
            continue;
        };
        if reported.insert(property) {
            findings.push(LintFinding::new(
                LintDiagnostic::ImportantDeclaration {
                    property: property.clone(),
                },
                span,
            ));
        }
    }
    findings
}

fn check_declaration_restrictions(
    property: &str,
    value: &str,
    settings: &LintSettings,
    span: Span,
    reported: &mut BTreeSet<(LintKind, LeanString, LeanString)>,
    findings: &mut Vec<LintFinding>,
) {
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

/// Appends enabled findings for captures, token selections, and variants of `utility`.
///
/// `theme` must match the utility's resolution, and `settings` supplies the policies.
/// Fixed implementation token uses follow their rule's option. Captured values and
/// authored variant arguments retain their checks. `span` identifies the use when no
/// variant range is recorded. New findings are appended without changing existing
/// entries in `findings`.
fn check_source_policies(
    utility: &CompiledUtility,
    theme: &ThemeIndex,
    settings: &LintSettings,
    span: Span,
    findings: &mut Vec<LintFinding>,
) {
    let mut reported = BTreeSet::new();
    for usage in &utility.diagnostic_data.arbitrary_uses {
        for property in value_properties(&usage.property, theme) {
            let (color, spacing) = arbitrary_value_flags(property, &usage.value, usage.color);
            if color
                && settings.enabled_for_detection(LintKind::NoArbitraryColors)
                && reported.insert((
                    LintKind::NoArbitraryColors,
                    property.to_lean_str(),
                    usage.value.clone(),
                ))
            {
                findings.push(LintFinding::new(
                    LintDiagnostic::ArbitraryColor {
                        property: property.to_lean_str(),
                        value: usage.value.clone(),
                    },
                    span,
                ));
            }
            if spacing
                && settings.enabled_for_detection(LintKind::NoArbitrarySpacing)
                && let Some(LintConfig::NoArbitrarySpacing(config)) =
                    settings.get(LintKind::NoArbitrarySpacing)
                && let Some((domain, _)) = property_value_domains(property, &usage.value)
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
    }
    check_tokens(
        utility
            .diagnostic_data
            .token_uses
            .iter()
            .filter(|usage| !usage.implementation || matches!(settings.get(LintKind::NoRestrictedThemeTokens), Some(LintConfig::NoRestrictedThemeTokens(config)) if config.check_implementation))
            .flat_map(|usage| usage.tokens.iter().map(LeanStr::as_str)),
        theme,
        settings,
        span,
        findings,
    );
    for (index, variant) in utility
        .variants
        .iter()
        .skip(utility.diagnostic_data.wrapper_variant_count)
        .enumerate()
    {
        findings.extend(check_variant(
            variant,
            theme,
            settings,
            utility
                .diagnostic_data
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
            if theme.token_property(token).is_some()
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
        resolved.diagnostic_data.tokens.iter().map(LeanStr::as_str),
        theme,
        settings,
        span,
        &mut findings,
    );
    if settings.enabled_for_detection(LintKind::NoArbitraryBreakpoints) {
        let conditions: BTreeSet<_> = resolved
            .diagnostic_data
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
            let selector = path_selector(&path);
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
    let Ok(Ok(utilities)) = resolve_utility_targets(raw, theme, VariantContext::Element) else {
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
