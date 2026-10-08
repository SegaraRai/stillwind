//! Classification of an authored variant body.
//!
//! A body must contain `@slot`, which marks where the utility's declarations go; a body
//! without a slot cannot produce a rule and is not a variant. The body is then
//! classified by its outermost construct: a media, container, or support condition, a
//! starting-style block, or a selector rule, in that priority order.
//!
//! A selector rule may also declare properties of its own (for example a `group`
//! variant that also sets a custom property). Those declarations are kept separate from
//! the utility's declarations so the emitter can place them correctly.

use cssparser::{Parser, Token};

use super::super::syntax::closing_brace;
use super::resolve::{VariantApplication, VariantDeclaration};

/// Classifies one variant body.
///
/// A body with several top-level blocks is a compound application and is resolved per
/// block, in authored order. [`None`] means the body has no slot, is empty, or is not a
/// construct the emitter can place.
pub(in crate::evaluation) fn resolve_variant_body(body: &str) -> Option<VariantApplication> {
    if body.is_empty() || !body.contains("@slot") {
        return None;
    }
    let blocks = split_variant_blocks(body)?;
    if blocks.len() > 1 {
        return blocks
            .into_iter()
            .map(resolve_variant_body)
            .collect::<Option<Vec<_>>>()
            .map(VariantApplication::Compound);
    }
    let body = body.trim();
    if body.starts_with('@') {
        let open = block_open(body)?;
        let close = closing_brace(body, open)?;
        let header = body[..open].trim();
        let (name, condition) = header
            .split_once(|c: char| c.is_whitespace() || c == '(')
            .map_or((header, ""), |(name, _)| {
                (name, header[name.len()..].trim())
            });
        let wrapper = match name {
            "@media" if !condition.is_empty() => VariantApplication::Media(condition.to_owned()),
            "@container" if !condition.is_empty() => {
                VariantApplication::Container(condition.to_owned())
            }
            "@supports" if !condition.is_empty() => VariantApplication::Supports(
                condition
                    .strip_prefix('(')
                    .and_then(|value| value.strip_suffix(')'))
                    .unwrap_or(condition)
                    .to_owned(),
            ),
            "@starting-style" if condition.is_empty() => VariantApplication::StartingStyle,
            _ => return None,
        };
        let inner = body[open + 1..close].trim();
        if inner == "@slot;" {
            return Some(wrapper);
        }
        let nested = resolve_variant_body(inner)?;
        if let VariantApplication::Media(condition) = wrapper {
            return match nested {
                VariantApplication::SelectorTemplate(selector) => {
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
    let rule = selector_rule_from_variant_body(body)?;
    if rule.declarations.is_empty() {
        Some(VariantApplication::SelectorTemplate(rule.selector))
    } else {
        Some(VariantApplication::SelectorRule {
            selector: rule.selector,
            declarations: rule.declarations,
        })
    }
}

/// Finds a rule body without treating quoted braces or function contents as blocks.
fn block_open(body: &str) -> Option<usize> {
    let mut parser = Parser::new(body);
    loop {
        let offset = parser.position().byte_index();
        if matches!(
            parser.next_including_whitespace_and_comments().ok()?,
            Token::CurlyBracketBlock
        ) {
            return Some(offset);
        }
    }
}

/// Splits a compound body into its top-level blocks, in authored order.
///
/// [`None`] means a block is unterminated; a body that is only whitespace has no
/// blocks.
fn split_variant_blocks(body: &str) -> Option<Vec<&str>> {
    let mut blocks = Vec::new();
    let mut cursor = 0;
    while cursor < body.len() {
        let start = body[cursor..]
            .char_indices()
            .find(|(_, character)| !character.is_whitespace())
            .map_or(body.len(), |(offset, _)| cursor + offset);
        if start == body.len() {
            break;
        }
        let open = block_open(&body[start..])? + start;
        let close = closing_brace(body, open)?;
        blocks.push(&body[start..=close]);
        cursor = close + 1;
    }
    (!blocks.is_empty()).then_some(blocks)
}

/// One selector rule inside a variant body: its selector and any declarations authored
/// beside the slot.
pub(in crate::evaluation) struct VariantSelectorRule {
    /// Selector template, containing `&` for the position of the utility.
    pub(in crate::evaluation) selector: String,
    /// Declarations stated by the rule itself, in authored order.
    pub(in crate::evaluation) declarations: Vec<VariantDeclaration>,
}

/// Reads a selector block from a variant body.
///
/// The selector must be the only construct in the body. Nested blocks,
/// an empty selector, and declarations without a property or value are all rejected, so
/// a malformed definition fails instead of emitting a selector the author did not
/// write.
fn selector_rule_from_variant_body(body: &str) -> Option<VariantSelectorRule> {
    let open = block_open(body)?;
    let close = closing_brace(body, open)?;
    if !body[close + 1..].trim().is_empty() {
        return None;
    }
    let selector = body[..open].trim();
    if selector.is_empty() || selector.starts_with('@') {
        return None;
    }

    let inner = &body[open + 1..close];
    let mut declarations = Vec::new();
    let mut start = 0;
    let mut depth = 0_u32;
    for (offset, character) in inner.char_indices() {
        match character {
            '(' | '[' => depth += 1,
            ')' | ']' => depth = depth.checked_sub(1)?,
            '{' | '}' => return None,
            ';' if depth == 0 => {
                let node = inner[start..offset].trim();
                start = offset + 1;
                if node.is_empty() || node == "@slot" {
                    continue;
                }
                let (property, value) = node.split_once(':')?;
                let property = property.trim();
                let value = value.trim();
                if property.is_empty() || value.is_empty() {
                    return None;
                }
                declarations.push(VariantDeclaration {
                    property: property.to_owned(),
                    value: value.to_owned(),
                });
            }
            _ => {}
        }
    }
    if !inner[start..].trim().is_empty() {
        return None;
    }
    Some(VariantSelectorRule {
        selector: selector.to_owned(),
        declarations,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

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
            (r#"[data-value="&{"] .\& /* & */ { @slot; }"#, false),
            (r#":is(&, [data-value="&"]) { @slot; }"#, true),
            ("::view-transition-old(*) { @slot; }", false),
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
            Some(VariantApplication::Supports("display:grid".into()))
        );
        assert_eq!(resolve_variant_body("&:hover {color:red;}"), None);
        assert!(split_variant_blocks("& { @slot; }").is_some());
        assert!(split_variant_blocks("& {").is_none());
        assert!(selector_rule_from_variant_body("& { @slot; } trailing").is_none());
    }
}
