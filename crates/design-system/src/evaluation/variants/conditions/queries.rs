//! Media and container queries read as conjunctions of simple conditions.

use cssparser::{Parser, Token};
use lean_string::{LeanStr, ToLeanStr, ToLeanString};

use stillwind_css::{CssCase, consume_css_block, normalize_css_fragment, split_top_level};

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
            key: format_args!(
                "{rule} {} {}",
                scope_name(scope),
                normalize_css_fragment(text, &[':'], CssCase::Keep)
            )
            .to_lean_str(),
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
    split_top_level(
        query,
        |token| matches!(token, Token::Ident(name) if name.eq_ignore_ascii_case("and")),
    )
    .into_iter()
    .map(str::trim)
    .filter(|part| !part.is_empty())
    .collect()
}

/// The words and commas of `query` outside parentheses.
fn top_level_words(query: &str) -> Vec<&str> {
    let mut parser = Parser::new(query);
    let mut words = Vec::new();
    loop {
        let start = parser.position().byte_index();
        let Ok(token) = parser.next_including_whitespace_and_comments().cloned() else {
            break;
        };
        let end = parser.position().byte_index();
        match &token {
            Token::Ident(_) => {
                words.push(&query[start..end]);
            }
            Token::Function(_) => {
                words.push(&query[start..end - 1]);
            }
            Token::Comma => {
                words.push(",");
            }
            _ => {}
        }
        if matches!(
            token,
            Token::Function(_)
                | Token::ParenthesisBlock
                | Token::SquareBracketBlock
                | Token::CurlyBracketBlock
        ) {
            let _ = consume_css_block(&mut parser);
        }
    }
    words
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
