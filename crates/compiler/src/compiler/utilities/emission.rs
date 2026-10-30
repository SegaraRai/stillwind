//! Emission of one class unit.
//!
//! A class unit's blocks are ordered before they are serialized: the plain block first,
//! then the variant blocks in the canonical variant order, then the rules that exist
//! only inside an at-rule from a CSS-defined utility. A plain utility that carries a
//! nested rule stays where it appears in the plain run, because splitting there would
//! move declarations relative to each other. Nested declarations that live inside the
//! same rule are inserted after the property they override, so a nested at-rule cannot
//! win by appearing before the value it is meant to replace.

use std::ops::Range;

use lean_string::{LeanString, ToLeanString};

use stillwind_css::{
    CssNode, CssNodeKind, declaration_ranges, important_declarations, parse_structure,
};
use stillwind_design_system::{
    NestedUtilityAtRule, NestedUtilityRule, ThemeIndex, UtilityImportance,
    compare_variant_sequences,
};
use stillwind_diagnostics::CompileDiagnostic;

use super::super::CssContribution;
use super::super::stylesheet::{applied_banners, expand_authored_directives};
use super::declaration_order::{combine_declarations_for_theme, order_utilities};
use super::render::render_utility_rule;
use super::resolve::CompiledUtility;
use crate::identifiers::IdentifierContext;

/// Emits modern color declarations without their generated `@supports` guards.
/// Authored conditions remain present.
pub(crate) const FLATTEN_COLOR_COMPATIBILITY: u8 = 1 << 0;

/// Writes the blocks of one class unit for `class_selector`.
///
/// The unit's class selector is rendered by the family that owns the class token, so
/// this function never escapes a name. The class is generated from the unit's canonical
/// content, so this function never has to deduplicate: two equal units were already
/// merged by the caller.
///
/// `FLAGS` is a bitmask. Use zero for normal CSS or [`FLATTEN_COLOR_COMPATIBILITY`]
/// when comparing canonical candidates.
pub(crate) fn emit_utility_rules<const FLAGS: u8>(
    contributions: &mut Vec<CssContribution>,
    class_selector: &str,
    utilities: &[CompiledUtility],
    theme: &ThemeIndex,
    identifiers: &IdentifierContext,
) -> Result<(), CompileDiagnostic> {
    let important;
    let utilities = if theme.important() == Some(&UtilityImportance::Declarations) {
        important = utilities.iter().map(important_utility).collect::<Vec<_>>();
        important.as_slice()
    } else {
        utilities
    };
    let mut css = LeanString::new();
    emit_utility_css::<FLAGS>(&mut css, class_selector, utilities, theme, identifiers)?;
    let mut required_properties = utilities
        .iter()
        .flat_map(|utility| utility.required_properties.clone())
        .collect::<Vec<_>>();
    // A utility body can apply other utilities, as a Tailwind plugin's `@apply p-4`
    // does. The rendered rules anchor the applications, so variants of the utility
    // also select the applied declarations.
    let applied_banners = applied_banners(&css, theme);
    if css.contains("@apply") {
        css = expand_authored_directives(&css, theme, &mut required_properties, identifiers)?;
    }
    // A utility body can define the keyframes and custom properties it uses, as
    // `@utility` and Tailwind plugin utilities do. Browsers ignore these rules inside a
    // style rule, so they move to the keyframes slot and are emitted only with a used
    // utility.
    let keyframes = if css.contains("@keyframes") || css.contains("@property") {
        let (remaining, definitions) = hoist_definitions(&css);
        css = remaining;
        definitions
    } else {
        LeanString::new()
    };
    let banners: Vec<_> = utilities
        .iter()
        .flat_map(|utility| utility.banners.clone())
        .chain(applied_banners)
        .collect();
    contributions.push(CssContribution {
        banners: banners.clone(),
        css: (css).into(),
        definition: false,
        required_properties,
        keyframes: false,
    });
    if !keyframes.is_empty() {
        contributions.push(CssContribution {
            banners,
            css: keyframes.into(),
            definition: false,
            required_properties: Vec::new(),
            keyframes: true,
        });
    }
    Ok(())
}

