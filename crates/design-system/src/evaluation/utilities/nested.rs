//! Structural splitting of a utility body.
//!
//! A single final selector block can be evaluated separately from the utility's direct
//! declarations. Bodies with several selectors keep their CSS structure and source
//! order. Conditional rules stay in those bodies so that splitting them cannot move
//! declarations across selectors.

use cssparser::Parser;
use lean_string::{LeanStr, LeanString, ToLeanStr, ToLeanString};

use stillwind_css::{
    CssNode, CssNodeKind, absolutize_nested_selector, parse_structure, validate_css_syntax,
};

/// A conditional declaration authored inside a CSS `@utility` body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NestedUtilityRule {
    /// True for the modern-color branch the compatibility path wraps in `@supports`;
    /// ordering metadata leaves these branches out.
    pub compatibility: bool,
    /// Which conditional wrapper the emitter must use.
    pub at_rule: NestedUtilityAtRule,
    /// Condition text without the at-rule keyword or braces.
    pub condition: LeanStr,
    /// Authored condition enclosing a generated compatibility query. The tuple
    /// contains its at-rule kind and condition text in the same format as `condition`.
    pub parent: Option<(NestedUtilityAtRule, LeanStr)>,
    /// Declarations to place inside the conditional.
    pub declaration: LeanStr,
    /// Whether the conditional was authored inside the utility's selector block (or as
    /// a CSS `@supports` declaration, which is nested by CSS). This preserves placement
    /// from the CSS source instead of classifying declarations by Tailwind property
    /// names.
    pub inside: bool,
}

/// The CSS conditional wrapper associated with a nested utility declaration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NestedUtilityAtRule {
    /// A media query, whose condition is emitted as authored.
    Media,
    /// A support query, whose author-supplied outer parentheses are removed and
    /// restored by the emitter.
    Supports,
}

/// A conditional declaration group split out of a utility body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::evaluation) struct ParsedNestedUtilityRule {
    /// Conditional at-rule represented by this parsed block.
    pub(in crate::evaluation) at_rule: NestedUtilityAtRule,
    /// Trimmed query text without the at-rule keyword or braces; supports removes its
    /// outer parentheses.
    pub(in crate::evaluation) condition: LeanStr,
    /// Trimmed content inside the conditional braces, before placeholder evaluation.
    pub(in crate::evaluation) body: LeanStr,
}

/// Splits a body into its direct declarations and its conditional rules.
///
/// Conditions are returned in authored order and their inner bodies are trimmed. A
/// conditional whose block is unterminated fails the split instead of leaving its text
/// in the direct declarations. A body containing selectors keeps its conditions in
/// place, so declarations after a nested rule retain their cascade order.
pub(in crate::evaluation) fn split_nested_conditions(
    body: &str,
) -> Option<(LeanString, Vec<ParsedNestedUtilityRule>)> {
    validate_css_syntax(body).ok()?;
    let nodes = parse_structure(body);
    if nodes.iter().any(contains_selector) {
        return Some((body.trim().to_lean_string(), Vec::new()));
    }
    let mut direct = LeanString::with_capacity(body.len());
    let mut nested = Vec::new();
    let mut cursor = 0;
    for node in nodes {
        let CssNodeKind::AtRule {
            name,
            prelude,
            block: Some(block),
        } = &node.kind
        else {
            continue;
        };
        let at_rule = match name.as_str() {
            "media" => NestedUtilityAtRule::Media,
            "supports" => NestedUtilityAtRule::Supports,
            _ => continue,
        };
        block.close?;
        direct.push_str(&body[cursor..node.range.start]);
        let condition = body[prelude.clone()].trim();
        let condition = match at_rule {
            NestedUtilityAtRule::Media => condition,
            NestedUtilityAtRule::Supports => condition.strip_prefix('(')?.strip_suffix(')')?.trim(),
        };
        nested.push(ParsedNestedUtilityRule {
            at_rule,
            condition: condition.to_lean_str(),
            body: body[block.contents.clone()].trim().to_lean_str(),
        });
        cursor = node.range.end;
    }
    direct.push_str(&body[cursor..]);
    Some((direct.trim().to_lean_string(), nested))
}

/// Returns whether `node` is or contains a style rule outside a keyframe definition.
/// Descendants of keyframe definitions are excluded from the search.
pub(super) fn contains_selector(node: &CssNode) -> bool {
    match &node.kind {
        CssNodeKind::StyleRule { .. } => true,
        CssNodeKind::AtRule { name, .. } if name.ends_with("keyframes") => false,
        _ => node.children().iter().any(contains_selector),
    }
}

/// Splits `body` into a normalized selector, the selected rule's contents, and the
/// optional CSS preceding that rule.
///
/// A single final style rule receives an explicit nesting selector; its contents and
/// preceding CSS are trimmed slices of `body`. When there is no direct style rule,
/// there are several, or another CSS node follows one, the result is
/// `(None, body, None)` so evaluation retains the whole body.
///
/// Returns `None` when CSS syntax validation fails or a selected rule has an unclosed
/// block. Trailing text outside CSS nodes also fails when a direct style rule exists.
pub(in crate::evaluation) fn split_utility_selector(
    body: &str,
) -> Option<(Option<LeanString>, &str, Option<&str>)> {
    validate_css_syntax(body).ok()?;
    let nodes = parse_structure(body);
    let mut rules = nodes
        .iter()
        .filter(|node| matches!(node.kind, CssNodeKind::StyleRule { .. }));
    let Some(node) = rules.next() else {
        return Some((None, body, None));
    };
    let last = nodes.last()?;
    if !Parser::new(&body[last.range.end..]).is_exhausted() {
        return None;
    }
    if rules.next().is_some() || nodes.last() != Some(node) {
        return Some((None, body, None));
    }
    let CssNodeKind::StyleRule { selector, block } = &node.kind else {
        return None;
    };
    block.close?;
    let selector_text = absolutize_nested_selector(&body[selector.clone()]);
    let before = body[..node.range.start].trim();
    Some((
        Some(selector_text),
        body[block.contents.clone()].trim(),
        (!before.is_empty()).then_some(before),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_conditions_and_rejects_incomplete_selector_bodies() {
        let (base, nested) =
            split_nested_conditions("color:red;@media (width > 1px) { color:blue; }").unwrap();
        assert_eq!(base, "color:red;");
        assert_eq!(
            nested,
            vec![ParsedNestedUtilityRule {
                at_rule: NestedUtilityAtRule::Media,
                condition: "(width > 1px)".into(),
                body: "color:blue;".into()
            }]
        );
        assert_eq!(
            split_utility_selector("x:1; &:hover {color:red;}"),
            Some((Some("&:hover".into()), "color:red;", Some("x:1;")))
        );
        assert_eq!(split_utility_selector("& {x:1;} trailing"), None);
        assert_eq!(
            split_utility_selector("[data-icon] {opacity:.7;}"),
            Some((Some("& [data-icon]".into()), "opacity:.7;", None))
        );
        assert!(split_nested_conditions("@media x {").is_none());
    }
}
