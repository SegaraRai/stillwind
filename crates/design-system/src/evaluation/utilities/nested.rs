//! Structural splitting of a utility body.
//!
//! A definition body can be a plain declaration list, a selector block, or a mix:
//! declarations before the selector apply to the utility itself and declarations inside
//! the block apply to the selected element. Conditional declarations (`@media`,
//! `@supports`) may appear at either level.
//!
//! Placement is taken from the authored CSS rather than inferred from property names,
//! which is why a conditional keeps the level it was written at. A body that cannot be
//! split cleanly is rejected, so a malformed definition cannot silently emit
//! declarations at the wrong nesting level.

use lean_string::{LeanStr, LeanString, ToLeanStr, ToLeanString};

use super::super::syntax::closing_brace;

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
/// in the direct declarations.
pub(in crate::evaluation) fn split_nested_conditions(
    body: &str,
) -> Option<(LeanString, Vec<ParsedNestedUtilityRule>)> {
    let mut direct = LeanString::with_capacity(body.len());
    let mut nested = Vec::new();
    let mut cursor = 0;
    while let Some((start, at_rule, keyword)) = next_nested_condition(body, cursor) {
        direct.push_str(&body[cursor..start]);
        let open = body[start..].find('{')? + start;
        let close = closing_brace(body, open)?;
        let condition = body[start + keyword.len()..open].trim();
        let condition = match at_rule {
            NestedUtilityAtRule::Media => condition.to_lean_string(),
            NestedUtilityAtRule::Supports => condition
                .strip_prefix('(')
                .and_then(|value| value.strip_suffix(')'))?
                .trim()
                .to_lean_string(),
        };
        nested.push(ParsedNestedUtilityRule {
            at_rule,
            condition: (condition).into(),
            body: body[open + 1..close].trim().to_lean_str(),
        });
        cursor = close + 1;
    }
    direct.push_str(&body[cursor..]);
    Some((direct.trim().to_lean_string(), nested))
}

/// Finds the earliest nested conditional at or after `cursor`.
///
/// Returns the offset, the at-rule kind, and the keyword, so the caller can slice the
/// condition without re-scanning. `@supports` wins a tie because a body cannot open
/// both at the same byte.
///
/// # Panics
///
/// Panics when `cursor` is past the end of `body` or does not lie on a UTF-8 character
/// boundary, because the search begins by slicing at `cursor`. Callers must pass `0` or
/// an offset just past a previously returned block in this same string; an offset from
/// another buffer, or advanced by bytes instead of scalars, panics.
fn next_nested_condition(
    body: &str,
    cursor: usize,
) -> Option<(usize, NestedUtilityAtRule, &'static str)> {
    let supports = body[cursor..]
        .find("@supports")
        .map(|offset| (cursor + offset, NestedUtilityAtRule::Supports, "@supports"));
    let media = body[cursor..]
        .find("@media")
        .map(|offset| (cursor + offset, NestedUtilityAtRule::Media, "@media"));
    match (supports, media) {
        (Some(supports), Some(media)) => Some(if supports.0 < media.0 {
            supports
        } else {
            media
        }),
        (Some(condition), None) | (None, Some(condition)) => Some(condition),
        (None, None) => None,
    }
}

/// Splits a body into an optional selector, its inner declarations, and the
/// declarations authored before the selector.
///
/// A body without `&` has no selector, and the whole body is returned as direct
/// declarations. A body whose selector block is not a complete selector - anything
/// after the closing brace, or a prelude without `&` - is rejected, because accepting
/// it would change which element the declarations apply to.
pub(in crate::evaluation) fn split_utility_selector(
    body: &str,
) -> Option<(Option<LeanString>, &str, Option<&str>)> {
    if !body.contains('&') {
        return Some((None, body, None));
    }

    let mut selector_start = 0;
    let mut open = None;
    let mut parenthesis_depth = 0_u32;
    for (index, character) in body.char_indices() {
        match character {
            '(' | '[' => parenthesis_depth += 1,
            ')' | ']' => parenthesis_depth = parenthesis_depth.checked_sub(1)?,
            ';' if parenthesis_depth == 0 => selector_start = index + 1,
            '{' if parenthesis_depth == 0 => {
                let prelude = body[selector_start..index].trim();
                if prelude.contains('&') {
                    open = Some(index);
                    break;
                }
            }
            _ => {}
        }
    }
    let open = open?;
    let close = closing_brace(body, open)?;
    if !body[close + 1..].trim().is_empty() {
        return None;
    }
    let selector = body[selector_start..open].trim();
    (!selector.is_empty()).then(|| {
        (
            Some(selector.to_lean_string()),
            body[open + 1..close].trim(),
            (!body[..selector_start].trim().is_empty()).then_some(body[..selector_start].trim()),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_conditions_and_rejects_trailing_selector_rules() {
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
        assert!(split_nested_conditions("@media x {").is_none());
    }
}