/// Moves the `@keyframes` and `@property` rules of rendered utility CSS out of their
/// style rules.
///
/// Each moved rule keeps its enclosing at-rules, such as `@media`, and loses its
/// enclosing style rules. A rule or at-rule whose block contained only moved rules is
/// removed with them. Returns the remaining CSS and the moved rules in source order.
fn hoist_definitions(css: &str) -> (LeanString, LeanString) {
    /// Collects the ranges to remove from `nodes` and the moved rules, and returns
    /// whether every node is removed.
    fn visit(
        css: &str,
        nodes: &[CssNode],
        conditions: &mut Vec<LeanString>,
        removed: &mut Vec<Range<usize>>,
        definitions: &mut LeanString,
    ) -> bool {
        let mut all = !nodes.is_empty();
        for node in nodes {
            let gone =
                match &node.kind {
                    CssNodeKind::AtRule { name, .. }
                        if matches!(name.as_str(), "keyframes" | "property") =>
                    {
                        let rule = &css[node.range.clone()];
                        definitions.push_str(&conditions.iter().rev().fold(
                            rule.to_lean_string(),
                            |body, condition| {
                                format_args!("{condition}{{{body}}}").to_lean_string()
                            },
                        ));
                        true
                    }
                    CssNodeKind::AtRule {
                        name,
                        prelude,
                        block: Some(block),
                    } => {
                        let prelude = &css[prelude.clone()];
                        conditions.push(if prelude.is_empty() {
                            format_args!("@{name}").to_lean_string()
                        } else {
                            format_args!("@{name} {prelude}").to_lean_string()
                        });
                        let start = removed.len();
                        let gone = visit(css, &block.children, conditions, removed, definitions);
                        conditions.pop();
                        if gone {
                            removed.truncate(start);
                        }
                        gone
                    }
                    CssNodeKind::StyleRule { block, .. } => {
                        let start = removed.len();
                        let gone = visit(css, &block.children, conditions, removed, definitions);
                        if gone {
                            removed.truncate(start);
                        }
                        gone
                    }
                    _ => false,
                };
            if gone {
                removed.push(node.range.clone());
            }
            all &= gone;
        }
        all
    }
    let mut removed = Vec::new();
    let mut definitions = LeanString::new();
    visit(
        css,
        &parse_structure(css),
        &mut Vec::new(),
        &mut removed,
        &mut definitions,
    );
    removed.sort_by_key(|range| range.start);
    let mut remaining = LeanString::new();
    let mut cursor = 0;
    for range in removed {
        remaining.push_str(&css[cursor..range.start]);
        cursor = range.end;
    }
    remaining.push_str(&css[cursor..]);
    (remaining, definitions)
}

/// Returns a copy of `utility` whose declarations, including those in conditional
/// rules, are `!important`.
fn important_utility(utility: &CompiledUtility) -> CompiledUtility {
    let mut utility = utility.clone();
    utility.declaration = important_declarations(&utility.declaration).into();
    for nested in &mut utility.nested {
        nested.declaration = important_declarations(&nested.declaration).into();
    }
    utility
}

fn emit_utility_css<const FLAGS: u8>(
    output: &mut LeanString,
    class_selector: &str,
    utilities: &[CompiledUtility],
    theme: &ThemeIndex,
    identifiers: &IdentifierContext,
) -> Result<(), CompileDiagnostic> {
    let utilities = order_utilities(utilities.iter().collect());
    let base = utilities
        .iter()
        .copied()
        .filter(|utility| {
            utility.variants.is_empty()
                && utility.variant_applications.is_empty()
                && utility.selector.is_none()
                && utility.container.is_none()
        })
        .collect::<Vec<_>>();
    let mut plain = Vec::new();
    for utility in base {
        if utility.nested.iter().any(|nested| nested.inside) {
            emit_base_utility_group::<FLAGS>(output, class_selector, &plain, theme);
            plain.clear();
            emit_base_utility_group::<FLAGS>(output, class_selector, &[utility], theme);
        } else {
            plain.push(utility);
        }
    }
    emit_base_utility_group::<FLAGS>(output, class_selector, &plain, theme);
    let mut variant_utilities = utilities
        .iter()
        .copied()
        .filter(|utility| {
            !utility.variants.is_empty()
                || !utility.variant_applications.is_empty()
                || utility.selector.is_some()
                || utility.container.is_some()
        })
        .collect::<Vec<_>>();
    variant_utilities.sort_by(|left, right| {
        compare_variant_sequences(&left.variants, &right.variants, theme).then_with(|| {
            left.order
                .compare(&left.candidate, &right.order, &right.candidate)
        })
    });
    for utility in variant_utilities {
        let declaration = assemble_nested_declarations::<FLAGS>(
            &combine_declarations_for_theme(&[utility], theme),
            &utility
                .nested
                .iter()
                .filter(|nested| nested.inside)
                .collect::<Vec<_>>(),
        );
        output.push_str(&render_utility_rule(
            class_selector,
            utility,
            theme,
            &declaration,
            None,
            identifiers,
        )?);
    }
    for utility in utilities {
        for nested in &utility.nested {
            let nested_in_rule = nested.inside;
            if nested_in_rule {
                continue;
            }
            output.push_str(&render_utility_rule(
                class_selector,
                utility,
                theme,
                &nested.declaration,
                Some(nested),
                identifiers,
            )?);
        }
    }
    Ok(())
}

