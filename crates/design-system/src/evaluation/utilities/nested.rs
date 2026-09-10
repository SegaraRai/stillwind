use super::super::syntax::closing_brace;

/// A conditional declaration authored inside a CSS `@utility` body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NestedUtilityRule {
    pub at_rule: NestedUtilityAtRule,
    pub condition: String,
    pub declaration: String,
    /// Whether the conditional was authored inside the utility's selector
    /// block (or as a CSS `@supports` declaration, which is nested by CSS).
    /// This preserves placement from the CSS source instead of classifying
    /// declarations by Tailwind property names.
    pub inside: bool,
}

/// The CSS conditional wrapper associated with a nested utility declaration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NestedUtilityAtRule {
    Media,
    Supports,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ParsedNestedUtilityRule {
    pub(crate) at_rule: NestedUtilityAtRule,
    pub(crate) condition: String,
    pub(crate) body: String,
}

pub(crate) fn split_nested_conditions(
    body: &str,
) -> Option<(String, Vec<ParsedNestedUtilityRule>)> {
    let mut direct = String::with_capacity(body.len());
    let mut nested = Vec::new();
    let mut cursor = 0;
    while let Some((start, at_rule, keyword)) = next_nested_condition(body, cursor) {
        direct.push_str(&body[cursor..start]);
        let open = body[start..].find('{')? + start;
        let close = closing_brace(body, open)?;
        let condition = body[start + keyword.len()..open].trim();
        let condition = match at_rule {
            NestedUtilityAtRule::Media => condition.to_owned(),
            NestedUtilityAtRule::Supports => condition
                .strip_prefix('(')
                .and_then(|value| value.strip_suffix(')'))?
                .trim()
                .to_owned(),
        };
        nested.push(ParsedNestedUtilityRule {
            at_rule,
            condition,
            body: body[open + 1..close].trim().to_owned(),
        });
        cursor = close + 1;
    }
    direct.push_str(&body[cursor..]);
    Some((direct.trim().to_owned(), nested))
}

pub(crate) fn next_nested_condition(
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

pub(crate) fn split_utility_selector(body: &str) -> Option<(Option<String>, &str, Option<&str>)> {
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
            Some(selector.to_owned()),
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
