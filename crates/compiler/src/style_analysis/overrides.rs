//! Overrides that leave properties of the overridden utility in place.
//!
//! A utility of a later layer overrides a utility of the same family in an earlier
//! layer, as `text-[0.8rem]` overrides `text-sm`. When the earlier utility also sets a
//! property that no utility of the later layer sets, such as the `line-height` of
//! `text-sm`, that value remains. tailwind-merge removes the whole earlier utility in
//! these cases, so code written for it changes appearance.
//!
//! A later layer is a direct utility of the call that overrides one of these:
//!
//! - a utility of a base, in any condition that lets both apply;
//! - a direct utility that is written before it under fewer conditions, such as `p-2`
//!   in `style("p-2", wide && "p-4")`;
//! - a utility of a recipe base value that the call lists in its layers.
//!
//! Utilities that differ in CSS variants are not compared, because tailwind-merge
//! keeps both of them too.

use std::collections::{BTreeMap, BTreeSet};

use lean_string::{LeanStr, LeanString};

use stillwind_css::{properties_interfere, property_covers};
use stillwind_diagnostics::{Label, LabelScope, LintDiagnostic, LintFinding, LintKind};

use super::composition::ResolvedSupply;
use super::{ResolvedStyle, RuleContext, SupplyKind, UseId};

pub(super) fn check(style: &ResolvedStyle<'_>) -> Vec<LintFinding> {
    if !style
        .root
        .lint_settings
        .enabled_for_detection(LintKind::PartialUtilityOverride)
    {
        return Vec::new();
    }
    let root = &style.root.identity;
    let earlier: Vec<_> = style
        .supplies
        .iter()
        .chain(&style.layers)
        .filter(|supply| matches!(supply.supply.kind, SupplyKind::Utility { .. }))
        .collect();
    let later: Vec<_> = style
        .supplies
        .iter()
        .filter(|supply| {
            supply.owner == *root && matches!(supply.supply.kind, SupplyKind::Utility { .. })
        })
        .collect();
    let declarations = declarations_by_site(style);
    let direct_declarations: Vec<_> = style
        .supplies
        .iter()
        .filter(|supply| {
            supply.owner == *root
                && matches!(
                    supply.supply.kind,
                    SupplyKind::Declaration {
                        provider: false,
                        ..
                    }
                )
        })
        .collect();
    let mut findings = Vec::new();
    let mut reported = BTreeSet::new();
    for current in &later {
        let SupplyKind::Utility {
            candidate,
            contexts,
            family: Some(family),
        } = &current.supply.kind
        else {
            continue;
        };
        for previous in &earlier {
            let SupplyKind::Utility {
                candidate: previous_candidate,
                contexts: previous_contexts,
                family: Some(previous_family),
            } = &previous.supply.kind
            else {
                continue;
            };
            if family != previous_family
                || candidate == previous_candidate
                || !overrides(style, current, previous)
                || reported.contains(&(&previous.supply.site, &current.supply.site))
            {
                continue;
            }
            let remaining = remaining_properties(
                style,
                &declarations,
                &direct_declarations,
                (current, contexts),
                (previous, previous_contexts),
            );
            if remaining.is_empty() {
                continue;
            }
            reported.insert((&previous.supply.site, &current.supply.site));
            let mut finding = LintFinding::new(
                LintDiagnostic::PartialUtilityOverride {
                    previous: previous_candidate.clone(),
                    utility: candidate.clone(),
                    properties: join(&remaining).into(),
                },
                current.supply.site.span,
            );
            if previous.supply.site.module == current.supply.site.module {
                finding.info.labels.push(Label {
                    scope: LabelScope::Range,
                    span: previous.supply.site.span,
                    message: Some("overridden utility".into()),
                });
            }
            findings.push(finding);
        }
    }
    findings
}

