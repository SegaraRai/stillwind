//! Lossless boundaries, priority edits, and `name: value` parsing for CSS declaration
//! fragments.

use std::ops::Range;

use cssparser::{Parser, Token};
use lean_string::{LeanStr, LeanString, ToLeanStr};

use crate::parser::{CssSyntaxError, consume_css_block, validate_css_syntax};

/// Why one statement of a declaration list is not a `name: value` declaration.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum DeclarationError {
    /// A component value contains malformed tokens or unclosed delimiters.
    #[error(transparent)]
    Syntax(#[from] CssSyntaxError),
    /// The statement does not start with an identifier.
    #[error("expected a declaration name")]
    MissingName,
    /// The name is not followed by `:`.
    #[error("expected `:` after a declaration name")]
    MissingColon,
    /// The value has no tokens.
    #[error("expected a declaration value")]
    EmptyValue,
    /// A value of an ordinary property contains a `{}` block.
    #[error("ordinary declaration values cannot contain `{{}}` blocks")]
    CurlyBlock,
}

/// One `name: value` declaration of a declaration list.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParsedDeclaration {
    /// Name with CSS escapes decoded.
    pub name: LeanStr,
    /// Authored value text without the semicolon, trimmed. Each run of whitespace and
    /// comments between top-level tokens becomes one space. Strings and the contents of
    /// parentheses, brackets, and functions keep their authored text, comments
    /// included.
    pub value: LeanStr,
}

/// Parses the statements of a declaration list, such as the body of an at-rule, in
/// authored order.
///
/// Malformed component values reject the whole list. Otherwise, each statement yields a
/// declaration or the reason that it is not one. After an error, parsing resumes after
/// the next top-level `;`. Empty statements yield nothing, and the last statement may
/// omit its semicolon.
pub fn parse_declaration_list(css: &str) -> Vec<Result<ParsedDeclaration, DeclarationError>> {
    if let Err(error) = validate_css_syntax(css) {
        return vec![Err(error.into())];
    }
    let mut parser = Parser::new(css);
    let mut statements = Vec::new();
    loop {
        let statement = match parser.next() {
            Err(_) => return statements,
            Ok(Token::Semicolon) => continue,
            Ok(Token::Ident(name)) => {
                let name = name.to_lean_str();
                declaration(&mut parser, name)
            }
            Ok(_) => {
                skip_statement(&mut parser);
                Err(DeclarationError::MissingName)
            }
        };
        statements.push(statement);
    }
}

/// Reads the `: value` rest of a declaration named `name` and consumes the statement
/// through its top-level `;`, also when the declaration is invalid.
fn declaration(
    parser: &mut Parser<'_>,
    name: LeanStr,
) -> Result<ParsedDeclaration, DeclarationError> {
    if parser.try_parse(Parser::expect_colon).is_err() {
        skip_statement(parser);
        return Err(DeclarationError::MissingColon);
    }
    let mut value = LeanString::new();
    loop {
        let start = parser.position();
        match parser.next_including_whitespace_and_comments() {
            Err(_) | Ok(Token::Semicolon) => {
                break;
            }
            Ok(Token::WhiteSpace(_) | Token::Comment(_)) => {
                if !value.is_empty() && !value.ends_with(' ') {
                    value.push(' ');
                }
                continue;
            }
            Ok(Token::CurlyBracketBlock) if !name.starts_with("--") => {
                skip_statement(parser);
                return Err(DeclarationError::CurlyBlock);
            }
            Ok(token) if is_block(token) => {
                let _ = consume_css_block(parser);
            }
            Ok(_) => {}
        }
        value.push_str(parser.slice_from(start));
    }
    let value = value.trim_end();
    if value.is_empty() {
        return Err(DeclarationError::EmptyValue);
    }
    Ok(ParsedDeclaration {
        name,
        value: value.to_lean_str(),
    })
}

/// Consumes tokens through the next top-level `;`, or to the end of input.
fn skip_statement(parser: &mut Parser<'_>) {
    while let Ok(token) = parser.next_including_whitespace_and_comments() {
        if matches!(token, Token::Semicolon) {
            return;
        }
    }
}

