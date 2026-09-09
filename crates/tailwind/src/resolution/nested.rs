use super::{CustomUtility, NestedUtilityAtRule, ParsedNestedUtilityRule};

pub(super) fn split_nested_conditions(
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
        nested.push((at_rule, condition, body[open + 1..close].trim().to_owned()));
        cursor = close + 1;
    }
    direct.push_str(&body[cursor..]);
    Some((direct.trim().to_owned(), nested))
}

pub(super) fn next_nested_condition(
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

pub(super) fn split_utility_selector(body: &str) -> Option<(Option<String>, &str, Option<&str>)> {
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

pub(super) fn closing_brace(value: &str, open: usize) -> Option<usize> {
    let mut depth = 0_u32;
    for (offset, character) in value[open..].char_indices() {
        match character {
            '{' => depth += 1,
            '}' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(open + offset);
                }
            }
            _ => {}
        }
    }
    None
}

pub(super) fn matching_utilities<'a>(
    utilities: &'a [CustomUtility],
    name: &'a str,
) -> impl Iterator<Item = &'a CustomUtility> {
    utilities.iter().rev().filter(move |utility| {
        utility.name == name
            || utility
                .name
                .strip_suffix('*')
                .is_some_and(|prefix| name.starts_with(prefix))
    })
}

pub(super) fn closing_parenthesis(value: &str, open: usize) -> Option<usize> {
    let mut depth = 0_u32;
    for (offset, character) in value[open..].char_indices() {
        match character {
            '(' => depth += 1,
            ')' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(open + offset);
                }
            }
            _ => {}
        }
    }
    None
}
