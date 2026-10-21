//! Experimental runtime conditions shared by value and variant resolution.
//!
//! [`RuntimeCondition`] parses presence/comparison syntax and produces its canonical
//! spelling; [`runtime_if_arguments`] splits value-function arguments. These helpers do
//! not inspect property registrations or decide the output strategy. Presence tests use
//! whitespace/initial boolean values; comparisons are lowered by the compiler to native
//! CSS style queries.

use std::fmt;

use cssparser::{ParseError, Parser, Token};
use lean_string::LeanString;

use stillwind_css::{ValueDomain, find_css_functions, matching_css_delimiter};

/// A parsed custom-property presence test or `--name:value` comparison.
///
/// Any number of leading `!` is accepted, and an odd count negates the test. Whitespace
/// and comments may separate them. A comparison with an empty value is a presence test,
/// because the boolean contract allows only whitespace and `initial` values.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeCondition<'a> {
    /// Whether an odd number of `!` precedes the test.
    pub negated: bool,
    /// Custom-property name, including its `--` prefix.
    pub name: &'a str,
    /// Trimmed comparison value, or `None` for a presence test. Never empty.
    pub value: Option<&'a str>,
}

impl<'a> RuntimeCondition<'a> {
    /// Parses `condition`.
    ///
    /// Returns `None` unless the text is a custom-property name with at least one
    /// character after `--`, optionally followed by `:` and a comparison value.
    /// Comparison values must not contain top-level commas, semicolons, exclamation
    /// marks, bad string/URL tokens, or unmatched closing delimiters. Surrounding
    /// whitespace and comments are accepted. Nested blocks are skipped by the
    /// tokenizer; their contents and omitted closing delimiters are not fully
    /// validated here. Browser evaluation determines comparison semantics and the
    /// effect of property registration.
    #[must_use]
    pub fn parse(condition: &'a str) -> Option<Self> {
        let mut parser = Parser::new(condition);
        let mut negated = false;
        let name = loop {
            let token = parser.next().ok()?.clone();
            let end = parser.position().byte_index();
            match token {
                Token::Delim('!') => {
                    negated = !negated;
                }
                // Escaped names are rejected, so the source slice equals the name.
                Token::Ident(name)
                    if name.starts_with("--")
                        && name.len() > 2
                        && condition[..end].ends_with(name.as_ref()) =>
                {
                    break &condition[end - name.len()..end];
                }
                _ => {
                    return None;
                }
            }
        };
        if parser.is_exhausted() {
            return Some(Self {
                negated,
                name,
                value: None,
            });
        }
        if !matches!(parser.next(), Ok(Token::Colon)) {
            return None;
        }
        let start = parser.position().byte_index();
        while let Ok(token) = parser.next() {
            if matches!(
                token,
                Token::Comma
                    | Token::Semicolon
                    | Token::Delim('!')
                    | Token::BadString(_)
                    | Token::BadUrl(_)
                    | Token::CloseParenthesis
                    | Token::CloseSquareBracket
                    | Token::CloseCurlyBracket
            ) {
                return None;
            }
        }
        let value = condition[start..].trim();
        Some(Self {
            negated,
            name,
            value: (!value.is_empty()).then_some(value),
        })
    }
}

/// Writes the canonical spelling: at most one `!`, then the name, then `:value` for a
/// comparison.
impl fmt::Display for RuntimeCondition<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.negated {
            formatter.write_str("!")?;
        }
        formatter.write_str(self.name)?;
        if let Some(value) = self.value {
            write!(formatter, ":{value}")?;
        }
        Ok(())
    }
}

/// Returns whether `condition` parses as a [`RuntimeCondition`].
#[must_use]
pub fn is_runtime_condition(condition: &str) -> bool {
    RuntimeCondition::parse(condition).is_some()
}

/// Splits `--if` arguments into a canonical condition and two branches.
///
/// Returns `None` unless there are exactly three top-level comma-separated arguments
/// and the condition parses as a [`RuntimeCondition`]. Branch whitespace is trimmed
/// and one complete pair of branch braces is removed; empty branches are allowed.
/// Strings, comments, and nested blocks do not introduce argument separators.
/// Branch contents are not validated as CSS values; whitespace inside removed braces
/// is retained in the returned slices.
#[must_use]
pub fn runtime_if_arguments(arguments: &str) -> Option<(RuntimeCondition<'_>, &str, &str)> {
    let (condition, branches) = split_arguments(arguments);
    let (yes, no) = split_arguments(branches?);
    let no = no?;
    let (_, extra) = split_arguments(no);
    if extra.is_some() {
        return None;
    }
    Some((
        RuntimeCondition::parse(condition)?,
        unbrace(yes.trim()),
        unbrace(no.trim()),
    ))
}