/// Returns declaration ranges excluding their top-level terminating semicolons.
/// Strings, comments, URLs and nested blocks do not introduce boundaries.
pub fn declaration_ranges(css: &str) -> Vec<Range<usize>> {
    let mut parser = Parser::new(css);
    let mut ranges = Vec::new();
    let mut start = 0;
    loop {
        let position = parser.position().byte_index();
        match parser.next_including_whitespace_and_comments() {
            Ok(Token::Semicolon) => {
                ranges.push(start..position);
                start = parser.position().byte_index();
            }
            Ok(token) if is_block(token) => {
                let _ = consume_css_block(&mut parser);
            }
            Ok(_) => {}
            Err(_) => {
                break;
            }
        }
    }
    if start < css.len() {
        ranges.push(start..css.len());
    }
    ranges
}

/// Adds priority to each nonempty declaration, preserving values and existing priority.
pub fn important_declarations(css: &str) -> LeanString {
    let mut output = LeanString::new();
    for range in declaration_ranges(css) {
        let declaration = &css[range];
        let mut parser = Parser::new(declaration);
        let mut previous_bang = false;
        let mut priority = false;
        let mut has_colon = false;
        let mut last_end = 0;
        while let Ok(token) = parser.next() {
            priority = previous_bang
                && matches!(token, Token::Ident(name) if name.eq_ignore_ascii_case("important"));
            previous_bang = matches!(token, Token::Delim('!'));
            has_colon |= matches!(token, Token::Colon);
            if is_block(token) {
                let _ = consume_css_block(&mut parser);
            }
            last_end = parser.position().byte_index();
        }
        if !has_colon {
            output.push_str(declaration);
            continue;
        }
        // Insert before trailing whitespace/comments, including a final line
        // comment-like spelling that CSS itself treats as ordinary tokens.
        output.push_str(&declaration[..last_end]);
        if !priority {
            output.push_str("!important");
        }
        output.push_str(&declaration[last_end..]);
        output.push(';');
    }
    output
}

fn is_block(token: &Token<'_>) -> bool {
    matches!(
        token,
        Token::Function(_)
            | Token::ParenthesisBlock
            | Token::SquareBracketBlock
            | Token::CurlyBracketBlock
    )
}

/// Finds declaration values using token boundaries, with an explicit stack for rules.
/// Custom-property brace values are opaque, as are all function and URL contents.
/// Returns nonoverlapping property/value byte ranges into `css`, sorted by source
/// position. Semicolons are excluded; surrounding declaration whitespace is retained.
pub fn declaration_values(css: &str) -> Vec<(Range<usize>, Range<usize>)> {
    let mut pending = Vec::new();
    pending.push(0..css.len());
    let mut values = Vec::new();
    while let Some(range) = pending.pop() {
        let fragment = &css[range.clone()];
        let mut parser = Parser::new(fragment);
        let mut start = 0;
        loop {
            let position = parser.position().byte_index();
            match parser.next_including_whitespace_and_comments() {
                Ok(Token::Semicolon) => {
                    add_declaration(fragment, range.start, start..position, &mut values);
                    start = parser.position().byte_index();
                }
                Ok(Token::CurlyBracketBlock) => {
                    let mut header = Parser::new(&fragment[start..position]);
                    let custom = matches!(header.next(), Ok(Token::Ident(name)) if name.starts_with("--"))
                        && matches!(header.next(), Ok(Token::Colon));
                    let inner = parser.position().byte_index();
                    let end = consume_css_block(&mut parser).map_or_else(
                        |_| parser.position().byte_index(),
                        |block| block.contents.end,
                    );
                    if !custom {
                        pending.push(range.start + inner..range.start + end);
                        start = parser.position().byte_index();
                    }
                }
                Ok(token) if is_block(token) => {
                    let _ = consume_css_block(&mut parser);
                }
                Ok(_) => {}
                Err(_) => {
                    add_declaration(fragment, range.start, start..fragment.len(), &mut values);
                    break;
                }
            }
        }
    }
    values.sort_by_key(|(property, _)| property.start);
    values
}

