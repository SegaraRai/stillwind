use super::{CompiledUtility, prepend_missing_declarations};

pub(super) fn render_utility_rule(
    class_name: &str,
    utility: &CompiledUtility,
    theme: &stillwind_css::ThemeIndex,
    declaration: &str,
    nested_rule: Option<&stillwind_tailwind::NestedUtilityRule>,
) -> String {
    let declaration = stillwind_css::fold_calc_expressions(declaration);
    let declaration = stillwind_css::normalize_serialized_declaration(&declaration);
    let mut paths = vec![Vec::new()];
    for variant in &utility.variants {
        let Some(application) = stillwind_tailwind::resolve_variant(variant, theme) else {
            continue;
        };
        let alternatives = match application {
            stillwind_tailwind::VariantApplication::Compound(applications) => applications,
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

pub(super) fn render_utility_rule_path(
    class_name: &str,
    utility: &CompiledUtility,
    declaration: &str,
    nested_rule: Option<&stillwind_tailwind::NestedUtilityRule>,
    applications: &[stillwind_tailwind::VariantApplication],
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
    for application in applications {
        match application {
            stillwind_tailwind::VariantApplication::Compound(_) => unreachable!(),
            stillwind_tailwind::VariantApplication::SelectorSuffix(suffix) => {
                selector.push_str(suffix);
            }
            stillwind_tailwind::VariantApplication::SelectorTemplate(template) => {
                selector = template.replace('&', &selector);
            }
            stillwind_tailwind::VariantApplication::SelectorRule {
                selector: template,
                declarations,
            } => {
                selector = template.replace('&', &selector);
                prepend_missing_declarations(&mut declaration, declarations);
            }
            stillwind_tailwind::VariantApplication::SelectorMedia {
                selector: template,
                condition,
            } => {
                selector = template.replace('&', &selector);
                media.push(condition.clone());
            }
            stillwind_tailwind::VariantApplication::SelectorMediaRule {
                selector: template,
                declarations,
                condition,
            } => {
                selector = template.replace('&', &selector);
                prepend_missing_declarations(&mut declaration, declarations);
                media.push(condition.clone());
            }
            stillwind_tailwind::VariantApplication::Media(condition) => {
                media.push(condition.clone());
            }
            stillwind_tailwind::VariantApplication::Container(condition) => {
                containers.push(condition.clone());
            }
            stillwind_tailwind::VariantApplication::Supports(condition) => {
                supports.push(condition.clone());
            }
            stillwind_tailwind::VariantApplication::StartingStyle => {
                starting_style = true;
            }
        }
    }
    let mut rule = format!("{selector}{{{declaration}}}");
    if let Some(nested) = nested_rule {
        rule = match nested.at_rule {
            stillwind_tailwind::NestedUtilityAtRule::Media => {
                format!("@media {}{{{rule}}}", nested.condition)
            }
            stillwind_tailwind::NestedUtilityAtRule::Supports => {
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
