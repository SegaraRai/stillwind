//! Conjunction and disjunction of media query lists.
//!
//! A media query list matches when any query matches, and a variant stack requires
//! every condition. [`MediaQueryList::and`] therefore distributes the conjunction over
//! the queries of both lists. A query keeps its media type separate from its condition,
//! because the grammar allows a type only at the start of a query: `print` and
//! `(width >= 40rem)` combine into `print and (width >= 40rem)`.
//!
//! A query negated with the `not` modifier, such as `not print`, negates its type and
//! condition together. Its conjunction with another query has no direct spelling, so it
//! expands into queries without the modifier: `not T and C` matches where the type is
//! not `T`, or where it is `T` and `C` fails. Only `print` and `screen` are matching
//! media types in Media Queries 4, so the complement of one is the other. Other type
//! names match nothing.

use cssparser::{Parser, Token};
use lean_string::{LeanString, ToLeanString};

use stillwind_css::split_top_level;

/// A media type after normalization.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MediaType {
    /// `all`, or no type.
    All,
    /// `print`.
    Print,
    /// `screen`.
    Screen,
    /// A type that matches no device, such as a deprecated or unknown type.
    Never,
}

impl MediaType {
    fn parse(name: &str) -> Self {
        if name.eq_ignore_ascii_case("all") {
            Self::All
        } else if name.eq_ignore_ascii_case("print") {
            Self::Print
        } else if name.eq_ignore_ascii_case("screen") {
            Self::Screen
        } else {
            Self::Never
        }
    }

    /// The types that match where this type does not.
    const fn complement(self) -> Option<Self> {
        match self {
            Self::All => None,
            Self::Print => Some(Self::Screen),
            Self::Screen => Some(Self::Print),
            Self::Never => Some(Self::All),
        }
    }

    /// The type that both types require, or `None` when no device has both.
    fn and(self, other: Self) -> Option<Self> {
        match (self, other) {
            (Self::Never, _) | (_, Self::Never) => None,
            (Self::All, other) | (other, Self::All) => Some(other),
            (left, right) => (left == right).then_some(left),
        }
    }

    const fn spelling(self) -> Option<&'static str> {
        match self {
            Self::All | Self::Never => None,
            Self::Print => Some("print"),
            Self::Screen => Some("screen"),
        }
    }
}

/// One media query without the `not` or `only` modifier.
#[derive(Clone, Debug, Eq, PartialEq)]
struct Query {
    media_type: MediaType,
    /// Conditions that all must hold, each a `<media-in-parens>` or a negation of one,
    /// in authored order without duplicates.
    conditions: Vec<LeanString>,
}

impl Query {
    fn and(&self, other: &Self) -> Option<Self> {
        let mut conditions = self.conditions.clone();
        for condition in &other.conditions {
            if !conditions.contains(condition) {
                conditions.push(condition.clone());
            }
        }
        Some(Self {
            media_type: self.media_type.and(other.media_type)?,
            conditions,
        })
    }

    fn render(&self, output: &mut LeanString) {
        let mut terms = self.conditions.iter().map(|condition| {
            if self.media_type.spelling().is_some() || self.conditions.len() > 1 {
                wrap_negation(condition)
            } else {
                condition.clone()
            }
        });
        match self.media_type.spelling() {
            Some(name) => {
                output.push_str(name);
            }
            None => {
                if let Some(first) = terms.next() {
                    output.push_str(&first);
                } else {
                    output.push_str("all");
                }
            }
        }
        for term in terms {
            output.push_str(" and ");
            output.push_str(&term);
        }
    }
}

/// A media query list, normalized so that it can be combined.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct MediaQueryList {
    /// Queries of which any may match. An empty list matches nothing.
    queries: Vec<Query>,
}