/// Checks both branches of a whole conditional against the utility's value domain.
/// Nested functions retain the domain's normal deferred-variable validation behavior.
/// Whole-conditional recursion beyond 32 levels rejects; syntax errors return false.
pub(crate) fn accepts_runtime_value(domain: ValueDomain, value: &str) -> bool {
    accepts(domain, value, 0)
}

/// Validates conditional arguments and delegates branch/value checks to `domain`.
/// `depth` counts enclosing whole-value conditionals; values above 32 return false.
/// Embedded conditionals are replaced with deferred variables for the domain check.
fn accepts(domain: ValueDomain, value: &str, depth: usize) -> bool {
    if depth > 32 {
        return false;
    }
    let functions = find_css_functions(value, &["--if"], 0);
    let Some(first) = functions.first() else {
        return domain.accepts(value);
    };
    if first.start == 0 && first.end == Some(value.len()) {
        return runtime_if_arguments(&value[first.arguments.clone()]).is_some_and(
            |(_, yes, no)| accepts(domain, yes, depth + 1) && accepts(domain, no, depth + 1),
        );
    }
    let mut normalized = LeanString::new();
    let mut cursor = 0;
    for function in functions {
        if runtime_if_arguments(&value[function.arguments.clone()]).is_none()
            || function.end.is_none()
        {
            return false;
        }
        if function.start < cursor {
            continue;
        }
        normalized.push_str(&value[cursor..function.start]);
        normalized.push_str("var(--sw-if-type)");
        cursor = function.end.unwrap_or(value.len());
    }
    normalized.push_str(&value[cursor..]);
    domain.accepts(&normalized)
}

/// Splits at the first top-level comma without trimming either returned slice.
/// Returns `(value, None)` when no separator is found; comments, strings, and nested
/// blocks keep their commas opaque. Tokenization errors end the search.
fn split_arguments(value: &str) -> (&str, Option<&str>) {
    let mut parser = Parser::new(value);
    loop {
        let start = parser.position().byte_index();
        match parser.next_including_whitespace_and_comments() {
            Ok(Token::Comma) => {
                return (
                    &value[..start],
                    Some(&value[parser.position().byte_index()..]),
                );
            }
            Ok(
                Token::Function(_)
                | Token::CurlyBracketBlock
                | Token::ParenthesisBlock
                | Token::SquareBracketBlock,
            ) => {
                let _ = parser.parse_nested_block(|nested| {
                    while nested.next_including_whitespace_and_comments().is_ok() {}
                    Ok::<_, ParseError<()>>(())
                });
            }
            Ok(_) => {}
            Err(_) => {
                return (value, None);
            }
        }
    }
}

/// Removes one brace pair only when it encloses the whole input; otherwise returns
/// the input unchanged. Neither branch trims whitespace or validates its contents.
fn unbrace(value: &str) -> &str {
    if value.starts_with('{') && matching_css_delimiter(value, 0, '{', '}') == Ok(value.len() - 1) {
        &value[1..value.len() - 1]
    } else {
        value
    }
}

#[cfg(test)]
mod tests {
    use lean_string::ToLeanString;

    use super::*;

    #[test]
    fn runtime_conditions_accept_any_negation_count_and_canonicalize_its_parity() {
        for (condition, canonical) in [
            ("--c", "--c"),
            ("!--c", "!--c"),
            (" ! --c ", "!--c"),
            ("!/* comment */--c", "!--c"),
            ("!!--c", "--c"),
            ("! !!--c", "!--c"),
            ("--c:", "--c"),
            ("!--c: ", "!--c"),
            ("--mode: compact ", "--mode:compact"),
        ] {
            let parsed = RuntimeCondition::parse(condition);
            assert_eq!(
                parsed
                    .map(|condition| condition.to_lean_string())
                    .as_deref(),
                Some(canonical),
                "{condition}"
            );
        }
        for condition in ["!--", "!", "--c extra", "var(--c)", "--c:a!", r"--\63 "] {
            assert_eq!(RuntimeCondition::parse(condition), None, "{condition}");
        }
        let (condition, yes, no) = runtime_if_arguments(" !!!--c, {a,b}, f(x,y) ").unwrap();
        assert_eq!(
            (condition.to_lean_string().as_str(), yes, no),
            ("!--c", "a,b", "f(x,y)")
        );
    }
}
