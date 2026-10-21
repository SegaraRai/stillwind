//! Classification of an authored variant body.
//!
//! A body must contain `@slot`, which marks where the utility's declarations go; a body
//! without a slot cannot produce a rule and is not a variant. The body is then
//! classified by its outermost construct: a media, container, or support condition, a
//! starting-style block, or a selector rule, in that priority order.
//!
//! A selector rule is a nested style rule, so a selector without `&` is relative to the
//! styled element as in CSS Nesting: `:hover { @slot; }` styles hovered descendants in
//! every context. A definition with [`VariantScope::Document`] keeps its selectors as
//! written, and an `&`-less selector of such a definition resolves to
//! [`VariantApplication::BareSelector`], which selects the element of a variant stack
//! that has not selected one yet.
//!
//! A selector rule may also declare properties of its own (for example a `group`
//! variant that also sets a custom property). Those declarations are kept separate from
//! the utility's declarations so the emitter can place them correctly.

use cssparser::Parser;
use lean_string::{LeanStr, ToLeanStr};

use stillwind_css::{
    CssNode, CssNodeKind, absolutize_nested_selector, parse_structure, selector_is_valid,
    selector_uses_subject, validate_css_syntax,
};

use super::application::{VariantApplication, VariantDeclaration};
use crate::VariantScope;

/// Classifies one variant body whose `&`-less selectors are nested style rules.
///
/// See [`resolve_scoped_variant_body`].
pub(in crate::evaluation) fn resolve_variant_body(body: &str) -> Option<VariantApplication> {
    resolve_scoped_variant_body(body, VariantScope::Nested)
}

/// Classifies one variant body of a definition with the declared `scope`.
///
/// A body with several top-level blocks is a compound application and is resolved per
/// block, in authored order. [`None`] means the body has no slot, is empty, or is not a
/// construct the emitter can place.
///
/// A selector rule without declarations resolves to [`VariantApplication::BareSelector`]
/// when `scope` is [`VariantScope::Document`] and no branch of its selector contains
/// `&`. A selector rule inside `@media` resolves to [`VariantApplication::SelectorMedia`]
/// with the same selector.
pub(in crate::evaluation) fn resolve_scoped_variant_body(
    body: &str,
    scope: VariantScope,
) -> Option<VariantApplication> {
    if body.is_empty() || !has_slot(&parse_structure(body)) {
        return None;
    }
    let blocks = split_variant_blocks(body)?;
    if blocks.len() > 1 {
        return blocks
            .into_iter()
            .map(|block| resolve_scoped_variant_body(block, scope))
            .collect::<Option<Vec<_>>>()
            .map(VariantApplication::Compound);
    }
    let body = body.trim();
    let nodes = parse_structure(body);
    if let [
        CssNode {
            kind:
                CssNodeKind::AtRule {
                    name,
                    prelude,
                    block: Some(block),
                },
            ..
        },
    ] = nodes.as_slice()
    {
        let condition = body[prelude.clone()].trim();
        let wrapper = match name.as_str() {
            "media" if !condition.is_empty() => VariantApplication::Media(condition.to_lean_str()),
            "container" if !condition.is_empty() => {
                VariantApplication::Container(condition.to_lean_str())
            }
            "supports" if !condition.is_empty() => {
                VariantApplication::Supports(condition.to_lean_str())
            }
            "starting-style" if condition.is_empty() => VariantApplication::StartingStyle,
            _ => return None,
        };
        let inner = body[block.contents.clone()].trim();
        if is_slot(&parse_structure(inner)) {
            return Some(wrapper);
        }
        let nested = resolve_scoped_variant_body(inner, scope)?;
        if let VariantApplication::Media(condition) = wrapper {
            return match nested {
                VariantApplication::SelectorTemplate(selector)
                | VariantApplication::BareSelector { selector, .. } => {
                    Some(VariantApplication::SelectorMedia {
                        selector,
                        condition,
                    })
                }
                VariantApplication::SelectorRule {
                    selector,
                    declarations,
                } => Some(VariantApplication::SelectorMediaRule {
                    selector,
                    declarations,
                    condition,
                }),
                other => Some(VariantApplication::Sequence(vec![
                    VariantApplication::Media(condition),
                    other,
                ])),
            };
        }
        return Some(VariantApplication::Sequence(vec![wrapper, nested]));
    }
    let rule = selector_rule_from_variant_body(body, scope)?;
    if rule.declarations.is_empty() {
        Some(match rule.top_level {
            Some(top_level) => VariantApplication::BareSelector {
                selector: rule.selector,
                top_level,
            },
            None => VariantApplication::SelectorTemplate(rule.selector),
        })
    } else {
        Some(VariantApplication::SelectorRule {
            selector: rule.selector,
            declarations: rule.declarations,
        })
    }
}

