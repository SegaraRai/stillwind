//! Rule rendering for one resolved utility.
//!
//! A utility renders to one rule per variant path: the selector starts as the utility's
//! own selector or as the unit's class selector, and each step rewrites it. The at
//! rules a step contributes wrap the finished rule in a fixed order - `@supports`,
//! `@media`, `@container`, then `@starting-style` - so output is deterministic and
//! conditions combine by containment instead of being merged into one query. Resolved
//! values are emitted without minification.

use stillwind_css::instantiate_selector;
use stillwind_design_system::{
    NestedUtilityAtRule, NestedUtilityRule, ThemeIndex, VariantApplication, resolve_variant,
};
use stillwind_diagnostics::CompileErrorKind;

use super::super::conditionals::{guard, isolate_rule};
use super::super::variants::{VariantStep, variant_paths};
use super::declaration_order::prepend_missing_declarations;
use super::resolve::CompiledUtility;
use crate::identifiers::IdentifierContext;

/// Renders one utility as one or more complete rules.
///
/// A utility that carries pre-resolved applications is rendered from those, without re-resolving
/// its variants; otherwise each variant is resolved and every alternative it expands to produces a
/// separate rule. A variant that no longer resolves is skipped rather than failing the rule, which
/// keeps one stale variant from hiding a whole unit. Runtime conditions guard the
/// final declarations, including variant additions, with unresolved `--if` expressions.
/// Their rules receive distinct support groups; callers must subsequently run the
/// conditional-lowering pass with compiler options before publishing CSS.
pub(in crate::compiler) fn render_utility_rule(
    class_selector: &str,
    utility: &CompiledUtility,
    theme: &ThemeIndex,
    declaration: &str,
    nested_rule: Option<&NestedUtilityRule>,
    identifiers: &IdentifierContext,
) -> Result<String, CompileErrorKind> {
    if !utility.variant_applications.is_empty() {
        return utility
            .variant_applications
            .iter()
            .map(|applications| {
                render_utility_rule_path(
                    class_selector,
                    utility,
                    declaration,
                    nested_rule,
                    applications,
                    identifiers,
                )
            })
            .collect();
    }
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
            render_utility_rule_path(
                class_selector,
                utility,
                declaration,
                nested_rule,
                applications,
                identifiers,
            )
        })
        .collect()
}

/// Renders all alternatives of an already-resolved variant path.
///
/// Runtime guards remain unresolved and use separate support groups, as in
/// [`render_utility_rule`]; no registry lookup or output-strategy choice occurs here.
pub(in crate::compiler) fn render_utility_rule_path(
    class_selector: &str,
    utility: &CompiledUtility,
    declaration: &str,
    nested_rule: Option<&NestedUtilityRule>,
    applications: &[VariantApplication],
    identifiers: &IdentifierContext,
) -> Result<String, CompileErrorKind> {
    variant_paths(applications)
        .iter()
        .map(|path| {
            render_utility_steps(
                class_selector,
                utility,
                declaration,
                nested_rule,
                path,
                identifiers,
            )
        })
        .collect()
}

