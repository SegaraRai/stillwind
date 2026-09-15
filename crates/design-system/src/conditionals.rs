//! Experimental runtime conditions shared by value and variant resolution.
//!
//! [`is_runtime_condition`] checks presence/comparison syntax; [`runtime_if_arguments`]
//! splits value-function arguments, and [`runtime_condition_value`] extracts a validated
//! comparison's right-hand side. These helpers do not inspect property registrations
//! or decide the output strategy. Presence tests use whitespace/initial boolean values;
//! comparisons are lowered by the compiler to native CSS style queries.

use cssparser::{ParseError, Parser, Token};

use stillwind_css::{ValueDomain, find_css_functions, matching_css_delimiter};

/// Validates a custom-property presence test or `--name:value` comparison.
/// Either form accepts at most one leading `!`. Comparison values must be nonempty
/// CSS tokens without top-level commas, semicolons, exclamation marks, bad string/URL
/// tokens, or unmatched closing delimiters. Surrounding whitespace and comments are
/// accepted. Nested blocks are skipped by the tokenizer; their contents and omitted
/// closing delimiters are not fully validated here. Browser evaluation determines
/// comparison semantics and the effect of property registration.
pub fn is_runtime_condition(condition: &str) -> bool {
    let condition = condition.trim();
    let mut parser = Parser::new(condition.strip_prefix('!').unwrap_or(condition));
    if !matches!(parser.next(), Ok(Token::Ident(name)) if name.starts_with("--") && name.len() > 2)
    {
        return false;
    }
    if parser.is_exhausted() {
        return true;
    }
    if !matches!(parser.next(), Ok(Token::Colon)) {
        return false;
    }
    let mut nonempty = false;
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
            return false;
        }
        nonempty = true;
    }
    nonempty
}

/// Returns the comparison value of a validated runtime condition, or `None` for
/// a presence test. The result excludes the name, colon, and surrounding whitespace.
/// A leading negation is ignored; callers must validate with [`is_runtime_condition`].
pub fn runtime_condition_value(condition: &str) -> Option<&str> {
    let condition = condition
        .trim()
        .strip_prefix('!')
        .unwrap_or(condition.trim());
    let mut parser = Parser::new(condition);
    parser.next().ok()?;
    if !matches!(parser.next(), Ok(Token::Colon)) {
        return None;
    }
    Some(condition[parser.position().byte_index()..].trim())
}

/// Splits `--if` arguments into a validated, optionally negated condition and two branches.
/// Returns `None` unless there are exactly three top-level comma-separated arguments
/// and the condition satisfies [`is_runtime_condition`]. Whitespace is trimmed and
/// one complete pair of branch braces is removed; empty branches are allowed.
/// Strings, comments, and nested blocks do not introduce argument separators.
/// Branch contents are not validated as CSS values; whitespace inside removed braces
/// is retained in the returned slices.
pub fn runtime_if_arguments(arguments: &str) -> Option<(&str, &str, &str)> {
    let (condition, branches) = split_arguments(arguments);
    let (yes, no) = split_arguments(branches?);
    let no = no?;
    let (_, extra) = split_arguments(no);
    (is_runtime_condition(condition.trim()) && extra.is_none()).then_some((
        condition.trim(),
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
    let mut normalized = String::new();
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
            Err(_) => return (value, None),
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
    use super::*;

    #[test]
    fn runtime_conditions_accept_at_most_one_negation() {
        for condition in ["--c", "!--c", " ! --c ", "!/* comment */--c"] {
            assert!(is_runtime_condition(condition), "{condition}");
        }
        for condition in ["!!--c", "! !--c", "!--", "!", "--c extra", "var(--c)"] {
            assert!(!is_runtime_condition(condition), "{condition}");
        }
        assert_eq!(
            runtime_if_arguments(" !--c, {a,b}, f(x,y) "),
            Some(("!--c", "a,b", "f(x,y)"))
        );
        assert!(runtime_if_arguments("!!--c,a,b").is_none());
    }
}
