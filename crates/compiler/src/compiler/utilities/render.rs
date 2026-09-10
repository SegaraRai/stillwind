use stillwind_css::{fold_calc_expressions, normalize_serialized_declaration};
use stillwind_design_system::{
    NestedUtilityAtRule, NestedUtilityRule, ThemeIndex, VariantApplication, resolve_variant,
};

use super::declaration_order::prepend_missing_declarations;
use super::resolve::CompiledUtility;
use crate::compiler::variants::{VariantStep, variant_paths};

pub(in crate::compiler) fn render_utility_rule(
    class_name: &str,
    utility: &CompiledUtility,
    theme: &ThemeIndex,
    declaration: &str,
    nested_rule: Option<&NestedUtilityRule>,
) -> String {
    let declaration = fold_calc_expressions(declaration);
    let declaration = normalize_serialized_declaration(&declaration);
    let mut paths = vec![Vec::new()];
    for variant in &utility.variants {
        let Some(application) = resolve_variant(variant, theme) else {
            continue;
        };
        let alternatives = match application {
            VariantApplication::Compound(applications) => applications,
            application => vec![application],
        };
        paths = paths
            .into_iter()
            .flat_map(|path| {
                alternatives.iter().cloned().map(move |application| {
                    let mut path = path.clone();
                    path.push(application);
                    path
                })
            })
            .collect();
    }
    paths
        .iter()
        .map(|applications| {
            render_utility_rule_path(class_name, utility, &declaration, nested_rule, applications)
        })
        .collect()
}

pub(in crate::compiler) fn render_utility_rule_path(
    class_name: &str,
    utility: &CompiledUtility,
    declaration: &str,
    nested_rule: Option<&NestedUtilityRule>,
    applications: &[VariantApplication],
) -> String {
    variant_paths(applications)
        .iter()
        .map(|path| render_utility_steps(class_name, utility, declaration, nested_rule, path))
        .collect()
}

fn render_utility_steps(
    class_name: &str,
    utility: &CompiledUtility,
    declaration: &str,
    nested_rule: Option<&NestedUtilityRule>,
    applications: &[VariantStep<'_>],
) -> String {
    let mut declaration = declaration.to_owned();
    let class_selector = format!(".{class_name}");
    let mut selector = utility.selector.as_deref().map_or_else(
        || class_selector.clone(),
        |selector| selector.replace('&', &class_selector),
    );
    let mut media = Vec::new();
    let mut containers = Vec::new();
    let mut supports = Vec::new();
    let mut starting_style = false;
    for &application in applications {
        match application {
            VariantStep::SelectorSuffix(suffix) => {
                selector.push_str(suffix);
            }
            VariantStep::SelectorTemplate(template) => {
                selector = template.replace('&', &selector);
            }
            VariantStep::SelectorRule {
                selector: template,
                declarations,
            } => {
                selector = template.replace('&', &selector);
                prepend_missing_declarations(&mut declaration, declarations);
            }
            VariantStep::SelectorMedia {
                selector: template,
                condition,
            } => {
                selector = template.replace('&', &selector);
                media.push(condition.to_owned());
            }
            VariantStep::SelectorMediaRule {
                selector: template,
                declarations,
                condition,
            } => {
                selector = template.replace('&', &selector);
                prepend_missing_declarations(&mut declaration, declarations);
                media.push(condition.to_owned());
            }
            VariantStep::Media(condition) => {
                media.push(condition.to_owned());
            }
            VariantStep::Container(condition) => {
                containers.push(condition.to_owned());
            }
            VariantStep::Supports(condition) => {
                supports.push(condition.to_owned());
            }
            VariantStep::StartingStyle => {
                starting_style = true;
            }
        }
    }
    let mut rule = format!("{selector}{{{declaration}}}");
    if let Some(nested) = nested_rule {
        rule = match nested.at_rule {
            NestedUtilityAtRule::Media => {
                format!("@media {}{{{rule}}}", nested.condition)
            }
            NestedUtilityAtRule::Supports => {
                format!("@supports ({}){{{rule}}}", nested.condition)
            }
        };
    }
    for condition in supports.into_iter().rev() {
        rule = format!("@supports ({condition}){{{rule}}}");
    }
    for condition in media.into_iter().rev() {
        rule = format!("@media {condition}{{{rule}}}");
    }
    for condition in containers.into_iter().rev() {
        rule = format!("@container {condition}{{{rule}}}");
    }
    if starting_style {
        rule = format!("@starting-style{{{rule}}}");
    }
    if let Some(container) = &utility.container {
        rule = format!("@container {container}{{{rule}}}");
    }
    rule
}