/// Applies the steps of one path to the selector and declaration body.
///
/// Selector steps rewrite the selector in place, so the reference element's variants
/// stay attached to the reference and the surrounding variants to the target.
/// Declarations a variant contributes are only prepended when the body does not already
/// set that property.
fn render_utility_steps(
    class_selector: &str,
    utility: &CompiledUtility,
    declaration: &str,
    nested_rule: Option<&NestedUtilityRule>,
    applications: &[VariantStep<'_>],
    identifiers: &IdentifierContext,
) -> Result<String, CompileErrorKind> {
    let mut declaration = declaration.to_owned();
    let class_selector = class_selector.to_owned();
    let mut selector = utility.selector.as_deref().map_or_else(
        || class_selector.clone(),
        |selector| instantiate_selector(selector, &class_selector),
    );
    let mut media = Vec::new();
    let mut containers = Vec::new();
    let mut supports = Vec::new();
    let mut starting_style = false;
    let mut conditions = Vec::new();
    for &application in applications {
        match application {
            VariantStep::SelectorSuffix(suffix) => {
                selector.push_str(suffix);
            }
            VariantStep::SelectorTemplate(template) => {
                selector = instantiate_selector(template, &selector);
            }
            VariantStep::SelectorRule {
                selector: template,
                declarations,
            } => {
                selector = instantiate_selector(template, &selector);
                prepend_missing_declarations(&mut declaration, declarations);
            }
            VariantStep::SelectorMedia {
                selector: template,
                condition,
            } => {
                selector = instantiate_selector(template, &selector);
                media.push(condition.to_owned());
            }
            VariantStep::SelectorMediaRule {
                selector: template,
                declarations,
                condition,
            } => {
                selector = instantiate_selector(template, &selector);
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
            VariantStep::Conditional(condition) => conditions.push(condition),
            VariantStep::StartingStyle => {
                starting_style = true;
            }
        }
    }
    let conditional = !conditions.is_empty();
    // Every conditional guards the body the previous one produced, and the gate declarations a
    // pass writes are part of that body, so the names only have to survive this rule's own steps.
    let mut gates = Vec::new();
    for condition in conditions {
        let guarded = guard(&declaration, condition, identifiers, &gates)?;
        declaration = guarded.css;
        gates.extend(guarded.gates);
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
    if conditional {
        isolate_rule(&rule, identifiers)
    } else {
        Ok(rule)
    }
}

#[cfg(test)]
mod render_step_tests {
    use stillwind_design_system::{UtilityOrder, VariantDeclaration};

    use super::*;
    use crate::identifiers::{
        DEFAULT_HASH_LENGTH, GeneratorSet, IdentifierContext, IdentifierMode,
    };

    fn identifiers() -> IdentifierContext {
        IdentifierContext::new(
            GeneratorSet::default(),
            IdentifierMode::Release,
            DEFAULT_HASH_LENGTH,
        )
    }

    fn utility(selector: Option<&str>, declaration: &str) -> CompiledUtility {
        CompiledUtility {
            variants: Vec::new(),
            variant_applications: Vec::new(),
            declaration: declaration.to_owned(),
            required_properties: Vec::new(),
            selector: selector.map(str::to_owned),
            container: None,
            nested: Vec::new(),
            order: UtilityOrder::Authored(None),
            candidate: "x".into(),
        }
    }

    /// The class selector is used directly, and a utility selector template is
    /// instantiated against it.
    #[test]
    fn render_utility_steps_uses_the_utility_selector_template() {
        let context = identifiers();
        assert_eq!(
            render_utility_steps(
                ".sw_x",
                &utility(None, "color:red;"),
                "color:red;",
                None,
                &[],
                &context
            )
            .unwrap(),
            ".sw_x{color:red;}"
        );
        assert_eq!(
            render_utility_steps(
                ".sw_x",
                &utility(Some("&:hover"), "color:red;"),
                "color:red;",
                None,
                &[],
                &context
            )
            .unwrap(),
            ".sw_x:hover{color:red;}"
        );
    }

    /// Selector steps rewrite the selector in place.
    #[test]
    fn render_utility_steps_rewrites_the_selector_in_place() {
        let context = identifiers();
        let steps = [VariantStep::SelectorSuffix(":hover")];
        assert_eq!(
            render_utility_steps(
                ".sw_x",
                &utility(None, "color:red;"),
                "color:red;",
                None,
                &steps,
                &context
            )
            .unwrap(),
            ".sw_x:hover{color:red;}"
        );
    }

    /// Condition steps wrap the rule from the inside out.
    #[test]
    fn render_utility_steps_wraps_conditions_in_order() {
        let context = identifiers();
        let steps = [
            VariantStep::Media("(min-width: 40rem)"),
            VariantStep::Supports("display:grid"),
            VariantStep::Container("(width > 10px)"),
            VariantStep::StartingStyle,
        ];
        assert_eq!(
            render_utility_steps(
                ".sw_x",
                &utility(None, "color:red;"),
                "color:red;",
                None,
                &steps,
                &context
            )
            .unwrap(),
            "@starting-style{@container (width > 10px){@media (min-width: 40rem){@supports (display:grid){.sw_x{color:red;}}}}}"
        );
    }

    /// A variant's declarations are only prepended when the body does not already set
    /// that property, and a nested rule wraps the whole result.
    #[test]
    fn render_utility_steps_prepends_missing_declarations_and_wraps_nested_rules() {
        let context = identifiers();
        let declarations = [VariantDeclaration {
            property: "color".to_owned(),
            value: "blue".to_owned(),
        }];
        let steps = [VariantStep::SelectorRule {
            selector: "&:hover",
            declarations: &declarations,
        }];
        assert_eq!(
            render_utility_steps(
                ".sw_x",
                &utility(None, "color:red;"),
                "color:red;",
                None,
                &steps,
                &context
            )
            .unwrap(),
            ".sw_x:hover{color:red;}"
        );
        assert_eq!(
            render_utility_steps(
                ".sw_x",
                &utility(None, "padding:0;"),
                "padding:0;",
                None,
                &steps,
                &context
            )
            .unwrap(),
            ".sw_x:hover{color:blue;padding:0;}"
        );
        let nested = NestedUtilityRule {
            compatibility: false,
            at_rule: NestedUtilityAtRule::Supports,
            condition: "display:grid".to_owned(),
            declaration: "display:grid;".to_owned(),
            inside: false,
        };
        assert_eq!(
            render_utility_steps(
                ".sw_x",
                &utility(None, "color:red;"),
                "color:red;",
                Some(&nested),
                &[],
                &context
            )
            .unwrap(),
            "@supports (display:grid){.sw_x{color:red;}}"
        );
    }
}