fn has_slot(nodes: &[CssNode]) -> bool {
    nodes.iter().any(|node| {
        matches!(&node.kind, CssNodeKind::AtRule { name, prelude, block: None } if name == "slot" && prelude.is_empty())
            || has_slot(node.children())
    })
}

fn is_slot(nodes: &[CssNode]) -> bool {
    matches!(nodes, [CssNode { kind: CssNodeKind::AtRule { name, prelude, block: None }, .. }] if name == "slot" && prelude.is_empty())
}

/// Splits complete top-level rule blocks, keeping their authored ranges.
fn split_variant_blocks(body: &str) -> Option<Vec<&str>> {
    validate_css_syntax(body).ok()?;
    let mut blocks = Vec::new();
    let mut cursor = 0;
    for node in parse_structure(body) {
        if !Parser::new(&body[cursor..node.range.start]).is_exhausted() {
            return None;
        }
        let (CssNodeKind::StyleRule { block, .. }
        | CssNodeKind::AtRule {
            block: Some(block), ..
        }) = &node.kind
        else {
            return None;
        };
        block.close?;
        cursor = node.range.end;
        blocks.push(&body[node.range]);
    }
    (!blocks.is_empty() && Parser::new(&body[cursor..]).is_exhausted()).then_some(blocks)
}

/// One selector rule inside a variant body: its selector and any declarations authored
/// beside the slot.
pub(in crate::evaluation) struct VariantSelectorRule {
    /// Selector template for use relative to `&`. Each branch contains `&`, except in a
    /// definition with [`VariantScope::Document`], which keeps its selector as written.
    pub(in crate::evaluation) selector: LeanStr,
    /// The selector as written, when the definition has [`VariantScope::Document`], the
    /// selector is valid without `&`, and no branch contains `&`.
    pub(in crate::evaluation) top_level: Option<LeanStr>,
    /// Declarations stated by the rule itself, in authored order.
    pub(in crate::evaluation) declarations: Vec<VariantDeclaration>,
}

