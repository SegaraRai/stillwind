//! Non-compound steps shared by authored and generated CSS renderers.
//!
//! A [`VariantStep`] is one effect a variant has on a rule, expressed without reference
//! to which renderer is applying it: a selector suffix or template, the declarations a
//! variant contributes, and the at-rules that contain everything else. Compound
//! applications are flattened here into alternative paths, so the authored `@variant`
//! path and the generated utility path expand the same inputs through the same code and
//! cannot drift apart.

use stillwind_design_system::{
    ThemeIndex, VariantApplication, VariantDeclaration, resolve_variant,
};
use stillwind_syntax::Variant;

/// One non-compound effect of a variant.
#[derive(Clone, Copy)]
pub(super) enum VariantStep<'a> {
    /// Custom-property test evaluated on the target element or pseudo-element.
    Conditional(
        /// Validated presence test or `--name:value` comparison, optionally prefixed by one `!`.
        &'a str,
    ),
    /// Appends a selector suffix to the current selector.
    SelectorSuffix(
        /// CSS selector suffix, including its combinators or punctuation; no separator is inserted.
        &'a str,
    ),
    /// Instantiates a selector template against the current selector.
    SelectorTemplate(
        /// CSS selector template with ampersands marking the current selector.
        &'a str,
    ),
    /// Applies a selector and contributes declarations.
    SelectorRule {
        /// CSS selector template with ampersand substitution sites, without braces.
        selector: &'a str,
        /// Variant declarations in their emission order.
        declarations: &'a [VariantDeclaration],
    },
    /// Applies a selector inside a media query.
    SelectorMedia {
        /// CSS selector template with ampersand substitution sites, without braces.
        selector: &'a str,
        /// Media query prelude without @media or braces; internal whitespace is retained.
        condition: &'a str,
    },
    /// Applies a selector and declarations inside a media query.
    SelectorMediaRule {
        /// CSS selector template with ampersand substitution sites, without braces.
        selector: &'a str,
        /// Variant declarations in their emission order.
        declarations: &'a [VariantDeclaration],
        /// Media query prelude without @media or braces; internal whitespace is retained.
        condition: &'a str,
    },
    /// A CSS `@media` wrapper.
    Media(
        /// Query prelude without `@media` or braces; includes required condition parentheses.
        &'a str,
    ),
    /// A CSS `@container` wrapper.
    Container(
        /// Container query prelude without `@container` or braces, including any container name.
        &'a str,
    ),
    /// A CSS `@supports` wrapper.
    Supports(
        /// Support condition without `@supports` or braces; the renderer adds outer parentheses.
        &'a str,
    ),
    /// A CSS `@starting-style` wrapper with no prelude.
    StartingStyle,
}

/// Expands alternatives and sequential compositions, preserving authored branch order.
///
/// The returned order is the order a reader sees in the source: alternatives of one compound
/// application keep their written order, and later applications only multiply the paths. The
/// Compound alternatives use an explicit stack; sequential children share this expansion.
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
                VariantApplication::Sequence(children) => {
                    alternatives.extend(variant_paths(children));
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
                VariantApplication::Conditional(condition) => VariantStep::Conditional(condition),
                VariantApplication::StartingStyle => VariantStep::StartingStyle,
            };
            alternatives.push(vec![step]);
        }
        paths = paths
            .into_iter()
            .flat_map(|path| {
                alternatives.iter().map(move |step| {
                    let mut path = path.clone();
                    path.extend(step);
                    path
                })
            })
            .collect();
    }
    paths
}

