//! Media and container queries read as conjunctions of simple conditions.

use lean_string::{LeanStr, LeanString, ToLeanStr, ToLeanString};

use super::atoms::{Bound, Condition, MEDIA_TYPE, Scope, Test};

/// Splits a container prelude into its container name and query.
pub(super) fn container_query(prelude: &str) -> (LeanStr, &str) {
    let prelude = prelude.trim();
    match prelude.split_once(char::is_whitespace) {
        Some((name, query)) if !name.starts_with('(') && !name.eq_ignore_ascii_case("not") => {
            (name.to_lean_str(), query.trim())
        }
        _ => (LeanStr::new(), prelude),
    }
}

/// The conjunction of conditions in a media query or container query.
///
/// A query with a top-level `,` or `or`, or a negation of a conjunction, becomes one
/// opaque condition.
pub(super) fn query_conditions(query: &str, scope: &Scope, variant: usize) -> Vec<Condition> {
    let rule = match scope {
        Scope::Media => "@media",
        Scope::Container(_) => "@container",
    };
    let opaque = |negated: bool, text: &str| Condition {
        variant,
        test: Test::Opaque {
            key: format_args!("{rule} {} {}", scope_name(scope), normalize(text)).to_lean_str(),
            negated,
        },
        display: format_args!("{rule} {query}").to_lean_str(),
    };
    let words = top_level_words(query);
    if words
        .iter()
        .any(|word| *word == "," || word.eq_ignore_ascii_case("or"))
    {
        return vec![opaque(false, query)];
    }
    let (negated, query) = strip_not(query);
    let parts = split_top_level_and(query);
    if negated && parts.len() > 1 {
        return vec![opaque(true, query)];
    }
    parts
        .into_iter()
        .filter_map(|part| {
            let part = part
                .strip_prefix("only ")
                .map_or(part, str::trim_start)
                .trim();
            if part.eq_ignore_ascii_case("all") {
                return (negated).then(|| opaque(true, part));
            }
            let test =
                feature_test(part, scope, negated).unwrap_or_else(|| opaque(negated, part).test);
            let display = if negated {
                format_args!("{rule} not {part}").to_lean_string()
            } else {
                format_args!("{rule} {part}").to_lean_string()
            };
            Some(Condition {
                variant,
                test,
                display: display.into(),
            })
        })
        .collect()
}

fn scope_name(scope: &Scope) -> &str {
    match scope {
        Scope::Media => "",
        Scope::Container(name) => name,
    }
}

/// Interprets one parenthesized feature test or media type.
fn feature_test(part: &str, scope: &Scope, negated: bool) -> Option<Test> {
    let Some(inner) = part
        .strip_prefix('(')
        .and_then(|part| part.strip_suffix(')'))
    else {
        let is_type = part.chars().all(|c| c.is_ascii_alphabetic() || c == '-')
            && *scope == Scope::Media
            && !part.is_empty();
        return is_type.then(|| Test::Discrete {
            scope: scope.clone(),
            feature: MEDIA_TYPE.into(),
            value: part.to_ascii_lowercase().into(),
            negated,
        });
    };
    if inner.contains(['(', ')']) {
        return None;
    }
    if let Some((feature, value)) = inner.split_once(':') {
        let feature = feature.trim().to_ascii_lowercase();
        let value = value.trim();
        for (prefix, bound) in [("min-", Bound::AtLeast), ("max-", Bound::AtMost)] {
            if let Some(feature) = feature.strip_prefix(prefix) {
                let (number, unit) = dimension(value)?;
                return Some(range(scope, feature, bound, number, unit, negated));
            }
        }
        if value.is_empty() || !value.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            return None;
        }
        return Some(Test::Discrete {
            scope: scope.clone(),
            feature: feature.into(),
            value: value.to_ascii_lowercase().into(),
            negated,
        });
    }
    let operator = ["<=", ">=", "<", ">"]
        .into_iter()
        .find_map(|operator| inner.split_once(operator).map(|parts| (operator, parts)))?;
    let (operator, (left, right)) = operator;
    let (left, right) = (left.trim(), right.trim());
    if right.contains(['<', '>', '=']) {
        return None;
    }
    let bound = match operator {
        "<=" => Bound::AtMost,
        ">=" => Bound::AtLeast,
        "<" => Bound::Below,
        _ => Bound::Above,
    };
    if let Some((number, unit)) = dimension(right) {
        is_feature_name(left).then(|| {
            range(
                scope,
                &left.to_ascii_lowercase(),
                bound,
                number,
                unit,
                negated,
            )
        })
    } else {
        // `40rem <= width` compares in the opposite direction.
        let (number, unit) = dimension(left)?;
        let flipped = match bound {
            Bound::AtMost => Bound::AtLeast,
            Bound::AtLeast => Bound::AtMost,
            Bound::Below => Bound::Above,
            Bound::Above => Bound::Below,
        };
        is_feature_name(right).then(|| {
            range(
                scope,
                &right.to_ascii_lowercase(),
                flipped,
                number,
                unit,
                negated,
            )
        })
    }
}