/// Writes one `<class selector>{...}` block for a run of plain utilities.
///
/// Nested rules whose property is present in the combined declarations are inlined right after that
/// property; the rest follow the declarations in their own order. Utilities with no declarations at
/// all produce no block, which is why a marker-only unit still costs nothing in the CSS.
fn emit_base_utility_group<const FLAGS: u8>(
    output: &mut LeanString,
    class_selector: &str,
    utilities: &[&CompiledUtility],
    theme: &ThemeIndex,
) {
    let base = combine_declarations_for_theme(utilities, theme);
    let nested = utilities
        .iter()
        .flat_map(|utility| &utility.nested)
        .filter(|nested| nested.inside)
        .collect::<Vec<_>>();
    if !base.is_empty() || !nested.is_empty() {
        output.push_str(class_selector);
        output.push('{');
        output.push_str(&assemble_nested_declarations::<FLAGS>(&base, &nested));
        output.push('}');
    }
}

/// Returns `base` with the rules in `nested` inserted at declaration boundaries.
///
/// Each rule follows the first base declaration whose property matches the start of
/// its declaration text. Rules without a matching property follow the base body.
/// Rules at the same boundary retain their input order.
///
/// `FLAGS` uses the emission flags of [`emit_utility_rules`]. Zero retains conditional
/// wrappers; [`FLATTEN_COLOR_COMPATIBILITY`] flattens generated color support queries
/// for comparison. It also merges compatibility branches with adjacent authored
/// parents when they share a condition and declaration boundary.
pub(in crate::compiler) fn assemble_nested_declarations<const FLAGS: u8>(
    base: &str,
    nested: &[&NestedUtilityRule],
) -> LeanString {
    let declarations = declaration_ranges(base)
        .into_iter()
        .map(|range| &base[range])
        .collect::<Vec<_>>();
    let comparison_rules;
    let comparison_refs;
    let nested = if FLAGS & FLATTEN_COLOR_COMPATIBILITY != 0
        && nested
            .iter()
            .any(|rule| rule.compatibility && rule.parent.is_some())
    {
        comparison_rules = merge_compatibility_parents(nested, &declarations);
        comparison_refs = comparison_rules.iter().collect::<Vec<_>>();
        &comparison_refs
    } else {
        nested
    };
    let anchors = nested
        .iter()
        .map(|rule| nested_anchor(rule, &declarations))
        .collect::<Vec<_>>();
    let mut output = LeanString::new();
    for (index, declaration) in declarations.iter().enumerate() {
        output.push_str(declaration);
        output.push(';');
        for (rule, anchor) in nested.iter().zip(&anchors) {
            if *anchor == Some(index) {
                emit_nested_rule::<FLAGS>(&mut output, rule);
            }
        }
    }
    for (rule, anchor) in nested.iter().zip(&anchors) {
        if anchor.is_none() {
            emit_nested_rule::<FLAGS>(&mut output, rule);
        }
    }
    output
}

/// Joins a generated compatibility declaration to the preceding authored condition
/// when both are emitted at the same declaration boundary. Sharing their declaration
/// list lets canonical minification remove the overridden fallback in one pass.
fn merge_compatibility_parents(
    nested: &[&NestedUtilityRule],
    declarations: &[&str],
) -> Vec<NestedUtilityRule> {
    let mut rules: Vec<NestedUtilityRule> = Vec::with_capacity(nested.len());
    for rule in nested {
        if rule.compatibility
            && let Some((at_rule, condition)) = &rule.parent
            && let Some(parent) = rules.last_mut()
            && !parent.compatibility
            && parent.parent.is_none()
            && parent.at_rule == *at_rule
            && parent.condition == *condition
            && nested_anchor(parent, declarations) == nested_anchor(rule, declarations)
        {
            parent.declaration = format_args!("{};{}", parent.declaration, rule.declaration)
                .to_lean_string()
                .into();
        } else {
            rules.push((*rule).clone());
        }
    }
    rules
}

