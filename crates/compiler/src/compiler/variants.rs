//! Non-compound steps shared by authored and generated CSS renderers.
//!
//! A [`VariantStep`] is one effect a variant has on a rule, expressed without reference to
//! which renderer is applying it: a selector suffix or template, the declarations a variant
//! contributes, and the at-rules that contain everything else. Compound applications are
//! flattened here into alternative paths, so the authored `@variant` path and the generated
//! utility path expand the same inputs through the same code and cannot drift apart.

use stillwind_design_system::{VariantApplication, VariantDeclaration};

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

#[cfg(test)]
mod tests {
    use super::super::stylesheet::authored_directives::render_authored_variant_path;
    use super::super::utilities::render::render_utility_rule_path;
    use super::super::utilities::resolve::CompiledUtility;
    use super::*;

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
        let css = render_authored_variant_path("color:red;", &applications);
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
            render_authored_variant_path("color:red;", &applications),
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