fn range(
    scope: &Scope,
    feature: &str,
    bound: Bound,
    value: f64,
    unit: LeanStr,
    negated: bool,
) -> Test {
    Test::Range {
        scope: scope.clone(),
        feature: feature.into(),
        bound: if negated { bound.negated() } else { bound },
        value,
        unit,
    }
}

fn is_feature_name(text: &str) -> bool {
    !text.is_empty() && text.chars().all(|c| c.is_ascii_alphabetic() || c == '-')
}

/// Parses a number with an optional unit, such as `40rem`, into its value and its
/// lowercase unit.
fn dimension(text: &str) -> Option<(f64, LeanStr)> {
    let end = text
        .find(|c: char| !(c.is_ascii_digit() || matches!(c, '.' | '-' | '+')))
        .unwrap_or(text.len());
    let (number, unit) = text.split_at(end);
    let value = number
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())?;
    unit.chars()
        .all(|c| c.is_ascii_alphabetic() || c == '%')
        .then(|| (value, unit.to_ascii_lowercase().into()))
}

/// Removes a leading `not` keyword and reports whether it was present.
pub(super) fn strip_not(text: &str) -> (bool, &str) {
    let text = text.trim();
    match text.get(..4) {
        Some(prefix)
            if prefix.eq_ignore_ascii_case("not ") || prefix.eq_ignore_ascii_case("not(") =>
        {
            (true, text[3..].trim_start())
        }
        _ => (false, text),
    }
}

/// Splits a query at top-level `and` keywords.
fn split_top_level_and(query: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0_usize;
    let mut start = 0;
    let bytes = query.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'(' => depth += 1,
            b')' => depth = depth.saturating_sub(1),
            b' ' if depth == 0
                && query
                    .get(index + 1..index + 5)
                    .is_some_and(|word| word.eq_ignore_ascii_case("and ")) =>
            {
                parts.push(query[start..index].trim());
                index += 4;
                start = index + 1;
            }
            _ => {}
        }
        index += 1;
    }
    parts.push(query[start..].trim());
    parts.retain(|part| !part.is_empty());
    parts
}

/// The words and commas of `query` outside parentheses.
fn top_level_words(query: &str) -> Vec<&str> {
    let mut words = Vec::new();
    let mut depth = 0_usize;
    let mut start = None;
    for (index, character) in query.char_indices() {
        match character {
            '(' => {
                if depth == 0
                    && let Some(word) = start.take()
                {
                    words.push(&query[word..index]);
                }
                depth += 1;
            }
            ')' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                if let Some(word) = start.take() {
                    words.push(&query[word..index]);
                }
                words.push(",");
            }
            character if depth == 0 && character.is_whitespace() => {
                if let Some(word) = start.take() {
                    words.push(&query[word..index]);
                }
            }
            _ if depth == 0 => {
                start.get_or_insert(index);
            }
            _ => {}
        }
    }
    if let Some(word) = start {
        words.push(&query[word..]);
    }
    words
}

/// Collapses whitespace runs to one space and removes spaces next to `(`, `)`, and
/// `:`, so that spellings that differ only in spacing compare equal.
pub(super) fn normalize(text: &str) -> LeanString {
    let mut output = LeanString::new();
    let mut space = false;
    for character in text.trim().chars() {
        if character.is_whitespace() {
            space = true;
            continue;
        }
        if space && !output.ends_with(['(', ':']) && !matches!(character, ')' | ':') {
            output.push(' ');
        }
        space = false;
        output.push(character);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn media_queries_become_ranges_features_and_types() {
        let media = |query: &str| {
            vec![
                query_conditions(query, &Scope::Media, 0)
                    .into_iter()
                    .map(|condition| condition.test)
                    .collect::<Vec<_>>(),
            ]
        };
        assert_eq!(
            media("(width >= 40rem)"),
            [[range(
                &Scope::Media,
                "width",
                Bound::AtLeast,
                40.0,
                "rem".into(),
                false
            )]]
        );
        assert_eq!(
            media("not (width < 40rem)"),
            [[range(
                &Scope::Media,
                "width",
                Bound::AtLeast,
                40.0,
                "rem".into(),
                false
            )]]
        );
        assert_eq!(
            media("(min-width: 600px)"),
            [[range(
                &Scope::Media,
                "width",
                Bound::AtLeast,
                600.0,
                "px".into(),
                false
            )]]
        );
        assert_eq!(
            media("(40rem<=width)"),
            [[range(
                &Scope::Media,
                "width",
                Bound::AtLeast,
                40.0,
                "rem".into(),
                false
            )]]
        );
        assert_eq!(
            media("not (hover: hover)"),
            [[Test::Discrete {
                scope: Scope::Media,
                feature: "hover".into(),
                value: "hover".into(),
                negated: true,
            }]]
        );
        assert_eq!(media("only print and (orientation:portrait)")[0].len(), 2);
        assert!(matches!(
            &media("print, (hover: hover)")[0][..],
            [Test::Opaque { negated: false, .. }]
        ));
        assert!(matches!(
            &media("not (a) and (b)")[0][..],
            [Test::Opaque { negated: true, .. }]
        ));
    }
}
