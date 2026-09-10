//! Non-compound steps shared by authored and generated CSS renderers.
use stillwind_design_system::{VariantApplication, VariantDeclaration};

#[derive(Clone, Copy)]
pub(super) enum VariantStep<'a> {
    SelectorSuffix(&'a str),
    SelectorTemplate(&'a str),
    SelectorRule {
        selector: &'a str,
        declarations: &'a [VariantDeclaration],
    },
    SelectorMedia {
        selector: &'a str,
        condition: &'a str,
    },
    SelectorMediaRule {
        selector: &'a str,
        declarations: &'a [VariantDeclaration],
        condition: &'a str,
    },
    Media(&'a str),
    Container(&'a str),
    Supports(&'a str),
    StartingStyle,
}

/// Expand alternatives without recursion, preserving authored branch order.
pub(super) fn variant_paths(applications: &[VariantApplication]) -> Vec<Vec<VariantStep<'_>>> {
    let mut paths = vec![Vec::new()];
    for application in applications {
        let mut pending = vec![application];
        let mut alternatives = Vec::new();
        while let Some(application) = pending.pop() {
            let step = match application {
                VariantApplication::Compound(children) => {
                    pending.extend(children.iter().rev());
                    continue;
                }
                VariantApplication::SelectorSuffix(suffix) => VariantStep::SelectorSuffix(suffix),
                VariantApplication::SelectorTemplate(selector) => {
                    VariantStep::SelectorTemplate(selector)
                }
                VariantApplication::SelectorRule {
                    selector,
                    declarations,
                } => VariantStep::SelectorRule {
                    selector,
                    declarations,
                },
                VariantApplication::SelectorMedia {
                    selector,
                    condition,
                } => VariantStep::SelectorMedia {
                    selector,
                    condition,
                },
                VariantApplication::SelectorMediaRule {
                    selector,
                    declarations,
                    condition,
                } => VariantStep::SelectorMediaRule {
                    selector,
                    declarations,
                    condition,
                },
                VariantApplication::Media(condition) => VariantStep::Media(condition),
                VariantApplication::Container(condition) => VariantStep::Container(condition),
                VariantApplication::Supports(condition) => VariantStep::Supports(condition),
                VariantApplication::StartingStyle => VariantStep::StartingStyle,
            };
            alternatives.push(step);
        }
        paths = paths
            .into_iter()
            .flat_map(|path| {
                alternatives.iter().map(move |step| {
                    let mut path = path.clone();
                    path.push(*step);
                    path
                })
            })
            .collect();
    }
    paths
}

#[cfg(test)]
mod tests {
    use super::super::stylesheet::authored_directives::render_authored_variant_path;
    use super::super::utilities::render::render_utility_rule_path;
    use super::super::utilities::resolve::CompiledUtility;
    use super::*;

    #[test]
    fn nested_alternatives_render_in_both_css_paths() {
        let applications = [
            VariantApplication::Compound(vec![
                VariantApplication::SelectorSuffix(":hover"),
                VariantApplication::Compound(vec![VariantApplication::SelectorSuffix(":focus")]),
            ]),
            VariantApplication::Media("print".into()),
        ];
        assert_eq!(
            render_authored_variant_path("color:red;", &applications),
            "@media print{&:hover{color:red;}}@media print{&:focus{color:red;}}"
        );
        let utility = CompiledUtility {
            variants: Vec::new(),
            declaration: "color:red;".into(),
            selector: None,
            container: None,
            nested: Vec::new(),
            builtin_order: None,
            candidate: String::new(),
        };
        assert_eq!(
            render_utility_rule_path("x", &utility, "color:red;", None, &applications),
            "@media print{.x:hover{color:red;}}@media print{.x:focus{color:red;}}"
        );
        assert_eq!(
            render_authored_variant_path("color:red;", &[VariantApplication::Compound(Vec::new())]),
            ""
        );
    }
}