/// Returns the ordinary properties that `previous` sets and that no direct declaration
/// overriding it sets, in the contexts that `current` and `previous` share. A context
/// counts only when `current` writes a property that interferes with a property of
/// `previous`, and when `previous` does not win there with `!important`. Each supply
/// comes with the contexts of its utility.
fn remaining_properties(
    style: &ResolvedStyle<'_>,
    declarations: &BTreeMap<&UseId, Vec<&ResolvedSupply<'_>>>,
    direct_declarations: &[&ResolvedSupply<'_>],
    (current, contexts): (&ResolvedSupply<'_>, &[RuleContext]),
    (previous, previous_contexts): (&ResolvedSupply<'_>, &[RuleContext]),
) -> BTreeSet<LeanStr> {
    let site = |supply: &ResolvedSupply<'_>| {
        declarations
            .get(&supply.supply.site)
            .map_or(&[][..], Vec::as_slice)
    };
    let overriding = direct_declarations
        .iter()
        .filter(|supply| supply.coexists(current) && overrides(style, supply, previous))
        .copied()
        .collect::<Vec<_>>();
    let mut remaining = BTreeSet::new();
    for context in contexts
        .iter()
        .filter(|context| previous_contexts.contains(context))
    {
        let written = properties(site(current), context, false);
        let replaced = properties(site(previous), context, false);
        if written.is_empty()
            || replaced.is_empty()
            || important(site(previous), context) && !important(site(current), context)
            || !written.iter().any(|written| {
                replaced
                    .iter()
                    .any(|replaced| properties_interfere(written, replaced))
            })
        {
            continue;
        }
        let set = properties(&overriding, context, false);
        remaining.extend(replaced.into_iter().filter(|property| {
            !property.starts_with("--")
                && !set
                    .iter()
                    .any(|written| written == property || property_covers(written, property))
        }));
    }
    remaining
}

/// Returns whether the CSS of `current`, a direct supply of the call, comes after the
/// CSS of `previous` whenever both apply. A supply of a base or of a recipe layer comes
/// first. Among direct supplies, `current` must be written after `previous` and under
/// more conditions.
fn overrides(
    style: &ResolvedStyle<'_>,
    current: &ResolvedSupply<'_>,
    previous: &ResolvedSupply<'_>,
) -> bool {
    if previous.owner == style.root.identity {
        // When the utility under more conditions is written first, as in
        // `style(big && "p-4", "p-2")`, `conflicting-utilities` reports the pair.
        current.extends(previous) && previous.supply.site < current.supply.site
    } else {
        current.coexists(previous)
    }
}

/// Groups the ordinary declarations of every supply, including inherited and recipe
/// layer supplies, by their source occurrence.
fn declarations_by_site<'s, 'a>(
    style: &'s ResolvedStyle<'a>,
) -> BTreeMap<&'s UseId, Vec<&'s ResolvedSupply<'a>>> {
    let mut declarations = BTreeMap::new();
    for supply in style.supplies.iter().chain(&style.layers) {
        if matches!(
            supply.supply.kind,
            SupplyKind::Declaration {
                provider: false,
                ..
            }
        ) {
            declarations
                .entry(&supply.supply.site)
                .or_insert_with(Vec::new)
                .push(supply);
        }
    }
    declarations
}

/// Returns the properties that `supplies` declare in `context`. `important` selects
/// only declarations with `!important`.
fn properties(
    supplies: &[&ResolvedSupply<'_>],
    context: &RuleContext,
    important: bool,
) -> BTreeSet<LeanStr> {
    supplies
        .iter()
        .filter_map(|supply| match &supply.supply.kind {
            SupplyKind::Declaration {
                property,
                context: declared,
                important: declared_important,
                ..
            } if declared == context && (!important || *declared_important) => {
                Some(property.clone())
            }
            _ => None,
        })
        .collect()
}

fn important(supplies: &[&ResolvedSupply<'_>], context: &RuleContext) -> bool {
    !properties(supplies, context, true).is_empty()
}

fn join(properties: &BTreeSet<LeanStr>) -> LeanString {
    let mut text = LeanString::new();
    for (index, property) in properties.iter().enumerate() {
        if index > 0 {
            text.push_str("`, `");
        }
        text.push_str(property);
    }
    text
}