/// Reads a selector block from a variant body of a definition with the declared
/// `scope`.
///
/// The returned selector has `&` inserted as [`absolutize_nested_selector`] describes,
/// unless `scope` is [`VariantScope::Document`].
/// The selector must be the only construct in the body. Nested blocks, an empty or
/// invalid selector, and declarations without a property or value are all rejected, so
/// a malformed definition fails instead of emitting a selector the author did not
/// write.
fn selector_rule_from_variant_body(body: &str, scope: VariantScope) -> Option<VariantSelectorRule> {
    validate_css_syntax(body).ok()?;
    let nodes = parse_structure(body);
    let [
        CssNode {
            range,
            kind: CssNodeKind::StyleRule { selector, block },
        },
    ] = nodes.as_slice()
    else {
        return None;
    };
    if !Parser::new(&body[range.end..]).is_exhausted() {
        return None;
    }
    block.close?;
    let selector = &body[selector.clone()];
    if selector.is_empty() || !has_slot(&block.children) {
        return None;
    }
    let mut declarations = Vec::new();
    for child in &block.children {
        match &child.kind {
            CssNodeKind::Declaration { property, value } => {
                let property = body[property.clone()].trim();
                let value = body[value.clone()].trim();
                if property.is_empty() || value.is_empty() {
                    return None;
                }
                declarations.push(VariantDeclaration {
                    property: property.to_lean_str(),
                    value: value.to_lean_str(),
                });
            }
            CssNodeKind::AtRule {
                name,
                prelude,
                block: None,
            } if name == "slot" && prelude.is_empty() => {}
            _ => return None,
        }
    }
    let top_level = (scope == VariantScope::Document
        && !selector_uses_subject(selector)
        && selector_is_valid(selector))
    .then(|| selector.to_lean_str());
    let selector = match scope {
        VariantScope::Nested => absolutize_nested_selector(selector).to_lean_str(),
        VariantScope::Document => selector.to_lean_str(),
    };
    selector_is_valid(&selector).then_some(VariantSelectorRule {
        selector,
        top_level,
        declarations,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_blocks_adjacent_to_parentheses_and_brackets() {
        for (body, expected) in [
            (
                "@media (hover:hover){&:hover{@slot;}}",
                VariantApplication::SelectorMedia {
                    selector: "&:hover".into(),
                    condition: "(hover:hover)".into(),
                },
            ),
            (
                "@supports (display:grid){@slot;}",
                VariantApplication::Supports("(display:grid)".into()),
            ),
            (
                "@container panel (width>1px){@slot;}",
                VariantApplication::Container("panel (width>1px)".into()),
            ),
            (
                "&:is(:hover,:focus){@slot;}",
                VariantApplication::SelectorTemplate("&:is(:hover,:focus)".into()),
            ),
            (
                r#"&[data-value="{"]{@slot;}"#,
                VariantApplication::SelectorTemplate(r#"&[data-value="{"]"#.into()),
            ),
        ] {
            assert_eq!(resolve_variant_body(body), Some(expected), "{body}");
        }
    }

    #[test]
    fn subject_dependency_comes_from_selector_tokens_inside_retained_wrappers() {
        for (body, expected) in [
            (
                "@media print { @supports (display:grid) { &:hover { @slot; } } }",
                true,
            ),
            (
                "@container panel (width > 1px) { @media print { @slot; } }",
                false,
            ),
            (r#":is(&, [data-value="&"]) { @slot; }"#, true),
        ] {
            let application = resolve_variant_body(body).unwrap();
            assert_eq!(
                application.uses_subject(),
                expected,
                "{body}: {application:?}"
            );
        }
    }

    #[test]
    fn recognizes_selector_media_supports_and_rejects_missing_slots() {
        assert_eq!(
            resolve_variant_body("&:hover { @slot; }"),
            Some(VariantApplication::SelectorTemplate("&:hover".into()))
        );
        assert_eq!(
            resolve_variant_body("@media (hover:hover) { @slot; }"),
            Some(VariantApplication::Media("(hover:hover)".into()))
        );
        assert_eq!(
            resolve_variant_body("@supports (display:grid) { @slot; }"),
            Some(VariantApplication::Supports("(display:grid)".into()))
        );
        assert_eq!(resolve_variant_body("&:hover {color:red;}"), None);
        for body in ["color: { @slot; }", "&: { @slot; }", ".a, { @slot; }"] {
            assert_eq!(resolve_variant_body(body), None, "{body}");
        }
        for (body, selector) in [
            (":hover { @slot; }", "& :hover"),
            ("> .item { @slot; }", "& > .item"),
            (":hover, .open { @slot; }", "& :hover, & .open"),
            ("&:hover, .open { @slot; }", "&:hover, & .open"),
            (
                r#"[data-value="&{"] .\& { @slot; }"#,
                r#"& [data-value="&{"] .\&"#,
            ),
            (
                "::view-transition-old(*) { @slot; }",
                "& ::view-transition-old(*)",
            ),
        ] {
            assert_eq!(
                resolve_variant_body(body),
                Some(VariantApplication::SelectorTemplate(selector.into())),
                "{body}"
            );
        }
        assert_eq!(
            resolve_variant_body("@media print { .open { @slot; } }"),
            Some(VariantApplication::SelectorMedia {
                selector: "& .open".into(),
                condition: "print".into(),
            })
        );
        assert_eq!(
            resolve_scoped_variant_body(":host { @slot; }", VariantScope::Document),
            Some(VariantApplication::BareSelector {
                selector: ":host".into(),
                top_level: ":host".into(),
            })
        );
        assert_eq!(
            resolve_scoped_variant_body(
                "@media print { :host { @slot; } }",
                VariantScope::Document
            ),
            Some(VariantApplication::SelectorMedia {
                selector: ":host".into(),
                condition: "print".into(),
            })
        );
        assert!(split_variant_blocks("& { @slot; }").is_some());
        assert!(split_variant_blocks("& {").is_none());
        assert!(
            selector_rule_from_variant_body("& { @slot; } trailing", VariantScope::Nested)
                .is_none()
        );
    }
}