impl MediaQueryList {
    /// Parses a media query list from a resolved variant.
    ///
    /// `text` must be a valid `<media-query-list>`, as variant resolution produces.
    /// Returns `None` for text this parser cannot split into queries.
    pub(super) fn parse(text: &str) -> Option<Self> {
        let mut queries = Vec::new();
        for query in split_top_level(text, |token| matches!(token, Token::Comma)) {
            queries.extend(parse_query(query.trim())?);
        }
        Some(Self { queries })
    }

    /// The list that matches where both lists match.
    #[must_use]
    pub(super) fn and(&self, other: &Self) -> Self {
        let mut queries: Vec<Query> = Vec::new();
        for left in &self.queries {
            for right in &other.queries {
                if let Some(query) = left.and(right)
                    && !queries.contains(&query)
                {
                    queries.push(query);
                }
            }
        }
        Self { queries }
    }

    /// Serializes the list, or returns `None` when it has no queries and matches no
    /// environment.
    pub(super) fn render(&self) -> Option<LeanString> {
        if self.queries.is_empty() {
            return None;
        }
        let mut output = LeanString::new();
        for (index, query) in self.queries.iter().enumerate() {
            if index > 0 {
                output.push_str(", ");
            }
            query.render(&mut output);
        }
        Some(output)
    }
}

/// Parses one media query into the queries without the `not` modifier that match the
/// same environments.
fn parse_query(query: &str) -> Option<Vec<Query>> {
    let mut parser = Parser::new(query);
    let first = match parser.next() {
        Ok(Token::Ident(name)) => Some(name.clone()),
        Ok(_) => None,
        Err(_) => return None,
    };
    let (negated, media_type, rest) = match first {
        Some(name) if name.eq_ignore_ascii_case("not") || name.eq_ignore_ascii_case("only") => {
            match parser.next() {
                Ok(Token::Ident(media_type)) => (
                    name.eq_ignore_ascii_case("not"),
                    Some(MediaType::parse(media_type)),
                    &query[parser.position().byte_index()..],
                ),
                // `not (condition)` negates a condition, not a query.
                _ => (false, None, query),
            }
        }
        Some(name) => (
            false,
            Some(MediaType::parse(&name)),
            &query[parser.position().byte_index()..],
        ),
        None => (false, None, query),
    };
    let condition = match media_type {
        Some(_) => {
            let rest = rest.trim();
            if rest.is_empty() {
                None
            } else {
                let mut parser = Parser::new(rest);
                match parser.next() {
                    Ok(Token::Ident(and)) if and.eq_ignore_ascii_case("and") => {
                        Some(rest[parser.position().byte_index()..].trim())
                    }
                    _ => return None,
                }
            }
        }
        None => Some(rest.trim()),
    };
    let media_type = media_type.unwrap_or(MediaType::All);
    let conditions = condition.map_or_else(Vec::new, conjuncts);
    if !negated {
        return Some(vec![Query {
            media_type,
            conditions,
        }]);
    }
    let mut queries = Vec::new();
    if let Some(complement) = media_type.complement() {
        queries.push(Query {
            media_type: complement,
            conditions: Vec::new(),
        });
    }
    if !conditions.is_empty() && media_type != MediaType::Never {
        queries.push(Query {
            media_type,
            conditions: vec![negate(&join_conjuncts(&conditions))],
        });
    }
    Some(queries)
}

/// Splits a condition into the operands of its top-level `and`.
///
/// A condition with a top-level `or` or `not` is one operand, parenthesized when it
/// combines several terms.
fn conjuncts(condition: &str) -> Vec<LeanString> {
    let mut keywords = Vec::new();
    let mut parser = Parser::new(condition);
    while let Ok(token) = parser.next() {
        if let Token::Ident(name) = token {
            keywords.push(name.to_ascii_lowercase());
        }
    }
    if keywords.iter().all(|keyword| keyword == "and") {
        return split_top_level(
            condition,
            |token| matches!(token, Token::Ident(name) if name.eq_ignore_ascii_case("and")),
        )
        .into_iter()
        .map(|term| term.trim().to_lean_string())
        .filter(|term| !term.is_empty())
        .collect();
    }
    if keywords.first().is_some_and(|keyword| keyword == "not") && keywords.len() == 1 {
        return vec![condition.to_lean_string()];
    }
    vec![format_args!("({condition})").to_lean_string()]
}