/// Whether every step of every path is a condition that can wrap a rule without a selector
/// of its own.
///
/// Definition-level rules such as keyframes and view-transition pseudo-elements have no
/// element to select, so only `@media` and `@supports` can contain them.
pub(super) fn conditions_only(paths: &[Vec<VariantStep<'_>>]) -> bool {
    paths
        .iter()
        .flatten()
        .all(|step| matches!(step, VariantStep::Media(_) | VariantStep::Supports(_)))
}

/// Returns the selector of `variant` when the rule it produces would apply to the document
/// instead of to the styled element.
///
/// A selector without the nesting subject is live wherever the stylesheet loads, which a
/// `style()` call must not author; document-wide CSS belongs in a stylesheet. Conditions and
/// selectors that keep the subject are unaffected.
pub(super) fn document_scoped_selector(variant: &Variant, theme: &ThemeIndex) -> Option<String> {
    let application = resolve_variant(variant, theme)?;
    stillwind_design_system::document_scoped_selector(&application).map(str::to_owned)
}

/// One condition wrapper a rule is nested under.
#[derive(Debug)]
pub(super) enum OptionCondition {
    /// `@media <condition>`
    Media(String),
    /// `@supports (<condition>)`
    Supports(String),
}

impl OptionCondition {
    /// The wrapper one resolved variant step produces.
    ///
    /// A step that cannot wrap a rule is not a condition, so the caller drops the utility rather
    /// than emitting it unconditionally.
    fn from_step(step: &VariantStep<'_>) -> Option<Self> {
        match step {
            VariantStep::Media(condition) => Some(Self::Media((*condition).to_owned())),
            VariantStep::Supports(condition) => Some(Self::Supports((*condition).to_owned())),
            _ => None,
        }
    }

    /// Wraps one rule in this condition.
    fn wrap(&self, rule: &str) -> String {
        match self {
            Self::Media(condition) => format!("@media {condition}{{{rule}}}"),
            Self::Supports(condition) => format!("@supports ({condition}){{{rule}}}"),
        }
    }
}

/// Wraps one rule in a condition path, outermost condition first.
pub(super) fn wrap_conditions(rule: &str, path: &[OptionCondition]) -> String {
    path.iter()
        .rev()
        .fold(rule.to_owned(), |wrapped, condition| {
            condition.wrap(&wrapped)
        })
}

/// The condition paths one variant list produces, or `None` when a variant cannot wrap a rule.
///
/// A view-transition pseudo-element has no element state of its own and no container ancestor,
/// so only media and supports conditions can wrap it. Any other variant fails the utility rather
/// than dropping its condition silently.
pub(super) fn condition_paths(
    variants: &[Variant],
    theme: &ThemeIndex,
) -> Option<Vec<Vec<OptionCondition>>> {
    let applications = condition_applications(variants, theme)?;
    variant_paths(&applications)
        .iter()
        .map(|path| path.iter().map(OptionCondition::from_step).collect())
        .collect()
}

/// Resolves a bucket utility's variants into the applications a pseudo-element rule can carry.
fn condition_applications(
    variants: &[Variant],
    theme: &ThemeIndex,
) -> Option<Vec<VariantApplication>> {
    let mut applications = Vec::new();
    for variant in variants {
        match resolve_variant(variant, theme)? {
            VariantApplication::Compound(children) => applications.extend(children),
            application => applications.push(application),
        }
    }
    conditions_only(&variant_paths(&applications)).then_some(applications)
}

#[cfg(test)]
mod tests {
    use stillwind_design_system::UtilityOrder;

    use super::super::stylesheet::authored_directives::render_authored_variant_path;
    use super::super::utilities::render::render_utility_rule_path;
    use super::super::utilities::resolve::CompiledUtility;
    use super::*;
    use crate::identifiers::IdentifierContext;

    /// Renders one authored variant path with the compiler's own helper names.
    fn render_variant_path(body: &str, applications: &[VariantApplication]) -> String {
        render_authored_variant_path(body, applications, &IdentifierContext::builtin())
            .expect("the built-in generator cannot reject a name")
    }

    #[test]
    fn sequential_wrappers_retain_every_condition_and_alternative() {
        let applications = [VariantApplication::Sequence(vec![
            VariantApplication::Container("panel (width > 1px)".into()),
            VariantApplication::Supports("display:grid".into()),
            VariantApplication::Compound(vec![
                VariantApplication::SelectorTemplate("&:hover".into()),
                VariantApplication::SelectorTemplate("&:focus".into()),
            ]),
        ])];
        let css = render_variant_path("color:red;", &applications);
        assert_eq!(
            css.matches("@container panel (width > 1px)").count(),
            2,
            "{css}"
        );
        assert_eq!(css.matches("@supports (display:grid)").count(), 2, "{css}");
        assert!(css.contains("&:hover") && css.contains("&:focus"), "{css}");
    }

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
            render_variant_path("color:red;", &applications),
            "@media print{&:hover{color:red;}}@media print{&:focus{color:red;}}"
        );
        let utility = CompiledUtility {
            required_properties: Vec::new(),
            variants: Vec::new(),
            variant_applications: Vec::new(),
            declaration: "color:red;".into(),
            selector: None,
            container: None,
            nested: Vec::new(),
            order: UtilityOrder::Authored(None),
            candidate: String::new(),
        };
        assert_eq!(
            render_utility_rule_path(
                ".x",
                &utility,
                "color:red;",
                None,
                &applications,
                &IdentifierContext::builtin(),
            )
            .expect("the built-in generator cannot reject a name"),
            "@media print{.x:hover{color:red;}}@media print{.x:focus{color:red;}}"
        );
        assert_eq!(
            render_variant_path("color:red;", &[VariantApplication::Compound(Vec::new())]),
            ""
        );
    }
}
