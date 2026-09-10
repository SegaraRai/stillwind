use stillwind_design_system::{
    NestedUtilityAtRule, NestedUtilityRule, ThemeIndex, compare_variant_sequences,
};

use super::declaration_order::{combine_declarations_for_theme, order_builtin_utilities};
use super::render::render_utility_rule;
use super::resolve::CompiledUtility;

pub(crate) fn emit_utility_rules(
    output: &mut String,
    class_name: &str,
    utilities: &[CompiledUtility],
    theme: &ThemeIndex,
) {
    let utilities = order_builtin_utilities(utilities.iter().collect());
    let base = utilities
        .iter()
        .copied()
        .filter(|utility| {
            utility.variants.is_empty() && utility.selector.is_none() && utility.container.is_none()
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
        let mut declaration = combine_declarations_for_theme(&[utility], theme);
        for nested in utility.nested.iter().filter(|nested| nested.inside) {
            insert_nested_declaration(&mut declaration, nested);
        }
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

pub(in crate::compiler) fn emit_base_utility_group(
    output: &mut String,
    class_name: &str,
    utilities: &[&CompiledUtility],
    theme: &ThemeIndex,
) {
    let base = combine_declarations_for_theme(utilities, theme);
    if !base.is_empty() {
        let mut nested_in_rule_css = String::new();
        let mut anchored_nested = Vec::<(String, String)>::new();
        output.push('.');
        output.push_str(class_name);
        output.push('{');
        for utility in utilities {
            for nested in &utility.nested {
                let nested_in_rule = nested.inside;
                if !nested_in_rule {
                    continue;
                }
                let mut nested_css = String::new();
                let nested_declaration = nested
                    .declaration
                    .replace(" deg", "deg")
                    .replace("deg*-1", "deg * -1");
                match nested.at_rule {
                    NestedUtilityAtRule::Media => {
                        nested_css.push_str("@media ");
                        nested_css.push_str(&nested.condition);
                        nested_css.push('{');
                        nested_css.push_str(&nested_declaration);
                        nested_css.push('}');
                    }
                    NestedUtilityAtRule::Supports => {
                        nested_css.push_str("@supports (");
                        nested_css.push_str(&nested.condition);
                        nested_css.push_str("){\n");
                        nested_css.push_str(&nested_declaration);
                        nested_css.push('}');
                    }
                }
                if let Some((property, _)) = nested.declaration.split_once(':')
                    && base.contains(&format!("{property}:"))
                {
                    anchored_nested.push((property.to_owned(), nested_css.clone()));
                }
                nested_in_rule_css.push_str(&nested_css);
            }
        }
        if anchored_nested.is_empty() {
            output.push_str(&base);
            output.push_str(&nested_in_rule_css);
        } else {
            let mut inlined = base.clone();
            for (property, nested_css) in anchored_nested {
                let marker = format!("{property}:");
                if let Some(start) = inlined.find(&marker)
                    && let Some(end) = inlined[start..].find(';')
                {
                    inlined.insert_str(start + end + 1, &nested_css);
                }
            }
            output.push_str(&inlined);
        }
        output.push('}');
    }
}

pub(in crate::compiler) fn insert_nested_declaration(
    declaration: &mut String,
    nested: &NestedUtilityRule,
) {
    let mut nested_css = String::new();
    match nested.at_rule {
        NestedUtilityAtRule::Media => {
            nested_css.push_str("@media ");
            nested_css.push_str(&nested.condition);
            nested_css.push('{');
        }
        NestedUtilityAtRule::Supports => {
            nested_css.push_str("@supports (");
            nested_css.push_str(&nested.condition);
            nested_css.push_str("){");
        }
    }
    nested_css.push_str(&nested.declaration);
    nested_css.push('}');
    let Some((property, _)) = nested.declaration.split_once(':') else {
        declaration.push_str(&nested_css);
        return;
    };
    let marker = format!("{}:", property.trim());
    let Some(start) = declaration.find(&marker) else {
        declaration.push_str(&nested_css);
        return;
    };
    let Some(end) = declaration[start..].find(';') else {
        declaration.push_str(&nested_css);
        return;
    };
    declaration.insert_str(start + end + 1, &nested_css);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_declarations_follow_the_property_they_override() {
        let mut declaration = "color:red;width:1px;".to_owned();
        let rule = NestedUtilityRule {
            at_rule: NestedUtilityAtRule::Media,
            condition: "print".into(),
            declaration: "color:blue;".into(),
            inside: false,
        };
        insert_nested_declaration(&mut declaration, &rule);
        assert_eq!(declaration, "color:red;@media print{color:blue;}width:1px;");
    }
}