/// Appends property/value ranges when `css[range]` starts with an identifier and colon.
/// `range` must lie on UTF-8 boundaries within `css`; `offset` translates its ranges
/// into the enclosing stylesheet. Unrecognized fragments leave `values` unchanged.
/// Panics if `range` is out of bounds or splits a UTF-8 character.
fn add_declaration(
    css: &str,
    offset: usize,
    range: Range<usize>,
    values: &mut Vec<(Range<usize>, Range<usize>)>,
) {
    let mut parser = Parser::new(&css[range.clone()]);
    if !matches!(parser.next(), Ok(Token::Ident(_))) {
        return;
    }
    if !matches!(parser.next(), Ok(Token::Colon)) {
        return;
    }
    let property_end = parser.position().byte_index() - 1;
    values.push((
        offset + range.start..offset + range.start + property_end,
        offset + range.start + parser.position().byte_index()..offset + range.end,
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declaration_value_ranges_preserve_whitespace_and_opaque_components() {
        for (css, expected) in [
            ("a:1;b:2;", vec![("a", "1"), ("b", "2")]),
            ("a:1;b:2", vec![("a", "1"), ("b", "2")]),
            ("", vec![]),
            ("no-colon;", vec![]),
            ("  a : 1 ; ", vec![("  a ", " 1 ")]),
            ("--x:{a:1};y:2;", vec![("--x", "{a:1}"), ("y", "2")]),
            (
                "x:url(a;b);y:calc(1;2);",
                vec![("x", "url(a;b)"), ("y", "calc(1;2)")],
            ),
            ("x:fn({a:1});y:2;", vec![("x", "fn({a:1})"), ("y", "2")]),
            ("@media x{a:1;}b:2;", vec![("a", "1"), ("b", "2")]),
        ] {
            let values = declaration_values(css);
            assert_eq!(
                values
                    .iter()
                    .map(|(property, value)| (&css[property.clone()], &css[value.clone()]))
                    .collect::<Vec<_>>(),
                expected,
                "{css}"
            );
            let mut previous = 0;
            for (property, value) in values {
                assert!(property.start >= previous);
                assert!(property.end <= value.start);
                previous = value.end;
            }
        }
    }

    #[test]
    fn declaration_lists_keep_quoted_delimiters_and_omit_comments() {
        let css = r#"
            /* note: ignored; */
            label: "a;b: c";
            icon: url("data:image/svg+xml;utf8,<svg/>");
            size: 1 /* inline */ 2;
            ;
            list: fn(a; b), [c];
            last: x
        "#;
        let declaration = |name: &str, value: &str| {
            Ok(ParsedDeclaration {
                name: name.into(),
                value: value.into(),
            })
        };
        assert_eq!(
            parse_declaration_list(css),
            vec![
                declaration("label", "\"a;b: c\""),
                declaration("icon", "url(\"data:image/svg+xml;utf8,<svg/>\")"),
                declaration("size", "1 2"),
                declaration("list", "fn(a; b), [c]"),
                declaration("last", "x"),
            ]
        );
    }

    #[test]
    fn declaration_lists_report_invalid_statements_and_resume_after_them() {
        assert_eq!(
            parse_declaration_list("a; 'b': x; c: ; d: {x;} y; e: ok"),
            vec![
                Err(DeclarationError::MissingColon),
                Err(DeclarationError::MissingName),
                Err(DeclarationError::EmptyValue),
                Err(DeclarationError::CurlyBlock),
                Ok(ParsedDeclaration {
                    name: "e".into(),
                    value: "ok".into(),
                }),
            ]
        );
    }

    #[test]
    fn priority_preserves_nested_values_and_existing_markers() {
        assert_eq!(
            important_declarations(
                "content:\"a;b\";background:url(data:image/svg+xml;a);color:red ! IMPORTANT /* keep */;"
            ),
            "content:\"a;b\"!important;background:url(data:image/svg+xml;a)!important;color:red ! IMPORTANT /* keep */;"
        );
        assert_eq!(
            important_declarations("content:'!important';"),
            "content:'!important'!important;"
        );
        assert_eq!(important_declarations(""), "");
    }
}
