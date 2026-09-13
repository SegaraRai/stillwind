//! Emission of one class unit.
//!
//! A class unit's blocks are ordered before they are serialized: the plain block first, then the
//! variant blocks in the canonical variant order, then the rules that exist only inside an at-rule
//! from a CSS-defined utility. A plain utility that carries a nested rule stays where it appears in
//! the plain run, because splitting there would move declarations relative to each other. Nested
//! declarations that live inside the same rule are inserted after the property they override, so a
//! nested at-rule cannot win by appearing before the value it is meant to replace.

use stillwind_css::declaration_ranges;
use stillwind_design_system::{
    NestedUtilityAtRule, NestedUtilityRule, ThemeIndex, compare_variant_sequences,
};

use super::declaration_order::{combine_declarations_for_theme, order_builtin_utilities};
use super::render::render_utility_rule;
use super::resolve::CompiledUtility;

/// Writes the blocks of one class unit for `class_name`.
///
/// The class name is generated from the unit's canonical content, so this function never has to
/// deduplicate: two equal units were already merged by the caller.
pub(crate) fn emit_utility_rules(
    output: &mut String,
    class_name: &str,
    utilities: &[CompiledUtility],
    theme: &ThemeIndex,
) {
    // Fragment transport preserves semantic support requests until the stylesheet
    // assembler places them in registration slots. They never reach final CSS.
    for utility in utilities {
        if !utility.required_properties.is_empty() {
            output.push_str("@stillwind-require-properties ");
            output.push_str(&utility.required_properties.join(" "));
            output.push(';');
        }
    }
    let utilities = order_builtin_utilities(utilities.iter().collect());
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
            emit_base_utility_group(output, class_name, &plain, theme);
            plain.clear();
            emit_base_utility_group(output, class_name, &[utility], theme);
        } else {
            plain.push(utility);
        }
    }
    emit_base_utility_group(output, class_name, &plain, theme);
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
        compare_variant_sequences(&left.variants, &right.variants, theme)
            .then_with(|| left.builtin_order.cmp(&right.builtin_order))
            .then_with(|| left.candidate.cmp(&right.candidate))
    });
    for utility in variant_utilities {
        let declaration = assemble_nested_declarations(
            &combine_declarations_for_theme(&[utility], theme),
            &utility
                .nested
                .iter()
                .filter(|nested| nested.inside)
                .collect::<Vec<_>>(),
        );
        output.push_str(&render_utility_rule(
            class_name,
            utility,
            theme,
            &declaration,
            None,
        ));
    }
    for utility in utilities {
        for nested in &utility.nested {
            let nested_in_rule = nested.inside;
            if nested_in_rule {
                continue;
            }
            output.push_str(&render_utility_rule(
                class_name,
                utility,
                theme,
                &nested.declaration,
                Some(nested),
            ));
        }
    }
}

/// Writes one `.class{...}` block for a run of plain utilities.
///
/// Nested rules whose property is present in the combined declarations are inlined right after that
/// property; the rest follow the declarations in their own order. Utilities with no declarations at
/// all produce no block, which is why a marker-only unit still costs nothing in the CSS.
pub(in crate::compiler) fn emit_base_utility_group(
    output: &mut String,
    class_name: &str,
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
        output.push('.');
        output.push_str(class_name);
        output.push('{');
        output.push_str(&assemble_nested_declarations(&base, &nested));
        output.push('}');
    }
}

/// Arranges conditional rules at declaration boundaries before emitting the body.
/// Each conditional is emitted once, in source order, including unmatched properties.
fn assemble_nested_declarations(base: &str, nested: &[&NestedUtilityRule]) -> String {
    let declarations = declaration_ranges(base)
        .into_iter()
        .map(|range| &base[range])
        .collect::<Vec<_>>();
    let anchors = nested
        .iter()
        .map(|rule| {
            let property = rule.declaration.split_once(':')?.0.trim();
            declarations.iter().position(|declaration| {
                declaration
                    .split_once(':')
                    .is_some_and(|(name, _)| name.trim() == property)
            })
        })
        .collect::<Vec<_>>();
    let mut output = String::new();
    for (index, declaration) in declarations.iter().enumerate() {
        output.push_str(declaration);
        output.push(';');
        for (rule, anchor) in nested.iter().zip(&anchors) {
            if *anchor == Some(index) {
                emit_nested_rule(&mut output, rule);
            }
        }
    }
    for (rule, anchor) in nested.iter().zip(&anchors) {
        if anchor.is_none() {
            emit_nested_rule(&mut output, rule);
        }
    }
    output
}

fn emit_nested_rule(output: &mut String, nested: &NestedUtilityRule) {
    match nested.at_rule {
        NestedUtilityAtRule::Media => {
            output.push_str("@media ");
            output.push_str(&nested.condition);
            output.push('{');
        }
        NestedUtilityAtRule::Supports => {
            output.push_str("@supports (");
            output.push_str(&nested.condition);
            output.push_str("){");
        }
    }
    output.push_str(&nested.declaration);
    output.push('}');
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_declarations_follow_the_property_they_override() {
        let declaration = "color:red;width:1px;";
        let rule = NestedUtilityRule {
            at_rule: NestedUtilityAtRule::Media,
            condition: "print".into(),
            declaration: "color:blue;".into(),
            inside: false,
        };
        let declaration = assemble_nested_declarations(declaration, &[&rule]);
        assert_eq!(declaration, "color:red;@media print{color:blue;}width:1px;");
    }

    #[test]
    fn nested_rules_keep_boundaries_unmatched_rules_and_source_order() {
        let first = NestedUtilityRule {
            at_rule: NestedUtilityAtRule::Media,
            condition: "print".into(),
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
            assemble_nested_declarations(
                "border-color:red;content:'color:;';color:red;color:black;",
                &[&first, &second, &unmatched]
            ),
            "border-color:red;content:'color:;';color:red;@media print{color:blue;}@media print{color:green;}color:black;@media print{opacity:0.5;}"
        );
        assert_eq!(
            assemble_nested_declarations("", &[&unmatched]),
            "@media print{opacity:0.5;}"
        );
    }
}