/// Returns the first base declaration whose property anchors this conditional rule.
/// A rule without a matching property follows the entire base declaration list.
fn nested_anchor(rule: &NestedUtilityRule, declarations: &[&str]) -> Option<usize> {
    let property = rule.declaration.split_once(':')?.0.trim();
    declarations.iter().position(|declaration| {
        declaration
            .split_once(':')
            .is_some_and(|(name, _)| name.trim() == property)
    })
}

fn emit_nested_rule<const FLAGS: u8>(output: &mut LeanString, nested: &NestedUtilityRule) {
    if let Some((at_rule, condition)) = &nested.parent {
        emit_condition(output, *at_rule, condition);
    }
    let guarded = FLAGS & FLATTEN_COLOR_COMPATIBILITY == 0 || !nested.compatibility;
    if guarded {
        emit_condition(output, nested.at_rule, &nested.condition);
    }
    output.push_str(&nested.declaration);
    if guarded {
        output.push('}');
    } else {
        output.push(';');
    }
    if nested.parent.is_some() {
        output.push('}');
    }
}

fn emit_condition(output: &mut LeanString, at_rule: NestedUtilityAtRule, condition: &str) {
    match at_rule {
        NestedUtilityAtRule::Media => {
            output.push_str("@media ");
            output.push_str(condition);
            output.push('{');
        }
        NestedUtilityAtRule::Supports => {
            output.push_str("@supports (");
            output.push_str(condition);
            output.push_str("){");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_declarations_follow_the_property_they_override() {
        let declaration = "color:red;width:1px;";
        let rule = NestedUtilityRule {
            policy_declarations: Vec::new(),
            compatibility: false,
            at_rule: NestedUtilityAtRule::Media,
            condition: "print".into(),
            parent: None,
            declaration: "color:blue;".into(),
            inside: false,
        };
        let declaration = assemble_nested_declarations::<0>(declaration, &[&rule]);
        assert_eq!(declaration, "color:red;@media print{color:blue;}width:1px;");
    }

    #[test]
    fn canonical_compatibility_keeps_distinct_declaration_boundaries() {
        let parent = NestedUtilityRule {
            policy_declarations: Vec::new(),
            compatibility: false,
            at_rule: NestedUtilityAtRule::Media,
            condition: "print".into(),
            parent: None,
            declaration: "outline-color:yellow;color:gray;".into(),
            inside: true,
        };
        let modern = NestedUtilityRule {
            policy_declarations: Vec::new(),
            compatibility: true,
            parent: Some((parent.at_rule, parent.condition.clone())),
            declaration: "color:blue;".into(),
            ..parent.clone()
        };
        let declaration = assemble_nested_declarations::<FLATTEN_COLOR_COMPATIBILITY>(
            "outline-color:green;color:black;",
            &[&parent, &modern],
        );
        assert!(
            declaration.find("color:blue;").unwrap() > declaration.find("color:black;").unwrap()
        );
    }

    #[test]
    fn nested_rules_keep_boundaries_unmatched_rules_and_source_order() {
        let first = NestedUtilityRule {
            policy_declarations: Vec::new(),
            compatibility: false,
            at_rule: NestedUtilityAtRule::Media,
            condition: "print".into(),
            parent: None,
            declaration: "color:blue;".into(),
            inside: true,
        };
        let second = NestedUtilityRule {
            declaration: "color:green;".into(),
            ..first.clone()
        };
        let unmatched = NestedUtilityRule {
            declaration: "opacity:0.5;".into(),
            ..first.clone()
        };
        assert_eq!(
            assemble_nested_declarations::<0>(
                "border-color:red;content:'color:;';color:red;color:black;",
                &[&first, &second, &unmatched]
            ),
            "border-color:red;content:'color:;';color:red;@media print{color:blue;}@media print{color:green;}color:black;@media print{opacity:0.5;}"
        );
        assert_eq!(
            assemble_nested_declarations::<0>("", &[&unmatched]),
            "@media print{opacity:0.5;}"
        );
    }
}