fn join_conjuncts(conditions: &[LeanString]) -> LeanString {
    conditions
        .iter()
        .map(wrap_negation)
        .collect::<Vec<_>>()
        .join(" and ")
        .to_lean_string()
}

/// Negates a condition: `(x)` becomes `not (x)`, `not (x)` becomes `(x)`, and a
/// combination of terms is parenthesized first.
fn negate(condition: &str) -> LeanString {
    let mut parser = Parser::new(condition);
    let (negated, block) = match parser.next() {
        Ok(Token::Ident(name)) => (name.eq_ignore_ascii_case("not"), false),
        Ok(Token::ParenthesisBlock | Token::Function(_)) => (false, true),
        _ => (false, false),
    };
    if negated {
        condition[parser.position().byte_index()..]
            .trim()
            .to_lean_string()
    } else if block && parser.is_exhausted() {
        format_args!("not {condition}").to_lean_string()
    } else {
        format_args!("not ({condition})").to_lean_string()
    }
}

/// Parenthesizes a negated term, which cannot be an operand of `and` without them.
fn wrap_negation(condition: &LeanString) -> LeanString {
    let mut parser = Parser::new(condition);
    match parser.next() {
        Ok(Token::Ident(name)) if name.eq_ignore_ascii_case("not") => {
            format_args!("({condition})").to_lean_string()
        }
        _ => condition.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn and(left: &str, right: &str) -> LeanString {
        MediaQueryList::parse(left)
            .unwrap()
            .and(&MediaQueryList::parse(right).unwrap())
            .render()
            .unwrap_or_else(|| "not all".into())
    }

    #[test]
    fn conditions_and_types_combine_into_one_query() {
        for (left, right, expected) in [
            (
                "(width >= 40rem)",
                "(prefers-reduced-motion: reduce)",
                "(width >= 40rem) and (prefers-reduced-motion: reduce)",
            ),
            ("print", "(width >= 40rem)", "print and (width >= 40rem)"),
            (
                "(width >= 40rem)",
                "screen and (color)",
                "screen and (width >= 40rem) and (color)",
            ),
            ("(a: 1) and (b: 2)", "(a: 1)", "(a: 1) and (b: 2)"),
            (
                "not (hover: hover)",
                "(a: 1)",
                "(not (hover: hover)) and (a: 1)",
            ),
            (
                "(a: 1) or (b: 2)",
                "(c: 3)",
                "((a: 1) or (b: 2)) and (c: 3)",
            ),
            ("all", "(a: 1)", "(a: 1)"),
        ] {
            assert_eq!(and(left, right), expected, "{left} AND {right}");
        }
    }

    #[test]
    fn lists_distribute_and_conflicting_types_match_nothing() {
        assert_eq!(
            and("(a: 1), (b: 2)", "(c: 3)"),
            "(a: 1) and (c: 3), (b: 2) and (c: 3)"
        );
        assert_eq!(and("print", "screen"), "not all");
        assert_eq!(and("tv", "(a: 1)"), "not all");
    }

    #[test]
    fn negated_queries_expand_before_conjunction() {
        assert_eq!(
            and("not print", "(width >= 40rem)"),
            "screen and (width >= 40rem)"
        );
        assert_eq!(
            and("not print and (color)", "(a: 1)"),
            "screen and (a: 1), print and (not (color)) and (a: 1)"
        );
        assert_eq!(
            and("not all and (color)", "(a: 1)"),
            "(not (color)) and (a: 1)"
        );
        assert_eq!(and("not tv", "(a: 1)"), "(a: 1)");
    }
}
